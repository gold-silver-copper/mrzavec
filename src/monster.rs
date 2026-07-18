use crate::{item::Item, map::Pos, rng::GameRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct MonsterTemplate {
    pub name: &'static str,
    pub carry: u8,
    pub flags: u16,
    pub exp: u32,
    pub level: i32,
    pub armor: i32,
    pub damage: &'static str,
}
pub const MEAN: u16 = 1;
pub const FLY: u16 = 2;
pub const REGEN: u16 = 4;
pub const GREED: u16 = 8;
pub const INVISIBLE: u16 = 16;
pub const HASTED: u16 = 32;
pub const SLOWED: u16 = 64;
pub const CANCELLED: u16 = 128;
pub const HELD: u16 = 256;
pub const CONFUSED: u16 = 512;
pub const GAZE_USED: u16 = 1024;

pub const MONSTERS: [MonsterTemplate; 26] = [
    mt("aquator", 0, MEAN, 20, 5, 2, "0x0/0x0"),
    mt("bat", 0, FLY, 1, 1, 3, "1x2"),
    mt("centaur", 15, 0, 17, 4, 4, "1x2/1x5/1x5"),
    mt("dragon", 100, MEAN, 5000, 10, -1, "1x8/1x8/3x10"),
    mt("emu", 0, MEAN, 2, 1, 7, "1x2"),
    mt("venus flytrap", 0, MEAN, 80, 8, 3, "0x0"),
    mt("griffin", 20, MEAN | FLY | REGEN, 2000, 13, 2, "4x3/3x5"),
    mt("hobgoblin", 0, MEAN, 3, 1, 5, "1x8"),
    mt("ice monster", 0, 0, 5, 1, 9, "0x0"),
    mt("jabberwock", 70, 0, 3000, 15, 6, "2x12/2x4"),
    mt("kestrel", 0, MEAN | FLY, 1, 1, 7, "1x4"),
    mt("leprechaun", 0, 0, 10, 3, 8, "1x1"),
    mt("medusa", 40, MEAN, 200, 8, 2, "3x4/3x4/2x5"),
    mt("nymph", 100, 0, 37, 3, 9, "0x0"),
    mt("orc", 15, GREED, 5, 1, 6, "1x8"),
    mt("phantom", 0, INVISIBLE, 120, 8, 3, "4x4"),
    mt("quagga", 0, MEAN, 15, 3, 3, "1x5/1x5"),
    mt("rattlesnake", 0, MEAN, 9, 2, 3, "1x6"),
    mt("snake", 0, MEAN, 2, 1, 5, "1x3"),
    mt("troll", 50, REGEN | MEAN, 120, 6, 4, "1x8/1x8/2x6"),
    mt("black unicorn", 0, MEAN, 190, 7, -2, "1x9/1x9/2x9"),
    mt("vampire", 20, REGEN | MEAN, 350, 8, 1, "1x10"),
    mt("wraith", 0, 0, 55, 5, 4, "1x6"),
    mt("xeroc", 30, 0, 100, 7, 7, "4x4"),
    mt("yeti", 30, 0, 50, 4, 6, "1x6/1x6"),
    mt("zombie", 0, MEAN, 6, 2, 8, "1x8"),
];
const fn mt(
    name: &'static str,
    carry: u8,
    flags: u16,
    exp: u32,
    level: i32,
    armor: i32,
    damage: &'static str,
) -> MonsterTemplate {
    MonsterTemplate {
        name,
        carry,
        flags,
        exp,
        level,
        armor,
        damage,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monster {
    pub id: u64,
    pub kind: u8,
    pub pos: Pos,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub armor: i32,
    pub experience: u32,
    pub flags: u16,
    pub awake: bool,
    pub disguise: char,
    pub turn: bool,
    pub inventory: Vec<Item>,
    pub destination: Option<Pos>,
    #[serde(default)]
    pub destination_is_room_gold: bool,
}

const LEVEL_MONSTERS: [u8; 26] = [
    10, 4, 1, 18, 7, 8, 17, 14, 25, 11, 2, 16, 0, 13, 24, 5, 19, 22, 15, 23, 20, 12, 21, 6, 9, 3,
];
const WANDERING_MONSTERS: [Option<u8>; 26] = [
    Some(10),
    Some(4),
    Some(1),
    Some(18),
    Some(7),
    None,
    Some(17),
    Some(14),
    Some(25),
    None,
    Some(2),
    Some(16),
    Some(0),
    None,
    Some(24),
    None,
    Some(19),
    Some(22),
    Some(15),
    None,
    Some(20),
    Some(12),
    Some(21),
    Some(6),
    Some(9),
    None,
];

pub fn random_kind(rng: &mut GameRng, depth: u32, wandering: bool) -> u8 {
    loop {
        let mut d = depth as i32 + rng.rnd(10) as i32 - 6;
        if d < 0 {
            d = rng.rnd(5) as i32;
        }
        if d > 25 {
            d = rng.rnd(5) as i32 + 21;
        }
        if wandering {
            if let Some(kind) = WANDERING_MONSTERS[d as usize] {
                return kind;
            }
        } else {
            return LEVEL_MONSTERS[d as usize];
        }
    }
}

pub fn create(id: u64, kind: u8, pos: Pos, depth: u32, rng: &mut GameRng) -> Monster {
    let mut monster = create_before_disguise(id, kind, pos, depth, rng);
    if kind == 23 {
        monster.disguise = roll_xeroc_disguise(depth, rng);
    }
    monster
}

pub(crate) fn create_before_disguise(
    id: u64,
    kind: u8,
    pos: Pos,
    depth: u32,
    rng: &mut GameRng,
) -> Monster {
    let template = MONSTERS[kind as usize];
    let add = depth.saturating_sub(26) as i32;
    let level = template.level + add;
    let hp = rng.roll(level as u32, 8);
    let mut experience_add = if level == 1 { hp / 8 } else { hp / 6 };
    if level > 9 {
        experience_add *= 20;
    } else if level > 6 {
        experience_add *= 4;
    }
    let experience = template.exp + add as u32 * 10 + experience_add.max(0) as u32;
    let mut flags = template.flags;
    if depth > 29 {
        flags |= HASTED;
    }
    Monster {
        id,
        kind,
        pos,
        hp,
        max_hp: hp,
        level,
        armor: template.armor - add,
        experience,
        flags,
        awake: false,
        disguise: (b'A' + kind) as char,
        turn: true,
        inventory: Vec::new(),
        destination: None,
        destination_is_room_gold: false,
    }
}

pub(crate) fn roll_xeroc_disguise(depth: u32, rng: &mut GameRng) -> char {
    const THINGS: [char; 10] = ['!', '?', '=', '/', ':', ')', ']', '%', '*', ','];
    let count = if depth >= 26 {
        THINGS.len()
    } else {
        THINGS.len() - 1
    };
    THINGS[rng.rnd(count as u32) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_monsters_scale_after_amulet_level() {
        let mut r = GameRng::new(1);
        let normal = create(1, 3, Pos::new(1, 1), 26, &mut r);
        let deep = create(2, 3, Pos::new(1, 1), 30, &mut r);
        assert_eq!(deep.level, normal.level + 4);
        assert_eq!(deep.armor, normal.armor - 4);
        assert!(deep.experience > normal.experience + 40);
        assert_eq!(normal.flags & HASTED, 0);
        assert_ne!(deep.flags & HASTED, 0);
    }

    #[test]
    fn xeroc_disguise_uses_the_original_depth_appropriate_object_glyphs() {
        let mut rng = GameRng::new(7);
        let shallow: std::collections::HashSet<_> = (0..200)
            .map(|id| create(id, 23, Pos::new(1, 1), 25, &mut rng).disguise)
            .collect();
        let shallow_glyphs = ['!', '?', '=', '/', ':', ')', ']', '%', '*']
            .into_iter()
            .collect();
        assert!(shallow.is_subset(&shallow_glyphs));
        assert!(!shallow.contains(&','));

        let deep: std::collections::HashSet<_> = (200..600)
            .map(|id| create(id, 23, Pos::new(1, 1), 26, &mut rng).disguise)
            .collect();
        assert!(deep.contains(&','));
    }
}
