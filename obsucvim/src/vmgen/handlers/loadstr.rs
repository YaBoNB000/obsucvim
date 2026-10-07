//! LoadStr — load a sealed string constant and unseal it into the
//! destination register. The constant table retains only the ciphertext.

/// f0 canonical; f1 renames temporaries and moves primitive lookup.
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "local q=C[b+1] local st=c local by=(\"\")[\"byte\"] local ch=(\"\")[\"char\"] local out=\"\" for i=1,#q do st=(st*257+d+i)%65521 out=out..ch((by(q,i)-st%256)%256) end V[a+1]=out".to_string(),
		_ => "local st=c local q=C[b+1] local out=\"\" local ch=(\"\")[\"char\"] local by=(\"\")[\"byte\"] for i=1,#q do st=(257*st+d+i)%65521 out=out..ch((by(q,i)-st%256)%256) end V[a+1]=out".to_string(),
	}
}
