#!/usr/bin/env python3
"""C5 research model: path-history-bound self-resealing block windows.

Design progression, each step falsified:
  V1  entry key derived from the moving path digest -> breaks even sequential
      revisits (the digest moves between encode and the next decode)
  V2  literal stored rolling key, shared vault -> correct, but (cipher,key)
      pair-copy is a capability: no path binding at all
  V3  per-frame copies + chain folding (digest, frame secret) -> correct and
      path/secret bound; honest boundaries measured separately

NOT the production VM; no production file is read or modified.
"""
import argparse
from dataclasses import dataclass, field
import json
from pathlib import Path
import random
import time

M = 268435456  # 2^28


def fold(*vals):
    out = 0
    for v in vals:
        out = (out * 48271 + v) % M
    return out


@dataclass
class Frame:
    fid: int
    secret: int
    digest: int = 11
    cipher: dict = field(default_factory=dict)
    chain: dict = field(default_factory=dict)


class NaiveDigestKey:
    """V1: entry key = fold(stored key, digest NOW)."""

    def __init__(self, master, build_keys):
        self.vault = {b: list(w) for b, w in master.items()}
        self.keys = dict(build_keys)
        self.digest = 11
        self.first_done = set()

    def visit(self, block, ops):
        if block in self.first_done:
            # "obvious" path binding: derive the entry key from the digest NOW
            key = fold(self.keys[block], self.digest)
        else:
            key = self.keys[block]  # literal build key on the first visit
            self.first_done.add(block)
        plain = [(w - key) % M for w in self.vault[block]]
        for op in ops:
            self.digest = fold(self.digest, op)
        self.keys[block] = fold(self.keys[block], self.digest)
        self.vault[block] = [(w + self.keys[block]) % M for w in plain]
        return plain


class RollingVault:
    """V2: literal stored rolling key, one shared vault (visit-bound only)."""

    def __init__(self, master, build_keys):
        self.vault = {b: list(w) for b, w in master.items()}
        self.keys = dict(build_keys)

    def visit(self, frame, block, ops):
        key = self.keys[block]
        plain = [(w - key) % M for w in self.vault[block]]
        for op in ops:
            frame.digest = fold(frame.digest, op)
        self.keys[block] = fold(self.keys[block], frame.digest)
        self.vault[block] = [(w + self.keys[block]) % M for w in plain]
        return plain


class FrameChainedVault:
    """V3: per-frame copies + per-frame chain folding digest and secret."""

    def __init__(self, master, build_keys):
        self.master = master
        self.build_keys = build_keys

    def entry_key(self, frame, block):
        return fold(frame.chain[block], frame.secret)

    def visit(self, frame, block, ops):
        if block not in frame.cipher:
            # First touch: the at-rest cipher is build-key encoded; the
            # frame-local chain (which folds digest+secret) starts AFTER
            # this visit's re-encode.
            frame.cipher[block] = list(self.master[block])
            frame.chain[block] = self.build_keys[block]
            key = self.build_keys[block]
        else:
            key = self.entry_key(frame, block)
        plain = [(w - key) % M for w in frame.cipher[block]]
        for op in ops:
            frame.digest = fold(frame.digest, op)
        frame.chain[block] = fold(frame.chain[block], frame.digest, frame.secret)
        out_key = self.entry_key(frame, block)
        frame.cipher[block] = [(w + out_key) % M for w in plain]
        return plain


def build_master(rng, blocks, length):
    master, keys = {}, {}
    for b in range(blocks):
        keys[b] = rng.randrange(1, M)
        master[b] = [rng.randrange(M) for _ in range(length)]
    return master, keys


def run(seed):
    start = time.monotonic()
    rng = random.Random(seed)
    master, build_keys = build_master(rng, 8, 24)
    counts = {}

    # --- V1: digest-derived entry key corrupts SEQUENTIAL revisits --------
    v1 = NaiveDigestKey(master, build_keys)
    p1 = v1.visit(3, [3, 4])
    p2 = v1.visit(3, [5])
    counts['v1_sequential_revisit_corrupts'] = int(p1 != p2)
    # sanity: first visit itself decoded correctly against the build encoding
    fresh = NaiveDigestKey(master, build_keys)
    p0 = fresh.visit(3, [])
    counts['v1_first_visit_correct'] = int(p0 == [(w - build_keys[3]) % M for w in master[3]])

    # --- V2: rolling literal key -------------------------------------------
    v2 = RollingVault(master, build_keys)
    frames = {0: Frame(0, rng.randrange(1, M))}
    seen = {}
    order = [0, 1, 2, 3, 2, 3, 3, 2, 1, 0]
    ok = True
    for block in order:
        p = v2.visit(frames[0], block, [block, 5])
        if block in seen and p != seen[block]:
            ok = False
        seen[block] = p
    counts['v2_sequential_stable'] = int(ok)
    # interleave two frames on the same block: shared literal key still works
    fa, fb = Frame(1, 3), Frame(2, 5)
    v2i = RollingVault(master, build_keys)
    a1 = v2i.visit(fa, 6, [1])
    b1 = v2i.visit(fb, 6, [2])
    a2 = v2i.visit(fa, 6, [3])
    counts['v2_interleave_stable'] = int(a1 == a2 == b1)
    # pair-copy oracle: (cipher, current key) decodes without any path
    key_copy = v2i.keys[6]
    cipher_copy = list(v2i.vault[6])
    oracle_plain = [(w - key_copy) % M for w in cipher_copy]
    honest_plain = v2i.visit(Frame(3, 9), 6, [4])
    counts['v2_pair_copy_oracle_succeeds'] = int(oracle_plain == honest_plain)

    # --- V3: per-frame chain + secret --------------------------------------
    v3 = FrameChainedVault(master, build_keys)
    fA = Frame(0, rng.randrange(1, M))
    seen = {}
    for block in [0, 1, 2, 3, 2, 3, 3, 2, 1, 0]:
        p = v3.visit(fA, block, [block, 5])
        if block in seen:
            assert p == seen[block], f'V3 revisit mismatch block {block}'
        seen[block] = p
    counts['v3_loops_stable'] = 1
    fB = Frame(1, rng.randrange(1, M))
    for round_ in range(50):
        a = v3.visit(fA, 2, [round_])
        b = v3.visit(fB, 2, [round_ + 1000])
        assert a == b == seen[2]
        v3.visit(fA, 3, [round_])
        v3.visit(fB, 3, [round_ + 1000])
    counts['v3_interleave_stable'] = 1

    def rec(depth):
        fr = Frame(100 + depth, rng.randrange(1, M))
        p1 = v3.visit(fr, 5, [depth])
        if depth:
            rec(depth - 1)
        p2 = v3.visit(fr, 5, [depth])
        assert p1 == p2

    rec(16)
    counts['v3_recursion_stable'] = 1

    # --- attacks on V3 ------------------------------------------------------
    fC = Frame(2, rng.randrange(1, M))
    v3.visit(fC, 6, [1, 2, 3])
    stale_cipher = list(fC.cipher[6])
    v3.visit(fC, 6, [4])  # chain advanced, cipher re-encoded
    now_plain = [(w - fold(fC.chain[6], fC.secret)) % M for w in fC.cipher[6]]
    stale_plain = [(w - fold(fC.chain[6], fC.secret)) % M for w in stale_cipher]
    counts['v3_stale_cipher_replay_rejected'] = int(stale_plain != now_plain)

    fD = Frame(3, rng.randrange(1, M))
    fD.cipher[6] = list(fC.cipher[6])
    fD.chain[6] = fC.chain[6]
    plainD = [(w - fold(fD.chain[6], fD.secret)) % M for w in fD.cipher[6]]
    counts['v3_cross_frame_pair_copy_rejected'] = int(plainD != now_plain)

    fE = Frame(4, fC.secret)
    fE.cipher[6] = list(fC.cipher[6])
    fE.chain[6] = fC.chain[6]
    plainE = [(w - fold(fE.chain[6], fE.secret)) % M for w in fE.cipher[6]]
    counts['v3_full_state_copy_accepted'] = int(plainE == now_plain)

    fF = Frame(5, rng.randrange(1, M))
    first_plain = v3.visit(fF, 7, [9])
    counts['v3_first_visit_build_key_only'] = int(
        first_plain == [(w - build_keys[7]) % M for w in master[7]])

    return {
        'scope': 'independent C5 model, NOT production VM; not admitted',
        'seed': seed,
        'elapsed_seconds': time.monotonic() - start,
        'counts': counts,
        'design_progression': [
            'V1 digest-derived entry key: the digest moves between encode and the next decode, so even sequential revisits decode garbage',
            'V2 literal rolling key on a shared vault: correct under interleave, but (cipher,key) pair-copy is a full capability — no path binding',
            'V3 per-frame copies + chain folding (digest, frame secret): the only variant of the three that is both correct and path/secret bound',
        ],
        'honest_boundaries': [
            'first-visit blocks are only build-key protected (same as today); path binding starts after the first re-encode',
            'full-state copy of a frame (cipher+chain+secret) still decodes: R004 physical limit',
            'static dump of untouched master blocks stays exactly as (un)protected as the current at-rest encoding',
            'per-frame copies cost memory proportional to frames x touched blocks',
        ],
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--seed', type=int, default=20261009)
    parser.add_argument('--output', type=Path,
                        default=Path(__file__).with_name('models3-results.json'))
    args = parser.parse_args()
    result = run(args.seed)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
