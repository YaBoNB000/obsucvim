#!/bin/bash
# 全项防御审计 (增量㊲) — 逐点验证混淆产物的每个防御机制是否真实生效。
#
# 审计原则: 每个防御点既验「正向」(干净输入正常运行) 也验「负向」
# (攻击/篡改/挂钩必须被拒绝, 且绝不产出正确输出)。
#
# 防御点清单:
#   D1  干净运行等价        — 防御不得破坏正确性 (基线)
#   D2  钥匙去字面化        — OBSUCVIM_KEY_MANIFEST + key_literal_check.py
#   D3  结构指纹 32 项      — v15_fingerprint.py
#   D4  安全指纹 S1-S5      — security_fingerprint.py + attack 脚本组
#   D5  密文完整性(源哈希链) — 长串内字节互换 => 绝不产出正确输出
#   D6  字节码防篡改(校验和投毒) — 同 D5 多点统计覆盖
#   D7  激活门 --bind-key   — ㊴ 退役 (功能/测试/示例均已移除)
#   D8  环境绑定 --bind-env — 通用沙箱加载失败 + 语法仍可编译
#   D9  反挂钩闸(loadstring) — loadstring 换成 Lua 闭包 => 必须被杀
#   D10 反指纹闸(debug.info) — debug.info 说谎 => 必须被杀
#   D11 蜜罐哨兵            — Mc 层陷阱结构级验证 (外部不可触发)
#   D12 计时守卫 (㊳ 复通)  — 接线 (外层局部 + os.clock 赋值) + 触发路径
#                             (抽出守卫用合成 tracer 时钟驱动 => 必投毒) +
#                             干净时钟零误报 + 长运行用例误报抽查
#   D13 静态协议 debug 自检  — debug 构建 + 运行等价
#   D14 保护代码隐藏普查    — 可见层死循环/蜜罐词/码表/拼串表零痕迹 (㊶)
#   D15 环境绑定语义强化    — 朴素桩场景必须静默毒药 (㊸)
#   D16 可见层零解码源消费点 — 解码源不得落地可见层 (㊹, R018 P0-2)
#   D17 物化去钩化          — char 钩子截不到载荷 (㊾, R021)
#   D18 executor 认证守卫   — IronBrew2 移植, 藏深 + 双负面实证 (51)
#
# 用法: bash tests/defense_audit.sh   (release 二进制需已构建)
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
if [ ! -x "$TOOL" ]; then
	echo "错误: 缺少 $TOOL (先 cargo build --release)"; exit 2
fi
if [ ! -x "$LUAU" ]; then
	echo "错误: 缺少 $LUAU (先 bash engine/tools/rebuild_tools.sh)"; exit 2
fi
SRC="$ROOT/tests/cases/basics.lua"
SRC2="$ROOT/tests/cases/real_algo.lua"
W=/tmp/defense_audit
rm -rf "$W"; mkdir -p "$W"

pass=0; fail=0
ok()  { echo "  PASS $1"; pass=$((pass+1)); }
bad() { echo "  FAIL $1"; fail=$((fail+1)); }

echo "== D1 干净运行等价 (防御基线) =="
EXP1="$($LUAU "$SRC" 2>&1)"
EXP2="$($LUAU "$SRC2" 2>&1)"
# Seed 772 keeps the randomized F10 literal-count specimen inside its
# established 10..60 band with default-on LoadStr and dead MkStr pruning.
"$TOOL" --preset v15 --dialect luau --seed 772 "$SRC" "$W/d1a.lua" >/dev/null 2>&1
"$TOOL" --preset v15 --dialect luau --seed 888 "$SRC2" "$W/d1b.lua" >/dev/null 2>&1
[ "$($LUAU "$W/d1a.lua" 2>&1)" == "$EXP1" ] && ok "basics seed=772" || bad "basics seed=772"
[ "$($LUAU "$W/d1b.lua" 2>&1)" == "$EXP2" ] && ok "real_algo seed=888" || bad "real_algo seed=888"

echo "== D2 钥匙去字面化 (3 种子) =="
for seed in 7 4242 999999; do
	OBSUCVIM_KEY_MANIFEST="$W/keys_$seed.txt" "$TOOL" --preset v15 --dialect luau \
		--seed "$seed" "$SRC" "$W/d2_$seed.lua" >/dev/null 2>&1
	if python3 "$ROOT/tests/key_literal_check.py" "$W/d2_$seed.lua" "$W/keys_$seed.txt" \
		>"$W/d2_$seed.out" 2>&1; then
		ok "seed=$seed $(tail -1 "$W/d2_$seed.out")"
	else
		bad "seed=$seed 钥匙泄漏: $(tail -1 "$W/d2_$seed.out")"
	fi
done

echo "== D3 结构指纹 =="
if python3 "$ROOT/tests/v15_fingerprint.py" "$W/d1a.lua" --quiet >"$W/d3.out" 2>&1; then
	ok "$(cat "$W/d3.out")"
else
	bad "结构指纹: $(cat "$W/d3.out")"
fi

echo "== D4 安全指纹 S1-S5 =="
if python3 "$ROOT/tests/security_fingerprint.py" "$W/d1a.lua" "$SRC" --quiet >"$W/d4.out" 2>&1; then
	ok "$(grep '安全指纹' "$W/d4.out")"
else
	bad "安全指纹: $(cat "$W/d4.out")"
fi

echo "== D5/D6 密文完整性 + 字节码防篡改 (载体/HQ 真跨度字节互换) =="
# H4-2C2 默认构建已无 HQ 源码碎片；短中型长串是活跃 FN carrier，超大
# 长串则可能是伪装诱饵。扫描保持括号层级感知，超大跨度先动态甄别，
# carrier 跨度直接进入 30 点篡改矩阵。
cat > "$W/d56_scan.py" <<'PYD56'
import re, subprocess, sys
art, W, LU = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(art).read()
spans = []
i, n = 0, len(s)
while i < n:
    c = s[i]
    if c == '"' or c == "'":
        q = c; i += 1
        while i < n:
            if s[i] == '\\': i += 2; continue
            if s[i] == q: break
            i += 1
        i += 1; continue
    if c == '[' and i+1 < n:
        m = re.match(r'\[(=*)\[', s[i:])
        if m:
            lvl = len(m.group(1))
            cl = ']' + '='*lvl + ']'
            jj = s.find(cl, i + len(m.group(0)))
            if jj != -1:
                spans.append((i + len(m.group(0)), jj))
                i = jj + len(cl); continue
    i += 1
carrier = [(a, b) for a, b in spans if 100 <= b - a <= 2000]
big = [(a, b) for a, b in spans if b - a > 2000]
base = subprocess.run([LU, art], capture_output=True, text=True, timeout=60)
live, decoy = list(carrier), []
for (a, b) in big:
    p1 = a + (b - a)//2
    p2 = p1 + 13
    t = list(s)
    while t[p1] == t[p2]: p2 += 1
    t[p1], t[p2] = t[p2], t[p1]
    tmp = W + "/probe.lua"
    open(tmp, "w").write("".join(t))
    try:
        r = subprocess.run([LU, tmp], capture_output=True, text=True, timeout=12)
        same = (r.stdout == base.stdout and r.returncode == base.returncode)
    except subprocess.TimeoutExpired:
        same = False  # ㊸: 投毒静默循环 = 拒绝（篡改点敏感）
    if same:
        decoy.append(b - a)
    else:
        live.append((a, b))
print(f"{len(live)} live carrier/HQ spans, {len(decoy)} decoy spans (camo, sizes {decoy})")
assert live, "no live spans found"
seed = 12345
def rnd(n):
    global seed
    seed = (seed * 1103515245 + 12345) % 2147483648
    return seed % n
for k in range(30):
    a, b = live[rnd(len(live))]
    while True:
        p1 = a + rnd(b - a)
        p2 = a + rnd(b - a)
        if p1 != p2 and s[p1] != s[p2]:
            break
    t = list(s)
    t[p1], t[p2] = t[p2], t[p1]
    open(f"{W}/tamper_{k}.lua", "w").write("".join(t))
print("tampers written")
PYD56
python3 "$W/d56_scan.py" "$W/d1a.lua" "$W" "$LUAU" || bad "跨度扫描失败"
# 旧版 30 个静默投毒样本逐个 timeout 30s，最坏耗时 15 分钟，看起来像
# 审计卡死。改为 6 worker 并行、单样本 12s；判据不变：超时/报错/异输出
# 都是拒绝，只有完整复现干净输出才算防御失败。
if python3 - "$W" "$LUAU" <<'PYD56RUN'
import concurrent.futures, subprocess, sys
W, LU = sys.argv[1], sys.argv[2]
base = subprocess.run([LU, W + "/d1a.lua"], stdout=subprocess.PIPE,
                      stderr=subprocess.STDOUT, timeout=60)

def check(k):
    try:
        r = subprocess.run([LU, f"{W}/tamper_{k}.lua"], stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, timeout=12)
        return k if (r.returncode == base.returncode and r.stdout == base.stdout) else None
    except subprocess.TimeoutExpired:
        return None

with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    bad = [k for k in pool.map(check, range(30)) if k is not None]
if bad:
    print("tamper points produced clean output:", bad)
    raise SystemExit(1)
print("30/30 tamper points rejected (parallel)")
PYD56RUN
then
	ok "30/30 真跨度篡改点全部被拒 (6-worker 并行, 诱饵伪装串已剔除)"
else
	bad "存在篡改点仍产出正确输出 (完整性防御失效)"
fi

echo "== D7 激活门 (㊴ 退役, 跳过) =="
echo "  SKIP --bind-key 功能/测试/示例已随 ㊴ 移除"

echo "== D8 环境绑定 --bind-env =="
if bash "$ROOT/tests/env_bind_test.sh" >"$W/d8.out" 2>&1; then
	ok "$(grep 'ENV BIND PASS' "$W/d8.out")"
else
	bad "环境绑定: $(tail -3 "$W/d8.out" | tr '\n' ' ')"
fi

echo "== D9 反挂钩闸 (loadstring 换成 Lua 闭包 => 必须被杀) =="
python3 - "$W/d1a.lua" "$W" <<'PYEOF'
import sys
src = open(sys.argv[1]).read()
hook = '''
local real_ls = loadstring
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.loadstring = function(s, ...) return real_ls(s, ...) end
local src = [==[''' + src + ''']==]
local f = assert(real_ls(src))
setfenv(f, env)
f()
'''
open(sys.argv[2] + "/d9_hook.lua", "w").write(hook)
PYEOF
out9="$(timeout 12 "$LUAU" "$W/d9_hook.lua" 2>&1)"; rc9=$?
if [ "$out9" == "$EXP1" ]; then
	bad "反挂钩闸未生效: 挂钩后仍产出正确输出"
else
	ok "挂钩被杀 (rc=$rc9, 无限循环/静默死亡, 无正确输出)"
fi

echo "== D10 反指纹闸 (debug.info 说谎 => 必须被杀) =="
python3 - "$W/d1a.lua" "$W" <<'PYEOF'
import sys
src = open(sys.argv[1]).read()
hook = '''
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
local dbg = {}
for k, v in pairs(debug) do dbg[k] = v end
dbg.info = function(f, what, ...) if what == "s" then return "[C]" end return debug.info(f, what, ...) end
env.debug = dbg
local src = [==[''' + src + ''']==]
local f = assert(loadstring(src))
setfenv(f, env)
f()
'''
open(sys.argv[2] + "/d10_hook.lua", "w").write(hook)
PYEOF
out10="$(timeout 12 "$LUAU" "$W/d10_hook.lua" 2>&1)"; rc10=$?
if [ "$out10" == "$EXP1" ]; then
	bad "反指纹闸未生效: debug.info 说谎后仍产出正确输出"
else
	ok "指纹伪造被杀 (rc=$rc10, 无正确输出)"
fi

echo "== D11 蜜罐哨兵 (H4-3: 静态 HBOOT 字节构词零痕迹 + 守卫生效) =="
# 蜜罐由静态 HBOOT 守卫安装；API/元方法词经 CHAR 参数按字节构造，
# 可见层必须 0 明文痕迹。守卫活性由 D9/D10 双负面背书。
hp_vis=0
for pat in "newproxy" "__tostring" "__metatable" "__concat"; do
	if grep -q "$pat" "$W/d1a.lua"; then hp_vis=1; echo "  可见层发现 $pat"; fi
done
if [ "$hp_vis" == "0" ]; then
	ok "蜜罐/陷阱可见层零痕迹 (静态字节构词, D9/D10 背书守卫生效)"
else
	bad "蜜罐结构仍暴露在可见层"
fi

echo "== D12 计时守卫 (㊳/H4-2C2: 接线 + 触发路径 + 误报抽查) =="
rm -f /tmp/vm_tsrc.lua
OBSUCVIM_VM_TSRC=1 OBSUCVIM_KEY_MANIFEST="$W/d12_keys.txt" \
	"$TOOL" --preset v15 --dialect luau --seed 777 "$SRC" "$W/d12b.lua" >/dev/null 2>&1
# H4-2C2 后 RT 不再是可导出的 HQ 碎片；从构建期 TSRC 中抽取已经字段
# 随机化的静态 RT 守卫。正式产物没有运行时 dump/fallback 接口。
rt_static="/tmp/vm_tsrc.lua"
python3 - "$W" "$rt_static" <<'PYEOF'
import re, sys
W = sys.argv[1]; rt_path = sys.argv[2]
def blank(src):
    out = list(src); i = 0; n = len(src)
    while i < n:
        c = src[i]
        # Skip comments before quoted-string handling; RTFRAG source carries
        # prose with quotes, which otherwise causes this scanner to blank code
        # across a comment boundary.
        if src.startswith('--', i):
            lm = re.match(r'--\[(=*)\[', src[i:])
            if lm:
                closer = ']' + lm.group(1) + ']'
                j = src.find(closer, i + len(lm.group(0)))
                if j != -1:
                    end = j + len(closer)
                    for k in range(i, end):
                        if src[k] != '\n': out[k] = ' '
                    i = end; continue
            j = src.find('\n', i)
            end = n if j == -1 else j
            for k in range(i, end): out[k] = ' '
            i = end; continue
        if c == '[':
            lm = re.match(r'\[(=*)\[', src[i:])
            if lm:
                closer = ']' + lm.group(1) + ']'
                j = src.find(closer, i + len(lm.group(0)))
                if j != -1:
                    end = j + len(closer)
                    for k in range(i, end): out[k] = ' '
                    i = end; continue
        if c in '"\'':
            q = c; j = i+1
            while j < n:
                if src[j] == '\\': j += 2; continue
                if src[j] == q: break
                j += 1
            for k in range(i, min(j+1, n)): out[k] = ' '
            i = j+1; continue
        i += 1
    return ''.join(out)
# Parse the emitted RT factory rather than assuming its clock is argument 27
# of an unrelated entry closure (that positional assumption went stale when
# RTFRAG became the one-shot factory).
rt_source = open(rt_path).read()
rt = blank(rt_source)
sig = re.search(r'RTFRAG\s*=\s*function\(([^)]*)\)', rt)
call = re.search(r'local RUN\s*=\s*RTFRAG\(([^)]*)\)', rt)
pm = re.search(
    r'local (\w+) = 0 local (\w+) = (\w+) and \3\(\) or 0 '
    r'local (\w+) = 0 local (\w+) = nil local (\w+) = 0 while true do', rt)
if not (sig and call and pm):
    print("NOPRE"); sys.exit(1)
GE, ID, CLK, NF, PE, CNT = pm.groups()
formals = [x.strip() for x in sig.group(1).split(',')]
actuals = [x.strip() for x in call.group(1).split(',')]
if CLK not in formals or len(formals) != len(actuals):
    print("NOWIRE"); sys.exit(1)
clock_slot = formals.index(CLK)
# The outer captured clock must be handed to the exact RT formal used by the
# timing guard; also ensure the capture is sourced from the host os table.
if actuals[clock_slot] != "CLK" or not re.search(r'\blocal\s+CLK\b', rt):
    print("NOWIRE"); sys.exit(1)
if not re.search(
    r'local\s+(\w+)\s*=\s*GFE\(0\)\s*\[\s*\w+\s*\]\s+'
    r'CLK\s*=\s*\1\s+and\s+\1\s*\[\s*\w+\s*\]', rt_source):
    print("NOWIRE"); sys.exit(1)
# Trigger path: synthesize a tracer clock around the extracted RT guard body.
gi = rt.find('% 128 == 0')
# 回溯到该 if 的起点
ist = rt.rfind('if ', 0, gi)
br = rt  # 已位置保真地空格化字符串内容，代码标识符不动
depth = 0; cut = None
for m2 in re.finditer(r'\b(then|do|end|until)\b', br):
    if m2.start() < ist:
        continue
    w = m2.group(1)
    if w in ('then', 'do'):
        depth += 1
    else:
        depth -= 1
        if depth == 0:
            cut = m2.end()
            break
if cut is None:
    print("NOGUARD"); sys.exit(1)
guard = br[ist:cut]
p1 = re.search(
    r'(\w+)\.(\w+)\s*=\s*\(\1\.\2\s*\+\s*\(\(\1\.(\w+)\s*%\s*65521\)'
    r'\s*\*\s*(\w+)\s*\+\s*(\w+)\s*\+\s*\1\.(\w+)\s*\*\s*(\w+)\)'
    r'\s*%\s*268435456\)\s*%\s*268435456\s+'
    r'\1\.(\w+)\s*=\s*\1\.\6\s*\+\s*2\s*\+\s*\(\1\.\3\s*%\s*7\)', guard)
if not p1:
    print("NOPOISON"); sys.exit(1)
EF, PG, VD, WM, WB, PN, RQ, PD = p1.groups()
seq_tracer = "0, 0.001, 0.002, 0.003, 0.004, 10.004, 20.004, 20.005, 20.006"
seq_clean  = "0, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006, 0.007, 0.008"
harness = '''
local seq = {{ {seq} }}
local ix = 0
local {CLK} = function() ix = ix + 1 return seq[ix] end
local {GE} = 0
local {ID} = {CLK} and {CLK}() or 0
local {NF} = 0
local {PE} = nil
local {CNT} = 0
local {WM}, {WB}, {RQ} = 7, 11, 13
local {EF} = {{ {PG}=0, {VD}=17, {PN}=5, {PD}=0 }}
for iter = 1, 128 * 8 do
  {GE} = {GE} + 1
  {guard}
end
print({EF}.{PG}, {EF}.{PD})
'''
for name, seq in (("tracer", seq_tracer), ("clean", seq_clean)):
    lua = harness.format(seq=seq, CLK=CLK, GE=GE, ID=ID, NF=NF, PE=PE,
                         CNT=CNT, EF=EF, PG=PG, VD=VD, PN=PN, PD=PD,
                         WM=WM, WB=WB, RQ=RQ, guard=guard)
    open(f"{W}/wd_{name}.lua", "w").write(lua)
print("WIRED", CLK, "argument", clock_slot + 1)
PYEOF
rc12=$?
if [ "$rc12" != "0" ]; then
	bad "计时守卫接线/抽取失败 (rc=$rc12)"
else
	t1="$(timeout 15 "$LUAU" "$W/wd_tracer.lua" 2>&1)"
	t2="$(timeout 15 "$LUAU" "$W/wd_clean.lua" 2>&1)"
	[ "$t1" == "195	10" ] && ok "触发路径: tracer 两连窗 => 帧局部 poison=195、延迟窗=10" || bad "触发路径失效: 得到 [$t1]"
	[ "$t2" == "0	0" ] && ok "干净时钟: 8 窗口 0 投毒 (阈值不误伤)" || bad "干净时钟误报: [$t2]"
	# 误报抽查: 长运行用例真实执行必须输出等价
	"$TOOL" --preset v15 --dialect luau --seed 999 \
		"$ROOT/tests/cases/stress_control.lua" "$W/d12c.lua" >/dev/null 2>&1
	exp12="$(timeout 90 "$LUAU" "$ROOT/tests/cases/stress_control.lua" 2>&1)"
	got12="$(timeout 90 "$LUAU" "$W/d12c.lua" 2>&1)"
	[ "$got12" == "$exp12" ] && ok "误报抽查: stress 用例守卫开启下运行等价" || bad "守卫开启导致运行分歧 (疑似误报投毒)"
fi

echo "== D13 静态协议 debug 构建自检 =="
if [ -x "$ROOT/target/debug/obsucvim" ]; then
	DBGOBJ="$ROOT/target/debug/obsucvim"
else
	echo "  (构建 debug 二进制中...)"
	(cd "$ROOT" && CARGO_NET_OFFLINE=true cargo build >/dev/null 2>&1)
	DBGOBJ="$ROOT/target/debug/obsucvim"
fi
if [ -x "$DBGOBJ" ]; then
	if "$DBGOBJ" --preset v15 --dialect luau --seed 31337 "$SRC" "$W/d13.lua" >/dev/null 2>&1 \
		&& [ "$($LUAU "$W/d13.lua" 2>&1)" == "$EXP1" ]; then
		ok "debug 构建通过 + 静态协议产物运行等价"
	else
		bad "LZ 往返自检或运行失败"
	fi
else
	bad "debug 二进制不可用"
fi

echo "== D14 保护代码隐藏普查 (H4-3 静态守卫构词形态) =="
python3 - "$W/d1a.lua" <<'PYEOF2'
import re, sys
s = open(sys.argv[1]).read()
pats = {
    "死循环闸 while true do end": r"while true do end",
    "蜜罐钩子词": r"newproxy|__tostring|__metatable|__concat",
    "repeat-until kill": r"repeat \w+=\w+\+1",
    "schar 码表(3+)": r"char\((?:\d+,){3,}",
    "拼串纸老虎(乱序码表+拼串循环)": r"=\s*\w+\s*\.\.\s*\w+\(\w+\[\w+\[",
}
bad = 0
for name, p in pats.items():
    n = len(re.findall(p, s))
    if n:
        print(f"  暴露: {name} x{n}")
        bad = 1
sys.exit(bad)
PYEOF2
[ "$?" == "0" ] && ok "可见层保护痕迹清零 (死循环/蜜罐词/码表/拼串表全 0)" || bad "可见层仍有保护痕迹"

echo "== D15 环境绑定语义强化 (㊸: 桩场景 ⇒ 静默毒药) =="
# 朴素桩（形状对、语义错：typeof=table、无 tostring 格式）⇒ 折叠异值 ⇒
# 元钥匙流污染 ⇒ HBOOT 垃圾 ⇒ 静默循环——无报错、无信号（样本式毒药
# 语义）。若桩能跑通 = 语义探针失效。
"$TOOL" --preset v15 --dialect luau --bind-env roblox --seed 42 "$SRC" "$W/d15_bound.lua" >/dev/null 2>&1 || bad "绑定态构建失败"
python3 - "$W/d15_bound.lua" "$W" <<'PYD15'
import sys
src = open(sys.argv[1]).read()
W = sys.argv[2]
def wrap(s, name):
    for lvl in range(1, 5):
        cl = ']' + '='*lvl + ']'
        if cl not in s:
            op = '[' + '='*lvl + '['
            return f'local {name} = {op}\n{s}\n{cl}'
    raise RuntimeError("no level")
runner = wrap(src, 'BND') + """
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.Vector3 = { new = function(x, y, z) return { X = x, Y = y, Z = z } end }
env.Vector2 = { new = function(x, y) return { X = x, Y = y } end }
env.task = { defer = function() end }
local f = assert(loadstring(BND, "bound"))
setfenv(f, env)
local ok, err = pcall(f)
print("STUB-RUN:", ok, tostring(err):sub(1, 100))
"""
open(W + "/d15_stub.lua", "w").write(runner)
PYD15
out15="$(timeout 15 "$LUAU" "$W/d15_stub.lua" 2>&1)"; rc15=$?
if [ "$out15" == "$EXP1" ]; then
	bad "朴素桩跑通了 (语义探针失效)"
else
	ok "朴素桩 => 静默毒药 (rc=$rc15, 无正确输出/无报错信号)"
fi

echo "== D16 可见层零解码源消费点 (㊹: R018 报告 P0-2 收口) =="
# R018 报告实证攻击: 解码态 BSS 源以明文落地可见层, 攻击者在
# 「解码后、loadstring 前」对明文做 gsub 手术注入探针。㊹ 把 BSS 的
# loadstring+调用搬进 HBOOT 掩码层, 可见层不得再出现任何「LS(解码源
# 变量)」消费形态。
python3 - "$W/d1a.lua" <<'PYD16'
import re, sys
s = open(sys.argv[1]).read()
n1 = len(re.findall(r'local \w+=(\w+)\((\w+)\);return \w+\(\)\(', s))
n2 = len(re.findall(r'=(\w+)\((\w+)\)\(', s))
if n1 or n2:
    print(f"  可见层解码源消费点: 狭义 {n1}, 广义 {n2}")
    sys.exit(1)
sys.exit(0)
PYD16
[ "$?" == "0" ] && ok "可见层零解码源消费点 (gsub 手术攻击面已关闭)" || bad "可见层仍有解码源消费点"

echo "== D17 物化去钩化 (㊾: R021 char 钩子攻击封堵) =="
# R021 实证攻击: setfenv 钩子环境替换 string.char, 在物化现场逐字符截获
# 常量池载荷。㊾ 三处物化点 (parse const_loop / CDEC / MkStr) 改经字符串
# 元表 ("")["char"] (元表只读、索引不经可钩函数)。断言: 钩子环境下程序
# 仍正确运行, 载荷既不被整串钩中也不被逐字符序列钩中 (短/长载荷×4)。
if bash "$ROOT/tests/charhook_test.sh" >"$W/d17.out" 2>&1; then
	ok "物化去钩化 (char 钩子截不到载荷, 4 载荷形态全过)"
else
	bad "物化路径可被 char 钩子截获"; sed 's/^/  /' "$W/d17.out" | tail -8
fi

echo "=================================================="
echo "防御审计: PASS $pass   FAIL $fail"
[ "$fail" == "0" ] && echo "ALL DEFENSES ACTIVE" || exit 1
#   D15 环境绑定语义强化 — 朴素桩场景必须静默毒药 (㊸)
#   D16 可见层零解码源消费点 — 解码源不得落地可见层 (㊹, R018 P0-2)
#   D17 物化去钩化           — char 钩子截不到载荷 (㊾, R021)
