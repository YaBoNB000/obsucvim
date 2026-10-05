//! Anti-debug / environment-integrity guard (user-provided design,
//! 2026-08-29): injected as a prelude in front of the obfuscated
//! payload on the standard pipeline; v15 injects a CHAR-encoded copy at
//! the head of the FC entry machine (keeps the 3-line form).
//!
//! Suggestion 2 (2026-09-05): the guard's five check stages live in
//! `src/anti/mod.rs` as independent stage modules (anti1/anti3 style).
//! `assemble_guard` composes all five stages in a RANDOM ORDER per
//! build (all stages always present, only ordering varies), so the
//! anti-debug sequence differs build-to-build and a fixed single-step
//! bypass cannot be reused.
//!
//! The guard is a self-contained IIFE assigned to a fresh local. It
//! verifies the runtime environment before the payload executes:
//!
//!   - core globals are real functions (type/pcall/xpcall/error/raw*)
//!   - getfenv/_G environment sanity + print/warn identity in env
//!   - table write/read roundtrip with a negative key
//!   - pcall(error) must FAIL (error must raise)
//!   - debug.info line probe: a probe function's reported line must
//!     equal the line extracted from its own raised error message;
//!     debug info source of `error` must stay "[C]"
//!   - loader hook integrity (suggestion 3): loadstring / load present
//!     but non-native (debug source ~= "[C]") => hooked; re-checked
//!     mid-staging in the verify handler too
//!   - newproxy metatable canaries (__tostring/__concat/__call
//!     tripwires) + table canaries with locked __metatable
//!   - unpack({}, 0, 64) must succeed
//!
//! On any failed check `abort()` hangs/poisons the environment
//! (`_ENV` swap + nil-index) instead of revealing the payload.
//!
//! The prelude goes through mangle + minify so its internals carry
//! build-random names; globals it references are untouched by mangle.

use crate::anti;
use crate::mangle;
use crate::minify;
use crate::parser;
use crate::printer;
use crate::rng::Rng;

/// The guard IIFE (verbatim user design; the trailing `print(255)`
/// test harness is NOT included — only `local _guard = (...)`).
const GUARD_PREAMBLE: &str = r###"local _guard = (function()
	-- ㊶: EV = 可见引导以 getfenv(1) 克隆的应用环境（setfenv 影子环境
	-- 的值也一并入克隆）。守卫位于 HBOOT 掩码层（标准环境加载），必须
	-- 经 EV 才能看到攻击者的影子环境替换——EV 缺省(旧管线)时退回裸全局。
	-- 字符串索引（本段在掩码层内，名字不可见）。
	local type = (EV and EV["type"]) or type
	local pcall = (EV and EV["pcall"]) or pcall
	local xpcall = (EV and EV["xpcall"]) or xpcall
	local error = (EV and EV["error"]) or error
	local rawget = (EV and EV["rawget"]) or rawget
	local rawset = (EV and EV["rawset"]) or rawset
	local getmetatable = (EV and EV["getmetatable"]) or getmetatable
	local setmetatable = (EV and EV["setmetatable"]) or setmetatable
	local tostring = (EV and EV["tostring"]) or tostring
	local tonumber = (EV and EV["tonumber"]) or tonumber
	local gmatch = (EV and EV["string"] and EV["string"]["gmatch"]) or (_G["string"] and _G["string"]["gmatch"])
	local unpack = (EV and EV["unpack"]) or _G["unpack"] or (_G["table"] and _G["table"]["unpack"])
	local print = (EV and EV["print"]) or print
	local warn = (EV and EV["warn"]) or (_G and _G["warn"])
	local newproxy = (EV and EV["newproxy"]) or newproxy
	local debugInfo = (EV and EV["debug"] and EV["debug"]["info"]) or (_G["debug"] and _G["debug"]["info"])
	local failed = false

	local function abort()
		-- ㊶: 纯挂起形态（_ENV 投毒在只读全局环境会抛错破坏挂起语义；
		-- 挂起本身已是终止——守卫现在位于 HBOOT 掩码层，形态不可见）。
		local al = 0
		local zm
		repeat
			al = al + 1
		until al == zm

		return (nil)[1]
	end

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

/// Assemble the guard IIFE from the anti/ stage modules (suggestion 2,
/// anti-folder). All five stages are always present (strong baseline
/// protection); only their ORDER is randomized per build, so the
/// anti-debug sequence differs build-to-build and a fixed single-step
/// bypass cannot be reused.
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


/// H4-3 静态 HBOOT 嵌入版守卫。API/类型/元方法字符串在进入 parser 前
/// 改写为 HBOOT `CHAR(...)` 参数构造式；局部名继续独立随机化，因此正式
/// 产物保持保护词零明文，同时不重建或加载任何 Lua 源码。返回（守卫语句、
/// 持有局部名）：IIFE 产生的 canaries 经 HBOOT 第 6 个密封值递出，并存入
/// 运行期状态的高位槽。
pub fn hboot_guard_source(rng: &mut Rng) -> (String, String) {
	let mut guard_src = assemble_guard(rng);
	// H4-3: HBOOT is static outer code, so the old source-mask no longer
	// hides guard anchors. Route sensitive environment/metamethod names
	// through the HBOOT CHAR parameter before parsing/mangling. Local
	// `newproxy` is still mangled; only its global fallback is removed because
	// EV is always supplied by the v15 bootstrap.
	let chars = |s: &str| {
		format!(
			"CHAR({})",
			s.bytes().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
		)
	};
	guard_src = guard_src.replace(
		"(EV and EV[\"newproxy\"]) or newproxy",
		&format!("EV[{}]", chars("newproxy")),
	);
	for name in ["__tostring", "__metatable", "__concat", "__call", "__iter"] {
		let expr = chars(name);
		guard_src = guard_src
			.replace(&format!(".{}", name), &format!("[{}]", expr))
			.replace(&format!("{} =", name), &format!("[{}] =", expr))
			.replace(&format!("\"{}\"", name), &expr);
	}
	// Hide the remaining guard string vocabulary. This is deliberately a
	// finite semantic list rather than a generic source-string decoder, so no
	// runtime source reconstruction surface is reintroduced.
	for text in [
		"type", "pcall", "xpcall", "error", "rawget", "rawset",
		"getmetatable", "setmetatable", "tostring", "tonumber", "string",
		"gmatch", "unpack", "table", "print", "warn", "debug", "info",
		"getfenv", "_G", "function", "number", "l", "s", "[C]",
		":(%d+):", "loadstring", "load", "tddxhpfcpi", "lsphhfstdm",
		"vymgglkhqr",
	] {
		guard_src = guard_src.replace(&format!("\"{}\"", text), &chars(text));
	}
	let mangled = match parser::parse(&guard_src, true).ok() {
		Some(mut block) => {
			let mut table = crate::symtab::resolve(&mut block);
			mangle::mangle(&mut table, rng, false);
			printer::print_chunk_luau(&table, &block)
		}
		None => guard_src.clone(),
	};
	// 守卫形态固定为 `local NAME = (function() ... end)()`——取 NAME。
	let trimmed = mangled.trim_start();
	let holder = trimmed
		.strip_prefix("local ")
		.and_then(|r| r.split('=').next())
		.map(|s| s.trim().to_string())
		.unwrap_or_else(|| "_hp".to_string());
	(mangled, holder)
}

/// Build the guard prelude: parse -> mangle -> print -> minify, so the
/// guard ships with build-random local names and compact form. Returns
/// the ready-to-prepend statement text (single line, no trailing
/// newline). On any internal failure returns the verbatim source
/// (protection must never silently disappear).
pub fn guard_prelude(rng: &mut Rng, luau: bool) -> String {
	let guard_src = assemble_guard(rng);
	let parsed = parser::parse(&guard_src, luau).ok();
	if let Some(mut block) = parsed {
		let mut table = crate::symtab::resolve(&mut block);
		mangle::mangle(&mut table, rng, false);
		let printed = if luau {
			printer::print_chunk_luau(&table, &block)
		} else {
			printer::print_chunk(&table, &block)
		};
		match minify::minify(&printed, luau) {
			Ok(m) => return m.trim_end_matches('\n').to_string(),
			Err(_) => {}
		}
	}
	guard_src
}
