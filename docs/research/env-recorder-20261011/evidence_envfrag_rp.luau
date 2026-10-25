-- ENVFRAG isolation: record phase A, replay phase B, compare EVD + report
-- the first divergence with args. Top-level errors are visible on exit.
local REAL_PRINT = print
local REAL_PCALL = pcall
local REAL_ERROR = error
local REAL_GETFENV = getfenv

local PASSTHROUGH = {
	print = true, getfenv = true, pcall = true, xpcall = true,
	setfenv = true, coroutine = true, pairs = true, ipairs = true,
	next = true, error = true, loadstring = true,
}

local log = {}

local function same_arg(a, b)
	if a == b then
		return true
	end
	-- values equal or same object; strings/numbers/booleans/nil covered by ==
	return false
end

local function record_entry(name, fv, ...)
	local args = table.pack(...)
	local r = table.pack(REAL_PCALL(fv, ...))
	local q = log[name]
	if not q then
		q = {}
		log[name] = q
	end
	if r[1] then
		local out = { n = r.n - 1 }
		for i = 2, r.n do
			out[i - 1] = r[i]
		end
		q[#q + 1] = { raised = false, results = out, args = args }
		return table.unpack(out, 1, out.n)
	else
		q[#q + 1] = { raised = true, val = r[2], args = args }
		REAL_ERROR(r[2], 0)
	end
end

local diverged = nil

local function replay_entry(name, ...)
	local q = log[name]
	local entry = q and table.remove(q, 1)
	if entry == nil then
		REAL_ERROR("DIVERGE-EMPTY:" .. name, 0)
	end
	local actual = table.pack(...)
	if actual.n ~= entry.args.n then
		diverged = "ARGCOUNT:" .. name .. " want " .. entry.args.n .. " got " .. actual.n
	elseif diverged == nil then
		for i = 1, actual.n do
			if not same_arg(entry.args[i], actual[i]) then
				diverged = "ARGVAL:" .. name .. "#" .. i
					.. " want " .. type(entry.args[i])
					.. " got " .. type(actual[i])
				break
			end
		end
	end
	if entry.raised then
		REAL_ERROR(entry.val, 0)
	end
	return table.unpack(entry.results, 1, entry.results.n)
end

local function make_env(replay)
	local real = REAL_GETFENV(0)
	local env = {}
	for k, v in pairs(real) do
		if PASSTHROUGH[k] then
			env[k] = v
		elseif type(v) == "function" then
			env[k] = replay
				and function(...) return replay_entry(k, ...) end
				or function(...) return record_entry(k, v, ...) end
		elseif type(v) == "table" then
			local proxy = {}
			for fk, fv in pairs(v) do
				local name = k .. "." .. fk
				if type(fv) == "function" then
					proxy[fk] = replay
						and function(...) return replay_entry(name, ...) end
						or function(...) return record_entry(name, fv, ...) end
				else
					proxy[fk] = fv
				end
			end
			env[k] = proxy
		else
			env[k] = v
		end
	end
	env._G = env
	return env
end

local SRC = [=[
function() local fR=bit32 local v_=buffer local bdo=string local wSfD=table local CnHw=math local nV=coroutine local lI=loadstring local unN=select local JfZ=pcall local YB=setmetatable local qls=rawequal local DV=rawget local BY=tostring local yi=bit32.bxor local DYx=buffer.create local bs=string.byte local hO=coroutine.resume local Bez=0 local tx=string.char(65,0,90) local cVFd=(tx=="\65\0\90") Bez=(Bez*45323+(cVFd and 0 or 1))%268435456 local Ih=string.sub("abcdef",2,-2) Bez=(Bez*63201+(Ih=="bcde" and 0 or 1))%268435456 local NSW=string.rep("xy",5) Bez=(Bez*2709+(NSW=="xyxyxyxyxy" and 0 or 1))%268435456 local h_=#tx Bez=(Bez*62909+(h_-3))%268435456 local _bW=string.byte(tx,2) Bez=(Bez*25523+_bW)%268435456 local lt=type(table.freeze)=="function" local gs=table.freeze({}) local HB=pcall(function() gs[1]=1 end) lt=lt and not HB Bez=(Bez*57487+(lt and 0 or 1))%268435456 local Ds=true local ik=loadstring("return 745, nil, 629") Ds=type(ik)=="function" and select("#",ik())==3 and select(3,ik())==629 local xPT,IYd=loadstring("local =") Ds=Ds and xPT==nil and type(IYd)=="string" Bez=(Bez*37043+(Ds and 0 or 1))%268435456 local tgr=select("#","h",nil,"t") Bez=(Bez*32837+(tgr-3))%268435456 local oLt=(-17)%5 Bez=(Bez*17917+(oLt-3))%268435456 local uv=math.floor(-1.25) Bez=(Bez*49485+(uv+2))%268435456 local jh=bit32.bxor(1241466966,3468229899) Bez=(Bez*65277+(jh-2269623645))%268435456 local LBt=bit32.band(1395253532,901105958) Bez=(Bez*4105+(LBt-287424772))%268435456 local MG_=true local vH=coroutine.create(function(sG) local nf=coroutine.yield(sG+1) return nf+2 end) local LtO,WGH=coroutine.resume(vH,9) local Wd,jH=coroutine.resume(vH,11) MG_=LtO and WGH==10 and Wd and jH==13 and coroutine.status(vH)=="dead" local dG=0 local Nr=coroutine.create(function() for Le=1,6 do coroutine.yield(Le) end end) for Le=1,6 do local sYw,RB=coroutine.resume(Nr) if sYw and RB==Le then dG=dG+1 end end MG_=MG_ and dG==6 and coroutine.close(Nr) Bez=(Bez*23265+(MG_ and 0 or 1))%268435456 local ldyU={__index=function() return "AA" end,__add=function(sG,nf) return sG.fL+nf.fL end,__tostring=function() return "o" end} local NyQu=setmetatable({fL=13},ldyU) local TgL=setmetatable({fL=7},ldyU) local JWw=(NyQu.Zn=="AA" and (NyQu+TgL)==20 and tostring(NyQu)=="o" and rawequal(NyQu,NyQu) and not rawequal(NyQu,TgL) and NyQu.fL==13 and rawget(NyQu,"zz")==nil) Bez=(Bez*39577+(JWw and 0 or 1))%268435456 local oO={} for Le=1,3 do local Yv=Le*10 oO[Le]=function(sG) Yv=Yv+sG return Yv end end local sI=(oO[1](4)==14 and oO[1](2)==16 and oO[2](1)==21 and oO[3](0)==30) Bez=(Bez*10805+(sI and 0 or 1))%268435456 local nw=true local go=buffer.fromstring("\82\77\9\13") nw=nw and buffer.len(go)==4 and buffer.readu8(go,0)==82 and buffer.readu8(go,3)==13 nw=nw and buffer.readu16(go,1)==2381 and buffer.readu32(go,0)==218713426 nw=nw and buffer.tostring(go)=="\82\77\9\13" local VMb=buffer.create(8) buffer.writeu16(VMb,0,27112) buffer.writeu32(VMb,2,572572594) nw=nw and buffer.readu8(VMb,0)==232 and buffer.readu8(VMb,1)==105 nw=nw and buffer.readu8(VMb,2)==178 and buffer.readu8(VMb,5)==34 buffer.writef64(VMb,0,0.5) nw=nw and buffer.readf64(VMb,0)==0.5 buffer.writestring(VMb,0,"Az") nw=nw and buffer.readstring(VMb,0,2)=="Az" buffer.fill(VMb,0,7,4) nw=nw and buffer.readu8(VMb,0)==7 and buffer.readu8(VMb,3)==7 local Nlt=buffer.create(4) buffer.writebits(Nlt,3,5,31) nw=nw and buffer.readbits(Nlt,3,5)==31 local Ko=buffer.fromstring("Az") buffer.copy(Ko,0,go,0,2) nw=nw and buffer.tostring(Ko)=="\82\77" nw=nw and not pcall(buffer.readu8,go,99) and not pcall(buffer.writeu8,VMb,-1,0) and not pcall(buffer.readstring,go,3,2) Bez=(Bez*37199+(nw and 0 or 1))%268435456 local ZI=(bit32==fR and buffer==v_ and string==bdo and table==wSfD and math==CnHw and coroutine==nV and loadstring==lI and select==unN and pcall==JfZ and setmetatable==YB and rawequal==qls and rawget==DV and tostring==BY and bit32.bxor==yi and buffer.create==DYx and string.byte==bs and coroutine.resume==hO) Bez=(Bez*56079+(ZI and 0 or 1))%268435456 return Bez end]=]

-- Phase A: record
local loaderA = loadstring("return " .. SRC, "fragA")
setfenv(loaderA, make_env(false))
local fragA = loaderA()
local okA, dA = REAL_PCALL(fragA)
REAL_PRINT("PHASE-A", "ok=" .. tostring(okA), "d=" .. tostring(dA))

local total = 0
local names = {}
for n, q in pairs(log) do
	total = total + #q
	names[#names + 1] = n .. "=" .. #q
end
table.sort(names)
REAL_PRINT("LOG", total, table.concat(names, " "))

-- Phase B: replay a fresh copy of the factory
local loaderB = loadstring("return " .. SRC, "fragB")
setfenv(loaderB, make_env(true))
local fragB = loaderB()
local okB, dB = REAL_PCALL(fragB)
REAL_PRINT("PHASE-B", "ok=" .. tostring(okB), "d=" .. tostring(dB))
if diverged then
	REAL_PRINT("DIVERGED", diverged)
else
	REAL_PRINT("DIVERGED", "none")
end
local leftover = 0
for _, q in pairs(log) do
	leftover = leftover + #q
end
REAL_PRINT("LEFTOVER", leftover)
REAL_ERROR("DONE", 0)
