
use crate::ast::{self, Expr, TableField};
use crate::parser;
use crate::rng::Rng;
use crate::vmgen::caps::ScaffoldPrim;
use crate::vmgen::isa::CARRIER_SPECIALS;

fn dot_value(path: &str) -> Expr {
	// invariant: path comes from the static v15 template table and always contains a dot
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

fn parse_expr(src: &str) -> Expr {
	let chunk = format!("local _ = {}", src);
	let mut block = match parser::parse(&chunk, true) {
		Ok(b) => b,
		Err(e) => {
			eprintln!("v15 emitter bug: {:?}\nsource:\n{}", e, chunk);
			// invariant: the v15 template sources are static strings that parse by construction
			panic!("v15 static source")
		}
	};
	// invariant: the v15 template source always contains at least one statement
	match block.stmts.pop().unwrap() {
		// invariant: the v15 template source ends with a single local declaration carrying one value
		ast::Stmt::Local { mut values, .. } => values.pop().unwrap().unwrap(),
		// invariant: the v15 template source ends with a local declaration by construction
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
	cap_slots: &[i64; 6],
	bind_env: Option<&str>,
	env_slots_in: Option<&[i64; 4]>,
) -> (Vec<TableField>, Option<[i64; 4]>) {
	let top = std::cmp::max(126, slot_max);
	let mut slots: Vec<i64> = (1..=top)
		.filter(|n| !exclude.contains(n) && !cap_slots.contains(n))
		.collect();
	rng.shuffle(&mut slots);
	let mut it = slots.into_iter();
	let mut fields: Vec<TableField> = Vec::new();

	// Capability-scoped scaffold bindings: emit ONLY the 6 primitives actually
	// consumed by the v15 chunk stream-cipher unmasker and 32-bit multiplier.
	for (prim, &slot) in ScaffoldPrim::ALL.iter().zip(cap_slots.iter()) {
		let path = prim.lua_expr();
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
				value: slot as f64,
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
		// invariant: the slot-name iterator is sized to the number of static slots consumed here
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
		// invariant: the slot-name iterator is sized to the number of static slots consumed here
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

#[allow(dead_code)]
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

/// Wrap a dispatch leaf in a genuine runtime state invariant (or leave it
/// unwrapped when binary-tree dispatch bounds already isolate `st`).
/// Never emits algebraically constant identities (`x^2 >= 0`, `(x*x+x)%2 == 0`, etc.).
fn state_guard_wrap(
	st: i64,
	code: String,
	rng: &mut crate::rng::Rng,
	vname: &str,
	sumslot: usize,
	auxslot: usize,
	sdone: i64,
) -> String {
	if rng.int(0, 9) < 5 {
		return code;
	}
	const MODS: [i64; 6] = [7, 11, 13, 17, 19, 23];
	let m = MODS[rng.int(0, 5) as usize];
	let k = rng.int(2, m - 1);
	let c = rng.int(1, 89);
	let rem = (st * k + c).rem_euclid(m);
	match rng.int(0, 3) {
		0 => format!(
			"if ({v}*{k}+{c})%{m}=={rem} then {code} else {v}={sdone} end",
			v = vname,
		),
		1 => format!(
			"if C[{auxslot}]+{v}>={st} then {code} else {v}={sdone} end",
			v = vname,
		),
		2 => {
			let wrong = (rem + rng.int(1, m - 1)).rem_euclid(m);
			format!(
				"if ({v}*{k}+{c})%{m}~={wrong} then {code} else C[{auxslot}]={v};{v}={sdone} end",
				v = vname,
			)
		}
		_ => {
			let wrong_v = st * k + rng.int(1, 29);
			format!(
				"if C[{sumslot}]>=0 and {v}*{k}~={wrong_v} then {code} else {v}={sdone} end",
				v = vname,
			)
		}
	}
}

/// Generate a polymorphic outer-machine staging step with varied arity (`5..=9`),
/// genuine rolling-register updates, and randomized return-tuple permutation.
#[allow(clippy::too_many_arguments)]
fn gen_filler_staging(
	fi: usize,
	ret: i64,
	nm: &mut Names,
	rng: &mut Rng,
	sumslot: usize,
	auxslot: usize,
	cap_slots: &[i64; 6],
	r_slot: i64,
	decoy_a: i64,
	decoy_b: i64,
) -> String {
	// Keep ~70% of staging functions at arity >= 7 (7, 8, or 9 formal params)
	// while ~30% use 5 or 6 formal params for genuine wrapper signature diversity.
	let extra_regs: usize = match fi % 10 {
		0 => 3, // total 5 params: (b, C, r0, r1, r2)
		1 | 2 => 4, // total 6 params
		3..=6 => 5, // total 7 params
		7 | 8 => 6, // total 8 params
		_ => 7, // total 9 params
	};
	let mut regs: Vec<String> = (0..extra_regs)
		.map(|_| nm.take_avoid(&["b", "C", "Q", "R", "d", "i"]))
		.collect();
	rng.shuffle(&mut regs);
	let params_csv = regs.join(",");

	// Note on outer-machine calling convention:
	// `lhs6` = `sv, s1, s2, s3, s4, s5` and steps return `ret, C, a0, a1, a2, a3, a4`,
	// so after step 1 `s1` (`regs[0]`) holds `C` while `s2..s5` (`regs[1..=4]`) hold
	// rolling integers. Always operate on `regs[1]`/`regs[2]` and emit integers in `ret_regs`.
	let r0 = &regs[1];
	let r1 = &regs[2];
	let r2 = if regs.len() > 3 { &regs[3] } else { &regs[1] };
	let int_pool: Vec<String> = regs[1..]
		.iter()
		.take(4)
		.cloned()
		.collect();
	let mut ret_regs: Vec<String> = (0..5)
		.map(|i| int_pool[i % int_pool.len()].clone())
		.collect();
	rng.shuffle(&mut ret_regs);

	let k1 = rng.int(3, 251);
	let k2 = rng.int(5, 509);
	let m1 = [4093i64, 8191, 16381, 32749, 65521][rng.int(0, 4) as usize];
	let tmp = nm.take_avoid(&["b", "C", "i"]);

	let body = match fi % 10 {
		0 => {
			ret_regs[0] = tmp.clone();
			format!(
				"local {tmp}=(({r0}+{r1})%{m1}<{thr})and {da} or {db};{tmp}=({tmp}+{r2}*{k1})%65536;",
				thr = m1 / 2,
				da = decoy_a,
				db = decoy_b,
			)
		}
		1 => {
			format!(
				"if ({r0}+{k1})%7<{thr} then {r0}+={k2} else {r1}=({r1}+{r2}+{k1})%{m1} end;",
				thr = rng.int(2, 5),
			)
		}
		2 => {
			ret_regs[1] = tmp.clone();
			format!(
				"local {tmp}=if {r0}>{r1} then ({r0}-{r1}+{k1})%{m1} else ({r1}-{r0}+{k2})%{m1};",
			)
		}
		3 => {
			let steps = rng.int(2, 4);
			ret_regs[0] = tmp.clone();
			format!(
				"local {tmp}={r0}%4096;for i=1,{steps} do {tmp}=({tmp}*{k1}+{r1}+i)%{m1} end;",
			)
		}
		4 => {
			let lim = rng.int(3, 9);
			let stp = rng.int(2, 5);
			ret_regs[2] = tmp.clone();
			format!(
				"local {tmp}={r0}%16+{lim};while {tmp}>{lim} do {tmp}-={stp};{r1}=({r1}+{tmp})%65536 end;",
			)
		}
		5 => {
			let dsel = nm.take_avoid(&["b", "C", "i", &tmp]);
			ret_regs[0] = tmp.clone();
			format!(
				"local {dsel}=(C[{sumslot}]~={r0})and {da} or {db};local {tmp}=((b[{r_slot}][1] or 0)%65536+{r1}+{dsel})%{m1};",
				da = decoy_a,
				db = decoy_b,
			)
		}
		6 => {
			let r_mod = rng.int(5, 11);
			format!(
				"if {r0}%{r_mod}==0 then {r0}=({r0}+{k1})%65536 elseif {r1}%3==1 then {r1}+={k2} else {r2}=({r2}+{r0})%{m1} end;",
			)
		}
		7 => {
			format!(
				"do local {tmp}=(({r0}~={r1})and {da} or {db});{r0}={r1};{r1}=({tmp}+{r2}*{k1})%{m1} end;",
				da = decoy_a,
				db = decoy_b,
			)
		}
		8 => {
			let bxor_slot = cap_slots[1];
			let dsel = nm.take_avoid(&["b", "C", "i", &tmp]);
			ret_regs[0] = tmp.clone();
			format!(
				"local {dsel}=(C[{auxslot}]==0)and {da} or {db};local {tmp}=b[{bxor_slot}]({r0}%65536,({r1}+{dsel})%65536);",
				da = decoy_a,
				db = decoy_b,
			)
		}
		_ => {
			ret_regs[0] = format!("({}+{})%{}", r1, k1, m1);
			ret_regs[3] = format!("({}+{})%65536", r0, k2);
			String::new()
		}
	};

	format!(
		"function(b,C,{params}) {body}return {ret},C,{rets} end",
		params = params_csv,
		body = body,
		ret = ret,
		rets = ret_regs.join(","),
	)
}

// 惰性局部（V-06/V-07 修复）：所有形态都探测一个运行期槽（C[slot]），
// 因此形态各异但都不可静态常量折叠；值本身不被任何后续代码使用。
fn idle_val(rng: &mut crate::rng::Rng, probe: &str, a: i64, b: i64) -> String {
	match rng.int(0, 4) {
		0 => format!("({probe}~=nil)and {a} or {b}"),
		1 => format!("({b})+(if {probe}==nil then 0 else {a}-{b})"),
		2 => format!("(({probe}~=nil)and {a} or {b})"),
		_ => format!("({b})+(({probe}~=nil)and({a}-{b})or 0)"),
	}
}

fn idle_local(rng: &mut crate::rng::Rng, name: &str, probe: &str, a: i64, b: i64) -> String {
	match rng.int(0, 3) {
		0 => format!("local {name}={};", idle_val(rng, probe, a, b)),
		1 => format!("local {name};if {probe}==nil then {name}={b} else {name}={a} end;"),
		2 => format!("local {name}={b};if {probe}~=nil then {name}={a} end;"),
		_ => {
			let tmp = ["_t", "_u", "_v"]
				.into_iter()
				.find(|&cand| cand != name && cand != probe)
				.unwrap_or("_w");
			format!("local {name};do local {tmp}={probe};{name}=({tmp}==nil)and {b} or {a} end;")
		}
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
	// invariant: the loop above pushes exactly six names
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
	let p1 = f1.pop().unwrap_or_else(|| crate::limits::fatal("input exceeds compiler limit: st table exhaustion"));
	let p2 = f2.pop().unwrap_or_else(|| crate::limits::fatal("input exceeds compiler limit: st table exhaustion"));
	st1[p1] = e1;
	st2[p2] = e2;
	format!("(b[{}][{}]+b[{}][{}])", s0, p1 + 1, s1slot, p2 + 1)
}

#[allow(clippy::too_many_arguments)]
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
	cap_slots: &[i64; 6],
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
	let mut fillers: Vec<String>;
	let mut state_i = 0usize;
	let step = |i: &mut usize| -> (i64, i64) {
		let d = states[*i];
		let r = states[*i + 1];
		*i += 1;
		(d, r)
	};

	let init_regs: [i64; 5] = [
		rng.int(17, 16381),
		rng.int(19, 16381),
		rng.int(23, 16381),
		rng.int(29, 16381),
		rng.int(31, 16381),
	];
	fields.push(named(
		init.clone(),
		parse_expr(&format!(
			"function(b,...) return true,{},{},{},{},{},{},nil,nil,nil,nil,nil end",
			states[0], init_regs[0], init_regs[1], init_regs[2], init_regs[3], init_regs[4]
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

	let (s_band, s_bxor, s_extract, s_lshift, s_rshift, s_tcat) = (
		cap_slots[0],
		cap_slots[1],
		cap_slots[2],
		cap_slots[3],
		cap_slots[4],
		cap_slots[5],
	);
	fields.push(named(
		mul.clone(),
		parse_expr(&format!(
			"function(b,u,z) local Z=b[{s_band}]; u,z=Z(u,4294967295),Z(z,4294967295); local o,w=Z(u,65535),b[{s_rshift}]; local K,G,q=w(u,16),Z(z,65535),w(z,16); return Z(o*G+b[{s_lshift}](Z(o*q+K*G,65535),16),4294967295)%4294967296 end",
		)),
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
			// V19 R5-b：非线性折叠（镜像运行时文本 _ax 同式混洗后四字节求和）。
			let kf = crate::vmgen::isa::fold_key_nl(ks_const as u64) as i64;
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
			let qf_src = idle_local(rng, &qf, &format!("C[{sumslot}]"), decoy_a, decoy_b);
			let src = format!(
				"function(b,C,{ra},{rb},{rc},{rd},{re}) local s=C[{sumslot}]; \
				{qf_src} \
				 C[{sumslot}]=b:{modulo}(b:{mul}(s,31)+{ksum}); \
				 return {ret},C,{rb},{rc},{rd},{re},{ra} end",
				sumslot = sumslot,
				modulo = modulo,
				mul = mul,
				ksum = ksum_e,
				ret = ret,
				ra = ra,
				rb = rb,
				rc = rc,
				rd = rd,
				re = re,
				qf_src = qf_src,
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
			// V19 R5-b：kn 前的非线性混洗临时量（镜像 fold_key_nl）。
			let nax = nm.take_avoid(CH_AVOID);
			let byte_emit = if divmod_bytes {
				let mut s = String::new();
				for step in 0..4u16 {
					s.push_str(&format!(
						"{t}[{ti}]=b[{sc}](b[{s_bxor}]({wd}%256,({kn}+{ti})%256));",
						t = nt, ti = nti, wd = nwd, kn = nkn, sc = prim_extra.1, s_bxor = s_bxor
					));
					if step < 3 {
						s.push_str(&format!("{}=({}-{}%256)/256;", nwd, nwd, nwd));
					}
					s.push_str(&format!("{}={}+1;", nti, nti));
				}
				s
			} else {
				format!(
					"{t}[#{t}+1]=b[{sc}](b[{s_bxor}]({wd}%256,({kn}+{ti})%256),b[{s_bxor}](b[{s_ext}]({wd},8,8),({kn}+{ti}+1)%256),b[{s_bxor}](b[{s_ext}]({wd},16,8),({kn}+{ti}+2)%256),b[{s_bxor}](b[{s_ext}]({wd},24,8),({kn}+{ti}+3)%256));{ti}={ti}+4;",
					t = nt, ti = nti, wd = nwd, kn = nkn, sc = prim_extra.1, s_bxor = s_bxor, s_ext = s_extract
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
					"b[{s_tc}]({t},nil,1,{blen})",
					s_tc = s_tcat, t = nt, blen = blen
				)
			} else if trim_style {
				format!(
					"(\"\")[\"sub\"](b[{s_tc}]({t},nil,1,{nw}),1,{blen})",
					s_tc = s_tcat, t = nt, nw = nw, blen = blen
				)
			} else {
				format!(
					"(\"\")[\"sub\"](b[{s_tc}]({t}),1,{blen})",
					s_tc = s_tcat, t = nt, blen = blen
				)
			};
			let src = format!(
				"function(b,C,{ra},{rb},{rc},{rd},{re}) \
				 local {km}=(b[{kfa}]+b[{kfb}])%4294967296;local {kc}=(b[{kfc}]-b[{kfd}])%4294967296; \
				 local {t}={{}}; local {g}={sw}; \
			local {kv}=b[{kg}](b); local {ax}=(({kv})+(({kv})%16384)*((({kv})-(({kv})%16384))/16384))%268435456; {ax}=(({ax})*257+((({ax})-(({ax})%65536))/65536))%268435456; local {kn}=((({ax})%256)+((({ax})-(({ax})%256))/256)%256+((({ax})-(({ax})%65536))/65536)%256+((({ax})-(({ax})%16777216))/16777216))%256; \
				 local {ti}=1; \
				 {segs}\
				 {qc_src} {qr_src} \
				 C[{idx}]={final_concat}; \
				 return {ret},C,{rc},{rb},{rd},{re},{ra} end",
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
				ax = nax,
				ti = nti,
				kg = kg_slot,
				kfa = kfrag[1],
				kfb = kfrag[2],
				kfc = kfrag[3],
				kfd = kfrag[4],
				ret = ret,
				ra = ra,
				rb = rb,
				rc = rc,
				rd = rd,
				re = re,
				qc_src = idle_local(rng, &qc, &format!("C[{}]", k * CHUNKS + j + 1), decoy_a, decoy_b),
				qr_src = idle_local(rng, &qr, &format!("C[{}]", k * CHUNKS + j + 1), decoy_b, decoy_a),
			);
			fields.push(named(name.clone(), parse_expr(&src)));
			leaves.push((
				st,
				leaf(&name),
			));
		}
	}

	for fi in 0..filler {
		let (st, ret) = step(&mut state_i);
		let name = nm.take();
		let decoy_a = states[rng.int(0, (states.len() / 2) as i64) as usize];
		let decoy_b = states[rng.int((states.len() / 2) as i64, (states.len() - 1) as i64) as usize];
		let src = gen_filler_staging(
			fi,
			ret,
			&mut nm,
			rng,
			sumslot,
			auxslot,
			cap_slots,
			st_slots[0],
			decoy_a,
			decoy_b,
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
			 C[{vmslot}]={vm}; return {ret},C,{rc},{rb},{rd},{re},{ra} end",
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
			 return {ret},C,{rb},{rc},{rd},{re},{ra} end",
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
				"{qv_src}s=b:{mod}(b:{mul}(s,31)+{acc}+{qv}-{qv});",
				mod = modulo,
				mul = mul,
				acc = acc,
				qv = qv,
				qv_src = idle_local(rng, &qv, "s", decoy_a, decoy_b),
			));
		}
		let src = format!(
			"function(b,C,{ra},{rb},{rc},{rd},{re}) {body} C[{verifyslot}]=s; \
			 return {ret},C,{rb},{rc},{re},{rd},{ra} end",
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
			"function(b,C,{n0},{n1},{n2},{n3},{n4}) if C[{verifyslot}]~=C[{sumslot}] then C[{sumslot}]=C[{sumslot}]+7919; C[{entryslot}]=nil; C[{vmslot}]=nil end; \
			 return {sdone},C,{n1},{n2},{n3},{n4},{n0} end",
			verifyslot = verifyslot,
			sumslot = sumslot,
			entryslot = entryslot,
			vmslot = vmslot,
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
			.map(|(st, code)| {
				(
					*st,
					state_guard_wrap(*st, code.clone(), rng, &sv, sumslot, auxslot, sdone),
				)
			})
			.collect();
		let tree = dispatch_tree(&sv, &wrapped, 0, wrapped.len());
		// invariant: dispatch_tree always emits at least one leaf
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

#[cfg(test)]
mod tests {
	use super::idle_local;

	#[test]
	fn idle_local_never_shadows_target_when_name_is_underscore_t() {
		for seed in 1u64..=128 {
			let mut rng = crate::rng::Rng::new(seed);
			let src = idle_local(&mut rng, "_t", "s", 83, 184);
			assert!(
				!src.contains("local _t;do local _t="),
				"seed {seed} shadowed target local _t: {src}"
			);
		}
	}
}
