use crate::{item::Item, map::Pos};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub strength: i32,
    pub experience: u64,
    pub level: i32,
    pub armor: i32,
    pub hp: i32,
    pub max_hp: i32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            strength: 16,
            experience: 0,
            level: 1,
            armor: 10,
            hp: 12,
            max_hp: 12,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    pub blind: bool,
    pub confused: bool,
    pub hallucinating: bool,
    pub levitating: bool,
    pub see_invisible: bool,
    pub detect_monsters: bool,
    pub hasted: bool,
    pub can_confuse_monster: bool,
    pub held_turns: u32,
    pub asleep_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub pos: Pos,
    pub stats: Stats,
    pub max_strength: i32,
    pub gold: i32,
    pub food_left: i32,
    pub conditions: Conditions,
    pub inventory: Vec<Item>,
    pub weapon: Option<u64>,
    pub armor: Option<u64>,
    pub rings: [Option<u64>; 2],
}

impl Player {
    pub fn new(pos: Pos) -> Self {
        Self {
            pos,
            stats: Stats::default(),
            max_strength: 16,
            gold: 0,
            food_left: 1300,
            conditions: Conditions::default(),
            inventory: Vec::new(),
            weapon: None,
            armor: None,
            rings: [None, None],
        }
    }
    pub fn armor_class(&self) -> i32 {
        let base = self
            .armor
            .and_then(|id| self.inventory.iter().find(|i| i.id == id))
            .map_or(10, |i| i.armor_class.unwrap_or(10));
        let protection: i32 = self
            .rings
            .iter()
            .flatten()
            .filter_map(|id| {
                self.inventory
                    .iter()
                    .find(|i| i.id == *id && i.kind == crate::item::ItemKind::Ring && i.which == 0)
            })
            .map(|r| r.armor_class.unwrap_or(0))
            .sum();
        base - protection
    }
}

pub const EXPERIENCE_LEVELS: [u64; 20] = [
    10, 20, 40, 80, 160, 320, 640, 1300, 2600, 5200, 13000, 26000, 50000, 100000, 200000, 400000,
    800000, 2_000_000, 4_000_000, 8_000_000,
];
