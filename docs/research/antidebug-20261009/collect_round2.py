#!/usr/bin/env python3
"""Summarize second-round evidence without changing first-round tests/results."""
import json
from pathlib import Path
import statistics
from sweep import integrity

HERE = Path(__file__).resolve().parent


def parse(name):
    text = (HERE / (name + '.log')).read_text()
    rows = [line.split('\t') for line in text.splitlines()]
    return {
        'passes': [r[1] for r in rows if r[0] == 'PASS'],
        'counterexamples': [r[1] for r in rows if r[0] == 'COUNTEREXAMPLE'],
        'benchmarks': [[float(x) for x in r[1:]] for r in rows if r[0] == 'BENCH'],
        'summary': [r[1:] for r in rows if r[0] == 'SUMMARY'],
    }


def main():
    names = ['inline-interval', 'boundary-order', 'completion-forgery',
             'primitive-contract', 'lease-cost', 'lease-cost-branch']
    results = {name: parse(name) for name in names}
    inline = results['inline-interval']['benchmarks']
    assert len(results['inline-interval']['passes']) == 19
    assert len(results['inline-interval']['counterexamples']) == 7
    assert len(inline) == 9
    perf = {'inline_object_ratio_median': statistics.median(row[-1] for row in inline)}
    for name in ['lease-cost', 'lease-cost-branch']:
        rows = results[name]['benchmarks']
        assert len(rows) == 12
        perf[name] = {
            'numeric_ratio_median': statistics.median(row[-2] for row in rows),
            'object_ratio_median': statistics.median(row[-1] for row in rows),
        }
    state = json.loads((HERE / 'extended-status.json').read_text())
    result = {
        'scope': 'foreground independent models + read-only source-layout study; not production integration',
        'bench_environment': 'background seed sweep remained running; no CPU isolation; no whole-VM performance claim',
        'experiments': results,
        'performance': perf,
        'slot_audit': json.loads((HERE / 'slot-audit-results.json').read_text()),
        'background_snapshot': state,
        'file_integrity': integrity(),
        'decision': 'not admitted; narrowed interval-only candidate lacks provenance/result integrity and integrated nonregression evidence',
    }
    assert not result['file_integrity']['changed']
    (HERE / 'round2-evidence.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(perf, indent=2))
    print('Background:', {k:state[k] for k in ['state', 'elapsed_seconds', 'attempted', 'passed', 'failed']})
    print('Source integrity:', result['file_integrity'])


if __name__ == '__main__':
    main()
