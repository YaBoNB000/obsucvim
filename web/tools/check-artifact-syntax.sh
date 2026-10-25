#!/usr/bin/env bash
# 产物语法/加载校验（Luau 真编译器，不是正则猜测）
#
#   ./web/tools/check-artifact-syntax.sh                 # 校验 artifacts/samples/obsucvim/suda-test/*.lua
#   ./web/tools/check-artifact-syntax.sh a.lua b.lua     # 校验指定文件
#   RUN=1 ./web/tools/check-artifact-syntax.sh           # 额外在 Luau CLI 里跑一遍
#
# 依赖（任一）：
#   LUAU_COMPILE=/path/to/luau-compile   —— 只做语法校验，最准
#   LUAU_BIN=/path/to/luau               —— 退回用 `luau -c` 之类，或 RUN=1 时执行
#   PATH 里有 luau-compile / luau
#
# 说明：语法通过 ≠ 能跑。混淆产物在目标宿主的可运行性要另一个测试
# （web/tools/test-artifact-runtime.py）来保证，两者互补。
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LUAU_COMPILE="${LUAU_COMPILE:-$(command -v luau-compile || true)}"
LUAU_BIN="${LUAU_BIN:-$(command -v luau || true)}"

if [ "$#" -gt 0 ]; then
  FILES=("$@")
else
  # 样本目录 2026-10-24 起不再入库（应主要求裁剪大体积测试件）；默认无件可验时直接跳过。
  shopt -s nullglob
  FILES=("$ROOT"/artifacts/samples/obsucvim/suda-test/*.lua)
  shopt -u nullglob
  if [ ${#FILES[@]} -eq 0 ]; then echo "SKIP: 默认样本目录已裁剪，传入具体文件路径再校验" >&2; exit 0; fi
fi

if [ ! -x "${LUAU_COMPILE:-/nonexistent}" ] && [ ! -x "${LUAU_BIN:-/nonexistent}" ]; then
  echo "SKIP: 找不到 luau-compile / luau（设 LUAU_COMPILE= 或 LUAU_BIN=）" >&2
  exit 0
fi

pass=0; fail=0
for f in "${FILES[@]}"; do
  [ -f "$f" ] || { echo "MISSING   $f"; fail=$((fail+1)); continue; }
  name="$(basename "$f")"
  bytes=$(stat -c%s "$f" 2>/dev/null || stat -f%z "$f")

  if [ -x "${LUAU_COMPILE:-/nonexistent}" ]; then
    if err=$("$LUAU_COMPILE" --binary "$f" 2>&1 >/dev/null) && [ -z "$err" ]; then
      printf 'SYNTAX OK   %-24s %9s B\n' "$name" "$bytes"; pass=$((pass+1))
    else
      printf 'SYNTAX FAIL %-24s %s\n' "$name" "$(echo "$err" | head -1)"; fail=$((fail+1)); continue
    fi
  else
    # 没有 luau-compile：用 luau 跑一次 load 检查（脚本本身会执行，仅作兜底）
    if out=$("$LUAU_BIN" "$f" 2>&1); then
      printf 'LOAD   OK   %-24s %9s B  stdout=%s\n' "$name" "$bytes" "$(echo "$out" | head -1)"
      pass=$((pass+1))
    else
      printf 'LOAD   FAIL %-24s %s\n' "$name" "$(echo "$out" | head -1)"; fail=$((fail+1)); continue
    fi
  fi

  if [ "${RUN:-0}" = "1" ] && [ -x "${LUAU_BIN:-/nonexistent}" ]; then
    out=$("$LUAU_BIN" "$f" 2>&1); rc=$?
    first=$(echo "$out" | grep -v '^stacktrace:' | head -1)
    printf '            run rc=%-3s %s\n' "$rc" "$first"
  fi
done

echo
echo "syntax: $pass ok, $fail failed"
[ "$fail" -eq 0 ]
