pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    pub fn seed_from_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678_9ABC_DEF0)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.unit() * (hi - lo)
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..64).filter(|_| a.next_u64() == b.next_u64()).count();
        assert!(same < 2);
    }

    #[test]
    fn unit_stays_in_range() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let v = rng.unit();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn range_respects_bounds() {
        let mut rng = Rng::new(9);
        for _ in 0..10_000 {
            let v = rng.range(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&v));
        }
    }

    #[test]
    fn below_is_bounded() {
        let mut rng = Rng::new(11);
        for _ in 0..10_000 {
            assert!(rng.below(10) < 10);
        }
        assert_eq!(rng.below(0), 0);
    }

    #[test]
    fn zero_seed_is_remapped() {
        let mut rng = Rng::new(0);
        assert_ne!(rng.next_u64(), 0);
    }
}
