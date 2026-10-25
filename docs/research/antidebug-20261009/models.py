#!/usr/bin/env python3
"""Independent falsification models. These do NOT implement the production VM."""
import argparse
from dataclasses import dataclass
import json
import math
from pathlib import Path
import random
import time


@dataclass
class Receipt:
    owner: object
    ticket: object
    phase: str


class CallGate:
    """Reference ownership model, not cryptographic authentication."""
    def __init__(self):
        self.owner = object()
        self.pending = None

    def begin(self):
        if self.pending is not None:
            return None
        self.pending = object()
        return Receipt(self.owner, self.pending, "postcall")

    def finish(self, receipt):
        valid = (self.pending is not None and receipt.owner is self.owner
                 and receipt.ticket is self.pending and receipt.phase == "postcall")
        if valid:
            self.pending = None
        return valid


class EdgeGate:
    def __init__(self, cfg):
        self.cfg = cfg
        self.pc = 0
        self.seq = 0

    def step(self, seq, src, dst):
        if seq != self.seq or src != self.pc or dst not in self.cfg[src]:
            return False
        self.pc, self.seq = dst, self.seq + 1
        return True


def run(seed, rounds):
    start = time.monotonic()
    rng = random.Random(seed)
    counts = dict(clean_receipts=0, cross_frame_rejected=0, replay_rejected=0,
                  wrong_phase_rejected=0, stolen_live_receipt_accepted=0,
                  full_state_rollback_accepted=0, clean_edges=0,
                  illegal_edges_rejected=0, repeated_edge_event_rejected=0,
                  legal_wrong_branch_accepted=0, self_reported_pc_accepted=0,
                  integer_cases=0, naive_double_mod_mismatch=0,
                  split16_mod_mismatch=0, affine_forgery_accepted=0)
    cfg = {0: (1, 2), 1: (3,), 2: (3,), 3: (0,)}
    mod = 2**32
    for _ in range(rounds):
        # Legitimate random interleaving between eight independent frames.
        gates = [CallGate() for _ in range(8)]
        receipts = [gate.begin() for gate in gates]
        order = list(range(8)); rng.shuffle(order)
        for i in order:
            gate, receipt = gates[i], receipts[i]
            counts['wrong_phase_rejected'] += not gate.finish(Receipt(receipt.owner, receipt.ticket, 'call'))
            counts['cross_frame_rejected'] += not gates[(i + 1) % 8].finish(receipt)
            ok = gate.finish(receipt)
            assert ok
            counts['clean_receipts'] += ok
            counts['replay_rejected'] += not gate.finish(receipt)
        # Same-process read access can steal the current live receipt. It
        # succeeds without doing the computation supposedly authenticated.
        gate = CallGate(); live = gate.begin()
        counts['stolen_live_receipt_accepted'] += gate.finish(live)
        # Restore ALL reference state, not just the visible frame fields.
        saved_owner, saved_pending = gate.owner, gate.pending
        replay = gate.begin(); snapshot = (gate.owner, gate.pending)
        assert gate.finish(replay)
        gate.owner, gate.pending = snapshot
        counts['full_state_rollback_accepted'] += gate.finish(replay)
        gate.owner, gate.pending = saved_owner, saved_pending
        edge = EdgeGate(cfg)
        for _ in range(rng.randint(4, 24)):
            src, seq = edge.pc, edge.seq
            dst = rng.choice(cfg[src])
            bad = next(n for n in cfg if n not in cfg[src])
            counts['illegal_edges_rejected'] += not edge.step(seq, src, bad)
            assert edge.step(seq, src, dst)
            counts['clean_edges'] += 1
            counts['repeated_edge_event_rejected'] += not edge.step(seq, src, dst)
        # The application condition requires edge 0->1. A CFG-only checker
        # accepts 0->2 as well. It does not authenticate the predicate result.
        counts['legal_wrong_branch_accepted'] += EdgeGate(cfg).step(0, 0, 2)
        # Wrong design: both producers consume the attacker-modified PC.
        attacker_pc = rng.randrange(2**24)
        witness = (attacker_pc * 257 + 17) % mod
        counts['self_reported_pc_accepted'] += witness == (attacker_pc * 257 + 17) % mod
        a, b = rng.randrange(mod), rng.randrange(mod)
        expected = a * b % mod
        naive = int(float(a) * float(b) % float(mod))
        # Each intermediate below is < 2^34, hence exactly representable.
        a0, a1, b0, b1 = a % 65536, a // 65536, b % 65536, b // 65536
        split = int((float(a0) * b0 + ((float(a0) * b1 + float(a1) * b0) % 65536) * 65536) % mod)
        counts['integer_cases'] += 1
        counts['naive_double_mod_mismatch'] += naive != expected
        counts['split16_mod_mismatch'] += split != expected
        coeff = rng.randrange(1, mod, 2)
        x, y, delta = rng.randrange(mod), rng.randrange(mod), rng.randrange(1, mod)
        tag = (coeff * x + y) % mod
        x2, y2 = (x + delta) % mod, (y - coeff * delta) % mod
        counts['affine_forgery_accepted'] += tag == (coeff * x2 + y2) % mod and x2 != x
    assert counts['split16_mod_mismatch'] == 0
    assert counts['clean_receipts'] == 8 * rounds
    assert counts['cross_frame_rejected'] == 8 * rounds
    # These are intentional failures of the *security hypothesis*, not bugs
    # to hide by weakening tests.
    assert counts['stolen_live_receipt_accepted'] == rounds
    assert counts['full_state_rollback_accepted'] == rounds
    assert counts['legal_wrong_branch_accepted'] == rounds
    assert counts['affine_forgery_accepted'] == rounds
    return {
        'scope': 'independent models, NOT production VM security tests',
        'seed': seed, 'rounds': rounds, 'elapsed_seconds': time.monotonic() - start,
        'counts': counts,
        'counterexample_notes': {
            'counter16_wrap': 'A 16-bit sequence alone repeats after 65536 events; widening delays but cannot prevent full-state rollback.',
            'passive_observation': 'Reading the same trace without changing state produces the same gate decisions; no clock-free observer detector is created.',
            'nan_self_equality': math.nan == math.nan,
            'infinity_subtraction_is_nan': math.isnan(math.inf - math.inf),
            '2^53_plus_one_rounds_back': float(2**53) + 1 == float(2**53),
        },
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--seed', type=int, default=20261009)
    parser.add_argument('--rounds', type=int, default=20000)
    parser.add_argument('--output', type=Path, default=Path(__file__).with_name('model-results.json'))
    args = parser.parse_args()
    result = run(args.seed, args.rounds)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
