#!/bin/bash
# 增量51 专项: --bind-executor（IronBrew2 executor 认证守卫移植）。
#
# 诚实边界先行：守卫要求完整 executor 内省语义（getconstants/getupvalues/
# getproto/setupvalue 对任意闭包保真工作），裸 Luau **无法构造保真桩**
# （语言本身不暴露闭包内省）——因此正面运行等价只能由真执行器验证，
# 本测试组以负面实证为主（与 IronBrew2 原设计「fail-closed」同口径）：
#   E1 语法校验通过（用户验收线）
#   E2 裸 Luau 运行 ⇒ 不得产生程序正确输出（认证失败 ⇒ 毒令牌 ⇒ 错钥）
#   E3 半吊子桩（API 面齐全但内省不保真）⇒ 同样被拒
#   E4 可见层零痕迹（全部 executor API 名 0 命中）
#   E5 字节级确定性
#   E6 开关关闭 ⇒ 与现役管线逐字节一致（回退保障）
# 被 source 进 run_tests.sh（TOOL/LUAU/LUAUC 已定义）；单独跑时自适应。

if [ -z "${TOOL:-}" ]; then
	ROOT="$(cd "$(dirname "$0")/.." && pwd)"
	TOOL="$ROOT/target/release/obsucvim"
	TOOLS_BIN=/home/user/tools/bin
	if [ ! -x "$TOOLS_BIN/luau" ] || [ ! -x "$TOOLS_BIN/luau-compile" ]; then
		TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
	fi
	LUAU="$TOOLS_BIN/luau"
	LUAUC="$TOOLS_BIN/luau-compile"
fi
# 独立跑时兜底父级计数器（被 run_tests.sh source 时共用其 pass/fail）
if [ -z "${fail:-}" ]; then fail=0; fi
if [ -z "${failed:-}" ]; then failed=(); fi

EXFAIL=0
exok()  { echo "  EX-PASS $1"; }
exbad() { echo "  EX-FAIL $1"; EXFAIL=$((EXFAIL+1)); fail=$((fail+1)); failed+=("exguard/$1"); }

EXDIR=/tmp/executor_guard_test
rm -rf "$EXDIR"; mkdir -p "$EXDIR"
cat > "$EXDIR/src.lua" <<'EOF'
local code = "exguard payload 51"
local n = 7
local ok = true
local t = { n * 3, n + 5 }
local function mix(a, b)
  if ok then
    return a + b
  end
  return a - b
end
local r = mix(t[1], t[2])
print(code, r)
EOF
EXPECT="exguard payload 51	33"

# E1: 绑定态产物语法校验
if "$TOOL" --preset v15 --seed 4242 --bind-executor "$EXDIR/src.lua" "$EXDIR/bound.lua" 2>/dev/null \
	&& "$LUAUC" "$EXDIR/bound.lua" >/dev/null 2>&1; then
	exok "E1 绑定态产物语法校验通过"
else
	exbad "E1 语法校验/生成失败"
fi

# E2: 裸 Luau ⇒ 无正确输出（毒杀；挂起或报错均可，正确输出不可）
out2="$(timeout 30 "$LUAU" "$EXDIR/bound.lua" 2>&1)"; rc2=$?
if [[ "$out2" != *"$EXPECT"* ]]; then
	exok "E2 裸 Luau 无正确输出 (rc=$rc2)"
else
	exbad "E2 裸 Luau 竟输出正确载荷"
fi

# E3: 半吊子桩（getgenv/identifyexecutor/checkcaller 齐全，但闭包内省
# 不保真——getconstants 恒空表）⇒ 行为探针拒绝 ⇒ 无正确输出。
# 桩脚本 = 影子环境注入（setfenv）+ 绑定产物内联（luau CLI 无 io）。
cat > "$EXDIR/stub_tpl.lua" <<'STUB'
local env = {}
for k, v in pairs(getfenv(0)) do env[k] = v end
local cap = {}
env.getgenv = function() return cap end
cap.getgenv = env.getgenv
cap.identifyexecutor = function() return "HalfShim", "0.1" end
cap.checkcaller = function() return true end
cap.iscclosure = function(f) return debug.info(f, "s") == "[C]" end
cap.islclosure = function(f) return debug.info(f, "s") ~= "[C]" end
cap.newcclosure = function(f) return f end
cap.getconstants = function() return {} end
cap.getupvalues = function() return {} end
cap.getprotos = function() return {} end
cap.setupvalue = function() return true end
cap.loadstring = loadstring
cap.typeof = env.typeof or type
cap.game = { GetService = function() return { ClassName = "Players" } end }
cap.Instance = { new = function() return {} end }
cap.Vector3 = setmetatable({ new = function() return setmetatable({ X = 0, Y = 0, Z = 0 }, { __tostring = function() return "0, 0, 0" end }) end }, {})
cap.task = { wait = function() end, spawn = function() end, defer = function() end }
local src = __BOUND_LITERAL__
local f = assert(loadstring(src, "bound"))
setfenv(f, env)
local okrun = pcall(f)
if not okrun then print("STUBDEAD") end
STUB
python3 - "$EXDIR/bound.lua" "$EXDIR/stub_tpl.lua" "$EXDIR/stub_run.lua" <<'PY'
import sys
bound, tpl, out = sys.argv[1], sys.argv[2], sys.argv[3]
src = open(bound, encoding="utf-8").read()
lit = '"' + src.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'
txt = open(tpl, encoding="utf-8").read().replace("__BOUND_LITERAL__", lit)
open(out, "w", encoding="utf-8").write(txt)
PY
out3="$(timeout 30 "$LUAU" "$EXDIR/stub_run.lua" 2>&1)"; rc3=$?
if [[ "$out3" != *"$EXPECT"* ]]; then
	exok "E3 半吊子桩被拒 (rc=$rc3)"
else
	exbad "E3 半吊子桩竟通过认证"
fi

# E4: 可见层零痕迹
miss=0
for w in identifyexecutor getgenv checkcaller iscclosure islclosure newcclosure \
	getconstants getupvalues getproto setupvalue GuardKey IronBrew; do
	if grep -q "$w" "$EXDIR/bound.lua"; then miss=$((miss+1)); fi
done
if [[ "$miss" == "0" ]]; then
	exok "E4 可见层零痕迹 (13 词全 0 命中)"
else
	exbad "E4 可见层有 $miss 个痕迹词命中"
fi

# E5: 字节级确定
"$TOOL" --preset v15 --seed 4242 --bind-executor "$EXDIR/src.lua" "$EXDIR/bound2.lua" 2>/dev/null
if cmp -s "$EXDIR/bound.lua" "$EXDIR/bound2.lua"; then
	exok "E5 字节级确定"
else
	exbad "E5 同种子两次构建不一致"
fi

# E6: 开关关闭(--no-bind-executor) ⇒ 与 2026-10-11 前的默认管线逐字节一致
# （回退保障；SHA 锚取自翻转默认前的二进制输出）
OFF_SHA=fcbba5a9918a0ef25a82d2d3b170fec41f049fd807e830010919453021eb666c
"$TOOL" --preset v15 --no-bind-executor --seed 4242 "$EXDIR/src.lua" "$EXDIR/off_a.lua" 2>/dev/null
"$TOOL" --preset v15 --no-bind-executor --seed 4242 "$EXDIR/src.lua" "$EXDIR/off_b.lua" 2>/dev/null
if cmp -s "$EXDIR/off_a.lua" "$EXDIR/off_b.lua"; then
	exok "E6 开关关闭字节级确定"
else
	exbad "E6 开关关闭两次构建不一致"
fi
if [ "$(sha256sum "$EXDIR/off_a.lua" | cut -d' ' -f1)" == "$OFF_SHA" ]; then
	exok "E6b --no-bind-executor 与翻转前默认逐字节一致(SHA 锚)"
else
	exbad "E6b --no-bind-executor 字节偏离翻转前默认"
fi

# E7: 底层保护默认开启（2026-10-11）——默认构建即绑定态：
# (a) 与显式 --bind-executor 逐字节一致 (b) 与 --no-bind 构建不同
# (c) 裸 Luau 运行不得产出正确载荷（同 E2 口径）
"$TOOL" --preset v15 --seed 4242 "$EXDIR/src.lua" "$EXDIR/def.lua" 2>/dev/null
if cmp -s "$EXDIR/def.lua" "$EXDIR/bound.lua"; then
	exok "E7a 默认构建 == 显式绑定构建(逐字节)"
else
	exbad "E7a 默认构建与显式绑定不一致"
fi
if cmp -s "$EXDIR/def.lua" "$EXDIR/off_a.lua"; then
	exbad "E7b 默认构建竟等价于关闭态"
else
	exok "E7b 默认构建携带守卫(≠关闭态)"
fi
out7="$(timeout 30 "$LUAU" "$EXDIR/def.lua" 2>&1)"; rc7=$?
if [[ "$out7" != *"$EXPECT"* ]]; then
	exok "E7c 默认构建裸 Luau 无正确输出 (rc=$rc7)"
else
	exbad "E7c 默认构建竟在裸 Luau 输出正确载荷"
fi

# E8: 非 v15 管线——隐式默认静默降级、显式请求仍报错
if "$TOOL" --preset low --seed 4242 "$EXDIR/src.lua" "$EXDIR/low_i.lua" 2>/dev/null; then
	exok "E8a 非 v15 隐式默认降级为关闭(构建成功)"
else
	exbad "E8a 非 v15 隐式默认应静默降级却失败"
fi
if "$TOOL" --preset low --bind-executor --seed 4242 "$EXDIR/src.lua" "$EXDIR/low_x.lua" 2>/dev/null; then
	exbad "E8b 非 v15 显式 --bind-executor 应报错"
else
	exok "E8b 非 v15 显式 --bind-executor 报错"
fi

echo "EXECUTOR GUARD: PASS $((11-EXFAIL))/11   FAIL $EXFAIL"
[ "$EXFAIL" -eq 0 ]
