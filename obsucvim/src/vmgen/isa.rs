
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
}

pub const N_OPS: usize = 43;

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
		(Op::MkStr, 41), (Op::LoadStr, 42),
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
		Op::LoadStr => 42,
	}
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

#[rustfmt::skip]
pub fn unused_operands(op: Op) -> [bool; 4] {
	match op {
		Op::Jmp => [true, false, true, true],
		Op::Jf | Op::Jt | Op::LoadNil | Op::LoadK | Op::Move
		| Op::Unm | Op::Not | Op::Len | Op::Closure
		| Op::VarArgTabN | Op::GetGlobal | Op::SetGlobal
		| Op::GetUp | Op::SetUp => [false, false, true, true],
		Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Pow
		| Op::Concat | Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq
		| Op::Ne | Op::Idiv | Op::GetTab | Op::SetTab | Op::TabN => [false, false, false, true],
		Op::NewTab | Op::VarArgTab | Op::VarArgC => [false, true, true, true],
		Op::CallT | Op::Call | Op::CallM | Op::Return | Op::MkStr | Op::LoadStr => [false, false, false, false],
		Op::CallE => [false, false, true, false],
		Op::Nop => [true, true, true, true],
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
}

impl ConstLcg {
	pub fn new(seed: u32, km: u32, kc: u32) -> ConstLcg {
		ConstLcg { state: seed, km, kc }
	}
	pub fn next_key(&mut self) -> u8 {
		self.state = ((self.km as u64 * self.state as u64 + self.kc as u64)
			% 268_435_456) as u32;
		fold_key(self.state as u64)
	}
	pub fn mask(&mut self, plain: u8) -> u8 {
		plain.wrapping_add(self.next_key())
	}
}

pub(crate) fn fold_key(state: u64) -> u8 {
	(((state % 256)
		+ ((state / 256) % 256)
		+ ((state / 65536) % 256)
		+ (state / 16777216))
		% 256) as u8
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
}
