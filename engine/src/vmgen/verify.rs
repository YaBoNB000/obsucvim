//! Stage 1 canonical bytecode verifier.
//!
//! `Op::meta()` is the single source of truth for operand semantics and for
//! how the scatter pass is allowed to remap each lane.  This module consumes
//! that metadata before CCPD construction and before the fused/plain record
//! stream is serialized.  A violation is a compiler failure, never a runtime
//! fallback.

use super::fuse::{FKind, FRec, FUSE_WHITELIST};
use super::isa::{Const, Instr, Op, OperandRole, PcEffect, ScatterMap};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerifierLimits {
	pub nregs: u16,
	/// Highest legal value in a direct register lane.  This differs from
	/// `nregs` after scatter, where direct lanes hold sparse physical slots
	/// while base lanes still hold logical frame offsets.
	pub max_register_value: u16,
	pub nconsts: usize,
	/// LoadStr metadata is embedded in the const operand itself, so it is not
	/// simply a pool index.  The maximum stripped index is checked here.
	pub nupvalues: usize,
	pub nprototypes: usize,
	pub max_canary_id: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationError {
	pub function: Option<usize>,
	pub pc: Option<usize>,
	pub message: String,
}

impl VerificationError {
	fn at(function: usize, pc: usize, message: impl Into<String>) -> Self {
		Self { function: Some(function), pc: Some(pc), message: message.into() }
	}
	fn global(message: impl Into<String>) -> Self {
		Self { function: None, pc: None, message: message.into() }
	}
}

impl std::fmt::Display for VerificationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match (self.function, self.pc) {
			(Some(function), Some(pc)) => write!(f, "bytecode verifier: fn {function} pc {pc}: {}", self.message),
			(Some(function), None) => write!(f, "bytecode verifier: fn {function}: {}", self.message),
			_ => write!(f, "bytecode verifier: {}", self.message),
		}
	}
}
impl std::error::Error for VerificationError {}

pub type VerifyResult<T> = Result<T, VerificationError>;

fn err<T>(function: usize, pc: usize, message: impl Into<String>) -> VerifyResult<T> {
	Err(VerificationError::at(function, pc, message))
}

fn check_reg(
	function: usize,
	pc: usize,
	value: u16,
	limits: &VerifierLimits,
	what: &str,
) -> VerifyResult<()> {
	if value > limits.max_register_value {
		return err(function, pc, format!("{what} register {value} outside 0..={}", limits.max_register_value));
	}
	Ok(())
}

fn check_base(
	function: usize,
	pc: usize,
	value: u16,
	limits: &VerifierLimits,
	what: &str,
) -> VerifyResult<()> {
	if limits.nregs == 0 || value >= limits.nregs {
		return err(function, pc, format!("{what} base {value} outside 0..{}", limits.nregs));
	}
	Ok(())
}

fn check_reg_range(
	function: usize,
	pc: usize,
	base: u16,
	count: u16,
	limits: &VerifierLimits,
	what: &str,
) -> VerifyResult<()> {
	check_reg(function, pc, base, limits, what)?;
	if count != 255 {
		let end = base as u32 + count as u32;
		if end > limits.nregs as u32 {
			return err(function, pc, format!("{what} register window ends at {end}, nregs {}", limits.nregs));
		}
	}
	Ok(())
}

fn verify_record_roles(
	function: usize,
	pc: usize,
	op: Op,
	operands: [u16; 4],
	record_count: usize,
	limits: &VerifierLimits,
) -> VerifyResult<()> {
	let meta = op.meta();
	let roles = meta.roles();
	for (lane, (&role, &value)) in roles.iter().zip(operands.iter()).enumerate() {
		match role {
			OperandRole::Unused => {}
			OperandRole::RegRead | OperandRole::RegWrite => {
				check_reg(function, pc, value, limits, "register")?;
			}
			OperandRole::RegRange => {
				// The range *length* lives in a neighboring lane for the
				// call-shaped ops; the op-specific checks below cover those.
				check_reg(function, pc, value, limits, "register range")?;
			}
			OperandRole::CallBase => {
				check_reg(function, pc, value, limits, "call base")?;
			}
			OperandRole::RegBase => {
				check_reg(function, pc, value, limits, "register base")?;
			}
			OperandRole::ConstIdx => {
				// LoadStr packs cache policy in the high bits of b; its real
				// index is the low 14 bits.  Other ConstIdx lanes are plain.
				let idx = if op == Op::LoadStr { value & 0x3fff } else { value } as usize;
				if idx >= limits.nconsts {
					return err(function, pc, format!("constant index {idx} outside 0..{}", limits.nconsts));
				}
			}
			OperandRole::UpvalIdx => {
				if value as usize >= limits.nupvalues {
					return err(function, pc, format!("upvalue index {value} outside 0..{}", limits.nupvalues));
				}
			}
			OperandRole::ProtoIdx => {
				if value as usize >= limits.nprototypes {
					return err(function, pc, format!("prototype index {value} outside 0..{}", limits.nprototypes));
				}
			}
			OperandRole::JumpTarget => {
				// Stored targets are 1-based, with len+1 permitted as the
				// terminal/exit sentinel used by tail jumps.
				if value == 0 || value as usize > record_count + 1 {
					return err(function, pc, format!("jump target {value} outside 1..={}", record_count + 1));
				}
			}
			OperandRole::ImmVal => {}
		}
		let _ = lane;
	}

	// Op-specific structural invariants that cannot be captured by single
	// lane roles.  These mirror the compiler emitters and handler reads.
	match op {
		Op::LoadNil => {
			if operands[1] == 0 {
				return err(function, pc, "LoadNil count must be non-zero");
			}
			check_reg_range(function, pc, operands[0], operands[1], limits, "LoadNil")?;
		}
		Op::LoadStr => {
			// c/d carry the full u16 seal seed/step (65521-domain LCG);
			// the ConstIdx side is checked by role_b above.
		}
		Op::MkStr => {
			// The packed length in d is validated at conversion time in
			// the compiler, before the per-function mask scrambles b/c/d;
			// by the final-record stage it is intentionally opaque.
		}
		Op::Canary => {
			if let Some(max_id) = limits.max_canary_id {
				if operands[0] > max_id {
					return err(function, pc, format!("canary id {} exceeds {max_id}", operands[0]));
				}
			} else {
				return err(function, pc, "Canary record in a build without canary capability");
			}
		}
		Op::TabN | Op::VarArgTabN => {
			// a=table register, b=counter register, c=value register (TabN).
			check_reg(function, pc, operands[1], limits, "TabN counter")?;
			if op == Op::TabN {
				check_reg(function, pc, operands[2], limits, "TabN value")?;
			}
		}
		Op::Call => {
			check_reg_range(function, pc, operands[0], operands[1].saturating_add(1), limits, "Call window")?;
			if operands[3] > 1 {
				return err(function, pc, "Call vararg flag must be 0 or 1");
			}
		}
		Op::CallE => {
			check_reg_range(function, pc, operands[0], operands[1].saturating_add(1), limits, "CallE window")?;
			if operands[3] & !0b11 != 0 {
				return err(function, pc, "CallE flags use only tail/vararg bits");
			}
		}
		Op::CallM => {
			check_reg_range(function, pc, operands[0], operands[1].saturating_add(1), limits, "CallM window")?;
			if operands[2] != 255 {
				check_reg_range(function, pc, operands[0], operands[2], limits, "CallM result window")?;
			}
			if operands[3] > 1 {
				return err(function, pc, "CallM vararg flag must be 0 or 1");
			}
		}
		Op::CallT => {
			// b and c nominate the table and counter registers used by the
			// append-tail-call handler; d=2*nfixed+(tail?1:0).
			check_reg(function, pc, operands[1], limits, "CallT table")?;
			check_reg(function, pc, operands[2], limits, "CallT counter")?;
		}
		Op::Return => {
			if operands[1] != 255 {
				check_reg_range(function, pc, operands[0], operands[1], limits, "Return window")?;
			}
			if operands[2] > 1 {
				return err(function, pc, "Return mode flag must be 0 or 1");
			}
		}
		Op::Closure => {
			// The following nups logical capture pseudo-instructions are
			// consumed by the handler via frame state, not represented as
			// records.  The prototype side is checked by role_b.  The
			// closure-site map in CCPD is checked separately by ccpd.rs.
		}
		Op::VarArgTab | Op::VarArgC => {}
		_ => {}
	}
	Ok(())
}

/// Verify the canonical instruction stream after label fixup/canaries but
/// before operand scatter and MkStr masking.  Jump targets are still 1-based
/// record indices and unused lanes retain compiler zeroes.
pub fn verify_pre_scatter(
	function: usize,
	code: &[Instr],
	consts: &[Const],
	nupvalues: usize,
	nprototypes: usize,
	nregs: u16,
	canary_enabled: bool,
) -> VerifyResult<()> {
	if code.is_empty() {
		return err(function, 0, "empty instruction stream");
	}
	if let Some(last) = code.last() {
		if last.op != Op::Return {
			return err(
				function,
				code.len(),
				format!("instruction stream does not end in Return (last op {:?})", last.op),
			);
		}
	}
	let limits = VerifierLimits {
		nregs,
		max_register_value: nregs.saturating_sub(1),
		nconsts: consts.len(),
		nupvalues,
		nprototypes,
		max_canary_id: if canary_enabled { Some(3) } else { None },
	};
	for (idx, ins) in code.iter().enumerate() {
		let pc = idx + 1;
		if ins.op == Op::Jmp && ins.a != 0 {
			return err(function, pc, "Jmp reserved lane a must be zero before noise fill");
		}
		verify_record_roles(function, pc, ins.op, ins.operands(), code.len(), &limits)?;
		// Unused lanes are derived from the canonical OpMeta roles (a
		// lane is unused iff its role is Unused).  MkStr needs no special
		// case here: all four of its lanes carry packed data, so none of
		// them is marked Unused in the metadata.
		let unused = ins.op.meta().unused_lanes();
		for lane in 0..4 {
			if unused[lane] && ins.operands()[lane] != 0 {
				return err(function, pc, format!("unused lane {} is non-zero before noise fill", lane + 1));
			}
		}
		if consts.get(ins.b as usize).is_none() {
			// only relevant for ConstIdx lanes; role check already done
		}
		match ins.op.meta().pc_effect {
			PcEffect::FrameReturn if ins.op != Op::Return => {
				return err(function, pc, "non-Return op marked FrameReturn");
			}
			_ => {}
		}
	}
	Ok(())
}

/// Verify the final plain/fused record stream immediately before CCPD/SoA
/// serialization.  Unused lanes may contain noise at this point, so only
/// metadata-designated semantic lanes and fused vocabulary are constrained.
pub fn verify_final_records(
	function: usize,
	records: &[FRec],
	limits: &VerifierLimits,
	fused_vocab: &[(u8, u8)],
	leaders: &[u16],
) -> VerifyResult<()> {
	if records.is_empty() {
		return err(function, 0, "empty final record stream");
	}
	if !leaders.contains(&1) {
		return err(function, 0, "leader set does not contain the entry record");
	}
	for &leader in leaders {
		if leader == 0 || leader as usize > records.len() + 1 {
			return err(function, 0, format!("leader {leader} outside record stream"));
		}
	}
	let whitelist: std::collections::BTreeSet<(u8, u8)> = FUSE_WHITELIST
		.iter()
		.map(|&(a, b)| (super::isa::op_index(a) as u8, super::isa::op_index(b) as u8))
		.collect();
	for (pair_idx, pair) in fused_vocab.iter().enumerate() {
		if !whitelist.contains(pair) {
			return err(function, 0, format!("fused vocabulary entry {pair_idx} is not whitelisted"));
		}
	}
	let mut saw_return = false;
	for (idx, rec) in records.iter().enumerate() {
		let pc = idx + 1;
		match rec.kind {
			FKind::Base(op) => {
				if op == Op::Return {
					saw_return = true;
				}
				verify_record_roles(function, pc, op, [rec.a, rec.b, rec.c, rec.d], records.len(), limits)?;
				// A branch target that names an actual record must be a
				// leader.  len+1 is the exit sentinel and is exempt.
				let meta = op.meta();
				for (lane, &role) in meta.roles().iter().enumerate() {
					if role == OperandRole::JumpTarget {
						let target = [rec.a, rec.b, rec.c, rec.d][lane];
						if target as usize <= records.len() && !leaders.contains(&target) {
							return err(function, pc, format!("jump target {target} is not a leader"));
						}
					}
				}
			}
			FKind::Fused(k) => {
				let Some(&(head_idx, tail_idx)) = fused_vocab.get(k as usize) else {
					return err(function, pc, format!("fused record references vocabulary index {k}"));
				};
				let head = super::isa::op_from_index(head_idx);
				let tail = super::isa::op_from_index(tail_idx);
				// Fused channel layout is [X.a, X.b, Y.a, Y.b].  Validate
				// each half under its own two-lane role prefix.
				verify_record_roles(function, pc, head, [rec.a, rec.b, 0, 0], records.len(), limits)?;
				verify_record_roles(function, pc, tail, [rec.c, rec.d, 0, 0], records.len(), limits)?;
			}
		}
	}
	if !saw_return {
		return err(function, records.len(), "final record stream contains no Return");
	}
	Ok(())
}

/// Small helper used by tests and mutation tooling.
pub fn verify_single(function: usize, ins: &Instr, limits: &VerifierLimits) -> VerifyResult<()> {
	verify_record_roles(function, 1, ins.op, ins.operands(), 1, limits)
}

/// Structural validation of one op's canonical metadata.  This is the
/// single validation path behind `ensure_metadata_complete`, which the
/// compiler runs on the real compilation path (see `vmgen::compile`);
/// it is also directly unit-testable with deliberately corrupted
/// metadata.  Every rule is an invariant the emitter, handlers, scatter
/// pass, and this verifier already rely on — a violation means the
/// metadata table itself is lying about the instruction encoding, so
/// compilation must fail closed instead of silently continuing.
pub fn validate_op_meta(op: Op, meta: &super::isa::OpMeta) -> VerifyResult<()> {
	let bad = |msg: &str| {
		Err(VerificationError::global(format!(
			"incomplete ISA metadata for {op:?}: {msg}"
		)))
	};
	if meta.op != op {
		return bad("meta.op does not match the opcode it describes");
	}
	let idx = super::isa::op_index(op);
	if meta.name.is_empty() || meta.name != super::isa::OP_NAMES[idx] {
		return bad("meta.name does not match OP_NAMES[op_index]");
	}
	let roles = meta.roles();
	let unused = meta.unused_lanes();
	for lane in 0..4 {
		if unused[lane] != (roles[lane] == OperandRole::Unused) {
			return bad("unused-lane derivation disagrees with lane role");
		}
		if unused[lane] && roles[lane].scatter() != ScatterMap::Identity {
			return bad("an unused lane must never be scatter-remapped");
		}
	}
	let jump_targets = roles.iter().filter(|&&r| r == OperandRole::JumpTarget).count();
	match meta.pc_effect {
		PcEffect::UnconditionalJmp | PcEffect::ConditionalJmp => {
			if jump_targets != 1 {
				return bad("a jump op must have exactly one JumpTarget lane");
			}
		}
		PcEffect::Fallthrough | PcEffect::FrameTransfer | PcEffect::FrameReturn => {
			if jump_targets != 0 {
				return bad("a non-jump op must not have a JumpTarget lane");
			}
		}
	}
	if (meta.pc_effect == PcEffect::UnconditionalJmp) != (op == Op::Jmp) {
		return bad("only Jmp may be an unconditional jump");
	}
	if (meta.pc_effect == PcEffect::FrameReturn) != (op == Op::Return) {
		return bad("only Return may transfer out of the frame");
	}
	if (op == Op::Nop) != unused.iter().all(|&u| u) {
		return bad("Nop must have all lanes unused; every other op uses at least one lane");
	}
	for (lane, &role) in roles.iter().enumerate() {
		match role {
			OperandRole::CallBase => {
				if lane != 0 || meta.pc_effect != PcEffect::FrameTransfer {
					return bad("CallBase is only legal as lane a of a frame-transfer call op");
				}
			}
			OperandRole::RegBase => {
				if lane != 0 {
					return bad("RegBase is only legal as lane a (base of a base+count range)");
				}
			}
			OperandRole::ProtoIdx => {
				if op != Op::Closure || lane != 1 {
					return bad("ProtoIdx is only legal as Closure lane b");
				}
			}
			_ => {}
		}
	}
	if meta.pc_effect == PcEffect::FrameTransfer && roles[0] != OperandRole::CallBase {
		return bad("a frame-transfer op must take its call base in lane a");
	}
	Ok(())
}

/// Validate the metadata of every opcode in the ISA.  Called once from
/// `vmgen::compile` so a broken metadata table fails the build on the
/// real compilation path, not just in unit tests.
pub fn ensure_metadata_complete() -> VerifyResult<()> {
	if super::isa::ALL_OPS.len() != super::isa::N_OPS {
		return Err(VerificationError::global(
			"ISA opcode table length does not match N_OPS".to_string(),
		));
	}
	for (idx, &op) in super::isa::ALL_OPS.iter().enumerate() {
		if super::isa::op_index(op) != idx || Op::from_index(idx) != Some(op) {
			return Err(VerificationError::global(format!(
				"ISA opcode table is not index-consistent at {op:?}"
			)));
		}
		validate_op_meta(op, &op.meta())?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::vmgen::isa::{Instr, Op};

	fn limits() -> VerifierLimits {
		VerifierLimits { nregs: 16, max_register_value: 15, nconsts: 8, nupvalues: 2, nprototypes: 3, max_canary_id: Some(3) }
	}

	#[test]
	fn metadata_is_complete_for_all_ops() {
		ensure_metadata_complete().unwrap();
		assert_eq!(super::super::isa::ALL_OPS.len(), 44);
	}

	#[test]
	fn invalid_metadata_is_rejected_by_the_validation_path() {
		// Every real opcode's metadata must validate.
		for &op in super::super::isa::ALL_OPS.iter() {
			assert!(validate_op_meta(op, &op.meta()).is_ok(), "{op:?}");
		}
		// (a) name/op identity corruption
		let mut m = Op::LoadK.meta();
		m.name = "Nope";
		assert!(validate_op_meta(Op::LoadK, &m).is_err());
		let mut m = Op::LoadK.meta();
		m.op = Op::Move;
		assert!(validate_op_meta(Op::LoadK, &m).is_err());
		// (b) a JumpTarget lane on a fallthrough op
		let mut m = Op::LoadK.meta();
		m.role_c = OperandRole::JumpTarget;
		assert!(validate_op_meta(Op::LoadK, &m).is_err());
		// (c) a jump pc_effect with no JumpTarget lane (the historical
		// Closure mislabel)
		let mut m = Op::Closure.meta();
		m.pc_effect = PcEffect::ConditionalJmp;
		assert!(validate_op_meta(Op::Closure, &m).is_err());
		// (d) all-unused lanes on a non-Nop op, and a used lane on Nop
		let mut m = Op::Move.meta();
		m.role_a = OperandRole::Unused;
		m.role_b = OperandRole::Unused;
		assert!(validate_op_meta(Op::Move, &m).is_err());
		let mut m = Op::Nop.meta();
		m.role_a = OperandRole::RegWrite;
		assert!(validate_op_meta(Op::Nop, &m).is_err());
		// (e) FrameReturn on a non-Return op
		let mut m = Op::LoadK.meta();
		m.pc_effect = PcEffect::FrameReturn;
		assert!(validate_op_meta(Op::LoadK, &m).is_err());
		// (f) a frame-transfer op without CallBase in lane a
		let mut m = Op::Call.meta();
		m.role_a = OperandRole::RegWrite;
		assert!(validate_op_meta(Op::Call, &m).is_err());
		// (g) CallBase away from lane a
		let mut m = Op::Call.meta();
		m.role_c = OperandRole::CallBase;
		assert!(validate_op_meta(Op::Call, &m).is_err());
		// (h) ProtoIdx outside Closure lane b
		let mut m = Op::LoadK.meta();
		m.role_b = OperandRole::ProtoIdx;
		assert!(validate_op_meta(Op::LoadK, &m).is_err());
	}

	#[test]
	fn pre_scatter_unused_lane_check_uses_metadata() {
		let consts = vec![Const::Num(1.0)];
		let run = |code: Vec<Instr>| {
			verify_pre_scatter(0, &code, &consts, 0, 0, 16, false)
		};
		// Move lane c is Unused in the metadata: a non-zero value before
		// noise fill is a compiler bug and must be rejected.
		assert!(run(vec![
			Instr::abcd(Op::Move, 0, 0, 7, 0),
			Instr::abcd(Op::Return, 0, 0, 0, 0),
		])
		.is_err());
		// VarArgTab lane b is unused (constant 0 from the emitter).
		assert!(run(vec![
			Instr::abcd(Op::VarArgTab, 0, 3, 0, 0),
			Instr::abcd(Op::Return, 0, 0, 0, 0),
		])
		.is_err());
		assert!(run(vec![
			Instr::abcd(Op::VarArgTab, 0, 0, 0, 0),
			Instr::abcd(Op::Return, 0, 0, 0, 0),
		])
		.is_ok());
		// CallE lane c is unused; CallM lane c (result count) is not.
		assert!(run(vec![
			Instr::abcd(Op::CallE, 0, 0, 9, 0),
			Instr::abcd(Op::Return, 0, 0, 0, 0),
		])
		.is_err());
		// MkStr carries packed bytes/length in b/c/d — no special case
		// is needed because the metadata marks every lane used.
		assert!(run(vec![
			Instr::abcd(Op::MkStr, 0, 500, 600, 700),
			Instr::abcd(Op::Return, 0, 0, 0, 0),
		])
		.is_ok());
	}

	#[test]
	fn valid_records_pass() {
		let recs = vec![
			FRec { kind: FKind::Base(Op::LoadK), a: 1, b: 2, c: 0, d: 0 },
			FRec { kind: FKind::Base(Op::Return), a: 0, b: 0, c: 0, d: 0 },
		];
		verify_final_records(0, &recs, &limits(), &[], &[1]).unwrap();
	}

	#[test]
	fn invalid_register_is_rejected() {
		let recs = vec![FRec { kind: FKind::Base(Op::LoadK), a: 99, b: 0, c: 0, d: 0 }];
		assert!(verify_final_records(0, &recs, &limits(), &[], &[1]).is_err());
	}

	#[test]
	fn invalid_jump_is_rejected() {
		let recs = vec![
			FRec { kind: FKind::Base(Op::Jmp), a: 0, b: 99, c: 0, d: 0 },
			FRec { kind: FKind::Base(Op::Return), a: 0, b: 0, c: 0, d: 0 },
		];
		assert!(verify_final_records(0, &recs, &limits(), &[], &[1, 2]).is_err());
	}

	#[test]
	fn mutation_battery_rejects_each_operand_class() {
		let l = limits();
		let cases = [
			Instr::abcd(Op::LoadK, 0, 99, 0, 0),
			Instr::abcd(Op::GetUp, 0, 99, 0, 0),
			Instr::abcd(Op::Closure, 0, 99, 0, 0),
			Instr::abcd(Op::Call, 15, 4, 0, 0),
			Instr::abcd(Op::Canary, 9, 0, 0, 0),
			Instr::abcd(Op::LoadStr, 0, 99, 0, 0),
		];
		for ins in cases {
			assert!(verify_single(0, &ins, &l).is_err(), "{ins:?}");
		}
	}
}
