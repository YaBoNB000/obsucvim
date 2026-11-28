/// Legacy (non-v15) `LoadStr` handler.
///
/// Phase 18 P1-1/P1-2 mirror of the v15 `M_LOAD_STR` micro-op: the constant
/// operand carries the cache policy in its top bits (0x8000 = cached,
/// 0x4000 = clear-after-use), `SC` is the per-frame string cache declared by
/// the `run` wrapper. Single-reference constants keep the plain inline decode.
pub fn code(fmt: u8) -> String {
	let decode = match fmt {
		0 => "local q=C[kf+1] local st=c local by=BYTE local ch=CHAR local up=UNP local tc=TCAT local n=#q local _pv=0 if n>0 then if n<=64 then local _ob={} for i=1,n do st=(st*257+d+i+_pv)%65521 local _x=(st+(st%256)*((st-(st%256))/256))%65521 _x=(_x*257+((_x-(_x%256))/256))%65521 local _v=(by(q,i)-(((_x%256)+(((_x-(_x%256))/256)%256))%256))%256 _ob[i]=_v _pv=_v end out=ch(up(_ob,1,n)) else local _ot,_on,_ob,_ok={},0,{},0 for i=1,n do st=(st*257+d+i+_pv)%65521 local _x=(st+(st%256)*((st-(st%256))/256))%65521 _x=(_x*257+((_x-(_x%256))/256))%65521 local _v=(by(q,i)-(((_x%256)+(((_x-(_x%256))/256)%256))%256))%256 _ok=_ok+1 _ob[_ok]=_v _pv=_v if _ok==64 then _on=_on+1 _ot[_on]=ch(up(_ob,1,64)) _ok=0 end end if _ok>0 then _on=_on+1 _ot[_on]=ch(up(_ob,1,_ok)) end out=tc(_ot) end end",
		_ => "local st=c local q=C[kf+1] local ch=CHAR local by=BYTE local up=UNP local tc=TCAT local n=#q local _pv=0 if n>0 then if n<=64 then local _ob={} for i=1,n do st=(257*st+d+i+_pv)%65521 local _x=(st+(st%256)*((st-(st%256))/256))%65521 _x=(_x*257+((_x-(_x%256))/256))%65521 local _v=(by(q,i)-(((_x%256)+(((_x-(_x%256))/256)%256))%256))%256 _ob[i]=_v _pv=_v end out=ch(up(_ob,1,n)) else local _ot,_on,_ob,_ok={},0,{},0 for i=1,n do st=(257*st+d+i+_pv)%65521 local _x=(st+(st%256)*((st-(st%256))/256))%65521 _x=(_x*257+((_x-(_x%256))/256))%65521 local _v=(by(q,i)-(((_x%256)+(((_x-(_x%256))/256)%256))%256))%256 _ok=_ok+1 _ob[_ok]=_v _pv=_v if _ok==64 then _on=_on+1 _ot[_on]=ch(up(_ob,1,64)) _ok=0 end end if _ok>0 then _on=_on+1 _ot[_on]=ch(up(_ob,1,_ok)) end out=tc(_ot) end end",
	};
	format!(
		"local kf=b local cached=kf>=32768 if cached then kf=kf-32768 end local clr=kf>=16384 if clr then kf=kf-16384 end local out if cached then out=_scache[kf+1] end if out==nil then {decode} if cached then _scache[kf+1]=out end end V[a+1]=out if clr then _scache[kf+1]=nil end"
	)
}
