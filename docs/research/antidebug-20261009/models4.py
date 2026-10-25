#!/usr/bin/env python3
"""C5 round-2 falsification: chain collisions at hot-block scale and
partial-observation attacker economics. Independent model, not the VM."""
import argparse
import json
from pathlib import Path
import random
import time

M = 28  # 2^28 chain space


def fold(*vals):
    out = 0
    for v in vals:
        out = (out * 48271 + v) % (1 << M)
    return out


def chain_collision_scan(seed, visits, n_blocks):
    """Walk per-block chains at R064-like hot-loop scale; count first collisions.

    digest advances globally with quasi-random ops (path), chains update only
    when their block is visited. This is the same V3 update as models3.py.
    """
    rng = random.Random(seed)
    space = 1 << M
    secret = rng.randrange(1, space)
    digest = 11
    chains = [rng.randrange(1, space) for _ in range(n_blocks)]
    seen = [set() for _ in range(n_blocks)]
    for c in chains:
        pass
    first_collision = None
    collisions = 0
    for step in range(visits):
        b = rng.randrange(n_blocks)  # hot distribution: uniform over few blocks
        digest = fold(digest, b * 131 + step % 97)
        chains[b] = fold(chains[b], digest, secret)
        key = fold(chains[b], secret)
        if key in seen[b]:
            collisions += 1
            if first_collision is None:
                first_collision = step
        else:
            seen[b].add(key)
    # expected birthday collisions for the per-block visit counts
    return {
        'visits': visits,
        'blocks': n_blocks,
        'collisions': collisions,
        'first_collision_step': first_collision,
        'analytic_expected_collisions': (
            (visits / n_blocks) ** 2 / (2 * (1 << M)) * n_blocks
        ),
    }


def counter_variant_scan(seed, visits, n_blocks):
    """Counter folded into the key. NOTE: cross-block key equality is
    irrelevant (different ciphers), so 'seen' must be PER BLOCK. The earlier
    global-set version of this scan overcounted by mixing blocks."""
    rng = random.Random(seed)
    space = 1 << M
    secret = rng.randrange(1, space)
    digest = 11
    chains = [rng.randrange(1, space) for _ in range(n_blocks)]
    counters = [0] * n_blocks
    seen = [set() for _ in range(n_blocks)]
    collisions = 0
    for step in range(visits):
        b = rng.randrange(n_blocks)
        digest = fold(digest, b * 131 + step % 97)
        counters[b] += 1
        chains[b] = fold(chains[b], digest, secret)
        key = fold(chains[b], secret, counters[b])
        if key in seen[b]:
            collisions += 1
        else:
            seen[b].add(key)
    return {'visits': visits, 'collisions': collisions,
            'note': 'per-block key collisions; counter does NOT restore uniqueness inside 2^28'}


def tag_variant_scan(seed, visits, n_blocks):
    """Per-visit tag = fold(chain, secret, count) stored with the cipher.
    A stale replay (old cipher + old tag) is accepted only if BOTH the key
    collides AND the stored tag matches the currently expected tag; the
    counter makes expected tags differ per visit, so acceptance should be 0."""
    rng = random.Random(seed)
    space = 1 << M
    secret = rng.randrange(1, space)
    digest = 11
    chains = [rng.randrange(1, space) for _ in range(n_blocks)]
    counters = [0] * n_blocks
    key_collisions = 0
    stale_accepted = 0
    for step in range(visits):
        b = rng.randrange(n_blocks)
        digest = fold(digest, b * 131 + step % 97)
        counters[b] += 1
        chains[b] = fold(chains[b], digest, secret)
        key = fold(chains[b], secret)
        tag = fold(chains[b], secret, counters[b])
        # replay any earlier visit of this block whose key equals the
        # current key: the old stored tag carries the old counter value
        # (a chain collision implies the same chain, but the counter moved)
        # -- model the attacker holding all stale (cipher, tag) pairs.
        # A stale pair is accepted iff key matches AND its tag equals the
        # currently expected tag; different counter => different tag.
        # We count key collisions explicitly and tag collisions among them.
        key_collisions += 1 if False else 0
    # direct pairwise analysis over the recorded walk
    rng = random.Random(seed)
    digest = 11
    chains = [rng.randrange(1, space) for _ in range(n_blocks)]
    counters = [0] * n_blocks
    history = [[] for _ in range(n_blocks)]  # (chain, count) per visit
    for step in range(visits):
        b = rng.randrange(n_blocks)
        digest = fold(digest, b * 131 + step % 97)
        counters[b] += 1
        chains[b] = fold(chains[b], digest, secret)
        history[b].append((chains[b], counters[b]))
    for b in range(n_blocks):
        seen = {}
        for chain, count in history[b]:
            key = fold(chain, secret)
            if key in seen:
                key_collisions += 1
                for old_count in seen[key]:
                    if fold(chain, secret, old_count) == fold(chain, secret, count):
                        stale_accepted += 1
                seen[key].append(count)
            else:
                seen[key] = [count]
    return {'visits': visits, 'key_collisions': key_collisions,
            'stale_replays_accepted_with_tag': stale_accepted}


def partial_observation_attack(seed, rounds, observe_fraction):
    """V3 frames under an attacker who reads the FULL frame state once at T0
    and then snapshots ciphers at the end, but only observes a fraction of the
    path events. Decode success = blocks whose final cipher the attacker can
    decode by simulating chain evolution from the observed events."""
    rng = random.Random(seed)
    space = 1 << M
    secret = rng.randrange(1, space)
    master_key = rng.randrange(1, space)
    blocks = 12
    plain = {b: [rng.randrange(space) for _ in range(8)] for b in range(blocks)}
    chain = {b: master_key for b in range(blocks)}
    cipher = {b: [(w + master_key) % space for w in plain[b]] for b in range(blocks)}
    touched = set()
    digest = 11

    def key_of(b):
        return fold(chain[b], secret)

    def visit(b, op):
        nonlocal digest
        k = master_key if b not in touched else key_of(b)
        words = [(w - k) % space for w in cipher[b]]
        touched.add(b)
        assert words == plain[b]
        digest = fold(digest, op)
        chain[b] = fold(chain[b], digest, secret)
        cipher[b] = [(w + key_of(b)) % space for w in plain[b]]

    log = []
    for r in range(rounds):
        b = rng.randrange(blocks)
        op = rng.randrange(1, space)
        visit(b, op)
        log.append((b, op))

    # attacker: full state at T0 is simulated by knowing secret+chain+digest
    # BEFORE the log; they then observe only a fraction of events.
    a_secret, a_digest = secret, 11
    a_chain = {b: master_key for b in range(blocks)}
    for (b, op) in log:
        if rng.random() < observe_fraction:
            a_digest = fold(a_digest, op)
            a_chain[b] = fold(a_chain[b], a_digest, a_secret)
    decoded = sum(
        1 for b in range(blocks)
        if [(w - fold(a_chain[b], a_secret)) % space for w in cipher[b]] == plain[b]
    )
    return {'rounds': rounds, 'observe_fraction': observe_fraction,
            'blocks_decoded': decoded, 'blocks_total': blocks}


def main():
    start = time.monotonic()
    rng_seed = 20261009
    result = {
        'scope': 'C5 round-2 falsification; independent model; not admitted',
        'chain_scan': [chain_collision_scan(rng_seed + s, 500_000, 8) for s in range(3)],
        'counter_variant': [counter_variant_scan(rng_seed + s, 500_000, 8) for s in range(3)],
        'tag_variant': [tag_variant_scan(rng_seed + s, 300_000, 8) for s in range(3)],
        'partial_observation': [
            partial_observation_attack(rng_seed, 2000, frac)
            for frac in (1.0, 0.999, 0.99, 0.9, 0.5)
        ],
        'elapsed_seconds': time.monotonic() - start,
    }
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
