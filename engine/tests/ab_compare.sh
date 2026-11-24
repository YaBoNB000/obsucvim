#!/usr/bin/env bash
# Reproducible Same-Input / Same-Seed A/B Comparison Harness (P2).
#
# Checks out the baseline commit (`f583c2c3c9c66770c38169dc6547d10c85025555` by
# default) into an isolated temporary `git worktree` under `/tmp`, builds its
# production release binary without touching the current working branch, removes
# the temporary worktree immediately, builds the current working-tree release
# binary, and runs `ab_compare.py` across 10 corpora x 6 seeds x 4 modes x 3 rounds.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"
export PATH="$REPO/.tools/bin:$HOME/.cargo/bin:$PATH"

BASELINE_REF="${AB_BASELINE_REF:-f583c2c3c9c66770c38169dc6547d10c85025555}"
ROUNDS="${AB_ROUNDS:-3}"
JSON_OUT="${AB_JSON_OUT:-$ROOT/tests/results/ab_baseline_f583c2c3.json}"
MD_OUT="${AB_MD_OUT:-$ROOT/tests/results/ab_baseline_f583c2c3.md}"

BASELINE_SHA="$(git -C "$REPO" rev-parse "${BASELINE_REF}^{commit}")"
CANDIDATE_SHA="$(git -C "$REPO" rev-parse HEAD)"
BASELINE_CACHE_DIR="$ROOT/target/ab_baseline_${BASELINE_SHA}"
BASELINE_CACHE_BIN="$BASELINE_CACHE_DIR/obsucvim"

TMP_DIR="$(mktemp -d /tmp/obsucvim_ab_bins.XXXXXX)"
WT_DIR="$(mktemp -d /tmp/obsucvim_ab_wt.XXXXXX)"
rm -rf "$WT_DIR"

cleanup() {
	if [ -d "$WT_DIR" ]; then
		git -C "$REPO" worktree remove --force "$WT_DIR" >/dev/null 2>&1 || rm -rf "$WT_DIR"
		git -C "$REPO" worktree prune >/dev/null 2>&1 || true
	fi
	rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if [ "${AB_REBUILD_BASELINE:-0}" = "1" ] || [ ! -x "$BASELINE_CACHE_BIN" ]; then
	echo "[ab] creating temporary detached worktree at baseline $BASELINE_SHA..."
	git -C "$REPO" worktree add --detach "$WT_DIR" "$BASELINE_SHA" >/dev/null
	echo "[ab] building baseline release binary..."
	cargo build --release --manifest-path "$WT_DIR/engine/Cargo.toml" --quiet
	mkdir -p "$BASELINE_CACHE_DIR"
	cp "$WT_DIR/engine/target/release/obsucvim" "$BASELINE_CACHE_BIN"
	echo "[ab] removing temporary baseline worktree..."
	git -C "$REPO" worktree remove --force "$WT_DIR" >/dev/null
	git -C "$REPO" worktree prune >/dev/null
else
	echo "[ab] using cached baseline binary for $BASELINE_SHA ($BASELINE_CACHE_BIN)"
fi
cp "$BASELINE_CACHE_BIN" "$TMP_DIR/obsucvim_baseline"

echo "[ab] building candidate release binary..."
cargo build --release --manifest-path "$ROOT/Cargo.toml" --quiet
cp "$ROOT/target/release/obsucvim" "$TMP_DIR/obsucvim_candidate"

python3 "$ROOT/tests/ab_compare.py" \
	--baseline-bin "$TMP_DIR/obsucvim_baseline" \
	--candidate-bin "$TMP_DIR/obsucvim_candidate" \
	--baseline-sha "$BASELINE_SHA" \
	--candidate-sha "$CANDIDATE_SHA" \
	--rounds "$ROUNDS" \
	--json-out "$JSON_OUT" \
	--md-out "$MD_OUT"
