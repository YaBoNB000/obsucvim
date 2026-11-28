
use crate::ast::*;
use crate::rng::Rng;
use crate::symtab::SymTable;
use crate::vmgen::isa::{self, Carrier, Const, Instr, Op, OpMap, OperandRole, ScatterMap};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
enum CellKind {
	Plain(u16),
	Slot(u16),
	Up(u16),
}

pub struct VmProgram {
	pub has_loadstr: bool,
	pub opmap: OpMap,
	pub slot_perm: [u8; 4],
	pub slot_perms: Vec<[u8; 4]>,
	pub lane_base: u8,
	pub lane_step: u8,
	pub carrier: Carrier,
	pub fns: Vec<Vec<u8>>,
	pub nop_sites: Vec<Vec<u16>>,
	pub ck_km: u32,
	pub ck_kc: u32,
	pub mk1: u16,
	pub mk2: u16,
	pub blob_km: u32,
	pub blob_kc: u32,
	pub blob_seed: u32,
	pub blob_step: u32,
	/// Phase B: per-prototype field layouts (index = fn slot, incl. main at fns.len()).
	pub proto_layouts: Vec<super::proto_layout::ProtoFieldLayout>,
	pub operand_sums: Vec<u64>,
	pub operand_sums2: Vec<u64>,
	pub chaff_folds: Vec<u16>,
	pub chaff_mul: u16,
	pub chaff_coeffs: [u16; 4],
	pub chaff_pos: u16,
	pub chaff_fn: u16,
	pub block_starts: Vec<Vec<u16>>,
	pub s_indices: Vec<u8>,
	pub consts_mask_safe: bool,
	/// Exact opcode usage and frequency across all compiled functions (after H3 noise).
	pub used_ops: [bool; isa::N_OPS],
	pub op_hist: [u32; isa::N_OPS],
	/// Constant pool breakdown `[nil, bool, int, float_or_nonfinite, str]`.
	pub const_counts: [u32; 5],
	pub has_float_const: bool,
	pub total_insns: u32,
	pub fused_insns: u32,
	/// V17 P1-b：每函数 fused 词汇（(op_index, op_index) 对，≤FUSE_TOP_K）；
	/// super_insn 关时恒空。fvm 掩码常量对（fv_m1/fv_mc）off 态为 0。
	pub fv_sets: Vec<Vec<(u8, u8)>>,
	pub fv_m1: u16,
	pub fv_mc: u16,
}

#[derive(Debug, Clone, Copy)]
struct Label(u32);

struct Ctx<'a> {
	program: &'a mut VmProgram,
	rng: &'a mut Rng,
	symtab: &'a crate::symtab::SymTable,
	lua51: bool,
	scatter: bool,
	super_insn: bool,
	next_fn_slot: usize,
	scopes: Vec<HashMap<SymId, CellKind>>,
	upvals: HashMap<SymId, u16>,
	upsrc: Vec<u16>,
	up_perm: Vec<usize>,
	next_reg: u16,
	nparams: u16,
	vararg: bool,
	consts: Vec<Const>,
	const_map: HashMap<(u8, u64), u16>,
	code: Vec<Instr>,
	labels: HashMap<u32, (Option<usize>, Vec<usize>)>,
	loops: Vec<(Label, Label)>,
	cell_loops: Vec<HashSet<SymId>>,
	/// Phase 18 P1-2: ciphertexts whose references all live in one loop-free
	/// function body, so their cache slot may be cleared after the last use.
	str_clearable: &'a HashSet<Vec<u8>>,
	/// Phase 18 P1-3: distributed runtime string-cache canaries are only emitted
	/// for guard-enabled builds (the canary fail path is the guard abort path).
	canary: bool,
	/// CCPD (rebased plan): when on, `finish` builds the per-function
	/// CFG + block variants and emits the id8 directory plus the variant
	/// bank as the code section. Off: no extra work, no rng draws.
	ccpd: bool,
}

fn derive_upvalue_permutation(seed: u64, fn_index: usize, count: usize, scatter: bool) -> Vec<usize> {
	let mut order: Vec<usize> = (0..count).collect();
	if scatter && count > 1 {
		let fn_salt = (fn_index as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
		let mut rng = Rng::new(seed ^ fn_salt ^ (count as u64).rotate_left(7));
		rng.shuffle(&mut order);
	}
	let mut old_to_new = vec![0usize; count];
	for (new_idx, &old_idx) in order.iter().enumerate() {
		old_to_new[old_idx] = new_idx;
	}
	old_to_new
}

fn const_key(c: &Const) -> (u8, u64) {
	match c {
		Const::Nil => (0, 0),
		Const::Bool(b) => (1, *b as u64),
		Const::Num(v) => (2, v.to_bits()),
		Const::Str(b) => {
			let mut h: u64 = 0xcbf2_9ce4_8422_2325;
			for x in b {
				h ^= *x as u64;
				h = h.wrapping_mul(0x1_0000_0001_b3);
			}
			(3, h)
		}
	}
}

/// CCPD runtime-audit ground truth (rebased plan 15.2): with
/// `OBSUCVIM_CCPD_AUDIT=<path>` set in an audit-enabled build, the
/// per-function CFG (incoming edge sets, edge table, closure sites)
/// is appended to that file; `tests/ccpd_runtime_audit_test.py`
/// cross-checks the runtime edge log against it. Production builds
/// compile the gate away (see `audit_gate`).
fn ccpd_audit_dump(line: &str) {
	if let Some(path) = crate::audit_gate::audit_var("OBSUCVIM_CCPD_AUDIT") {
		use std::io::Write;
		if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
			let _ = writeln!(f, "{line}");
		}
	}
}

fn noise_audit(line: &str) {
	if let Some(path) = crate::audit_gate::audit_var("OBSUCVIM_NOISE_AUDIT") {
		use std::io::Write;
		if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
			let _ = writeln!(f, "{line}");
		}
	}
}

#[derive(Default)]
struct ScatterNoiseStats {
	high_regs: usize,
	nil_holes: usize,
	decoy_holes: usize,
}

pub(crate) fn operand_perm_from_code(code: u8) -> [u8; 4] {
	let mut rem = code as usize % 24;
	let mut pool = vec![0u8, 1, 2, 3];
	let mut out = [0u8; 4];
	for (slot, fact) in [6usize, 2, 1, 1].iter().enumerate() {
		let pick = if slot == 3 { 0 } else { rem / *fact };
		if slot != 3 {
			rem %= *fact;
		}
		out[slot] = pool.remove(pick);
	}
	out
}

fn sample_noise_operand(rng: &mut Rng, lane: &[u16], all: &[u16]) -> u16 {
	let pool = if lane.is_empty() { all } else { lane };
	let roll = rng.int(0, 99);
	if roll < 58 && !pool.is_empty() {
		pool[rng.int(0, pool.len() as i64 - 1) as usize]
	} else if roll < 86 {
		rng.int(0, 31) as u16
	} else if roll < 97 {
		rng.int(96, 4095) as u16
	} else {
		rng.int(0, 65535) as u16
	}
}

impl<'a> Ctx<'a> {
	fn new_main(
		program: &'a mut VmProgram,
		rng: &'a mut Rng,
		symtab: &'a crate::symtab::SymTable,
		lua51: bool,
		scatter: bool,
		super_insn: bool,
		str_clearable: &'a HashSet<Vec<u8>>,
		canary: bool,
		ccpd: bool,
	) -> Ctx<'a> {
		Ctx {
			program,
			rng,
			symtab,
			lua51,
			scatter,
			super_insn,
			next_fn_slot: 0,
			scopes: vec![HashMap::new()],
			upvals: HashMap::new(),
			upsrc: Vec::new(),
			up_perm: Vec::new(),
			next_reg: 0,
			nparams: 0,
			vararg: false,
			consts: Vec::new(),
			const_map: HashMap::new(),
			code: Vec::new(),
			labels: HashMap::new(),
			loops: Vec::new(),
			cell_loops: Vec::new(),
			str_clearable,
			canary,
			ccpd,
		}
	}

	fn tmp(&mut self) -> u16 {
		let r = self.next_reg;
		self.next_reg += 1;
		r
	}

	fn reserve(&mut self, n: u16) -> u16 {
		let r = self.next_reg;
		self.next_reg += n;
		r
	}

	fn kidx(&mut self, c: Const) -> u16 {
		// Non-finite numbers (+Inf, -Inf, NaN) are encoded via dedicated tags 5/6/7
		// in `Const::encode` and `EP` tags `tm[7..9]`, so `consts_mask_safe` remains
		// true for the entire program (no plaintext constant-table fallback).
		let k = const_key(&c);
		if let Some(&i) = self.const_map.get(&k) {
			return i;
		}
		let i = self.consts.len() as u16;
		self.consts.push(c);
		self.const_map.insert(k, i);
		i
	}

	fn kidx_unique(&mut self, c: Const) -> u16 {
		let i = self.consts.len() as u16;
		self.consts.push(c);
		i
	}

	fn emit(&mut self, ins: Instr) -> usize {
		self.code.push(ins);
		self.code.len() - 1
	}

	fn insert_noise_instr(&mut self, pos: usize, ins: Instr) {
		self.code.insert(pos, ins);
		for (_, (at, jumps)) in self.labels.iter_mut() {
			if let Some(a) = at {
				if *a >= pos {
					*a += 1;
				}
			}
			for j in jumps.iter_mut() {
				if *j >= pos + 1 {
					*j += 1;
				}
			}
		}
	}

	fn new_label(&mut self) -> Label {
		Label(self.rng.int(1, 1_000_000_000) as u32)
	}

	fn here(&mut self, l: &Label) {
		let e = self
			.labels
			.entry(l.0)
			.or_insert_with(|| (None, Vec::new()));
		e.0 = Some(self.code.len());
	}

	fn jmp(&mut self, op: Op, reg: u16, l: &Label) {
		let pos = self.emit(Instr::ab(op, reg, 0));
		self.labels
			.entry(l.0)
			.or_insert_with(|| (None, Vec::new()))
			.1
			.push(pos + 1);
	}



	fn lookup(&self, s: SymId) -> Option<CellKind> {
		for sc in self.scopes.iter().rev() {
			if let Some(&k) = sc.get(&s) {
				return Some(k);
			}
		}
		None
	}

	fn declare(&mut self, s: SymId, k: CellKind) {
		// invariant: push_scope() installs the root scope at compiler construction and pop_scope never removes it
		self.scopes.last_mut().unwrap().insert(s, k);
	}

	fn cur_loop_captures(&self) -> Option<&HashSet<SymId>> {
		self.cell_loops.last()
	}

	fn push_scope(&mut self) {
		self.scopes.push(HashMap::new());
	}

	fn pop_scope(&mut self) {
		self.scopes.pop();
	}

	/// The canonical ISA table is the only operand-policy source.  Scatter
	/// behavior is derived from each lane's semantic role, so compiler,
	/// verifier, fusion, and CCPD cannot drift into separate role tables.
	fn op_roles(op: Op) -> [OperandRole; 4] {
		op.meta().roles()
	}

	fn scatter_operands(&mut self, nregs: u16) -> (Vec<u16>, ScatterNoiseStats) {
		let mut stats = ScatterNoiseStats::default();
		let n = nregs as usize;
		let n_high = if n == 0 {
			0
		} else {
			self.rng.int(((n + 4) / 5).max(1) as i64, ((n + 1) / 2).max(1) as i64)
				as usize
		};
		let n_low = n.saturating_sub(n_high);
		let low_max = n.max(1) + (n / 2).min(64).max(8);
		let high_start = (low_max + 1).max(96);
		let high_end = 4095usize.min(u16::MAX as usize);
		if !(high_start <= u16::MAX as usize
			&& high_end >= high_start
			&& high_end - high_start + 1 >= n_high)
		{
			crate::limits::fatal("input exceeds compiler limit: H3 sparse register pool exhausted");
		}
		let mut low_pool: Vec<u16> = (1..=low_max as u16).collect();
		let mut high_pool: Vec<u16> = (high_start as u16..=high_end as u16).collect();
		self.rng.shuffle(&mut low_pool);
		self.rng.shuffle(&mut high_pool);
		let mut sigma: Vec<u16> = low_pool[..n_low].to_vec();
		sigma.extend_from_slice(&high_pool[..n_high]);
		self.rng.shuffle(&mut sigma);
		stats.high_regs = sigma.iter().filter(|&&v| v >= 96).count();

		let nc = self.consts.len();
		let pad_lo = (nc / 3).max(3);
		let pad_hi = (nc * 2 / 3).max(pad_lo + 3);
		let pad = self.rng.int(pad_lo as i64, pad_hi as i64) as usize;
		let cmax = nc + pad;
		let mut cpos: Vec<usize> = (0..cmax).collect();
		self.rng.shuffle(&mut cpos);
		let kappa: Vec<usize> = cpos[..nc].to_vec();
		let mut consts2: Vec<Const> = vec![Const::Nil; cmax];
		let mut used = vec![false; cmax];
		for (i, c) in self.consts.drain(..).enumerate() {
			consts2[kappa[i]] = c;
			used[kappa[i]] = true;
		}
		let nil_pct = self.rng.int(28, 72);
		for j in 0..cmax {
			if used[j] {
				continue;
			}
			if self.rng.int(0, 99) < nil_pct {
				stats.nil_holes += 1;
				continue;
			}
			stats.decoy_holes += 1;
			consts2[j] = match self.rng.int(0, 4) {
				0 => Const::Num(self.rng.int(0, 31) as f64),
				1 => Const::Num(self.rng.int(96, 4095) as f64),
				2 => Const::Num(self.rng.int(4096, 999_999) as f64),
				3 => Const::Bool(self.rng.int(0, 1) == 1),
				_ => Const::Str(vec![self.rng.int(65, 90) as u8, self.rng.int(97, 122) as u8]),
			};
		}
		self.consts = consts2;

		let nu = self.upsrc.len();
		let mut consumed: Vec<usize> = (0..nu).collect();
		self.rng.shuffle(&mut consumed);
		let tau = self.up_perm.clone();
		// invariant: up_perm is built by derive_upvalue_permutation(ups.len()) for this very frame, so its length is nu by construction
		assert_eq!(tau.len(), nu, "upvalue permutation length mismatch");
		let old_upsrc = std::mem::take(&mut self.upsrc);
		self.upsrc.resize(nu, 0);
		for (old_idx, &new_idx) in tau.iter().enumerate() {
			self.upsrc[new_idx] = old_upsrc[old_idx];
		}

		for ins in self.code.iter_mut() {
			let roles = Self::op_roles(ins.op);
			let ops = [ins.a, ins.b, ins.c, ins.d];
			for k in 0..4 {
				let v = ops[k] as usize;
				let nv = match roles[k].scatter() {
					ScatterMap::RegisterZeroBased => sigma[v] - 1,
					ScatterMap::ConstantPermutation => kappa[v] as u16,
					ScatterMap::UpvaluePermutation => tau[v] as u16,
					ScatterMap::Identity => ops[k],
				};
				match k {
					0 => ins.a = nv,
					1 => ins.b = nv,
					2 => ins.c = nv,
					_ => ins.d = nv,
				}
			}
		}

		(sigma, stats)
	}

	/// Phase 18 P1-1/P1-2: annotate every `LoadStr` operand with its cache
	/// policy. Runs on the final instruction stream (after constant pruning and
	/// operand scatter) so the flag bits can never be scrambled by a remap.
	///
	///   bit 15 (0x8000) = cached: the constant has >1 static reference, so the
	///                    handler decodes once per frame and reuses the slot.
	///   bit 14 (0x4000) = clear-after-use: this is the last static reference
	///                    AND every reference lives in one loop-free function.
	///
	/// Single-reference constants keep a plain index: the handler decodes
	/// inline and never touches the cache table.
	/// Phase 18 P1-3: distribute the four runtime string-cache canaries over the
	/// generated instruction stream.
	///
	/// The 4 split points are `floor(n / 5) * k` for `k = 1..4` (never the very
	/// first or the very last instruction). Which canary id lands on which split
	/// point is decided by `Rng::shuffle`, so the same source yields a different
	/// distribution per seed. Programs shorter than 5 instructions get all four
	/// canaries immediately before the final instruction instead of forcing a
	/// five-way split of an already tiny stream.
	///
	/// Jump targets are absolute 1-based instruction indices at this point (the
	/// label fixup pass already ran), so every insertion shifts the targets that
	/// point at or after the insertion point.
	fn insert_canaries(&mut self) {
		if !self.canary {
			return;
		}
		let n = self.code.len();
		if n == 0 {
			return;
		}
		let mut ids: Vec<u16> = vec![0, 1, 2, 3];
		self.rng.shuffle(&mut ids);
		// H3-6: never insert inside an open call-result window. The consumer
		// instructions of a multi-result call address the window relative to
		// the call, so a canary dropped between the call and its consumer
		// shifts the window and corrupts the operands. Only window-free
		// indices are used.
		let window = call_window_mask(&self.code);
		let allowed: Vec<usize> = (0..n).filter(|&i| !window[i]).collect();
		if allowed.is_empty() {
			return;
		}
		let mut placements: Vec<(usize, u16)> = Vec::with_capacity(4);
		for k in 1..=4usize {
			let ideal = if n >= 5 { (n / 5) * k } else { n - 1 };
			// Nearest window-free index to the ideal split point (ties broken
			// towards the lower index so placement stays seed-stable).
			let mut best: Option<usize> = None;
			let mut best_d = usize::MAX;
			for &i in &allowed {
				let d = i.abs_diff(ideal);
				if d < best_d {
					best_d = d;
					best = Some(i);
				}
			}
			if let Some(pos) = best {
				if !placements.iter().any(|&(q, _)| q == pos) {
					placements.push((pos, ids[k - 1]));
				}
			}
		}
		// Insert back to front so earlier split points stay valid. This runs
		// before the H3 noise pass and the label fixup, so it must go through
		// `insert_noise_instr` (which keeps `self.labels` in sync) instead of
		// patching already-fixed absolute jump targets.
		placements.sort_by(|a, b| b.0.cmp(&a.0));
		for (pos, id) in placements {
			self.insert_noise_instr(pos, Instr::abcd(Op::Canary, id, 0, 0, 0));
		}
	}

	fn apply_loadstr_flags(&mut self) {
		loadstr_flags(&mut self.code, &self.consts, self.str_clearable);
	}

	fn finish(
		mut self,
		mut nregs: u16,
		fn_index: usize,
	) -> (Vec<u8>, Vec<u16>, u64, u64, u16, Vec<u16>) {
		let needs_return = self
			.code
			.last()
			.map(|i| i.op != Op::Return)
			.unwrap_or(true);
		if needs_return {
			self.emit(Instr::abcd(Op::Return, 0, 0, 0, 0));
		}
		let base_code_len = self.code.len();
		// Phase 18 P1-3: distribute the runtime canaries before the H3 noise pass.
		// `nop_sites` records absolute 1-based instruction positions that the VM
		// neutralizes at load time, so every insertion has to happen before those
		// positions are computed (the noise pass and the label fixup both run
		// after this point).
		self.insert_canaries();
		let nop_count: usize;
		let mut active_count = 0usize;
		let mut dead_regs = 0usize;
		if self.scatter {
			let mut nr = self.rng.fork(0x5343_4154_5445_5200 ^ fn_index as u64);

			dead_regs = nr.int(2, 4) as usize;
			let dead_base = nregs;
			nregs = nregs
				.checked_add(dead_regs as u16)
				.unwrap_or_else(|| {
					crate::limits::fatal("input exceeds compiler limit: H3 dead register overflow")
				});
			let decoy_k_count = nr.int(2, 5) as usize;
			let mut decoy_ks = Vec::with_capacity(decoy_k_count);
			for _ in 0..decoy_k_count {
				let v = if nr.int(0, 99) < 75 {
					nr.int(0, 31)
				} else {
					nr.int(96, 4095)
				};
				decoy_ks.push(self.kidx(Const::Num(v as f64)));
			}

			let total_lo = (base_code_len / 12).max(2);
			let total_hi = (base_code_len / 5).max(total_lo + 1);
			let total = nr.int(total_lo as i64, total_hi as i64) as usize;
			active_count = nr.int(1, (total / 2).max(1) as i64) as usize;
			nop_count = total - active_count;
			let mut active_flags = vec![true; active_count];
			active_flags.extend(std::iter::repeat(false).take(nop_count));
			nr.shuffle(&mut active_flags);

			let orig_len = self.code.len();
			let window_mask = call_window_mask(&self.code);
			let allowed: Vec<usize> = (0..orig_len)
				.filter(|&p| !window_mask[p])
				.collect();
			let mut placements: Vec<(usize, Instr)> = Vec::with_capacity(active_flags.len());

			for active in active_flags {
				let ins = if active {
					let dst = dead_base + nr.int(0, dead_regs as i64 - 1) as u16;
					match nr.int(0, 3) {
						0 => Instr::ab(
							Op::LoadK,
							dst,
							decoy_ks[nr.int(0, decoy_ks.len() as i64 - 1) as usize],
						),
						1 => {
							let src = dead_base + nr.int(0, dead_regs as i64 - 1) as u16;
							Instr::ab(Op::Move, dst, src)
						}
						2 => Instr::ab(Op::LoadNil, dst, 1),
						_ => Instr::ab(Op::NewTab, dst, 0),
					}
				} else {
					Instr::op(Op::Nop)
				};
				let drawn = nr.int(0, orig_len as i64 - 1) as usize;
				let pos = if window_mask[drawn] {
					allowed[drawn % allowed.len()]
				} else {
					drawn
				};
				placements.push((pos, ins));
			}
			placements.sort_by(|a, b| b.0.cmp(&a.0));
			for (pos, ins) in placements {
				self.insert_noise_instr(pos, ins);
			}

			let final_mask = call_window_mask(&self.code);
			for (idx, ins) in self.code.iter().enumerate() {
				// invariant: self-check of the H3-6 noise placer, which only ever targets
				// dead registers outside open call-result windows by its own construction
				let is_window_op = matches!(
					ins.op,
					Op::Call | Op::CallE | Op::CallM | Op::CallT | Op::Return
				);
				assert!(
					!(final_mask[idx]
						&& !is_window_op
						&& ins.op != Op::Nop
						&& matches!(Self::op_roles(ins.op)[0], OperandRole::RegWrite | OperandRole::RegRange | OperandRole::RegBase | OperandRole::CallBase)
						&& ins.a >= dead_base),
					"H3-6 invariant violated: fn {} instruction {} targets dead \
					 register {} inside an open call-result window",
					fn_index,
					idx,
					ins.a
				);
			}
		} else {
			nop_count = (self.code.len() / 10).max(1);
			for _ in 0..nop_count {
				// Never append past the final instruction: the stream must
				// keep ending in Return (the bytecode verifier enforces
				// this, and a trailing Nop would be dead weight anyway).
				let pos = self.rng.int(0, self.code.len() as i64 - 1) as usize;
				self.insert_noise_instr(pos, Instr::op(Op::Nop));
			}
		}
		for (_, (pos, jumps)) in &self.labels {
			if let Some(base) = pos {
				let target = (*base as u16) + 1;
				for &p in jumps {
					self.code[p - 1].b = target;
				}
			}
		}
		if let Err(e) = super::verify::verify_pre_scatter(
			fn_index,
			&self.code,
			&self.consts,
			self.upsrc.len(),
			self.program.fns.len(),
			nregs,
			self.canary,
		) {
			crate::limits::fatal(&e.to_string());
		}
		let mut mkstr_candidates = HashSet::new();
		for ins in self.code.iter_mut() {
			if ins.op == Op::LoadK {
				if let Some(Const::Str(bs)) = self.consts.get(ins.b as usize) {
					if bs.len() <= 5 {
						mkstr_candidates.insert(ins.b);
						let mut bytes = [0u16; 5];
						for (i, &b) in bs.iter().enumerate() {
							bytes[i] = b as u16;
						}
						let len = bs.len() as u16;
						ins.op = Op::MkStr;
						ins.b = bytes[0] + bytes[1] * 256;
						ins.c = bytes[2] + bytes[3] * 256;
						ins.d = bytes[4] + len * 256;
					}
				}
			}
		}
		prune_dead_short_string_constants(
			&mut self.code,
			&mut self.consts,
			&mkstr_candidates,
		);
		let (s_tab, scatter_stats) = if self.scatter {
			let (slots, stats) = self.scatter_operands(nregs);
			(Some(slots), stats)
		} else {
			(None, ScatterNoiseStats::default())
		};
		let mk1 = self.program.mk1 as u32;
		let mk2 = self.program.mk2 as u32;
		for ins in self.code.iter_mut() {
			if ins.op == Op::MkStr {
				// Structural invariant checked here, the last point where
				// the packed length in d is still readable (the mask below
				// intentionally scrambles it for the final record stream).
				if ins.d / 256 > 5 {
					crate::limits::fatal("bytecode verifier: MkStr packed length exceeds 5");
				}
				let msk = (mk1 * ins.a as u32 + mk2) % 65536;
				ins.b = ((ins.b as u32 + msk) % 65536) as u16;
				ins.c = ((ins.c as u32 + msk) % 65536) as u16;
				ins.d = ((ins.d as u32 + msk) % 65536) as u16;
			}
		}
		self.apply_loadstr_flags();
		let map = self.program.opmap.clone();
		let perm: [u8; 4] = if self.scatter {
			self.program.slot_perms[fn_index]
		} else {
			self.program.slot_perm
		};
		// V17 P1-b：leaders 提前到塌缩之前（塌缩禁忌输入 + 块边界数据源）；
		// 公式与不融合路径下的原位计算逐句相同——off 态零行为差。
		let mut leaders: Vec<u16> = vec![1];
		{
			let ncode0 = self.code.len();
			for (idx, ins) in self.code.iter().enumerate() {
				if matches!(ins.op, Op::Jmp | Op::Jf | Op::Jt) {
					if ins.b >= 1 && ins.b as usize <= ncode0 {
						leaders.push(ins.b);
					}
					if idx + 2 <= ncode0 {
						leaders.push(idx as u16 + 2);
					}
				}
			}
			leaders.sort_unstable();
			leaders.dedup();
		}
		// V17 P1-b：数据流驱动塌缩（统计→确定性选词→collapse→leader 重映射）。
		// 全程零 rng；映射不变量失守时整体回退本函数（release 兜底 debug_assert）。
		let mut fv_vocab: Vec<(u8, u8)> = Vec::new();
		let fused_stream: Option<Vec<super::fuse::FRec>> = if self.super_insn {
			use super::fuse;
			let st = fuse::stats(&self.code);
			let vocab = fuse::select_vocab(&st, fuse::FUSE_TOP_K);
			if vocab.is_empty() {
				None
			} else {
				let lset: std::collections::BTreeSet<u16> = leaders.iter().cloned().collect();
				let (frecs, old_to_new) = fuse::collapse(&self.code, &vocab, &lset);
				let mapped: Vec<u16> =
					leaders.iter().map(|&t| old_to_new[(t - 1) as usize]).collect();
				if mapped.iter().any(|&m| m == 0) {
					None
				} else {
					let mut nl = mapped;
					nl.sort_unstable();
					nl.dedup();
					leaders = nl;
					fv_vocab = vocab;
					Some(frecs)
				}
			}
		} else {
			None
		};
		// 统一记录域：fused 流或 self.code 的 Plain 映射（后者与前实现逐条同构）。
		let mut recs: Vec<super::fuse::FRec> = match &fused_stream {
			Some(fr) => fr.clone(),
			None => self
				.code
				.iter()
				.map(|i| super::fuse::FRec {
					kind: super::fuse::FKind::Base(i.op),
					a: i.a,
					b: i.b,
					c: i.c,
					d: i.d,
				})
				.collect(),
		};
		fn rec_unused(k: super::fuse::FKind) -> [bool; 4] {
			match k {
				super::fuse::FKind::Base(op) => isa::unused_operands(op),
				super::fuse::FKind::Fused(_) => [false, false, false, false],
			}
		}
		let nops: Vec<u16> = recs
			.iter()
			.enumerate()
			.filter(|(_, r)| r.kind == super::fuse::FKind::Base(Op::Nop))
			.map(|(p, _)| p as u16)
			.collect();
		let mut real_lanes: [Vec<u16>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
		let mut real_all = Vec::new();
		let mut real_total = 0usize;
		let mut real_high = 0usize;
		for r in &recs {
			let unused = rec_unused(r.kind);
			let vals = [r.a, r.b, r.c, r.d];
			for k in 0..4 {
				if !unused[k] {
					real_lanes[k].push(vals[k]);
					real_all.push(vals[k]);
					real_total += 1;
					if vals[k] >= 96 {
						real_high += 1;
					}
				}
			}
		}
		let mut noise_total = 0usize;
		let mut noise_high = 0usize;
		if self.scatter {
			let mut fill_rng = self.rng.fork(0x4649_4c4c_0000_0000 ^ fn_index as u64);
			for r in recs.iter_mut() {
				let unused = rec_unused(r.kind);
				let mut vals = [r.a, r.b, r.c, r.d];
				for k in 0..4 {
					if unused[k] {
						vals[k] = sample_noise_operand(&mut fill_rng, &real_lanes[k], &real_all);
						noise_total += 1;
						if vals[k] >= 96 {
							noise_high += 1;
						}
					}
				}
				r.a = vals[0];
				r.b = vals[1];
				r.c = vals[2];
				r.d = vals[3];
			}
		} else {
			for r in recs.iter_mut() {
				let m = rec_unused(r.kind);
				if m[0] { r.a = self.rng.int(0, 65535) as u16; }
				if m[1] { r.b = self.rng.int(0, 65535) as u16; }
				if m[2] { r.c = self.rng.int(0, 65535) as u16; }
				if m[3] { r.d = self.rng.int(0, 65535) as u16; }
			}
		}
		let chaff_mod = 65_521u64;
		let mut chaff = (((fn_index as u64 + 1) * self.program.chaff_fn as u64)
			% chaff_mod) as u64;
		// CCPD: the artifact carries no SA..SD streams for the runtime
		// qf fold to read, so the fold is defined as empty and the
		// template emits an empty QP list; `chaff_fold` then reduces to
		// the base value the runtime reproduces exactly. Legacy path
		// (ccpd off) folds Nop lanes as before, byte-for-byte.
		for (p, r) in recs.iter().enumerate() {
			if !self.ccpd && r.kind == super::fuse::FKind::Base(Op::Nop) {
				let logical = [r.a as u64, r.b as u64, r.c as u64, r.d as u64];
				let physical = [
					logical[perm[0] as usize],
					logical[perm[1] as usize],
					logical[perm[2] as usize],
					logical[perm[3] as usize],
				];
				let mix = physical[0] * self.program.chaff_coeffs[0] as u64
					+ physical[1] * self.program.chaff_coeffs[1] as u64
					+ physical[2] * self.program.chaff_coeffs[2] as u64
					+ physical[3] * self.program.chaff_coeffs[3] as u64
					+ (p as u64 + 1) * self.program.chaff_pos as u64;
				chaff = (chaff * self.program.chaff_mul as u64 + mix) % chaff_mod;
			}
		}
		let chaff_fold = (chaff + 1) as u16;
		let mut osum: u64 = 0;
		for r in &recs {
			osum = (osum + r.a as u64 + r.b as u64 + r.c as u64 + r.d as u64) % 4294967296;
			self.program.total_insns = self.program.total_insns.saturating_add(1);
			match r.kind {
				super::fuse::FKind::Base(op) => {
					let oi = isa::op_index(op);
					self.program.used_ops[oi] = true;
					self.program.op_hist[oi] = self.program.op_hist[oi].saturating_add(1);
				}
				super::fuse::FKind::Fused(fk) => {
					self.program.fused_insns = self.program.fused_insns.saturating_add(1);
					if let Some(&(o1, o2)) = fv_vocab.get(fk as usize) {
						if (o1 as usize) < isa::N_OPS {
							self.program.used_ops[o1 as usize] = true;
							self.program.op_hist[o1 as usize] =
								self.program.op_hist[o1 as usize].saturating_add(1);
						}
						if (o2 as usize) < isa::N_OPS {
							self.program.used_ops[o2 as usize] = true;
							self.program.op_hist[o2 as usize] =
								self.program.op_hist[o2 as usize].saturating_add(1);
						}
					}
				}
			}
		}
		for c in &self.consts {
			match c {
				Const::Nil => self.program.const_counts[0] += 1,
				Const::Bool(_) => self.program.const_counts[1] += 1,
				Const::Num(v) => {
					if v.is_finite() {
						let iv = *v as i64;
						if (*v == iv as f64)
							&& (-1_125_899_906_842_623..=1_125_899_906_842_623).contains(&iv)
						{
							self.program.const_counts[2] += 1;
						} else {
							self.program.const_counts[3] += 1;
							self.program.has_float_const = true;
						}
					} else {
						// Dedicated tags 5/6/7 in BSS + tm[7..9] in CDEC decode 1/0, -1/0, 0/0
						// directly without calling `tonumber`!
						self.program.const_counts[3] += 1;
					}
				}
				Const::Str(_) => self.program.const_counts[4] += 1,
			}
		}
		let mut osum2: u64 = 0;
		let ncode64 = recs.len() as u64;
		for (p, r) in recs.iter().enumerate() {
			let vals = [r.a as u64, r.b as u64, r.c as u64, r.d as u64];
			for st in 0..4usize {
				let k = perm[st] as usize;
				let pos = (st as u64) * ncode64 + (p as u64) + 1;
				osum2 = (osum2 + (pos % 4294967296) * vals[k]) % 4294967296;
			}
		}
		let final_limits = super::verify::VerifierLimits {
			nregs,
			max_register_value: if self.scatter { 4095 } else { nregs.saturating_sub(1) },
			nconsts: self.consts.len(),
			nupvalues: self.upsrc.len(),
			nprototypes: self.program.fns.len(),
			max_canary_id: if self.canary { Some(3) } else { None },
		};
		if let Err(e) = super::verify::verify_final_records(
			fn_index,
			&recs,
			&final_limits,
			&fv_vocab,
			&leaders,
		) {
			crate::limits::fatal(&e.to_string());
		}
		// CCPD (rebased plan, Phase 1/4): with the final record stream and
		// remapped leaders fixed, build the CFG + block variants and the
		// id8 directory. The salt is domain-separated from the blob keys,
		// exactly like the layout/chaff derivations above: toggling CCPD
		// never perturbs the shared rng stream, and with CCPD off this
		// block does not run at all.
		let ccpd_data: Option<super::ccpd::CcpdFunctionData> = if self.ccpd {
			let ccpd_salt = (self.program.blob_seed
				^ self.program.blob_step.rotate_left(7)
				^ 0x4343_5044)
				% 268_435_456;
			let fn_perm_code =
				super::ccpd::perm_code_of(&perm).expect("slot permutation is a bijection");
			Some(
				super::ccpd::build_function_ccpd(
					&recs,
					&leaders,
					&map,
					self.program.s_indices[fn_index] as usize,
					fn_index,
					ccpd_salt,
					fn_perm_code,
				)
				.unwrap_or_else(|e| {
					crate::limits::fatal(&format!("input cannot be CCPD-encoded: {e}"))
				}),
			)
		} else {
			None
		};
		if let Some(data) = &ccpd_data {
			// CCPD integrity anchor (replaces the legacy SA..SD lane
			// sums, which do not exist in this representation): the
			// runtime recomputes these over the payload bank bytes in
			// parse_fn, and any bank tamper perturbs iz1/iz2/bz exactly
			// like a legacy stream tamper does.
			let (ck, ck2) = super::ccpd::bank_checksums(&data.bank);
			osum = ck;
			osum2 = ck2;
			if crate::audit_gate::audit_var("OBSUCVIM_CCPD_AUDIT").is_some() {
				ccpd_audit_dump(&format!(
					"FN {} BLOCKS {} EDGES {} ENTRY {} SALT {}",
					fn_index,
					data.cfg.blocks.len(),
					data.cfg.edges.len(),
					data.entry_edge_id(),
					data.build_salt
				));
				for b in &data.cfg.blocks {
					let inc: Vec<String> = data
						.cfg
						.incoming(b.id)
						.iter()
						.map(|e| e.id.0.to_string())
						.collect();
					ccpd_audit_dump(&format!("FN {} INC {} {}", fn_index, b.id, inc.join(",")));
				}
				for e in &data.cfg.edges {
					let tgt = if e.target_block == super::ccpd::EXIT_BLOCK {
						"EXIT".to_string()
					} else {
						e.target_block.to_string()
					};
					ccpd_audit_dump(&format!(
						"FN {} EDGE {} {:?} {} {}",
						fn_index,
						e.id.0,
						e.kind,
						e.source_pc.map(|pc| pc as u32).unwrap_or(0),
						tgt
					));
				}
				for (site, child) in &data.closure_sites {
					ccpd_audit_dump(&format!("FN {fn_index} CSITE {site} {child}"));
				}
			}
		}
		let mut secs: Vec<(usize, Vec<u8>)> = Vec::new();
		let mut head = Vec::new();
		isa::encode_u16var(&mut head, nregs);
		isa::encode_u16var(&mut head, self.nparams);
		isa::encode_u16var(&mut head, self.vararg as u16);
		secs.push((0, head));
		let ckseed = self.rng.int(0, 65535) as u16;
		let mut cksec = Vec::new();
		isa::encode_u16var(&mut cksec, ckseed);
		secs.push((1, cksec));
		let mut ups = Vec::new();
		isa::encode_u16var(&mut ups, self.upsrc.len() as u16);
		for s in &self.upsrc {
			isa::encode_u16var(&mut ups, *s);
		}
		secs.push((2, ups));
		let mut csec: Vec<u8> = Vec::new();
		let mut lcg =
			isa::ConstLcg::new(ckseed as u32, self.program.ck_km, self.program.ck_kc);
		for c in &self.consts {
			c.encode(&mut csec, &mut lcg);
		}
		// Phase 18 P4 audit: the section length below is a `u16` varint; reject
		// instead of wrapping it and emitting an unloadable artifact.
		if csec.len() > crate::limits::MAX_CONSTS_SECTION_BYTES {
			crate::limits::fatal(&format!(
				"input exceeds compiler limit: constant section = {} bytes, max allowed = {} per function",
				csec.len(),
				crate::limits::MAX_CONSTS_SECTION_BYTES
			));
		}
		let mut consts_sec = Vec::new();
		isa::encode_u16var(&mut consts_sec, self.consts.len() as u16);
		isa::encode_u16var(&mut consts_sec, csec.len() as u16);
		consts_sec.extend_from_slice(&csec);
		secs.push((3, consts_sec));
		if let Some(s_tab) = &s_tab {
			let mut slots = Vec::new();
			isa::encode_u16var(&mut slots, s_tab.len() as u16);
			for s in s_tab {
				isa::encode_u16var(&mut slots, *s);
			}
			secs.push((4, slots));
		}
		let mut code = Vec::new();
		isa::encode_u16var(&mut code, recs.len() as u16);
		let s_idx = self.program.s_indices[fn_index] as usize;
		if let Some(data) = &ccpd_data {
			// CCPD code representation (plan 8.3): the section carries
			// the variant payload bank and nothing else; no canonical
			// SoA stream of this function remains in the artifact.
			isa::encode_varint(&mut code, data.bank.len() as u32);
			code.extend_from_slice(&data.bank);
		} else {
			super::fuse::encode_soa_fused(&recs, &map, &perm, s_idx, &mut code);
		}
		secs.push((5, code));
		if let Some(data) = &ccpd_data {
			secs.push((8, data.encode_section()));
		}
		// V17 P1-b：fv 段（每函数融合词汇→tail-less handler 码位对，+fvm 掩码后的 u16 流）。
		// super 开时恒发（含空词汇 len=0），与模板 NSECT 口径一致。
		if self.super_insn {
			let fvm = ((ckseed as u32 * self.program.fv_m1 as u32
				+ self.program.fv_mc as u32)
				% 65536) as u16;
			let mut fv = Vec::new();
			isa::encode_u16var(&mut fv, fv_vocab.len() as u16);
			// V19 R5-b：掩码按 1-based 条目下标逐条非线性演化（镜像 trampoline 文本）。
			for (j, &pair) in fv_vocab.iter().enumerate() {
				let m = super::fuse::fv_mask_nl(fvm, j as u32 + 1);
				let e = super::fuse::pack_fv_entry(pair, s_idx, m)
					// invariant: the vocabulary pair table is a static whitelist; the lookup key is a compile-time opcode name
					.expect("vocab pairs are whitelist-only by construction");
				isa::encode_u16var(&mut fv, e);
			}
			if crate::audit_gate::audit_enabled("OBSUCVIM_FUSE_DBG") {
				eprintln!(
					"[fuse] fn={} s_idx={} ckseed={} fvm={} vocab={:?}",
					fn_index, s_idx, ckseed, fvm, fv_vocab
				);
			}
			secs.push((7, fv));
			self.program.fv_sets[fn_index] = fv_vocab.clone();
		}
		self.rng.shuffle(&mut secs);
		let tags = self.program.proto_layouts[fn_index].as_tags();
		let mut blob: Vec<u8> = Vec::with_capacity(self.code.len() * 6 + 96);
		for (id, body) in &secs {
			assert_ne!(*id, 6, "unread padding sections must not be emitted");
			blob.push(tags[*id]);
			blob.extend_from_slice(body);
		}
		let km = self.program.blob_km as u64;
		let kc = self.program.blob_kc as u64;
		let base = (self.program.blob_seed as u64
			+ fn_index as u64 * self.program.blob_step as u64)
			% 268_435_456;
		// V19 R5-a：stage-1 blob —— 非线性折叠（fold_key_nl）+ 明文级联
		// （前一明文字节并入 LCG 推进；跨 ksplit 边界连续携带，双镜面同构）。
		let ksplit = blob.len().min(64);
		let mut state = base;
		let mut prev: u64 = 0;
		for byte in blob.iter_mut().take(ksplit) {
			state = (km * state + kc + prev) % 268_435_456;
			let pt = *byte as u64;
			*byte = byte.wrapping_add(isa::fold_key_nl(state));
			prev = pt;
		}
		let mut hf: u64 = 0;
		for &byte in blob.iter().take(ksplit) {
			hf = (hf * 31 + byte as u64) % 268_435_456;
		}
		let mut state = (base + hf) % 268_435_456;
		for byte in blob.iter_mut().skip(ksplit) {
			state = (km * state + kc + prev) % 268_435_456;
			let pt = *byte as u64;
			*byte = byte.wrapping_add(isa::fold_key_nl(state));
			prev = pt;
		}
		if self.scatter {
			noise_audit(&format!(
				"fn={} base={} final={} nop={} active={} dead={} reg_hi={} real_n={} real_hi={} noise_n={} noise_hi={} nil={} decoy={} chaff={}",
				fn_index,
				base_code_len,
				recs.len(),
				nop_count,
				active_count,
				dead_regs,
				scatter_stats.high_regs,
				real_total,
				real_high,
				noise_total,
				noise_high,
				scatter_stats.nil_holes,
				scatter_stats.decoy_holes,
				chaff_fold,
			));
		}
		(blob, nops, osum, osum2, chaff_fold, leaders)
	}
}


struct FnAnalysis {
	direct_up: Vec<SymId>,
	nested_up: Vec<SymId>,
}

fn expr_refs(e: &Expr, out: &mut HashSet<SymId>) {
	match e {
		Expr::Ident { sym: Some(s), .. } => {
			out.insert(*s);
		}
		Expr::Dot { obj, .. } => expr_refs(obj, out),
		Expr::Index { obj, idx } => {
			expr_refs(obj, out);
			expr_refs(idx, out);
		}
		Expr::Call { func, args } => {
			expr_refs(func, out);
			for a in args {
				expr_refs(a, out);
			}
		}
		Expr::Method { obj, args, .. } => {
			expr_refs(obj, out);
			for a in args {
				expr_refs(a, out);
			}
		}
		Expr::Paren { e } => expr_refs(e, out),
		Expr::Bin { l, r, .. } => {
			expr_refs(l, out);
			expr_refs(r, out);
		}
		Expr::Un { e, .. } => expr_refs(e, out),
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				expr_refs(cond, out);
				expr_refs(value, out);
			}
			expr_refs(elseb, out);
		}
		Expr::Table { fields } => {
			for f in fields {
				match f {
					TableField::Array(e) => expr_refs(e, out),
					TableField::Key { key, value } => {
						expr_refs(key, out);
						expr_refs(value, out);
					}
				}
			}
		}
		_ => {}
	}
}

fn collect_bodies<'a>(block: &'a Block, out: &mut Vec<(&'a Block, &'a [SymId])>) {
	for s in &block.stmts {
		match s {
			Stmt::LocalFunc { func, .. } => out.push((&func.body, &func.param_syms)),
			Stmt::FuncDecl { func, .. } => out.push((&func.body, &func.param_syms)),
			_ => {}
		}
		match s {
			Stmt::Local { values, .. } => {
				for v in values {
					if let Some(e) = v {
						expr_bodies(e, out);
					}
				}
			}
			Stmt::Assign { values, .. } => {
				for v in values {
					expr_bodies(v, out);
				}
			}
			Stmt::ExprStmt(e) => expr_bodies(e, out),
			Stmt::Return(es) => {
				for e in es {
					expr_bodies(e, out);
				}
			}
			Stmt::If { cond, thenb, elsifs, elseb } => {
				expr_bodies(cond, out);
				collect_bodies(thenb, out);
				for (c, b) in elsifs {
					expr_bodies(c, out);
					collect_bodies(b, out);
				}
				if let Some(b) = elseb {
					collect_bodies(b, out);
				}
			}
			Stmt::While { cond, body } => {
				expr_bodies(cond, out);
				collect_bodies(body, out);
			}
			Stmt::Repeat { body, cond } => {
				collect_bodies(body, out);
				expr_bodies(cond, out);
			}
			Stmt::ForNum { start, limit, step, body, .. } => {
				expr_bodies(start, out);
				expr_bodies(limit, out);
				if let Some(st) = step {
					expr_bodies(st, out);
				}
				collect_bodies(body, out);
			}
			Stmt::ForGen { iters, body, .. } => {
				for i in iters {
					expr_bodies(i, out);
				}
				collect_bodies(body, out);
			}
			Stmt::Do(b) => collect_bodies(b, out),
			_ => {}
		}
	}
}

// Collect only closure bodies created in this lexical function. Expressions and
// control-flow blocks are traversed, but a function literal is a scope boundary.
fn collect_direct_bodies<'a>(block: &'a Block, out: &mut Vec<(&'a Block, &'a [SymId])>) {
	for s in &block.stmts {
		match s {
			Stmt::LocalFunc { func, .. } | Stmt::FuncDecl { func, .. } => {
				out.push((&func.body, &func.param_syms));
			}
			_ => {}
		}
		match s {
			Stmt::Local { values, .. } => {
				for e in values.iter().flatten() {
					expr_direct_bodies(e, out);
				}
			}
			Stmt::Assign { targets, values } => {
				for e in targets.iter().chain(values) {
					expr_direct_bodies(e, out);
				}
			}
			Stmt::ExprStmt(e) => expr_direct_bodies(e, out),
			Stmt::Return(es) => {
				for e in es {
					expr_direct_bodies(e, out);
				}
			}
			Stmt::If { cond, thenb, elsifs, elseb } => {
				expr_direct_bodies(cond, out);
				collect_direct_bodies(thenb, out);
				for (c, b) in elsifs {
					expr_direct_bodies(c, out);
					collect_direct_bodies(b, out);
				}
				if let Some(b) = elseb {
					collect_direct_bodies(b, out);
				}
			}
			Stmt::While { cond, body } => {
				expr_direct_bodies(cond, out);
				collect_direct_bodies(body, out);
			}
			Stmt::Repeat { body, cond } => {
				collect_direct_bodies(body, out);
				expr_direct_bodies(cond, out);
			}
			Stmt::ForNum { start, limit, step, body, .. } => {
				expr_direct_bodies(start, out);
				expr_direct_bodies(limit, out);
				if let Some(e) = step {
					expr_direct_bodies(e, out);
				}
				collect_direct_bodies(body, out);
			}
			Stmt::ForGen { iters, body, .. } => {
				for e in iters {
					expr_direct_bodies(e, out);
				}
				collect_direct_bodies(body, out);
			}
			Stmt::Do(b) => collect_direct_bodies(b, out),
			_ => {}
		}
	}
}

fn expr_direct_bodies<'a>(e: &'a Expr, out: &mut Vec<(&'a Block, &'a [SymId])>) {
	match e {
		Expr::Function { body, param_syms, .. } => out.push((body, param_syms)),
		Expr::Paren { e } | Expr::Un { e, .. } => expr_direct_bodies(e, out),
		Expr::Dot { obj, .. } => expr_direct_bodies(obj, out),
		Expr::Index { obj, idx } => {
			expr_direct_bodies(obj, out);
			expr_direct_bodies(idx, out);
		}
		Expr::Call { func, args } => {
			expr_direct_bodies(func, out);
			for a in args {
				expr_direct_bodies(a, out);
			}
		}
		Expr::Method { obj, args, .. } => {
			expr_direct_bodies(obj, out);
			for a in args {
				expr_direct_bodies(a, out);
			}
		}
		Expr::Bin { l, r, .. } => {
			expr_direct_bodies(l, out);
			expr_direct_bodies(r, out);
		}
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				expr_direct_bodies(cond, out);
				expr_direct_bodies(value, out);
			}
			expr_direct_bodies(elseb, out);
		}
		Expr::Table { fields } => {
			for f in fields {
				match f {
					TableField::Array(e) => expr_direct_bodies(e, out),
					TableField::Key { key, value } => {
						expr_direct_bodies(key, out);
						expr_direct_bodies(value, out);
					}
				}
			}
		}
		_ => {}
	}
}

fn expr_bodies<'a>(e: &'a Expr, out: &mut Vec<(&'a Block, &'a [SymId])>) {
	match e {
		Expr::Function { body, param_syms, .. } => {
			out.push((body, param_syms));
			collect_bodies(body, out);
		}
		Expr::Paren { e } => expr_bodies(e, out),
		Expr::Dot { obj, .. } => expr_bodies(obj, out),
		Expr::Index { obj, idx } => {
			expr_bodies(obj, out);
			expr_bodies(idx, out);
		}
		Expr::Call { func, args } => {
			expr_bodies(func, out);
			for a in args {
				expr_bodies(a, out);
			}
		}
		Expr::Method { obj, args, .. } => {
			expr_bodies(obj, out);
			for a in args {
				expr_bodies(a, out);
			}
		}
		Expr::Bin { l, r, .. } => {
			expr_bodies(l, out);
			expr_bodies(r, out);
		}
		Expr::Un { e, .. } => expr_bodies(e, out),
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				expr_bodies(cond, out);
				expr_bodies(value, out);
			}
			expr_bodies(elseb, out);
		}
		Expr::Table { fields } => {
			for f in fields {
				match f {
					TableField::Array(e) => expr_bodies(e, out),
					TableField::Key { key, value } => {
						expr_bodies(key, out);
						expr_bodies(value, out);
					}
				}
			}
		}
		_ => {}
	}
}

fn collect_declared_all(block: &Block, out: &mut Vec<SymId>) {
	for s in &block.stmts {
		match s {
			Stmt::Local { syms, values, .. } => {
				out.extend(syms.iter().copied());
				for v in values {
					if let Some(e) = v {
						collect_declared_expr(e, out);
					}
				}
			}
			Stmt::LocalFunc { sym, .. } => out.push(*sym),
			Stmt::FuncDecl { .. } => {}
			Stmt::Assign { values, .. } => {
				for v in values {
					collect_declared_expr(v, out);
				}
			}
			Stmt::ExprStmt(e) => collect_declared_expr(e, out),
			Stmt::If { thenb, elsifs, elseb, .. } => {
				collect_declared_all(thenb, out);
				for (_, b) in elsifs {
					collect_declared_all(b, out);
				}
				if let Some(b) = elseb {
					collect_declared_all(b, out);
				}
			}
			Stmt::While { body, .. } => collect_declared_all(body, out),
			Stmt::Repeat { body, .. } => collect_declared_all(body, out),
			Stmt::Do(b) => collect_declared_all(b, out),
			Stmt::ForNum { var_sym, body, .. } => {
				out.push(*var_sym);
				collect_declared_all(body, out);
			}
			Stmt::ForGen { syms, body, .. } => {
				out.extend(syms.iter().copied());
				collect_declared_all(body, out);
			}
			_ => {}
		}
	}
}

// Expressions do not declare locals in their enclosing function. A function
// literal is a distinct scope, so its locals must never enter this set.
fn collect_declared_expr(_e: &Expr, _out: &mut Vec<SymId>) {}

fn declared_set_of(block: &Block) -> HashSet<SymId> {
	let mut v = Vec::new();
	collect_declared_all(block, &mut v);
	v.into_iter().collect()
}

fn loop_capture_set(
	body: &Block,
	cond: Option<&Expr>,
	declared: &HashSet<SymId>,
) -> HashSet<SymId> {
	let mut bodies: Vec<(&Block, &[SymId])> = Vec::new();
	collect_bodies(body, &mut bodies);
	if let Some(c) = cond {
		expr_bodies(c, &mut bodies);
	}
	let mut cap = HashSet::new();
	for (b, p) in &bodies {
		let a = analyze(b, p);
		for &s in &a.direct_up {
			if declared.contains(&s) {
				cap.insert(s);
			}
		}
		for &s in &a.nested_up {
			if declared.contains(&s) {
				cap.insert(s);
			}
		}
	}
	cap
}

fn collect_block_refs_own(block: &Block, out: &mut HashSet<SymId>) {
	for s in &block.stmts {
		match s {
			Stmt::LocalFunc { .. } | Stmt::FuncDecl { .. } => {
				if let Stmt::FuncDecl { obj, .. } = s {
					if let Some(o) = obj {
						expr_refs(o, out);
					}
				}
			}
			Stmt::Local { values, .. } => {
				for v in values {
					if let Some(e) = v {
						expr_refs(e, out);
					}
				}
			}
			Stmt::Assign { targets, values } => {
			for t in targets {
				expr_refs(t, out);
			}
			for v in values {
				expr_refs(v, out);
			}
		}
		Stmt::ExprStmt(e) => expr_refs(e, out),
		Stmt::If { cond, thenb, elsifs, elseb } => {
			expr_refs(cond, out);
			refs_in_sub(thenb, out);
			for (c, b) in elsifs {
				expr_refs(c, out);
				refs_in_sub(b, out);
			}
			if let Some(b) = elseb {
				refs_in_sub(b, out);
			}
		}
		Stmt::While { cond, body } => {
			expr_refs(cond, out);
			refs_in_sub(body, out);
		}
		Stmt::Repeat { body, cond } => {
			refs_in_sub(body, out);
			expr_refs(cond, out);
		}
		Stmt::ForNum {
			start, limit, step, body, ..
		} => {
			expr_refs(start, out);
			expr_refs(limit, out);
			if let Some(st) = step {
				expr_refs(st, out);
			}
			refs_in_sub(body, out);
		}
		Stmt::ForGen { iters, body, .. } => {
			for i in iters {
				expr_refs(i, out);
			}
			refs_in_sub(body, out);
		}
		Stmt::Do(b) => refs_in_sub(b, out),
		Stmt::Return(es) => {
			for e in es {
				expr_refs(e, out);
			}
		}
		_ => {}
		}
	}
}

fn refs_in_sub(block: &Block, out: &mut HashSet<SymId>) {
	collect_block_refs_own(block, out);
}

fn analyze(block: &Block, params: &[SymId]) -> FnAnalysis {
	let mut declared = Vec::new();
	declared.extend(params.iter().copied());
	collect_declared_all(block, &mut declared);
	let declared_set: HashSet<SymId> = declared.iter().copied().collect();

	let mut direct = HashSet::new();
	collect_block_refs_own(block, &mut direct);
	direct.retain(|s| !declared_set.contains(s));

	let mut bodies: Vec<(&Block, &[SymId])> = Vec::new();
	collect_direct_bodies(block, &mut bodies);
	let mut nested = HashSet::new();
	for (child, child_params) in &bodies {
		// Each child contributes both its own external references and the
		// references it must forward to its descendants. Filter symbols owned
		// here: they are captured from this frame, not from this function's
		// enclosing frame. Recursing over only direct children prevents sibling
		// scopes from leaking locals into this closure's descriptor set.
		let child_analysis = analyze(child, child_params);
		for s in child_analysis.direct_up.iter().chain(&child_analysis.nested_up) {
			if !declared_set.contains(s) {
				nested.insert(*s);
			}
		}
	}

	let mut direct_up: Vec<SymId> = direct.iter().copied().collect();
	direct_up.sort_unstable();
	let mut nested_up: Vec<SymId> = nested.iter().copied().collect();
	nested_up.sort_unstable();
	nested_up.dedup();
	FnAnalysis {
		direct_up,
		nested_up,
	}
}


/// Phase 18 P1-1/P1-2: annotate every `LoadStr` operand with its cache policy.
///
///   bit 15 (0x8000) = cached: the constant has more than one static reference,
///                    so the handler decodes once per frame and reuses the slot.
///   bit 14 (0x4000) = clear-after-use: this is the last static reference AND
///                    every reference lives in one loop-free function body.
///
/// Single-reference constants keep a plain index: the handler decodes inline
/// and never touches the cache table. Returns `(cached, cleared)` counts.
pub(crate) fn loadstr_flags(
	code: &mut [Instr],
	consts: &[Const],
	clearable: &HashSet<Vec<u8>>,
) -> (usize, usize) {
	let mut counts: HashMap<u16, usize> = HashMap::new();
	let mut last: HashMap<u16, usize> = HashMap::new();
	for (i, ins) in code.iter().enumerate() {
		if ins.op == Op::LoadStr {
			*counts.entry(ins.b).or_insert(0) += 1;
			last.insert(ins.b, i);
		}
	}
	if counts.is_empty() {
		return (0, 0);
	}
	let mut cached = 0usize;
	let mut cleared = 0usize;
	for (i, ins) in code.iter_mut().enumerate() {
		if ins.op != Op::LoadStr {
			continue;
		}
		let k = ins.b;
		// no headroom for the policy bits: leave the index untouched
		if k >= 0x4000 {
			continue;
		}
		let is_cached = counts.get(&k).copied().unwrap_or(0) > 1;
		let is_last = last.get(&k) == Some(&i);
		let is_clearable = match consts.get(k as usize) {
			Some(Const::Str(bytes)) => clearable.contains(bytes.as_slice()),
			_ => false,
		};
		let clear = is_cached && is_last && is_clearable;
		if is_cached {
			cached += 1;
		}
		if clear {
			cleared += 1;
		}
		ins.b = k
			| if is_cached { 0x8000 } else { 0 }
			| if clear { 0x4000 } else { 0 };
	}
	(cached, cleared)
}

fn prune_dead_short_string_constants(
	code: &mut [Instr],
	consts: &mut Vec<Const>,
	candidates: &HashSet<u16>,
) -> usize {
	if candidates.is_empty() {
		return 0;
	}
	let mut live = vec![false; consts.len()];
	for ins in code.iter() {
		let roles = Ctx::op_roles(ins.op);
		let ops = ins.operands();
		for i in 0..4 {
			if roles[i] == OperandRole::ConstIdx {
				let idx = ops[i] as usize;
				if idx < live.len() {
					live[idx] = true;
				}
			}
		}
	}
	let mut remove = vec![false; consts.len()];
	for &candidate in candidates {
		let idx = candidate as usize;
		if idx < consts.len()
			&& !live[idx]
			&& matches!(consts[idx], Const::Str(ref bytes) if bytes.len() <= 5)
		{
			remove[idx] = true;
		}
	}
	let removed = remove.iter().filter(|&&drop| drop).count();
	if removed == 0 {
		return 0;
	}
	let mut remap = vec![0u16; consts.len()];
	let mut compact = Vec::with_capacity(consts.len() - removed);
	for (old, value) in consts.drain(..).enumerate() {
		if !remove[old] {
			remap[old] = compact.len() as u16;
			compact.push(value);
		}
	}
	for ins in code.iter_mut() {
		let roles = Ctx::op_roles(ins.op);
		let mut ops = ins.operands();
		for i in 0..4 {
			if roles[i] == OperandRole::ConstIdx {
				let old = ops[i] as usize;
				debug_assert!(old < remap.len() && !remove[old]);
				ops[i] = remap[old];
			}
		}
		ins.a = ops[0];
		ins.b = ops[1];
		ins.c = ops[2];
		ins.d = ops[3];
	}
	*consts = compact;
	removed
}

fn compile_chunk(
	block: &Block,
	table: &SymTable,
	rng: &mut Rng,
	lua51: bool,
	scatter: bool,
	super_insn: bool,
	canary: bool,
	proto_layout: bool,
	ccpd: bool,
) -> VmProgram {
	let str_clearable = crate::vmgen::strseal::sealed_clearable(block);
	let total = count_fns_count(block);
	let has_loadstr = crate::vmgen::strseal::contains_sealed_literals(block);
	let mut perm: Vec<u8> = vec![0, 1, 2, 3];
	rng.shuffle(&mut perm);
	// invariant: perm is built as a fixed 4-element permutation in compile()
	let slot_perm: [u8; 4] = perm.try_into().unwrap();
	let ck_km = (rng.int(1_048_577, 33_000_001) | 1) as u32;
	let ck_kc = (rng.int(1_048_576, 268_000_000) | 1) as u32;
	let mk1 = rng.int(1, 65535) as u16;
	let mk2 = rng.int(0, 65535) as u16;
	let blob_km = (rng.int(1_048_577, 33_000_001) | 1) as u32;
	let blob_kc = (rng.int(1_048_576, 268_000_000) | 1) as u32;
	let blob_seed = rng.int(1_048_576, 268_435_455) as u32;
	let blob_step = rng.int(1_048_576, 268_435_455) as u32;
	// Phase B: per-prototype field layouts (`total` child functions plus the
	// main chunk at index `total`). Disabled => one shared layout (legacy).
	// Layout randomness comes from a domain-separated stream derived from
	// the blob parameters (same pattern as chaff_rng below): toggling or
	// resizing layouts must not perturb the main rng stream, so every other
	// subsystem keeps its baseline draw sequence for a given seed.
	let layout_seed = (blob_seed as u64)
		^ (blob_step as u64).rotate_left(29)
		^ (ck_kc as u64).rotate_left(11)
		^ 0x4C41_5954_4F55_5431; // "LAYOUT1" domain separator
	let mut layout_rng = Rng::new(layout_seed);
	let proto_layouts =
		super::proto_layout::program_layouts(&mut layout_rng, total + 1, proto_layout);
	// V17 P1-b：fv 掩码常量——门控于 super_insn，off 态不消耗 rng（字节恒等前提）。
	let (fv_m1, fv_mc) = if super_insn {
		((rng.int(1, 65535) | 1) as u16, rng.int(1, 65535) as u16)
	} else {
		(0, 0)
	};
	let chaff_seed = (blob_seed as u64)
		^ (blob_step as u64).rotate_left(17)
		^ (ck_km as u64).rotate_left(33)
		^ blob_kc as u64;
	let mut chaff_rng = Rng::new(chaff_seed ^ 0x4844_3441);
	let chaff_mul = chaff_rng.int(1, 65_520) as u16;
	let chaff_coeffs = [
		chaff_rng.int(1, 65_520) as u16,
		chaff_rng.int(1, 65_520) as u16,
		chaff_rng.int(1, 65_520) as u16,
		chaff_rng.int(1, 65_520) as u16,
	];
	let chaff_pos = chaff_rng.int(1, 65_520) as u16;
	let chaff_fn = chaff_rng.int(1, 65_520) as u16;
	let lane_steps = [1u8, 5, 7, 11, 13, 17, 19, 23];
	let lane_base = (chaff_seed % 24) as u8;
	let lane_step = lane_steps[((chaff_seed / 24) as usize) % lane_steps.len()];
	let slot_perms: Vec<[u8; 4]> = (0..=total)
		.map(|fi| {
			let code = (lane_base as usize + (fi + 1) * lane_step as usize) % 24;
			operand_perm_from_code(code as u8)
		})
		.collect();
	let mut program = VmProgram {
		has_loadstr,
		opmap: OpMap::new_with_count(
			rng,
			if has_loadstr { isa::N_OPS } else { isa::N_OPS - 1 },
		),
		slot_perm,
		slot_perms,
		lane_base,
		lane_step,
		carrier: Carrier::new(rng),
		fns: vec![Vec::new(); total],
		nop_sites: vec![Vec::new(); total],
		operand_sums: vec![0; total],
		operand_sums2: vec![0; total],
		chaff_folds: vec![0; total],
		chaff_mul,
		chaff_coeffs,
		chaff_pos,
		chaff_fn,
		ck_km,
		ck_kc,
		mk1,
		mk2,
		blob_km,
		blob_kc,
		blob_seed,
		blob_step,
		proto_layouts,
		consts_mask_safe: true,
		used_ops: [false; isa::N_OPS],
		op_hist: [0; isa::N_OPS],
		const_counts: [0; 5],
		has_float_const: false,
		total_insns: 0,
		fused_insns: 0,
		block_starts: vec![Vec::new(); total + 1],
		s_indices: vec![0; total + 1],
		fv_sets: vec![Vec::new(); total + 1],
		fv_m1,
		fv_mc,
	};
	{
		let main_index = program.fns.len();
        program.s_indices[main_index] = if scatter { rng.int(0, 3) as u8 } else { 0 };
        let mut ctx = Ctx::new_main(
        &mut program,
        rng,
        table,
        lua51,
        scatter,
        super_insn,
        &str_clearable,
        canary,
        ccpd,
    );
		ctx.compile_block(block);
		let nregs = ctx.next_reg;
		let (bytes, nops, osum, osum2, chaff, blocks) = ctx.finish(nregs, main_index);
		program.block_starts[main_index] = blocks;
		program.fns.push(bytes);
		program.nop_sites.push(nops);
		program.operand_sums.push(osum);
		program.operand_sums2.push(osum2);
		program.chaff_folds.push(chaff);
	}
	if let Some(path) = crate::audit_gate::audit_var("OBSUCVIM_PROFILE_AUDIT") {
		let mut op_entries = Vec::new();
		for (i, &cnt) in program.op_hist.iter().enumerate() {
			if cnt > 0 {
				op_entries.push(format!("\"{}\":{}", isa::OP_NAMES[i], cnt));
			}
		}
		let json = format!(
			"{{\"fn_count\":{},\"total_insns\":{},\"fused_insns\":{},\"consts\":{{\"nil\":{},\"bool\":{},\"int\":{},\"float\":{},\"str\":{},\"total\":{}}},\"consts_mask_safe\":{},\"op_hist\":{{{}}}}}\n",
			program.fns.len(),
			program.total_insns,
			program.fused_insns,
			program.const_counts[0],
			program.const_counts[1],
			program.const_counts[2],
			program.const_counts[3],
			program.const_counts[4],
			program.const_counts.iter().sum::<u32>(),
			program.consts_mask_safe,
			op_entries.join(",")
		);
		let _ = std::fs::write(path, json);
	}
	program
}

fn count_fns(block: &Block, n: &mut usize) {
	for s in &block.stmts {
		count_stmt(s, n);
	}
}

fn count_fns_count(block: &Block) -> usize {
	let mut n = 0;
	count_fns(block, &mut n);
	n
}

fn count_stmt(s: &Stmt, n: &mut usize) {
	match s {
		Stmt::Local { values, .. } => {
			for v in values {
				if let Some(e) = v {
					count_expr(e, n);
				}
			}
		}
		Stmt::LocalFunc { func, .. } => {
			*n += 1;
			count_fns(&func.body, n);
		}
		Stmt::FuncDecl { func, obj, .. } => {
			if let Some(o) = obj {
				count_expr(o, n);
			}
			*n += 1;
			count_fns(&func.body, n);
		}
		Stmt::Assign { targets, values } => {
			for t in targets {
				count_expr(t, n);
			}
			for v in values {
				count_expr(v, n);
			}
		}
		Stmt::ExprStmt(e) => {
			count_expr(e, n);
		}
		Stmt::Return(es) => {
			for e in es {
				count_expr(e, n);
			}
		}
		Stmt::If { cond, thenb, elsifs, elseb } => {
			count_expr(cond, n);
			count_fns(thenb, n);
			for (c, b) in elsifs {
				count_expr(c, n);
				count_fns(b, n);
			}
			if let Some(b) = elseb {
				count_fns(b, n);
			}
		}
		Stmt::While { cond, body } => {
			count_expr(cond, n);
			count_fns(body, n);
		}
		Stmt::Repeat { body, cond } => {
			count_fns(body, n);
			count_expr(cond, n);
		}
		Stmt::ForNum { start, limit, step, body, .. } => {
			count_expr(start, n);
			count_expr(limit, n);
			if let Some(st) = step {
				count_expr(st, n);
			}
			count_fns(body, n);
		}
		Stmt::ForGen { iters, body, .. } => {
			for i in iters {
				count_expr(i, n);
			}
			count_fns(body, n);
		}
		Stmt::Do(b) => {
			count_fns(b, n);
		}
		_ => {}
	}
}

fn count_expr(e: &Expr, n: &mut usize) {
	match e {
		Expr::Function { body, .. } => {
			*n += 1;
			count_fns(body, n);
		}
		Expr::Paren { e } => count_expr(e, n),
		Expr::Dot { obj, .. } => count_expr(obj, n),
		Expr::Index { obj, idx } => {
			count_expr(obj, n);
			count_expr(idx, n);
		}
		Expr::Call { func, args } => {
			count_expr(func, n);
			for a in args {
				count_expr(a, n);
			}
		}
		Expr::Method { obj, args, .. } => {
			count_expr(obj, n);
			for a in args {
				count_expr(a, n);
			}
		}
		Expr::Bin { l, r, .. } => {
			count_expr(l, n);
			count_expr(r, n);
		}
		Expr::Un { e, .. } => count_expr(e, n),
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				count_expr(cond, n);
				count_expr(value, n);
			}
			count_expr(elseb, n);
		}
		Expr::Table { fields } => {
			for f in fields {
				match f {
					TableField::Array(e) => count_expr(e, n),
					TableField::Key { key, value } => {
						count_expr(key, n);
						count_expr(value, n);
					}
				}
			}
		}
		_ => {}
	}
}

impl<'a> Ctx<'a> {
	fn compile_block(&mut self, block: &Block) {
		self.push_scope();
		for s in block.stmts.iter() {
			self.compile_stmt(s);
		}
		self.pop_scope();
	}

	fn compile_stmt(&mut self, s: &Stmt) {
		match s {
			Stmt::Local { names, syms, values } => {
				let _ = names;
				self.compile_local(syms, values);
			}
			Stmt::LocalFunc { name, sym, func } => {
				let _ = name;
				let r = self.tmp();
				self.declare(*sym, CellKind::Plain(r));
				self.compile_function(r, &func.body, &func.params, &func.param_syms, false);
			}
			Stmt::FuncDecl { obj, name, func, .. } => {
				if let Some(o) = obj {
					let t = self.tmp();
					self.compile_expr(o, t);
					let r = self.tmp();
					self.compile_function(
						r,
						&func.body,
						&func.params,
						&func.param_syms,
						func.has_self,
					);
					let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
					let kreg = self.tmp();
					self.emit(Instr::ab(Op::LoadK, kreg, k));
					self.emit(Instr::abc(Op::SetTab, t, kreg, r));
				} else {
					let r = self.tmp();
					self.compile_function(
						r,
						&func.body,
						&func.params,
						&func.param_syms,
						false,
					);
					let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
					self.emit(Instr::ab(Op::SetGlobal, r, k));
				}
			}
			_ => self.compile_stmt_rest(s),
		}
	}

	fn compile_local(&mut self, syms: &[SymId], values: &[Option<Expr>]) {
		let n = syms.len();
		let nv = values.len();
		let last_is_call = nv > 0
			&& values
				.last()
				.and_then(|v| v.as_ref())
				.map(is_call)
				.unwrap_or(false);
		let last_is_vararg =
			matches!(values.last(), Some(Some(e)) if matches!(e, Expr::Vararg));
		let npre = if nv > 0 && (last_is_call || last_is_vararg) {
			(nv - 1).min(n)
		} else {
			nv
		};
		let mut plain: Vec<u16> = Vec::with_capacity(n);
		let mut valreg: Vec<u16> = Vec::with_capacity(n);
		for _ in 0..n {
			plain.push(self.tmp());
		}
		let nlead = npre.min(n);
		for i in 0..nlead {
			let r = plain[i];
			match values.get(i) {
				Some(Some(e)) => {
					self.compile_expr(e, r)
				}
				_ => {
					let _ = self.emit(Instr::ab(Op::LoadNil, r, 1));
				}
			}
			valreg.push(r);
		}
		for i in nlead..nv.saturating_sub(1) {
			let r = self.tmp();
			match values.get(i) {
				Some(Some(e)) => {
					self.compile_expr(e, r)
				}
				_ => {
					let _ = self.emit(Instr::ab(Op::LoadNil, r, 1));
				}
			}
		}
		if nv > 0 && (last_is_call || last_is_vararg) && npre < n {
			let nres = n - npre;
			if last_is_call {
				// invariant: guarded by `if last_is_call` three lines above, which is derived from values[nv-1] being Some
				let call = values[nv - 1].as_ref().unwrap();
				let (nargs, has_vararg) = call_arg_info(call);
				let freg = self.reserve(1 + nargs.max(1));
				self.compile_call_into(call, freg, nres as u16, has_vararg);
				for i in 0..nres {
					let r = plain[npre + i];
					self.emit(Instr::ab(Op::Move, r, freg + 1 + i as u16));
					valreg.push(r);
				}
			} else {
				let vt = self.tmp();
				self.emit(Instr::ab(Op::VarArgTab, vt, 0));
				for i in 0..nres {
					let kreg = self.tmp();
					let k = self.kidx(Const::Num((i + 1) as f64));
					self.emit(Instr::ab(Op::LoadK, kreg, k));
					let r = plain[npre + i];
					self.emit(Instr::abc(Op::GetTab, r, vt, kreg));
					valreg.push(r);
				}
			}
		} else if nv > 0 && npre < nv {
			let i = nv - 1;
			let r = self.tmp();
			match values.get(i) {
				Some(Some(e)) => {
					self.compile_expr(e, r)
				}
				_ => {
					let _ = self.emit(Instr::ab(Op::LoadNil, r, 1));
				}
			}
		}
		while valreg.len() < n {
			let r = self.tmp();
			let knil = self.kidx(Const::Nil);
			self.emit(Instr::ab(Op::LoadK, r, knil));
			valreg.push(r);
		}
		for (i, s) in syms.iter().enumerate() {
			let kind = self.finalize_local(*s, plain[i], valreg[i]);
			self.declare(*s, kind);
		}
	}

	fn cell_from_value(&mut self, valreg: u16) -> CellKind {
		let sr = self.tmp();
		let ct = self.tmp();
		let k1 = self.kidx(Const::Num(1.0));
		let kreg = self.tmp();
		self.emit(Instr::ab(Op::LoadK, kreg, k1));
		self.emit(Instr::ab(Op::NewTab, ct, 0));
		self.emit(Instr::abc(Op::SetTab, ct, kreg, valreg));
		self.emit(Instr::ab(Op::Move, sr, ct));
		CellKind::Slot(sr)
	}

	fn finalize_local(&mut self, s: SymId, plain_reg: u16, valreg: u16) -> CellKind {
		if self
			.cur_loop_captures()
			.map(|c| c.contains(&s))
			.unwrap_or(false)
		{
			return self.cell_from_value(valreg);
		}
		if valreg != plain_reg {
			self.emit(Instr::ab(Op::Move, plain_reg, valreg));
		}
		CellKind::Plain(plain_reg)
	}

	fn compile_stmt_rest(&mut self, s: &Stmt) {
		match s {
			Stmt::Assign { targets, values } => {
				let mut tinfos: Vec<Target> = Vec::new();
				for t in targets {
					tinfos.push(self.eval_target(t));
				}
				let n = targets.len();
				let nv = values.len();
				let last_is_call = nv > 0 && values.last().map(is_call).unwrap_or(false);
				let last_is_vararg =
					nv > 0 && matches!(values.last(), Some(&Expr::Vararg));
				let npre = if nv > 0 && (last_is_call || last_is_vararg) {
					(nv - 1).min(n)
				} else {
					nv
				};
				let nlead = npre.min(n);
				let mut lead_regs: Vec<u16> = Vec::new();
				for i in 0..nlead {
					let r = self.tmp();
					self.compile_expr(&values[i], r);
					lead_regs.push(r);
				}
				for i in nlead..nv.saturating_sub(1) {
					let r = self.tmp();
					self.compile_expr(&values[i], r);
				}
				let mut exp_base: Option<u16> = None;
				let mut nres: usize = 0;
				if nv > 0 && (last_is_call || last_is_vararg) && npre < n {
					nres = (n - npre) as usize;
					if last_is_call {
						let call = values[nv - 1].clone();
						let (nargs, has_vararg) = call_arg_info(&call);
						let freg = self.reserve(1 + nargs.max(1));
						self.compile_call_into(&call, freg, nres as u16, has_vararg);
						exp_base = Some(freg + 1);
					} else {
						let vt = self.tmp();
						self.emit(Instr::ab(Op::VarArgTab, vt, 0));
						let base = self.reserve(nres as u16);
						for i in 0..nres {
							let kreg = self.tmp();
							let k = self.kidx(Const::Num((i + 1) as f64));
							self.emit(Instr::ab(Op::LoadK, kreg, k));
							self.emit(Instr::abc(Op::GetTab, base + i as u16, vt, kreg));
						}
						exp_base = Some(base);
					}
				} else if nv > 0 && npre < nv {
					let i = nv - 1;
					let r = self.tmp();
					self.compile_expr(&values[i], r);
				}
				for i in 0..nlead {
					self.store_target(&tinfos[i], lead_regs[i]);
				}
				if let Some(base) = exp_base {
					for i in 0..nres {
						self.store_target(&tinfos[npre + i], base + i as u16);
					}
				}
				let knil = self.kidx(Const::Nil);
				let assigned = if nv > 0 && (last_is_call || last_is_vararg) && npre < n {
					n
				} else {
					npre.min(n)
				};
				for i in assigned..n {
					let r = self.tmp();
					let _ = self.emit(Instr::ab(Op::LoadK, r, knil));
					self.store_target(&tinfos[i], r);
				}
			}
			Stmt::ExprStmt(e) => {
				let r = self.tmp();
				self.compile_expr(e, r);
			}
			Stmt::If { cond, thenb, elsifs, elseb } => {
				let t = self.tmp();
				self.compile_expr(cond, t);
				let l_end = self.new_label();
				let mut l_next = self.new_label();
				self.jmp(Op::Jf, t, &l_next);
				self.compile_block(thenb);
				self.jmp(Op::Jmp, 0, &l_end);
				for (c, b) in elsifs {
					self.here(&l_next);
					l_next = self.new_label();
					let t2 = self.tmp();
					self.compile_expr(c, t2);
					self.jmp(Op::Jf, t2, &l_next);
					self.compile_block(b);
					self.jmp(Op::Jmp, 0, &l_end);
				}
				self.here(&l_next);
				if let Some(b) = elseb {
					self.compile_block(b);
				}
				self.here(&l_end);
			}
			Stmt::While { cond, body } => {
				let cap = loop_capture_set(body, None, &declared_set_of(body));
				self.cell_loops.push(cap);
				let l_top = self.new_label();
				let l_end = self.new_label();
				self.here(&l_top);
				let t = self.tmp();
				self.compile_expr(cond, t);
				self.jmp(Op::Jf, t, &l_end);
				self.loops.push((l_end.clone(), l_top.clone()));
				self.push_scope();
				self.compile_block(body);
				self.pop_scope();
				self.loops.pop();
				self.jmp(Op::Jmp, 0, &l_top);
				self.here(&l_end);
				self.cell_loops.pop();
			}
			Stmt::Repeat { body, cond } => {
				let cap = loop_capture_set(body, Some(cond), &declared_set_of(body));
				self.cell_loops.push(cap);
				let l_top = self.new_label();
				let l_check = self.new_label();
				let l_end = self.new_label();
				self.here(&l_top);
				self.loops.push((l_end.clone(), l_check.clone()));
				self.push_scope();
				for s in body.stmts.iter() {
					self.compile_stmt(s);
				}
				self.here(&l_check);
				let t = self.tmp();
				self.compile_expr(cond, t);
				self.jmp(Op::Jf, t, &l_top);
				self.here(&l_end);
				self.pop_scope();
				self.loops.pop();
				self.cell_loops.pop();
			}
			Stmt::ForNum {
				var,
				var_sym,
				start,
				limit,
				step,
				body,
			} => {
				let _ = var;
				let rl = self.tmp();
				let rs = self.tmp();
				let rc = self.tmp();
				self.compile_expr(start, rc);
				self.compile_expr(limit, rl);
				match step {
					Some(st) => self.compile_expr(st, rs),
					None => {
						let k = self.kidx(Const::Num(1.0));
						self.emit(Instr::ab(Op::LoadK, rs, k));
					}
				}
				let mut declared = declared_set_of(body);
				declared.insert(*var_sym);
				let cap = loop_capture_set(body, None, &declared);
				let var_captured = cap.contains(var_sym);
				self.cell_loops.push(cap);
				let l_top = self.new_label();
				let l_end = self.new_label();
				let l_inc = self.new_label();
				self.here(&l_top);
				let k0 = self.kidx(Const::Num(0.0));
				let r0 = self.tmp();
				self.emit(Instr::ab(Op::LoadK, r0, k0));
				let t1 = self.tmp();
				self.emit(Instr::abc(Op::Ge, t1, rs, r0));
				let t2 = self.tmp();
				self.emit(Instr::abc(Op::Gt, t2, rc, rl));
				let t3 = self.tmp();
				self.emit_and(t3, t1, t2);
				let t4 = self.tmp();
				self.emit(Instr::abc(Op::Lt, t4, rs, r0));
				let t5 = self.tmp();
				self.emit(Instr::abc(Op::Lt, t5, rc, rl));
				let t6 = self.tmp();
				self.emit_and(t6, t4, t5);
				let t7 = self.tmp();
				self.emit_or(t7, t3, t6);
				self.jmp(Op::Jt, t7, &l_end);
				self.loops.push((l_end.clone(), l_inc.clone()));
				self.push_scope();
				if var_captured {
					let kind = self.cell_from_value(rc);
					self.declare(*var_sym, kind);
				} else {
					let rv = self.tmp();
					self.emit(Instr::ab(Op::Move, rv, rc));
					self.declare(*var_sym, CellKind::Plain(rv));
				}
				self.compile_block(body);
				self.pop_scope();
				self.loops.pop();
				self.cell_loops.pop();
				self.here(&l_inc);
				self.emit(Instr::abc(Op::Add, rc, rc, rs));
				self.jmp(Op::Jmp, 0, &l_top);
				self.here(&l_end);
			}
			Stmt::ForGen { vars, syms, iters, body } => {
				let _ = vars;
				let mut declared = declared_set_of(body);
				for s in syms {
					declared.insert(*s);
				}
				let cap = loop_capture_set(body, None, &declared);
				let sym_captured: Vec<bool> = syms.iter().map(|s| cap.contains(s)).collect();
				self.cell_loops.push(cap);
				let rit = self.tmp();
				let rstt = self.tmp();
				let rctl = self.tmp();
			if iters.len() == 1 && is_call(&iters[0]) {
				let call = iters[0].clone();
				let (nargs, has_vararg) = call_arg_info(&call);
				let freg = self.reserve(1 + nargs.max(1));
				self.compile_call_into(&call, freg, 3, has_vararg);
				self.emit(Instr::ab(Op::Move, rit, freg + 1));
				self.emit(Instr::ab(Op::Move, rstt, freg + 2));
				self.emit(Instr::ab(Op::Move, rctl, freg + 3));
			} else {
				if iters.len() > 0 {
					self.compile_expr(&iters[0], rit);
				}
				if iters.len() > 1 {
					self.compile_expr(&iters[1], rstt);
				} else {
					self.emit(Instr::ab(Op::LoadNil, rstt, 1));
				}
				if iters.len() > 2 {
					self.compile_expr(&iters[2], rctl);
				} else {
					// The control variable must start nil on EVERY entry to
					// this loop.  Registers are frame slots that keep their
					// values across statements, so without this the previous
					// loop's final key leaks in and `next(t, lastkey)`
					// terminates a re-entered loop immediately.
					self.emit(Instr::ab(Op::LoadNil, rctl, 1));
				}
			}
				let l_top = self.new_label();
				let l_end = self.new_label();
				self.here(&l_top);
				self.loops.push((l_end.clone(), l_top.clone()));
				let nv = syms.len() as u16;
				let freg = self.tmp();
				let _ = self.reserve(2 + nv.max(2).max(2));
				self.emit(Instr::ab(Op::Move, freg, rit));
				self.emit(Instr::ab(Op::Move, freg + 1, rstt));
				self.emit(Instr::ab(Op::Move, freg + 2, rctl));
				self.emit(Instr::abcd(Op::Call, freg, 2, nv, 0));
				let tnil = self.tmp();
				let knil = self.kidx(Const::Nil);
				self.emit(Instr::ab(Op::LoadK, tnil, knil));
				let teq = self.tmp();
				self.emit(Instr::abc(Op::Eq, teq, freg + 1, tnil));
				self.jmp(Op::Jt, teq, &l_end);
				self.emit(Instr::ab(Op::Move, rctl, freg + 1));
				self.push_scope();
				for (i, s) in syms.iter().enumerate() {
					if sym_captured[i] {
						let kind = self.cell_from_value(freg + 1 + i as u16);
						self.declare(*s, kind);
					} else {
						let r = self.tmp();
						self.emit(Instr::ab(Op::Move, r, freg + 1 + i as u16));
						self.declare(*s, CellKind::Plain(r));
					}
				}
				self.compile_block(body);
				self.pop_scope();
				self.loops.pop();
				self.cell_loops.pop();
				self.jmp(Op::Jmp, 0, &l_top);
				self.here(&l_end);
			}
			Stmt::Break => {
				// invariant: the parser rejects `break` outside a loop (parser.rs stat()), so loops is non-empty here
				let (l_end, _) = *self.loops.last().expect("break outside loop");
				self.jmp(Op::Jmp, 0, &l_end);
			}
			Stmt::Continue => {
				// invariant: the parser rejects `continue` outside a loop (parser.rs stat()), so loops is non-empty here
				let (_, l_cont) = *self.loops.last().expect("continue outside loop");
				self.jmp(Op::Jmp, 0, &l_cont);
			}
				Stmt::Return(es) => {
					if es.is_empty() {
						self.emit(Instr::ab(Op::Return, 0, 0));
					// invariant: the `es.is_empty()` branch returns before this line, so the slice is non-empty
					} else if is_call(es.last().unwrap()) {
						let pre = es.len() - 1;
						let base = self.reserve(pre as u16);
						for (i, e) in es.iter().take(pre).enumerate() {
							self.compile_expr(e, base + i as u16);
						}
						// invariant: the enclosing `else if is_call(es.last().unwrap())` branch proved the last element exists
						let call = es.last().unwrap().clone();
						let (nargs, has_vararg) = call_arg_info(&call);
						let freg = self.reserve(1 + nargs.max(1));
						self.compile_call_into(&call, freg, 255, has_vararg);
						self.emit(Instr::abcd(Op::Return, base, 255, 0, pre as u16));
					// invariant: the `es.is_empty()` branch returns before this line, so the slice is non-empty
					} else if matches!(es.last().unwrap(), Expr::Vararg) {
						let pre = es.len() - 1;
						let base = self.reserve(pre as u16);
						for (i, e) in es.iter().take(pre).enumerate() {
							self.compile_expr(e, base + i as u16);
						}
						self.emit(Instr::abcd(Op::Return, base, 255, 1, pre as u16));
					} else {
						let n = es.len() as u16;
						let base = self.reserve(n);
						for (i, e) in es.iter().enumerate() {
							self.compile_expr(e, base + i as u16);
						}
						self.emit(Instr::abcd(Op::Return, base, n, 0, 0));
					}
				}
			Stmt::Do(b) => self.compile_block(b),
			_ => {}
		}
	}

	fn emit_and(&mut self, dst: u16, l: u16, r: u16) {
		self.emit(Instr::ab(Op::Move, dst, l));
		let lskip = self.new_label();
		self.jmp(Op::Jf, dst, &lskip);
		self.emit(Instr::ab(Op::Move, dst, r));
		self.here(&lskip);
	}

	fn emit_or(&mut self, dst: u16, l: u16, r: u16) {
		self.emit(Instr::ab(Op::Move, dst, l));
		let lskip = self.new_label();
		self.jmp(Op::Jt, dst, &lskip);
		self.emit(Instr::ab(Op::Move, dst, r));
		self.here(&lskip);
	}

	fn compile_expr(&mut self, e: &Expr, dst: u16) {
		match e {
			Expr::Num { value, .. } => {
				let k = self.kidx(Const::Num(*value));
				self.emit(Instr::ab(Op::LoadK, dst, k));
			}
			Expr::Str { bytes, .. } | Expr::LongStr { bytes } => {
				let k = self.kidx(Const::Str(bytes.clone()));
				self.emit(Instr::ab(Op::LoadK, dst, k));
			}
			Expr::SealedStr { bytes, seed, step } => {
				let cipher = crate::vmgen::strseal::encode(bytes, *seed, *step);
				// Phase 18 P1-1: dedup the sealed constant so repeated
				// references to the same plaintext share one pool slot (that is
				// what makes the runtime cache and its reference count real).
				let k = self.kidx(Const::Str(cipher));
				self.program.has_loadstr = true;
				self.emit(Instr::abcd(Op::LoadStr, dst, k, *seed, *step));
			}
			Expr::IfExpr { arms, elseb } => {
				let l_end = self.new_label();
				for (cond, val) in arms {
					let l_next = self.new_label();
					let t = self.tmp();
					self.compile_expr(cond, t);
					self.jmp(Op::Jf, t, &l_next);
					self.compile_expr(val, dst);
					self.jmp(Op::Jmp, 0, &l_end);
					self.here(&l_next);
				}
				self.compile_expr(elseb, dst);
				self.here(&l_end);
			}
			Expr::Bool { value } => {
				let k = self.kidx(Const::Bool(*value));
				self.emit(Instr::ab(Op::LoadK, dst, k));
			}
			Expr::Nil => {
				let k = self.kidx(Const::Nil);
				self.emit(Instr::ab(Op::LoadK, dst, k));
			}
				Expr::Vararg => {
					let t = self.tmp();
					self.emit(Instr::ab(Op::VarArgTab, t, 0));
					let kreg = self.tmp();
					let k1 = self.kidx(Const::Num(1.0));
					self.emit(Instr::ab(Op::LoadK, kreg, k1));
					self.emit(Instr::abc(Op::GetTab, dst, t, kreg));
				}
			Expr::Paren { e } => self.compile_expr(e, dst),
			Expr::Ident { name, sym } => match sym {
				Some(s) => {
					if let Some(k) = self.lookup(*s) {
						match k {
							CellKind::Plain(r) => {
								self.emit(Instr::ab(Op::Move, dst, r));
							}
							CellKind::Slot(sr) => {
								let k1 = self.kidx(Const::Num(1.0));
								let kreg = self.tmp();
								self.emit(Instr::ab(Op::LoadK, kreg, k1));
								self.emit(Instr::abc(Op::GetTab, dst, sr, kreg));
							}
							CellKind::Up(u) => {
								self.emit(Instr::ab(Op::GetUp, dst, u));
							}
						}
					} else if let Some(u) = self.upvals.get(s).copied() {
						self.emit(Instr::ab(Op::GetUp, dst, u));
					} else {
						let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
						self.emit(Instr::ab(Op::GetGlobal, dst, k));
					}
				}
				None => {
					let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
					self.emit(Instr::ab(Op::GetGlobal, dst, k));
				}
			},
			Expr::Dot { obj, name } => {
				let t = self.tmp();
				self.compile_expr(obj, t);
				let kreg = self.tmp();
				let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
				self.emit(Instr::ab(Op::LoadK, kreg, k));
				self.emit(Instr::abc(Op::GetTab, dst, t, kreg));
			}
			Expr::Index { obj, idx } => {
				let t = self.tmp();
				let k = self.tmp();
				self.compile_expr(obj, t);
				self.compile_expr(idx, k);
				self.emit(Instr::abc(Op::GetTab, dst, t, k));
			}
			Expr::Bin { op, l, r } => match op {
				BinOp::And => {
					self.compile_expr(l, dst);
					let lskip = self.new_label();
					self.jmp(Op::Jf, dst, &lskip);
					self.compile_expr(r, dst);
					self.here(&lskip);
				}
				BinOp::Or => {
					self.compile_expr(l, dst);
					let lskip = self.new_label();
					self.jmp(Op::Jt, dst, &lskip);
					self.compile_expr(r, dst);
					self.here(&lskip);
				}
				_ => {
					let tl = self.tmp();
					let tr = self.tmp();
					self.compile_expr(l, tl);
					self.compile_expr(r, tr);
					let iop = match op {
						BinOp::Add => Op::Add,
						BinOp::Sub => Op::Sub,
						BinOp::Mul => Op::Mul,
						BinOp::Div => Op::Div,
						BinOp::Idiv => Op::Idiv,
						BinOp::Mod => Op::Mod,
						BinOp::Pow => Op::Pow,
						BinOp::Concat => Op::Concat,
						BinOp::Eq => Op::Eq,
						BinOp::Ne => Op::Ne,
						BinOp::Lt => Op::Lt,
						BinOp::Gt => Op::Gt,
						BinOp::Le => Op::Le,
						BinOp::Ge => Op::Ge,
						// invariant: the match is exhaustive over the AST constructors that compile_expr accepts
						_ => unreachable!(),
					};
					self.emit(Instr::abc(iop, dst, tl, tr));
				}
			},
			Expr::Un { op, e } => {
				if *op == UnOp::Len && matches!(e.as_ref(), Expr::Vararg) {
					self.emit(Instr::ab(Op::VarArgC, dst, 0));
					return;
				}
				let t = self.tmp();
				self.compile_expr(e, t);
				let uop = match op {
					UnOp::Minus => Op::Unm,
					UnOp::Not => Op::Not,
					UnOp::Len => Op::Len,
				};
				self.emit(Instr::ab(uop, dst, t));
			}
			Expr::Call { .. } => {
				let (nargs, has_vararg) = call_arg_info(e);
				let freg = self.reserve(1 + nargs.max(1));
				self.compile_call_into(e, freg, 1, has_vararg);
				if freg + 1 != dst {
					self.emit(Instr::ab(Op::Move, dst, freg + 1));
				}
			}
			Expr::Method { obj, name, args } => {
				let t = self.tmp();
				self.compile_expr(obj, t);
				let kreg = self.tmp();
				let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
				self.emit(Instr::ab(Op::LoadK, kreg, k));
				let n = (args.len() + 1) as u16;
				let freg = self.reserve(1 + n);
				self.emit(Instr::abc(Op::GetTab, freg, t, kreg));
				self.emit(Instr::ab(Op::Move, freg + 1, t));
				for (i, a) in args.iter().enumerate() {
					self.compile_expr(a, freg + 2 + i as u16);
				}
				self.emit(Instr::abcd(Op::Call, freg, n, 1, 0));
				if freg + 1 != dst {
					self.emit(Instr::ab(Op::Move, dst, freg + 1));
				}
			}
			Expr::Table { fields } => {
				self.emit(Instr::ab(Op::NewTab, dst, 0));
				let cnt = self.tmp();
				let k0 = self.kidx(Const::Num(0.0));
				self.emit(Instr::ab(Op::LoadK, cnt, k0));
				let last = fields.len().saturating_sub(1);
				if self.lua51 {
					let mut pending: Vec<u16> = Vec::new();
					let mut tail_call: Option<Expr> = None;
					let mut tail_vararg = false;
					for (i, f) in fields.iter().enumerate() {
						match f {
							TableField::Key { key, value } => {
								let kv = self.tmp();
								let vv = self.tmp();
								self.compile_expr(key, kv);
								self.compile_expr(value, vv);
								self.emit(Instr::abc(Op::SetTab, dst, kv, vv));
							}
							TableField::Array(e) => {
								if i == last && is_call(e) {
									tail_call = Some(e.clone());
								} else if i == last && matches!(e, Expr::Vararg) {
									tail_vararg = true;
								} else {
									let v = self.tmp();
									self.compile_expr(e, v);
									pending.push(v);
								}
							}
						}
					}
					for v in pending {
						self.emit(Instr::abc(Op::TabN, dst, cnt, v));
					}
					if tail_vararg {
						self.emit(Instr::ab(Op::VarArgTabN, dst, cnt));
					}
					if let Some(e) = tail_call {
						let (nargs, has_vararg) = call_arg_info(&e);
let _ = has_vararg;
						let freg = self.reserve(1 + nargs.max(1));
						self.compile_call_t(&e, freg, dst, cnt);
					}
				} else {
					for (i, f) in fields.iter().enumerate() {
						match f {
							TableField::Array(e) => {
								if i == last && is_call(e) {
									let (nargs, has_vararg) = call_arg_info(e);
let _ = has_vararg;
									let freg = self.reserve(1 + nargs.max(1));
									self.compile_call_t(e, freg, dst, cnt);
								} else if i == last && matches!(e, Expr::Vararg) {
									self.emit(Instr::ab(Op::VarArgTabN, dst, cnt));
								} else {
									let v = self.tmp();
									self.compile_expr(e, v);
									self.emit(Instr::abc(Op::TabN, dst, cnt, v));
								}
							}
							TableField::Key { key, value } => {
								let kv = self.tmp();
								let vv = self.tmp();
								self.compile_expr(key, kv);
								self.compile_expr(value, vv);
								self.emit(Instr::abc(Op::SetTab, dst, kv, vv));
							}
						}
					}
				}
			}
			Expr::Function {
				params,
				param_syms,
				vararg,
				body,
			} => {
				self.compile_function(dst, body, params, param_syms, *vararg);
			}
		}
	}

	fn call_tail<'e>(e: &'e Expr) -> Option<&'e Expr> {
		let args = match e {
			Expr::Call { args, .. } => args,
			Expr::Method { args, .. } => args,
			_ => return None,
		};
		args.last().filter(|a| is_call(a))
	}

	fn call_nfixed(e: &Expr) -> (u16, bool) {
		let (args, is_m) = match e {
			Expr::Call { args, .. } => (args, false),
			Expr::Method { args, .. } => (args, true),
			// invariant: the match is exhaustive over the AST constructors that compile_expr accepts
			_ => unreachable!(),
		};
		let has_tail = args.last().map(is_call).unwrap_or(false);
		let fixed = if has_tail {
			(args.len() - 1) as u16 + if is_m { 1 } else { 0 }
		} else {
			args.iter().filter(|a| !matches!(a, Expr::Vararg)).count() as u16
				+ if is_m { 1 } else { 0 }
		};
		(fixed, has_tail)
	}

	fn compile_call_prefix(&mut self, e: &Expr, freg: u16, nfixed: u16) {
		match e {
			Expr::Call { func, args } => {
				self.compile_expr(func, freg);
				for (i, a) in args.iter().enumerate() {
					if (i as u16) >= nfixed {
						break;
					}
					if matches!(a, Expr::Vararg) {
						continue;
					}
					self.compile_expr(a, freg + 1 + i as u16);
				}
			}
			Expr::Method { obj, name, args } => {
				let treg = self.tmp();
				self.compile_expr(obj, treg);
				let kreg = self.tmp();
				let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
				self.emit(Instr::ab(Op::LoadK, kreg, k));
				self.emit(Instr::abc(Op::GetTab, freg, treg, kreg));
				self.emit(Instr::ab(Op::Move, freg + 1, treg));
				for (i, a) in args.iter().enumerate() {
					if (i as u16) >= nfixed - 1 {
						break;
					}
					if matches!(a, Expr::Vararg) {
						continue;
					}
					self.compile_expr(a, freg + 2 + i as u16);
				}
			}
			// invariant: the match is exhaustive over the AST constructors that compile_expr accepts
			_ => unreachable!(),
		}
	}

	fn compile_call_expand(&mut self, e: &Expr, freg: u16) {
		let (nfixed, has_tail) = Self::call_nfixed(e);
		self.compile_call_prefix(e, freg, nfixed);
		let mut d: u16 = 0;
		if has_tail {
			// invariant: the call is guarded by `is_call(e)` at the call site
			let tail = Self::call_tail(e).unwrap().clone();
			let (inargs, _) = call_arg_info(&tail);
			let ifreg = freg + nfixed + 1;
			if inargs > 0 {
				self.reserve(inargs);
			}
			self.compile_call_expand(&tail, ifreg);
			d |= 2;
		}
		let varg = match e {
			Expr::Call { args, .. } => args.iter().any(|a| matches!(a, Expr::Vararg)),
			Expr::Method { args, .. } => args.iter().any(|a| matches!(a, Expr::Vararg)),
			_ => false,
		};
		if varg {
			d |= 1;
		}
		self.emit(Instr::abcd(Op::CallE, freg, nfixed, 0, d));
	}

	fn compile_call_into(&mut self, e: &Expr, freg: u16, nres: u16, has_vararg: bool) {
		let (nfixed, has_tail) = Self::call_nfixed(e);
		self.compile_call_prefix(e, freg, nfixed);
		if has_tail {
			// invariant: the call is guarded by `is_call(e)` at the call site
			let tail = Self::call_tail(e).unwrap().clone();
			let (inargs, _) = call_arg_info(&tail);
			let ifreg = freg + nfixed + 1;
			if inargs > 0 {
				self.reserve(inargs);
			}
			self.compile_call_expand(&tail, ifreg);
			self.emit(Instr::abcd(Op::CallM, freg, nfixed, nres, has_vararg as u16));
		} else {
			self.emit(Instr::abcd(Op::Call, freg, nfixed, nres, has_vararg as u16));
		}
	}

	fn compile_call_t(&mut self, e: &Expr, freg: u16, t: u16, cnt: u16) {
		let (nfixed, has_tail) = Self::call_nfixed(e);
		self.compile_call_prefix(e, freg, nfixed);
		let mut d = nfixed * 2;
		if has_tail {
			// invariant: the call is guarded by `is_call(e)` at the call site
			let tail = Self::call_tail(e).unwrap().clone();
			let (inargs, _) = call_arg_info(&tail);
			let ifreg = freg + nfixed + 1;
			if inargs > 0 {
				self.reserve(inargs);
			}
			self.compile_call_expand(&tail, ifreg);
			d |= 1;
		}
		self.emit(Instr::abcd(Op::CallT, freg, t, cnt, d));
	}

	fn compile_function(
		&mut self,
		dst: u16,
		body: &Block,
		params: &[String],
		param_syms: &[SymId],
		vararg: bool,
	) {
		let _ = params;
		let analysis = analyze(body, param_syms);

		let mut ups: Vec<SymId> = analysis.nested_up.clone();
		for s in &analysis.direct_up {
			if !ups.contains(s) {
				ups.push(*s);
			}
		}
		ups.sort_unstable();
		ups.dedup();
		if crate::audit_gate::audit_enabled("OBSUCVIM_VM_DBG") {
			let nm = |v: Vec<SymId>| {
				let t: Vec<String> = v
					.iter()
					.map(|&s| format!("{}({})", self.symtab.name_of(s), s))
					.collect();
				t.join(",")
			};
			let scopes_dbg: Vec<String> = self
				.scopes
				.iter()
				.map(|sc| {
					let e: Vec<String> = sc
						.iter()
						.map(|(s, r)| format!("{}({})@{:?}", self.symtab.name_of(*s), s, r))
						.collect();
					e.join("|")
				})
				.collect();
			eprintln!(
				"[fn] direct_up=[{}] nested_up=[{}] ups=[{}]",
				nm(analysis.direct_up.clone()),
				nm(analysis.nested_up.clone()),
				nm(ups.clone())
			);
			eprintln!("[fn] scopes={:?}", scopes_dbg);
		}

		let child_index = self.next_fn_slot;
		self.next_fn_slot += 1;
		// C5 fail-closed path. The check has to precede *every* index derived
		// from child_index (s_indices / block_starts / fns): `fns` has `total`
		// entries but `s_indices` has `total + 1`, so an undercount of two or
		// more used to escape this check and die as a bare index-out-of-bounds
		// panic on the s_indices write instead (P4 audit). The precondition
		// itself is an internal invariant: count_fns_count walks the same
		// Block that is compiled and every compile_function call corresponds to
		// exactly one counted node (Stmt::LocalFunc / Stmt::FuncDecl /
		// Expr::Function); `c5_count_fns_matches_compiled_closure_slots`
		// pins that for every syntactic closure position.
		if child_index >= self.program.fns.len() { crate::limits::fatal(&format!("input exceeds compiler limit: slot overflow: child_index={} len={} next={}", child_index, self.program.fns.len(), self.next_fn_slot)); }
		let up_seed = (self.program.blob_seed as u64)
			^ (self.program.blob_km as u64).rotate_left(11)
			^ (self.program.blob_kc as u64).rotate_left(23)
			^ (self.program.ck_km as u64).rotate_left(37)
			^ (self.program.ck_kc as u64).rotate_left(49);
		let child_up_perm =
			derive_upvalue_permutation(up_seed, child_index, ups.len(), self.scatter);

		let mut descriptors: Vec<u16> = Vec::new();
		for s in &ups {
			let mut found = None;
			for sc in self.scopes.iter().rev() {
				if let Some(&k) = sc.get(s) {
					found = Some(k);
					break;
				}
			}
			match found {
				Some(CellKind::Plain(r)) => descriptors.push(r + 1),
				Some(CellKind::Slot(sr)) => descriptors.push(sr + 1 | 0x8000),
				Some(CellKind::Up(u)) => {
					let parent_idx = *self.up_perm.get(u as usize)
						// invariant: up_perm is built with one entry per upvalue slot by derive_upvalue_permutation
						.expect("parent upvalue permutation missing");
					descriptors.push(parent_idx as u16 + 1 | 0xC000);
				}
				None => crate::limits::fatal(&format!(
					"input exceeds compiler limit: upvalue descriptor miss: sym {} not in any enclosing scope",
					s
				)),
			}
		}

		self.program.s_indices[child_index] = if self.scatter { self.rng.int(0, 3) as u8 } else { 0 };
		let nparams = param_syms.len() as u16;
		let mut child = Ctx {
			program: self.program,
			rng: self.rng,
			symtab: self.symtab,
			lua51: self.lua51,
			scatter: self.scatter,
			next_fn_slot: self.next_fn_slot,
			scopes: vec![HashMap::new()],
			upvals: HashMap::new(),
			upsrc: descriptors,
			up_perm: child_up_perm,
			next_reg: nparams,
			nparams,
			vararg,
			consts: Vec::new(),
			const_map: HashMap::new(),
			code: Vec::new(),
			labels: HashMap::new(),
			loops: Vec::new(),
			cell_loops: Vec::new(),
			str_clearable: self.str_clearable,
			super_insn: self.super_insn,
			canary: self.canary,
			ccpd: self.ccpd,
		};
		for (i, s) in ups.iter().enumerate() {
			child.upvals.insert(*s, i as u16);
		}
		child.push_scope();
		for (i, s) in param_syms.iter().enumerate() {
			child.declare(*s, CellKind::Plain(i as u16));
		}
		for s in &ups {
			let u = child.upvals[s];
			child.declare(*s, CellKind::Up(u));
		}
		child.compile_block(body);
		child.pop_scope();
		let slot_end = child.next_fn_slot;
		let nregs = child.next_reg;
		let (bytes, nops, osum, osum2, chaff, blocks) = child.finish(nregs, child_index);
			self.program.block_starts[child_index] = blocks;
		self.next_fn_slot = slot_end;
		self.program.fns[child_index] = bytes;
		self.program.nop_sites[child_index] = nops;
		self.program.operand_sums[child_index] = osum;
		self.program.operand_sums2[child_index] = osum2;
		self.program.chaff_folds[child_index] = chaff;

		self.emit(Instr::ab(Op::Closure, dst, child_index as u16));
	}

	fn eval_target(&mut self, t: &Expr) -> Target {
		match t {
			Expr::Ident { name, sym } => match sym {
				Some(s) => Target::Local(*s),
				None => Target::Global(self.kidx(Const::Str(name.as_bytes().to_vec()))),
			},
			Expr::Dot { obj, name } => {
				let treg = self.tmp();
				self.compile_expr(obj, treg);
				let kreg = self.tmp();
				let k = self.kidx(Const::Str(name.as_bytes().to_vec()));
				self.emit(Instr::ab(Op::LoadK, kreg, k));
				Target::Index2 { t: treg, k: kreg }
			}
			Expr::Index { obj, idx } => {
				let treg = self.tmp();
				let kreg = self.tmp();
				self.compile_expr(obj, treg);
				self.compile_expr(idx, kreg);
				Target::Index2 { t: treg, k: kreg }
			}
			// Not a resource-budget rejection: the AST accepted a target the
			// compiler cannot store into (e.g. `f() = 1`). Report it as what it
			// is, on the same single-line fail-closed channel.
			_ => crate::limits::fatal("invalid assignment target"),
		}
	}

	fn store_target(&mut self, t: &Target, vreg: u16) {
		match t {
			Target::Local(s) => {
				if let Some(k) = self.lookup(*s) {
					match k {
						CellKind::Plain(r) => {
							self.emit(Instr::ab(Op::Move, r, vreg));
							if let Some(u) = self.upvals.get(&s).copied() {
								self.emit(Instr::ab(Op::SetUp, u, r));
							}
						}
						CellKind::Slot(sr) => {
							let k1 = self.kidx(Const::Num(1.0));
							let kreg = self.tmp();
							self.emit(Instr::ab(Op::LoadK, kreg, k1));
							self.emit(Instr::abc(Op::SetTab, sr, kreg, vreg));
						}
						CellKind::Up(u) => {
							self.emit(Instr::ab(Op::SetUp, u, vreg));
						}
					}
				} else if let Some(u) = self.upvals.get(s).copied() {
					self.emit(Instr::ab(Op::SetUp, u, vreg));
				} else {
					crate::limits::fatal("input exceeds compiler limit: store to unknown local");
				}
			}
			Target::Global(k) => {
				self.emit(Instr::ab(Op::SetGlobal, vreg, *k));
			}
			Target::Index2 { t, k } => {
				self.emit(Instr::abc(Op::SetTab, *t, *k, vreg));
			}
		}
	}
}

#[derive(Debug, Clone)]
enum Target {
	Local(SymId),
	Global(u16),
	Index2 { t: u16, k: u16 },
}

fn is_call(e: &Expr) -> bool {
	matches!(e, Expr::Call { .. } | Expr::Method { .. })
}

pub(crate) fn call_window_mask(code: &[Instr]) -> Vec<bool> {
	let mut mask = vec![false; code.len()];
	let mut open = false;
	for (idx, ins) in code.iter().enumerate() {
		let opens = matches!(ins.op, Op::Call | Op::CallE | Op::CallM);
		let closes =
			matches!(ins.op, Op::Call | Op::CallE | Op::CallM | Op::CallT | Op::Return);
		if open {
			mask[idx] = true;
			if closes {
				open = false;
			}
		}
		if opens {
			open = true;
		}
	}
	mask
}

fn call_arg_info(e: &Expr) -> (u16, bool) {
	match e {
		Expr::Call { args, .. } => {
			let n = args
				.iter()
				.filter(|a| !matches!(a, Expr::Vararg))
				.count() as u16;
			let v = args.iter().any(|a| matches!(a, Expr::Vararg));
			(n, v)
		}
		Expr::Method { args, .. } => ((args.len() + 1) as u16, false),
		_ => (0, false),
	}
}


pub fn compile(
	block: &Block,
	table: &SymTable,
	rng: &mut Rng,
	lua51: bool,
	scatter: bool,
	super_insn: bool,
	canary: bool,
	proto_layout: bool,
	ccpd: bool,
) -> VmProgram {
	compile_chunk(block, table, rng, lua51, scatter, super_insn, canary, proto_layout, ccpd)
}

#[cfg(test)]
mod dbg {
	use super::*;
	use crate::parser;
	use crate::symtab;
	use crate::vmgen::fuse;

	fn varint(src: &[u8], p: &mut usize) -> u32 {
		let (v, next) = isa::decode_varint(src, *p).expect("valid proto varint");
		*p = next;
		v
	}

	fn unmask_blob(program: &VmProgram, fi: usize) -> Vec<u8> {
		let cipher = &program.fns[fi];
		let split = cipher.len().min(64);
		let base = (program.blob_seed as u64 + fi as u64 * program.blob_step as u64)
			% 268_435_456;
		let prefix_fold = cipher[..split]
			.iter()
			.fold(0u64, |h, &b| (h * 31 + b as u64) % 268_435_456);
		let mut out = cipher.clone();
		let mut state = base;
		let mut prev: u64 = 0;
		for b in out.iter_mut().take(split) {
			state = (program.blob_km as u64 * state + program.blob_kc as u64 + prev)
				% 268_435_456;
			*b = b.wrapping_sub(isa::fold_key_nl(state));
			prev = *b as u64;
		}
		let mut state = (base + prefix_fold) % 268_435_456;
		for b in out.iter_mut().skip(split) {
			state = (program.blob_km as u64 * state + program.blob_kc as u64 + prev)
				% 268_435_456;
			*b = b.wrapping_sub(isa::fold_key_nl(state));
			prev = *b as u64;
		}
		out
	}

	/// V17 P1-b：fused 模式编译后的 blob 自封完整性——逐 section 走查：
	/// id 7(fv) 必须出现、各段消耗到 blob 恰好结尾、fv 条数与 fv_sets 一致。
	fn section_walk(program: &VmProgram, fi: usize) -> Vec<usize> {
		let blob = unmask_blob(program, fi);
		let mut p = 0usize;
		let mut seen = Vec::new();
		while p < blob.len() {
			let tag = blob[p];
			p += 1;
			let id = program.proto_layouts[fi]
				.field_of(tag)
				.map(|f| f.id() as usize)
				.expect("known section tag");
			assert_ne!(id, 6, "unread padding sections must not be emitted");
			seen.push(id);
			match id {
				0 => {
					for _ in 0..3 {
						varint(&blob, &mut p);
					}
				}
				1 => {
					varint(&blob, &mut p);
				}
				2 => {
					let nups = varint(&blob, &mut p) as usize;
					for _ in 0..nups {
						varint(&blob, &mut p);
					}
				}
				3 => {
					let _nconst = varint(&blob, &mut p) as usize;
					let len = varint(&blob, &mut p) as usize;
					p += len;
				}
				4 => {
					let nslots = varint(&blob, &mut p) as usize;
					for _ in 0..nslots {
						varint(&blob, &mut p);
					}
				}
				5 => {
					let ncode = varint(&blob, &mut p) as usize;
					p += ncode;
					for _ in 0..4 {
						for _ in 0..ncode {
							varint(&blob, &mut p);
						}
					}
				}
				6 => panic!("unread padding sections must not be emitted"),
				7 => {
					let nv = varint(&blob, &mut p) as usize;
					for _ in 0..nv {
						varint(&blob, &mut p);
					}
				}
				// invariant: the match is exhaustive over the AST constructors that compile_expr accepts
				_ => unreachable!(),
			}
		}
		assert_eq!(p, blob.len(), "section walk must consume the blob exactly");
		seen
	}

	#[test]
	fn super_insn_blob_layout_and_vocab_consistency() {
		// Move-Move / LoadK-Move 密集的双函数源：词汇必须非空
		let src = "local function f(a, b) local x = a local y = x local z = y + 1 \
			local function g(q) local w = q return w end \
			return z, g(z) end return f(1, 2)";
		let mut block = parser::parse(src, true).unwrap();
		let table = symtab::resolve(&mut block);
		let mut rng = crate::rng::Rng::new(4242);
		let prog = compile_chunk(&block, &table, &mut rng, false, true, true, false, true, false);
		assert!(prog.fv_sets.iter().any(|v| !v.is_empty()), "super on: vocab must be non-empty here");
		for (fi, _) in prog.fns.iter().enumerate() {
			let walk = section_walk(&prog, fi.min(prog.fns.len() - 1));
			assert!(walk.contains(&7), "fv section must be present when super on (fn {fi})");
		}
		// 词汇合法性：全部是白名单对、规模 ≤ topK
		for v in &prog.fv_sets {
			assert!(v.len() <= fuse::FUSE_TOP_K);
			for &(pa, pb) in v {
				let key = (isa::op_from_index(pa), isa::op_from_index(pb));
				assert!(fuse::FUSE_WHITELIST.contains(&key), "non-whitelist pair leaked: {key:?}");
			}
		}
	}

	fn sealed_pool_strings(program: &VmProgram, fi: usize) -> Vec<Vec<u8>> {
		let blob = unmask_blob(program, fi);
		let mut p = 0usize;
		let mut ckseed = None;
		let mut cinfo = None;
		while p < blob.len() {
			let tag = blob[p];
			p += 1;
			let id = program.proto_layouts[fi]
				.field_of(tag)
				.map(|f| f.id() as usize)
				.expect("known section tag");
			match id {
				0 => {
					for _ in 0..3 {
						varint(&blob, &mut p);
					}
				}
				1 => ckseed = Some(varint(&blob, &mut p)),
				2 => {
					let nups = varint(&blob, &mut p) as usize;
					for _ in 0..nups {
						varint(&blob, &mut p);
					}
				}
				3 => {
					let nconst = varint(&blob, &mut p) as usize;
					let len = varint(&blob, &mut p) as usize;
					cinfo = Some((nconst, p, len));
					p += len;
				}
				4 => {
					let nslots = varint(&blob, &mut p) as usize;
					for _ in 0..nslots {
						varint(&blob, &mut p);
					}
				}
				5 => {
					let ncode = varint(&blob, &mut p) as usize;
					p += ncode;
					for _ in 0..4 {
						for _ in 0..ncode {
							varint(&blob, &mut p);
						}
					}
				}
				6 => panic!("unread padding sections must not be emitted"),
				7 => {
					// V17 fv 段：count + count×u16var
					let nv = varint(&blob, &mut p) as usize;
					for _ in 0..nv {
						varint(&blob, &mut p);
					}
				}
				// invariant: the match is exhaustive over the AST constructors that compile_expr accepts
				_ => unreachable!(),
			}
		}
		let (nconst, start, len) = cinfo.expect("constant section");
		let end = start + len;
		let bytes = &blob[start..end];
		let mut q = 0usize;
		let mut lcg = isa::ConstLcg::new(
			ckseed.expect("constant seed"),
			program.ck_km,
			program.ck_kc,
		);
		let mut strings = Vec::new();
		for _ in 0..nconst {
			let ty = lcg.unmask(bytes[q]);
			q += 1;
			match ty {
				0 => {}
				1 => {
					lcg.unmask(bytes[q]);
					q += 1;
				}
				3 => {
					let lo = lcg.unmask(bytes[q]) as usize;
					q += 1;
					let hi = lcg.unmask(bytes[q]) as usize;
					q += 1;
					let n = lo | (hi << 8);
					let mut s = Vec::with_capacity(n);
					for _ in 0..n {
						s.push(lcg.unmask(bytes[q]));
						q += 1;
					}
					strings.push(s);
				}
				4 => {
					for _ in 0..2 {
						loop {
							let b = lcg.unmask(bytes[q]);
							q += 1;
							if b < 128 {
								break;
							}
						}
					}
				}
				other => panic!("unexpected constant tag {other}"),
			}
		}
		assert_eq!(q, bytes.len(), "constant section consumed exactly");
		strings
	}

	#[test]
	fn sealed_literal_stays_ciphertext_after_proto_and_constant_unmasking() {
		let plain = b"r062-pool-canary-opaque";
		let src = format!(r#"return "{}""#, String::from_utf8_lossy(plain));
		let mut block = parser::parse(&src, true).unwrap();
		let table = symtab::resolve(&mut block);
		let mut seal_rng = crate::rng::Rng::new(0x5230_3632);
		crate::vmgen::strseal::seal_literals(&mut block, &mut seal_rng);
		let (seed, step) = match &block.stmts[0] {
			Stmt::Return(values) => match &values[0] {
				Expr::SealedStr { seed, step, .. } => (*seed, *step),
				other => panic!("expected sealed literal, got {other:?}"),
			},
			other => panic!("expected return, got {other:?}"),
		};
		let expected_cipher = crate::vmgen::strseal::encode(plain, seed, step);
		let mut rng = crate::rng::Rng::new(77);
		let program = compile_chunk(&block, &table, &mut rng, false, true, false, false, true, false);
		assert!(program.has_loadstr);
		let strings = sealed_pool_strings(&program, program.fns.len() - 1);
		assert!(strings.iter().any(|s| s == &expected_cipher));
		assert!(!strings.iter().any(|s| s == plain));
	}

	#[test]
	fn orphaned_mkstr_literals_are_pruned_without_dropping_shared_constants() {
		let mut consts = vec![
			Const::Str(b"suda".to_vec()),
			Const::Str(b"key".to_vec()),
			Const::Num(6.0),
		];
		let mut code = vec![
			Instr::abcd(Op::MkStr, 0, 0x7562, 0x6461, 0x0004),
			Instr::ab(Op::GetGlobal, 1, 1),
			Instr::ab(Op::LoadK, 2, 2),
		];
		let candidates = HashSet::from([0u16, 1u16]);
		let removed = prune_dead_short_string_constants(&mut code, &mut consts, &candidates);
		assert_eq!(removed, 1);
		assert_eq!(consts.len(), 2);
		assert!(matches!(&consts[0], Const::Str(v) if v == b"key"));
		assert!(matches!(&consts[1], Const::Num(v) if *v == 6.0));
		assert_eq!(code[1].b, 0, "shared global-name constant remapped");
		assert_eq!(code[2].b, 1, "numeric constant remapped");
	}

	#[test]
#[ignore]
	fn dbg_header() {
		let src = std::fs::read_to_string("/tmp/nt5.lua").unwrap();
		let mut block = parser::parse(&src, false).unwrap();
		let mut table = symtab::resolve(&mut block);
		eprintln!("count_fns={}", count_fns_count(&block));
		if let crate::ast::Stmt::If { thenb, .. } = &block.stmts[0] {
			eprintln!("thenb stmts: {}", thenb.stmts.len());
			for (si, s) in thenb.stmts.iter().enumerate() {
				eprintln!("  thenb[{}] = {:?}", si, std::mem::discriminant(s));
				if let crate::ast::Stmt::Local { values, .. } = s {
					for (vi, v) in values.iter().enumerate() {
						eprintln!("    value[{}] = {:?}", vi, v.as_ref().map(|e| std::mem::discriminant(e)));
					}
				}
			}
		}
		for (si, s) in block.stmts.iter().enumerate() {
			eprintln!("stmt[{}] = {:?}", si, std::mem::discriminant(s));
			if let crate::ast::Stmt::Local { values, .. } = s {
				for (vi, v) in values.iter().enumerate() {
					if let Some(e) = v {
						eprintln!("  value[{}] = {:?}", vi, std::mem::discriminant(e));
						if let crate::ast::Expr::Function { body, .. } = e {
							eprintln!("    fn body stmts: {}", body.stmts.len());
						}
					} else {
						eprintln!("  value[{}] = None", vi);
					}
				}
			}
		}
		let mut rng = crate::rng::Rng::new(42);
		let prog = compile_chunk(&block, &table, &mut rng, true, false, false, false, true, false);
		eprintln!("fns={}", prog.fns.len());
		let names = ["Jmp","Jf","Jt","LoadNil","LoadK","Move","Add","Sub","Mul","Div","Mod","Pow","Concat","Unm","Not","Len","Lt","Le","Gt","Ge","Eq","Ne","Idiv","NewTab","GetTab","SetTab","TabN","CallT","Closure","Call","VarArgTab","VarArgC","VarArgTabN","GetGlobal","SetGlobal","GetUp","SetUp","Return","Nop","CallE","CallM","MkStr","LoadStr"];
		let mut wire2name: Vec<Option<&str>> = vec![None; 256];
		for (i, nm) in names.iter().enumerate() {
			wire2name[prog.opmap.to_wire[0][i] as usize] = Some(*nm);
		}
		for (fi, b) in prog.fns.iter().enumerate() {
			eprintln!("=== FN[{}] len={}", fi, b.len());
			let u16 = |p: usize| (b[p] as u16) | ((b[p + 1] as u16) << 8);
			let mut p = 0;
			let nregs = u16(p); p += 2;
			let nparams = u16(p); p += 2;
			let vararg = b[p]; p += 1;
			let nups = u16(p); p += 2;
			eprintln!("nregs={nregs} nparams={nparams} vararg={vararg} nups={nups}");
			p += 2 * nups as usize;
			let nconst = u16(p); p += 2;
			for i in 0..nconst as usize {
				let t = b[p]; p += 1;
				match t {
					0 => eprintln!("  C[{i}] = nil"),
					1 => { eprintln!("  C[{i}] = bool {}", b[p] == 1); p += 1; }
					_ => {
						let l = u16(p) as usize; p += 2;
						let txt = String::from_utf8_lossy(&b[p..p + l]).to_string();
						let kind = if t == 2 { "num" } else { "str" };
						eprintln!("  C[{i}] = {kind} {:?}", &txt[..txt.len().min(20)]);
						p += l;
					}
				}
			}
			let perm = &prog.slot_perms[fi];
			let ncode = u16(p) as usize;
			p += 2;
			let mut ops: Vec<u8> = Vec::with_capacity(ncode);
			for _ in 0..ncode {
				ops.push(b[p]);
				p += 1;
			}
			let mut streams = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
			for s in 0..4 {
				for _ in 0..ncode {
					let (v, np) = isa::decode_varint(b, p).expect("varint");
					streams[s].push(v as u16);
					p = np;
				}
			}
			for i in 0..ncode {
				let nm = wire2name[ops[i] as usize].unwrap_or("??");
				let mut vals = [0u16; 4];
				for s in 0..4 {
					vals[perm[s] as usize] = streams[s][i];
				}
				eprintln!(
					"  [{i}] {nm} a={} b={} c={} d={}",
					vals[0], vals[1], vals[2], vals[3]
				);
			}
		}
	}

	#[test]
	fn call_window_mask_covers_call_to_consumer() {
		let code = [
			Instr::op(Op::Nop),
			Instr::abcd(Op::Call, 0, 0, 255, 1),
			Instr::op(Op::Nop),
			Instr::abcd(Op::Return, 0, 255, 0, 0),
		];
		assert_eq!(call_window_mask(&code), [false, false, true, true]);
	}

	#[test]
	fn call_window_mask_chains_consumers_and_closes_on_return() {
		let code = [
			Instr::abcd(Op::CallE, 0, 0, 0, 0),
			Instr::abcd(Op::CallM, 0, 1, 255, 0),
			Instr::abcd(Op::Return, 0, 255, 0, 0),
			Instr::op(Op::Nop),
		];
		assert_eq!(call_window_mask(&code), [false, true, true, false]);
	}

	#[test]
	fn call_window_mask_empty_without_calls() {
		let code = [
			Instr::abcd(Op::LoadK, 0, 0, 0, 0),
			Instr::abcd(Op::Add, 0, 0, 0, 0),
			Instr::abcd(Op::Return, 0, 0, 0, 0),
		];
		assert_eq!(call_window_mask(&code), [false, false, false]);
	}

	#[test]
	fn call_window_mask_callt_closes_but_never_opens() {
		let code = [
			Instr::abcd(Op::Call, 0, 0, 255, 0),
			Instr::abcd(Op::CallT, 0, 0, 0, 0),
			Instr::op(Op::Nop),
		];
		assert_eq!(call_window_mask(&code), [false, true, false]);
	}

	#[test]
	fn scatter_layout_properties() {
		let src = "local function f(a, b, c) local x = a + b * c; local y = x - a; return y, x end return f(1, 2, 3)";
		let mut block = parser::parse(src, true).unwrap();
		let table = symtab::resolve(&mut block);
		let mut rng = crate::rng::Rng::new(7);
		let prog = compile_chunk(&block, &table, &mut rng, false, true, false, false, true, false);
		let mut saw_scattered_operand = false;
		let tags = &prog.proto_layouts;
		for (fi, b) in prog.fns.iter().enumerate() {
			let base = (prog.blob_seed as u64 + fi as u64 * prog.blob_step as u64) % 268_435_456;
			let ksplit = b.len().min(64);
			let mut buf: Vec<u8> = b.clone();
			let mut state = base;
			let mut prev: u64 = 0;
			for byte in buf.iter_mut().take(ksplit) {
				state = (prog.blob_km as u64 * state + prog.blob_kc as u64 + prev)
					% 268_435_456;
				*byte = byte.wrapping_sub(isa::fold_key_nl(state));
				prev = *byte as u64;
			}
			let mut hf: u64 = 0;
			for &byte in b.iter().take(ksplit) {
				hf = (hf * 31 + byte as u64) % 268_435_456;
			}
			let mut state = (base + hf) % 268_435_456;
			for byte in buf.iter_mut().skip(ksplit) {
				state = (prog.blob_km as u64 * state + prog.blob_kc as u64 + prev)
					% 268_435_456;
				*byte = byte.wrapping_sub(isa::fold_key_nl(state));
				prev = *byte as u64;
			}
			let mut p = 0usize;
			let mut nregs = 0usize;
			let mut slots: Vec<u16> = Vec::new();
			let mut ncode = 0usize;
			let mut streams = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
			let mut got = 0;
			let tags = tags[fi].as_tags();
			while got < 6 {
				let tag = buf[p];
				p += 1;
				assert_ne!(tag, tags[6], "unread padding sections must not be emitted");
				if tag == tags[0] {
					let (v, np) = isa::decode_varint(&buf, p).unwrap();
					nregs = v as usize;
					p = np;
					let (_, np) = isa::decode_varint(&buf, p).unwrap();
p = np;
					let (_, np) = isa::decode_varint(&buf, p).unwrap();
p = np;
					got += 1;
				} else if tag == tags[1] {
					let (_, np) = isa::decode_varint(&buf, p).unwrap();
p = np;
					got += 1;
				} else if tag == tags[2] {
					let (mut nu, np) = isa::decode_varint(&buf, p).unwrap();
					p = np;
					while nu > 0 {
						let (_, n2) = isa::decode_varint(&buf, p).unwrap();
						p = n2;
						nu -= 1;
					}
					got += 1;
				} else if tag == tags[3] {
					let (_, np) = isa::decode_varint(&buf, p).unwrap();
p = np;
					let (csl, np) = isa::decode_varint(&buf, p).unwrap();
					p = np + csl as usize;
					got += 1;
				} else if tag == tags[4] {
					let (mut ns, np) = isa::decode_varint(&buf, p).unwrap();
					p = np;
					slots.clear();
					while ns > 0 {
						let (v, n2) = isa::decode_varint(&buf, p).unwrap();
						slots.push(v as u16);
						p = n2;
						ns -= 1;
					}
					got += 1;
				} else {
					if tag != tags[5] {
						panic!(
							"unknown section tag {tag} at p={} got={} tags={:?}",
							p, got, tags
						);
					}
					let (v, np) = isa::decode_varint(&buf, p).unwrap();
					ncode = v as usize;
					p = np;
p += ncode;
					for s in 0..4 {
						for _ in 0..ncode {
							let (sv, n2) = isa::decode_varint(&buf, p).unwrap();
							streams[s].push(sv as u16);
							p = n2;
						}
					}
					got += 1;
				}
			}
			assert_eq!(slots.len(), nregs, "S table length must equal nregs");
			let mut sorted = slots.clone();
			sorted.sort_unstable(); sorted.dedup();
			assert_eq!(sorted.len(), nregs, "S slots must be distinct");
			for &s in &slots {
				assert!((1..=4095).contains(&s), "slot in H3 sparse range");
			}
			if nregs > 0 {
				assert!(
					slots.iter().any(|&s| s >= 96),
					"H3 must place some real registers in the high noise-like range"
				);
			}
			let perm = &prog.slot_perms[fi];
			for i in 0..ncode {
				let mut vals = [0u16; 4];
				for s in 0..4 {
					vals[perm[s] as usize] = streams[s][i];
				}
				if vals.iter().any(|&v| v as usize > nregs) {
					saw_scattered_operand = true;
				}
			}
		}
		assert!(saw_scattered_operand, "some wire operand must be scattered");
	}

	#[test]
	fn upvalue_permutation_is_bijective_and_seed_stable() {
		for count in 0..64 {
			let p = derive_upvalue_permutation(0x1234_5678_9abc_def0, 17, count, true);
			assert_eq!(p.len(), count);
			let mut sorted = p.clone();
			sorted.sort_unstable();
			assert_eq!(sorted, (0..count).collect::<Vec<_>>());
			assert_eq!(p, derive_upvalue_permutation(0x1234_5678_9abc_def0, 17, count, true));
			assert_eq!(derive_upvalue_permutation(0, 2, count, false), (0..count).collect::<Vec<_>>());
		}
	}
}

#[cfg(test)]
mod p1_cache {
	use super::*;
	use crate::parser;
	use crate::vmgen::strseal;

	fn seal(src: &str) -> Block {
		let mut block = parser::parse(src, true).expect("test chunk parses");
		strseal::seal_literals(&mut block, &mut Rng::new(4242));
		block
	}

	/// P1-1 验收 1：10 个字符串常量，7 个单次引用、3 个多次引用。
	/// 单次引用常量必须保持裸索引（不写缓存表）；多次引用常量必须带 cached 位。
	#[test]
	fn single_reference_constants_never_enter_the_cache() {
		let mut src = String::new();
		for i in 0..7 {
			src.push_str(&format!("local s{i} = \"SINGLE_USE_MARKER_{i:02}\"\n"));
		}
		for i in 0..3 {
			src.push_str(&format!(
				"print(\"MULTI_USE_MARKER_{i:02}\", \"MULTI_USE_MARKER_{i:02}\", \"MULTI_USE_MARKER_{i:02}\")\n"
			));
		}
		let mut block = seal(&src);
		// Materialise the sealed literals the way the compiler does.
		let mut code: Vec<Instr> = Vec::new();
		let mut consts: Vec<Const> = Vec::new();
		let mut idx: HashMap<Vec<u8>, u16> = HashMap::new();
		let mut push = |bytes: &[u8], seed: u16, step: u16, code: &mut Vec<Instr>, consts: &mut Vec<Const>| {
			let cipher = strseal::encode(bytes, seed, step);
			let k = *idx.entry(cipher.clone()).or_insert_with(|| {
				let k = consts.len() as u16;
				consts.push(Const::Str(cipher.clone()));
				k
			});
			code.push(Instr::abcd(Op::LoadStr, 0, k, seed, step));
		};
		for stmt in &block.stmts {
			if let Stmt::Local { values, .. } = stmt {
				for v in values.iter().flatten() {
					if let Expr::SealedStr { bytes, seed, step } = v {
						push(bytes, *seed, *step, &mut code, &mut consts);
					}
				}
			}
			if let Stmt::ExprStmt(Expr::Call { args, .. }) = stmt {
				for a in args {
					if let Expr::SealedStr { bytes, seed, step } = a {
						push(bytes, *seed, *step, &mut code, &mut consts);
					}
				}
			}
		}
		let clearable = strseal::sealed_clearable(&block);
		let (cached, cleared) = loadstr_flags(&mut code, &consts, &clearable);
		assert_eq!(cached, 9, "3 multi-use constants x 3 references");
		assert_eq!(cleared, 3, "each multi-use constant clears after its last use");
		let flagged_single = code
			.iter()
			.filter(|ins| ins.op == Op::LoadStr && ins.b & 0x8000 != 0)
			.count();
		assert_eq!(flagged_single, 9, "every reference of a multi-use constant is flagged");
		let unflagged = code
			.iter()
			.filter(|ins| ins.op == Op::LoadStr && ins.b < 0x4000)
			.count();
		assert_eq!(unflagged, 7, "the 7 single-use constants stay out of the cache");
		let cleared_flags = code
			.iter()
			.filter(|ins| ins.op == Op::LoadStr && ins.b & 0x4000 != 0)
			.count();
		assert_eq!(cleared_flags, 3);
	}

	/// P1-2 范围声明：循环体内的引用不允许清空（可能再次被执行）。
	#[test]
	fn constants_referenced_inside_a_loop_are_never_cleared() {
		let block = seal("local s = \"LOOP_MARKER\"\nfor i = 1, 3 do print(s, s) end");
		let clearable = strseal::sealed_clearable(&block);
		assert!(clearable.is_empty(), "loop back-edge must veto clearing");
	}

	/// P1-2 范围声明：被内层闭包引用（跨函数）的常量不允许清空。
	#[test]
	fn constants_referenced_from_a_nested_closure_are_never_cleared() {
		let block = seal(
			"local function outer() return \"CAPTURE_MARKER\" end\nlocal function inner() return \"CAPTURE_MARKER\" end\nprint(outer(), inner())",
		);
		let total = strseal::sealed_total_counts(&block);
		assert_eq!(total.len(), 1, "identical plaintext dedups to one constant");
		assert_eq!(total.values().next().copied(), Some(2));
		let clearable = strseal::sealed_clearable(&block);
		assert!(
			clearable.is_empty(),
			"a constant referenced from two different frames must veto clearing"
		);
	}

	/// P1-2 正向用例：同一函数、两次引用、无循环、无闭包 → 可清空。
	#[test]
	fn constants_used_twice_in_a_loop_free_function_are_clearable() {
		let block = seal("local s = \"CLEAR_ME\"\nprint(s, s)");
		let clearable = strseal::sealed_clearable(&block);
		assert_eq!(clearable.len(), 1, "exactly the loop-free multi-use constant");
	}

	/// P1-1 前置：相同明文必须共享同一个密封常量，否则多次引用无从谈起。
	#[test]
fn identical_plaintexts_share_one_sealed_constant() {
		let block = seal("local a = \"SAME\"\nlocal b = \"SAME\"\nprint(a, b)");
		let mut seeds = Vec::new();
		for stmt in &block.stmts {
			if let Stmt::Local { values, .. } = stmt {
				for v in values.iter().flatten() {
					if let Expr::SealedStr { seed, step, .. } = v {
						seeds.push((*seed, *step));
					}
				}
			}
		}
		assert_eq!(seeds.len(), 2);
		assert_eq!(seeds[0], seeds[1], "identical plaintext must dedup");
	}
}

/// Phase 18 P0-3 场景 E / P4 审查补充：C5（`child_index >= fns.len()`）的前置条件是
/// `count_fns_count` 漏计某个会被 `compile_function` 编译成子函数的 AST 节点。本用例把
/// 闭包放进 `count_stmt` / `count_expr` 枚举到的**每一个**语法位置，断言
/// 计数 == 实际编译出的子函数槽数（`program.fns.len() - 1`，main 占最后一槽）。
/// 同时钉住 P4 审查发现的那条输入可达 panic：函数体内的 `break`/`continue`
/// 不跨越函数边界，必须在解析阶段干净拒绝。
#[cfg(test)]
mod p0_c5_slots {
	use super::*;
	use crate::parser;
	use crate::symtab;
	use crate::vmgen::strseal;

	const POSITIONS: &str = "local function lf() return 1 end
local a = function() return 2 end
local obj = {}
function obj:m(x) return function() return x end end
function obj.f() return 3 end
local t = { function() return 4 end, k = function() return 5 end, [1] = function() return 6 end }
local key = (function() return 9 end)()
t[key] = 9
print(function() return 7 end)
local r = (function() return 8 end)()
local x = true and function() return 10 end or 1
local y = not function() return false end
local i = t[function() return 1 end]
local z = a and (function() return 1 end) or function() return 2 end
local acc = 0
if (function() return true end)() then acc = acc + (function() return 1 end)()
elseif (function() return false end)() then acc = acc + (function() return 2 end)()
else acc = acc + (function() return 3 end)() end
while (function() return false end)() do local w = function() end end
local u
repeat u = function() acc = acc + 4 end until (function() return true end)()
u()
for i = (function() return 1 end)(), (function() return 2 end)(), (function() return 1 end)() do local b = function() return i end end
for k, v in (function() return pairs({}) end)() do local c = function() end end
do local d = function() acc = acc + 5 end d() end
local e = (\"x\"):rep(1, (function() return \"\" end)())
local m = (\"abc\"):sub((function() return 1 end)(), 2)
local n = t.a
local q = if x then function() return 11 end else function() return 12 end
local up = 6
local function o() return function() return function() return up end end end
local s = \"x\" .. (function() return \"y\" end)()
print(lf(), a(), obj.f(), obj:m(7)(), r, i, z, acc, e, m, q(), o()()(), s)
";

	fn counted_vs_compiled(src: &str) -> (usize, usize) {
		// 与 main.rs 的流水线同序：parse -> symtab -> seal -> vmgen::compile。
		let mut block = parser::parse(src, true).expect("test chunk parses");
		let table = symtab::resolve(&mut block);
		strseal::seal_literals(&mut block, &mut Rng::new(4242));
		let counted = count_fns_count(&block);
		let program = compile(&block, &table, &mut Rng::new(42), false, true, false, true, true, false);
		(counted, program.fns.len() - 1)
	}

	#[test]
	fn c5_count_fns_matches_compiled_closure_slots() {
		let (counted, slots) = counted_vs_compiled(POSITIONS);
		// 语料里没有注释/字符串包含 "function"，关键字个数就是闭包个数。
		assert_eq!(POSITIONS.matches("function").count(), 40);
		assert_eq!(counted, 40, "count_fns_count must see every closure");
		assert_eq!(counted, slots, "count must equal the number of compiled closure slots");
	}

	#[test]
	fn c5_break_or_continue_inside_function_inside_loop_is_a_parse_error() {
		let cases = [
			"while true do local function f() break end end",
			"while true do local f = function() break end end",
			"for i=1,2 do local f = function() continue end end",
			"repeat local function f() break end until false",
			"for k,v in pairs({}) do local function f() break end end",
			"while true do pcall(function() break end) end",
			"while true do local f = function() return function() break end end end",
			"while true do t = { function() continue end } end",
			"while true do local x = true and function() break end or 1 end",
		];
		for src in cases {
			match parser::parse(src, true) {
				Err(e) => assert!(e.msg.contains("outside of a loop"), "{src} -> {}", e.msg),
				Ok(_) => panic!("must be rejected by the parser: {src}"),
			}
		}
		// 对照：合法位置（循环体内、非函数体）仍然接受。
		assert!(parser::parse("for i=1,2 do if i==2 then break end end", true).is_ok());
		assert!(parser::parse("for i=1,2 do if i==2 then continue end end", true).is_ok());
		assert!(parser::parse("while true do local f = function() return 1 end f() break end", true).is_ok());
	}
}

