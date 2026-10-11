#!/usr/bin/env python3
"""Read-only production seed sweep and external receipt-adapter comparison.

This is NOT an integrated candidate. All temporary Lua goes in work/.
No sleeps to consume the budget. --seconds limits new work admission; a running
case may take up to its subprocess deadlines to finish. SIGTERM saves status.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import signal
import statistics
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
STOP = False
MARKER = b'__OBSUCVIM_RESEARCH_CPU__\t'


def on_stop(signum, frame):
    global STOP
    STOP = True


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def integrity():
    baseline = json.loads((HERE / 'baseline-files.json').read_text())
    changes = []
    for name, expected in baseline.items():
        path = ROOT / name
        if not path.is_file() or digest(path) != expected:
            changes.append(name)
    return {'checked': len(baseline), 'changed': changes}


def execute(command, timeout=30):
    start = time.monotonic()
    try:
        run = subprocess.run([str(x) for x in command], capture_output=True, timeout=timeout)
        return run.returncode, run.stdout, run.stderr, time.monotonic() - start
    except subprocess.TimeoutExpired as exc:
        return 124, exc.stdout or b'', exc.stderr or b'', time.monotonic() - start


def wrapped(body, protected):
    # No new loadstring: wrapper embeds unchanged generated body inside a
    # closure. Both A/B variants include that same additional closure.
    adapter = '''local function invoke(f)
  local owner, ticket = {}, {}
  local receipt = {owner, ticket, "postcall"}
  local pending = ticket
  local r = table.pack(pcall(f))
  local valid = receipt[1] == owner and receipt[2] == pending and receipt[3] == "postcall"
  pending = nil
  receipt[1], receipt[2], receipt[3] = nil, nil, nil
  if not valid then error("research-receipt-mismatch", 0) end
  if not r[1] then error(r[2], 0) end
  return table.unpack(r, 2, r.n)
end
''' if protected else '''local function invoke(f)
  local r = table.pack(pcall(f))
  if not r[1] then error(r[2], 0) end
  return table.unpack(r, 2, r.n)
end
'''
    return ('local function payload(...)\n' + body + '\nend\n' + adapter
            + 'local started = os.clock()\ninvoke(payload)\n'
            + 'print("__OBSUCVIM_RESEARCH_CPU__", os.clock() - started)\n')


def measured_output(output):
    before, sep, tail = output.rpartition(MARKER)
    if not sep:
        return output, None
    try:
        cpu = float(tail.strip())
    except ValueError:
        return output, None
    return before, cpu


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=float, default=18000)
    parser.add_argument('--max-cases', type=int, default=0)
    parser.add_argument('--seed', type=int, default=20261009)
    parser.add_argument('--prefix', default='extended')
    args = parser.parse_args()
    if not args.prefix.replace('-', '').replace('_', '').isalnum():
        raise SystemExit('invalid prefix')
    if args.seconds <= 0:
        raise SystemExit('seconds must be positive')
    for sig in [signal.SIGTERM, signal.SIGINT]:
        signal.signal(sig, on_stop)
    start = time.monotonic()
    work = HERE / 'work' / args.prefix
    work.mkdir(parents=True, exist_ok=True)
    tool = ROOT / 'obsucvim/target/release/obsucvim'
    luau = ROOT / '.tools/bin/luau'
    compile_ = ROOT / '.tools/bin/luau-compile'
    cases = sorted((ROOT / 'obsucvim/tests/cases').glob('*.lua'))
    cases.extend([ROOT / 'handoff/R064_7kb_src.lua', ROOT / 'handoff/R066_src.lua'])
    cases = [p for p in cases if p.is_file()]
    rng = random.Random(args.seed)
    logpath = HERE / (args.prefix + '.csv')
    statuspath = HERE / (args.prefix + '-status.json')
    if logpath.exists() or statuspath.exists():
        raise SystemExit('Refusing to overwrite evidence; choose a new --prefix')
    status = {
        'scope': 'unchanged production engine + external root-call adapter, NOT per-instruction integration',
        'state': 'running', 'planned_seconds': args.seconds, 'rng_seed': args.seed,
        'engine_sha256': digest(tool), 'platform': platform.platform(),
        'corpus_files': len(cases), 'attempted': 0, 'passed': 0, 'failed': 0,
        'failures': [], 'cpu_ratios': [], 'generation_sizes': [],
        'source_reference_failures': [],
    }
    def save(state='running'):
        current = dict(status)
        current['state'] = state
        current['elapsed_seconds'] = time.monotonic() - start
        ratios = current.pop('cpu_ratios')
        sizes = current.pop('generation_sizes')
        current['root_adapter_cpu_ratio_median'] = statistics.median(ratios) if ratios else None
        current['min_target_bytes'] = min(sizes) if sizes else None
        current['max_target_bytes'] = max(sizes) if sizes else None
        current['file_integrity'] = integrity()
        temp = statuspath.with_suffix('.tmp')
        temp.write_text(json.dumps(current, indent=2) + '\n')
        temp.replace(statuspath)
    expected = {}
    for path in cases:
        result = execute([luau, path])
        if result[0] != 0:
            status['source_reference_failures'].append(str(path.relative_to(ROOT)))
        expected[path] = result[:3]
    fields = ['index', 'case', 'seed', 'ok', 'reason', 'target_bytes', 'build_seconds',
              'direct_seconds', 'baseline_cpu', 'receipt_cpu', 'baseline_code', 'receipt_code']
    with logpath.open('w', newline='') as file:
        writer = csv.DictWriter(file, fieldnames=fields)
        writer.writeheader(); file.flush()
        save()
        # First pass covers every case before random sampling begins.
        iteration = 0
        while not STOP and time.monotonic() - start < args.seconds:
            if args.max_cases and iteration >= args.max_cases:
                break
            source = cases[iteration] if iteration < len(cases) else rng.choice(cases)
            seed = [42, 64075, 65065, 66066][iteration % 4] if iteration < len(cases) else rng.randrange(1, 2**64)
            target = work / 'target.lua'
            base = work / 'baseline.lua'
            protected = work / 'receipt.lua'
            name = str(source.relative_to(ROOT))
            row = {key: '' for key in fields}
            row.update(index=iteration, case=name, seed=seed, ok=False, reason='')
            try:
                rc, out, err, duration = execute([tool, '--seed', seed, source, target], timeout=90)
                row['build_seconds'] = duration
                if rc != 0:
                    raise RuntimeError('build:' + str(rc))
                row['target_bytes'] = target.stat().st_size
                status['generation_sizes'].append(row['target_bytes'])
                rc, _, err, _ = execute([compile_, target])
                if rc != 0:
                    raise RuntimeError('compile:' + str(rc))
                drc, dout, derr, dt = execute([luau, target])
                row['direct_seconds'] = dt
                ref_rc, ref_out, ref_err = expected[source]
                if (drc, dout) != (ref_rc, ref_out):
                    raise RuntimeError('production-output-difference:' + str(drc))
                body = target.read_text()
                base.write_text(wrapped(body, False))
                protected.write_text(wrapped(body, True))
                paths = [base, protected] if iteration % 2 == 0 else [protected, base]
                measurements = {}
                for path in paths:
                    code, stdout, stderr, _ = execute([luau, path])
                    content, cpu = measured_output(stdout)
                    measurements[path] = (code, content, cpu)
                    if code != drc or content != dout or cpu is None:
                        raise RuntimeError(('baseline-adapter' if path == base else 'receipt-adapter') + '-difference:' + str(code))
                bc, _, bt = measurements[base]
                pc, _, pt = measurements[protected]
                row.update(baseline_cpu=bt, receipt_cpu=pt, baseline_code=bc, receipt_code=pc)
                if bt > 0:
                    status['cpu_ratios'].append(pt / bt)
                row['ok'] = True
                status['passed'] += 1
            except Exception as exc:
                row['reason'] = str(exc)[:240]
                status['failed'] += 1
                if len(status['failures']) < 25:
                    status['failures'].append({'index': iteration, 'case': name, 'seed': seed, 'reason': row['reason']})
                    dest = work / ('failure-' + str(iteration))
                    dest.mkdir(exist_ok=True)
                    for path in [target, base, protected]:
                        if path.exists():
                            (dest / path.name).write_bytes(path.read_bytes())
            finally:
                status['attempted'] += 1
                writer.writerow(row); file.flush()
                iteration += 1
                if iteration % 25 == 0 or not row['ok']:
                    save()
                    print(json.dumps({k: status[k] for k in ['attempted', 'passed', 'failed']}), flush=True)
                for path in [target, base, protected]:
                    path.unlink(missing_ok=True)
        state = 'stopped' if STOP else ('completed-case-limit' if args.max_cases and iteration >= args.max_cases else 'completed-time-budget')
        save(state)
    print(statuspath.read_text(), flush=True)
    return 1 if status['failed'] or integrity()['changed'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
