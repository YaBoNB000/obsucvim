#!/bin/bash
# ㊾ 物化去钩化回归测试（R021 回合）。
#
# 背景: R021 报告实证攻击——setfenv 注入钩子环境、替换 `string.char`，
# 在字符串物化现场逐字符截获常量池载荷。根因 = 三处物化点都用了块环境
# 捕获的可钩 `string.char`：① parse 期 `const_loop`（boot 物化全池）
# ② 帧期惰性 `CDEC` ③ `MkStr` 短串 handler。㊾ 三处全改经字符串元表
# `("")["char"]`（元表只读、索引不经可钩函数）。
#
# 本测试断言: 钩住 `string.char` 后——
#   (a) 程序仍正确运行（输出逐字一致）；
#   (b) 载荷既不被整串钩中，也不被逐字符序列钩中（短/长载荷各一，
#       分别覆盖 MkStr 与常量池路径）。
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
CH_TMP="$(mktemp -d)"
if [[ "$_STANDALONE" == "1" ]]; then trap 'rm -rf "$CH_TMP"' EXIT; fi
gf() { echo "FAIL [charhook:$1] $2"; fail=$((fail+1)); failed+=("charhook/$1"); }

# 生成钩子环境复测器: $1=载荷 输出到 $2
mk_hook() {
	local pay="$1" out="$2" art="$3"
	printf 'local x = "%s"\nprint(x)\n' "$pay" > "$CH_TMP/src_$pay.lua"
	"$TOOL" --preset v15 --dialect luau --seed 41207963 "$CH_TMP/src_$pay.lua" "$art" 2>/dev/null || return 1
	python3 - "$pay" "$art" "$out" <<'PYEOF'
import sys
pay, art, out = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(art).read()
wrap = None
for lvl in range(1, 5):
    cl = ']' + '='*lvl + ']'
    if cl not in s:
        wrap = f'local ART = [{ "="*lvl }[\n{s}\n]{ "="*lvl }]\n'
        break
body = r'''
local PAY = "XPLACEHOLDERX"
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
local captured = {}
local hooked_string = {}
for k, v in pairs(string) do hooked_string[k] = v end
local oc = string.char
hooked_string.char = function(...)
  local res = oc(...)
  captured[#captured+1] = res
  return res
end
env.string = hooked_string
local f = assert(loadstring(ART, "artifact"))
setfenv(f, env)
local ok, err = pcall(f)
local full = 0
for _, c in ipairs(captured) do if c == PAY then full = full + 1 end end
local consec = 0
local chars = {}
for i = 1, #PAY do chars[i] = PAY:sub(i,i) end
for i = 1, #captured - (#PAY-1) do
  local m = true
  for j = 1, #PAY do if captured[i+j-1] ~= chars[j] then m = false break end end
  if m then consec = consec + 1 end
end
print("RUN_OK:" .. tostring(ok))
print("FULL_HOOK:" .. full)
print("CHARSEQ_HOOK:" .. consec)
'''
wrap += body.replace("XPLACEHOLDERX", pay)
open(out, 'w').write(wrap)
PYEOF
}

check_pay() {
	local pay="$1" tag="$2"
	mk_hook "$pay" "$CH_TMP/hook_$tag.lua" "$CH_TMP/art_$tag.lua" || { gf "$tag" "构建失败"; return; }
	local outp; outp="$(timeout 40 "$LUAU" "$CH_TMP/hook_$tag.lua" 2>&1)"
	local run_ok full_hook cseq_hook
	run_ok="$(printf '%s\n' "$outp" | grep '^RUN_OK:' | cut -d: -f2)"
	full_hook="$(printf '%s\n' "$outp" | grep '^FULL_HOOK:' | cut -d: -f2)"
	cseq_hook="$(printf '%s\n' "$outp" | grep '^CHARSEQ_HOOK:' | cut -d: -f2)"
	if [[ "$run_ok" != "true" ]]; then
		gf "$tag" "钩子环境下程序未正确运行 (RUN_OK=$run_ok)"
	elif [[ "$full_hook" != "0" ]]; then
		gf "$tag" "载荷被整串钩中 ($full_hook 次)"
	elif [[ "$cseq_hook" != "0" ]]; then
		gf "$tag" "载荷被逐字符序列钩中 ($cseq_hook 次)"
	else
		pass=$((pass+1))
	fi
}

# 短载荷(<=5B, MkStr 路径) + 长载荷(常量池路径)
check_pay "Zq7" "short3"
check_pay "Zq7xW" "short5"
check_pay "beryl 8m2q" "long10"
check_pay "Zq7xWvNp3" "long9"

if [[ "$_STANDALONE" == "1" ]]; then
	echo "CHARHOOK PASS: $pass   FAIL: $fail"
	[[ "$fail" != "0" ]] && { echo "failed: ${failed[*]}"; exit 1; }
	echo "ALL CHARHOOK GREEN"
else
	rm -rf "$CH_TMP"
fi
