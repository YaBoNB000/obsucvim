//! Compile-time gate for white-box audit and debug environment hooks.
//!
//! Sensitive export / dump hooks (`OBSUCVIM_KEY_MANIFEST`, `OBSUCVIM_VM_RAW`,
//! `OBSUCVIM_VM_TSRC`, `OBSUCVIM_VM_DBG`, `OBSUCVIM_HBOOT_DBG`, `OBSUCVIM_NOISE_AUDIT`,
//! `OBSUCVIM_PROFILE_AUDIT`, etc.) are ONLY compiled in debug builds, `cargo test`,
//! or explicit `--features audit-hooks` builds. Production `cargo build --release`
//! compiles them to constant `None` / `false` so neither key material nor internal
//! template source can be exfiltrated via process environment variables.

#[inline]
pub fn audit_var(name: &str) -> Option<String> {
	#[cfg(any(debug_assertions, test, feature = "audit-hooks"))]
	{
		std::env::var(name).ok()
	}
	#[cfg(not(any(debug_assertions, test, feature = "audit-hooks")))]
	{
		let _ = name;
		None
	}
}

#[inline]
pub fn audit_enabled(name: &str) -> bool {
	audit_var(name).is_some()
}
