
pub struct Rng {
	state: u64,
}

const MOD: u64 = 2_147_483_647;
const A: u64 = 48_271;

impl Rng {
	pub fn new(seed: u64) -> Rng {
		let mut s = seed % (MOD - 1);
		if s < 1 {
			s = 12_345;
		}
		Rng { state: s }
	}

	pub fn next_raw(&mut self) -> u64 {
		let lo = self.state % 256;
		let hi = (self.state - lo) / 256;
		let term1 = (A * lo) % MOD;
		let term2 = ((A * hi) % MOD * 256) % MOD;
		self.state = (term1 + term2) % MOD;
		if self.state == 0 {
			self.state = 1;
		}
		self.state
	}

	pub fn int(&mut self, min: i64, max: i64) -> i64 {
		if max < min {
			return min;
		}
		let span = (max - min + 1) as u64;
		min + (self.next_raw() as u128 * span as u128 / MOD as u128) as i64
	}

	pub fn shuffle<T: Clone>(&mut self, t: &mut Vec<T>) {
		for i in (1..t.len()).rev() {
			let j = self.int(0, i as i64) as usize;
			t.swap(i, j);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reproducible() {
		let mut a = Rng::new(42);
		let mut b = Rng::new(42);
		for _ in 0..1000 {
			assert_eq!(a.next_raw(), b.next_raw());
		}
	}

	#[test]
	fn range() {
		let mut r = Rng::new(7);
		for _ in 0..10000 {
			let v = r.int(3, 9);
			assert!((3..=9).contains(&v));
		}
	}
}
