//! RNG на основе ChaCha20 с энтропией от ОС.
//! Каждые 2 МиБ в состояние подмешиваются новые 32 байта от ОС.

#[cfg(not(any(unix, windows)))]
compile_error!("skhoron-pwgen поддерживает только Unix и Windows");

pub trait RandomSource {
    fn next_u64(&mut self) -> std::io::Result<u64>;
}

fn zeroize(buf: &mut [u8]) {
    for byte in buf {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] ^= s[a];
    s[d] = s[d].rotate_left(16);

    s[c] = s[c].wrapping_add(s[d]);
    s[b] ^= s[c];
    s[b] = s[b].rotate_left(12);

    s[a] = s[a].wrapping_add(s[b]);
    s[d] ^= s[a];
    s[d] = s[d].rotate_left(8);

    s[c] = s[c].wrapping_add(s[d]);
    s[b] ^= s[c];
    s[b] = s[b].rotate_left(7);
}

fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut init = [0u32; 16];
    init[0] = 0x6170_7865;
    init[1] = 0x3320_646e;
    init[2] = 0x7962_2d32;
    init[3] = 0x6b20_6574;
    for (i, chunk) in key.chunks_exact(4).enumerate() {
        init[4 + i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }
    init[12] = counter;
    for (i, chunk) in nonce.chunks_exact(4).enumerate() {
        init[13 + i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }

    let mut s = init;
    for _ in 0..10 {
        quarter_round(&mut s, 0, 4, 8, 12);
        quarter_round(&mut s, 1, 5, 9, 13);
        quarter_round(&mut s, 2, 6, 10, 14);
        quarter_round(&mut s, 3, 7, 11, 15);
        quarter_round(&mut s, 0, 5, 10, 15);
        quarter_round(&mut s, 1, 6, 11, 12);
        quarter_round(&mut s, 2, 7, 8, 13);
        quarter_round(&mut s, 3, 4, 9, 14);
    }
    for (x, y) in s.iter_mut().zip(init.iter()) {
        *x = x.wrapping_add(*y);
    }

    let mut out = [0u8; 64];
    for (chunk, word) in out.chunks_exact_mut(4).zip(s.iter()) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    out
}


#[cfg(unix)]
fn os_fill(buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buf)
}

#[cfg(windows)]
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(
        algorithm: *mut core::ffi::c_void,
        buffer: *mut u8,
        length: u32,
        flags: u32,
    ) -> i32;
}

#[cfg(windows)]
fn os_fill(buf: &mut [u8]) -> std::io::Result<()> {
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("BCryptGenRandom вернул ошибку {status}"),
        ))
    }
}

const RESEED_EVERY_BLOCKS: u32 = 1 << 16;

pub struct SkhoronRng {
    key: [u8; 32],
    buf: [u8; 32],
    pos: usize,
    blocks: u32,
}

impl SkhoronRng {
    pub fn new() -> std::io::Result<Self> {
        let mut key = [0u8; 32];
        if let Err(e) = os_fill(&mut key) {
            zeroize(&mut key);
            return Err(e);
        }
        Ok(SkhoronRng {
            key,
            buf: [0u8; 32],
            pos: 32,
            blocks: 0,
        })
    }

    fn refill(&mut self) -> std::io::Result<()> {
        let mut block = chacha20_block(&self.key, 0, &[0u8; 12]);
        self.key.copy_from_slice(&block[..32]);
        self.buf.copy_from_slice(&block[32..]);
        zeroize(&mut block);
        self.pos = 0;
        self.blocks += 1;
        if self.blocks >= RESEED_EVERY_BLOCKS {
            self.reseed()?;
        }
        Ok(())
    }

    fn reseed(&mut self) -> std::io::Result<()> {
        let mut fresh = [0u8; 32];
        if let Err(e) = os_fill(&mut fresh) {
            zeroize(&mut fresh);
            return Err(e);
        }
        for (k, f) in self.key.iter_mut().zip(fresh.iter()) {
            *k ^= *f;
        }
        zeroize(&mut fresh);
        self.blocks = 0;
        Ok(())
    }
}

impl RandomSource for SkhoronRng {
    fn next_u64(&mut self) -> std::io::Result<u64> {
        if self.pos >= self.buf.len() {
            self.refill()?;
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.buf[self.pos..self.pos + 8]);
        zeroize(&mut self.buf[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(u64::from_le_bytes(bytes))
    }
}

impl Drop for SkhoronRng {
    fn drop(&mut self) {
        zeroize(&mut self.key);
        zeroize(&mut self.buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn chacha20_rfc8439_section_2_3_2() {
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = i as u8;
        }
        let nonce: [u8; 12] = hex("000000090000004a00000000").try_into().unwrap();
        let expected = hex(
            "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e\
             d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e",
        );
        assert_eq!(chacha20_block(&key, 1, &nonce).to_vec(), expected);
    }

    #[test]
    fn chacha20_rfc8439_appendix_a1_vector_1() {
        let expected = hex(
            "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7\
             da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586",
        );
        assert_eq!(
            chacha20_block(&[0u8; 32], 0, &[0u8; 12]).to_vec(),
            expected
        );
    }

    #[test]
    fn output_values_are_distinct() {
        let mut rng = SkhoronRng::new().unwrap();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            assert!(seen.insert(rng.next_u64().unwrap()));
        }
    }

    #[test]
    fn key_is_replaced_after_every_block() {
        let mut rng = SkhoronRng::new().unwrap();
        let before = rng.key;
        rng.next_u64().unwrap();
        assert_ne!(before, rng.key);
    }

    #[test]
    fn reseed_path_works() {
        let mut rng = SkhoronRng::new().unwrap();
        rng.blocks = RESEED_EVERY_BLOCKS - 1;
        for _ in 0..8 {
            rng.next_u64().unwrap();
        }
        assert!(rng.blocks < RESEED_EVERY_BLOCKS);
    }

    #[test]
    fn bits_are_balanced() {
        let mut rng = SkhoronRng::new().unwrap();
        let n = 100_000u64;
        let ones: u64 = (0..n)
            .map(|_| rng.next_u64().unwrap().count_ones() as u64)
            .sum();
        let expected = n * 32;
        // сигма ≈ sqrt(n*64)/2 ≈ 1265, допуск около 8 сигм
        assert!(ones.abs_diff(expected) < 10_000, "ones = {ones}");
    }
}