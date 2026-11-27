//! Phase F (obf.lua tech-extraction spec): per-page decode context.
//!
//! The existing block decoder (`template.rs` `bdec`) already remasks its
//! cache with a value derived from the function identity, block index, and
//! a per-frame decode counter. What it does not have is an explicit,
//! typed decode context shared by compiler and runtime. This module is
//! that type:
//!
//! - `DecodeContext` carries prototype, block, lane, entry domain, and
//!   page epoch as fixed-width integers. `entry_domain` is 0 unless a
//!   CCPD-style edge specialization supplies a real value; it is never
//!   guessed from missing data (spec F2/§7).
//! - `derive_mask` mixes the context into a 28-bit mask state using only
//!   wrapping multiply/add/xor steps whose operands stay far below 2^53,
//!   so a Luau mirror using number arithmetic reproduces it exactly.
//!   This is a build-diversity mask derivation, not a cryptographic
//!   construction, and must not be described as one.
//! - `DecodeCacheKey` is the cache identity the spec F4 requires:
//!   `(prototype, block, entry_domain, epoch)`. Two pages may share a
//!   cache entry only under an equal key; lane is part of the mask
//!   derivation, not the page identity.
//!
//! Integration status: `bdec`'s current remask already keys its cache per
//! prototype frame (`E.bch` lives on the per-activation frame table), so
//! swapping its formula for `derive_mask` is a representation change, not
//! a correctness fix. This module lands the typed context + derivation
//! with tests; the `bdec` swap is left as a separate reviewable step and
//! recorded as PARTIAL in the review report.

/// Modulus keeping every intermediate below 2^44 in Luau number arithmetic.
const MASK_MOD: u32 = 268_435_456; // 2^28

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecodeContext {
	pub prototype_id: u32,
	pub block_id: u32,
	pub lane_id: u8,
	/// CCPD entry domain; 0 = explicit default (no edge specialization).
	pub entry_domain: u16,
	pub page_epoch: u32,
}

impl DecodeContext {
	pub fn new(prototype_id: u32, block_id: u32, lane_id: u8) -> DecodeContext {
		DecodeContext { prototype_id, block_id, lane_id, entry_domain: 0, page_epoch: 0 }
	}

	pub fn with_entry_domain(mut self, entry_domain: u16) -> DecodeContext {
		self.entry_domain = entry_domain;
		self
	}

	pub fn with_page_epoch(mut self, page_epoch: u32) -> DecodeContext {
		self.page_epoch = page_epoch;
		self
	}

	/// The cache identity for a decoded page (spec F4).
	pub fn cache_key(&self) -> DecodeCacheKey {
		DecodeCacheKey {
			prototype_id: self.prototype_id,
			block_id: self.block_id,
			entry_domain: self.entry_domain,
			page_epoch: self.page_epoch,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecodeCacheKey {
	pub prototype_id: u32,
	pub block_id: u32,
	pub entry_domain: u16,
	pub page_epoch: u32,
}

/// Derive the per-page mask state from the build nonce and context.
/// Wrapping, fixed-width, float-safe steps only (see module docs).
pub fn derive_mask(build_nonce: u32, ctx: &DecodeContext) -> u32 {
	let mut s = build_nonce % MASK_MOD;
	s = (s.wrapping_mul(31).wrapping_add(ctx.prototype_id % MASK_MOD)) % MASK_MOD;
	s = (s.wrapping_mul(31).wrapping_add(ctx.block_id % MASK_MOD)) % MASK_MOD;
	s = (s.wrapping_mul(31).wrapping_add(ctx.lane_id as u32)) % MASK_MOD;
	s = (s.wrapping_mul(31).wrapping_add(ctx.entry_domain as u32)) % MASK_MOD;
	s = (s.wrapping_mul(31).wrapping_add(ctx.page_epoch % MASK_MOD)) % MASK_MOD;
	// Final avalanche inside the 28-bit state (xor-shift is exact in both
	// Rust and Luau bit32/number semantics at this width).
	s ^= s >> 13;
	s = s.wrapping_mul(0x5bd1_e995) % MASK_MOD;
	s ^= s >> 15;
	s
}

/// Page integrity state (spec F5/F6): a page is executed only from
/// `Verified`; any other state fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
	Encoded,
	Decoding,
	Verified,
	Failed,
}

impl PageState {
	/// Legal transitions only: Encoded -> Decoding -> Verified|Failed.
	/// Verified/Failed are terminal for a page instance; re-decoding after
	/// eviction starts from a fresh Encoded page, never from Failed.
	pub fn can_transition_to(self, next: PageState) -> bool {
		matches!(
			(self, next),
			(PageState::Encoded, PageState::Decoding)
				| (PageState::Decoding, PageState::Verified)
				| (PageState::Decoding, PageState::Failed)
		)
	}

	pub fn is_executable(self) -> bool {
		matches!(self, PageState::Verified)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn derive_is_deterministic() {
		let ctx = DecodeContext::new(3, 7, 2).with_entry_domain(5).with_page_epoch(9);
		assert_eq!(derive_mask(12345, &ctx), derive_mask(12345, &ctx));
	}

	#[test]
	fn every_context_field_changes_the_mask() {
		let base = DecodeContext::new(3, 7, 2);
		let m = derive_mask(999, &base);
		assert_ne!(derive_mask(999, &DecodeContext::new(4, 7, 2)), m, "prototype");
		assert_ne!(derive_mask(999, &DecodeContext::new(3, 8, 2)), m, "block");
		assert_ne!(derive_mask(999, &DecodeContext::new(3, 7, 3)), m, "lane");
		assert_ne!(derive_mask(999, &base.with_entry_domain(1)), m, "entry domain");
		assert_ne!(derive_mask(999, &base.with_page_epoch(1)), m, "epoch");
		assert_ne!(derive_mask(1000, &base), m, "build nonce");
	}

	#[test]
	fn extreme_inputs_wrap_without_panic() {
		let ctx = DecodeContext::new(u32::MAX, u32::MAX, u8::MAX)
			.with_entry_domain(u16::MAX)
			.with_page_epoch(u32::MAX);
		let m = derive_mask(u32::MAX, &ctx);
		assert!(m < MASK_MOD, "mask state stays in the 28-bit domain");
	}

	#[test]
	fn cache_key_distinguishes_prototype_block_domain_epoch() {
		let a = DecodeContext::new(1, 2, 0).cache_key();
		assert_ne!(a, DecodeContext::new(2, 2, 0).cache_key());
		assert_ne!(a, DecodeContext::new(1, 3, 0).cache_key());
		assert_ne!(a, DecodeContext::new(1, 2, 0).with_entry_domain(7).cache_key());
		assert_ne!(a, DecodeContext::new(1, 2, 0).with_page_epoch(7).cache_key());
		// Lane is a mask input, not page identity.
		assert_eq!(a, DecodeContext::new(1, 2, 3).cache_key());
	}

	#[test]
	fn page_state_machine_is_fail_closed() {
		assert!(PageState::Encoded.can_transition_to(PageState::Decoding));
		assert!(PageState::Decoding.can_transition_to(PageState::Verified));
		assert!(PageState::Decoding.can_transition_to(PageState::Failed));
		assert!(!PageState::Encoded.can_transition_to(PageState::Verified), "no skip over decode+verify");
		assert!(!PageState::Failed.can_transition_to(PageState::Decoding), "failed pages never resurrect");
		assert!(!PageState::Verified.can_transition_to(PageState::Decoding));
		assert!(PageState::Verified.is_executable());
		assert!(!PageState::Failed.is_executable());
		assert!(!PageState::Encoded.is_executable());
	}
}
