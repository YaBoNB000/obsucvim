#!/usr/bin/env python3
"""C6 model v2: environment RECORDER attacks vs defenses, corrected.

v1 had a circular boot_and_run (the program re-sealed with whatever key it
computed, so ANY fake environment passed). v2 separates the two different
jobs and falsifies each honestly:

  verification   decode key must equal a BUILD-TIME mirror -> only
                 constant challenges can be verified offline
  reseal binding runtime-random keys protect snapshots (C5 family), but
                 they authenticate NOTHING about the environment

Consequences tested here:
  P1  constant-probe verification: a one-pass recording defeats it
  P4  coverage: one missing probe of n silently garbles the key
  P3  stability tension (52 lesson): an unstable probe mirrored from one
      variant silently garbles a LEGIT user on another variant
  P2  dual-path internal consistency: kills pure replay of novel queries,
      but a SYMMETRIC fake (same answer for both paths) passes everything
  P5  stateful probe: kills stateless replay, passes with stateful fake

Independent model; NOT the production VM.
"""
import argparse
import json
from pathlib import Path
import random
import time

MOD = 1 << 28


def fold(*vals):
    out = 0
    for v in vals:
        out = (out * 48271 + v) % MOD
    return out


class RealEnv:
    def __init__(self, variant=0):
        self.variant = variant
        self.store = {}

    def stable(self, x):
        return (x * 2654435761 + 7) % (1 << 32)

    def stable2(self, x):
        # a second route with the same semantics (dual-path partner)
        y = x % (1 << 32)
        acc = 7
        for _ in range(1):
            acc = (acc + y * 2654435761) % (1 << 32)
        return acc

    def unstable(self, x):
        base = (x * 97 + 31) % 100000
        return base if self.variant == 0 else base + 1

    def put(self, k, v):
        self.store[k] = v

    def get(self, k):
        return self.store.get(k)


class Recorder:
    """In a REAL environment; records exact (fn, arg) -> answer pairs."""

    def __init__(self, env):
        self.env = env
        self.answers = {}

    def query(self, fn, arg):
        answer = getattr(self.env, fn)(arg) if isinstance(arg, int) or fn in ('get',) \
            else None
        # unify: support put(k,v) style via explicit API below
        return answer

    def record_call(self, fn, *args):
        answer = getattr(self.env, fn)(*args)
        self.answers[(fn, args)] = answer
        return answer


class ExactReplayer:
    """Answers only exact recorded queries (record-and-replay tool)."""

    def __init__(self, answers):
        self.answers = answers
        self.state = {}

    def call(self, fn, *args):
        return self.answers.get((fn, args))


class SymmetricFake:
    """Returns one recorded value for ANY argument of a known function,
    and the SAME value on both routes of a dual-path pair."""

    def __init__(self, answers):
        self.answers = answers
        self.state = {}

    def call(self, fn, *args):
        for (f, _), a in self.answers.items():
            if f == fn:
                return a
        return None


class StatefulReplayer:
    """Exact replay PLUS maintained state (a small emulator)."""

    def __init__(self, answers):
        self.answers = answers
        self.state = {}

    def call(self, fn, *args):
        if fn == 'put':
            self.state[args[0]] = args[1]
            return None
        if fn == 'get':
            return self.state.get(args[0])
        return self.answers.get((fn, args))


class CombinedFake:
    """Exact answers for recorded constants, PLUS a same-value fallback for
    any novel query in the stable/stable2 family. Models an attacker who has
    REVERSED the dual-path check and understands the pairing."""

    def __init__(self, answers):
        self.answers = answers

    def call(self, fn, *args):
        if (fn, args) in self.answers:
            return self.answers[(fn, args)]
        for (f, _), a in self.answers.items():
            if f in ('stable', 'stable2') and fn in ('stable', 'stable2'):
                return a
        return None


class SemanticEmulator:
    """Implements the probed semantics itself."""

    def __init__(self, variant=0):
        self.variant = variant
        self.state = {}

    def call(self, fn, *args):
        if fn == 'stable':
            return (args[0] * 2654435761 + 7) % (1 << 32)
        if fn == 'stable2':
            return (7 + (args[0] % (1 << 32)) * 2654435761) % (1 << 32)
        if fn == 'unstable':
            base = (args[0] * 97 + 31) % 100000
            return base if self.variant == 0 else base + 1
        if fn == 'put':
            self.state[args[0]] = args[1]
            return None
        if fn == 'get':
            return self.state.get(args[0])
        return None


class Program:
    """Build side mirrors the expected answers of CONSTANT probes only."""

    def __init__(self, plain, k_build, const_probes, mirror_env, extra_gate=None):
        self.k = k_build
        self.probes = const_probes
        self.expected = [mirror_env.call(fn, *args) for (fn, args) in const_probes]
        self.extra_gate = extra_gate  # runtime gate over nonce queries
        key = fold(k_build, *[a % MOD for a in self.expected])
        self.cipher = [(p + key) % MOD for p in plain]
        self.plain = plain

    def run(self, envish):
        answers = []
        for (fn, args) in self.probes:
            a = envish.call(fn, *args)
            answers.append(a if a is not None else 0)
        poison = False
        if self.extra_gate is not None:
            poison = self.extra_gate(envish)
        key = fold(self.k, *[a % MOD for a in answers])
        if poison:
            key = (key + 12345679) % MOD
        return [(w - key) % MOD for w in self.cipher]


def run(seed):
    start = time.monotonic()
    rng = random.Random(seed)
    plain = [rng.randrange(MOD) for _ in range(16)]
    k_build = rng.randrange(1, MOD)
    results = {}

    def ok(out):
        return out == plain

    real0 = RealEnv(0)

    # --- P1: one recorded pass defeats constant-probe verification -------
    probes = [('stable', (12345,)), ('stable', (999,)), ('unstable', (7,))]
    # NOTE: the unstable probe is mirrored from variant-0 here on purpose;
    # P1 only demonstrates the recording replay, P3 shows the FP fallout.
    rec = Recorder(real0)
    recording = {(fn, args): rec.record_call(fn, *args) for (fn, args) in probes}
    prog = Program(plain, k_build, probes, SemanticEmulator(variant=0))
    results['p1_exact_replay_succeeds'] = ok(prog.run(ExactReplayer(recording)))

    # --- P4: coverage — one missing probe of n garbles silently -----------
    partial = dict(list(recording.items())[:-1])
    results['p4_one_missing_probe_garbles'] = not ok(prog.run(ExactReplayer(partial)))

    # --- P3: stability tension (52 lesson) ---------------------------------
    prog_v0mirror = Program(plain, k_build, [('unstable', (7,))],
                            SemanticEmulator(variant=0))
    results['p3_variant0_legit_ok'] = ok(prog_v0mirror.run(SemanticEmulator(variant=0)))
    results['p3_variant1_legit_user_silent_garbage'] = not ok(
        prog_v0mirror.run(SemanticEmulator(variant=1)))
    results['p3_emulator_wrong_variant_also_garbles'] = not ok(
        Program(plain, k_build, [('unstable', (7,))],
                SemanticEmulator(variant=1)).run(SemanticEmulator(variant=0)))

    # --- P2: dual-path internal consistency on a NOVEL nonce --------------
    nonce = rng.randrange(1 << 30)

    def dual_gate(envish):
        a1 = envish.call('stable', nonce)
        a2 = envish.call('stable2', nonce)
        if a1 is None or a2 is None:
            return True  # unanswerable novel query -> poison
        return a1 != a2

    prog2 = Program(plain, k_build, probes, SemanticEmulator(variant=0), extra_gate=dual_gate)
    # exact replayer: novel nonce not in the recording -> poisoned
    results['p2_exact_replay_of_novel_query_fails'] = not ok(
        prog2.run(ExactReplayer(recording)))
    # symmetric fake WITHOUT family knowledge: dies (novel stable2 -> None,
    # or wrong constants via single-value policy) -- blind replay tooling
    results['p2_blind_symmetric_fake_fails'] = not ok(prog2.run(SymmetricFake(recording)))
    # combined fake WITH family knowledge: exact constants + same-value
    # fallback on both routes -> consistency holds -> passes (honest boundary)
    results['p2_combined_fake_with_pairing_knowledge_passes'] = ok(
        prog2.run(CombinedFake(recording)))
    # semantic emulator passes
    results['p2_semantic_emulator_passes'] = ok(prog2.run(SemanticEmulator(variant=0)))

    # --- P5: stateful probe -------------------------------------------------
    def state_gate(envish):
        v = rng.randrange(1 << 30)
        envish.call('put', 'm', v)
        return envish.call('get', 'm') != v

    prog5 = Program(plain, k_build, probes, SemanticEmulator(variant=0),
                    extra_gate=state_gate)
    results['p5_stateless_replay_fails'] = not ok(prog5.run(ExactReplayer(recording)))
    results['p5_stateful_fake_passes'] = ok(prog5.run(StatefulReplayer(recording)))
    results['p5_semantic_emulator_passes'] = ok(prog5.run(SemanticEmulator(variant=0)))

    return {
        'scope': 'C6 anti-recorder model v2 (corrected); independent; not admitted',
        'seed': seed,
        'elapsed_seconds': time.monotonic() - start,
        'results': results,
        'headline_corrections': [
            'v1 nonce-bound "verification" was circular: a reseal keyed on whatever '
            'the fake environment answered is self-consistent and verifies NOTHING. '
            'Offline verification fundamentally requires build-time mirrors, i.e. '
            'constant challenges.',
            'dual-path consistency kills pure replay of novel queries but is '
            'trivially satisfied by a symmetric fake; it only forces replay tools '
            'to become emulators.',
            'every real defense here is a cost ladder rung, not a wall: exact replay '
            '< symmetric fake < stateful fake < full semantic emulator < in-run hook (R004).',
        ],
        'false_positive_ledger': [
            'unstable probe mirrored from one variant garbles legit users on other variants (P3)',
            'stateful probes garble environments that legitimately reset state',
            'dual-path partners must be provably identical across ALL target variants '
            '(the 52 constraint, applied twice)',
        ],
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--seed', type=int, default=20261009)
    parser.add_argument('--output', type=Path,
                        default=Path(__file__).with_name('models6-results.json'))
    args = parser.parse_args()
    result = run(args.seed)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
