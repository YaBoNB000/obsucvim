//! Phase B (obf.lua tech-extraction spec): per-prototype field layout.
//!
//! The prototype wire format is a sequence of sections. Before this module
//! the section *order* was shuffled per function but the section *tag bytes*
//! came from one build-wide table, so every prototype in a build shared the
//! same field->slot mapping. `ProtoFieldLayout` gives each prototype its own
//! randomized semantic-field -> slot (tag byte) bijection.
//!
//! The semantic field set mirrors the real section schema emitted by
//! `compiler::Ctx::finish` (spec B1: do not invent fields the schema does
//! not have). Section id 6 is reserved and must never be emitted; it is not
//! a field. Slot values are tag bytes in `1..=250` (0 is never a valid tag).
//!
//! Compatibility note: obsucvim artifacts embed their own runtime, so the
//! layout directory travels inside the generated artifact (the per-function
//! tag table in `template::generate`) and there is no cross-version wire
//! compatibility surface. The encoded directory form below is the canonical
//! compiler-side representation, used for self-checks and tests; decoding is
//! fail-closed (duplicate slot, unknown field, missing required field, or
//! out-of-range slot are all errors, never a guessed fallback).

use crate::rng::Rng;

/// Semantic prototype fields, indexed by their stable section id.
/// Id 6 is intentionally absent: it is a reserved, never-emitted section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoFieldId {
	Head = 0,
	CkSeed = 1,
	Upvalues = 2,
	Constants = 3,
	Slots = 4,
	Code = 5,
	FusedVocab = 7,
}

impl ProtoFieldId {
	pub const ALL: [ProtoFieldId; 7] = [
		ProtoFieldId::Head,
		ProtoFieldId::CkSeed,
		ProtoFieldId::Upvalues,
		ProtoFieldId::Constants,
		ProtoFieldId::Slots,
		ProtoFieldId::Code,
		ProtoFieldId::FusedVocab,
	];

	/// Fields that every prototype directory must contain. `Slots` and
	/// `FusedVocab` are conditional sections (scatter / super-insn only).
	pub const REQUIRED: [ProtoFieldId; 5] = [
		ProtoFieldId::Head,
		ProtoFieldId::CkSeed,
		ProtoFieldId::Upvalues,
		ProtoFieldId::Constants,
		ProtoFieldId::Code,
	];

	pub fn from_id(id: u8) -> Option<ProtoFieldId> {
		match id {
			0 => Some(ProtoFieldId::Head),
			1 => Some(ProtoFieldId::CkSeed),
			2 => Some(ProtoFieldId::Upvalues),
			3 => Some(ProtoFieldId::Constants),
			4 => Some(ProtoFieldId::Slots),
			5 => Some(ProtoFieldId::Code),
			7 => Some(ProtoFieldId::FusedVocab),
			_ => None,
		}
	}

	pub fn id(self) -> u8 {
		self as u8
	}
}

/// Number of addressable slots in the layout (section ids 0..=7).
pub const LAYOUT_SLOTS: usize = 8;
/// Tag bytes are drawn from `1..=250`; 0 and 251..=255 never identify a field.
pub const MAX_TAG: u8 = 250;

/// One prototype's field layout: a bijection between the seven semantic
/// fields and seven distinct tag bytes. Index 6 of `semantic_to_slot` is
/// always 0 (reserved, unused).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoFieldLayout {
	semantic_to_slot: [u8; LAYOUT_SLOTS],
	layout_nonce: u32,
}

impl ProtoFieldLayout {
	/// Build a layout from an explicit semantic-id -> tag table.
	/// Fails closed on any non-bijective or out-of-range input.
	pub fn from_slots(semantic_to_slot: [u8; LAYOUT_SLOTS], layout_nonce: u32) -> Result<ProtoFieldLayout, String> {
		let layout = ProtoFieldLayout { semantic_to_slot, layout_nonce };
		layout.validate()?;
		Ok(layout)
	}

	/// Draw an independent randomized layout for one prototype.
	pub fn random(rng: &mut Rng, fn_index: usize) -> ProtoFieldLayout {
		let mut pool: Vec<u8> = (1..=MAX_TAG).collect();
		rng.shuffle(&mut pool);
		let mut semantic_to_slot = [0u8; LAYOUT_SLOTS];
		for (k, field) in ProtoFieldId::ALL.iter().enumerate() {
			semantic_to_slot[field.id() as usize] = pool[k];
		}
		// The nonce only labels the layout for compiler-side diagnostics;
		// it is derived from the drawn bytes so equal layouts share a nonce.
		let mut nonce: u32 = (fn_index as u32).wrapping_mul(0x9e37_79b9);
		for (k, &slot) in semantic_to_slot.iter().enumerate() {
			nonce ^= (slot as u32) << ((k % 4) * 8);
			nonce = nonce.rotate_left(7).wrapping_mul(0x0100_0193);
		}
		ProtoFieldLayout { semantic_to_slot, layout_nonce: nonce }
	}

	/// The legacy build-wide layout: every prototype shares one mapping.
	pub fn shared(tags: [u8; LAYOUT_SLOTS]) -> Result<ProtoFieldLayout, String> {
		Self::from_slots(tags, 0)
	}

	pub fn slot_of(&self, field: ProtoFieldId) -> u8 {
		self.semantic_to_slot[field.id() as usize]
	}

	/// Reverse lookup: which semantic field owns this tag byte in this
	/// prototype? `None` for bytes that are not field tags here.
	pub fn field_of(&self, slot: u8) -> Option<ProtoFieldId> {
		if slot == 0 {
			return None;
		}
		ProtoFieldId::ALL
			.iter()
			.copied()
			.find(|&f| self.semantic_to_slot[f.id() as usize] == slot)
	}

	pub fn layout_nonce(&self) -> u32 {
		self.layout_nonce
	}

	/// The raw 8-entry table indexed by section id (entry 6 is 0), in the
	/// shape the template's per-function tag table consumes.
	pub fn as_tags(&self) -> [u8; LAYOUT_SLOTS] {
		self.semantic_to_slot
	}

	/// Bijection + range validation. Every used field must own a distinct
	/// tag in `1..=MAX_TAG`; the reserved slot 6 must stay empty.
	pub fn validate(&self) -> Result<(), String> {
		if self.semantic_to_slot[6] != 0 {
			return Err("proto layout: reserved section 6 must not be assigned".to_string());
		}
		let mut seen = [false; 256];
		for field in ProtoFieldId::ALL {
			let slot = self.semantic_to_slot[field.id() as usize];
			if slot == 0 || slot > MAX_TAG {
				return Err(format!(
					"proto layout: field {:?} has out-of-range slot {slot}",
					field
				));
			}
			if seen[slot as usize] {
				return Err(format!("proto layout: duplicate slot {slot}"));
			}
			seen[slot as usize] = true;
		}
		// Reverse map must be the exact inverse of the forward map.
		for field in ProtoFieldId::ALL {
			if self.field_of(self.slot_of(field)) != Some(field) {
				return Err(format!("proto layout: reverse map mismatch for {:?}", field));
			}
		}
		Ok(())
	}

	/// Canonical directory encoding: for each field in `ProtoFieldId::ALL`
	/// order, one `(field_id, slot)` pair. This is the form a future wire
	/// directory would carry; today the same information is emitted into the
	/// artifact as the per-function tag table (see module docs).
	pub fn encode_directory(&self) -> Vec<u8> {
		let mut out = Vec::with_capacity(ProtoFieldId::ALL.len() * 2);
		for field in ProtoFieldId::ALL {
			out.push(field.id());
			out.push(self.slot_of(field));
		}
		out
	}

	/// Fail-closed directory decoding: unknown field id, duplicate field,
	/// duplicate slot, out-of-range slot, truncated input, or a missing
	/// required field are all errors.
	pub fn decode_directory(bytes: &[u8]) -> Result<ProtoFieldLayout, String> {
		if bytes.len() % 2 != 0 {
			return Err("proto layout directory: truncated pair".to_string());
		}
		let mut semantic_to_slot = [0u8; LAYOUT_SLOTS];
		let mut present = [false; LAYOUT_SLOTS];
		for pair in bytes.chunks_exact(2) {
			let field = ProtoFieldId::from_id(pair[0])
				.ok_or_else(|| format!("proto layout directory: unknown field id {}", pair[0]))?;
			if present[field.id() as usize] {
				return Err(format!("proto layout directory: duplicate field {:?}", field));
			}
			present[field.id() as usize] = true;
			semantic_to_slot[field.id() as usize] = pair[1];
		}
		for field in ProtoFieldId::REQUIRED {
			if !present[field.id() as usize] {
				return Err(format!("proto layout directory: missing required field {:?}", field));
			}
		}
		// Conditional fields default to distinct fresh slots only if the
		// directory omitted them *and* the caller marks them unused; the
		// canonical encoding always carries all seven, so treat a missing
		// conditional field as an incomplete directory instead of guessing.
		for field in ProtoFieldId::ALL {
			if !present[field.id() as usize] {
				return Err(format!("proto layout directory: missing field {:?}", field));
			}
		}
		Self::from_slots(semantic_to_slot, 0)
	}
}

/// Build the layout set for a whole program: one entry per function slot
/// (`fns.len() + 1` entries including the main chunk, matching the indexing
/// of `VmProgram::fns` plus the main index at `fns.len()`).
/// With `enabled == false` every prototype shares the first drawn layout
/// (the pre-Phase-B behavior), which keeps the feature individually
/// switchable without changing call sites.
pub fn program_layouts(rng: &mut Rng, fn_count_with_main: usize, enabled: bool) -> Vec<ProtoFieldLayout> {
	if fn_count_with_main == 0 {
		return Vec::new();
	}
	if !enabled {
		let shared = ProtoFieldLayout::random(rng, 0);
		return vec![shared; fn_count_with_main];
	}
	(0..fn_count_with_main)
		.map(|fi| ProtoFieldLayout::random(rng, fi))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn rng(seed: u64) -> Rng {
		Rng::new(seed)
	}

	#[test]
	fn random_layout_is_bijective() {
		let mut r = rng(42);
		for fi in 0..64 {
			let layout = ProtoFieldLayout::random(&mut r, fi);
			layout.validate().expect("random layout must validate");
			assert_eq!(layout.as_tags()[6], 0, "reserved section stays empty");
		}
	}

	#[test]
	fn directory_round_trip() {
		let mut r = rng(7);
		for fi in 0..32 {
			let layout = ProtoFieldLayout::random(&mut r, fi);
			let decoded = ProtoFieldLayout::decode_directory(&layout.encode_directory())
				.expect("canonical directory must decode");
			assert_eq!(decoded.as_tags(), layout.as_tags());
		}
	}

	#[test]
	fn directory_rejects_duplicate_slot() {
		let mut tags = [0u8; LAYOUT_SLOTS];
		for field in ProtoFieldId::ALL {
			tags[field.id() as usize] = 9; // every field on the same slot
		}
		let mut bytes = Vec::new();
		for field in ProtoFieldId::ALL {
			bytes.push(field.id());
			bytes.push(tags[field.id() as usize]);
		}
		assert!(ProtoFieldLayout::decode_directory(&bytes).is_err());
	}

	#[test]
	fn directory_rejects_unknown_field_and_truncation() {
		assert!(ProtoFieldLayout::decode_directory(&[6, 10]).is_err(), "section 6 is not a field");
		assert!(ProtoFieldLayout::decode_directory(&[0]).is_err(), "odd length");
		assert!(ProtoFieldLayout::decode_directory(&[99, 10]).is_err(), "unknown id");
	}

	#[test]
	fn directory_rejects_missing_required_field() {
		let mut r = rng(3);
		let layout = ProtoFieldLayout::random(&mut r, 0);
		let full = layout.encode_directory();
		// Drop the Head pair (first two bytes).
		assert!(ProtoFieldLayout::decode_directory(&full[2..]).is_err());
	}

	#[test]
	fn layouts_differ_across_prototypes_and_seeds() {
		let mut r = rng(11);
		let layouts = program_layouts(&mut r, 8, true);
		let first = layouts[0].as_tags();
		assert!(
			layouts.iter().skip(1).any(|l| l.as_tags() != first),
			"per-prototype layouts must not all coincide"
		);
		let mut r2 = rng(12);
		let other = program_layouts(&mut r2, 8, true);
		assert_ne!(other[0].as_tags(), first, "seed change must change layout");
	}

	#[test]
	fn disabled_mode_shares_one_layout() {
		let mut r = rng(5);
		let layouts = program_layouts(&mut r, 6, false);
		assert!(layouts.iter().all(|l| l.as_tags() == layouts[0].as_tags()));
	}

	#[test]
	fn nested_prototypes_get_independent_layouts() {
		// Parent/child prototypes are just different fn indices; their
		// layouts must be independently valid and (here) distinct.
		let mut r = rng(99);
		let layouts = program_layouts(&mut r, 4, true);
		for (i, a) in layouts.iter().enumerate() {
			a.validate().unwrap();
			for b in layouts.iter().skip(i + 1) {
				assert_ne!(a.as_tags(), b.as_tags());
			}
		}
	}
}
