//! Pre-compilation resource budget limits calibrated via §0.4 binary search.

pub const MAX_TOPLEVEL_FUNC_DECLS: usize = 32; // calibrated from bad=100: low=41, high=42
pub const MAX_TOPLEVEL_STMTS: usize = 2385; // calibrated from bad=3000: low=2982, high=2983
pub const MAX_CLOSURE_NEST_DEPTH: usize = 15; // calibrated from bad=20: low=19, high=20
pub const MAX_EXPR_PARSE_DEPTH: usize = 2906; // calibrated from bad=20000: low=3633, high=3634

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
