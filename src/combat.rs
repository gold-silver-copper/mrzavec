use crate::rng::GameRng;

pub const STR_HIT: [i32; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 3,
];
pub const STR_DAMAGE: [i32; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 3, 4, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 6,
];
pub const WEAPON_DAMAGE: [&str; 9] = [
    "2x4", "3x4", "1x1", "1x1", "1x6", "4x4", "1x1", "1x2", "2x3",
];
pub const HURLED_DAMAGE: [&str; 9] = [
    "1x3", "1x2", "1x1", "2x3", "1x4", "1x2", "1x3", "2x4", "1x6",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack {
    pub level: i32,
    pub strength: i32,
    pub hit_bonus: i32,
    pub damage_bonus: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub hit: bool,
    pub damage: i32,
}

pub fn parse_damage(value: &str) -> Option<Vec<(u32, u32)>> {
    value
        .split('/')
        .map(|part| {
            let (number, sides) = part.split_once('x')?;
            Some((number.parse().ok()?, sides.parse().ok()?))
        })
        .collect()
}

pub fn swing(rng: &mut GameRng, attacker_level: i32, defender_armor: i32, bonus: i32) -> bool {
    let need = (20 - attacker_level) - defender_armor;
    rng.rnd(20) as i32 + bonus >= need
}

/// Implements `roll_em`: every slash-separated damage group makes an
/// independent attack roll and successful groups add their damage.
pub fn resolve(
    rng: &mut GameRng,
    attack: Attack,
    defender_armor: i32,
    damage: &str,
    defender_running: bool,
) -> i32 {
    resolve_outcome(rng, attack, defender_armor, damage, defender_running).damage
}

/// Resolve both whether any damage group hit and the resulting damage. Rogue
/// uses successful `0x0` attacks for monster effects, so zero damage does not
/// imply a miss.
pub fn resolve_outcome(
    rng: &mut GameRng,
    attack: Attack,
    defender_armor: i32,
    damage: &str,
    defender_running: bool,
) -> Outcome {
    let strength = attack.strength.clamp(0, 31) as usize;
    let sleeping_bonus = if defender_running { 0 } else { 4 };
    let mut outcome = Outcome {
        hit: false,
        damage: 0,
    };
    for (number, sides) in parse_damage(damage).unwrap_or_default() {
        if swing(
            rng,
            attack.level,
            defender_armor,
            attack.hit_bonus + STR_HIT[strength] + sleeping_bonus,
        ) {
            outcome.hit = true;
            outcome.damage +=
                (rng.roll(number, sides) + attack.damage_bonus + STR_DAMAGE[strength]).max(0);
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_original_dice_forms() {
        assert_eq!(
            parse_damage("1x8/1x8/2x6"),
            Some(vec![(1, 8), (1, 8), (2, 6)])
        );
        assert_eq!(parse_damage("0x0/0x0"), Some(vec![(0, 0), (0, 0)]));
        assert_eq!(parse_damage("nonsense"), None);
    }

    #[test]
    fn damage_is_seeded_and_nonnegative() {
        let attack = Attack {
            level: 20,
            strength: 16,
            hit_bonus: 0,
            damage_bonus: -100,
        };
        assert_eq!(resolve(&mut GameRng::new(4), attack, 10, "3x4", true), 0);
    }

    #[test]
    fn zero_dice_can_hit_without_dealing_damage() {
        let attack = Attack {
            level: 100,
            strength: 10,
            hit_bonus: 0,
            damage_bonus: 0,
        };
        let outcome = resolve_outcome(&mut GameRng::new(1), attack, 9, "0x0", true);
        assert!(outcome.hit);
        assert_eq!(outcome.damage, 0);
    }

    #[test]
    fn sleeping_defender_gets_four_point_hit_penalty() {
        let attack = Attack {
            level: 1,
            strength: 16,
            hit_bonus: 0,
            damage_bonus: 0,
        };
        let mut running_hits = 0;
        let mut sleeping_hits = 0;
        for seed in 1..=1000 {
            running_hits += (resolve(&mut GameRng::new(seed), attack, 5, "1x1", true) > 0) as i32;
            sleeping_hits += (resolve(&mut GameRng::new(seed), attack, 5, "1x1", false) > 0) as i32;
        }
        assert!(sleeping_hits > running_hits);
    }
}
