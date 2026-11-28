#!/bin/bash
# 增量㉑ 环境绑定测试（--bind-env）。
# 绑定态产物只在目标运行时（Roblox）可加载；通用分析沙箱（luau CLI）在
# 加载期即失败（原语槽引用 Vector3/Vector2/task，构造即 index nil）。
# 故绑定态产物不进通用运行等价矩阵，只做两项验证：
#   (a) 语法校验通过（luau-compile）；
#   (b) CLI 加载失败（绑定生效——这正是防御目的）。
# 同时验证未绑定产物不受影响（零参数直接可运行）。
# 可独立运行，也可被 run_tests.sh source（复用计数器）。
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="${OBSUCVIM_BIN:-$ROOT/target/release/obsucvim}"
AUDIT_TOOL="${OBSUCVIM_AUDIT_BIN:-$ROOT/target/audit/release/obsucvim}"
if [ ! -x "$AUDIT_TOOL" ]; then
	if [ -x "$ROOT/target/debug/obsucvim" ]; then AUDIT_TOOL="$ROOT/target/debug/obsucvim"; else AUDIT_TOOL="$TOOL"; fi
fi
SRC="$ROOT/tests/cases/basics.lua"

TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
	pass=0; fail=0; failed=()
	_STANDALONE=1
else
	_STANDALONE=0
fi
BG_TMP="$(mktemp -d)"
if [[ "$_STANDALONE" == "1" ]]; then trap 'rm -rf "$BG_TMP"' EXIT; fi

gf() { echo "FAIL [envbind:$1] $2"; fail=$((fail+1)); failed+=("envbind/$1"); }

# 生成绑定态产物
if ! "$TOOL" --preset v15 --dialect luau --bind-env roblox --seed 42 "$SRC" "$BG_TMP/bound.lua" 2>"$BG_TMP/err"; then
	gf "build" "tool failed: $(head -1 "$BG_TMP/err")"
else
	# (a) 语法校验通过
	if "$LUAUC" "$BG_TMP/bound.lua" >/dev/null 2>&1; then
		pass=$((pass+1))
	else
		gf "syntax" "绑定产物语法校验失败"
	fi
	# (b) CLI 加载失败（绑定生效）
	timeout 5 "$LUAU" "$BG_TMP/bound.lua" >/dev/null 2>&1; rc=$?
	if [[ "$rc" != "0" ]]; then
		pass=$((pass+1))
	else
		gf "leak" "绑定产物在通用沙箱竟然跑起来了（绑定失效）"
	fi
	# (c) 绑定生效且保持密封：阶段 D 后目标运行时 API 引用不再以明文
	# 出现在产物中（密封是预期形态，不得为迎合旧断言恢复明文）。
	# 等价断言 = 无明文 API 引用 + 通用沙箱加载失败（(b) 已验）+
	# 与未绑定产物字节不同（绑定确实注入了槽捕获）。
	"$TOOL" --preset v15 --dialect luau --seed 42 "$SRC" "$BG_TMP/unbound_ref.lua" 2>/dev/null
	if ! grep -q "Vector3\|Vector2\|task\." "$BG_TMP/bound.lua" \
		&& ! cmp -s "$BG_TMP/bound.lua" "$BG_TMP/unbound_ref.lua"; then
		pass=$((pass+1))
	else
		gf "noref" "绑定产物密封/差异断言失败（明文 API 引用或与未绑定产物相同）"
	fi
fi

# (d) ㊸/㊺ 环境绑定去中心化形态：绑定态 = 模块表槽捕获（Vector3.new 等，
# 混入原语槽族）+ 引导三段槽位间接调用 + ef1 双消费（㊺: hqi 解掩种子
# 前移消费 + 元钥匙流消费）；未绑定态只保留无害的 ef1 声明，无折叠消费。
# （㊺ 正向等价/顺序守卫见 faithful_stub_test.sh。）
rm -f /tmp/vm_tsrc.lua
OBSUCVIM_VM_TSRC=1 "$AUDIT_TOOL" --preset v15 --dialect luau --bind-env roblox --seed 42 "$SRC" "$BG_TMP/bound2.lua" >/dev/null 2>&1
if grep -q "ef1%\|ef1 %" /tmp/vm_tsrc.lua 2>/dev/null && grep -q "\[5[0-9][0-9][0-9]\]" /tmp/vm_tsrc.lua 2>/dev/null; then
	pass=$((pass+1))
else
	gf "envval" "绑定态缺少环境值折叠消费（ef1 钥匙流项 / 槽捕获）"
fi
rm -f /tmp/vm_tsrc.lua
OBSUCVIM_VM_TSRC=1 "$AUDIT_TOOL" --preset v15 --dialect luau --seed 42 "$SRC" "$BG_TMP/unbound2.lua" >/dev/null 2>&1
if grep -q "ef1%\|ef1 %" /tmp/vm_tsrc.lua 2>/dev/null; then
	gf "envval-leak" "未绑定产物混入环境值折叠消费"
else
	pass=$((pass+1))
fi

# 未绑定对照：零参数直接可运行（无回归）
"$TOOL" --preset v15 --dialect luau --seed 42 "$SRC" "$BG_TMP/unbound.lua" 2>/dev/null
o1="$(timeout 30 "$LUAU" "$SRC" 2>&1)"; r1=$?
o2="$(timeout 30 "$LUAU" "$BG_TMP/unbound.lua" 2>&1)"; r2=$?
if [[ "$r1" == "$r2" && "$o1" == "$o2" ]]; then
	pass=$((pass+1))
else
	gf "unbound" "未绑定产物回归（rc $r1 vs $r2）"
fi

if [[ "$_STANDALONE" == "1" ]]; then
	echo "ENV BIND PASS: $pass   FAIL: $fail"
	[[ "$fail" != "0" ]] && { echo "failed: ${failed[*]}"; exit 1; }
	echo "ALL ENV BIND GREEN"
else
	rm -rf "$BG_TMP"
fi
