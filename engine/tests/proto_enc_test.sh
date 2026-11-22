#!/bin/bash
# 增量㉒ (选项B — B-2 碎片即用即毁) 测试：原型加密集散。
#
# 防御目标（R006 壁垒①）：进程生命周期内任何时刻都不存在完整解码态
# 程序——boot 解析出的原型表立即由 ENC 碎片重编码为密文并销毁平表
# （PF = nil）；每次调用经 DDEC 碎片按帧解码出新表，帧退即毁。闭包只
# 捕获密文原型（makefn(pf, ...) + E.k 子原型图）。
#
# 检查项：
#   (1) pe-enc   模板形态：ENC/DDEC 接线齐全、PF 销毁、入口走 MPF、
#                makefn 不再查 PF 平表、Closure 经 E.k 取子原型；
#   (2) pe-frags H4-2C2 后 HQ 源码碎片 = 0；BSS/RT 均为一次性
#                数值初始化协议，Handler/continuation/Call 均 loaderless；
#   (3) pe-run   嵌套闭包程序运行等价（3 种子）；
#   (4) pe-const 常量往返：小数/大整数/布尔/含转义字符串逐字还原；
#   (5) pe-rec   深递归 + 闭包计数器运行等价。
# 可独立运行，也可被 run_tests.sh source（复用计数器）。
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"

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
PE_TMP="$(mktemp -d)"
if [[ "$_STANDALONE" == "1" ]]; then trap 'rm -rf "$PE_TMP"' EXIT; fi

gf() { echo "FAIL [protoenc:$1] $2"; fail=$((fail+1)); failed+=("protoenc/$1"); }

# ---------- 用例 ----------
cat > "$PE_TMP/nested.lua" <<'EOF'
local function outer(x)
  local function inner(y)
    return x + y * 2
  end
  return inner(x) + inner(10)
end
local t = {}
for i = 1, 6 do t[i] = outer(i) end
print(table.concat(t, ","))
EOF

cat > "$PE_TMP/consts.lua" <<'EOF'
local pi = 3.14159
local big = 1000000000000000
local neg = -0.000125
local s1 = "tab\tnl\nquote\"end"
local s2 = ""
local b1, b2 = true, false
print(pi, big, neg, pi + neg, s1, #s1, s2 == "", b1, b2, 2 ^ 52 + 1, 0x7FFFFFFF)
EOF

cat > "$PE_TMP/rec.lua" <<'EOF'
local function fib(n)
  if n < 2 then return n end
  return fib(n - 1) + fib(n - 2)
end
local function make_counter(start)
  local c = start
  return function(step)
    c = c + step
    return c
  end
end
local ctr = make_counter(100)
print(fib(14), ctr(7), ctr(11), ctr(1))
EOF

# ---------- (1) 模板形态 ----------
# （OBSUCVIM_VM_TSRC 写死输出到 /tmp/vm_tsrc.lua）
rm -f /tmp/vm_tsrc.lua
if OBSUCVIM_VM_TSRC=1 "$TOOL" --preset v15 --dialect luau --seed 42 \
	"$PE_TMP/nested.lua" "$PE_TMP/nested.lua.out" 2>"$PE_TMP/err"; then
	ts=/tmp/vm_tsrc.lua
	ok=1
	# 注：E.k[b + 1]（Closure 取子原型）与编码体（㉓ 起）都在加密碎片内，
	# TSRC 不可见，其存在性由 (3)/(5) 的嵌套闭包运行等价间接验证。
	# ㉓：解码态原型不再落地可见层——BSS 直接返回编码态树根。
	# H4-2C1/C2：BSS 与 RT 都由静态、字段随机的一次性数值协议装配；
	# TSRC 只保留离散 RTFRAG 值交接，禁止再从 HW 源码碎片提取。
	# HBOOT 装配调用仍透传 FN/NOPA，并只向外返回 MPF/SALT 等密封值。
	# H2-11 (壳参数序重排): 装配调用的实参序每构建重排，故 FN/NOPA 的
	# 存在性改为「在 _hbf()(...) 实参表内」的结构判定，不再假设相邻序。
	for pat in "MPF, SALT" "local HBOOT" "HBOOT = nil" "local RTFRAG" \
		"local RUN = RTFRAG(" "RTFRAG = nil" "return RUN(MPF"; do
		if ! grep -q "$pat" "$ts"; then ok=0; gf "enc" "模板缺少形态: $pat"; fi
	done
	# H2-11: 装配调用实参须含 FN 与 NOPA（序任意）。
	if ! python3 - "$ts" <<'PYCHK'
import re, sys
src = open(sys.argv[1], encoding="utf-8", errors="ignore").read()
src = re.sub(r"\[=*\[.*?\]=*\]", "X", src, flags=re.S)
m = re.search(r"PCAL\(function\(\) return HBOOT\(([^)]*)\) end\)", src)
if not m:
    print("no static HBOOT call"); sys.exit(1)
args = [a.strip() for a in m.group(1).split(",")]
if "FN" not in args or "NOPA" not in args:
    print("FN/NOPA missing:", args); sys.exit(1)
sys.exit(0)
PYCHK
	then ok=0; gf "enc" "静态 HBOOT 调用实参缺 FN/NOPA"; fi
	# 平表/解码态/可见运行时残留必须消失（㉗：探针目标清零）。
	# CT/CX 两字母会与随机局部名合法碰撞，不能作为文本 oracle；其旧表
	# 行为由 H4 zero-loader 审计与下方 HQ=0 联合约束。
	# 先剥掉 HQ 长串密文（base-94 随机字节会巧合命中任何模式）。
	sed -E 's/\[=*\[.*\]=*\]/[[X]]/' "$ts" > "$ts.stripped"
	for pat in "PF\[#FN\]" "PF\[idx\]" "= ENCF(" "PF = nil" "LS(BSS)" \
		"LS(HB)" "table.concat(MH)" "_hbf" "RTFRAG = HW" \
		"newE = function" "makefn(pf, V, upsf, S)" "run = function(pf" \
		"DDEC = HW"; do
		if grep -q "$pat" "$ts.stripped"; then ok=0; gf "enc" "模板残留可见运行时/解码态形态: $pat"; fi
	done
	[[ "$ok" == "1" ]] && pass=$((pass+1))
else
	gf "enc" "构建失败: $(head -1 "$PE_TMP/err")"
fi

# ---------- (2) 碎片计数 ----------
n_hq=$(grep -c "HQ\[" /tmp/vm_tsrc.lua 2>/dev/null || true)
n_hq=${n_hq:-0}
if [[ "$n_hq" == "0" ]]; then
	pass=$((pass+1))
else
	gf "frags" "H4-2C2 HQ 源码碎片数 $n_hq != 0（BSS/RT/Call/continuation/Handler 应全部 loaderless）"
fi

# ---------- (3) 嵌套闭包运行等价（3 种子） ----------
o1="$(timeout 30 "$LUAU" "$PE_TMP/nested.lua" 2>&1)"; r1=$?
ok=1
for seed in 1 777 31337; do
	"$TOOL" --preset v15 --dialect luau --seed "$seed" \
		"$PE_TMP/nested.lua" "$PE_TMP/n.$seed.lua" >/dev/null 2>&1
	o2="$(timeout 30 "$LUAU" "$PE_TMP/n.$seed.lua" 2>&1)"; r2=$?
	if [[ "$r1" != "$r2" || "$o1" != "$o2" ]]; then
		ok=0; gf "run" "nested seed=$seed 不等价 (rc $r1 vs $r2)"
	fi
done
[[ "$ok" == "1" ]] && pass=$((pass+1))

# ---------- (4) 常量往返 ----------
o1="$(timeout 30 "$LUAU" "$PE_TMP/consts.lua" 2>&1)"; r1=$?
"$TOOL" --preset v15 --dialect luau --seed 42 \
	"$PE_TMP/consts.lua" "$PE_TMP/consts.out.lua" >/dev/null 2>&1
o2="$(timeout 30 "$LUAU" "$PE_TMP/consts.out.lua" 2>&1)"; r2=$?
if [[ "$r1" == "$r2" && "$o1" == "$o2" ]]; then
	pass=$((pass+1))
else
	gf "const" "常量往返失真 (rc $r1 vs $r2): '$o1' vs '$o2'"
fi

# ---------- (5) 深递归 + 闭包计数器 ----------
o1="$(timeout 30 "$LUAU" "$PE_TMP/rec.lua" 2>&1)"; r1=$?
"$TOOL" --preset v15 --dialect luau --seed 999999 \
	"$PE_TMP/rec.lua" "$PE_TMP/rec.out.lua" >/dev/null 2>&1
o2="$(timeout 30 "$LUAU" "$PE_TMP/rec.out.lua" 2>&1)"; r2=$?
if [[ "$r1" == "$r2" && "$o1" == "$o2" ]]; then
	pass=$((pass+1))
else
	gf "rec" "递归/闭包失真 (rc $r1 vs $r2): '$o1' vs '$o2'"
fi

# ---------- (6) H4-2C2 默认构建零 HQ 源码碎片 ----------
# BSS/RT 已静态数值化；默认（未启用 executor-bind）不应再有任何可由
# OBSUCVIM_FRAG_SAN 导出的源码载荷。若重新出现即代表隐藏 fallback 回归。
rm -rf "$PE_TMP/frags"
mkdir -p "$PE_TMP/frags"
OBSUCVIM_FRAG_SAN="$PE_TMP/frags" "$TOOL" --preset v15 --dialect luau \
	--seed 42 "$PE_TMP/nested.lua" "$PE_TMP/nested.san.lua" >/dev/null 2>&1
if compgen -G "$PE_TMP/frags/san_*.src" >/dev/null; then
	gf "sanitize" "H4-2C2 默认构建仍导出 HQ 源码碎片"
else
	pass=$((pass+1))
fi

if [[ "$_STANDALONE" == "1" ]]; then
	echo "PROTO ENC PASS: $pass   FAIL: $fail"
	[[ "$fail" != "0" ]] && { echo "failed: ${failed[*]}"; exit 1; }
	echo "ALL PROTO ENC GREEN"
else
	rm -rf "$PE_TMP"
fi
