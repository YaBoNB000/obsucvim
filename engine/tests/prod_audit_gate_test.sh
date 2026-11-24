#!/usr/bin/env bash
# Mandatory Production vs Audit-Hooks Release Hygiene & Byte-Equivalence Gate.
#
# Enforces all Phase 17 §5.1 / P1-B invariants across ALL 12 `OBSUCVIM_*` env vars:
#   - File / path hooks: `OBSUCVIM_KEY_MANIFEST`, `OBSUCVIM_NOISE_AUDIT`,
#     `OBSUCVIM_PROFILE_AUDIT`, `OBSUCVIM_VM_TSRC`, `OBSUCVIM_VM_TSRC_PATH`
#   - Output-downgrade hook: `OBSUCVIM_VM_RAW` (early-returns unminified VM AST)
#   - Stderr diagnostic hooks: `OBSUCVIM_VM_DBG`, `OBSUCVIM_HBOOT_DBG`,
#     `OBSUCVIM_BUILD_DBG`, `OBSUCVIM_BIND_DBG`, `OBSUCVIM_FUSE_DBG`, `OBSUCVIM_TIMING`
#
# Invariants verified:
# 1. `PROD_BIN` (`cargo build --release`, without `audit-hooks`) MUST NOT export any
#    audit files, MUST NOT emit any stdout/stderr, and MUST produce byte-identical
#    minified output whether all 12 `OBSUCVIM_*` env vars (including `OBSUCVIM_VM_RAW=1`)
#    are set or completely unset.
# 2. `AUDIT_BIN` (`--features audit-hooks`, isolated in `target/audit/release/obsucvim`)
#    MUST exist (never silently skipped), MUST emit requested audit manifests + debug
#    stderr, MUST honor `OBSUCVIM_VM_RAW=1` when explicitly requested, and MUST emit
#    byte-for-byte identical obfuscated Lua output (`cmp -s`) to `PROD_BIN` across
#    `v15`, `v15 --no-guard`, `v15 --super-insn --bind-env`, and `vm` modes.
# 3. Built-in negative self-tests verify the gate fails closed on missing `AUDIT_BIN`,
#    leaked diagnostic files, or 1-byte output drift.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_TOOLS="$(cd "$ROOT/.." && pwd)/.tools/bin"
export PATH="$REPO_TOOLS:$HOME/.cargo/bin:$PATH"
PROD_BIN="${OBSUCVIM_PROD_BIN:-$ROOT/target/release/obsucvim}"
AUDIT_TARGET_DIR="${OBSUCVIM_AUDIT_TARGET_DIR:-$ROOT/target/audit}"
AUDIT_BIN="${OBSUCVIM_AUDIT_BIN:-$AUDIT_TARGET_DIR/release/obsucvim}"
NO_AUTOBUILD="${OBSUCVIM_NO_AUTOBUILD:-0}"
SKIP_SELFTEST="${OBSUCVIM_GATE_SKIP_SELFTEST:-0}"

ensure_bin() {
	local bin_path="$1"
	local label="$2"
	shift 2
	if [ ! -x "$bin_path" ]; then
		if [ "$NO_AUTOBUILD" = "1" ]; then
			echo "FAIL: missing mandatory $label binary at $bin_path (OBSUCVIM_NO_AUTOBUILD=1)" >&2
			exit 2
		fi
		echo "[gate] building $label binary: cargo $*"
		CARGO_NET_OFFLINE=true cargo "$@"
	elif [ "$NO_AUTOBUILD" != "1" ] && [ -d "$ROOT/src" ]; then
		if [ -n "$(find "$ROOT/src" "$ROOT/Cargo.toml" -newer "$bin_path" -print -quit 2>/dev/null)" ]; then
			echo "[gate] refreshing stale $label binary: cargo $*"
			CARGO_NET_OFFLINE=true cargo "$@"
		fi
	fi
	if [ ! -x "$bin_path" ]; then
		echo "FAIL: $label binary still not executable at $bin_path" >&2
		exit 2
	fi
}

ensure_bin "$PROD_BIN" "production-release" \
	build --release --manifest-path "$ROOT/Cargo.toml"

ensure_bin "$AUDIT_BIN" "audit-hooks-release" \
	build --release --features audit-hooks --target-dir "$AUDIT_TARGET_DIR" --manifest-path "$ROOT/Cargo.toml"

if [ "$(realpath "$PROD_BIN")" = "$(realpath "$AUDIT_BIN")" ]; then
	echo "FAIL: PROD_BIN and AUDIT_BIN point to the same file ($PROD_BIN)" >&2
	exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/src.lua" <<'EOF'
local function fib(n)
	local a, b = 0, 1
	for _ = 1, n do a, b = b, a + b end
	return a
end
print("GATE", fib(10))
EOF

run_pair_check() {
	local tag="$1"
	shift
	rm -f /tmp/vm_raw.lua /tmp/vm_tsrc.lua

	# Clean production baseline (zero OBSUCVIM_* env vars)
	env -u OBSUCVIM_KEY_MANIFEST -u OBSUCVIM_NOISE_AUDIT -u OBSUCVIM_PROFILE_AUDIT \
		-u OBSUCVIM_VM_RAW -u OBSUCVIM_VM_TSRC -u OBSUCVIM_VM_TSRC_PATH \
		-u OBSUCVIM_VM_DBG -u OBSUCVIM_HBOOT_DBG -u OBSUCVIM_BUILD_DBG \
		-u OBSUCVIM_BIND_DBG -u OBSUCVIM_FUSE_DBG -u OBSUCVIM_TIMING \
		"$PROD_BIN" "$@" "$TMP/src.lua" "$TMP/prod_clean_${tag}.lua" \
		>"$TMP/prod_clean_out_${tag}.txt" 2>"$TMP/prod_clean_err_${tag}.txt"

	# Production run with ALL 12 OBSUCVIM_* hooks set (including OBSUCVIM_VM_RAW=1)
	OBSUCVIM_KEY_MANIFEST="$TMP/prod_keys_${tag}.txt" \
	OBSUCVIM_NOISE_AUDIT="$TMP/prod_noise_${tag}.txt" \
	OBSUCVIM_PROFILE_AUDIT="$TMP/prod_prof_${tag}.txt" \
	OBSUCVIM_VM_RAW=1 \
	OBSUCVIM_VM_TSRC=1 \
	OBSUCVIM_VM_TSRC_PATH="$TMP/prod_tsrc_${tag}.lua" \
	OBSUCVIM_VM_DBG=1 \
	OBSUCVIM_HBOOT_DBG=1 \
	OBSUCVIM_BUILD_DBG=1 \
	OBSUCVIM_BIND_DBG=1 \
	OBSUCVIM_FUSE_DBG=1 \
	OBSUCVIM_TIMING=1 \
		"$PROD_BIN" "$@" "$TMP/src.lua" "$TMP/prod_out_${tag}.lua" \
		>"$TMP/prod_stdout_${tag}.txt" 2>"$TMP/prod_stderr_${tag}.txt"

	# Verify OBSUCVIM_VM_RAW=1 + all other env vars had ZERO effect on PROD_BIN output
	if ! cmp -s "$TMP/prod_clean_${tag}.lua" "$TMP/prod_out_${tag}.lua"; then
		echo "FAIL [$tag]: PROD_BIN output changed when OBSUCVIM_VM_RAW / audit env vars were set!" >&2
		exit 1
	fi

	for leaked in \
		"$TMP/prod_keys_${tag}.txt" \
		"$TMP/prod_noise_${tag}.txt" \
		"$TMP/prod_prof_${tag}.txt" \
		"$TMP/prod_tsrc_${tag}.lua" \
		"/tmp/vm_raw.lua" \
		"/tmp/vm_tsrc.lua"
	do
		if [ -e "$leaked" ]; then
			echo "FAIL [$tag]: production release binary exported sensitive file: $leaked" >&2
			exit 1
		fi
	done

	if [ -s "$TMP/prod_stdout_${tag}.txt" ]; then
		echo "FAIL [$tag]: production release binary emitted unexpected stdout:" >&2
		cat "$TMP/prod_stdout_${tag}.txt" >&2
		exit 1
	fi

	if [ -s "$TMP/prod_stderr_${tag}.txt" ]; then
		echo "FAIL [$tag]: production release binary emitted debug/timing stderr:" >&2
		cat "$TMP/prod_stderr_${tag}.txt" >&2
		exit 1
	fi

	# Audit-hooks run with all non-mutating diagnostic hooks enabled
	OBSUCVIM_KEY_MANIFEST="$TMP/audit_keys_${tag}.txt" \
	OBSUCVIM_NOISE_AUDIT="$TMP/audit_noise_${tag}.txt" \
	OBSUCVIM_PROFILE_AUDIT="$TMP/audit_prof_${tag}.txt" \
	OBSUCVIM_VM_TSRC=1 \
	OBSUCVIM_VM_TSRC_PATH="$TMP/audit_tsrc_${tag}.lua" \
	OBSUCVIM_VM_DBG=1 \
	OBSUCVIM_HBOOT_DBG=1 \
	OBSUCVIM_BUILD_DBG=1 \
	OBSUCVIM_BIND_DBG=1 \
	OBSUCVIM_FUSE_DBG=1 \
	OBSUCVIM_TIMING=1 \
		"$AUDIT_BIN" "$@" "$TMP/src.lua" "$TMP/audit_out_${tag}.lua" \
		>"$TMP/audit_stdout_${tag}.txt" 2>"$TMP/audit_stderr_${tag}.txt"

	if [ ! -s "$TMP/audit_tsrc_${tag}.lua" ]; then
		echo "FAIL [$tag]: audit-hooks binary failed to emit OBSUCVIM_VM_TSRC_PATH" >&2
		exit 1
	fi
	if [ ! -s "$TMP/audit_stderr_${tag}.txt" ]; then
		echo "FAIL [$tag]: audit-hooks binary failed to emit diagnostic stderr" >&2
		exit 1
	fi
	if [[ "$tag" == v15_* ]]; then
		if [ ! -s "$TMP/audit_keys_${tag}.txt" ] || [ ! -s "$TMP/audit_noise_${tag}.txt" ] || [ ! -s "$TMP/audit_prof_${tag}.txt" ]; then
			echo "FAIL [$tag]: audit-hooks binary failed to emit v15 audit manifests (keys/noise/prof)" >&2
			exit 1
		fi
	fi

	if ! cmp -s "$TMP/prod_out_${tag}.lua" "$TMP/audit_out_${tag}.lua"; then
		echo "FAIL [$tag]: audit-hooks changed emitted artifact bytes vs production build" >&2
		exit 1
	fi
	rm -f /tmp/vm_raw.lua /tmp/vm_tsrc.lua
}

run_pair_check "v15_s42"          --preset v15 --dialect luau --seed 42
run_pair_check "v15_s4242"        --preset v15 --dialect luau --seed 4242
run_pair_check "v15_ng_s42"       --preset v15 --no-guard --dialect luau --seed 42
run_pair_check "v15_super_bind"   --preset v15 --super-insn --bind-env roblox --dialect luau --seed 42
run_pair_check "vm_s42"           --preset vm --dialect luau --seed 42

# Verify OBSUCVIM_VM_RAW=1 IS active on AUDIT_BIN (returns unminified raw AST != minified output)
OBSUCVIM_VM_RAW=1 "$AUDIT_BIN" --preset vm --dialect luau --seed 42 \
	"$TMP/src.lua" "$TMP/audit_vm_raw.lua" >/dev/null 2>&1
if cmp -s "$TMP/prod_out_vm_s42.lua" "$TMP/audit_vm_raw.lua"; then
	echo "FAIL [vm_raw]: AUDIT_BIN did not honor OBSUCVIM_VM_RAW=1" >&2
	exit 1
fi

# Built-in negative self-tests to guarantee the gate itself fails closed.
if [ "$SKIP_SELFTEST" != "1" ]; then
	# 1. Missing AUDIT_BIN with NO_AUTOBUILD=1 must exit non-zero and never claim PASS.
	if OBSUCVIM_NO_AUTOBUILD=1 OBSUCVIM_GATE_SKIP_SELFTEST=1 \
		OBSUCVIM_PROD_BIN="$PROD_BIN" OBSUCVIM_AUDIT_BIN="$TMP/missing_audit_bin" \
		bash "$0" >"$TMP/neg1.out" 2>"$TMP/neg1.err"; then
		echo "FAIL [selftest-1]: gate succeeded despite missing AUDIT_BIN" >&2
		exit 1
	fi
	if grep -q "PROD AUDIT GATE PASS" "$TMP/neg1.out" "$TMP/neg1.err"; then
		echo "FAIL [selftest-1]: gate printed PASS despite missing AUDIT_BIN" >&2
		exit 1
	fi

	# 2. Simulated leaked diagnostic file on PROD_BIN must exit non-zero.
	cat > "$TMP/leaky_prod.sh" <<EOF
#!/usr/bin/env bash
[ -n "\${OBSUCVIM_KEY_MANIFEST:-}" ] && echo "LEAK=1" > "\$OBSUCVIM_KEY_MANIFEST"
exec "$PROD_BIN" "\$@"
EOF
	chmod +x "$TMP/leaky_prod.sh"
	if OBSUCVIM_NO_AUTOBUILD=1 OBSUCVIM_GATE_SKIP_SELFTEST=1 \
		OBSUCVIM_PROD_BIN="$TMP/leaky_prod.sh" OBSUCVIM_AUDIT_BIN="$AUDIT_BIN" \
		bash "$0" >"$TMP/neg2.out" 2>"$TMP/neg2.err"; then
		echo "FAIL [selftest-2]: gate succeeded despite leaked OBSUCVIM_KEY_MANIFEST" >&2
		exit 1
	fi

	# 3. Simulated 1-byte output drift on AUDIT_BIN must exit non-zero.
	cat > "$TMP/drifted_audit.sh" <<EOF
#!/usr/bin/env bash
if [ "\${OBSUCVIM_VM_RAW:-}" = "1" ]; then
	exec "$AUDIT_BIN" "\$@"
fi
"$AUDIT_BIN" "\$@"
out_file="\${!#}"
printf ' ' >> "\$out_file"
EOF
	chmod +x "$TMP/drifted_audit.sh"
	if OBSUCVIM_NO_AUTOBUILD=1 OBSUCVIM_GATE_SKIP_SELFTEST=1 \
		OBSUCVIM_PROD_BIN="$PROD_BIN" OBSUCVIM_AUDIT_BIN="$TMP/drifted_audit.sh" \
		bash "$0" >"$TMP/neg3.out" 2>"$TMP/neg3.err"; then
		echo "FAIL [selftest-3]: gate succeeded despite 1-byte output drift" >&2
		exit 1
	fi
fi

echo "PROD AUDIT GATE PASS: release binary exports 0 sensitive artifacts across all 12 OBSUCVIM_* hooks; mandatory audit-hooks build verified byte-identical across 5 build modes (+3 negative self-tests)"
