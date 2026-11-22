use std::io;

pub struct Rng {
	key: [u32; 8],
	nonce: [u32; 3],
	counter: u32,
	block: [u32; 16],
	index: usize,
}

const CHACHA_CONST: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

impl Rng {
	pub fn new(seed: u64) -> Rng {
		let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
		let mut key_bytes = [0u8; 32];
		for chunk in key_bytes.chunks_exact_mut(8) {
			state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
			let mut z = state;
			z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
			z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
			z ^= z >> 31;
			chunk.copy_from_slice(&z.to_le_bytes());
		}
		Rng::from_material(&key_bytes, &[0; 12])
	}

	pub fn from_entropy() -> Result<Rng, String> {
		let mut material = [0u8; 44];
		fill_entropy(&mut material).map_err(|e| format!("system entropy unavailable: {e}"))?;
		let mut key = [0u8; 32];
		let mut nonce = [0u8; 12];
		key.copy_from_slice(&material[..32]);
		nonce.copy_from_slice(&material[32..]);
		Ok(Rng::from_material(&key, &nonce))
	}

	fn from_material(key_bytes: &[u8; 32], nonce_bytes: &[u8; 12]) -> Rng {
		let mut key = [0u32; 8];
		for (i, word) in key.iter_mut().enumerate() {
			let start = i * 4;
			*word = u32::from_le_bytes([
				key_bytes[start],
				key_bytes[start + 1],
				key_bytes[start + 2],
				key_bytes[start + 3],
			]);
		}
		let mut nonce = [0u32; 3];
		for (i, word) in nonce.iter_mut().enumerate() {
			let start = i * 4;
			*word = u32::from_le_bytes([
				nonce_bytes[start],
				nonce_bytes[start + 1],
				nonce_bytes[start + 2],
				nonce_bytes[start + 3],
			]);
		}
		Rng {
			key,
			nonce,
			counter: 0,
			block: [0; 16],
			index: 16,
		}
	}

	pub fn fork(&mut self, domain: u64) -> Rng {
		let mut key_bytes = [0u8; 32];
		for (i, chunk) in key_bytes.chunks_exact_mut(8).enumerate() {
			let salt = 0xd1b5_4a32_d192_ed03u64.wrapping_mul(i as u64 + 1);
			let word = self.next_raw() ^ domain.rotate_left((i * 13) as u32) ^ salt;
			chunk.copy_from_slice(&word.to_le_bytes());
		}
		let domain_bytes = domain.to_le_bytes();
		let nonce = [
			domain_bytes[0],
			domain_bytes[1],
			domain_bytes[2],
			domain_bytes[3],
			domain_bytes[4],
			domain_bytes[5],
			domain_bytes[6],
			domain_bytes[7],
			0xa5,
			0x5a,
			0x3c,
			0xc3,
		];
		Rng::from_material(&key_bytes, &nonce)
	}

	fn next_raw(&mut self) -> u64 {
		let lo = self.next_u32() as u64;
		let hi = self.next_u32() as u64;
		lo | (hi << 32)
	}

	pub fn int(&mut self, min: i64, max: i64) -> i64 {
		if max < min {
			return min;
		}
		let span = (max as i128 - min as i128 + 1) as u128;
		let range = 1u128 << 64;
		let limit = range - range % span;
		loop {
			let value = self.next_raw() as u128;
			if value < limit {
				return (min as i128 + (value % span) as i128) as i64;
			}
		}
	}

	pub fn shuffle<T: Clone>(&mut self, t: &mut Vec<T>) {
		for i in (1..t.len()).rev() {
			let j = self.int(0, i as i64) as usize;
			t.swap(i, j);
		}
	}

	fn next_u32(&mut self) -> u32 {
		if self.index == 16 {
			self.block = chacha20_block(&self.key, &self.nonce, self.counter);
			self.counter = self.counter.wrapping_add(1);
			self.index = 0;
		}
		let value = self.block[self.index];
		self.index += 1;
		value
	}
}

fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
	state[a] = state[a].wrapping_add(state[b]);
	state[d] = (state[d] ^ state[a]).rotate_left(16);
	state[c] = state[c].wrapping_add(state[d]);
	state[b] = (state[b] ^ state[c]).rotate_left(12);
	state[a] = state[a].wrapping_add(state[b]);
	state[d] = (state[d] ^ state[a]).rotate_left(8);
	state[c] = state[c].wrapping_add(state[d]);
	state[b] = (state[b] ^ state[c]).rotate_left(7);
}

fn chacha20_block(key: &[u32; 8], nonce: &[u32; 3], counter: u32) -> [u32; 16] {
	let mut initial = [0u32; 16];
	initial[..4].copy_from_slice(&CHACHA_CONST);
	initial[4..12].copy_from_slice(key);
	initial[12] = counter;
	initial[13..16].copy_from_slice(nonce);
	let mut state = initial;
	for _ in 0..10 {
		quarter_round(&mut state, 0, 4, 8, 12);
		quarter_round(&mut state, 1, 5, 9, 13);
		quarter_round(&mut state, 2, 6, 10, 14);
		quarter_round(&mut state, 3, 7, 11, 15);
		quarter_round(&mut state, 0, 5, 10, 15);
		quarter_round(&mut state, 1, 6, 11, 12);
		quarter_round(&mut state, 2, 7, 8, 13);
		quarter_round(&mut state, 3, 4, 9, 14);
	}
	for i in 0..16 {
		state[i] = state[i].wrapping_add(initial[i]);
	}
	state
}

#[cfg(unix)]
fn fill_entropy(bytes: &mut [u8]) -> io::Result<()> {
	use std::io::Read;
	let mut source = std::fs::File::open("/dev/urandom")?;
	source.read_exact(bytes)
}

#[cfg(windows)]
fn fill_entropy(bytes: &mut [u8]) -> io::Result<()> {
	use std::ffi::c_void;
	#[link(name = "bcrypt")]
	extern "system" {
		fn BCryptGenRandom(
			algorithm: *mut c_void,
			buffer: *mut u8,
			length: u32,
			flags: u32,
		) -> i32;
	}
	let status = unsafe {
		BCryptGenRandom(
			std::ptr::null_mut(),
			bytes.as_mut_ptr(),
			bytes.len() as u32,
			2,
		)
	};
	if status >= 0 {
		Ok(())
	} else {
		Err(io::Error::from_raw_os_error(status))
	}
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
fn fill_entropy(bytes: &mut [u8]) -> io::Result<()> {
	#[link(wasm_import_module = "wasi_snapshot_preview1")]
	extern "C" {
		#[link_name = "random_get"]
		fn wasi_random_get(buffer: *mut u8, length: usize) -> u16;
	}
	let status = unsafe { wasi_random_get(bytes.as_mut_ptr(), bytes.len()) };
	if status == 0 {
		Ok(())
	} else {
		Err(io::Error::from_raw_os_error(status as i32))
	}
}

#[cfg(not(any(unix, windows, all(target_arch = "wasm32", target_os = "wasi"))))]
fn fill_entropy(_bytes: &mut [u8]) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"no system entropy source is implemented for this target",
	))
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

	#[test]
	fn signed_range_and_invalid_range() {
		let mut r = Rng::new(11);
		for _ in 0..1000 {
			let value = r.int(-17, -4);
			assert!((-17..=-4).contains(&value));
		}
		assert_eq!(r.int(9, 3), 9);
	}

	#[test]
	fn fork_is_reproducible_and_domain_separated() {
		let mut a = Rng::new(123);
		let mut b = Rng::new(123);
		let mut c = a.fork(1);
		let mut d = b.fork(1);
		let mut e = Rng::new(123).fork(2);
		for _ in 0..32 {
			assert_eq!(c.next_raw(), d.next_raw());
		}
		assert_ne!(c.next_raw(), e.next_raw());
	}

	#[test]
	fn chacha20_rfc8439_block_vector() {
		let key = [
			0x0302_0100,
			0x0706_0504,
			0x0b0a_0908,
			0x0f0e_0d0c,
			0x1312_1110,
			0x1716_1514,
			0x1b1a_1918,
			0x1f1e_1d1c,
		];
		let nonce = [0x0900_0000, 0x4a00_0000, 0x0000_0000];
		let block = chacha20_block(&key, &nonce, 1);
		assert_eq!(
			block,
			[
				0xe4e7_f110,
				0x1559_3bd1,
				0x1fdd_0f50,
				0xc471_20a3,
				0xc7f4_d1c7,
				0x0368_c033,
				0x9aaa_2204,
				0x4e6c_d4c3,
				0x4664_82d2,
				0x09aa_9f07,
				0x05d7_c214,
				0xa202_8bd9,
				0xd19c_12b5,
				0xb94e_16de,
				0xe883_d0cb,
				0x4e3c_50a2,
			]
		);
	}

	#[test]
	fn entropy_initializes_independent_streams() {
		let mut a = Rng::from_entropy().expect("OS entropy must be available for the test");
		let mut b = Rng::from_entropy().expect("OS entropy must be available for the test");
		let a_sample: Vec<u64> = (0..4).map(|_| a.next_raw()).collect();
		let b_sample: Vec<u64> = (0..4).map(|_| b.next_raw()).collect();
		assert_ne!(a_sample, b_sample);
	}
}
