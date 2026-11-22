
use crate::rng::Rng;

const MOD: i64 = 268_435_456;

fn mul(rng: &mut Rng) -> i64 {
	rng.int(17, 65_521) | 1
}

fn esc(bytes: &[u8]) -> String {
	bytes
		.iter()
		.map(|b| format!("\\{b}"))
		.collect::<Vec<_>>()
		.join("")
}

fn g_arith(rng: &mut Rng) -> String {
	let a = rng.int(0, 4_294_967_295);
	let b = rng.int(0, 4_294_967_295);
	let e_xor = a ^ b;
	let c = rng.int(0, 4_294_967_295);
	let d2 = rng.int(0, 4_294_967_295);
	let e_and = c & d2;
	format!(
		"local a1=select(\"#\",\"h\",nil,\"t\") d=(d*{p1}+(a1-3))%{m}
local a2=(-17)%5 d=(d*{p2}+(a2-3))%{m}
local a3=math.floor(-1.25) d=(d*{p3}+(a3+2))%{m}
local a4=bit32.bxor({a},{b}) d=(d*{p4}+(a4-{e_xor}))%{m}
local a5=bit32.band({c},{d2}) d=(d*{p5}+(a5-{e_and}))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		p3 = mul(rng),
		p4 = mul(rng),
		p5 = mul(rng),
		m = MOD,
		a = a,
		b = b,
		e_xor = e_xor as i64,
		c = c,
		d2 = d2,
		e_and = e_and as i64,
	)
}

fn g_string(rng: &mut Rng) -> String {
	let n = rng.int(2, 5);
	format!(
		"local s1=string.char(65,0,90) local k1=(s1==\"\\65\\0\\90\")
d=(d*{p1}+(k1 and 0 or 1))%{m}
local s2=string.sub(\"abcdef\",2,-2) d=(d*{p2}+(s2==\"bcde\" and 0 or 1))%{m}
local s3=string.rep(\"xy\",{n}) d=(d*{p3}+(s3==\"{rep}\" and 0 or 1))%{m}
local n1=#s1 d=(d*{p4}+(n1-3))%{m}
local b1=string.byte(s1,2) d=(d*{p5}+b1)%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		p3 = mul(rng),
		p4 = mul(rng),
		p5 = mul(rng),
		m = MOD,
		n = n,
		rep = "xy".repeat(n as usize),
	)
}

fn g_meta(rng: &mut Rng) -> String {
	format!(
		"local mt={{__index=function() return \"AA\" end,__add=function(x,y) return x.v+y.v end,__tostring=function() return \"o\" end}}
local t1=setmetatable({{v=13}},mt) local t2=setmetatable({{v=7}},mt)
local k2=(t1.zz==\"AA\" and (t1+t2)==20 and tostring(t1)==\"o\" and rawequal(t1,t1) and not rawequal(t1,t2) and t1.v==13 and rawget(t1,\"zz\")==nil)
d=(d*{p1}+(k2 and 0 or 1))%{m}
local cl={{}} for i=1,3 do local c=i*10 cl[i]=function(x) c=c+x return c end end
local k3=(cl[1](4)==14 and cl[1](2)==16 and cl[2](1)==21 and cl[3](0)==30)
d=(d*{p2}+(k3 and 0 or 1))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		m = MOD,
	)
}

fn g_buffer(rng: &mut Rng) -> String {
	let mut bytes = [0u8; 4];
	for b in bytes.iter_mut() {
		*b = rng.int(1, 255) as u8;
	}
	let le16 = bytes[1] as i64 + (bytes[2] as i64) * 256;
	let le32 = bytes[0] as i64
		+ (bytes[1] as i64) * 256
		+ (bytes[2] as i64) * 65_536
		+ (bytes[3] as i64) * 16_777_216;
	let w16 = rng.int(0, 65_535);
	let w32 = rng.int(0, 4_294_967_295);
	let b0 = w16 % 256;
	let b1 = w16 / 256;
	let b2 = w32 % 256;
	let b5 = w32 / 16_777_216;
	let f64 = [0.5, 1.5, -0.25, 0.75, 123.5][rng.int(0, 4) as usize];
	let bits = rng.int(1, 31);
	format!(
		"local okb=true
local bf=buffer.fromstring(\"{esc}\")
okb=okb and buffer.len(bf)==4 and buffer.readu8(bf,0)=={r0} and buffer.readu8(bf,3)=={r3}
okb=okb and buffer.readu16(bf,1)=={le16} and buffer.readu32(bf,0)=={le32}
okb=okb and buffer.tostring(bf)==\"{esc}\"
local bw=buffer.create(8) buffer.writeu16(bw,0,{w16}) buffer.writeu32(bw,2,{w32})
okb=okb and buffer.readu8(bw,0)=={b0} and buffer.readu8(bw,1)=={b1}
okb=okb and buffer.readu8(bw,2)=={b2} and buffer.readu8(bw,5)=={b5}
buffer.writef64(bw,0,{f64}) okb=okb and buffer.readf64(bw,0)=={f64}
buffer.writestring(bw,0,\"Az\") okb=okb and buffer.readstring(bw,0,2)==\"Az\"
buffer.fill(bw,0,7,4) okb=okb and buffer.readu8(bw,0)==7 and buffer.readu8(bw,3)==7
local bb=buffer.create(4) buffer.writebits(bb,3,5,{bits}) okb=okb and buffer.readbits(bb,3,5)=={bits}
local bc=buffer.fromstring(\"Az\") buffer.copy(bc,0,bf,0,2) okb=okb and buffer.tostring(bc)==\"{e2}\"
okb=okb and not pcall(buffer.readu8,bf,99) and not pcall(buffer.writeu8,bw,-1,0) and not pcall(buffer.readstring,bf,3,2)
d=(d*{p}+(okb and 0 or 1))%{m}",
		esc = esc(&bytes),
		e2 = esc(&bytes[..2]),
		r0 = bytes[0],
		r3 = bytes[3],
		le16 = le16,
		le32 = le32,
		w16 = w16,
		w32 = w32,
		b0 = b0,
		b1 = b1,
		b2 = b2,
		b5 = b5,
		f64 = f64,
		bits = bits,
		p = mul(rng),
		m = MOD,
	)
}

fn g_freeze(rng: &mut Rng) -> String {
	format!(
		"local okf=type(table.freeze)==\"function\"
local fz=table.freeze({{}}) local fo=pcall(function() fz[1]=1 end)
okf=okf and not fo
d=(d*{p}+(okf and 0 or 1))%{m}",
		p = mul(rng),
		m = MOD,
	)
}

fn g_coroutine(rng: &mut Rng) -> String {
	let n = rng.int(3, 8);
	format!(
		"local okr=true
local co=coroutine.create(function(x) local y=coroutine.yield(x+1) return y+2 end)
local y1,y2=coroutine.resume(co,9) local y3,y4=coroutine.resume(co,11)
okr=y1 and y2==10 and y3 and y4==13 and coroutine.status(co)==\"dead\"
local cn=0 local co2=coroutine.create(function() for i=1,{n} do coroutine.yield(i) end end)
for i=1,{n} do local ro,vo=coroutine.resume(co2) if ro and vo==i then cn=cn+1 end end
okr=okr and cn=={n} and coroutine.close(co2)
d=(d*{p}+(okr and 0 or 1))%{m}",
		n = n,
		p = mul(rng),
		m = MOD,
	)
}

fn g_compile(rng: &mut Rng) -> String {
	let a = rng.int(1, 999);
	let b = rng.int(1, 999);
	format!(
		"local okl=true
local f1=loadstring(\"return {a}, nil, {b}\")
okl=type(f1)==\"function\" and select(\"#\",f1())==3 and select(3,f1())=={b}
local f2,e2=loadstring(\"local =\")
okl=okl and f2==nil and type(e2)==\"string\"
d=(d*{p}+(okl and 0 or 1))%{m}",
		a = a,
		b = b,
		p = mul(rng),
		m = MOD,
	)
}

pub fn build(rng: &mut Rng) -> String {
	let mut groups = vec![
		g_arith(rng),
		g_string(rng),
		g_meta(rng),
		g_buffer(rng),
		g_freeze(rng),
		g_coroutine(rng),
		g_compile(rng),
	];
	rng.shuffle(&mut groups);
	let mut out = String::from("return function()\n");
	out.push_str(
		"local r1=bit32 local r2=buffer local r3=string local r4=table 
local r5=math local r6=coroutine local r7=loadstring local r8=select 
local r9=pcall local r10=setmetatable local r11=rawequal local r12=rawget 
local r13=tostring local r14=bit32.bxor local r15=buffer.create 
local r16=string.byte local r17=coroutine.resume\n",
	);
	out.push_str("local d=0\n");
	for g in &groups {
		out.push_str(g);
		out.push('\n');
	}
	out.push_str(&format!(
		"local q1=(bit32==r1 and buffer==r2 and string==r3 and table==r4 and math==r5 
and coroutine==r6 and loadstring==r7 and select==r8 and pcall==r9 and setmetatable==r10 
and rawequal==r11 and rawget==r12 and tostring==r13 and bit32.bxor==r14 
and buffer.create==r15 and string.byte==r16 and coroutine.resume==r17)\n
d=(d*{p}+(q1 and 0 or 1))%{m}\n",
		p = mul(rng),
		m = MOD,
	));
	out.push_str("return d\nend");
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn factory_shape_is_stable() {
		let mut rng = Rng::new(20261011);
		let src = build(&mut rng);
		assert!(src.starts_with("return function()\n"));
		assert!(src.ends_with("return d\nend"));
		for needle in [
			"select(\"#\"",
			"bit32.bxor(",
			"buffer.fromstring(",
			"table.freeze",
			"coroutine.create(",
			"loadstring(",
			"rawequal(t1,t1)",
		] {
			assert!(src.contains(needle), "missing {needle}");
		}
		for banned in ["rawget(t1,\"v\")", "==\"zz\"", "return k end"] {
			assert!(!src.contains(banned), "rename-unsafe pattern: {banned}");
		}
		let capture_count = src
			.match_indices("local r")
			.filter(|(i, _)| src[*i + 7..].chars().next().is_some_and(|c| c.is_ascii_digit()))
			.count();
		assert_eq!(capture_count, 17, "exactly 17 identity captures expected");
		for m in ["local y1,y2=", "local y3,y4="] {
			assert!(src.contains(m), "coroutine locals must not shadow captures");
		}
		let mut rng2 = Rng::new(20261011);
		assert_eq!(src, build(&mut rng2));
	}

	#[test]
	fn generic_check_excludes_roblox_and_executor_apis() {
		let src = build(&mut Rng::new(20261011));
		for banned in [
			"Enum.",
			"Random.new",
			"task.spawn",
			"Instance.new",
			"clonefunction",
			"identifyexecutor",
			"checkcaller",
		] {
			assert!(!src.contains(banned), "generic check leaked {banned}");
		}
	}

	#[test]
	fn randomization_varies_across_seeds() {
		let a = build(&mut Rng::new(1));
		let b = build(&mut Rng::new(2));
		assert_ne!(a, b);
	}
}
