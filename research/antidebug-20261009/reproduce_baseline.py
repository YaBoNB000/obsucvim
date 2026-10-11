#!/usr/bin/env python3
"""Investigate a seed-dependent baseline discrepancy; NEVER edits engine/corpus."""
import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SEED = 10770986916448599986
TOOL = ROOT / 'obsucvim/target/release/obsucvim'
LUAU = ROOT / '.tools/bin/luau'
SOURCE = ROOT / 'obsucvim/tests/cases/stress_multival.lua'


def command(args):
    try:
        p = subprocess.run([str(x) for x in args], capture_output=True, timeout=15)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return 124, b'', b'timeout'


def main():
    start = time.monotonic()
    work = HERE / 'work/reproduce'
    work.mkdir(parents=True, exist_ok=True)
    rc, expected, _ = command([LUAU, SOURCE]); assert rc == 0
    rows = []
    modes = {'v15': [], 'v15-unminified': ['--no-minify'],
             'v15-no-string-seal': ['--no-vm-seal-strings'],
             'legacy-vm': ['--preset', 'vm'], 'low': ['--preset', 'low']}
    for mode, flags in modes.items():
        target = work / (mode + '.lua')
        rc, _, err = command([TOOL, '--seed', SEED, *flags, SOURCE, target])
        if rc:
            rows.append({'mode': mode, 'build_code': rc})
            continue
        results = []
        for _ in range(10):
            rc, actual, err = command([LUAU, target])
            results.append({'code': rc, 'matches': rc == 0 and actual == expected,
                            'stdout_sha256': hashlib.sha256(actual).hexdigest()})
        diff = ''.join(difflib.unified_diff(expected.decode().splitlines(True), actual.decode().splitlines(True), fromfile='source', tofile=mode))
        rows.append({'mode': mode, 'target_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
                     'runs': results, 'diff': diff})
    # Limited line-deletion reduction. Failing predicate specifically retains
    # the fwd1 vararg mismatch; syntax/dependency failures are NOT successes.
    original = SOURCE.read_text().splitlines(True)
    trialsrc, trialout = work / 'reduce-input.lua', work / 'reduce-output.lua'
    trials = 0
    def discrepancy(lines):
        nonlocal trials
        trials += 1
        trialsrc.write_text(''.join(lines))
        sr, so, _ = command([LUAU, trialsrc])
        if sr or b'fwd1:\t1\t2\t3\n' not in so:
            return False
        br, _, _ = command([TOOL, '--seed', SEED, trialsrc, trialout])
        if br:
            return False
        tr, to, _ = command([LUAU, trialout])
        return tr == 0 and b'fwd1:\t1\t2\t3\n' not in to and b'fwd1:' in to
    lines = original
    width = max(1, len(lines) // 2)
    while width >= 1 and trials < 160:
        index = 0
        while index < len(lines) and trials < 160:
            candidate = lines[:index] + lines[index + width:]
            if discrepancy(candidate):
                lines = candidate
            else:
                index += width
        width //= 2
    minimal = HERE / 'baseline-reduced.lua'
    minimal.write_text(''.join(lines))
    assert discrepancy(lines), 'must retain the observed discrepancy'
    reduced_expected = command([LUAU, minimal])[1].decode()
    reduced_actual = command([LUAU, trialout])[1].decode()
    result = {'scope': 'existing engine discrepancy discovered BEFORE candidate adapter execution',
              'seed': SEED, 'source': str(SOURCE.relative_to(ROOT)),
              'source_sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
              'modes': rows, 'reduction_trials': trials, 'original_lines': len(original),
              'reduced_lines': len(lines), 'reduced_source_stdout': reduced_expected,
              'reduced_target_stdout': reduced_actual,
              'root_cause': 'not established; no production fix attempted',
              'elapsed_seconds': time.monotonic() - start}
    (HERE / 'baseline-discrepancy.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'modes': [(row['mode'], sum(not x['matches'] for x in row.get('runs', []))) for row in rows],
                      'reduced_lines': len(lines), 'reduction_trials': trials, 'elapsed_seconds': result['elapsed_seconds']}, indent=2))


if __name__ == '__main__':
    main()
