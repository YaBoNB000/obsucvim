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

// ---------------------------------------------------------------------
// Stage 1.1: handler-evidence lane access.
//
// `OpMeta` stays the single authoritative definition of operand roles.
// What was missing in Stage 1 was a check that the stated roles match
// what the handlers *actually do* with each lane.  The access facts
// below are NOT a second hand-maintained role table: they are derived
// mechanically, on demand, from the generated handler bodies
// (`handlers::gen`) — the same text the template splices into the
// dispatch loop.  If a handler changes which lane it reads, the derived
// facts follow automatically; if a handler invents an idiom the
// extractor does not recognize, validation fails closed instead of
// silently approving the metadata.
//
// Recognized idioms (all present in the current 44 handlers):
//   register read/write   V[..], V[S[..]], O[..] (v15 spill), S[..]
//   register window       the bracket index adds a loop/offset term
//   constant read         C[..]           (receiver `G[C[..]]` included)
//   prototype read        E.k[..] or the first `makefn(..)` argument
//   upvalue access        ups[..]
//   jump                  `pc = <lane>`
//   immediate (bare)      any other whole-word use of the lane variable
// Lane aliases (`local base = a + 1`, `local kf = b`, `local s = S[..]`)
// are resolved: an alias counts for its lane when it heads a bracket
// index expression.  String literals and `--` / `--[[ ]]` comments are
// stripped in one lexical pass; a plain `=` counts as a write while a
// Luau compound assignment (`+=`, `..=`, ...) counts as a read *and* a
// write of the slot; and lanes that occur only inside a never-read
// dead local (the Nop noise shapes) do not count as accesses.
// ---------------------------------------------------------------------

/// How one operand lane is actually accessed by an op's handler bodies,
/// unioned over every format variant, both legacy and v15 bodies, and
/// (for the mask-carrying op) more than one mask pair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaneAccess {
	/// Handler reads the register file through this lane (`V[..]`).
	pub reg_read: bool,
	/// Handler writes the register file through this lane.
	pub reg_write: bool,
	/// Register access whose index adds a computed offset (the lane is
	/// the base of a register window, not a single scattered slot).
	pub reg_range: bool,
	/// Handler reads the constant pool through this lane (`C[..]`).
	pub const_read: bool,
	/// Handler reads a prototype index through this lane.
	pub proto_read: bool,
	/// Handler indexes the upvalue table through this lane (`ups[..]`).
	pub upval: bool,
	/// Handler assigns this lane to the program counter (`pc = lane`).
	pub jump: bool,
	/// Handler uses the lane as a bare immediate (count, flag, packed
	/// payload, alias binding, mask arithmetic, ...).
	pub bare: bool,
}

impl LaneAccess {
	fn any(self) -> bool {
		self.reg_read
			|| self.reg_write
			|| self.reg_range
			|| self.const_read
			|| self.proto_read
			|| self.upval
			|| self.jump
			|| self.bare
	}

	fn absorb(&mut self, other: LaneAccess) {
		self.reg_read |= other.reg_read;
		self.reg_write |= other.reg_write;
		self.reg_range |= other.reg_range;
		self.const_read |= other.const_read;
		self.proto_read |= other.proto_read;
		self.upval |= other.upval;
		self.jump |= other.jump;
		self.bare |= other.bare;
	}

	fn describe(self) -> String {
		let mut parts: Vec<&str> = Vec::new();
		if self.reg_read {
			parts.push("register read");
		}
		if self.reg_write {
			parts.push("register write");
		}
		if self.reg_range {
			parts.push("register window");
		}
		if self.const_read {
			parts.push("constant-pool read");
		}
		if self.proto_read {
			parts.push("prototype read");
		}
		if self.upval {
			parts.push("upvalue access");
		}
		if self.jump {
			parts.push("pc assignment");
		}
		if self.bare {
			parts.push("bare immediate use");
		}
		if parts.is_empty() {
			"no handler access".to_string()
		} else {
			parts.join(", ")
		}
	}
}

const LANE_NAMES: [&str; 4] = ["a", "b", "c", "d"];

fn lane_index(token: &str) -> Option<usize> {
	LANE_NAMES.iter().position(|&n| n == token)
}

fn is_ident_start(c: char) -> bool {
	c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
	c.is_ascii_alphanumeric() || c == '_'
}

/// Identifier tokens of `chars` as (start, end-exclusive, text).
fn ident_tokens(chars: &[char]) -> Vec<(usize, usize, String)> {
	let mut out = Vec::new();
	let mut i = 0;
	while i < chars.len() {
		if is_ident_start(chars[i]) {
			let start = i;
			while i < chars.len() && is_ident_char(chars[i]) {
				i += 1;
			}
			out.push((start, i, chars[start..i].iter().collect()));
		} else {
			i += 1;
		}
	}
	out
}

fn skip_ws(chars: &[char], mut i: usize) -> usize {
	while i < chars.len() && chars[i].is_whitespace() {
		i += 1;
	}
	i
}

/// Blank out string literals and comments in one lexical pass, so
/// words inside error messages (e.g. the standalone "a" in "on a ...
/// value") or inside `--` / `--[[ ]]` commentary are never mistaken
/// for lane operands.  String literals collapse to a single space;
/// comment text is replaced by spaces but newlines are preserved so
/// statement boundaries stay where the author put them.  Handling the
/// two in a single pass matters: a `"` inside a comment must not open
/// a phantom string, and a `--` inside a string must not open a
/// phantom comment.
fn strip_strings_and_comments(text: &str) -> Vec<char> {
	let src: Vec<char> = text.chars().collect();
	let mut out: Vec<char> = Vec::with_capacity(src.len());
	let mut i = 0;
	while i < src.len() {
		if src[i] == '"' {
			// String literal: collapse to one space.
			out.push(' ');
			i += 1;
			while i < src.len() {
				if src[i] == '\\' {
					i += 2;
				} else if src[i] == '"' {
					i += 1;
					break;
				} else {
					i += 1;
				}
			}
			continue;
		}
		if src[i] == '-' && src.get(i + 1) == Some(&'-') {
			// Comment.  `--[=*[` opens a long comment closed by the
			// matching `]=*]`; anything else runs to end of line.
			let mut j = i + 2;
			let mut level: Option<usize> = None;
			if src.get(j) == Some(&'[') {
				let mut eq = 0usize;
				let mut k = j + 1;
				while src.get(k) == Some(&'=') {
					eq += 1;
					k += 1;
				}
				if src.get(k) == Some(&'[') {
					level = Some(eq);
					j = k + 1;
				}
			}
			match level {
				Some(eq) => {
					let start = i;
					let mut end = src.len();
					let mut k = j;
					while k < src.len() {
						if src[k] == ']' {
							let mut eq2 = 0usize;
							let mut m = k + 1;
							while src.get(m) == Some(&'=') {
								eq2 += 1;
								m += 1;
							}
							if eq2 == eq && src.get(m) == Some(&']') {
								end = m + 1;
								break;
							}
						}
						k += 1;
					}
					out.extend(src[start..end].iter().map(|&c| {
						if c == '\n' {
							'\n'
						} else {
							' '
						}
					}));
					i = end;
				}
				None => {
					while i < src.len() && src[i] != '\n' {
						out.push(' ');
						i += 1;
					}
				}
			}
			continue;
		}
		out.push(src[i]);
		i += 1;
	}
	out
}

/// Which assignment operator, if any, starts at `i`?  A plain `=` (but
/// not the `==` comparison) overwrites its target.  The Luau compound
/// forms `+= -= *= /= %= ^=` and `..=` *read* the target's old value
/// and then write the updated one — semantically a read-modify-write,
/// so the extractor reports both accesses for them.  A `-`/`..` that
/// is merely an operator (`V[a] - 1`, `V[a] .. x`) is no assignment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AssignKind {
	/// No assignment operator here (plain read context).
	None,
	/// `=` — write only.
	Plain,
	/// `+=`, `..=`, ... — read and write.
	Compound,
}

fn assignment_kind_at(chars: &[char], i: usize) -> AssignKind {
	if i >= chars.len() {
		return AssignKind::None;
	}
	match chars[i] {
		'=' => {
			if chars.get(i + 1) == Some(&'=') {
				AssignKind::None
			} else {
				AssignKind::Plain
			}
		}
		'+' | '-' | '*' | '/' | '%' | '^' => {
			if chars.get(i + 1) == Some(&'=') {
				AssignKind::Compound
			} else {
				AssignKind::None
			}
		}
		'.' => {
			if chars.get(i + 1) == Some(&'.') && chars.get(i + 2) == Some(&'=') {
				AssignKind::Compound
			} else {
				AssignKind::None
			}
		}
		_ => AssignKind::None,
	}
}

/// Is the token at `i` any assignment operator (plain or compound)?
/// Used where only the target-vs-read distinction matters, e.g. when
/// classifying occurrences of a local for deadness: an occurrence on
/// the left of `+=` is an assignment target, not a consumption of a
/// previously computed value.
fn is_assignment_at(chars: &[char], i: usize) -> bool {
	assignment_kind_at(chars, i) != AssignKind::None
}

/// Index of the `]` matching the `[` at `open`, honoring nesting.
fn match_bracket(chars: &[char], open: usize) -> Option<usize> {
	let mut depth = 0usize;
	for (i, &c) in chars.iter().enumerate().skip(open) {
		match c {
			'[' => depth += 1,
			']' => {
				depth -= 1;
				if depth == 0 {
					return Some(i);
				}
			}
			_ => {}
		}
	}
	None
}

/// Collect lane aliases: `local <id> = <lane> ...`,
/// `local <id> = <known alias> ...`, and slot aliases
/// `local <id> = S[ <lane expr> ]`.  An alias only matters when it
/// heads a bracket index expression; value aliases derived from a lane
/// (argument counts, cache keys) are tracked identically, but they are
/// only ever consulted in head position, where counts never appear.
fn collect_aliases(chars: &[char]) -> Vec<(String, usize)> {
	let mut aliases: Vec<(String, usize)> = Vec::new();
	let toks = ident_tokens(chars);
	let resolve = |aliases: &Vec<(String, usize)>, tok: &str| -> Option<usize> {
		lane_index(tok).or_else(|| aliases.iter().find(|(n, _)| n == tok).map(|&(_, l)| l))
	};
	let mut ti = 0;
	while ti < toks.len() {
		if toks[ti].2 == "local" && ti + 1 < toks.len() {
			let (id_start, id_end, id) = toks[ti + 1].clone();
			let _ = id_start;
			let eq = skip_ws(chars, id_end);
			if eq < chars.len() && chars[eq] == '=' && chars.get(eq + 1) != Some(&'=') {
				let rhs = skip_ws(chars, eq + 1);
				if rhs < chars.len() && is_ident_start(chars[rhs]) {
					let rhs_end = {
						let mut e = rhs;
						while e < chars.len() && is_ident_char(chars[e]) {
							e += 1;
						}
						e
					};
					let head: String = chars[rhs..rhs_end].iter().collect();
					if head == "S" {
						// Slot alias: `local s = S[ <expr> ]`.
						let open = skip_ws(chars, rhs_end);
						if open < chars.len() && chars[open] == '[' {
							if let Some(close) = match_bracket(chars, open) {
								for (_, _, tok) in ident_tokens(&chars[open + 1..close]) {
									if let Some(lane) = resolve(&aliases, &tok) {
										aliases.push((id.clone(), lane));
										break;
									}
								}
							}
						}
					} else if let Some(lane) = resolve(&aliases, &head) {
						aliases.push((id.clone(), lane));
					}
				}
			}
		}
		ti += 1;
	}
	aliases
}

/// End of the statement starting at or before `from`: the next `;`,
/// newline, or following `local` statement, whichever comes first.
fn statement_end(chars: &[char], from: usize) -> usize {
	let mut i = from;
	while i < chars.len() {
		if chars[i] == ';' || chars[i] == '\n' {
			return i;
		}
		if chars[i] == ' ' || chars[i] == '\t' {
			let j = skip_ws(chars, i);
			if ident_tokens(&chars[j..]).first().map(|t| t.2.as_str()) == Some("local") && j > from {
				return i;
			}
			i = j.max(i + 1);
			continue;
		}
		i += 1;
	}
	chars.len()
}

/// Spans of never-read dead locals — the Nop noise shapes compute
/// into `_n`, which no handler ever reads back, so those lane
/// mentions carry no semantics.  Detection runs on the *unmasked*
/// text: an alias whose only uses sit inside brackets (`local kf = b`
/// feeding `C[kf + 1]`) is very much alive, and blanking the brackets
/// first would hide those uses and misjudge it as dead.  The spans
/// are only applied as masks at the end of `extract_access`, once the
/// bracket/jump accesses have been recorded.
fn dead_local_spans(chars: &[char]) -> Vec<(usize, usize)> {
	let toks = ident_tokens(chars);
	let mut dead_spans: Vec<(usize, usize)> = Vec::new();
	let mut ti = 0;
	while ti < toks.len() {
		if toks[ti].2 == "local" && ti + 1 < toks.len() {
			let (id_start, id_end, id) = &toks[ti + 1];
			let after = skip_ws(chars, *id_end);
			let single_def = after < chars.len()
				&& (chars[after] == '=' || is_ident_start(chars[after]));
			if single_def && id.as_str() != "local" {
				// A local is dead when every other occurrence is an
				// assignment target, never a read.
				let mut reads = 0usize;
				let mut spans = vec![(toks[ti].0, statement_end(chars, toks[ti].0))];
				for (s, e, tok) in &toks {
					if tok != id || *s == *id_start {
						continue;
					}
					let j = skip_ws(chars, *e);
					if is_assignment_at(chars, j) {
						spans.push((*s, statement_end(chars, *s)));
					} else {
						reads += 1;
					}
				}
				if reads == 0 {
					dead_spans.extend(spans);
				}
			}
		}
		ti += 1;
	}
	dead_spans
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BracketKind {
	Reg,
	Const,
	Proto,
	Upval,
}

/// Extract the lane accesses of one generated handler body.
fn extract_access(text: &str) -> [LaneAccess; 4] {
	let mut chars = strip_strings_and_comments(text);
	let aliases = collect_aliases(&chars);
	// Detect dead locals now, while bracket contents are still
	// visible; apply the spans only after bracket/jump extraction.
	let dead_spans = dead_local_spans(&chars);
	let mut acc = [LaneAccess::default(); 4];

	// Bracketed accesses: V[..], C[..], O[..], S[..], ups[..], E.k[..].
	let mut candidates: Vec<(usize, BracketKind, bool, usize)> = Vec::new(); // (start, kind, is_slot_map, open)
	let mut i = 0;
	while i < chars.len() {
		let rest: String = chars[i..].iter().take(4).collect();
		if rest.starts_with("E.k[") {
			candidates.push((i, BracketKind::Proto, false, i + 3));
			i += 4;
			continue;
		}
		if rest.starts_with("ups[")
			&& (i == 0 || !is_ident_char(chars[i - 1]))
		{
			candidates.push((i, BracketKind::Upval, false, i + 3));
			i += 4;
			continue;
		}
		if i + 1 < chars.len()
			&& chars[i + 1] == '['
			&& matches!(chars[i], 'V' | 'C' | 'O' | 'S')
			&& (i == 0 || !is_ident_char(chars[i - 1]))
		{
			let kind = match chars[i] {
				'C' => BracketKind::Const,
				_ => BracketKind::Reg,
			};
			candidates.push((i, kind, chars[i] == 'S', i + 1));
			i += 2;
			continue;
		}
		i += 1;
	}
	let mut masked_until = 0usize;
	for (start, kind, is_slot_map, open) in candidates {
		if start < masked_until {
			continue; // nested inside an already-processed bracket
		}
		let Some(close) = match_bracket(&chars, open) else {
			continue;
		};
		let content = ident_tokens(&chars[open + 1..close]);
		// The lane of an index expression is its first lane/alias
		// token; later identifiers are offset values, not the index.
		let mut lane_of: Option<(usize, String)> = None;
		for (_, _, tok) in &content {
			if let Some(l) = lane_index(tok) {
				lane_of = Some((l, tok.clone()));
				break;
			}
			if let Some(&(_, l)) = aliases.iter().find(|(n, _)| n == tok) {
				lane_of = Some((l, tok.clone()));
				break;
			}
		}
		if let Some((lane, head_tok)) = lane_of {
			let range = content
				.iter()
				.any(|(_, _, t)| t != &head_tok && t != "S");
			match kind {
				BracketKind::Const => acc[lane].const_read = true,
				BracketKind::Proto => acc[lane].proto_read = true,
				BracketKind::Upval => acc[lane].upval = true,
				BracketKind::Reg => {
					let after = skip_ws(&chars, close + 1);
					let assign = if is_slot_map {
						AssignKind::None
					} else {
						assignment_kind_at(&chars, after)
					};
					match assign {
						// Plain `=` overwrites the slot without
						// consuming its old value.
						AssignKind::Plain => acc[lane].reg_write = true,
						// Compound assignment is a read-modify-
						// write: the old slot value feeds the
						// update, so both accesses are real.
						AssignKind::Compound => {
							acc[lane].reg_read = true;
							acc[lane].reg_write = true;
						}
						AssignKind::None => acc[lane].reg_read = true,
					}
					if range {
						acc[lane].reg_range = true;
					}
				}
			}
		}
		for c in &mut chars[start..=close] {
			*c = ' ';
		}
		masked_until = masked_until.max(close + 1);
	}

	// Prototype reads through the legacy `makefn(<lane> + 1, ..)` form.
	let toks = ident_tokens(&chars);
	for (s, e, tok) in &toks {
		if tok == "makefn" {
			let open = skip_ws(&chars, *e);
			if open < chars.len() && chars[open] == '(' {
				let mut j = open + 1;
				while j < chars.len() && chars[j] != ',' {
					j += 1;
				}
				for (_, _, t) in ident_tokens(&chars[open + 1..j]) {
					if let Some(l) = lane_index(&t) {
						acc[l].proto_read = true;
					}
				}
				let end = j.min(chars.len());
				for c in &mut chars[*s..end] {
					*c = ' ';
				}
			}
		}
	}

	// Jump targets: `pc = <lane>`.
	let toks = ident_tokens(&chars);
	for (s, e, tok) in &toks {
		if tok == "pc" {
			let eq = skip_ws(&chars, *e);
			if eq < chars.len() && chars[eq] == '=' && chars.get(eq + 1) != Some(&'=') {
				let v = skip_ws(&chars, eq + 1);
				if v < chars.len() && is_ident_start(chars[v]) {
					let mut ve = v;
					while ve < chars.len() && is_ident_char(chars[ve]) {
						ve += 1;
					}
					let name: String = chars[v..ve].iter().collect();
					if let Some(l) = lane_index(&name) {
						acc[l].jump = true;
						for c in &mut chars[*s..ve] {
							*c = ' ';
						}
					}
				}
			}
		}
	}

	// Dead noise locals (Nop) must not count as lane use.
	for (s, e) in &dead_spans {
		let end = (*e).min(chars.len());
		for c in &mut chars[*s..end] {
			*c = ' ';
		}
	}

	// Whatever lane variables remain are bare immediate uses.
	for (_, _, tok) in ident_tokens(&chars) {
		if let Some(l) = lane_index(&tok) {
			acc[l].bare = true;
		}
	}
	acc
}

/// How `op`'s handlers actually access each operand lane, derived
/// from the generated handler bodies themselves.  This is evidence
/// about the handlers, mechanically extracted — it is not a second
/// role table, and it is only ever compared against `OpMeta`, never
/// consulted by the compiler, scatter pass, or verifier record checks.
///
/// The handler bodies are pure functions of static tables (format
/// index, v15 flag, mask pair), so the derived facts cannot change
/// during a process run; they are computed once and cached.  Before
/// this cache existed, every `ensure_metadata_complete()` call
/// re-generated and re-scanned every handler text (~27 ms), which the
/// test suite and any repeated-validation tooling paid per call.
pub fn handler_lane_access(op: Op) -> [LaneAccess; 4] {
	static CACHE: std::sync::OnceLock<[[LaneAccess; 4]; super::isa::N_OPS]> =
		std::sync::OnceLock::new();
	let table = CACHE.get_or_init(|| {
		let mut table = [[LaneAccess::default(); 4]; super::isa::N_OPS];
		for (idx, &op) in super::isa::ALL_OPS.iter().enumerate() {
			table[idx] = derive_handler_lane_access(op);
		}
		table
	});
	table[super::isa::op_index(op)]
}

/// The uncached derivation behind `handler_lane_access`: generate
/// every handler body variant for `op` and union the extracted lane
/// accesses.
fn derive_handler_lane_access(op: Op) -> [LaneAccess; 4] {
	let name = super::isa::OP_NAMES[super::isa::op_index(op)];
	let mut acc = [LaneAccess::default(); 4];
	for v15 in [false, true] {
		let nf = super::handlers::n_formats(name, v15).max(1);
		for fmt in 0..nf {
			// The mask pair only rewrites numeric literals inside the
			// packed-string body; sample a second pair at fmt 0 so a
			// mask-dependent lane idiom could not hide.
			let masks: &[(u16, u16)] = if fmt == 0 { &[(0, 0), (1, 2)] } else { &[(0, 0)] };
			for &mk in masks {
				let mut pool = super::strpool::StrPool::new(false);
				let text = super::handlers::gen(name, fmt, v15, &mut pool, mk, false);
				let a = extract_access(&text);
				for lane in 0..4 {
					acc[lane].absorb(a[lane]);
				}
			}
		}
	}
	acc
}

/// Check one lane's stated role against the handler-derived access
/// facts.  The rules express what each role *means* operationally:
/// a role is a promise about how the lane is consumed, and the handler
/// bodies are the ground truth for that consumption.
fn check_lane_role(
	op: Op,
	lane: usize,
	role: OperandRole,
	acc: &LaneAccess,
) -> VerifyResult<()> {
	let bad = |msg: String| {
		Err(VerificationError::global(format!(
			"incomplete ISA metadata for {op:?}: lane {} (index {lane}): {msg}",
			LANE_NAMES[lane]
		)))
	};
	let reg = acc.reg_read || acc.reg_write || acc.reg_range;
	match role {
		OperandRole::Unused => {
			if acc.any() {
				return bad(format!(
					"role is Unused but the handler body accesses this lane ({})",
					acc.describe()
				));
			}
		}
		OperandRole::RegRead => {
			if !acc.reg_read {
				return bad(format!(
					"role is RegRead but the handler never reads a register through this lane ({})",
					acc.describe()
				));
			}
			// A register *window* read through this lane would make
			// it a base (RegBase/RegRange), not a direct slot read.
			// In-place counter updates (TabN/VarArgTabN/CallT) are
			// the one sanctioned write beside the read.
			if acc.reg_range || acc.const_read || acc.proto_read || acc.upval || acc.jump || acc.bare {
				return bad(format!(
					"role is RegRead but the handler also uses this lane as a non-register operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::RegWrite => {
			if !acc.reg_write {
				return bad(format!(
					"role is RegWrite but the handler never writes a register through this lane ({})",
					acc.describe()
				));
			}
			// RegWrite promises a pure output slot: the handler
			// produces the register, it does not consume it.  A lane
			// the handler also reads is an input or counter (RegRead
			// territory), and a lane written as a whole window is a
			// range/base, not a direct slot.  Bare use stays legal:
			// MkStr derives its byte mask from the target index
			// itself, which is part of writing that slot.
			if acc.reg_read || acc.reg_range || acc.const_read || acc.proto_read || acc.upval || acc.jump {
				return bad(format!(
					"role is RegWrite but the handler also uses this lane as a non-register operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::RegRange => {
			if !(acc.reg_range && acc.reg_write) {
				return bad(format!(
					"role is RegRange but the handler does not write a register window through this lane ({})",
					acc.describe()
				));
			}
			if acc.const_read || acc.proto_read || acc.upval || acc.jump || acc.bare {
				return bad(format!(
					"role is RegRange but the handler also uses this lane as a non-register operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::RegBase => {
			if !acc.reg_range {
				return bad(format!(
					"role is RegBase but the handler never addresses a register window through this lane ({})",
					acc.describe()
				));
			}
			if acc.const_read || acc.proto_read || acc.upval || acc.jump || acc.bare {
				return bad(format!(
					"role is RegBase but the handler also uses this lane as a non-register operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::CallBase => {
			// A call base is defined by two accesses every call
			// handler performs: it reads the function sitting at the
			// base slot, and it addresses the argument/result window
			// relative to the base.  Either one missing means the
			// lane is not a call base at all.
			if !(acc.reg_read && acc.reg_range) {
				return bad(format!(
					"role is CallBase but the handler does not read a call window base through this lane ({})",
					acc.describe()
				));
			}
			if acc.const_read || acc.proto_read || acc.upval || acc.jump {
				return bad(format!(
					"role is CallBase but the handler also uses this lane as a non-register operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::ConstIdx => {
			if !acc.const_read {
				return bad(format!(
					"role is ConstIdx but the handler never reads the constant pool through this lane ({})",
					acc.describe()
				));
			}
			if reg || acc.proto_read || acc.upval || acc.jump {
				return bad(format!(
					"role is ConstIdx but the handler also uses this lane as a non-constant operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::ProtoIdx => {
			if !acc.proto_read {
				return bad(format!(
					"role is ProtoIdx but the handler never reads a prototype through this lane ({})",
					acc.describe()
				));
			}
			if reg || acc.const_read || acc.upval || acc.jump || acc.bare {
				return bad(format!(
					"role is ProtoIdx but the handler also uses this lane as a non-prototype operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::UpvalIdx => {
			if !acc.upval {
				return bad(format!(
					"role is UpvalIdx but the handler never indexes the upvalue table through this lane ({})",
					acc.describe()
				));
			}
			if reg || acc.const_read || acc.proto_read || acc.jump || acc.bare {
				return bad(format!(
					"role is UpvalIdx but the handler also uses this lane as a non-upvalue operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::JumpTarget => {
			if !acc.jump {
				return bad(format!(
					"role is JumpTarget but the handler never assigns this lane to pc ({})",
					acc.describe()
				));
			}
			if reg || acc.const_read || acc.proto_read || acc.upval || acc.bare {
				return bad(format!(
					"role is JumpTarget but the handler also uses this lane as a non-target operand ({})",
					acc.describe()
				));
			}
		}
		OperandRole::ImmVal => {
			if !acc.bare {
				return bad(format!(
					"role is ImmVal but the handler never uses this lane as a bare immediate ({})",
					acc.describe()
				));
			}
			if reg || acc.const_read || acc.proto_read || acc.upval || acc.jump {
				return bad(format!(
					"role is ImmVal but the handler uses this lane as an indexed operand ({})",
					acc.describe()
				));
			}
		}
	}
	Ok(())
}

/// Structural and semantic validation of one op's canonical metadata.
/// This is the single validation path behind
/// `ensure_metadata_complete`, which the compiler runs on the real
/// compilation path (see `vmgen::compile`); it is also directly
/// unit-testable with deliberately corrupted metadata.  Every rule is
/// an invariant the emitter, handlers, scatter pass, and this verifier
/// already rely on — a violation means the metadata table itself is
/// lying about the instruction encoding, so compilation must fail
/// closed instead of silently continuing.
///
/// Beyond the Stage 1 structural rules, Stage 1.1 adds two semantic
/// layers, neither of which is a second role table:
///   * positional role laws that follow from the ISA's own structure
///     (a jump's target lives in the lane its handler assigns to `pc`;
///     every constant-pool read goes through lane b; ...), and
///   * a comparison of every lane's role against the access facts
///     mechanically extracted from the generated handler bodies
///     (`handler_lane_access`), so a role that disagrees with the
///     handler's real register/constant/jump consumption is rejected
///     with the opcode name, the lane, and the reason.
pub fn validate_op_meta(op: Op, meta: &super::isa::OpMeta) -> VerifyResult<()> {
	let bad = |msg: &str| {
		Err(VerificationError::global(format!(
			"incomplete ISA metadata for {op:?}: {msg}"
		)))
	};
	let bad_lane = |lane: usize, msg: &str| {
		Err(VerificationError::global(format!(
			"incomplete ISA metadata for {op:?}: lane {} (index {lane}): {msg}",
			LANE_NAMES[lane]
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
					return bad_lane(
						lane,
						"CallBase is only legal as lane a of a frame-transfer call op",
					);
				}
			}
			OperandRole::RegBase => {
				if lane != 0 {
					return bad_lane(
						lane,
						"RegBase is only legal as lane a (base of a base+count range)",
					);
				}
			}
			OperandRole::ProtoIdx => {
				if op != Op::Closure || lane != 1 {
					return bad_lane(lane, "ProtoIdx is only legal as Closure lane b");
				}
			}
			_ => {}
		}
	}
	if meta.pc_effect == PcEffect::FrameTransfer && roles[0] != OperandRole::CallBase {
		return bad("a frame-transfer op must take its call base in lane a");
	}

	// --- Stage 1.1 positional role laws --------------------------------
	// A jump target is not merely "present somewhere": it must sit in
	// the lane the handler assigns to `pc`, which for every jump in
	// this ISA is lane b (`pc = b`).  Moving the JumpTarget role to
	// any other lane is therefore rejected here even before the
	// handler-evidence comparison below runs.
	match meta.pc_effect {
		PcEffect::UnconditionalJmp => {
			let want = [
				OperandRole::Unused,
				OperandRole::JumpTarget,
				OperandRole::Unused,
				OperandRole::Unused,
			];
			for lane in 0..4 {
				if roles[lane] != want[lane] {
					return bad_lane(
						lane,
						"an unconditional jump is exactly [Unused, JumpTarget, Unused, Unused]: \
						 the handler reads only lane b as `pc = b`",
					);
				}
			}
		}
		PcEffect::ConditionalJmp => {
			if roles[1] != OperandRole::JumpTarget {
				return bad_lane(
					1,
					"a conditional jump must carry its JumpTarget in lane b (handler: `pc = b`)",
				);
			}
			if !matches!(roles[0], OperandRole::RegRead | OperandRole::RegRange) {
				return bad_lane(
					0,
					"a conditional jump must read its condition register in lane a",
				);
			}
			for lane in 2..4 {
				if roles[lane] != OperandRole::Unused {
					return bad_lane(
						lane,
						"a conditional jump uses only lane a (condition) and lane b (target)",
					);
				}
			}
		}
		_ => {}
	}
	// Every constant-pool read in the handlers goes through lane b
	// (`C[b + 1]`, in LoadStr through its `kf = b` alias), so a
	// ConstIdx role anywhere else describes an index the handler
	// never consumes as a constant.
	for lane in 0..4 {
		if roles[lane] == OperandRole::ConstIdx && lane != 1 {
			return bad_lane(
				lane,
				"ConstIdx is only legal in lane b: every handler constant-pool read is `C[b + 1]`",
			);
		}
	}
	if roles.iter().filter(|&&r| r == OperandRole::UpvalIdx).count() > 1 {
		return bad("an op may index the upvalue table through at most one lane");
	}
	if meta.pc_effect == PcEffect::FrameReturn && roles[0] != OperandRole::RegBase {
		return bad_lane(
			0,
			"a frame-return op must take its return-window base in lane a (RegBase)",
		);
	}

	// --- Stage 1.1 handler-evidence comparison -------------------------
	// The roles are now compared lane by lane against what the
	// generated handler bodies actually do.  This catches mislabels
	// that are structurally well-formed but semantically wrong, e.g.
	// LoadK's constant index restated as an immediate: the handler
	// reads `C[b + 1]`, so an ImmVal promise for lane b is false.
	let access = handler_lane_access(op);
	for lane in 0..4 {
		check_lane_role(op, lane, roles[lane], &access[lane])?;
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

	// ---- Stage 1.1: semantic validation of OpMeta ---------------------

	/// Assert that `validate_op_meta` rejects `meta` for `op`, and that
	/// the error names the opcode and the offending lane.  Returns the
	/// error text so a case can pin the stated reason as well.
	fn expect_meta_reject(op: Op, meta: &super::super::isa::OpMeta, lane: &str) -> String {
		match validate_op_meta(op, meta) {
			Ok(()) => panic!("{op:?} corrupted metadata was accepted"),
			Err(e) => {
				let s = e.to_string();
				assert!(
					s.contains(op.meta().name),
					"error must name the opcode: {s}"
				);
				assert!(
					s.contains(&format!("lane {lane}")),
					"error must name lane {lane}: {s}"
				);
				s
			}
		}
	}

	#[test]
	fn stage11_semantic_negative_battery() {
		// The real table must pass the strengthened validation for
		// every opcode (this is also what `vmgen::compile` runs once
		// at entry via `ensure_metadata_complete`).
		for &op in super::super::isa::ALL_OPS.iter() {
			assert!(validate_op_meta(op, &op.meta()).is_ok(), "{op:?}");
		}
		assert!(ensure_metadata_complete().is_ok());

		// (1) LoadK: the handler reads `C[b + 1]`, so lane b is a
		// constant index.  Restating it as an immediate must be
		// rejected — structurally the mutated table is well-formed,
		// only the handler evidence convicts it.
		let mut m = Op::LoadK.meta();
		m.role_b = OperandRole::ImmVal;
		let s = expect_meta_reject(Op::LoadK, &m, "b");
		assert!(s.contains("constant"), "{s}");

		// (2) Jmp: move the JumpTarget from lane b to lane a.
		let mut m = Op::Jmp.meta();
		m.role_a = OperandRole::JumpTarget;
		m.role_b = OperandRole::Unused;
		expect_meta_reject(Op::Jmp, &m, "a");

		// (3) Jf: move the JumpTarget from lane b to lane c.
		let mut m = Op::Jf.meta();
		m.role_b = OperandRole::Unused;
		m.role_c = OperandRole::JumpTarget;
		let s = expect_meta_reject(Op::Jf, &m, "b");
		assert!(s.contains("JumpTarget"), "{s}");

		// (4) Jt: move the JumpTarget from lane b to lane d.
		let mut m = Op::Jt.meta();
		m.role_b = OperandRole::Unused;
		m.role_d = OperandRole::JumpTarget;
		expect_meta_reject(Op::Jt, &m, "b");

		// (5) A real RegRead lane restated as Unused: Move's handler
		// reads `V[b + 1]`, so Unused is a lie the noise fill and the
		// pre-scatter zero check would have acted on.
		let mut m = Op::Move.meta();
		m.role_b = OperandRole::Unused;
		let s = expect_meta_reject(Op::Move, &m, "b");
		assert!(s.contains("Unused"), "{s}");

		// (6) A real RegWrite lane restated as an immediate: LoadK's
		// handler writes `V[a + 1] = ...`.
		let mut m = Op::LoadK.meta();
		m.role_a = OperandRole::ImmVal;
		expect_meta_reject(Op::LoadK, &m, "a");

		// (7) An Unused lane promoted to a register role.  Scatter
		// would happily remap the lane (that is the hazard), the
		// derived unused-lane set would shrink — and the verifier-side
		// validation must refuse before either consumer sees the
		// corrupted table.
		let mut m = Op::Jmp.meta();
		m.role_c = OperandRole::RegRead;
		assert_eq!(m.roles()[2].scatter(), ScatterMap::RegisterZeroBased);
		assert!(!m.unused_lanes()[2]);
		expect_meta_reject(Op::Jmp, &m, "c");
		// The uncorrupted metadata the compiler actually uses still
		// derives lane c as unused, so verifier and scatter agree.
		assert!(Op::Jmp.meta().unused_lanes()[2]);
		assert_eq!(
			Op::Jmp.meta().roles()[2].scatter(),
			ScatterMap::Identity
		);

		// Bonus class swaps the battery above does not enumerate:
		// prototype index restated as a constant index (Closure lane
		// b), upvalue index restated as a register (GetUp lane b),
		// and a register-window base restated as a plain write
		// (Return lane a).
		let mut m = Op::Closure.meta();
		m.role_b = OperandRole::ConstIdx;
		expect_meta_reject(Op::Closure, &m, "b");
		let mut m = Op::GetUp.meta();
		m.role_b = OperandRole::RegRead;
		expect_meta_reject(Op::GetUp, &m, "b");
		let mut m = Op::Return.meta();
		m.role_a = OperandRole::RegWrite;
		expect_meta_reject(Op::Return, &m, "a");
	}

	#[test]
	fn stage11_handler_evidence_covers_every_op_and_lane() {
		// Test A (semantic form): for all 44 ops x 4 lanes, the role
		// in the single authoritative table must line up with the
		// handler-derived access facts — a used lane is really
		// accessed by the handler, an Unused lane is never accessed.
		// This enumerates the actual opcode set (`ALL_OPS`), so a
		// future opcode without truthful metadata fails here instead
		// of being defaulted into acceptance.
		assert_eq!(super::super::isa::ALL_OPS.len(), super::super::isa::N_OPS);
		let mut covered = 0usize;
		for &op in super::super::isa::ALL_OPS.iter() {
			let meta = op.meta();
			let access = handler_lane_access(op);
			for lane in 0..4 {
				let role = meta.roles()[lane];
				if role == OperandRole::Unused {
					assert!(
						!access[lane].any(),
						"{op:?} lane {lane}: Unused role but handler accesses the lane ({})",
						access[lane].describe()
					);
				} else {
					assert!(
						access[lane].any(),
						"{op:?} lane {lane}: role {role:?} but the handler never accesses the lane"
					);
				}
				covered += 1;
			}
		}
		assert_eq!(covered, 44 * 4);
	}

	#[test]
	fn stage11_extract_access_ignores_comments() {
		// A line comment full of fake accesses contributes nothing;
		// only the real statement after it counts.
		let acc = extract_access("-- V[b + 1] = C[c + 1] and pc = d\nV[a + 1] = C[b + 1]");
		assert!(acc[0].reg_write);
		assert!(acc[1].const_read);
		assert!(!acc[2].any(), "lane c only appeared in a comment");
		assert!(!acc[3].any(), "lane d only appeared in a comment");
		// Trailing comment on a code line.
		let acc = extract_access("V[a + 1] = 1 -- pc = b");
		assert!(acc[0].reg_write);
		assert!(!acc[1].any());
		// Long comments, plain and leveled, spanning lines.
		let acc = extract_access("--[[\npc = b\nV[c + 1] = 1\n]]\nV[a + 1] = 1");
		assert!(acc[0].reg_write);
		assert!(!acc[1].any());
		assert!(!acc[2].any());
		let acc = extract_access("--[==[ V[b + 1] ]==] V[a + 1] = 1");
		assert!(acc[0].reg_write);
		assert!(!acc[1].any());
		// A comment wedged between an index and its assignment does
		// not break write detection.
		let acc = extract_access("V[a + 1] --[[x]] = C[b + 1]");
		assert!(acc[0].reg_write && acc[1].const_read);
		// A quote character inside a comment must not swallow the
		// code that follows the comment.
		let acc = extract_access("-- an \"unterminated quote\nV[a + 1] = C[b + 1]");
		assert!(acc[0].reg_write && acc[1].const_read);
	}

	#[test]
	fn stage11_extract_access_distinguishes_assignment_kinds() {
		// (a) Plain assignment overwrites the slot: write, no read.
		let acc = extract_access("V[a + 1] = 1");
		assert!(
			acc[0].reg_write && !acc[0].reg_read,
			"plain assignment must be write-only: {acc:?}"
		);
		// Cross-lane: the source of a plain assignment is a pure
		// read, the destination stays write-only.
		let acc = extract_access("V[a + 1] = V[b + 1]");
		assert!(acc[0].reg_write && !acc[0].reg_read, "{acc:?}");
		assert!(acc[1].reg_read && !acc[1].reg_write, "{acc:?}");

		// (b) Luau compound assignment is a read-modify-write: the
		// old slot value feeds the update, so both accesses count.
		for text in [
			"V[a + 1] += 1",
			"V[a + 1] -= 1",
			"V[a + 1] *= 2",
			"V[a + 1] /= 2",
			"V[a + 1] %= 2",
			"V[a + 1] ^= 2",
			"V[a + 1] ..= y",
		] {
			let acc = extract_access(text);
			assert!(
				acc[0].reg_write && acc[0].reg_read,
				"{text}: compound assignment must count as read AND write"
			);
		}

		// (c) Look-alikes that are plain reads must stay reads.
		for text in [
			"if V[a + 1] == 2 then pc = b end",
			"if V[a + 1] >= 2 then pc = b end",
			"local q = V[a + 1] - 1 if q then pc = b end",
			"local q = V[a + 1] .. y if q then pc = b end",
		] {
			let acc = extract_access(text);
			assert!(
				acc[0].reg_read && !acc[0].reg_write,
				"{text}: plain read misclassified as a write"
			);
			assert!(acc[1].jump, "{text}: jump lost");
		}
	}

	#[test]
	fn stage11_extract_access_complex_aliases() {
		// Chained aliases: an index alias pointing at another alias,
		// and a slot alias used through a second name.
		let acc = extract_access(
			"local base = a + 1 local slot = S[base] local dst = slot V[dst] = V[base]",
		);
		assert!(acc[0].reg_write, "write through the slot alias chain");
		assert!(acc[0].reg_read, "read through the index alias");
		// Chained constant alias (the LoadStr `kf = b` shape, one
		// link longer): the constant index still belongs to lane b.
		let acc = extract_access(
			"local kf = b local k2 = kf local v = C[k2 + 1] V[a + 1] = v",
		);
		assert!(acc[1].const_read && !acc[1].reg_read && !acc[1].reg_write);
		assert!(acc[0].reg_write);
		// A value alias derived from a lane is an *offset*, never the
		// index head: lane b is consumed as a bare count, and only
		// lane a is register-addressed (as a window).
		let acc = extract_access("local n = b local t = V[a + n + 1]");
		assert!(acc[0].reg_read && acc[0].reg_range);
		assert!(
			!acc[1].reg_read && !acc[1].reg_range && acc[1].bare,
			"offset alias leaked a register access onto lane b"
		);
		// Alias-to-alias constant-one offset (`local y = x + 1` with
		// `x` itself a lane alias) still resolves to the lane.
		let acc = extract_access("local x = a local y = x + 1 V[y] = 1");
		assert!(acc[0].reg_write);
	}

	#[test]
	fn stage11_extract_access_multiline_expressions() {
		// Assignment, index arithmetic, and the `pc` assignment all
		// split across lines.
		let acc = extract_access("V[a + 1] =\n\tC[b +\n\t1]\npc =\nc");
		assert!(acc[0].reg_write);
		assert!(acc[1].const_read);
		assert!(acc[2].jump);
		// Alias definition and window index split across lines.
		let acc = extract_access("local base =\n\ta + 1\nV[base +\n\ti] = 1");
		assert!(acc[0].reg_write && acc[0].reg_range);
		// The LoadStr alias shape with newlines between statements.
		let acc = extract_access("local kf =\n\tb\nlocal q = C[kf + 1]\nV[a + 1] = q");
		assert!(acc[1].const_read);
		assert!(acc[0].reg_write);
	}

	#[test]
	fn stage11_role_rules_reject_unacceptable_mixed_access() {
		// Synthetic access facts against `check_lane_role` directly:
		// these pin which mixtures each role tolerates, based on the
		// audit of the real 44x4 access table (see HANDOFF 20.10).
		let acc = |f: fn(&mut LaneAccess)| {
			let mut a = LaneAccess::default();
			f(&mut a);
			a
		};
		// RegWrite is a pure output slot (MkStr's mask-on-index bare
		// use is the one sanctioned extra).
		let pure_write = acc(|a| a.reg_write = true);
		assert!(check_lane_role(Op::LoadK, 0, OperandRole::RegWrite, &pure_write).is_ok());
		assert!(check_lane_role(Op::MkStr, 0, OperandRole::RegWrite, &acc(|a| {
			a.reg_write = true;
			a.bare = true;
		}))
		.is_ok());
		assert!(check_lane_role(Op::LoadK, 0, OperandRole::RegWrite, &acc(|a| {
			a.reg_write = true;
			a.reg_read = true;
		}))
		.is_err(), "RegWrite must not also consume its slot");
		// End to end: a compound assignment in a real handler-body
		// shape produces exactly that read+write mixture — a pure
		// output (RegWrite) lane must refuse it, while a counter
		// (RegRead) lane is precisely the role that accepts it.
		let extracted = extract_access("V[a + 1] += 1");
		assert!(
			check_lane_role(Op::LoadK, 0, OperandRole::RegWrite, &extracted[0]).is_err(),
			"RegWrite must not accept the mixed access of a compound assignment"
		);
		assert!(
			check_lane_role(Op::TabN, 1, OperandRole::RegRead, &extracted[0]).is_ok(),
			"RegRead is the role that sanctions read-modify-write"
		);
		assert!(check_lane_role(Op::LoadK, 0, OperandRole::RegWrite, &acc(|a| {
			a.reg_write = true;
			a.reg_range = true;
		}))
		.is_err(), "RegWrite must not write a whole window");
		// RegRead tolerates the in-place counter update but not a
		// window base (that is RegBase) and not a pure write.
		assert!(check_lane_role(Op::TabN, 1, OperandRole::RegRead, &acc(|a| {
			a.reg_read = true;
			a.reg_write = true;
		}))
		.is_ok(), "counter read+update is the sanctioned RegRead mixture");
		assert!(check_lane_role(Op::TabN, 1, OperandRole::RegRead, &acc(|a| {
			a.reg_read = true;
			a.reg_range = true;
		}))
		.is_err(), "RegRead must not be a window base");
		assert!(check_lane_role(Op::TabN, 1, OperandRole::RegRead, &acc(|a| {
			a.reg_write = true;
		}))
		.is_err(), "RegRead without a read is not a read");
		// CallBase requires both halves of being a call base: read
		// the function at the base slot, address the window.
		assert!(check_lane_role(Op::Call, 0, OperandRole::CallBase, &acc(|a| {
			a.reg_read = true;
			a.reg_range = true;
		}))
		.is_ok());
		assert!(check_lane_role(Op::Call, 0, OperandRole::CallBase, &acc(|a| {
			a.reg_read = true;
		}))
		.is_err(), "CallBase without the window is a plain register");
		assert!(check_lane_role(Op::Call, 0, OperandRole::CallBase, &acc(|a| {
			a.reg_write = true;
			a.reg_range = true;
		}))
		.is_err(), "CallBase that never reads the function slot is not a call base");
	}

	/// The scatter family a lane *should* get, derived from the
	/// handler evidence alone (never from `OpMeta`): constant readers
	/// take the constant permutation, upvalue indexers the upvalue
	/// permutation, register-window bases stay unmapped (the frame
	/// layout, not sigma, addresses them), direct register slots take
	/// the register permutation, and everything else is identity.
	fn expected_scatter(acc: &LaneAccess) -> ScatterMap {
		if acc.const_read {
			ScatterMap::ConstantPermutation
		} else if acc.upval {
			ScatterMap::UpvaluePermutation
		} else if acc.reg_range {
			ScatterMap::Identity
		} else if acc.reg_read || acc.reg_write {
			ScatterMap::RegisterZeroBased
		} else {
			ScatterMap::Identity
		}
	}

	#[test]
	fn stage11_scatter_unscatter_roundtrip_all_ops() {
		// Test C: every op x lane.  The expected ScatterMap comes
		// from the handler evidence (independent of the table under
		// test); the transform itself replicates the compiler's
		// scatter match arms, and the inverse models the runtime
		// handler read (`V[s + 1]`, `C[k + 1]`, `ups[t + 1]`), i.e.
		// the unscatter the generated code performs implicitly.
		let sigma: Vec<u16> = vec![5, 2, 8, 1, 7, 3, 6, 4]; // logical -> physical slot (1-based)
		let kappa: Vec<u16> = vec![4, 0, 5, 2, 1, 3]; // logical -> shuffled pool slot
		let tau: Vec<u16> = vec![2, 3, 0, 1]; // logical -> permuted upvalue slot
		// Runtime-side models, indexed exactly as the handlers index.
		let mut frame = vec![u16::MAX; 16];
		for (logical, &phys) in sigma.iter().enumerate() {
			frame[phys as usize] = logical as u16;
		}
		let mut pool = vec![u16::MAX; kappa.len()];
		for (logical, &slot) in kappa.iter().enumerate() {
			pool[slot as usize] = logical as u16;
		}
		let mut ups = vec![u16::MAX; tau.len()];
		for (logical, &slot) in tau.iter().enumerate() {
			ups[slot as usize] = logical as u16;
		}
		let apply = |map: ScatterMap, v: u16| -> u16 {
			match map {
				ScatterMap::RegisterZeroBased => sigma[v as usize] - 1,
				ScatterMap::ConstantPermutation => kappa[v as usize],
				ScatterMap::UpvaluePermutation => tau[v as usize],
				ScatterMap::Identity => v,
			}
		};
		let invert = |map: ScatterMap, s: u16| -> u16 {
			match map {
				// Handler reads V[s + 1] from the scattered frame.
				ScatterMap::RegisterZeroBased => frame[(s + 1) as usize],
				// Handler reads C[s + 1] from the shuffled pool.
				ScatterMap::ConstantPermutation => pool[s as usize],
				// Handler reads ups[s + 1] from the permuted cells.
				ScatterMap::UpvaluePermutation => ups[s as usize],
				ScatterMap::Identity => s,
			}
		};

		let mut reg_const_pairs = 0usize;
		for &op in super::super::isa::ALL_OPS.iter() {
			let meta = op.meta();
			let access = handler_lane_access(op);
			let mut maps = [ScatterMap::Identity; 4];
			for lane in 0..4 {
				let want = expected_scatter(&access[lane]);
				let got = meta.roles()[lane].scatter();
				assert_eq!(
					got, want,
					"{op:?} lane {lane}: metadata scatter {got:?} disagrees with handler evidence {want:?}"
				);
				maps[lane] = got;
				// A valid sentinel per family domain, chosen from the
				// evidence family rather than the table being tested.
				let v: u16 = match want {
					ScatterMap::RegisterZeroBased => 3,
					ScatterMap::ConstantPermutation => 2,
					ScatterMap::UpvaluePermutation => 1,
					ScatterMap::Identity => 7,
				};
				let scattered = apply(got, v);
				assert_eq!(
					invert(got, scattered),
					v,
					"{op:?} lane {lane}: scatter/unscatter roundtrip lost the operand"
				);
				if want == ScatterMap::Identity {
					assert_eq!(scattered, v, "{op:?} lane {lane}: identity lane was remapped");
				}
			}
			// No role conflict inside one instruction: a raw value
			// that is valid in both the register and constant domains
			// must be remapped independently per lane and invert back
			// per lane (LoadK is the canonical case).
			let reg_lane = (0..4).find(|&l| maps[l] == ScatterMap::RegisterZeroBased);
			let const_lane = (0..4).find(|&l| maps[l] == ScatterMap::ConstantPermutation);
			if let (Some(r), Some(c)) = (reg_lane, const_lane) {
				let raw = 2u16;
				let sr = apply(maps[r], raw);
				let sc = apply(maps[c], raw);
				assert_ne!(sr, sc, "{op:?}: register/constant remaps collided");
				assert_eq!(invert(maps[r], sr), raw);
				assert_eq!(invert(maps[c], sc), raw);
				reg_const_pairs += 1;
			}
		}
		assert!(reg_const_pairs > 0, "no op exercised register+constant lanes");
		// OperandRole::RegRange is part of the role vocabulary even
		// though no current op uses it; its scatter family must stay
		// the register permutation and roundtrip like one.
		assert_eq!(
			OperandRole::RegRange.scatter(),
			ScatterMap::RegisterZeroBased
		);
		assert_eq!(invert(ScatterMap::RegisterZeroBased, apply(ScatterMap::RegisterZeroBased, 3)), 3);
	}
}
