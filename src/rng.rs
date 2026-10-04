//! Источник случайности: криптографический генератор операционной системы.
//! Unix: чтение /dev/urandom. Windows: BCryptGenRandom. Внешних крейтов нет.

#[cfg(not(any(unix, windows)))]
compile_error!("skhoron-pwgen поддерживает только Unix и Windows");

pub trait RandomSource {
    fn next_u64(&mut self) -> u64;
}

pub struct OsRng {
    #[cfg(unix)]
    file: std::fs::File,
}

#[cfg(unix)]
impl OsRng {
    pub fn new() -> std::io::Result<Self> {
        Ok(OsRng {
            file: std::fs::File::open("/dev/urandom")?,
        })
    }
}

#[cfg(unix)]
impl RandomSource for OsRng {
    fn next_u64(&mut self) -> u64 {
        use std::io::Read;
        let mut buf = [0u8; 8];
        self.file
            .read_exact(&mut buf)
            .expect("не удалось прочитать /dev/urandom");
        u64::from_le_bytes(buf)
    }
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
impl OsRng {
    pub fn new() -> std::io::Result<Self> {
        Ok(OsRng {})
    }
}

#[cfg(windows)]
impl RandomSource for OsRng {
    fn next_u64(&mut self) -> u64 {
        const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
        let mut buf = [0u8; 8];
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        assert!(status == 0, "BCryptGenRandom вернул ошибку {status}");
        u64::from_le_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_rng_returns_different_values() {
        let mut rng = OsRng::new().unwrap();
        let a = rng.next_u64();
        let b = rng.next_u64();
        assert_ne!(a, b);
    }
}