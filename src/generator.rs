use crate::rng::RandomSource;

pub const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
pub const DIGITS: &str = "0123456789";
pub const SYMBOLS: &str = "!@#$%^&*";
pub const SIMILAR: &str = "0O1lI";

pub const MAX_LENGTH: usize = 4096;
pub const MAX_COUNT: usize = 10_000;

#[derive(Clone, Debug)]
pub struct CharsetOptions {
    pub upper: bool,
    pub lower: bool,
    pub digits: bool,
    pub symbols: bool,
    pub no_similar: bool,
    pub exclude: String,
}

impl Default for CharsetOptions {
    fn default() -> Self {
        CharsetOptions {
            upper: true,
            lower: true,
            digits: true,
            symbols: true,
            no_similar: false,
            exclude: String::new(),
        }
    }
}

pub fn build_charset(opts: &CharsetOptions) -> Result<Vec<char>, String> {
    let mut all = String::new();

    if opts.upper {
        all.push_str(UPPER);
    }
    if opts.lower {
        all.push_str(LOWER);
    }
    if opts.digits {
        all.push_str(DIGITS);
    }
    if opts.symbols {
        all.push_str(SYMBOLS);
    }

    let mut out: Vec<char> = Vec::new();

    for c in all.chars() {
        if opts.no_similar && SIMILAR.contains(c) {
            continue;
        }
        if opts.exclude.contains(c) {
            continue;
        }
        if !out.contains(&c) {
            out.push(c);
        }
    }

    if out.is_empty() {
        return Err(
            "набор символов пуст: включи хотя бы один набор или убери исключения".to_string(),
        );
    }

    Ok(out)
}

pub fn check_params(
    length: usize,
    count: usize,
    charset_len: usize,
    repeats: bool,
) -> Result<(), String> {
    if length == 0 || length > MAX_LENGTH {
        return Err(format!("длина должна быть от 1 до {MAX_LENGTH}"));
    }
    if count == 0 || count > MAX_COUNT {
        return Err(format!("количество должно быть от 1 до {MAX_COUNT}"));
    }
    if !repeats && length > charset_len {
        return Err(format!(
            "без повторов длина не может превышать размер набора: в наборе {charset_len} символов, запрошено {length}"
        ));
    }

    Ok(())
}

/// Равномерное число в диапазоне 0..n.
pub fn gen_below<R: RandomSource>(rng: &mut R, n: u64) -> Result<u64, String> {
    if n == 0 {
        return Err("gen_below: n должно быть больше нуля".to_string());
    }

    let threshold = n.wrapping_neg() % n;

    loop {
        let x = rng
            .next_u64()
            .map_err(|e| format!("ошибка источника случайности: {e}"))?;

        if x >= threshold {
            return Ok(x % n);
        }
    }
}

pub fn generate<R: RandomSource>(
    rng: &mut R,
    charset: &[char],
    length: usize,
    repeats: bool,
) -> Result<String, String> {
    check_params(length, 1, charset.len(), repeats)?;

    let mut out = String::with_capacity(length);

    if repeats {
        for _ in 0..length {
            let i = gen_below(rng, charset.len() as u64)? as usize;
            out.push(charset[i]);
        }
    } else {
        let mut pool = charset.to_vec();

        for _ in 0..length {
            let i = gen_below(rng, pool.len() as u64)? as usize;
            out.push(pool.swap_remove(i));
        }
    }

    Ok(out)
}

/// Энтропия пароля в битах при равновероятном выборе.
pub fn entropy_bits(charset_len: usize, length: usize, repeats: bool) -> f64 {
    if repeats {
        length as f64 * (charset_len as f64).log2()
    } else {
        (0..length.min(charset_len))
            .map(|i| ((charset_len - i) as f64).log2())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct XorShift(u64);

    impl RandomSource for XorShift {
        fn next_u64(&mut self) -> std::io::Result<u64> {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            Ok(x)
        }
    }

    fn rng() -> XorShift {
        XorShift(0x9E37_79B9_7F4A_7C15)
    }

    #[test]
    fn default_charset_has_70_chars() {
        let cs = build_charset(&CharsetOptions::default()).unwrap();
        assert_eq!(cs.len(), 70);
    }

    #[test]
    fn no_similar_removes_ambiguous_chars() {
        let opts = CharsetOptions {
            no_similar: true,
            ..CharsetOptions::default()
        };
        let cs = build_charset(&opts).unwrap();
        assert_eq!(cs.len(), 65);

        for c in SIMILAR.chars() {
            assert!(!cs.contains(&c));
        }
    }

    #[test]
    fn exclude_removes_given_chars() {
        let opts = CharsetOptions {
            exclude: "abc!".to_string(),
            ..CharsetOptions::default()
        };
        let cs = build_charset(&opts).unwrap();
        assert_eq!(cs.len(), 66);
        assert!(!cs.contains(&'a') && !cs.contains(&'!'));
    }

    #[test]
    fn empty_charset_is_error() {
        let opts = CharsetOptions {
            upper: false,
            lower: false,
            digits: false,
            symbols: false,
            ..CharsetOptions::default()
        };
        assert!(build_charset(&opts).is_err());
    }

    #[test]
    fn gen_below_stays_in_range_and_covers_all_values() {
        let mut r = rng();
        let mut seen = [false; 7];

        for _ in 0..2000 {
            let v = gen_below(&mut r, 7).unwrap();
            assert!(v < 7);
            seen[v as usize] = true;
        }

        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn gen_below_rejects_zero() {
        let mut r = rng();
        assert!(gen_below(&mut r, 0).is_err());
    }

    #[test]
    fn with_repeats_has_right_length_and_alphabet() {
        let cs = build_charset(&CharsetOptions::default()).unwrap();
        let pw = generate(&mut rng(), &cs, 200, true).unwrap();

        assert_eq!(pw.chars().count(), 200);
        assert!(pw.chars().all(|c| cs.contains(&c)));
    }

    #[test]
    fn without_repeats_all_chars_unique() {
        let cs = build_charset(&CharsetOptions::default()).unwrap();
        let pw = generate(&mut rng(), &cs, 50, false).unwrap();
        let unique: HashSet<char> = pw.chars().collect();

        assert_eq!(pw.chars().count(), 50);
        assert_eq!(unique.len(), 50);
    }

    #[test]
    fn without_repeats_full_length_is_a_permutation() {
        let cs = vec!['a', 'b', 'c'];
        let pw = generate(&mut rng(), &cs, 3, false).unwrap();
        let mut chars: Vec<char> = pw.chars().collect();

        chars.sort();
        assert_eq!(chars, cs);
    }

    #[test]
    fn no_repeat_longer_than_charset_is_error() {
        let cs = vec!['a', 'b', 'c'];

        assert!(generate(&mut rng(), &cs, 4, false).is_err());
        assert!(generate(&mut rng(), &cs, 4, true).is_ok());
    }

    #[test]
    fn zero_and_huge_length_are_errors() {
        let cs = vec!['a', 'b'];

        assert!(generate(&mut rng(), &cs, 0, true).is_err());
        assert!(generate(&mut rng(), &cs, MAX_LENGTH + 1, true).is_err());
    }

    struct Seq(Vec<u64>, usize);

    impl RandomSource for Seq {
        fn next_u64(&mut self) -> std::io::Result<u64> {
            let v = self.0[self.1];
            self.1 += 1;
            Ok(v)
        }
    }

    #[test]
    fn gen_below_rejects_rejection_threshold() {
        let mut r = Seq(vec![0, 5], 0);

        assert_eq!(gen_below(&mut r, 3).unwrap(), 2);
        assert_eq!(r.1, 2);
    }

    struct FailingRng;

    impl RandomSource for FailingRng {
        fn next_u64(&mut self) -> std::io::Result<u64> {
            Err(std::io::Error::other("test RNG failure"))
        }
    }

    #[test]
    fn random_source_errors_are_propagated() {
        let mut r = FailingRng;
        assert!(generate(&mut r, &['a', 'b'], 8, true).is_err());
    }

    #[test]
    fn entropy_does_not_underflow_on_oversized_length() {
        assert!(entropy_bits(3, 5, false).is_finite());
    }

    #[test]
    fn entropy_values() {
        assert!((entropy_bits(64, 10, true) - 60.0).abs() < 1e-9);
        assert!((entropy_bits(3, 3, false) - 6f64.log2()).abs() < 1e-9);
    }
}