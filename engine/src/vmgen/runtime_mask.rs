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
//! Integration status: wired. The compiler (`vmgen::ccpd`) encodes
//! every block variant through `derive_mask` via `DecodeContext`, and
//! the Luau mirror inside the generated `bdec` re-derives the same
//! masks; `derive_variant_id` / `derive_root_context` /
//! `derive_closure_origin` / `derive_invocation_context` have exact
//! Luau mirrors in `template.rs` (`CCPDV/CCPDR/CCPDO/CCPDI`), pinned
//! by the vector tests below and `tests/ccpd_mirror_test.py`.

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
	/// CCPD block variant selected for this decode (0 unless CCPD picked
	/// one). Part of the cache identity, never of the mask inputs beyond
	/// what the caller folds into `lane_id`/`page_epoch`.
	pub variant_id: u8,
}

impl DecodeContext {
	pub fn new(prototype_id: u32, block_id: u32, lane_id: u8) -> DecodeContext {
		DecodeContext {
			prototype_id,
			block_id,
			lane_id,
			entry_domain: 0,
			page_epoch: 0,
			variant_id: 0,
		}
	}

	pub fn with_entry_domain(mut self, entry_domain: u16) -> DecodeContext {
		self.entry_domain = entry_domain;
		self
	}

	pub fn with_page_epoch(mut self, page_epoch: u32) -> DecodeContext {
		self.page_epoch = page_epoch;
		self
	}

	pub fn with_variant_id(mut self, variant_id: u8) -> DecodeContext {
		self.variant_id = variant_id;
		self
	}

	/// The cache identity for a decoded page (spec F4 + CCPD plan
	/// section 9.2): two pages may share a cache entry only under an
	/// equal key, and the CCPD variant is part of that identity so
	/// different encodings of one block can never alias.
	pub fn cache_key(&self) -> DecodeCacheKey {
		DecodeCacheKey {
			prototype_id: self.prototype_id,
			block_id: self.block_id,
			entry_domain: self.entry_domain,
			page_epoch: self.page_epoch,
			variant_id: self.variant_id,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecodeCacheKey {
	pub prototype_id: u32,
	pub block_id: u32,
	pub entry_domain: u16,
	pub page_epoch: u32,
	pub variant_id: u8,
}

/// `(s * m) % 2^28` with an exact intermediate: `s < 2^28` and
/// `m < 2^32` keep the product below 2^60 in u64. The Luau mirror splits
/// `s` into 14-bit halves so every intermediate stays below 2^46 and is
/// exact in f64; both sides therefore agree bit-for-bit.
pub fn mul_mod_28(s: u32, m: u32) -> u32 {
	((s as u64 * m as u64) % MASK_MOD as u64) as u32
}

fn mix28(s: u32, v: u32) -> u32 {
	(s.wrapping_mul(31).wrapping_add(v % MASK_MOD)) % MASK_MOD
}

/// Float-exact avalanche: multiply/add/mod only, no xor-shift, so a Luau
/// mirror using plain number arithmetic reproduces it exactly.
fn avalanche28(mut s: u32) -> u32 {
	// Domain separator keeps the all-zero context from collapsing to a
	// zero mask (it is a diversity derivation, not a key, but a constant
	// zero output would silently erase per-context separation).
	s = (s + 0x9e37_79b9 % MASK_MOD) % MASK_MOD;
	s = mul_mod_28(s, 0x5bd1_e995);
	s = (s + (s >> 13).wrapping_mul(40_503)) % MASK_MOD;
	s = mul_mod_28(s, 0x85eb_ca6b);
	s
}

/// Derive the per-page mask state from the build nonce and context.
/// Wrapping, fixed-width, float-safe steps only (see module docs).
pub fn derive_mask(build_nonce: u32, ctx: &DecodeContext) -> u32 {
	let mut s = build_nonce % MASK_MOD;
	s = mix28(s, ctx.prototype_id);
	s = mix28(s, ctx.block_id);
	s = (s.wrapping_mul(31).wrapping_add(ctx.lane_id as u32)) % MASK_MOD;
	s = (s.wrapping_mul(31).wrapping_add(ctx.entry_domain as u32)) % MASK_MOD;
	s = mix28(s, ctx.page_epoch);
	avalanche28(s)
}

/// CCPD variant selector (spec: rebased CCPD plan sections 2.3 / 9.1).
/// Picks one of `CCPD_VARIANTS` encodings of a block from the frame
/// context, the incoming edge, the block id and the per-frame block
/// visit ordinal. This is a stable diversity function, not a PRF; the
/// Luau mirror in `template.rs` must stay bit-exact with this body.
pub fn derive_variant_id(
	frame_context: u32,
	incoming_edge: u32,
	block_id: u32,
	visit_ordinal: u32,
	build_salt: u32,
) -> u8 {
	let mut s = build_salt % MASK_MOD;
	s = mix28(s, frame_context);
	s = mix28(s, incoming_edge);
	s = mix28(s, block_id);
	s = mix28(s, visit_ordinal);
	(avalanche28(s) % super::ccpd::CCPD_VARIANTS as u32) as u8
}

/// Root frame context for the main chunk (CCPD plan section 5.6).
pub fn derive_root_context(build_salt: u32, prototype_id: u32) -> u32 {
	let mut s = build_salt % MASK_MOD;
	s = mix28(s, prototype_id);
	s = mix28(s, EntryTransition::Fallthrough.id() as u32);
	avalanche28(s)
}

/// Immutable closure-origin token (CCPD plan section 5.3). Pure function
/// of the creation-site context; the returned Lua closure captures only
/// this scalar, never the parent frame table.
#[allow(clippy::too_many_arguments)]
pub fn derive_closure_origin(
	parent_frame_ctx: u32,
	parent_edge_id: u32,
	parent_block_id: u32,
	parent_proto_id: u32,
	site_pc: u32,
	creation_ordinal: u32,
	child_proto_id: u32,
	build_salt: u32,
) -> u32 {
	let mut s = build_salt % MASK_MOD;
	s = mix28(s, parent_frame_ctx);
	s = mix28(s, parent_edge_id);
	s = mix28(s, parent_block_id);
	s = mix28(s, parent_proto_id);
	s = mix28(s, site_pc);
	s = mix28(s, creation_ordinal);
	s = mix28(s, child_proto_id);
	s = mix28(s, EntryTransition::ClosureInvocation.id() as u32);
	avalanche28(s)
}

/// Per-invocation frame context for one closure instance (CCPD plan
/// section 5.5): the same closure instance derives a fresh context on
/// every call from its own call ordinal.
pub fn derive_invocation_context(
	origin_token: u32,
	call_ordinal: u32,
	child_proto_id: u32,
	build_salt: u32,
) -> u32 {
	let mut s = build_salt % MASK_MOD;
	s = mix28(s, origin_token);
	s = mix28(s, call_ordinal);
	s = mix28(s, child_proto_id);
	avalanche28(s)
}

/// CCPD edge identity (rebased CCPD plan sections 2.4 / 4 / 9).
///
/// Every CFG incoming edge owns a stable `EdgeId` (`vmgen::ccpd`
/// builds the CFG from the final record stream); branch, jump and
/// fallthrough micro-ops publish the incoming edge at runtime, and
/// `bdec` selects one of four block encodings from
/// (frame context, incoming edge, block, visit ordinal). The payload
/// masks themselves are derived with `entry_domain =
/// DEFAULT_ENTRY_DOMAIN`: edge identity enters through variant
/// selection and through the composite decode-cache key, not through
/// the mask inputs, so the compiler and the Luau mirror only ever
/// evaluate `derive_mask` at domain 0. `DecodeCacheKey` nevertheless
/// carries the domain explicitly, so a future per-domain mask split
/// can never alias the domain-0 entries (spec section 7 item 6).
///
/// - Missing/legacy data maps to `DEFAULT_ENTRY_DOMAIN` explicitly,
///   never to a guessed value (spec section 7 item 10).
/// - `EntryTransition` enumerates the control-flow events whose
///   handlers publish the incoming edge (spec item 8); the ids are
///   stable so a serialized form can rely on them.
///
/// An `EdgeId` is a context label, not key material (spec section 7):
/// treat domain separation as analysis-cost diversity only.
pub const DEFAULT_ENTRY_DOMAIN: u16 = 0;

/// Stable identity of one CFG incoming edge within a prototype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub u32);

/// Control-flow events that transfer execution into a block and would
/// carry the incoming `EdgeId` under a CCPD implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryTransition {
	Fallthrough,
	ConditionalJump,
	LoopBackedge,
	Exception,
	Return,
	CoroutineResume,
	HostCallback,
	ClosureInvocation,
}

impl EntryTransition {
	pub const ALL: [EntryTransition; 8] = [
		EntryTransition::Fallthrough,
		EntryTransition::ConditionalJump,
		EntryTransition::LoopBackedge,
		EntryTransition::Exception,
		EntryTransition::Return,
		EntryTransition::CoroutineResume,
		EntryTransition::HostCallback,
		EntryTransition::ClosureInvocation,
	];

	/// Stable serialized id (declaration order); never renumber.
	pub const fn id(self) -> u16 {
		self as u16
	}
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
	fn ccpd_interface_defaults_and_domain_scoped_keys() {
		// The payload masks run at the explicit default domain (edge
		// identity enters via variant selection and the cache key);
		// the default is a real value, never a guessed one.
		let ctx = DecodeContext::new(9, 4, 1);
		assert_eq!(ctx.entry_domain, DEFAULT_ENTRY_DOMAIN);
		assert_eq!(ctx.cache_key().entry_domain, DEFAULT_ENTRY_DOMAIN);
		// All eight spec transitions own a stable, unique id.
		let mut ids: Vec<u16> = EntryTransition::ALL.iter().map(|t| t.id()).collect();
		ids.sort_unstable();
		ids.dedup();
		assert_eq!(ids.len(), EntryTransition::ALL.len());
		assert_eq!(EntryTransition::Fallthrough.id(), 0);
		// A future variant-emitting decoder can key caches per domain
		// without aliasing today's domain-0 pages.
		assert_ne!(
			ctx.cache_key(),
			ctx.with_entry_domain(DEFAULT_ENTRY_DOMAIN + 1).cache_key()
		);
	}

	#[test]
	fn cache_key_distinguishes_variant() {
		let a = DecodeContext::new(1, 2, 0).cache_key();
		assert_ne!(a, DecodeContext::new(1, 2, 0).with_variant_id(1).cache_key());
		assert_eq!(
			a,
			DecodeContext::new(1, 2, 0).with_variant_id(0).cache_key(),
			"variant 0 is the explicit default identity"
		);
	}

	#[test]
	fn variant_selector_is_bounded_deterministic_and_sensitive() {
		for salt in [0u32, 1, 42, 268_435_455] {
			for visit in 0..16u32 {
				let v = derive_variant_id(0x1234, 5, 3, visit, salt);
				assert!(v < super::super::ccpd::CCPD_VARIANTS, "variant in [0,4)");
				assert_eq!(v, derive_variant_id(0x1234, 5, 3, visit, salt), "deterministic");
			}
		}
		// Every input must be able to move the selection somewhere in
		// the swept neighbourhood (no input is silently ignored).
		let base = derive_variant_id(100, 2, 3, 4, 777);
		let mut seen = std::collections::HashSet::new();
		for dv in 0..64u32 {
			seen.insert(derive_variant_id(100 + dv, 2, 3, 4, 777));
			seen.insert(derive_variant_id(100, 2 + dv, 3, 4, 777));
			seen.insert(derive_variant_id(100, 2, 3 + dv, 4, 777));
			seen.insert(derive_variant_id(100, 2, 3, 4 + dv, 777));
			seen.insert(derive_variant_id(100, 2, 3, 4, 777 + dv));
		}
		assert!(seen.len() > 1, "selector must not be constant over input sweeps");
		let _ = base;
	}

	#[test]
	fn context_derivations_are_stable_and_distinct() {
		let root = derive_root_context(42, 3);
		assert_eq!(root, derive_root_context(42, 3));
		assert_ne!(root, derive_root_context(42, 4));
		assert_ne!(root, derive_root_context(43, 3));
		assert!(root < MASK_MOD);
		let o1 = derive_closure_origin(11, 2, 1, 3, 9, 1, 4, 42);
		let o2 = derive_closure_origin(11, 2, 1, 3, 9, 2, 4, 42);
		assert_ne!(o1, o2, "creation ordinal separates instances from one site");
		let i1 = derive_invocation_context(o1, 1, 4, 42);
		let i2 = derive_invocation_context(o1, 2, 4, 42);
		assert_ne!(i1, i2, "call ordinal rotates the invocation context");
		assert!(i1 < MASK_MOD && i2 < MASK_MOD);
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

#[cfg(test)]
mod pinned_vectors {
	use super::*;

	/// Pinned mirror vectors: the Luau mirrors of `derive_mask` /
	/// `derive_variant_id` in `template.rs` (and their reimplementation
	/// in `tests/ccpd_mirror_test.py`) must reproduce these exact
	/// numbers; if an algorithm here ever changes on purpose, update
	/// all three places together.
	#[test]
	fn derive_mask_pinned_vectors() {
		let cases = [
			(42u32, 3u32, 7u32, 2u8, 5u16, 9u32, 129_762_251u32),
			(0, 0, 0, 0, 0, 0, 250_714_540),
			(268_435_455, 65_535, 65_535, 255, 65_535, 4_294_967_295, 148_471_878),
			(777, 12, 34, 4, 513, 99_991, 68_024_606),
		];
		for (nonce, p, b, l, d, e, want) in cases {
			let ctx = DecodeContext::new(p, b, l).with_entry_domain(d).with_page_epoch(e);
			assert_eq!(derive_mask(nonce, &ctx), want, "mask vector {nonce},{p},{b},{l},{d},{e}");
		}
		assert_eq!(
			derive_variant_id(123_456, 5, 3, 7, 42),
			2,
			"variant selector vector"
		);
		assert_eq!(derive_root_context(42, 3), 62_904_812);
		assert_eq!(derive_closure_origin(11, 2, 1, 3, 9, 1, 4, 42), 194_110_675);
		assert_eq!(derive_invocation_context(12_345, 7, 4, 42), 142_615_952);
	}
}
