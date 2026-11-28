
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
	Jmp,
	Jf,
	Jt,
	LoadNil,
	LoadK,
	Move,
	Add,
	Sub,
	Mul,
	Div,
	Mod,
	Pow,
	Concat,
	Unm,
	Not,
	Len,
	Lt,
	Le,
	Gt,
	Ge,
	Eq,
	Ne,
	Idiv,
	NewTab,
	GetTab,
	SetTab,
	TabN,
	CallT,
	Closure,
	Call,
	VarArgTab,
	VarArgC,
	VarArgTabN,
	GetGlobal,
	SetGlobal,
	GetUp,
	SetUp,
	Return,
	Nop,
	CallE,
	CallM,
	MkStr,
	LoadStr,
	Canary,
}

pub const N_OPS: usize = 44;

pub fn op_base() -> [u8; N_OPS] {
	#[allow(clippy::declare_interior_mutable_const)]
	#[rustfmt::skip]
	let t: [(Op, u8); N_OPS] = [
		(Op::Jmp, 0), (Op::Jf, 1), (Op::Jt, 2), (Op::LoadNil, 3),
		(Op::LoadK, 4), (Op::Move, 5), (Op::Add, 6), (Op::Sub, 7),
		(Op::Mul, 8), (Op::Div, 9), (Op::Mod, 10), (Op::Pow, 11),
		(Op::Concat, 12), (Op::Unm, 13), (Op::Not, 14), (Op::Len, 15),
		(Op::Lt, 16), (Op::Le, 17), (Op::Gt, 18), (Op::Ge, 19),
		(Op::Eq, 20), (Op::Ne, 21), (Op::Idiv, 22), (Op::NewTab, 23),
		(Op::GetTab, 24), (Op::SetTab, 25), (Op::TabN, 26), (Op::CallT, 27),
		(Op::Closure, 28), (Op::Call, 29), (Op::VarArgTab, 30), (Op::VarArgC, 31),
		(Op::VarArgTabN, 32), (Op::GetGlobal, 33), (Op::SetGlobal, 34),
		(Op::GetUp, 35), (Op::SetUp, 36), (Op::Return, 37), (Op::Nop, 38),
		(Op::CallE, 39), (Op::CallM, 40),
		(Op::MkStr, 41), (Op::Canary, 42), (Op::LoadStr, 43),
	];
	let mut out = [0u8; N_OPS];
	for (op, code) in t {
		let idx = op_index(op);
		out[idx] = code;
	}
	out
}

pub fn op_index(op: Op) -> usize {
	match op {
		Op::Jmp => 0,
		Op::Jf => 1,
		Op::Jt => 2,
		Op::LoadNil => 3,
		Op::LoadK => 4,
		Op::Move => 5,
		Op::Add => 6,
		Op::Sub => 7,
		Op::Mul => 8,
		Op::Div => 9,
		Op::Mod => 10,
		Op::Pow => 11,
		Op::Concat => 12,
		Op::Unm => 13,
		Op::Not => 14,
		Op::Len => 15,
		Op::Lt => 16,
		Op::Le => 17,
		Op::Gt => 18,
		Op::Ge => 19,
		Op::Eq => 20,
		Op::Ne => 21,
		Op::Idiv => 22,
		Op::NewTab => 23,
		Op::GetTab => 24,
		Op::SetTab => 25,
		Op::TabN => 26,
		Op::CallT => 27,
		Op::Closure => 28,
		Op::Call => 29,
		Op::VarArgTab => 30,
		Op::VarArgC => 31,
		Op::VarArgTabN => 32,
		Op::GetGlobal => 33,
		Op::SetGlobal => 34,
		Op::GetUp => 35,
		Op::SetUp => 36,
		Op::Return => 37,
		Op::Nop => 38,
		Op::CallE => 39,
		Op::CallM => 40,
		Op::MkStr => 41,
		Op::Canary => 42,
		Op::LoadStr => 43,
	}
}

/// op_index 的反查（V17 fused 词汇回映射用；i ∈ [0, N_OPS)）。
pub fn op_from_index(i: u8) -> Op {
	const REV: [Op; N_OPS] = [
		Op::Jmp, Op::Jf, Op::Jt, Op::LoadNil, Op::LoadK, Op::Move, Op::Add, Op::Sub,
		Op::Mul, Op::Div, Op::Mod, Op::Pow, Op::Concat, Op::Unm, Op::Not, Op::Len,
		Op::Lt, Op::Le, Op::Gt, Op::Ge, Op::Eq, Op::Ne, Op::Idiv, Op::NewTab,
		Op::GetTab, Op::SetTab, Op::TabN, Op::CallT, Op::Closure, Op::Call,
		Op::VarArgTab, Op::VarArgC, Op::VarArgTabN, Op::GetGlobal, Op::SetGlobal,
		Op::GetUp, Op::SetUp, Op::Return, Op::Nop, Op::CallE, Op::CallM,
		Op::MkStr, Op::Canary, Op::LoadStr,
	];
	REV[i as usize]
}

#[derive(Debug, Clone)]
pub struct OpMap {
	pub to_wire: [[u8; N_OPS]; 4],
	pub nop_alias: [u8; 4],
	pub s_values: [u8; 4],
}

impl OpMap {
	pub fn new(rng: &mut crate::rng::Rng) -> OpMap {
		Self::new_with_count(rng, N_OPS)
	}

	pub fn new_with_count(rng: &mut crate::rng::Rng, active_count: usize) -> OpMap {
		// invariant: the two callers pass isa::N_OPS or isa::N_OPS - 1 (compile-time constants)
		assert!(active_count > 0 && active_count <= N_OPS);
		let mut to_wire = [[0u8; N_OPS]; 4];
		let mut nop_alias = [0u8; 4];
		for t in 0..4 {
			let base = (t * 64) as u8;
			let mut perm: Vec<u8> = (0..active_count as u8).map(|x| x.wrapping_add(base)).collect();
			for i in (1..perm.len()).rev() {
				let j = rng.int(0, i as i64) as usize;
				perm.swap(i, j);
			}
			let mut used = [false; 256];
			for &w in perm.iter() {
				used[w as usize] = true;
			}
			let mut alias = base.wrapping_add(active_count as u8);
			for w in 0..256u16 {
				let ww = base.wrapping_add(w as u8);
				if !used[ww as usize] {
					alias = ww;
					break;
				}
			}
			for i in 0..active_count {
				to_wire[t][i] = perm[i];
			}
			for i in active_count..N_OPS {
				to_wire[t][i] = alias;
			}
			nop_alias[t] = alias;
		}
		OpMap {
			to_wire,
			nop_alias,
			s_values: [251, 99, 8, 140],
		}
	}

	pub fn code(&self, op: Op) -> u8 {
		self.to_wire[0][op_index(op)]
	}

	pub fn code_for(&self, op: Op, s_idx: usize) -> u8 {
		self.to_wire[s_idx][op_index(op)]
	}

	/// V17 P1-b：fused trampoline 码位 = 方言 base + 48 + k。
	/// （per-dialect 区间 64 码位：基础 op 至多 44（base+0..43）+ nop_alias
	/// 至多 base+44（active_count=44 时）→ tramp 占 [base+48, base+52)，
	/// 与 alias 恒不相交；旧布局 base+44+k 在 active_count=44 时与
	/// nop_alias 同槽，tramp(0) 被 hw_expand 的 alias 拷贝静默覆盖。）
	pub fn fused_tramp_code(s_idx: usize, k: usize) -> u8 {
		(s_idx * 64 + 48 + k) as u8
	}

	/// V17 P1-b：tail-less 融合件 handler 码位 = 方言 base + 52 + j，
	/// j 为 FUSE_TAIL_OPS 内序号（不带 chain_epilogue 的版本，专供 trampoline 内嵌调用）。
	pub fn fused_tail_code(s_idx: usize, j: usize) -> u8 {
		(s_idx * 64 + 52 + j) as u8
	}

	pub fn codes_text(&self) -> String {
		let mut s = String::new();
		for i in 0..N_OPS {
			if i > 0 {
				s.push(',');
			}
			s.push_str(&self.to_wire[0][i].to_string());
		}
		s
	}

	pub fn codes_text_for(&self, s_idx: usize) -> String {
		let mut s = String::new();
		for i in 0..N_OPS {
			if i > 0 {
				s.push(',');
			}
			s.push_str(&self.to_wire[s_idx][i].to_string());
		}
		s
	}
}


#[derive(Debug, Clone, Copy)]
pub struct Instr {
	pub op: Op,
	pub a: u16,
	pub b: u16,
	pub c: u16,
	pub d: u16,
}

impl Instr {
	pub fn new(op: Op, a: u16, b: u16, c: u16, d: u16) -> Instr {
		Instr { op, a, b, c, d }
	}
	pub fn op(op: Op) -> Instr {
		Instr {
			op,
			a: 0,
			b: 0,
			c: 0,
			d: 0,
		}
	}
	pub fn ab(op: Op, a: u16, b: u16) -> Instr {
		Instr {
			op,
			a,
			b,
			c: 0,
			d: 0,
		}
	}
	pub fn abc(op: Op, a: u16, b: u16, c: u16) -> Instr {
		Instr {
			op,
			a,
			b,
			c,
			d: 0,
		}
	}
	pub fn abcd(op: Op, a: u16, b: u16, c: u16, d: u16) -> Instr {
		Instr { op, a, b, c, d }
	}
	pub fn operands(&self) -> [u16; 4] {
		[self.a, self.b, self.c, self.d]
	}
}

pub fn push_u16(out: &mut Vec<u8>, v: u16) {
	out.push((v & 0xff) as u8);
	out.push((v >> 8) as u8);
}

pub fn encode_varint(out: &mut Vec<u8>, v: u32) {
	if v < 128 {
		out.push(v as u8);
	} else if v < 16_384 {
		out.push(((v % 128) + 128) as u8);
		out.push((v / 128) as u8);
	} else if v < 2_097_152 {
		out.push(((v % 128) + 128) as u8);
		out.push((((v / 128) % 128) + 128) as u8);
		out.push((v / 16_384) as u8);
	} else {
		out.push(((v % 128) + 128) as u8);
		out.push((((v / 128) % 128) + 128) as u8);
		out.push((((v / 16_384) % 128) + 128) as u8);
		out.push((v / 2_097_152) as u8);
	}
}

pub fn encode_u16var(out: &mut Vec<u8>, v: u16) {
	encode_varint(out, v as u32);
}

pub fn decode_varint(src: &[u8], p: usize) -> Option<(u32, usize)> {
	let b1 = *src.get(p)? as u32;
	if b1 < 128 {
		return Some((b1, p + 1));
	}
	let b2 = *src.get(p + 1)? as u32;
	if b2 < 128 {
		return Some(((b1 - 128) + b2 * 128, p + 2));
	}
	let b3 = *src.get(p + 2)? as u32;
	if b3 < 128 {
		let v = (b1 - 128) + (b2 - 128) * 128 + b3 * 16_384;
		return Some((v, p + 3));
	}
	let b4 = *src.get(p + 3)? as u32;
	let v = (b1 - 128) + (b2 - 128) * 128 + (b3 - 128) * 16_384 + b4 * 2_097_152;
	Some((v, p + 4))
}

pub fn encode_soa(code: &[Instr], map: &OpMap, slot_perm: &[u8; 4], out: &mut Vec<u8>) {
	for ins in code {
		out.push(map.code(ins.op));
	}
	for sl in 0..4 {
		let which = slot_perm[sl] as usize;
		for ins in code {
			encode_u16var(out, ins.operands()[which]);
		}
	}
}

pub fn encode_soa_for(code: &[Instr], map: &OpMap, slot_perm: &[u8; 4], s_idx: usize, out: &mut Vec<u8>) {
	for ins in code {
		out.push(map.code_for(ins.op, s_idx));
	}
	for sl in 0..4 {
		let which = slot_perm[sl] as usize;
		for ins in code {
			encode_u16var(out, ins.operands()[which]);
		}
	}
}


#[derive(Debug, Clone)]
pub enum Const {
	Nil,
	Bool(bool),
	Num(f64),
	Str(Vec<u8>),
}

pub struct ConstLcg {
	pub state: u32,
	pub km: u32,
	pub kc: u32,
	/// V19 R5-b：明文级联——前一明文字节并入推进（镜像 const_loop `mb()` 的 `_cpv`）。
	pub pv: u8,
}

impl ConstLcg {
	pub fn new(seed: u32, km: u32, kc: u32) -> ConstLcg {
		ConstLcg { state: seed, km, kc, pv: 0 }
	}
	pub fn next_key(&mut self) -> u8 {
		self.state = ((self.km as u64 * self.state as u64
			+ self.kc as u64
			+ self.pv as u64)
			% 268_435_456) as u32;
		fold_key_nl(self.state as u64)
	}
	/// 编码侧：出钥 + 级联携带明文。
	pub fn mask(&mut self, plain: u8) -> u8 {
		let k = self.next_key();
		self.pv = plain;
		plain.wrapping_add(k)
	}
	/// 解码侧镜像（测试通路）：出钥、解掩码、级联携带解出的明文。
	pub fn unmask(&mut self, cipher: u8) -> u8 {
		let k = self.next_key();
		let plain = cipher.wrapping_sub(k);
		self.pv = plain;
		plain
	}
}

/// 旧线性四字节求和折叠（仅留作 fold_key_nl 区分度对照与残留引用）。
pub(crate) fn fold_key(state: u64) -> u8 {
	(((state % 256)
		+ ((state / 256) % 256)
		+ ((state / 65536) % 256)
		+ (state / 16777216))
		% 256) as u8
}

/// V19 R5-a（熵坍缩首批）：28-bit 全态非线性折叠（交叉半积 + 高位回混），
/// 镜像为模板里同式 Lua 文本（double 精确域内：最大中间值 ~2^36 < 2^53）。
/// 替代「四字节求和」线性折叠——读写两侧的仿射相消面就此灭绝（L5/R5 口径）。
pub(crate) fn fold_key_nl(state: u64) -> u8 {
	let mut x = (state + (state % 16384) * (state / 16384)) % 268_435_456;
	x = (x * 257 + x / 65536) % 268_435_456;
	(((x % 256) + ((x / 256) % 256) + ((x / 65536) % 256) + (x / 16_777_216)) % 256) as u8
}

pub fn num_to_dyadic(v: f64) -> (bool, u64, i32) {
	let bits = v.to_bits();
	let neg = bits >> 63 != 0;
	let exp = ((bits >> 52) & 0x7FF) as i32;
	let mant = bits & 0x000F_FFFF_FFFF_FFFF;
	if exp == 0 {
		(neg, mant, -1074)
	} else {
		(neg, mant | 0x0010_0000_0000_0000, exp - 1023 - 52)
	}
}

pub fn encode_varint64(out: &mut Vec<u8>, mut v: u64) {
	while v >= 128 {
		out.push(((v % 128) + 128) as u8);
		v /= 128;
	}
	out.push(v as u8);
}

impl Const {
	pub fn encode(&self, out: &mut Vec<u8>, lcg: &mut ConstLcg) {
		match self {
			Const::Nil => out.push(lcg.mask(0)),
			Const::Bool(b) => {
				out.push(lcg.mask(1));
				out.push(lcg.mask(*b as u8));
			}
			Const::Num(v) => {
				if v.is_nan() {
					out.push(lcg.mask(7));
				} else if v.is_infinite() {
					out.push(lcg.mask(if v.is_sign_negative() { 6 } else { 5 }));
				} else {
					out.push(lcg.mask(4));
					let (neg, m, k) = num_to_dyadic(*v);
					let mut mbytes: Vec<u8> = Vec::new();
					encode_varint64(&mut mbytes, m);
					for b in &mbytes {
						out.push(lcg.mask(*b));
					}
					let kpack = (((k + 2048) * 2 + neg as i32) as u64) & 0xFFFF;
					let mut kbytes: Vec<u8> = Vec::new();
					encode_varint64(&mut kbytes, kpack);
					for b in &kbytes {
						out.push(lcg.mask(*b));
					}
				}
			}
			Const::Str(b) => {
				out.push(lcg.mask(3));
				let l = b.len() as u16;
				out.push(lcg.mask((l & 0xFF) as u8));
				out.push(lcg.mask((l >> 8) as u8));
				for byte in b {
					out.push(lcg.mask(*byte));
				}
			}
		}
	}
}


pub const CARRIER_SPECIALS: [u8; 10] = [
	b'"', b'\'', b'%', b' ', b'$', b'!', b'~', b'#', b'}', b'&',
];

#[derive(Debug, Clone)]
pub struct Carrier {
	pub alphabet: [u8; 94],
	pub reserved: u8,
	pub tokens: [String; 10],
}

impl Carrier {
	pub fn new(rng: &mut crate::rng::Rng) -> Carrier {
		let mut glyphs: Vec<u8> = (32u8..=126).collect();
		rng.shuffle(&mut glyphs);
		let reserved = glyphs[0];
		// invariant: glyphs is a fixed 95-byte static table, so glyphs[1..] has exactly 94 bytes
		let alphabet: [u8; 94] = glyphs[1..].try_into().unwrap();
		let mut tokens = dummy_tokens();
		for i in 0..10 {
			loop {
				let t = gen_token(rng, reserved);
				if tokens[..i].iter().any(|x| x == &t) {
					continue;
				}
				tokens[i] = t;
				break;
			}
		}
		Carrier {
			alphabet,
			reserved,
			tokens,
		}
	}

	pub fn encode(&self, data: &[u8]) -> Vec<u8> {
		let n = data.len() as u32;
		let mut src = Vec::with_capacity(data.len() + 8);
		src.extend_from_slice(&n.to_le_bytes());
		src.extend_from_slice(data);
		while src.len() % 4 != 0 {
			src.push(0);
		}
		let mut digits = Vec::with_capacity(src.len() / 4 * 5);
		for chunk in src.chunks(4) {
			let mut v = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
			let mut d = [0u8; 5];
			for i in 0..5 {
				d[4 - i] = self.alphabet[(v % 94) as usize];
				v /= 94;
			}
			digits.extend_from_slice(&d);
		}
		let mut out = Vec::with_capacity(digits.len() + 32);
		for &b in &digits {
			if let Some(idx) = CARRIER_SPECIALS.iter().position(|&s| s == b) {
				out.extend_from_slice(self.tokens[idx].as_bytes());
			} else {
				out.push(b);
			}
		}
		out
	}

	pub fn decode(&self, enc: &[u8]) -> Option<Vec<u8>> {
		let mut s = Vec::with_capacity(enc.len());
		let mut p = 0;
		while p < enc.len() {
			if enc[p] == self.reserved {
				if p + 5 > enc.len() {
					return None;
				}
				let tok = &enc[p..p + 5];
				let mut hit = None;
				for i in 0..10 {
					if self.tokens[i].as_bytes() == tok {
						hit = Some(CARRIER_SPECIALS[i]);
						break;
					}
				}
				s.push(hit?);
				p += 5;
			} else {
				s.push(enc[p]);
				p += 1;
			}
		}
		if s.len() % 5 != 0 {
			return None;
		}
		let mut inv = [255u8; 256];
		for (i, &ch) in self.alphabet.iter().enumerate() {
			inv[ch as usize] = i as u8;
		}
		let mut raw = Vec::with_capacity(s.len() / 5 * 4);
		for chunk in s.chunks(5) {
			let mut v: u32 = 0;
			for &ch in chunk {
				let d = inv[ch as usize];
				if d == 255 {
					return None;
				}
				v = v.checked_mul(94)?.checked_add(d as u32)?;
			}
			raw.extend_from_slice(&v.to_le_bytes());
		}
		if raw.len() < 4 {
			return None;
		}
		let n = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
		if raw.len() < 4 + n {
			return None;
		}
		Some(raw[4..4 + n].to_vec())
	}
}

fn dummy_tokens() -> [String; 10] {
	std::array::from_fn(|i| format!("Tk{i:03}"))
}

fn gen_token(rng: &mut crate::rng::Rng, reserved: u8) -> String {
	const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
	let mut s = String::with_capacity(5);
	s.push(reserved as char);
	for _ in 0..4 {
		s.push(C[rng.int(0, (C.len() - 1) as i64) as usize] as char);
	}
	s
}

pub fn num_text(v: f64) -> String {
	if v.is_nan() {
		return "0.0/0.0".to_string();
	}
	if v.is_infinite() {
		return if v < 0.0 {
			"-math.huge".to_string()
		} else {
			"math.huge".to_string()
		};
	}
	if v.fract() == 0.0 && v.abs() < 1e15 {
		format!("{}", v as i64)
	} else {
		format!("{:?}", v)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn varint_roundtrip_tiers() {
		let samples = [
			0u32, 1, 127, 128, 255, 16_383, 16_384, 65_535, 100_000, 2_097_151,
			2_097_152, 10_000_000,
		];
		for &v in &samples {
			let mut o = Vec::new();
			encode_varint(&mut o, v);
			let (got, n) = decode_varint(&o, 0).unwrap();
			assert_eq!(got, v, "v={v} bytes={o:?}");
			assert_eq!(n, o.len());
			let expect_len = if v < 128 {
				1
			} else if v < 16_384 {
				2
			} else if v < 2_097_152 {
				3
			} else {
				4
			};
			assert_eq!(o.len(), expect_len, "tier len v={v}");
		}
	}

	#[test]
	fn carrier_roundtrip_many() {
		let mut rng = crate::rng::Rng::new(42);
		let c = Carrier::new(&mut rng);
		for &msg in &[
			&b""[..],
			b"a",
			b"hello",
			&[0u8, 1, 2, 255, 128, 10, 13][..],
			&(0u8..=255).collect::<Vec<u8>>(),
		] {
			let enc = c.encode(msg);
			assert!(
				!enc.contains(&c.reserved) || enc.windows(5).any(|w| w[0] == c.reserved),
				"reserved only as token prefix"
			);
			assert_eq!(c.decode(&enc).as_deref(), Some(msg));
		}
		let mut rng2 = crate::rng::Rng::new(7);
		let c2 = Carrier::new(&mut rng2);
		let a = c.encode(b"payload-xyz");
		let b = c2.encode(b"payload-xyz");
		assert_ne!(a, b);
	}

	#[test]
	fn fold_key_nl_pinned() {
		// 手工复核（2026-10-25）：与模板 unmask_pre 同式 double 语义。
		assert_eq!(fold_key_nl(0), 0);
		assert_eq!(fold_key_nl(1), 2); // x=1 → 257 → 1+1
		assert_eq!(fold_key_nl(16384), 128); // x=16384 → 4210688 → 0+64+64+0
		assert_eq!(fold_key_nl(268_435_455), {
			// 手算：state=2^28-1 ⇒ state%16384=16383, state/16384=16383
			// x=(2^28-1+16383*16383)%2^28 → 逐位在下面展开核对
			let x1 = (268_435_455u64 + 16383 * 16383) % 268_435_456;
			let x2 = (x1 * 257 + x1 / 65536) % 268_435_456;
			(((x2 % 256) + ((x2 / 256) % 256) + ((x2 / 65536) % 256) + (x2 / 16_777_216)) % 256)
				as u8
		});
		// 与旧线性折叠的区分度（抽样 4096 态不得恒等且非仿射）
		let mut diff = 0;
		for g in 0..4096u64 {
			if fold_key_nl(g) != fold_key(g) {
				diff += 1;
			}
		}
		assert!(diff > 3000, "nonlinear fold must diverge broadly, diff={diff}");
	}

	#[test]
	fn op_meta_covers_all_44_ops_consistently() {
		assert_eq!(ALL_OPS.len(), N_OPS);
		for (i, &op) in ALL_OPS.iter().enumerate() {
			assert_eq!(op_index(op), i);
			assert_eq!(Op::from_index(i), Some(op));
			let m = op.meta();
			assert_eq!(m.op, op);
			assert_eq!(m.name, OP_NAMES[i]);
			assert_eq!(Op::from_name(m.name), Some(op));
			// Any opcode that alters PC non-linearly, calls host functions, or is
			// multi-return sensitive must NEVER be marked fuse_head_ok or fuse_tail_ok.
			if m.pc_effect != PcEffect::Fallthrough || m.may_call_host || m.multiret_sensitive {
				assert!(
					!m.fuse_head_ok && !m.fuse_tail_ok,
					"unsafe fuse flag on {}",
					m.name
				);
			}
		}
		assert_eq!(Op::from_index(N_OPS), None);
	}

	#[test]
	fn unused_lanes_are_derived_from_roles_for_every_op() {
		for &op in ALL_OPS.iter() {
			let m = op.meta();
			let roles = m.roles();
			let unused = m.unused_lanes();
			for lane in 0..4 {
				// The derivation itself: unused iff role is Unused.
				assert_eq!(
					unused[lane],
					roles[lane] == OperandRole::Unused,
					"{op:?} lane {lane}"
				);
				// Scatter consistency: unused lanes are never remapped;
				// register lanes always go through the register sigma.
				match roles[lane] {
					OperandRole::Unused => {
						assert_eq!(
							roles[lane].scatter(),
							ScatterMap::Identity,
							"{op:?} lane {lane}"
						)
					}
					OperandRole::RegRead | OperandRole::RegWrite | OperandRole::RegRange => {
						assert_eq!(
							roles[lane].scatter(),
							ScatterMap::RegisterZeroBased,
							"{op:?} lane {lane}"
						)
					}
					OperandRole::ConstIdx => {
						assert_eq!(
							roles[lane].scatter(),
							ScatterMap::ConstantPermutation,
							"{op:?} lane {lane}"
						)
					}
					OperandRole::UpvalIdx => {
						assert_eq!(
							roles[lane].scatter(),
							ScatterMap::UpvaluePermutation,
							"{op:?} lane {lane}"
						)
					}
					_ => assert_eq!(
						roles[lane].scatter(),
						ScatterMap::Identity,
						"{op:?} lane {lane}"
					),
				}
			}
		}
	}

	#[test]
	fn unused_lane_semantics_match_emitters_and_handlers() {
		// Jmp: only the jump target in b is semantic.
		assert_eq!(Op::Jmp.meta().unused_lanes(), [true, false, true, true]);
		// Nop: pure noise, every lane is fill.
		assert_eq!(Op::Nop.meta().unused_lanes(), [true, true, true, true]);
		// MkStr packs bytes+length into b/c/d: no lane may be noise-filled.
		assert_eq!(Op::MkStr.meta().unused_lanes(), [false, false, false, false]);
		// Regression pins for the three lanes the legacy table and the
		// metadata once disagreed on: the emitters write constant 0 and
		// the handlers never read these lanes, so they are unused.
		assert_eq!(Op::VarArgTab.meta().unused_lanes(), [false, true, true, true]);
		assert_eq!(Op::VarArgC.meta().unused_lanes(), [false, true, true, true]);
		assert_eq!(Op::CallE.meta().unused_lanes(), [false, false, true, false]);
		// ...while CallM lane c is the result count and stays semantic.
		assert_eq!(Op::CallM.meta().unused_lanes(), [false, false, false, false]);
		// Call/CallT/Return/LoadStr use all four lanes.
		assert_eq!(Op::Call.meta().unused_lanes(), [false; 4]);
		assert_eq!(Op::CallT.meta().unused_lanes(), [false; 4]);
		assert_eq!(Op::Return.meta().unused_lanes(), [false; 4]);
		assert_eq!(Op::LoadStr.meta().unused_lanes(), [false; 4]);
		// Jumps carry their target in b only.
		assert_eq!(Op::Jf.meta().unused_lanes(), [false, false, true, true]);
		assert_eq!(Op::Jt.meta().unused_lanes(), [false, false, true, true]);
	}

	#[test]
	fn const_encode_roundtrips_ieee754_signed_zero_nan_inf_and_binary_strings() {
		let samples = vec![
			Const::Nil,
			Const::Bool(false),
			Const::Bool(true),
			Const::Num(0.0),
			Const::Num(-0.0),
			Const::Num(f64::INFINITY),
			Const::Num(f64::NEG_INFINITY),
			Const::Num(f64::NAN),
			Const::Num(9_007_199_254_740_991.0),
			Const::Num(-9_007_199_254_740_991.0),
			Const::Num(1.2345678901234567e-200),
			Const::Num(f64::MIN_POSITIVE),
			Const::Str(Vec::new()),
			Const::Str((0u8..=255).collect()),
			Const::Str(vec![b'Z'; 300]),
		];

		let mut enc = Vec::new();
		let mut enc_lcg = ConstLcg::new(0x1234_5678, 1_103_515_245 % 268_435_456 | 1, 12_345);
		for c in &samples {
			c.encode(&mut enc, &mut enc_lcg);
		}

		let mut dec_lcg = ConstLcg::new(0x1234_5678, 1_103_515_245 % 268_435_456 | 1, 12_345);
		let mut pos = 0usize;
		let mut mb = || {
			let b = dec_lcg.unmask(enc[pos]);
			pos += 1;
			b
		};

		for expected in &samples {
			let tag = mb();
			match expected {
				Const::Nil => assert_eq!(tag, 0),
				Const::Bool(b) => {
					assert_eq!(tag, 1);
					assert_eq!(mb() == 1, *b);
				}
				Const::Num(ev) if ev.is_nan() => assert_eq!(tag, 7),
				Const::Num(ev) if ev.is_infinite() => {
					assert_eq!(tag, if ev.is_sign_negative() { 6 } else { 5 });
				}
				Const::Num(ev) => {
					assert_eq!(tag, 4);
					let mut m: u64 = 0;
					let mut sh: u64 = 1;
					loop {
						let bb = mb() as u64;
						if bb < 128 {
							m += bb * sh;
							break;
						}
						m += (bb - 128) * sh;
						sh *= 128;
					}
					let mut kp: u64 = 0;
					let mut sh2: u64 = 1;
					loop {
						let bb = mb() as u64;
						if bb < 128 {
							kp += bb * sh2;
							break;
						}
						kp += (bb - 128) * sh2;
						sh2 *= 128;
					}
					let kk = (kp / 2) as i32 - 2048;
					let mut v = m as f64;
					if kk >= 0 {
						for _ in 0..kk {
							v *= 2.0;
						}
					} else {
						for _ in 0..(-kk) {
							v /= 2.0;
						}
					}
					if kp % 2 == 1 {
						v = -v;
					}
					assert_eq!(
						v.to_bits(),
						ev.to_bits(),
						"IEEE-754 bit mismatch for {ev:?} (got {v:?})"
					);
				}
				Const::Str(eb) => {
					assert_eq!(tag, 3);
					let lo = mb() as usize;
					let hi = mb() as usize;
					let len = lo + hi * 256;
					assert_eq!(len, eb.len());
					let got: Vec<u8> = (0..len).map(|_| mb()).collect();
					assert_eq!(&got, eb);
				}
			}
		}
		assert_eq!(pos, enc.len());
	}

	#[test]
	fn all_44_ops_have_parseable_handlers_in_luau_and_lua51() {
		for (i, &_op) in ALL_OPS.iter().enumerate() {
			let name = OP_NAMES[i];
			for &v15 in &[false, true] {
				let nf = super::super::handlers::n_formats(name, v15);
				assert!(nf >= 1, "handler {name} has 0 formats");
				for fmt in 0..nf {
					for &luau_target in &[true, false] {
						if v15 && !luau_target {
							continue;
						}
						let mut pool = super::super::strpool::StrPool::new(!v15);
						let body = super::super::handlers::gen(
							name,
							fmt,
							v15,
							&mut pool,
							(17, 29),
							luau_target,
						);
						let wrapped = format!("local function _h()\n{}\nend\n", body);
						assert!(
							crate::parser::parse(&wrapped, luau_target).is_ok(),
							"handler {name} fmt={fmt} v15={v15} luau={luau_target} failed to parse:\n{wrapped}"
						);
					}
				}
			}
		}
	}
}

/// Canonical ordered names of all 44 VM opcodes, matching `op_index` / `ALL_OPS` 1-to-1.
pub const OP_NAMES: [&str; N_OPS] = [
	"Jmp", "Jf", "Jt", "LoadNil", "LoadK", "Move", "Add", "Sub", "Mul", "Div",
	"Mod", "Pow", "Concat", "Unm", "Not", "Len", "Lt", "Le", "Gt", "Ge", "Eq",
	"Ne", "Idiv", "NewTab", "GetTab", "SetTab", "TabN", "CallT", "Closure",
	"Call", "VarArgTab", "VarArgC", "VarArgTabN", "GetGlobal", "SetGlobal",
	"GetUp", "SetUp", "Return", "Nop", "CallE", "CallM", "MkStr", "Canary", "LoadStr",
];

/// Canonical ordered table of all 44 VM opcodes, matching `op_index` / `OP_NAMES` 1-to-1.
pub const ALL_OPS: [Op; N_OPS] = [
	Op::Jmp, Op::Jf, Op::Jt, Op::LoadNil, Op::LoadK, Op::Move, Op::Add, Op::Sub,
	Op::Mul, Op::Div, Op::Mod, Op::Pow, Op::Concat, Op::Unm, Op::Not, Op::Len,
	Op::Lt, Op::Le, Op::Gt, Op::Ge, Op::Eq, Op::Ne, Op::Idiv, Op::NewTab,
	Op::GetTab, Op::SetTab, Op::TabN, Op::CallT, Op::Closure, Op::Call,
	Op::VarArgTab, Op::VarArgC, Op::VarArgTabN, Op::GetGlobal, Op::SetGlobal,
	Op::GetUp, Op::SetUp, Op::Return, Op::Nop, Op::CallE, Op::CallM,
	Op::MkStr, Op::Canary, Op::LoadStr,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandRole {
	Unused,
	RegRead,
	RegWrite,
	RegRange,
	RegBase,
	CallBase,
	ConstIdx,
	ProtoIdx,
	UpvalIdx,
	JumpTarget,
	ImmVal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcEffect {
	Fallthrough,
	UnconditionalJmp,
	ConditionalJmp,
	FrameTransfer,
	FrameReturn,
}

/// How the scatter pass is allowed to transform one operand lane.  This is
/// deliberately separate from the lane's semantic role: e.g. a JumpTarget is
/// semantically a PC but must not be sigma-remapped as a register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScatterMap {
	Identity,
	RegisterZeroBased,
	ConstantPermutation,
	UpvaluePermutation,
}

impl OperandRole {
	pub const fn scatter(self) -> ScatterMap {
		match self {
			OperandRole::RegRead | OperandRole::RegWrite | OperandRole::RegRange => {
				ScatterMap::RegisterZeroBased
			}
			OperandRole::ConstIdx => ScatterMap::ConstantPermutation,
			OperandRole::UpvalIdx => ScatterMap::UpvaluePermutation,
			OperandRole::CallBase
			| OperandRole::RegBase
			| OperandRole::JumpTarget
			| OperandRole::ProtoIdx
			| OperandRole::ImmVal
			| OperandRole::Unused => ScatterMap::Identity,
		}
	}
}

impl OpMeta {
	pub const fn roles(self) -> [OperandRole; 4] {
		[self.role_a, self.role_b, self.role_c, self.role_d]
	}

	/// Lanes that carry no semantic value for this op, mechanically
	/// derived from the canonical roles: a lane is unused iff its role is
	/// `OperandRole::Unused`.  This is the ONLY unused-lane definition in
	/// the tree — the noise/NOP fill in the compiler and the pre-scatter
	/// zero-lane check in the verifier both consume it.  (It replaces a
	/// legacy hand-maintained per-op table that had drifted from
	/// `Op::meta()` on VarArgTab/VarArgC lane b and CallE lane c; those
	/// metas now state the truth: the emitter always writes 0 there and
	/// no handler reads the lane.)
	pub const fn unused_lanes(self) -> [bool; 4] {
		[
			matches!(self.role_a, OperandRole::Unused),
			matches!(self.role_b, OperandRole::Unused),
			matches!(self.role_c, OperandRole::Unused),
			matches!(self.role_d, OperandRole::Unused),
		]
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpMeta {
	pub op: Op,
	pub name: &'static str,
	pub role_a: OperandRole,
	pub role_b: OperandRole,
	pub role_c: OperandRole,
	pub role_d: OperandRole,
	pub pc_effect: PcEffect,
	pub may_call_host: bool,
	pub touches_env_or_upv: bool,
	pub multiret_sensitive: bool,
	pub fuse_head_ok: bool,
	pub fuse_tail_ok: bool,
}

impl Op {
	pub fn from_index(idx: usize) -> Option<Op> {
		ALL_OPS.get(idx).copied()
	}

	pub fn from_name(name: &str) -> Option<Op> {
		OP_NAMES
			.iter()
			.position(|&n| n == name)
			.and_then(Self::from_index)
	}

	/// Explicit semantic boundary metadata for handler polymorphism, wrapper
	/// arity specialization, continuation epilogue selection, and super-insn fusion.
	pub const fn meta(self) -> OpMeta {
		use OperandRole::*;
		use PcEffect::*;
		match self {
			Op::Jmp => OpMeta {
				op: self,
				name: "Jmp",
				role_a: Unused,
				role_b: JumpTarget,
				role_c: Unused,
				role_d: Unused,
				pc_effect: UnconditionalJmp,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Jf => OpMeta {
				op: self,
				name: "Jf",
				role_a: RegRead,
				role_b: JumpTarget,
				role_c: Unused,
				role_d: Unused,
				pc_effect: ConditionalJmp,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Jt => OpMeta {
				op: self,
				name: "Jt",
				role_a: RegRead,
				role_b: JumpTarget,
				role_c: Unused,
				role_d: Unused,
				pc_effect: ConditionalJmp,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::LoadNil => OpMeta {
				op: self,
				name: "LoadNil",
				role_a: RegBase,
				role_b: ImmVal,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: true,
				fuse_tail_ok: false,
			},
			Op::LoadK => OpMeta {
				op: self,
				name: "LoadK",
				role_a: RegWrite,
				role_b: ConstIdx,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: true,
				fuse_tail_ok: true,
			},
			Op::Canary => OpMeta {
				op: self,
				name: "Canary",
				role_a: ImmVal,
				role_b: Unused,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: true,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::MkStr => OpMeta {
				op: self,
				name: "MkStr",
				role_a: RegWrite,
				role_b: ImmVal,
				role_c: ImmVal,
				role_d: ImmVal,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::LoadStr => OpMeta {
				op: self,
				name: "LoadStr",
				role_a: RegWrite,
				role_b: ConstIdx,
				role_c: ImmVal,
				role_d: ImmVal,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Move => OpMeta {
				op: self,
				name: "Move",
				role_a: RegWrite,
				role_b: RegRead,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: true,
				fuse_tail_ok: true,
			},
			Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Pow | Op::Idiv => OpMeta {
				op: self,
				name: match self {
					Op::Add => "Add",
					Op::Sub => "Sub",
					Op::Mul => "Mul",
					Op::Div => "Div",
					Op::Mod => "Mod",
					Op::Pow => "Pow",
					Op::Idiv => "Idiv",
					_ => "",
				},
				role_a: RegWrite,
				role_b: RegRead,
				role_c: RegRead,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Unm => OpMeta {
				op: self,
				name: "Unm",
				role_a: RegWrite,
				role_b: RegRead,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Not => OpMeta {
				op: self,
				name: "Not",
				role_a: RegWrite,
				role_b: RegRead,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Len => OpMeta {
				op: self,
				name: "Len",
				role_a: RegWrite,
				role_b: RegRead,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Concat | Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => OpMeta {
				op: self,
				name: match self {
					Op::Concat => "Concat",
					Op::Eq => "Eq",
					Op::Ne => "Ne",
					Op::Lt => "Lt",
					Op::Le => "Le",
					Op::Gt => "Gt",
					Op::Ge => "Ge",
					_ => "",
				},
				role_a: RegWrite,
				role_b: RegRead,
				role_c: RegRead,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::NewTab => OpMeta {
				op: self,
				name: "NewTab",
				role_a: RegWrite,
				role_b: Unused,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::GetTab => OpMeta {
				op: self,
				name: "GetTab",
				role_a: RegWrite,
				role_b: RegRead,
				role_c: RegRead,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::SetTab => OpMeta {
				op: self,
				name: "SetTab",
				role_a: RegRead,
				role_b: RegRead,
				role_c: RegRead,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::TabN => OpMeta {
				op: self,
				name: "TabN",
				role_a: RegRead,
				role_b: RegRead,
				role_c: RegRead,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::GetUp => OpMeta {
				op: self,
				name: "GetUp",
				role_a: RegWrite,
				role_b: UpvalIdx,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: true,
				multiret_sensitive: false,
				fuse_head_ok: true,
				fuse_tail_ok: false,
			},
			Op::SetUp => OpMeta {
				op: self,
				name: "SetUp",
				role_a: UpvalIdx,
				role_b: RegRead,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: true,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::GetGlobal => OpMeta {
				op: self,
				name: "GetGlobal",
				role_a: RegWrite,
				role_b: ConstIdx,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: true,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::SetGlobal => OpMeta {
				op: self,
				name: "SetGlobal",
				role_a: RegRead,
				role_b: ConstIdx,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: true,
				touches_env_or_upv: true,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Closure => OpMeta {
				op: self,
				name: "Closure",
				role_a: RegWrite,
				role_b: ProtoIdx,
				role_c: Unused,
				role_d: Unused,
				// Consumes `nups` inline capture pseudo-instructions by
				// advancing E.ip — control still falls through to the
				// next record, so this is Fallthrough, not a jump (no
				// consumer ever read the old ConditionalJmp value; the
				// only pc_effect consumer matches FrameReturn).
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: true,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Call => OpMeta {
				op: self,
				name: "Call",
				role_a: CallBase,
				role_b: ImmVal,
				role_c: ImmVal,
				role_d: ImmVal,
				pc_effect: FrameTransfer,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			// CallE and CallM share (base, nfixed, ?, flags) shape but NOT
			// lane c semantics, so they cannot share one metadata literal:
			// CallM c is the result count (255 = multiret), while CallE c
			// is emitted as a constant 0 and never read by the handler.
			Op::CallE => OpMeta {
				op: self,
				name: "CallE",
				role_a: CallBase,
				role_b: ImmVal,
				role_c: Unused,
				role_d: ImmVal,
				pc_effect: FrameTransfer,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::CallM => OpMeta {
				op: self,
				name: "CallM",
				role_a: CallBase,
				role_b: ImmVal,
				role_c: ImmVal,
				role_d: ImmVal,
				pc_effect: FrameTransfer,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::CallT => OpMeta {
				op: self,
				name: "CallT",
				role_a: CallBase,
				role_b: RegRead,
				role_c: RegRead,
				role_d: ImmVal,
				pc_effect: FrameTransfer,
				may_call_host: true,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::VarArgTab => OpMeta {
				op: self,
				name: "VarArgTab",
				role_a: RegWrite,
				// b is emitted as a constant 0 and never read by the
				// handler (the vararg count arrives via frame state).
				role_b: Unused,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::VarArgC => OpMeta {
				op: self,
				name: "VarArgC",
				role_a: RegWrite,
				// b is emitted as a constant 0 and never read by the
				// handler (the vararg count arrives via frame state).
				role_b: Unused,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Return => OpMeta {
				op: self,
				name: "Return",
				role_a: RegBase,
				role_b: ImmVal,
				role_c: ImmVal,
				role_d: ImmVal,
				pc_effect: FrameReturn,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::Nop => OpMeta {
				op: self,
				name: "Nop",
				role_a: Unused,
				role_b: Unused,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: false,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
			Op::VarArgTabN => OpMeta {
				op: self,
				name: "VarArgTabN",
				role_a: RegRead,
				role_b: RegRead,
				role_c: Unused,
				role_d: Unused,
				pc_effect: Fallthrough,
				may_call_host: false,
				touches_env_or_upv: false,
				multiret_sensitive: true,
				fuse_head_ok: false,
				fuse_tail_ok: false,
			},
		}
	}
}
