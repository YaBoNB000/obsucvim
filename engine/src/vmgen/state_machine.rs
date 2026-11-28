//! Phase D (obf.lua tech-extraction spec): decoder state-machine IR,
//! verifier, label randomizer, and Luau emitter.
//!
//! The existing section reader in `template.rs` is a hand-written
//! if/elseif walk embedded in a string template. This module provides the
//! checked IR the spec requires so decoder logic can be *modeled, verified,
//! and emitted* instead of hand-maintained: every state has a unique id,
//! every non-terminal state has a guarded exit whose final transition is an
//! unconditional fallthrough, cycles must make cursor progress, temporaries
//! must be initialized before use, and emitted Luau dispatches over
//! randomized per-build state labels (never bare 0,1,2,... in order).
//!
//! First modeled domain (spec D2): the section reader — header read,
//! bounded varint read, section-directory read, prototype-field descriptor
//! read, lazy-field payload read. The reference interpreter in the tests
//! executes the same IR the emitter consumes, so IR/emitter drift is caught
//! by construction (the emitter is generated from the verified IR only).

use crate::rng::Rng;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TempId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardExpr {
	/// Always taken (must be the last transition of its state).
	Always,
	/// Taken when the temp holds a value `< bound`.
	TempLt { temp: TempId, bound: u32 },
	/// Taken when the temp equals `value`.
	TempEq { temp: TempId, value: u32 },
}

impl GuardExpr {
	fn reads(&self) -> Option<TempId> {
		match self {
			GuardExpr::Always => None,
			GuardExpr::TempLt { temp, .. } | GuardExpr::TempEq { temp, .. } => Some(*temp),
		}
	}
	fn holds(&self, temps: &HashMap<TempId, u32>) -> Result<bool, String> {
		match self {
			GuardExpr::Always => Ok(true),
			GuardExpr::TempLt { temp, bound } => {
				let v = temps.get(temp).ok_or("guard reads uninitialized temp")?;
				Ok(*v < *bound)
			}
			GuardExpr::TempEq { temp, value } => {
				let v = temps.get(temp).ok_or("guard reads uninitialized temp")?;
				Ok(*v == *value)
			}
		}
	}
}

/// Controlled action vocabulary (spec §3.3): no arbitrary Lua strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateAction {
	ReadByte { dst: TempId },
	ReadVarint { dst: TempId, cursor: TempId },
	DecodeOperand { lane: u8, index: TempId, dst: TempId },
	ResolvePrototypeField { proto: TempId, field: u8, dst: TempId },
	UpdateCursor { cursor: TempId, delta: i32 },
	UpdateSeed { seed: TempId, input: TempId },
	WriteCache { key: TempId, value: TempId },
	Finish { result: TempId },
}

impl StateAction {
	fn reads(&self) -> Vec<TempId> {
		match self {
			StateAction::ReadByte { .. } => vec![],
			StateAction::ReadVarint { cursor, .. } => vec![*cursor],
			StateAction::DecodeOperand { index, .. } => vec![*index],
			StateAction::ResolvePrototypeField { proto, .. } => vec![*proto],
			StateAction::UpdateCursor { cursor, .. } => vec![*cursor],
			StateAction::UpdateSeed { seed, input } => vec![*seed, *input],
			StateAction::WriteCache { key, value } => vec![*key, *value],
			StateAction::Finish { result } => vec![*result],
		}
	}
	fn writes(&self) -> Vec<TempId> {
		match self {
			StateAction::ReadByte { dst } => vec![*dst],
			StateAction::ReadVarint { dst, .. } => vec![*dst],
			StateAction::DecodeOperand { dst, .. } => vec![*dst],
			StateAction::ResolvePrototypeField { dst, .. } => vec![*dst],
			StateAction::UpdateCursor { cursor, .. } => vec![*cursor],
			StateAction::UpdateSeed { seed, .. } => vec![*seed],
			StateAction::WriteCache { .. } => vec![],
			StateAction::Finish { .. } => vec![],
		}
	}
	fn is_finish(&self) -> bool {
		matches!(self, StateAction::Finish { .. })
	}
	fn advances_cursor(&self) -> bool {
		matches!(self, StateAction::UpdateCursor { delta, .. } if *delta != 0)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransition {
	pub from: StateId,
	pub guard: GuardExpr,
	pub actions: Vec<StateAction>,
	pub next: StateId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineSpec {
	pub name: String,
	pub initial_state: StateId,
	pub terminal_states: Vec<StateId>,
	/// Terminal states that represent fail-closed rejection (subset of
	/// `terminal_states`); reaching one aborts decoding with an error.
	pub failure_states: Vec<StateId>,
	/// Temporaries the emitted context initializes before the first state
	/// (e.g. cursor/accumulator zeroed by the runtime template).
	pub entry_temps: Vec<TempId>,
	/// Declared states in declaration order (also the layout order before
	/// label randomization).
	pub state_layout: Vec<StateId>,
	pub transitions: Vec<StateTransition>,
}

impl StateMachineSpec {
	fn outgoing(&self, s: StateId) -> Vec<&StateTransition> {
		self.transitions.iter().filter(|t| t.from == s).collect()
	}

	/// Full graph verification (spec §3.3 generator requirements + D5):
	/// unique ids, known endpoints, reachability, guarded exits with an
	/// unconditional final fallthrough, terminal states have no outgoing
	/// transitions and are only left via `Finish`, progress on cycles, and
	/// must-be-initialized temporaries on every path.
	pub fn verify(&self) -> Result<(), String> {
		let states: BTreeSet<StateId> = self.state_layout.iter().copied().collect();
		if states.len() != self.state_layout.len() {
			return Err(format!("{}: duplicate state id in layout", self.name));
		}
		if !states.contains(&self.initial_state) {
			return Err(format!("{}: initial state not declared", self.name));
		}
		let terminals: BTreeSet<StateId> = self.terminal_states.iter().copied().collect();
		if terminals.len() != self.terminal_states.len() || terminals.is_empty() {
			return Err(format!("{}: terminal states empty or duplicated", self.name));
		}
		for t in &self.transitions {
			if !states.contains(&t.from) || !states.contains(&t.next) {
				return Err(format!("{}: transition touches undeclared state", self.name));
			}
			if terminals.contains(&t.from) {
				return Err(format!("{}: terminal state {:?} has outgoing transition", self.name, t.from));
			}
			let finishes = t.actions.iter().filter(|a| a.is_finish()).count();
			if finishes > 0 && !terminals.contains(&t.next) {
				return Err(format!("{}: Finish action must lead to a terminal state", self.name));
			}
		}
		for &s in &self.state_layout {
			if terminals.contains(&s) {
				continue;
			}
			let out = self.outgoing(s);
			if out.is_empty() {
				return Err(format!("{}: non-terminal state {:?} has no outgoing transition", self.name, s));
			}
			if !matches!(out.last().map(|t| t.guard), Some(GuardExpr::Always)) {
				return Err(format!(
					"{}: state {:?} lacks an unconditional final fallthrough",
					self.name, s
				));
			}
		}
		for f in &self.failure_states {
			if !terminals.contains(f) {
				return Err(format!("{}: failure state {:?} is not terminal", self.name, f));
			}
		}
		// Reachability from the initial state.
		let mut reachable: BTreeSet<StateId> = BTreeSet::new();
		let mut stack = vec![self.initial_state];
		while let Some(s) = stack.pop() {
			if reachable.insert(s) {
				for t in self.outgoing(s) {
					stack.push(t.next);
				}
			}
		}
		for &s in &self.state_layout {
			if !reachable.contains(&s) {
				return Err(format!("{}: unreachable state {:?}", self.name, s));
			}
		}
		// At least one terminal must be reachable.
		if !self.terminal_states.iter().any(|t| reachable.contains(t)) {
			return Err(format!("{}: no terminal state reachable", self.name));
		}
		// Cycle progress: every SCC that contains a cycle must advance a
		// cursor somewhere inside the SCC.
		for scc in self.strongly_connected_components() {
			let cyclic = scc.len() > 1
				|| self.outgoing(scc[0]).iter().any(|t| t.next == scc[0]);
			if !cyclic {
				continue;
			}
			let inset: BTreeSet<StateId> = scc.iter().copied().collect();
			let progresses = self
				.transitions
				.iter()
				.filter(|t| inset.contains(&t.from) && inset.contains(&t.next))
				.any(|t| t.actions.iter().any(|a| a.advances_cursor()));
			if !progresses {
				return Err(format!(
					"{}: cycle through {:?} makes no cursor progress",
					self.name, scc[0]
				));
			}
		}
		// Must-be-initialized analysis (meet over predecessors, entry = {}).
		let mut in_sets: BTreeMap<StateId, BTreeSet<TempId>> = BTreeMap::new();
		let all_temps: BTreeSet<TempId> = self
			.transitions
			.iter()
			.flat_map(|t| t.actions.iter().flat_map(|a| a.writes()))
			.collect();
		for &s in &self.state_layout {
			in_sets.insert(
				s,
				if s == self.initial_state {
					self.entry_temps.iter().copied().collect()
				} else {
					all_temps.clone()
				},
			);
		}
		loop {
			let mut changed = false;
			for &s in &self.state_layout {
				if s == self.initial_state {
					continue;
				}
				let preds: Vec<StateId> = self
					.transitions
					.iter()
					.filter(|t| t.next == s)
					.map(|t| t.from)
					.collect();
				if preds.is_empty() {
					continue;
				}
				let mut meet: Option<BTreeSet<TempId>> = None;
				for p in preds {
					let out = self.state_out_writes(p, &in_sets);
					meet = Some(match meet {
						None => out,
						Some(m) => m.intersection(&out).copied().collect(),
					});
				}
				let meet = meet.unwrap_or_default();
				if in_sets[&s] != meet {
					in_sets.insert(s, meet);
					changed = true;
				}
			}
			if !changed {
				break;
			}
		}
		for t in &self.transitions {
			let mut known = in_sets[&t.from].clone();
			if let Some(g) = t.guard.reads() {
				if !known.contains(&g) {
					return Err(format!(
						"{}: guard in state {:?} reads uninitialized temp {:?}",
						self.name, t.from, g
					));
				}
			}
			for a in &t.actions {
				for r in a.reads() {
					// A temp written earlier in the same transition is fine.
					if !known.contains(&r) {
						return Err(format!(
							"{}: action in state {:?} reads uninitialized temp {:?}",
							self.name, t.from, r
						));
					}
				}
				for w in a.writes() {
					known.insert(w);
				}
			}
		}
		Ok(())
	}

	fn state_out_writes(&self, s: StateId, in_sets: &BTreeMap<StateId, BTreeSet<TempId>>) -> BTreeSet<TempId> {
		let mut out = in_sets[&s].clone();
		for t in self.outgoing(s) {
			for a in &t.actions {
				for w in a.writes() {
					out.insert(w);
				}
			}
		}
		out
	}

	/// Tarjan SCC (iterative driver over the small decoder graphs).
	fn strongly_connected_components(&self) -> Vec<Vec<StateId>> {
		let mut index_of: HashMap<StateId, usize> = HashMap::new();
		let mut low: HashMap<StateId, usize> = HashMap::new();
		let mut on_stack: BTreeSet<StateId> = BTreeSet::new();
		let mut stack: Vec<StateId> = Vec::new();
		let mut next_index = 0usize;
		let mut result = Vec::new();
		fn strongconnect(
			v: StateId,
			spec: &StateMachineSpec,
			index_of: &mut HashMap<StateId, usize>,
			low: &mut HashMap<StateId, usize>,
			on_stack: &mut BTreeSet<StateId>,
			stack: &mut Vec<StateId>,
			next_index: &mut usize,
			result: &mut Vec<Vec<StateId>>,
		) {
			index_of.insert(v, *next_index);
			low.insert(v, *next_index);
			*next_index += 1;
			stack.push(v);
			on_stack.insert(v);
			for t in spec.outgoing(v) {
				let w = t.next;
				if !index_of.contains_key(&w) {
					strongconnect(w, spec, index_of, low, on_stack, stack, next_index, result);
					let (lw, lv) = (low[&w], low[&v]);
					low.insert(v, lw.min(lv));
				} else if on_stack.contains(&w) {
					let (iw, lv) = (index_of[&w], low[&v]);
					low.insert(v, iw.min(lv));
				}
			}
			if low[&v] == index_of[&v] {
				let mut scc = Vec::new();
				loop {
					// invariant: Tarjan's algorithm only unwinds nodes it pushed.
					let w = stack.pop().expect("SCC stack non-empty by construction");
					on_stack.remove(&w);
					scc.push(w);
					if w == v {
						break;
					}
				}
				result.push(scc);
			}
		}
		for &s in &self.state_layout {
			if !index_of.contains_key(&s) {
				strongconnect(
					s, self, &mut index_of, &mut low, &mut on_stack, &mut stack,
					&mut next_index, &mut result,
				);
			}
		}
		result
	}

	/// Assign each state a distinct randomized `u16` label. Labels are drawn
	/// from `1..=65535` shuffled, so the emitted dispatcher never exposes
	/// the declaration order as consecutive state numbers.
	pub fn randomize_labels(&self, rng: &mut Rng) -> HashMap<StateId, u16> {
		let mut pool: Vec<u16> = (1..=u16::MAX).collect();
		rng.shuffle(&mut pool);
		let mut out = HashMap::new();
		for (k, &s) in self.state_layout.iter().enumerate() {
			out.insert(s, pool[k]);
		}
		out
	}

	/// Emit a Luau dispatcher for this machine. The generated function takes
	/// a context table `ctx` whose `act` field is an array of action
	/// handlers keyed by the action's position in `self.transitions`
	/// declaration order flattened over states — handlers are supplied by
	/// the runtime template, this emitter owns only control structure:
	/// one leaf per state, guarded transitions in declaration order, an
	/// explicit invalid-state failure, and `return` at terminal states.
	pub fn emit_luau(&self, labels: &HashMap<StateId, u16>, fn_name: &str) -> Result<String, String> {
		self.verify()?;
		let mut out = String::new();
		out.push_str(&format!("local function {fn_name}(ctx)\n"));
		out.push_str(&format!("  local st = {}\n", labels[&self.initial_state]));
		out.push_str("  while true do\n");
		// Balanced dispatch over sorted labels.
		let mut sorted: Vec<StateId> = self.state_layout.clone();
		sorted.sort_by_key(|s| labels[s]);
		out.push_str(&self.emit_dispatch(&sorted, labels, 0, sorted.len(), 2));
		out.push_str("    error(\"invalid decoder state\")\n");
		out.push_str("  end\nend\n");
		Ok(out)
	}

	fn emit_dispatch(
		&self,
		sorted: &[StateId],
		labels: &HashMap<StateId, u16>,
		lo: usize,
		hi: usize,
		indent: usize,
	) -> String {
		let pad = " ".repeat(indent);
		if hi - lo == 1 {
			let s = sorted[lo];
			return self.emit_state_leaf(s, labels, indent);
		}
		let mid = (lo + hi) / 2;
		let thr = labels[&sorted[mid - 1]];
		format!(
			"{pad}if st <= {thr} then\n{left}{pad}else\n{right}{pad}end\n",
			left = self.emit_dispatch(sorted, labels, lo, mid, indent),
			right = self.emit_dispatch(sorted, labels, mid, hi, indent),
		)
	}

	fn emit_state_leaf(&self, s: StateId, labels: &HashMap<StateId, u16>, indent: usize) -> String {
		let pad = " ".repeat(indent);
		let mut out = String::new();
		if self.terminal_states.contains(&s) {
			if self.failure_states.contains(&s) {
				out.push_str(&format!(
					"{pad}if st == {} then return nil, ctx.err end\n",
					labels[&s]
				));
			} else {
				out.push_str(&format!("{pad}if st == {} then return ctx.result end\n", labels[&s]));
			}
			return out;
		}
		out.push_str(&format!("{pad}if st == {} then\n", labels[&s]));
		out.push_str(&self.emit_transitions(&self.outgoing(s), labels, indent + 2));
		out.push_str(&format!("{pad}end\n"));
		out
	}

	fn emit_transitions(&self, ts: &[&StateTransition], labels: &HashMap<StateId, u16>, indent: usize) -> String {
		let pad = " ".repeat(indent);
		let t = ts[0];
		let mut body = String::new();
		for a in &t.actions {
			body.push_str(&format!("{pad}ctx.act.{} (ctx, {})\n", action_key(a), action_args(a)));
		}
		body.push_str(&format!("{pad}st = {}\n", labels[&t.next]));
		let cond = match t.guard {
			GuardExpr::Always => return body,
			GuardExpr::TempLt { temp, bound } => format!("ctx.t{} < {}", temp.0, bound),
			GuardExpr::TempEq { temp, value } => format!("ctx.t{} == {}", temp.0, value),
		};
		format!(
			"{pad}if {cond} then\n{body}{pad}else\n{rest}{pad}end\n",
			rest = self.emit_transitions(&ts[1..], labels, indent)
		)
	}

}

fn action_key(a: &StateAction) -> &'static str {
	match a {
		StateAction::ReadByte { .. } => "read_byte",
		StateAction::ReadVarint { .. } => "read_varint",
		StateAction::DecodeOperand { .. } => "decode_operand",
		StateAction::ResolvePrototypeField { .. } => "resolve_field",
		StateAction::UpdateCursor { .. } => "update_cursor",
		StateAction::UpdateSeed { .. } => "update_seed",
		StateAction::WriteCache { .. } => "write_cache",
		StateAction::Finish { .. } => "finish",
	}
}

fn action_args(a: &StateAction) -> String {
	match a {
		StateAction::ReadByte { dst } => format!("{}", dst.0),
		StateAction::ReadVarint { dst, cursor } => format!("{}, {}", dst.0, cursor.0),
		StateAction::DecodeOperand { lane, index, dst } => format!("{}, {}, {}", lane, index.0, dst.0),
		StateAction::ResolvePrototypeField { proto, field, dst } => {
			format!("{}, {}, {}", proto.0, field, dst.0)
		}
		StateAction::UpdateCursor { cursor, delta } => format!("{}, {}", cursor.0, delta),
		StateAction::UpdateSeed { seed, input } => format!("{}, {}", seed.0, input.0),
		StateAction::WriteCache { key, value } => format!("{}, {}", key.0, value.0),
		StateAction::Finish { result } => format!("{}", result.0),
	}
}

/// The first modeled decoder machine (spec D2): reads a bounded varint
/// (section lengths / counts), one byte at a time, failing closed on
/// truncation or overlong encodings. Used by the tests as the reference
/// shape for header/varint/section-directory readers; field-descriptor
/// reading is the same loop with a different `Finish` consumer.
pub fn section_varint_machine() -> StateMachineSpec {
	let (start, loop_, more, done, fail) =
		(StateId(0), StateId(1), StateId(2), StateId(3), StateId(4));
	let (cursor, acc, byte, nbytes) = (TempId(0), TempId(1), TempId(2), TempId(3));
	StateMachineSpec {
		name: "section_varint".to_string(),
		initial_state: start,
		terminal_states: vec![done, fail],
		failure_states: vec![fail],
		entry_temps: vec![cursor, acc, nbytes],
		state_layout: vec![start, loop_, more, done, fail],
		transitions: vec![
			StateTransition {
				from: start,
				guard: GuardExpr::Always,
				actions: vec![StateAction::ReadByte { dst: byte }],
				next: loop_,
			},
			StateTransition {
				from: loop_,
				guard: GuardExpr::TempLt { temp: byte, bound: 128 },
				actions: vec![
					StateAction::UpdateSeed { seed: acc, input: byte },
					StateAction::Finish { result: acc },
				],
				next: done,
			},
			StateTransition {
				from: loop_,
				guard: GuardExpr::Always,
				actions: vec![
					StateAction::UpdateSeed { seed: acc, input: byte },
					StateAction::UpdateCursor { cursor, delta: 1 },
					StateAction::UpdateCursor { cursor: nbytes, delta: 1 },
				],
				next: more,
			},
			StateTransition {
				from: more,
				guard: GuardExpr::TempLt { temp: nbytes, bound: 5 },
				actions: vec![StateAction::ReadByte { dst: byte }],
				next: loop_,
			},
			StateTransition {
				from: more,
				guard: GuardExpr::Always,
				actions: vec![StateAction::Finish { result: acc }],
				next: fail,
			},
		],
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Tiny reference interpreter for the IR, used to check that label
	/// randomization and guard structure do not change semantics, and that
	/// malformed inputs fail closed instead of panicking.
	struct Interp<'a> {
		spec: &'a StateMachineSpec,
		buf: &'a [u8],
		temps: HashMap<TempId, u32>,
		cache: HashMap<u32, u32>,
		steps: usize,
	}

	impl<'a> Interp<'a> {
		fn run(&mut self) -> Result<u32, String> {
			let mut state = self.spec.initial_state;
			loop {
				self.steps += 1;
				if self.steps > 10_000 {
					return Err("step budget exceeded".to_string());
				}
				if self.spec.failure_states.contains(&state) {
					return Err("decoder rejected input (fail-closed)".to_string());
				}
				if self.spec.terminal_states.contains(&state) {
					return self.temps.get(&TempId(1)).copied().ok_or_else(|| "no result".to_string());
				}
				let mut advanced = false;
				for t in self.spec.outgoing(state) {
					if t.guard.holds(&self.temps)? {
						for a in &t.actions {
							self.exec(*a)?;
						}
						state = t.next;
						advanced = true;
						break;
					}
				}
				if !advanced {
					return Err("no transition taken".to_string());
				}
			}
		}

		fn exec(&mut self, a: StateAction) -> Result<(), String> {
			match a {
				StateAction::ReadByte { dst } => {
					// The modeled reader peeks the byte at the cursor temp
					// (temp 0); a read past the end fails closed.
					let cur = *self.temps.get(&TempId(0)).unwrap_or(&0) as usize;
					let b = self.buf.get(cur).ok_or("truncated buffer")?;
					self.temps.insert(dst, *b as u32);
				}
				StateAction::ReadVarint { dst, cursor } => {
					let mut cur = *self.temps.get(&cursor).ok_or("no cursor")? as usize;
					let mut val: u32 = 0;
					let mut shift = 0;
					loop {
						let b = *self.buf.get(cur).ok_or("truncated varint")?;
						cur += 1;
						if shift >= 28 && b > 0x0f {
							return Err("varint overflow".to_string());
						}
						val |= ((b & 0x7f) as u32) << shift;
						if b < 0x80 {
							break;
						}
						shift += 7;
						if shift > 28 {
							return Err("varint too long".to_string());
						}
					}
					self.temps.insert(cursor, cur as u32);
					self.temps.insert(dst, val);
				}
				StateAction::DecodeOperand { dst, index, .. } => {
					let i = *self.temps.get(&index).ok_or("no index")?;
					self.temps.insert(dst, i.wrapping_mul(2));
				}
				StateAction::ResolvePrototypeField { dst, field, .. } => {
					self.temps.insert(dst, field as u32);
				}
				StateAction::UpdateCursor { cursor, delta } => {
					let v = self.temps.entry(cursor).or_insert(0);
					*v = v.wrapping_add(delta as u32);
				}
				StateAction::UpdateSeed { seed, input } => {
					let inp = *self.temps.get(&input).ok_or("no input")?;
					let v = self.temps.entry(seed).or_insert(0);
					*v = v.wrapping_mul(131).wrapping_add(inp & 0x7f);
				}
				StateAction::WriteCache { key, value } => {
					let k = *self.temps.get(&key).ok_or("no key")?;
					let v = *self.temps.get(&value).ok_or("no value")?;
					self.cache.insert(k, v);
				}
				StateAction::Finish { .. } => {}
			}
			Ok(())
		}
	}

	fn run_spec(spec: &StateMachineSpec, buf: &[u8]) -> Result<u32, String> {
		// Entry temps are zeroed by the emitted context, mirroring runtime init.
		let mut it = Interp { spec, buf, temps: HashMap::new(), cache: HashMap::new(), steps: 0 };
		for t in &spec.entry_temps {
			it.temps.insert(*t, 0);
		}
		it.run()
	}

	#[test]
	fn varint_machine_verifies() {
		section_varint_machine().verify().expect("modeled machine must verify");
	}

	#[test]
	fn verifier_rejects_unreachable_and_dead_states() {
		let mut spec = section_varint_machine();
		spec.state_layout.push(StateId(9));
		assert!(spec.verify().is_err(), "unreachable/dead state must fail");
	}

	#[test]
	fn verifier_rejects_cycle_without_progress() {
		let a = StateId(0);
		let b = StateId(1);
		let spec = StateMachineSpec {
			name: "loopy".to_string(),
			initial_state: a,
			terminal_states: vec![b],
			failure_states: vec![],
			entry_temps: vec![],
			state_layout: vec![a, b],
			transitions: vec![StateTransition {
				from: a,
				guard: GuardExpr::Always,
				actions: vec![StateAction::ReadByte { dst: TempId(0) }],
				next: a,
			}],
		};
		assert!(spec.verify().is_err());
	}

	#[test]
	fn verifier_rejects_uninitialized_temp_read() {
		let a = StateId(0);
		let b = StateId(1);
		let spec = StateMachineSpec {
			name: "uninit".to_string(),
			initial_state: a,
			terminal_states: vec![b],
			failure_states: vec![],
			entry_temps: vec![],
			state_layout: vec![a, b],
			transitions: vec![StateTransition {
				from: a,
				guard: GuardExpr::Always,
				actions: vec![StateAction::Finish { result: TempId(7) }],
				next: b,
			}],
		};
		assert!(spec.verify().is_err());
	}

	#[test]
	fn labels_are_randomized_and_unique() {
		let spec = section_varint_machine();
		let mut rng = Rng::new(42);
		let labels = spec.randomize_labels(&mut rng);
		let values: BTreeSet<u16> = labels.values().copied().collect();
		assert_eq!(values.len(), spec.state_layout.len(), "labels unique");
		assert!(
			!spec.state_layout.iter().enumerate().all(|(i, s)| labels[s] as usize == i + 1),
			"labels must not be the sequential declaration order"
		);
	}

	#[test]
	fn emitted_luau_has_one_leaf_per_state_and_invalid_fallthrough() {
		let spec = section_varint_machine();
		let mut rng = Rng::new(1);
		let labels = spec.randomize_labels(&mut rng);
		let lua = spec.emit_luau(&labels, "sm_section_varint").expect("emit");
		for s in &spec.state_layout {
			let needle = format!("st == {}", labels[s]);
			assert_eq!(lua.matches(&needle).count(), 1, "exactly one leaf per state: {needle}");
		}
		assert!(lua.contains("while true do"));
		assert!(lua.contains("return ctx.result"), "terminal leaf returns");
		assert!(lua.contains("return nil, ctx.err"), "failure leaf fails closed");
		assert!(lua.contains("invalid decoder state"), "invalid label fails closed");
		// Structural balance: every emitted `if` has a matching `end`.
		let ifs = lua.matches(" then").count();
		let ends = lua.matches("end").count();
		assert_eq!(ends, ifs + 2, "emitted Luau must be end-balanced (while + function): ifs={ifs} ends={ends}");
	}

	#[test]
	fn reference_interpreter_fails_closed_on_truncation() {
		let spec = section_varint_machine();
		// Empty, truncated, and overlong inputs must error, never panic.
		assert!(run_spec(&spec, &[]).is_err());
		assert!(run_spec(&spec, &[0x80]).is_err());
		assert!(run_spec(&spec, &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80]).is_err());
		assert_eq!(run_spec(&spec, &[0x2a]).unwrap(), 42);
		assert!(run_spec(&spec, &[0x80, 0x01]).is_ok());
	}
}
