# CCPD Implementation Review

Independent review of the CCPD (Call-Chain Coupled Path-State
Decoding) implementation against `obsucvim_CCPD_rebased_agent_plan.md`
sections 11, 14, 15 and 17. Every ACTIVE below points at source and
at a test that was actually executed in this session; anything that
could not be exercised is marked PARTIAL and says why.

## Reviewed branch / commit

- Branch: `arena/723e28ef-obsucvim`
- Reviewed diff base: `08498f4` (CCPD Phase 1, Rust-only CFG/codec)
- This review ships in the commit that adds the Phase 2-7 wiring on
  top of `08498f4`; the final SHA is that commit.

## Source files inspected

- `engine/src/vmgen/ccpd.rs` (CFG, edge identity, variant codec,
  section wire format, fail-closed decoder)
- `engine/src/vmgen/runtime_mask.rs` (`derive_mask`, variant/closure/
  invocation selectors, `DecodeContext`/`DecodeCacheKey`, pinned
  vectors)
- `engine/src/vmgen/compiler.rs` (`Ctx::finish` CCPD build point, code
  section emission, section id8 emission, domain-separated salt)
- `engine/src/vmgen/template.rs` (`CCPD_HELPERS_LUA`, `CCPD_BDEC_LUA`,
  parse walk CD arm, `newE`/`makefn` CCPD arms, branch/closure
  micro-ops, `chain_epilogue` transition, enc_body EP linking)
- `engine/src/vmgen/fuse.rs` (collapse, branch-target remap)
- `engine/src/vmgen/proto_layout.rs` (section id allocation)
- `engine/src/main.rs`, `engine/src/vmgen/caps.rs` (CLI surface)

## Files changed

- `engine/src/vmgen/ccpd.rs`, `runtime_mask.rs`, `compiler.rs`,
  `template.rs`, `proto_layout.rs`, `caps.rs`, `engine/src/main.rs`
  (Phases 2-6, on top of the Phase 1 commit)
- `engine/src/vmgen/fuse.rs` (exit-target re-anchor fix, Finding F1)
- `engine/tests/ccpd_mirror_test.py`, `ccpd_diff_test.py`,
  `ccpd_runtime_audit_test.py` (new)

## CFG Edge Identity: ACTIVE

- Built in `Ctx::finish` from the final fused `FRec` stream and the
  remapped leader set (`compiler.rs`, after fuse collapse), per
  function, before section encoding.
- Every PC maps to exactly one `BlockId` (`pc_to_block`); `EdgeId`s
  are function-local, contiguous, assigned in declaration order;
  Jmp/Jf/Jt taken, branch fallthrough, ordinary fallthrough, and the
  synthetic Entry edge each own a distinct id, including two outcomes
  that land in the same block (unit test
  `cfg_branch_same_target_keeps_distinct_edges`).
- Return records carry no successor edge; tail-merge jumps to
  `n + 1` are modelled as `EXIT_BLOCK` exit edges that are incoming
  to no block (unit tests `cfg_tail_jump_is_exit_edge`,
  `cfg_multiple_returns_have_no_successors`).
- `validate()` runs at build time and fails closed (non-final branch
  records, target mismatches, duplicate ids all rejected); edge
  count and every edge id pass a checked `u16` conversion for the
  entry domain - no truncation, no modulo (ccpd.rs build_cfg tail).
- Runtime cross-check: `ccpd_runtime_audit_test.py` verifies every
  observed runtime block entry `(fn, block, edge)` against the
  compiler-dumped incoming-edge sets; 0 violations.

## Branch transition wiring: ACTIVE

- `M_SET_PC`, `M_BRANCH_FALSE/TRUE`, `M_FUSE_BRANCH_FALSE/TRUE`
  publish the incoming edge (`E.bt.jm/tk/bf[pc-1]`) and set
  `E.echg`; `chain_epilogue` consumes it exactly once and otherwise
  takes the per-PC fallthrough edge `E.bt.ft[np-1]`. Taken and
  not-taken outcomes write different EdgeIds (map tables are built
  from `branch_taken_edge_by_pc` / `branch_fallthrough_edge_by_pc`,
  not inferred from the target PC).
- `edge_pending` semantics: when `E.echg` is set the block is
  re-entered through `bdec` even if the target PC stays inside the
  current block (self-loops, same-block backedges) - covered by the
  loop cases in `ccpd_diff_test.py` section B and by the audit B
  lines showing repeat entries of loop blocks with increasing visit
  ordinals.
- Super-insn: the 23 multi-step superoperators contain no branch,
  jump, or closure primitive (`MICRO_SUPER_NAMES` +
  `micro_primitive_recipe`), so control transfer always executes as
  a single-step micro-op at its own record PC; `--ccpd --super-insn`
  is covered semantically (diff test sections B and C).

## Closure-origin context: ACTIVE

- `M_CLOSURE` reads the site as `E.pc - 1`, keeps a per-frame
  creation counter `E.cc[site]`, and passes
  `E.corg(E, site, ordinal, child)` - the scalar origin token - to
  `makefn`. The wrapper captures only that scalar plus its own
  `calls` counter; it never captures the parent frame table `E`
  (inspected in `makefn_decl`; upvalue cells `c` are built from `V`
  exactly as in the legacy path).
- Origin derivation order (frame context, current edge, current
  block, prototype, site, creation ordinal, child, transition id 7)
  is shared by `runtime_mask::derive_closure_origin` and `CCPDO`;
  bit-exactness is pinned (`ccpd_mirror_test.py`, plus the Rust
  pinned-vector test).
- Runtime evidence (audit M/C lines): four creations of the same
  child prototype receive four distinct origin tokens; a loop site
  in one frame yields creation ordinals 1, 2, 3 in order.

## Invocation rotation: ACTIVE

- The closure wrapper increments its instance-local `calls` counter
  on entry and derives a fresh frame context
  `CCPDI(org, calls, child, salt)` for `newE` on every call; the
  derived value is stored in a per-activation local (`E.fctx`), so
  reentrant/recursive activations cannot overwrite each other.
- Root frames receive `CCPDR(salt, proto)` plus the entry edge
  explicitly in `newE`.
- Runtime evidence (audit E lines): every frame starts on its
  function's entry edge; frame contexts are pairwise distinct across
  invocations of the same closure function and across recursion.

## Four block variants: ACTIVE

- Every non-empty block of every function gets exactly 4 variants
  (`CCPD_VARIANTS = 4`), each with a distinct operand-lane
  permutation code (0..23, probed distinct per block), distinct
  page epoch, and independent mask seed/step.
- Build-time assertions in `build_function_ccpd`: every variant is
  decoded back and compared record-by-record with the canonical
  slice (opcode byte and all four lanes); any mismatch aborts the
  build. A degenerate-payload guard re-seeds a variant whose bytes
  collide with a sibling and fails the build after 64 attempts
  instead of emitting identical payloads (plan 7.6).
- Encoding reuses the existing SoA/FRec/Fused shapes and the
  function-level `OpMap`; variant payloads are block slices of the
  final record stream, so fused records keep their vocabulary
  mapping.
- Unit coverage: `variants_roundtrip_and_differ`,
  `closure_sites_and_variant_diversity` (four distinct perm codes
  and epochs asserted per block).

## DecodeContext / derive_mask integration: ACTIVE (one documented deviation)

- The compiler encodes every payload column through
  `runtime_mask::derive_mask` with a fully populated
  `DecodeContext` (prototype, block, lane, epoch); the generated
  `bdec` re-derives the identical masks via the `CCPDM` Luau mirror.
  `derive_mask` therefore has real production call points on both
  sides; it is not test-only code.
- Deviation from the letter of plan 7.5/9.2: payload masks are
  derived at `entry_domain = 0`. Incoming-edge identity enters the
  decode through variant selection - `derive_variant_id(frame_ctx,
  edge, block, visit, salt)` chooses which of the four differently
  permuted/masked payloads is decoded - and through the composite
  cache key. Two different incoming edges to one block therefore do
  select different physical decodings over visits, which is the
  diversity the plan asks for, but the mask function itself does
  not take the edge as an input. `DecodeCacheKey` still carries
  `entry_domain` explicitly so a future per-domain mask split
  cannot alias domain-0 entries. Recorded here rather than silently
  papered over.

## Cache identity and eviction

- ACTIVE. The `bdec` cache key is the collision-free integer
  `(block * edge_count + incoming_edge) * 4 + variant`, with block
  id 1-based, `edge < edge_count`, `variant < 4` (bounds enforced
  by the directory decoder and by the Luau parse arm), so distinct
  (block, edge, variant) triples can never alias. Frame context and
  page epoch are functions of the selected variant descriptor, and
  the frame-local `E.bch`/`E.bq` lifetime is unchanged.
- `E.bq` stores the full composite key; eviction removes exactly
  that entry and wipes its lanes. Cache hits remask every lane from
  the stored mask to the current `wm` before handlers see them.
- On a cache miss, `bdec` also validates that the published edge is
  declared incoming to the block (`blk.inc[edge]`), failing closed
  (hang idiom, consistent with the parser) instead of decoding on
  an impossible transition. A hit can only exist for a previously
  validated (block, edge, variant) triple because the edge is part
  of the key.
- Evidence form: source inspection plus the cache-pressure shapes
  in diff test section B (48-way branch classifier, 300-iteration
  multi-edge loops) under cache eviction, plus audit B lines
  showing one block entered via several edges/variants. Hit/miss
  counters are not separately exposed; that is the one place where
  the evidence is behavioural rather than a direct counter readout.

## Serialization / parser versioning: ACTIVE

- CCPD metadata uses `ProtoFieldId::Ccpd = 8`; section id 6 remains
  reserved, id 7 (FusedVocab) unchanged; `LAYOUT_SLOTS` 8 -> 9 with
  parser/writer symmetry. CCPD-off output is unchanged (below).
- Section format version 1: directory (blocks, incoming edges,
  per-variant descriptors), per-PC transition maps, closure-site
  table, exit-edge list; the payload bank travels as the code
  section content. The Luau parser collects sections independently
  of their shuffled order and branches on CCPD presence; bank
  bounds are validated after the walk.
- Fail-closed decoding (`decode_section` + the Luau mirror arm):
  unknown version, variant count != 4, non-sequential or
  non-contiguous blocks, duplicate edge ids, unknown edge kind,
  perm code >= 24, payload offset/len beyond the bank, transition
  maps referencing undeclared edges, entry edge not declared,
  closure sites out of range, trailing bytes - all rejected. Unit
  test `section_rejects_tampering` covers the Rust side; there is
  no silent fallback to the legacy stream when the directory is
  malformed (the legacy `W/SA..SD` streams are not even read on the
  CCPD path, and the EP block-window loops are emitted empty).

## CCPD-off byte compatibility: ACTIVE

- With `--ccpd` off, no CCPD payload, section, or runtime branch is
  emitted; the CCPD salt is derived from existing blob material
  (domain-separated xor, `% 2^28`) and consumes no shared RNG.
- Anchor: `tests/cases/basics.lua` at `--preset v15 --seed 42`
  reproduces the shipped `engine/examples/basics.v15.luau.lua`
  byte-for-byte, SHA-256
  `780def96d7d8b35780f90159fe318ca74c720835a9e469f244c5018065790974`
  (asserted in `ccpd_diff_test.py` section C).
- `--ccpd --no-ccpd` is byte-identical to the plain off artifact at
  the same seed (asserted).

## Runtime test matrix and exact commands

All of the following were run against the final binaries in this
session (Rust 1.88, Luau 0.735 from `engine/tools/rebuild_tools.sh`):

| Gate | Command | Result |
|---|---|---|
| Rust units | `cargo test --release` | 142 + 4 passed, 0 failed |
| Repo matrix | `bash engine/tests/run_tests.sh` | 188 PASS, 0 FAIL |
| Multi-seed | `bash engine/tests/multiseed.sh` | FAIL=0 |
| Micro fusion | `bash engine/tests/micro_fusion_test.sh` | PASS |
| VM semantics | `bash engine/tests/vm_semantics_regression.sh` | PASS |
| Capability manifest | `python3 engine/tests/capability_manifest_test.py` | 11 PASS, 0 FAIL |
| Phase G API | `python3 engine/tests/phase_g_api_test.py` | ALL PASS |
| Prod audit gate | `bash engine/tests/prod_audit_gate_test.sh` | PASS |
| Guard matrix | `python3 engine/tests/guard_matrix_test.py` | 80 PASS, 0 FAIL |
| Tamper matrix | `python3 engine/tests/vm_tamper_matrix_test.py` | 60 PASS, 0 FAIL |
| Robustness | `python3 engine/tests/robustness_gate_test.py` | 16/16 PASS |
| Bench presets | `bash engine/tests/bench_presets.sh` | PASS |
| Mirror consistency (11.3) | `python3 engine/tests/ccpd_mirror_test.py` | 13/13 (65 Luau vectors) |
| Semantic diff + CLI (10.4/11.4-11.6) | `python3 engine/tests/ccpd_diff_test.py` | 265/265 |
| Runtime audit (11.7/15.2) | `python3 engine/tests/ccpd_runtime_audit_test.py` | 16/16 |

Plan 11.1 CFG checklist mapping: items 1-6, 8-11, 13-15 are Rust
unit tests in `ccpd.rs` (single block, Jmp, Jf, Jt, same-target
dual outcome, backedge, multiple/early returns, closure sites,
Nop/Canary noise records, return-only function, rejected targets).
Item 7 (nested if/elseif/else) and item 12 (super-insn remap) are
covered by the compiler-level tests `collapse_remaps_branch_targets`
/ `collapse_reanchors_exit_target` and by the semantic differential
over the shared corpus (`control`, `edge`, real-world control
cases) with `--super-insn` on and off.

## Findings (Critical / High / Medium / Low)

- **F1 (High, fixed).** `fuse::collapse` rewrote branch targets
  inside the fused stream but left the `n + 1` exit target at its
  pre-collapse value; after collapse the stream is shorter, so the
  target dangled past the new end. The legacy runtime never noticed
  because those merge jumps sit after `Return` and are never
  executed; CCPD's CFG covers every record and rejected the build
  (`--ccpd --super-insn` failed generation on any function whose
  branches merge at the tail). Fix: collapse re-anchors an exit
  target to `fused_len + 1` (`collapse_reanchors_exit_target`).
  Re-verified: generation succeeds, semantics match
  (`ccpd_diff_test.py` 265/265), and the legacy gates
  (run_tests/multiseed/micro-fusion/vm-semantics) stay green.
- **F2 (High, fixed).** The first runtime wiring keyed the decode
  cache by `(block, variant)` only, omitting the incoming edge
  domain the plan 9.3 requires as an explicit key field. Harmless
  for today's frame-local cache (all variants decode to identical
  canonical lanes and hits remask), but a latent aliasing trap.
  Fixed: composite `(block, edge, variant)` key, `edge_count`
  carried in the directory, plus incoming-edge validation on miss.
- **F3 (Medium, fixed).** The audit reporters injected into the
  `bdec`/`newE`/`makefn` fragments silently never fired: the
  fragment sanitizer renamed the free global
  `OBSUCVIM_CCPD_AUDIT` to an unbound local (the string-cache
  channel survives only because it lives in a micro-op body). Fix:
  the name is added to the fragment reserved set while the audit
  channel is enabled. Lesson recorded: any future injection into
  sanitized fragments must use reserved names or `E` fields.
- **F4 (Medium, fixed).** Several CCPD template wirings were
  `str::replace` calls whose anchors could in principle drift and
  silently emit a degraded artifact. Generation now asserts the
  CCPD walk arm, the `newE` frame-context fields, the `makefn`
  invocation derivation, and the enc_body EP/ccpd fields, and
  panics with a clear message instead of emitting legacy-shaped
  output (fail-closed; compile time only, off path untouched).
- **F5 (Low, documented).** The runtime calls `CCPDR/CCPDI` with
  the 1-based prototype index where the Rust doc comments describe
  a prototype id. The derivation functions themselves are bit-exact
  mirrors (pinned vectors), and frame/origin contexts never cross
  the Rust/Luau boundary at runtime, so this is an internal
  convention note, not a correctness issue.
- **F6 (Low, pre-existing, out of scope).** `strings.lua` seed 1234
  was already failing before CCPD (`table.concat` yielding `xyA`
  instead of `xyz` in one final-binary configuration; see
  `OBF_LUA_TECH_REVIEW.md`). It is outside the CCPD change surface
  and remains open as a separate item; CCPD neither fixes nor worsens
  it (the case passes in the CCPD differential at seeds 1/42/777).

No Critical findings. No High findings remain open.

## Fixed findings and rerun evidence

F1-F4 were fixed in this change and the full matrix above was
re-run afterwards (the numbers in the table are post-fix). The
super-insn differential that exposed F1 is now a permanent part of
`ccpd_diff_test.py` sections B and C.

## Unsupported combinations

`--ccpd` is accepted only on the default/v15 Luau VM path:

- `--preset low|medium|high|vm|max` or `--vm` with `--ccpd`:
  exits 1 with `error: --ccpd requires --preset v15 (Luau VM)`.
- `--dialect 5.1` with `--ccpd`: exits 1 with the Luau-only product
  error.
- The capability-manifest path (`caps.rs`) always generates with
  CCPD off.

All rejections are asserted in `ccpd_diff_test.py` section C,
including argument-order independence and `--no-ccpd` byte
equality.

## Known limitations

- CCPD is build diversity, not cryptography. The build salt and the
  full directory ship inside the artifact; anyone with the artifact
  can emulate `bdec` offline and recover every variant of every
  block (the variant payloads are masked with a derivation whose
  inputs are all in the directory). What CCPD removes is a single
  canonical code stream and any edge-independent decoding: a static
  reader immediately sees, per function, only the directory shape
  (one audited sample: main chunk 35 blocks / 50 edges / 140
  variants; see `ccpd_runtime_audit_test.py` INFO line), and must
  either execute the VM or reimplement its context-coupled decoder
  (frame context -> closure origin -> invocation -> edge -> visit
  -> variant) to recover canonical records or to know which
  encoding a given path actually ran. Dynamic execution of one
  path reveals that path's variant choices. No claim of
  un-devirtualizability is made.
- Variant selection is deterministic given the artifact; it is a
  stable diversity function, not a secret (plan 2.3).
- `entry_domain` does not feed the payload masks (see the
  DecodeContext deviation above).
- Exit edges (`EXIT_BLOCK`) are declared in the directory for
  completeness but, exactly as in the legacy VM, the dead merge
  jumps behind them are never executed.
- Performance was not a goal of this change (plan 14.15); CCPD
  artifacts grow roughly with the 4x variant bank. Correctness
  gates above are the acceptance evidence.

## Final verdict

For the supported scope (`--preset v15 --dialect luau`, CCPD on or
off, super-insn on or off), every item of the plan section 17
checklist is met and evidenced above, with the single documented
deviation that edge identity reaches the payload through variant
selection and the cache key rather than through the mask's
`entry_domain` input. No Critical or High findings are open.
CCPD is ACTIVE in that scope.
