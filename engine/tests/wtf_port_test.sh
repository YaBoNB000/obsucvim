#!/bin/bash
# WTF-PORT Phase 1 regression: --env-check battery (envcheck.rs).
#
# Positive: clean runs stay output-equivalent; default builds stay
# byte-identical to the committed examples; same seed is deterministic.
# Negative: each check group has a targeted mock environment -- a wrong-
# semantics mock must NOT reproduce the source output (the difference
# folds into the SLT/MZD lanes and kills the run silently; there is no
# error-message oracle to grep for, by construction).
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
if [ ! -x "$TOOL" ]; then
	echo "error: missing $TOOL (cargo build --release first)" >&2
	exit 2
fi
if [ ! -x "$LUAU" ]; then
	echo "error: missing $LUAU (bash engine/tools/rebuild_tools.sh)" >&2
	exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
CANARY_SRC="$TMP/canary.lua"
cat > "$CANARY_SRC" <<'EOF'
local t = {}
for i = 1, 6 do t[i] = i * 7 end
local s = 0
for i = 1, 6 do s = s + t[i] end
print("EC-OK", s, #t, ("x"):rep(3))
EOF
EXPECTED="$($LUAU "$CANARY_SRC")"

fail=0
pass=0
ok() { pass=$((pass + 1)); }
bad() { fail=$((fail + 1)); echo "FAIL $1"; }

# --- 1. clean equivalence across seeds --------------------------------------
for seed in 42 4242 999999 20261011; do
	out="$TMP/clean_$seed.lua"
	if ! "$TOOL" --env-check --seed "$seed" "$CANARY_SRC" "$out" 2>"$TMP/err"; then
		bad "clean-build seed=$seed: $(head -1 "$TMP/err")"
		continue
	fi
	got="$(timeout 30 "$LUAU" "$out" 2>&1)"
	if [[ "$got" == "$EXPECTED" ]]; then ok; else bad "clean-run seed=$seed"; fi
done

# --- 2. determinism ----------------------------------------------------------
"$TOOL" --env-check --seed 42 "$CANARY_SRC" "$TMP/d1.lua"
"$TOOL" --env-check --seed 42 "$CANARY_SRC" "$TMP/d2.lua"
if cmp -s "$TMP/d1.lua" "$TMP/d2.lua"; then ok; else bad "determinism"; fi

# The generic guard keeps its default-on behavior, and both explicit switches
# remain accepted: --guard matches the default; --no-guard changes the output.
"$TOOL" --seed 42 "$CANARY_SRC" "$TMP/guard_default.lua"
"$TOOL" --guard --seed 42 "$CANARY_SRC" "$TMP/guard_on.lua"
if cmp -s "$TMP/guard_default.lua" "$TMP/guard_on.lua"; then ok; else bad "--guard should match the default"; fi
"$TOOL" --no-guard --seed 42 "$CANARY_SRC" "$TMP/guard_off.lua"
if cmp -s "$TMP/guard_default.lua" "$TMP/guard_off.lua"; then bad "--no-guard did not disable the guard"; else ok; fi

# Retired executor/key-host switches must fail as unknown options.
for flag in --bind-executor --no-bind-executor --no-executor-guard --no-key-host; do
	if "$TOOL" "$flag" "$CANARY_SRC" "$TMP/obsolete.lua" >/dev/null 2>&1; then
		bad "obsolete option still accepted: $flag"
	else
		ok
	fi
done
if "$TOOL" --key-host delta "$CANARY_SRC" "$TMP/obsolete.lua" >/dev/null 2>&1; then
	bad "obsolete option still accepted: --key-host"
else
	ok
fi

# --- 3. default output identity vs committed examples ------------------------
for name in basics multival; do
	"$TOOL" --seed 42 "$ROOT/tests/cases/$name.lua" "$TMP/default_$name.lua" 2>/dev/null
	if cmp -s "$TMP/default_$name.lua" "$ROOT/examples/$name.v15.luau.lua"; then
		ok
	else
		bad "default byte identity: $name"
	fi
done

# --- 4. mock-environment kill tests -----------------------------------------
# Build the canary artifact once; every mock harness wraps it in a cloned
# environment with exactly one check group broken. A NO-OP mock must
# reproduce the canary output first (harness sanity gate) -- only then do
# the kill-mocks count.
"$TOOL" --env-check --seed 42 "$CANARY_SRC" "$TMP/canary_artifact.lua" 2>/dev/null

mk_mock() {
	# $1 name, $2 mock-fragment path, $3 output harness path
	python3 "$ROOT/tests/wtf_port_mockgen.py" "$1" "$2" "$TMP/canary_artifact.lua" "$3"
}

run_mock() { # $1 name, $2 mock luau fragment -> asserts output != expected
	echo "$2" > "$TMP/frag.lua"
	mk_mock "$1" "$TMP/frag.lua" "$TMP/mock_$1.lua"
	if [ ! -s "$TMP/mock_$1.lua" ]; then
		bad "mock $1: harness build failed"
		return
	fi
	got="$(timeout 30 "$LUAU" "$TMP/mock_$1.lua" 2>&1)"
	if [[ "$got" == *"$EXPECTED"* ]]; then
		bad "mock $1: canary output SURVIVED (check group not wired)"
	else
		ok
	fi
}

# sanity gate: an UNMODIFIED cloned environment must reproduce the output
echo '' > "$TMP/frag.lua"
mk_mock noop "$TMP/frag.lua" "$TMP/mock_noop.lua"
noop_got="$(timeout 30 "$LUAU" "$TMP/mock_noop.lua" 2>&1)"
if [[ "$noop_got" == *"$EXPECTED"* ]]; then
	ok
else
	bad "noop mock must reproduce canary output (harness broken): $noop_got"
fi

# bit32 wrong value (arith group)
run_mock bit32-wrong 'do local c, real = copylib("bit32")
c.bxor = function() return 12345 end
end'

# buffer big-endian writes (buffer group, LE layout check)
run_mock buffer-bigendian 'do local c, real = copylib("buffer")
c.writeu16 = function(b, o, v) real.writeu8(b, o, v // 256) real.writeu8(b, o + 1, v % 256) end
c.writeu32 = function(b, o, v) real.writeu8(b, o, v // 16777216) real.writeu8(b, o + 1, v // 65536 % 256) real.writeu8(b, o + 2, v // 256 % 256) real.writeu8(b, o + 3, v % 256) end
end'

# buffer function missing (envcheck errors -> PCAL false -> constant poison)
run_mock buffer-missing 'do local c = copylib("buffer")
c.copy = nil
end'

# table.freeze no-op (freeze group)
run_mock freeze-noop 'do local c, real = copylib("table")
c.freeze = function(t) return t end
end'

# loadstring always compiles (compile group)
run_mock loadstring-ok 'ENV.loadstring = function() return function() end end'

# coroutine.resume drops values (coroutine group)
run_mock coro-drop 'do local c, real = copylib("coroutine")
c.resume = function() return true end
end'

# mid-run swap of bit32.bxor (identity sweep)
run_mock identity-swap 'do local c, real = copylib("bit32")
local n = 0
c.bxor = function(a, b)
    n = n + 1
    if n > 1 then c.bxor = function() return 1 end end
    return real.bxor(a, b)
end
end'

# --- 5. no-oracle smoke: poisoned runs carry no fixed signature -------------
# (algebraic lanes only; different mocks must not share one exact error tail)
sig1="$(timeout 30 "$LUAU" "$TMP/mock_bit32-wrong.lua" 2>&1 | tail -c 120)"
sig2="$(timeout 30 "$LUAU" "$TMP/mock_freeze-noop.lua" 2>&1 | tail -c 120)"
if [[ -n "$sig1" && -n "$sig2" && "$sig1" != "$sig2" ]]; then
	ok
else
	bad "poison signatures must differ per mock (no shared oracle)"
fi

echo "wtf-port env-check regression: $pass PASS, $fail FAIL"
[ "$fail" -eq 0 ]
