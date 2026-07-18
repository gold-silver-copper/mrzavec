use crate::{map::Pos, rng::GameRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Potion,
    Scroll,
    Food,
    Weapon,
    Armor,
    Ring,
    Stick,
    Amulet,
    Gold,
    Bizarre(char),
}

impl ItemKind {
    pub fn glyph(self) -> char {
        match self {
            Self::Potion => '!',
            Self::Scroll => '?',
            Self::Food => ':',
            Self::Weapon => ')',
            Self::Armor => ']',
            Self::Ring => '=',
            Self::Stick => '/',
            Self::Amulet => ',',
            Self::Gold => '*',
            Self::Bizarre(glyph) => glyph,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub kind: ItemKind,
    pub which: u8,
    pub pos: Option<Pos>,
    pub count: u32,
    pub group: u64,
    pub pack_letter: Option<char>,
    #[serde(default = "default_in_pack")]
    pub in_pack: bool,
    pub charges: i16,
    pub hit_plus: i16,
    pub damage_plus: i16,
    pub armor_class: Option<i32>,
    #[serde(default)]
    pub gold_value: i32,
    pub cursed: bool,
    pub known: bool,
    pub protected: bool,
    pub label: Option<String>,
    pub dropped_once: bool,
}

impl Item {
    pub fn basic(id: u64, kind: ItemKind, which: u8) -> Self {
        Self {
            id,
            kind,
            which,
            pos: None,
            count: 1,
            group: 0,
            pack_letter: None,
            in_pack: true,
            charges: 0,
            hit_plus: 0,
            damage_plus: 0,
            armor_class: None,
            gold_value: 0,
            cursed: false,
            known: false,
            protected: false,
            label: None,
            dropped_once: false,
        }
    }
    pub fn gold(id: u64, value: i32) -> Self {
        let mut item = Self::basic(id, ItemKind::Gold, 0);
        item.gold_value = value;
        item
    }
    pub fn stacks_with(&self, other: &Self) -> bool {
        (matches!(
            self.kind,
            ItemKind::Potion | ItemKind::Scroll | ItemKind::Food
        ) && self.kind == other.kind
            && self.which == other.which)
            || (self.kind == ItemKind::Weapon
                && other.kind == ItemKind::Weapon
                && self.which == other.which
                && self.group != 0
                && self.group == other.group)
    }
}

fn default_in_pack() -> bool {
    true
}

pub const KIND_WEIGHTS: [u32; 7] = [26, 36, 16, 7, 7, 4, 4];
pub const POTION_WEIGHTS: [u32; 14] = [7, 8, 8, 13, 3, 13, 6, 6, 2, 5, 5, 13, 5, 6];
pub const SCROLL_WEIGHTS: [u32; 18] = [7, 4, 2, 3, 7, 10, 10, 6, 7, 10, 3, 2, 5, 8, 4, 7, 3, 2];
pub const RING_WEIGHTS: [u32; 14] = [9, 9, 5, 10, 10, 1, 10, 8, 8, 4, 9, 5, 7, 5];
pub const STICK_WEIGHTS: [u32; 14] = [12, 6, 3, 3, 3, 15, 10, 10, 11, 9, 1, 6, 6, 5];
pub const WEAPON_WEIGHTS: [u32; 9] = [11, 11, 12, 12, 8, 10, 12, 12, 12];
pub const POTION_NAMES: [&str; 14] = [
    "confusion",
    "hallucination",
    "poison",
    "gain strength",
    "see invisible",
    "healing",
    "monster detection",
    "magic detection",
    "raise level",
    "extra healing",
    "haste self",
    "restore strength",
    "blindness",
    "levitation",
];
pub const SCROLL_NAMES: [&str; 18] = [
    "monster confusion",
    "magic mapping",
    "hold monster",
    "sleep",
    "enchant armor",
    "identify potion",
    "identify scroll",
    "identify weapon",
    "identify armor",
    "identify ring, wand or staff",
    "scare monster",
    "food detection",
    "teleportation",
    "enchant weapon",
    "create monster",
    "remove curse",
    "aggravate monsters",
    "protect armor",
];
pub const RING_NAMES: [&str; 14] = [
    "protection",
    "add strength",
    "sustain strength",
    "searching",
    "see invisible",
    "adornment",
    "aggravate monster",
    "dexterity",
    "increase damage",
    "regeneration",
    "slow digestion",
    "teleportation",
    "stealth",
    "maintain armor",
];
pub const STICK_NAMES: [&str; 14] = [
    "light",
    "invisibility",
    "lightning",
    "fire",
    "cold",
    "polymorph",
    "magic missile",
    "haste monster",
    "slow monster",
    "drain life",
    "nothing",
    "teleport away",
    "teleport to",
    "cancellation",
];
pub const WEAPON_NAMES: [&str; 9] = [
    "mace",
    "long sword",
    "short bow",
    "arrow",
    "dagger",
    "two handed sword",
    "dart",
    "shuriken",
    "spear",
];
pub const ARMOR_NAMES: [&str; 8] = [
    "leather armor",
    "ring mail",
    "studded leather armor",
    "scale mail",
    "chain mail",
    "splint mail",
    "banded mail",
    "plate mail",
];
pub const ARMOR_WEIGHTS: [u32; 8] = [20, 15, 15, 13, 12, 10, 10, 5];
pub const ARMOR_CLASS: [i32; 8] = [8, 7, 7, 6, 5, 4, 4, 3];

pub fn weighted_index(rng: &mut GameRng, weights: &[u32]) -> usize {
    let total: u32 = weights.iter().sum();
    let mut roll = rng.rnd(total);
    for (i, &w) in weights.iter().enumerate() {
        if roll < w {
            return i;
        }
        roll -= w;
    }
    weights.len() - 1
}

pub fn generate(rng: &mut GameRng, id: u64, no_food: u8) -> Item {
    let category = if no_food > 3 {
        2
    } else {
        weighted_index(rng, &KIND_WEIGHTS)
    };
    let kind = [
        ItemKind::Potion,
        ItemKind::Scroll,
        ItemKind::Food,
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Ring,
        ItemKind::Stick,
    ][category];
    let which = match kind {
        ItemKind::Potion => weighted_index(rng, &POTION_WEIGHTS),
        ItemKind::Armor => weighted_index(rng, &ARMOR_WEIGHTS),
        ItemKind::Food if rng.rnd(10) == 0 => 1,
        ItemKind::Food => 0,
        ItemKind::Scroll => weighted_index(rng, &SCROLL_WEIGHTS),
        ItemKind::Weapon => weighted_index(rng, &WEAPON_WEIGHTS),
        ItemKind::Ring => weighted_index(rng, &RING_WEIGHTS),
        ItemKind::Stick => weighted_index(rng, &STICK_WEIGHTS),
        _ => 0,
    };
    let mut item = Item::basic(id, kind, which as u8);
    if kind == ItemKind::Armor {
        item.armor_class = Some(ARMOR_CLASS[which]);
        let r = rng.rnd(100);
        if r < 20 {
            item.cursed = true;
            item.armor_class = item.armor_class.map(|x| x + rng.rnd(3) as i32 + 1)
        } else if r < 28 {
            item.armor_class = item.armor_class.map(|x| x - rng.rnd(3) as i32 - 1)
        }
    }
    if kind == ItemKind::Weapon {
        item.count = match which {
            4 => rng.rnd(4) + 2,
            3 | 6 | 7 => rng.rnd(8) + 8,
            _ => 1,
        };
        if matches!(which, 3 | 4 | 6 | 7) {
            item.group = id;
        }
        let r = rng.rnd(100);
        if r < 10 {
            item.cursed = true;
            item.hit_plus = -(rng.rnd(3) as i16 + 1)
        } else if r < 15 {
            item.hit_plus = rng.rnd(3) as i16 + 1
        }
    }
    if kind == ItemKind::Stick {
        item.charges = if which == 0 {
            (rng.rnd(10) + 10) as i16
        } else {
            (rng.rnd(5) + 3) as i16
        };
    }
    if kind == ItemKind::Ring {
        if matches!(which, 0 | 1 | 7 | 8) {
            let value = rng.rnd(3) as i32;
            item.armor_class = Some(if value == 0 { -1 } else { value });
            if value == 0 {
                item.cursed = true
            }
        }
        if matches!(which, 6 | 11) {
            item.cursed = true
        }
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_probability_tables_total_one_hundred() {
        for table in [
            &KIND_WEIGHTS[..],
            &POTION_WEIGHTS,
            &SCROLL_WEIGHTS,
            &RING_WEIGHTS,
            &STICK_WEIGHTS,
            &WEAPON_WEIGHTS,
            &ARMOR_WEIGHTS,
        ] {
            assert_eq!(table.iter().sum::<u32>(), 100);
        }
    }
    #[test]
    fn light_sticks_have_ten_to_nineteen_charges() {
        let mut found = 0;
        let mut rng = GameRng::new(1);
        for id in 0..10000 {
            let item = generate(&mut rng, id, 0);
            if item.kind == ItemKind::Stick && item.which == 0 {
                assert!((10..20).contains(&item.charges));
                found += 1
            }
        }
        assert!(found > 0);
    }

    #[test]
    fn missile_weapons_are_generated_in_original_stack_ranges() {
        let mut rng = GameRng::new(77);
        for which in 0..9 {
            let mut found = None;
            for id in 0..20_000 {
                let item = generate(&mut rng, id, 0);
                if item.kind == ItemKind::Weapon && item.which == which {
                    found = Some(item.count);
                    break;
                }
            }
            let count = found.expect("weapon kind should occur");
            match which {
                4 => assert!((2..=5).contains(&count)),
                3 | 6 | 7 => assert!((8..=15).contains(&count)),
                _ => assert_eq!(count, 1),
            }
        }
    }
    #[test]
    fn prolonged_food_drought_forces_food_generation() {
        let mut rng = GameRng::new(78);
        for id in 0..100 {
            assert_eq!(generate(&mut rng, id, 4).kind, ItemKind::Food);
        }
    }
}
