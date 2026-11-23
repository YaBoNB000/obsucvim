//! Deterministic VM Primitive & Capability Manifest (P1).
//!
//! Replaces static, unconditional host-API lists (`PRIM_DOTS`, `PRIM_BARE`, and
//! fixed `PRIM_SRC` slots) with an explicit two-tier capability manifest:
//!   1. `CapabilityTier::InterpreterBase`: primitives strictly required by the
//!      VM boot loader, constant table proxy, and frame trampoline.
//!   2. `CapabilityTier::ProgramDriven`: optional primitives and runtime helpers
//!      derived deterministically from the compiled `VmProgram` (`used_ops`,
//!      `has_float_const`, `fv_sets`) and explicit build flags.
//!
//! Dynamic global accesses (`_G`, `getfenv`, metatables, mutable library tables,
//! and shadowed locals) remain runtime-resolved through `G` (`Op::GetGlobal` /
//! `Op::SetGlobal` / `Op::GetTab`) and are never frozen at compile time.

use super::compiler::VmProgram;
use super::isa::{op_index, Op, ALL_OPS, N_OPS};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityTier {
	InterpreterBase,
	ProgramDriven,
}

/// Primitives placed in the outer `v15` module table and consumed by the
/// chunk stream-cipher unmasker and 32-bit multiplier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScaffoldPrim {
	BitBand,
	BitBxor,
	BitExtract,
	BitLshift,
	BitRshift,
	TableConcat,
}

impl ScaffoldPrim {
	pub const ALL: [ScaffoldPrim; 6] = [
		ScaffoldPrim::BitBand,
		ScaffoldPrim::BitBxor,
		ScaffoldPrim::BitExtract,
		ScaffoldPrim::BitLshift,
		ScaffoldPrim::BitRshift,
		ScaffoldPrim::TableConcat,
	];

	pub const fn lua_expr(self) -> &'static str {
		match self {
			ScaffoldPrim::BitBand => "bit32.band",
			ScaffoldPrim::BitBxor => "bit32.bxor",
			ScaffoldPrim::BitExtract => "bit32.extract",
			ScaffoldPrim::BitLshift => "bit32.lshift",
			ScaffoldPrim::BitRshift => "bit32.rshift",
			ScaffoldPrim::TableConcat => "table.concat",
		}
	}
}

/// Primitives bound inside the VM interpreter template (`P[slot]`).
/// Discriminant order defines the canonical seed-independent manifest order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum VmPrim {
	// --- Interpreter base primitives ---
	StrByte = 0,
	StrSub = 1,
	StrChar = 2,
	EnvGet = 3,
	Unpack = 4,
	MathFloor = 5,
	Type = 6,
	RawGet = 7,
	RawSet = 8,
	SetMetatable = 9,
	Pcall = 10,
	ToString = 11,
	TableConcat = 12,
	// --- Program / opcode / flag-driven optional primitives ---
	Error = 13,
	GetMetatable = 14,
	Select = 15,
	ToNumber = 16,
	TablePack = 17,
	Next = 18,
}

#[allow(dead_code)]
impl VmPrim {
	pub const ALL: [VmPrim; 19] = [
		VmPrim::StrByte,
		VmPrim::StrSub,
		VmPrim::StrChar,
		VmPrim::EnvGet,
		VmPrim::Unpack,
		VmPrim::MathFloor,
		VmPrim::Type,
		VmPrim::RawGet,
		VmPrim::RawSet,
		VmPrim::SetMetatable,
		VmPrim::Pcall,
		VmPrim::ToString,
		VmPrim::TableConcat,
		VmPrim::Error,
		VmPrim::GetMetatable,
		VmPrim::Select,
		VmPrim::ToNumber,
		VmPrim::TablePack,
		VmPrim::Next,
	];

	pub const BASE: [VmPrim; 11] = [
		VmPrim::StrByte,
		VmPrim::StrSub,
		VmPrim::StrChar,
		VmPrim::EnvGet,
		VmPrim::Unpack,
		VmPrim::MathFloor,
		VmPrim::Type,
		VmPrim::SetMetatable,
		VmPrim::Pcall,
		VmPrim::ToString,
		VmPrim::TableConcat,
	];

	pub const fn tier(self) -> CapabilityTier {
		match self {
			VmPrim::StrByte
			| VmPrim::StrSub
			| VmPrim::StrChar
			| VmPrim::EnvGet
			| VmPrim::Unpack
			| VmPrim::MathFloor
			| VmPrim::Type
			| VmPrim::SetMetatable
			| VmPrim::Pcall
			| VmPrim::ToString
			| VmPrim::TableConcat => CapabilityTier::InterpreterBase,
			VmPrim::RawGet
			| VmPrim::RawSet
			| VmPrim::Error
			| VmPrim::GetMetatable
			| VmPrim::Select
			| VmPrim::ToNumber
			| VmPrim::TablePack
			| VmPrim::Next => CapabilityTier::ProgramDriven,
		}
	}

	/// Internal template local binding identifier (before `rename_vm_locals`).
	pub const fn template_ident(self) -> &'static str {
		match self {
			VmPrim::StrByte => "BYTE",
			VmPrim::StrSub => "SUB",
			VmPrim::StrChar => "CHAR",
			VmPrim::EnvGet => "GFE",
			VmPrim::Unpack => "UNP",
			VmPrim::MathFloor => "FLR",
			VmPrim::Type => "TYP",
			VmPrim::RawGet => "RGET",
			VmPrim::RawSet => "RSET",
			VmPrim::SetMetatable => "SMT",
			VmPrim::Pcall => "PCAL",
			VmPrim::ToString => "TSTR",
			VmPrim::TableConcat => "TCAT",
			VmPrim::Error => "ERR",
			VmPrim::GetMetatable => "GMT",
			VmPrim::Select => "SEL",
			VmPrim::ToNumber => "TONUM",
			VmPrim::TablePack => "TPACK",
			VmPrim::Next => "NXT",
		}
	}

	/// Lua expression used to acquire the primitive at VM initialization,
	/// including explicit safe fallbacks where standard dialects/sandboxes vary.
	pub const fn lua_bind_expr(self) -> &'static str {
		match self {
			VmPrim::StrByte => "(\"\")[\"byte\"]",
			VmPrim::StrSub => "(\"\")[\"sub\"]",
			VmPrim::StrChar => "(\"\")[\"char\"]",
			// Safe fallback when `getfenv` is absent or restricted in a sandbox:
			// returns `_G` without mutating or assuming `setfenv`.
			VmPrim::EnvGet => "(type(getfenv)==\"function\" and getfenv) or function() return _G end",
			// Standard Lua 5.1/Luau `unpack` with `table.unpack` fallback.
			VmPrim::Unpack => "unpack or (table and table.unpack)",
			VmPrim::MathFloor => "math.floor",
			VmPrim::Type => "type",
			VmPrim::RawGet => "rawget",
			VmPrim::RawSet => "rawset",
			VmPrim::SetMetatable => "setmetatable",
			VmPrim::Pcall => "pcall",
			VmPrim::ToString => "tostring",
			VmPrim::TableConcat => "table.concat",
			VmPrim::Error => "error",
			VmPrim::GetMetatable => "getmetatable",
			VmPrim::Select => "select",
			VmPrim::ToNumber => "tonumber",
			VmPrim::TablePack => "(table and table.pack) or function(...) return {n=select(\"#\",...),...} end",
			VmPrim::Next => "next",
		}
	}
}

/// Table-driven declaration of every VM primitive required by a given `Op`.
pub const fn op_required_prims(op: Op) -> &'static [VmPrim] {
	use VmPrim::*;
	match op {
		Op::Jmp
		| Op::Jf
		| Op::Jt
		| Op::LoadNil
		| Op::LoadK
		| Op::Move
		| Op::Not
		| Op::NewTab
		| Op::TabN
		| Op::GetUp
		| Op::SetUp
		| Op::VarArgTab
		| Op::VarArgC
		| Op::VarArgTabN
		| Op::Return
		| Op::Nop => &[],
		Op::MkStr => &[StrChar, MathFloor],
		Op::LoadStr => &[StrByte, StrChar, Unpack, TableConcat, MathFloor],
		Op::GetGlobal | Op::SetGlobal => &[EnvGet],
		Op::Closure => &[Select],
		Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Pow => {
			&[Type, ToNumber, GetMetatable, Error]
		}
		Op::Idiv => &[Type, ToNumber, GetMetatable, Error, MathFloor],
		Op::Unm => &[Type, ToNumber, GetMetatable, Error],
		Op::Len => &[Type, GetMetatable, Pcall, SetMetatable],
		Op::Concat => &[Type, GetMetatable, Error],
		Op::Lt | Op::Le | Op::Gt | Op::Ge => &[Type, GetMetatable, Error],
		Op::Eq | Op::Ne => &[Type, GetMetatable],
		Op::GetTab => &[Type, RawGet, GetMetatable, ToString, Error],
		Op::SetTab => &[Type, RawGet, RawSet, GetMetatable, ToString, Error],
		Op::Call | Op::CallE | Op::CallM | Op::CallT => {
			&[Type, GetMetatable, Error, Unpack, TablePack]
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityManifest {
	pub active_ops: [bool; N_OPS],
	pub vm_prims: Vec<VmPrim>,
	pub scaffold_prims: Vec<ScaffoldPrim>,
	pub has_float_const: bool,
	pub needs_mget: bool,
	pub needs_len_meta: bool,
	pub needs_resolve_call: bool,
	pub needs_makefn: bool,
	pub guard: bool,
	pub env_check: bool,
	pub bind_env_roblox: bool,
}

#[allow(dead_code)]
impl CapabilityManifest {
	/// Deterministically derive the minimal sound capability manifest from the
	/// final compiled `VmProgram` and active build flags.
	pub fn from_program(
		prog: &VmProgram,
		guard: bool,
		env_check: bool,
		bind_env: Option<&str>,
	) -> Self {
		let mut active_ops = prog.used_ops;
		if prog.has_loadstr {
			active_ops[op_index(Op::LoadStr)] = true;
		}

		let mut prim_present = [false; VmPrim::ALL.len()];
		for &p in &VmPrim::BASE {
			prim_present[p as usize] = true;
		}

		for (idx, &used) in active_ops.iter().enumerate() {
			if used {
				if let Some(op) = Op::from_index(idx) {
					for &p in op_required_prims(op) {
						prim_present[p as usize] = true;
					}
				}
			}
		}

		if prog.has_float_const {
			prim_present[VmPrim::ToNumber as usize] = true;
		}
		if guard {
			prim_present[VmPrim::Next as usize] = true;
		}

		let vm_prims: Vec<VmPrim> = VmPrim::ALL
			.iter()
			.copied()
			.filter(|&p| prim_present[p as usize])
			.collect();

		let needs_len_meta = active_ops[op_index(Op::Len)];
		let needs_resolve_call = active_ops[op_index(Op::Call)]
			|| active_ops[op_index(Op::CallE)]
			|| active_ops[op_index(Op::CallM)]
			|| active_ops[op_index(Op::CallT)];
		let needs_makefn = active_ops[op_index(Op::Closure)];
		let needs_mget = prim_present[VmPrim::GetMetatable as usize];

		Self {
			active_ops,
			vm_prims,
			scaffold_prims: ScaffoldPrim::ALL.to_vec(),
			has_float_const: prog.has_float_const,
			needs_mget,
			needs_len_meta,
			needs_resolve_call,
			needs_makefn,
			guard,
			env_check,
			bind_env_roblox: bind_env == Some("roblox"),
		}
	}

	/// Full capability manifest (all 43 ops active).
	pub fn all(guard: bool, env_check: bool, bind_env: Option<&str>) -> Self {
		Self {
			active_ops: [true; N_OPS],
			vm_prims: VmPrim::ALL.to_vec(),
			scaffold_prims: ScaffoldPrim::ALL.to_vec(),
			has_float_const: true,
			needs_mget: true,
			needs_len_meta: true,
			needs_resolve_call: true,
			needs_makefn: true,
			guard,
			env_check,
			bind_env_roblox: bind_env == Some("roblox"),
		}
	}

	#[inline]
	pub fn has_prim(&self, p: VmPrim) -> bool {
		self.vm_prims.contains(&p)
	}

	#[inline]
	pub fn is_op_active(&self, op: Op) -> bool {
		self.active_ops[op_index(op)]
	}

	pub fn active_op_names(&self) -> Vec<&'static str> {
		ALL_OPS
			.iter()
			.filter(|&&op| self.is_op_active(op))
			.map(|op| op.meta().name)
			.collect()
	}

	pub fn vm_prim_names(&self) -> Vec<&'static str> {
		self.vm_prims.iter().map(|p| p.template_ident()).collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parser;
	use crate::symtab;

	fn compile_luau(src: &str, seed: u64, scatter: bool) -> VmProgram {
		let mut block = parser::parse(src, true).expect("parse luau");
		let table = symtab::resolve(&mut block);
		let mut rng = crate::rng::Rng::new(seed);
		super::super::compiler::compile(&block, &table, &mut rng, false, scatter, false)
	}

	#[test]
	fn minimal_const_return_omits_optional_prims() {
		let prog = compile_luau("return 42", 7, true);
		let cap = CapabilityManifest::from_program(&prog, false, false, None);
		assert!(!cap.has_prim(VmPrim::ToNumber), "no float or arith -> no tonumber");
		assert!(!cap.has_prim(VmPrim::GetMetatable), "no meta ops -> no getmetatable");
		assert!(!cap.has_prim(VmPrim::Error), "no error-raising ops -> no error");
		assert!(!cap.has_prim(VmPrim::Select), "no closure/call -> no select");
		assert!(!cap.has_prim(VmPrim::TablePack), "no call -> no table.pack");
		assert!(!cap.has_prim(VmPrim::Next), "guard off -> no next");
		assert!(!cap.needs_makefn && !cap.needs_resolve_call && !cap.needs_len_meta);
	}

	#[test]
	fn capability_manifest_is_seed_invariant_for_program_semantics() {
		let src = "local t = {1, 2, 3}; local function f(x) return x + #t end; return f(4.5)";
		let m1 = CapabilityManifest::from_program(&compile_luau(src, 1, true), true, false, None);
		let m2 = CapabilityManifest::from_program(&compile_luau(src, 999999, true), true, false, None);
		assert_eq!(m1.vm_prims, m2.vm_prims);
		assert_eq!(m1.scaffold_prims, m2.scaffold_prims);
		assert!(m1.has_prim(VmPrim::ToNumber));
		assert!(m1.has_prim(VmPrim::GetMetatable));
		assert!(m1.has_prim(VmPrim::Select));
		assert!(m1.has_prim(VmPrim::TablePack));
		assert!(m1.has_prim(VmPrim::Next));
		assert!(m1.needs_len_meta);
		assert!(m1.needs_makefn);
		assert!(m1.needs_resolve_call);
	}

	#[test]
	fn every_op_declared_prims_are_valid() {
		for &op in &ALL_OPS {
			for &p in op_required_prims(op) {
				assert!(VmPrim::ALL.contains(&p));
			}
		}
	}
}
