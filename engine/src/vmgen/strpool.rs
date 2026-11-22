
use std::collections::HashMap;

pub struct StrPool {
	use_ms: bool,
	entries: Vec<String>,
	index: HashMap<String, usize>,
}

impl StrPool {
	pub fn new(use_ms: bool) -> StrPool {
		StrPool { use_ms, entries: Vec::new(), index: HashMap::new() }
	}
	pub fn lit(&mut self, s: &str) -> String {
		if !self.use_ms {
			return format!(
				"\"{}\"",
				crate::printer::print_string_bytes(s.as_bytes(), false)
			);
		}
		let n = self.entries.len();
		let e = *self.index.entry(s.to_string()).or_insert(n);
		if e == n {
			self.entries.push(s.to_string());
		}
		format!("MS[{e}]", e = e + 1)
	}
	pub fn boot_block(&self) -> String {
		let mut s = String::from("local MS = {}\n");
		for (i, e) in self.entries.iter().enumerate() {
			let codes: Vec<String> = e.bytes().map(|b| b.to_string()).collect();
			s.push_str(&format!("  MS[{}] = CHAR({})\n", i + 1, codes.join(", ")));
		}
		s
	}
}

#[cfg(test)]
mod tests {
	use super::StrPool;

	#[test]
	fn inline_literals_escape_quotes_and_backslashes() {
		let mut pool = StrPool::new(false);
		let literal = pool.lit(" with 'quote' and \\\"slash\\\"");
		assert!(crate::parser::parse(&format!("local s={literal}"), false).is_ok());
	}
}
