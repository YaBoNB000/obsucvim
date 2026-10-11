
use crate::anti;
use crate::mangle;
use crate::minify;
use crate::parser;
use crate::printer;
use crate::rng::Rng;

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


pub fn hboot_guard_source(rng: &mut Rng) -> (String, String) {
	let mut guard_src = assemble_guard(rng);
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
	let guard_src = assemble_guard(rng);
	let parsed = parser::parse(&guard_src, luau).ok();
	if let Some(mut block) = parsed {
		let mut table = crate::symtab::resolve(&mut block);
			mangle::mangle(&mut table, rng);
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
