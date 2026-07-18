use serde::{Deserialize, Serialize};

/// Serializable deterministic generator used by the simulation and save files.
/// The C implementations used platform-specific `random`; gameplay only relies
/// on uniform bounded draws, so a fixed generator makes Rust saves portable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRng {
    state: u64,
}

impl GameRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }

    pub fn rnd(&mut self, range: u32) -> u32 {
        if range == 0 {
            0
        } else {
            self.next_u32() % range
        }
    }

    pub fn roll(&mut self, number: u32, sides: u32) -> i32 {
        (0..number).map(|_| self.rnd(sides) as i32 + 1).sum()
    }

    pub fn spread(&mut self, number: i32) -> i32 {
        number - number / 20 + self.rnd((number / 10) as u32) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_stream_is_repeatable() {
        let mut a = GameRng::new(42);
        let mut b = GameRng::new(42);
        assert_eq!(
            (0..100).map(|_| a.next_u32()).collect::<Vec<_>>(),
            (0..100).map(|_| b.next_u32()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn dice_stay_in_range() {
        let mut rng = GameRng::new(7);
        for _ in 0..100 {
            assert!((3..=18).contains(&rng.roll(3, 6)));
        }
    }

    #[test]
    fn spread_matches_the_original_exclusive_upper_bound() {
        let mut rng = GameRng::new(8);
        for _ in 0..1_000 {
            assert!((67..=73).contains(&rng.spread(70)));
            assert_eq!(rng.spread(5), 5);
        }
    }
}
