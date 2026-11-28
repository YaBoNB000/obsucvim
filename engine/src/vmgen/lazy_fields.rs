//! Phase C (obf.lua tech-extraction spec): lazy prototype-field resolution.
//!
//! Prototype metadata (constants, child prototypes, upvalue descriptors)
//! does not have to be materialized when a prototype is first built: a
//! field can stay an encoded descriptor until something actually reads it,
//! then decode once and cache. This module is the compiler-side model of
//! that mechanism:
//!
//! - `LazyFieldPayload` is the descriptor classification. These names live
//!   only in the compiler; the emitted artifact carries numeric slot ids
//!   from `proto_layout`, never these identifiers.
//! - `LazyFieldStatus` tracks each field independently through
//!   Unresolved -> Resolving -> Resolved / Failed.
//! - `LazyResolver` is the reference semantics the Luau emitter mirrors:
//!   first read decodes, later reads hit the cache, `nil` results are
//!   cached with sentinel semantics (a resolved-nil field is *not*
//!   re-decoded), recursive resolution of the same field is a deterministic
//!   error, a failed decode stays failed with a stable error, and caches
//!   are isolated per prototype.
//!
//! Runtime-integration status (honest scope): the current template reader
//! already decodes the constants section in a second pass after locating
//! it (`template.rs` parse stage 2), and child prototypes are built by
//! `makefn` on closure creation. Replacing the eager constants table with
//! a proxy resolver in the emitted runtime is *not* done in this change;
//! this module pins the semantics and emits the resolver shape so the
//! runtime swap can be reviewed against a tested reference.

use std::collections::HashMap;

/// Descriptor classification for a not-yet-materialized prototype field.
/// Compiler-side only; never serialized by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LazyFieldPayload {
	EncodedConstant { index: u32 },
	EncodedChildPrototype { index: u32 },
	EncodedUpvalueDescriptor { index: u32 },
	EncodedLineInfo { index: u32 },
	EncodedMetadata { index: u32 },
}

/// A materialized field value. `Nil` is a real cached value: paired with
/// `LazyFieldStatus::Resolved` it is the sentinel-cached nil the spec
/// requires (the cache records *that* the field resolved, so nil is not
/// confused with "not yet decoded").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LazyValue {
	Nil,
	Int(i64),
	Text(Vec<u8>),
	ChildPrototype(u32),
	UpvalueDescriptor(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LazyFieldStatus {
	Unresolved,
	Resolving,
	Resolved,
	Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldEntry {
	payload: LazyFieldPayload,
	status: LazyFieldStatus,
	value: Option<LazyValue>,
	error: Option<String>,
	decode_count: usize,
}

/// Reference resolver: one instance per build, keyed by
/// `(prototype, field)` so caches can never leak across prototypes.
pub struct LazyResolver {
	entries: HashMap<(u32, u32), FieldEntry>,
}

impl LazyResolver {
	pub fn new() -> Self {
		LazyResolver { entries: HashMap::new() }
	}

	/// Register a field descriptor in the Unresolved state.
	pub fn register(&mut self, proto: u32, field: u32, payload: LazyFieldPayload) {
		self.entries.insert(
			(proto, field),
			FieldEntry { payload, status: LazyFieldStatus::Unresolved, value: None, error: None, decode_count: 0 },
		);
	}

	pub fn status(&self, proto: u32, field: u32) -> Option<LazyFieldStatus> {
		self.entries.get(&(proto, field)).map(|e| e.status)
	}

	pub fn decode_count(&self, proto: u32, field: u32) -> usize {
		self.entries.get(&(proto, field)).map(|e| e.decode_count).unwrap_or(0)
	}

	/// Resolve a field, decoding on first access only.
	///
	/// - Resolved: return the cached value (including cached nil).
	/// - Resolving: the field is being decoded further up the stack;
	///   recursive access is a deterministic error, never infinite recursion.
	/// - Failed: return the original stable error; never retry silently.
	/// - Unresolved: run `decode`, cache the outcome (value or failure).
	pub fn resolve<D>(&mut self, proto: u32, field: u32, decode: &mut D) -> Result<LazyValue, String>
	where
		D: FnMut(LazyFieldPayload) -> Result<LazyValue, String>,
	{
		let key = (proto, field);
		let entry = self
			.entries
			.get(&key)
			.ok_or_else(|| format!("lazy field: unregistered field {field} of prototype {proto}"))?;
		match entry.status {
			LazyFieldStatus::Resolved => {
				// invariant: Resolved always carries a value slot; a cached
				// nil is `Some(LazyValue::Nil)`, distinct from "no value".
				return Ok(entry.value.clone().expect("resolved field carries a value"));
			}
			LazyFieldStatus::Resolving => {
				return Err(format!(
					"lazy field: recursive resolution of field {field} of prototype {proto}"
				));
			}
			LazyFieldStatus::Failed => {
				return Err(entry
					.error
					.clone()
					.unwrap_or_else(|| "lazy field: resolution failed".to_string()));
			}
			LazyFieldStatus::Unresolved => {}
		}
		let payload = entry.payload;
		if let Some(e) = self.entries.get_mut(&key) {
			e.status = LazyFieldStatus::Resolving;
			e.decode_count += 1;
		}
		let outcome = decode(payload);
		// invariant: the entry was inserted by `register` and only this
		// function mutates statuses, so the key is present here.
		let entry = self.entries.get_mut(&key).expect("entry registered above");
		match outcome {
			Ok(value) => {
				entry.status = LazyFieldStatus::Resolved;
				entry.value = Some(value.clone());
				Ok(value)
			}
			Err(err) => {
				entry.status = LazyFieldStatus::Failed;
				entry.error = Some(err.clone());
				Err(err)
			}
		}
	}
}

impl Default for LazyResolver {
	fn default() -> Self {
		Self::new()
	}
}

/// Emit the Luau resolver factory. The generated code mirrors
/// `LazyResolver` semantics: numeric state codes (1=unresolved,
/// 2=resolving, 3=resolved, 4=failed) held in a per-prototype state table,
/// a unique sentinel table for cached nil, caching in a hidden values table,
/// and stable errors for recursion and prior failure. `decode_field` is
/// supplied by the runtime template and receives the numeric descriptor.
///
/// This emitter is currently exercised by tests; wiring it into
/// `template.rs`'s parse output is a separate, reviewable step (see the
/// module-level scope note).
pub fn emit_luau_resolver(fn_name: &str) -> String {
	format!(
		r#"local {fn_name}_NIL = {{}}
local function {fn_name}(descriptors, states, decode_field)
  local values = {{}}
  local errs = {{}}
  local proxy = {{}}
  return setmetatable(proxy, {{
    __index = function(self, key)
      local st = states[key]
      if st == 3 then
        local v = values[key]
        if v == {fn_name}_NIL then return nil end
        return v
      end
      if st == 2 then error(2, 0) end
      if st == 4 then error(errs[key] or "lazy field: resolution failed", 0) end
      local descriptor = descriptors[key]
      if descriptor == nil then return nil end
      states[key] = 2
      local ok, value = pcall(decode_field, descriptor)
      if not ok then
        states[key] = 4
        errs[key] = value
        error(value, 0)
      end
      states[key] = 3
      if value == nil then
        values[key] = {fn_name}_NIL
        return nil
      end
      values[key] = value
      return value
    end,
  }})
end
"#
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn resolver_with(proto: u32, field: u32, payload: LazyFieldPayload) -> LazyResolver {
		let mut r = LazyResolver::new();
		r.register(proto, field, payload);
		r
	}

	#[test]
	fn first_read_decodes_second_read_hits_cache() {
		let mut r = resolver_with(0, 1, LazyFieldPayload::EncodedConstant { index: 0 });
		let mut calls = 0;
		let mut decode = |_| {
			calls += 1;
			Ok(LazyValue::Int(7))
		};
		assert_eq!(r.resolve(0, 1, &mut decode).unwrap(), LazyValue::Int(7));
		assert_eq!(r.resolve(0, 1, &mut decode).unwrap(), LazyValue::Int(7));
		assert_eq!(calls, 1, "second read must not re-decode");
		assert_eq!(r.status(0, 1), Some(LazyFieldStatus::Resolved));
	}

	#[test]
	fn nil_is_cached_with_sentinel_semantics() {
		let mut r = resolver_with(0, 2, LazyFieldPayload::EncodedConstant { index: 1 });
		let mut calls = 0;
		let mut decode = |_| {
			calls += 1;
			Ok(LazyValue::Nil)
		};
		assert_eq!(r.resolve(0, 2, &mut decode).unwrap(), LazyValue::Nil);
		assert_eq!(r.resolve(0, 2, &mut decode).unwrap(), LazyValue::Nil);
		assert_eq!(calls, 1, "cached nil must not re-decode");
	}

	#[test]
	fn recursive_resolution_is_a_deterministic_error() {
		let mut r = resolver_with(0, 3, LazyFieldPayload::EncodedChildPrototype { index: 0 });
		// Drive the Resolving branch directly (a decode closure cannot
		// re-borrow the resolver in safe Rust): after a successful resolve,
		// force the state back to Resolving and confirm rejection.
		let mut decode = |_| Ok(LazyValue::ChildPrototype(0));
		assert_eq!(r.resolve(0, 3, &mut decode).unwrap(), LazyValue::ChildPrototype(0));
		// Status machine: force Resolving and confirm resolve() rejects.
		r.entries.get_mut(&(0, 3)).unwrap().status = LazyFieldStatus::Resolving;
		let mut decode2 = |_| Ok(LazyValue::Nil);
		let err = r.resolve(0, 3, &mut decode2).unwrap_err();
		assert!(err.contains("recursive"), "unexpected error: {err}");
	}

	#[test]
	fn failure_is_stable_and_never_returns_partial() {
		let mut r = resolver_with(1, 0, LazyFieldPayload::EncodedMetadata { index: 0 });
		let mut calls = 0;
		let mut decode = |_| {
			calls += 1;
			Err("corrupt descriptor".to_string())
		};
		let e1 = r.resolve(1, 0, &mut decode).unwrap_err();
		let e2 = r.resolve(1, 0, &mut decode).unwrap_err();
		assert_eq!(e1, e2, "failure error must be stable");
		assert_eq!(calls, 1, "failed field must not be retried silently");
		assert_eq!(r.status(1, 0), Some(LazyFieldStatus::Failed));
	}

	#[test]
	fn caches_are_isolated_per_prototype() {
		let mut r = LazyResolver::new();
		r.register(0, 1, LazyFieldPayload::EncodedConstant { index: 0 });
		r.register(1, 1, LazyFieldPayload::EncodedConstant { index: 0 });
		let mut decode = |_| Ok(LazyValue::Int(1));
		r.resolve(0, 1, &mut decode).unwrap();
		assert_eq!(r.status(0, 1), Some(LazyFieldStatus::Resolved));
		assert_eq!(r.status(1, 1), Some(LazyFieldStatus::Unresolved), "other prototype untouched");
		assert_eq!(r.decode_count(1, 1), 0);
	}

	#[test]
	fn child_prototype_identity_is_stable() {
		let mut r = resolver_with(0, 5, LazyFieldPayload::EncodedChildPrototype { index: 3 });
		let mut decode = |_| Ok(LazyValue::ChildPrototype(3));
		let a = r.resolve(0, 5, &mut decode).unwrap();
		let b = r.resolve(0, 5, &mut decode).unwrap();
		assert_eq!(a, b, "same descriptor identity on every read");
		assert_eq!(r.decode_count(0, 5), 1);
	}

	#[test]
	fn unregistered_field_is_an_error_not_nil() {
		let mut r = LazyResolver::new();
		let mut decode = |_| Ok(LazyValue::Nil);
		assert!(r.resolve(9, 9, &mut decode).is_err());
	}

	#[test]
	fn emitted_resolver_has_sentinel_states_and_rawset_cache() {
		let lua = emit_luau_resolver("make_lazy_proto");
		assert!(lua.contains("_NIL = {}"), "nil sentinel table");
		assert!(lua.contains("values[key] = make_lazy_proto_NIL"), "nil cached via sentinel in hidden table");
		assert!(!lua.contains("rawset(self, key"), "sentinel must not be stored on the proxy itself");
		assert!(lua.contains("states[key] = 2"), "resolving state");
		assert!(lua.contains("states[key] = 3"), "resolved state");
		assert!(lua.contains("states[key] = 4"), "failed state");
		assert!(lua.contains("recursive resolution"), "recursion error");
		assert!(lua.contains("__index = function(self, key)"), "proxy shape");
	}
}
