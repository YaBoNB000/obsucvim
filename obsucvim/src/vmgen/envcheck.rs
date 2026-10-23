
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


fn g_enum(rng: &mut Rng) -> String {
	format!(
		"local z1=Enum.HumanoidRigType
local z2=z1[\"FromName\"](z1,\"R6\") local z3=z1[\"FromName\"](z1,\"R6\") local z4=z1[\"FromValue\"](z1,0) local z5=z1[\"R6\"]
local k4=(z2==z3 and z2==z4 and z2==z5 and z2[\"Name\"]==\"R6\" and z2[\"Value\"]==0 and z2[\"EnumType\"]==z1)
d=(d*{p1}+(k4 and 0 or 1))%{m}
local k5=(typeof(Enum)==\"Enums\" and typeof(z1)==\"Enum\" and typeof(z2)==\"EnumItem\" and getmetatable(Enum)==\"The metatable is locked\" and getmetatable(z1)==\"The metatable is locked\" and getmetatable(z2)==\"The metatable is locked\")
d=(d*{p2}+(k5 and 0 or 1))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		m = MOD,
	)
}

fn g_random(rng: &mut Rng) -> String {
	// 只做「结构/语义」判定，不比对精确浮点。Delta 这类执行器把 Random 用另一套
	// PRNG 实现（实测 seed 51018400 首个值 0.19639633043786484，而官方是
	// 0.9598971928030796），比对数值等于把执行器实现差异当成作弊。
	let s1 = rng.int(1, 2_147_483_646);
	let s2 = rng.int(1, 2_147_483_646);
	let s3 = rng.int(1, 2_147_483_646);
	format!(
		"local z6=Random.new(51018400)
local z6a=z6[\"NextNumber\"](z6)
local k6=(type(z6a)==\"number\" and z6a>=0 and z6a<1)
d=(d*{p1}+(k6 and 0 or 1))%{m}
local z6b=z6[\"NextInteger\"](z6,189,1164)
local k7=(type(z6b)==\"number\" and math.floor(z6b)==z6b and z6b>=189 and z6b<=1164)
d=(d*{p2}+(k7 and 0 or 1))%{m}
local z6c=z6[\"NextNumber\"](z6)
local k8=(type(z6c)==\"number\" and z6c>=0 and z6c<1 and z6c~=z6a)
d=(d*{p3}+(k8 and 0 or 1))%{m}
local z7=Random.new({s1}) local z8=Random.new({s1})
local k9=(z7[\"NextNumber\"](z7)==z8[\"NextNumber\"](z8))
d=(d*{p4}+(k9 and 0 or 1))%{m}
local z9=Random.new({s2}) z9[\"NextNumber\"](z9)
local za=z9[\"Clone\"](z9) local zb=za[\"NextNumber\"](za)
local zc=za[\"Clone\"](za) local zd=zc[\"NextNumber\"](zc) local ze=za[\"NextNumber\"](za)
local k10=(zd==ze)
d=(d*{p5}+(k10 and 0 or 1))%{m}
local zt=Random.new({s3}) local zw=Random.new({s3}+1)
local k11=(zt[\"NextNumber\"](zt)~=zw[\"NextNumber\"](zw))
d=(d*{p6}+(k11 and 0 or 1))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		p3 = mul(rng),
		p4 = mul(rng),
		p5 = mul(rng),
		p6 = mul(rng),
		s1 = s1,
		s2 = s2,
		s3 = s3,
		m = MOD,
	)
}

fn g_task(rng: &mut Rng) -> String {
	format!(
		"local zf=false
local zg=task.spawn(function() zf=true end)
local k11=(zf and type(zg)==\"thread\")
d=(d*{p1}+(k11 and 0 or 1))%{m}
local zh=false
local zi=task.delay(86400, function() zh=true end)
local zj,zl=pcall(task.cancel, zi)
local k12=(type(zi)==\"thread\" and not zh and zj)
d=(d*{p2}+(k12 and 0 or 1))%{m}
local k13=(not pcall(task.wait, {{}}))
d=(d*{p3}+(k13 and 0 or 1))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		p3 = mul(rng),
		m = MOD,
	)
}

fn g_member(rng: &mut Rng) -> String {
	// 「点调用方法必须报错」这一条保留（语义），但不再依赖官方 Luau 的精确错误文案
	// （"Expected ':' not '.'"）。不同执行器的解析器文案不同，Delta 的报错格式
	// 就与官方不一致；拿文案当否决权 = 版本一换就全灭。
	format!(
		"local zm=game:GetService(\"RunService\")
local zn,zo=pcall(function() return zm[\"IsStudio\"](nil) end)
local k14=(not zn and type(zo)==\"string\" and #zo>0)
d=(d*{p1}+(k14 and 0 or 1))%{m}
local zk,zl=pcall(function() return zm:IsStudio() end)
local k14b=(zk and type(zl)==\"boolean\")
d=(d*{p2}+(k14b and 0 or 1))%{m}
local zp,zq=pcall(function() return game.GetChildren(nil) end)
local k15=(not zp and type(zq)==\"string\" and #zq>0)
d=(d*{p3}+(k15 and 0 or 1))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		p3 = mul(rng),
		m = MOD,
	)
}

fn g_instance(rng: &mut Rng) -> String {
	format!(
		"local zr=Instance.new(\"Folder\") local zs=Instance.new(\"Folder\")
zs[\"Parent\"]=zr
local k16=(typeof(zr)==\"Instance\" and zr[\"ClassName\"]==\"Folder\" and type(zr[\"Name\"])==\"string\" and #zr[\"GetChildren\"](zr)==1)
d=(d*{p1}+(k16 and 0 or 1))%{m}
local zu=zr[\"ChildRemoved\"]
local zv=zu[\"Connect\"](zu, function() end)
local k17=(zv[\"Connected\"]==true)
zv[\"Disconnect\"](zv)
k17=k17 and zv[\"Connected\"]==false
d=(d*{p2}+(k17 and 0 or 1))%{m}
zr[\"Destroy\"](zr)
local zx,zy=pcall(function() zs[\"Parent\"]=zr end)
local k18=(not zx and type(zy)==\"string\" and #zy>0)
d=(d*{p3}+(k18 and 0 or 1))%{m}",
		p1 = mul(rng),
		p2 = mul(rng),
		p3 = mul(rng),
		m = MOD,
	)
}

fn g_exec(rng: &mut Rng) -> String {
	format!(
		"local k19=true
if type(_G[\"clonefunction\"])==\"function\" then
local zz=_G[\"clonefunction\"](pcall)
local z10,z11=zz(function() return 37 end)
k19=(type(zz)==\"function\" and z10 and z11==37)
end
d=(d*{p}+(k19 and 0 or 1))%{m}",
		p = mul(rng),
		m = MOD,
	)
}

pub fn build(rng: &mut Rng, tier2: bool) -> String {
	let mut groups = vec![
		g_arith(rng),
		g_string(rng),
		g_meta(rng),
		g_buffer(rng),
		g_freeze(rng),
		g_coroutine(rng),
		g_compile(rng),
	];
	if tier2 {
		groups.extend([
			g_enum(rng),
			g_random(rng),
			g_task(rng),
			g_member(rng),
			g_instance(rng),
			g_exec(rng),
		]);
	}
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
	if tier2 {
		out.push_str(
			"local r18=game local r19=Enum local r20=Instance local r21=Random \
local r22=task local r23=game.GetService local r24=Enum.HumanoidRigType \
local r25=Instance.new local r26=Random.new local r27=task.spawn\n",
		);
	}
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
	if tier2 {
		out.push_str(&format!(
			"local q2=(game==r18 and Enum==r19 and Instance==r20 and Random==r21 \
and task==r22 and game.GetService==r23 and Enum.HumanoidRigType==r24 \
and Instance.new==r25 and Random.new==r26 and task.spawn==r27)\n\
d=(d*{p}+(q2 and 0 or 1))%{m}\n",
			p = mul(rng),
			m = MOD,
		));
	}
	out.push_str("return d\nend");
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn factory_shape_is_stable() {
		let mut rng = Rng::new(20261011);
		let src = build(&mut rng, false);
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
		assert_eq!(src, build(&mut rng2, false));
	}

		#[test]
	fn tier2_flag_changes_source_and_keeps_tier1_shape() {
		let src = build(&mut Rng::new(20261011), true);
		for needle in [
			"Enum.HumanoidRigType",
			"Random.new(51018400)",
			"math.floor(z6b)==z6b",
			"task.spawn(function()",
			"zm:IsStudio()",
			"Instance.new(\"Folder\")",
			"_G[\"clonefunction\"]",
			"local q2=(game==r18",
		] {
			assert!(src.contains(needle), "tier2 missing {needle}");
		}
		assert_eq!(
			src.match_indices("local r")
				.filter(|(i, _)| src[*i + 7..].chars().next().is_some_and(|c| c.is_ascii_digit()))
				.count(),
			27
		);
		let mut rng2 = Rng::new(20261011);
		assert_eq!(src, build(&mut rng2, true));
	}

	#[test]
	fn tier2_false_contains_no_roblox_surface() {
		let src = build(&mut Rng::new(20261011), false);
		for banned in [
			"Enum.",
			"Random.new",
			"task.spawn",
			"Instance.new",
			"clonefunction",
		] {
			assert!(!src.contains(banned), "tier2=false leaked {banned}");
		}
	}

	#[test]
	fn randomization_varies_across_seeds() {
		let a = build(&mut Rng::new(1), false);
		let b = build(&mut Rng::new(2), false);
		assert_ne!(a, b);
	}
}
