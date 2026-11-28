use crate::anti;
use crate::mangle;
use crate::minify;
use crate::parser;
use crate::printer;
use crate::rng::Rng;

const GUARD_PREAMBLE: &str = r###"local _guard = (function()
	local function abort()
		local al = 0
		local az = 1
		local zm = 65536
		while al < zm do
			al = al + 1
			az = (az * 80659 + 95050511 + al) % 268435456
		end
		local zf = false
		zf = zf[zf]
		local zn
		return zn[az]
	end

	local _raw_type = type
	local _raw_pcall = pcall
	if _raw_type(_raw_type) ~= "function" or _raw_type(_raw_pcall) ~= "function" then
		abort()
	end

	-- Bootstrap env (plaintext-surface repair): `getfenv`/`_G`/`type`/
	-- `pcall` are the guard's documented bootstrap anchors. Every other
	-- host name is resolved through this env table with sealed keys, so
	-- no bare API identifier is needed as a pick() fallback.
	local _genv
	if getfenv then
		local _okg, _g0 = _raw_pcall(getfenv, 0)
		if _okg and _raw_type(_g0) == "table" then
			_genv = _g0
		end
	end
	if _raw_type(_genv) ~= "table" then
		_genv = _G
	end

	local function pick(k, fb)
		if _raw_type(EV) == "table" then
			local ok, v = _raw_pcall(function()
				return EV[k]
			end)
			if not ok then
				return false
			end
			if v ~= nil then
				return v
			end
		end
		if _raw_type(_G) == "table" then
			local ok, v = _raw_pcall(function()
				return _G[k]
			end)
			if not ok then
				return false
			end
			if v ~= nil then
				return v
			end
		end
		if _raw_type(_genv) == "table" then
			local ok, v = _raw_pcall(function()
				return _genv[k]
			end)
			if not ok then
				return false
			end
			if v ~= nil then
				return v
			end
		end
		return fb
	end

	local type = pick("type", nil)
	local pcall = pick("pcall", nil)
	local xpcall = pick("xpcall", nil)
	local error = pick("error", nil)
	local rawget = pick("rawget", nil)
	local rawset = pick("rawset", nil)
	local getmetatable = pick("getmetatable", nil)
	local setmetatable = pick("setmetatable", nil)
	local tostring = pick("tostring", nil)
	local tonumber = pick("tonumber", nil)
	local selectFn = pick("select", nil)
	local print = pick("print", nil)
	local warn = pick("warn", nil)
	local newproxy = pick("newproxy", nil)
	local nextFn = pick("next", NXT)
	local osTable = pick("os", nil)
	local dbgTable = pick("debug", nil)
	local debugInfo
	if _raw_type(dbgTable) == "table" then
		local okd, di = _raw_pcall(function()
			return dbgTable["info"]
		end)
		if okd then
			debugInfo = di
		else
			debugInfo = false
		end
	end
	local unpackFn = pick("unpack", nil)
	local tblTable = pick("table", nil)
	local tableUnpackFn
	if _raw_type(tblTable) == "table" then
		local oku, tu = _raw_pcall(function()
			return tblTable["unpack"]
		end)
		if oku then
			tableUnpackFn = tu
		else
			tableUnpackFn = false
		end
	end

	local failed = false

	local function tripwire()
		failed = true
		abort()
	end

	local canaries = {}

	local env

"###;

const GUARD_EPILOGUE: &str = r###"
	if failed then
		abort()
	end

	return canaries
end)()"###;

fn assemble_guard(rng: &mut Rng) -> String {
	let mut order: Vec<usize> = (0..anti::STAGES.len()).collect();
	rng.shuffle(&mut order);
	let mut src = String::from(GUARD_PREAMBLE);
	for &i in &order {
		src.push_str(anti::STAGES[i].1);
		src.push('\n');
	}
	src.push('\n');
	for &i in &order {
		src.push('\t');
		src.push_str(anti::STAGES[i].2);
		src.push('\n');
	}
	src.push_str(GUARD_EPILOGUE);
	src
}


/// Replace readable guard literals with char-code expressions built by
/// `chars`. Dot/ctor metamethod forms are rewritten too, so a stage body
/// cannot leak `.__index` or `__tostring =` shapes. Both guard emission
/// paths (HBOOT embed and legacy prelude) share this one list.
fn seal_guard_text(src: &str, chars: &dyn Fn(&str) -> String) -> String {
	let mut out = src.to_string();
	for name in [
		"__tostring", "__metatable", "__concat", "__call", "__iter", "__index",
		"__newindex", "__len", "__mode", "__eq", "__lt", "__le", "__unm",
	] {
		let expr = chars(name);
		out = out
			.replace(&format!(".{}", name), &format!("[{}]", expr))
			.replace(&format!("{} =", name), &format!("[{}] =", expr))
			.replace(&format!("\"{}\"", name), &expr);
	}
	for text in [
		"type", "pcall", "xpcall", "error", "rawget", "rawset",
		"getmetatable", "setmetatable", "tostring", "tonumber", "select", "string",
		"gmatch", "unpack", "next", "table", "print", "warn", "newproxy", "debug",
		"info", "os", "clock", "getfenv", "_G", "function", "number",
		"boolean", "l", "s", "#", "[C]", ":(%d+):", "loadstring", "load",
		"tddxhpfcpi", "lsphhfstdm", "vymgglkhqr",
		// Phase 18 P1-3 string-cache canary stage literals.
		"char", "byte", "A", "Z", "abc",
	] {
		out = out.replace(&format!("\"{}\"", text), &chars(text));
	}
	out
}

pub fn hboot_guard_source(rng: &mut Rng) -> (String, String) {
	let chars = |s: &str| {
		format!(
			"CHAR({})",
			s.bytes().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
		)
	};
	let guard_src = seal_guard_text(&assemble_guard(rng), &chars);
	let mangled = match parser::parse(&guard_src, true).ok() {
		Some(mut block) => {
			let mut table = crate::symtab::resolve(&mut block);
			mangle::mangle(&mut table, rng);
			printer::print_chunk_luau(&table, &block)
		}
		None => guard_src.clone(),
	};
	let trimmed = mangled.trim_start();
	let holder = trimmed
		.strip_prefix("local ")
		.and_then(|r| r.split('=').next())
		.map(|s| s.trim().to_string())
		.unwrap_or_else(|| "_hp".to_string());
	(mangled, holder)
}

pub fn guard_prelude(rng: &mut Rng, luau: bool) -> String {
	// Seal the same name list as hboot_guard_source, using a locally
	// built char function (the prelude has no CHAR prim in scope).
	let chars = |s: &str| {
		format!(
			"_gc({})",
			s.bytes().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
		)
	};
	let sealed = seal_guard_text(&assemble_guard(rng), &chars);
	let guard_src = format!(
		"local EV, NXT = nil, nil\nlocal _gc = (\"\")[\"char\"]\n{}",
		sealed
	);
	let parsed = parser::parse(&guard_src, luau).ok();
	if let Some(mut block) = parsed {
		let mut table = crate::symtab::resolve(&mut block);
		mangle::mangle(&mut table, rng);
		let printed = if luau {
			printer::print_chunk_luau(&table, &block)
		} else {
			printer::print_chunk(&table, &block)
		};
		if let Ok(m) = minify::minify(&printed, luau) {
			return m.trim_end_matches('\n').to_string();
		}
	}
	guard_src
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hboot_guard_parses_and_leaves_no_plaintext_defense_words() {
		let mut rng = Rng::new(42);
		let (src, holder) = hboot_guard_source(&mut rng);
		assert!(!holder.is_empty());
		assert!(
			parser::parse(&src, true).is_ok(),
			"hboot_guard_source must produce valid Luau"
		);
		assert!(
			!src.contains('"'),
			"unhidden string literal leaked in hboot_guard_source: {}",
			src
		);
		for word in [
			"\"newproxy\"",
			"\"__tostring\"",
			"\"__metatable\"",
			"\"loadstring\"",
			"\"xpcall\"",
			"\"debug\"",
		] {
			assert!(
				!src.contains(word),
				"plaintext defense literal leaked in hboot_guard_source: {}",
				word
			);
		}
	}

	#[test]
	fn guard_prelude_parses_in_both_dialects() {
		let mut rng1 = Rng::new(7);
		let s_luau = guard_prelude(&mut rng1, true);
		assert!(parser::parse(&s_luau, true).is_ok());
		let mut rng2 = Rng::new(7);
		let s_51 = guard_prelude(&mut rng2, false);
		assert!(parser::parse(&s_51, false).is_ok());
	}
}
