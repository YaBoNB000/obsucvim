#!/usr/bin/env python3
"""Rerun the exact lease experiment with failure-only branches instead of
normal-path assert calls. A benchmark control, not a production optimization.
Keeps the original source/log to avoid selecting only a favorable result.
"""
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
replacements = {
    'assert(E.phase == 0)': 'if E.phase ~= 0 then error("phase", 0) end',
    'assert(E.seq == expected and E.phase == 1 and E.pc == pc and E.family == family)':
        'if E.seq ~= expected or E.phase ~= 1 or E.pc ~= pc or E.family ~= family then error("state", 0) end',
    'assert(E.args.n == n and EQ(E.args, args) and EQ(E.f, f))':
        'if E.args.n ~= n or not EQ(E.args, args) or not EQ(E.f, f) then error("identity", 0) end',
    'if mode == 2 then assert(EQ(E.ticket, ticket)); E.ticket = nil end':
        'if mode == 2 then if not EQ(E.ticket, ticket) then error("ticket", 0) end; E.ticket = nil end',
}
text = (HERE / 'lease_cost.luau').read_text()
for old, new in replacements.items():
    assert text.count(old) == 1, old
    text = text.replace(old, new)
work = HERE / 'work/branch-benchmark'
work.mkdir(parents=True, exist_ok=True)
path = work / 'lease_cost_branch.luau'
path.write_text(text)
run = subprocess.run([str(ROOT / '.tools/bin/luau'), str(path)], capture_output=True, timeout=30)
(HERE / 'lease-cost-branch.log').write_bytes(run.stdout + run.stderr)
print(run.stdout.decode())
if run.returncode:
    print(run.stderr.decode())
raise SystemExit(run.returncode)
