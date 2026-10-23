#!/usr/bin/env bash
# Rebuild the WASI obfuscator from source and install it into web/tools/.
#
#   ./web/tools/build-engine-wasm.sh            # build, verify R067 anchor, install
#   ./web/tools/build-engine-wasm.sh --check    # verify the installed engine only
#   ./web/tools/build-engine-wasm.sh --force    # install even if the anchor mismatches
#
# Why this exists: obsucvim.wasm is a build artifact, it is NOT in git. The repo
# source is at R067 (full noise density); the binary that shipped in the Windows
# package was built later, from commit af7fc32 ("Reduce v15 output size"), which
# halved the noise and cut junk/decoy density. Running this script rebuilds the
# R067 engine byte-for-byte and proves it against the documented anchor.
#
# Requirements: rustc + cargo with the wasm32-wasip1 target.
#   rustup target add wasm32-wasip1
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENGINE_SRC="$ROOT/obsucvim"
TARGET="$ROOT/web/tools/obsucvim.wasm"
VERIFY="$ROOT/web/tools/verify-engine-version.mjs"

mode="build"
for arg in "$@"; do
  case "$arg" in
    --check) mode="check" ;;
    --force) force="1" ;;
    -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

if [ "$mode" = "check" ]; then
  node "$VERIFY" "$TARGET"
  exit $?
fi

command -v cargo >/dev/null 2>&1 || {
  echo "cargo not found. Install Rust (https://rustup.rs) and run: rustup target add wasm32-wasip1" >&2
  exit 2
}
command -v rustc >/dev/null 2>&1 || { echo "rustc not found" >&2; exit 2; }

echo "[1/4] target wasm32-wasip1"
rustup target add wasm32-wasip1 >/dev/null 2>&1 || true
rustc --version

echo "[2/4] build (this takes a few minutes)"
cd "$ENGINE_SRC"
cargo build --release --target wasm32-wasip1

BUILT="$ENGINE_SRC/target/wasm32-wasip1/release/obsucvim.wasm"
if [ ! -f "$BUILT" ]; then
  BUILT="$(find "$ENGINE_SRC/target" -name obsucvim.wasm -type f | head -1)"
fi
[ -f "$BUILT" ] || { echo "build produced no obsucvim.wasm" >&2; exit 1; }
echo "      $BUILT ($(stat -c%s "$BUILT") bytes)"

echo "[3/4] verify the R067 anchor"
cp "$BUILT" /tmp/obsucvim-built.wasm
if node "$VERIFY" /tmp/obsucvim-built.wasm | tee /tmp/obsucvim-verify.txt | grep -q "verdict  : R067"; then
  echo "      byte-for-byte R067 ✓"
else
  echo "      anchor mismatch:"
  sed 's/^/      /' /tmp/obsucvim-verify.txt
  if [ "${force:-}" != "1" ]; then
    echo "      refusing to install (use --force to override)" >&2
    exit 1
  fi
fi

echo "[4/4] install into web/tools/"
if [ -f "$TARGET" ]; then
  cp "$TARGET" "$TARGET.bak"
  echo "      previous engine backed up to $(basename "$TARGET").bak"
fi
cp "$BUILT" "$TARGET"
sha256sum "$TARGET"

cat <<'EOF'

Done. Next steps:
  1) node web/tools/verify-engine-version.mjs          # should say R067
  2) cd web && ./tools/rebuild-package.sh --base releases/Obsucvim-Web-Windows-x64.zip
  3) commit + push, then upgrade the server with Update-Obsucvim.ps1 -Sha256 <new hash>
  4) re-publish every script so the new engine's output is what users receive
EOF
