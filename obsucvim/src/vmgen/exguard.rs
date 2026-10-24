
use crate::rng::Rng;

const M28: u64 = 268_435_456;
const M31: u64 = 2_147_483_647;
const M32: u64 = 4_294_967_296;

pub struct ExgConsts {
	pub attest_token: u64,
	pub poison: u64,
	pub xm1: u64,
	pub xm2: u64,
	pub xte: u64,
}

/// v16-a 转录绑定解码钥：编译期宿主模型，全部来自真机实测。
/// k2a = fold31(tostring(Vector3.new(1,2,3)))（h=7; h=(h*31+b)%2147483647）
/// k2b = fold31(typeof(Vector3.new(1,2,3)))
/// k3c = isStudio*1 + checkcaller*2 + identifyexecutor 名长*4
/// 设计约束：不混版本串（易变）、不混名串字节（无熵）；产物里不出现这些期望值，
/// 运行期观测值现算现折，宿主不符 → 转录失步 → ATTEST/TOK/SLT/st 全链失步 → 静默投毒。
#[derive(Clone, Copy)]
pub struct KeyHost {
	pub k2a: u64,
	pub k2b: u64,
	pub k3c: u64,
}

pub fn keyhost_profile(name: &str) -> Option<KeyHost> {
	match name {
		// 2026-10-24 真 Delta 1.1.743.1136 实测回传（GateProbe v2 [XG-K1..K3]）：
		// tostring(Vector3.new(1,2,3))="1, 2, 3" → fold31=1143546169
		// typeof="Vector3" → fold31=1317214663
		// IsStudio=false、checkcaller=true、名长=5 → 0*1 + 1*2 + 5*4 = 22
		"delta" => Some(KeyHost { k2a: 1_143_546_169, k2b: 1_317_214_663, k3c: 22 }),
		_ => None,
	}
}

pub fn build(rng: &mut Rng, keyhost: Option<&KeyHost>) -> (String, ExgConsts) {
	let decoy_seed = rng.int(1, 2_147_483_646) as u64;
	let salt_a = (decoy_seed ^ 0x6E62_4EB7) % M31;
	let salt_b = rng.int(1, 2_147_483_646) as u64;
	let salt_c = rng.int(1, 2_147_483_646) as u64;
	let payload_seal_salt = decoy_seed ^ rng.int(1, u32::MAX as i64) as u64;
	let attest_token = rng.int(1, u32::MAX as i64) as u64;
	let payload_state_salt = attest_token ^ rng.int(1, u32::MAX as i64) as u64;
	let constant_expected = rng.int(1_000_000, 999_999_999) as u64;
	let upvalue_expected = rng.int(1_000_000, 999_999_999) as u64;
	let upvalue_changed = rng.int(1_000_000, 999_999_999) as u64;
	let proto_constant = rng.int(1_000_000, 999_999_999) as u64;
	let proto_input = rng.int(1_000, 99_999) as u64;
	let proto_expected = (proto_constant + proto_input) % M31;
	let load_expected = rng.int(1_000_000, 999_999_999) as u64;
	let c_input = rng.int(1_000, 99_999) as u64;
	let transcript_seed = rng.int(1, 2_147_483_646) as u64;
	let mut transcript_expected = transcript_seed;
	for v in [
		constant_expected,
		upvalue_expected,
		upvalue_changed,
		proto_expected,
		load_expected,
		c_input,
		] {
		transcript_expected = (transcript_expected.wrapping_mul(31).wrapping_add(v)) % M32;
	}
	if let Some(kh) = keyhost {
		// v16-a：宿主观测折项追加在既有六项之后；运行期同位置折叠现算值。
		for v in [kh.k2a, kh.k2b, kh.k3c] {
			transcript_expected = (transcript_expected.wrapping_mul(31).wrapping_add(v)) % M32;
		}
	}
	let offset = (attest_token + M32 - transcript_expected) % M32;
	let poison = rng.int(1, 2_147_483_646) as u64;
	let interval = rng.int(16, 64);
	let jitter = std::cmp::max(5, interval / 3);
	let heavy = 3 + decoy_seed % 3;
let xm1 = rng.int(1024, 999_999) as u64;
	let xm2 = rng.int(1, (M28 - 1) as i64) as u64;
	let xte = (attest_token * xm1 + xm2) % M28;

	let canary: String = {
		const CH: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
		let len = rng.int(14, 20) as usize;
		let mut s = String::new();
		s.push(CH[rng.int(0, (CH.len() - 1) as i64) as usize] as char);
		for _ in 1..len {
			if rng.int(0, 1) == 0 {
				s.push((b'0' + rng.int(0, 9) as u8) as char);
			} else {
				s.push(CH[rng.int(0, (CH.len() - 1) as i64) as usize] as char);
			}
		}
		s
	};
	let invalid_chunk: String = {
		const CH: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
		let len = rng.int(10, 16) as usize;
		let mut s = String::new();
		for _ in 0..len {
			s.push(CH[rng.int(0, 25) as usize] as char);
		}
		s
	};
	let mut entries: Vec<(String, String)> = [
		("TK_GETGENV", "getgenv"),
		("TK_IDENTIFY", "identifyexecutor"),
		("TK_CHECKCALLER", "checkcaller"),
		("TK_ISC", "iscclosure"),
		("TK_ISL", "islclosure"),
		("TK_NEWC", "newcclosure"),
		("TK_LOADSTRING", "loadstring"),
		("TK_TYPEOF", "typeof"),
		("TK_GAME", "game"),
		("TK_INSTANCE", "Instance"),
		("TK_VECTOR3", "Vector3"),
		("TK_TASK", "task"),
		("TK_PLAYERS", "Players"),
		("TK_GETINFO", "getinfo"),
		("TK_INFO", "info"),
		("TK_GETCONSTANTS", "getconstants"),
		("TK_GETUPVALUES", "getupvalues"),
		("TK_GETPROTO", "getproto"),
		("TK_GETPROTOS", "getprotos"),
		("TK_SETUPVALUE", "setupvalue"),
		("TK_DEBUG", "debug"),
		("TK_STRING", "string"),
		("TK_TABLE", "table"),
		("TK_MATH", "math"),
		("TK_PCALL", "pcall"),
		("TK_TYPE", "type"),
		("TK_RAWGET", "rawget"),
		("TK_RAWSET", "rawset"),
		("TK_RAWEQUAL", "rawequal"),
		("TK_NEXT", "next"),
		("TK_SELECT", "select"),
		("TK_TONUMBER", "tonumber"),
		("TK_TOSTRING", "tostring"),
		("TK_SETMETATABLE", "setmetatable"),
		("TK_GETMETATABLE", "getmetatable"),
		("TK_GETFENV", "getfenv"),
		("TK_UNPACK", "unpack"),
		("TK_BYTE", "byte"),
		("TK_CHAR", "char"),
		("TK_SUB", "sub"),
		("TK_CONCAT", "concat"),
		("TK_INSERT", "insert"),
		("TK_LDEXP", "ldexp"),
		("TK_FLOOR", "floor"),
		("TK_ABS", "abs"),
		("TK_NEW", "new"),
		("TK_WAIT", "wait"),
		("TK_SPAWN", "spawn"),
		("TK_DEFER", "defer"),
		("TK_GETSERVICE", "GetService"),
		("TK_CLASSNAME", "ClassName"),
		("TK_TFUNCTION", "function"),
		("TK_TTABLE", "table"),
		("TK_TSTRING", "string"),
		("TK_TUSERDATA", "userdata"),
		("TK_TNUMBER", "number"),
		("TK_CMARK", "[C]"),
		("TK_LOADPFX", "return "),
	]
	.iter()
	.map(|(k, v)| (k.to_string(), v.to_string()))
	.collect();
	entries.push(("TK_CANARY".to_string(), canary.clone()));
	entries.push(("TK_INVALID".to_string(), invalid_chunk.clone()));
	entries.push(("TK_INVALIDSRC".to_string(), "return )".to_string()));
	// v16-a：新 token 只在 --key-host 开时注入（关态产物字节必须零变化）
	if keyhost.is_some() {
		entries.push(("TK_RUNSERVICE".to_string(), "RunService".to_string()));
		entries.push(("TK_ISSTUDIO".to_string(), "IsStudio".to_string()));
	}
	rng.shuffle(&mut entries);
	let mut bytes: Vec<u8> = Vec::new();
let mut descriptors: Vec<(i64, usize, usize, i64, i64)> = Vec::new();
	let mut tokens: Vec<i64> = Vec::new();
	let mut token_of: Vec<(String, i64)> = Vec::new();
	for (_k, v) in entries.iter() {
		let npad = rng.int(1, 5);
		for _ in 0..npad {
			bytes.push(rng.int(0, 255) as u8);
		}
		let mut token;
		loop {
			token = rng.int(10_000, 2_147_483_646);
			if !tokens.contains(&token) {
				break;
			}
		}
		tokens.push(token);
		let start = bytes.len() + 1;
		let seed = rng.int(1, 255);
		let step = rng.int(1, 255);
		for (i, b) in v.bytes().enumerate() {
			bytes.push(((b as i64 + seed + (i as i64 + 1) * step) % 256) as u8);
		}
		descriptors.push((token, start, v.len(), seed, step));
		token_of.push((_k.clone(), token));
	}
	let npad = rng.int(3, 8);
	for _ in 0..npad {
		bytes.push(rng.int(0, 255) as u8);
	}
	rng.shuffle(&mut descriptors);
	let tok = |name: &str| -> i64 {
		token_of
			.iter()
			.find(|(k, _)| k == name)
			.map(|(_, t)| *t)
			.unwrap()
	};
	let bytes_s = bytes
		.iter()
		.map(|b| b.to_string())
		.collect::<Vec<_>>()
		.join(",");
	let desc_s = descriptors
		.iter()
		.map(|(t, s, l, sd, st)| format!("[{}]={{{},{},{},{}}}", t, s, l, sd, st))
		.collect::<Vec<_>>()
		.join(",");

	// 前置自检：在进入任何"静默投毒"逻辑之前，先把"这台宿主到底缺什么"说出来。
	// 不加这一段时，缺 API 的宿主会被 TRIP 污染状态，然后在 VM 里随机位置炸掉，
	// 报出的错误（如 attempt to perform concatenation on a nil value）与真实原因无关，
	// 线上无法诊断。这里只做"报告"，不做放行：检测到缺失仍然 error() 终止。
	let mut src = String::new();

	src.push_str(&format!(
		"return function()\nlocal VKB = {{{}}}\nlocal VKM = {{{}}}\nlocal VKC = {{}}\nlocal function VK(t) local v = VKC[t] if v then return v end local r = VKM[t] if not r then return nil end local s = \"\" for i = 1, r[2] do s = s .. string.char((VKB[r[1] + i - 1] - r[3] - i * r[4]) % 256) end VKC[t] = s return s end\n",
		bytes_s, desc_s
	));
	src.clear();
	src.push_str(&format!(
		"return function()\nlocal VKB = {{{}}}\nlocal VKM = {{{}}}\nlocal VKC = {{}}\n",
		bytes_s, desc_s
	));
	src.push_str(
		"local _ch\nlocal _why = \"\"\ndo\nlocal function _isch(f)\nlocal _w1 = coroutine.wrap(function() return f(65) end)\nlocal o1, r1 = pcall(_w1)\nif not o1 or r1 ~= \"A\" then return \"65:\" .. tostring(r1) end\nlocal _w2 = coroutine.wrap(function() return f(90) end)\nlocal o2, r2 = pcall(_w2)\nif not o2 or r2 ~= \"Z\" then return \"90:\" .. tostring(r2) end\nlocal _w3 = coroutine.wrap(function() return f(48) end)\nlocal o3, r3 = pcall(_w3)\nif not o3 or r3 ~= \"0\" then return \"48:\" .. tostring(r3) end\nlocal _w4 = coroutine.wrap(function() return f(95) end)\nlocal o4, r4 = pcall(_w4)\nif not o4 or r4 ~= \"_\" then return \"95:\" .. tostring(r4) end\nreturn nil end\nlocal function _scan(t)\nfor _k, _v in pairs(t) do\nif type(_v) == \"function\" then local _rr = _isch(_v) if _rr == nil then return _v end end\nend\nreturn nil end\nlocal _mok, _mt = pcall(getmetatable, \"\")\n_why = \"M=\" .. tostring(_mok) .. \"/\" .. type(_mt)\nif _mok and type(_mt) == \"table\" then\nlocal _sok, _sr = pcall(function()\nlocal r = _scan(_mt)\nif not r then\nfor _k, _v in pairs(_mt) do\nif type(_v) == \"table\" then r = _scan(_v) if r then break end end\nend\nend\nreturn r end)\nif _sok then _ch = _sr end\nend\nif not _ch then\nlocal _eok0, _env0 = pcall(getfenv, 0)\nif _eok0 and type(_env0) == \"table\" then\nlocal _sok2, _sr2 = pcall(function()\nfor _k, _tab in pairs(_env0) do\nif type(_tab) == \"table\" then\nlocal r2 = _scan(_tab)\nif r2 then return r2 end\nend\nend\nreturn nil end)\nif _sok2 then _ch = _sr2 end\nend\nend\nif not _ch then\nlocal _lok0, _lv0 = pcall(function() return (\"\\0\")[\"char\"] end)\nif _lok0 and type(_lv0) == \"function\" then\nlocal _rr0 = _isch(_lv0)\nif _rr0 == nil then _ch = _lv0 else _why = _why .. \";L=\" .. _rr0 end\nelse\n_why = _why .. \";L=\" .. tostring(_lok0) .. \"/\" .. type(_lv0)\nend\nend\nend\nif not _ch then return { 0, function() return 0 end } end\n",
	);
	src.push_str(
		"local _pc, _ty, _rg = pcall, type, rawget\nlocal function _er(e, k) local te = _ty(e) local v = nil if te == \"table\" then local rok, rv = _pc(_rg, e, k) if rok then v = rv end end if v ~= nil then return v end local ok, iv = _pc(function() return e[k] end) if ok then return iv end return nil end\n",
	);
	src.push_str(
		"local function VK(t) local v = VKC[t] if v then return v end local r = VKM[t] if not r then return nil end local s = \"\" for i = 1, r[2] do s = s .. _ch((VKB[r[1] + i - 1] - r[3] - i * r[4]) % 256) end VKC[t] = s return s end\n",
	);
	src.push_str(&format!(
		"local _eok, ENV = pcall(getfenv, 0)\nif not _eok or type(ENV) ~= \"table\" then ENV = nil end\nlocal function ERD(e, k) if type(e) ~= \"table\" then return nil end local v = rawget(e, k) if v ~= nil then return v end local ok, iv = pcall(function() return e[k] end) if ok then return iv end return nil end\nlocal GGENV = ERD(ENV, VK({}))\nlocal _cok, CENV = false, nil\nif type(GGENV) == \"function\" then _cok, CENV = pcall(GGENV) end\nif not _cok or type(CENV) ~= \"table\" then CENV = nil end\nlocal function LKP(k) local v = ERD(CENV, k) if v == nil then v = ERD(ENV, k) end return v end\n",
		tok("TK_GETGENV")
	));
	src.push_str(&format!(
		"local STBL = ERD(ENV, VK({}))\nlocal TTBL = ERD(ENV, VK({}))\nlocal MTBL = ERD(ENV, VK({}))\nlocal DTBL = LKP(VK({}))\nlocal PCALV = LKP(VK({}))\nlocal TYPV = LKP(VK({}))\nlocal RGETV = LKP(VK({}))\nlocal RSETV = LKP(VK({}))\nlocal RAWEQV = LKP(VK({}))\nlocal NEXTV = LKP(VK({}))\nlocal SELV = LKP(VK({}))\nlocal TONUMV = LKP(VK({}))\nlocal TOSTRV = LKP(VK({}))\nlocal SMTV = LKP(VK({}))\nlocal GMTV = LKP(VK({}))\nlocal UNPV = LKP(VK({}))\nlocal FLRV = MTBL and ERD(MTBL, VK({}))\nlocal ABSV = MTBL and ERD(MTBL, VK({}))\nlocal LDEXPV = MTBL and ERD(MTBL, VK({}))\nlocal SBYTEV = STBL and ERD(STBL, VK({}))\nlocal SCHARV = STBL and ERD(STBL, VK({}))\nlocal SSUBV = STBL and ERD(STBL, VK({}))\nlocal TCONCV = TTBL and ERD(TTBL, VK({}))\nlocal TINSEV = TTBL and ERD(TTBL, VK({}))\nlocal TUNPV = TTBL and ERD(TTBL, VK({}))\nlocal IDENTV = LKP(VK({}))\nlocal CHKCV = LKP(VK({}))\nlocal ISCV = LKP(VK({}))\nlocal ISLV = LKP(VK({}))\nlocal NEWCV = LKP(VK({}))\nlocal LSTRV = LKP(VK({}))\nlocal TYPHOF = LKP(VK({}))\nlocal GAMEV = LKP(VK({}))\nlocal INSTV = LKP(VK({}))\nlocal VEC3V = LKP(VK({}))\nlocal TASKV = LKP(VK({}))\nlocal GINFOV = DTBL and ERD(DTBL, VK({}))\nlocal GINFOSV = DTBL and ERD(DTBL, VK({}))\nlocal GCONSTV = DTBL and ERD(DTBL, VK({}))\nlocal GUPVV = DTBL and ERD(DTBL, VK({}))\nlocal GPROTOV = DTBL and ERD(DTBL, VK({}))\nlocal GPROTOSV = DTBL and ERD(DTBL, VK({}))\nlocal SUPVV = DTBL and ERD(DTBL, VK({}))\nlocal INSPECTV = TYPV and TYPV(GINFOSV) == \"function\" and GINFOSV or GINFOV\nlocal LOADPX = VK({})\n_pc, _ty, _rg = PCALV, TYPV, RGETV\n",
		tok("TK_STRING"),
		tok("TK_TABLE"),
		tok("TK_MATH"),
		tok("TK_DEBUG"),
		tok("TK_PCALL"),
		tok("TK_TYPE"),
		tok("TK_RAWGET"),
		tok("TK_RAWSET"),
		tok("TK_RAWEQUAL"),
		tok("TK_NEXT"),
		tok("TK_SELECT"),
		tok("TK_TONUMBER"),
		tok("TK_TOSTRING"),
		tok("TK_SETMETATABLE"),
		tok("TK_GETMETATABLE"),
		tok("TK_UNPACK"),
		tok("TK_FLOOR"),
		tok("TK_ABS"),
		tok("TK_LDEXP"),
		tok("TK_BYTE"),
		tok("TK_CHAR"),
		tok("TK_SUB"),
		tok("TK_CONCAT"),
		tok("TK_INSERT"),
		tok("TK_UNPACK"),
		tok("TK_IDENTIFY"),
		tok("TK_CHECKCALLER"),
		tok("TK_ISC"),
		tok("TK_ISL"),
		tok("TK_NEWC"),
		tok("TK_LOADSTRING"),
		tok("TK_TYPEOF"),
		tok("TK_GAME"),
		tok("TK_INSTANCE"),
		tok("TK_VECTOR3"),
		tok("TK_TASK"),
		tok("TK_GETINFO"),
		tok("TK_INFO"),
		tok("TK_GETCONSTANTS"),
		tok("TK_GETUPVALUES"),
		tok("TK_GETPROTO"),
		tok("TK_GETPROTOS"),
		tok("TK_SETUPVALUE"),
		tok("TK_LOADPFX")
	));

	// 前置自检（R067.4 重写）：只复用守卫自己已经绑定的原语（TYPV/LKP 解析后的
	// IDENTV/CHKCV/.../SUPVV），不再出现任何裸执行器标识符。sanitize_frag 会把任何
	// 非保留字标识符改名（getgenv → Idu 之类），裸写必坏；字符串字面量不受影响。
	// 判定集与 SCHALL 第一道闸门逐一对应，因此报告出的缺失清单就是闸门真实要求。
	src.push_str(
		r#"do
local _miss = ""
local function _chk(n, v) if TYPV(v) ~= "function" then _miss = _miss .. n .. "," end end
_chk("identifyexecutor", IDENTV)
_chk("checkcaller", CHKCV)
_chk("iscclosure", ISCV)
_chk("islclosure", ISLV)
_chk("newcclosure", NEWCV)
_chk("loadstring", LSTRV)
_chk("typeof", TYPHOF)
if TYPV(GINFOV) ~= "function" and TYPV(GINFOSV) ~= "function" then _miss = _miss .. "debug.info," end
_chk("debug.getconstants", GCONSTV)
_chk("debug.getupvalues", GUPVV)
if TYPV(GPROTOV) ~= "function" and TYPV(GPROTOSV) ~= "function" then _miss = _miss .. "debug.getproto," end
_chk("debug.setupvalue", SUPVV)
if TYPV(GAMEV) ~= "table" and TYPV(GAMEV) ~= "userdata" then _miss = _miss .. "game," end
if TYPV(INSTV) ~= "table" then _miss = _miss .. "Instance," end
if TYPV(VEC3V) ~= "table" then _miss = _miss .. "Vector3," end
if TYPV(TASKV) ~= "table" then _miss = _miss .. "task," end
if _miss ~= "" then error("[Obsucvim] this executor lacks APIs the hardened build requires: " .. _miss .. " - use the compatible build (--no-bind-executor) here, or a Synapse-class executor", 0) end
end
"#,
	);
	src.push_str(&format!(
		"local COUNTER = 0\nlocal NEXTP = 1\nlocal EPOCH = 0\nlocal STATE = {} % 2147483647\nlocal ATTEST = 0\nlocal SEALA = (STATE * 65599 + {} + ATTEST) % 2147483647\nlocal SEALB = (STATE * 4099 + {} + ATTEST % 104729) % 2147483647\nlocal SEALC = (STATE * 8191 + {} + (ATTEST % 65521) * 131) % 2147483647\nlocal TRIPPED = false\nlocal ATTESTED = false\nlocal TOK = {}\nlocal PSTATE = 0\nlocal PSEAL = 0\nlocal PACTIVE = false\nlocal UPV = {}\n",
		decoy_seed, salt_a, salt_b, salt_c, poison, upvalue_expected
	));
	if keyhost.is_some() {
		// v16-a 诊断寄存器：SCHALL 观测结果落 chunk 作用域，trace 模式 XF(18) 随 _tr 带出
		src.push_str("local KHXA = -1\nlocal KHXB = -1\nlocal KHXC = -1\n");
	}
	src.push_str(
		"local function TRIP() TRIPPED = true ATTEST = 0 TOK = POISONV end\n",
	);
	src.push_str(&format!("local POISONV = {}\n", poison));
	src.push_str(
		"local function BXR(l, r) l = l % 2147483648 r = r % 2147483648 local v = 0 local b = 1 for i = 0, 30 do local lb = l % 2 local rb = r % 2 if lb ~= rb then v = v + b end l = (l - lb) / 2 r = (r - rb) / 2 b = b * 2 end return v end\n",
	);
	src.push_str(&format!(
		"local function PSEALX() local lo = PSTATE % 65536 local hi = (PSTATE - lo) / 65536 return (lo * 65599 + hi * 257 + STATE * 4099 + SEALA + SEALB + SEALC + ATTEST + {}) % 2147483647 end\nlocal function RESEAL() SEALA = (STATE * 65599 + {} + ATTEST) % 2147483647 SEALB = (STATE * 4099 + {} + ATTEST % 104729) % 2147483647 SEALC = (STATE * 8191 + {} + (ATTEST % 65521) * 131) % 2147483647 end\n",
		payload_seal_salt, salt_a, salt_b, salt_c
	));

	let mut ci = String::new();
	ci.push_str("local function CIDENT() if TYPV == nil then return false end\n");
	ci.push_str(&format!(
		"if ERD(ENV, VK({})) ~= STBL or ERD(ENV, VK({})) ~= TTBL or ERD(ENV, VK({})) ~= MTBL or (ERD(ENV, VK({})) ~= DTBL and ERD(CENV, VK({})) ~= DTBL) or ERD(ENV, VK({})) ~= PCALV or ERD(ENV, VK({})) ~= TYPV or ERD(ENV, VK({})) ~= RGETV or ERD(ENV, VK({})) ~= RSETV or ERD(ENV, VK({})) ~= NEXTV or ERD(ENV, VK({})) ~= GMTV or ERD(ENV, VK({})) ~= SMTV or ERD(ENV, VK({})) ~= RAWEQV or ERD(ENV, VK({})) ~= TOSTRV or ERD(ENV, VK({})) ~= SELV or ERD(ENV, VK({})) ~= TONUMV or ERD(STBL, VK({})) ~= SBYTEV or ERD(STBL, VK({})) ~= SCHARV or ERD(STBL, VK({})) ~= SSUBV or ERD(TTBL, VK({})) ~= TCONCV or ERD(TTBL, VK({})) ~= TINSEV or ERD(MTBL, VK({})) ~= LDEXPV then return false end\n",
		tok("TK_STRING"),
		tok("TK_TABLE"),
		tok("TK_MATH"),
		tok("TK_DEBUG"),
		tok("TK_DEBUG"),
		tok("TK_PCALL"),
		tok("TK_TYPE"),
		tok("TK_RAWGET"),
		tok("TK_RAWSET"),
		tok("TK_NEXT"),
		tok("TK_GETMETATABLE"),
		tok("TK_SETMETATABLE"),
		tok("TK_RAWEQUAL"),
		tok("TK_TOSTRING"),
		tok("TK_SELECT"),
		tok("TK_TONUMBER"),
		tok("TK_BYTE"),
		tok("TK_CHAR"),
		tok("TK_SUB"),
		tok("TK_CONCAT"),
		tok("TK_INSERT"),
		tok("TK_LDEXP")
	));
	ci.push_str(&format!(
		"if not ENV or TYPV(GGENV) ~= \"function\" or ERD(ENV, VK({})) ~= GGENV then return false end\nlocal _gok, _genv2 = PCALV(GGENV) if not _gok or _genv2 ~= CENV then return false end\n",
		tok("TK_GETGENV")
	));
	ci.push_str(&format!(
		"if LKP(VK({})) ~= IDENTV or LKP(VK({})) ~= CHKCV or LKP(VK({})) ~= ISCV or LKP(VK({})) ~= ISLV or LKP(VK({})) ~= NEWCV or LKP(VK({})) ~= LSTRV or LKP(VK({})) ~= TYPHOF or LKP(VK({})) ~= GAMEV or LKP(VK({})) ~= INSTV or LKP(VK({})) ~= VEC3V or LKP(VK({})) ~= TASKV then return false end\n",
		tok("TK_IDENTIFY"),
		tok("TK_CHECKCALLER"),
		tok("TK_ISC"),
		tok("TK_ISL"),
		tok("TK_NEWC"),
		tok("TK_LOADSTRING"),
		tok("TK_TYPEOF"),
		tok("TK_GAME"),
		tok("TK_INSTANCE"),
		tok("TK_VECTOR3"),
		tok("TK_TASK")
	));
	ci.push_str(&format!(
		"if not DTBL or ERD(DTBL, VK({})) ~= GINFOV or ERD(DTBL, VK({})) ~= GINFOSV or ERD(DTBL, VK({})) ~= GCONSTV or ERD(DTBL, VK({})) ~= GUPVV or ERD(DTBL, VK({})) ~= GPROTOV or ERD(DTBL, VK({})) ~= GPROTOSV or ERD(DTBL, VK({})) ~= SUPVV then return false end\nreturn true end\n",
		tok("TK_GETINFO"),
		tok("TK_INFO"),
		tok("TK_GETCONSTANTS"),
		tok("TK_GETUPVALUES"),
		tok("TK_GETPROTO"),
		tok("TK_GETPROTOS"),
		tok("TK_SETUPVALUE")
	));
	src.push_str(&ci);

	src.push_str(
		"local function TCONTAINS(vals, exp) if TYPV(vals) ~= \"table\" then return false end for _k, it in NEXTV, vals do if it == exp then return true end end return false end\nlocal function TEMPTY(vals) return TYPV(vals) == \"table\" and NEXTV(vals) == nil end\nlocal function CLASSIFIES(f, expc) if TYPV(f) ~= \"function\" then return false end local cok, cres = PCALV(ISCV, f) local lok, lres = PCALV(ISLV, f) return cok and lok and cres == expc and lres == (not expc) end\n",
	);

	let mut sc = String::new();
	sc.push_str("local function SCHALL()\n");
	sc.push_str(&format!(
		"if TYPV(IDENTV) ~= \"function\" or TYPV(CHKCV) ~= \"function\" or TYPV(ISCV) ~= \"function\" or TYPV(ISLV) ~= \"function\" or TYPV(NEWCV) ~= \"function\" or TYPV(LSTRV) ~= \"function\" or TYPV(TYPHOF) ~= \"function\" or TYPV(INSPECTV) ~= \"function\" or TYPV(GCONSTV) ~= \"function\" or TYPV(GUPVV) ~= \"function\" or (TYPV(GPROTOV) ~= \"function\" and TYPV(GPROTOSV) ~= \"function\") or TYPV(SUPVV) ~= \"function\" then return false, 0 end\n"
	));
	sc.push_str(&format!(
		"if CENV == ENV then return false, 0 end\nlocal _thold = ERD(ENV, VK({})) local _cold = ERD(CENV, VK({})) local _tm, _cm = {{}}, {{}} local _sep, _per = false, false\nlocal _canok = PCALV(function() RSETV(ENV, VK({}), _tm) _sep = ERD(CENV, VK({})) ~= _tm RSETV(CENV, VK({}), _cm) local _rok, _renv = PCALV(GGENV) _per = _rok and _renv == CENV and ERD(_renv, VK({})) == _cm end)\nlocal _tr1 = PCALV(RSETV, ENV, VK({}), _thold) local _tr2 = PCALV(RSETV, CENV, VK({}), _cold)\nif not _canok or not _tr1 or not _tr2 or not _sep or not _per then return false, 0 end\n",
		tok("TK_CANARY"),
		tok("TK_CANARY"),
		tok("TK_CANARY"),
		tok("TK_CANARY"),
		tok("TK_CANARY"),
		tok("TK_CANARY"),
		tok("TK_CANARY"),
		tok("TK_CANARY")
	));
	sc.push_str(
		"local _i1ok, _in1, _iv1 = PCALV(IDENTV) local _i2ok, _in2, _iv2 = PCALV(IDENTV)\nif not _i1ok or not _i2ok or TYPV(_in1) ~= \"string\" or TYPV(_iv1) ~= \"string\" or TYPV(_iv2) ~= \"string\" or #_in1 < 1 or #_in1 > 128 or #_iv1 > 128 or _in1 ~= _in2 or _iv1 ~= _iv2 then return false, 0 end\nlocal _ccok, _cc = PCALV(CHKCV) if not _ccok or _cc ~= true then return false, 0 end\n",
	);
	sc.push_str(&format!(
		"if (TYPV(GAMEV) ~= \"table\" and TYPV(GAMEV) ~= \"userdata\") or TYPV(INSTV) ~= \"table\" or TYPV(VEC3V) ~= \"table\" or TYPV(TASKV) ~= \"table\" then return false, 0 end\nlocal _hok, _hver, _hd = PCALV(function()\nlocal _d = \"\"\n_d = _d .. \"g:\" .. TYPV(GAMEV)\nlocal _gs = ERD(GAMEV, VK({}))\nlocal _ok8, _v8 = PCALV(function() return GAMEV[VK({})] end)\n_d = _d .. \",rp:\" .. tostring(_ok8) .. \"/\" .. TYPV(_v8) .. \",er:\" .. TYPV(_gs)\nif _gs == nil and _ok8 then _gs = _v8 end\n_d = _d .. \",gs:\" .. TYPV(_gs)\nlocal _pok, _players = PCALV(_gs, GAMEV, VK({}))\n_d = _d .. \",pl:\" .. tostring(_pok) .. \"/\" .. TYPV(_players)\nlocal _cn = nil\nif _players ~= nil then local _cok2, _cv = PCALV(function() return ERD(_players, VK({})) end) if _cok2 then _cn = _cv end if _cn == nil then local _cok3, _cv3 = PCALV(function() return _players[VK({})] end) if _cok3 then _cn = _cv3 end end end\n_d = _d .. \",cn:\" .. tostring(_cn)\nlocal _vn = ERD(VEC3V, VK({}))\nif _vn == nil then local _nkok, _nkv = PCALV(function() return VEC3V[VK({})] end) if _nkok then _vn = _nkv end end\n_d = _d .. \",vn:\" .. TYPV(_vn)\nlocal _vok, _vec = PCALV(_vn)\n_d = _d .. \",v3:\" .. tostring(_vok) .. \"/\" .. TYPHOF(_vec)\nlocal _tv = TYPHOF(SMTV({{}}, {{}}))\n_d = _d .. \",smt:\" .. tostring(_tv)\nlocal _inew = ERD(INSTV, VK({}))\nif _inew == nil then local _iok2, _iv2 = PCALV(function() return INSTV[VK({})] end) if _iok2 then _inew = _iv2 end end\n_d = _d .. \",in:\" .. TYPV(_inew)\nlocal _tw = ERD(TASKV, VK({}))\nlocal _tsp = ERD(TASKV, VK({}))\nlocal _tdf = ERD(TASKV, VK({}))\n_d = _d .. \",tw:\" .. TYPV(_tw) .. \",tsp:\" .. TYPV(_tsp) .. \",tdf:\" .. TYPV(_tdf)\nlocal _ver = _pok == true and _players ~= nil and _cn == VK({}) and _vok == true and TYPHOF(_vec) == VK({}) and _tv == \"table\" and TYPV(_inew) == \"function\" and TYPV(_tw) == \"function\" and TYPV(_tsp) == \"function\" and TYPV(_tdf) == \"function\"\nreturn _ver == true, _d\nend)\nif not _hok or _hver ~= true then return false, 0 end\n",
		tok("TK_GETSERVICE"),
		tok("TK_GETSERVICE"),
		tok("TK_PLAYERS"),
		tok("TK_CLASSNAME"),
		tok("TK_CLASSNAME"),
		tok("TK_NEW"),
		tok("TK_NEW"),
		tok("TK_NEW"),
		tok("TK_NEW"),
		tok("TK_WAIT"),
		tok("TK_SPAWN"),
		tok("TK_DEFER"),
		tok("TK_PLAYERS"),
		tok("TK_VECTOR3")
	));
	sc.push_str(
		"local _prims = { SBYTEV, SCHARV, SSUBV, TCONCV, TINSEV, LDEXPV, SELV, PCALV, TYPV, TOSTRV, TONUMV, RGETV, RSETV, RAWEQV, NEXTV, SMTV, GMTV, TUNPV or UNPV, INSPECTV }\nfor _pi = 1, #_prims do if not CLASSIFIES(_prims[_pi], true) then return false, 0 end end\nlocal function _lprobe(v) return v end\nif not CLASSIFIES(_lprobe, false) then return false, 0 end\n",
	);
	sc.push_str(
		"local _c1ok = PCALV(GCONSTV, SBYTEV) local _c2ok, _c2up = PCALV(GUPVV, SBYTEV) local _c3ok = PCALV(SUPVV, SBYTEV, 1, 0)\nif _c1ok or _c3ok or (_c2ok and not TEMPTY(_c2up)) then return false, 0 end\nif TYPV(GPROTOV) == \"function\" then local _c4ok = PCALV(GPROTOV, SBYTEV, 1) if _c4ok then return false, 0 end end\nif TYPV(GPROTOSV) == \"function\" then local _c5ok = PCALV(GPROTOSV, SBYTEV) if _c5ok then return false, 0 end end\n",
	);
	sc.push_str(&format!(
		"local TR = {}\nlocal function TRW(v) TR = (TR * 31 + v) % 4294967296 end\nlocal function _cprobe() return {} end\nlocal _k1ok, _k1 = PCALV(GCONSTV, _cprobe) if not _k1ok or TYPV(_k1) ~= \"table\" then return false, 0 end TRW({})\n",
		transcript_seed, constant_expected, constant_expected
	));
	sc.push_str(&format!(
		"local function _uprobe(v) UPV = (UPV + v) % 2147483647 return UPV end\nlocal _k2ok, _k2 = PCALV(GUPVV, _uprobe) if not _k2ok or TYPV(_k2) ~= \"table\" then return false, 0 end TRW({})\nlocal _k3a = PCALV(SUPVV, _uprobe, 1, {}) local _k3b = PCALV(SUPVV, _uprobe, 1, {})\nif not _k3a or not _k3b or _uprobe(0) ~= {} then return false, 0 end TRW({})\n",
		upvalue_expected, upvalue_changed, upvalue_expected, upvalue_expected, upvalue_changed
	));
	sc.push_str(&format!(
		"local function _pprobe() local function _pchild(v) return (v + {}) % 2147483647 end return _pchild end\nlocal _actp = _pprobe() local _aok, _ares = PCALV(_actp, {})\nif not _aok or _ares ~= {} or not CLASSIFIES(_actp, false) then return false, 0 end\nlocal _saw = false\nif TYPV(GPROTOV) == \"function\" then local _p1ok = PCALV(GPROTOV, _pprobe, 1) if not _p1ok then return false, 0 end _saw = true local _p2ok, _p2 = PCALV(GPROTOV, _pprobe, 1, true) if not _p2ok or TYPV(_p2) ~= \"table\" then return false, 0 end end\nif TYPV(GPROTOSV) == \"function\" then local _p3ok, _p3 = PCALV(GPROTOSV, _pprobe) if not _p3ok or TYPV(_p3) ~= \"table\" then return false, 0 end _saw = true end\nif not _saw then return false, 0 end TRW({})\n",
		proto_constant, proto_input, proto_expected, proto_expected
	));
	sc.push_str(&format!(
		"local _ivok, _ivf, _ive = PCALV(LSTRV, VK({}), VK({}))\nif not _ivok or _ivf ~= nil or TYPV(_ive) ~= \"string\" or #_ive < 1 then return false, 0 end\nlocal _lsrc = LOADPX .. TOSTRV({})\nlocal _lok, _lf = PCALV(LSTRV, _lsrc) if not _lok or TYPV(_lf) ~= \"function\" then return false, 0 end\nlocal _l2ok, _lv = PCALV(_lf) if not _l2ok or _lv ~= {} or not CLASSIFIES(_lf, false) then return false, 0 end\nlocal _l3ok, _l3 = PCALV(GCONSTV, _lf) if not _l3ok or not TCONTAINS(_l3, {}) then return false, 0 end TRW({})\n",
		tok("TK_INVALIDSRC"),
		tok("TK_INVALID"),
		load_expected, load_expected, load_expected, load_expected
	));
	sc.push_str(&format!(
		"local function _cbody(v) return ABSV(v) end\nlocal _wok, _wrapped = PCALV(NEWCV, _cbody) if not _wok or TYPV(_wrapped) ~= \"function\" then return false, 0 end\nlocal _w2ok, _wv = PCALV(_wrapped, -{})\nif not _w2ok or _wv ~= {} or not CLASSIFIES(_wrapped, true) then return false, 0 end\nlocal _w3ok, _w3 = PCALV(GUPVV, _wrapped) if _w3ok and not TEMPTY(_w3) then return false, 0 end TRW({})\n",
		c_input, c_input, c_input
	));
	if keyhost.is_some() {
		// v16-a 转录绑定解码钥：运行期观测现算现折（产物内无期望值常量）；
		// 观测全程在 pcall 内，API 缺失/异常 → 折零 → 与编译期模型失配 → 静默失步。
		sc.push_str(&format!(
			"local _kha = 0 local _khb = 0 local _khc = 0\nlocal _khok = PCALV(function()\nlocal function _f31(s) local h = 7 for i = 1, #s do h = (h * 31 + SBYTEV(s, i)) % 2147483647 end return h end\nlocal _vn2 = ERD(VEC3V, VK({}))\nif _vn2 == nil then _vn2 = VEC3V[VK({})] end\nlocal _v3 = _vn2(1, 2, 3)\n_kha = _f31(TOSTRV(_v3))\n_khb = _f31(TYPHOF(_v3))\nlocal _gs2 = ERD(GAMEV, VK({}))\nif _gs2 == nil then _gs2 = GAMEV[VK({})] end\nlocal _rs2 = _gs2(GAMEV, VK({}))\nlocal _isf2 = ERD(_rs2, VK({}))\nif _isf2 == nil then _isf2 = _rs2[VK({})] end\nlocal _bs = (_isf2(_rs2) == true) and 1 or 0\nlocal _cok3, _cres3 = PCALV(CHKCV)\nlocal _bc = (_cok3 and _cres3 == true) and 1 or 0\n_khc = _bs + _bc * 2 + #_in1 * 4\nend)\nif not _khok then _kha, _khb, _khc = 0, 0, 0 end\nKHXA, KHXB, KHXC = _kha, _khb, _khc\nTRW(_kha) TRW(_khb) TRW(_khc)\n",
			tok("TK_NEW"),
			tok("TK_NEW"),
			tok("TK_GETSERVICE"),
			tok("TK_GETSERVICE"),
			tok("TK_RUNSERVICE"),
			tok("TK_ISSTUDIO"),
			tok("TK_ISSTUDIO")
		));
	}
	sc.push_str("return true, TR end\n");
	src.push_str(&sc);

	src.push_str(&format!(
		"local PROBE = function(vmword, force)\nCOUNTER = COUNTER + 1\nif TRIPPED then return POISONV end\nif PACTIVE and PSEAL ~= PSEALX() then TRIP() return POISONV end\nlocal _lo = vmword % 65536 local _hi = (vmword - _lo) / 65536\nPSTATE = (PSTATE * 4093 + _lo * 257 + _hi * 17 + COUNTER * 31 + STATE + ATTEST + {}) % 2147483647\nPSTATE = BXR(PSTATE, vmword % 2147483648) % 2147483647\nPACTIVE = true\nSTATE = (STATE + PSTATE % 65521 + COUNTER * 17 + EPOCH) % 2147483647\nRESEAL()\nPSEAL = PSEALX()\nif not force and COUNTER < NEXTP then return TOK end\nif SEALB ~= (STATE * 4099 + {} + ATTEST % 104729) % 2147483647 then TRIP() return POISONV end\nEPOCH = EPOCH + 1\nlocal _heavy = force or not ATTESTED or EPOCH % {} == 0\nlocal _valid = CIDENT()\nlocal _tr = {}\nif _valid and _heavy then _valid, _tr = SCHALL() end\nif _valid and _tr ~= {} then _valid = false end\nif not _valid then TRIP() return POISONV end\nif SEALC ~= (STATE * 8191 + {} + (ATTEST % 65521) * 131) % 2147483647 then TRIP() return POISONV end\nif not ATTESTED then ATTEST = (_tr + {}) % 4294967296 ATTESTED = true end\nif ATTEST ~= {} then TRIP() return POISONV end\nTOK = ATTEST\nSTATE = (STATE * 48271 + COUNTER + EPOCH * 17 + ATTEST % 65521) % 2147483647\nRESEAL()\nif PACTIVE then PSEAL = PSEALX() end\nNEXTP = COUNTER + {} + STATE % {}\nreturn TOK end\n",
		payload_state_salt, salt_b, heavy, transcript_expected, transcript_expected, salt_c, offset, attest_token, interval, jitter
	));
	src.push_str(&format!(
		"PROBE(0, true)\nreturn {{ TOK, PROBE }}\nend"
	));

	let bad = "local function TRIP() TRIPPED = true ATTEST = 0 TOK = POISONV end\n";
	let pos = src.find(bad).expect("trip decl");
	let after = src[pos + bad.len()..].to_string();
	let decl_line = format!("local POISONV = {}\n", poison);
	let after = after.strip_prefix(&decl_line).expect("poison decl").to_string();
	let mut fixed = String::with_capacity(src.len());
	fixed.push_str(&src[..pos]);
	fixed.push_str(&decl_line);
	fixed.push_str(bad);
	fixed.push_str(&after);

	if std::env::var("OBSUCVIM_EXG_TRACE").is_ok() {
		let tns = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_nanos())
			.unwrap_or(0);
		let trace_tag: String = (0..4)
			.map(|i| (b'a' + ((tns >> (i * 5)) % 26) as u8) as char)
			.collect();
		let xf = format!(
			"return function()\nlocal XFM = {{\"A\",\"B\",\"C\",\"D\",\"E\",\"F\",\"G\",\"H\",\"I\",\"J\",\"K\",\"L\",\"M\",\"N\",\"O\",\"P\",\"Q\",\"R\",\"S\",\"T\",\"U\"}}\nlocal function XF(c, d) if d ~= nil then error(\"Luau: The debug library is required on Luau platforms. Please open a support ticket. (\" .. XFM[c] .. \"-\" .. \"{}\" .. \") \" .. tostring(d), 0) else error(\"Luau: The debug library is required on Luau platforms. Please open a support ticket. (\" .. XFM[c] .. \"-\" .. \"{}\" .. \")\", 0) end end\n",
			trace_tag, trace_tag
		);
		fixed = fixed.replacen("return function()\n", &xf, 1);
		fixed = fixed.replace(
			"if not _ch then return { 0, function() return 0 end } end",
			"if not _ch then XF(1, _why) end",
		);
		let gates: &[(&str, u32)] = &[
			("if CENV == ENV then return false, 0 end", 3),
			("if not CLASSIFIES(_prims[_pi], true) then return false, 0 end", 9),
			("if not CLASSIFIES(_lprobe, false) then return false, 0 end", 9),
			("(_c2ok and not TEMPTY(_c2up)) then return false, 0 end", 10),
			("if _c4ok then return false, 0 end", 10),
			("if _c5ok then return false, 0 end", 10),
			("TYPV(_k1) ~= \"table\" then return false, 0 end", 11),
			("TYPV(_k2) ~= \"table\" then return false, 0 end", 12),
			("or not CLASSIFIES(_actp, false) then return false, 0 end", 14),
			("if not _p1ok then return false, 0 end", 14),
			("TYPV(_p2) ~= \"table\" then return false, 0 end", 14),
			("TYPV(_p3) ~= \"table\" then return false, 0 end", 14),
			("if not _saw then return false, 0 end", 14),
			("#_ive < 1 then return false, 0 end", 15),
			("TYPV(_lf) ~= \"function\" then return false, 0 end", 16),
			("TYPV(_wrapped) ~= \"function\" then return false, 0 end", 17),
			("if _w3ok and not TEMPTY(_w3) then return false, 0 end", 17),
		];
		for (g, code) in gates {
			let new_g = g.replace("return false, 0 end", &format!("XF({}) end", code));
			fixed = fixed.replace(g, &new_g);
		}
		fixed = fixed.replace(
			"or TYPV(SUPVV) ~= \"function\" then return false, 0 end",
			"or TYPV(SUPVV) ~= \"function\" then XF(2, TYPV(IDENTV) .. \"/\" .. TYPV(CHKCV) .. \"/\" .. TYPV(ISCV) .. \"/\" .. TYPV(ISLV) .. \"/\" .. TYPV(NEWCV) .. \"/\" .. TYPV(LSTRV) .. \"/\" .. TYPV(TYPHOF) .. \"/\" .. TYPV(INSPECTV) .. \"/\" .. TYPV(GCONSTV) .. \"/\" .. TYPV(GUPVV) .. \"/\" .. TYPV(GPROTOV) .. \"/\" .. TYPV(GPROTOSV) .. \"/\" .. TYPV(SUPVV)) end",
		);
		fixed = fixed.replace(
			"or not _sep or not _per then return false, 0 end",
			"or not _sep or not _per then XF(4, tostring(_canok) .. \"/\" .. tostring(_tr1) .. \"/\" .. tostring(_tr2) .. \"/\" .. tostring(_sep) .. \"/\" .. tostring(_per)) end",
		);
		fixed = fixed.replace(
			"_in1 ~= _in2 or _iv1 ~= _iv2 then return false, 0 end",
			"_in1 ~= _in2 or _iv1 ~= _iv2 then XF(5, tostring(_i1ok) .. \"/\" .. tostring(_i2ok) .. \"/\" .. tostring(_in1) .. \"/\" .. tostring(_iv1) .. \"/\" .. tostring(_in2) .. \"/\" .. tostring(_iv2)) end",
		);
		fixed = fixed.replace(
			"_cc ~= true then return false, 0 end",
			"_cc ~= true then XF(6, tostring(_ccok) .. \"/\" .. tostring(_cc)) end",
		);
		fixed = fixed.replace(
			"TYPV(TASKV) ~= \"table\" then return false, 0 end",
			"TYPV(TASKV) ~= \"table\" then XF(7, TYPV(GAMEV) .. \"/\" .. TYPV(INSTV) .. \"/\" .. TYPV(VEC3V) .. \"/\" .. TYPV(TASKV)) end",
		);
		fixed = fixed.replace(
			"if not _hok or _hver ~= true then return false, 0 end",
			"if not _hok or _hver ~= true then XF(8, tostring(_hok) .. \"/\" .. tostring(_hver) .. \":\" .. tostring(_hd)) end",
		);
		fixed = fixed.replace(
			&format!("_uprobe(0) ~= {} then return false, 0 end", upvalue_expected),
			&format!("_uprobe(0) ~= {} then XF(13) end", upvalue_expected),
		);
		fixed = fixed.replace(
			&format!("_lv ~= {} then return false, 0 end", load_expected),
			&format!("_lv ~= {} then XF(16) end", load_expected),
		);
		fixed = fixed.replace(
			&format!("not TCONTAINS(_l3, {}) then return false, 0 end", load_expected),
			&format!("not TCONTAINS(_l3, {}) then XF(16) end", load_expected),
		);
		fixed = fixed.replace(
			&format!("_wv ~= {} then return false, 0 end", c_input),
			&format!("_wv ~= {} then XF(17) end", c_input),
		);
		fixed = fixed.replace(
			&format!("if ATTEST ~= {} then TRIP() return POISONV end", attest_token),
			&format!(
				"if ATTEST ~= {} then XF(20, tostring(ATTEST)) end",
				attest_token
			),
		);
		fixed = fixed.replace(
			"if not _valid then TRIP() return POISONV end",
			"if not _valid then XF(18, tostring(_heavy) .. \"/\" .. tostring(_tr)) end",
		);
		if keyhost.is_some() {
			// v16-a：trace 模式把三项观测值随 XF(18) 带出，便于真机对表
			fixed = fixed.replace(
				"XF(18, tostring(_heavy) .. \"/\" .. tostring(_tr)) end",
				"XF(18, tostring(_heavy) .. \"/\" .. tostring(_tr) .. \"/\" .. tostring(KHXA) .. \"/\" .. tostring(KHXB) .. \"/\" .. tostring(KHXC)) end",
			);
		}
		fixed = fixed.replace("TRIP() return POISONV", "XF(21)");
	}

	(fixed, ExgConsts { attest_token, poison, xm1, xm2, xte })
}
