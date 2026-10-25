#!/usr/bin/env python3
"""Read-only arithmetic layout check. Proposed slots are NOT allocated in VM."""
import hashlib
import json
from pathlib import Path
import re
import time

HERE = Path(__file__).resolve().parent
ROOT = next((q for q in HERE.parents if (q / 'engine' / 'Cargo.toml').is_file()), HERE.parent.parent)
TEMPLATE = ROOT / 'engine/src/vmgen/template.rs'


def main():
    started = time.monotonic()
    text = TEMPLATE.read_text()
    # Fail if the reviewed layout changes instead of silently auditing an
    # unrelated toy formula. These checks are not a Rust parser/proof.
    anchors = [
        'E[E.cb+{}+{}*E.cs]', 'family * 256, logical',
        'cb = 25 + (vd % 4096) * 2048', 'rb = 1305 + (vd % 4096) * 2048',
        '% 16 + 1) as u8', 'for _ci=0,8 do E[E.cb+{page}+_ci*E.cs]=nil end',
    ]
    assert all(a in text for a in anchors), 'review source layout again'
    layouts = 0
    collisions = []
    # vd spans 0..65520, but numerical slot locations depend only on vd%4096.
    for vmod in range(4096):
        for step in range(1, 17):
            cb, rb = 25 + vmod * 2048, 1305 + vmod * 2048
            used = {i: ('core', i) for i in range(1, 25)}
            for logical in range(4):
                position = rb + logical * step
                assert position not in used
                used[position] = ('return', logical)
            for family in range(1, 5):
                # Existing 0..8 plus exploratory slots 9..12.
                for logical in range(13):
                    position = cb + family * 256 + logical * step
                    if position in used:
                        collisions.append([vmod, step, family, logical, used[position]])
                    used[position] = (family, logical)
            layouts += 1
    assert not collisions
    # The limit is not "we can append arbitrary fields".
    overflow = []
    for family in range(1, 4):
        for step in range(1, 17):
            for logical in range(9, 33):
                if family * 256 + logical * step == (family + 1) * 256:
                    overflow.append({'family': family, 'step': step, 'logical': logical,
                                     'alias': f'family {family+1}, logical 0'})
    assert {'family': 1, 'step': 16, 'logical': 16, 'alias': 'family 2, logical 0'} in overflow
    # Same base is not a globally unique frame ID; E tables isolate frames.
    vd1, vd2 = 1, 4097
    cb1, cb2 = 25 + (vd1 % 4096) * 2048, 25 + (vd2 % 4096) * 2048
    assert cb1 == cb2
    result = {
        'scope': 'arithmetic namespace study, no implementation or security proof',
        'template_sha256': hashlib.sha256(TEMPLATE.read_bytes()).hexdigest(),
        'layouts': layouts,
        'existing_call_logical_indices': '0..8',
        'exploratory_call_logical_indices': '9..12',
        'collisions_with_reviewed_existing_numeric_fields': len(collisions),
        'old_cleanup_leaves_new_indices': [9, 10, 11, 12],
        'appending_too_many_fields_collision_examples': overflow,
        'base_is_not_owner_identity_example': {'vd': [vd1, vd2], 'shared_cb': cb1},
        'not_checked': ['all emitted/mangled field uses', 'future fields', 'gc lifetime',
                        'actual production execution with added fields', 'attacker resistance'],
        'elapsed_seconds': time.monotonic() - started,
    }
    (HERE / 'slot-audit-results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
