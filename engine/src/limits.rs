//! Pre-compilation resource budget limits calibrated via §0.4 binary search
//! (`tools/calibrate_limits.py`).
//!
//! low / high are the measured safe / failing values on the final parser:
//!   MAX_TOPLEVEL_FUNC_DECLS  low=41    high=42     -> floor(41*0.8)    = 32
//!   MAX_TOPLEVEL_STMTS       low=2995  high=2996   -> floor(2995*0.8)  = 2396
//!                             (retained 2385 from the earlier low=2982 run:
//!                              the more conservative of the two measurements)
//!   MAX_CLOSURE_NEST_DEPTH   low=19    high=20     -> floor(19*0.8)    = 15
//!                             (C3 sample no longer panics in the current tree;
//!                              the earlier calibration is retained unchanged)
//!   MAX_EXPR_PARSE_DEPTH     low=3421  high=3422   -> floor(3421*0.8)  = 2736
//!   MAX_BLOCK_NEST_DEPTH     low=7070  high=7071   -> floor(7070*0.8)  = 5656

pub const MAX_TOPLEVEL_FUNC_DECLS: usize = 32; // calibrated from bad=100: low=41, high=42
pub const MAX_TOPLEVEL_STMTS: usize = 2385; // calibrated from bad=3000: low=2982, high=2983
pub const MAX_CLOSURE_NEST_DEPTH: usize = 15; // calibrated from bad=20: low=19, high=20
pub const MAX_EXPR_PARSE_DEPTH: usize = 2736; // calibrated from bad=20000: low=3421, high=3422
pub const MAX_BLOCK_NEST_DEPTH: usize = 5656; // calibrated from bad=20000: low=7070, high=7071

/// Structural bound (not a bisected budget): the per-function constants
/// section stores its own byte length as a `u16` varint
/// (`encode_u16var(&mut consts_sec, csec.len() as u16)` in `Ctx::finish`),
/// so a function whose encoded constants exceed 65,535 B used to wrap that
/// length and ship an artifact that fails at load time instead of being
/// rejected (found by the Phase 18 P4 audit: a single 65,495-byte sealed
/// literal in one function was the first failing specimen).
pub const MAX_CONSTS_SECTION_BYTES: usize = 65_535;

pub struct DepthGuard {
    depth: usize,
    max: usize,
    label: &'static str,
}

impl DepthGuard {
    pub fn new(max: usize, label: &'static str) -> Self {
        Self { depth: 0, max, label }
    }

    pub fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > self.max {
            self.depth -= 1;
            return Err(format!(
                "input exceeds compiler limit: {} = {}, max allowed = {}",
                self.label,
                self.depth + 1,
                self.max
            ));
        }
        Ok(())
    }

    pub fn exit(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}

pub fn fatal(msg: &str) -> ! {
    eprintln!("error: {}", msg);
    std::process::exit(1);
}
