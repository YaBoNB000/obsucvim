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

/// Fine-grained activation driver for each `VmPrim` in `CapabilityManifest`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimDriver {
	/// Unconditionally required by the VM carrier, constant proxy, or trampoline.
	InterpreterBase,
	/// Required only when one or more VM opcodes referencing it are active.
	OpcodeDriven,
	/// Required when float constants exist (`CDEC`) or numeric coercion ops are active.
	OpcodeOrConstDriven,
	/// Required only when anti-tamper Guard (`--guard` / default `v15`) is enabled.
	GuardFlag,
}

/// Authoritative single-source-of-truth metadata for each `VmPrim`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimMeta {
	pub prim: VmPrim,
	pub tier: CapabilityTier,
	pub driver: PrimDriver,
	pub template_ident: &'static str,
	pub lua_bind_expr: &'static str,
	pub luau_only: bool,
	pub v15_only: bool,
	pub has_fallback: bool,
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
	pub const COUNT: usize = 19;

	pub const ALL: [VmPrim; Self::COUNT] = [
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

	pub const fn meta(self) -> PrimMeta {
		use CapabilityTier::*;
		use PrimDriver as D;
		match self {
			VmPrim::StrByte => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "BYTE",
				lua_bind_expr: "(\"\")[\"byte\"]",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::StrSub => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "SUB",
				lua_bind_expr: "(\"\")[\"sub\"]",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::StrChar => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "CHAR",
				lua_bind_expr: "(\"\")[\"char\"]",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::EnvGet => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "GFE",
				// Safe fallback when `getfenv` is absent in a restricted sandbox:
				// preserves native `getfenv` identity when present, otherwise resolves `_G`.
				lua_bind_expr:
					"(type(getfenv)==\"function\" and getfenv) or function() return type(_G)==\"table\" and _G or {} end",
				luau_only: false,
				v15_only: false,
				has_fallback: true,
			},
			VmPrim::Unpack => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "UNP",
				// Scheme A contract: native `unpack` -> `table.unpack` -> pure-Lua slice
				// unpacker preserving (i, j) bounds, empty i > j intervals (0 returns),
				// and both intermediate and trailing `nil`s (`select("#", ...)` exact).
				lua_bind_expr:
					"(type(unpack)==\"function\" and unpack) or (type(table)==\"table\" and type(table.unpack)==\"function\" and table.unpack) or (function() local function _uf(t, i, j) if i == nil then i = 1 end if j == nil then j = #t end if i > j then return end local r = j - i if r == 0 then return t[i] elseif r == 1 then return t[i], t[i + 1] elseif r == 2 then return t[i], t[i + 1], t[i + 2] elseif r == 3 then return t[i], t[i + 1], t[i + 2], t[i + 3] end return t[i], t[i + 1], t[i + 2], t[i + 3], _uf(t, i + 4, j) end return _uf end)()",
				luau_only: false,
				v15_only: false,
				has_fallback: true,
			},
			VmPrim::MathFloor => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "FLR",
				lua_bind_expr: "math.floor",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::Type => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "TYP",
				lua_bind_expr: "type",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::RawGet => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeDriven,
				template_ident: "RGET",
				lua_bind_expr: "rawget",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::RawSet => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeDriven,
				template_ident: "RSET",
				lua_bind_expr: "rawset",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::SetMetatable => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "SMT",
				lua_bind_expr: "setmetatable",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::Pcall => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "PCAL",
				lua_bind_expr: "pcall",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::ToString => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "TSTR",
				lua_bind_expr: "tostring",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::TableConcat => PrimMeta {
				prim: self,
				tier: InterpreterBase,
				driver: D::InterpreterBase,
				template_ident: "TCAT",
				lua_bind_expr: "table.concat",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::Error => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeDriven,
				template_ident: "ERR",
				lua_bind_expr: "error",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::GetMetatable => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeDriven,
				template_ident: "GMT",
				lua_bind_expr: "getmetatable",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::Select => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeDriven,
				template_ident: "SEL",
				lua_bind_expr: "select",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::ToNumber => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeOrConstDriven,
				template_ident: "TONUM",
				lua_bind_expr: "tonumber",
				luau_only: false,
				v15_only: false,
				has_fallback: false,
			},
			VmPrim::TablePack => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::OpcodeDriven,
				template_ident: "TPACK",
				// Equivalent `n = select("#", ...)` multivalue pack preserving middle & trailing nils.
				lua_bind_expr:
					"(type(table)==\"table\" and type(table.pack)==\"function\" and table.pack) or function(...) return {n=select(\"#\",...),...} end",
				luau_only: true,
				v15_only: true,
				has_fallback: true,
			},
			VmPrim::Next => PrimMeta {
				prim: self,
				tier: ProgramDriven,
				driver: D::GuardFlag,
				template_ident: "NXT",
				lua_bind_expr: "next",
				luau_only: false,
				v15_only: true,
				has_fallback: false,
			},
		}
	}

	pub const fn tier(self) -> CapabilityTier {
		self.meta().tier
	}

	pub const fn driver(self) -> PrimDriver {
		self.meta().driver
	}

	/// Internal template local binding identifier (before `rename_vm_locals`).
	pub const fn template_ident(self) -> &'static str {
		self.meta().template_ident
	}

	/// Lua expression used to acquire the primitive at VM initialization,
	/// including explicit safe fallbacks where standard dialects/sandboxes vary.
	pub const fn lua_bind_expr(self) -> &'static str {
		self.meta().lua_bind_expr
	}

	pub const fn is_luau_only(self) -> bool {
		self.meta().luau_only
	}

	pub const fn is_v15_only(self) -> bool {
		self.meta().v15_only
	}

	pub const fn has_fallback(self) -> bool {
		self.meta().has_fallback
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
		// Phase 18 P1-3: distributed runtime string-cache canaries.
		Op::Canary => &[StrChar, StrByte, Type],
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
			&[Type, GetMetatable, Error, Unpack, Select, TablePack]
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

	/// Full capability manifest (all 44 ops active).
	pub fn all(guard: bool, env_check: bool, bind_env: Option<&str>) -> Self {
		Self {
			active_ops: [true; N_OPS],
			vm_prims: VmPrim::ALL
				.iter()
				.copied()
				.filter(|&p| p != VmPrim::Next || guard)
				.collect(),
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

	/// Authoritative gate deciding whether `prim` is bound into `P[slot]` for a
	/// template build (`v15` vs legacy `vm`, `luau_target` vs generic Lua 5.1).
	#[inline]
	pub fn prim_enabled_in_template(&self, prim: VmPrim, v15: bool, luau_target: bool) -> bool {
		if prim.is_luau_only() && !luau_target {
			return false;
		}
		if prim.is_v15_only() && !v15 {
			return false;
		}
		if v15 {
			self.has_prim(prim)
		} else {
			prim.driver() != PrimDriver::GuardFlag
		}
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
		super::super::compiler::compile(&block, &table, &mut rng, false, scatter, false, false)
	}

	fn emit_template_for(src: &str, seed: u64, v15: bool, luau: bool, guard: bool) -> String {
		let mut block = parser::parse(src, luau).expect("parse");
		let table = symtab::resolve(&mut block);
		let mut rng = crate::rng::Rng::new(seed);
		let prog = super::super::compiler::compile(&block, &table, &mut rng, !luau, v15, false, guard);
		let caps = CapabilityManifest::from_program(&prog, guard, false, None);
		let carrier_lengths: Vec<usize> = prog
			.fns
			.iter()
			.map(|b| prog.carrier.encode(b).len())
			.collect();
		super::super::template::generate(
			&prog.opmap,
			&prog.slot_perm,
			&prog.slot_perms,
			(prog.lane_base, prog.lane_step),
			&prog.carrier,
			&carrier_lengths,
			luau,
			&mut rng,
			prog.fns.len(),
			v15,
			prog.has_loadstr,
			&prog.nop_sites,
			&prog.operand_sums,
			&prog.operand_sums2,
			&prog.chaff_folds,
			(prog.chaff_mul, prog.chaff_coeffs, prog.chaff_pos, prog.chaff_fn),
			(prog.ck_km, prog.ck_kc),
			(prog.mk1, prog.mk2),
			(prog.blob_km, prog.blob_kc, prog.blob_seed, prog.blob_step),
			prog.section_tags,
			prog.consts_mask_safe,
			&prog.block_starts,
			&prog.s_indices,
			false,
			&prog.fv_sets,
			(prog.fv_m1, prog.fv_mc),
			None,
			false,
			guard,
			guard,
			&caps,
		)
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
	fn every_op_declared_prims_are_valid_and_discriminants_match_index() {
		let mut idents = std::collections::BTreeSet::new();
		for (idx, &p) in VmPrim::ALL.iter().enumerate() {
			assert_eq!(p as usize, idx, "VmPrim::ALL order must match repr(u8)");
			let meta = p.meta();
			assert_eq!(meta.prim, p);
			assert!(!meta.template_ident.is_empty());
			assert!(!meta.lua_bind_expr.is_empty());
			assert!(idents.insert(meta.template_ident), "duplicate template_ident");
			let is_base = VmPrim::BASE.contains(&p);
			assert_eq!(is_base, meta.tier == CapabilityTier::InterpreterBase);
		}
		for &op in &ALL_OPS {
			for &p in op_required_prims(op) {
				assert!(VmPrim::ALL.contains(&p));
			}
		}
	}

	#[test]
	fn template_consumes_caps_lua_bind_expr_and_gates_luau_and_guard_prims() {
		// 1. v15 + guard + call: must emit fallback expressions from VmPrim::lua_bind_expr
		//    and bind `next` (`VmPrim::Next`).
		let t_guard_call = emit_template_for("print(1, 2)", 42, true, true, true);
		assert!(
			t_guard_call.contains(VmPrim::EnvGet.lua_bind_expr()),
			"template must emit VmPrim::EnvGet::lua_bind_expr()"
		);
		assert!(
			t_guard_call.contains(VmPrim::Unpack.lua_bind_expr()),
			"template must emit VmPrim::Unpack::lua_bind_expr()"
		);
		assert!(
			t_guard_call.contains(VmPrim::TablePack.lua_bind_expr()),
			"template must emit VmPrim::TablePack::lua_bind_expr() when calls exist"
		);
		assert!(
			t_guard_call.contains("= next\n"),
			"guard=true v15 template must bind VmPrim::Next"
		);
		let _ = parser::parse(&t_guard_call, true).expect("t_guard_call parses as valid Luau");

		// 2. v15 + --no-guard + no call: must omit TablePack (`TPACK`) and Next (`NXT`)
		let t_min_noguard = emit_template_for("return 42", 42, true, true, false);
		assert!(!t_min_noguard.contains("table.pack"), "no call -> no table.pack");
		assert!(!t_min_noguard.contains("TPACK"), "no call -> no unbound TPACK local");
		assert!(!t_min_noguard.contains("= next\n"), "guard=false -> no next binding");
		assert!(!t_min_noguard.contains("NXT"), "guard=false -> no unbound NXT local");
		let _ = parser::parse(&t_min_noguard, true).expect("t_min_noguard parses as valid Luau");

		// 3. Non-Luau template path (`luau = false`): must never bind Luau-only `TablePack`
		//    and must parse cleanly under generic Lua 5.1 parser mode.
		let t_lua51 = emit_template_for("print(1, 2)", 42, false, false, false);
		assert!(!t_lua51.contains("table.pack"), "non-Luau template must not bind table.pack");
		assert!(!t_lua51.contains("TPACK"), "non-Luau template must not emit TPACK");
		assert!(t_lua51.contains(VmPrim::EnvGet.lua_bind_expr()));
		assert!(t_lua51.contains(VmPrim::Unpack.lua_bind_expr()));
		let _ = parser::parse(&t_lua51, false).expect("t_lua51 parses as valid Lua 5.1");
	}
}
