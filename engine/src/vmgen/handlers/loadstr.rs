
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "local q=C[b+1] local st=c local by=(\"\")[\"byte\"] local ch=(\"\")[\"char\"] local out=\"\" local _pv=0 for i=1,#q do st=(st*257+d+i+_pv)%65521 local _x=(st+(st%256)*((st-(st%256))/256))%65521 _x=(_x*257+((_x-(_x%256))/256))%65521 local _v=(by(q,i)-(((_x%256)+(((_x-(_x%256))/256)%256))%256))%256 out=out..ch(_v) _pv=_v end V[a+1]=out".to_string(),
		_ => "local st=c local q=C[b+1] local out=\"\" local ch=(\"\")[\"char\"] local by=(\"\")[\"byte\"] local _pv=0 for i=1,#q do st=(257*st+d+i+_pv)%65521 local _x=(st+(st%256)*((st-(st%256))/256))%65521 _x=(_x*257+((_x-(_x%256))/256))%65521 local _v=(by(q,i)-(((_x%256)+(((_x-(_x%256))/256)%256))%256))%256 out=out..ch(_v) _pv=_v end V[a+1]=out".to_string(),
	}
}
