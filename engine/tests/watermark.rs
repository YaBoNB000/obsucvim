//! Exercise the actual CLI boundary, not the internal printer (which must
//! remain free of branding when it emits embedded interpreter fragments).
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const HEADER: &str = "-- This file was obfuscated by Obsucvim.\n";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "obsucvim-watermark-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("input.lua"), "local a = 'watermark test'\nprint(a)\n").unwrap();
        Self(path)
    }
    fn run(&self, flags: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_obsucvim"))
            .current_dir(&self.0)
            .args(flags)
            .output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn check(output: &Output) {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::str::from_utf8(&output.stdout).unwrap();
    assert!(!text.contains(HEADER), "release artifact must not carry the brand watermark");
    assert!(!text.to_lowercase().contains("obsucvim"), "release artifact must not carry brand text");
    assert!(!text.trim().is_empty());
}

#[test]
fn every_preset_and_minify_mode_has_no_brand_watermark() {
    let f = Fixture::new();
    for preset in ["v15", "low", "medium", "high", "vm", "max"] {
        for mode in ["--minify", "--no-minify"] {
            let flags = ["--preset", preset, mode, "--seed", "42", "input.lua"];
            let stdout = f.run(&flags);
            check(&stdout);
            let mut to_file = flags.to_vec();
            to_file.push("output.lua");
            let file = f.run(&to_file);
            assert!(file.status.success());
            assert!(file.stdout.is_empty());
            assert_eq!(std::fs::read(f.0.join("output.lua")).unwrap(), stdout.stdout);
        }
    }
}

#[test]
fn default_and_output_flag_match_and_seed_is_deterministic() {
    let f = Fixture::new();
    let first = f.run(&["--seed", "42", "input.lua"]);
    check(&first);
    assert_eq!(first.stdout, f.run(&["--seed", "42", "input.lua"]).stdout);
    let to_file = f.run(&["--seed", "42", "-o", "output.lua", "input.lua"]);
    assert!(to_file.status.success());
    assert!(to_file.stdout.is_empty());
    assert_eq!(first.stdout, std::fs::read(f.0.join("output.lua")).unwrap());
}

#[test]
fn help_version_and_failed_generation_are_not_watermarked() {
    let f = Fixture::new();
    for flag in ["--help", "--version"] {
        let output = f.run(&[flag]);
        assert!(output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains(HEADER));
    }
    std::fs::write(f.0.join("input.lua"), "local = broken(").unwrap();
    let output = f.run(&["input.lua", "output.lua"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!f.0.join("output.lua").exists());
}

#[test]
fn source_watermark_comment_is_not_carried_into_output() {
    let f = Fixture::new();
    std::fs::write(f.0.join("input.lua"), format!("{HEADER}print('ok')\n")).unwrap();
    check(&f.run(&["--preset", "low", "--seed", "42", "input.lua"]));
}
