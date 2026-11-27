//! Phase G1 (obf.lua tech-extraction spec): runtime API capability slots.
//!
//! The interpreter binds every host primitive it may touch through the
//! `VmPrim` allowlist (caps.rs is the single source of truth for which
//! primitives exist, their template identifiers, and their Lua binding
//! expressions). This module owns the per-build slot layout:
//!
//! - Every build draws a fresh permutation of the slot pool, so the
//!   primitive -> slot mapping changes between builds (G1: "每次构建
//!   打乱 slot layout") while staying reproducible for a fixed seed.
//! - Slots are unique by construction (drawn without replacement from
//!   `1..=SLOT_POOL`), so two primitives can never alias one slot.
//! - A primitive the capability manifest disables for a given program
//!   still receives a manifest position but binds `nil` in the template;
//!   it is never callable. `validate_layout` pins the converse: every
//!   *enabled* primitive must hold a unique, in-range slot, so a layout
//!   bug fails at generation time instead of as a stray nil call in the
//!   emitted artifact.
//!
//! This is capability isolation / build diversity, not a secrecy
//! mechanism: the binding expressions are visible in the output.

use super::caps::VmPrim;
use crate::rng::Rng;

/// Slot numbers are drawn from `1..=SLOT_POOL` (0 stays unused so a
/// missing binding reads as nil rather than aliasing slot 0).
pub const SLOT_POOL: u8 = 80;

/// Assign one unique slot per `VmPrim` (indexed by `VmPrim as usize`,
/// matching the manifest order in `VmPrim::ALL`).
pub fn assign_slots(rng: &mut Rng) -> [u8; VmPrim::COUNT] {
	let mut pool: Vec<u8> = (1..=SLOT_POOL).collect();
	rng.shuffle(&mut pool);
	let mut out = [0u8; VmPrim::COUNT];
	out.copy_from_slice(&pool[..VmPrim::COUNT]);
	out
}

/// Layout invariants for one build. `enabled` is the capability
/// manifest's per-program activation predicate (see
/// `CapabilityManifest::prim_enabled_in_template`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiSlotError {
	/// An enabled primitive has no slot (0 = unassigned).
	MissingSlot(VmPrim),
	/// Two enabled primitives share one slot.
	DuplicateSlot { slot: u8, first: VmPrim, second: VmPrim },
	/// A slot fell outside the pool the runtime table is sized for.
	OutOfRange(VmPrim, u8),
}

pub fn validate_layout(
	slots: &[u8; VmPrim::COUNT],
	enabled: &dyn Fn(VmPrim) -> bool,
) -> Result<(), ApiSlotError> {
	let mut seen: [Option<VmPrim>; (SLOT_POOL as usize) + 1] = [None; (SLOT_POOL as usize) + 1];
	for prim in VmPrim::ALL {
		if !enabled(prim) {
			continue;
		}
		let slot = slots[prim as usize];
		if slot == 0 {
			return Err(ApiSlotError::MissingSlot(prim));
		}
		if slot > SLOT_POOL {
			return Err(ApiSlotError::OutOfRange(prim, slot));
		}
		if let Some(first) = seen[slot as usize] {
			return Err(ApiSlotError::DuplicateSlot { slot, first, second: prim });
		}
		seen[slot as usize] = Some(prim);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn assignment_is_deterministic_per_seed() {
		let a = assign_slots(&mut Rng::new(42));
		let b = assign_slots(&mut Rng::new(42));
		assert_eq!(a, b, "same seed must reproduce the same slot layout");
	}

	#[test]
	fn assignment_is_unique_and_in_range() {
		for seed in 1..=64u64 {
			let slots = assign_slots(&mut Rng::new(seed));
			validate_layout(&slots, &|_| true).unwrap_or_else(|e| {
				panic!("seed {seed}: layout must validate with all prims enabled: {e:?}")
			});
		}
	}

	#[test]
	fn layout_varies_across_builds() {
		let base = assign_slots(&mut Rng::new(1));
		let mut distinct = 0;
		for seed in 2..=33u64 {
			if assign_slots(&mut Rng::new(seed)) != base {
				distinct += 1;
			}
		}
		assert!(distinct >= 28, "slot layout should change between builds (got {distinct}/32)");
	}

	#[test]
	fn validate_rejects_missing_duplicate_and_out_of_range() {
		let mut slots = assign_slots(&mut Rng::new(7));
		let all = |_: VmPrim| true;

		let saved = slots[VmPrim::StrChar as usize];
		slots[VmPrim::StrChar as usize] = 0;
		assert_eq!(validate_layout(&slots, &all), Err(ApiSlotError::MissingSlot(VmPrim::StrChar)));
		slots[VmPrim::StrChar as usize] = saved;

		slots[VmPrim::StrByte as usize] = slots[VmPrim::StrChar as usize];
		assert!(matches!(
			validate_layout(&slots, &all),
			Err(ApiSlotError::DuplicateSlot { .. })
		));

		let mut slots = assign_slots(&mut Rng::new(7));
		slots[VmPrim::StrByte as usize] = SLOT_POOL + 1;
		assert_eq!(
			validate_layout(&slots, &all),
			Err(ApiSlotError::OutOfRange(VmPrim::StrByte, SLOT_POOL + 1))
		);

		// Disabled primitives are exempt: they bind nil by design.
		let mut slots = assign_slots(&mut Rng::new(7));
		slots[VmPrim::Next as usize] = 0;
		assert!(validate_layout(&slots, &|p| p != VmPrim::Next).is_ok());
	}
}
