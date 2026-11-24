
pub mod caps;
pub mod compiler;
pub mod envcheck;
pub mod fuse;
pub mod handlers;
pub mod isa;
pub mod strpool;
pub mod strseal;
pub mod template;
pub mod v15;

pub use compiler::compile;

pub(crate) fn manifest_key(name: &str, value: u64) {
	if let Some(p) = crate::audit_gate::audit_var("OBSUCVIM_KEY_MANIFEST") {
		use std::io::Write;
		if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
			let _ = writeln!(f, "{name}={value}");
		}
	}
}
