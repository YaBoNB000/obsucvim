
use crate::rng::Rng;
use crate::vmgen::handlers;
use crate::vmgen::isa::{fold_key, Carrier, OpMap, CARRIER_SPECIALS, N_OPS};
use crate::vmgen::strpool::StrPool;

const OP_NAMES: [&str; N_OPS] = [
	"Jmp", "Jf", "Jt", "LoadNil", "LoadK", "Move", "Add", "Sub", "Mul", "Div",
	"Mod", "Pow", "Concat", "Unm", "Not", "Len", "Lt", "Le", "Gt", "Ge", "Eq",
	"Ne", "Idiv", "NewTab", "GetTab", "SetTab", "TabN", "CallT", "Closure",
	"Call", "VarArgTab", "VarArgC", "VarArgTabN", "GetGlobal", "SetGlobal",
	"GetUp", "SetUp", "Return", "Nop", "CallE", "CallM", "MkStr", "LoadStr",
];

const PRIM_SRC: [&str; 15] = [
	"(\"\")[\"byte\"]",
	"(\"\")[\"sub\"]",
	"(\"\")[\"char\"]",
	"getfenv",
	"unpack",
	"math.floor",
	"type",
	"error",
	"getmetatable",
	"rawget",
	"rawset",
	"setmetatable",
	"select",
	"pcall",
	"tonumber",
];

const PRIM_NAME: [&str; 15] = [
	"BYTE", "SUB", "CHAR", "GFE", "UNP", "FLR", "TYP", "ERR", "GMT", "RGET",
	"RSET", "SMT", "SEL", "PCAL", "TONUM",
];

fn gen_dispatch_tree(
	items: &[(String, u8)],
	body_of: &mut dyn FnMut(&str) -> String,
	rng: &mut Rng,
	depth: u32,
) -> String {
	let n = items.len();
	if n == 1 {
		let name = items[0].0.as_str();
		return format!("if oc == OC.{} then\n{}\nend", name, body_of(name));
	}
	if depth >= 3 {
		let mut v: Vec<(String, u8)> = items.to_vec();
		rng.shuffle(&mut v);
		let mut s = String::new();
		for (i, (idx, _)) in v.iter().enumerate() {
			let name = idx.as_str();
			if i == 0 {
				s.push_str(&format!("if oc == OC.{} then\n{}\n", name, body_of(name)));
			} else {
				s.push_str(&format!(
					"elseif oc == OC.{} then\n{}\n",
					name,
					body_of(name)
				));
			}
		}
		s.push_str("end");
		return s;
	}
	let mut sc: Vec<u8> = items.iter().map(|&(_, c)| c).collect();
	sc.sort_unstable();
	let mut gaps: Vec<(u8, u8)> = Vec::new();
	for w in 1..sc.len() {
		if sc[w] > sc[w - 1] + 1 {
			gaps.push((sc[w - 1], sc[w]));
		}
	}
	if !gaps.is_empty() {
		let (lo, hi) = gaps[rng.int(0, (gaps.len() - 1) as i64) as usize];
		let p: u8 = if hi as i64 - lo as i64 == 2 {
			lo + 1
		} else {
			rng.int(lo as i64 + 1, hi as i64 - 1) as u8
		};
		let (left, right): (Vec<(String, u8)>, Vec<(String, u8)>) =
			items.iter().cloned().partition(|&(_, c)| c < p);
		let left_s = gen_dispatch_tree(&left, body_of, rng, depth + 1);
		let right_s = gen_dispatch_tree(&right, body_of, rng, depth + 1);
		if rng.int(0, 1) == 0 {
			format!("if oc < {} then\n{}\nelse\n{}\nend", p, left_s, right_s)
		} else {
			format!("if oc > {} then\n{}\nelse\n{}\nend", p - 1, right_s, left_s)
		}
	} else {
		let k = rng.int(1, (sc.len() - 1) as i64) as usize;
		let p = sc[k - 1];
		let (left, right): (Vec<(String, u8)>, Vec<(String, u8)>) =
			items.iter().cloned().partition(|&(_, c)| c <= p);
		let left_s = gen_dispatch_tree(&left, body_of, rng, depth + 1);
		let right_s = gen_dispatch_tree(&right, body_of, rng, depth + 1);
		if rng.int(0, 1) == 0 {
			format!("if oc <= {} then\n{}\nelse\n{}\nend", p, left_s, right_s)
		} else {
			format!("if oc < {} then\n{}\nelse\n{}\nend", p + 1, left_s, right_s)
		}
	}
}

fn lua_dstr(bytes: &[u8]) -> String {
	let mut s = String::from("\"");
	for &b in bytes {
		s.push_str(&format!("\\{b:03}"));
	}
	s.push('"');
	s
}

fn prim_slots(rng: &mut Rng) -> [u8; 15] {
	let mut pool: Vec<u8> = (1..=80).collect();
	rng.shuffle(&mut pool);
	let mut out = [0u8; 15];
	out.copy_from_slice(&pool[..15]);
	out
}

fn obf_num(n: u64, rng: &mut Rng) -> String {
let x = rng.int(0, 1_099_511_627_775) as i64;
	let y = rng.int(0, 1_099_511_627_775) as i64;
	let z = x + y - n as i64;
	format!("({} + {} - {})", x, y, z)
}

use super::manifest_key;

const KEY_MOD: i64 = 268_435_456;

struct KeyEmitter<'a> {
	carrier_lengths: &'a [usize],
}

impl<'a> KeyEmitter<'a> {
	fn new(carrier_lengths: &'a [usize]) -> KeyEmitter<'a> {
		assert!(!carrier_lengths.is_empty(), "KeyEmitter needs the main carrier");
		KeyEmitter { carrier_lengths }
	}

	fn key_expr(
		&mut self,
		key: i64,
		anchor: Option<(&str, i64)>,
		rng: &mut Rng,
	) -> String {
		self.key_expr_with_data(key, anchor, "FN", rng)
	}

	fn key_expr_with_data(
		&mut self,
		key: i64,
		anchor: Option<(&str, i64)>,
		data_expr: &str,
		rng: &mut Rng,
	) -> String {
		let key = key.rem_euclid(KEY_MOD);
		let anchor_value = anchor.map(|(_, value)| value).unwrap_or(0).rem_euclid(KEY_MOD);
		let anchor_expr = anchor.map(|(expr, _)| expr).unwrap_or("0");

		let (seed, mul, step, bias, encoded) = loop {
			let seed = rng.int(0, KEY_MOD - 1);
			let mul = rng.int(257, 65_535);
			let step = rng.int(1, 65_535);
			let bias = rng.int(0, KEY_MOD - 1);
			let mut state = (seed + anchor_value) % KEY_MOD;
			for (index, &length) in self.carrier_lengths.iter().enumerate() {
				state = (state * mul
					+ length as i64
					+ (index as i64 + 1) * step
					+ bias)
					% KEY_MOD;
			}
			let encoded = (key - state).rem_euclid(KEY_MOD);
			if state != 0
				&& encoded != key
				&& seed != key
				&& mul != key
				&& step != key
				&& bias != key
			{
				break (seed, mul, step, bias, encoded);
			}
		};

		format!(
			"(function(d) local s=({seed}+({anchor_expr}))%{modulus} for i=1,#d do s=(s*{mul}+#d[i]+i*{step}+{bias})%{modulus} end return ({encoded}+s)%{modulus} end)({data})",
			seed = seed,
			anchor_expr = anchor_expr,
			modulus = KEY_MOD,
			mul = mul,
			step = step,
			bias = bias,
			encoded = encoded,
			data = data_expr,
		)
	}

	fn dead_zero(&mut self, rng: &mut Rng) -> String {
		self.key_expr(0, None, rng)
	}
}

fn boot_names_block(rng: &mut Rng, names: &[(&str, &str)], obf_num: &dyn Fn(i64, &mut Rng) -> String) -> String {
	let mut pool: Vec<u8> = Vec::new();
	for (_v, name) in names {
		for &b in name.as_bytes() {
			if !pool.contains(&b) {
				pool.push(b);
			}
		}
	}
	let nbd_seed = rng.int(1_048_576, 268_435_455);
	let nbd_m = (rng.int(1_048_577, 33_000_001) | 1) as i64;
	let nbd_c = rng.int(1_048_576, 268_435_455);
	let mut st = nbd_seed as u64;
	let mut arr: Vec<u8> = Vec::new();
	for &b in &pool {
		st = (nbd_m as u64 * st + nbd_c as u64) % 268_435_456;
		let f = ((st % 256)
			+ ((st / 256) % 256)
			+ ((st / 65536) % 256)
			+ (st / 16_777_216))
			% 256;
		arr.push(((b as u64 + f) % 256) as u8);
	}
	let ta = "NBa";
	let mut out = format!(
		"local {} = {{{}}}\n",
		ta,
		arr.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
	);
	out.push_str(&format!(
		"local NBs = {}\nfor NBi = 1, {} do\n  NBs = ({} * NBs + {}) % 268435456\n  {}[NBi] = ({}[NBi] - (((NBs % 256) + (FLR(NBs / 256) % 256) + (FLR(NBs / 65536) % 256) + FLR(NBs / 16777216)) % 256)) % 256\nend\n",
		obf_num(nbd_seed, rng),
		pool.len(),
		obf_num(nbd_m, rng),
		obf_num(nbd_c, rng),
		ta, ta
	));
	for (v, name) in names {
		let idx: Vec<String> = name
			.bytes()
			.map(|b| {
				format!(
					"{}[{}]",
					ta,
					pool.iter().position(|&x| x == b).unwrap() + 1
				)
			})
			.collect();
		out.push_str(&format!("local {} = CHAR({})\n", v, idx.join(", ")));
	}
	out
}

fn sanitize_name(
	rng: &mut Rng,
	reserved: &std::collections::HashSet<String>,
	used: &std::collections::HashSet<String>,
) -> String {
	const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";
	loop {
		let len = match rng.int(0, 99) {
			0..=59 => 2,
			60..=89 => 3,
			_ => 4,
		};
		let mut name = String::with_capacity(len);
		for k in 0..len {
			let pool = if k == 0 { &CHARS[..] } else { CHARS };
			name.push(pool[rng.int(0, pool.len() as i64 - 1) as usize] as char);
		}
		if !reserved.contains(&name) && !used.contains(&name) {
			return name;
		}
	}
}

pub const SH_M1: u64 = 67_108_859;
pub const SH_M2: u64 = 67_108_819;

fn sanitize_frag(
	src: &str,
	rng: &mut Rng,
	map: &mut std::collections::HashMap<String, String>,
	reserved: &std::collections::HashSet<String>,
	used: &mut std::collections::HashSet<String>,
) -> String {
	static GLOBALS: &[&str] = &[
		"table", "string", "math", "bit32", "os", "debug", "buffer",
		"task", "coroutine", "utf8", "_G", "game", "workspace", "Instance",
		"Vector3", "Vector2", "Enum", "Random",
	];
	let is_global = |w: &str| GLOBALS.contains(&w);
	static RUNTIME_FIELDS: &[&str] = &["n"];
	let b = src.as_bytes();
	let mut out = String::with_capacity(src.len());
	let mut i = 0usize;
	let mut quote: Option<u8> = None;
	let mut escaped = false;
	let mut pending_space = false;
let mut prev_word: Option<String> = None;
let mut after_dot = false;
	while i < b.len() {
		let c = b[i];
		if let Some(q) = quote {
			out.push(c as char);
			if escaped {
				escaped = false;
			} else if c == b'\\' {
				escaped = true;
			} else if c == q {
				quote = None;
			}
			i += 1;
			continue;
		}
		match c {
			b'\'' | b'"' => {
				if pending_space {
					out.push(' ');
					pending_space = false;
				}
				quote = Some(c);
				out.push(c as char);
				prev_word = None;
				after_dot = false;
				i += 1;
			}
			b'-' if i + 1 < b.len() && b[i + 1] == b'-' => {
				while i < b.len() && b[i] != b'\n' {
					i += 1;
				}
				pending_space = true;
			}
			b'\n' | b'\r' | b' ' | b'\t' => {
				pending_space = true;
				i += 1;
			}
			b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
				let start = i;
				while i < b.len()
					&& (b[i].is_ascii_alphanumeric() || b[i] == b'_')
				{
					i += 1;
				}
				let word = &src[start..i];
				if pending_space {
					out.push(' ');
					pending_space = false;
				}
				let keep_field = RUNTIME_FIELDS.contains(&word)
					|| (after_dot
						&& prev_word.as_deref().map(is_global).unwrap_or(false));
				if reserved.contains(word) || keep_field {
					out.push_str(word);
				} else {
					if !map.contains_key(word) {
						let name = sanitize_name(rng, reserved, used);
						used.insert(name.clone());
						map.insert(word.to_string(), name);
					}
					out.push_str(&map[word]);
				}
				prev_word = Some(word.to_string());
				after_dot = false;
			}
			_ => {
				if pending_space {
					out.push(' ');
					pending_space = false;
				}
				out.push(c as char);
				after_dot = c == b'.' || c == b':';
				if !after_dot {
					prev_word = None;
				}
				i += 1;
			}
		}
	}
	out
}


const M_READ_V: usize = 0;
const M_WRITE_V: usize = 1;
const M_READ_C: usize = 2;
const M_NOT: usize = 3;
const M_NEW_TABLE: usize = 4;
const M_GET_GLOBAL: usize = 5;
const M_SET_GLOBAL: usize = 6;
const M_READ_UP: usize = 7;
const M_CELL_GET: usize = 8;
const M_CELL_SET: usize = 9;
const M_VARGC: usize = 10;
const M_SET_PC: usize = 11;
const M_BRANCH_FALSE: usize = 12;
const M_BRANCH_TRUE: usize = 13;
const M_ADD: usize = 14;
const M_SUB: usize = 15;
const M_MUL: usize = 16;
const M_DIV: usize = 17;
const M_MOD: usize = 18;
const M_POW: usize = 19;
const M_IDIV: usize = 20;
const M_UNM: usize = 21;
const M_LEN: usize = 22;
const M_CONCAT: usize = 23;
const M_LT: usize = 24;
const M_LE: usize = 25;
const M_GT: usize = 26;
const M_GE: usize = 27;
const M_EQ: usize = 28;
const M_NE: usize = 29;
const M_LOAD_NIL_RANGE: usize = 30;
const M_TABN: usize = 31;
const M_VARG_TABLE: usize = 32;
const M_VARG_TABN: usize = 33;
const M_CLOSURE: usize = 34;
const M_GET_TABLE: usize = 35;
const M_SET_TABLE: usize = 36;
const M_NOP: usize = 37;
const M_MKSTR: usize = 38;
const M_RETURN: usize = 39;
const M_CALL: usize = 40;
const M_CALLE: usize = 41;
const M_CALLM: usize = 42;
const M_CALLT: usize = 43;
const M_POST_CALL: usize = 44;
const M_POST_CALLE: usize = 45;
const M_POST_CALLM: usize = 46;
const M_POST_CALLT: usize = 47;
const M_LOAD_STR: usize = 48;
const M_FUSE_LOAD_K: usize = 49;
const M_FUSE_MOVE: usize = 50;
const M_FUSE_NEW_TABLE: usize = 51;
const M_FUSE_NOT: usize = 52;
const M_FUSE_GET_UP: usize = 53;
const M_FUSE_SET_UP: usize = 54;
const M_FUSE_VARGC: usize = 55;
const M_FUSE_BRANCH_FALSE: usize = 56;
const M_FUSE_BRANCH_TRUE: usize = 57;
const M_FUSE_VARARG_TABLE: usize = 58;
const M_SUPER_BASE: usize = 59;
const MICRO_SUPER_NAMES: [&str; 23] = [
	"LoadStr", "GetGlobal", "SetGlobal", "Add", "Sub", "Mul", "Div", "Mod", "Pow", "Idiv",
	"Unm", "Len", "Concat", "Lt", "Le", "Gt", "Ge", "Eq", "Ne", "TabN", "VarArgTabN",
	"GetTab", "SetTab",
];
const M_COUNT: usize = M_SUPER_BASE + MICRO_SUPER_NAMES.len();

struct MicroLits {
	number: String,
	string: String,
	boolean: String,
	nil: String,
	function: String,
	table_: String,
	index: String,
	newindex: String,
	char_: String,
	byte_: String,
	empty: String,
	add: String,
	sub: String,
	mul: String,
	div: String,
	mod_: String,
	pow: String,
	unm: String,
	len: String,
	concat: String,
	lt: String,
	le: String,
	eq: String,
	arith_msg: String,
	compare_msg: String,
	concat_msg: String,
	index_msg: String,
	value_msg: String,
	with_msg: String,
	mk1: u16,
	mk2: u16,
}

impl MicroLits {
	fn new(pool: &mut StrPool, mk: (u16, u16), include_loadstr: bool) -> MicroLits {
		MicroLits {
			number: pool.lit("number"),
			string: pool.lit("string"),
			boolean: pool.lit("boolean"),
			nil: pool.lit("nil"),
			function: pool.lit("function"),
			table_: pool.lit("table"),
			index: pool.lit("__index"),
			newindex: pool.lit("__newindex"),
			char_: pool.lit("char"),
			byte_: if include_loadstr { pool.lit("byte") } else { String::new() },
			empty: pool.lit(""),
			add: pool.lit("__add"),
			sub: pool.lit("__sub"),
			mul: pool.lit("__mul"),
			div: pool.lit("__div"),
			mod_: pool.lit("__mod"),
			pow: pool.lit("__pow"),
			unm: pool.lit("__unm"),
			len: pool.lit("__len"),
			concat: pool.lit("__concat"),
			lt: pool.lit("__lt"),
			le: pool.lit("__le"),
			eq: pool.lit("__eq"),
			arith_msg: pool.lit("attempt to perform arithmetic on a "),
			compare_msg: pool.lit("attempt to compare "),
			concat_msg: pool.lit("attempt to perform concatenation on a "),
			index_msg: pool.lit("attempt to index a "),
			value_msg: pool.lit(" value"),
			with_msg: pool.lit(" with "),
			mk1: mk.0,
			mk2: mk.1,
		}
	}
}

#[derive(Clone)]
struct MicroShard {
	codes: [u8; M_COUNT],
	field_perm: [usize; 3],
	shape: u8,
}

fn new_micro_shard(rng: &mut Rng, shape: u8) -> MicroShard {
	let mut pool: Vec<u8> = (7..=250).collect();
	rng.shuffle(&mut pool);
	let codes: [u8; M_COUNT] = pool[..M_COUNT].try_into().unwrap();
	let mut fp = vec![0usize, 1, 2];
	rng.shuffle(&mut fp);
	MicroShard {
		codes,
		field_perm: [fp[0], fp[1], fp[2]],
		shape,
	}
}

fn micro_primitive_recipe(name: &str) -> Option<Vec<(usize, u8, u8)>> {
	let p = match name {
		"LoadK" => vec![(M_FUSE_LOAD_K, 0, 0)],
		"LoadStr" => vec![(M_READ_C, 1, 2), (M_LOAD_STR, 1, 1), (M_WRITE_V, 1, 1)],
		"Move" => vec![(M_FUSE_MOVE, 0, 0)],
		"NewTab" => vec![(M_FUSE_NEW_TABLE, 0, 0)],
		"Not" => vec![(M_FUSE_NOT, 0, 0)],
		"GetGlobal" => vec![
			(M_READ_C, 1, 2),
			(M_GET_GLOBAL, 2, 1),
			(M_WRITE_V, 1, 2),
		],
		"SetGlobal" => vec![(M_READ_C, 1, 2), (M_READ_V, 2, 1), (M_SET_GLOBAL, 1, 2)],
		"GetUp" => vec![(M_FUSE_GET_UP, 0, 0)],
		"SetUp" => vec![(M_FUSE_SET_UP, 0, 0)],
		"VarArgC" => vec![(M_FUSE_VARGC, 0, 0)],
		"Jmp" => vec![(M_SET_PC, 2, 0)],
		"Jf" => vec![(M_FUSE_BRANCH_FALSE, 0, 0)],
		"Jt" => vec![(M_FUSE_BRANCH_TRUE, 0, 0)],
		"Add" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_ADD, 3, 0), (M_WRITE_V, 1, 3)],
		"Sub" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_SUB, 3, 0), (M_WRITE_V, 1, 3)],
		"Mul" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_MUL, 3, 0), (M_WRITE_V, 1, 3)],
		"Div" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_DIV, 3, 0), (M_WRITE_V, 1, 3)],
		"Mod" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_MOD, 3, 0), (M_WRITE_V, 1, 3)],
		"Pow" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_POW, 3, 0), (M_WRITE_V, 1, 3)],
		"Idiv" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_IDIV, 3, 0), (M_WRITE_V, 1, 3)],
		"Unm" => vec![(M_READ_V, 1, 2), (M_UNM, 2, 1), (M_WRITE_V, 1, 2)],
		"Len" => vec![(M_READ_V, 1, 2), (M_LEN, 2, 1), (M_WRITE_V, 1, 2)],
		"Concat" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_CONCAT, 3, 0), (M_WRITE_V, 1, 3)],
		"Lt" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_LT, 3, 0), (M_WRITE_V, 1, 3)],
		"Le" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_LE, 3, 0), (M_WRITE_V, 1, 3)],
		"Gt" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_GT, 3, 0), (M_WRITE_V, 1, 3)],
		"Ge" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_GE, 3, 0), (M_WRITE_V, 1, 3)],
		"Eq" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_EQ, 3, 0), (M_WRITE_V, 1, 3)],
		"Ne" => vec![(M_READ_V, 1, 2), (M_READ_V, 2, 3), (M_NE, 3, 0), (M_WRITE_V, 1, 3)],
		"LoadNil" => vec![(M_LOAD_NIL_RANGE, 1, 2)],
		"TabN" => vec![
			(M_READ_V, 1, 1),
			(M_READ_V, 2, 2),
			(M_READ_V, 3, 3),
			(M_TABN, 0, 0),
			(M_WRITE_V, 2, 2),
		],
		"VarArgTab" => vec![(M_FUSE_VARARG_TABLE, 0, 0)],
		"VarArgTabN" => vec![
			(M_READ_V, 1, 1),
			(M_READ_V, 2, 2),
			(M_VARG_TABN, 0, 0),
			(M_WRITE_V, 2, 2),
		],
		"Closure" => vec![(M_CLOSURE, 1, 2)],
		"GetTab" => vec![
			(M_READ_V, 1, 2),
			(M_READ_V, 2, 3),
			(M_GET_TABLE, 3, 0),
			(M_WRITE_V, 1, 3),
		],
		"SetTab" => vec![
			(M_READ_V, 1, 1),
			(M_READ_V, 2, 2),
			(M_READ_V, 3, 3),
			(M_SET_TABLE, 0, 0),
		],
		"Nop" | "NopA" => vec![(M_NOP, 0, 0)],
		"MkStr" => vec![(M_MKSTR, 0, 0)],
		"Return" => vec![(M_RETURN, 0, 0)],
		_ => return None,
	};
	Some(p)
}

fn micro_recipe(name: &str) -> Option<Vec<(usize, u8, u8)>> {
	let primitive = micro_primitive_recipe(name)?;
	if primitive.len() <= 1 {
		return Some(primitive);
	}
	let slot = MICRO_SUPER_NAMES
		.iter()
		.position(|candidate| *candidate == name)
		.unwrap_or_else(|| panic!("multi-record micro recipe missing superoperator: {name}"));
	Some(vec![(M_SUPER_BASE + slot, 0, 0)])
}

fn micro_call_recipe(name: &str) -> Option<Vec<(usize, u8, u8)>> {
	match name {
		"Call" => Some(vec![(M_CALL, 0, 0)]),
		"CallE" => Some(vec![(M_CALLE, 0, 0)]),
		"CallM" => Some(vec![(M_CALLM, 0, 0)]),
		"CallT" => Some(vec![(M_CALLT, 0, 0)]),
		_ => None,
	}
}

fn micro_post_call_recipe(name: &str) -> Option<Vec<(usize, u8, u8)>> {
	match name {
		"Call" => Some(vec![(M_POST_CALL, 0, 0)]),
		"CallE" => Some(vec![(M_POST_CALLE, 0, 0)]),
		"CallM" => Some(vec![(M_POST_CALLM, 0, 0)]),
		"CallT" => Some(vec![(M_POST_CALLT, 0, 0)]),
		_ => None,
	}
}

fn micro_binary_body(expr: &str, meta: &str, lit: &MicroLits) -> String {
	format!(
		"local u,w=T[1],T[2] if TYP(u)=={number} and TYP(w)=={number} then T[x]={expr} else local f,h=mget(u,{meta}) if not f then local g,h2=mget(w,{meta}) f=g h=h or h2 end if f then T[x]=f(u,w) elseif h then T[x]={expr} else ERR({msg}..TYP(u)..{value},0) end end",
		number = lit.number,
		meta = meta,
		msg = lit.arith_msg,
		value = lit.value_msg,
	)
}

fn micro_compare_body(native: &str, meta: &str, swapped: bool, lit: &MicroLits) -> String {
	let call = if swapped { "f(w,u)" } else { "f(u,w)" };
	format!(
		"local u,w=T[1],T[2] local tu,tw=TYP(u),TYP(w) if (tu=={number} and tw=={number}) or (tu=={string} and tw=={string}) then T[x]={native} else local f,h=mget(u,{meta}) if not f then local g,h2=mget(w,{meta}) f=g h=h or h2 end if f then T[x]={call} elseif h then T[x]={native} else ERR({msg}..tu..{with_}..tw,0) end end",
		number = lit.number,
		string = lit.string,
		meta = meta,
		msg = lit.compare_msg,
		with_ = lit.with_msg,
	)
}

fn call_slot(logical: usize, family: usize) -> String {
	format!("E[E.cb+{}+{}*E.cs]", family * 256, logical)
}

fn frame_slot(logical: usize) -> String {
	format!("E[E.rb+{}*E.cs]", logical)
}

fn micro_op_body(index: usize, lit: &MicroLits, nop: &str) -> String {
	if index >= M_SUPER_BASE {
		return micro_super_body(index, lit, nop);
	}
	match index {
		M_FUSE_LOAD_K => {
			"local q = C[O[2] + 1] V[O[1] + 1] = q".to_string()
		}
		M_FUSE_MOVE => {
			"local q = V[O[2] + 1] V[O[1] + 1] = q".to_string()
		}
		M_FUSE_NEW_TABLE => "local q = {} V[O[1] + 1] = q".to_string(),
		M_FUSE_NOT => "local q = not V[O[2] + 1] V[O[1] + 1] = q".to_string(),
		M_FUSE_GET_UP => {
			"local q = ups[O[2] + 1] local w = q.v[q.i] V[O[1] + 1] = w".to_string()
		}
		M_FUSE_SET_UP => {
			"local q = ups[O[1] + 1] local w = V[O[2] + 1] q.v[q.i] = w".to_string()
		}
		M_FUSE_VARGC => "V[O[1] + 1] = vargc".to_string(),
		M_FUSE_BRANCH_FALSE => {
			"if not V[O[1] + 1] then E.pc = O[2] end".to_string()
		}
		M_FUSE_BRANCH_TRUE => "if V[O[1] + 1] then E.pc = O[2] end".to_string(),
		M_FUSE_VARARG_TABLE => {
			"local q = {} for j = 1, vargc do q[j] = vargs[1 + ((j + E.vd - 1) % vargc)] end V[O[1] + 1] = q".to_string()
		}
		M_READ_V => "T[x] = V[O[y] + 1]".to_string(),
		M_WRITE_V => "V[O[x] + 1] = T[y]".to_string(),
		M_READ_C => "T[x] = C[O[y] + 1]".to_string(),
		M_LOAD_STR => format!(
			"local q=T[y] local st=O[3] local step=O[4] local by={empty}[{byte_}] local ch={empty}[{char_}] local out={empty} for i=1,#q do st=(st*257+step+i)%65521 out=out..ch((by(q,i)-st%256)%256) end T[x]=out",
			empty = lit.empty,
			byte_ = lit.byte_,
			char_ = lit.char_,
		),
		M_NOT => "T[x] = not T[y]".to_string(),
		M_NEW_TABLE => "T[x] = {}".to_string(),
		M_GET_GLOBAL => "T[x] = G[T[y]]".to_string(),
		M_SET_GLOBAL => "G[T[x]] = T[y]".to_string(),
		M_READ_UP => "T[x] = ups[O[y] + 1]".to_string(),
		M_CELL_GET => "local q = T[y]; T[x] = q.v[q.i]".to_string(),
		M_CELL_SET => "local q = T[x]; q.v[q.i] = T[y]".to_string(),
		M_VARGC => "T[x] = vargc".to_string(),
		M_SET_PC => "E.pc = O[x]".to_string(),
		M_BRANCH_FALSE => "if not T[x] then E.pc = O[y] end".to_string(),
		M_BRANCH_TRUE => "if T[x] then E.pc = O[y] end".to_string(),
		M_ADD => micro_binary_body("u+w", &lit.add, lit),
		M_SUB => micro_binary_body("u-w", &lit.sub, lit),
		M_MUL => micro_binary_body("u*w", &lit.mul, lit),
		M_DIV => micro_binary_body("u/w", &lit.div, lit),
		M_MOD => micro_binary_body("u%w", &lit.mod_, lit),
		M_POW => micro_binary_body("u^w", &lit.pow, lit),
		M_IDIV => "T[x] = FLOOR(T[1] / T[2])".to_string(),
		M_UNM => format!(
			"local u=T[y] if TYP(u)=={number} then T[x]=-u else local f,hidden=mget(u,{meta}) if f then T[x]=f(u) elseif hidden then T[x]=-u else ERR({msg}..TYP(u)..{value},0) end end",
			number = lit.number, meta = lit.unm, msg = lit.arith_msg, value = lit.value_msg,
		),
		M_LEN => format!(
			"local u=T[y] local f=HAS_LEN_META and mget(u,{meta}) if f then T[x]=f(u) else T[x]=#u end",
			meta = lit.len,
		),
		M_CONCAT => format!(
			"local u,w=T[1],T[2] local tu,tw=TYP(u),TYP(w) if (tu=={number} or tu=={string}) and (tw=={number} or tw=={string}) then T[x]=u..w else local f,h=mget(u,{meta}) if not f then local g,h2=mget(w,{meta}) f=g h=h or h2 end if f then T[x]=f(u,w) elseif h then T[x]=u..w else ERR({msg}..tu..{value},0) end end",
			number = lit.number, string = lit.string, meta = lit.concat,
			msg = lit.concat_msg, value = lit.value_msg,
		),
		M_LT => micro_compare_body("u<w", &lit.lt, false, lit),
		M_LE => micro_compare_body("u<=w", &lit.le, false, lit),
		M_GT => micro_compare_body("u>w", &lit.lt, true, lit),
		M_GE => micro_compare_body("u>=w", &lit.le, true, lit),
		M_EQ | M_NE => {
			let result = if index == M_NE { "not eqv" } else { "eqv" };
			format!(
				"local u,w=T[1],T[2] local tu,tw=TYP(u),TYP(w) local eqv if tu==tw and (tu=={number} or tu=={string} or tu=={boolean} or tu=={nil_}) then eqv=u==w else local f=mget(u,{meta}) or mget(w,{meta}) if f then eqv=f(u,w) else eqv=u==w end end T[x]={result}",
				number = lit.number, string = lit.string, boolean = lit.boolean,
				nil_ = lit.nil, meta = lit.eq,
			)
		}
		M_LOAD_NIL_RANGE => "for j=1,O[y] do V[S[O[x]+j]]=nil end".to_string(),
		M_TABN => "T[2]=T[2]+1 T[1][T[2]]=T[3]".to_string(),
		M_VARG_TABLE => "local q={} for j=1,vargc do q[j]=vargs[1+((j+E.vd-1)%vargc)] end T[x]=q".to_string(),
		M_VARG_TABN => "for j=1,vargc do T[1][T[2]+j]=vargs[1+((j+E.vd-1)%vargc)] end T[2]=T[2]+vargc".to_string(),
		M_CLOSURE => "V[O[x]+1]=makefn(E.bt[O[y]],V,ups,S)".to_string(),
		M_GET_TABLE => format!(
			"local o,j=T[1],T[2] local r if TYP(o)=={string} then r=o[j] elseif TYP(o)=={table_} then r=RGET(o,j) if r==nil then local f,hidden=mget(o,{index}) if TYP(f)=={function} then r=f(o,j) elseif f~=nil then r=f[j] elseif hidden then r=o[j] end end else local f,hidden=mget(o,{index}) if TYP(f)=={function} then r=f(o,j) elseif f~=nil then r=f[j] elseif hidden then r=o[j] else ERR({msg}..TYP(o)..{value},0) end end T[x]=r",
			table_ = lit.table_, string = lit.string, index = lit.index, function = lit.function,
			msg = lit.index_msg, value = lit.value_msg,
		),
		M_SET_TABLE => format!(
			"local o,j,w=T[1],T[2],T[3] if TYP(o)~={table_} then local f,hidden=mget(o,{newindex}) if TYP(f)=={function} then f(o,j,w) elseif f~=nil then RSET(f,j,w) elseif hidden then o[j]=w else ERR({msg}..TYP(o)..{value},0) end else if RGET(o,j)==nil then local f,hidden=mget(o,{newindex}) if TYP(f)=={function} then f(o,j,w) elseif f~=nil then RSET(f,j,w) elseif hidden then o[j]=w else RSET(o,j,w) end else RSET(o,j,w) end end",
			table_ = lit.table_, newindex = lit.newindex, function = lit.function,
			msg = lit.index_msg, value = lit.value_msg,
		),
		M_NOP => nop.to_string(),
		M_MKSTR => format!(
			"local ms=({mk1}*O[1]+{mk2})%65536 local q1=(O[2]-ms)%65536 local q2=(O[3]-ms)%65536 local q3=(O[4]-ms)%65536 local n=FLOOR(q3/256) local q={{q1%256,FLOOR(q1/256),q2%256,FLOOR(q2/256),q3%256}} local ch={empty}[{char_}] local s={empty} for j=1,n do s=s..ch(q[j]) end V[O[1]+1]=s",
			mk1 = lit.mk1, mk2 = lit.mk2, empty = lit.empty, char_ = lit.char_,
		),
		M_RETURN => {
			let lb = frame_slot(0);
			let ln = frame_slot(1);
			let out_slot = frame_slot(2);
			let total_slot = frame_slot(3);
			format!("local out={{}} local n=O[2] local total if n==255 then local pre=O[4] for j=1,pre do out[j]=V[S[O[1]+j]] end if O[3]==1 then for j=1,vargc do out[pre+j]=vargs[1+((j+E.vd-1)%vargc)] end total=pre+vargc else for j=1,{ln} do local s=S[{lb}+j] if s then out[pre+j]=V[s] else out[pre+j]=X[{lb}+j] end end total=pre+{ln} end else for j=1,n do out[j]=V[S[O[1]+j]] end total=n end {out_slot}=out {total_slot}=total E.done=(E.vd+E.cs)%65521", lb=lb, ln=ln, out_slot=out_slot, total_slot=total_slot)
		}
		M_CALL => {
			let z: Vec<String> = (0..7).map(|i| call_slot(i, 1)).collect();
			format!("local bs=O[1]+1 local f=V[S[bs]] local fn,selfv=resolve_call(f) local off=selfv and 1 or 0 local nargs=O[2]+off local av={{}} if off==1 then av[1]=f end for j=1,O[2] do av[off+j]=V[S[bs+j]] end if O[4]==1 then for j=1,vargc do av[nargs+j]=vargs[1+((j+E.vd-1)%vargc)] end nargs=nargs+vargc end {f}=fn {a}=av {n}=nargs {oa}=O[1] {ob}=O[2] {oc}=O[3] {od}=O[4] E.callx=(E.vd+1)%17", f=z[0], a=z[1], n=z[2], oa=z[3], ob=z[4], oc=z[5], od=z[6])
		}
		M_CALLE => {
			let z: Vec<String> = (0..7).map(|i| call_slot(i, 2)).collect();
			format!("local bs=O[1]+1 local f=V[S[bs]] local fn,selfv=resolve_call(f) local off=selfv and 1 or 0 local nf=O[2] local va=O[4]%2==1 local tail=O[4]>=2 local nargs=nf+off if tail then nargs=nargs+V[S[bs+nf+1]] end local av={{}} if off==1 then av[1]=f end local lim=tail and (nf+V[S[bs+nf+1]]) or nf for j=1,lim do if tail and j>nf then local z=bs+j+1 local s=S[z] if s then av[off+j]=V[s] else av[off+j]=X[z] end else av[off+j]=V[S[bs+j]] end end if va then for j=1,vargc do av[nargs+j]=vargs[1+((j+E.vd-1)%vargc)] end nargs=nargs+vargc end {f}=fn {a}=av {n}=nargs {oa}=O[1] {ob}=O[2] {oc}=O[3] {od}=O[4] E.callx=(E.vd+2)%17", f=z[0], a=z[1], n=z[2], oa=z[3], ob=z[4], oc=z[5], od=z[6])
		}
		M_CALLM => {
			let z: Vec<String> = (0..7).map(|i| call_slot(i, 3)).collect();
			format!("local bs=O[1]+1 local f=V[S[bs]] local fn,selfv=resolve_call(f) local off=selfv and 1 or 0 local nf=O[2] local nt=V[S[bs+nf+1]] local nargs=nf+nt+off local av={{}} if off==1 then av[1]=f end for j=1,nf+nt do if j<=nf then av[off+j]=V[S[bs+j]] else local z=bs+j+1 local s=S[z] if s then av[off+j]=V[s] else av[off+j]=X[z] end end end if O[4]==1 then for j=1,vargc do av[nargs+j]=vargs[1+((j+E.vd-1)%vargc)] end nargs=nargs+vargc end {f}=fn {a}=av {n}=nargs {oa}=O[1] {ob}=O[2] {oc}=O[3] {od}=O[4] E.callx=(E.vd+3)%17", f=z[0], a=z[1], n=z[2], oa=z[3], ob=z[4], oc=z[5], od=z[6])
		}
		M_CALLT => {
			let z: Vec<String> = (0..9).map(|i| call_slot(i, 4)).collect();
			format!("local f=V[S[O[1]+1]] local fn,selfv=resolve_call(f) local off=selfv and 1 or 0 local nf=FLOOR(O[4]/2) local tail=O[4]%2==1 local nt=tail and V[S[O[1]+nf+2]] or 0 local nargs=nf+off+nt local av={{}} if off==1 then av[1]=f end for j=1,nf+nt do if tail and j>nf then local z=O[1]+j+2 local s=S[z] if s then av[off+j]=V[s] else av[off+j]=X[z] end else av[off+j]=V[S[O[1]+j+1]] end end {f}=fn {a}=av {n}=nargs {oa}=O[1] {ob}=O[2] {oc}=O[3] {od}=O[4] {tt}=V[O[2]+1] {tn}=V[O[3]+1] E.callx=(E.vd+4)%17", f=z[0], a=z[1], n=z[2], oa=z[3], ob=z[4], oc=z[5], od=z[6], tt=z[7], tn=z[8])
		}
		M_POST_CALL | M_POST_CALLM => {
			let lb = frame_slot(0);
			let ln = frame_slot(1);
			format!("local bs=O[1]+1 {lb}=bs {ln}=N local wn=N if O[3]~=255 and O[3]>wn then wn=O[3] end for j=1,wn do local s=S[bs+j] if s then V[s]=R[j] else X[bs+j]=R[j] end end", lb=lb, ln=ln)
		}
		M_POST_CALLE => "local bs=O[1]+1 for j=1,N do local s=S[bs+j] if s then V[s]=R[j] else X[bs+j]=R[j] end end V[S[bs]]=N".to_string(),
		M_POST_CALLT => {
			let q = call_slot(7, 4);
			let n = call_slot(8, 4);
			format!("local q={q} local n={n} for j=1,N do q[n+j]=R[j] end V[O[3]+1]=n+N", q=q, n=n)
		}
		_ => unreachable!(),
	}
}

fn micro_super_body(index: usize, lit: &MicroLits, nop: &str) -> String {
	let slot = index
		.checked_sub(M_SUPER_BASE)
		.expect("superoperator index below base");
	let name = *MICRO_SUPER_NAMES
		.get(slot)
		.expect("superoperator index out of range");
	let primitive = micro_primitive_recipe(name).expect("superoperator recipe missing");
	let mut body = String::new();
	for (kind, x, y) in primitive {
		let step = micro_op_body(kind, lit, nop);
		body.push_str(&format!("do local x={x} local y={y} {step} end "));
	}
	body
}

#[cfg(test)]
mod micro_fusion_tests {
	use super::*;

	fn is_fused(kind: usize) -> bool {
		(kind >= M_SUPER_BASE && kind < M_COUNT)
			|| matches!(
				kind,
				M_FUSE_LOAD_K
					| M_FUSE_MOVE
					| M_FUSE_NEW_TABLE
					| M_FUSE_NOT
					| M_FUSE_GET_UP
					| M_FUSE_SET_UP
					| M_FUSE_VARGC
					| M_FUSE_BRANCH_FALSE
					| M_FUSE_BRANCH_TRUE
					| M_FUSE_VARARG_TABLE
			)
	}

	#[test]
	fn safe_internal_sequences_emit_one_superoperator() {
		let cases = [
			("LoadK", M_FUSE_LOAD_K),
			("Move", M_FUSE_MOVE),
			("NewTab", M_FUSE_NEW_TABLE),
			("Not", M_FUSE_NOT),
			("GetUp", M_FUSE_GET_UP),
			("SetUp", M_FUSE_SET_UP),
			("VarArgC", M_FUSE_VARGC),
			("Jf", M_FUSE_BRANCH_FALSE),
			("Jt", M_FUSE_BRANCH_TRUE),
			("VarArgTab", M_FUSE_VARARG_TABLE),
		];
		for (name, expected_kind) in cases {
			assert_eq!(
				micro_recipe(name).unwrap(),
				vec![(expected_kind, 0, 0)],
				"{name} should decode as one fused record"
			);
		}
	}

	#[test]
	fn multi_primitive_superoperators_expand_in_original_order() {
		let mut pool = StrPool::new(false);
		let lit = MicroLits::new(&mut pool, (17, 31), false);
		for (slot, name) in MICRO_SUPER_NAMES.iter().enumerate() {
			let recipe = micro_recipe(name).unwrap();
			assert_eq!(recipe, vec![(M_SUPER_BASE + slot, 0, 0)], "{name}");
			assert!(is_fused(recipe[0].0));
			let primitive = micro_primitive_recipe(name).unwrap();
			assert!(primitive.len() > 1, "{name} should represent a multi-step recipe");
			let body = micro_op_body(recipe[0].0, &lit, "");
			let mut after = 0;
			for (kind, x, y) in primitive {
				let inner = micro_op_body(kind, &lit, "");
				let needle = format!("do local x={x} local y={y} {inner} end ");
				let offset = body[after..]
					.find(&needle)
					.unwrap_or_else(|| panic!("{name} superoperator lost/reordered {inner}"));
				after += offset + needle.len();
			}
		}
	}

	#[test]
	fn every_multi_step_isa_recipe_has_one_superoperator() {
		for name in OP_NAMES {
			if let Some(primitive) = micro_primitive_recipe(name) {
				if primitive.len() > 1 {
					assert!(MICRO_SUPER_NAMES.contains(&name), "{name} lacks a superoperator");
					assert_eq!(micro_recipe(name).unwrap().len(), 1, "{name}");
				}
			}
		}
	}

	#[test]
	fn call_and_return_boundaries_remain_separate_phases() {
		for name in ["Call", "CallE", "CallM", "CallT"] {
			assert_eq!(micro_call_recipe(name).unwrap().len(), 1, "{name}");
			assert_eq!(micro_post_call_recipe(name).unwrap().len(), 1, "{name} post-call");
		}
		assert_eq!(micro_recipe("Return").unwrap(), vec![(M_RETURN, 0, 0)]);
	}

	#[test]
	fn core_string_primitives_bypass_replaceable_library_table() {
		assert_eq!(PRIM_SRC[0], "(\"\")[\"byte\"]");
		assert_eq!(PRIM_SRC[1], "(\"\")[\"sub\"]");
		assert_eq!(PRIM_SRC[2], "(\"\")[\"char\"]");
	}

	#[test]
	fn fused_bodies_do_not_use_shard_scratch_temporaries() {
		let mut pool = StrPool::new(false);
		let lit = MicroLits::new(&mut pool, (17, 31), false);
		for kind in [
			M_FUSE_LOAD_K,
			M_FUSE_MOVE,
			M_FUSE_NEW_TABLE,
			M_FUSE_NOT,
			M_FUSE_GET_UP,
			M_FUSE_SET_UP,
			M_FUSE_VARGC,
			M_FUSE_BRANCH_FALSE,
			M_FUSE_BRANCH_TRUE,
			M_FUSE_VARARG_TABLE,
		] {
			let body = micro_op_body(kind, &lit, "");
			assert!(body.contains("V["), "fused body {kind} lost its destination");
			assert!(!body.contains("T["), "fused body {kind} uses shard scratch state");
		}
	}
}

// R073 体积：n 份 _mx 微解释器逐字节同源（codes/field_perm/carrier 均为参数），
// 工厂化 = 1 份共享函数体 _MXF + 每 shard 一个薄包装闭包。md 滚动方言不变
// （每 shard 仍注册全 kind 集），运行期语义与逐份展开逐字节等价。
#[allow(dead_code)]
fn micro_tree(items: &[(u8, String)]) -> String {
	if items.len() == 1 {
		return items[0].1.clone();
	}
	let mid = items.len() / 2;
	let pivot = items[mid].0;
	format!(
		"if op < {} then {} else {} end",
		pivot,
		micro_tree(&items[..mid]),
		micro_tree(&items[mid..])
	)
}

fn micro_factory_source(
	kinds: &[usize],
	variant: u8,
	lit: &MicroLits,
	nop: &str,
	rng: &mut Rng,
) -> (String, Vec<usize>) {
	let mut items: Vec<(usize, String)> = kinds
		.iter()
		.map(|&i| (i, micro_op_body(i, lit, nop)))
		.collect();
	rng.shuffle(&mut items);
	let order: Vec<usize> = items.iter().map(|x| x.0).collect();
	let dispatch = if items.is_empty() {
		String::new()
	} else {
		match variant % 2 {
			0 => items
				.iter()
				.enumerate()
				.map(|(i, (_, body))| {
					format!(
						"{} op == zC[{}] then {} ",
						if i == 0 { "if" } else { "elseif" },
						i + 1,
						body
					)
				})
				.collect::<String>() + "end",
			_ => items
				.iter()
				.enumerate()
				.map(|(i, (_, body))| {
					format!(
						"{} zC[{}] == op then {} ",
						if i == 0 { "if" } else { "elseif" },
						i + 1,
						body
					)
				})
				.collect::<String>() + "end",
		}
	};
	let src = format!(
		"local function _MXF(E,a,b,c,d,o,R,N,zB,zM,zS,zC,z1,z2,z3) local V,C,S,X,G,vargs,vargc,ups,makefn,mget,resolve_call,HAS_LEN_META,FLOOR,ERR,TYP,GMT,RGET,RSET=E[1],E[2],E[3],E[4],E[5],E[6],E[7],E[8],E[9],E[10],E[11],E[13],E[15],E[16],E[17],E[18],E[19],E[20] local vd=E.vd vargc=(vargc-vd)%65536 local O={{a,b,c,d}} local T={{}} local n=(BYTE(zB,o)-(zM+o*zS)%256)%256 local ip=o+1 local pe=ip+n-1 while ip<=pe do local d1=(BYTE(zB,ip)-(zM+ip*zS)%256)%256 local d2=(BYTE(zB,ip+1)-(zM+(ip+1)*zS)%256)%256 local d3=(BYTE(zB,ip+2)-(zM+(ip+2)*zS)%256)%256 ip=ip+3 local DK={{d1,d2,d3}} local op,x,y=DK[z1],DK[z2],DK[z3] {dispatch} end for _ti=1,3 do T[_ti]=nil end DK=nil for _oi=1,4 do O[_oi]=nil end end ",
		dispatch = dispatch,
	);
	(src, order)
}

fn micro_shard_wrapper(index: usize, shard: &MicroShard, order: &[usize]) -> String {
	let mut logical_pos = [0usize; 3];
	for (physical, logical) in shard.field_perm.iter().enumerate() {
		logical_pos[*logical] = physical + 1;
	}
	let csv = order
		.iter()
		.map(|&k| shard.codes[k].to_string())
		.collect::<Vec<_>>()
		.join(",");
	format!(
		"local _mx{i}=function(E,a,b,c,d,o,R,N) return _MXF(E,a,b,c,d,o,R,N,_mb{i},_ms{i},_mt{i},{{{csv}}},{p1},{p2},{p3}) end ",
		i = index,
		csv = csv,
		p1 = logical_pos[0],
		p2 = logical_pos[1],
		p3 = logical_pos[2],
	)
}

fn encode_micro_program(
	recipe: &[(usize, u8, u8)],
	shard: &MicroShard,
) -> Vec<u8> {
	let mut plain = Vec::with_capacity(recipe.len() * 3);
	for &(kind, x, y) in recipe {
		let logical = [shard.codes[kind], x, y];
		for &logical_index in &shard.field_perm {
			plain.push(logical[logical_index]);
		}
	}
	plain
}

#[derive(Clone)]
struct MicroProgramRef {
	shard: usize,
	offset: usize,
}

fn register_micro_variant(
	recipe: &[(usize, u8, u8)],
	shard_index: usize,
	micro_shards: &[MicroShard],
	micro_used: &mut [std::collections::HashSet<usize>],
	micro_carriers: &mut [Vec<u8>],
	rng: &mut Rng,
) -> MicroProgramRef {
	for &(kind, _, _) in recipe {
		micro_used[shard_index].insert(kind);
	}
	let plain = encode_micro_program(recipe, &micro_shards[shard_index]);
	assert!(plain.len() <= 255, "micro descriptor too large");
	let carrier = &mut micro_carriers[shard_index];
	let offset = carrier.len() + 1;
	carrier.push(plain.len() as u8);
	carrier.extend_from_slice(&plain);
	let pad_seed = rng.int(1, 255) as u8;
	let pad_step = (rng.int(1, 255) | 1) as u8;
	let pad_len = (pad_seed % 4) as usize;
	for i in 0..pad_len {
		carrier.push(pad_seed.wrapping_add((i as u8).wrapping_mul(pad_step)));
	}
	MicroProgramRef { shard: shard_index, offset }
}

fn micro_variant_dispatch(
	variants: &[MicroProgramRef],
	head_args: &str,
	tail_args: &str,
	rng: &mut Rng,
) -> String {
	let mut order: Vec<usize> = (0..variants.len()).collect();
	rng.shuffle(&mut order);
	let mut out = String::new();
	for (branch, &i) in order.iter().enumerate() {
		let v = &variants[i];
		let cond = if branch == 0 {
			if rng.int(0, 1) == 0 {
				format!("if E.md=={}", v.shard)
			} else {
				format!("if {}==E.md", v.shard)
			}
		} else if rng.int(0, 1) == 0 {
			format!("elseif E.md=={}", v.shard)
		} else {
			format!("elseif {}==E.md", v.shard)
		};
		out.push_str(&format!(
			"{cond} then _mx{shard}({head},{offset}{tail}) ",
			cond = cond,
			shard = v.shard,
			offset = v.offset,
			head = head_args,
			tail = tail_args,
		));
	}
	out.push_str("end");
	out
}

fn remap_fragment_fields(
	src: &str,
	map: &std::collections::HashMap<String, String>,
) -> String {
	let bytes = src.as_bytes();
	let mut out = String::with_capacity(src.len());
	let mut i = 0usize;
	while i < bytes.len() {
		if bytes[i] == b'.'
			&& (i == 0 || bytes[i - 1] != b'.')
			&& i + 1 < bytes.len()
			&& (bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_')
		{
			out.push('.');
			i += 1;
			let start = i;
			while i < bytes.len()
				&& (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
			{
				i += 1;
			}
			let field = &src[start..i];
			out.push_str(map.get(field).map(String::as_str).unwrap_or(field));
		} else {
			out.push(bytes[i] as char);
			i += 1;
		}
	}
	out
}

fn chain_epilogue(rng: &mut Rng, wk: &[String; 4]) -> String {
	const NP_POOL: [&str; 6] = ["np", "gz", "hz", "kz", "pz", "wz"];
	const OC_POOL: [&str; 6] = ["oc", "jz", "qz", "xz", "yz", "nz"];
	const H_POOL: [&str; 6] = ["hx", "fx", "zx", "vx", "mx", "jx"];
	const B_POOL: [&str; 6] = ["bi", "bz", "dz", "ez", "fz", "iz"];
	let np = NP_POOL[rng.int(0, 5) as usize];
	let oc = OC_POOL[rng.int(0, 5) as usize];
	let h = H_POOL[rng.int(0, 5) as usize];
	let bi = B_POOL[rng.int(0, 5) as usize];
	let budget = if rng.int(0, 1) == 0 {
		"E.b = E.b - 1; if E.b == 0 then return end; "
	} else {
		"E.b = E.b - 1; if E.b <= 0 then return end; "
	};
	let trans = format!(
		"if {np} < E.bp or {np} > E.be then E.bdec(E, E.bm[{np}]) end; local {bi} = {np} - E.bp + 1",
		np = np,
		bi = bi
	);
	let args = format!(
		"({wa}[{bi}]-E.br)%65536, ({wb}[{bi}]-E.br)%65536, ({wc}[{bi}]-E.br)%65536, ({wd}[{bi}]-E.br)%65536",
		wa = wk[0],
		wb = wk[1],
		wc = wk[2],
		wd = wk[3],
		bi = bi
	);
	match rng.int(0, 4) {
		0 => format!(
			"{budget}local {np} = E.pc; {trans}; local {oc} = (E.ww[{bi}]-E.br)%256; E.pc = {np} + 1; return HW2[({oc} + AV) % 256](E, {args})",
			budget = budget, np = np, trans = trans, oc = oc, bi = bi, args = args
		),
		1 => format!(
			"{budget}local {np} = E.pc; E.pc = {np} + 1; {trans}; local {oc} = (E.ww[{bi}]-E.br)%256; local {h} = HW2[({oc} + AV) % 256]; return {h}(E, {args})",
			budget = budget, np = np, trans = trans, oc = oc, h = h, bi = bi, args = args
		),
		2 => format!(
			"{budget}local {np} = E.pc; {trans}; E.pc = {np} + 1; return HW2[((E.ww[{bi}]-E.br)%256 + AV) % 256](E, {args})",
			budget = budget, np = np, trans = trans, bi = bi, args = args
		),
		3 => format!(
			"{budget}local {np} = E.pc; {trans}; local {h} = HW2[((E.ww[{bi}]-E.br)%256 + AV) % 256]; E.pc = {np} + 1; return {h}(E, {args})",
			budget = budget, np = np, trans = trans, h = h, bi = bi, args = args
		),
		_ => format!(
			"{budget}local {np} = E.pc; {trans}; local {oc} = (E.ww[{bi}]-E.br)%256; local oa, ob, od2, oe = {args}; E.pc = {np} + 1; return HW2[({oc} + AV) % 256](E, oa, ob, od2, oe)",
			budget = budget, np = np, trans = trans, oc = oc, bi = bi, args = args
		),
	}
}

pub fn generate(
	map: &OpMap,
	slot_perm: &[u8; 4],
	slot_perms: &[[u8; 4]],
	lane_dialect: (u8, u8),
	carrier: &Carrier,
	carrier_lengths: &[usize],
	rng: &mut Rng,
	n_fns: usize,
	v15: bool,
	include_loadstr: bool,
	nop_sites: &[Vec<u16>],
	operand_sums: &[u64],
	operand_sums2: &[u64],
	chaff_folds: &[u16],
	chaff_formula: (u16, [u16; 4], u16, u16),
	ck: (u32, u32),
	mk: (u16, u16),
	blobk: (u32, u32, u32, u32),
	tags: [u8; 8],
	consts_safe: bool,
	block_starts: &[Vec<u16>],
	s_indices: &[u8],
	super_insn: bool,
	fv_sets: &[Vec<(u8, u8)>],
	fvck: (u16, u16),
	bind_env: Option<&str>,
	env_slots: Option<&[i64; 4]>,
	executor_bind: bool,
	executor_guard: bool,
	key_host: Option<&super::exguard::KeyHost>,
	env_check: bool,
	guard: bool,
) -> String {
	let active_op_count = if include_loadstr { N_OPS } else { N_OPS - 1 };
	let mut oc_items = Vec::new();
	for (i, name) in OP_NAMES.iter().take(active_op_count).enumerate() {
		oc_items.push(format!("{name} = {}", map.to_wire[0][i]));
	}
	let oc_table = format!("local OC = {{{}}}", oc_items.join(", "));
	let mut ke = KeyEmitter::new(carrier_lengths);
	let mut qf_checksum: i64 = 0;
	for &v in chaff_folds {
		qf_checksum = (qf_checksum * 257 + v as i64) % KEY_MOD;
	}
	let chaff_micro = (((chaff_formula.1[2] as u64) * 269
		+ chaff_formula.0 as u64 * 7
		+ chaff_formula.3 as u64)
		% 65_520
		+ 1) as u16;
	manifest_key("CHAFF_CK", qf_checksum as u64);
	manifest_key("CHAFF_MICRO", chaff_micro as u64);
	let oc_boot: String = if v15 {
		let ocm = rng.int(1, 65535);
		let occ = rng.int(1, 65535);
		let mut pairs: Vec<String> = Vec::new();
		for i in 0..active_op_count {
			let mask = (ocm + i as i64 * occ) % 65536;
			let stored = (map.to_wire[0][i] as i64 + mask) % 256;
			pairs.push(format!("{}, {}", i, stored));
		}
		rng.shuffle(&mut pairs);
		let p1 = rng.int(0, 255);
		let p2 = ((map.nop_alias[0] as i64 - p1) % 256 + 256) % 256;
		let ocm_e = ke.key_expr(ocm, Some(("#ocp", 2 * active_op_count as i64)), rng);
		let occ_e = ke.key_expr(occ, Some(("#ocp", 2 * active_op_count as i64)), rng);
		let nopa_e = ke.key_expr((p1 + p2).rem_euclid(KEY_MOD), None, rng);
		format!(
			"local OCt = {{}}\n  do\n    local ocp = {{{}}}\n    local oi = 1\n    while oi <= #ocp do\n      local x = ocp[oi]\n      OCt[x] = (ocp[oi + 1] - ({} + x * {}) % 65536) % 256\n      oi = oi + 2\n    end\n  end\n  local NOPA = ({}) % 256",
			pairs.join(", "), ocm_e, occ_e, nopa_e
		)
	} else {
		String::new()
	};
	let oc_table = if v15 { oc_boot } else { oc_table };

	let nop = handlers::nop::body(rng);
	let mut pool = StrPool::new(v15);
	let mut fmt_of: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
	for name in OP_NAMES.iter().take(active_op_count) {
		let n = handlers::n_formats(name, v15);
		fmt_of.insert((*name).to_string(), rng.int(0, (n - 1) as i64) as u8);
	}
	let stream_names = ["SA", "SB", "SC", "SD"];
	let mut operand_stream = [0u8; 4];
	for (sl, &op_idx) in slot_perm.iter().enumerate() {
		operand_stream[op_idx as usize] = sl as u8;
	}
	let op_prefix = format!(
		"local a = {}[pc]; local b = {}[pc]; local c = {}[pc]; local d = {}[pc]; pc = pc + 1;",
		stream_names[operand_stream[0] as usize],
		stream_names[operand_stream[1] as usize],
		stream_names[operand_stream[2] as usize],
		stream_names[operand_stream[3] as usize],
	);
	let mut body_of = |name: &str| -> String {
		let core = if name == "Nop" || name == "NopA" {
			nop.clone()
		} else {
			handlers::gen(name, fmt_of[name], v15, &mut pool, mk)
		};
		if v15 {
			if core.is_empty() {
				op_prefix.clone()
			} else {
				format!("{} {}", op_prefix, core)
			}
		} else {
			core
		}
	};
	let mut items: Vec<(String, u8)> = (0..active_op_count)
		.map(|i| (OP_NAMES[i].to_string(), map.to_wire[0][i]))
		.collect();
	if v15 {
		items.push(("NopA".to_string(), map.nop_alias[0]));
	}
	rng.shuffle(&mut items);
	let branches = gen_dispatch_tree(&items, &mut body_of, rng, 0);

	let ladder = |assign: &dyn Fn(&str) -> String| -> String {
		format!(
			"local b1 = BYTE(s, p)
          if b1 < 128 then {a1}; p = p + 1 else
            local b2 = BYTE(s, p + 1)
            if b2 < 128 then {a2}; p = p + 2 else
              local b3 = BYTE(s, p + 2)
              if b3 < 128 then local v = (b1 - 128) + (b2 - 128) * 128 + b3 * 16384; if v >= 2147483648 then v = v - 4294967296 end; {a3}; p = p + 3 else
                local b4 = BYTE(s, p + 3); local v = (b1 - 128) + (b2 - 128) * 128 + (b3 - 128) * 16384 + b4 * 2097152; if v >= 2147483648 then v = v - 4294967296 end; {a4}; p = p + 4
              end
            end
          end",
			a1 = assign("b1"),
			a2 = assign("(b1 - 128) + b2 * 128"),
			a3 = assign("v"),
			a4 = assign("v"),
		)
	};
	let stream_read = |name: &str| -> String {
		let sink: Box<dyn Fn(&str) -> String> =
			Box::new(move |x: &str| format!("{}[i] = {}", name, x));
		format!(
			"{} = {{}}
        for i = 1, ncode do
          {}
        end",
			name,
			ladder(&sink)
		)
	};
	let fold_read = || -> String {
		let sink: Box<dyn Fn(&str) -> String> = Box::new(|x: &str| {
			format!("ck = (ck + {}) % 4294967296", x)
		});
		format!(
			"for i = 1, ncode do
          {}
        end",
			ladder(&sink)
		)
	};
	let fold_read2 = |s_idx: i64| -> String {
		let sink: Box<dyn Fn(&str) -> String> = Box::new(move |x: &str| {
			format!(
				"ck2 = (ck2 + (({} * ncode + i) * ({})) % 4294967296) % 4294967296",
				s_idx, x
			)
		});
		format!(
			"for i = 1, ncode do
          {}
        end",
			ladder(&sink)
		)
	};

	let const_loop = r#"local _mc = ("")["char"]
    local function mb()
      cks = (CKM * cks + CKC) % 268435456
      local b = (BYTE(s, p) - ((cks % 256) + (FLR(cks / 256) % 256) + (FLR(cks / 65536) % 256) + FLR(cks / 16777216)) % 256) % 256
      p = p + 1
      return b
    end
    for i = 1, nconst do
      local t = mb()
      if t == 0 then
        C[i] = nil
      elseif t == 1 then
        C[i] = mb() == 1
      elseif t == 3 then
        local lo = mb()
        local hi = mb()
        local l = lo + hi * 256
        local xs = ""
        for j = 1, l do
          xs = xs .. _mc(mb())
        end
        C[i] = xs
      else
        local m = 0
        local sh = 1
        while true do
          local bb = mb()
          if bb < 128 then m = m + bb * sh; break end
          m = m + (bb - 128) * sh
          sh = sh * 128
        end
        local kp = 0
        local sh2 = 1
        while true do
          local bb = mb()
          if bb < 128 then kp = kp + bb * sh2; break end
          kp = kp + (bb - 128) * sh2
          sh2 = sh2 * 128
        end
        local kk = FLR(kp / 2) - 2048
        local pw = 1
        if kk >= 0 then for j = 1, kk do pw = pw * 2 end else for j = 1, 0 - kk do pw = pw / 2 end end
        local v = m * pw
        if kp % 2 == 1 then v = 0 - v end
        C[i] = v
      end
    end"#;
	let ckj = rng.int(1_048_576, 268_435_455) as i64;
	let ck_consts = format!(
		"local CKM = {}\n  local CKC = {}\n  local BKM = {}\n  local BKC = {}\n  local BSEED = {}\n  local BSTEP = {}\n  local CKJ = {}\n  ",
		ke.key_expr(ck.0 as i64, None, rng),
		ke.key_expr(ck.1 as i64, None, rng),
		ke.key_expr(blobk.0 as i64, None, rng),
		ke.key_expr(blobk.1 as i64, None, rng),
		ke.key_expr(blobk.2 as i64, None, rng),
		ke.key_expr(blobk.3 as i64, None, rng),
		ke.key_expr(ckj, None, rng),
	);
	manifest_key("CKM", ck.0 as u64);
	manifest_key("CKC", ck.1 as u64);
	manifest_key("BKM", blobk.0 as u64);
	manifest_key("BKC", blobk.1 as u64);
	manifest_key("BSEED", blobk.2 as u64);
	manifest_key("BSTEP", blobk.3 as u64);
	manifest_key("CKJ", ckj as u64);
	let tag_defs = format!(
		"local TH = {}\n    local TCK = {}\n    local TU = {}\n    local TC = {}\n    local TS = {}\n    local TDC = {}\n    local NSECT = {}\n    {}",
		tags[0],
		tags[1],
		tags[2],
		tags[3],
		tags[4],
		tags[6],
		if v15 { if super_insn { 7 } else { 6 } } else { 5 },
		if super_insn { format!("local TFV = {}", tags[7]) } else { String::new() }
	);
	let unmask_pre = r#"s = decarrier(s)
    local n = #s
    local ksplit = n
    if ksplit > 64 then ksplit = 64 end
    local um = {}
    local g = (BSEED + (fi - 1) * BSTEP) % 268435456
    for i = 1, ksplit do
      g = (BKM * g + BKC) % 268435456
      um[i] = CHAR((BYTE(s, i) - (((g % 256) + (FLR(g / 256) % 256) + (FLR(g / 65536) % 256) + FLR(g / 16777216)) % 256)) % 256)
    end
    local hf = 0
    for i = 1, ksplit do
      hf = (hf * 31 + BYTE(s, i)) % 268435456
    end
    g = (BSEED + (fi - 1) * BSTEP + hf) % 268435456
    for i = ksplit + 1, n do
      g = (BKM * g + BKC) % 268435456
      um[i] = CHAR((BYTE(s, i) - (((g % 256) + (FLR(g / 256) % 256) + (FLR(g / 65536) % 256) + FLR(g / 16777216)) % 256)) % 256)
    end
    s = table.concat(um)
    __TAGS__local p = 1
    local got = 0
    local cstart = 0"#;
	let walk_head = r#"local tag = BYTE(s, p); p = p + 1
        if tag == TH then
          local v
          v, p = r16(s, p); nregs = v
          v, p = r16(s, p); nparams = v
          v, p = r16(s, p); vararg = v
          got = got + 1
        elseif tag == TCK then
          local v; v, p = r16(s, p); cks = v
          got = got + 1
        elseif tag == TU then
          local nu; nu, p = r16(s, p)
          upsrc = {}
          for i = 1, nu do local v; v, p = r16(s, p); upsrc[i] = v end
          got = got + 1
        elseif tag == TC then
          local v
          v, p = r16(s, p); nconst = v
          v, p = r16(s, p); csl = v
          cstart = p
          p = p + csl
          got = got + 1
        elseif tag == TS then
          local ns; ns, p = r16(s, p)
          S = {}
          for i = 1, ns do local v; v, p = r16(s, p); S[i] = v end
          got = got + 1
        elseif tag == TDC then
          local dl; dl, p = r16(s, p)
          p = p + dl
__FVARM__        else
          local v; v, p = r16(s, p); ncode = v
          W = {}
          for i = 1, ncode do W[i] = BYTE(s, p); p = p + 1 end
          __CODE_EXTRA__
          got = got + 1
        end"#;
	// V17 P1-b：fv 段解析支臂 + 返回表字段（super 开启时）。
	// R072 实测定案（两层根因）：
	//  1. parse_fn 在 v15 下被嵌进 BSS fragment 内层装载函数，词法作用域里
	//     **没有** FN 载体 → fvck 系数不能用 ke.key_expr（data_expr="FN" 解析成 nil）。
	//  2. cks 会被 st2 常量解码（mb/lw 每字节推进 LCG）**就地演化**——在 return
	//     表构造点取 cks 已是演化终值，与编译端 fvm=(原始ckseed*m1+mc) 不符
	//     （实测 Uh≡18139 vs ckseed=59509，fvm 垃圾→trampoline 解码落空槽）。
	//  故：TCK 支臂读完原始 ckseed 立刻快照 fvm0，返回表只引快照；
	//  系数走 obf_num 纯数值混淆（放弃对载体长度的意外绑定，见计划文档 §4）。
	let (fv_walk_arm, fv_ret_fields, fv_locals): (String, String, &str) =
		if super_insn {
			(
				String::from(
					"        elseif tag == TFV then\n          local nv2; nv2, p = r16(s, p)\n          fvt = {}\n          for _fj = 1, nv2 do local fv1; fv1, p = r16(s, p); fvt[_fj] = fv1 end\n          got = got + 1\n",
				),
				String::from(", fv = fvt, fvm = fvm0"),
				", fvt, fvm0",
			)
		} else {
			(String::new(), String::new(), "")
		};
	// TCK 支臂快照注入（仅在 super 开启的 v15 路径生效；non-v15 拒绝 super，
	// off 态 walk_head 保持逐字节不变 = 锚点前提）。
	let e1_e2: Option<(String, String)> = if super_insn {
		Some((obf_num(fvck.0 as u64, rng), obf_num(fvck.1 as u64, rng)))
	} else {
		None
	};
	let walk_head_super: String = if let Some((e1, e2)) = &e1_e2 {
		walk_head.replace(
			"v, p = r16(s, p); cks = v\n          got = got + 1",
			&format!(
				"v, p = r16(s, p); cks = v\n          fvm0 = (v * {e1} + {e2}) % 65536\n          got = got + 1"
			),
		)
	} else {
		String::new()
	};
	let walk_head_emit: &str = if walk_head_super.is_empty() {
		walk_head
	} else {
		&walk_head_super
	};
	let parse_fn = if v15 {
		let code_extra = r#"local p0 = p
          __STREAMS__
          local pend = p
          p = p0
          local ck = 0
          __FOLDS__
          p = p0
          local ck2 = 0
          __FOLDS2__
          CK = ck
          CK2 = ck2
          p = pend"#;
		String::from(
			r#"local function parse(s, fi)
    __UNMASK__
    local st = 1
    local nregs, nparams, vararg, cks, csl, upsrc, nconst, C, S, ncode, W, SA, SB, SC, SD, CK, CK2__FVLOC__
    while st <= 2 do
      if st == 1 then
        __WALK__
        if got >= NSECT then st = 2 end
      else
        p = cstart
        C = {}
        __CONSTS__
        if p ~= cstart + csl then CK = (CK or 0) + p + 1; CK2 = (CK2 or 0) + cstart + csl + 1 end
        st = 3
      end
    end
    return { nregs = nregs, nparams = nparams, vararg = vararg, upsrc = upsrc, C = C, S = S, ck = CK, ck2 = CK2, W = W, SA = SA, SB = SB, SC = SC, SD = SD__FVRET__ }
  end"#
            .replace("__UNMASK__", unmask_pre)
            .replace("__TAGS__", &tag_defs)
            .replace("__WALK__", walk_head_emit)
            .replace("__CODE_EXTRA__", code_extra)
            .replace("__CONSTS__", const_loop)
            .replace(
                "__STREAMS__",
                &format!(
                    "{}\n        {}\n        {}\n        {}",
                    stream_read("SA"),
                    stream_read("SB"),
                    stream_read("SC"),
                    stream_read("SD")
                ),
            )
            .replace(
                "__FOLDS__",
                &format!(
                    "{}\n        {}\n        {}\n        {}",
                    fold_read(),
                    fold_read(),
                    fold_read(),
                    fold_read()
                ),
            )
            .replace("__FVLOC__", fv_locals)
            .replace("__FVARM__", &fv_walk_arm)
            .replace("__FVRET__", &fv_ret_fields)
            .replace(
                "__FOLDS2__",
                &format!(
                    "{}\n        {}\n        {}\n        {}",
                    fold_read2(0),
                    fold_read2(1),
                    fold_read2(2),
                    fold_read2(3)
                ),
            ),
		)
	} else {
		let code_extra =
			r#"SA, SB, SC, SD = rstream(), rstream(), rstream(), rstream()"#;
		String::from(
			r#"local function parse(s, fi)
    __UNMASK__
    local nregs, nparams, vararg, cks, csl, upsrc, nconst, C, ncode, W, SA, SB, SC, SD
    local function rstream()
      local T = {}
      for i = 1, ncode do
        local v, np = r16(s, p); p = np; T[i] = v
      end
      return T
    end
    while got < NSECT do
      __WALK__
    end
    p = cstart
    C = {}
    __CONSTS__
    if p ~= cstart + csl then while true do end end
    return { nregs = nregs, nparams = nparams, vararg = vararg, upsrc = upsrc, C = C, W = W, SA = SA, SB = SB, SC = SC, SD = SD }
  end"#,
		)
		.replace("__UNMASK__", unmask_pre)
		.replace("__TAGS__", &tag_defs)
		.replace("__WALK__", walk_head_emit)
		.replace("__CODE_EXTRA__", code_extra)
		.replace("__CONSTS__", const_loop)
		.replace("__FVARM__", &fv_walk_arm)
		.replace("__FVRET__", &fv_ret_fields)
	};

	let decode_seg = if v15 {
		String::new()
	} else {
		String::from("for i = 1, #FN do PF[i] = parse(FN[i], i) end")
	};

	let parse_fn_vis = if v15 { String::new() } else { parse_fn.clone() };

	let v15_selfmod = if v15 {
		let mut sm = String::new();
		let df_e = ke.dead_zero(rng);
		sm.push_str(&format!("  local DA = {{}}\n  local DP = 1\n  local DF = {}\n  while DF > 0 do\n    local f = DA[DP]\n    if f >= 4 then\n      if f < 6 then\n        if f ~= 5 then DF = 0 else DF = 0 end\n      else DF = 0 end\n    elseif f < 2 then DF = 0\n    else DF = 0 end\n", df_e));
		let n_decoys = rng.int(4, 6);
		for _ in 0..n_decoys {
			let i1 = rng.int(0, (active_op_count - 1) as i64);
			let i2 = rng.int(0, (active_op_count - 1) as i64);
			sm.push_str(&format!(
				"    local oc = DA[DP]\n    if oc then\n      local a = DA[DP]\n      local b = DA[DP]\n      local c = DA[DP]\n      local d = DA[DP]\n      DP = DP + 1\n      if oc == OCt[{}] then DA[DP] = a elseif oc == OCt[{}] then DA[DP] = b else local r = DA[DP](a, b, c, d); if r then DF = r[1] end end\n    end\n",
				i1, i2
			));
		}
		sm.push_str("    DP = DP + 1\n  end\n");
		sm.push_str("  CKM = nil; CKC = nil; BKM = nil; BKC = nil; BSEED = nil; BSTEP = nil\n");
		sm.push_str("  OCt = nil\n");
		sm
	} else {
		String::new()
	};

	if std::env::var("OBSUCVIM_VM_DBG").is_ok() {
		eprintln!("[gen] slot_perm={:?} reserved={}", slot_perm, carrier.reserved);
	}
	let mut pos_of = [1u8; 4];
	for (sl, &op_idx) in slot_perm.iter().enumerate() {
		pos_of[op_idx as usize] = sl as u8 + 1;
	}

	let slots = prim_slots(rng);
	let mut bind_idx: Vec<usize> = (0..PRIM_SRC.len()).collect();
	rng.shuffle(&mut bind_idx);
	let mut p_fill = String::from("local P = {}\n");
	for &i in &bind_idx {
		p_fill.push_str(&format!("  P[{}] = {}\n", slots[i], PRIM_SRC[i]));
	}
	let unpack_lhs: Vec<&str> = bind_idx.iter().map(|&i| PRIM_NAME[i]).collect();
	let unpack_rhs: Vec<String> = bind_idx
		.iter()
		.map(|&i| format!("P[{}]", slots[i]))
		.collect();
	let prim_unpack = format!(
		"local {} = {}",
		unpack_lhs.join(", "),
		unpack_rhs.join(", ")
	);

	let mut run_idx: Vec<usize> = (0..PRIM_SRC.len()).collect();
	rng.shuffle(&mut run_idx);
	let run_lhs: Vec<&str> = run_idx.iter().map(|&i| PRIM_NAME[i]).collect();
	let run_rhs: Vec<String> = run_idx
		.iter()
		.map(|&i| format!("P[{}]", slots[i]))
		.collect();
	let mut run_unpack = format!(
		"local {} = {}",
		run_lhs.join(", "),
		run_rhs.join(", ")
	);

	let akm = (rng.int(1_048_577, 33_000_001) | 1) as u32;
	let akc = (rng.int(1_048_576, 268_000_000) | 1) as u32;
	let aseed = rng.int(1_048_576, 268_435_455) as u32;
	let mut astate = aseed as u64;
	let masked_alpha: Vec<String> = carrier
		.alphabet
		.iter()
		.map(|&ch| {
			astate = (akm as u64 * astate + akc as u64) % 268_435_456;
			ch.wrapping_add(fold_key(astate)).to_string()
		})
		.collect();
	let aseed_e = ke.key_expr(aseed as i64, Some(("#APH", 94)), rng);
	let akm_e = ke.key_expr(akm as i64, Some(("#APH", 94)), rng);
	let akc_e = ke.key_expr(akc as i64, Some(("#APH", 94)), rng);
	manifest_key("APH_SEED", aseed as u64);
	manifest_key("APH_KM", akm as u64);
	manifest_key("APH_KC", akc as u64);
	let al_lines = format!(
		"local APH = {{{}}}\n  local AL = {{}}\n  do\n    local ast = {}\n    for i = 1, 94 do\n      ast = ({} * ast + {}) % 268435456\n      AL[(APH[i] - (((ast % 256) + (FLR(ast / 256) % 256) + (FLR(ast / 65536) % 256) + FLR(ast / 16777216)) % 256)) % 256] = i - 1\n    end\n  end\n",
		masked_alpha.join(", "), aseed_e, akm_e, akc_e
	);
	let tkm = (rng.int(1_048_577, 33_000_001) | 1) as u32;
	let tkc = (rng.int(1_048_576, 268_000_000) | 1) as u32;
	let tkseed = rng.int(1_048_576, 268_435_455) as u32;
	let mut tkstate = tkseed as u64;
	let mut tk_bytes: Vec<u8> = Vec::new();
	for i in 0..10 {
		tk_bytes.extend_from_slice(carrier.tokens[i].as_bytes());
	}
	tk_bytes.extend_from_slice(&CARRIER_SPECIALS);
	let masked_tk: Vec<String> = tk_bytes
		.iter()
		.map(|&b| {
			tkstate = (tkm as u64 * tkstate + tkc as u64) % 268_435_456;
			b.wrapping_add(fold_key(tkstate)).to_string()
		})
		.collect();
	let tkseed_e = ke.key_expr(tkseed as i64, Some(("#TKD", 60)), rng);
	let tkm_e = ke.key_expr(tkm as i64, Some(("#TKD", 60)), rng);
	let tkc_e = ke.key_expr(tkc as i64, Some(("#TKD", 60)), rng);
	manifest_key("TK_SEED", tkseed as u64);
	manifest_key("TK_KM", tkm as u64);
	manifest_key("TK_KC", tkc as u64);
	let tk_lines = format!(
		"local TKD = {{{}}}\n  local TK = {{}}\n  do\n    local tst = {}\n    local tb = {{}}\n    for i = 1, 60 do\n      tst = ({} * tst + {}) % 268435456\n      tb[i] = (TKD[i] - (((tst % 256) + (FLR(tst / 256) % 256) + (FLR(tst / 65536) % 256) + FLR(tst / 16777216)) % 256)) % 256\n    end\n    for i = 0, 9 do\n      TK[CHAR(tb[i * 5 + 1], tb[i * 5 + 2], tb[i * 5 + 3], tb[i * 5 + 4], tb[i * 5 + 5])] = CHAR(tb[50 + i + 1])\n    end\n  end\n",
		masked_tk.join(", "), tkseed_e, tkm_e, tkc_e
	);

	let use_hub = rng.int(0, 1) == 1;
	let mut tup: Vec<usize> = (0..6).collect();
	rng.shuffle(&mut tup);
	let srcs = [
		"W[pc]".to_string(),
		"SA[pc]".to_string(),
		"SB[pc]".to_string(),
		"SC[pc]".to_string(),
		"SD[pc]".to_string(),
		"pc + 1".to_string(),
	];
	let names = ["h0", "h1", "h2", "h3", "h4", "h5"];
	let mut inv = [0usize; 6];
	for (slot, &logical) in tup.iter().enumerate() {
		inv[logical] = slot;
	}
	let fetch_assign = format!(
		"local oc = {n0}; local t1 = {n1}; local t2 = {n2}; local t3 = {n3}; local t4 = {n4}; pc = {n5}\n      local a = t{pa}; local b = t{pb}; local c = t{pc_}; local d = t{pd}",
		n0 = names[inv[0]],
		n1 = names[inv[1]],
		n2 = names[inv[2]],
		n3 = names[inv[3]],
		n4 = names[inv[4]],
		n5 = names[inv[5]],
		pa = pos_of[0],
		pb = pos_of[1],
		pc_ = pos_of[2],
		pd = pos_of[3],
	);
	let hub_ret: Vec<String> = tup.iter().map(|&i| srcs[i].clone()).collect();
	let hub_fn = format!(
		"local function hub(W, SA, SB, SC, SD, pc)\n    return {}\n  end",
		hub_ret.join(", ")
	);
	let (hub_decl, fetch) = if v15 {
		(String::new(), "local oc = W[pc]".to_string())
	} else if use_hub {
		(
			hub_fn,
			format!(
				"local {}, {}, {}, {}, {}, {} = hub(W, SA, SB, SC, SD, pc)\n      {fetch_assign}",
				names[0], names[1], names[2], names[3], names[4], names[5]
			),
		)
	} else {
		let binds: Vec<String> = (0..6)
			.map(|slot| format!("local {} = {}", names[slot], srcs[tup[slot]]))
			.collect();
		(
			"-- inline decode hub".to_string(),
			format!("{}\n      {fetch_assign}", binds.join("\n      ")),
		)
	};

	let mut fields = vec!["W", "SA", "SB", "SC", "SD", "C"];
	if v15 {
		fields.push("S");
	}
	rng.shuffle(&mut fields);
	let mut run_soa = format!(
		"local {} = {}",
		fields.join(", "),
		fields
			.iter()
			.map(|f| format!("pf.{f}"))
			.collect::<Vec<_>>()
			.join(", ")
	);

	let mut params = Vec::new();
	for i in 1..=n_fns {
		params.push(format!("F{i}"));
	}
	let params = params.join(", ");

	let trampoline_src = if v15 {
		const CHAIN_BUDGET: i64 = 16;
		let budget_e = obf_num(CHAIN_BUDGET as u64, rng);
		let frame_cleanup = "local ws={E.ww,E.w0,E.w1,E.w2,E.w3} for wl=1,5 do local wt=ws[wl] if wt then for wx=1,#wt do wt[wx]=nil end end end if E.bq then for qi=1,#E.bq do local bc=E.bch[E.bq[qi]] if bc then for wl=1,5 do local wt=bc[wl] if wt then for wx=1,#wt do wt[wx]=nil end bc[wl]=nil end end end end end E.bch={} E.bq={} E.ww,E.w0,E.w1,E.w2,E.w3=nil,nil,nil,nil,nil E.br=nil E.er=nil ";
		Some(format!(
			"local ti = 0\n    local _tw = CLK and CLK() or 0\n    local _wcount = 0\n    local _minbase = nil\n    local _tslow = 0\n    while true do\n      E.b = {}\n      if E.callx ~= nil then\n        local ck = (E.callx - E.vd) % 17\n        local cx = ck == 1 and XE1 or ck == 2 and XE2 or ck == 3 and XE3 or XE4\n        E.callx = nil\n        cx(E)\n      else\n        local cs = ti % 4\n        local cf = cs == 0 and XC1 or cs == 1 and XC2 or cs == 2 and XC3 or XC4\n        cf(E)\n        ti = ti + 1\n      end\n      E.pn = E.pn + 1\n      if E.pn == E.pd then E.md = (E.md + FLOOR((E.pg + 268435455) / 268435456) * (1 + E.pg % (E.mx - 1))) % E.mx end\n      if CLK and ti % 128 == 0 then local _tn = CLK() local _dt = _tn - _tw _tw = _tn _wcount = _wcount + 1 if _wcount <= 4 then if not _minbase or _dt < _minbase then _minbase = _dt end else if _minbase and _minbase > 0 and _dt > _minbase * 100 then _tslow = _tslow + 1 else _tslow = 0 end if _tslow >= 2 then E.pg=(E.pg+((E.vd%65521)*WM+WB+E.pn*RQ)%268435456)%268435456 E.pd=E.pn+2+(E.vd%7) _tslow=0 end end end\n      if E.done ~= nil then local _fo={out_slot} local _fn={total_slot} {cleanup}for _wi=0,3 do E[E.rb+_wi*E.cs]=nil end return U(_fo, 1, _fn) end\n    end",
			budget_e,
			out_slot = frame_slot(2),
			total_slot = frame_slot(3),
			cleanup = frame_cleanup,
		))
	} else {
		None
	};

	let makefn_decl = if v15 {
		String::from(
			"local function makefn(pf, V, upsf, S)
    local fi = ((pf.sx - PM - 1) % 65521) + 1
    local qf = QF[fi]
    local mst = (qf * MQ + fi * MI + MA) % 65536
    local mu = pf.us
    local c = {}
    for i = 1, #mu do
      mst = (mst * MM + MA + i) % 65536
      local src = (mu[i] - mst) % 65536
      if src >= 49152 then
        c[i] = upsf[src - 49152]
      elseif src >= 32768 then
        c[i] = { v = V[S[src - 32768]], i = 1 }
      else
        c[i] = { v = V, i = S[src] }
      end
      src = nil
    end
    local npa = (pf.np - (qf * NQ + fi * NI + MA) % 65536) % 65536
    local vd = (qf * NQ + fi * NI + MM) % 65521
    mu = nil
    mst = nil
    return function(...)
      local argc = SEL(\"#\", ...)
      local all = { ... }
      local vargc = argc - npa
      if vargc < 0 then vargc = 0 end
      local vargs = {}
      for i = 1, vargc do vargs[1 + ((i + vd - 1) % vargc)] = all[npa + i] end
      local V2 = {}
      for i = 1, npa do V2[pf.S[i]] = all[i] end
      -- 增量⑲: the trampoline lives INSIDE the closure (one frame per
      -- recursion level instead of closure+run)
      local E = newE(pf, V2, c, vargs, vargc)
      TRAMPOLINE
    end
  end",
		)
		.replace("TRAMPOLINE", trampoline_src.as_deref().unwrap())
	} else {
		String::from(
			"local function makefn(idx, V, upsf)
    local pf = PF[idx]
    local c = {}
    for i = 1, #pf.upsrc do
      local src = pf.upsrc[i]
      if src >= 49152 then
        c[i] = upsf[src - 49152]
      elseif src >= 32768 then
        c[i] = { v = V[src - 32768], i = 1 }
      else
        c[i] = { v = V, i = src }
      end
    end
    return function(...)
      local argc = SEL(\"#\", ...)
      local all = { ... }
      local vargc = argc - pf.nparams
      if vargc < 0 then vargc = 0 end
      local vargs = {}
      for i = 1, vargc do vargs[i] = all[pf.nparams + i] end
      local V2 = {}
      for i = 1, pf.nparams do V2[i] = all[i] end
      -- real TCO: tail call into run so deep tail recursion reuses the
      -- frame instead of stacking one per level
      return run(pf, V2, c, vargs, vargc)
    end
  end",
		)
	};

	let u16_fn = r#"local function u16(B, p)
    return BYTE(B, p) + BYTE(B, p + 1) * 256
  end"#;
	let r16_fn = r#"local function r16(B, p)
    local b1 = BYTE(B, p)
    if b1 < 128 then
      return b1, p + 1
    end
    local b2 = BYTE(B, p + 1)
    if b2 < 128 then
      return (b1 - 128) + b2 * 128, p + 2
    end
    local b3 = BYTE(B, p + 2)
    if b3 < 128 then
      local v = (b1 - 128) + (b2 - 128) * 128 + b3 * 16384
      if v >= 2147483648 then v = v - 4294967296 end
      return v, p + 3
    end
    local b4 = BYTE(B, p + 3)
    local v = (b1 - 128) + (b2 - 128) * 128 + (b3 - 128) * 16384 + b4 * 2097152
    if v >= 2147483648 then v = v - 4294967296 end
    return v, p + 4
  end"#;
	let decarrier_fn = format!(
		r#"local function decarrier(s)
    local acc = ""
    local i = 1
    local n = #s
    local resv = {}
    while i <= n do
      if BYTE(s, i) == resv then
        acc = acc .. (TK[SUB(s, i, i + 4)] or "")
        i = i + 5
      else
        acc = acc .. SUB(s, i, i)
        i = i + 1
      end
    end
    local raw = ""
    n = #acc
    for i = 1, n, 5 do
      local v = 0
      v = v * 94 + AL[BYTE(acc, i)]
      v = v * 94 + AL[BYTE(acc, i + 1)]
      v = v * 94 + AL[BYTE(acc, i + 2)]
      v = v * 94 + AL[BYTE(acc, i + 3)]
      v = v * 94 + AL[BYTE(acc, i + 4)]
      local b1 = v % 256; v = FLR(v / 256)
      local b2 = v % 256; v = FLR(v / 256)
      local b3 = v % 256; v = FLR(v / 256)
      local b4 = v % 256
      raw = raw .. CHAR(b1, b2, b3, b4)
    end
    local ln = BYTE(raw, 1) + BYTE(raw, 2) * 256 + BYTE(raw, 3) * 65536 + BYTE(raw, 4) * 16777216
    return SUB(raw, 5, 4 + ln)
  end"#,
		carrier.reserved
	);

	let mut helpers = vec![u16_fn.to_string(), r16_fn.to_string(), decarrier_fn];
	rng.shuffle(&mut helpers);
	let helpers = format!("{}\n  {}\n  {}", al_lines, tk_lines, helpers.join("\n  "));

	let wk = if v15 {
		[
			"E.w0".to_string(),
			"E.w1".to_string(),
			"E.w2".to_string(),
			"E.w3".to_string(),
		]
	} else {
		[
			format!("E.w{}", operand_stream[0]),
			format!("E.w{}", operand_stream[1]),
			format!("E.w{}", operand_stream[2]),
			format!("E.w{}", operand_stream[3]),
		]
	};
	let n_micro_shards = if v15 { rng.int(3, 5) as usize } else { 1 };
	let mut micro_route_steps: Vec<usize> = (1..n_micro_shards)
		.filter(|s| {
			let mut a = *s;
			let mut b = n_micro_shards;
			while b != 0 {
				let r = a % b;
				a = b;
				b = r;
			}
			a == 1
		})
		.collect();
	if micro_route_steps.is_empty() {
		micro_route_steps.push(1);
	}
	let micro_route_base = if v15 {
		rng.int(0, n_micro_shards as i64 - 1) as usize
	} else {
		0
	};
	let micro_route_step = micro_route_steps
		[rng.int(0, micro_route_steps.len() as i64 - 1) as usize];
	if v15 {
		manifest_key("MICRO_ROUTE_BASE", micro_route_base as u64);
		manifest_key("MICRO_ROUTE_STEP", micro_route_step as u64);
		manifest_key("MICRO_ROUTE_N", n_micro_shards as u64);
	}
	let env_names = [
		"V", "C", "S", "O", "G", "vargs", "(vargc+vd)%65536", "ups", "makefn", "mget",
		"resolve_call", "callcap", "HAS_LEN_META", "CHAR", "FLOOR", "ERR",
		"TYP", "GMT", "RGET", "RSET", "U", "MS",
		"AV", "HW2",
	];
	let e_table = format!(
		"{{{}, pc = 1, b = 0, callx = nil, done = nil, vd = vd, cs = CS, cb = 25 + (vd % 4096) * 2048, rb = 1305 + (vd % 4096) * 2048, tp = TP, pn = 0, pd = 2 + ((QF[fi] * RQ + fi * RI + vd) % 15), pg = pg, mx = {}, md = (fi * {} + {}) % {}}}",
		env_names.join(", "), n_micro_shards, micro_route_step, micro_route_base, n_micro_shards
	);
	let hfrag: String;
	let handler_defs = if v15 {
		let call_names = ["Call", "CallE", "CallM", "CallT"];
		let micro_lits = MicroLits::new(&mut pool, mk, include_loadstr);
		let mut micro_shards = Vec::with_capacity(n_micro_shards);
		let mut micro_used: Vec<std::collections::HashSet<usize>> =
			vec![std::collections::HashSet::new(); n_micro_shards];
		let mut micro_fill = String::new();
		for i in 0..n_micro_shards {
			micro_shards.push(new_micro_shard(rng, i as u8));
		}
		let mut micro_carriers: Vec<Vec<u8>> = vec![Vec::new(); n_micro_shards];
		for (name, wire) in &items {
			if let Some(recipe) = micro_recipe(name) {
				let mut variants = Vec::with_capacity(n_micro_shards);
				for shard in 0..n_micro_shards {
					let v = register_micro_variant(
						&recipe,
						shard,
						&micro_shards,
						&mut micro_used,
						&mut micro_carriers,
						rng,
					);
					variants.push(v);
				}
				let micro_tail = if name == "Return" {
					"return".to_string()
				} else {
					chain_epilogue(rng, &wk)
				};
				let route = micro_variant_dispatch(&variants, "E,a,b,c,d", "", rng);
				micro_fill.push_str(&format!(
					"HW2[({wire}+AV)%256]=function(E,a,b,c,d) {route}; {tail} end ",
					wire = *wire as u16,
					route = route,
					tail = micro_tail,
				));
				continue;
			}

			let ci = call_names
				.iter()
				.position(|n| *n == name.as_str())
				.unwrap_or_else(|| panic!("H4-5C source handler fallback reached: {}", name));

			let post_recipe = micro_post_call_recipe(name).expect("post-call recipe");
			let mut post_variants = Vec::with_capacity(n_micro_shards);
			for shard in 0..n_micro_shards {
				let v = register_micro_variant(
					&post_recipe,
					shard,
					&micro_shards,
					&mut micro_used,
					&mut micro_carriers,
					rng,
				);
				post_variants.push(v);
			}
			let family = ci + 1;
			let z: Vec<String> = (0..7).map(|i| call_slot(i, family)).collect();
			let post_route = micro_variant_dispatch(
				&post_variants,
				&format!("E,{},{},{},{}", z[3], z[4], z[5], z[6]),
				",_r,_r.n",
				rng,
			);
			let post_tail = chain_epilogue(rng, &wk);
			micro_fill.push_str(&format!(
				"CX{executor}=function(E) local _r=E.tp({func}(E[21]({args},1,{nargs}))) {route}; for _ci=0,8 do E[E.cb+{page}+_ci*E.cs]=nil end {tail} end ",
				executor = family,
				page = family * 256,
				func = z[0],
				args = z[1],
				nargs = z[2],
				route = post_route,
				tail = post_tail,
			));

			let recipe = micro_call_recipe(name).expect("numeric Call recipe");
			let mut variants = Vec::with_capacity(n_micro_shards);
			for shard in 0..n_micro_shards {
				let v = register_micro_variant(
					&recipe,
					shard,
					&micro_shards,
					&mut micro_used,
					&mut micro_carriers,
					rng,
				);
				variants.push(v);
			}
			let route = micro_variant_dispatch(&variants, "E,a,b,c,d", "", rng);
			micro_fill.push_str(&format!(
				"HW2[({wire}+AV)%256]=function(E,a,b,c,d) {route}; return end ",
				wire = *wire as u16,
				route = route,
			));
		}
		// V17 P1-b：super-insn 发射端——tail-less 白名单件 + trampoline 派发槽。
		// 码位布局（防幻影修正：nop_alias 已占 base+43，空闲区自 base+44 起 20 码位）：
		//   trampoline_k : base(t)+44+k  (k < FUSE_TOP_K)
		//   tail-less_j  : base(t)+48+j  (j = FUSE_TAIL_OPS 内序号)
		// trampoline 运行时经 E.bt.fv[k+1]（+fvm 解掩码）现解本函数词汇的
		// (wA,wB) 两个 tail-less 码位，顺序内嵌调用；链尾照旧 chain_epilogue。
		if super_insn {
			let mut used_t: std::collections::BTreeSet<usize> =
				std::collections::BTreeSet::new();
			for (fi, v) in fv_sets.iter().enumerate() {
				if !v.is_empty() && fi < s_indices.len() {
					used_t.insert(s_indices[fi] as usize);
				}
			}
			for &t in &used_t {
				for (j, op) in super::fuse::FUSE_TAIL_OPS.iter().enumerate() {
					let op_name = OP_NAMES[super::isa::op_index(*op)];
					let recipe = micro_recipe(op_name)
						.expect("whitelist op must have a micro recipe");
					let mut variants = Vec::with_capacity(n_micro_shards);
					for shard in 0..n_micro_shards {
						let v = register_micro_variant(
							&recipe,
							shard,
							&micro_shards,
							&mut micro_used,
							&mut micro_carriers,
							rng,
						);
						variants.push(v);
					}
					let route = micro_variant_dispatch(&variants, "E,a,b,c,d", "", rng);
					micro_fill.push_str(&format!(
						"HW2[({w}+AV)%256]=function(E,a,b,c,d) {route}; return end ",
						w = super::isa::OpMap::fused_tail_code(t, j) as u16,
						route = route,
					));
				}
				for k in 0..super::fuse::FUSE_TOP_K {
					let w = super::isa::OpMap::fused_tramp_code(t, k) as u16;
					let tail = chain_epilogue(rng, &wk);
					micro_fill.push_str(&format!(
						"HW2[({w}+AV)%256]=function(E,a,b,c,d) local fv=E.bt.fv local fe=(fv[{k1}]-E.bt.fvm)%65536 local wa=fe%256 HW2[(wa+AV)%256](E,a,b,c,d) fe=((fe-wa)/256)%256 HW2[(fe+AV)%256](E,c,d,c,d); {tail} end ",
						w = w,
						k1 = k + 1,
						tail = tail,
					));
				}
			}
			manifest_key("FUSE_DIALECTS", used_t.len() as u64);
		}
		let micro_wrappers = std::mem::take(&mut micro_fill);
		let mut micro_checksum: u64 = 0;
		for carrier in &micro_carriers {
			for &value in carrier {
				micro_checksum = (micro_checksum * 257 + value as u64) % KEY_MOD as u64;
			}
		}
		micro_fill.push_str("local _mc=0 ");
		for i in 0..n_micro_shards {
			let at_km = (rng.int(1_048_577, 33_000_001) | 1) as i64;
			let at_kc = rng.int(1_048_576, 268_000_000) as i64;
			let at_seed = rng.int(1_048_576, 268_435_455) as i64;
			let sess_coef = rng.int(1, 255) as i64;
			let sess_step = (rng.int(1, 255) | 1) as i64;
			let mut state = at_seed as u64;
			let mut encrypted = micro_carriers[i].clone();
			for byte in &mut encrypted {
				state = (at_km as u64 * state + at_kc as u64) % KEY_MOD as u64;
				*byte = byte.wrapping_add(fold_key(state));
			}
			let km_e = ke.key_expr(at_km, None, rng);
			let kc_e = ke.key_expr(at_kc, None, rng);
			let seed_e = ke.key_expr(at_seed, None, rng);
			let coef_e = obf_num(sess_coef as u64, rng);
			let step_e = obf_num(sess_step as u64, rng);
			manifest_key(&format!("MICRO_AT_KM_{}", i), at_km as u64);
			manifest_key(&format!("MICRO_AT_KC_{}", i), at_kc as u64);
			manifest_key(&format!("MICRO_AT_SEED_{}", i), at_seed as u64);
			manifest_key(&format!("MICRO_SESSION_COEF_{}", i), sess_coef as u64);
			manifest_key(&format!("MICRO_SESSION_STEP_{}", i), sess_step as u64);
			micro_fill.push_str(&format!(
				"local _mb{shard} local _ms{shard}=(SALT%256+(CHQ%256)*({coef})+{shard}*17)%256 local _mt{shard}=({sstep}) do local _src={carrier} local _out=\"\" local _ch=(\"\")[\"char\"] local _g=({seed}) local _akm=({km}) local _akc=({kc}) for _mi=1,#_src do _g=(_akm*_g+_akc)%268435456 local _v=(BYTE(_src,_mi)-((_g%256)+(FLR(_g/256)%256)+(FLR(_g/65536)%256)+FLR(_g/16777216))%256)%256 _mc=(_mc*257+_v)%268435456 _out=_out.._ch((_v+(_ms{shard}+_mi*_mt{shard})%256)%256) end _mb{shard}=_out _src=nil _out=nil _g=nil _akm=nil _akc=nil end ",
				shard = i,
				coef = coef_e,
				sstep = step_e,
				carrier = lua_dstr(&encrypted),
				seed = seed_e,
				km = km_e,
				kc = kc_e,
			));
		}
		// R073：工厂化发射——先出一份共享 _MXF，再按 shard 出薄包装。
		{
			let mut kinds_set: std::collections::HashSet<usize> =
				std::collections::HashSet::new();
			for used in &micro_used {
				kinds_set.extend(used.iter().copied());
			}
			let mut kinds_all: Vec<usize> = kinds_set.into_iter().collect();
			kinds_all.sort_unstable();
			if !kinds_all.is_empty() {
				let variant = rng.int(0, 1) as u8;
				let (fsrc, order) = micro_factory_source(&kinds_all, variant, &micro_lits, &nop, rng);
				micro_fill.push_str(&fsrc);
				for (i, shard) in micro_shards.iter().enumerate() {
					micro_fill.push_str(&micro_shard_wrapper(i, shard, &order));
				}
			}
		}
		micro_fill.push_str(&micro_wrappers);
		let chaff_micro_e = obf_num(chaff_micro as u64, rng);
		let micro_chaff_expected =
			(micro_checksum as i64 + qf_checksum * chaff_micro as i64) % KEY_MOD;
		let micro_expected_e = ke.key_expr(micro_chaff_expected, None, rng);
		micro_fill.push_str(&format!(
			"_mc=(_mc+CHQ*({chaff}))%268435456 MZD=(_mc-({expected}))%268435456 ",
			chaff = chaff_micro_e,
			expected = micro_expected_e,
		));
		manifest_key("MICRO_CK", micro_checksum);
		manifest_key("MICRO_CHAFF_CK", micro_chaff_expected as u64);
		for i in 0..4usize {
			micro_fill.push_str(&format!(
				"CT{}=function(E) local AV,HW2=E[23],E[24] {} end ",
				i + 1,
				chain_epilogue(rng, &wk)
			));
		}
		let pseed = rng.int(1_048_576, 268_435_455) as i64;
		let pstep = rng.int(1_048_576, 268_435_455) as i64;
		let pkm = (rng.int(1_048_577, 33_000_001) | 1) as i64;
		let pkc = rng.int(1_048_576, 268_000_000) as i64;
		let pseed_e = ke.key_expr(pseed, None, rng);
		let pstep_e = ke.key_expr(pstep, None, rng);
		let pkm_e = ke.key_expr(pkm, None, rng);
		let pkc_e = ke.key_expr(pkc, None, rng);
		manifest_key("PF_SEED", pseed as u64);
		manifest_key("PF_STEP", pstep as u64);
		manifest_key("PF_KM", pkm as u64);
		manifest_key("PF_KC", pkc as u64);
		let pblock = rng.int(1_048_576, 268_435_455) as i64;
		let pblock_e = ke.key_expr(pblock, None, rng);
		manifest_key("PF_BLOCK", pblock as u64);
		        let closure_idx = OP_NAMES.iter().position(|n| *n == "Closure").unwrap();
        let closure_cond = if v15 {
            (0..4)
                .map(|tt| format!("Wt[j]=={}", map.to_wire[tt][closure_idx]))
                .collect::<Vec<_>>()
                .join(" or ")
        } else {
            format!("Wt[j]=={}", map.to_wire[0][closure_idx])
        };
		let (cenv_probe, cenv_exp_v, cenv_active) = if let Some(es) = env_slots {
			let cea: Vec<i64> = (0..5).map(|_| rng.int(1, 999)).collect();
			let cec: Vec<i64> = (0..9).map(|_| rng.int(1, 999)).collect();
			let fold31c = |t: &str| -> i64 {
				let mut h: i64 = 0;
				for b in t.bytes() {
					h = (h * 31 + b as i64) % KEY_MOD;
				}
				h
			};
			let cf_ty = fold31c("Vector3Vector3");
			let cf_ts = fold31c(&format!("{}, {}, {}", cea[0], cea[1], cea[2]));
			let cesum = (cea[0] * cec[0]
				+ cea[1] * cec[1]
				+ cea[2] * cec[2]
				+ cea[3] * cec[3]
				+ cea[4] * cec[4])
				% KEY_MOD;
			let ceinner = (cesum * cec[5] + cf_ty * cec[6] + cf_ts * cec[7]) % KEY_MOD;
			let cenv_exp = (ceinner * cec[8]) % KEY_MOD;
			manifest_key("CENV_EXP", cenv_exp as u64);
			let probe = format!(
				"CENVPROBE = function() local cok, cef = PCAL(function() local E1 = b[{s0}]({a0}, {a1}, {a2}) local E7 = b[{s0}]({a0b}, {a1b}, {a2b}) local E3 = 0 do local t1 = b[{s3}](E1) for pi = 1, #t1 do E3 = (E3 * 31 + BYTE(t1, pi)) % 268435456 end local t2 = b[{s3}](E7) for pi = 1, #t2 do E3 = (E3 * 31 + BYTE(t2, pi)) % 268435456 end end local E4 = b[{s1}]({a3}, {a4}) local E6 = 0 do local ts = TSTR(E1) for pi = 1, #ts do E6 = (E6 * 31 + BYTE(ts, pi)) % 268435456 end end return (((E1.X * {c0} + E1.Y * {c1} + E1.Z * {c2} + E4.X * {c3} + E4.Y * {c4}) * {c5} + E3 * {c6} + E6 * {c7}) * {c8}) % 268435456 end) if not cok then cef = 99999999 end return cef end\n    ",
				s0 = es[0], s1 = es[1], s3 = es[3],
				a0 = obf_num(cea[0] as u64, rng),
				a1 = obf_num(cea[1] as u64, rng),
				a2 = obf_num(cea[2] as u64, rng),
				a0b = obf_num((cea[0] + 1) as u64, rng),
				a1b = obf_num((cea[1] + 2) as u64, rng),
				a2b = obf_num((cea[2] + 3) as u64, rng),
				a3 = obf_num(cea[3] as u64, rng),
				a4 = obf_num(cea[4] as u64, rng),
				c0 = obf_num(cec[0] as u64, rng),
				c1 = obf_num(cec[1] as u64, rng),
				c2 = obf_num(cec[2] as u64, rng),
				c3 = obf_num(cec[3] as u64, rng),
				c4 = obf_num(cec[4] as u64, rng),
				c5 = obf_num(cec[5] as u64, rng),
				c6 = obf_num(cec[6] as u64, rng),
				c7 = obf_num(cec[7] as u64, rng),
				c8 = obf_num(cec[8] as u64, rng),
			);
			(probe, cenv_exp, true)
		} else {
			(String::new(), 0, false)
		};
		let (exg_src, exg_consts) = if executor_guard {
			let (s, c) = super::exguard::build(rng, key_host);
			(Some(s), Some(c))
		} else {
			(None, None)
		};
		let env_src = if env_check {
			Some(super::envcheck::build(rng, env_check && executor_bind))
		} else {
			None
		};
		let xte_off = if let Some(c) = &exg_consts {
			format!(" + {}", obf_num(c.xte, rng))
		} else {
			String::new()
		};
		let (xm3, xm4, xtec_off) = if let Some(c) = &exg_consts {
let xm3v = rng.int(1024, 999_999) as u64;
			let xm4v = rng.int(1, (268_435_456u64 - 1) as i64) as u64;
			let xtec = (c.attest_token * xm3v + xm4v) % 268_435_456;
			(xm3v, xm4v, format!(" + {}", obf_num(xtec, rng)))
		} else {
			(0, 0, String::new())
		};
		let (chaff_mul, chaff_coeffs, chaff_pos, chaff_fn) = chaff_formula;
		let chaff_const = ((chaff_coeffs[1] as u64 * 263
			+ chaff_fn as u64 * 5
			+ chaff_pos as u64 * 3)
			% 65_520
			+ 1) as u16;
		let chaff_const_e = obf_num(chaff_const as u64, rng);
		manifest_key("CHAFF_CONST", chaff_const as u64);
		let dialect_coeff = |salt: u64| -> u16 {
			(((chaff_mul as u64 * (271 + salt)
				+ chaff_pos as u64 * (13 + salt)
				+ chaff_fn as u64 * (7 + salt)
				+ chaff_coeffs[(salt as usize) % 4] as u64)
				% 65_520)
				+ 1) as u16
		};
		let dialect = [
			dialect_coeff(1),
			dialect_coeff(2),
			dialect_coeff(3),
			dialect_coeff(4),
			dialect_coeff(5),
			dialect_coeff(6),
		];
		let dialect_e: Vec<String> = dialect
			.iter()
			.map(|v| obf_num(*v as u64, rng))
			.collect();
		assert_eq!(slot_perms.len(), n_fns, "H4-5B lane dialect count");
		let mut lane_perm_ck = 0u64;
		for perm in slot_perms {
			for &v in perm {
				lane_perm_ck = (lane_perm_ck * 5 + v as u64 + 1) % KEY_MOD as u64;
			}
		}
		manifest_key("DIALECT_LANE_CK", lane_perm_ck);
		let lane_base_e = obf_num(lane_dialect.0 as u64, rng);
		let lane_step_e = obf_num(lane_dialect.1 as u64, rng);
		manifest_key("DIALECT_LANE_BASE", lane_dialect.0 as u64);
		manifest_key("DIALECT_LANE_STEP", lane_dialect.1 as u64);
		for (name, value) in ["FM_Q", "FM_I", "FC_Q", "FC_I", "FB_Q", "FB_I"]
			.iter()
			.zip(dialect.iter())
		{
			manifest_key(&format!("DIALECT_{}", name), *value as u64);
		}
		let tag_base = ((chaff_fn as u64 * 17 + chaff_pos as u64) % 720) as u16;
		let mut tag_step = ((chaff_mul as u64 * 19 + chaff_coeffs[3] as u64) % 719 + 1) as u16;
		loop {
			let mut a = tag_step as u32;
			let mut b = 720u32;
			while b != 0 {
				let r = a % b;
				a = b;
				b = r;
			}
			if a == 1 {
				break;
			}
			tag_step = (tag_step % 719) + 1;
		}
		let tag_base_e = obf_num(tag_base as u64, rng);
		let tag_step_e = obf_num(tag_step as u64, rng);
		manifest_key("DIALECT_TAG_BASE", tag_base as u64);
		manifest_key("DIALECT_TAG_STEP", tag_step as u64);
		let rekey = [
			dialect_coeff(7),
			dialect_coeff(8),
			dialect_coeff(9),
			dialect_coeff(10),
		];
		let rekey_e: Vec<String> = rekey
			.iter()
			.map(|v| obf_num(*v as u64, rng))
			.collect();
		for (name, value) in ["PERIOD_Q", "PERIOD_I", "STATE_Q", "STATE_P"]
			.iter()
			.zip(rekey.iter())
		{
			manifest_key(&format!("DIALECT_REKEY_{}", name), *value as u64);
		}
		let block_cache_cap =
			((tag_base as u64 + lane_dialect.0 as u64 + chaff_fn as u64) % 3) as u8;
		let block_cache_cap_e = obf_num(block_cache_cap as u64, rng);
		manifest_key("BLOCK_CACHE_CAP", block_cache_cap as u64);
		let window_mul =
			((rekey[2] as u64 * 281 + tag_step as u64 * 3) % 65_535 + 1) as u16;
		let window_block =
			((rekey[3] as u64 * 283 + lane_dialect.1 as u64 * 5) % 65_535 + 1) as u16;
		let window_mul_e = obf_num(window_mul as u64, rng);
		let window_block_e = obf_num(window_block as u64, rng);
		manifest_key("WINDOW_RESEED_M", window_mul as u64);
		manifest_key("WINDOW_RESEED_B", window_block as u64);
		let call_slot_step = ((rekey[0] as u64 + tag_step as u64 + chaff_fn as u64) % 16 + 1) as u8;
		let h7_seed = qf_checksum as u64
			^ (chaff_mul as u64).rotate_left(11)
			^ (tag_base as u64).rotate_left(23)
			^ (window_mul as u64).rotate_left(37);
		let mut h7_rng = Rng::new(h7_seed ^ 0x4834_3744_4941_4c45);
		let call_slot_step_e = obf_num(call_slot_step as u64, &mut h7_rng);
		manifest_key("CALL_SLOT_STEP", call_slot_step as u64);
		let metadata = [
			dialect_coeff(11),
			dialect_coeff(12),
			dialect_coeff(13),
			dialect_coeff(14),
			dialect_coeff(15),
			dialect_coeff(16),
			dialect_coeff(17),
			dialect_coeff(18),
			dialect_coeff(19),
			dialect_coeff(20),
		];
		let metadata_e: Vec<String> = metadata
			.iter()
			.map(|v| obf_num(*v as u64, &mut h7_rng))
			.collect();
		for (name, value) in [
			"UP_Q", "UP_I", "UP_M", "UP_A", "NP_Q", "NP_I",
			"PID_A", "CHILD_M", "CHILD_Q", "CHILD_I",
		]
			.iter()
			.zip(metadata.iter())
		{
			manifest_key(&format!("METADATA_{}", name), *value as u64);
		}
		let const_num_step = ((ckj * 257 + metadata[4] as i64 * 17 + 1_048_576) % (KEY_MOD - 1)) + 1;
		let mut const_str_step = ((ckj * 263 + metadata[5] as i64 * 19 + 2_097_152) % (KEY_MOD - 1)) + 1;
		if const_str_step == const_num_step {
			const_str_step = const_str_step % (KEY_MOD - 1) + 1;
		}
		let const_num_step_e = obf_num(const_num_step as u64, &mut h7_rng);
		let const_str_step_e = obf_num(const_str_step as u64, &mut h7_rng);
		manifest_key("CONST_NUM_STEP", const_num_step as u64);
		manifest_key("CONST_STR_STEP", const_str_step as u64);
		let c_block_enc = if consts_safe {
			let cenv_off = if cenv_active {
				format!(" + {}", obf_num(cenv_exp_v as u64, rng))
			} else {
				String::new()
			};
			let mut s = String::from(
				"local Ct = pf.C local m = 0 for j in pairs(Ct) do if j > m then m = j end end local ct, cn, csb, csl, co = {}, {}, {}, {}, {} local si = 1 for j = 1, m do co[j] = si local cbase = (PS + i * PT + SALT + qf * CQC + ((bz%65521)*((i*KI+j*NI+qf)%65521))%268435456",
			);
			s.push_str(&cenv_off);
			s.push_str(&xtec_off);
			s.push_str(
				") % 268435456 local v=Ct[j] local tv=TYP(v) local stj=(cbase+j*CKJ)%268435456 stj=(KM*stj+KC)%268435456 stj=(KM*stj+KC)%268435456 local pj=tv==\"string\" and CSJ or CNJ local stp=(cbase+j*pj)%268435456 stp=(KM*stp+KC)%268435456 stp=(KM*stp+KC)%268435456 if tv == \"number\" then if v % 1 == 0 and v > -1125899906842624 and v < 1125899906842624 then ct[j] = (tm[2] + stj) % 65536 cn[j] = v + stp else local ns = TOSTR(v) ct[j] = (tm[6] + stj) % 65536 csl[j] = #ns for cb = 1, #ns do stp = (KM * stp + KC) % 268435456 csb[si] = (BYTE(ns, cb) + stp % 256) % 256 si = si + 1 end end elseif tv == \"string\" then ct[j] = (tm[3] + stj) % 65536 csl[j] = #v for cb = 1, #v do stp = (KM * stp + KC) % 268435456 csb[si] = (BYTE(v, cb) + stp % 256) % 256 si = si + 1 end elseif tv == \"boolean\" then if v then ct[j] = (tm[4] + stj) % 65536 else ct[j] = (tm[5] + stj) % 65536 end else ct[j] = (tm[1] + stj) % 65536 end end",
			);
			s
		} else {
			String::from(
				"local Ct = pf.C local m = 0 for j in pairs(Ct) do if j > m then m = j end end local ct, cn, csb, csl = nil, nil, nil, nil",
			)
		};
		// V17 P1-b 修复（R072）：EP[i] 是选择性字段拷贝，原表漏拷 fv/fvm，
		// 导致 E.bt.fv == nil、trampoline 首跳即崩（"attempt to index nil with
		// number"，实测 [TRT k=0 fvnil] 抓获）。super 开时补拷；off 态恒空串，
		// 锚点产物不受影响。
		let c_fields_enc: String = if consts_safe {
			"m = m, ct = ct, cn = cn, csb = csb, csl = csl, co = co".to_string()
		} else {
			"m = m, C = Ct".to_string()
		} + if super_insn {
			", fv = pf.fv, fvm = pf.fvm"
		} else {
			""
		};
		let os_list = operand_sums
			.iter()
			.map(|s| obf_num(*s, rng))
			.collect::<Vec<_>>()
			.join(", ");
		let os_list2 = operand_sums2
			.iter()
			.map(|s| obf_num(*s, rng))
			.collect::<Vec<_>>()
			.join(", ");
		let mut zw_inner = String::new();
		for (fi, sites) in nop_sites.iter().enumerate() {
			for &p in sites {
				zw_inner.push_str(&format!(
					"do local _nf = PF_[{fi}] local _np = {pos} if (_nf.SA[_np] + _nf.SC[_np] + _np) % 2 == 0 then _nf.W[_np] = NOPA end end; ",
					fi = fi + 1,
					pos = p as usize + 1,
				));
			}
		}
		let s_idx_lists: Vec<String> = s_indices.iter().map(|v| v.to_string()).collect();
		let bl_lists: Vec<String> = block_starts
			.iter()
			.map(|starts| {
				format!(
					"{{{}}}",
					starts
						.iter()
						.map(|v| v.to_string())
						.collect::<Vec<_>>()
						.join(", ")
				)
			})
			.collect();
		let stream_bias: [String; 5] = [
			obf_num(rng.int(0, 65535) as u64, rng),
			obf_num(rng.int(0, 65535) as u64, rng),
			obf_num(rng.int(0, 65535) as u64, rng),
			obf_num(rng.int(0, 65535) as u64, rng),
			obf_num(rng.int(0, 65535) as u64, rng),
		];
		let block_stream_names = ["W", "SA", "SB", "SC", "SD"];
		let mut block_word_perm: Vec<usize> = (0..5).collect();
		rng.shuffle(&mut block_word_perm);
let chaff_lane = rng.int(1, 4) as usize;
		manifest_key("CHAFF_LANE", chaff_lane as u64);
		let qpos_lists: Vec<String> = nop_sites
			.iter()
			.map(|sites| {
				format!(
					"{{{}}}",
					sites
						.iter()
						.map(|p| (*p as usize + 1).to_string())
						.collect::<Vec<_>>()
						.join(",")
				)
			})
			.collect();
		let qf_expected = chaff_folds
			.iter()
			.map(|v| obf_num(*v as u64, rng))
			.collect::<Vec<_>>()
			.join(",");
		let qf_checksum_e = ke.key_expr(qf_checksum, None, rng);
		let chaff_mul_e = obf_num(chaff_mul as u64, rng);
		let chaff_coeff_e: Vec<String> = chaff_coeffs
			.iter()
			.map(|v| obf_num(*v as u64, rng))
			.collect();
		let chaff_pos_e = obf_num(chaff_pos as u64, rng);
		let chaff_fn_e = obf_num(chaff_fn as u64, rng);
		let chaff_block = (((chaff_mul as u64) * 257
			+ chaff_pos as u64
			+ chaff_coeffs[0] as u64 * 17)
			% 65_520
			+ 1) as u16;
		let chaff_block_e = obf_num(chaff_block as u64, rng);
		manifest_key("CHAFF_BLOCK", chaff_block as u64);
		let enc_lanes = block_word_perm
			.iter()
			.map(|&lane| {
				let qterm = if lane == chaff_lane { " + qf" } else { "" };
				format!(
					"st = (fkm * st + fkc) % 268435456 bx[xo] = (pf.{}[p] + {} + st % 65536{}) % 65536 xo = xo + 1",
					block_stream_names[lane], stream_bias[lane], qterm
				)
			})
			.collect::<Vec<_>>()
			.join(" ");
		for (physical, &logical) in block_word_perm.iter().enumerate() {
			manifest_key(
				match logical {
					0 => "BLOCK_POS_W",
					1 => "BLOCK_POS_SA",
					2 => "BLOCK_POS_SB",
					3 => "BLOCK_POS_SC",
					_ => "BLOCK_POS_SD",
				},
				physical as u64,
			);
		}
		let enc_body = format!(
			"local P = PF_ local EP = {{}} local QAG=0 local SALT = 0 do local _st = {{}} local _ts = TOSTR(_st) for _i = 1, #_ts do SALT = (SALT * 31 + BYTE(_ts, _i)) % 268435456 end end local BL = {{ {blists} }} local S_IDX = {{{s_idxs}}} local QP={{{qpos}}} local CM,CA,CB,CC,CD,CP,CF,CQB,CQC,FQ,FI,KQ,KI,BQ,BI,LB,LS,TB,TS,RQ,RI,RS,RP,MQ,MI,MM,MA,NQ,NI,PM,CM2,CQ2,CI2,CNJ,CSJ={cm},{ca},{cb},{cc},{cd},{cp},{cf},{cqb},{cqc},{fq},{fi},{kq},{ki},{bq},{bi},{lb},{ls},{tb},{ts},{rq},{ri},{rs},{rp},{mq},{mi},{mm},{ma},{nq},{ni},{pm},{cm2},{cq2},{ci2},{cnj},{csj} for i = 1, #P do local pf = P[i] local bz=(BD+(pf.iz1%65521)*(1+(i+FQ)%65521)+(pf.iz2%65521)*(1+(i+KQ)%65521))%268435456 pf.iz1=nil pf.iz2=nil local qf=(i*CF)%65521 local qp=QP[i] for qj=1,#qp do local q=qp[qj] qf=(qf*CM+pf.SA[q]*CA+pf.SB[q]*CB+pf.SC[q]*CC+pf.SD[q]*CD+q*CP)%65521 end qf=qf+1 QAG=(QAG*257+qf)%268435456 local fkm=2*((KM+qf*FQ+i*FI)%16000000)+1 local fkc=(KC+qf*KQ+i*KI)%268435456 local fpb=(PB+qf*BQ+i*BI)%268435456 local frp=3+((qf*RQ+i*RI)%7) local dp=(LB+i*LS)%24 local pl={{0,1,2,3}} local pm={{}} local ds={{6,2,1,1}} for ps=1,4 do local pk=ps==4 and 0 or FLR(dp/ds[ps]) if ps<4 then dp=dp%ds[ps] end pm[ps]=pl[pk+1] for z=pk+1,#pl-1 do pl[z]=pl[z+1] end pl[#pl]=nil end local pbs=1 for ps=1,4 do if pm[ps]==1 then pbs=ps break end end local td=(TB+i*TS+(bz%720)*(1+MQ%719))%720 local tl={{0,1,2,3,4,5}} local tm={{}} local tf={{120,24,6,2,1,1}} for tx=1,6 do local tk=tx==6 and 0 or FLR(td/tf[tx]) if tx<6 then td=td%tf[tx] end tm[tx]=tl[tk+1] for z=tk+1,#tl-1 do tl[z]=tl[z+1] end tl[#tl]=nil end local bs = BL[i] local nb = #bs local bl, bo, bm = {{}}, {{}}, {{}} for bi = 1, nb do local s0 = bs[bi] local e0 = (bi < nb) and (bs[bi + 1] - 1) or #pf.W bl[bi] = e0 - s0 + 1 bm[s0] = bi bo[bi] = (bi > 1) and (bo[bi - 1] + bl[bi - 1] * 5) or 1 end local ST={{pf.SA,pf.SB,pf.SC,pf.SD}} local Wt,ks=pf.W,ST[pbs] local km = {{}} for j = 1, #Wt do if {cw} then local ci=ks[j]+1 local ct=(ci*CM2+qf*CQ2+i*CI2+(bz%65521)*CI2)%65521 ks[j]=ct km[#km+1]=ct km[#km+1]=ci end end local bx = {{}} local xo = 1 for bi = 1, nb do local s0 = bs[bi] local st = (PS + i * PT + s0 * fpb + SALT + qf * CQB + ((bz%65521)*((i*FI+qf*FQ+s0*RP)%65521))%268435456{xteoff}) % 268435456 for p = s0, s0 + bl[bi] - 1 do {enc_lanes} if (p-s0+1)%frp==0 then st=(st+qf*RS+p*RP)%268435456 end end end local st = (PS + i * PT + SALT) % 268435456 {cblock} local mst=(qf*MQ+i*MI+MA+(bz%65536)*((i+MQ)%65536))%65536 local mu={{}} for uj=1,#pf.upsrc do mst=(mst*MM+MA+uj)%65536 mu[uj]=(pf.upsrc[uj]+mst)%65536 end local mnp=(pf.nparams+(qf*NQ+i*NI+MA+(bz%65536)*((i+NI)%65536))%65536)%65536 mst=nil EP[i] = {{ bs = bs, bl = bl, bo = bo, bm = bm, bx = bx, sx = (i+PM)%65521, S = pf.S, dispatchS = S_IDX[i], us = mu, np = mnp, km = km, {cfields} }} end for i = 1, #EP do local km = EP[i].km for j = 1, #km, 2 do EP[i][km[j]] = EP[km[j+1]] km[j]=nil km[j+1]=nil end EP[i].km = nil end return EP[#EP], SALT, QAG",
			blists = bl_lists.join(", "),
			s_idxs = s_idx_lists.join(", "),
			qpos = qpos_lists.join(","),
			cm = chaff_mul_e,
			ca = chaff_coeff_e[0],
			cb = chaff_coeff_e[1],
			cc = chaff_coeff_e[2],
			cd = chaff_coeff_e[3],
			cp = chaff_pos_e,
			cf = chaff_fn_e,
			cqb = chaff_block_e,
			cqc = chaff_const_e,
			fq = dialect_e[0],
			fi = dialect_e[1],
			kq = dialect_e[2],
			ki = dialect_e[3],
			bq = dialect_e[4],
			bi = dialect_e[5],
			lb = lane_base_e,
			ls = lane_step_e,
			tb = tag_base_e,
			ts = tag_step_e,
			rq = rekey_e[0],
			ri = rekey_e[1],
			rs = rekey_e[2],
			rp = rekey_e[3],
			mq = metadata_e[0],
			mi = metadata_e[1],
			mm = metadata_e[2],
			ma = metadata_e[3],
			nq = metadata_e[4],
			ni = metadata_e[5],
			pm = metadata_e[6],
			cm2 = metadata_e[7],
			cq2 = metadata_e[8],
			ci2 = metadata_e[9],
			cnj = const_num_step_e,
			csj = const_str_step_e,
			cw = closure_cond,
			cblock = c_block_enc,
			cfields = c_fields_enc,
			xteoff = xte_off,
			enc_lanes = enc_lanes,
		);
		let ck_poison = rng.int(1, 268_435_455);
		let ck_poison_e = ke.key_expr(ck_poison, None, rng);
		manifest_key("BSS_CKPOISON", ck_poison as u64);
		let mut phase_pool: Vec<u8> = (17..=239).collect();
		rng.shuffle(&mut phase_pool);
		let phase_codes = [phase_pool[0], phase_pool[1], phase_pool[2]];
		let phase_mask = rng.int(1, 255) as u8;
		let phase_step = (rng.int(1, 255) | 1) as u8;
		let phase_stored: Vec<u8> = phase_codes
			.iter()
			.enumerate()
			.map(|(i, code)| {
				((*code as usize + phase_mask as usize + (i + 1) * phase_step as usize)
					% 256) as u8
			})
			.collect();
		let mut phase_checksum: i64 = 0;
		for &v in std::iter::once(&phase_mask)
			.chain(std::iter::once(&phase_step))
			.chain(phase_stored.iter())
		{
			phase_checksum = (phase_checksum * 257 + v as i64) % KEY_MOD;
		}
		let phase_checksum_e = ke.key_expr(phase_checksum, None, rng);
		manifest_key("BSS_PHASE_CK", phase_checksum as u64);
		let phase_parse = "local di=1 while di<=#FN_ do local pz=parse(FN_[di],di) PF_[di]=pz local d1=(pz.ck-OS[di])%4294967296 local d2=(pz.ck2-OS2[di])%4294967296 pz.iz1=((d1%65536)*(1+(PS+di*KM)%65521)+FLR(d1/65536)*(1+(PT+di*KC)%65521))%268435456 pz.iz2=((d2%65536)*(1+(PT+di*KM)%65521)+FLR(d2/65536)*(1+(PS+di*KC)%65521))%268435456 di=di+1 end".to_string();
		let phase_rewrite = if zw_inner.is_empty() {
			"local _nz=PB%2 _nz=_nz-_nz".to_string()
		} else {
			zw_inner.clone()
		};
		let phase_encode = enc_body.clone();
				let mut phase_branches = vec![
			(phase_codes[0], phase_parse),
			(phase_codes[1], phase_rewrite),
			(phase_codes[2], phase_encode),
		];
		rng.shuffle(&mut phase_branches);
		let phase_dispatch = phase_branches
			.iter()
			.enumerate()
			.map(|(i, (code, body))| {
				let head = if i == 0 {
					if rng.int(0, 1) == 0 {
						format!("if _op=={}", code)
					} else {
						format!("if {}==_op", code)
					}
				} else if rng.int(0, 1) == 0 {
					format!("elseif _op=={}", code)
				} else {
					format!("elseif {}==_op", code)
				};
				format!("{} then {} ", head, body)
			})
			.collect::<String>()
			+ "else BD=(BD+BSSKPX)%268435456 end";
		let mut bss_static_src = format!(
			"function(BYTE,CHAR,FLR,SUB,AL,TK,decarrier,r16,CKM,CKC,BKM,BKC,BSEED,BSTEP,TYP,TOSTR) local OS={{{os}}} local OS2={{{os2}}} {parse} return function(FN_,NOPA,PS,PT,KM,KC,PB,CKJ) local PF_={{}} local _bp={{{program}}} local _bc=0 _bc=(_bc*257+{mask})%268435456 _bc=(_bc*257+{step})%268435456 for _bj=1,#_bp do _bc=(_bc*257+_bp[_bj])%268435456 end local BD=(_bc-{expected})%268435456 local _bi=1 while _bi<=#_bp do local _op=(_bp[_bi]-({mask}+_bi*{step})%256)%256 {dispatch_arm}{dispatch} _bi=_bi+1 end end end",
			os = os_list,
			os2 = os_list2,
			parse = parse_fn,
			program = phase_stored.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
			mask = phase_mask,
			step = phase_step,
			expected = "BSSKCX",
			dispatch = phase_dispatch,
			dispatch_arm = String::new(),
		);
		let tag_decode_rt = format!(
			"local td=({}+fi*{}+(PZ2%720)*(1+NQ%719)+(PZ3%720)*(1+NI%719)+(PZ4%720)*(1+MI%719)+(MZD%720)*(1+MM%719))%720 local tl={{0,1,2,3,4,5}} local tm={{}} local tf={{120,24,6,2,1,1}} for tx=1,6 do local tk=tx==6 and 0 or FLOOR(td/tf[tx]) if tx<6 then td=td%tf[tx] end tm[tx]=tl[tk+1] for z=tk+1,#tl-1 do tl[z]=tl[z+1] end tl[#tl]=nil end ",
			tag_base_e, tag_step_e
		);
		let cdec_inner = if consts_safe {
			let exg_active = exg_consts.is_some();
			let (xm3_e, xm4_e) = if exg_active {
				(obf_num(xm3, rng), obf_num(xm4, rng))
			} else {
				(String::new(), String::new())
			};
			let probe_call = match (cenv_active, exg_active) {
				(true, true) => "local cef = CENVPROBE() local xtok2 = XPROBE(fi) ",
				(true, false) => "local cef = CENVPROBE() ",
				(false, true) => "local xtok2 = XPROBE(fi) ",
				_ => "",
			};
			let mut base_env_s = String::new();
			if cenv_active {
				base_env_s.push_str(" + cef");
			}
			if exg_active {
				base_env_s.push_str(&format!(
					" + (xtok2 * {} + {}) % 268435456",
					xm3_e, xm4_e
				));
			}
			let mut s = String::from(
				"function(q, CHAR, UNP, TONUM) local fi=((q.sx-PM-1)%65521)+1 local ct, cn, csb, csl, co = q.ct, q.cn, q.csb, q.csl, q.co ",
			);
			s.push_str(&tag_decode_rt);
			s.push_str(probe_call);
			s.push_str(&format!(
				"local pzc=((PZ2%65521)*((fi+KQ)%65521)+(PZ3%65521)*((QF[fi]+KI)%65521)+(PZ4%65521)*((fi+NQ)%65521)+(MZD%65521)*((QF[fi]+NI)%65521))%268435456 local base = (PS + fi * PT + SLTC + QF[fi] * CQC + pzc{}) % 268435456 ",
				base_env_s
			));
			s.push_str(
				"local SCH = (\"\")[\"char\"] local C = {} local done = {} local MT = {} MT.__index = function(t, j) if done[j] then return rawget(t, j) end if ct[j] == nil then done[j] = true return nil end local stj = (base + j * CKJ) % 268435456 stj = (KM * stj + KC) % 268435456 stj = (KM * stj + KC) % 268435456 local ty = (ct[j] - stj) % 65536 local pj=ty==tm[3] and CSJ or CNJ local stp=(base+j*pj)%268435456 stp=(KM*stp+KC)%268435456 stp=(KM*stp+KC)%268435456 local v if ty == tm[2] then v = cn[j] - stp elseif ty == tm[3] or ty == tm[6] then local l = csl[j] local si = co[j] local s = \"\" for x = 1, l do stp = (KM * stp + KC) % 268435456 s = s .. SCH((csb[si] - stp % 256) % 256) si = si + 1 end if ty == tm[3] then return s else v = TONUM(s) end elseif ty == tm[4] then v = true elseif ty == tm[5] then v = false end done[j] = true rawset(t, j, v) return v end return setmetatable(C, MT) end",
			);
			s
		} else {
			String::from(
				"function(q, CHAR, UNP, TONUM) local C = q.C return C end",
			)
		};

		let dec_lanes = block_word_perm
			.iter()
			.map(|&lane| {
				let qterm = if lane == chaff_lane {
					" - QF[fi]"
				} else {
					""
				};
				format!(
					"st = (fkm * st + fkc) % 268435456 t{}[i] = (x[off] - st % 65536 - {}{}) % 65536 off = off + 1",
					lane, stream_bias[lane], qterm
				)
			})
			.collect::<Vec<_>>()
			.join(" ");
		let cache_pack = block_word_perm
			.iter()
			.map(|lane| format!("u{}", lane))
			.collect::<Vec<_>>()
			.join(", ");
		let mut cache_pos = [0usize; 5];
		for (physical, &logical) in block_word_perm.iter().enumerate() {
			cache_pos[logical] = physical + 1;
		}
		let cache_hit = (0..5)
			.map(|logical| format!("c[{}]", cache_pos[logical]))
			.collect::<Vec<_>>()
			.join(", ");
		let cache_remask = block_word_perm
			.iter()
			.enumerate()
			.map(|(physical, lane)| {
				format!(
					"c[{pos}][wx]=(c[{pos}][wx]-om+wm)%{modulus}",
					pos = physical + 1,
					modulus = if *lane == 0 { 256 } else { 65536 },
				)
			})
			.collect::<Vec<_>>()
			.join(" ");
		let lane_normalize = "local dp=(LB+fi*LS)%24 local pl={0,1,2,3} local pm={} local ds={6,2,1,1} for ps=1,4 do local pk=ps==4 and 0 or FLOOR(dp/ds[ps]) if ps<4 then dp=dp%ds[ps] end pm[ps]=pl[pk+1] for z=pk+1,#pl-1 do pl[z]=pl[z+1] end pl[#pl]=nil end local pv={t1,t2,t3,t4} local u={} for ps=1,4 do u[pm[ps]+1]=pv[ps] end local u0,u1,u2,u3,u4=t0,u[1],u[2],u[3],u[4]";
		let wipe_current = "if BC==0 and E.ww then local old={E.ww,E.w0,E.w1,E.w2,E.w3} for wl=1,5 do local wt=old[wl] if wt then for wx=1,#wt do wt[wx]=nil end end end E.ww,E.w0,E.w1,E.w2,E.w3=nil,nil,nil,nil,nil end";
		let cache_store = format!(
			"local nc={{ {cache_pack},s0,s0+l-1,wm }} if BC>0 then E.bch[bi]=nc E.bq[#E.bq+1]=bi if #E.bq>BC then local oid=E.bq[1] for qx=1,#E.bq-1 do E.bq[qx]=E.bq[qx+1] end E.bq[#E.bq]=nil local oc=E.bch[oid] E.bch[oid]=nil if oc then for wl=1,5 do local wt=oc[wl] if wt then for wx=1,#wt do wt[wx]=nil end oc[wl]=nil end end end end end",
			cache_pack = cache_pack,
		);
		let bdec_src = format!(
			"local function bdec(E, bi) local pf=E.bt local fi=((pf.sx-PM-1)%65521)+1 E.er=E.er+1 local wm=(E.er*WM+bi*WB+fi)%65536 local c = E.bch[bi] if c then local om=c[8] for wx=1,#c[1] do {remask} end c[8]=wm E.ww, E.w0, E.w1, E.w2, E.w3 = {cache_hit} E.bp, E.be = c[6], c[7] E.br=wm return end {wipe} local qi=QF[fi] local fkm=2*((KM+qi*FQ+fi*FI)%16000000)+1 local fkc=(KC+qi*KQ+fi*KI)%268435456 local fpb=(PB+qi*BQ+fi*BI)%268435456 local frp=3+((qi*RQ+fi*RI)%7) local s0 = pf.bs[bi] local l = pf.bl[bi] local pzb=((PZ1%65521)*((fi+FQ)%65521)+(PZ3%65521)*((qi+KI)%65521)+(PZ4%65521)*((bi+RI)%65521)+(MZD%65521)*((s0+RQ)%65521)+(E.pg%65521)*((s0+fi+RP)%65521))%268435456 local st = (PS + fi * PT + s0 * fpb + SLT + QF[fi] * CQB + pzb) % 268435456 local t0, t1, t2, t3, t4 = {{}}, {{}}, {{}}, {{}}, {{}} local x = pf.bx local off = pf.bo[bi] for i = 1, l do {dec_lanes} if i%frp==0 then st=(st+qi*RS+(s0+i-1)*RP)%268435456 end end {normalize} for wx=1,#u0 do u0[wx]=(u0[wx]+wm)%256 u1[wx]=(u1[wx]+wm)%65536 u2[wx]=(u2[wx]+wm)%65536 u3[wx]=(u3[wx]+wm)%65536 u4[wx]=(u4[wx]+wm)%65536 end {store} E.ww, E.w0, E.w1, E.w2, E.w3 = u0, u1, u2, u3, u4 E.bp, E.be = s0, s0 + l - 1 E.br=wm end",
			cache_hit = cache_hit,
			remask = cache_remask,
			wipe = wipe_current,
			dec_lanes = dec_lanes,
			normalize = lane_normalize,
			store = cache_store,
		);
		let rt_newe = format!(
			"newE = function(pf, V, ups, vargs, vargc)\n    local fi=((pf.sx-PM-1)%65521)+1\n    local vd=(QF[fi]*NQ+fi*NI+MM)%65521\n    local pg=((PZ1%65521)*((fi+FQ)%65521)+(PZ2%65521)*((QF[fi]+KQ)%65521)+(PZ3%65521)*((fi+NI)%65521)+(PZ4%65521)*((QF[fi]+RI)%65521)+(MZD%65521)*((fi+MM)%65521))%268435456\n    {}\n    local C = CDEC(pf, CHAR, UNP, TONUM)\n    local S = pf.S\n    local O = {{}}\n    local E = {}\n    E.bt = pf\n    E.bm = pf.bm\n    E.bch = {{}}\n    E.bq = {{}}\n    E.er = 0\n    E.br = 0\n    E.bdec = bdec\n    bdec(E, 1)\n    return E\n  end\n  ",
			run_unpack, e_table
		);
		let (sig_tail, xtfb_line) = if let Some(c) = &exg_consts {
			(
				", XTOK, XPROBE".to_string(),
				format!(
					"local XTFB = (XTOK * {} + {}) % 268435456\n  SLT = (SLT + XTFB) % 268435456\n  ",
					obf_num(c.xm1, rng),
					obf_num(c.xm2, rng)
				),
			)
		} else {
			(String::new(), String::new())
		};
		let (sig_tail, xtfb_line, env_coef, env_coef2) = if env_check {
			let ec = rng.int(1024, 65_521) as u64;
			let ec2 = rng.int(1024, 65_521) as u64;
			(
				format!("{sig_tail}, EVD"),
				format!(
					"{xtfb_line}SLT = (SLT + EVD * {}) % 268435456\n  ",
					obf_num(ec, rng)
				),
				ec,
				ec2,
			)
		} else {
			(sig_tail, xtfb_line, 0, 0)
		};
		let bdec_assign = bdec_src.replacen("local function bdec", "bdec = function", 1);
		let makefn_assign =
			makefn_decl.replacen("local function makefn", "makefn = function", 1);
		let run_assign = format!(
			"run = function(pf,V,ups,vargs,vargc) local E=newE(pf,V,ups,vargs,vargc) {} end",
			trampoline_src.clone().unwrap(),
		);
		let mut rt_phase_pool: Vec<u8> = (17..=239).collect();
		rng.shuffle(&mut rt_phase_pool);
		let rt_phase_codes = [
			rt_phase_pool[0],
			rt_phase_pool[1],
			rt_phase_pool[2],
			rt_phase_pool[3],
			rt_phase_pool[4],
		];
		let rt_phase_mask = rng.int(1, 255) as u8;
		let rt_phase_step = (rng.int(1, 255) | 1) as u8;
		let rt_phase_stored: Vec<u8> = rt_phase_codes
			.iter()
			.enumerate()
			.map(|(i, code)| {
				((*code as usize
					+ rt_phase_mask as usize
					+ (i + 1) * rt_phase_step as usize)
					% 256) as u8
			})
			.collect();
		let mut rt_phase_checksum: i64 = 0;
		for &v in std::iter::once(&rt_phase_mask)
			.chain(std::iter::once(&rt_phase_step))
			.chain(rt_phase_stored.iter())
		{
			rt_phase_checksum = (rt_phase_checksum * 257 + v as i64) % KEY_MOD;
		}
		let rt_phase_poison = rng.int(1, KEY_MOD - 1);
		let rt_phase_checksum_e = ke.key_expr(rt_phase_checksum, None, rng);
		let rt_phase_poison_e = ke.key_expr(rt_phase_poison, None, rng);
		manifest_key("RT_PHASE_CK", rt_phase_checksum as u64);
		manifest_key("RT_PHASE_POISON", rt_phase_poison as u64);
		let mut rt_phase_branches = vec![
			(rt_phase_codes[0], format!("CDEC={}", cdec_inner)),
			(rt_phase_codes[1], bdec_assign),
			(rt_phase_codes[2], makefn_assign),
			(rt_phase_codes[3], rt_newe),
			(rt_phase_codes[4], run_assign),
		];
		rng.shuffle(&mut rt_phase_branches);
		let rt_phase_dispatch = rt_phase_branches
			.iter()
			.enumerate()
			.map(|(i, (code, body))| {
				let head = if i == 0 {
					if rng.int(0, 1) == 0 {
						format!("if _op=={}", code)
					} else {
						format!("if {}==_op", code)
					}
				} else if rng.int(0, 1) == 0 {
					format!("elseif _op=={}", code)
				} else {
					format!("elseif {}==_op", code)
				};
				format!("{} then {} ", head, body)
			})
			.collect::<String>()
			+ "else SLT=(SLT+RTPKX)%268435456 SLTC=(SLTC+RTPKX)%268435456 end";
		let mut rt_static_src = format!(
			"function(PS,PT,KM,KC,PB,SLT,P,G,U,FLOOR,MS,AV,HW2,TP,mget,resolve_call,callcap,HAS_LEN_META,XC1,XC2,XC3,XC4,XE1,XE2,XE3,XE4,CLK,H1,H2,CB1,CB2,CBC,CC1,CC2,CCC,CKJ,CENVPROBE,MZD{sigtail}) local SEL=P[{select_slot}] local PZ1=(H1*CB1+H2*CB2+CBC)%268435456 local PZ2=(H1*CC1+H2*CC2+CCC)%268435456 local SLTC=SLT local CQB={cqb} local CQC={cqc} local BC={bc} local WM,WB={wm},{wb} local CS={cs} local CNJ,CSJ={cnj},{csj} local FQ,FI,KQ,KI,BQ,BI,LB,LS,RQ,RI,RS,RP,MQ,MI,MM,MA,NQ,NI,PM={fq},{fi},{kq},{ki},{bq},{bi},{lb},{ls},{rq},{ri},{rs},{rp},{mq},{mi},{mm},{ma},{nq},{ni},{pm} local QF={{{qf}}} local _qk=0 for _qi=1,#QF do _qk=(_qk*257+QF[_qi])%268435456 end local PZ3=(_qk-RTQCKX)%268435456 {xtfb}local CDEC,bdec,newE,makefn,run local _rp={{{program}}} local _rc=0 _rc=(_rc*257+{mask})%268435456 _rc=(_rc*257+{step})%268435456 for _rj=1,#_rp do _rc=(_rc*257+_rp[_rj])%268435456 end local PZ4=(_rc-RTPCX)%268435456 local _ri=1 while _ri<=#_rp do local _op=(_rp[_ri]-({mask}+_ri*{step})%256)%256 {dispatch} _ri=_ri+1 end _rp=nil return run end",
			sigtail = sig_tail,
			select_slot = slots[12],
			xtfb = xtfb_line,
			qf = qf_expected,
			cqb = chaff_block_e,
			cqc = chaff_const_e,
			bc = block_cache_cap_e,
			wm = window_mul_e,
			wb = window_block_e,
			cs = call_slot_step_e,
			cnj = const_num_step_e,
			csj = const_str_step_e,
			fq = dialect_e[0],
			fi = dialect_e[1],
			kq = dialect_e[2],
			ki = dialect_e[3],
			bq = dialect_e[4],
			bi = dialect_e[5],
			lb = lane_base_e,
			ls = lane_step_e,
			rq = rekey_e[0],
			ri = rekey_e[1],
			rs = rekey_e[2],
			rp = rekey_e[3],
			mq = metadata_e[0],
			mi = metadata_e[1],
			mm = metadata_e[2],
			ma = metadata_e[3],
			nq = metadata_e[4],
			ni = metadata_e[5],
			pm = metadata_e[6],
			program = rt_phase_stored
				.iter()
				.map(|v| v.to_string())
				.collect::<Vec<_>>()
				.join(","),
			mask = rt_phase_mask,
			step = rt_phase_step,
			dispatch = rt_phase_dispatch,
		);
		let mut exg_static_src = exg_src.map(|s| {
			s.strip_prefix("return ")
				.expect("EXG factory source")
				.to_string()
		});
		let mut env_static_src = env_src.map(|s| {
			s.strip_prefix("return ")
				.expect("ENV factory source")
				.to_string()
		});
		let mut frag_reserved: std::collections::HashSet<String> =
			crate::mangle::RESERVED.iter().map(|x| x.to_string()).collect();
		for g in [
			"pairs", "ipairs", "next", "select", "unpack", "type", "tostring",
			"tonumber", "print", "error", "assert", "pcall", "xpcall",
			"setmetatable", "getmetatable", "rawget", "rawset", "rawequal",
			"string", "table", "math", "bit32", "os", "debug", "buffer",
			"task", "coroutine", "loadstring", "load", "getfenv", "setfenv",
			"newproxy", "collectgarbage", "warn", "utf8", "typeof", "_G",
			"_VERSION", "Vector3", "Vector2", "Instance", "Enum", "Random", "game",
			"workspace", "tick", "wait",
		] {
			frag_reserved.insert(g.to_string());
		}
		for g in [
			"__index", "__newindex", "__tostring", "__concat", "__call",
			"__iter", "__metatable", "__eq", "__lt", "__le", "__add",
			"__sub", "__mul", "__div", "__mod", "__pow", "__unm",
			"__len", "__mode", "__close", "_ENV", "BSSKPX", "BSSKCX",
			"RTPKX", "RTPCX", "RTQCKX",
		] {
			frag_reserved.insert(g.to_string());
		}
		let mut frag_map: std::collections::HashMap<String, String> =
			std::collections::HashMap::new();
		let mut frag_used: std::collections::HashSet<String> =
			std::collections::HashSet::new();
		for field in ["cf", "ca", "cn", "oa", "ob", "oc2", "od", "ctt", "cnt"] {
			let mapped = sanitize_name(rng, &frag_reserved, &frag_used);
			frag_used.insert(mapped.clone());
			frag_map.insert(field.to_string(), mapped);
		}
		bss_static_src = sanitize_frag(
			&bss_static_src,
			rng,
			&mut frag_map,
			&frag_reserved,
			&mut frag_used,
		);
		bss_static_src = bss_static_src
			.replace("BSSKPX", "_BSSPKX_")
			.replace("BSSKCX", "_BSSPCX_");
		let bss_fill = format!(
			"local _BSSPKX_=({}) local _BSSPCX_=({}) BSS={} ",
			ck_poison_e, phase_checksum_e, bss_static_src
		);
		rt_static_src = sanitize_frag(
			&rt_static_src,
			rng,
			&mut frag_map,
			&frag_reserved,
			&mut frag_used,
		);
		rt_static_src = rt_static_src
			.replace("RTPKX", "_RTPKX_")
			.replace("RTPCX", "_RTPCX_")
			.replace("RTQCKX", "_RTQCKX_");
		let rt_fill = format!(
			"local _RTPKX_=({}) local _RTPCX_=({}) local _RTQCKX_=({}) RTFRAG={} ",
			rt_phase_poison_e, rt_phase_checksum_e, qf_checksum_e, rt_static_src
		);
		let exg_fill = if let Some(src) = exg_static_src.take() {
			let clean = sanitize_frag(
				&src,
				rng,
				&mut frag_map,
				&frag_reserved,
				&mut frag_used,
			);
			format!("EXGFRAG={} ", clean)
		} else {
			String::new()
		};
		let env_fill = if let Some(src) = env_static_src.take() {
			let clean = sanitize_frag(
				&src,
				rng,
				&mut frag_map,
				&frag_reserved,
				&mut frag_used,
			);
			format!("ENVFRAG={} ", clean)
		} else {
			String::new()
		};
		let env_decl = if env_check {
			"\n  local EVD"
		} else {
			""
		};
		let env_frag_decl = if env_check {
			"\n  local ENVFRAG"
		} else {
			""
		};
		let env_extract = if env_check {
			"\n    local _eo, _ev = PCAL(ENVFRAG) ENVFRAG = nil EVD = (_eo and _ev or 12345679) % 268435456 "
		} else {
			""
		};
		let env_mzd = if env_check {
			format!(
				" MZD = (MZD + EVD * {}) % 268435456 ",
				obf_num(env_coef2, rng)
			)
		} else {
			String::new()
		};
		micro_fill = remap_fragment_fields(&micro_fill, &frag_map);
		let sh_iv1 = rng.int(0, SH_M1 as i64 - 1) as u64;
		let sh_iv2 = rng.int(0, SH_M2 as i64 - 1) as u64;
		let sh_iv1_e = ke.key_expr_with_data(sh_iv1 as i64, None, "FN_", rng);
		let sh_iv2_e = ke.key_expr_with_data(sh_iv2 as i64, None, "FN_", rng);
		manifest_key("CHAIN_IV1", sh_iv1);
		manifest_key("CHAIN_IV2", sh_iv2);
		let (guard_embed, guard_holder) = if guard {
			let (g, holder) = crate::guard::hboot_guard_source(rng);
			(g + " ", holder)
		} else {
			(String::new(), String::new())
		};
        let vault_nonce = rng.int(1, 268435456);
        let vault_nonce_name = format!("_vn{}", rng.int(10, 99));
        let vault_name = format!("_vt{}", rng.int(10, 99));
        let vault_get_name = format!("_vg{}", rng.int(10, 99));
        let vault_ckj_e = ke.key_expr(ckj, None, rng);
        let vault_def = format!(
            "local {vn} = {nonce} local {vt} = {{ [{vn}] = {{RPK1,RPK2,RPK3,RPK4,RPK5,{ckj},BSS,HBD}} }} local {vg} = function(k) local v={vt}[k]; {vt}[k]=nil; return v[1],v[2],v[3],v[4],v[5],v[6],v[7],v[8] end ",
            vn = vault_nonce_name,
            vt = vault_name,
            vg = vault_get_name,
            nonce = vault_nonce,
            ckj = vault_ckj_e
        );
        let shell_params: Vec<&str> = vec![
            "AL", "BYTE", "CHAR", "FLR", "SUB", "CARR1_", "CARR2_", "CARR3_", "CARR4_", "EV",
            "FN_", "NOPA_", "TK_", "DC1", "R16", "CKM_", "CKC_", "BKM_",
            "BKC_", "BSEED_", "BSTEP_", "TYP_", "TSTR_", "GET_", "NONCE_",
        ];
        let shell_args: Vec<String> = {
            let mut v: Vec<String> = vec![
                "AL", "BYTE", "CHAR", "FLR", "SUB", "FN", "FN", "FN", "FN", "EV",
                "FN", "NOPA", "TK", "decarrier", "r16", "CKM", "CKC", "BKM",
                "BKC", "BSEED", "BSTEP", "TYP", "TSTR",
            ].into_iter().map(|x| x.to_string()).collect();
            v.push(vault_get_name.clone());
            v.push(vault_nonce_name.clone());
            v
        };
        let n_par = shell_params.len();
        let shell_param_names: Vec<String> = (0..n_par)
            .map(|i| format!("_sp{}_{}", rng.int(1000, 9999), i))
            .collect();
        let mut par_perm: Vec<usize> = (0..n_par).collect();
        rng.shuffle(&mut par_perm);
        let shell_sig: String = par_perm
            .iter()
            .map(|&i| shell_param_names[i].clone())
            .collect::<Vec<_>>()
            .join(", ");
        let shell_bind = format!(
            "local {} = {} ",
            shell_params.join(", "),
            shell_param_names.join(", ")
        );

		let hpret = if guard {
			format!(", {}", guard_holder)
		} else {
			String::new()
		};
		let (hpret_decl, hpret_nil_r, hpret_ret) = if guard {
			(format!(", _O7"), format!(", nil"), format!(", _O7"))
		} else {
			(String::new(), String::new(), String::new())
		};
		let mut hb_phase_pool: Vec<u8> = (17..=239).collect();
		rng.shuffle(&mut hb_phase_pool);
		let hb_phase_codes = [hb_phase_pool[0], hb_phase_pool[1], hb_phase_pool[2]];
		let hb_phase_mask = rng.int(1, 255) as u8;
		let hb_phase_step = (rng.int(1, 255) | 1) as u8;
		let hb_phase_stored: Vec<u8> = hb_phase_codes
			.iter()
			.enumerate()
			.map(|(i, code)| {
				((*code as usize + hb_phase_mask as usize + (i + 1) * hb_phase_step as usize)
					% 256) as u8
			})
			.collect();
		let mut hb_phase_checksum: i64 = 0;
		for &v in std::iter::once(&hb_phase_mask)
			.chain(std::iter::once(&hb_phase_step))
			.chain(hb_phase_stored.iter())
		{
			hb_phase_checksum = (hb_phase_checksum * 257 + v as i64) % KEY_MOD;
		}
		let hb_phase_poison = rng.int(1, KEY_MOD - 1);
		let hb_phase_checksum_e = ke.key_expr_with_data(hb_phase_checksum, None, "FN_", rng);
		let hb_phase_poison_e = ke.key_expr_with_data(hb_phase_poison, None, "FN_", rng);
		manifest_key("HBOOT_PHASE_CK", hb_phase_checksum as u64);
		manifest_key("HBOOT_PHASE_POISON", hb_phase_poison as u64);
		let hb_factory =
			"_lv1=BSSF_(BYTE,CHAR,FLR,SUB,AL,TK_,DC1,R16,CKM_,CKC_,BKM_,BKC_,BSEED_,BSTEP_,TYP_,TSTR_)".to_string();
		let hb_execute =
			"_b1,_b2,_b3=_lv1(FN_,NOPA_,PS_,PT_,KM_,KC_,(PB_+HD1)%268435456,CKJ_)".to_string();
		let hb_seal = "_ready=1".to_string();
		let mut hb_phase_branches = vec![
			(hb_phase_codes[0], hb_factory),
			(hb_phase_codes[1], hb_execute),
			(hb_phase_codes[2], hb_seal),
		];
		rng.shuffle(&mut hb_phase_branches);
		let hb_phase_dispatch = hb_phase_branches
			.iter()
			.enumerate()
			.map(|(i, (code, body))| {
				let head = if i == 0 {
					if rng.int(0, 1) == 0 {
						format!("if _op=={}", code)
					} else {
						format!("if {}==_op", code)
					}
				} else if rng.int(0, 1) == 0 {
					format!("elseif _op=={}", code)
				} else {
					format!("elseif {}==_op", code)
				};
				format!("{} then {} ", head, body)
			})
			.collect::<String>()
			+ &format!(
				"else HD1=(HD1+({p}))%268435456 HD2=(HD2+({p})*({m}+1))%268435456 end",
				p = hb_phase_poison_e,
				m = hb_phase_mask,
			);
		let hboot_src = format!(
			"function({shell_sig}) {shell_bind}{guard}local PS_,PT_,KM_,KC_,PB_,CKJ_,BSSF_,HBD_=GET_(NONCE_) local HD1=HBD_ local HD2=HBD_*257 local _hp={{{program}}} local _hc=0 _hc=(_hc*257+{mask})%268435456 _hc=(_hc*257+{step})%268435456 for _hj=1,#_hp do _hc=(_hc*257+_hp[_hj])%268435456 end local _he=(_hc-({expected}))%268435456 HD1=(HD1+_he)%268435456 HD2=(HD2+_he*({mask}+1))%268435456 local _lv1,_b1,_b2,_b3 local _ready=0 local _hi=1 while _hi<=#_hp do local _op=(_hp[_hi]-({mask}+_hi*{step})%256)%256 {dispatch} _hi=_hi+1 end if _ready~=1 then HD1=(HD1+({poison}))%268435456 HD2=(HD2+({poison})*({step}+1))%268435456 end _hp=nil local HW={{}} local h1=({iv1}+HD1)%{m1} local h2=({iv2}+HD2)%{m2} local _O1,_O2,_O3,_O4,_O5,_O6{hpret_decl}=HW,_b1,_b2,h1,h2,_b3{hpret} HW,_b1,_b2,h1,h2,_b3{hpret}=nil,nil,nil,nil,nil,nil{hpret_nil_r} return function() return _O1,_O2,_O3,_O4,_O5,_O6{hpret_ret} end end",
			guard = guard_embed,
			hpret = hpret,
			shell_sig = shell_sig,
			shell_bind = shell_bind,
			program = hb_phase_stored
				.iter()
				.map(|v| v.to_string())
				.collect::<Vec<_>>()
				.join(","),
			mask = hb_phase_mask,
			step = hb_phase_step,
			expected = hb_phase_checksum_e,
			poison = hb_phase_poison_e,
			dispatch = hb_phase_dispatch,
			hpret_decl = hpret_decl,
			hpret_nil_r = hpret_nil_r,
			hpret_ret = hpret_ret,
			iv1 = sh_iv1_e,
			iv2 = sh_iv2_e,
			m1 = SH_M1,
			m2 = SH_M2,
		);
		if std::env::var("OBSUCVIM_HBOOT_DBG").is_ok() {
			eprintln!("HBOOT_POST_SAN: {}", hboot_src);
		}
		let hboot_static_src =
			sanitize_frag(&hboot_src, rng, &mut frag_map, &frag_reserved, &mut frag_used);
		let hboot_fill = format!("HBOOT={} ", hboot_static_src);
		let sh1 = sh_iv1 as i64;
		let sh2 = sh_iv2 as i64;
		let sh_cb1 = rng.int(1_048_576, 33_554_431) | 1;
		let sh_cb2 = rng.int(1_048_576, 33_554_431) | 1;
		let sh_cc1 = rng.int(1_048_576, 33_554_431) | 1;
		let sh_cc2 = rng.int(1_048_576, 33_554_431) | 1;
		let sh_cbc = (KEY_MOD - (sh1 * sh_cb1 + sh2 * sh_cb2) % KEY_MOD) % KEY_MOD;
		let sh_ccc = (KEY_MOD - (sh1 * sh_cc1 + sh2 * sh_cc2) % KEY_MOD) % KEY_MOD;
		let sh_cb1_e = ke.key_expr(sh_cb1, None, rng);
		let sh_cb2_e = ke.key_expr(sh_cb2, None, rng);
		let sh_cbc_e = ke.key_expr(sh_cbc, None, rng);
		let sh_cc1_e = ke.key_expr(sh_cc1, None, rng);
		let sh_cc2_e = ke.key_expr(sh_cc2, None, rng);
		let sh_ccc_e = ke.key_expr(sh_ccc, None, rng);
		manifest_key("SH_CB1", sh_cb1 as u64);
		manifest_key("SH_CB2", sh_cb2 as u64);
		manifest_key("SH_CBC", sh_cbc as u64);
		manifest_key("SH_CC1", sh_cc1 as u64);
		manifest_key("SH_CC2", sh_cc2 as u64);
		manifest_key("SH_CCC", sh_ccc as u64);
		let (env_gate_a, env_gate_b, env_gate_c, env_term, env_comp, env_exp) =
			if let Some(es) = env_slots {
				let ea: Vec<i64> = (0..5).map(|_| rng.int(1, 999)).collect();
				let ec: Vec<i64> = (0..9).map(|_| rng.int(1, 999)).collect();
				let fold31 = |t: &str| -> i64 {
					let mut h: i64 = 0;
					for b in t.bytes() {
						h = (h * 31 + b as i64) % KEY_MOD;
					}
					h
				};
let f_ty = fold31("Vector3Vector3");
				let f_ts = fold31(&format!("{}, {}, {}", ea[0], ea[1], ea[2]));
				let esum = (ea[0] * ec[0]
					+ ea[1] * ec[1]
					+ ea[2] * ec[2]
					+ ea[3] * ec[3]
					+ ea[4] * ec[4])
					% KEY_MOD;
				let einner = (esum * ec[5] + f_ty * ec[6] + f_ts * ec[7]) % KEY_MOD;
				let env_exp = (einner * ec[8]) % KEY_MOD;
				let env_mix = rng.int(1_048_576, KEY_MOD - 1);
				let env_mix_e = ke.key_expr(env_mix, None, rng);
				let env_mix_hi = (env_mix * 65536) % KEY_MOD;
				let env_mix_hi_e = ke.key_expr(env_mix_hi, None, rng);
				manifest_key("ENV_EXP", env_exp as u64);
				manifest_key("ENV_MIX", env_mix as u64);
				manifest_key("ENV_MIX_HI", env_mix_hi as u64);
				let gate_a = format!(
					"local EN1 = b[{s0}]({a0}, {a1}, {a2}) local EN7 = b[{s0}]({a0b}, {a1b}, {a2b}) local EN3 = 0 do local ENt = b[{s3}](EN1) for ENi = 1, #ENt do EN3 = (EN3 * 31 + BYTE(ENt, ENi)) % 268435456 end local ENt2 = b[{s3}](EN7) for ENi = 1, #ENt2 do EN3 = (EN3 * 31 + BYTE(ENt2, ENi)) % 268435456 end end\n    ",
					s0 = es[0], s3 = es[3],
					a0 = obf_num(ea[0] as u64, rng),
					a1 = obf_num(ea[1] as u64, rng),
					a2 = obf_num(ea[2] as u64, rng),
					a0b = obf_num((ea[0] + 1) as u64, rng),
					a1b = obf_num((ea[1] + 2) as u64, rng),
					a2b = obf_num((ea[2] + 3) as u64, rng),
				);
				let gate_b = format!(
					"local EN4 = b[{s1}]({a3}, {a4}) local EN6 = 0 do local ENs = TSTR(EN1) for ENi = 1, #ENs do EN6 = (EN6 * 31 + BYTE(ENs, ENi)) % 268435456 end end\n    ",
					s1 = es[1],
					a3 = obf_num(ea[3] as u64, rng),
					a4 = obf_num(ea[4] as u64, rng),
				);
				let gate_c = format!(
					"ef1 = (((EN1.X * {c0} + EN1.Y * {c1} + EN1.Z * {c2} + EN4.X * {c3} + EN4.Y * {c4}) * {c5} + EN3 * {c6} + EN6 * {c7}) * {c8}) % 268435456\n    ",
					c0 = obf_num(ec[0] as u64, rng),
					c1 = obf_num(ec[1] as u64, rng),
					c2 = obf_num(ec[2] as u64, rng),
					c3 = obf_num(ec[3] as u64, rng),
					c4 = obf_num(ec[4] as u64, rng),
					c5 = obf_num(ec[5] as u64, rng),
					c6 = obf_num(ec[6] as u64, rng),
					c7 = obf_num(ec[7] as u64, rng),
					c8 = obf_num(ec[8] as u64, rng),
				);
				(
					gate_a,
					gate_b,
					gate_c,
					format!(
						" + (ef1 % 65536) * {} + FLR(ef1 / 65536) * {}",
						env_mix_e, env_mix_hi_e
					),
					(env_exp * env_mix) % KEY_MOD,
					env_exp,
				)
			} else {
				(String::new(), String::new(), String::new(), String::new(), 0, 0)
			};
		let c_fold: i64 = {
			let mut h: i64 = 0;
			for &b in b"[C]".iter() {
				h = (h * 31 + b as i64) % KEY_MOD;
			}
			h
		};
		let meta_seed = rng.int(1_048_576, KEY_MOD - 1);
		let meta_seed_e = ke.key_expr(meta_seed, None, rng);
		let mut boot = String::new();
		let v_ls = "hls";
		let v_dbg = "hdbg";
		let v_inf = "hinf";
		let v_os = "hos";
		let v_clock = "hclk";
		let v_s = "hsarg";
			let v_ts = "hts";
		boot.push_str(&boot_names_block(
			rng,
			&[
				(v_ts, "tostring"), (v_ls, "loadstring"), (v_dbg, "debug"),
				(v_inf, "info"), (v_s, "s"), (v_os, "os"),
				(v_clock, "clock"),
			],
			&|v: i64, r: &mut Rng| obf_num(v as u64, r),
		));
		let probe_mix = rng.int(1_048_576, KEY_MOD - 1);
		let probe_mix_e = ke.key_expr(probe_mix, None, rng);
		let nlok_delta = rng.int(1_048_576, KEY_MOD - 1);
		let nlok_delta_e = ke.key_expr(nlok_delta, None, rng);
		manifest_key("NLOK_DELTA", nlok_delta as u64);
		let strlit = pool.lit("string");
		let probe_mix_hi = (probe_mix * 65536) % KEY_MOD;
		let probe_mix_hi_e = ke.key_expr(probe_mix_hi, None, rng);
		manifest_key("PROBE_MIX_HI", probe_mix_hi as u64);
		let comp = (meta_seed - c_fold * probe_mix - env_comp).rem_euclid(KEY_MOD);
		let comp_e = ke.key_expr(comp, None, rng);
		manifest_key("SEED_COMP", comp as u64);
		let meta_seed_full = format!(
			"({} + (pb % 65536) * {} + FLR(pb / 65536) * {}{}) % 268435456",
			comp_e, probe_mix_e, probe_mix_hi_e, env_term
		);
		let hbd_expr = format!(
			"(({})-({}))%268435456",
			meta_seed_full, meta_seed_e
		);
		if std::env::var("OBSUCVIM_BIND_DBG").is_ok() {
			eprintln!(
				"MIRROR hbd_clean=0 meta_seed={} comp={} env_exp={} c_fold={}",
				meta_seed, comp, env_exp, c_fold
			);
		}
		let ct_fill = String::new();
		let cx_fill = String::new();
		let hp_recv = if guard { String::from(", HPH") } else { String::new() };
		let hp_stash = if guard {
			format!("HW2[{}] = HPH", 256 + rng.int(1, 200))
		} else {
			String::new()
		};
		let (xtok_decl, xtok_extract) = if exg_consts.is_some() {
			(
				String::from("\n  local XTOK, XPROBE"),
				String::from("\n    local _xg = EXGFRAG() EXGFRAG = nil XTOK, XPROBE = _xg[1], _xg[2]"),
			)
		} else {
			(String::new(), String::new())
		};
		let mut av_order: Vec<usize> = vec![0, 1, 2];
		rng.shuffle(&mut av_order);
		let mut av_fold = String::new();
		for &src in &av_order {
			match src {
				0 => av_fold.push_str(&format!(
					"local avt = {{}} local avs = TSTR(avt) for avi = 1, #avs do AV = (AV * {} + BYTE(avs, avi)) % 268435456 end ",
					obf_num(rng.int(17, 47) as u64, rng)
				)),
				1 => av_fold.push_str(&format!(
					"local avf = function() end local afs = TSTR(avf) for avi = 1, #afs do AV = (AV * {} + BYTE(afs, avi)) % 268435456 end ",
					obf_num(rng.int(17, 47) as u64, rng)
				)),
				_ => av_fold.push_str(&format!(
					"local avc = CLK and CLK() or 0 local avn = FLR(avc * {}) for avj = 1, 5 do AV = (AV * {} + avn % 256) % 268435456 avn = FLR(avn / 256) end ",
					obf_num((rng.int(100_001, 9_999_999) | 1) as u64, rng),
					obf_num(rng.int(17, 47) as u64, rng)
				)),
			}
		}
        let hw_expand = if v15 {
            // R073 体积：别名拷贝逐对展开折叠为 pairs 表循环（语义逐条等价）。
            let mut hw_pairs: Vec<String> = Vec::new();
            for i in 0..active_op_count {
                let base = map.to_wire[0][i] as u16;
                for tt in 1..4 {
                    let w = map.to_wire[tt][i] as u16;
                    hw_pairs.push(format!("{{{},{}}}", w, base));
                }
            }
            for tt in 1..4 {
                let base = map.nop_alias[0] as u16;
                let w = map.nop_alias[tt] as u16;
                hw_pairs.push(format!("{{{},{}}}", w, base));
            }
            if hw_pairs.is_empty() {
                String::new()
            } else {
                format!("do local _av=AV; local _al={{{}}}; for _i=1,#_al do local p=_al[_i]; HW2[(p[1]+_av)%256]=HW2[(p[2]+_av)%256] end end", hw_pairs.join(","))
            }
        } else {
            String::new()
        };
        let vault_args: String = par_perm
            .iter()
            .map(|&i| shell_args[i].clone())
            .collect::<Vec<_>>()
            .join(", ");

		let build = format!(
			r#"{boot}local HW = {{}}
  local HBOOT
  local BSS
  local AV = 0
  local HW2 = {{}}
  local CT1, CT2, CT3, CT4
  local CX1, CX2, CX3, CX4
  local RTFRAG
  local EXGFRAG{env_frag_decl}
  local RPK1, RPK2, RPK3, RPK4, RPK5, RPK6, RPK7, RPK8, RPK9, RPK10, RPK11
  local HBD
  local CHQ
  local SALT
  local MZD = 0
  local SH1, SH2
  local HPH
  local PD = function() local z0, z1 = 0, 1 while z0 < 4294967296 do z0 = (z0 * 1103515245 + 12345) % 268435456 z1 = (z1 * 80659 + 95050511) % 268435456 if z0 == z1 and z0 == 123456789 then break end end end
  local MPF
  local CLK
  local CENVPROBE{xtok_decl}{env_decl}
  {hboot_fill}{exg_fill}{env_fill}do
    local LS = GFE(0)[{v_ls}]
    local TSTR = GFE(0)[{v_ts}]
    local DBG = GFE(0)[{v_dbg}]
    local INF = DBG and DBG[{v_inf}]
    local OS_ = GFE(0)[{v_os}]
    CLK = OS_ and OS_[{v_clock}]
    local pb = 0
    local ef1 = 0
-- NOTE (H4-3): ef1 与 native-loader probe 先完成折叠，再计算 HBD。
-- 忠实环境 HBD=0；失真环境直接污染 BSS/RT 双钥匙，不再解密 HBOOT 源码。
-- NOTE (㊳ 计时守卫复通): the assignment above feeds the OUTER `local CLK`
-- (no shadowing local) — the RUN call outside this do-block hands it to the
-- RT fragment as its timing source. History: ㉙ introduced the watchdog,
-- ㊱ found the old shadowing declaration dangled a mangled reference past
-- `end` (the seed=1 "attempt to call a table value" capture path) and parked
-- the guard on an explicit nil binding; symtab block scoping is fixed since,
-- so the wiring is restored deliberately. Guard semantics (RT fragment):
-- every 128 dispatch steps sample os.clock; first 4 samples set a MIN
-- baseline; two CONSECUTIVE windows slower than 100x baseline perturb a
-- frame-local numeric lane. Its per-function delay changes both later block
-- keys and the selected micro shard; no global flag or fixed poison literal.
    do
      local ok, sr = PCAL(function() return INF(LS, {v_s}) end)
      if ok and TYP(sr) == {strlit} then
        for i = 1, #sr do pb = (pb * 31 + BYTE(sr, i)) % 268435456 end
      end
      pb = (pb + (ok and 0 or {nlok_delta})) % 268435456
    end
    {env_a}{env_b}{env_c}{env_probe}local EV = {{}}
    do
      local _evs = GFE(1)
      if TYP(_evs) == "table" then
        local _evk, _evv = next(_evs)
        while _evk ~= nil do EV[_evk] = _evv _evk, _evv = next(_evs, _evk) end
      end
    end
    do
      {av_fold}
    end
-- NOTE (H4-2C1): BSS is now a one-shot static numeric phase machine. It is
-- passed through vault[nonce], consumed inside HBOOT, and cleared immediately;
-- no BSS source string or BSS loader call exists at runtime.
    HBD = {hbd}
    {bss_fill}{rt_fill}RPK1 = ({pseed} + AV) % 268435456
    RPK2 = ({pstep} + AV) % 268435456
    RPK3 = ({pkm} + AV) % 268435456
    RPK4 = ({pkc} + AV) % 268435456
    RPK5 = ({pblock} + AV) % 268435456
    RPK6 = {shcb1}
    RPK7 = {shcb2}
    RPK8 = {shcbc}
    RPK9 = {shcc1}
    RPK10 = {shcc2}
    RPK11 = {shccc}
    {vault_def}local _hok, _r1 = PCAL(function() return HBOOT({vault_args}) end) HBOOT = nil BSS = nil
    if not _hok then {hbootfail} end
    HW, MPF, SALT, SH1, SH2, CHQ{hp_recv} = _r1() _r1 = nil{xtok_extract}{env_extract}
    {hp_stash}
    {ct_fill}{cx_fill}for w, f in pairs(HW) do if w < 256 then HW2[(w + AV) % 256] = f end end
    {micro_fill}{hw_expand}{env_mzd}
    HW = nil
  end"#,
		boot = boot,
		hboot_fill = hboot_fill,
		exg_fill = exg_fill,
		hbd = hbd_expr,
		micro_fill = micro_fill,
		bss_fill = bss_fill,
		rt_fill = rt_fill,
            hw_expand = hw_expand,
		hp_recv = hp_recv,
		hp_stash = hp_stash,
		vault_def = vault_def,
		vault_args = vault_args,
		hbootfail = "PD()",
		xtok_decl = xtok_decl,
		xtok_extract = xtok_extract,
		env_decl = env_decl,
		env_frag_decl = env_frag_decl,
		env_extract = env_extract,
		env_fill = env_fill,
		env_mzd = env_mzd,
		av_fold = av_fold,
			env_a = env_gate_a,
			env_b = env_gate_b,
			env_c = env_gate_c,
			env_probe = cenv_probe,
			strlit = strlit,
			v_ls = v_ls, v_ts = v_ts, v_dbg = v_dbg, v_inf = v_inf,
			v_os = v_os, v_clock = v_clock,
			v_s = v_s,
			nlok_delta = nlok_delta_e,
			pseed = pseed_e, pstep = pstep_e, pkm = pkm_e, pkc = pkc_e,
			pblock = pblock_e,
			shcb1 = sh_cb1_e, shcb2 = sh_cb2_e, shcbc = sh_cbc_e,
			shcc1 = sh_cc1_e, shcc2 = sh_cc2_e, shccc = sh_ccc_e,
		);
		hfrag = build.clone();
		if std::env::var("OBSUCVIM_BUILD_DBG").is_ok() {
			eprintln!("BUILD_DBG: {}", build);
		}
		String::new()
	} else {
		hfrag = String::new();
		String::new()
	};
	let (fetch, branches) = if v15 {
		(String::new(), String::new())
	} else {
		(fetch, branches)
	};

	let callcap_src = if v15 {
		format!(
			"local TP = GFE(0)[{table}][{pack}]
  local function callcap(f, args, nargs)
    local r = TP(f(U(args, 1, nargs)))
    return r, r.n
  end",
			table = pool.lit("table"),
			pack = pool.lit("pack"),
		)
	} else {
		format!(
			"local function callcap(f, args, nargs)
    local w = function(...)
      local t = {{ ... }}
      return t, SEL({hash}, ...)
    end
    return w(f(U(args, 1, nargs)))
  end",
			hash = pool.lit("#"),
		)
	};
	let rt_helpers = format!(
		"local function mget(x, k)
    local mt = GMT(x)
    if TYP(mt) == {table} then return mt[k], false end
    return nil, mt ~= nil
  end
  local function resolve_call(f)
    if TYP(f) == {function} then return f, false end
    local mt = GMT(f)
    if TYP(mt) == {table} then
      local cc = mt[{call}]
      if TYP(cc) == {function} then return cc, true end
      if TYP(cc) == {table} then
        local cf = cc[f]
        if TYP(cf) == {function} then return cf, false end
      end
    elseif mt ~= nil then
      -- A protected __metatable value is intentionally opaque. Keep the
      -- object callable through Luau's native path instead of indexing the
      -- protection sentinel as if it were the real metatable.
      return f, false
    end
    ERR({callmsg} .. TYP(f) .. {value}, 0)
  end
  {callcap}",
		function = pool.lit("function"),
		call = pool.lit("__call"),
		table = pool.lit("table"),
		callmsg = pool.lit("attempt to call a "),
		value = pool.lit(" value"),
		callcap = callcap_src,
	);
	let ms_block = if v15 { pool.boot_block() } else { String::new() };

	let vm_params = params.clone();
	let entry_pf = if v15 { "MPF" } else { "PF[#FN]" };
	let entry_run = if v15 { "RUN" } else { "run" };
	let entry_tail = format!(
		"local vargs = {{}}\n  local V2 = {{}}\n  return {}({}, V2, {{}}, vargs, 0)",
		entry_run, entry_pf
	);
	let run_block: String = if v15 {
		run_unpack = String::new();
		run_soa = String::new();
		let call_tail = format!(
			"{}{}",
			if executor_guard { ", XTOK, XPROBE" } else { "" },
			if env_check { ", EVD" } else { "" }
		);
		format!(
			"local RUN = RTFRAG(RPK1, RPK2, RPK3, RPK4, RPK5, SALT, P, G, U, FLOOR, MS, AV, HW2, TP, mget, resolve_call, callcap, HAS_LEN_META, CT1, CT2, CT3, CT4, CX1, CX2, CX3, CX4, CLK, SH1, SH2, RPK6, RPK7, RPK8, RPK9, RPK10, RPK11, CKJ, CENVPROBE, MZD{}) RTFRAG = nil",
			call_tail
		)
	} else {
		let run_head = String::new();
		let pc_decl = String::from("local pc = 1\n    ");
		let ln_decl = "local lastn = 0\n    local lastbase = 0\n    ";
		let run_loop = format!("while true do\n      {}\n      {}\n    end", fetch, branches);
		format!(
			"local run\n  {mk}\n  run = function(pf, V, ups, vargs, vargc)\n    {run_head}{run_unpack}\n    {run_soa}\n    {pc_decl}{o_decl}{ln_decl}{handler_defs}{run_loop}\n  end",
			mk = makefn_decl,
			run_head = run_head,
			run_unpack = run_unpack,
			run_soa = run_soa,
			pc_decl = pc_decl,
			o_decl = String::new(),
			ln_decl = ln_decl,
			handler_defs = handler_defs,
			run_loop = run_loop,
		)
	};
	format!(
		r#"local VM = function({vm_params})
  local FN = {{{params}}}
  
  {oc_table}
  local PF = {{}}
  {p_fill}  {prim_unpack}
  {ms_block}{helpers}
  {ck_consts}{parse_fn}
  {decode_seg}
  {hfrag}{v15_selfmod}
  local G = GFE(0)
  local U = UNP
  local FLOOR = FLR
  local _probe = SMT({{}}, {{ __len = function() return 99 end }})
  local HAS_LEN_META = (_probe == nil) or false
  do
    local okp, vp = PCAL(function() return #_probe end)
    HAS_LEN_META = okp and vp == 99
  end
  {rt_helpers}
  {hub_decl}
  {run_block}
  {entry_tail}
end
"#,
	vm_params = vm_params,
	entry_tail = entry_tail,
	run_block = run_block,
	params = params,
		oc_table = oc_table,
		p_fill = p_fill,
		prim_unpack = prim_unpack,
		helpers = helpers,
		hub_decl = hub_decl,
		hfrag = hfrag,
		v15_selfmod = v15_selfmod,
		decode_seg = decode_seg,
		ck_consts = ck_consts,
		parse_fn = parse_fn_vis,
		rt_helpers = rt_helpers,
		ms_block = ms_block,
	)
}
