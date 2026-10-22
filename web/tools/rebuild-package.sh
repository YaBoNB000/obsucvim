#!/usr/bin/env bash
# 重建 Windows 便携包：以「上一个包」为底座（含 runtime/node.exe 与 node_modules），
# 用仓库里当前的程序文件覆盖，重算清单后重新打包。
#
# 用法（在 web/ 目录下）：
#   ./tools/rebuild-package.sh --base ../release-assets/obsucvim-web-win64/Obsucvim-Web-Windows-x64.zip
#
# 为什么要以旧包为底座：runtime/node.exe（Windows 版 Node）和 node_modules/nodemailer
# 在 Linux 沙箱里造不出来，但升级包必须带着它们。
#
# 脚本会强制把 .bat / .cmd / .ps1 写成 CRLF：LF 结尾的批处理在 cmd.exe 里
# 会出现 goto 跳错、括号块报错之类的怪问题。
set -euo pipefail

ROOT_REPO=$(cd "$(dirname "$0")/../.." && pwd)
WEB="$ROOT_REPO/web"
BASE_ZIP=""
OUT_ZIP="$WEB/releases/Obsucvim-Web-Windows-x64.zip"
PKG_DIR_NAME="obsucvim-web-win64"

while [ $# -gt 0 ]; do
  case "$1" in
    --base) BASE_ZIP="$2"; shift 2 ;;
    --out) OUT_ZIP="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

[ -n "$BASE_ZIP" ] || { echo "--base <previous zip> is required" >&2; exit 2; }
[ -f "$BASE_ZIP" ] || { echo "base zip not found: $BASE_ZIP" >&2; exit 2; }
[ -f "$WEB/dist/index.html" ] || { echo "web/dist/index.html missing: run npm run build first" >&2; exit 2; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

echo "[1/5] 解压底座包"
python3 - "$BASE_ZIP" "$WORK" <<'PY'
import sys, zipfile
zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])
PY
PKG="$WORK/$PKG_DIR_NAME"
[ -d "$PKG" ] || { echo "base zip layout unexpected" >&2; exit 2; }

echo "[2/5] 覆盖当前程序文件"
cp "$WEB/dist/index.html" "$PKG/dist/index.html"
for f in "$WEB"/tools/*.mjs "$WEB"/tools/*.luau "$WEB"/tools/obsucvim.wasm; do
  [ -f "$f" ] || continue
  case "$(basename "$f")" in test-*) continue ;; esac   # 测试脚本不随包发布
  cp "$f" "$PKG/tools/$(basename "$f")"
done
cp "$WEB/SCRIPT-KEYS.md" "$PKG/SCRIPT-KEYS.md"
cp "$WEB/docs/HOSTING-V2-PLAN.md" "$PKG/HOSTING-V2-PLAN.md"
cp "$WEB/deploy/README-Windows.md" "$PKG/README-Windows.md"
cp "$WEB/deploy/README-UPDATE.md" "$PKG/README-UPDATE.md"
cp "$WEB/deploy/Start-Obsucvim.bat" "$WEB/deploy/Start-Obsucvim.cmd" "$WEB/deploy/Start-Obsucvim.ps1" "$PKG/"
cp "$WEB/deploy/Update-Obsucvim.ps1" "$PKG/Update-Obsucvim.ps1"
cp "$WEB/deploy/self-update.mjs" "$PKG/tools/self-update.mjs"

echo "[3/5] 统一换行符（bat/cmd/ps1 -> CRLF，其余 -> LF）并刷新 BUILD-INFO"
python3 - "$PKG" <<'PY'
import datetime, pathlib, re, sys
root = pathlib.Path(sys.argv[1])
text_suffix = {'.bat': 'crlf', '.cmd': 'crlf', '.ps1': 'crlf', '.mjs': 'lf', '.md': 'lf', '.txt': 'lf', '.luau': 'lf', '.json': 'lf', '.js': 'lf'}
changed = 0
for p in root.rglob('*'):
    if not p.is_file():
        continue
    mode = text_suffix.get(p.suffix.lower())
    if not mode:
        continue
    data = p.read_bytes()
    if data[:3] == b'\xef\xbb\xbf':
        data = data[3:]
    text = data.decode('utf-8')
    normalized = text.replace('\r\n', '\n').replace('\r', '\n')
    if mode == 'crlf':
        normalized = normalized.replace('\n', '\r\n')
    out = normalized.encode('utf-8')
    if out != data:
        p.write_bytes(out)
        changed += 1
info = root / 'BUILD-INFO.txt'
if info.exists():
    s = info.read_text(encoding='utf-8').replace('\r\n', '\n')
    now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    s = re.sub(r'^Build date: .*$', f'Build date: {now}', s, flags=re.M)
    info.write_bytes(s.encode('utf-8'))
print(f'    {changed} 个文本文件已规范化换行')
PY

echo "[4/5] 重算 SHA256SUMS.txt"
python3 - "$PKG" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
lines = []
for p in sorted(root.rglob('*'), key=lambda q: q.relative_to(root).as_posix()):
    if not p.is_file() or p.name == 'SHA256SUMS.txt':
        continue
    rel = './' + p.relative_to(root).as_posix()
    lines.append(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {rel}')
(root / 'SHA256SUMS.txt').write_bytes(('\n'.join(lines) + '\n').encode('utf-8'))
print(f'    {len(lines)} 个文件')
PY

echo "[5/5] 打包"
mkdir -p "$(dirname "$OUT_ZIP")"
python3 - "$WORK" "$PKG_DIR_NAME" "$OUT_ZIP" <<'PY'
import os, sys, zipfile
work, name, out = sys.argv[1], sys.argv[2], sys.argv[3]
root = os.path.join(work, name)
files = []
for dirpath, dirnames, filenames in os.walk(root):
    dirnames.sort()
    for f in sorted(filenames):
        full = os.path.join(dirpath, f)
        files.append((full, os.path.relpath(full, work).replace(os.sep, '/')))
with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as z:
    z.writestr(name + '/', '')
    for full, rel in files:
        z.write(full, rel)
print(f'    {len(files) + 1} 个条目，{os.path.getsize(out)} 字节')
PY

HASH=$(sha256sum "$OUT_ZIP" | cut -d' ' -f1)
printf '%s  %s\n' "$HASH" "$(basename "$OUT_ZIP")" > "$OUT_ZIP.sha256"
echo
echo "包：$OUT_ZIP"
echo "SHA-256：$HASH"
