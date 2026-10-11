
use crate::ast::{self, Expr, TableField};
use crate::parser;
use crate::rng::Rng;
use crate::vmgen::isa::CARRIER_SPECIALS;

const PRIM_DOTS: &[&str] = &[
	"buffer.fill",
	"buffer.create",
	"buffer.len",
	"buffer.copy",
	"buffer.tostring",
	"buffer.fromstring",
	"buffer.readstring",
	"buffer.readu8",
	"buffer.readu16",
	"buffer.readi16",
	"buffer.readu32",
	"buffer.readi32",
	"buffer.readf32",
	"buffer.readf64",
	"buffer.writeu8",
	"buffer.writei8",
	"buffer.writeu32",
	"bit32.bxor",
	"bit32.band",
	"bit32.bor",
	"bit32.bnot",
	"bit32.lshift",
	"bit32.rshift",
	"bit32.countrz",
	"string.byte",
	"string.char",
	"string.sub",
	"string.rep",
	"string.find",
	"string.match",
	"string.gmatch",
	"string.gsub",
	"string.format",
	"string.pack",
	"string.unpack",
	"table.create",
	"table.pack",
	"table.insert",
	"table.concat",
	"table.move",
	"coroutine.create",
	"coroutine.resume",
	"coroutine.yield",
	"coroutine.wrap",
	"coroutine.status",
	"coroutine.running",
	"coroutine.close",
	"coroutine.isyieldable",
	"vector.create",
];

const PRIM_BARE: &[&str] = &[
	"typeof", "setfenv", "getfenv", "pcall", "xpcall", "error", "next",
	"select", "unpack", "tonumber", "tostring", "rawget", "rawset",
	"assert", "type", "setmetatable", "getmetatable",
];

const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";

fn dot_value(path: &str) -> Expr {
	let (lib, name) = path.split_once('.').unwrap();
	if lib == "string" && matches!(name, "byte" | "sub" | "char") {
		return Expr::Index {
			obj: Box::new(Expr::Str { bytes: Vec::new(), is_binary: false }),
			idx: Box::new(Expr::Str {
				bytes: name.as_bytes().to_vec(),
				is_binary: false,
			}),
		};
	}
	Expr::Dot {
		obj: Box::new(Expr::Ident {
			name: lib.to_string(),
			sym: None,
		}),
		name: name.to_string(),
	}
}

pub fn boot_name(rng: &mut Rng) -> String {
	let base = ALPHA[rng.int(0, 52) as usize] as char;
	format!("{}C", base)
}

fn parse_expr(src: &str) -> Expr {
	let chunk = format!("local _ = {}", src);
	let mut block = match parser::parse(&chunk, true) {
		Ok(b) => b,
		Err(e) => {
			eprintln!("v15 emitter bug: {:?}\nsource:\n{}", e, chunk);
			panic!("v15 static source")
		}
	};
	match block.stmts.pop().unwrap() {
		ast::Stmt::Local { mut values, .. } => values.pop().unwrap().unwrap(),
		_ => unreachable!(),
	}
}

fn lcg_factory(rng: &mut Rng, states: usize) -> Expr {
	let cell = "b[1][4][b[1][7]]";
	let step = |rng: &mut Rng, cell: &str| -> String {
		let m = rng.int(100_001, 1_100_001);
		let c = rng.int(1_000_000, 268_000_000);
		format!("({m}*{cell}+{c})%268435456", m = m, cell = cell, c = c)
	};
	let mut body = String::from("local u=0;while true do ");
	for s in 0..states {
		if s == 0 {
			body.push_str("if u<=0 then ");
		} else {
			body.push_str(&format!("elseif u<={} then ", s));
		}
		body.push_str(&format!("{cell}={v};", cell = cell, v = step(rng, cell)));
		body.push_str(&format!(
			"{cell},u={v},{ns};",
			cell = cell,
			v = step(rng, cell),
			ns = s + 1
		));
	}
	body.push_str("else return;end;end");
	let params = if states >= 2 { "b,b,b,u" } else { "b,b" };
	let src = format!(
		"function({params})return function() {body} end end",
		params = params,
		body = body
	);
	parse_expr(&src)
}

pub fn module_fields(
	rng: &mut Rng,
	exclude: &[i64],
	slot_max: i64,
	bind_env: Option<&str>,
	env_slots_in: Option<&[i64; 4]>,
) -> (Vec<TableField>, Option<[i64; 4]>) {
	let top = std::cmp::max(126, slot_max);
	let mut slots: Vec<i64> = (1..=top)
		.filter(|n| !exclude.contains(n))
		.collect();
	rng.shuffle(&mut slots);
	let mut it = slots.into_iter();
	let mut fields: Vec<TableField> = Vec::new();

	for path in PRIM_DOTS.iter().chain(PRIM_BARE.iter()) {
		let n = it.next().unwrap();
		let value = if path.contains('.') {
			dot_value(path)
		} else {
			Expr::Ident {
				name: path.to_string(),
				sym: None,
			}
		};
		fields.push(TableField::Key {
			key: Expr::Num {
				value: n as f64,
				isfloat: false,
			},
			value,
		});
	}
	let mut env_slots_out: Option<[i64; 4]> = None;
	if bind_env == Some("roblox") {
		if let Some(es_in) = env_slots_in {
			for (path, n) in ["Vector3.new", "Vector2.new", "task.defer", "typeof"]
				.iter()
				.zip(es_in.iter())
			{
				let value = if path.contains('.') {
					dot_value(path)
				} else {
					Expr::Ident { name: path.to_string(), sym: None }
				};
				fields.push(TableField::Key {
					key: Expr::Num { value: *n as f64, isfloat: false },
					value,
				});
			}
			env_slots_out = Some(*es_in);
		}
	}
	for states in [2usize, 3usize] {
		let n = it.next().unwrap();
		fields.push(TableField::Key {
			key: Expr::Num {
				value: n as f64,
				isfloat: false,
			},
			value: lcg_factory(rng, states),
		});
	}
	{
		let n = it.next().unwrap();
		fields.push(TableField::Key {
			key: Expr::Num {
				value: n as f64,
				isfloat: false,
			},
			value: Expr::Num {
				value: 0.0,
				isfloat: false,
			},
		});
	}

	let named_str = |name: &str, val: &str| TableField::Key {
		key: Expr::Str {
			bytes: name.as_bytes().to_vec(),
			is_binary: false,
		},
		value: Expr::Str {
			bytes: val.as_bytes().to_vec(),
			is_binary: false,
		},
	};
	fields.push(named_str("AL", "n"));
	fields.push(named_str("BL", "__index"));
	fields.push(named_str("EL", ": "));
	fields.push(named_str("PL", "?"));
	fields.push(named_str("cL", "string"));
	fields.push(TableField::Key {
		key: Expr::Str {
			bytes: b"gL".to_vec(),
			is_binary: false,
		},
		value: Expr::Str {
			bytes: b":(%d+)[:\r\n]".to_vec(),
			is_binary: false,
		},
	});
	fields.push(TableField::Key {
		key: Expr::Str {
			bytes: b"QL".to_vec(),
			is_binary: false,
		},
		value: Expr::Bool { value: false },
	});
	fields.push(TableField::Key {
		key: Expr::Str {
			bytes: b"s".to_vec(),
			is_binary: false,
		},
		value: Expr::Table { fields: Vec::new() },
	});
	{
		let mut elems = vec![Expr::Num {
			value: 24123.0,
			isfloat: false,
		}];
		for _ in 0..8 {
			elems.push(Expr::Num {
				value: rng.int(0, 4_294_967_295) as f64,
				isfloat: false,
			});
		}
		fields.push(TableField::Key {
			key: Expr::Str {
				bytes: b"h".to_vec(),
				is_binary: false,
			},
			value: Expr::Table { fields: elems.into_iter().map(TableField::Array).collect() },
		});
	}

	rng.shuffle(&mut fields);
	(fields, env_slots_out)
}

fn lua_bytes_lit(bytes: &[u8]) -> String {
	let mut s = String::with_capacity(bytes.len() * 4 + 2);
	s.push('"');
	for &b in bytes {
		match b {
			b'"' => s.push_str("\\\""),
			b'\\' => s.push_str("\\\\"),
			32..=126 => s.push(b as char),
			_ => s.push_str(&format!("\\{:03}", b)),
		}
	}
	s.push('"');
	s
}

struct Names {
	pool: Vec<String>,
	fallback: usize,
	reserved: std::collections::HashSet<String>,
}

impl Names {
	fn new(rng: &mut Rng) -> Names {
		let mut pool = Vec::new();
		let singles: Vec<&str> = vec![
			"a", "c", "d", "e", "f", "g", "i", "j", "k", "l", "m", "n",
			"o", "p", "q", "r", "t", "u", "v", "w", "x", "y", "z", "A",
			"B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
			"N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y",
			"Z", "_",
		];
		for b in singles.iter() {
			pool.push(b.to_string());
			pool.push(format!("{}L", b));
			pool.push(format!("{}C", b));
		}
		for a in singles.iter() {
			for b in singles.iter() {
				pool.push(format!("{}{}", a, b));
			}
		}
		rng.shuffle(&mut pool);
		let reserved = [
			"AL", "BL", "EL", "PL", "QL", "XC", "cL", "dL", "gL", "RC",
			"pC", "h", "s", "b", "do", "if", "in", "or",
		];
		pool.retain(|n| !reserved.contains(&n.as_str()));
		const KW: &[&str] = &[
			"and", "break", "do", "else", "elseif", "end", "false", "for",
			"function", "if", "in", "local", "nil", "not", "or", "repeat",
			"return", "then", "true", "until", "while", "continue",
		];
		pool.retain(|n| {
			!KW.contains(&n.as_str())
				&& !KW.contains(&(n.clone() + "t").as_str())
				&& !KW.contains(&(n.clone() + "o").as_str())
				&& !KW.contains(&(n.clone() + "i").as_str())
		});
		let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
		pool.retain(|n| seen.insert(n.clone()));
		Names { pool, fallback: 0, reserved: std::collections::HashSet::new() }
	}
	fn reserve(&mut self, names: &[String]) {
		for n in names {
			self.reserved.insert(n.clone());
		}
		self.pool.retain(|n| !self.reserved.contains(n));
	}
	fn take(&mut self) -> String {
		match self.pool.pop() {
			Some(n) => n,
			None => {
				self.fallback += 1;
				let mut cand = format!("vN{}", self.fallback);
				while self.reserved.contains(&cand) {
					self.fallback += 1;
					cand = format!("vN{}", self.fallback);
				}
				cand
			}
		}
	}
	fn take_avoid(&mut self, forbidden: &[&str]) -> String {
		let mut stashed: Vec<String> = Vec::new();
		let result = loop {
			match self.pool.pop() {
				Some(n) if !forbidden.contains(&n.as_str()) => break n,
				Some(n) => stashed.push(n),
				None => {
					self.fallback += 1;
					let mut cand = format!("vN{}", self.fallback);
					while self.reserved.contains(&cand)
						|| forbidden.contains(&cand.as_str())
					{
						self.fallback += 1;
						cand = format!("vN{}", self.fallback);
					}
					break cand;
				}
			}
		};
		self.pool.extend(stashed);
		result
	}
}

fn opaque_wrap(code: String, rng: &mut crate::rng::Rng, vname: &str) -> String {
	let preds: [String; 4] = [
		format!("((({v}*{v})+{v})%2)==0", v = vname),
		format!("(({v}%3)*({v}%3))>=0", v = vname),
		format!("(({v}+1)*({v}+1))>=({v}*{v})", v = vname),
		format!("((({v}%5)*({v}%5))*(({v}%5)*({v}%5)))>=0", v = vname),
	];
	let p = &preds[rng.int(0, 3) as usize];
	if rng.int(0, 1) == 0 {
		format!("if {} then {} else local _={} end", p, code, vname)
	} else {
		format!("if not({}) then local _={} else {} end", p, vname, code)
	}
}

fn dispatch_tree(
	vname: &str,
	leaves: &[(i64, String)],
	lo: usize,
	hi: usize,
) -> String {
	if hi - lo == 1 {
		return leaves[lo].1.clone();
	}
	let mid = (lo + hi) / 2;
	let thr = leaves[mid - 1].0;
	format!(
		"if {}<={} then {} else {} end",
		vname,
		thr,
		dispatch_tree(vname, leaves, lo, mid),
		dispatch_tree(vname, leaves, mid, hi)
	)
}

pub const CHUNKS: usize = 5;

pub const BW_SLOT_WORDS: usize = 90;

pub fn decoy_slots(max_carrier_len: usize, avoid: &[i64]) -> (i64, i64) {
	let mut d1 = max_carrier_len as i64 + 500;
	let mut d2 = d1 + 1;
	while avoid.contains(&d1) {
		d1 += 2;
	}
	while avoid.contains(&d2) || d2 == d1 {
		d2 += 2;
	}
	(d1, d2)
}

fn coded_name(out: &mut String, var: &str, name: &str, sc_slot: i64, rng: &mut Rng) {
	let codes: Vec<u8> = name.bytes().collect();
	let mut pos: Vec<usize> = (0..codes.len()).collect();
	rng.shuffle(&mut pos);
	out.push_str(&format!("local {var}t={{}};"));
	for (i, &c) in codes.iter().enumerate() {
		out.push_str(&format!("{var}t[{}]={c};", pos[i] + 1));
	}
	let mut inv = vec![0usize; codes.len()];
	for (i, &p) in pos.iter().enumerate() {
		inv[i] = p + 1;
	}
	out.push_str(&format!("local {var}o={{"));
	out.push_str(&inv.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
	out.push_str(&format!(
		"}};local {var}=\"\";for {var}i=1,#{var}o do {var}={var}..b[{sc}]({var}t[{var}o[{var}i]]) end;",
		sc = sc_slot
	));
}

fn draw_state_names(nm: &mut Names) -> [String; 6] {
	const FIXED: &[&str] = &[
		"b", "C", "u", "z", "Z", "o", "w", "K", "G", "q", "M", "F", "H",
		"E", "J", "B",
	];
	let mut forbid: Vec<String> = FIXED.iter().map(|s| s.to_string()).collect();
	let mut out: Vec<String> = Vec::new();
	for _ in 0..6 {
		let fs: Vec<&str> = forbid.iter().map(|s| s.as_str()).collect();
		let n = nm.take_avoid(&fs);
		forbid.push(n.clone());
		out.push(n);
	}
	out.try_into().unwrap()
}

fn st_pair_expr(
	st1: &mut [i64],
	st2: &mut [i64],
	f1: &mut Vec<usize>,
	f2: &mut Vec<usize>,
	s0: i64,
	s1slot: i64,
	v: i64,
	rng: &mut Rng,
) -> String {
	let e1 = if v > 0 { rng.int(0, v) } else { 0 };
	let e2 = v - e1;
	let p1 = f1.pop().expect("st table exhaustion");
	let p2 = f2.pop().expect("st table exhaustion");
	st1[p1] = e1;
	st2[p2] = e2;
	format!("(b[{}][{}]+b[{}][{}])", s0, p1 + 1, s1slot, p2 + 1)
}

pub fn scaffold(
	rng: &mut Rng,
	interp_src: &str,
	vm_name: &str,
	carriers: &[Vec<u8>],
	runner1: i64,
	runner2: i64,
	ks_slot: i64,
	kg_slot: i64,
	decoy1: i64,
	decoy2: i64,
	carrier: &crate::vmgen::isa::Carrier,
	bw_slots: &[i64],
	prim_extra: (i64, i64),
	kfrag: &[i64],
	st_slots: &[i64],
	interp_globals: &[String],
) -> (Vec<TableField>, String) {
	let carrier_tokens: &[String] = &carrier.tokens;
	let mut nm = Names::new(rng);
	nm.reserve(interp_globals);
	let state = draw_state_names(&mut nm);
	let sv = state[0].clone();
	let rhs5 = format!(
		"{},{},{},{},{}",
		state[1], state[2], state[3], state[4], state[5]
	);
	let lhs6 = format!("{},{}", sv, rhs5);
	let leaf = |name: &str| {
		format!("{lhs6}=b:{name}(C,{rhs5});continue;")
	};
let fc = nm.take();
let init = nm.take();
let xl = nm.take();
let init2 = nm.take();
	let init3 = nm.take();
let mul = nm.take();
let modulo = nm.take();
let loop_name = nm.take();
let ctl = nm.take();
let ehandler = nm.take();
let v1h = nm.take();
let v2h = nm.take();

	let n = carriers.len();

	let ks_seed: i64 = rng.int(1_048_576, 268_435_455);
	let lcm = |rng: &mut Rng| rng.int(1_048_577, 33_000_001) | 1;
	let lcc = |rng: &mut Rng| rng.int(1_048_576, 268_000_000) | 1;
	let (km1, kc1) = (lcm(rng), lcc(rng));
	let (km2, kc2) = (lcm(rng), lcc(rng));
	let (km3, kc3) = (lcm(rng), lcc(rng));
	crate::vmgen::manifest_key("KS_SEED", ks_seed as u64);
	crate::vmgen::manifest_key("KG_M1", km1 as u64);
	crate::vmgen::manifest_key("KG_C1", kc1 as u64);
	crate::vmgen::manifest_key("KG_M2", km2 as u64);
	crate::vmgen::manifest_key("KG_C2", kc2 as u64);
	crate::vmgen::manifest_key("KG_M3", km3 as u64);
	crate::vmgen::manifest_key("KG_C3", kc3 as u64);
	let mut ks_state: i64 = ks_seed;
	let split_field = |v: i64, rng: &mut Rng| -> Expr {
		let d = rng.int(10_000_000, 99_999_999);
		parse_expr(&format!("({}-{})", v + d, d))
	};
	let ks_frag_a = rng.int(0, 268_435_455);
	let ks_frag_b = (ks_seed - ks_frag_a).rem_euclid(268_435_456);
	let mut st1: Vec<i64> = (0..48).map(|_| rng.int(1_000_000, 4_294_967_295)).collect();
	let mut st2: Vec<i64> = (0..48).map(|_| rng.int(1_000_000, 4_294_967_295)).collect();
	let mut st1_free: Vec<usize> = {
		let mut v: Vec<usize> = (0..48).collect();
		rng.shuffle(&mut v);
		v
	};
	let mut st2_free: Vec<usize> = {
		let mut v: Vec<usize> = (0..48).collect();
		rng.shuffle(&mut v);
		v
	};

	let rt_name = nm.take();

	let chunked: Vec<Vec<Vec<u8>>> = carriers
		.iter()
		.map(|c| {
			let base = c.len() / CHUNKS;
			let rem = c.len() % CHUNKS;
			let mut out = Vec::new();
			let mut s = 0;
			for i in 0..CHUNKS {
				let len = base + if i < rem { 1 } else { 0 };
				out.push(c[s..s + len].to_vec());
				s += len;
			}
			out
		})
		.collect();

	let vmslot = n * CHUNKS + 1;
	let entryslot = vmslot + 1;
	let sumslot = entryslot + 1;
	let verifyslot = sumslot + 1;
	let auxslot = verifyslot + 1;

	const MIN_STAGING: usize = 100;
	let natural = n * (1 + CHUNKS);
	let filler = if natural < MIN_STAGING { MIN_STAGING - natural } else { 0 };
	let nsteps = natural + filler + 4;
	let need = (nsteps + 1) as i64;
	let mut ids: Vec<i64> = (3..=236).collect();
	if (ids.len() as i64) < need {
		ids.extend(237..=236 + need);
	}
	rng.shuffle(&mut ids);
	let mut states: Vec<i64> = ids[..need as usize].to_vec();
	states.sort();
	let sdone = states[nsteps as usize];

	let mut fields: Vec<TableField> = Vec::new();
	let mut leaves: Vec<(i64, String)> = Vec::new();
	let named = |name: String, value: Expr| TableField::Key {
		key: Expr::Str {
			bytes: name.into_bytes(),
			is_binary: false,
		},
		value,
	};
	let slotted = |slot: i64, value: Expr| TableField::Key {
		key: Expr::Num {
			value: slot as f64,
			isfloat: false,
		},
		value,
	};
	fields.push(slotted(ks_slot, split_field(ks_frag_a, rng)));
	fields.push(slotted(kfrag[0], split_field(ks_frag_b, rng)));
	fields.push(slotted(
		kg_slot,
		parse_expr(&format!(
			"function(b) local x=b[{ks}];if b[{fb}]~=nil then x=(x+b[{fb}])%268435456;b[{ks}]=x;b[{fb}]=nil end;local u=0;while true do if u<=0 then x=(x*{m1}+{c1})%268435456;u=1 elseif u<=1 then local y=(x*{m2}+{c2})%268435456;x=y;u=2 else local z=(x*{m3}+{c3})%268435456;b[{ks}]=z;return z end end end",
			ks = ks_slot,
			fb = kfrag[0],
			m1 = st_pair_expr(&mut st1, &mut st2, &mut st1_free, &mut st2_free, st_slots[0], st_slots[1], km1, rng),
			c1 = st_pair_expr(&mut st1, &mut st2, &mut st1_free, &mut st2_free, st_slots[0], st_slots[1], kc1, rng),
			m2 = st_pair_expr(&mut st1, &mut st2, &mut st1_free, &mut st2_free, st_slots[0], st_slots[1], km2, rng),
			c2 = st_pair_expr(&mut st1, &mut st2, &mut st1_free, &mut st2_free, st_slots[0], st_slots[1], kc2, rng),
			m3 = st_pair_expr(&mut st1, &mut st2, &mut st1_free, &mut st2_free, st_slots[0], st_slots[1], km3, rng),
			c3 = st_pair_expr(&mut st1, &mut st2, &mut st1_free, &mut st2_free, st_slots[0], st_slots[1], kc3, rng),
		)),
	));
	let mut fillers: Vec<String> = ["p1", "p2", "p3", "p4", "p5"].iter().map(|s| s.to_string()).collect();
	let mut state_i = 0usize;
	let mut step = |i: &mut usize| -> (i64, i64) {
		let d = states[*i];
		let r = states[*i + 1];
		*i += 1;
		(d, r)
	};

	fields.push(named(
		init.clone(),
		parse_expr(&format!(
			"function(b,...) return true,{},nil,nil,nil,nil,nil,nil,nil,nil,nil,nil end",
			states[0]
		)),
	));
	fields.push(named(
		init2.clone(),
		parse_expr(&format!(
			"function(b,...) return true,{},nil,nil,nil,nil,nil,nil,nil,nil end",
			sdone + 1
		)),
	));
	fields.push(named(
		init3.clone(),
		parse_expr(&format!(
			"function(b,...) return true,{},nil,nil,nil,nil,nil,nil,nil,nil,nil,nil,nil,nil,nil,nil end",
			sdone + 2
		)),
	));

	fields.push(named(
		mul.clone(),
		parse_expr(
			"function(b,u,z) local Z=bit32.band; u,z=Z(u,4294967295),Z(z,4294967295); local o,w=Z(u,65535),bit32.rshift; local K,G,q=w(u,16),Z(z,65535),w(z,16); return Z(o*G+bit32.lshift(Z(o*q+K*G,65535),16),4294967295)%4294967296 end",
		),
	));
	fields.push(named(
		modulo.clone(),
		parse_expr("function(b,b) return b%4294967296 end"),
	));

	fields.push(TableField::Key {
		key: Expr::Num { value: prim_extra.0 as f64, isfloat: false },
		value: dot_value("string.byte"),
	});
	fields.push(TableField::Key {
		key: Expr::Num { value: prim_extra.1 as f64, isfloat: false },
		value: dot_value("string.char"),
	});
	for &dk in &[decoy1, decoy2] {
		let src = format!(
			"function(b) local q=7; q=(213*q+225)%268435456; \
			 q=(q*213+225)%268435456; return q end"
		);
		fields.push(slotted(dk, parse_expr(&src)));
	}

	{
		let mut entries: Vec<TableField> = Vec::new();
		for (i, tok) in carrier_tokens.iter().enumerate() {
			entries.push(TableField::Key {
				key: Expr::Str {
					bytes: vec![CARRIER_SPECIALS[i]],
					is_binary: false,
				},
				value: Expr::Str {
					bytes: tok.as_bytes().to_vec(),
					is_binary: false,
				},
			});
		}
		fields.push(named(
			rt_name.clone(),
			Expr::Table { fields: entries },
		));
	}

	let hb_name = nm.take();
	let alphabet = carrier.alphabet;
	let mut all_words: Vec<u32> = Vec::new();
let mut chunk_info: Vec<(usize, usize, usize)> = Vec::new();
	for chunks in chunked.iter() {
		for chunk in chunks.iter() {
			ks_state = (km1 * ks_state + kc1) % 268435456;
			ks_state = (km2 * ks_state + kc2) % 268435456;
			ks_state = (km3 * ks_state + kc3) % 268435456;
			let ks_const: i64 = ks_state;
			let kf = crate::vmgen::isa::fold_key(ks_const as u64) as i64;
			let mut xb: Vec<u8> = chunk
				.iter()
				.enumerate()
				.map(|(i, &c)| {
					let key = ((kf + (i as i64 + 1)) % 256) as u8;
					c ^ key
				})
				.collect();
			let byte_len = xb.len();
			while xb.len() % 4 != 0 {
				xb.push(0);
			}
let start_word = all_words.len() + 1;
			for w4 in xb.chunks(4) {
				all_words.push(u32::from_le_bytes([w4[0], w4[1], w4[2], w4[3]]));
			}
			let num_words = xb.len() / 4;
			chunk_info.push((start_word, num_words, byte_len));
		}
	}
	let bm0 = rng.int(1, 4_294_967_295) as i64;
	let bc0 = (rng.int(1, 4_294_967_295) | 1) as i64;
	crate::vmgen::manifest_key("BW_M0", bm0 as u64);
	crate::vmgen::manifest_key("BW_C0", bc0 as u64);
	let bm0_a = rng.int(0, 4_294_967_295);
	let bm0_b = (bm0 - bm0_a).rem_euclid(4_294_967_296);
	let bc0_a = rng.int(0, 4_294_967_295);
	let bc0_b = (bc0_a - bc0).rem_euclid(4_294_967_296);
	fields.push(slotted(kfrag[1], split_field(bm0_a, rng)));
	fields.push(slotted(kfrag[2], split_field(bm0_b, rng)));
	fields.push(slotted(kfrag[3], split_field(bc0_a, rng)));
	fields.push(slotted(kfrag[4], split_field(bc0_b, rng)));
	let n_slots_needed = (all_words.len() + BW_SLOT_WORDS - 1) / BW_SLOT_WORDS;
	assert!(
		n_slots_needed <= bw_slots.len(),
		"not enough BW slots: need {} have {}",
		n_slots_needed,
		bw_slots.len()
	);
	let masked: Vec<u32> = all_words
		.iter()
		.enumerate()
		.map(|(gi, &w)| {
			let g = gi as i64 + 1;
			let key = (bm0 + g * bc0) % 4_294_967_296;
			((w as i64 + key) % 4_294_967_296) as u32
		})
		.collect();
	for (si, slot) in bw_slots.iter().take(n_slots_needed).enumerate() {
		let lo = si * BW_SLOT_WORDS;
		let hi = masked.len().min(lo + BW_SLOT_WORDS);
		let mut chars: Vec<u8> = Vec::with_capacity((hi - lo) * 5);
		for &w in &masked[lo..hi] {
			let mut v = w;
			let mut d = [0u8; 5];
			for k in 0..5 {
				d[4 - k] = 33 + (v % 94) as u8;
				v /= 94;
			}
			chars.extend_from_slice(&d);
		}
		fields.push(TableField::Key {
			key: Expr::Num { value: *slot as f64, isfloat: false },
			value: Expr::LongStr { bytes: chars },
		});
	}
	let mut hb_bytes: Vec<u8> = Vec::new();
	while hb_bytes.len() < 10_500 {
		hb_bytes.push(alphabet[rng.int(0, 93) as usize]);
	}
	fields.push(named(
		hb_name.clone(),
		Expr::LongStr { bytes: hb_bytes.clone() },
	));

	for (k, chunks) in chunked.iter().enumerate() {
		{
			fillers = (0..5).map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d"])).collect();
			rng.shuffle(&mut fillers);
			let (ra, rb, rc, rd, re) = (fillers[0].as_str(), fillers[1].as_str(), fillers[2].as_str(), fillers[3].as_str(), fillers[4].as_str());
			let (st, ret) = step(&mut state_i);
			let name = nm.take();
			let ksum: usize = carriers[k].iter().map(|&b| b as usize).sum();
			crate::vmgen::manifest_key(&format!("KSUM_{}", k), ksum as u64);
			let ksum_e = st_pair_expr(
				&mut st1, &mut st2, &mut st1_free, &mut st2_free,
				st_slots[0], st_slots[1], ksum as i64, rng,
			);
			let decoy_a = states[rng.int(0, (states.len() / 2) as i64) as usize];
			let decoy_b = states[rng.int((states.len() / 2) as i64, (states.len() - 1) as i64) as usize];
			let qf = nm.take_avoid(&["b", "C", "s"]);
			let src = format!(
				"function(b,C,{ra},{rb},{rc},{rd},{re}) local s=C[{sumslot}]; \
				local {qf}=if C[{sumslot}]>=0 then {da} else {db}; \
				 C[{sumslot}]=b:{modulo}(b:{mul}(s,31)+{ksum}); \
				 return {ret},C,{rb},{rc},{rd},{re},{ra} end",
				sumslot = sumslot,
				da = decoy_a,
				db = decoy_b,
				modulo = modulo,
				mul = mul,
				ksum = ksum_e,
				ret = ret,
				ra = ra,
				rb = rb,
				rc = rc,
				rd = rd,
				re = re,
				qf = qf,
			);
			fields.push(named(name.clone(), parse_expr(&src)));
			leaves.push((
				st,
				leaf(&name),
			));
		}
		for (j, chunk) in chunks.iter().enumerate() {
			fillers = (0..5).map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d"])).collect();
			rng.shuffle(&mut fillers);
			let (ra, rb, rc, rd, re) = (fillers[0].as_str(), fillers[1].as_str(), fillers[2].as_str(), fillers[3].as_str(), fillers[4].as_str());
			let (st, ret) = step(&mut state_i);
			let name = nm.take();
			let _ = chunk;
			let (sw, nw, blen) = chunk_info[k * CHUNKS + j];
			let ew = sw + nw - 1;
			const CH_AVOID: &[&str] = &["b", "C"];
			let nkm = nm.take_avoid(CH_AVOID);
			let nkc = nm.take_avoid(CH_AVOID);
			let nt = nm.take_avoid(CH_AVOID);
			let ng = nm.take_avoid(CH_AVOID);
			let nws = nm.take_avoid(CH_AVOID);
			let nwi = nm.take_avoid(CH_AVOID);
			let nwd = nm.take_avoid(CH_AVOID);
			let nkv = nm.take_avoid(CH_AVOID);
			let nti = nm.take_avoid(CH_AVOID);
			let while_words = rng.int(0, 1) == 1;
			let divmod_bytes = rng.int(0, 1) == 1;
			let add_unmask = rng.int(0, 1) == 1;
			let trim_style = rng.int(0, 1) == 1;
			let nkn = nm.take_avoid(CH_AVOID);
			let byte_emit = if divmod_bytes {
				let mut s = String::new();
				for step in 0..4u16 {
					s.push_str(&format!(
						"{t}[{ti}]=b[{sc}](bit32.bxor({wd}%256,({kn}+{ti})%256));",
						t = nt, ti = nti, wd = nwd, kn = nkn, sc = prim_extra.1
					));
					if step < 3 {
						s.push_str(&format!("{}=({}-{}%256)/256;", nwd, nwd, nwd));
					}
					s.push_str(&format!("{}={}+1;", nti, nti));
				}
				s
			} else {
				format!(
					"{t}[#{t}+1]=b[{sc}](bit32.bxor({wd}%256,({kn}+{ti})%256),bit32.bxor(bit32.extract({wd},8,8),({kn}+{ti}+1)%256),bit32.bxor(bit32.extract({wd},16,8),({kn}+{ti}+2)%256),bit32.bxor(bit32.extract({wd},24,8),({kn}+{ti}+3)%256));{ti}={ti}+4;",
					t = nt, ti = nti, wd = nwd, kn = nkn, sc = prim_extra.1
				)
			};
			let word_src = if rng.int(0, 1) == 0 {
				let nco = nm.take_avoid(CH_AVOID);
				let cs: Vec<String> = (0..5)
					.map(|_| nm.take_avoid(CH_AVOID))
					.collect();
				let unpack = format!(
					"local {co}=({wi}-1)*5+1;local {c}=b[{sb}]({ws},{co},{co}+4);",
					co = nco, wi = nwi, c = cs.join(","), ws = nws, sb = prim_extra.0
				);
				let horn = format!(
					"(((({c1}-33)*94+{c2}-33)*94+{c3}-33)*94+{c4}-33)*94+{c5}-33",
					c1 = cs[0], c2 = cs[1], c3 = cs[2], c4 = cs[3], c5 = cs[4]
				);
				let unmask = if add_unmask {
					format!(
						"local {wd}=({h}+(4294967296-({km}+{gc}*{kc})%4294967296))%4294967296;{gc}={gc}+1;",
						wd = nwd, h = horn, km = nkm, gc = ng, kc = nkc
					)
				} else {
					format!(
						"local {wd}=({h}-({km}+{gc}*{kc})%4294967296)%4294967296;{gc}={gc}+1;",
						wd = nwd, h = horn, km = nkm, gc = ng, kc = nkc
					)
				};
				format!("{}{}{}", unpack, unmask, byte_emit)
			} else {
				let byte_at = |k: i64| {
					format!(
						"(b[{sb}]({ws},({wi}-1)*5+{k})-33)",
						ws = nws, wi = nwi, k = k, sb = prim_extra.0
					)
				};
				let vhorn = format!(
					"(((({b1})*94+{b2})*94+{b3})*94+{b4})*94+{b5}",
					b1 = byte_at(1),
					b2 = byte_at(2),
					b3 = byte_at(3),
					b4 = byte_at(4),
					b5 = byte_at(5)
				);
				let unmask = if add_unmask {
					format!(
						"local {wd}=(({vhorn}+(4294967296-({km}+{gc}*{kc})%4294967296))%4294967296);{gc}={gc}+1;",
						wd = nwd, vhorn = vhorn, km = nkm, gc = ng, kc = nkc
					)
				} else {
					format!(
						"local {wd}=(({vhorn}-({km}+{gc}*{kc})%4294967296)%4294967296);{gc}={gc}+1;",
						wd = nwd, vhorn = vhorn, km = nkm, gc = ng, kc = nkc
					)
				};
				format!("{}{}", unmask, byte_emit)
			};
			let mut seg_code = String::new();
			let mut g = sw;
			while g <= ew {
let si = (g - 1) / BW_SLOT_WORDS;
let local_lo = (g - 1) % BW_SLOT_WORDS + 1;
				let slot_end_global = (si + 1) * BW_SLOT_WORDS;
				let seg_hi = ew.min(slot_end_global);
				let local_hi = local_lo + (seg_hi - g);
				seg_code.push_str(&format!("local {ws}=b[{slot}]; ", ws = nws, slot = bw_slots[si]));
				if while_words {
					seg_code.push_str(&format!(
						"local {wi}={lo};while {wi}<={hi} do {body}{wi}={wi}+1 end; ",
						wi = nwi, lo = local_lo, hi = local_hi, body = word_src
					));
				} else {
					seg_code.push_str(&format!(
						"for {wi}={lo},{hi} do {body}end; ",
						wi = nwi, lo = local_lo, hi = local_hi, body = word_src
					));
				}
				g = seg_hi + 1;
			}
			let decoy_a = states[rng.int(0, (states.len() / 2) as i64) as usize];
			let decoy_b = states[rng.int((states.len() / 2) as i64, (states.len() - 1) as i64) as usize];
			let qc = nm.take_avoid(CH_AVOID);
			let qr = nm.take_avoid(CH_AVOID);
			let final_concat = if trim_style && divmod_bytes {
				format!(
					"table.concat({t},nil,1,{blen})",
					t = nt, blen = blen
				)
			} else if trim_style {
				format!(
					"(\"\")[\"sub\"](table.concat({t},nil,1,{nw}),1,{blen})",
					t = nt, nw = nw, blen = blen
				)
			} else {
				format!(
					"(\"\")[\"sub\"](table.concat({t}),1,{blen})",
					t = nt, blen = blen
				)
			};
			let src = format!(
				"function(b,C,{ra},{rb},{rc},{rd},{re}) \
				 local {km}=(b[{kfa}]+b[{kfb}])%4294967296;local {kc}=(b[{kfc}]-b[{kfd}])%4294967296; \
				 local {t}={{}}; local {g}={sw}; \
				 local {kv}=b[{kg}](b); local {kn}=(({kv}%256)+bit32.extract({kv},8,8)+bit32.extract({kv},16,8)+bit32.extract({kv},24,4))%256; \
				 local {ti}=1; \
				 {segs}\
				 local {qc}=C[{idx}]~=nil and {da} or {db}; \
				 local {qr}=if {da}~={db} then {da} else {db}; \
				 C[{idx}]={final_concat}; \
				 return {ret},C,{rc},{ra},{rd},{re},{rb} end",
				idx = k * CHUNKS + j + 1,
				segs = seg_code,
				sw = sw,
				final_concat = final_concat,
				t = nt,
				g = ng,
				km = nkm,
				kc = nkc,
				kv = nkv,
				kn = nkn,
				ti = nti,
				kg = kg_slot,
				kfa = kfrag[1],
				kfb = kfrag[2],
				kfc = kfrag[3],
				kfd = kfrag[4],
				da = decoy_a,
				db = decoy_b,
				ret = ret,
				ra = ra,
				rb = rb,
				rc = rc,
				rd = rd,
				re = re,
				qc = qc,
				qr = qr,
			);
			fields.push(named(name.clone(), parse_expr(&src)));
			leaves.push((
				st,
				leaf(&name),
			));
		}
	}

	for _ in 0..filler {
		fillers = (0..5).map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d"])).collect();
		rng.shuffle(&mut fillers);
		let (ra, rb, rc, rd, re) = (fillers[0].as_str(), fillers[1].as_str(), fillers[2].as_str(), fillers[3].as_str(), fillers[4].as_str());
		let (st, ret) = step(&mut state_i);
		let name = nm.take();
		let decoy_a = states[rng.int(0, (states.len() / 2) as i64) as usize];
		let decoy_b = states[rng.int((states.len() / 2) as i64, (states.len() - 1) as i64) as usize];
		let qn = nm.take_avoid(&["b", "C"]);
		let src = format!(
			"function(b,C,{ra},{rb},{rc},{rd},{re}) local {qn}=C[{ss}]~=nil and {da} or {db}; \
			 return {ret},C,{rb},{rc},{rd},{re},{ra} end",
			ss = sumslot,
			da = decoy_a,
			db = decoy_b,
			ret = ret,
			ra = ra,
			rb = rb,
			rc = rc,
			rd = rd,
			re = re,
			qn = qn,
		);
		fields.push(named(name.clone(), parse_expr(&src)));
		leaves.push((
			st,
			leaf(&name),
		));
	}

	{
		fillers = (0..5).map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d"])).collect();
		rng.shuffle(&mut fillers);
		let (ra, rb, rc, rd, re) = (fillers[0].as_str(), fillers[1].as_str(), fillers[2].as_str(), fillers[3].as_str(), fillers[4].as_str());
		let (st, ret) = step(&mut state_i);
		let src = format!(
			"function(b,C,{ra},{rb},{rc},{rd},{re}) {interp} \
			 C[{vmslot}]={vm}; return {ret},C,{rc},{ra},{rd},{re},{rb} end",
			interp = interp_src,
			vmslot = vmslot,
			vm = vm_name,
			ret = ret,
			ra = ra,
			rb = rb,
			rc = rc,
			rd = rd,
			re = re,
		);
		fields.push(slotted(runner1, parse_expr(&src)));
		leaves.push((
			st,
			format!("{lhs6}=b[{}](b,C,{sv},{rhs5});continue;", runner1),
		));
	}

	{
		fillers = (0..5).map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d"])).collect();
		rng.shuffle(&mut fillers);
		let (ra, rb, rc, rd, re) = (fillers[0].as_str(), fillers[1].as_str(), fillers[2].as_str(), fillers[3].as_str(), fillers[4].as_str());
		let (st, ret) = step(&mut state_i);
		let cargs: Vec<String> = (0..n)
			.map(|k| {
				let base = k * CHUNKS + 1;
				(0..CHUNKS)
					.map(|j| format!("C[{}]", base + j))
					.collect::<Vec<_>>()
					.join("..")
			})
			.collect();
		let wrap_n: i64 = rng.int(4, 40);
		let vm_args = cargs.join(",");
		let src = format!(
			"function(b,C,{ra},{rb},{rc},{rd},{re}) \
			 local E=function(...) return C[{vmslot}]({vm_args}) end; \
			 C[{auxslot}],C[{entryslot}]={wrap_n},function(...) return b[{r2}](b,E,...) end; \
			 b.{ehandler}=C[{entryslot}]; \
			 return {ret},C,{ra},{rc},{rd},{re},{rb} end",
			vmslot = vmslot,
			vm_args = vm_args,
			auxslot = auxslot,
			entryslot = entryslot,
			wrap_n = wrap_n,
			r2 = runner2,
			ehandler = ehandler,
			ret = ret,
			ra = ra,
			rb = rb,
			rc = rc,
			rd = rd,
			re = re,
		);
		fields.push(named(ehandler.clone(), parse_expr(&src)));
		leaves.push((
			st,
			leaf(&ehandler),
		));
	}

	{
		fillers = (0..5).map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d"])).collect();
		rng.shuffle(&mut fillers);
		let (ra, rb, rc, rd, re) = (fillers[0].as_str(), fillers[1].as_str(), fillers[2].as_str(), fillers[3].as_str(), fillers[4].as_str());
		let (st, ret) = step(&mut state_i);
		let mut body = String::from("local s=0;");
		const V1H_FORBID: &[&str] = &["C", "E", "i", "Q", "d", "eo"];
		let mut accs: Vec<String> = Vec::new();
		for _k in 0..n {
			accs.push(nm.take_avoid(V1H_FORBID));
		}
		for k in 0..n {
			let base = k * CHUNKS + 1;
			let acc = &accs[k];
			body.push_str(&format!("local {acc}=0;"));
			for j in 0..CHUNKS {
				let c = base + j;
				match (k + j) % 3 {
					0 => body.push_str(&format!(
						"for i=1,#C[{c}] do {acc}={acc}+b[{sb}](C[{c}],i) end;",
						c = c,
						acc = acc,
						sb = prim_extra.0,
					)),
					1 => body.push_str(&format!(
						"local i=1;while i<=#C[{c}] do {acc}={acc}+b[{sb}](C[{c}],i);i=i+1 end;",
						c = c,
						acc = acc,
						sb = prim_extra.0,
					)),
					_ => body.push_str(&format!(
						"for i=#C[{c}],1,-1 do {acc}={acc}+b[{sb}](C[{c}],i) end;",
						c = c,
						acc = acc,
						sb = prim_extra.0,
					)),
				}
			}
			let decoy_a = states[rng.int(0, (states.len() / 2) as i64) as usize];
			let decoy_b = states[rng.int((states.len() / 2) as i64, (states.len() - 1) as i64) as usize];
			let qv = nm.take_avoid(&["b", "C", "s", "E", "i", "Q"]);
			body.push_str(&format!(
				"local {qv}=if s>=0 then {da} else {db};s=b:{mod}(b:{mul}(s,31)+{acc}+{qv}-{qv});",
				da = decoy_a,
				db = decoy_b,
				mod = modulo,
				mul = mul,
				acc = acc,
				qv = qv,
			));
		}
		let src = format!(
			"function(b,C,{ra},{rb},{rc},{rd},{re}) {body} C[{verifyslot}]=s; \
			 return {ret},C,{rb},{ra},{rc},{re},{rd} end",
			body = body,
			verifyslot = verifyslot,
			ret = ret,
			ra = ra,
			rb = rb,
			rc = rc,
			rd = rd,
			re = re,
		);
		fields.push(named(v1h.clone(), parse_expr(&src)));
		leaves.push((
			st,
			leaf(&v1h),
		));
	}
	{
let (st, _ret) = step(&mut state_i);
		let v2n: Vec<String> = (0..5).map(|_| nm.take_avoid(&["b", "C"])).collect();
		let src = format!(
			"function(b,C,{n0},{n1},{n2},{n3},{n4}) if C[{verifyslot}]~=C[{sumslot}] then C[{sumslot}]=C[{sumslot}]+7919 end; \
			 return {sdone},C,{n1},{n2},{n3},{n4},{n0} end",
			verifyslot = verifyslot,
			sumslot = sumslot,
			sdone = sdone,
			n0 = v2n[0],
			n1 = v2n[1],
			n2 = v2n[2],
			n3 = v2n[3],
			n4 = v2n[4],
		);
		fields.push(named(v2h.clone(), parse_expr(&src)));
		leaves.push((
			st,
			leaf(&v2h),
		));
	}

	fields.push(named(
		ctl.clone(),
		parse_expr(&format!(
			"function(b,C,{sv}) local h=if {sv}>={} then 2 else 1; \
			 if h==2 then return 2,C[{}] end; return 1 end",
			sdone, entryslot, sv = sv
		)),
	));

	{
		let wrapped: Vec<(i64, String)> = leaves
			.iter()
			.map(|(st, code)| (*st, opaque_wrap(code.clone(), rng, &sv)))
			.collect();
		let tree = dispatch_tree(&sv, &wrapped, 0, wrapped.len());
		let max_leaf = leaves.last().unwrap().0;
		let src = format!(
			"function(b,C,{lhs6}) while true do if {sv}<={} then {} \
			 else local h,E=b:{}(C,{sv}); if h==2 then return 2,E end; {sv}=h end end end",
			max_leaf, tree, ctl, lhs6 = lhs6, sv = sv
		);
		fields.push(named(loop_name.clone(), parse_expr(&src)));
	}

	{
		let mid = states[(nsteps as usize) / 2];
		let src = format!(
			"function(b,C,{lhs6}) local K={{[7]='{}'}}; \
			 while true do if {sv}<={} then \
			 return b[K[7]](b,C,{sv},{rhs5}) else return b:{}(C,{sv},{rhs5}) end end end",
			loop_name, mid, loop_name, lhs6 = lhs6, sv = sv, rhs5 = rhs5
		);
		fields.push(named(xl.clone(), parse_expr(&src)));
	}

	{
		let src = format!(
			"function(b,...) local u,z,Z,o,w,K,G,q,M,F,H,E=b:{init}(); \
			 local {lhs6},C=z,Z,o,w,K,G,{{}}; C[{ss}]=0; C[{ax}]=0; \
		 do local u=1; local w=u+1-1; local o=w-u; if o~=0 then C[{ss}]=o end end; \
			 local J={{[4]='{xl}',[7]='{xl}'}}; \
			 while u do local h2,B=b[J[4]](b,C,{sv},{rhs5}); \
			if h2==2 then return B end end end",
			init = init,
			ss = sumslot,
			ax = auxslot,
			xl = xl,
			lhs6 = lhs6,
			sv = sv,
			rhs5 = rhs5,
		);
		fields.push(named(fc.clone(), parse_expr(&src)));
	}

	fields.push(slotted(
		runner2,
		parse_expr("function(b,E,...) return E(...) end"),
	));

	{
		let mk_arr = |vals: &Vec<i64>| Expr::Table {
			fields: vals
				.iter()
				.map(|&w| TableField::Array(Expr::Num { value: w as f64, isfloat: false }))
				.collect(),
		};
		fields.push(slotted(st_slots[0], mk_arr(&st1)));
		fields.push(slotted(st_slots[1], mk_arr(&st2)));
	}

	(fields, fc)
}
