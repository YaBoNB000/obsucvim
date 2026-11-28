//! CCPD (Call-Chain Coupled Path-State Decoding) compiler side.
//!
//! Rebased plan, Phase A/D/E: from the *final* fused record stream and
//! the final leader set of one function this module builds
//!
//! - a complete CFG whose every incoming edge owns a stable `EdgeId`,
//! - four differently-encoded payload variants of every basic block
//!   (independent operand-lane permutation + mask parameters, all
//!   decoding to the identical canonical `FRec` slice), and
//! - the versioned CCPD directory (prototype section id 8) plus the
//!   variant payload bank (carried by the code section id 5 under
//!   `--ccpd`, replacing the canonical SoA stream so no directly
//!   decodable legacy copy of the code remains in the artifact).
//!
//! `EdgeId` and the context derivations live in `runtime_mask`; this
//! module owns the CFG, the variant codec and the wire format. All
//! sizes are checked: an edge count that does not fit the u16
//! `entry_domain` or a block/record count beyond the u16 record format
//! is a compile error, never a truncation.
//!
//! Wire format of the CCPD directory section (all integers are the
//! `isa` varint unless marked `rb` = one raw byte; the payload bank
//! itself travels in the code section, see plan section 8.3):
//!
//! ```text
//! rb  format_version = 1
//! rb  variant_count = 4
//! var block_count
//! var edge_count                (including the synthetic Entry edge)
//! var build_salt                (< 2^28)
//! rb  fn_perm_code              (0..23, legacy slot_perm of the fn)
//! rb  nop_code, nop_alias, closure_code   (wire opcodes for this fn)
//! var entry_edge_id             (raw EdgeId of the Entry edge)
//! var total_records
//! repeat block_count:
//!   var block_id                (must equal the running index)
//!   var start_pc, end_pc        (1-based, inclusive)
//!   var incoming_edge_count
//!   repeat incoming_edge_count:
//!     var edge_id
//!     rb  kind_byte             (kind 0..4, | 0x80 when backedge)
//!     var source_pc_or_0        (0 = synthetic Entry edge)
//!   repeat 4:
//!     rb  variant_id            (0..3)
//!     rb  operand_perm_code     (0..23)
//!     var page_epoch
//!     var payload_offset        (relative to the variant bank)
//!     var payload_len
//!     var mask_seed
//!     var mask_step
//! var map_record_count          (must equal total_records)
//! repeat map_record_count:
//!   var jump_edge_id_plus_1             (0 = none)
//!   var branch_taken_edge_id_plus_1     (0 = none)
//!   var branch_fallthrough_edge_id_plus_1
//!   var fallthrough_edge_id_plus_1
//! var closure_site_count
//! repeat closure_site_count:
//!   var site_pc                   (1-based PC of the Closure record)
//!   var child_fn_index            (0-based prototype index)
//! var exit_edge_count
//! repeat exit_edge_count:
//!   var edge_id                   (edge whose target is the exit)
//! ```
//!
//! A variant payload is block-local SoA: `record_count` masked opcode
//! bytes followed by four columns of `record_count` masked u16 varints
//! each. Physical lane column `j` carries logical lane `perm[j]`, where
//! `perm = operand_perm_from_code(operand_perm_code)`. The mask added
//! to the value at record `i` (0-based) in slot `s` (0 = opcode,
//! 1..=4 = physical lanes) is `derive_mask(mask_seed, ctx)` with
//! `ctx = (fn_index, block_id, lane = s, entry_domain = 0,
//! page_epoch = page_epoch + i * mask_step mod 2^28)`, taken mod 256
//! for opcodes and mod 65536 for lanes.

use super::fuse::{FKind, FRec};
use super::isa::{self, Op, OpMap};
use super::runtime_mask::{derive_mask, DecodeContext, EdgeId};

/// Variants per block, fixed for the first CCPD version (plan 0.4).
pub const CCPD_VARIANTS: u8 = 4;
/// CCPD directory format version (plan 8.2).
pub const CCPD_FORMAT_VERSION: u8 = 1;

const MOD28: u32 = 268_435_456;

/// Sentinel `target_block` for edges whose target PC is `n + 1` (one
/// past the last record): the compiler emits such jumps for branch
/// merges at the function tail. They are exit edges - no block owns
/// them as incoming. They ARE reachable whenever a non-returning
/// branch falls through to the tail merge (the legacy VM has no
/// block there either and historically faulted on the same path);
/// the CCPD epilogue therefore routes `pc == cd.tn + 1` to the
/// frame's implicit zero-value return instead of `bdec`
/// (CCPD repair plan P1).
pub const EXIT_BLOCK: u16 = u16::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcpdEdgeKind {
	Entry,
	Jump,
	BranchTaken,
	BranchFallthrough,
	Fallthrough,
}

impl CcpdEdgeKind {
	pub fn code(self) -> u8 {
		self as u8
	}

	pub fn from_code(code: u8) -> Option<CcpdEdgeKind> {
		Some(match code {
			0 => CcpdEdgeKind::Entry,
			1 => CcpdEdgeKind::Jump,
			2 => CcpdEdgeKind::BranchTaken,
			3 => CcpdEdgeKind::BranchFallthrough,
			4 => CcpdEdgeKind::Fallthrough,
			_ => return None,
		})
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcpdBlock {
	pub id: u16,
	/// 1-based logical record PC of the first record.
	pub start_pc: u16,
	/// 1-based logical record PC of the last record (inclusive).
	pub end_pc: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcpdEdge {
	pub id: EdgeId,
	/// `None` only for the synthetic Entry edge.
	pub source_block: Option<u16>,
	pub target_block: u16,
	/// PC of the record that transfers control (branch/jump record, or
	/// the last record of the source block for fallthrough). `None`
	/// only for the Entry edge.
	pub source_pc: Option<u16>,
	pub kind: CcpdEdgeKind,
	pub backedge: bool,
}

/// Per-function CFG over the final record stream. The `*_by_pc` maps
/// are indexed by `pc - 1` (vector index 0 corresponds to VM PC 1),
/// matching the 1-based Lua tables the runtime builds from them.
#[derive(Debug, Clone)]
pub struct CcpdFunctionCfg {
	pub blocks: Vec<CcpdBlock>,
	pub edges: Vec<CcpdEdge>,
	pub pc_to_block: Vec<u16>,
	pub jump_edge_by_pc: Vec<Option<u32>>,
	pub branch_taken_edge_by_pc: Vec<Option<u32>>,
	pub branch_fallthrough_edge_by_pc: Vec<Option<u32>>,
	pub fallthrough_edge_by_pc: Vec<Option<u32>>,
}

fn block_of(pc_to_block: &[u16], pc: u16) -> Option<u16> {
	pc_to_block.get((pc as usize).checked_sub(1)?).copied()
}

/// Build the CFG from the final (post-fusion, post-remap) record
/// stream and leader set. Branch records are always block-final in
/// this compiler (every branch pushes `pc + 1` as a leader); anything
/// else fails closed in `validate`.
pub fn build_cfg(recs: &[FRec], leaders: &[u16]) -> Result<CcpdFunctionCfg, String> {
	let n = recs.len();
	if n == 0 {
		return Err("ccpd: empty record stream".to_string());
	}
	if n > u16::MAX as usize {
		return Err(format!("ccpd: record count {n} exceeds u16 record format"));
	}
	let mut starts: Vec<u16> = leaders.to_vec();
	starts.sort_unstable();
	starts.dedup();
	if starts.first() != Some(&1) {
		return Err("ccpd: first leader must be PC 1".to_string());
	}
	if starts.iter().any(|&s| s == 0 || s as usize > n) {
		return Err("ccpd: leader outside record range".to_string());
	}
	let mut blocks = Vec::with_capacity(starts.len());
	for (i, &start) in starts.iter().enumerate() {
		let end = if i + 1 < starts.len() { starts[i + 1] - 1 } else { n as u16 };
		blocks.push(CcpdBlock { id: i as u16, start_pc: start, end_pc: end });
	}
	let mut pc_to_block = vec![0u16; n];
	for b in &blocks {
		for pc in b.start_pc..=b.end_pc {
			pc_to_block[(pc - 1) as usize] = b.id;
		}
	}
	let is_leader = |pc: u16| starts.binary_search(&pc).is_ok();

	let mut edges: Vec<CcpdEdge> = Vec::new();
	let mut push_edge = |edges: &mut Vec<CcpdEdge>,
	                     source_block: Option<u16>,
	                     target_block: u16,
	                     source_pc: Option<u16>,
	                     kind: CcpdEdgeKind,
	                     backedge: bool|
	 -> u32 {
		let id = edges.len() as u32;
		edges.push(CcpdEdge {
			id: EdgeId(id),
			source_block,
			target_block,
			source_pc,
			kind,
			backedge,
		});
		id
	};

	// Synthetic Entry edge into the first block.
	push_edge(&mut edges, None, 0, None, CcpdEdgeKind::Entry, false);

	let none_map = vec![None; n];
	let mut jump_edge_by_pc = none_map.clone();
	let mut branch_taken_edge_by_pc = none_map.clone();
	let mut branch_fallthrough_edge_by_pc = none_map.clone();
	let mut fallthrough_edge_by_pc = none_map;

	for b in &blocks {
		let last = &recs[(b.end_pc - 1) as usize];
		match last.kind {
			FKind::Base(Op::Jmp) => {
				let target = last.b;
				if target == 0 || target as usize > n + 1 {
					return Err(format!(
						"ccpd: jump at pc {} targets out-of-range pc {}",
						b.end_pc, target
					));
				}
				if target as usize == n + 1 {
					// Exit edge (branch merge at the function tail).
					let id = push_edge(
						&mut edges,
						Some(b.id),
						EXIT_BLOCK,
						Some(b.end_pc),
						CcpdEdgeKind::Jump,
						false,
					);
					jump_edge_by_pc[(b.end_pc - 1) as usize] = Some(id);
					continue;
				}
				if !is_leader(target) {
					return Err(format!(
						"ccpd: jump at pc {} targets non-leader pc {}",
						b.end_pc, target
					));
				}
				let tb = block_of(&pc_to_block, target).expect("leader has a block");
				let backedge = tb <= b.id;
				let id = push_edge(
					&mut edges,
					Some(b.id),
					tb,
					Some(b.end_pc),
					CcpdEdgeKind::Jump,
					backedge,
				);
				jump_edge_by_pc[(b.end_pc - 1) as usize] = Some(id);
			}
			FKind::Base(Op::Jf) | FKind::Base(Op::Jt) => {
				let target = last.b;
				if target == 0 || target as usize > n + 1 {
					return Err(format!(
						"ccpd: branch at pc {} targets out-of-range pc {}",
						b.end_pc, target
					));
				}
				let taken_exit = target as usize == n + 1;
				if !taken_exit && !is_leader(target) {
					return Err(format!(
						"ccpd: branch at pc {} targets non-leader pc {}",
						b.end_pc, target
					));
				}
				let tb = if taken_exit {
					EXIT_BLOCK
				} else {
					block_of(&pc_to_block, target).expect("leader has a block")
				};
				let backedge = !taken_exit && tb <= b.id;
				let taken = push_edge(
					&mut edges,
					Some(b.id),
					tb,
					Some(b.end_pc),
					CcpdEdgeKind::BranchTaken,
					backedge,
				);
				branch_taken_edge_by_pc[(b.end_pc - 1) as usize] = Some(taken);
				let next = b.id as usize + 1;
				let fall_target = if next < blocks.len() {
					blocks[next].id
				} else if b.end_pc as usize == n {
					// Last record of the stream: fallthrough exits.
					EXIT_BLOCK
				} else {
					return Err(format!(
						"ccpd: conditional branch at pc {} has no fallthrough block",
						b.end_pc
					));
				};
				let fall = push_edge(
					&mut edges,
					Some(b.id),
					fall_target,
					Some(b.end_pc),
					CcpdEdgeKind::BranchFallthrough,
					false,
				);
				branch_fallthrough_edge_by_pc[(b.end_pc - 1) as usize] = Some(fall);
			}
			FKind::Base(Op::Return) => {}
			_ => {
				let next = b.id as usize + 1;
				if next < blocks.len() {
					let id = push_edge(
						&mut edges,
						Some(b.id),
						blocks[next].id,
						Some(b.end_pc),
						CcpdEdgeKind::Fallthrough,
						false,
					);
					fallthrough_edge_by_pc[(b.end_pc - 1) as usize] = Some(id);
				} else if b.end_pc as usize != n {
					return Err("ccpd: trailing block does not end the stream".to_string());
				}
			}
		}
	}

	if edges.len() > u16::MAX as usize + 1 {
		return Err(format!(
			"ccpd: edge count {} cannot be encoded in the u16 entry domain",
			edges.len()
		));
	}
	// Every edge id must survive the checked conversion into the
	// runtime entry domain (plan 2.4): never truncate or wrap.
	for e in &edges {
		if u16::try_from(e.id.0).is_err() {
			return Err(format!("ccpd: edge id {} exceeds u16 entry domain", e.id.0));
		}
	}

	let cfg = CcpdFunctionCfg {
		blocks,
		edges,
		pc_to_block,
		jump_edge_by_pc,
		branch_taken_edge_by_pc,
		branch_fallthrough_edge_by_pc,
		fallthrough_edge_by_pc,
	};
	cfg.validate(recs)?;
	Ok(cfg)
}

impl CcpdFunctionCfg {
	/// Structural validation (plan 4.5). `build_cfg` already runs it;
	/// it is public so tests and the compiler can re-check after any
	/// future transformation.
	pub fn validate(&self, recs: &[FRec]) -> Result<(), String> {
		let n = recs.len();
		if self.pc_to_block.len() != n {
			return Err("ccpd validate: pc_to_block does not cover the stream".to_string());
		}
		// Blocks: unique ids, contiguous, non-overlapping, gapless.
		let mut expect_start = 1u16;
		for (i, b) in self.blocks.iter().enumerate() {
			if b.id as usize != i {
				return Err("ccpd validate: block ids not sequential".to_string());
			}
			if b.start_pc != expect_start || b.end_pc < b.start_pc {
				return Err("ccpd validate: block ranges not contiguous".to_string());
			}
			expect_start = b.end_pc + 1;
		}
		if expect_start as usize != n + 1 {
			return Err("ccpd validate: blocks do not cover the stream".to_string());
		}
		// Edges: unique ids, exactly one Entry, legal endpoints.
		let mut seen = vec![false; self.edges.len()];
		let mut entry_count = 0usize;
		for e in &self.edges {
			let idx = e.id.0 as usize;
			if idx >= self.edges.len() || seen[idx] {
				return Err("ccpd validate: duplicate or out-of-range edge id".to_string());
			}
			seen[idx] = true;
			if e.target_block != EXIT_BLOCK
				&& (e.target_block as usize) >= self.blocks.len()
			{
				return Err("ccpd validate: edge target block invalid".to_string());
			}
			if let Some(sb) = e.source_block {
				if (sb as usize) >= self.blocks.len() {
					return Err("ccpd validate: edge source block invalid".to_string());
				}
			}
			if e.kind == CcpdEdgeKind::Entry {
				entry_count += 1;
				if e.source_block.is_some() || e.source_pc.is_some() || e.target_block != 0 {
					return Err("ccpd validate: malformed Entry edge".to_string());
				}
			}
		}
		if entry_count != 1 {
			return Err("ccpd validate: exactly one Entry edge required".to_string());
		}
		// Per-record transition classification must agree with the maps.
		for (i, r) in recs.iter().enumerate() {
			let pc = (i + 1) as u16;
			let block = &self.blocks[self.pc_to_block[i] as usize];
			let is_final = pc == block.end_pc;
			match r.kind {
				FKind::Base(Op::Jmp) => {
					if !is_final {
						return Err("ccpd validate: jump record is not block-final".to_string());
					}
					let id = self.jump_edge_by_pc[i]
						.ok_or("ccpd validate: jump record has no edge")?;
					let e = &self.edges[id as usize];
					let target_ok = if e.target_block == EXIT_BLOCK {
						r.b as usize == recs.len() + 1
					} else {
						self.blocks[e.target_block as usize].start_pc == r.b
					};
					if e.kind != CcpdEdgeKind::Jump || !target_ok {
						return Err("ccpd validate: jump edge target mismatch".to_string());
					}
				}
				FKind::Base(Op::Jf) | FKind::Base(Op::Jt) => {
					if !is_final {
						return Err("ccpd validate: branch record is not block-final".to_string());
					}
					let tid = self.branch_taken_edge_by_pc[i]
						.ok_or("ccpd validate: branch has no taken edge")?;
					let fid = self.branch_fallthrough_edge_by_pc[i]
						.ok_or("ccpd validate: branch has no fallthrough edge")?;
					if tid == fid {
						return Err(
							"ccpd validate: taken/fallthrough edges must differ".to_string()
						);
					}
					let te = &self.edges[tid as usize];
					let taken_ok = if te.target_block == EXIT_BLOCK {
						r.b as usize == recs.len() + 1
					} else {
						self.blocks[te.target_block as usize].start_pc == r.b
					};
					if te.kind != CcpdEdgeKind::BranchTaken || !taken_ok {
						return Err("ccpd validate: taken edge target mismatch".to_string());
					}
					let fe = &self.edges[fid as usize];
					let fall_ok = fe.target_block as usize == block.id as usize + 1
						|| (fe.target_block == EXIT_BLOCK
							&& block.id as usize + 1 == self.blocks.len());
					if fe.kind != CcpdEdgeKind::BranchFallthrough || !fall_ok {
						return Err(
							"ccpd validate: fallthrough edge target mismatch".to_string()
						);
					}
				}
				FKind::Base(Op::Return) => {
					if self.jump_edge_by_pc[i].is_some()
						|| self.branch_taken_edge_by_pc[i].is_some()
						|| self.fallthrough_edge_by_pc[i].is_some()
					{
						return Err("ccpd validate: return record carries an edge".to_string());
					}
				}
				_ => {
					if self.jump_edge_by_pc[i].is_some()
						|| self.branch_taken_edge_by_pc[i].is_some()
						|| self.branch_fallthrough_edge_by_pc[i].is_some()
					{
						return Err(
							"ccpd validate: non-branch record carries a branch edge".to_string()
						);
					}
					// Fused records are data movement only; the fuse
					// whitelist guarantees they are never jumps, returns
					// or closure creations, so the branch maps above are
					// the only constraint on them.
				}
			}
		}
		// Map entries must reverse-resolve to edges of the right kind.
		for (i, id) in self.fallthrough_edge_by_pc.iter().enumerate() {
			if let Some(id) = id {
				let e = &self.edges[*id as usize];
				if e.kind != CcpdEdgeKind::Fallthrough || e.source_pc != Some((i + 1) as u16) {
					return Err("ccpd validate: fallthrough map mismatch".to_string());
				}
			}
		}
		Ok(())
	}

	/// Incoming edges of one block, in edge-id order (Entry included
	/// for block 0).
	pub fn incoming(&self, block_id: u16) -> Vec<&CcpdEdge> {
		self.edges.iter().filter(|e| e.target_block == block_id).collect()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcpdBlockVariant {
	pub variant_id: u8,
	/// Logical lane carried by each physical column: `perm[j]` is the
	/// logical lane (0=a, 1=b, 2=c, 3=d) stored in physical column j.
	pub operand_perm: [u8; 4],
	pub operand_perm_code: u8,
	pub page_epoch: u32,
	pub payload_offset: u32,
	pub payload_len: u32,
	pub mask_seed: u32,
	pub mask_step: u32,
}

#[derive(Debug, Clone)]
pub struct CcpdBlockVariants {
	pub block_id: u16,
	pub start_pc: u16,
	pub record_count: u16,
	pub variants: [CcpdBlockVariant; CCPD_VARIANTS as usize],
}

/// Everything the compiler embeds for one prototype under `--ccpd`.
#[derive(Debug, Clone)]
pub struct CcpdFunctionData {
	pub cfg: CcpdFunctionCfg,
	pub blocks: Vec<CcpdBlockVariants>,
	/// Concatenated variant payloads; offsets in the descriptors are
	/// relative to this bank (the code section carries it verbatim).
	pub bank: Vec<u8>,
	pub build_salt: u32,
	/// Lehmer code (0..23) of the function's legacy slot permutation,
	/// needed by the runtime to reproduce legacy checksum weights.
	pub fn_perm_code: u8,
	pub nop_code: u8,
	pub nop_alias: u8,
	pub clos_code: u8,
	/// `(site_pc, child_fn_index)` for every Closure record, in PC
	/// order. The runtime links parent EP tables to child EPs through
	/// these (the legacy km stream rewrite has no CCPD counterpart).
	pub closure_sites: Vec<(u16, u16)>,
}

impl CcpdFunctionData {
	pub fn entry_edge_id(&self) -> u32 {
		self.cfg
			.edges
			.iter()
			.find(|e| e.kind == CcpdEdgeKind::Entry)
			.map(|e| e.id.0)
			.unwrap_or(0)
	}

	pub fn total_records(&self) -> u32 {
		self.blocks.iter().map(|b| b.record_count as u32).sum()
	}
}

/// Deterministic per-(function, block, variant) derivation word. This
/// is build-diversity derivation, not a key: identical inputs always
/// produce identical artifacts for a given build salt.
fn variant_word(build_salt: u32, fn_index: u32, block_id: u32, variant_id: u8, tag: u32) -> u32 {
	derive_mask(
		build_salt ^ tag.wrapping_mul(0x9e37_79b9),
		&DecodeContext::new(fn_index, block_id, variant_id).with_page_epoch(tag),
	)
}

fn opcode_byte(rec: &FRec, map: &OpMap, s_idx: usize) -> u8 {
	match rec.kind {
		FKind::Base(op) => map.code_for(op, s_idx),
		FKind::Fused(k) => OpMap::fused_tramp_code(s_idx, k as usize),
	}
}

fn lane_of(rec: &FRec, lane: u8) -> u16 {
	match lane {
		0 => rec.a,
		1 => rec.b,
		2 => rec.c,
		_ => rec.d,
	}
}

fn payload_key(
	mask_seed: u32,
	fn_index: u32,
	block_id: u32,
	slot: u8,
	page_epoch: u32,
	mask_step: u32,
	record_idx: u32,
) -> u32 {
	let epoch = ((page_epoch as u64 + record_idx as u64 * mask_step as u64) % MOD28 as u64) as u32;
	derive_mask(
		mask_seed,
		&DecodeContext::new(fn_index, block_id, slot).with_page_epoch(epoch),
	)
}

/// Encode one variant payload for a block slice. `perm[j]` is the
/// logical lane written into physical column j.
pub fn encode_variant_payload(
	recs: &[FRec],
	perm: &[u8; 4],
	mask_seed: u32,
	page_epoch: u32,
	mask_step: u32,
	map: &OpMap,
	s_idx: usize,
	fn_index: u32,
	block_id: u32,
) -> Vec<u8> {
	let mut out = Vec::with_capacity(recs.len() * 6);
	for (i, rec) in recs.iter().enumerate() {
		let k = payload_key(mask_seed, fn_index, block_id, 0, page_epoch, mask_step, i as u32);
		out.push(((opcode_byte(rec, map, s_idx) as u32 + k % 256) % 256) as u8);
	}
	for (j, &logical) in perm.iter().enumerate() {
		for (i, rec) in recs.iter().enumerate() {
			let k = payload_key(
				mask_seed,
				fn_index,
				block_id,
				(j + 1) as u8,
				page_epoch,
				mask_step,
				i as u32,
			);
			let masked = (lane_of(rec, logical) as u32 + k % 65536) % 65536;
			isa::encode_u16var(&mut out, masked as u16);
		}
	}
	out
}

/// Decode one variant payload back to canonical `(opcode_byte, lanes)`
/// records. Used by the compiler self-check and by tests; the Luau
/// runtime mirror performs the same steps inside `bdec`.
#[allow(clippy::too_many_arguments)]
pub fn decode_variant_payload(
	payload: &[u8],
	record_count: usize,
	perm: &[u8; 4],
	mask_seed: u32,
	page_epoch: u32,
	mask_step: u32,
	fn_index: u32,
	block_id: u32,
) -> Result<Vec<(u8, [u16; 4])>, String> {
	if payload.len() < record_count {
		return Err("ccpd payload: truncated opcode column".to_string());
	}
	let mut opcodes = Vec::with_capacity(record_count);
	for i in 0..record_count {
		let k = payload_key(mask_seed, fn_index, block_id, 0, page_epoch, mask_step, i as u32);
		opcodes.push(((payload[i] as u32 + 256 - k % 256) % 256) as u8);
	}
	let mut pos = record_count;
	let mut columns: Vec<Vec<u16>> = vec![Vec::with_capacity(record_count); 4];
	for j in 0..4usize {
		for i in 0..record_count {
			let (v, next) = isa::decode_varint(payload, pos)
				.ok_or("ccpd payload: truncated lane column")?;
			pos = next;
			let k = payload_key(
				mask_seed,
				fn_index,
				block_id,
				(j + 1) as u8,
				page_epoch,
				mask_step,
				i as u32,
			);
			columns[j].push(((v + 65536 - k % 65536) % 65536) as u16);
		}
	}
	if pos != payload.len() {
		return Err("ccpd payload: trailing bytes".to_string());
	}
	let mut out = Vec::with_capacity(record_count);
	for i in 0..record_count {
		let mut lanes = [0u16; 4];
		for j in 0..4usize {
			lanes[perm[j] as usize] = columns[j][i];
		}
		out.push((opcodes[i], lanes));
	}
	Ok(out)
}

/// Build the complete per-function CCPD payload: CFG, four variants
/// per block, the bank, and a decode self-check of every variant.
#[allow(clippy::too_many_arguments)]
pub fn build_function_ccpd(
	recs: &[FRec],
	leaders: &[u16],
	map: &OpMap,
	s_idx: usize,
	fn_index: usize,
	build_salt: u32,
	fn_perm_code: u8,
) -> Result<CcpdFunctionData, String> {
	let cfg = build_cfg(recs, leaders)?;
	let mut bank: Vec<u8> = Vec::new();
	let mut blocks = Vec::with_capacity(cfg.blocks.len());
	for block in &cfg.blocks {
		let slice = &recs[(block.start_pc - 1) as usize..=(block.end_pc - 1) as usize];
		let mut variants: Vec<CcpdBlockVariant> = Vec::with_capacity(CCPD_VARIANTS as usize);
		let mut used_perm_codes: Vec<u8> = Vec::new();
		let mut payloads: Vec<Vec<u8>> = Vec::new();
		for v in 0..CCPD_VARIANTS {
			// Distinct lane permutation per variant (deterministic
			// probing keeps all four codes different within a block).
			let mut code =
				(variant_word(build_salt, fn_index as u32, block.id as u32, v, 1) % 24) as u8;
			while used_perm_codes.contains(&code) {
				code = (code + 1) % 24;
			}
			used_perm_codes.push(code);
			let perm = super::compiler::operand_perm_from_code(code);
			let mut epoch =
				variant_word(build_salt, fn_index as u32, block.id as u32, v, 2) % MOD28;
			while variants.iter().any(|x: &CcpdBlockVariant| x.page_epoch == epoch) {
				epoch = (epoch + 1) % MOD28;
			}
			let mut mask_seed =
				variant_word(build_salt, fn_index as u32, block.id as u32, v, 3);
			let mask_step =
				variant_word(build_salt, fn_index as u32, block.id as u32, v, 4) | 1;
			let mut payload = encode_variant_payload(
				slice, &perm, mask_seed, epoch, mask_step, map, s_idx, fn_index as u32,
				block.id as u32,
			);
			// Degenerate guard (plan 7.6): four identical payloads
			// would silently drop the diversity this feature exists
			// for; re-seed deterministically instead of emitting them.
			let mut guard = 0u32;
			while payloads.iter().any(|p| *p == payload) && guard < 64 {
				guard += 1;
				mask_seed = mask_seed.wrapping_add(0x9e37 + guard);
				payload = encode_variant_payload(
					slice, &perm, mask_seed, epoch, mask_step, map, s_idx,
					fn_index as u32, block.id as u32,
				);
			}
			if payloads.iter().any(|p| *p == payload) {
				return Err(format!(
					"ccpd: cannot diversify variant {v} of block {}",
					block.id
				));
			}
			// Roundtrip self-check: every variant must decode to the
			// exact canonical slice (plan 7.6 / 11.2), at build time.
			let decoded = decode_variant_payload(
				&payload,
				slice.len(),
				&perm,
				mask_seed,
				epoch,
				mask_step,
				fn_index as u32,
				block.id as u32,
			)?;
			for (i, rec) in slice.iter().enumerate() {
				let (opb, lanes) = decoded[i];
				if opb != opcode_byte(rec, map, s_idx)
					|| lanes != [rec.a, rec.b, rec.c, rec.d]
				{
					return Err(format!(
						"ccpd: variant {v} of block {} record {} does not roundtrip",
						block.id, i
					));
				}
			}
			let offset = bank.len() as u32;
			let len = payload.len() as u32;
			bank.extend_from_slice(&payload);
			payloads.push(payload);
			variants.push(CcpdBlockVariant {
				variant_id: v,
				operand_perm: perm,
				operand_perm_code: code,
				page_epoch: epoch,
				payload_offset: offset,
				payload_len: len,
				mask_seed,
				mask_step,
			});
		}
		blocks.push(CcpdBlockVariants {
			block_id: block.id,
			start_pc: block.start_pc,
			record_count: slice.len() as u16,
			variants: variants
				.try_into()
				.expect("exactly CCPD_VARIANTS variants are built"),
		});
	}
	let closure_sites = recs
		.iter()
		.enumerate()
		.filter(|(_, r)| r.kind == FKind::Base(Op::Closure))
		.map(|(i, r)| (i as u16 + 1, r.b))
		.collect();
	Ok(CcpdFunctionData {
		cfg,
		blocks,
		bank,
		build_salt,
		fn_perm_code,
		nop_code: map.code_for(Op::Nop, s_idx),
		nop_alias: map.nop_alias[s_idx],
		clos_code: map.code_for(Op::Closure, s_idx),
		closure_sites,
	})
}

/// Integrity checksums over the payload bank, mirroring the shape of
/// the legacy SA..SD lane sums (`ck` plain, `ck2` position-weighted).
/// The compiler stores these in `operand_sums[fi]` under `--ccpd`; the
/// Luau parser recomputes them over the bank bytes it read, so an
/// untampered artifact reproduces them exactly.
pub fn bank_checksums(bank: &[u8]) -> (u64, u64) {
	let mut ck: u64 = 0;
	let mut ck2: u64 = 0;
	for (i, &b) in bank.iter().enumerate() {
		ck = (ck + b as u64) % 4_294_967_296;
		ck2 = (ck2 + (i as u64 + 1) * b as u64) % 4_294_967_296;
	}
	(ck, ck2)
}

/// Inverse of `compiler::operand_perm_from_code`: the Lehmer code
/// (0..23) of a lane permutation, or `None` if it is not a bijection.
pub fn perm_code_of(perm: &[u8; 4]) -> Option<u8> {
	(0..24).find(|&code| super::compiler::operand_perm_from_code(code) == *perm)
}

fn wvar(out: &mut Vec<u8>, v: u32) {
	isa::encode_varint(out, v);
}

impl CcpdFunctionData {
	/// Serialize the CCPD directory (prototype section id 8). The
	/// payload bank is *not* repeated here; it is the code section's
	/// content under `--ccpd` (plan 8.3).
	pub fn encode_section(&self) -> Vec<u8> {
		let mut out = Vec::new();
		out.push(CCPD_FORMAT_VERSION);
		out.push(CCPD_VARIANTS);
		wvar(&mut out, self.cfg.blocks.len() as u32);
		wvar(&mut out, self.cfg.edges.len() as u32);
		wvar(&mut out, self.build_salt % MOD28);
		out.push(self.fn_perm_code);
		out.push(self.nop_code);
		out.push(self.nop_alias);
		out.push(self.clos_code);
		wvar(&mut out, self.entry_edge_id());
		wvar(&mut out, self.total_records());
		for bv in &self.blocks {
			wvar(&mut out, bv.block_id as u32);
			wvar(&mut out, bv.start_pc as u32);
			let end_pc = bv.start_pc as u32 + bv.record_count as u32 - 1;
			wvar(&mut out, end_pc);
			let incoming = self.cfg.incoming(bv.block_id);
			wvar(&mut out, incoming.len() as u32);
			for e in incoming {
				wvar(&mut out, e.id.0);
				out.push(e.kind.code() | if e.backedge { 0x80 } else { 0 });
				wvar(&mut out, e.source_pc.map(|p| p as u32).unwrap_or(0));
			}
			for v in &bv.variants {
				out.push(v.variant_id);
				out.push(v.operand_perm_code);
				wvar(&mut out, v.page_epoch);
				wvar(&mut out, v.payload_offset);
				wvar(&mut out, v.payload_len);
				wvar(&mut out, v.mask_seed);
				wvar(&mut out, v.mask_step);
			}
		}
		wvar(&mut out, self.cfg.pc_to_block.len() as u32);
		for i in 0..self.cfg.pc_to_block.len() {
			wvar(&mut out, self.cfg.jump_edge_by_pc[i].map(|e| e + 1).unwrap_or(0));
			wvar(
				&mut out,
				self.cfg.branch_taken_edge_by_pc[i].map(|e| e + 1).unwrap_or(0),
			);
			wvar(
				&mut out,
				self.cfg.branch_fallthrough_edge_by_pc[i].map(|e| e + 1).unwrap_or(0),
			);
			wvar(
				&mut out,
				self.cfg.fallthrough_edge_by_pc[i].map(|e| e + 1).unwrap_or(0),
			);
		}
		wvar(&mut out, self.closure_sites.len() as u32);
		for &(site_pc, child) in &self.closure_sites {
			wvar(&mut out, site_pc as u32);
			wvar(&mut out, child as u32);
		}
		let exit_ids: Vec<u32> = self
			.cfg
			.edges
			.iter()
			.filter(|e| e.target_block == EXIT_BLOCK)
			.map(|e| e.id.0)
			.collect();
		wvar(&mut out, exit_ids.len() as u32);
		for id in exit_ids {
			wvar(&mut out, id);
		}
		out
	}
}

/// Parsed form of the CCPD directory (compiler/test side mirror of the
/// Luau parser in `template.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct CcpdSection {
	pub build_salt: u32,
	pub fn_perm_code: u8,
	pub nop_code: u8,
	pub nop_alias: u8,
	pub clos_code: u8,
	pub entry_edge_id: u32,
	pub total_records: u32,
	pub blocks: Vec<CcpdSectionBlock>,
	pub jump_edge_by_pc: Vec<Option<u32>>,
	pub branch_taken_edge_by_pc: Vec<Option<u32>>,
	pub branch_fallthrough_edge_by_pc: Vec<Option<u32>>,
	pub fallthrough_edge_by_pc: Vec<Option<u32>>,
	pub closure_sites: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CcpdSectionBlock {
	pub block_id: u16,
	pub start_pc: u16,
	pub end_pc: u16,
	pub incoming: Vec<(u32, u8, u32)>,
	pub variants: [CcpdBlockVariant; CCPD_VARIANTS as usize],
}

struct Reader<'a> {
	bytes: &'a [u8],
	pos: usize,
}

impl<'a> Reader<'a> {
	fn rb(&mut self) -> Result<u8, String> {
		let b = self
			.bytes
			.get(self.pos)
			.copied()
			.ok_or("ccpd section: truncated")?;
		self.pos += 1;
		Ok(b)
	}

	fn var(&mut self) -> Result<u32, String> {
		let (v, next) =
			isa::decode_varint(self.bytes, self.pos).ok_or("ccpd section: truncated varint")?;
		self.pos = next;
		Ok(v)
	}
}

/// Fail-closed directory decoder (plan 8.5). `bank_len` bounds every
/// payload offset/length pair.
pub fn decode_section(bytes: &[u8], bank_len: usize) -> Result<CcpdSection, String> {
	let mut r = Reader { bytes, pos: 0 };
	if r.rb()? != CCPD_FORMAT_VERSION {
		return Err("ccpd section: unknown format version".to_string());
	}
	if r.rb()? != CCPD_VARIANTS {
		return Err("ccpd section: variant_count must be 4".to_string());
	}
	let block_count = r.var()? as usize;
	let edge_count = r.var()? as usize;
	let build_salt = r.var()?;
	let fn_perm_code = r.rb()?;
	if fn_perm_code >= 24 {
		return Err("ccpd section: fn perm code out of range".to_string());
	}
	let nop_code = r.rb()?;
	let nop_alias = r.rb()?;
	let clos_code = r.rb()?;
	let entry_edge_id = r.var()?;
	let total_records = r.var()?;
	let mut blocks = Vec::with_capacity(block_count);
	let mut edge_ids: Vec<u32> = Vec::new();
	let mut spans: Vec<(u64, u64)> = Vec::new();
	let mut expected_start = 1u32;
	for i in 0..block_count {
		let block_id = r.var()?;
		if block_id as usize != i {
			return Err("ccpd section: block ids not sequential".to_string());
		}
		let start_pc = r.var()?;
		let end_pc = r.var()?;
		if start_pc != expected_start || end_pc < start_pc {
			return Err("ccpd section: block ranges not contiguous".to_string());
		}
		expected_start = end_pc + 1;
		let incoming_count = r.var()? as usize;
		let mut incoming = Vec::with_capacity(incoming_count);
		for _ in 0..incoming_count {
			let edge_id = r.var()?;
			let kind_byte = r.rb()?;
			if CcpdEdgeKind::from_code(kind_byte & 0x7f).is_none() {
				return Err("ccpd section: unknown edge kind".to_string());
			}
			let source_pc = r.var()?;
			if edge_ids.contains(&edge_id) {
				return Err("ccpd section: edge id listed twice".to_string());
			}
			edge_ids.push(edge_id);
			incoming.push((edge_id, kind_byte, source_pc));
		}
		let mut variants = Vec::with_capacity(CCPD_VARIANTS as usize);
		for v in 0..CCPD_VARIANTS {
			let variant_id = r.rb()?;
			if variant_id != v {
				return Err("ccpd section: variant ids not 0..3 in order".to_string());
			}
			let operand_perm_code = r.rb()?;
			if operand_perm_code >= 24 {
				return Err("ccpd section: operand perm code out of range".to_string());
			}
			let page_epoch = r.var()?;
			let payload_offset = r.var()?;
			let payload_len = r.var()?;
			let mask_seed = r.var()?;
			let mask_step = r.var()?;
			let end = payload_offset as u64 + payload_len as u64;
			if end > bank_len as u64 {
				return Err("ccpd section: payload out of bank bounds".to_string());
			}
			// Format minimum (repair plan P2): every record needs one
			// opcode byte plus four one-byte varints, so a shorter
			// payload cannot decode this block. The runtime parser
			// enforces the same rule over `pl`.
			if (payload_len as u64) < 5 * (end_pc as u64 - start_pc as u64 + 1) {
				return Err("ccpd section: payload shorter than format minimum".to_string());
			}
			spans.push((payload_offset as u64, end));
			variants.push(CcpdBlockVariant {
				variant_id,
				operand_perm: super::compiler::operand_perm_from_code(operand_perm_code),
				operand_perm_code,
				page_epoch,
				payload_offset,
				payload_len,
				mask_seed,
				mask_step,
			});
		}
		blocks.push(CcpdSectionBlock {
			block_id: block_id as u16,
			start_pc: start_pc as u16,
			end_pc: end_pc as u16,
			incoming,
			variants: variants
				.try_into()
				.expect("exactly CCPD_VARIANTS variants were read"),
		});
	}
	if expected_start != total_records + 1 {
		return Err("ccpd section: blocks do not cover total_records".to_string());
	}
	if !edge_ids.contains(&entry_edge_id) {
		return Err("ccpd section: entry edge not declared".to_string());
	}
	// The encoder appends payloads back-to-back in block/variant
	// order, so the spans must tile the bank exactly: no overlap,
	// no gap (repair plan P2; the runtime parser mirrors this).
	let mut cursor = 0u64;
	for (start, end) in &spans {
		if *start != cursor {
			return Err("ccpd section: payload spans overlap or leave a gap".to_string());
		}
		cursor = *end;
	}
	if cursor != bank_len as u64 {
		return Err("ccpd section: payload spans do not cover the bank".to_string());
	}
	let map_count = r.var()?;
	if map_count != total_records {
		return Err("ccpd section: transition map size mismatch".to_string());
	}
	// The four maps are interleaved per PC on the wire.
	let _ = map_count;
	let mut jump = Vec::with_capacity(map_count as usize);
	let mut taken = Vec::with_capacity(map_count as usize);
	let mut bfall = Vec::with_capacity(map_count as usize);
	let mut fall = Vec::with_capacity(map_count as usize);
	for _ in 0..map_count {
		let mut one = |r: &mut Reader<'_>| -> Result<Option<u32>, String> {
			let v = r.var()?;
			if v == 0 {
				Ok(None)
			} else {
				let id = v - 1;
				if id as usize >= edge_count {
					Err("ccpd section: transition map references unknown edge".to_string())
				} else {
					Ok(Some(id))
				}
			}
		};
		jump.push(one(&mut r)?);
		taken.push(one(&mut r)?);
		bfall.push(one(&mut r)?);
		fall.push(one(&mut r)?);
	}
	let site_count = r.var()?;
	let mut closure_sites = Vec::with_capacity(site_count as usize);
	for _ in 0..site_count {
		let site_pc = r.var()?;
		let child = r.var()?;
		if site_pc < 1 || site_pc > total_records {
			return Err("ccpd section: closure site pc out of range".to_string());
		}
		closure_sites.push((site_pc, child));
	}
	let exit_count = r.var()?;
	for _ in 0..exit_count {
		let id = r.var()?;
		if edge_ids.contains(&id) {
			return Err("ccpd section: duplicate exit edge id".to_string());
		}
		edge_ids.push(id);
	}
	// Every declared edge is either incoming to some block or an exit
	// edge, and every map reference must resolve to one of them.
	if edge_ids.len() != edge_count {
		return Err("ccpd section: edge_count does not match declared edges".to_string());
	}
	for map in [&jump, &taken, &bfall, &fall] {
		for id in map.iter().flatten() {
			if !edge_ids.contains(id) {
				return Err(
					"ccpd section: transition map references undeclared edge".to_string()
				);
			}
		}
	}
	if r.pos != bytes.len() {
		return Err("ccpd section: trailing bytes".to_string());
	}
	Ok(CcpdSection {
		build_salt,
		fn_perm_code,
		nop_code,
		nop_alias,
		clos_code,
		entry_edge_id,
		total_records,
		blocks,
		jump_edge_by_pc: jump,
		branch_taken_edge_by_pc: taken,
		branch_fallthrough_edge_by_pc: bfall,
		fallthrough_edge_by_pc: fall,
		closure_sites,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::rng::Rng;

	fn rec(op: Op, a: u16, b: u16, c: u16, d: u16) -> FRec {
		FRec { kind: FKind::Base(op), a, b, c, d }
	}

	fn fused(k: u8, a: u16, b: u16, c: u16, d: u16) -> FRec {
		FRec { kind: FKind::Fused(k), a, b, c, d }
	}

	#[test]
	fn cfg_single_block_function() {
		let recs = vec![rec(Op::LoadK, 1, 2, 0, 0), rec(Op::Return, 0, 0, 0, 0)];
		let cfg = build_cfg(&recs, &[1]).expect("cfg");
		assert_eq!(cfg.blocks.len(), 1);
		assert_eq!(cfg.edges.len(), 1, "only the Entry edge");
		assert_eq!(cfg.edges[0].kind, CcpdEdgeKind::Entry);
		assert_eq!(cfg.pc_to_block, vec![0, 0]);
	}

	#[test]
	fn cfg_jump_and_leaders() {
		// 1: Jmp -> 3 ; 2: LoadK ; 3: Return
		let recs = vec![
			rec(Op::Jmp, 0, 3, 0, 0),
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 2, 3]).expect("cfg");
		assert_eq!(cfg.blocks.len(), 3);
		let jump = cfg.jump_edge_by_pc[0].expect("jump edge");
		let e = cfg.edges[jump as usize];
		assert_eq!(e.kind, CcpdEdgeKind::Jump);
		assert_eq!(e.target_block, 2);
		assert_eq!(e.source_pc, Some(1));
		// Middle block falls through.
		assert!(cfg.fallthrough_edge_by_pc[1].is_some());
	}

	#[test]
	fn cfg_branch_both_outcomes() {
		// 1: Jf -> 3 ; 2: LoadK ; 3: Return
		let recs = vec![
			rec(Op::Jf, 0, 3, 0, 0),
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 2, 3]).expect("cfg");
		let taken = cfg.branch_taken_edge_by_pc[0].expect("taken");
		let fall = cfg.branch_fallthrough_edge_by_pc[0].expect("fallthrough");
		assert_ne!(taken, fall, "outcomes own distinct EdgeIds");
		assert_eq!(cfg.edges[taken as usize].target_block, 2);
		assert_eq!(cfg.edges[fall as usize].target_block, 1);
	}

	#[test]
	fn cfg_branch_same_target_keeps_distinct_edges() {
		// 1: Jt -> 2 ; 2: Return ; both outcomes land in block 1.
		let recs = vec![rec(Op::Jt, 0, 2, 0, 0), rec(Op::Return, 0, 0, 0, 0)];
		let cfg = build_cfg(&recs, &[1, 2]).expect("cfg");
		let taken = cfg.branch_taken_edge_by_pc[0].expect("taken");
		let fall = cfg.branch_fallthrough_edge_by_pc[0].expect("fallthrough");
		assert_ne!(taken, fall);
		assert_eq!(cfg.edges[taken as usize].target_block, 1);
		assert_eq!(cfg.edges[fall as usize].target_block, 1);
		assert_eq!(cfg.incoming(1).len(), 2, "block 1 has two distinct incoming edges");
	}

	#[test]
	fn cfg_loop_backedge_flagged() {
		// 1: LoadK ; 2: Jf -> 1 ; 3: Return
		let recs = vec![
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Jf, 0, 1, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 2, 3]).expect("cfg");
		let taken = cfg.edges[cfg.branch_taken_edge_by_pc[1].expect("taken") as usize];
		assert_eq!(taken.target_block, 0);
		assert!(taken.backedge, "taken edge to an earlier block is a backedge");
		let fall = cfg.edges[cfg.branch_fallthrough_edge_by_pc[1].expect("fall") as usize];
		assert!(!fall.backedge);
	}

	#[test]
	fn cfg_rejects_nonleader_target() {
		let recs = vec![
			rec(Op::Jmp, 0, 2, 0, 0),
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		assert!(build_cfg(&recs, &[1, 3]).is_err(), "target 2 is not a leader");
	}

	#[test]
	fn cfg_rejects_out_of_range_target() {
		let recs = vec![rec(Op::Jmp, 0, 9, 0, 0), rec(Op::Return, 0, 0, 0, 0)];
		assert!(build_cfg(&recs, &[1]).is_err());
		let recs0 = vec![rec(Op::Jmp, 0, 0, 0, 0), rec(Op::Return, 0, 0, 0, 0)];
		assert!(build_cfg(&recs0, &[1]).is_err());
	}

	#[test]
	fn cfg_fused_records_are_not_branches() {
		// Fused pair stream ending in Return; block boundary after fused.
		let recs = vec![
			fused(0, 1, 2, 3, 4),
			rec(Op::LoadK, 5, 6, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 3]).expect("cfg");
		assert_eq!(cfg.blocks.len(), 2);
		assert!(cfg.fallthrough_edge_by_pc[0].is_none(), "fallthrough hangs on block end");
		assert!(cfg.fallthrough_edge_by_pc[1].is_some());
		assert!(cfg.jump_edge_by_pc.iter().all(|e| e.is_none()));
	}

	#[test]
	fn cfg_multiple_returns_have_no_successors() {
		// 1: Jf -> 3 ; 2: Return ; 3: Return
		let recs = vec![
			rec(Op::Jf, 0, 3, 0, 0),
			rec(Op::Return, 1, 0, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 2, 3]).expect("cfg");
		for e in &cfg.edges {
			assert_ne!(e.source_block, Some(1), "return block has no outgoing edge");
			assert_ne!(e.source_block, Some(2), "return block has no outgoing edge");
		}
	}

	fn sample_recs() -> Vec<FRec> {
		vec![
			rec(Op::LoadK, 1, 700, 0, 0),
			rec(Op::Move, 2, 1, 0, 0),
			fused(0, 3, 900, 4, 5),
			rec(Op::Jf, 2, 6, 0, 0),
			rec(Op::LoadK, 3, 701, 0, 0),
			rec(Op::Return, 1, 0, 0, 0),
		]
	}

	#[test]
	fn variants_roundtrip_and_differ() {
		let mut rng = Rng::new(5);
		let map = OpMap::new(&mut rng);
		let recs = sample_recs();
		let data = build_function_ccpd(&recs, &[1, 5, 6], &map, 2, 3, 0x00AB_CDEF, 7)
			.expect("build ccpd data");
		let expected: Vec<(u8, [u16; 4])> = recs
			.iter()
			.map(|r| (opcode_byte(r, &map, 2), [r.a, r.b, r.c, r.d]))
			.collect();
		for bv in &data.blocks {
			let slice = &expected[(bv.start_pc - 1) as usize
				..(bv.start_pc - 1) as usize + bv.record_count as usize];
			let mut payloads = Vec::new();
			for v in &bv.variants {
				assert!(v.operand_perm.iter().collect::<std::collections::HashSet<_>>().len() == 4);
				let payload = &data.bank
					[v.payload_offset as usize..(v.payload_offset + v.payload_len) as usize];
				payloads.push(payload.to_vec());
				let decoded = decode_variant_payload(
					payload,
					bv.record_count as usize,
					&v.operand_perm,
					v.mask_seed,
					v.page_epoch,
					v.mask_step,
					3,
					bv.block_id as u32,
				)
				.expect("decode variant");
				assert_eq!(decoded, slice, "variant {} block {} roundtrip", v.variant_id, bv.block_id);
			}
			assert!(
				!(payloads[0] == payloads[1]
					&& payloads[1] == payloads[2]
					&& payloads[2] == payloads[3]),
				"variants of a block must not all be byte-identical"
			);
		}
	}

	#[test]
	fn cfg_jt_both_outcomes() {
		// 1: Jt -> 3 ; 2: LoadK ; 3: Return
		let recs = vec![
			rec(Op::Jt, 0, 3, 0, 0),
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 2, 3]).expect("cfg");
		let taken = cfg.branch_taken_edge_by_pc[0].expect("taken");
		let fall = cfg.branch_fallthrough_edge_by_pc[0].expect("fallthrough");
		assert_ne!(taken, fall);
		assert_eq!(cfg.edges[taken as usize].kind, CcpdEdgeKind::BranchTaken);
		assert_eq!(cfg.edges[taken as usize].target_block, 2);
		assert_eq!(cfg.edges[fall as usize].kind, CcpdEdgeKind::BranchFallthrough);
		assert_eq!(cfg.edges[fall as usize].target_block, 1);
	}

	#[test]
	fn cfg_return_only_function() {
		let recs = vec![rec(Op::Return, 0, 0, 0, 0)];
		let cfg = build_cfg(&recs, &[1]).expect("cfg");
		assert_eq!(cfg.blocks.len(), 1);
		assert_eq!(cfg.edges.len(), 1, "entry edge only, return has no successor");
		assert_eq!(cfg.incoming(0).len(), 1);
	}

	#[test]
	fn cfg_noise_records_create_no_branch_edges() {
		// Nop / Canary / dead-register noise records are ordinary
		// fallthrough content in the final stream.
		let recs = vec![
			rec(Op::Nop, 0, 0, 0, 0),
			rec(Op::Canary, 1, 0, 0, 0),
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1]).expect("cfg");
		assert_eq!(cfg.blocks.len(), 1);
		assert!(cfg.jump_edge_by_pc.iter().all(|e| e.is_none()));
		assert!(cfg.branch_taken_edge_by_pc.iter().all(|e| e.is_none()));
	}

	#[test]
	fn cfg_tail_jump_is_exit_edge() {
		// 1: LoadK ; 2: Jmp -> 3 (= n + 1, the tail merge exit)
		let recs = vec![rec(Op::LoadK, 1, 2, 0, 0), rec(Op::Jmp, 0, 3, 0, 0)];
		let cfg = build_cfg(&recs, &[1]).expect("cfg");
		let id = cfg.jump_edge_by_pc[1].expect("jump edge");
		let e = cfg.edges[id as usize];
		assert_eq!(e.target_block, EXIT_BLOCK);
		assert_eq!(e.kind, CcpdEdgeKind::Jump);
		assert!(cfg.blocks.iter().all(|b| cfg.incoming(b.id).iter().all(|x| x.id != e.id)),
			"exit edge is incoming to no block");
	}

	#[test]
	fn cfg_tail_taken_is_exit_edge() {
		// 1: LoadK ; 2: Jf -> 4 (= n + 1) ; 3: Return
		// The taken outcome leaves the function at the tail while the
		// fallthrough outcome lands on the Return record.
		let recs = vec![
			rec(Op::LoadK, 1, 2, 0, 0),
			rec(Op::Jf, 0, 4, 0, 0),
			rec(Op::Return, 0, 0, 0, 0),
		];
		let cfg = build_cfg(&recs, &[1, 3]).expect("cfg");
		let taken = cfg.edges[cfg.branch_taken_edge_by_pc[1].expect("taken edge") as usize];
		assert_eq!(taken.kind, CcpdEdgeKind::BranchTaken);
		assert_eq!(taken.target_block, EXIT_BLOCK);
		let fall = cfg.edges[cfg.branch_fallthrough_edge_by_pc[1].expect("fall edge") as usize];
		assert_eq!(fall.kind, CcpdEdgeKind::BranchFallthrough);
		assert_eq!(fall.target_block, 1, "fallthrough lands on the Return block");
	}

	#[test]
	fn cfg_tail_fallthrough_is_exit_edge() {
		// 1: LoadK ; 2: Jf -> 1 (last record: taken loops back,
		// fallthrough leaves the function at the tail)
		let recs = vec![rec(Op::LoadK, 1, 2, 0, 0), rec(Op::Jf, 0, 1, 0, 0)];
		let cfg = build_cfg(&recs, &[1, 2]).expect("cfg");
		let taken = cfg.edges[cfg.branch_taken_edge_by_pc[1].expect("taken edge") as usize];
		assert_eq!(taken.kind, CcpdEdgeKind::BranchTaken);
		assert_eq!(taken.target_block, 0);
		assert!(taken.backedge, "taken edge loops back to the first block");
		let fall = cfg.edges[cfg.branch_fallthrough_edge_by_pc[1].expect("fall edge") as usize];
		assert_eq!(fall.kind, CcpdEdgeKind::BranchFallthrough);
		assert_eq!(fall.target_block, EXIT_BLOCK);
		assert!(cfg.blocks.iter().all(|b| cfg.incoming(b.id).iter().all(|x| x.id != fall.id)),
			"exit edge is incoming to no block");
	}

	#[test]
	fn cfg_tail_plain_fallthrough_has_no_exit_edge() {
		// 1: LoadK ; 2: LoadK (stream ending on a plain record). The
		// compiler always terminates real functions with a Return
		// record, so this shape has no declared exit edge; the
		// runtime epilogue still treats pc == cd.tn + 1 as the exit
		// continuation, which needs no edge id (repair plan P1).
		let recs = vec![rec(Op::LoadK, 1, 2, 0, 0), rec(Op::LoadK, 3, 4, 0, 0)];
		let cfg = build_cfg(&recs, &[1]).expect("cfg");
		assert_eq!(cfg.edges.len(), 1, "only the Entry edge");
		assert!(cfg.fallthrough_edge_by_pc.iter().all(|e| e.is_none()));
	}

	#[test]
	fn closure_sites_and_variant_diversity() {
		let mut rng = Rng::new(5);
		let map = OpMap::new(&mut rng);
		let recs = vec![
			rec(Op::Closure, 1, 2, 0, 0),
			rec(Op::LoadK, 3, 700, 0, 0),
			rec(Op::Return, 1, 0, 0, 0),
		];
		let data = build_function_ccpd(&recs, &[1], &map, 2, 3, 77, 7).expect("build");
		assert_eq!(data.closure_sites, vec![(1, 2)], "site pc + child index from the Closure record");
		for bv in &data.blocks {
			let codes: std::collections::HashSet<u8> =
				bv.variants.iter().map(|v| v.operand_perm_code).collect();
			assert_eq!(codes.len(), 4, "four distinct lane permutations per block");
			let epochs: std::collections::HashSet<u32> =
				bv.variants.iter().map(|v| v.page_epoch).collect();
			assert_eq!(epochs.len(), 4, "four distinct page epochs per block");
		}
	}

	#[test]
	fn section_roundtrip() {
		let mut rng = Rng::new(5);
		let map = OpMap::new(&mut rng);
		let recs = sample_recs();
		let data = build_function_ccpd(&recs, &[1, 5, 6], &map, 2, 3, 0x00AB_CDEF, 7)
			.expect("build ccpd data");
		let bytes = data.encode_section();
		let decoded = decode_section(&bytes, data.bank.len()).expect("decode section");
		assert_eq!(decoded.build_salt, 0x00AB_CDEF);
		assert_eq!(decoded.fn_perm_code, 7);
		assert_eq!(decoded.total_records, 6);
		assert_eq!(decoded.blocks.len(), data.blocks.len());
		assert_eq!(decoded.jump_edge_by_pc, data.cfg.jump_edge_by_pc);
		assert_eq!(decoded.branch_taken_edge_by_pc, data.cfg.branch_taken_edge_by_pc);
		assert_eq!(
			decoded.branch_fallthrough_edge_by_pc,
			data.cfg.branch_fallthrough_edge_by_pc
		);
		assert_eq!(decoded.fallthrough_edge_by_pc, data.cfg.fallthrough_edge_by_pc);
		for (a, b) in decoded.blocks.iter().zip(data.blocks.iter()) {
			assert_eq!(a.variants, b.variants);
			assert_eq!(a.start_pc, b.start_pc);
		}
	}

	#[test]
	fn section_rejects_tampering() {
		let mut rng = Rng::new(5);
		let map = OpMap::new(&mut rng);
		let recs = sample_recs();
		let data = build_function_ccpd(&recs, &[1, 5, 6], &map, 2, 3, 99, 7).expect("data");
		// Unknown version.
		let mut bytes = data.encode_section();
		bytes[0] = 9;
		assert!(decode_section(&bytes, data.bank.len()).is_err());
		// Wrong variant count.
		let mut bytes = data.encode_section();
		bytes[1] = 3;
		assert!(decode_section(&bytes, data.bank.len()).is_err());
		// Perm code out of range.
		let mut tampered = data.clone();
		tampered.blocks[0].variants[0].operand_perm_code = 24;
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
		// Payload out of bank bounds.
		let mut tampered = data.clone();
		tampered.blocks[0].variants[0].payload_len = u32::MAX / 2;
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
		// Transition map referencing an unknown edge.
		let mut tampered = data.clone();
		tampered.cfg.jump_edge_by_pc[0] = Some(4000);
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
	}

	#[test]
	fn payload_bounds_reject_truncation_and_trailing() {
		let mut rng = Rng::new(5);
		let map = OpMap::new(&mut rng);
		let recs = sample_recs();
		let data = build_function_ccpd(&recs, &[1, 5, 6], &map, 2, 3, 7, 7).expect("data");
		let bv = &data.blocks[0];
		let v = &bv.variants[0];
		let payload = &data.bank
			[v.payload_offset as usize..(v.payload_offset + v.payload_len) as usize];
		let decode = |p: &[u8]| {
			decode_variant_payload(
				p,
				bv.record_count as usize,
				&v.operand_perm,
				v.mask_seed,
				v.page_epoch,
				v.mask_step,
				3,
				bv.block_id as u32,
			)
		};
		decode(payload).expect("well-formed payload decodes");
		// One byte short: the last lane column is truncated.
		assert!(decode(&payload[..payload.len() - 1]).is_err());
		// One trailing byte the decoder never consumes.
		let mut trailing = payload.to_vec();
		trailing.push(0);
		assert!(decode(&trailing).is_err(), "trailing byte must be rejected");
		// A varint whose continuation byte is cut off at the end.
		let mut cut = payload.to_vec();
		*cut.last_mut().expect("payload nonempty") = 0xFF;
		assert!(decode(&cut).is_err(), "truncated varint must be rejected");
	}

	#[test]
	fn section_span_rules() {
		let mut rng = Rng::new(5);
		let map = OpMap::new(&mut rng);
		let recs = sample_recs();
		let data = build_function_ccpd(&recs, &[1, 5, 6], &map, 2, 3, 99, 7).expect("data");
		// Exact bank length is accepted.
		assert!(decode_section(&data.encode_section(), data.bank.len()).is_ok());
		// Declared bank one byte short or long: spans no longer
		// cover the bank exactly.
		assert!(decode_section(&data.encode_section(), data.bank.len() - 1).is_err());
		assert!(decode_section(&data.encode_section(), data.bank.len() + 1).is_err());
		// Length extended by one: total span exceeds the bank.
		let mut tampered = data.clone();
		tampered.blocks[0].variants[0].payload_len += 1;
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
		// Below the format minimum of five bytes per record.
		let mut tampered = data.clone();
		tampered.blocks[0].variants[0].payload_len = 4;
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
		// Zero length.
		let mut tampered = data.clone();
		tampered.blocks[0].variants[0].payload_len = 0;
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
		// Overlapping span: variant 1 starts inside variant 0.
		let mut tampered = data.clone();
		tampered.blocks[0].variants[1].payload_offset =
			tampered.blocks[0].variants[0].payload_offset;
		assert!(decode_section(&tampered.encode_section(), data.bank.len()).is_err());
	}
}
