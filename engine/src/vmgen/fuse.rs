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
	/// 原 ISA op
	Base(Op),
	/// 融合对（值为该函数词汇表内序号）
	Fused(u8),
}

/// tail-less 融合件 handler 的 op 序（与 isa::OpMap::fused_tail_code 的 j 槽一致）：
/// 白名单对出现的全部 op（首 ∈ {LoadK,Move,LoadNil,GetUp}，次 ∈ {Move,LoadK}）。
pub const FUSE_TAIL_OPS: [Op; 4] = [Op::LoadK, Op::Move, Op::LoadNil, Op::GetUp];

/// tail-less 序号；白名单外 op 不应出现（返回 None 即防御性拒融的来源）。
pub fn tail_index(op: Op) -> Option<u8> {
	FUSE_TAIL_OPS.iter().position(|&o| o == op).map(|p| p as u8)
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
/// 同时强制校验 `OpMeta::fuse_head_ok` / `fuse_tail_ok` 语义边界。
pub fn stats(code: &[Instr]) -> Vec<((u8, u8), u32)> {
	let wl: BTreeSet<(u8, u8)> = FUSE_WHITELIST
		.iter()
		.filter(|&&(x, y)| x.meta().fuse_head_ok && y.meta().fuse_tail_ok)
		.map(|&(x, y)| (op_index(x) as u8, op_index(y) as u8))
		.collect();
	let mut m: BTreeMap<(u8, u8), u32> = BTreeMap::new();
	for w in code.windows(2) {
		if !w[0].op.meta().fuse_head_ok || !w[1].op.meta().fuse_tail_ok {
			continue;
		}
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
			kind: FKind::Base(ins.op),
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
		if rec.kind == FKind::Base(Op::Jmp)
			|| rec.kind == FKind::Base(Op::Jf)
			|| rec.kind == FKind::Base(Op::Jt)
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

/// 发射端（P1-b）：fused 记录流的 SoA 编码——opcode 列按「Base→方言基础码、
/// Fused(k)→`OpMap::fused_tramp_code(s_idx, k)`」写；四条 u16var 通道流仍按
/// slot_perm 落物理道（散射/置换面零改动）。FKind::Fused 的通道布局恒为
/// [X.a, X.b, Y.a, Y.b]（白名单对全部 2+2 通道，无空洞，无噪声填充口）。
pub fn encode_soa_fused(
	recs: &[FRec],
	map: &super::isa::OpMap,
	slot_perm: &[u8; 4],
	s_idx: usize,
	out: &mut Vec<u8>,
) {
	for r in recs {
		let code = match r.kind {
			FKind::Base(op) => map.code_for(op, s_idx),
			FKind::Fused(k) => super::isa::OpMap::fused_tramp_code(s_idx, k as usize),
		};
		out.push(code);
	}
	for sl in 0..4 {
		let which = slot_perm[sl] as usize;
		for r in recs {
			let v = [r.a, r.b, r.c, r.d][which];
			super::isa::encode_u16var(out, v);
		}
	}
}

/// V19 R5-b：fv 条目掩码——65536 域非线性折叠（交叉半积 + 高位回混），
/// 按 1-based 条目下标逐条演化（base = 每函数 fvm 快照）。
/// 镜像为模板 fused trampoline 内的同式 Lua 文本 `_m=...`，替代「恒 fvm 常量
/// 减掩」线性面（静态可常折面灭绝之一）。双精度精确域内最大中间值 < 2^25。
pub fn fv_mask_nl(base: u16, j: u32) -> u16 {
	let b = base as u64;
	let mut m = (b + j as u64 * (b % 251 + 1)) % 65_536;
	m = (m + (m % 256) * (m / 256)) % 65_536;
	m = (m * 257 + m / 64) % 65_536;
	m as u16
}

/// 每函数 FV 段打包：词汇对 (opA, opB) → 单条 u16 = tailCodeA + tailCodeB*256，
/// 存链表项前加 mask（mask 由调用方以 fv_mask_nl(fvm 快照, 1-based 下标) 现算）。
/// 返回 None = 词汇对含白名单外 op（防御：不应发生，tail_index 已过滤）。
pub fn pack_fv_entry(pair: (u8, u8), s_idx: usize, mask: u16) -> Option<u16> {
	let (oa, ob) = (
		super::isa::op_from_index(pair.0),
		super::isa::op_from_index(pair.1),
	);
	let (ja, jb) = (tail_index(oa)?, tail_index(ob)?);
	let wa = super::isa::OpMap::fused_tail_code(s_idx, ja as usize) as u32;
	let wb = super::isa::OpMap::fused_tail_code(s_idx, jb as usize) as u32;
	Some(((wa + wb * 256 + mask as u32) % 65536) as u16)
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
		assert!(matches!(out.last().unwrap().kind, FKind::Base(Op::Jf)));
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

	#[test]
	fn op_index_roundtrip() {
		for i in 0..crate::vmgen::isa::N_OPS {
			assert_eq!(
				op_index(crate::vmgen::isa::op_from_index(i as u8)),
				i,
				"op_from_index/op_index mismatch at {i}"
			);
		}
	}

	#[test]
	fn encode_soa_fused_layout_matches_base_stream_structure() {
		use crate::vmgen::isa::{encode_soa_for, Instr, OpMap};
		let map = OpMap::new(&mut crate::rng::Rng::new(5));
		let perm: [u8; 4] = [2, 0, 3, 1];
		// base-only：与既有 encode_soa_for 逐字节一致（不变量：Base 无新行为）
		let recs = vec![
			FRec { kind: FKind::Base(Op::LoadK), a: 3, b: 9, c: 0, d: 0 },
			FRec { kind: FKind::Base(Op::Return), a: 1, b: 0, c: 0, d: 0 },
		];
		let mut a = Vec::new();
		encode_soa_fused(&recs, &map, &perm, 2, &mut a);
		let mut b = Vec::new();
		encode_soa_for(
			&[Instr::abcd(Op::LoadK, 3, 9, 0, 0), Instr::abcd(Op::Return, 1, 0, 0, 0)],
			&map, &perm, 2, &mut b,
		);
		assert_eq!(a, b);
		// fused 码位：base+44+k（k=0），四条通道序 = slot_perm 物理序
		let recs2 = vec![FRec { kind: FKind::Fused(0), a: 7, b: 8, c: 9, d: 10 }];
		let mut out = Vec::new();
		encode_soa_fused(&recs2, &map, &perm, 1, &mut out);
		assert_eq!(out[0], OpMap::fused_tramp_code(1, 0));
		// perm [2,0,3,1] → 物理道序 c=9, a=7, d=10, b=8（u16var 直连，均 <128：单字节）
		assert_eq!(&out[1..], &[9u8, 7, 10, 8]);
	}

	#[test]
	fn pack_fv_entry_roundtrip_and_whitelist_only() {
		let lk = op_index(Op::LoadK) as u8;
		let mv = op_index(Op::Move) as u8;
		let mask = 51_007u16;
		let e = pack_fv_entry((lk, mv), 3, mask).expect("whitelist pair packs");
		let plain = (e as u32 + 65536 - mask as u32) % 65536;
		let wa = plain % 256;
		let wb = plain / 256;
		assert_eq!(wa, crate::vmgen::isa::OpMap::fused_tail_code(3, 0) as u32); // LoadK → j=0
		assert_eq!(wb, crate::vmgen::isa::OpMap::fused_tail_code(3, 1) as u32); // Move → j=1
		assert!(pack_fv_entry((lk, op_index(Op::Add) as u8), 3, 0).is_none());
	}
}
