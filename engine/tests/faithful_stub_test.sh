#!/bin/bash
# ㊺ 忠实桩等价测试（--bind-env 绑定态的「正向」验证 + 读前写顺序守卫）。
#
# 背景: ㊸..㊹ 存在两处被掩盖的错误——
#   (1) 读前写: 段 B/C 排在 metavm 种子消费点之后 ⇒ ef1 恒以初值 0 被
#       消费 ⇒ 环境值绑定实为死代码，且补偿常数按「ef1 折出 env_exp」
#       设计 ⇒ 绑定态产物在目标运行时也解出垃圾、PD 挂死。
#   (2) 镜像错位: 门 (gate_c) 运行期形态 ((S·ec5+EN3·ec6+EN6·ec7)·ec8)
#       与 Rust 镜像分组 ((S+f_ty·ec5+f_ts·ec6)·ec7) 系数错位 ⇒ 即便
#       顺序修好，忠实环境折出的 ef1 也不等于镜像期望的 env_exp。
# ㊺ 两处同修。本测试给出此前完全缺失的**正向**断言:
#   (a) 忠实仿真（__tostring 元方法给 "x, y, z"、typeof 槽给 "Vector3"、
#       分量如实回传）⇒ 绑定态产物必须跑通且输出与源程序逐字一致。
#   (b) 顺序守卫: TSRC 里 ef1 的赋值点必须早于 H4-3 HBD 消费点。
#   (c) HBD 必须真实引用 ef1，把环境折叠送入 BSS/RT 双钥匙。
#   (d) HBD 同时引用 pb，保留原生 loader 探针的独立消费面。
#
# 关键仿真约束（Luau CLI 限制，也即真实攻击者需达到的仿真深度）:
#   * getfenv(0) 恒返回真 _G（setfenv 影子不可见、_G 冻结、协程环境不
#     继承）⇒ 引导层经 GFE(0) 取的 tostring 是**真** tostring。桩对象的
#     字符串语义必须走 __tostring 元方法（真 tostring 会尊重它）。
#   * typeof 槽经模块表构造期裸标识符捕获 ⇒ 走块环境 ⇒ setfenv 的
#     env.typeof 桩可见（返回 "Vector3"，其余委托真 typeof）。
#
# 可独立运行: bash tests/faithful_stub_test.sh；也可被 run_tests.sh source。
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
SRC="$ROOT/tests/cases/basics.lua"

TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
	pass=0; fail=0; failed=()
	_STANDALONE=1
else
	_STANDALONE=0
fi
FS_TMP="$(mktemp -d)"
if [[ "$_STANDALONE" == "1" ]]; then trap 'rm -rf "$FS_TMP"' EXIT; fi
gf() { echo "FAIL [faithful:$1] $2"; fail=$((fail+1)); failed+=("faithful/$1"); }

EXP="$(timeout 30 "$LUAU" "$SRC" 2>&1)"

# ---- 生成绑定态产物（调试转储 TSRC 供顺序守卫使用）----
rm -f /tmp/vm_tsrc.lua
if ! OBSUCVIM_VM_TSRC=1 "$TOOL" --preset v15 --dialect luau --bind-env roblox \
		--seed 42 "$SRC" "$FS_TMP/bound.lua" 2>"$FS_TMP/err"; then
	gf "build" "绑定态构建失败: $(head -1 "$FS_TMP/err")"
else
	# ---- (b) 顺序守卫: ef1 赋值点必须早于全部消费点 ----
	if [[ -f /tmp/vm_tsrc.lua ]]; then
		asg="$(grep -n 'ef1 = (((' /tmp/vm_tsrc.lua | head -1 | cut -d: -f1)"
		use_hbd="$(grep -n 'HBD = .*ef1 % 65536' /tmp/vm_tsrc.lua | head -1 | cut -d: -f1)"
		if [[ -z "$asg" || -z "$use_hbd" ]]; then
			gf "order-missing" "TSRC 缺 ef1 赋值/HBD 消费点（asg=$asg use=$use_hbd）"
		elif [[ "$asg" -lt "$use_hbd" ]]; then
			pass=$((pass+1))
		else
			gf "order" "ef1 赋值(行$asg)晚于 HBD 消费(行$use_hbd)——读前写回归"
		fi
		# ---- (c) H4-3 环境值进入 HBD ----
		if grep -q 'HBD = .*ef1 % 65536' /tmp/vm_tsrc.lua; then
			pass=$((pass+1))
		else
			gf "forward" "HBD 未引用 ef1（环境绑定未进入静态 HBOOT 钥匙）"
		fi
		# ---- (d) 原生 loader 探针也进入 HBD ----
		if grep -q 'HBD = .*pb % 65536' /tmp/vm_tsrc.lua; then
			pass=$((pass+1))
		else
			gf "pb-dual" "HBD 未引用 pb（原生性探针消费面丢失）"
		fi
	else
		gf "tsrc" "TSRC 转储缺失（无法做顺序守卫）"
		pass=$((pass+1)) # 前移存在性无法验证，不重复计罚
		asg=; use_hbd=
		[[ -n "$asg" ]] || true
	fi
	rm -f /tmp/vm_tsrc.lua

	# ---- (a) 忠实仿真正向等价 ----
	python3 - "$FS_TMP/bound.lua" "$FS_TMP" <<'PYF'
import sys
src = open(sys.argv[1]).read()
W = sys.argv[2]
def wrap(s, name):
    for lvl in range(1, 5):
        cl = ']' + '='*lvl + ']'
        if cl not in s:
            return f'local {name} = [{ "="*lvl }[\n{s}\n]{ "="*lvl }]'
    raise RuntimeError("no level")
runner = wrap(src, 'BND') + """
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
local rtypeof = typeof
local V3M = { __tostring = function(s)
  return s.X .. ", " .. s.Y .. ", " .. s.Z
end }
local V2M = {}
env.typeof = function(v)
  if rtypeof(v) == "table" then
    local mt = getmetatable(v)
    if mt == V3M then return "Vector3" end
    if mt == V2M then return "Vector2" end
  end
  return rtypeof(v)
end
env.Vector3 = { new = function(x, y, z) return setmetatable({X=x, Y=y, Z=z}, V3M) end }
env.Vector2 = { new = function(x, y) return setmetatable({X=x, Y=y}, V2M) end }
env.task = { defer = function() end }
local f = assert(loadstring(BND, "bound"))
setfenv(f, env)
local ok, err = pcall(f)
print("FAITHFUL-RUN:", ok, tostring(err):sub(1, 120))
"""
open(W + "/faithful.lua", "w").write(runner)
PYF
	out="$(timeout 30 "$LUAU" "$FS_TMP/faithful.lua" 2>&1)"; rc=$?
	prog_out="$(printf '%s\n' "$out" | grep -v '^FAITHFUL-RUN:')"
	if [[ "$rc" == "124" ]]; then
		gf "run" "忠实仿真超时（种子错钥/挂死形态）"
	elif echo "$out" | grep -qE "FAITHFUL-RUN:[[:space:]]*true" && [[ "$prog_out" == "$EXP" ]]; then
		pass=$((pass+1))
	else
		gf "run" "忠实仿真未跑通或输出不一致 (rc=$rc)"
	fi

	# ---- 哑桩负例（㊿ 探针必须检出）: Vector3/Vector2 只给裸 table、
	# 不覆写 typeof/tostring ⇒ typeof="table"、tostring=地址 ⇒ 探针折出
	# 异值 ⇒ 常量解码错钥 ⇒ 静默毒药（挂死/无载荷输出）。断言：载荷串
	# 绝不出现在输出里。
	python3 - "$FS_TMP/bound.lua" "$FS_TMP" <<'PYN'
import sys
src = open(sys.argv[1]).read()
W = sys.argv[2]
def wrap(s, name):
    for lvl in range(1, 5):
        cl = ']' + '='*lvl + ']'
        if cl not in s:
            return f'local {name} = [{ "="*lvl }[\n{s}\n]{ "="*lvl }]'
    raise RuntimeError("no level")
runner = wrap(src, 'BND') + """
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.Vector3 = { new = function(x, y, z) return {X=x, Y=y, Z=z} end }
env.Vector2 = { new = function(x, y) return {X=x, Y=y} end }
env.task = { defer = function() end }
local f = assert(loadstring(BND, "bound"))
setfenv(f, env)
local ok, err = pcall(f)
print("NAIVE-RUN:", ok, tostring(err):sub(1, 80))
"""
open(W + "/naive.lua", "w").write(runner)
PYN
	nout="$(timeout 12 "$LUAU" "$FS_TMP/naive.lua" 2>&1)"
	# 哑桩下绝不能吐出源程序的真实输出（载荷被静默毒杀）
	if [[ "$nout" == *"$EXP"* ]]; then
		gf "naive" "哑桩竟跑通了（环境探针失效）"
	else
		pass=$((pass+1))
	fi
fi

if [[ "$_STANDALONE" == "1" ]]; then
	echo "FAITHFUL PASS: $pass   FAIL: $fail"
	[[ "$fail" != "0" ]] && { echo "failed: ${failed[*]}"; exit 1; }
	echo "ALL FAITHFUL GREEN"
else
	rm -rf "$FS_TMP"
fi
