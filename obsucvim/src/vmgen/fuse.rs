//! V17 数据流驱动的函数级超指令融合——编译期数据层（纯函数，零 rng）。
//!
//! 设计约束（动工前写死）：
//! 1. 确定性：同输入永远同输出（BTree 序 + 显式 tie-break），不消耗 rng——
//!    off/on 产物字节恒等由结构保证（本模块从不改动输入流，发射端在 P1-b 接线）。
//! 2. 副作用安全：白名单只收「纯数据搬运」相邻对（无 metamethod/无调用/无 pc 写），
//!    且两指令合计逻辑通道 ≤ 4（fused 记录通道布局 [X.a, X.b, Y.a, Y.b]）。
//! 3. 跳入禁忌：对 (i, i+1) 可融 ⟺ (i+2) ∉ leaders（1-based），否则该对跳过；
//!    塌缩后所有分支目标经 old→new 映射表重映射。
//!
//! pc 语义（compiler.rs 实证）：b 槽 = 1-based 目标记录索引
//! （finish:540 `target = base + 1`；finish:764 leaders 收集 ins.b 与 fall-through
//! idx+2，同为 1-based）。

use super::isa::{op_index, Instr, Op};
use std::collections::{BTreeMap, BTreeSet};

/// 起步白名单（文档 V17 §5 定稿前的保守集；扩名单需重走副作用安全论证）。
/// 全部为 2 通道 + 2 通道组合，融合体恰好占满 4 逻辑通道。
pub const FUSE_WHITELIST: &[(Op, Op)] = &[
	(Op::LoadK, Op::Move),
	(Op::Move, Op::Move),
	(Op::LoadNil, Op::Move),
	(Op::GetUp, Op::Move),
	(Op::Move, Op::LoadK),
];

/// 每函数融合词汇的候选上限（OpMap 每方言区间 64 码位、基础 op 43，
/// 空闲 21；K 宁小勿大，小函数占比高时词汇表噪声比失控）。
pub const FUSE_TOP_K: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FKind {
	/// 原 ISA op（值为 op_index）
	Base(u8),
	/// 融合对（值为该函数词汇表内序号）
	Fused(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FRec {
	pub kind: FKind,
	pub a: u16,
	pub b: u16,
	pub c: u16,
	pub d: u16,
}

/// 统计：白名单相邻对的频次（确定性 BTree 序输出，key = (op_index(a), op_index(b))）。
pub fn stats(code: &[Instr]) -> Vec<((u8, u8), u32)> {
	let wl: BTreeSet<(u8, u8)> = FUSE_WHITELIST
		.iter()
		.map(|&(x, y)| (op_index(x) as u8, op_index(y) as u8))
		.collect();
	let mut m: BTreeMap<(u8, u8), u32> = BTreeMap::new();
	for w in code.windows(2) {
		let key = (op_index(w[0].op) as u8, op_index(w[1].op) as u8);
		if wl.contains(&key) {
			*m.entry(key).or_insert(0) += 1;
		}
	}
	m.into_iter().collect()
}

/// 选择：频次降序，tie 用 (op1, op2) 升序（确定性）；取 top-K。
pub fn select_vocab(stats: &[((u8, u8), u32)], k: usize) -> Vec<(u8, u8)> {
	let mut v: Vec<((u8, u8), u32)> = stats.to_vec();
	v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
	v.into_iter()
		.take(k)
		.map(|(pair, _)| pair)
		.collect()
}

/// 塌缩 + 分支目标重映射。
/// 返回 (新记录流, old(1-based) → new(1-based) 位置映射表)。
/// 跳入禁忌：pair 第二条（1-based 位置 i+2）在 leaders 中 → 不融，两条原样保留。
/// 断言级防御：若某分支目标仍指向被吞掉的记录 → 该对回退不融（不变量已由禁忌保证，
/// 这里只是 belt-and-braces，防未来 leaders 收集口径变化）。
pub fn collapse(
	code: &[Instr],
	vocab: &[(u8, u8)],
	leaders: &BTreeSet<u16>,
) -> (Vec<FRec>, Vec<u16>) {
	let n = code.len();
	// vocabulary 查询表（op_index,op_index) → fused index
	let lut: BTreeMap<(u8, u8), u8> = vocab
		.iter()
		.enumerate()
		.map(|(i, &p)| (p, i as u8))
		.collect();
	let mut old_to_new: Vec<u16> = vec![0; n + 1]; // old_to_new[i] = 1-based 旧位置 i 的新位置
	let mut out: Vec<FRec> = Vec::with_capacity(n);
	let mut i = 0usize; // 0-based
	while i < n {
		let pos1 = (i + 1) as u16; // 本指令 1-based
		let pair_key = if i + 1 < n {
			Some((op_index(code[i].op) as u8, op_index(code[i + 1].op) as u8))
		} else {
			None
		};
		let second_is_leader = leaders.contains(&((i + 2) as u16));
		if let Some(&fk) = pair_key.and_then(|k| lut.get(&k)) {
			if !second_is_leader {
				old_to_new[i] = (out.len() + 1) as u16;
				old_to_new[i + 1] = 0; // 被吞
				out.push(FRec {
					kind: FKind::Fused(fk),
					a: code[i].a,
					b: code[i].b,
					c: code[i + 1].a,
					d: code[i + 1].b,
				});
				i += 2;
				continue;
			}
		}
		old_to_new[i] = (out.len() + 1) as u16;
		let ins = code[i];
		out.push(FRec {
			kind: FKind::Base(op_index(ins.op) as u8),
			a: ins.a,
			b: ins.b,
			c: ins.c,
			d: ins.d,
		});
		i += 1;
	}
	// 分支目标重映射
	let mut remapped = out.clone();
	for rec in remapped.iter_mut() {
		if rec.kind == FKind::Base(op_index(Op::Jmp) as u8)
			|| rec.kind == FKind::Base(op_index(Op::Jf) as u8)
			|| rec.kind == FKind::Base(op_index(Op::Jt) as u8)
		{
			let tgt = rec.b as usize;
			if tgt >= 1 && tgt <= n {
				let nt = old_to_new[tgt - 1];
				debug_assert!(nt != 0, "branch target fell onto a swallowed record");
				rec.b = nt;
			}
		}
	}
	(remapped, old_to_new)
}

/// 演练门面：统计 → 选择 → 塌缩，全部只读；返回 (词汇表, 塌缩后记录数)。
/// P1-a 阶段发射端未接线（on==off 字节恒等由「本函数不改动输入」保证）；
/// P1-b 起发射端直接消费 collapse 的结果。
pub fn plan(code: &[Instr], leaders: &[u16]) -> (Vec<(u8, u8)>, usize) {
	let st = stats(code);
	let vocab = select_vocab(&st, FUSE_TOP_K);
	let leaders_set: BTreeSet<u16> = leaders.iter().cloned().collect();
	let (fused, _) = collapse(code, &vocab, &leaders_set);
	debug_assert!(!fused.is_empty() || code.is_empty());
	(vocab, fused.len())
}

#[cfg(test)]
mod fuse_tests {
	use super::*;
	use crate::vmgen::isa::Op;

	fn ins(op: Op) -> Instr {
		Instr::abcd(op, 1, 2, 3, 4)
	}

	/// 构造：LoadK, Move, LoadK, Move, LoadK, Move
	fn tri_pairs() -> Vec<Instr> {
		vec![
			ins(Op::LoadK),
			ins(Op::Move),
			ins(Op::LoadK),
			ins(Op::Move),
			ins(Op::LoadK),
			ins(Op::Move),
		]
	}

	#[test]
	fn stats_counts_whitelisted_pairs_only() {
		let code = vec![
			ins(Op::LoadK),
			ins(Op::Move),   // (LoadK,Move) #1
			ins(Op::Add),    // (Move,Add) 非白名单
			ins(Op::Nop),    // (Add,Nop) 非白名单
			ins(Op::LoadK),  // (Nop,LoadK) 非白名单
			ins(Op::Move),   // (LoadK,Move) #2
		];
		let st = stats(&code);
		assert_eq!(st, vec![((op_index(Op::LoadK) as u8, op_index(Op::Move) as u8), 2)]);
	}

	#[test]
	fn select_vocab_is_deterministic_and_tie_breaks_by_pair() {
		let a = (op_index(Op::Move) as u8, op_index(Op::Move) as u8);
		let b = (op_index(Op::LoadK) as u8, op_index(Op::Move) as u8);
		let st = vec![(a, 5), (b, 5), ((op_index(Op::GetUp) as u8, op_index(Op::Move) as u8), 9)];
		let v1 = select_vocab(&st, 4);
		let v2 = select_vocab(&st, 4);
		assert_eq!(v1, v2);
		// (GetUp,Move)=9 居首；同频两对按 key 升序
		assert_eq!(v1[0], (op_index(Op::GetUp) as u8, op_index(Op::Move) as u8));
		assert_eq!(v1.len(), 3);
	}

	#[test]
	fn collapse_fuses_non_leader_pairs_and_halves_stream() {
		let code = tri_pairs();
		let st = stats(&code);
		let vocab = select_vocab(&st, 4);
		let leaders: BTreeSet<u16> = [1u16].into_iter().collect();
		let (out, _map) = collapse(&code, &vocab, &leaders);
		assert_eq!(out.len(), 3);
		for r in &out {
			assert!(matches!(r.kind, FKind::Fused(0)));
			assert_eq!((r.a, r.b, r.c, r.d), (1, 2, 1, 2));
		}
	}

	#[test]
	fn collapse_never_fuses_into_a_jump_target() {
		let mut code = tri_pairs();
		// 使位置 4（0-based 3，第二对 Move）成为跳入边界：Jf → 目标 4
		code.push(Instr::ab(Op::Jf, 9, 4));
		let st = stats(&code);
		let vocab = select_vocab(&st, 4);
		let leaders: BTreeSet<u16> = [1u16, 4, 5].into_iter().collect();
		let (out, _map) = collapse(&code, &vocab, &leaders);
		// pairs @1-2 融；@3-4 因第二条（1-based 4）是 leader 不融；@5-6 融；Jf 保持 Base
		let fused_count = out
			.iter()
			.filter(|r| matches!(r.kind, FKind::Fused(_)))
			.count();
		assert_eq!(fused_count, 2);
		assert!(matches!(
			out.last().unwrap().kind,
			FKind::Base(k) if k == op_index(Op::Jf) as u8
		));
	}

	#[test]
	fn collapse_remaps_branch_targets() {
		let mut code = tri_pairs();
		code.push(Instr::abcd(Op::LoadK, 7, 7, 0, 0)); // 位置 7
		code.push(Instr::ab(Op::Jmp, 0, 7)); // 跳到旧 7
		let st = stats(&code);
		let vocab = select_vocab(&st, 4);
		let leaders: BTreeSet<u16> = [1u16, 7, 9].into_iter().collect();
		let (out, _map) = collapse(&code, &vocab, &leaders);
		// 旧流: LK,M LK,M LK,M LoadK(7) Jmp(8) → 新流: F F F LoadK(4) Jmp(5)
		let jmp = out.last().unwrap();
		assert_eq!(jmp.b, 4, "branch to collapsed-adjacent target must remap");
	}

	#[test]
	fn collapse_is_deterministic() {
		let code = tri_pairs();
		let st = stats(&code);
		let vocab = select_vocab(&st, 4);
		let leaders: BTreeSet<u16> = [1u16].into_iter().collect();
		let (o1, _) = collapse(&code, &vocab, &leaders);
		let (o2, _) = collapse(&code, &vocab, &leaders);
		assert_eq!(o1, o2);
	}
}
