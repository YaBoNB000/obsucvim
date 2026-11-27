//! Phase E (obf.lua tech-extraction spec): per-function dispatch-tree
//! generation for the non-v15 VM dispatcher, moved out of `template.rs`.
//!
//! The generator consumes the function's semantic-opcode -> wire-code items
//! (from the existing `OpMap`; no second opcode enum is introduced) and
//! emits a randomized comparison tree: split points are drawn from code
//! gaps or interior codes, comparison direction varies, and deep trees
//! degrade to a shuffled equality chain. `verify_items` is the generation
//! gate: duplicate names, duplicate wire codes, or an empty item set are
//! rejected before any tree is emitted, so every valid opcode reaches
//! exactly one handler leaf by construction.
//!
//! Runtime note: wire codes outside the emitted set cannot be produced by
//! the compiler (the code section only contains mapped codes, integrity
//! sums cover the operand streams); an unmatched code falls through the
//! tree without executing a handler, as before this module existed. A
//! runtime invalid-opcode trap would need a reverse code table in every
//! artifact and is intentionally not added here.

use crate::rng::Rng;

/// Validate dispatch items before tree generation: non-empty, unique
/// handler names, unique wire codes.
pub fn verify_items(items: &[(String, u8)]) -> Result<(), String> {
	if items.is_empty() {
		return Err("dispatch tree: no opcode items".to_string());
	}
	let mut names: Vec<&str> = items.iter().map(|(n, _)| n.as_str()).collect();
	names.sort_unstable();
	for w in names.windows(2) {
		if w[0] == w[1] {
			return Err(format!("dispatch tree: duplicate handler name {}", w[0]));
		}
	}
	let mut codes: Vec<u8> = items.iter().map(|(_, c)| *c).collect();
	codes.sort_unstable();
	for w in codes.windows(2) {
		if w[0] == w[1] {
			return Err(format!("dispatch tree: duplicate wire code {}", w[0]));
		}
	}
	Ok(())
}

pub fn gen_dispatch_tree(
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


#[cfg(test)]
mod tests {
	use super::*;

	fn items_for(codes: &[u8]) -> Vec<(String, u8)> {
		codes
			.iter()
			.enumerate()
			.map(|(i, &c)| (format!("Op{i}"), c))
			.collect()
	}

	#[test]
	fn verify_rejects_duplicates_and_empty() {
		assert!(verify_items(&[]).is_err());
		assert!(verify_items(&[("A".to_string(), 1), ("A".to_string(), 2)]).is_err());
		assert!(verify_items(&[("A".to_string(), 1), ("B".to_string(), 1)]).is_err());
		assert!(verify_items(&items_for(&[3, 1, 2])).is_ok());
	}

	#[test]
	fn every_opcode_reaches_exactly_one_leaf() {
		// All 44 semantic opcodes under a randomized wire mapping.
		let mut rng = Rng::new(42);
		let map = crate::vmgen::isa::OpMap::new(&mut rng);
		let names = [
			"Jmp", "Jf", "Jt", "LoadNil", "LoadK", "Move", "Add", "Sub", "Mul", "Div",
			"Mod", "Pow", "Concat", "Unm", "Not", "Len", "Lt", "Le", "Gt", "Ge", "Eq",
			"Ne", "Idiv", "NewTab", "GetTab", "SetTab", "TabN", "CallT", "Closure",
			"Call", "VarArgTab", "VarArgC", "VarArgTabN", "GetGlobal", "SetGlobal",
			"GetUp", "SetUp", "Return", "Nop", "CallE", "CallM", "MkStr", "Canary",
			"LoadStr",
		];
		let items: Vec<(String, u8)> = names
			.iter()
			.enumerate()
			.map(|(i, n)| (n.to_string(), map.to_wire[0][i]))
			.collect();
		verify_items(&items).expect("opmap items must verify");
		let mut body_of = |name: &str| format!("-- handler {name}");
		let tree = gen_dispatch_tree(&items, &mut body_of, &mut rng, 0);
		for name in names {
			let needle = format!("OC.{name} then");
			assert_eq!(tree.matches(&needle).count(), 1, "exactly one leaf for {name}");
		}
	}

	#[test]
	fn tree_shape_varies_with_rng() {
		let items = items_for(&(0..20).collect::<Vec<u8>>());
		let mut body_of = |name: &str| format!("-- {name}");
		let mut r1 = Rng::new(1);
		let mut r2 = Rng::new(2);
		let t1 = gen_dispatch_tree(&items, &mut body_of, &mut r1, 0);
		let t2 = gen_dispatch_tree(&items, &mut body_of, &mut r2, 0);
		assert_ne!(t1, t2, "different seeds should shape the tree differently");
	}
}
