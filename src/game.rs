use crate::{
    AMULET_LEVEL, MAX_PACK,
    combat::{self, Attack},
    command::{Command, CommandResult, Direction, WizardCommand},
    effects::{Effect, Scheduler},
    generation::{Level, begin_layout, dig_passages, dig_room, finish_level},
    item::{Item, ItemKind, generate},
    map::{Pos, Terrain, Trap},
    monster::{self, MONSTERS, Monster},
    player::Player,
    rng::GameRng,
};
use interslavic::Case;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

fn trap_name(trap: Trap) -> String {
    crate::lang::phrase(
        &crate::lang::TRAP_LEX[trap_index(trap)],
        Case::Nom,
        interslavic::Number::Singular,
    )
}

const fn trap_index(trap: Trap) -> usize {
    match trap {
        Trap::TrapDoor => 0,
        Trap::Arrow => 1,
        Trap::SleepGas => 2,
        Trap::Bear => 3,
        Trap::Teleport => 4,
        Trap::PoisonDart => 5,
        Trap::Rust => 6,
        Trap::Mysterious => 7,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndState {
    Playing,
    Dead,
    Quit,
    Won,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Options {
    pub fruit: String,
    pub terse: bool,
    pub fight_flush: bool,
    pub jump: bool,
    pub see_floor: bool,
    pub passgo: bool,
    pub tombstone: bool,
    pub inventory_style: InventoryStyle,
    pub name: String,
    pub save_file: String,
    pub score_file: String,
    pub lock_file: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryStyle {
    Overwrite,
    Slow,
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knowledge {
    pub potions: Vec<bool>,
    pub scrolls: Vec<bool>,
    pub rings: Vec<bool>,
    pub sticks: Vec<bool>,
    pub guesses: Vec<Option<String>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearances {
    /// Indices into `lang::COLOR_ADJ`.
    pub potion_colors: Vec<usize>,
    pub scroll_titles: Vec<String>,
    /// Indices into `lang::STONE_LEX`.
    pub ring_stones: Vec<usize>,
    pub ring_stone_values: Vec<i32>,
    /// Wood sticks are staves, metal ones wands.
    pub stick_is_staff: Vec<bool>,
    /// Indices into `lang::WOOD_LEX` (staff) or `lang::METAL_LEX` (wand).
    pub stick_materials: Vec<usize>,
}

impl Appearances {
    fn new(rng: &mut GameRng) -> Self {
        // Slavic-flavored gibberish syllables for scroll titles (magic
        // language, deliberately not lexicon words).
        const SYLLABLES: &[&str] = &[
            "a", "ba", "bez", "blě", "bo", "brě", "bry", "bųd", "va", "vel", "vih", "vlk", "vod",
            "voz", "vy", "gla", "glų", "gně", "go", "gord", "grom", "gų", "da", "dvo", "dob",
            "dol", "drě", "dug", "dy", "đu", "e", "ě", "ęz", "ža", "žel", "živ", "žu", "za", "zvě",
            "zim", "zlo", "zna", "zra", "i", "iz", "ju", "jed", "jęt", "ka", "kam", "kli", "kně",
            "ko", "kra", "krů", "kry", "ku", "kva", "ky", "la", "lěs", "li", "lo", "lų", "ly",
            "ma", "mě", "mir", "mlå", "mo", "mų", "my", "na", "ne", "něg", "ni", "no", "nų", "o",
            "ob", "ogn", "od", "op", "ora", "ost", "pa", "per", "pě", "pi", "plå", "po", "pra",
            "prě", "prų", "pŕst", "pu", "ra", "rěč", "ri", "ro", "rů", "ry", "sa", "svě", "se",
            "sě", "si", "skri", "sla", "slo", "sně", "so", "sta", "stra", "su", "sų", "sy", "ta",
            "tvo", "te", "tě", "ti", "tma", "to", "tri", "tu", "ty", "u", "us", "hlå", "ho", "hra",
            "cvě", "ce", "ci", "ča", "če", "či", "čud", "ša", "še", "ši", "šum", "yr", "ųt",
        ];
        // Startup calls `init_names`, `init_colors`, `init_stones`, then
        // `init_materials`; preserve that RNG ordering as well as each table's
        // distribution.
        let scroll_titles = (0..18)
            .map(|_| {
                (0..rng.rnd(3) + 2)
                    .map(|_| {
                        (0..rng.rnd(3) + 1)
                            .map(|_| SYLLABLES[rng.rnd(SYLLABLES.len() as u32) as usize])
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        let potion_colors = choose_unique_indices(rng, crate::lang::COLOR_ADJ.len(), 14);
        const STONE_VALUES: &[i32] = &[
            25, 40, 50, 40, 300, 300, 225, 5, 50, 150, 300, 50, 50, 15, 60, 200, 220, 63, 350, 285,
            200, 50, 60, 70, 300, 80,
        ];
        let ring_stones = choose_unique_indices(rng, crate::lang::STONE_LEX.len(), 14);
        let ring_stone_values = ring_stones
            .iter()
            .map(|&index| STONE_VALUES[index])
            .collect();
        let mut used_wood = Vec::new();
        let mut used_metal = Vec::new();
        let mut stick_is_staff = Vec::new();
        let mut stick_materials = Vec::new();
        for _ in 0..14 {
            loop {
                let (is_staff, len, used) = if rng.rnd(2) == 0 {
                    (false, crate::lang::METAL_LEX.len(), &mut used_metal)
                } else {
                    (true, crate::lang::WOOD_LEX.len(), &mut used_wood)
                };
                let index = rng.rnd(len as u32) as usize;
                if !used.contains(&index) {
                    used.push(index);
                    stick_is_staff.push(is_staff);
                    stick_materials.push(index);
                    break;
                }
            }
        }
        Self {
            potion_colors,
            scroll_titles,
            ring_stones,
            ring_stone_values,
            stick_is_staff,
            stick_materials,
        }
    }
}

fn choose_unique_indices(rng: &mut GameRng, len: usize, count: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..len).collect();
    for i in 0..count {
        let j = i + rng.rnd((indices.len() - i) as u32) as usize;
        indices.swap(i, j);
    }
    indices.truncate(count);
    indices
}

fn uppercase_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        // Unicode-aware: Interslavic initials (ž, č, š…) must capitalize too.
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentifyKind {
    Potion,
    Scroll,
    Weapon,
    Armor,
    RingOrStick,
}

impl Default for Knowledge {
    fn default() -> Self {
        Self {
            potions: vec![false; 14],
            scrolls: vec![false; 18],
            rings: vec![false; 14],
            sticks: vec![false; 14],
            guesses: vec![None; 60],
        }
    }
}
impl Default for Options {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let options = Self {
            fruit: "slime-mold".into(),
            terse: false,
            fight_flush: false,
            jump: false,
            see_floor: true,
            passgo: false,
            tombstone: true,
            // `playit` selects INV_CLEAR whenever the terminal can clear to
            // end-of-line. The Bevy glyph surface always has that ability.
            inventory_style: InventoryStyle::Clear,
            #[cfg(not(target_arch = "wasm32"))]
            name: std::env::var("USER").unwrap_or_else(|_| "player".into()),
            #[cfg(target_arch = "wasm32")]
            name: "player".into(),
            #[cfg(not(target_arch = "wasm32"))]
            save_file: format!("{home}/.rogue.save.json"),
            #[cfg(target_arch = "wasm32")]
            save_file: "default".into(),
            #[cfg(not(target_arch = "wasm32"))]
            score_file: format!("{home}/.rogue.scores.json"),
            #[cfg(target_arch = "wasm32")]
            score_file: "local".into(),
            #[cfg(not(target_arch = "wasm32"))]
            lock_file: format!("{home}/.rogue.scores.lock"),
            #[cfg(target_arch = "wasm32")]
            lock_file: "unused".into(),
        };
        #[cfg(not(target_arch = "wasm32"))]
        let mut options = options;
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(value) = std::env::var("ROGUEOPTS") {
            options.apply_rogue_options(&value);
        }
        options
    }
}

impl Options {
    pub fn apply_rogue_options(&mut self, value: &str) {
        for raw in value.split(',') {
            let option = raw.trim_start_matches(|ch: char| !ch.is_ascii_alphabetic());
            let name_len = option.bytes().take_while(u8::is_ascii_alphabetic).count();
            if name_len == 0 {
                continue;
            }
            let name = &option[..name_len];
            let boolean = [
                ("terse", &mut self.terse),
                ("flush", &mut self.fight_flush),
                ("jump", &mut self.jump),
                ("seefloor", &mut self.see_floor),
                ("passgo", &mut self.passgo),
                ("tombstone", &mut self.tombstone),
            ];
            let mut matched = false;
            for (option_name, target) in boolean {
                if option_name.starts_with(name) {
                    *target = true;
                    matched = true;
                    break;
                }
                if let Some(positive) = name.strip_prefix("no")
                    && option_name.starts_with(positive)
                {
                    *target = false;
                    matched = true;
                    break;
                }
            }
            if matched {
                continue;
            }
            let setting = option[name_len..]
                .chars()
                .skip(1)
                .skip_while(|ch| *ch == '=')
                .collect::<String>();
            if "inven".starts_with(name) {
                let mut setting = setting;
                if let Some(first) = setting.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                if "Overwrite".starts_with(&setting) {
                    self.inventory_style = InventoryStyle::Overwrite;
                } else if "Slow".starts_with(&setting) {
                    self.inventory_style = InventoryStyle::Slow;
                } else if "Clear".starts_with(&setting) {
                    self.inventory_style = InventoryStyle::Clear;
                }
            } else if !setting.is_empty() {
                let setting = normalize_option_string(&setting);
                if "name".starts_with(name) {
                    self.name = setting;
                } else if "fruit".starts_with(name) {
                    self.fruit = setting;
                } else if "file".starts_with(name) {
                    self.save_file = setting;
                } else if "score".starts_with(name) {
                    self.score_file = setting;
                } else if "lock".starts_with(name) {
                    self.lock_file = setting;
                }
            }
        }
    }
}

pub fn normalize_option_string(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
        .take(50)
        .collect();
    let Some(rest) = value.strip_prefix('~') else {
        return value;
    };
    #[cfg(target_arch = "wasm32")]
    return rest.trim_start_matches('/').to_owned();
    #[cfg(not(target_arch = "wasm32"))]
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    #[cfg(not(target_arch = "wasm32"))]
    let rest = rest.trim_start_matches('/');
    #[cfg(not(target_arch = "wasm32"))]
    if rest.is_empty() {
        home
    } else {
        format!("{}/{rest}", home.trim_end_matches('/'))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub save_version: u32,
    pub dungeon_number: u32,
    pub rng: GameRng,
    pub dungeon: Level,
    pub depth: u32,
    pub max_depth: u32,
    pub player: Player,
    pub monsters: Vec<Monster>,
    pub floor_items: Vec<Item>,
    pub messages: Vec<String>,
    #[serde(default)]
    pub message_serial: u64,
    #[serde(default)]
    pub recall_message: String,
    pub turn: u64,
    pub end: EndState,
    pub death_cause: Option<String>,
    pub has_amulet: bool,
    pub seen_stairs: bool,
    pub options: Options,
    pub next_id: u64,
    pub no_food: u8,
    pub scheduler: Scheduler,
    pub quiet_turns: u32,
    pub hungry_state: u8,
    pub wandering_countdown: i32,
    pub knowledge: Knowledge,
    pub appearances: Appearances,
    pub pending_identification: Option<IdentifyKind>,
    pub pending_call: Option<(ItemKind, u8)>,
    pub wizard: bool,
    pub no_score: bool,
    pub flytrap_hits: i32,
    pub flytrap_holder: Option<u64>,
    pub haste_phase: bool,
    pub skip_world_once: bool,
    #[serde(default)]
    pub player_is_running: bool,
    pub last_command: Option<char>,
    pub last_item: Option<u64>,
    pub last_direction: Option<Direction>,
    pub last_hand: Option<usize>,
    #[serde(default)]
    pub previous_command: Option<char>,
    #[serde(default)]
    pub previous_item: Option<u64>,
    #[serde(default)]
    pub previous_direction: Option<Direction>,
    #[serde(default)]
    pub previous_hand: Option<usize>,
    #[serde(default)]
    pub hallucinated_items: Vec<(u64, char)>,
    #[serde(default)]
    pub hallucinated_monsters: Vec<(u64, char)>,
    #[serde(default)]
    pub hallucinated_stairs: Option<char>,
    #[serde(skip)]
    fight_target: Option<u64>,
    #[serde(skip)]
    fight_kamikaze: bool,
    #[serde(skip)]
    fight_safety_max_hit: i32,
    /// Pointer travel is an ephemeral input gesture. Saving or restoring a
    /// game must never resume unattended movement.
    #[serde(skip)]
    travel_target: Option<Pos>,
    /// A monotonic-within-step damage signal keeps same-turn healing from
    /// hiding an interruption. It is cleared whenever pointer travel stops.
    #[serde(skip)]
    travel_damage_seen: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct TravelSnapshot {
    depth: u32,
    hp: i32,
    held_turns: u32,
    asleep_turns: u32,
    visible_monsters: HashSet<u64>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = GameRng::new(seed);
        // `init_player` rolls the starting arrows before appearance tables and
        // before the first dungeon is dug.
        let starting_arrows = rng.rnd(15) + 25;
        let appearances = Appearances::new(&mut rng);
        let dungeon = begin_layout(&mut rng);
        let mut g = Self {
            save_version: 13,
            dungeon_number: seed as u32,
            rng,
            dungeon,
            depth: 1,
            max_depth: 1,
            player: Player::new(Pos::new(0, 0)),
            monsters: vec![],
            floor_items: vec![],
            messages: vec![
                "dobro ⟨lp:dojdti:m:pl⟩ v ⟨n:temnica:acc:pl:U⟩ ⟨n:pohibel:gen:U⟩".into(),
            ],
            message_serial: 1,
            recall_message: "dobro ⟨lp:dojdti:m:pl⟩ v ⟨n:temnica:acc:pl:U⟩ ⟨n:pohibel:gen:U⟩"
                .into(),
            turn: 0,
            end: EndState::Playing,
            death_cause: None,
            has_amulet: false,
            seen_stairs: false,
            options: Options::default(),
            next_id: 1,
            no_food: 0,
            scheduler: Scheduler::default(),
            quiet_turns: 0,
            hungry_state: 0,
            wandering_countdown: 0,
            knowledge: Knowledge::default(),
            appearances,
            pending_identification: None,
            pending_call: None,
            wizard: false,
            no_score: false,
            flytrap_hits: 0,
            flytrap_holder: None,
            haste_phase: false,
            skip_world_once: false,
            player_is_running: false,
            last_command: None,
            last_item: None,
            last_direction: None,
            last_hand: None,
            previous_command: None,
            previous_item: None,
            previous_direction: None,
            previous_hand: None,
            hallucinated_items: Vec::new(),
            hallucinated_monsters: Vec::new(),
            hallucinated_stairs: None,
            fight_target: None,
            fight_kamikaze: false,
            fight_safety_max_hit: 0,
            travel_target: None,
            travel_damage_seen: false,
        };
        g.init_inventory(starting_arrows);
        g.build_current_level();
        g.wandering_countdown = g.rng.spread(70) + 4;
        g
    }
    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn init_inventory(&mut self, starting_arrows: u32) {
        let mut food = Item::basic(self.id(), ItemKind::Food, 0);
        food.pack_letter = Some('a');
        food.known = true;
        let mut armor = Item::basic(self.id(), ItemKind::Armor, 1);
        armor.pack_letter = Some('b');
        armor.armor_class = Some(6);
        armor.known = true;
        let armor_id = armor.id;
        let mut mace = Item::basic(self.id(), ItemKind::Weapon, 0);
        mace.pack_letter = Some('c');
        mace.hit_plus = 1;
        mace.damage_plus = 1;
        mace.known = true;
        let mace_id = mace.id;
        let mut bow = Item::basic(self.id(), ItemKind::Weapon, 2);
        bow.pack_letter = Some('d');
        bow.hit_plus = 1;
        bow.known = true;
        let mut arrows = Item::basic(self.id(), ItemKind::Weapon, 3);
        arrows.pack_letter = Some('e');
        arrows.count = starting_arrows;
        arrows.group = arrows.id;
        arrows.known = true;
        self.player.inventory = vec![food, armor, mace, bow, arrows];
        self.player.armor = Some(armor_id);
        self.player.weapon = Some(mace_id);
    }
    fn populate_room(&mut self, room: u8) {
        if self.dungeon.rooms[room as usize].gone {
            return;
        }
        let place_gold = self.rng.rnd(2) == 0 && (!self.has_amulet || self.depth >= self.max_depth);
        if place_gold {
            let value = (self.rng.rnd(50 + 10 * self.depth) + 2) as i32;
            let mut gold = Item::gold(self.id(), value);
            if let (true, Some(pos)) = self.reference_room_floor_attempt(room, 0, false) {
                gold.pos = Some(pos);
                self.dungeon.rooms[room as usize].gold = Some(pos);
                self.dungeon.rooms[room as usize].gold_value = value as u32;
                self.floor_items.push(gold)
            }
        }
        if self.rng.rnd(100) < if place_gold { 80 } else { 25 } {
            self.spawn_monster_in_room(room, false)
        }
    }

    fn populate_things(&mut self) {
        if !(self.has_amulet && self.depth < self.max_depth) {
            if self.rng.rnd(20) == 0 {
                self.populate_treasure_room();
            }
            for _ in 0..9 {
                if self.rng.rnd(100) >= 36 {
                    continue;
                }
                let id = self.id();
                let mut item = generate(&mut self.rng, id, self.no_food);
                if item.kind == ItemKind::Food {
                    self.no_food = 0
                }
                if let Some(p) = self.reference_floor_position(false) {
                    item.pos = Some(p);
                    self.floor_items.push(item)
                }
            }
            if self.depth >= AMULET_LEVEL && !self.has_amulet {
                let id = self.id();
                let mut a = Item::basic(id, ItemKind::Amulet, 0);
                if let Some(pos) = self.reference_floor_position(false) {
                    a.pos = Some(pos);
                    self.floor_items.push(a)
                }
            }
        }
    }

    fn populate_treasure_room(&mut self) {
        let Some(room) = self.reference_random_room() else {
            return;
        };
        let room_data = self.dungeon.rooms[room as usize].clone();
        let interior = ((room_data.width - 2) * (room_data.height - 2)) as usize;
        let capacity = interior.saturating_sub(2).min(8);
        if capacity == 0 {
            return;
        }
        let item_count = self.rng.rnd(capacity as u32) as usize + 2;
        for _ in 0..item_count {
            let (_, Some(pos)) = self.reference_room_floor_attempt(room, 20, false) else {
                break;
            };
            let id = self.id();
            let mut item = generate(&mut self.rng, id, self.no_food);
            if item.kind == ItemKind::Food {
                self.no_food = 0;
            }
            item.pos = Some(pos);
            self.floor_items.push(item);
        }
        let monster_count = (self.rng.rnd(capacity as u32) as usize + 2)
            .max(item_count + 2)
            .min(interior);
        for _ in 0..monster_count {
            let (found, pos) = self.reference_room_floor_attempt(room, 10, true);
            if !found {
                continue;
            }
            let pos = pos.expect("a successful limited floor search has a position");
            let kind = monster::random_kind(&mut self.rng, self.depth + 1, false);
            let mut monster = self.make_monster(kind, pos, self.depth + 1, false, true);
            monster.flags |= monster::MEAN;
            self.monsters.insert(0, monster);
        }
    }

    fn empty_floor_positions(&self) -> Vec<Pos> {
        self.dungeon
            .map
            .iter()
            .filter_map(|(p, c)| {
                (self.is_room_base_terrain(c)
                    && p != self.player.pos
                    && self.monsters.iter().all(|m| m.pos != p))
                .then_some(p)
            })
            .collect()
    }

    fn place_player_on_empty_floor(&mut self) {
        for _ in 0..10_000 {
            let Some(pos) = self.reference_floor_position(true) else {
                return;
            };
            if self.floor_items.iter().any(|item| item.pos == Some(pos)) {
                continue;
            }
            if self
                .dungeon
                .map
                .get(pos)
                .is_some_and(|cell| matches!(cell.terrain, Terrain::Floor | Terrain::Passage))
            {
                self.player.pos = pos;
                return;
            }
        }
    }

    fn is_room_base_terrain(&self, cell: &crate::map::Cell) -> bool {
        let Some(room) = cell.room.and_then(|id| self.dungeon.rooms.get(id as usize)) else {
            return false;
        };
        if room.maze {
            cell.terrain == Terrain::Passage
        } else {
            cell.terrain == Terrain::Floor
        }
    }

    fn room_floor_candidate_valid(&self, room: u8, pos: Pos, monster: bool) -> bool {
        let Some(cell) = self.dungeon.map.get(pos) else {
            return false;
        };
        if monster {
            (cell.terrain.passable() || self.floor_items.iter().any(|item| item.pos == Some(pos)))
                && self.monsters.iter().all(|candidate| candidate.pos != pos)
        } else {
            cell.room == Some(room)
                && self.is_room_base_terrain(cell)
                && !cell.trap_revealed
                && self.floor_items.iter().all(|item| item.pos != Some(pos))
        }
    }

    fn reference_room_floor_attempt(
        &mut self,
        room: u8,
        limit: usize,
        monster: bool,
    ) -> (bool, Option<Pos>) {
        let room_data = self.dungeon.rooms[room as usize].clone();
        let attempts = if limit == 0 { 10_000 } else { limit };
        let mut last = None;
        for _ in 0..attempts {
            let x = room_data.top_left.x + self.rng.rnd((room_data.width - 2) as u32) as i32 + 1;
            let y = room_data.top_left.y + self.rng.rnd((room_data.height - 2) as u32) as i32 + 1;
            let pos = Pos::new(x, y);
            last = Some(pos);
            if self.room_floor_candidate_valid(room, pos, monster) {
                return (true, last);
            }
        }
        (false, last)
    }

    fn reference_floor_position(&mut self, monster: bool) -> Option<Pos> {
        for _ in 0..10_000 {
            let room = self.rng.rnd(self.dungeon.rooms.len() as u32) as u8;
            let room_data = &self.dungeon.rooms[room as usize];
            if room_data.gone {
                continue;
            }
            let x = room_data.top_left.x + self.rng.rnd((room_data.width - 2) as u32) as i32 + 1;
            let y = room_data.top_left.y + self.rng.rnd((room_data.height - 2) as u32) as i32 + 1;
            let pos = Pos::new(x, y);
            if self.room_floor_candidate_valid(room, pos, monster) {
                return Some(pos);
            }
        }
        None
    }

    fn reference_random_room(&mut self) -> Option<u8> {
        for _ in 0..10_000 {
            let room = self.rng.rnd(self.dungeon.rooms.len() as u32) as u8;
            if !self.dungeon.rooms[room as usize].gone {
                return Some(room);
            }
        }
        None
    }

    fn spawn_random_monster(&mut self, wandering: bool) {
        let pos = if wandering {
            let mut selected = None;
            for _ in 0..500 {
                let Some(pos) = self.reference_monster_floor_position() else {
                    break;
                };
                if self.area_key(pos) != self.area_key(self.player.pos) {
                    selected = Some(pos);
                    break;
                }
            }
            let Some(pos) = selected else { return };
            pos
        } else {
            let spots = self.empty_floor_positions();
            let Some(pos) = spots.get(self.rng.rnd(spots.len() as u32) as usize) else {
                return;
            };
            *pos
        };
        let kind = monster::random_kind(&mut self.rng, self.depth, wandering);
        let monster = self.make_monster(kind, pos, self.depth, wandering, !wandering);
        self.monsters.insert(0, monster);
        if wandering {
            let index = 0;
            if self.player.conditions.detect_monsters && self.player.conditions.hallucinating {
                let id = self.monsters[index].id;
                let glyph = (b'A' + self.rng.rnd(26) as u8) as char;
                self.hallucinated_monsters.push((id, glyph));
            }
            self.monsters[index].destination = self.find_monster_item_destination(index);
            self.monsters[index].destination_is_room_gold = false;
        }
    }
    fn spawn_monster_in_room(&mut self, room: u8, wandering: bool) {
        let (found, pos) = self.reference_room_floor_attempt(room, 0, true);
        if !found {
            return;
        }
        let pos = pos.expect("an unlimited successful floor search has a position");
        let kind = monster::random_kind(&mut self.rng, self.depth, wandering);
        let monster = self.make_monster(kind, pos, self.depth, wandering, true);
        self.monsters.insert(0, monster);
    }

    fn make_monster(
        &mut self,
        kind: u8,
        pos: Pos,
        depth: u32,
        awake: bool,
        give_pack: bool,
    ) -> Monster {
        let id = self.id();
        self.make_monster_with_id(id, kind, pos, depth, awake, give_pack)
    }

    fn make_monster_with_id(
        &mut self,
        id: u64,
        kind: u8,
        pos: Pos,
        depth: u32,
        awake: bool,
        give_pack: bool,
    ) -> Monster {
        let aggravated = self.wears_ring(6);
        let mut monster = monster::create_before_disguise(id, kind, pos, depth, &mut self.rng);
        if aggravated {
            monster.awake = true;
            monster.destination = self.find_monster_item_destination_for(&monster);
        }
        if kind == 23 {
            monster.disguise = monster::roll_xeroc_disguise(depth, &mut self.rng);
        }
        monster.awake |= awake;
        if give_pack
            && depth >= self.max_depth
            && self.rng.rnd(100) < MONSTERS[kind as usize].carry as u32
        {
            let item_id = self.id();
            let item = generate(&mut self.rng, item_id, self.no_food);
            if item.kind == ItemKind::Food {
                self.no_food = 0;
            }
            monster.inventory.push(item);
        }
        monster
    }

    /// Begin pointer travel toward a remembered dungeon tile. The destination
    /// and every intermediate tile must already be known to the player; the
    /// route never consults unseen terrain.
    pub fn start_travel(&mut self, destination: Pos) -> bool {
        if self.end != EndState::Playing || destination == self.player.pos {
            self.travel_target = None;
            return false;
        }
        if self.travel_first_step(destination).is_none() {
            self.travel_target = None;
            return false;
        }
        self.travel_target = Some(destination);
        self.travel_damage_seen = false;
        true
    }

    pub fn cancel_travel(&mut self) {
        self.travel_target = None;
        self.travel_damage_seen = false;
    }

    pub fn is_traveling(&self) -> bool {
        self.travel_target.is_some()
    }

    fn travel_snapshot(&self) -> TravelSnapshot {
        TravelSnapshot {
            depth: self.depth,
            hp: self.player.stats.hp,
            held_turns: self.player.conditions.held_turns,
            asleep_turns: self.player.conditions.asleep_turns,
            visible_monsters: self
                .monsters
                .iter()
                .filter(|monster| self.can_see_monster(monster))
                .map(|monster| monster.id)
                .collect(),
        }
    }

    fn travel_interrupted(
        &self,
        before: &TravelSnapshot,
        expected: Pos,
        stepped_on_trap: bool,
        result: CommandResult,
    ) -> bool {
        !result.consumed_turn
            || self.end != EndState::Playing
            || self.depth != before.depth
            || self.player.pos != expected
            || self.player.stats.hp < before.hp
            || self.travel_damage_seen
            || self.player.conditions.held_turns > before.held_turns
            || self.player.conditions.asleep_turns > before.asleep_turns
            || stepped_on_trap
            || self.monsters.iter().any(|monster| {
                self.can_see_monster(monster) && !before.visible_monsters.contains(&monster.id)
            })
    }

    /// Execute one ordinary movement command along the current pointer route.
    /// Returns the ordinary command result so callers can observe whether the
    /// attempted step consumed a turn.
    pub fn advance_travel(&mut self) -> CommandResult {
        let Some(destination) = self.travel_target else {
            return CommandResult::FREE;
        };
        let Some(next) = self.travel_first_step(destination) else {
            self.cancel_travel();
            return CommandResult::FREE;
        };
        let dx = next.x - self.player.pos.x;
        let dy = next.y - self.player.pos.y;
        let Some(direction) = Direction::from_delta(dx, dy) else {
            self.cancel_travel();
            return CommandResult::FREE;
        };
        let stepped_on_trap = self
            .dungeon
            .map
            .get(next)
            .is_some_and(|cell| cell.trap.is_some())
            && !self.player.conditions.levitating
            && self.floor_items.iter().all(|item| item.pos != Some(next));
        self.travel_damage_seen = false;
        let before = self.travel_snapshot();
        let result = self.execute(Command::Move(direction));
        let interrupted = self.travel_interrupted(&before, next, stepped_on_trap, result);
        self.travel_damage_seen = false;
        if next == destination || interrupted {
            self.cancel_travel();
        }
        result
    }

    fn travel_first_step(&self, destination: Pos) -> Option<Pos> {
        let start = self.player.pos;
        if !self.travel_tile_known(destination) {
            return None;
        }
        let mut frontier = VecDeque::from([start]);
        let mut parent = HashMap::from([(start, start)]);
        const DIRECTIONS: [Direction; 8] = [
            Direction::Left,
            Direction::Down,
            Direction::Up,
            Direction::Right,
            Direction::UpLeft,
            Direction::UpRight,
            Direction::DownLeft,
            Direction::DownRight,
        ];
        while let Some(current) = frontier.pop_front() {
            if current == destination {
                break;
            }
            for direction in DIRECTIONS {
                let (dx, dy) = direction.delta();
                let next = current.offset(dx, dy);
                if parent.contains_key(&next) || !self.travel_tile_known(next) {
                    continue;
                }
                if dx != 0
                    && dy != 0
                    && (!self.travel_tile_known(current.offset(dx, 0))
                        || !self.travel_tile_known(current.offset(0, dy)))
                {
                    continue;
                }
                parent.insert(next, current);
                frontier.push_back(next);
            }
        }
        if !parent.contains_key(&destination) {
            return None;
        }
        let mut step = destination;
        while parent[&step] != start {
            step = parent[&step];
        }
        Some(step)
    }

    fn travel_tile_known(&self, pos: Pos) -> bool {
        self.dungeon
            .map
            .get(pos)
            .is_some_and(|cell| cell.seen && cell.terrain.passable())
    }

    fn damage_player(&mut self, damage: i32) {
        if damage <= 0 {
            return;
        }
        if self.travel_target.is_some() {
            self.travel_damage_seen = true;
        }
        self.player.stats.hp -= damage;
    }

    pub fn execute(&mut self, command: Command) -> CommandResult {
        if self.end != EndState::Playing {
            return CommandResult::FREE;
        }
        self.begin_command();
        self.execute_after_begin(command)
    }

    /// Execute a command after the caller has performed the reference
    /// pre-command `look`. This lets the UI put prompts after that look
    /// without repeating its wake rolls when the prompt is resolved.
    pub fn execute_after_begin(&mut self, command: Command) -> CommandResult {
        if self.end != EndState::Playing {
            return CommandResult::FREE;
        }
        if self.player.conditions.asleep_turns > 0 {
            self.player.conditions.asleep_turns -= 1;
            if self.player.conditions.asleep_turns == 0 {
                self.player_is_running = true;
                self.message("⟨v2:mogti⟩ opęť dvigati sę");
            }
            self.advance_player_turn();
            return CommandResult::TURN;
        }
        let result = match command {
            Command::Move(d) => self.move_player(d),
            Command::Run(d) => self.run_player(d, false),
            Command::RunUntilInteresting(d) => self.run_player(d, true),
            Command::Rest => CommandResult::TURN,
            Command::Search => {
                self.search();
                CommandResult::TURN
            }
            Command::Pickup => self.pickup(),
            Command::TakeOff => self.take_off(),
            Command::Down => self.descend(),
            Command::Up => self.ascend(),
            Command::Inventory
            | Command::PickyInventory
            | Command::IdentifyObject
            | Command::Help
            | Command::Discoveries
            | Command::Options
            | Command::Recall
            | Command::Redraw
            | Command::Version
            | Command::LegalSpace
            | Command::Shell
            | Command::Suspend
            | Command::Quaff
            | Command::Read
            | Command::Eat
            | Command::Wield
            | Command::Wear
            | Command::PutOnRing
            | Command::RemoveRing
            | Command::Drop
            | Command::Zap
            | Command::Throw
            | Command::Fight { .. }
            | Command::MoveWithoutPickup
            | Command::IdentifyTrap
            | Command::Repeat
            | Command::Call
            | Command::CurrentWeapon
            | Command::CurrentArmor
            | Command::CurrentRings
            | Command::CurrentStats
            | Command::ToggleWizard => CommandResult::FREE,
            Command::Wizard(wizard_command) => {
                if self.wizard {
                    self.wizard_command(wizard_command)
                } else {
                    self.message_without_recall(format!(
                        "⟨a:nepraviľny:komanda:nom⟩ ⟨n:komanda:nom⟩ '{}'",
                        command_label(wizard_command_char(wizard_command))
                    ))
                }
                CommandResult::FREE
            }
            Command::Quit => {
                self.end = EndState::Quit;
                CommandResult::FREE
            }
            Command::Save | Command::Cancel => CommandResult::FREE,
            Command::Unknown(ch) => {
                self.message_without_recall(format!(
                    "⟨a:nepraviľny:komanda:nom⟩ ⟨n:komanda:nom⟩ '{}'",
                    command_label(ch)
                ));
                CommandResult::FREE
            }
        };
        if result.consumed_turn {
            self.advance_player_turn()
        }
        result
    }

    fn die(&mut self, cause: impl Into<String>) {
        self.death_cause = Some(crate::lang::speak(&cause.into()));
        self.end = EndState::Dead;
    }

    /// Death cause in the genitive (rendered after "od" on the end screens).
    fn monster_killer(kind: u8) -> String {
        crate::lang::phrase(
            &crate::lang::MONSTER_LEX[kind as usize],
            Case::Gen,
            interslavic::Number::Singular,
        )
    }

    pub fn eat(&mut self, id: u64) -> CommandResult {
        let Some(item) = self.player.inventory.iter().find(|i| i.id == id) else {
            return CommandResult::FREE;
        };
        if item.kind != ItemKind::Food {
            self.message(if self.options.terse {
                "to ne jest ⟨adv:jedlivy⟩!"
            } else {
                "fuj, od ⟨toj:gen:n⟩ ⟨lp:byti:n⟩ by ⟨ty:dat⟩ nedobro"
            });
            return CommandResult::TURN;
        }
        let fruit = item.which == 1;
        if self.player.weapon == Some(id) {
            self.player.weapon = None;
        }
        self.consume_one(id);
        self.player.food_left = self.player.food_left.max(0);
        self.player.food_left = (self.player.food_left + 1100 + self.rng.rnd(400) as i32).min(2000);
        self.hungry_state = 0;
        if fruit {
            self.message(format!("mmm, {}... kako ⟨adv:vkųsny⟩!", self.options.fruit));
        } else if self.rng.rnd(100) > 70 {
            self.player.stats.experience += 1;
            self.message(
                "fuj, ⟨toj:nom:f⟩ ⟨n:jeda:nom⟩ ⟨v3:imati⟩ ⟨a:užasny:vkųs:acc⟩ ⟨n:vkųs:acc⟩",
            );
            self.check_experience();
        } else {
            self.message(if self.player.conditions.hallucinating {
                "o, hura, to ⟨lp:byti:n⟩ ⟨adv:vkųsny⟩"
            } else {
                "mmm, to ⟨lp:byti:n⟩ ⟨adv:vkųsny⟩"
            });
        }
        CommandResult::TURN
    }

    pub fn quaff(&mut self, id: u64) -> CommandResult {
        let Some(item) = self.player.inventory.iter().find(|item| item.id == id) else {
            return CommandResult::FREE;
        };
        if item.kind != ItemKind::Potion {
            self.message(if self.options.terse {
                "to ne jest ⟨adv:pitny⟩"
            } else {
                "fuj! Začto ⟨v2:hotěti⟩ piti to?"
            });
            return CommandResult::TURN;
        }
        let which = item.which;
        if self.player.weapon == Some(id) {
            self.player.weapon = None;
        }
        self.consume_one(id);
        let mut identified = false;
        match which {
            0 => {
                let hallucinating = self.player.conditions.hallucinating;
                identified = !hallucinating;
                self.player.conditions.confused = true;
                self.scheduler
                    .add_or_lengthen(Effect::Confusion, self.rng.spread(20));
                self.message(if hallucinating {
                    "kako ⟨a:divny:čuťje:nom⟩ ⟨n:čuťje:nom⟩!"
                } else {
                    "⟨vim:počekati⟩, čto tu ⟨v3h:stajati:staje⟩ sę. A? Čto? Kto?"
                })
            }
            1 => {
                identified = true;
                if !self.player.conditions.hallucinating {
                    self.seen_stairs = self.currently_visible(self.dungeon.stairs);
                }
                self.player.conditions.hallucinating = true;
                self.scheduler
                    .add_or_lengthen(Effect::Hallucination, self.rng.spread(850));
                self.refresh_hallucination_visuals();
                self.message("o, hura!  Vse ⟨v3:izględati⟩ tako ⟨adv:kosmičny⟩!")
            }
            2 => {
                identified = true;
                if self.wears_ring(2) {
                    self.message("na moment jest ⟨ty:dat⟩ nedobro");
                } else {
                    self.player.stats.strength =
                        (self.player.stats.strength - self.rng.rnd(3) as i32 - 1).max(3);
                    self.message("sejčas jest ⟨ty:dat⟩ mnogo nedobro");
                    if self.player.conditions.hallucinating {
                        self.player.conditions.hallucinating = false;
                        self.scheduler.cancel(Effect::Hallucination);
                        self.message("vse sejčas ⟨v3:izględati⟩ TAKO ⟨adv:nudny⟩.");
                    }
                }
            }
            3 => {
                identified = true;
                self.player.stats.strength = (self.player.stats.strength + 1).min(31);
                let base_strength = self.player.stats.strength - self.ring_strength_bonus();
                self.player.max_strength = self.player.max_strength.max(base_strength.clamp(3, 31));
                self.message("⟨v2:čuti⟩ sę ⟨cav:siľny⟩.  ⟨a:kaky:myšca:nom:pl:U⟩ ⟨n:myšca:nom:pl⟩!")
            }
            4 => {
                let was_blind = self.player.conditions.blind;
                self.player.conditions.see_invisible = true;
                self.player.conditions.blind = false;
                self.scheduler.cancel(Effect::Blindness);
                self.scheduler
                    .add_or_lengthen(Effect::SeeInvisible, self.rng.spread(850));
                self.message(format!(
                    "⟨toj:nom⟩ napitȯk ⟨v3:imati⟩ ⟨n:vkųs:acc⟩ ⟨n:sok:gen⟩ iz {}",
                    crate::lang::decl_guess(
                        &self.options.fruit,
                        Case::Gen,
                        interslavic::Number::Singular
                    )
                ));
                if was_blind {
                    self.message(if self.player.conditions.hallucinating {
                        "hura!  Vse opęť jest ⟨adv:kosmičny⟩"
                    } else {
                        "⟨n:zavěsa:nom⟩ ⟨n:ťma:gen⟩ ⟨v3:izčezati⟩"
                    });
                }
            }
            5 => {
                identified = true;
                self.player.stats.hp += self.rng.roll(self.player.stats.level as u32, 4);
                if self.player.stats.hp > self.player.stats.max_hp {
                    self.player.stats.max_hp += 1;
                    self.player.stats.hp = self.player.stats.max_hp
                }
                if self.player.conditions.blind {
                    self.player.conditions.blind = false;
                    self.scheduler.cancel(Effect::Blindness);
                    self.message(if self.player.conditions.hallucinating {
                        "hura!  Vse opęť jest ⟨adv:kosmičny⟩"
                    } else {
                        "⟨n:zavěsa:nom⟩ ⟨n:ťma:gen⟩ ⟨v3:izčezati⟩"
                    });
                }
                self.message("⟨v2:načinati⟩ čuti sę ⟨cav:dobry⟩")
            }
            6 => {
                identified = self
                    .monsters
                    .iter()
                    .any(|monster| !self.can_see_monster(monster));
                self.scheduler
                    .add_fuse(Effect::MonsterDetection, self.rng.spread(20));
                self.set_monster_detection(true);
                self.message(if identified {
                    "⟨v2:čuti⟩ ⟨n:blizkosť:acc⟩ ⟨n:čudovišče:gen:pl⟩"
                } else if self.player.conditions.hallucinating {
                    "na moment ⟨v2:čuti⟩ sę normaľno, potom to ⟨v3:prěhoditi⟩"
                } else {
                    "na moment ⟨v2:čuti⟩ sę ⟨adv:divny⟩, potom to ⟨v3:prěhoditi⟩"
                })
            }
            7 => {
                identified = !self.magic_positions().is_empty();
                self.message(if identified {
                    "⟨v2:čuti⟩ ⟨n:blizkosť:acc⟩ ⟨n:čar:gen:pl⟩ na ⟨toj:loc⟩ ⟨n:stųpenj:loc⟩"
                } else if self.player.conditions.hallucinating {
                    "na moment ⟨v2:čuti⟩ sę normaľno, potom to ⟨v3:prěhoditi⟩"
                } else {
                    "na moment ⟨v2:čuti⟩ sę ⟨adv:divny⟩, potom to ⟨v3:prěhoditi⟩"
                })
            }
            8 => {
                identified = true;
                self.message("naglo vse ⟨v2:dělati⟩ mnogo bolje umělo");
                self.player.stats.experience = crate::player::EXPERIENCE_LEVELS
                    .get((self.player.stats.level - 1) as usize)
                    .copied()
                    .unwrap_or(0)
                    + 1;
                self.check_experience();
            }
            9 => {
                identified = true;
                self.player.stats.hp += self.rng.roll(self.player.stats.level as u32, 8);
                if self.player.stats.hp > self.player.stats.max_hp {
                    if self.player.stats.hp > self.player.stats.max_hp + self.player.stats.level + 1
                    {
                        self.player.stats.max_hp += 1
                    }
                    self.player.stats.max_hp += 1;
                    self.player.stats.hp = self.player.stats.max_hp
                }
                if self.player.conditions.blind {
                    self.player.conditions.blind = false;
                    self.scheduler.cancel(Effect::Blindness);
                    self.message(if self.player.conditions.hallucinating {
                        "hura!  Vse opęť jest ⟨adv:kosmičny⟩"
                    } else {
                        "⟨n:zavěsa:nom⟩ ⟨n:ťma:gen⟩ ⟨v3:izčezati⟩"
                    });
                }
                if self.player.conditions.hallucinating {
                    self.player.conditions.hallucinating = false;
                    self.scheduler.cancel(Effect::Hallucination);
                    self.message("vse sejčas ⟨v3:izględati⟩ TAKO ⟨adv:nudny⟩.");
                }
                self.message("⟨v2:načinati⟩ čuti sę mnogo ⟨cav:dobry⟩")
            }
            10 => {
                identified = true;
                self.skip_world_once = true;
                if self.player.conditions.hasted {
                    self.player.conditions.asleep_turns += self.rng.rnd(8);
                    self.player_is_running = false;
                    self.player.conditions.hasted = false;
                    self.haste_phase = false;
                    self.scheduler.cancel(Effect::Haste);
                    self.message("⟨v2:omlěvati⟩")
                } else {
                    self.player.conditions.hasted = true;
                    self.scheduler
                        .add_or_lengthen(Effect::Haste, self.rng.rnd(4) as i32 + 4);
                    self.message("⟨v2:čuti⟩, že ⟨v2:dvigati⟩ sę mnogo ⟨cav:bystry⟩")
                }
            }
            11 => {
                self.player.stats.strength =
                    (self.player.max_strength + self.ring_strength_bonus()).clamp(3, 31);
                self.message("hej, to jest mnogo ⟨adv:vkųsny⟩.  ⟨v2:čuti:U⟩ ⟨n:teplo:acc⟩ po ⟨a:veś:tělo:loc⟩ ⟨n:tělo:loc⟩")
            }
            12 => {
                identified = true;
                self.player.conditions.blind = true;
                self.scheduler
                    .add_or_lengthen(Effect::Blindness, self.rng.spread(850));
                self.message(if self.player.conditions.hallucinating {
                    "o ne!  Vse jest ⟨adv:temny⟩!  Pomoć!"
                } else {
                    "⟨n:plašč:nom⟩ ⟨n:ťma:gen⟩ ⟨v3:padati⟩ okolo ⟨ty:gen:f⟩"
                })
            }
            13 => {
                identified = true;
                self.player.conditions.levitating = true;
                self.scheduler
                    .add_or_lengthen(Effect::Levitation, self.rng.spread(30));
                self.message(if self.player.conditions.hallucinating {
                    "o, hura!  ⟨v2:letěti:U⟩ v ⟨n:vȯzduh:loc⟩!"
                } else {
                    "⟨v2:načinati⟩ letěti v ⟨n:vȯzduh:loc⟩"
                })
            }
            _ => unreachable!(),
        }
        self.knowledge.potions[which as usize] |= identified;
        self.prepare_call(ItemKind::Potion, which);
        CommandResult::TURN
    }

    fn item_is_magic(item: &Item) -> bool {
        match item.kind {
            ItemKind::Armor => {
                item.protected
                    || item.armor_class != Some(crate::item::ARMOR_CLASS[item.which as usize])
            }
            ItemKind::Weapon => item.hit_plus != 0 || item.damage_plus != 0,
            ItemKind::Potion
            | ItemKind::Scroll
            | ItemKind::Ring
            | ItemKind::Stick
            | ItemKind::Amulet => true,
            ItemKind::Food | ItemKind::Gold | ItemKind::Bizarre(_) => false,
        }
    }

    pub fn magic_positions(&self) -> Vec<Pos> {
        let mut positions: Vec<Pos> = self
            .floor_items
            .iter()
            .filter(|item| Self::item_is_magic(item))
            .filter_map(|item| item.pos)
            .collect();
        if !self.floor_items.is_empty() {
            positions.extend(self.monsters.iter().filter_map(|monster| {
                monster
                    .inventory
                    .iter()
                    .any(Self::item_is_magic)
                    .then_some(monster.pos)
            }));
        }
        positions.sort_by_key(|pos| (pos.y, pos.x));
        positions.dedup();
        positions
    }

    pub fn food_positions(&self) -> Vec<Pos> {
        self.floor_items
            .iter()
            .filter(|item| item.kind == ItemKind::Food)
            .filter_map(|item| item.pos)
            .collect()
    }

    pub fn read_scroll(&mut self, id: u64) -> CommandResult {
        let Some(item) = self.player.inventory.iter().find(|item| item.id == id) else {
            return CommandResult::FREE;
        };
        if item.kind != ItemKind::Scroll {
            self.message(if self.options.terse {
                "ne ⟨v2:imati⟩ ⟨čto:gen⟩ čitati"
            } else {
                "na ⟨toj:loc⟩ ničto ne jest ⟨pp:napisati:n⟩"
            });
            return CommandResult::TURN;
        }
        let which = item.which;
        if self.player.weapon == Some(id) {
            self.player.weapon = None;
        }
        self.consume_one(id);
        let mut identified = false;
        match which {
            0 => {
                self.player.conditions.can_confuse_monster = true;
                let color = self.pick_color("črveny");
                self.message(format!(
                    "⟨a:tvoj:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩ ⟨v3p:načinati⟩ světiti sę {}",
                    crate::lang::color_adv(color)
                ))
            }
            1 => {
                identified = true;
                let ps: Vec<_> = self.dungeon.map.iter().map(|(p, _)| p).collect();
                for p in ps {
                    if let Some(c) = self.dungeon.map.get_mut(p) {
                        match c.terrain {
                            Terrain::SecretDoor
                            | Terrain::SecretDoorHorizontal
                            | Terrain::SecretDoorVertical => {
                                c.terrain = Terrain::Door;
                                c.seen = true;
                                c.remembered = '+';
                            }
                            Terrain::SecretPassage => {
                                c.terrain = Terrain::Passage;
                                c.seen = true;
                                c.remembered = '#';
                            }
                            Terrain::Door | Terrain::Passage | Terrain::Stairs => {
                                c.seen = true;
                                c.remembered = c.terrain.glyph();
                            }
                            Terrain::Floor if c.trap.is_some() && !c.trap_revealed => {
                                c.trap_revealed = true;
                                c.seen = true;
                                c.remembered = '^';
                            }
                            _ => {}
                        }
                    }
                }
                self.message("o, sejčas na ⟨toj:loc⟩ ⟨n:svitȯk:loc⟩ jest karta")
            }
            2 => {
                let mut held = 0;
                for m in &mut self.monsters {
                    if (m.pos.x - self.player.pos.x).abs() <= 2
                        && (m.pos.y - self.player.pos.y).abs() <= 2
                        && m.awake
                    {
                        m.flags |= monster::HELD;
                        m.awake = false;
                        held += 1
                    }
                }
                self.message(if held > 1 {
                    "⟨n:čudovišče:nom:pl⟩ okolo ⟨ty:gen:f⟩ ne ⟨v3p:mogti⟩ dvigati sę"
                } else if held == 1 {
                    "⟨n:čudovišče:nom⟩ ne ⟨v3:mogti⟩ dvigati sę"
                } else {
                    "⟨v2:imati⟩ ⟨a:divny:čuťje:acc⟩ ⟨n:čuťje:acc⟩ ⟨n:utrata:gen⟩"
                });
                identified = held > 0;
            }
            3 => {
                identified = true;
                self.player.conditions.asleep_turns += self.rng.rnd(5) + 4;
                self.player_is_running = false;
                self.message("⟨v2:usnųti⟩")
            }
            4 => {
                if let Some(a) = self
                    .player
                    .armor
                    .and_then(|id| self.player.inventory.iter_mut().find(|i| i.id == id))
                {
                    a.armor_class = a.armor_class.map(|v| v - 1);
                    a.cursed = false;
                    let color = self.pick_color("srěbrny");
                    self.message(format!(
                        "⟨a:tvoj:brȯnja:nom⟩ ⟨n:brȯnja:nom⟩ na moment ⟨v3:světiti⟩ sę {}",
                        crate::lang::color_adv(color)
                    ))
                }
            }
            5..=9 => {
                identified = true;
                self.pending_identification = Some(match which {
                    5 => IdentifyKind::Potion,
                    6 => IdentifyKind::Scroll,
                    7 => IdentifyKind::Weapon,
                    8 => IdentifyKind::Armor,
                    _ => IdentifyKind::RingOrStick,
                });
                self.message(format!(
                    "toj svitȯk jest svitȯk {}",
                    crate::lang::scroll_effect_gen(which as usize)
                ))
            }
            10 => self.message("⟨v2:slyšati⟩ daleko ⟨a:bezumny:směh:acc⟩ ⟨n:směh:acc⟩"),
            11 => {
                identified = self.floor_items.iter().any(|i| i.kind == ItemKind::Food);
                self.message(if identified {
                    "⟨a:tvoj:nos:nom⟩ ⟨n:nos:nom⟩ ⟨v3:svŕběti⟩ i ⟨v2:čuti⟩ ⟨n:zapah:acc⟩ ⟨n:jeda:gen⟩"
                } else {
                    "⟨a:tvoj:nos:nom⟩ ⟨n:nos:nom⟩ ⟨v3:svŕběti⟩"
                })
            }
            12 => {
                let old_area = self.area_key(self.player.pos);
                self.teleport_player();
                identified = self.area_key(self.player.pos) != old_area;
            }
            13 => {
                if let Some(w) = self.player.weapon.and_then(|id| {
                    self.player
                        .inventory
                        .iter_mut()
                        .find(|i| i.id == id && i.kind == ItemKind::Weapon)
                }) {
                    w.cursed = false;
                    if self.rng.rnd(2) == 0 {
                        w.hit_plus += 1
                    } else {
                        w.damage_plus += 1
                    }
                    let name = crate::lang::phrase(
                        &crate::lang::WEAPON_LEX[w.which as usize],
                        Case::Nom,
                        interslavic::Number::Singular,
                    );
                    let color = self.pick_color("modry");
                    self.message(format!(
                        "{name} na moment ⟨v3:světiti⟩ sę {}",
                        crate::lang::color_adv(color)
                    ))
                } else {
                    self.message("⟨v2:imati⟩ ⟨a:divny:čuťje:acc⟩ ⟨n:čuťje:acc⟩ ⟨n:utrata:gen⟩")
                }
            }
            14 => {
                let mut count = 0;
                let mut destination = None;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let pos = self.player.pos.offset(dx, dy);
                        if !self.passable(pos)
                            || self.monsters.iter().any(|monster| monster.pos == pos)
                            || self.floor_items.iter().any(|item| {
                                item.pos == Some(pos)
                                    && item.kind == ItemKind::Scroll
                                    && item.which == 10
                            })
                        {
                            continue;
                        }
                        count += 1;
                        if self.rng.rnd(count) == 0 {
                            destination = Some(pos);
                        }
                    }
                }
                if let Some(pos) = destination {
                    let kind = monster::random_kind(&mut self.rng, self.depth, false);
                    let monster = self.make_monster(kind, pos, self.depth, false, false);
                    self.monsters.insert(0, monster);
                } else {
                    self.message(
                        "⟨v2:slyšati⟩ daleko ⟨a:slaby:krik:acc⟩ ⟨n:krik:acc⟩ ⟨n:bolj:gen⟩",
                    );
                }
            }
            15 => {
                for id in [
                    self.player.weapon,
                    self.player.armor,
                    self.player.rings[0],
                    self.player.rings[1],
                ]
                .into_iter()
                .flatten()
                {
                    if let Some(i) = self.player.inventory.iter_mut().find(|i| i.id == id) {
                        i.cursed = false
                    }
                }
                self.message(if self.player.conditions.hallucinating {
                    "⟨v2:čuti⟩ ⟨n:jedinstvo:acc⟩ s ⟨n:vsemir:ins:U⟩"
                } else {
                    "⟨v2:čuti⟩, že někto ⟨ty:acc⟩ ⟨v3:strěgti⟩"
                })
            }
            16 => {
                self.aggravate_monsters();
                self.message("⟨v2:slyšati⟩ ⟨a:vysoky:zvųk:acc⟩ ⟨n:zvųk:acc⟩")
            }
            17 => {
                if let Some(a) = self
                    .player
                    .armor
                    .and_then(|id| self.player.inventory.iter_mut().find(|i| i.id == id))
                {
                    a.protected = true;
                    let color = self.pick_color("zlåty");
                    self.message(format!(
                        "⟨a:tvoj:brȯnja:acc⟩ ⟨n:brȯnja:acc⟩ ⟨v3:pokryvati⟩ ⟨ap:migati:ščit:nom⟩ {} ⟨n:ščit:nom⟩",
                        crate::lang::color_masc_nom(color)
                    ))
                } else {
                    self.message("⟨v2:imati⟩ ⟨a:divny:čuťje:acc⟩ ⟨n:čuťje:acc⟩ ⟨n:utrata:gen⟩")
                }
            }
            _ => unreachable!(),
        }
        self.knowledge.scrolls[which as usize] |= identified;
        self.prepare_call(ItemKind::Scroll, which);
        CommandResult::TURN
    }

    pub fn wield(&mut self, id: u64) -> CommandResult {
        if let Some(old) = self.player.weapon {
            if !self.dropcheck_item(old) {
                return CommandResult::TURN;
            }
            self.player.weapon = Some(old);
        }
        let Some(item) = self.player.inventory.iter().find(|item| item.id == id) else {
            return CommandResult::FREE;
        };
        if item.kind == ItemKind::Armor {
            self.message("ne ⟨v2:mogti⟩ dŕžati ⟨n:brȯnja:acc⟩ kako orųžje");
            return CommandResult::FREE;
        }
        if self.player.weapon == Some(id) {
            self.message("to uže ⟨v2:koristati⟩");
            return CommandResult::FREE;
        }
        if self.player.armor == Some(id) || self.player.rings.contains(&Some(id)) {
            self.message("to uže ⟨v2:koristati⟩");
            return CommandResult::FREE;
        }
        let letter = item.pack_letter.unwrap_or('?');
        self.player.weapon = Some(id);
        self.message(if self.options.terse {
            format!("orųžje: {} ({letter})", self.inventory_name(item, true))
        } else {
            format!(
                "sejčas ⟨v2:dŕžati⟩ {} ({letter})",
                self.inventory_name_case(item, true, Case::Acc)
            )
        });
        CommandResult::TURN
    }
    pub fn wear(&mut self, id: u64) -> CommandResult {
        if self.player.armor.is_some() {
            self.message(if self.options.terse {
                "uže ⟨v2:nositi⟩ ⟨n:brȯnja:acc⟩"
            } else {
                "uže ⟨v2:nositi⟩ ⟨n:brȯnja:acc⟩.  Pŕvo ⟨v2:musěti⟩ sjęti ⟨ona:acc⟩"
            });
            return CommandResult::FREE;
        }
        if !self
            .player
            .inventory
            .iter()
            .any(|i| i.id == id && i.kind == ItemKind::Armor)
        {
            self.message("ne ⟨v2:mogti⟩ nositi to");
            return CommandResult::TURN;
        }
        self.after_turn();
        if self.end != EndState::Playing {
            return CommandResult::FREE;
        }
        self.player.armor = Some(id);
        if let Some(armor) = self.player.inventory.iter_mut().find(|item| item.id == id) {
            armor.known = true;
        }
        let armor = self
            .player
            .inventory
            .iter()
            .find(|item| item.id == id)
            .unwrap();
        self.message(if self.options.terse {
            format!("⟨v2:nositi⟩ sejčas: {}", self.inventory_name(armor, true))
        } else {
            format!(
                "⟨v2:naděvati⟩ {}",
                self.inventory_name_case(armor, true, Case::Acc)
            )
        });
        CommandResult::TURN
    }
    pub fn take_off(&mut self) -> CommandResult {
        let Some(id) = self.player.armor else {
            self.message("ne ⟨v2:nositi⟩ ⟨n:brȯnja:acc⟩");
            return CommandResult::FREE;
        };
        let item = self
            .player
            .inventory
            .iter()
            .find(|item| item.id == id)
            .unwrap();
        let name = self.inventory_name(item, true);
        let name_acc = self.inventory_name_case(item, true, Case::Acc);
        let letter = item.pack_letter.unwrap_or('?');
        if !self.dropcheck_item(id) {
            return CommandResult::TURN;
        }
        self.player.armor = None;
        self.message(if self.options.terse {
            format!("uže ne ⟨v2:nositi⟩: {letter}) {name}")
        } else {
            format!("⟨v2:snimati⟩ {name_acc} ({letter})")
        });
        CommandResult::TURN
    }
    pub fn put_on_ring(&mut self, id: u64, hand: usize) -> CommandResult {
        if !self
            .player
            .inventory
            .iter()
            .any(|i| i.id == id && i.kind == ItemKind::Ring)
        {
            self.message(if self.options.terse {
                "to ne jest pŕstenj"
            } else {
                "⟨adv:trudny⟩ ⟨lp:byti:n⟩ by naděti to na pŕst"
            });
            return CommandResult::TURN;
        }
        if self.player.weapon == Some(id)
            || self.player.armor == Some(id)
            || self.player.rings.contains(&Some(id))
        {
            self.message("to uže ⟨v2:koristati⟩");
            return CommandResult::TURN;
        }
        if hand > 1 || self.player.rings[hand].is_some() {
            self.message(if self.options.terse {
                "uže ⟨v2:nositi⟩ dva ⟨n:pŕstenj:nom:pl⟩"
            } else {
                "uže ⟨v2:imati⟩ pŕstenj na ⟨a:každy:rųka:loc⟩ ⟨n:rųka:loc⟩"
            });
            return CommandResult::TURN;
        }
        self.player.rings[hand] = Some(id);
        if let Some((which, bonus)) = self
            .player
            .inventory
            .iter()
            .find(|i| i.id == id)
            .map(|r| (r.which, r.armor_class.unwrap_or(0)))
        {
            if which == 1 {
                self.player.stats.strength = (self.player.stats.strength + bonus).clamp(3, 31)
            }
            if which == 4 {
                self.player.conditions.see_invisible = true
            }
            if which == 6 {
                self.aggravate_monsters();
            }
        }
        let ring = self
            .player
            .inventory
            .iter()
            .find(|item| item.id == id)
            .unwrap();
        let name = self.inventory_name(ring, true);
        let letter = ring.pack_letter.unwrap_or('?');
        let name_acc = self.inventory_name_case(ring, true, Case::Acc);
        self.message(if self.options.terse {
            format!("{name} ({letter})")
        } else {
            format!("⟨v2:naděvati⟩ {name_acc} ({letter})")
        });
        CommandResult::TURN
    }
    pub fn remove_ring(&mut self, hand: usize) -> CommandResult {
        let Some(id) = self.player.rings.get(hand).copied().flatten() else {
            self.message("ne ⟨v2:nositi⟩ ⟨taky:acc⟩ ⟨n:pŕstenj:acc⟩");
            return CommandResult::FREE;
        };
        let ring = self
            .player
            .inventory
            .iter()
            .find(|item| item.id == id)
            .unwrap();
        let name = self.inventory_name(ring, true);
        let letter = ring.pack_letter.unwrap_or('?');
        if !self.dropcheck_item(id) {
            return CommandResult::TURN;
        }
        self.message(format!("⟨v2:snimati⟩ {name}({letter})"));
        CommandResult::TURN
    }

    fn dropcheck_item(&mut self, id: u64) -> bool {
        let Some(item) = self.player.inventory.iter().find(|item| item.id == id) else {
            return true;
        };
        let equipped = self.player.weapon == Some(id)
            || self.player.armor == Some(id)
            || self.player.rings.contains(&Some(id));
        if !equipped {
            return true;
        }
        if item.cursed {
            self.message("ne ⟨v2:mogti⟩.  ⟨v3:izględati:U⟩, že to jest ⟨pp:proklęti:n⟩");
            return false;
        }
        if self.player.weapon == Some(id) {
            self.player.weapon = None;
        } else if self.player.armor == Some(id) {
            self.after_turn();
            self.player.armor = None;
        } else if let Some(hand) = self.player.rings.iter().position(|ring| *ring == Some(id)) {
            let which = item.which;
            let bonus = item.armor_class.unwrap_or(0);
            self.player.rings[hand] = None;
            if which == 1 {
                self.player.stats.strength = (self.player.stats.strength - bonus).clamp(3, 31);
            } else if which == 4 {
                self.player.conditions.see_invisible = false;
                self.scheduler.cancel(Effect::SeeInvisible);
            }
        }
        true
    }
    pub fn drop_item(&mut self, id: u64) -> CommandResult {
        if !self.dungeon.map.get(self.player.pos).is_some_and(|cell| {
            matches!(cell.terrain, Terrain::Floor | Terrain::Passage) && !cell.trap_revealed
        }) || self
            .floor_items
            .iter()
            .any(|i| i.pos == Some(self.player.pos))
        {
            self.message("tam uže něčto jest");
            return CommandResult::FREE;
        }
        let Some(index) = self.player.inventory.iter().position(|i| i.id == id) else {
            return CommandResult::FREE;
        };
        if !self.dropcheck_item(id) {
            return CommandResult::TURN;
        }
        let mut item = self.player.inventory.remove(index);
        if item.count > 1
            && matches!(
                item.kind,
                ItemKind::Potion | ItemKind::Scroll | ItemKind::Food
            )
        {
            let remaining = item.count - 1;
            item.count = 1;
            let mut rest = item.clone();
            rest.id = id;
            rest.count = remaining;
            self.player.inventory.insert(index, rest);
            item.id = self.id();
        }
        item.pos = Some(self.player.pos);
        item.pack_letter = None;
        item.dropped_once = true;
        if item.kind == ItemKind::Amulet {
            self.has_amulet = false;
        }
        let dropped_terse = self.inventory_name(&item, true);
        let dropped_name = self.inventory_name_case(&item, true, Case::Acc);
        self.floor_items.push(item);
        self.message(if self.options.terse {
            format!("⟨pp:ostaviti:n⟩: {dropped_terse}")
        } else {
            format!("⟨v2:ostavjati⟩ {dropped_name}")
        });
        CommandResult::TURN
    }

    pub fn finish_action(&mut self, result: CommandResult) {
        if result.consumed_turn {
            self.advance_player_turn();
        }
    }
    pub fn identify_item(&mut self, id: u64) -> CommandResult {
        let Some(index) = self.player.inventory.iter().position(|item| item.id == id) else {
            return CommandResult::FREE;
        };
        let required = self.pending_identification;
        let allowed = required.is_none_or(|kind| match kind {
            IdentifyKind::Potion => self.player.inventory[index].kind == ItemKind::Potion,
            IdentifyKind::Scroll => self.player.inventory[index].kind == ItemKind::Scroll,
            IdentifyKind::Weapon => self.player.inventory[index].kind == ItemKind::Weapon,
            IdentifyKind::Armor => self.player.inventory[index].kind == ItemKind::Armor,
            IdentifyKind::RingOrStick => matches!(
                self.player.inventory[index].kind,
                ItemKind::Ring | ItemKind::Stick
            ),
        });
        if !allowed {
            let required = match required.expect("unrestricted identification accepts all items") {
                IdentifyKind::Potion => "napitȯk",
                IdentifyKind::Scroll => "svitȯk",
                IdentifyKind::Weapon => "orųžje",
                IdentifyKind::Armor => "⟨n:brȯnja:acc⟩",
                IdentifyKind::RingOrStick => "pŕstenj, žezlo ili posoh",
            };
            self.message(format!("⟨v2:musěti⟩ opoznati {required}"));
            return CommandResult::FREE;
        }
        let item = &mut self.player.inventory[index];
        item.known = true;
        match item.kind {
            ItemKind::Potion => self.knowledge.potions[item.which as usize] = true,
            ItemKind::Scroll => self.knowledge.scrolls[item.which as usize] = true,
            ItemKind::Ring => self.knowledge.rings[item.which as usize] = true,
            ItemKind::Stick => self.knowledge.sticks[item.which as usize] = true,
            _ => {}
        }
        self.pending_identification = None;
        let description = self.inventory_name(&self.player.inventory[index], false);
        self.message(description);
        CommandResult::FREE
    }
    pub fn call_item(&mut self, id: u64, label: String) -> CommandResult {
        let Some(index) = self.player.inventory.iter().position(|item| item.id == id) else {
            return CommandResult::FREE;
        };
        let item = &self.player.inventory[index];
        if item.kind == ItemKind::Food {
            self.message("ne ⟨v2:mogti⟩ to nikako nazvati");
            return CommandResult::FREE;
        }
        let known = match item.kind {
            ItemKind::Potion => self.knowledge.potions[item.which as usize],
            ItemKind::Scroll => self.knowledge.scrolls[item.which as usize],
            ItemKind::Ring => self.knowledge.rings[item.which as usize],
            ItemKind::Stick => self.knowledge.sticks[item.which as usize],
            _ => false,
        };
        if known {
            self.message("to uže jest ⟨pp:opoznati:n⟩");
            return CommandResult::FREE;
        }
        let value = Some(label);
        if let Some(guess) = Self::guess_index(item.kind, item.which) {
            self.knowledge.guesses[guess] = value;
        } else {
            self.player.inventory[index].label = value;
        }
        self.message("");
        CommandResult::FREE
    }

    fn prepare_call(&mut self, kind: ItemKind, which: u8) {
        let known = match kind {
            ItemKind::Potion => self.knowledge.potions[which as usize],
            ItemKind::Scroll => self.knowledge.scrolls[which as usize],
            _ => return,
        };
        let Some(index) = Self::guess_index(kind, which) else {
            return;
        };
        if known {
            self.knowledge.guesses[index] = None;
        } else if self.knowledge.guesses[index].is_none() {
            self.pending_call = Some((kind, which));
        }
    }

    pub fn finish_pending_call(&mut self, label: String) {
        let Some((kind, which)) = self.pending_call.take() else {
            return;
        };
        let canonical = matches!(
            label.as_str(),
            " ;?" | "? ;" | " ?" | ";?" | "? " | "?;" | "?"
        );
        let label = if canonical {
            match kind {
                ItemKind::Potion => crate::lang::potion_effect_gen(which as usize),
                ItemKind::Scroll => crate::lang::scroll_effect_gen(which as usize),
                _ => label,
            }
        } else {
            label
        };
        if let Some(index) = Self::guess_index(kind, which) {
            self.knowledge.guesses[index] = Some(label);
        }
        self.message("");
    }

    fn guess_index(kind: ItemKind, which: u8) -> Option<usize> {
        let offset = match kind {
            ItemKind::Potion => 0,
            ItemKind::Scroll => 14,
            ItemKind::Ring => 32,
            ItemKind::Stick => 46,
            _ => return None,
        };
        Some(offset + which as usize)
    }

    pub fn item_guess(&self, item: &Item) -> Option<&str> {
        Self::guess_index(item.kind, item.which)
            .and_then(|index| self.knowledge.guesses[index].as_deref())
    }

    /// Prose item name in the nominative (the common sentence-subject /
    /// listing form). Case-governed slots use `item_name_case`.
    pub fn item_name(&self, item: &Item) -> String {
        self.item_name_case(item, Case::Nom)
    }

    /// Prose item name declined to the case its sentence governs. The
    /// magic-effect genitive ("napitȯk lěčeńja") is invariant; only the
    /// head noun and any agreeing adjective decline.
    pub fn item_name_case(&self, item: &Item, case: Case) -> String {
        use crate::lang::{self, adj_for, decl, material_of, phrase};
        use interslavic::Number::Singular;
        let called = || {
            self.item_guess(item)
                .or(item.label.as_deref())
                .map(|label| format!(" «{label}»"))
                .unwrap_or_default()
        };
        match item.kind {
            ItemKind::Potion => {
                let head = decl(&lang::POTION, case, Singular);
                if self.knowledge.potions[item.which as usize] {
                    format!("{head} {}", lang::potion_effect_gen(item.which as usize))
                } else {
                    let color =
                        lang::COLOR_ADJ[self.appearances.potion_colors[item.which as usize]];
                    format!(
                        "{} {head}{}",
                        adj_for(color, &lang::POTION, case, Singular),
                        called()
                    )
                }
            }
            ItemKind::Scroll => {
                let head = decl(&lang::SCROLL, case, Singular);
                if self.knowledge.scrolls[item.which as usize] {
                    format!("{head} {}", lang::scroll_effect_gen(item.which as usize))
                } else {
                    format!(
                        "{head} '{}'{}",
                        self.appearances.scroll_titles[item.which as usize],
                        called()
                    )
                }
            }
            ItemKind::Ring => {
                let head = decl(&lang::RING, case, Singular);
                if self.knowledge.rings[item.which as usize] {
                    let bonus = if item.known && matches!(item.which, 0 | 1 | 7 | 8) {
                        format!(" {:+}", item.armor_class.unwrap_or(0))
                    } else {
                        String::new()
                    };
                    format!(
                        "{head} {}{}",
                        lang::ring_effect_gen(item.which as usize),
                        bonus
                    )
                } else {
                    let stone = &lang::STONE_LEX[self.appearances.ring_stones[item.which as usize]];
                    format!("{head} {}{}", material_of(stone), called())
                }
            }
            ItemKind::Stick => {
                let is_staff = self.appearances.stick_is_staff[item.which as usize];
                let head_lex = if is_staff { &lang::STAFF } else { &lang::WAND };
                let head = decl(head_lex, case, Singular);
                if self.knowledge.sticks[item.which as usize] {
                    let charges = if item.known {
                        format!(" [{}]", item.charges)
                    } else {
                        String::new()
                    };
                    format!(
                        "{head} {}{}",
                        lang::stick_effect_gen(item.which as usize),
                        charges
                    )
                } else {
                    let material = &lang::stick_material_lex(
                        is_staff,
                        self.appearances.stick_materials[item.which as usize],
                    );
                    format!("{head} {}{}", material_of(material), called())
                }
            }
            ItemKind::Weapon if item.known => format!(
                "{} {:+}/{:+}",
                phrase(&lang::WEAPON_LEX[item.which as usize], case, Singular),
                item.hit_plus,
                item.damage_plus
            ),
            ItemKind::Weapon => phrase(&lang::WEAPON_LEX[item.which as usize], case, Singular),
            ItemKind::Armor if item.known => format!(
                "{} [ohråna {}]",
                phrase(&lang::ARMOR_LEX[item.which as usize], case, Singular),
                10 - item.armor_class.unwrap_or(10)
            ),
            ItemKind::Armor => phrase(&lang::ARMOR_LEX[item.which as usize], case, Singular),
            ItemKind::Food if item.which == 1 => {
                lang::decl_guess(&self.options.fruit, case, Singular)
            }
            ItemKind::Food => format!(
                "{} {}",
                decl(&lang::FOOD_PORTION, case, Singular),
                lang::food_gen()
            ),
            ItemKind::Amulet => format!("{} ⟨n:Jendor:gen⟩", decl(&lang::AMULET, case, Singular)),
            ItemKind::Gold => decl(&lang::GOLD_COIN, case, interslavic::Number::Plural),
            ItemKind::Bizarre(glyph) => {
                format!(
                    "něčto {} {}",
                    crate::lang::color_adv("divny"),
                    command_label(glyph)
                )
            }
        }
    }

    pub fn inventory_name(&self, item: &Item, drop: bool) -> String {
        self.inventory_name_case(item, drop, Case::Nom)
    }

    /// Count-aware pack name declined for the sentence slot it fills.
    /// Numeral government still overrides for 5+ (genitive plural).
    pub fn inventory_name_case(&self, item: &Item, drop: bool, case: Case) -> String {
        use crate::lang::{self, Phrase, adj_for, decl, material_of, phrase};
        use interslavic::Number::{Plural, Singular};
        let count = item.count;
        // Numeral government: 1 → case sg, 2–4 → case pl, 5+ → Gen pl.
        let counted_num =
            |n: u32| -> interslavic::Number { if n == 1 { Singular } else { Plural } };
        let counted_case = |n: u32| -> Case {
            if (2..=4).contains(&n) || n == 1 {
                case
            } else {
                Case::Gen
            }
        };
        let head_of = |n: u32, l: &lang::Lex| -> String {
            let form = decl(l, counted_case(n), counted_num(n));
            if n == 1 { form } else { format!("{n} {form}") }
        };
        let phrase_of = |n: u32, p: &Phrase| -> String {
            let form = phrase(p, counted_case(n), counted_num(n));
            if n == 1 { form } else { format!("{n} {form}") }
        };
        let guess = self.item_guess(item).or(item.label.as_deref());
        let signed = |value: i32| format!("{value:+}");
        let mut name = match item.kind {
            ItemKind::Potion => {
                let color = lang::COLOR_ADJ[self.appearances.potion_colors[item.which as usize]];
                let known = self.knowledge.potions[item.which as usize];
                if known || guess.is_some() {
                    let head = head_of(count, &lang::POTION);
                    if known {
                        format!(
                            "{head} {}({color})",
                            lang::potion_effect_gen(item.which as usize)
                        )
                    } else {
                        format!("{head} «{}»({color})", guess.unwrap())
                    }
                } else {
                    let form = format!(
                        "{} {}",
                        adj_for(
                            color,
                            &lang::POTION,
                            counted_case(count),
                            counted_num(count)
                        ),
                        decl(&lang::POTION, counted_case(count), counted_num(count))
                    );
                    if count == 1 {
                        form
                    } else {
                        format!("{count} {form}")
                    }
                }
            }
            ItemKind::Scroll => {
                let head = head_of(count, &lang::SCROLL);
                if self.knowledge.scrolls[item.which as usize] {
                    format!("{head} {}", lang::scroll_effect_gen(item.which as usize))
                } else if let Some(guess) = guess {
                    format!("{head} «{guess}»")
                } else {
                    format!(
                        "{head} '{}'",
                        self.appearances.scroll_titles[item.which as usize]
                    )
                }
            }
            ItemKind::Ring => {
                let stone = &lang::STONE_LEX[self.appearances.ring_stones[item.which as usize]];
                let known = self.knowledge.rings[item.which as usize];
                if known || guess.is_some() {
                    let head = head_of(count, &lang::RING);
                    let bonus = if item.known && matches!(item.which, 0 | 1 | 7 | 8) {
                        format!(" [{}]", signed(item.armor_class.unwrap_or(0)))
                    } else {
                        String::new()
                    };
                    if known {
                        format!(
                            "{head} {}{bonus}({})",
                            lang::ring_effect_gen(item.which as usize),
                            stone.lemma
                        )
                    } else {
                        format!("{head} «{}»{bonus}({})", guess.unwrap(), stone.lemma)
                    }
                } else {
                    format!("{} {}", head_of(count, &lang::RING), material_of(stone))
                }
            }
            ItemKind::Stick => {
                let is_staff = self.appearances.stick_is_staff[item.which as usize];
                let material = lang::stick_material_lex(
                    is_staff,
                    self.appearances.stick_materials[item.which as usize],
                );
                let head_lex = if is_staff { &lang::STAFF } else { &lang::WAND };
                let known = self.knowledge.sticks[item.which as usize];
                if known || guess.is_some() {
                    let head = head_of(count, head_lex);
                    let charges = if item.known {
                        if self.options.terse {
                            format!(" [{}]", item.charges)
                        } else {
                            format!(" [{} ⟨n:naboj:gen:pl⟩]", item.charges)
                        }
                    } else {
                        String::new()
                    };
                    if known {
                        format!(
                            "{head} {}{charges}({})",
                            lang::stick_effect_gen(item.which as usize),
                            material.lemma
                        )
                    } else {
                        format!("{head} «{}»{charges}({})", guess.unwrap(), material.lemma)
                    }
                } else {
                    format!("{} {}", head_of(count, head_lex), material_of(&material))
                }
            }
            ItemKind::Food if item.which == 1 => {
                let form =
                    lang::decl_guess(&self.options.fruit, counted_case(count), counted_num(count));
                if count == 1 {
                    form
                } else {
                    format!("{count} {form}")
                }
            }
            ItemKind::Food if count == 1 => {
                format!(
                    "{} {}",
                    decl(&lang::FOOD_PORTION, case, Singular),
                    lang::food_gen()
                )
            }
            ItemKind::Food => {
                format!(
                    "{} {}",
                    head_of(count, &lang::FOOD_PORTION),
                    lang::food_gen()
                )
            }
            ItemKind::Weapon => {
                let weapon = &lang::WEAPON_LEX[item.which as usize];
                let bonus = if item.known {
                    format!(
                        "{},{} ",
                        signed(item.hit_plus.into()),
                        signed(item.damage_plus.into())
                    )
                } else {
                    String::new()
                };
                let called = item
                    .label
                    .as_deref()
                    .map(|label| format!(" «{label}»"))
                    .unwrap_or_default();
                format!("{bonus}{}{called}", phrase_of(count, weapon))
            }
            ItemKind::Armor => {
                let armor = &lang::ARMOR_LEX[item.which as usize];
                let called = item
                    .label
                    .as_deref()
                    .map(|label| format!(" «{label}»"))
                    .unwrap_or_default();
                if item.known {
                    let enchantment = crate::item::ARMOR_CLASS[item.which as usize]
                        - item.armor_class.unwrap_or(10);
                    let protection = 10 - item.armor_class.unwrap_or(10);
                    let protection_word = if self.options.terse { "" } else { "ohråna " };
                    format!(
                        "{} {} [{protection_word}{protection}]{called}",
                        signed(enchantment),
                        phrase(armor, case, Singular)
                    )
                } else {
                    format!("{}{called}", phrase(armor, case, Singular))
                }
            }
            ItemKind::Amulet => format!(
                "{} ⟨n:Jendor:gen⟩",
                decl(&lang::AMULET, Case::Nom, Singular)
            ),
            ItemKind::Gold => format!(
                "{} {}",
                item.gold_value,
                interslavic::quantified(
                    item.gold_value as u64,
                    lang::GOLD_COIN.lemma,
                    case,
                    lang::GOLD_COIN.gender,
                    lang::GOLD_COIN.animacy,
                )
            ),
            ItemKind::Bizarre(glyph) => {
                format!(
                    "něčto {} {}",
                    crate::lang::color_adv("divny"),
                    command_label(glyph)
                )
            }
        };
        if drop && let Some(first) = name.get_mut(0..1) {
            first.make_ascii_lowercase();
        }
        name
    }

    pub fn throw_item(&mut self, id: u64, direction: Direction) -> CommandResult {
        let Some(index) = self.player.inventory.iter().position(|i| i.id == id) else {
            return CommandResult::FREE;
        };
        if !self.dropcheck_item(id) {
            return CommandResult::TURN;
        }
        if self.player.weapon == Some(id)
            || self.player.armor == Some(id)
            || self.player.rings.contains(&Some(id))
        {
            self.message("to uže ⟨v2:koristati⟩");
            return CommandResult::TURN;
        }
        let mut projectile = self.player.inventory[index].clone();
        projectile.count = 1;
        if self.player.inventory[index].count > 1 {
            self.player.inventory[index].count -= 1
        } else {
            self.player.inventory.remove(index);
        }
        projectile.id = self.id();
        projectile.pack_letter = None;
        let (dx, dy) = direction.delta();
        let mut pos = self.player.pos;
        loop {
            let next = pos.offset(dx, dy);
            if let Some(mi) = self.monsters.iter().position(|m| m.pos == next) {
                if !self.thrown_attack(mi, &projectile) {
                    self.land_projectile(projectile, next, true);
                }
                break;
            }
            if !self.passable(next)
                || self
                    .dungeon
                    .map
                    .get(next)
                    .is_some_and(|c| c.terrain == Terrain::Door)
            {
                self.land_projectile(projectile, next, true);
                break;
            }
            pos = next;
        }
        CommandResult::TURN
    }

    fn fall_position(&mut self, center: Pos) -> Option<Pos> {
        let mut count = 0;
        let mut destination = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let pos = center.offset(dx, dy);
                if pos == self.player.pos
                    || self.floor_items.iter().any(|other| other.pos == Some(pos))
                {
                    continue;
                }
                if self.dungeon.map.get(pos).is_some_and(|cell| {
                    matches!(cell.terrain, Terrain::Floor | Terrain::Passage) && !cell.trap_revealed
                }) {
                    count += 1;
                    if self.rng.rnd(count) == 0 {
                        destination = Some(pos);
                    }
                }
            }
        }
        destination
    }

    fn land_projectile(&mut self, mut item: Item, center: Pos, report_vanish: bool) {
        if let Some(pos) = self.fall_position(center) {
            item.pos = Some(pos);
            self.floor_items.push(item);
        } else if report_vanish {
            let name = if item.kind == ItemKind::Weapon {
                crate::lang::phrase(
                    &crate::lang::WEAPON_LEX[item.which as usize],
                    Case::Nom,
                    interslavic::Number::Singular,
                )
            } else {
                "prědmet".to_string()
            };
            self.message(format!(
                "{name} ⟨v3:izčezati⟩ pri ⟨n:udar:loc⟩ o ⟨n:zemja:acc⟩"
            ));
        }
    }

    fn thrown_attack(&mut self, index: usize, item: &Item) -> bool {
        self.quiet_turns = 0;
        self.runto_monster(index);
        if self.reveal_xeroc(index) {
            self.message(if self.player.conditions.hallucinating {
                "strašno!  To jest ⟨a:zly:stvorjeńje:nom⟩ ⟨n:stvorjeńje:nom⟩!"
            } else {
                "⟨vim:počekati⟩!  To jest kserok!"
            });
        }
        let bow = (item.kind == ItemKind::Weapon && item.which == 3)
            .then(|| {
                self.player.weapon.and_then(|id| {
                    self.player.inventory.iter().find(|candidate| {
                        candidate.id == id
                            && candidate.kind == ItemKind::Weapon
                            && candidate.which == 2
                    })
                })
            })
            .flatten();
        let (damage, mut hit, mut bonus) = match item.kind {
            ItemKind::Weapon => (
                if item.which == 3 && bow.is_none() {
                    combat::WEAPON_DAMAGE[item.which as usize]
                } else {
                    combat::HURLED_DAMAGE[item.which as usize]
                },
                item.hit_plus as i32,
                item.damage_plus as i32,
            ),
            ItemKind::Stick => ("1x1", 0, 0),
            _ => ("0x0", 0, 0),
        };
        if let Some(bow) = bow {
            hit += bow.hit_plus as i32;
            bonus += bow.damage_plus as i32
        }
        let attack = Attack {
            level: self.player.stats.level,
            strength: self.player.stats.strength,
            hit_bonus: hit,
            damage_bonus: bonus,
        };
        let outcome = combat::resolve_outcome(
            &mut self.rng,
            attack,
            self.monsters[index].armor,
            damage,
            self.monsters[index].awake,
        );
        if !outcome.hit {
            let defender = self.monster_message_name(index, Case::Gen);
            self.message(if item.kind == ItemKind::Weapon {
                format!(
                    "{} ⟨v3:letěti⟩ mimo {defender}",
                    crate::lang::phrase(
                        &crate::lang::WEAPON_LEX[item.which as usize],
                        Case::Nom,
                        interslavic::Number::Singular
                    )
                )
            } else {
                format!("⟨v2:udarjati⟩ mimo {defender}")
            });
            return false;
        }
        self.monsters[index].hp -= outcome.damage;
        if self.player.conditions.can_confuse_monster {
            self.player.conditions.can_confuse_monster = false;
            self.monsters[index].flags |= monster::CONFUSED;
            let color = self.pick_color("črveny");
            self.message(format!(
                "⟨a:tvoj:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩ ⟨v3p:prěstavati⟩ světiti sę {}",
                crate::lang::color_adv(color)
            ));
        }
        if self.monsters[index].hp <= 0 {
            self.kill_monster(index)
        } else {
            let defender = self.monster_message_name(index, Case::Acc);
            self.message(if item.kind == ItemKind::Weapon {
                format!(
                    "{} ⟨v3:udarjati⟩ {defender}",
                    crate::lang::phrase(
                        &crate::lang::WEAPON_LEX[item.which as usize],
                        Case::Nom,
                        interslavic::Number::Singular
                    )
                )
            } else {
                format!("⟨v2:udarjati⟩ {defender}")
            })
        }
        true
    }

    pub fn zap(&mut self, id: u64, direction: Direction) -> CommandResult {
        let Some(index) = self
            .player
            .inventory
            .iter()
            .position(|i| i.id == id && i.kind == ItemKind::Stick)
        else {
            self.message("ne ⟨v2:mogti⟩ čarovati s ⟨toj:ins⟩!");
            return CommandResult::FREE;
        };
        if self.player.inventory[index].charges <= 0 {
            self.message("ničto ne ⟨v3h:stajati:staje⟩ sę");
            return CommandResult::TURN;
        }
        let which = self.player.inventory[index].which;
        let mut identified = matches!(which, 0 | 2 | 3 | 4 | 6);
        if which == 9 && self.player.stats.hp < 2 {
            self.message("ne ⟨v2:imati⟩ dosť ⟨n:sila:gen⟩ za to");
            return CommandResult::TURN;
        }
        let target = self.first_monster(direction, 80);
        if target.is_some_and(|i| self.monsters[i].kind == 5) {
            self.flytrap_holder = None;
        }
        match which {
            0 => {
                let room_id = self
                    .dungeon
                    .map
                    .get(self.player.pos)
                    .and_then(|cell| cell.room);
                if room_id
                    .and_then(|id| self.dungeon.rooms.get(id as usize))
                    .is_none_or(|room| room.gone)
                {
                    self.message("koridor ⟨v3:světiti⟩ sę i potom ⟨v3:gasnųti⟩")
                } else {
                    if let Some(room) =
                        room_id.and_then(|id| self.dungeon.rooms.get_mut(id as usize))
                    {
                        room.dark = false;
                    }
                    self.update_visibility();
                    if self.options.terse {
                        self.message("⟨n:komnata:nom⟩ jest ⟨pp:osvětliti:f⟩")
                    } else {
                        let color = self.pick_color("modry");
                        self.message(format!("⟨n:komnata:nom⟩ jest ⟨pp:osvětliti:f⟩ ⟨ap:migati:světlo:ins⟩ {} ⟨n:světlo:ins⟩", crate::lang::color_ins_n(color)))
                    }
                }
            }
            1 => {
                if let Some(i) = target {
                    self.monsters[i].flags |= monster::INVISIBLE
                }
            }
            2..=4 => self.fire_bolt(direction, which),
            5 => {
                if let Some(i) = target {
                    let visible = self.currently_visible(self.monsters[i].pos);
                    let kind = self.rng.rnd(26) as u8;
                    let old = self.monsters.remove(i);
                    let mut replacement =
                        self.make_monster_with_id(old.id, kind, old.pos, self.depth, false, false);
                    replacement.inventory = old.inventory;
                    self.monsters.insert(0, replacement);
                    identified |= visible && self.can_see_monster(&self.monsters[0]);
                }
            }
            6 => {
                if let Some(i) = target {
                    if self.monster_saves(i, 3) {
                        self.message(if self.options.terse {
                            "⟨n:strěla:nom⟩ ⟨v3:izčezati⟩"
                        } else {
                            "⟨n:strěla:nom⟩ ⟨v3:izčezati⟩ v ⟨n:oblåk:loc⟩ ⟨n:dym:gen⟩"
                        });
                        self.player.inventory[index].charges -= 1;
                        self.knowledge.sticks[which as usize] = true;
                        return CommandResult::TURN;
                    }
                    self.runto_monster(i);
                    let (weapon_hit, weapon_damage) = self
                        .player
                        .weapon
                        .and_then(|id| self.player.inventory.iter().find(|item| item.id == id))
                        .map_or((0, 0), |weapon| {
                            (weapon.hit_plus as i32, weapon.damage_plus as i32)
                        });
                    let attack = Attack {
                        level: self.player.stats.level,
                        strength: self.player.stats.strength,
                        hit_bonus: 100 + weapon_hit,
                        damage_bonus: 1 + weapon_damage,
                    };
                    let outcome = combat::resolve_outcome(
                        &mut self.rng,
                        attack,
                        self.monsters[i].armor,
                        "1x4",
                        self.monsters[i].awake,
                    );
                    self.monsters[i].hp -= outcome.damage;
                    let target_name = self.monster_message_name(i, Case::Acc);
                    self.message(format!("⟨v2:udarjati⟩ {target_name}"));
                    if outcome.hit && self.player.conditions.can_confuse_monster {
                        self.player.conditions.can_confuse_monster = false;
                        self.monsters[i].flags |= monster::CONFUSED;
                        let color = self.pick_color("črveny");
                        self.message(format!(
                            "⟨a:tvoj:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩ ⟨v3p:prěstavati⟩ světiti sę {}",
                            crate::lang::color_adv(color)
                        ));
                        if !self.player.conditions.blind {
                            let confused_name = self.monster_message_name(i, Case::Nom);
                            self.message(format!("{confused_name} ⟨v3:izględati⟩ ⟨pp:smųtiti:n⟩"));
                        }
                    }
                    if self.monsters[i].hp <= 0 {
                        self.kill_monster(i);
                    }
                } else {
                    self.message(if self.options.terse {
                        "⟨n:strěla:nom⟩ ⟨v3:izčezati⟩"
                    } else {
                        "⟨n:strěla:nom⟩ ⟨v3:izčezati⟩ v ⟨n:oblåk:loc⟩ ⟨n:dym:gen⟩"
                    })
                }
            }
            7 => {
                if let Some(i) = target {
                    if self.monsters[i].flags & monster::SLOWED != 0 {
                        self.monsters[i].flags &= !monster::SLOWED
                    } else {
                        self.monsters[i].flags |= monster::HASTED
                    }
                    self.runto_monster(i)
                }
            }
            8 => {
                if let Some(i) = target {
                    if self.monsters[i].flags & monster::HASTED != 0 {
                        self.monsters[i].flags &= !monster::HASTED
                    } else {
                        self.monsters[i].flags |= monster::SLOWED
                    }
                    self.monsters[i].turn = true;
                    self.runto_monster(i)
                }
            }
            9 => self.drain_life(),
            10 => {}
            11 => {
                if let Some(i) = target {
                    while let Some(pos) = self.reference_monster_floor_position() {
                        if pos != self.player.pos {
                            self.monsters[i].pos = pos;
                            self.monsters[i].awake = true;
                            self.monsters[i].destination = None;
                            self.monsters[i].destination_is_room_gold = false;
                            break;
                        }
                    }
                }
            }
            12 => {
                if let Some(i) = target {
                    let (dx, dy) = direction.delta();
                    let near = self.player.pos.offset(dx, dy);
                    if self.passable(near)
                        && self
                            .monsters
                            .iter()
                            .enumerate()
                            .all(|(j, m)| j == i || m.pos != near)
                    {
                        self.monsters[i].pos = near;
                        self.monsters[i].awake = true;
                        self.monsters[i].destination = None;
                        self.monsters[i].destination_is_room_gold = false;
                    }
                }
            }
            13 => {
                if let Some(i) = target {
                    self.monsters[i].flags |= monster::CANCELLED;
                    self.monsters[i].flags &= !monster::INVISIBLE;
                    self.monsters[i].disguise = (b'A' + self.monsters[i].kind) as char
                }
            }
            _ => unreachable!(),
        }
        self.player.inventory[index].charges -= 1;
        self.knowledge.sticks[which as usize] |= identified;
        CommandResult::TURN
    }

    fn first_monster(&self, direction: Direction, limit: usize) -> Option<usize> {
        let (dx, dy) = direction.delta();
        let mut p = self.player.pos;
        for _ in 0..limit {
            p = p.offset(dx, dy);
            if let Some(i) = self.monsters.iter().position(|m| m.pos == p)
                && (self.can_see_monster(&self.monsters[i])
                    || self.player.conditions.detect_monsters)
            {
                return Some(i);
            }
            if !self.passable(p) {
                break;
            }
        }
        None
    }

    fn reference_monster_floor_position(&mut self) -> Option<Pos> {
        self.reference_floor_position(true)
    }
    fn monster_saves(&mut self, index: usize, which: i32) -> bool {
        let need = 14 + which - self.monsters[index].level / 2;
        self.rng.roll(1, 20) >= need
    }

    fn runto_monster(&mut self, index: usize) {
        self.monsters[index].awake = true;
        self.monsters[index].flags &= !monster::HELD;
        let destination = self.find_monster_item_destination(index);
        self.monsters[index].destination = destination;
        self.monsters[index].destination_is_room_gold = false;
    }

    fn fire_bolt(&mut self, direction: Direction, which: u8) {
        self.fire_bolt_from(self.player.pos, direction, which, None);
    }

    fn fire_bolt_from(
        &mut self,
        start: Pos,
        direction: Direction,
        which: u8,
        source_kind: Option<u8>,
    ) {
        let name = match which {
            2 => "mȯlnja",
            3 => "plåmenj",
            4 => "led",
            _ => unreachable!(),
        };
        let (mut dx, mut dy) = direction.delta();
        let mut pos = start;
        let mut hit_player = source_kind.is_some();
        let mut changed = false;
        let mut travelled = 0;
        let mut attempts = 0;
        while travelled < 6 && attempts < 64 {
            attempts += 1;
            pos = pos.offset(dx, dy);
            let terrain = self.dungeon.map.get(pos).map(|cell| cell.terrain);
            let boundary = terrain.is_none_or(|terrain| !terrain.passable())
                || (terrain == Some(Terrain::Door) && pos != self.player.pos);
            if boundary {
                if !changed {
                    hit_player = !hit_player;
                }
                changed = false;
                dx = -dx;
                dy = -dy;
                self.message(format!("{name} ⟨v3:odskočiti⟩"));
                continue;
            }
            travelled += 1;
            if !hit_player
                && let Some(index) = self.monsters.iter().position(|monster| monster.pos == pos)
            {
                hit_player = true;
                changed = !changed;
                if !self.monster_saves(index, 3) {
                    if self.monsters[index].kind == 3 && which == 3 {
                        self.message(if self.options.terse {
                            "⟨n:plåmenj:nom⟩ ⟨v3:odskočiti⟩"
                        } else {
                            "⟨n:plåmenj:nom⟩ ⟨v3:odskočiti⟩ od ⟨n:drakon:gen⟩"
                        });
                    } else {
                        self.runto_monster(index);
                        let target = self.monster_message_name(index, Case::Acc);
                        let outcome = combat::resolve_outcome(
                            &mut self.rng,
                            Attack {
                                level: self.player.stats.level,
                                strength: self.player.stats.strength,
                                hit_bonus: 100,
                                damage_bonus: 0,
                            },
                            self.monsters[index].armor,
                            "6x6",
                            self.monsters[index].awake,
                        );
                        debug_assert!(outcome.hit);
                        self.monsters[index].hp -= outcome.damage;
                        self.message(format!("{name} ⟨v3:udarjati⟩ {target}"));
                        if self.monsters[index].hp <= 0 {
                            self.kill_monster(index);
                        }
                    }
                    break;
                }
                if source_kind.is_none() {
                    self.runto_monster(index);
                }
                let target = self.monster_message_name(index, Case::Gen);
                self.message(if self.options.terse {
                    format!("{name} ⟨v3:hybiti⟩")
                } else {
                    format!("{name} ⟨v3:letěti⟩ mimo {target}")
                });
            } else if hit_player && pos == self.player.pos {
                hit_player = false;
                changed = !changed;
                if !self.player_saves(3) {
                    let damage = self.rng.roll(6, 6);
                    self.damage_player(damage);
                    if self.player.stats.hp <= 0 {
                        if let Some(kind) = source_kind {
                            self.die(Self::monster_killer(kind));
                        } else {
                            self.die("⟨a:čarovny:strěla:gen⟩ ⟨n:strěla:gen⟩");
                        }
                        break;
                    }
                    self.message(format!("{name} ⟨v3:udarjati⟩ ⟨ty:acc⟩"));
                    break;
                }
                self.message(format!("{name} ⟨v3:letěti⟩ mimo ⟨ty:gen:f⟩"));
            }
        }
    }
    fn drain_life(&mut self) {
        debug_assert!(self.player.stats.hp >= 2);
        let player_cell = self
            .dungeon
            .map
            .get(self.player.pos)
            .copied()
            .unwrap_or_default();
        let targets: Vec<usize> = self
            .monsters
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let monster_cell = self.dungeon.map.get(m.pos).copied().unwrap_or_default();
                let in_scope = if player_cell.terrain == Terrain::Door {
                    monster_cell.room == player_cell.room
                        || (player_cell.passage.is_some()
                            && monster_cell.passage == player_cell.passage)
                } else if let Some(room) = player_cell.room {
                    monster_cell.room == Some(room)
                } else {
                    player_cell.passage.is_some() && monster_cell.passage == player_cell.passage
                };
                in_scope.then_some(i)
            })
            .collect();
        if targets.is_empty() {
            self.message("⟨n:koža:nom⟩ ⟨ty:acc⟩ ⟨v3:mråviti⟩");
            return;
        }
        let damage = self.player.stats.hp - self.player.stats.hp / 2;
        self.damage_player(damage);
        let damage = self.player.stats.hp / targets.len() as i32;
        for &i in targets.iter().rev() {
            self.monsters[i].hp -= damage;
            if self.monsters[i].hp <= 0 {
                self.kill_monster(i);
            } else {
                self.runto_monster(i);
            }
        }
    }

    fn consume_one(&mut self, id: u64) {
        if let Some(index) = self.player.inventory.iter().position(|i| i.id == id) {
            if self.player.inventory[index].count > 1 {
                self.player.inventory[index].count -= 1
            } else {
                self.player.inventory.remove(index);
                if self.player.weapon == Some(id) {
                    self.player.weapon = None
                }
            }
        }
    }
    fn move_player(&mut self, d: Direction) -> CommandResult {
        self.move_player_inner(d, true).0
    }
    pub fn move_without_pickup(&mut self, d: Direction) -> CommandResult {
        self.move_player_inner(d, false).0
    }
    pub fn fight_direction(&mut self, d: Direction, kamikaze: bool) -> CommandResult {
        let (dx, dy) = d.delta();
        let p = self.player.pos.offset(dx, dy);
        let Some(index) = self.monsters.iter().position(|m| m.pos == p) else {
            self.message(if self.options.terse {
                "tam ne jest ⟨n:čudovišče:gen⟩"
            } else {
                "ne ⟨v2:viděti⟩ tam ⟨n:čudovišče:acc⟩"
            });
            return CommandResult::FREE;
        };
        if dx != 0
            && dy != 0
            && (!self.passable(self.player.pos.offset(dx, 0))
                || !self.passable(self.player.pos.offset(0, dy)))
        {
            return CommandResult::FREE;
        }
        if !self.can_see_monster(&self.monsters[index]) && !self.player.conditions.detect_monsters {
            self.message(if self.options.terse {
                "tam ne jest ⟨n:čudovišče:gen⟩"
            } else {
                "ne ⟨v2:viděti⟩ tam ⟨n:čudovišče:acc⟩"
            });
            return CommandResult::FREE;
        }
        let target = self.monsters[index].id;
        self.fight_target = Some(target);
        self.fight_kamikaze = kamikaze;
        self.fight_safety_max_hit = 0;
        while let Some(index) = self
            .monsters
            .iter()
            .position(|monster| monster.id == target)
        {
            self.player_attack_inner(index, true);
            self.advance_player_turn();
            if self.end != EndState::Playing {
                break;
            }
            if self.fight_target != Some(target) {
                break;
            }
            self.begin_command();
        }
        self.fight_target = None;
        self.fight_kamikaze = false;
        self.fight_safety_max_hit = 0;
        CommandResult::FREE
    }
    pub fn identify_trap(&mut self, d: Direction) -> CommandResult {
        let (dx, dy) = d.delta();
        let p = self.player.pos.offset(dx, dy);
        let Some(cell) = self.dungeon.map.get_mut(p) else {
            self.message(if self.options.terse {
                "tam ne jest ⟨n:pasť:gen⟩"
            } else {
                "ne ⟨v2:nahoditi⟩ tam ⟨n:pasť:acc⟩"
            });
            return CommandResult::FREE;
        };
        if cell.trap_revealed
            && let Some(trap) = cell.trap
        {
            let shown = if self.player.conditions.hallucinating {
                crate::lang::phrase(
                    &crate::lang::TRAP_LEX[self.rng.rnd(8) as usize],
                    Case::Nom,
                    interslavic::Number::Singular,
                )
            } else {
                trap_name(trap)
            };
            self.message(if self.options.terse {
                shown.to_string()
            } else {
                format!("⟨v2:nahoditi:U⟩: {shown}")
            })
        } else {
            self.message(if self.options.terse {
                "tam ne jest ⟨n:pasť:gen⟩"
            } else {
                "ne ⟨v2:nahoditi⟩ tam ⟨n:pasť:acc⟩"
            })
        }
        CommandResult::FREE
    }
    /// Return the command result, whether movement reached `hit_bound`, and
    /// whether the original cleared `running` on this path.
    fn move_player_inner(&mut self, d: Direction, pickup: bool) -> (CommandResult, bool, bool) {
        if self.player.conditions.held_turns > 0 {
            self.player.conditions.held_turns -= 1;
            self.message("ješče ne ⟨v2:mogti⟩ izlězti iz ⟨a:medvěďji:pasť:gen⟩ ⟨n:pasť:gen⟩");
            return (CommandResult::TURN, false, false);
        }
        let confused_move = self.player.conditions.confused && self.rng.rnd(5) != 0;
        let (dx, dy) = if confused_move {
            let dy = self.rng.rnd(3) as i32 - 1;
            let dx = self.rng.rnd(3) as i32 - 1;
            (dx, dy)
        } else {
            d.delta()
        };
        if dx == 0 && dy == 0 {
            return (CommandResult::FREE, false, true);
        }
        let to = self.player.pos.offset(dx, dy);
        let destination_has_item = self.floor_items.iter().any(|item| item.pos == Some(to));
        if confused_move
            && (!self.passable(to)
                || self.monsters.iter().any(|monster| monster.pos == to)
                || self.floor_items.iter().any(|item| {
                    item.pos == Some(to) && item.kind == ItemKind::Scroll && item.which == 10
                }))
        {
            return (CommandResult::FREE, false, true);
        }
        let enters_concealed_trap = self.dungeon.map.get(to).is_some_and(|cell| {
            cell.terrain == Terrain::Floor && cell.trap.is_some() && !cell.trap_revealed
        }) && !destination_has_item;
        if let Some(holder) = self.flytrap_holder
            && !enters_concealed_trap
            && self
                .monsters
                .iter()
                .find(|monster| monster.pos == to)
                .is_none_or(|monster| monster.id != holder)
        {
            self.message("něčto ⟨ty:acc⟩ ⟨v3:dŕžati⟩");
            return (CommandResult::TURN, false, false);
        }
        if dx != 0 && dy != 0 {
            let a = self.player.pos.offset(dx, 0);
            let b = self.player.pos.offset(0, dy);
            if !self.passable(a) || !self.passable(b) {
                return (CommandResult::FREE, false, true);
            }
        }
        if let Some(index) = self.monsters.iter().position(|m| m.pos == to) {
            self.player_attack(index);
            return (CommandResult::TURN, false, true);
        }
        if self.passable(to) {
            let triggers_trap = !destination_has_item
                && !self.player.conditions.levitating
                && self
                    .dungeon
                    .map
                    .get(to)
                    .is_some_and(|cell| cell.trap.is_some());
            let stops_running =
                destination_has_item
                    || triggers_trap
                    || self.dungeon.map.get(to).is_some_and(|cell| {
                        matches!(cell.terrain, Terrain::Door | Terrain::Stairs)
                    });
            let entering_room = self
                .dungeon
                .map
                .get(self.player.pos)
                .is_some_and(|cell| cell.passage.is_some())
                && self
                    .dungeon
                    .map
                    .get(to)
                    .is_some_and(|cell| cell.terrain == Terrain::Door);
            if entering_room && let Some(room) = self.dungeon.map.get(to).and_then(|cell| cell.room)
            {
                self.wake_room_monsters(room);
            }
            self.player.pos = to;
            if to == self.dungeon.stairs {
                self.seen_stairs = true;
            }
            if !destination_has_item {
                self.trigger_trap();
            }
            self.update_visibility();
            if pickup {
                self.auto_pickup();
            } else if !self.player.conditions.levitating
                && let Some(item) = self.floor_items.iter().find(|item| item.pos == Some(to))
            {
                self.message(if self.options.terse {
                    format!("tu: {}", self.inventory_name(item, true))
                } else {
                    format!(
                        "⟨v2:stųpati⟩ na {}",
                        self.inventory_name_case(item, true, Case::Acc)
                    )
                });
            }
            return (CommandResult::TURN, false, stops_running);
        }
        (CommandResult::FREE, true, true)
    }

    fn run_player(&mut self, direction: Direction, stop_at_intersections: bool) -> CommandResult {
        let mut direction = direction;
        let cautious = stop_at_intersections && !self.player.conditions.blind;
        for _ in 0..255 {
            let (dx, dy) = direction.delta();
            let before = self.player.pos;
            let (step, hit_boundary, stops_running) = self.move_player_inner(direction, true);
            if self.player.pos == before {
                let current = self.dungeon.map.get(before).copied().unwrap_or_default();
                if hit_boundary
                    && self.options.passgo
                    && current.terrain == Terrain::Passage
                    && !self.player.conditions.blind
                    && let Some(turn) = self.single_passage_turn(direction)
                {
                    direction = turn;
                    continue;
                }
                if step.consumed_turn && !stops_running {
                    self.advance_player_turn_while_running();
                    if self.end != EndState::Playing {
                        return CommandResult::FREE;
                    }
                    self.begin_command();
                    continue;
                }
                return step;
            }
            if self.end != EndState::Playing {
                return step;
            }
            let cell = self
                .dungeon
                .map
                .get(self.player.pos)
                .copied()
                .unwrap_or_default();
            let forward = self.player.pos.offset(dx, dy);
            if stops_running {
                return CommandResult::TURN;
            }
            self.advance_player_turn_while_running();
            if self.end != EndState::Playing {
                return CommandResult::FREE;
            }
            self.begin_command();
            if cautious && self.cautious_run_should_stop(direction) {
                return CommandResult::FREE;
            }
            if self.options.passgo
                && cell.terrain == Terrain::Passage
                && !self.passable(forward)
                && !self.player.conditions.blind
                && let Some(turn) = self.single_passage_turn(direction)
            {
                direction = turn;
                continue;
            }
            if !self.passable(forward) {
                return CommandResult::FREE;
            }
        }
        CommandResult::FREE
    }

    /// Mirror the `door_stop` portion of Rogue's `look`: inspect only the
    /// forward-facing neighborhood, stop before interesting glyphs and
    /// orthogonal doors, and count orthogonal passage exits.
    fn cautious_run_should_stop(&self, direction: Direction) -> bool {
        fn step_ok(glyph: char) -> bool {
            !matches!(glyph, ' ' | '|' | '-') && !glyph.is_ascii_alphabetic()
        }

        fn in_forward_sector(direction: Direction, dx: i32, dy: i32) -> bool {
            match direction {
                Direction::Left => dx != 1,
                Direction::Down => dy != -1,
                Direction::Up => dy != 1,
                Direction::Right => dx != -1,
                Direction::UpLeft => dx + dy < 1,
                Direction::UpRight => dy - dx < 1,
                Direction::DownRight => dx + dy > -1,
                Direction::DownLeft => dy - dx > -1,
            }
        }

        let map_glyph = |pos: Pos| {
            if let Some(item) = self.floor_items.iter().find(|item| item.pos == Some(pos)) {
                return item.kind.glyph();
            }
            self.dungeon.map.get(pos).map_or(' ', |cell| {
                if cell.trap_revealed {
                    '^'
                } else {
                    cell.terrain.glyph()
                }
            })
        };

        let player = self.player.pos;
        let Some(player_cell) = self.dungeon.map.get(player) else {
            return true;
        };
        let player_glyph = map_glyph(player);
        let mut passages = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let pos = player.offset(dx, dy);
                let Some(cell) = self.dungeon.map.get(pos) else {
                    continue;
                };
                let mut glyph = map_glyph(pos);
                if glyph == ' ' {
                    continue;
                }
                if player_glyph != '+'
                    && glyph != '+'
                    && player_cell.passage.is_some() != cell.passage.is_some()
                {
                    continue;
                }
                if dx != 0
                    && dy != 0
                    && !step_ok(map_glyph(player.offset(dx, 0)))
                    && !step_ok(map_glyph(player.offset(0, dy)))
                {
                    continue;
                }

                if let Some(monster) = self.monsters.iter().find(|monster| monster.pos == pos) {
                    if self.player.conditions.detect_monsters
                        && monster.flags & monster::INVISIBLE != 0
                    {
                        return true;
                    }
                    if self.can_see_monster(monster) {
                        glyph = monster.disguise;
                    }
                }
                if !in_forward_sector(direction, dx, dy) {
                    continue;
                }
                match glyph {
                    '+' => {
                        if dx == 0 || dy == 0 {
                            return true;
                        }
                    }
                    '#' => {
                        if dx == 0 || dy == 0 {
                            passages += 1;
                        }
                    }
                    '.' | '|' | '-' | ' ' => {}
                    _ => return true,
                }
            }
        }
        passages > 1
    }

    fn single_passage_turn(&self, direction: Direction) -> Option<Direction> {
        let candidates: &[Direction] = match direction {
            Direction::Left | Direction::Right => &[Direction::Up, Direction::Down],
            Direction::Up | Direction::Down => &[Direction::Left, Direction::Right],
            _ => return None,
        };
        let turns: Vec<Direction> = candidates
            .iter()
            .copied()
            .filter(|candidate| {
                let (dx, dy) = candidate.delta();
                let pos = self.player.pos.offset(dx, dy);
                self.dungeon.map.get(pos).is_some_and(|cell| {
                    cell.terrain.passable()
                        && matches!(cell.terrain, Terrain::Passage | Terrain::Door)
                })
            })
            .collect();
        if turns.len() == 1 {
            Some(turns[0])
        } else {
            None
        }
    }

    fn trigger_trap(&mut self) {
        if self.player.conditions.levitating {
            return;
        }
        let trap = self.dungeon.map.get(self.player.pos).and_then(|c| c.trap);
        let Some(trap) = trap else { return };
        if let Some(c) = self.dungeon.map.get_mut(self.player.pos) {
            c.trap_revealed = true;
        }
        match trap {
            Trap::TrapDoor => {
                self.depth += 1;
                self.max_depth = self.max_depth.max(self.depth);
                self.new_level();
                self.message("⟨v2:padati⟩ v ⟨n:pasť:acc⟩!")
            }
            Trap::Arrow => {
                if combat::swing(
                    &mut self.rng,
                    self.player.stats.level - 1,
                    self.player.stats.armor,
                    1,
                ) {
                    let damage = self.rng.roll(1, 6);
                    self.damage_player(damage);
                    if self.player.stats.hp <= 0 {
                        self.die("⟨n:strěla:gen⟩");
                        self.message("⟨n:strěla:nom⟩ ⟨ty:acc⟩ ⟨lp:ubiti:f⟩");
                        return;
                    }
                    self.message("o ne! ⟨n:strěla:nom:U⟩ ⟨ty:acc⟩ ⟨lp:raniti:f⟩")
                } else {
                    // `init_weapon(ARROW)` randomizes the normal missile stack
                    // before `be_trapped` overwrites its count with one.
                    let _ = self.rng.rnd(8);
                    let mut count = 0;
                    let mut destination = None;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let pos = self.player.pos.offset(dx, dy);
                            if pos != self.player.pos
                                && self.dungeon.map.get(pos).is_some_and(|cell| {
                                    matches!(cell.terrain, Terrain::Floor | Terrain::Passage)
                                        && !cell.trap_revealed
                                })
                                && self.floor_items.iter().all(|item| item.pos != Some(pos))
                            {
                                count += 1;
                                if self.rng.rnd(count) == 0 {
                                    destination = Some(pos);
                                }
                            }
                        }
                    }
                    if let Some(destination) = destination {
                        let mut arrow = Item::basic(self.id(), ItemKind::Weapon, 3);
                        arrow.count = 1;
                        arrow.pos = Some(destination);
                        self.floor_items.push(arrow);
                    }
                    self.message("⟨n:strěla:nom⟩ ⟨v3:letěti⟩ mimo ⟨ty:gen:f⟩")
                }
            }
            Trap::SleepGas => {
                self.player.conditions.asleep_turns += self.rng.spread(5) as u32;
                self.player_is_running = false;
                self.message("⟨a:divny:mgla:nom⟩ ⟨a:běly:mgla:nom⟩ ⟨n:mgla:nom⟩ ⟨v3:okrųžati⟩ ⟨ty:acc⟩ i ⟨v2:usnųti⟩")
            }
            Trap::Bear => {
                self.player.conditions.held_turns += self.rng.spread(3) as u32;
                self.message("⟨a:medvěďji:pasť:nom⟩ ⟨n:pasť:nom⟩ ⟨ty:acc⟩ ⟨v3:loviti⟩")
            }
            Trap::Teleport => {
                self.teleport_player();
            }
            Trap::PoisonDart => {
                if combat::swing(
                    &mut self.rng,
                    self.player.stats.level + 1,
                    self.player.stats.armor,
                    1,
                ) {
                    let damage = self.rng.roll(1, 4);
                    self.damage_player(damage);
                    if self.player.stats.hp <= 0 {
                        self.die("⟨n:drotik:gen⟩");
                        self.message("⟨a:jadny:drotik:nom⟩ ⟨n:drotik:nom⟩ ⟨ty:acc⟩ ⟨lp:ubiti:m⟩");
                        return;
                    }
                    if !self.wears_ring(2) && !self.player_saves(0) {
                        self.player.stats.strength = (self.player.stats.strength - 1).max(3)
                    }
                    self.message(
                        "⟨a:maly:drotik:nom⟩ ⟨n:drotik:nom⟩ ⟨v3:udarjati⟩ ⟨ty:acc⟩ v ⟨n:ramę:acc⟩",
                    )
                } else {
                    self.message("⟨a:maly:drotik:nom⟩ ⟨n:drotik:nom⟩ ⟨v3:letěti⟩ mimo ⟨a:tvoj:uho:gen⟩ ⟨n:uho:gen⟩ i ⟨v3:izčezati⟩")
                }
            }
            Trap::Rust => {
                self.message("⟨n:struja:nom⟩ ⟨n:voda:gen⟩ ⟨v3:udarjati⟩ ⟨ty:acc⟩ v ⟨n:glåva:acc⟩");
                self.rust_armor()
            }
            Trap::Mysterious => {
                use crate::lang::{self, adj_for, lex};
                use interslavic::{Animacy, Gender, Number};
                let random_color =
                    |g: &mut GameRng| lang::COLOR_ADJ[g.rnd(lang::COLOR_ADJ.len() as u32) as usize];
                let message = match self.rng.rnd(11) {
                    0 => "naglo ⟨v2:byti⟩ v ⟨a:paraleľny:svět:loc⟩ ⟨n:svět:loc⟩".into(),
                    1 => {
                        let color = random_color(&mut self.rng);
                        let svetlo = lex("světlo", Gender::Neuter, Animacy::Inanimate);
                        format!(
                            "⟨n:světlo:nom⟩ tu naglo ⟨v3:izględati⟩ {}",
                            adj_for(color, &svetlo, Case::Nom, Number::Singular)
                        )
                    }
                    2 => "⟨v2:čuti⟩ ⟨n:ubod:acc⟩ v ⟨n:šija:loc⟩".into(),
                    3 => "⟨a:pestry:linija:nom:pl⟩ ⟨n:linija:nom:pl⟩ ⟨v3p:tancevati⟩ okolo ⟨ty:gen:f⟩ i ⟨v3p:izčezati⟩".into(),
                    4 => {
                        let color = random_color(&mut self.rng);
                        let svetlo = lex("světlo", Gender::Neuter, Animacy::Inanimate);
                        format!(
                            "{} ⟨n:světlo:nom⟩ ⟨v3:světiti⟩ v ⟨a:tvoj:oko:acc:pl⟩ ⟨n:oko:acc:pl⟩",
                            adj_for(color, &svetlo, Case::Nom, Number::Singular)
                        )
                    }
                    5 => "⟨n:strěla:nom⟩ ⟨v3:letěti⟩ mimo ⟨a:tvoj:uho:gen⟩ ⟨n:uho:gen⟩!".into(),
                    6 => {
                        let color = random_color(&mut self.rng);
                        let iskra = lex("iskra", Gender::Feminine, Animacy::Inanimate);
                        format!(
                            "{} ⟨n:iskra:nom:pl⟩ ⟨v3p:tancevati⟩ po ⟨a:tvoj:brȯnja:loc⟩ ⟨n:brȯnja:loc⟩",
                            adj_for(color, &iskra, Case::Nom, Number::Plural)
                        )
                    }
                    7 => "naglo ⟨v2:čuti⟩ ⟨a:veliky:žęđa:acc⟩ ⟨n:žęđa:acc⟩".into(),
                    8 => "čas naglo ⟨v3:běgti⟩ ⟨cav:bystry⟩".into(),
                    9 => "čas sejčas ⟨v3:běgti⟩ pomalo".into(),
                    _ => {
                        let color = random_color(&mut self.rng);
                        let torba = lex("torba", Gender::Feminine, Animacy::Inanimate);
                        format!(
                            "⟨a:tvoj:torba:nom⟩ ⟨n:torba:nom⟩ ⟨v3h:stajati:staje⟩ sę {}!",
                            adj_for(color, &torba, Case::Nom, Number::Singular)
                        )
                    }
                };
                self.message(message)
            }
        }
        if self.player.stats.hp <= 0 {
            self.die("⟨n:pasť:gen⟩");
        }
    }
    fn passable(&self, p: Pos) -> bool {
        self.dungeon
            .map
            .get(p)
            .is_some_and(|c| c.terrain.passable())
    }
    /// Rogue's `roomin` treats a passage number as the location identity even
    /// on a doorway cell that also belongs to a room.
    fn area_key(&self, p: Pos) -> Option<(bool, u8)> {
        let cell = self.dungeon.map.get(p)?;
        cell.passage
            .map(|passage| (true, passage))
            .or_else(|| cell.room.map(|room| (false, room)))
    }
    fn auto_pickup(&mut self) {
        if self.player.conditions.levitating {
            return;
        }
        if self
            .floor_items
            .iter()
            .any(|i| i.pos == Some(self.player.pos))
        {
            let _ = self.pickup();
        }
    }
    fn pickup(&mut self) -> CommandResult {
        let Some(index) = self
            .floor_items
            .iter()
            .position(|i| i.pos == Some(self.player.pos))
        else {
            self.message(if self.options.terse {
                "tu ⟨ničto:gen⟩ ne jest"
            } else {
                "tu ne jest ⟨ničto:gen⟩, čto ⟨v2:mogti⟩ vzęti"
            });
            return CommandResult::TURN;
        };
        if self.player.conditions.levitating {
            self.message("ne ⟨v2:mogti⟩.  ⟨v2:letěti:U⟩ nad ⟨n:zemja:ins⟩!");
            return CommandResult::TURN;
        }
        let mut item = self.floor_items.remove(index);
        if item.kind == ItemKind::Scroll && item.which == 10 && item.dropped_once {
            self.redirect_monsters_from(self.player.pos);
            self.message("svitȯk ⟨v3:råzpadati⟩ sę v pråh, kȯgda ⟨v2:brati⟩ ⟨on:acc:f⟩");
            return CommandResult::TURN;
        }
        if item.kind == ItemKind::Gold {
            self.redirect_monsters_from(self.player.pos);
            if let Some(room) = self
                .dungeon
                .map
                .get(self.player.pos)
                .and_then(|cell| cell.room)
            {
                self.dungeon.rooms[room as usize].gold_value = 0;
            }
            self.player.gold += item.gold_value;
            self.message(if self.options.terse {
                format!(
                    "{} {}",
                    item.gold_value,
                    interslavic::quantified(
                        item.gold_value as u64,
                        crate::lang::GOLD_COIN.lemma,
                        Case::Nom,
                        crate::lang::GOLD_COIN.gender,
                        crate::lang::GOLD_COIN.animacy,
                    )
                )
            } else {
                format!(
                    "⟨v2:nahoditi⟩ {} {}",
                    item.gold_value,
                    interslavic::quantified(
                        item.gold_value as u64,
                        crate::lang::GOLD_COIN.lemma,
                        Case::Acc,
                        crate::lang::GOLD_COIN.gender,
                        crate::lang::GOLD_COIN.animacy,
                    )
                )
            });
            return CommandResult::TURN;
        }
        let grouped_weapon_merge = item.kind == ItemKind::Weapon
            && self
                .player
                .inventory
                .iter()
                .any(|existing| existing.stacks_with(&item));
        let item_weight = match item.kind {
            ItemKind::Potion | ItemKind::Scroll | ItemKind::Food => item.count as usize,
            ItemKind::Weapon if grouped_weapon_merge => 0,
            _ => 1,
        };
        if self.pack_count().saturating_add(item_weight) > MAX_PACK {
            let name = self.inventory_name(&item, true);
            self.message(if self.options.terse {
                "ne jest ⟨n:město:gen⟩"
            } else {
                "v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩ ne jest ⟨n:město:gen⟩"
            });
            self.message(if self.options.terse {
                format!("tu: {name}")
            } else {
                format!(
                    "⟨v2:stųpati⟩ na {}",
                    self.inventory_name_case(&item, true, Case::Acc)
                )
            });
            self.floor_items.push(item);
            return CommandResult::TURN;
        }
        self.redirect_monsters_from(self.player.pos);
        if item.kind == ItemKind::Amulet {
            self.has_amulet = true
        }
        if let Some(existing_index) = self
            .player
            .inventory
            .iter()
            .position(|existing| existing.stacks_with(&item))
        {
            self.player.inventory[existing_index].count += item.count;
            let existing = &self.player.inventory[existing_index];
            let name = self.inventory_name_case(existing, !self.options.terse, Case::Acc);
            let letter = existing.pack_letter.unwrap_or('?');
            self.message(if self.options.terse {
                format!("{name} ({letter})")
            } else {
                format!("⟨v2:podbirati⟩ {name} ({letter})")
            });
            return CommandResult::TURN;
        }
        item.pos = None;
        item.pack_letter = self.next_pack_letter();
        let insert_at = self
            .player
            .inventory
            .iter()
            .rposition(|existing| existing.kind == item.kind)
            .map_or(self.player.inventory.len(), |index| index + 1);
        let name = self.inventory_name_case(&item, !self.options.terse, Case::Acc);
        let letter = item.pack_letter.unwrap_or('?');
        self.player.inventory.insert(insert_at, item);
        self.message(if self.options.terse {
            format!("{name} ({letter})")
        } else {
            format!("⟨v2:podbirati⟩ {name} ({letter})")
        });
        CommandResult::TURN
    }

    fn redirect_monsters_from(&mut self, pos: Pos) {
        for monster in &mut self.monsters {
            if monster.destination == Some(pos) && !monster.destination_is_room_gold {
                monster.destination = None;
                monster.destination_is_room_gold = false;
            }
        }
    }
    pub fn pack_count(&self) -> usize {
        self.player
            .inventory
            .iter()
            .filter(|item| item.in_pack)
            .map(|item| match item.kind {
                ItemKind::Potion | ItemKind::Scroll | ItemKind::Food => item.count as usize,
                _ => 1,
            })
            .sum()
    }
    pub fn inventory_index_for_letter(&self, letter: char) -> Option<usize> {
        self.player
            .inventory
            .iter()
            .position(|item| item.in_pack && item.pack_letter == Some(letter))
            .or_else(|| {
                let index = (letter as u8).checked_sub(b'a')? as usize;
                self.player
                    .inventory
                    .get(index)
                    .filter(|item| item.in_pack && item.pack_letter.is_none())
                    .map(|_| index)
            })
    }
    fn next_pack_letter(&self) -> Option<char> {
        ('a'..='z').find(|letter| {
            self.player
                .inventory
                .iter()
                .filter(|item| item.in_pack)
                .all(|item| item.pack_letter != Some(*letter))
        })
    }
    fn search(&mut self) {
        let penalty = i32::from(self.player.conditions.hallucinating) * 3
            + i32::from(self.player.conditions.blind) * 2;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let p = self.player.pos.offset(dx, dy);
                let Some(cell) = self.dungeon.map.get(p).copied() else {
                    continue;
                };
                if matches!(
                    cell.terrain,
                    Terrain::SecretDoor
                        | Terrain::SecretDoorHorizontal
                        | Terrain::SecretDoorVertical
                ) {
                    if self.rng.rnd((5 + penalty) as u32) == 0 {
                        let c = self.dungeon.map.get_mut(p).unwrap();
                        c.terrain = Terrain::Door;
                        self.message("⟨a:tajny:dvėri:nom:pl⟩ ⟨n:dvėri:nom:pl⟩");
                    }
                } else if cell.terrain == Terrain::SecretPassage {
                    if self.rng.rnd((3 + penalty) as u32) == 0 {
                        let c = self.dungeon.map.get_mut(p).unwrap();
                        c.terrain = Terrain::Passage;
                        self.message("tajny prohod");
                    }
                } else if cell.trap.is_some()
                    && !cell.trap_revealed
                    && self.rng.rnd((2 + penalty) as u32) == 0
                    && let Some(c) = self.dungeon.map.get_mut(p)
                {
                    c.trap_revealed = true;
                    let trap_lex = if self.player.conditions.hallucinating {
                        &crate::lang::TRAP_LEX[self.rng.rnd(8) as usize]
                    } else {
                        &crate::lang::TRAP_LEX[trap_index(cell.trap.unwrap())]
                    };
                    self.message(if self.options.terse {
                        crate::lang::phrase(trap_lex, Case::Nom, interslavic::Number::Singular)
                    } else {
                        format!(
                            "⟨v2:nahoditi⟩ {}",
                            crate::lang::phrase(trap_lex, Case::Acc, interslavic::Number::Singular)
                        )
                    });
                }
            }
        }
    }
    fn descend(&mut self) -> CommandResult {
        if self.player.conditions.levitating {
            self.message("ne ⟨v2:mogti⟩.  ⟨v2:letěti:U⟩ nad ⟨n:zemja:ins⟩!");
            return CommandResult::FREE;
        }
        if self.player.pos != self.dungeon.stairs {
            self.message("ne ⟨v2:viděti⟩ ⟨n:pųť:acc⟩ dolu");
            return CommandResult::FREE;
        }
        self.depth += 1;
        self.max_depth = self.max_depth.max(self.depth);
        self.new_level();
        CommandResult::FREE
    }
    fn ascend(&mut self) -> CommandResult {
        if self.player.conditions.levitating {
            self.message("ne ⟨v2:mogti⟩.  ⟨v2:letěti:U⟩ nad ⟨n:zemja:ins⟩!");
            return CommandResult::FREE;
        }
        if self.player.pos != self.dungeon.stairs {
            self.message("ne ⟨v2:viděti⟩ ⟨n:pųť:acc⟩ gorě");
            return CommandResult::FREE;
        }
        if !self.has_amulet {
            self.message("⟨a:tvoj:pųť:nom⟩ ⟨n:pųť:nom⟩ jest ⟨adv:čarovny⟩ ⟨pp:zablokovati:m⟩");
            return CommandResult::FREE;
        }
        if self.depth == 1 {
            self.depth = 0;
            self.end = EndState::Won;
            return CommandResult::FREE;
        }
        self.depth -= 1;
        self.new_level();
        self.message("⟨v2:čuti⟩ ⟨a:siľny:bolj:acc⟩ ⟨n:bolj:acc⟩ v ⟨n:želųdȯk:loc⟩");
        CommandResult::FREE
    }
    fn new_level(&mut self) {
        self.cancel_travel();
        self.player.conditions.held_turns = 0;
        self.flytrap_holder = None;
        self.flytrap_hits = 0;
        self.max_depth = self.max_depth.max(self.depth);
        self.seen_stairs = false;
        self.hallucinated_items.clear();
        self.hallucinated_monsters.clear();
        self.hallucinated_stairs = None;
        self.monsters.clear();
        self.floor_items.clear();
        self.dungeon = begin_layout(&mut self.rng);
        self.build_current_level();
        if self.player.conditions.detect_monsters {
            self.set_monster_detection(true);
        }
    }

    fn build_current_level(&mut self) {
        for room in 0..9u8 {
            dig_room(&mut self.dungeon, room, self.depth, &mut self.rng);
            self.populate_room(room);
        }
        dig_passages(&mut self.dungeon, self.depth, &mut self.rng);
        self.no_food = self.no_food.saturating_add(1);
        self.populate_things();
        let occupied: Vec<Pos> = self
            .floor_items
            .iter()
            .filter_map(|item| item.pos)
            .collect();
        finish_level(&mut self.rng, self.depth, &mut self.dungeon, &occupied);
        self.place_player_on_empty_floor();
        self.update_visibility();
    }
    fn after_turn(&mut self) {
        self.turn += 1;
        self.move_monsters();
        if self.end != EndState::Playing {
            return;
        }
        self.heal();
        self.digest();
        if self.end != EndState::Playing {
            return;
        }
        self.tick_effects();
        self.wandering_countdown -= 1;
        if self.wandering_countdown <= 0 {
            if self.rng.roll(1, 6) == 4 {
                self.spawn_random_monster(true);
                self.wandering_countdown = self.rng.spread(70) + 4;
            } else {
                self.wandering_countdown = 4;
            }
        }
    }
    fn automatic_ring_checks(&mut self) {
        for hand in 0..2 {
            let which = self.player.rings[hand].and_then(|id| {
                self.player
                    .inventory
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.which)
            });
            match which {
                Some(3) => self.search(),
                Some(11) if self.rng.rnd(50) == 0 => self.random_teleport(),
                _ => {}
            }
        }
    }
    fn advance_player_turn(&mut self) {
        self.advance_player_turn_inner(false);
    }

    /// Advance a turn after an intermediate step of an automatic run.  The
    /// reference still runs the world daemons, but its `visuals` daemon skips
    /// hallucination redraws while both `running` and `jump` are true.
    fn advance_player_turn_while_running(&mut self) {
        self.advance_player_turn_inner(true);
    }

    fn advance_player_turn_inner(&mut self, running: bool) {
        if self.skip_world_once {
            self.skip_world_once = false;
            return;
        }
        if self.player.conditions.hasted && !self.haste_phase {
            self.haste_phase = true;
        } else {
            self.haste_phase = false;
            self.after_turn();
            if self.end == EndState::Playing {
                self.automatic_ring_checks();
                if !(running && self.options.jump) {
                    self.refresh_hallucination_visuals();
                }
            }
        }
    }
    fn random_teleport(&mut self) {
        self.teleport_player();
    }
    fn teleport_player(&mut self) {
        if let Some(destination) = self.reference_monster_floor_position() {
            self.player.pos = destination;
            self.flytrap_holder = None;
            self.flytrap_hits = 0;
            self.player.conditions.held_turns = 0;
            self.update_visibility();
        }
    }

    #[cfg(test)]
    fn teleport_positions(&self, room_id: u8) -> Vec<Pos> {
        let Some(room) = self.dungeon.rooms.get(room_id as usize) else {
            return Vec::new();
        };
        self.dungeon
            .map
            .iter()
            .filter_map(|(pos, cell)| {
                let inside = if room.maze {
                    pos.x >= room.top_left.x
                        && pos.x <= room.top_left.x + room.width
                        && pos.y >= room.top_left.y
                        && pos.y <= room.top_left.y + room.height
                } else {
                    pos.x > room.top_left.x
                        && pos.x < room.top_left.x + room.width - 1
                        && pos.y > room.top_left.y
                        && pos.y < room.top_left.y + room.height - 1
                };
                (inside
                    && cell.terrain.passable()
                    && self.monsters.iter().all(|monster| monster.pos != pos))
                .then_some(pos)
            })
            .collect()
    }

    fn player_attack(&mut self, index: usize) {
        self.player_attack_inner(index, false)
    }

    fn player_attack_inner(&mut self, index: usize, suppress_messages: bool) {
        self.quiet_turns = 0;
        self.runto_monster(index);
        if self.reveal_xeroc(index) {
            self.message(if self.player.conditions.hallucinating {
                "strašno!  To jest ⟨a:zly:stvorjeńje:nom⟩ ⟨n:stvorjeńje:nom⟩!"
            } else {
                "⟨vim:počekati⟩!  To jest kserok!"
            });
            return;
        }
        let weapon = self
            .player
            .weapon
            .and_then(|id| self.player.inventory.iter().find(|i| i.id == id));
        let (damage, mut hit_plus, mut damage_plus) = match weapon {
            None => ("1x4", 0, 0),
            Some(w) if w.kind == ItemKind::Weapon => (
                combat::WEAPON_DAMAGE[w.which as usize],
                w.hit_plus as i32,
                w.damage_plus as i32,
            ),
            Some(w) if w.kind == ItemKind::Stick => (
                if self.appearances.stick_is_staff[w.which as usize] {
                    "2x3"
                } else {
                    "1x1"
                },
                0,
                0,
            ),
            Some(_) => ("0x0", 0, 0),
        };
        if weapon.is_some() {
            for ring in self
                .player
                .rings
                .iter()
                .flatten()
                .filter_map(|id| self.player.inventory.iter().find(|i| i.id == *id))
            {
                if ring.which == 7 {
                    hit_plus += ring.armor_class.unwrap_or(0)
                } else if ring.which == 8 {
                    damage_plus += ring.armor_class.unwrap_or(0)
                }
            }
        }
        let attack = Attack {
            level: self.player.stats.level,
            strength: self.player.stats.strength,
            hit_bonus: hit_plus,
            damage_bonus: damage_plus,
        };
        let outcome = combat::resolve_outcome(
            &mut self.rng,
            attack,
            self.monsters[index].armor,
            damage,
            self.monsters[index].awake,
        );
        if !outcome.hit {
            if !suppress_messages {
                let defender = self.monster_message_name(index, Case::Gen);
                let message = self.attack_miss_message(None, Some(&defender));
                self.message(message);
            }
            return;
        }
        self.monsters[index].hp -= outcome.damage;
        if self.player.conditions.can_confuse_monster {
            self.player.conditions.can_confuse_monster = false;
            self.monsters[index].flags |= monster::CONFUSED;
            let color = self.pick_color("črveny");
            self.message(format!(
                "⟨a:tvoj:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩ ⟨v3p:prěstavati⟩ světiti sę {}",
                crate::lang::color_adv(color)
            ));
        }
        if self.monsters[index].hp <= 0 {
            self.kill_monster(index);
        } else if !suppress_messages {
            let defender = self.monster_message_name(index, Case::Acc);
            let message = self.attack_hit_message(None, Some(&defender));
            self.message(message);
        }
    }

    fn monster_message_name(&mut self, index: usize, case: Case) -> String {
        if !self.can_see_monster(&self.monsters[index]) && !self.player.conditions.detect_monsters {
            // "ono"/"něčto" are invariant across cases.
            return if self.options.terse { "ono" } else { "něčto" }.into();
        }
        let kind = if self.player.conditions.hallucinating {
            let glyph = self.glyph_at(self.monsters[index].pos);
            if glyph.is_ascii_uppercase() {
                glyph as u8 - b'A'
            } else {
                self.rng.rnd(26) as u8
            }
        } else {
            self.monsters[index].kind
        };
        crate::lang::phrase(
            &crate::lang::MONSTER_LEX[kind as usize],
            case,
            interslavic::Number::Singular,
        )
    }

    fn reveal_xeroc(&mut self, index: usize) -> bool {
        if self.monsters[index].kind != 23
            || self.monsters[index].disguise == 'X'
            || self.player.conditions.blind
        {
            return false;
        }
        self.monsters[index].disguise = 'X';
        if self.player.conditions.hallucinating {
            let id = self.monsters[index].id;
            let glyph = (b'A' + self.rng.rnd(26) as u8) as char;
            if let Some((_, current)) = self
                .hallucinated_monsters
                .iter_mut()
                .find(|(monster_id, _)| *monster_id == id)
            {
                *current = glyph;
            } else {
                self.hallucinated_monsters.push((id, glyph));
            }
        }
        true
    }

    /// `attacker`: Nom-case name (None = the player, addressed as "ty").
    /// `defender`: Acc-case name (None = the player, "tebe").
    fn attack_hit_message(&mut self, attacker: Option<&str>, defender: Option<&str>) -> String {
        if self.options.terse {
            return crate::lang::speak(&match attacker {
                Some(subject) => format!("{} ⟨v3:udarjati⟩", uppercase_first(subject)),
                None => "⟨v2:udarjati⟩".to_string(),
            });
        }
        let target = defender.unwrap_or("tebe");
        match attacker {
            None => {
                const PLAYER: [&str; 4] = [
                    "odlično ⟨v2:udarjati⟩",
                    "⟨v2:udarjati⟩",
                    "⟨v2:raniti⟩",
                    "⟨v2:mahati⟩ i ⟨v2:udarjati⟩",
                ];
                crate::lang::speak(&format!("{} {target}", PLAYER[self.rng.rnd(4) as usize]))
            }
            Some(subject) => {
                const MONSTER: [&str; 4] = [
                    "odlično ⟨v3:udarjati⟩",
                    "⟨v3:udarjati⟩",
                    "⟨v3:raniti⟩",
                    "⟨v3:mahati⟩ i ⟨v3:udarjati⟩",
                ];
                crate::lang::speak(&format!(
                    "{} {} {target}",
                    uppercase_first(subject),
                    MONSTER[self.rng.rnd(4) as usize]
                ))
            }
        }
    }

    /// Nom attacker / GENITIVE defender (miss phrasings are built on
    /// `mimo` + Gen and negation + Gen: the dictionary marks `hybiti`
    /// intransitive — verb_info, interslavic 0.12.0 — so the old
    /// accusative object was ungrammatical).
    fn attack_miss_message(&mut self, attacker: Option<&str>, defender: Option<&str>) -> String {
        if self.options.terse {
            return crate::lang::speak(&match attacker {
                Some(subject) => format!("{} ⟨v3:hybiti⟩", uppercase_first(subject)),
                None => "⟨v2:hybiti⟩".to_string(),
            });
        }
        let target = defender.unwrap_or("tebe");
        match attacker {
            None => {
                const PLAYER: [&str; 4] = [
                    "⟨v2:udarjati⟩ mimo",
                    "⟨v2:mahati⟩ i ⟨v2:udarjati⟩ mimo",
                    "jedva ⟨v2:udarjati⟩ mimo",
                    "ne ⟨v2:udarjati⟩",
                ];
                crate::lang::speak(&format!("{} {target}", PLAYER[self.rng.rnd(4) as usize]))
            }
            Some(subject) => {
                const MONSTER: [&str; 4] = [
                    "⟨v3:udarjati⟩ mimo",
                    "⟨v3:mahati⟩ i ⟨v3:udarjati⟩ mimo",
                    "jedva ⟨v3:udarjati⟩ mimo",
                    "ne ⟨v3:udarjati⟩",
                ];
                crate::lang::speak(&format!(
                    "{} {} {target}",
                    uppercase_first(subject),
                    MONSTER[self.rng.rnd(4) as usize]
                ))
            }
        }
    }

    fn kill_monster(&mut self, index: usize) {
        let defeated = self.monster_message_name(index, Case::Acc);
        let monster = self.monsters.remove(index);
        if monster.kind == 5 {
            self.flytrap_holder = None;
            self.flytrap_hits = 0;
        }
        self.player.stats.experience += u64::from(monster.experience);
        self.message(format!("⟨v2:ubivati⟩ {defeated}"));
        self.drop_monster_inventory(monster);
        self.check_experience();
    }

    fn drop_monster_inventory(&mut self, mut monster: Monster) {
        if monster.kind == 11 {
            let gold_can_fall = self.fall_position(monster.pos).is_some();
            if gold_can_fall && self.depth >= self.max_depth {
                let mut amount = self.rng.rnd(50 + 10 * self.depth) + 2;
                if self.player_saves(3) {
                    for _ in 0..4 {
                        amount += self.rng.rnd(50 + 10 * self.depth) + 2;
                    }
                }
                let gold = Item::gold(self.id(), amount as i32);
                monster.inventory.insert(0, gold);
            }
        }
        for item in monster.inventory.drain(..) {
            self.land_projectile(item, monster.pos, false);
        }
    }

    fn check_experience(&mut self) {
        let new_level = crate::player::EXPERIENCE_LEVELS
            .iter()
            .position(|threshold| *threshold > self.player.stats.experience)
            .map_or(crate::player::EXPERIENCE_LEVELS.len() + 1, |index| {
                index + 1
            }) as i32;
        let old_level = self.player.stats.level;
        self.player.stats.level = new_level;
        if new_level > old_level {
            let gain = self.rng.roll((new_level - old_level) as u32, 10);
            self.player.stats.max_hp += gain;
            self.player.stats.hp += gain;
            self.message(format!("⟨v2:dostigati⟩ ⟨n:stųpenj:gen⟩ {new_level}"));
        }
    }

    pub fn begin_command(&mut self) {
        if self.end == EndState::Playing {
            self.wake_nearby_monsters();
        }
    }

    pub fn remember_command(&mut self, command: char) {
        self.previous_command = self.last_command;
        self.previous_item = self.last_item;
        self.previous_direction = self.last_direction;
        self.previous_hand = self.last_hand;
        self.last_command = Some(command);
        self.last_direction = None;
        self.last_item = None;
        self.last_hand = None;
    }

    pub fn reset_last_command(&mut self) {
        self.last_command = self.previous_command;
        self.last_item = self.previous_item;
        self.last_direction = self.previous_direction;
        self.last_hand = self.previous_hand;
    }

    fn wake_nearby_monsters(&mut self) {
        let player_cell = self.dungeon.map.get(self.player.pos).copied();
        let mut ids: Vec<u64> = self
            .monsters
            .iter()
            .filter(|candidate| {
                if self.player.conditions.detect_monsters
                    && candidate.flags & monster::INVISIBLE != 0
                {
                    return false;
                }
                let Some(monster_cell) = self.dungeon.map.get(candidate.pos) else {
                    return false;
                };
                let Some(player_cell) = player_cell else {
                    return false;
                };
                // look(true) parity: every monster in the shared LIT room is
                // re-rolled each turn (reference: command.c runs look(true)
                // before every command; misc.c wake_monster gives sleeping
                // mean monsters a fresh rnd(3)!=0 chance per sighting).
                if let (Some(proom), Some(mroom)) = (player_cell.room, monster_cell.room)
                    && proom == mroom
                    && self
                        .dungeon
                        .rooms
                        .get(proom as usize)
                        .is_some_and(|room| !room.dark)
                {
                    return true;
                }
                // Dark rooms and corridors: lamp-radius adjacency, as before.
                if candidate.pos.distance2(self.player.pos) > 2 {
                    return false;
                }
                if player_cell.terrain != Terrain::Door
                    && monster_cell.terrain != Terrain::Door
                    && player_cell.passage.is_some() != monster_cell.passage.is_some()
                {
                    return false;
                }
                candidate.pos.x == self.player.pos.x
                    || candidate.pos.y == self.player.pos.y
                    || (self.passable(Pos::new(candidate.pos.x, self.player.pos.y))
                        && self.passable(Pos::new(self.player.pos.x, candidate.pos.y)))
            })
            .map(|monster| monster.id)
            .collect();
        ids.sort_by_key(|id| {
            let pos = self
                .monsters
                .iter()
                .find(|monster| monster.id == *id)
                .unwrap()
                .pos;
            (pos.y, pos.x)
        });
        let area = self.area_key(self.player.pos);
        self.wake_monsters(ids, area);
    }

    fn wake_room_monsters(&mut self, room: u8) {
        let mut ids: Vec<u64> = self
            .monsters
            .iter()
            .filter(|monster| {
                monster.disguise.is_ascii_uppercase()
                    && self
                        .dungeon
                        .map
                        .get(monster.pos)
                        .is_some_and(|cell| cell.room == Some(room))
            })
            .map(|monster| monster.id)
            .collect();
        ids.sort_by_key(|id| {
            let pos = self
                .monsters
                .iter()
                .find(|monster| monster.id == *id)
                .unwrap()
                .pos;
            (pos.y, pos.x)
        });
        self.wake_monsters(ids, Some((false, room)));
    }

    fn wake_monsters(&mut self, ids: Vec<u64>, area: Option<(bool, u8)>) {
        for id in ids {
            let Some(index) = self.monsters.iter().position(|monster| monster.id == id) else {
                continue;
            };
            if !self.monsters[index].awake {
                let wake_roll = self.rng.rnd(3) != 0;
                if wake_roll
                    && self.monsters[index].flags & monster::MEAN != 0
                    && self.monsters[index].flags & monster::HELD == 0
                    && !self.wears_ring(12)
                    && !self.player.conditions.levitating
                {
                    self.monsters[index].awake = true;
                    self.monsters[index].destination = None;
                    self.monsters[index].destination_is_room_gold = false;
                }
            }
            let lit_area = area.is_some_and(|(passage, id)| {
                !passage
                    && self
                        .dungeon
                        .rooms
                        .get(id as usize)
                        .is_some_and(|room| !room.dark)
            });
            if self.monsters[index].kind == 12
                && !self.player.conditions.blind
                && !self.player.conditions.hallucinating
                && self.monsters[index].flags & (monster::CANCELLED | monster::GAZE_USED) == 0
                && self.monsters[index].awake
                && (lit_area || self.monsters[index].pos.distance2(self.player.pos) < 3)
            {
                self.monsters[index].flags |= monster::GAZE_USED;
                if !self.player_saves(3) {
                    self.player.conditions.confused = true;
                    let duration = self.rng.spread(20);
                    self.scheduler.add_or_lengthen(Effect::Confusion, duration);
                    let name = self.monster_message_name(index, Case::Gen);
                    self.message(if name == "ono" || name == "něčto" {
                        "⟨on:gen:f⟩ ⟨n:poględ:nom⟩ ⟨ty:acc⟩ ⟨lp:smųtiti:m⟩".into()
                    } else {
                        format!("⟨n:poględ:nom⟩ {name} ⟨ty:acc⟩ ⟨lp:smųtiti:m⟩")
                    });
                }
            }
            if self.monsters[index].flags & monster::GREED != 0 && !self.monsters[index].awake {
                self.monsters[index].awake = true;
                let room_gold = area.and_then(|(passage, room)| {
                    (!passage)
                        .then(|| self.dungeon.rooms.get(room as usize))
                        .flatten()
                        .filter(|room| room.gold_value != 0)
                        .and_then(|room| room.gold)
                });
                self.monsters[index].destination = room_gold;
                self.monsters[index].destination_is_room_gold = room_gold.is_some();
            }
        }
    }

    fn move_monsters(&mut self) {
        let mut index = 0;
        while index < self.monsters.len() && self.end == EndState::Playing {
            if !self.monsters[index].awake || self.monsters[index].flags & monster::HELD != 0 {
                index += 1;
                continue;
            }
            let monster_id = self.monsters[index].id;
            let original_position = self.monsters[index].pos;
            let was_fight_target = self.fight_target == Some(monster_id);
            let mut alive = self.move_scheduled_monster(index);
            if alive
                && index < self.monsters.len()
                && self.monsters[index].flags & monster::FLY != 0
                && self.monsters[index].pos.distance2(self.player.pos) >= 3
                && !self.move_scheduled_monster(index)
            {
                alive = false;
            }
            if !alive || index >= self.monsters.len() {
                if was_fight_target && self.monsters.iter().all(|monster| monster.id != monster_id)
                {
                    self.fight_target = None;
                    self.fight_kamikaze = false;
                }
                continue;
            }
            if was_fight_target
                && self.monsters[index].id == monster_id
                && self.monsters[index].pos != original_position
            {
                self.fight_target = None;
            }
            index += 1;
        }
    }

    fn move_scheduled_monster(&mut self, index: usize) -> bool {
        let steps = Self::scheduled_monster_steps(&mut self.monsters[index]);
        for _ in 0..steps {
            if index >= self.monsters.len() || !self.move_monster_step(index) {
                return false;
            }
        }
        true
    }

    fn scheduled_monster_steps(monster: &mut Monster) -> usize {
        let mut steps = usize::from(monster.flags & monster::SLOWED == 0 || monster.turn);
        steps += usize::from(monster.flags & monster::HASTED != 0);
        monster.turn = !monster.turn;
        steps
    }

    fn move_monster_step(&mut self, index: usize) -> bool {
        let greedy_without_room_gold = self.monsters[index].flags & monster::GREED != 0
            && self
                .area_key(self.monsters[index].pos)
                .and_then(|(passage, room)| {
                    (!passage).then(|| self.dungeon.rooms.get(room as usize))
                })
                .flatten()
                .is_none_or(|room| room.gold_value == 0);
        if greedy_without_room_gold {
            self.monsters[index].destination = None;
            self.monsters[index].destination_is_room_gold = false;
        }
        let kind = self.monsters[index].kind;
        let from = self.monsters[index].pos;
        let destination = self.monsters[index].destination.unwrap_or(self.player.pos);
        let chase_target = self.monster_chase_target(from, destination);
        let distance = from.distance2(self.player.pos);
        if kind == 3
            && distance <= 36
            && self.chase_area(from) == self.chase_area(destination)
            && (from.x == self.player.pos.x
                || from.y == self.player.pos.y
                || (from.x - self.player.pos.x).abs() == (from.y - self.player.pos.y).abs())
            && self.monsters[index].flags & monster::CANCELLED == 0
            && self.rng.rnd(5) == 0
        {
            self.quiet_turns = 0;
            let dx = (self.player.pos.x - from.x).signum();
            let dy = (self.player.pos.y - from.y).signum();
            let direction =
                Direction::from_delta(dx, dy).expect("dragon breath is a straight shot");
            self.fire_bolt_from(from, direction, 3, Some(kind));
            return true;
        }
        let confused = self.monsters[index].flags & monster::CONFUSED != 0;
        let random = (confused && self.rng.rnd(5) != 0)
            || (kind == 15 && self.rng.rnd(5) == 0)
            || (kind == 1 && self.rng.rnd(2) == 0);
        let next = if random {
            let dy = self.rng.rnd(3) as i32 - 1;
            let dx = self.rng.rnd(3) as i32 - 1;
            let attempt = from.offset(dx, dy);
            if self.rng.rnd(20) == 0 {
                self.monsters[index].flags &= !monster::CONFUSED;
            }
            if attempt == from || self.legal_monster_step(index, from, attempt) {
                attempt
            } else {
                from
            }
        } else {
            let mut current_distance = from.distance2(chase_target);
            let mut selected = from;
            let mut place_count = 1;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let candidate = from.offset(dx, dy);
                    if !self.legal_monster_step(index, from, candidate) {
                        continue;
                    }
                    let candidate_distance = candidate.distance2(chase_target);
                    if candidate_distance < current_distance {
                        place_count = 1;
                        selected = candidate;
                        current_distance = candidate_distance;
                    } else if candidate_distance == current_distance {
                        place_count += 1;
                        if self.rng.rnd(place_count) == 0 {
                            selected = candidate;
                            current_distance = candidate_distance;
                        }
                    }
                }
            }
            selected
        };
        let chase_continues = next.distance2(chase_target) != 0 && next != self.player.pos;
        if !chase_continues && chase_target == self.player.pos {
            let before = self.monsters.len();
            self.monster_attack(index);
            return self.monsters.len() == before;
        }
        let mut stop_running = false;
        if !chase_continues && chase_target == destination {
            if !self.monsters[index].destination_is_room_gold
                && let Some(item_index) = self
                    .floor_items
                    .iter()
                    .position(|item| item.pos == Some(destination))
            {
                let mut item = self.floor_items.remove(item_index);
                item.pos = None;
                self.monsters[index].inventory.insert(0, item);
                self.monsters[index].destination = self.find_monster_item_destination(index);
                self.monsters[index].destination_is_room_gold = false;
            }
            stop_running = kind != 5;
        } else if kind == 5 {
            return true;
        }
        self.monsters[index].pos = next;
        let active_destination = self.monsters[index].destination.unwrap_or(self.player.pos);
        if stop_running && next == active_destination {
            self.monsters[index].awake = false;
        }
        true
    }

    fn legal_monster_step(&self, index: usize, from: Pos, candidate: Pos) -> bool {
        if candidate == from
            || !self.passable(candidate)
            || self
                .monsters
                .iter()
                .enumerate()
                .any(|(other, monster)| other != index && monster.pos == candidate)
            || self.floor_items.iter().any(|item| {
                item.pos == Some(candidate) && item.kind == ItemKind::Scroll && item.which == 10
            })
        {
            return false;
        }
        let dx = candidate.x - from.x;
        let dy = candidate.y - from.y;
        dx == 0
            || dy == 0
            || (self.passable(from.offset(dx, 0)) && self.passable(from.offset(0, dy)))
    }

    /// Chase-routing area: ROOM-first, matching do_chase's `t_room`/`roomin`
    /// semantics (a door-stander counts as being in the room). `area_key`
    /// stays passage-first for the wake/visibility logic that needs it.
    fn chase_area(&self, p: Pos) -> Option<(bool, u8)> {
        let cell = self.dungeon.map.get(p)?;
        cell.room
            .map(|room| (false, room))
            .or_else(|| cell.passage.map(|passage| (true, passage)))
    }

    fn monster_chase_target(&self, from: Pos, destination: Pos) -> Pos {
        let Some(area) = self.chase_area(from) else {
            return destination;
        };
        if Some(area) == self.chase_area(destination) {
            // Same room (or same corridor): head straight for the target —
            // this is what unfreezes a door-stander attacking into the room.
            return destination;
        }
        let mut exits: Vec<Pos> = if area.0 {
            self.dungeon
                .passage_exits
                .get(area.1 as usize)
                .cloned()
                .unwrap_or_default()
        } else {
            self.dungeon.rooms[area.1 as usize].exits.clone()
        };
        // do_chase's door clause: a door-stander routing to another room
        // considers its passage's exits too, keeping the running minimum
        // (the C code's `goto over` without resetting mindist).
        if let Some(cell) = self.dungeon.map.get(from)
            && cell.terrain == Terrain::Door
            && let Some(passage) = cell.passage
            && let Some(passage_exits) = self.dungeon.passage_exits.get(passage as usize)
        {
            exits.extend(passage_exits.iter().copied());
        }
        exits
            .into_iter()
            .min_by_key(|exit| exit.distance2(destination))
            .unwrap_or(destination)
    }

    /// Mirror chase.c's `find_dest`: a running monster outside the player's
    /// room may divert to the first unclaimed object in its room according to
    /// its carry probability.
    fn find_monster_item_destination(&mut self, index: usize) -> Option<Pos> {
        let monster = self.monsters[index].clone();
        self.find_monster_item_destination_for(&monster)
    }

    fn find_monster_item_destination_for(&mut self, monster: &Monster) -> Option<Pos> {
        let probability = MONSTERS[monster.kind as usize].carry;
        let area = self.area_key(monster.pos);
        let player_area = self.area_key(self.player.pos);
        if probability == 0 || area == player_area || self.can_see_monster(monster) {
            return None;
        }
        for item in self.floor_items.iter().rev() {
            let Some(pos) = item.pos else { continue };
            if item.kind == ItemKind::Scroll && item.which == 10 {
                continue;
            }
            if self.area_key(pos) != area {
                continue;
            }
            if self.rng.rnd(100) >= probability as u32 {
                continue;
            }
            if self.monsters.iter().any(|monster| {
                !monster.destination_is_room_gold && monster.destination == Some(pos)
            }) {
                continue;
            }
            return Some(pos);
        }
        None
    }

    fn aggravate_monsters(&mut self) {
        for monster in &mut self.monsters {
            monster.awake = true;
            monster.flags &= !monster::HELD;
        }
        for index in 0..self.monsters.len() {
            self.monsters[index].destination = self.find_monster_item_destination(index);
            self.monsters[index].destination_is_room_gold = false;
        }
    }

    fn record_fight_hit(&mut self, damage: i32) {
        if self.fight_target.is_some() && !self.fight_kamikaze {
            self.fight_safety_max_hit = self.fight_safety_max_hit.max(damage);
            if self.player.stats.hp <= self.fight_safety_max_hit {
                self.fight_target = None;
            }
        }
    }

    fn monster_attack(&mut self, index: usize) {
        self.quiet_turns = 0;
        let kind = self.monsters[index].kind;
        let monster_id = self.monsters[index].id;
        self.reveal_xeroc(index);
        let attacker_name = self.monster_message_name(index, Case::Nom);
        if self.fight_target.is_some_and(|target| target != monster_id) {
            self.fight_target = None;
            self.fight_kamikaze = false;
        }
        let suppress_attack_message = self.fight_target == Some(monster_id);
        let template = MONSTERS[kind as usize];
        let attack = Attack {
            level: self.monsters[index].level,
            strength: 10,
            hit_bonus: 0,
            damage_bonus: 0,
        };
        let flytrap_damage =
            (kind == 5 && self.flytrap_hits > 0).then(|| format!("{}x1", self.flytrap_hits));
        let outcome = combat::resolve_outcome(
            &mut self.rng,
            attack,
            self.player.armor_class(),
            flytrap_damage.as_deref().unwrap_or(template.damage),
            self.player_is_running,
        );
        if kind == 5 && self.monsters[index].flags & monster::CANCELLED == 0 {
            if !outcome.hit {
                self.damage_player(self.flytrap_hits);
                if self.player.stats.hp <= 0 {
                    self.die(Self::monster_killer(kind));
                    return;
                }
                if !suppress_attack_message {
                    let message = self.attack_miss_message(Some(&attacker_name), None);
                    self.message(message);
                }
                return;
            }
            let old_hp = self.player.stats.hp;
            self.damage_player(outcome.damage);
            if !suppress_attack_message {
                let message = self.attack_hit_message(Some(&attacker_name), None);
                self.message(message);
            }
            if self.player.stats.hp <= 0 {
                self.die(Self::monster_killer(kind));
                return;
            }
            self.record_fight_hit(old_hp - self.player.stats.hp);
            self.flytrap_hits += 1;
            self.flytrap_holder = Some(self.monsters[index].id);
            self.damage_player(1);
            if self.player.stats.hp <= 0 {
                self.die(Self::monster_killer(kind))
            }
            return;
        }
        if !outcome.hit {
            if kind == 5 && self.flytrap_hits > 0 {
                self.damage_player(self.flytrap_hits);
                if self.player.stats.hp <= 0 {
                    self.die(Self::monster_killer(kind));
                    return;
                }
            }
            if kind != 8 && !suppress_attack_message {
                let message = self.attack_miss_message(Some(&attacker_name), None);
                self.message(message);
            }
            return;
        }
        let old_hp = self.player.stats.hp;
        self.damage_player(outcome.damage);
        if kind != 8 && !suppress_attack_message {
            let message = self.attack_hit_message(Some(&attacker_name), None);
            self.message(message);
        }
        if self.player.stats.hp <= 0 {
            self.die(Self::monster_killer(kind));
            return;
        }
        self.record_fight_hit(old_hp - self.player.stats.hp);
        if self
            .monsters
            .get(index)
            .is_some_and(|m| m.flags & monster::CANCELLED != 0)
        {
            return;
        }
        match kind {
            0 => self.rust_armor_inner(self.fight_target.is_none()),
            8 => {
                self.player_is_running = false;
                let was_awake = self.player.conditions.asleep_turns > 0;
                self.player.conditions.asleep_turns += self.rng.rnd(2) + 2;
                if !was_awake {
                    self.message(if self.options.terse {
                        "ne ⟨v2:mogti⟩ dvigati sę".into()
                    } else {
                        format!("{attacker_name} ⟨ty:acc⟩ ⟨v3:zamražati⟩")
                    });
                }
                if self.player.conditions.asleep_turns > 50 {
                    self.die("⟨n:hlåd:gen⟩");
                }
            }
            17 if !self.player_saves(0) => {
                if self.wears_ring(2) {
                    if self.fight_target.is_none() {
                        self.message(if self.options.terse {
                            "⟨n:ukųs:nom⟩ ne ⟨v3:škoditi⟩ ⟨ty:dat⟩"
                        } else {
                            "⟨n:ukųs:nom⟩ na moment ⟨ty:acc⟩ ⟨v3:oslabjati⟩"
                        });
                    }
                } else {
                    self.player.stats.strength = (self.player.stats.strength - 1).max(3);
                    self.message(if self.options.terse {
                        "⟨n:jad:nom⟩ ⟨ty:acc⟩ ⟨v3:oslabjati⟩"
                    } else {
                        "⟨v2:čuti⟩ ⟨n:ukųs:acc⟩ v ⟨n:noga:loc⟩ i ⟨v2:čuti⟩ sę ⟨cav:slaby⟩"
                    });
                }
            }
            22 if self.rng.rnd(100) < 15 => self.drain_level(),
            21 if self.rng.rnd(100) < 30 => {
                let loss = self.rng.roll(1, 3);
                self.drain_max_hp(loss, "⟨n:vampir:gen⟩");
            }
            11 => {
                let old_gold = self.player.gold;
                let mut amount = self.rng.rnd(50 + 10 * self.depth) + 2;
                if !self.player_saves(3) {
                    for _ in 0..4 {
                        amount += self.rng.rnd(50 + 10 * self.depth) + 2;
                    }
                }
                self.player.gold = self.player.gold.saturating_sub(amount as i32);
                self.monsters.remove(index);
                if self.player.gold != old_gold {
                    self.message(
                        "⟨a:tvoj:torba:nom⟩ ⟨n:torba:nom⟩ ⟨v3h:stajati:staje⟩ sę ⟨cav:legky⟩",
                    );
                }
            }
            13 => self.nymph_steal(index),
            _ => {}
        }
    }

    fn wears_ring(&self, which: u8) -> bool {
        self.player.rings.iter().flatten().any(|id| {
            self.player
                .inventory
                .iter()
                .any(|i| i.id == *id && i.which == which)
        })
    }
    fn ring_strength_bonus(&self) -> i32 {
        self.player
            .rings
            .iter()
            .flatten()
            .filter_map(|id| self.player.inventory.iter().find(|item| item.id == *id))
            .filter(|item| item.kind == ItemKind::Ring && item.which == 1)
            .map(|item| item.armor_class.unwrap_or(0))
            .sum()
    }
    fn player_saves(&mut self, mut which: i32) -> bool {
        if which == 3 {
            which -= self
                .player
                .rings
                .iter()
                .flatten()
                .filter_map(|id| self.player.inventory.iter().find(|item| item.id == *id))
                .filter(|item| item.kind == ItemKind::Ring && item.which == 0)
                .map(|item| item.armor_class.unwrap_or(0))
                .sum::<i32>();
        }
        let need = 14 + which - self.player.stats.level / 2;
        self.rng.roll(1, 20) >= need
    }
    fn rust_armor(&mut self) {
        self.rust_armor_inner(true)
    }
    fn rust_armor_inner(&mut self, report_protection: bool) {
        let Some(id) = self.player.armor else { return };
        let Some(armor) = self.player.inventory.iter().find(|item| item.id == id) else {
            return;
        };
        if armor.which == 0 || armor.armor_class.is_none_or(|value| value >= 9) {
            return;
        }
        if armor.protected || self.wears_ring(13) {
            if report_protection {
                self.message("⟨n:rđa:nom⟩ naglo ⟨v3:izčezati⟩");
            }
            return;
        }
        let armor = self
            .player
            .inventory
            .iter_mut()
            .find(|item| item.id == id)
            .expect("equipped armor remains in the pack");
        armor.armor_class = armor.armor_class.map(|value| value + 1);
        self.message(if self.options.terse {
            "⟨a:tvoj:brȯnja:nom⟩ ⟨n:brȯnja:nom⟩ ⟨v3:slaběti⟩"
        } else {
            "⟨a:tvoj:brȯnja:nom⟩ ⟨n:brȯnja:nom⟩ sejčas ⟨v3:izględati⟩ ⟨cav:slaby⟩. O ne!"
        });
    }
    fn drain_level(&mut self) {
        if self.player.stats.experience == 0 {
            self.die("⟨n:prizrak:gen⟩");
            return;
        }
        self.player.stats.level -= 1;
        self.player.stats.experience = if self.player.stats.level == 0 {
            self.player.stats.level = 1;
            0
        } else {
            crate::player::EXPERIENCE_LEVELS[self.player.stats.level as usize - 1] + 1
        };
        let loss = self.rng.roll(1, 10);
        self.drain_max_hp(loss, "⟨n:prizrak:gen⟩");
    }
    fn drain_max_hp(&mut self, loss: i32, cause: &str) {
        self.player.stats.max_hp -= loss;
        let damage = self.player.stats.hp - (self.player.stats.hp - loss).max(1);
        self.damage_player(damage);
        if self.player.stats.max_hp <= 0 {
            self.die(cause);
            return;
        }
        self.message("naglo ⟨v2:čuti⟩ sę ⟨cav:slaby⟩");
    }
    fn nymph_steal(&mut self, index: usize) {
        let mut choice = None;
        let mut eligible = 0_u32;
        for (inventory_index, item) in self.player.inventory.iter().enumerate() {
            let equipped = self.player.weapon == Some(item.id)
                || self.player.armor == Some(item.id)
                || self.player.rings.contains(&Some(item.id));
            if !equipped && Self::item_is_magic(item) {
                eligible += 1;
                if self.rng.rnd(eligible) == 0 {
                    choice = Some(inventory_index);
                }
            }
        }
        let Some(choice) = choice else { return };
        let mut stolen = self.player.inventory[choice].clone();
        stolen.count = 1;
        stolen.pack_letter = None;
        if self.player.inventory[choice].count > 1 {
            self.player.inventory[choice].count -= 1;
        } else {
            self.player.inventory.remove(choice);
        }
        self.monsters.remove(index);
        let name = self.inventory_name_case(&stolen, true, Case::Acc);
        self.message(format!("⟨ona:nom:f⟩ ⟨vpf3:ukrasti:f⟩ {name}!"));
    }

    fn digest(&mut self) {
        if self.player.food_left <= 0 {
            let old_food = self.player.food_left;
            self.player.food_left -= 1;
            if old_food < -850 {
                self.die("⟨n:glåd:gen⟩");
                self.message("⟨n:glåd:nom⟩ ⟨ty:acc⟩ ⟨lp:ubiti:m⟩");
            } else if self.player.conditions.asleep_turns == 0 && self.rng.rnd(5) == 0 {
                self.player.conditions.asleep_turns += self.rng.rnd(8) + 4;
                self.hungry_state = 3;
                self.message(if self.player.conditions.hallucinating {
                    if self.options.terse {
                        "Panika!"
                    } else {
                        "⟨n:glåd:nom⟩ ⟨v3:prěmagati⟩ ⟨ty:acc⟩.  Panika!"
                    }
                } else if self.options.terse {
                    "⟨v2:omlěvati:U⟩"
                } else {
                    "od ⟨n:nedostatȯk:gen⟩ ⟨n:jeda:gen⟩ ne ⟨v2:imati⟩ ⟨n:sila:gen:pl⟩.  ⟨v2:omlěvati:U⟩"
                });
            }
            return;
        }
        let rings: Vec<u8> = self
            .player
            .rings
            .iter()
            .flatten()
            .filter_map(|id| self.player.inventory.iter().find(|i| i.id == *id))
            .map(|r| r.which)
            .collect();
        let uses = [1, 1, 1, -3, -5, 0, 0, -3, -3, 2, -2, 0, 1, 1];
        let mut ring_cost = 0;
        for which in rings {
            let raw = uses[which as usize];
            let mut eat = if raw < 0 {
                i32::from(self.rng.rnd((-raw) as u32) == 0)
            } else {
                raw
            };
            if which == 10 {
                eat = -eat
            }
            ring_cost += eat
        }
        let old_food = self.player.food_left;
        self.player.food_left -= 1 + ring_cost - (self.has_amulet as i32);
        if self.player.food_left < 150 && old_food >= 150 {
            self.hungry_state = 2;
            self.message(if self.player.conditions.hallucinating {
                "od ⟨n:glåd:gen⟩ ne ⟨v2:mogti⟩ dobro dvigati sę"
            } else {
                "⟨v2:načinati⟩ čuti sę ⟨adv:slaby⟩"
            })
        } else if self.player.food_left < 300 && old_food >= 300 {
            self.hungry_state = 1;
            self.message(if self.player.conditions.hallucinating {
                "⟨v2:imati⟩ ⟨a:veliky:apetit:acc⟩ ⟨n:apetit:acc⟩"
            } else if self.options.terse {
                "⟨v2:čuti⟩ ⟨n:glåd:acc⟩"
            } else {
                "⟨v2:načinati⟩ čuti ⟨n:glåd:acc⟩"
            })
        }
    }
    fn heal(&mut self) {
        let old_hp = self.player.stats.hp;
        self.quiet_turns += 1;
        let level = self.player.stats.level;
        if (level < 8 && self.quiet_turns + 2 * level as u32 > 20)
            || (level >= 8 && self.quiet_turns >= 3)
        {
            let gain = if level < 8 {
                1
            } else {
                self.rng.rnd((level - 7) as u32) as i32 + 1
            };
            self.player.stats.hp = (self.player.stats.hp + gain).min(self.player.stats.max_hp);
        }
        for _ in 0..self
            .player
            .rings
            .iter()
            .flatten()
            .filter(|id| {
                self.player
                    .inventory
                    .iter()
                    .any(|i| i.id == **id && i.which == 9)
            })
            .count()
        {
            self.player.stats.hp = (self.player.stats.hp + 1).min(self.player.stats.max_hp)
        }
        if self.player.stats.hp != old_hp {
            self.quiet_turns = 0
        }
    }
    fn tick_effects(&mut self) {
        for effect in self.scheduler.tick() {
            match effect {
                Effect::Confusion => {
                    self.player.conditions.confused = false;
                    self.message(if self.player.conditions.hallucinating {
                        "sejčas ⟨v2:čuti⟩ sę menje ⟨adv:kosmičny⟩"
                    } else {
                        "sejčas ⟨v2:čuti⟩ sę menje ⟨pp:smųtiti:n⟩"
                    });
                }
                Effect::Hallucination => {
                    self.player.conditions.hallucinating = false;
                    self.message("vse sejčas ⟨v3:izględati⟩ TAKO ⟨adv:nudny⟩.");
                }
                Effect::SeeInvisible => self.player.conditions.see_invisible = false,
                Effect::Blindness => {
                    self.player.conditions.blind = false;
                    self.message(if self.player.conditions.hallucinating {
                        "hura!  Vse opęť jest ⟨adv:kosmičny⟩"
                    } else {
                        "⟨n:zavěsa:nom⟩ ⟨n:ťma:gen⟩ ⟨v3:izčezati⟩"
                    });
                }
                Effect::Levitation => {
                    self.player.conditions.levitating = false;
                    self.message(if self.player.conditions.hallucinating {
                        "buh!  ⟨v2:padati:U⟩ na ⟨n:zemja:acc⟩"
                    } else {
                        "legko ⟨v2:spušćati⟩ sę na ⟨n:zemja:acc⟩"
                    });
                }
                Effect::Haste => {
                    self.player.conditions.hasted = false;
                    self.haste_phase = false;
                    self.message("⟨v2:čuti⟩, že vse ⟨v2:dělati⟩ pomalo");
                }
                Effect::MonsterDetection => self.player.conditions.detect_monsters = false,
            }
        }
    }
    pub fn update_visibility(&mut self) {
        if self.player.conditions.blind {
            return;
        }
        let positions: Vec<Pos> = self
            .dungeon
            .map
            .iter()
            .filter_map(|(p, _)| self.currently_visible(p).then_some(p))
            .collect();
        for p in positions {
            if let Some(c) = self.dungeon.map.get_mut(p) {
                if c.wizard_revealed
                    && matches!(
                        c.terrain,
                        Terrain::SecretDoor
                            | Terrain::SecretDoorHorizontal
                            | Terrain::SecretDoorVertical
                    )
                {
                    c.wizard_revealed = false;
                }
                c.seen = true;
                c.remembered = if c.wizard_revealed && c.terrain == Terrain::SecretPassage {
                    c.remembered
                } else if c.trap_revealed {
                    '^'
                } else {
                    c.terrain.glyph()
                }
            }
        }
    }

    fn refresh_hallucination_visuals(&mut self) {
        self.hallucinated_items.clear();
        self.hallucinated_monsters.clear();
        self.hallucinated_stairs = None;
        if !self.player.conditions.hallucinating {
            return;
        }
        let visible_items: Vec<u64> = self
            .floor_items
            .iter()
            .filter(|item| item.pos.is_some_and(|pos| self.currently_visible(pos)))
            .map(|item| item.id)
            .collect();
        for id in visible_items {
            let glyph = self.random_thing_glyph();
            self.hallucinated_items.push((id, glyph));
        }
        if !self.seen_stairs && self.currently_visible(self.dungeon.stairs) {
            self.hallucinated_stairs = Some(self.random_thing_glyph());
        }
        let visible_monsters: Vec<(u64, bool)> = self
            .monsters
            .iter()
            .filter_map(|monster| {
                let seen = self.can_see_monster(monster);
                (seen || self.player.conditions.detect_monsters).then_some((
                    monster.id,
                    seen && monster.kind == 23 && monster.disguise != 'X',
                ))
            })
            .collect();
        for (id, disguised_xeroc) in visible_monsters {
            let glyph = if disguised_xeroc {
                self.random_thing_glyph()
            } else {
                (b'A' + self.rng.rnd(26) as u8) as char
            };
            self.hallucinated_monsters.push((id, glyph));
        }
    }

    fn random_thing_glyph(&mut self) -> char {
        const THINGS: [char; 10] = ['!', '?', '=', '/', ':', ')', ']', '%', '*', ','];
        let count = if self.depth >= AMULET_LEVEL {
            THINGS.len()
        } else {
            THINGS.len() - 1
        };
        THINGS[self.rng.rnd(count as u32) as usize]
    }

    pub fn glyph_at(&self, p: Pos) -> char {
        if p == self.player.pos {
            return '@';
        }
        if let Some(m) = self.monsters.iter().find(|m| m.pos == p) {
            let normally_seen = self.can_see_monster(m);
            if normally_seen || self.player.conditions.detect_monsters {
                return if self.player.conditions.hallucinating {
                    self.hallucinated_monsters
                        .iter()
                        .find_map(|(id, glyph)| (*id == m.id).then_some(*glyph))
                        .unwrap_or(m.disguise)
                } else if normally_seen {
                    m.disguise
                } else {
                    (b'A' + m.kind) as char
                };
            }
        }
        if let Some(i) = self.floor_items.iter().find(|i| i.pos == Some(p))
            && self.currently_visible(p)
        {
            return if self.player.conditions.hallucinating {
                self.hallucinated_items
                    .iter()
                    .find_map(|(id, glyph)| (*id == i.id).then_some(*glyph))
                    .unwrap_or_else(|| i.kind.glyph())
            } else {
                i.kind.glyph()
            };
        }
        self.remembered_glyph_at(p)
    }
    fn remembered_glyph_at(&self, p: Pos) -> char {
        self.dungeon.map.get(p).map_or(' ', |c| {
            if c.seen {
                if c.trap_revealed {
                    '^'
                } else if c.wizard_revealed {
                    c.remembered
                } else if c.terrain == Terrain::Floor
                    && self.currently_visible(p)
                    && self
                        .dungeon
                        .map
                        .get(self.player.pos)
                        .and_then(|cell| cell.room)
                        .and_then(|id| self.dungeon.rooms.get(id as usize))
                        .is_some_and(|room| room.dark)
                    && !self.options.see_floor
                {
                    ' '
                } else if c.terrain == Terrain::Stairs
                    && self.player.conditions.hallucinating
                    && !self.seen_stairs
                    && self.currently_visible(p)
                {
                    self.hallucinated_stairs.unwrap_or('>')
                } else {
                    c.terrain.glyph()
                }
            } else {
                c.remembered
            }
        })
    }
    fn currently_visible(&self, p: Pos) -> bool {
        if self.player.conditions.blind {
            return false;
        }
        if p.distance2(self.player.pos) < 3 {
            if p.x != self.player.pos.x
                && p.y != self.player.pos.y
                && !self.passable(Pos::new(p.x, self.player.pos.y))
                && !self.passable(Pos::new(self.player.pos.x, p.y))
            {
                return false;
            }
            return true;
        }
        let room = self.dungeon.map.get(self.player.pos).and_then(|c| c.room);
        room.is_some()
            && self.dungeon.map.get(p).and_then(|c| c.room) == room
            && !room
                .and_then(|id| self.dungeon.rooms.get(id as usize))
                .is_some_and(|r| r.dark)
    }
    fn can_see_monster(&self, monster: &Monster) -> bool {
        self.currently_visible(monster.pos)
            && (monster.flags & monster::INVISIBLE == 0
                || self.player.conditions.see_invisible
                || self.wears_ring(4))
    }
    /// The intended color lemma, or a random one while hallucinating.
    fn pick_color(&mut self, ordinary: &'static str) -> &'static str {
        if self.player.conditions.hallucinating {
            crate::lang::COLOR_ADJ[self.rng.rnd(crate::lang::COLOR_ADJ.len() as u32) as usize]
        } else {
            ordinary
        }
    }
    pub fn message(&mut self, text: impl Into<String>) {
        let text = crate::lang::speak(&text.into());
        self.recall_message.clone_from(&text);
        self.messages.push(text);
        self.message_serial = self.message_serial.wrapping_add(1);
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
    pub fn message_without_recall(&mut self, text: impl Into<String>) {
        self.messages.push(crate::lang::speak(&text.into()));
        self.message_serial = self.message_serial.wrapping_add(1);
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
    pub fn remember_message(&mut self, text: impl Into<String>) {
        self.recall_message = crate::lang::speak(&text.into());
    }
    pub fn set_wizard(&mut self, enabled: bool) {
        self.wizard = enabled;
        if enabled {
            self.no_score = true;
            self.set_monster_detection(true);
            self.message(format!(
                "naglo ⟨v2:znati⟩ vse, tako kako Ken Arnold, o ⟨n:temnica:loc⟩ #{}",
                self.dungeon_number
            ))
        } else {
            self.set_monster_detection(false);
            self.message("uže ne ⟨v2:byti⟩ čarovnik")
        }
    }
    pub fn set_startup_wizard(&mut self) {
        self.wizard = true;
        self.no_score = true;
        self.set_monster_detection(true);
    }
    fn set_monster_detection(&mut self, enabled: bool) {
        self.player.conditions.detect_monsters = enabled;
        if !enabled || !self.player.conditions.hallucinating {
            return;
        }
        self.hallucinated_monsters.clear();
        for monster in &self.monsters {
            let glyph = (b'A' + self.rng.rnd(26) as u8) as char;
            self.hallucinated_monsters.push((monster.id, glyph));
        }
    }
    fn wizard_command(&mut self, command: WizardCommand) {
        match command {
            WizardCommand::Coordinates => {
                self.message(format!("@ {},{}", self.player.pos.y, self.player.pos.x))
            }
            WizardCommand::PackCount => self.message(format!("inpack = {}", self.pack_count())),
            WizardCommand::Down => {
                self.depth += 1;
                self.max_depth = self.max_depth.max(self.depth);
                self.new_level()
            }
            WizardCommand::Up => {
                self.depth = self.depth.saturating_sub(1);
                self.new_level()
            }
            WizardCommand::Map => {}
            WizardCommand::Teleport => self.random_teleport(),
            WizardCommand::Food => self.message(format!(
                "⟨n:jeda:gen⟩ ⟨lp:ostati:n⟩: {}",
                self.player.food_left
            )),
            WizardCommand::AddPassages => {
                let positions: Vec<Pos> = self.dungeon.map.iter().map(|(pos, _)| pos).collect();
                for pos in positions {
                    let cell = self.dungeon.map.get_mut(pos).unwrap();
                    let glyph = match cell.terrain {
                        Terrain::Passage | Terrain::SecretPassage => Some('#'),
                        Terrain::Door
                        | Terrain::SecretDoor
                        | Terrain::SecretDoorHorizontal
                        | Terrain::SecretDoorVertical => Some('+'),
                        _ => None,
                    };
                    if let Some(glyph) = glyph {
                        cell.seen = true;
                        cell.remembered = glyph;
                        cell.wizard_revealed = matches!(
                            cell.terrain,
                            Terrain::SecretPassage
                                | Terrain::SecretDoor
                                | Terrain::SecretDoorHorizontal
                                | Terrain::SecretDoorVertical
                        );
                    }
                }
            }
            WizardCommand::Detect => {
                self.set_monster_detection(!self.player.conditions.detect_monsters)
            }
            WizardCommand::Power => self.wizard_power(),
            WizardCommand::Create
            | WizardCommand::Charge
            | WizardCommand::Identify
            | WizardCommand::GroundInventory
            | WizardCommand::List => {}
        }
    }
    fn wizard_power(&mut self) {
        for _ in 0..9 {
            self.player.stats.experience =
                crate::player::EXPERIENCE_LEVELS[(self.player.stats.level - 1) as usize];
            self.check_experience()
        }
        let sword_id = self.id();
        let mut sword = Item::basic(sword_id, ItemKind::Weapon, 5);
        sword.in_pack = self.pack_count() < MAX_PACK;
        sword.pack_letter = if sword.in_pack {
            self.next_pack_letter()
        } else {
            None
        };
        if !sword.in_pack {
            self.message(if self.options.terse {
                "ne jest ⟨n:město:gen⟩"
            } else {
                "v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩ ne jest ⟨n:město:gen⟩"
            });
        }
        sword.hit_plus = 1;
        sword.damage_plus = 1;
        let sword_at = self
            .player
            .inventory
            .iter()
            .rposition(|item| item.kind == ItemKind::Weapon)
            .map_or(self.player.inventory.len(), |index| index + 1);
        self.player.inventory.insert(sword_at, sword);
        self.player.weapon = Some(sword_id);
        let armor_id = self.id();
        let mut armor = Item::basic(armor_id, ItemKind::Armor, 7);
        armor.in_pack = self.pack_count() < MAX_PACK;
        armor.pack_letter = if armor.in_pack {
            self.next_pack_letter()
        } else {
            None
        };
        if !armor.in_pack {
            self.message(if self.options.terse {
                "ne jest ⟨n:město:gen⟩"
            } else {
                "v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩ ne jest ⟨n:město:gen⟩"
            });
        }
        armor.armor_class = Some(-5);
        armor.known = true;
        let armor_at = self
            .player
            .inventory
            .iter()
            .rposition(|item| item.kind == ItemKind::Armor)
            .map_or(self.player.inventory.len(), |index| index + 1);
        self.player.inventory.insert(armor_at, armor);
        self.player.armor = Some(armor_id)
    }
    pub fn wizard_charge(&mut self, id: u64) {
        if self.wizard
            && let Some(item) = self
                .player
                .inventory
                .iter_mut()
                .find(|i| i.id == id && i.kind == ItemKind::Stick)
        {
            item.charges = 10000
        }
    }
    pub fn wizard_create(&mut self, kind: ItemKind, which: u8) {
        self.wizard_create_blessed(kind, which, 'n')
    }
    pub fn wizard_create_bizarre(&mut self, glyph: char) {
        self.wizard_create_blessed(ItemKind::Bizarre(glyph), 0, 'n')
    }
    pub fn wizard_create_gold(&mut self, amount: i32) {
        if !self.wizard {
            return;
        }
        if self.pack_count() >= MAX_PACK {
            self.message(if self.options.terse {
                "ne jest ⟨n:město:gen⟩"
            } else {
                "v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩ ne jest ⟨n:město:gen⟩"
            });
            return;
        }
        let id = self.id();
        let mut gold = Item::gold(id, amount);
        gold.pack_letter = self.next_pack_letter();
        let name = self.inventory_name(&gold, !self.options.terse);
        let letter = gold.pack_letter.unwrap_or('?');
        self.player.inventory.push(gold);
        self.message(if self.options.terse {
            format!("{name} ({letter})")
        } else {
            format!("v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩: {name} ({letter})")
        });
    }
    pub fn wizard_create_blessed(&mut self, kind: ItemKind, which: u8, blessing: char) {
        if !self.wizard {
            return;
        }
        let max = match kind {
            ItemKind::Potion => 14,
            ItemKind::Scroll => 18,
            ItemKind::Weapon => 9,
            ItemKind::Armor => 8,
            ItemKind::Ring | ItemKind::Stick => 14,
            _ => 1,
        };
        if which >= max {
            return;
        }
        if self.pack_count() >= MAX_PACK {
            self.message(if self.options.terse {
                "ne jest ⟨n:město:gen⟩"
            } else {
                "v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩ ne jest ⟨n:město:gen⟩"
            });
            return;
        }
        let id = self.id();
        let mut item = Item::basic(id, kind, which);
        item.pack_letter = self.next_pack_letter();
        if kind == ItemKind::Amulet {
            self.has_amulet = true;
        }
        if kind == ItemKind::Weapon {
            item.count = match which {
                4 => self.rng.rnd(4) + 2,
                3 | 6 | 7 => self.rng.rnd(8) + 8,
                _ => 1,
            };
            if matches!(which, 3 | 4 | 6 | 7) {
                item.group = id;
            }
            if blessing == '-' {
                item.hit_plus = -(self.rng.rnd(3) as i16 + 1);
                item.damage_plus = -(self.rng.rnd(3) as i16 + 1);
                item.cursed = true;
            } else if blessing == '+' {
                item.hit_plus = self.rng.rnd(3) as i16 + 1;
                item.damage_plus = self.rng.rnd(3) as i16 + 1;
            }
        }
        if kind == ItemKind::Armor {
            let mut armor = crate::item::ARMOR_CLASS[which as usize];
            if blessing == '-' {
                armor += self.rng.rnd(3) as i32 + 1;
                item.cursed = true;
            } else if blessing == '+' {
                armor -= self.rng.rnd(3) as i32 + 1;
            }
            item.armor_class = Some(armor)
        }
        if kind == ItemKind::Ring {
            if matches!(which, 0 | 1 | 7 | 8) {
                item.armor_class = Some(if blessing == '-' {
                    item.cursed = true;
                    -1
                } else {
                    self.rng.rnd(2) as i32 + 1
                });
            } else if matches!(which, 6 | 11) {
                item.cursed = true;
            }
        }
        if kind == ItemKind::Stick {
            item.charges = if which == 0 {
                (self.rng.rnd(10) + 10) as i16
            } else {
                (self.rng.rnd(5) + 3) as i16
            }
        }
        let insert_at = self
            .player
            .inventory
            .iter()
            .rposition(|existing| existing.kind == item.kind)
            .map_or(self.player.inventory.len(), |index| index + 1);
        let name = self.inventory_name_case(&item, !self.options.terse, Case::Acc);
        let letter = item.pack_letter.unwrap_or('?');
        self.player.inventory.insert(insert_at, item);
        self.message(if self.options.terse {
            format!("{name} ({letter})")
        } else {
            format!("⟨v2:podbirati⟩ {name} ({letter})")
        });
    }
}

fn command_label(ch: char) -> String {
    match ch {
        ch if ch <= '\u{1f}' => format!("^{}", (b'@' + ch as u8) as char),
        '\u{7f}' => "^?".into(),
        ch => ch.to_string(),
    }
}

fn wizard_command_char(command: WizardCommand) -> char {
    match command {
        WizardCommand::Coordinates => '|',
        WizardCommand::Create => 'C',
        WizardCommand::PackCount => '$',
        WizardCommand::GroundInventory => '\u{7}',
        WizardCommand::Identify => '\u{17}',
        WizardCommand::Down => '\u{4}',
        WizardCommand::Up => '\u{1}',
        WizardCommand::Map => '\u{6}',
        WizardCommand::Teleport => '\u{14}',
        WizardCommand::Food => '\u{5}',
        WizardCommand::AddPassages => '\u{11}',
        WizardCommand::Detect => '\u{18}',
        WizardCommand::Charge => '~',
        WizardCommand::Power => '\u{9}',
        WizardCommand::List => '*',
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn door_stander_targets_player_inside_the_room() {
        // Regression: chase routing resolved door tiles passage-first, so a
        // monster standing in a doorway with the player inside the room
        // targeted the door under its own feet and never moved.
        let g = Game::new(7);
        let mut checked = 0;
        for y in 0..crate::DISPLAY_HEIGHT as i32 {
            for x in 0..crate::DISPLAY_WIDTH as i32 {
                let door = Pos::new(x, y);
                let Some(cell) = g.dungeon.map.get(door) else {
                    continue;
                };
                if cell.terrain != Terrain::Door {
                    continue;
                }
                let Some(room) = cell.room else { continue };
                // any floor cell of the same room stands in for the player
                let Some(inside) = (0..crate::DISPLAY_HEIGHT as i32)
                    .flat_map(|iy| (0..crate::DISPLAY_WIDTH as i32).map(move |ix| Pos::new(ix, iy)))
                    .find(|p| {
                        g.dungeon
                            .map
                            .get(*p)
                            .is_some_and(|c| c.room == Some(room) && c.terrain == Terrain::Floor)
                    })
                else {
                    continue;
                };
                assert_eq!(
                    g.monster_chase_target(door, inside),
                    inside,
                    "door-stander at {door:?} must head straight for {inside:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "level must contain at least one room door");
    }

    #[test]
    fn lit_room_mean_monsters_wake_within_a_few_turns() {
        // Regression: waking was a single roll at room entry; the reference
        // re-rolls every turn for every visible monster (look(true)), so a
        // mean monster in a lit room must wake almost immediately.
        let mut g = Game::new(11);
        let player_room = g
            .dungeon
            .map
            .get(g.player.pos)
            .and_then(|cell| cell.room)
            .expect("player starts in a room");
        g.dungeon.rooms[player_room as usize].dark = false;
        // a far corner of the same room, beyond the old adjacency radius
        let corner = (0..crate::DISPLAY_HEIGHT as i32)
            .flat_map(|y| (0..crate::DISPLAY_WIDTH as i32).map(move |x| Pos::new(x, y)))
            .filter(|p| {
                g.dungeon
                    .map
                    .get(*p)
                    .is_some_and(|c| c.room == Some(player_room) && c.terrain == Terrain::Floor)
            })
            .max_by_key(|p| p.distance2(g.player.pos))
            .expect("room has floor");
        assert!(
            corner.distance2(g.player.pos) > 2,
            "corner must be non-adjacent"
        );
        let id = g.id();
        let mut monster = monster::create(id, 20, corner, g.depth, &mut g.rng); // troll: MEAN
        monster.awake = false;
        assert!(monster.flags & monster::MEAN != 0);
        g.monsters.push(monster);
        let mut woke_after = None;
        for turn in 1..=15 {
            g.begin_command();
            if g.monsters.iter().find(|m| m.id == id).unwrap().awake {
                woke_after = Some(turn);
                break;
            }
        }
        assert!(
            woke_after.is_some(),
            "mean monster in a shared lit room must wake within 15 turns"
        );
    }

    use super::*;
    use crate::command::Direction;

    fn straight_test_passage(seed: u64) -> (Game, Pos) {
        let mut game = Game::new(seed);
        game.monsters.clear();
        game.floor_items.clear();
        game.wandering_countdown = 10_000;
        let start = game.player.pos;
        for dy in -1..=1 {
            for dx in -1..=4 {
                let cell = game.dungeon.map.get_mut(start.offset(dx, dy)).unwrap();
                cell.terrain = Terrain::Void;
                cell.room = None;
                cell.passage = None;
                cell.trap = None;
                cell.trap_revealed = false;
            }
        }
        for dx in 0..=3 {
            let cell = game.dungeon.map.get_mut(start.offset(dx, 0)).unwrap();
            cell.terrain = Terrain::Passage;
            cell.passage = Some(0);
            cell.seen = true;
            cell.remembered = '#';
        }
        (game, start)
    }

    #[test]
    fn pointer_travel_routes_only_over_player_known_tiles() {
        let (mut game, start) = straight_test_passage(8_001);
        let hidden = start.offset(1, 0);
        game.dungeon.map.get_mut(hidden).unwrap().seen = false;

        assert!(!game.start_travel(start.offset(3, 0)));
        assert!(!game.is_traveling());

        game.dungeon.map.get_mut(hidden).unwrap().seen = true;
        assert!(game.start_travel(start.offset(3, 0)));
        assert_eq!(game.travel_first_step(start.offset(3, 0)), Some(hidden));
    }

    #[test]
    fn pointer_travel_executes_one_ordinary_turn_per_step_and_cancels() {
        let (mut game, start) = straight_test_passage(8_002);
        let initial_turn = game.turn;
        assert!(game.start_travel(start.offset(3, 0)));

        let result = game.advance_travel();

        assert!(result.consumed_turn);
        assert_eq!(game.player.pos, start.offset(1, 0));
        assert_eq!(game.turn, initial_turn + 1);
        assert!(game.is_traveling());
        game.cancel_travel();
        assert!(!game.is_traveling());
        assert_eq!(game.advance_travel(), CommandResult::FREE);
        assert_eq!(game.player.pos, start.offset(1, 0));
    }

    #[test]
    fn pointer_travel_stops_on_traps_and_newly_visible_monsters() {
        let (mut trapped, start) = straight_test_passage(8_003);
        trapped
            .dungeon
            .map
            .get_mut(start.offset(1, 0))
            .unwrap()
            .trap = Some(Trap::Bear);
        assert!(trapped.start_travel(start.offset(3, 0)));
        trapped.advance_travel();
        assert_eq!(trapped.player.pos, start.offset(1, 0));
        assert!(!trapped.is_traveling());

        let (mut spotted, start) = straight_test_passage(8_004);
        let id = spotted.id();
        let mut monster =
            monster::create(id, 0, start.offset(2, 0), spotted.depth, &mut spotted.rng);
        monster.awake = false;
        spotted.monsters.push(monster);
        assert!(!spotted.can_see_monster(&spotted.monsters[0]));
        assert!(spotted.start_travel(start.offset(3, 0)));
        spotted.advance_travel();
        assert!(spotted.can_see_monster(&spotted.monsters[0]));
        assert!(!spotted.is_traveling());
    }

    #[test]
    fn pointer_travel_stops_after_damage_and_attacks_an_adjacent_monster() {
        let (mut damaged, start) = straight_test_passage(8_005);
        assert!(damaged.start_travel(start.offset(1, 0)));
        let ring_id = damaged.id();
        damaged
            .player
            .inventory
            .push(Item::basic(ring_id, ItemKind::Ring, 9));
        damaged.player.rings[0] = Some(ring_id);
        let before = damaged.travel_snapshot();
        let hp = damaged.player.stats.hp;
        damaged.damage_player(1);
        damaged.heal();
        assert_eq!(damaged.player.stats.hp, hp);
        assert!(damaged.travel_interrupted(&before, start, false, CommandResult::TURN));

        let (mut attacking, start) = straight_test_passage(8_006);
        let id = attacking.id();
        let monster = monster::create(
            id,
            0,
            start.offset(1, 0),
            attacking.depth,
            &mut attacking.rng,
        );
        attacking.monsters.push(monster);
        let initial_turn = attacking.turn;
        assert!(attacking.start_travel(start.offset(1, 0)));

        let result = attacking.advance_travel();

        assert!(result.consumed_turn);
        assert_eq!(attacking.player.pos, start);
        assert_eq!(attacking.turn, initial_turn + 1);
        assert!(!attacking.is_traveling());
    }

    #[test]
    fn pointer_travel_interruption_classifier_covers_every_core_stop() {
        let (base, start) = straight_test_passage(8_007);
        let before = base.travel_snapshot();
        assert!(!base.travel_interrupted(&before, start, false, CommandResult::TURN));
        assert!(base.travel_interrupted(&before, start, false, CommandResult::FREE));
        assert!(base.travel_interrupted(&before, start.offset(1, 0), false, CommandResult::TURN));
        assert!(base.travel_interrupted(&before, start, true, CommandResult::TURN));

        let mut changed = base.clone();
        changed.depth += 1;
        assert!(changed.travel_interrupted(&before, start, false, CommandResult::TURN));
        changed = base.clone();
        changed.player.stats.hp -= 1;
        assert!(changed.travel_interrupted(&before, start, false, CommandResult::TURN));
        changed = base.clone();
        changed.player.conditions.held_turns += 1;
        assert!(changed.travel_interrupted(&before, start, false, CommandResult::TURN));
        changed = base.clone();
        changed.player.conditions.asleep_turns += 1;
        assert!(changed.travel_interrupted(&before, start, false, CommandResult::TURN));
        changed = base.clone();
        changed.end = EndState::Quit;
        assert!(changed.travel_interrupted(&before, start, false, CommandResult::TURN));

        changed = base;
        assert!(changed.start_travel(start.offset(1, 0)));
        changed.new_level();
        assert!(!changed.is_traveling());
    }

    #[test]
    fn movement_consumes_a_turn() {
        let mut g = Game::new(1);
        let t = g.turn;
        g.execute(Command::Move(Direction::Left));
        assert_eq!(g.turn, t + 1)
    }

    #[test]
    fn rogueopts_parser_sets_original_boolean_string_and_inventory_options() {
        let mut options = Options::default();
        options.apply_rogue_options(
            "terse,flush,jump,noseefloor,passgo,notombstone,inven=clear,name=Rodney,fruit=mango,file=/tmp/save,score=/tmp/score,lock=/tmp/lock",
        );
        assert!(options.terse && options.fight_flush && options.jump && options.passgo);
        assert!(!options.see_floor && !options.tombstone);
        assert_eq!(options.inventory_style, InventoryStyle::Clear);
        assert_eq!(options.name, "Rodney");
        assert_eq!(options.fruit, "mango");
        assert_eq!(options.save_file, "/tmp/save");
        assert_eq!(options.score_file, "/tmp/score");
        assert_eq!(options.lock_file, "/tmp/lock");
    }

    #[test]
    fn rogueopts_preserves_reference_abbreviations_tildes_and_input_limit() {
        let mut options = Options::default();
        let long_fruit = "x".repeat(60);
        options.apply_rogue_options(&format!(
            "ter,nose,inv=c,na===Rodney,fr={long_fruit},fi=~/save"
        ));

        assert!(options.terse);
        assert!(!options.see_floor);
        assert_eq!(options.inventory_style, InventoryStyle::Clear);
        assert_eq!(options.name, "Rodney");
        assert_eq!(options.fruit, "x".repeat(50));
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        assert_eq!(
            options.save_file,
            format!("{}/save", home.trim_end_matches('/'))
        );
    }

    #[test]
    fn option_strings_discard_nonprinting_bytes() {
        assert_eq!(normalize_option_string("abc\u{7} def"), "abc def");
    }
    #[test]
    fn failed_stair_command_is_free() {
        let mut g = Game::new(2);
        g.player.pos = Pos::new(0, 1);
        let t = g.turn;
        assert!(!g.execute(Command::Down).consumed_turn);
        assert_eq!(g.turn, t)
    }
    #[test]
    fn amulet_enables_upward_journey_and_victory() {
        let mut g = Game::new(3);
        g.has_amulet = true;
        g.depth = 1;
        g.player.pos = g.dungeon.stairs;
        g.execute(Command::Up);
        assert_eq!(g.end, EndState::Won);
        assert_eq!(g.depth, 0)
    }

    #[test]
    fn stair_transitions_are_free_clear_holds_and_report_ascent() {
        let mut down = Game::new(4);
        down.player.pos = down.dungeon.stairs;
        down.player.conditions.held_turns = 5;
        down.flytrap_holder = Some(999);
        down.flytrap_hits = 4;
        let turn = down.turn;
        assert!(!down.execute(Command::Down).consumed_turn);
        assert_eq!(down.turn, turn);
        assert_eq!(down.depth, 2);
        assert_eq!(down.player.conditions.held_turns, 0);
        assert!(down.flytrap_holder.is_none());
        assert_eq!(down.flytrap_hits, 0);

        down.has_amulet = true;
        down.player.pos = down.dungeon.stairs;
        assert!(!down.execute(Command::Up).consumed_turn);
        assert_eq!(down.depth, 1);
        assert_eq!(
            down.messages.last().map(String::as_str),
            Some("čuješ siľny bolj v želųdku")
        );
    }

    #[test]
    fn original_starting_pack_is_equipped() {
        let g = Game::new(5);
        assert_eq!(g.player.inventory.len(), 5);
        assert!(g.player.weapon.is_some());
        assert_eq!(g.player.armor_class(), 6);
        assert!(
            (25..40).contains(
                &g.player
                    .inventory
                    .iter()
                    .find(|i| i.kind == ItemKind::Weapon && i.which == 3)
                    .unwrap()
                    .count
            )
        );
    }

    #[test]
    fn pack_letters_stay_stable_and_reuse_the_lowest_open_letter() {
        let mut g = Game::new(2);
        let armor = g.player.inventory[1].id;
        assert_eq!(g.player.inventory[2].pack_letter, Some('c'));

        assert_eq!(g.drop_item(armor), CommandResult::TURN);
        assert_eq!(g.player.inventory[1].pack_letter, Some('c'));
        assert_eq!(g.inventory_index_for_letter('c'), Some(1));

        assert_eq!(g.pickup(), CommandResult::TURN);
        assert_eq!(g.player.inventory.last().unwrap().pack_letter, Some('b'));
        assert_eq!(g.player.inventory[1].pack_letter, Some('c'));
    }

    #[test]
    fn picked_up_items_join_their_existing_inventory_category() {
        let mut g = Game::new(3);
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(g.player.pos);
        g.floor_items.push(potion);
        assert_eq!(g.pickup(), CommandResult::TURN);

        let mut second_food = Item::basic(g.id(), ItemKind::Food, 1);
        second_food.pos = Some(g.player.pos);
        g.floor_items.push(second_food);
        assert_eq!(g.pickup(), CommandResult::TURN);

        assert_eq!(g.player.inventory[0].kind, ItemKind::Food);
        assert_eq!(g.player.inventory[1].kind, ItemKind::Food);
        assert_eq!(g.player.inventory[1].pack_letter, Some('g'));
        assert_eq!(g.inventory_index_for_letter('c'), Some(3));
        assert_eq!(g.player.inventory.last().unwrap().kind, ItemKind::Potion);
    }

    #[test]
    fn potion_effect_and_fuse_survive_turns() {
        let mut g = Game::new(6);
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Potion, 0));
        assert!(g.quaff(id).consumed_turn);
        assert!(g.player.conditions.confused);
        assert!(
            g.scheduler
                .fuses
                .iter()
                .any(|f| f.effect == Effect::Confusion)
        );
    }

    #[test]
    fn potion_messages_and_cleansing_order_match_the_reference() {
        fn drink(game: &mut Game, which: u8) {
            let id = game.id();
            game.player
                .inventory
                .push(Item::basic(id, ItemKind::Potion, which));
            game.quaff(id);
        }

        let mut g = Game::new(61);
        g.messages.clear();
        g.player.conditions.hallucinating = true;
        drink(&mut g, 0);
        assert_eq!(g.messages.last().unwrap(), "kako divno čuťje!");

        g.messages.clear();
        drink(&mut g, 12);
        assert_eq!(g.messages.last().unwrap(), "o ne!  Vse jest temno!  Pomoć!");

        g.messages.clear();
        drink(&mut g, 9);
        assert_eq!(
            g.messages,
            [
                "hura!  Vse opęť jest kosmično",
                "vse sejčas izględaje TAKO nudno.",
                "načinaješ čuti sę mnogo lěpje",
            ]
        );

        g.player.conditions.hallucinating = true;
        g.messages.clear();
        drink(&mut g, 2);
        assert_eq!(
            g.messages,
            [
                "sejčas jest ti mnogo nedobro",
                "vse sejčas izględaje TAKO nudno."
            ]
        );

        g.player.conditions.hallucinating = true;
        g.messages.clear();
        drink(&mut g, 13);
        assert_eq!(g.messages.last().unwrap(), "o, hura!  Letiš v vȯzduhu!");
    }

    #[test]
    fn raise_level_potion_uses_the_reference_experience_value() {
        let mut g = Game::new(62);
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Potion, 8));

        g.quaff(id);

        assert_eq!(g.player.stats.level, 2);
        assert_eq!(
            g.messages[g.messages.len() - 2..],
            [
                "naglo vse dělaješ mnogo bolje umělo",
                "dostigaješ stųpene 2"
            ]
        );
        assert_eq!(
            g.player.stats.experience,
            crate::player::EXPERIENCE_LEVELS[0] + 1
        );
    }

    #[test]
    fn raise_level_potion_preserves_the_level_twenty_one_sentinel_behavior() {
        let mut g = Game::new(211);
        g.player.stats.level = 21;
        g.player.stats.experience = crate::player::EXPERIENCE_LEVELS[19] + 1;
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Potion, 8));

        g.quaff(id);

        assert_eq!(g.player.stats.experience, 1);
        assert_eq!(g.player.stats.level, 1);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("naglo vse dělaješ mnogo bolje umělo")
        );
    }

    #[test]
    fn scroll_messages_preserve_singular_and_hallucinated_forms() {
        fn read(game: &mut Game, which: u8) {
            let id = game.id();
            game.player
                .inventory
                .push(Item::basic(id, ItemKind::Scroll, which));
            game.read_scroll(id);
        }

        let mut g = Game::new(63);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut monster = monster::create(g.id(), 0, pos, g.depth, &mut g.rng);
        monster.awake = true;
        g.monsters.push(monster);
        g.messages.clear();
        read(&mut g, 2);
        assert_eq!(g.messages.last().unwrap(), "čudovišče ne može dvigati sę");

        g.player.conditions.hallucinating = true;
        g.messages.clear();
        read(&mut g, 0);
        assert!(crate::lang::COLOR_ADJ.iter().any(|color| {
            g.messages.last().unwrap()
                == &format!(
                    "tvoje rųky načinajųt světiti sę {}",
                    crate::lang::color_adv(color)
                )
        }));

        g.messages.clear();
        read(&mut g, 15);
        assert_eq!(g.messages.last().unwrap(), "čuješ jedinstvo s Vsemirom");

        g.messages.clear();
        read(&mut g, 5);
        assert_eq!(
            g.messages.last().unwrap(),
            "toj svitȯk jest svitȯk za opoznańje napitkov"
        );
    }

    #[test]
    fn cursed_equipment_cannot_be_removed() {
        let mut g = Game::new(7);
        let id = g.player.armor.unwrap();
        g.player
            .inventory
            .iter_mut()
            .find(|i| i.id == id)
            .unwrap()
            .cursed = true;
        assert!(g.take_off().consumed_turn);
        assert_eq!(g.player.armor, Some(id));
    }

    #[test]
    fn reference_invalid_and_cursed_equipment_attempts_still_consume_turns() {
        let mut g = Game::new(70);
        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));
        g.player.armor = None;
        assert_eq!(g.wear(food), CommandResult::TURN);
        assert_eq!(g.put_on_ring(food, 0), CommandResult::TURN);

        let weapon = g.player.weapon.unwrap();
        g.player
            .inventory
            .iter_mut()
            .find(|item| item.id == weapon)
            .unwrap()
            .cursed = true;
        assert_eq!(g.wield(food), CommandResult::TURN);
        assert_eq!(g.throw_item(weapon, Direction::Right), CommandResult::TURN);
        assert_eq!(g.drop_item(weapon), CommandResult::TURN);
        assert_eq!(g.player.weapon, Some(weapon));
        assert!(g.player.inventory.iter().any(|item| item.id == weapon));
    }

    #[test]
    fn successful_equipment_messages_include_reference_item_names_and_letters() {
        let mut g = Game::new(71);
        g.player.weapon = None;
        let weapon = g
            .player
            .inventory
            .iter()
            .find(|item| item.kind == ItemKind::Weapon)
            .unwrap()
            .id;
        let letter = g
            .player
            .inventory
            .iter()
            .find(|item| item.id == weapon)
            .unwrap()
            .pack_letter
            .unwrap();

        assert_eq!(g.wield(weapon), CommandResult::TURN);
        assert!(g.messages.last().unwrap().starts_with("sejčas dŕžiš "));
        assert!(g.messages.last().unwrap().ends_with(&format!("({letter})")));
    }

    #[test]
    fn revealed_trap_is_recorded_after_trigger() {
        let mut g = Game::new(8);
        let target = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .map(|(dx, dy)| g.player.pos.offset(dx, dy))
            .find(|p| g.passable(*p))
            .unwrap();
        g.dungeon.map.get_mut(target).unwrap().trap = Some(Trap::SleepGas);
        g.player.pos = target;
        g.trigger_trap();
        assert!(g.dungeon.map.get(target).unwrap().trap_revealed);
        assert!(g.player.conditions.asleep_turns > 0);
    }

    #[test]
    fn revealed_traps_render_and_remain_in_map_memory() {
        let mut g = Game::new(2270);
        let target = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(target).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::Bear);
        cell.trap_revealed = true;
        cell.seen = true;
        cell.remembered = '.';

        assert_eq!(g.glyph_at(target), '^');

        g.update_visibility();
        assert_eq!(g.dungeon.map.get(target).unwrap().remembered, '^');
        assert_eq!(g.glyph_at(target), '^');
    }

    #[test]
    fn sleep_duration_starts_with_the_next_skipped_command() {
        let mut g = Game::new(9);
        g.monsters.clear();
        g.player.conditions.asleep_turns = 2;
        g.player_is_running = false;

        g.after_turn();
        assert_eq!(g.player.conditions.asleep_turns, 2);

        assert_eq!(g.execute(Command::Search), CommandResult::TURN);
        assert_eq!(g.player.conditions.asleep_turns, 1);
        assert_eq!(g.execute(Command::Search), CommandResult::TURN);
        assert_eq!(g.player.conditions.asleep_turns, 0);
        assert!(g.player_is_running);
        assert_eq!(g.messages.last().unwrap(), "možeš opęť dvigati sę");
    }

    #[test]
    fn mysterious_trap_messages_use_the_original_color_text() {
        let mut saw_colored_message = false;
        for seed in 0..200 {
            let mut g = Game::new(seed);
            g.dungeon.map.get_mut(g.player.pos).unwrap().trap = Some(Trap::Mysterious);
            g.trigger_trap();
            let message = g.messages.last().unwrap();
            let fixed = [
                "naglo jesi v paraleľnom světu",
                "čuješ ubod v šiji",
                "pestre linije tancujųt okolo tebe i izčezajųt",
                "strěla leti mimo tvojego uha!",
                "naglo čuješ velikų žęđų",
                "čas naglo běži bystrěje",
                "čas sejčas běži pomalo",
            ];
            let colored = crate::lang::COLOR_ADJ.iter().any(|color| {
                let n = crate::lang::color_adv(color);
                let f = crate::lang::adj_for(
                    color,
                    &crate::lang::lex(
                        "iskra",
                        interslavic::Gender::Feminine,
                        interslavic::Animacy::Inanimate,
                    ),
                    Case::Nom,
                    interslavic::Number::Plural,
                );
                let f_sg = crate::lang::adj_for(
                    color,
                    &crate::lang::lex(
                        "torba",
                        interslavic::Gender::Feminine,
                        interslavic::Animacy::Inanimate,
                    ),
                    Case::Nom,
                    interslavic::Number::Singular,
                );
                message == &format!("světlo tu naglo izględaje {n}")
                    || message == &format!("{n} světlo světi v tvoje oči")
                    || message == &format!("{f} iskry tancujųt po tvojej brȯnji")
                    || message == &format!("tvoja torba staje sę {f_sg}!")
            });
            assert!(fixed.contains(&message.as_str()) || colored, "{message}");
            saw_colored_message |= colored;
        }
        assert!(saw_colored_message);
    }

    #[test]
    fn teleport_accepts_items_stairs_and_hidden_traps_like_find_floor() {
        let mut g = Game::new(10);
        g.monsters.clear();
        g.floor_items.clear();
        let target = g
            .dungeon
            .map
            .iter()
            .find_map(|(pos, cell)| {
                (pos != g.player.pos && cell.room.is_some() && g.is_room_base_terrain(cell))
                    .then_some(pos)
            })
            .unwrap();
        let teleport_squares: Vec<Pos> = g
            .dungeon
            .rooms
            .iter()
            .filter(|room| !room.gone)
            .flat_map(|room| g.teleport_positions(room.id))
            .collect();
        for pos in teleport_squares {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Void;
        }
        let target_cell = g.dungeon.map.get_mut(target).unwrap();
        target_cell.terrain = Terrain::Stairs;
        target_cell.trap = Some(Trap::Bear);
        let mut item = Item::basic(g.id(), ItemKind::Food, 0);
        item.pos = Some(target);
        g.floor_items.push(item);

        g.teleport_player();

        assert_eq!(g.player.pos, target);
        let cell = g.dungeon.map.get(target).unwrap();
        assert_eq!(cell.terrain, Terrain::Stairs);
        assert_eq!(cell.trap, Some(Trap::Bear));
        assert!(g.floor_items.iter().any(|item| item.pos == Some(target)));
    }

    #[test]
    fn player_teleport_uses_reference_room_and_coordinate_retry_rng() {
        let mut g = Game::new(215);
        let mut expected = g.clone();
        let destination = expected.reference_monster_floor_position().unwrap();

        g.teleport_player();

        assert_eq!(g.player.pos, destination);
        assert_eq!(g.rng, expected.rng);
    }

    #[test]
    fn populated_items_never_overlap_generated_traps() {
        for seed in 1..500 {
            let mut g = Game::new(seed);
            g.depth = 30;
            g.new_level();
            assert!(g.floor_items.iter().all(|item| {
                item.pos.is_none_or(|pos| {
                    g.dungeon
                        .map
                        .get(pos)
                        .is_none_or(|cell| cell.trap.is_none())
                })
            }));
        }
    }

    #[test]
    fn rust_reports_protection_and_uses_the_verbose_original_message() {
        let mut g = Game::new(11);
        let armor_id = g.player.armor.unwrap();
        let armor = g
            .player
            .inventory
            .iter_mut()
            .find(|item| item.id == armor_id)
            .unwrap();
        armor.protected = true;
        let original_ac = armor.armor_class;

        g.rust_armor();
        assert_eq!(g.messages.last().unwrap(), "rđa naglo izčezaje");
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == armor_id)
                .unwrap()
                .armor_class,
            original_ac
        );

        g.player
            .inventory
            .iter_mut()
            .find(|item| item.id == armor_id)
            .unwrap()
            .protected = false;
        g.rust_armor();
        assert_eq!(
            g.messages.last().unwrap(),
            "tvoja brȯnja sejčas izględaje slaběje. O ne!"
        );
    }

    #[test]
    fn cancellation_stick_cancels_first_monster_in_line() {
        let mut g = Game::new(10);
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mid = g.id();
        let mut phantom = monster::create(mid, 15, pos, g.depth, &mut g.rng);
        phantom.flags |= monster::CONFUSED;
        g.monsters = vec![phantom];
        g.player.conditions.see_invisible = true;
        let sid = g.id();
        let mut stick = Item::basic(sid, ItemKind::Stick, 13);
        stick.charges = 1;
        g.player.inventory.push(stick);
        assert!(g.zap(sid, Direction::Right).consumed_turn);
        assert_ne!(g.monsters[0].flags & monster::CANCELLED, 0);
        assert_ne!(g.monsters[0].flags & monster::CONFUSED, 0);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|i| i.id == sid)
                .unwrap()
                .charges,
            0
        );
    }

    #[test]
    fn targeted_stick_passes_over_an_undetected_invisible_monster() {
        let mut g = Game::new(210);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let phantom_id = g.id();
        g.monsters
            .push(monster::create(phantom_id, 15, pos, g.depth, &mut g.rng));
        let stick_id = g.id();
        let mut cancellation = Item::basic(stick_id, ItemKind::Stick, 13);
        cancellation.charges = 1;
        g.player.inventory.push(cancellation);

        g.zap(stick_id, Direction::Right);

        assert_eq!(g.monsters[0].flags & monster::CANCELLED, 0);
        assert_ne!(g.monsters[0].flags & monster::INVISIBLE, 0);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == stick_id)
                .unwrap()
                .charges,
            0
        );
    }

    #[test]
    fn targeting_flytrap_releases_hold_without_resetting_constriction_damage() {
        let mut g = Game::new(1001);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let flytrap_id = g.id();
        g.monsters
            .push(monster::create(flytrap_id, 5, pos, g.depth, &mut g.rng));
        g.flytrap_holder = Some(flytrap_id);
        g.flytrap_hits = 4;
        let stick_id = g.id();
        let mut stick = Item::basic(stick_id, ItemKind::Stick, 13);
        stick.charges = 1;
        g.player.inventory.push(stick);

        g.zap(stick_id, Direction::Right);

        assert_eq!(g.flytrap_holder, None);
        assert_eq!(g.flytrap_hits, 4);
        assert_ne!(g.monsters[0].flags & monster::CANCELLED, 0);
    }

    #[test]
    fn light_stick_handles_corridors_and_terse_room_messages() {
        let mut corridor = Game::new(12);
        let stick = corridor.id();
        let mut light = Item::basic(stick, ItemKind::Stick, 0);
        light.charges = 2;
        corridor.player.inventory.push(light);
        let cell = corridor.dungeon.map.get_mut(corridor.player.pos).unwrap();
        cell.room = None;
        cell.passage = Some(1);
        cell.terrain = Terrain::Passage;

        corridor.zap(stick, Direction::Right);

        assert_eq!(
            corridor.messages.last().unwrap(),
            "koridor světi sę i potom gasne"
        );
        assert!(corridor.knowledge.sticks[0]);

        let mut room = Game::new(13);
        room.options.terse = true;
        let stick = room.id();
        let mut light = Item::basic(stick, ItemKind::Stick, 0);
        light.charges = 1;
        room.player.inventory.push(light);
        room.zap(stick, Direction::Right);
        assert_eq!(room.messages.last().unwrap(), "komnata jest osvětljena");
    }

    #[test]
    fn magic_missile_hits_without_vanishing_and_uses_weapon_bonus() {
        let mut g = Game::new(14);
        g.monsters.clear();
        let target = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(target).unwrap().terrain = Terrain::Floor;
        let monster_id = g.id();
        let mut victim = monster::create(monster_id, 0, target, g.depth, &mut g.rng);
        victim.level = -100;
        victim.hp = 100;
        victim.max_hp = 100;
        g.monsters.push(victim);
        let weapon = g.player.weapon.unwrap();
        g.player
            .inventory
            .iter_mut()
            .find(|item| item.id == weapon)
            .unwrap()
            .damage_plus = 20;
        g.player.conditions.can_confuse_monster = true;
        let stick = g.id();
        let mut missile = Item::basic(stick, ItemKind::Stick, 6);
        missile.charges = 1;
        g.player.inventory.push(missile);
        g.messages.clear();

        g.zap(stick, Direction::Right);

        assert!(g.monsters[0].hp <= 78);
        assert_ne!(g.monsters[0].flags & monster::CONFUSED, 0);
        assert_eq!(
            g.messages.first().map(String::as_str),
            Some("udarjaješ akvatora")
        );
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("akvator izględaje smųćeno")
        );
        assert!(
            g.messages
                .iter()
                .all(|message| !message.contains("vanishes"))
        );
    }

    #[test]
    fn drain_life_only_affects_the_current_passage() {
        let mut g = Game::new(15);
        g.monsters.clear();
        g.player.stats.hp = 20;
        let player = g.player.pos;
        let same = player.offset(1, 0);
        let other = player.offset(0, 1);
        for (pos, passage) in [(player, 1), (same, 1), (other, 2)] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Passage;
            cell.room = None;
            cell.passage = Some(passage);
        }
        for pos in [same, other] {
            let id = g.id();
            let mut monster = monster::create(id, 0, pos, g.depth, &mut g.rng);
            monster.hp = 100;
            monster.max_hp = 100;
            g.monsters.push(monster);
        }

        g.drain_life();

        assert_eq!(g.player.stats.hp, 10);
        assert_eq!(g.monsters[0].hp, 90);
        assert_eq!(g.monsters[1].hp, 100);
    }

    #[test]
    fn teleport_away_stick_accepts_items_stairs_and_hidden_traps() {
        let mut g = Game::new(16);
        g.monsters.clear();
        g.floor_items.clear();
        let target = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(target).unwrap().terrain = Terrain::Floor;
        let monster_id = g.id();
        g.monsters
            .push(monster::create(monster_id, 0, target, g.depth, &mut g.rng));
        let destination = g
            .dungeon
            .rooms
            .iter()
            .filter(|room| !room.gone)
            .flat_map(|room| g.teleport_positions(room.id))
            .find(|pos| *pos != g.player.pos && *pos != target)
            .unwrap();
        g.monsters[0].destination = Some(destination);
        let teleport_squares: Vec<_> = g
            .dungeon
            .rooms
            .iter()
            .filter(|room| !room.gone)
            .flat_map(|room| g.teleport_positions(room.id))
            .collect();
        for pos in teleport_squares {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Void;
        }
        let cell = g.dungeon.map.get_mut(destination).unwrap();
        cell.terrain = Terrain::Stairs;
        cell.trap = Some(Trap::Bear);
        let mut food = Item::basic(g.id(), ItemKind::Food, 0);
        food.pos = Some(destination);
        g.floor_items.push(food);
        let stick = g.id();
        let mut teleport = Item::basic(stick, ItemKind::Stick, 11);
        teleport.charges = 1;
        g.player.inventory.push(teleport);

        g.zap(stick, Direction::Right);

        assert_eq!(g.monsters[0].pos, destination);
        assert_eq!(g.monsters[0].destination, None);
    }

    #[test]
    fn teleport_to_redirects_an_item_seeking_monster_to_the_player() {
        let mut g = Game::new(1601);
        g.monsters.clear();
        let target = g.player.pos.offset(2, 0);
        let near = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(near).unwrap().terrain = Terrain::Floor;
        g.dungeon.map.get_mut(target).unwrap().terrain = Terrain::Floor;
        let monster_id = g.id();
        let mut monster = monster::create(monster_id, 14, target, g.depth, &mut g.rng);
        monster.destination = Some(g.dungeon.stairs);
        g.monsters.push(monster);
        let stick_id = g.id();
        let mut stick = Item::basic(stick_id, ItemKind::Stick, 12);
        stick.charges = 1;
        g.player.inventory.push(stick);

        g.zap(stick_id, Direction::Right);

        assert_eq!(g.monsters[0].pos, near);
        assert!(g.monsters[0].awake);
        assert_eq!(g.monsters[0].destination, None);
    }

    #[test]
    fn polymorph_is_not_identified_when_the_replacement_is_invisible() {
        let seed = (1..1000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(26) == 15
            })
            .unwrap();
        let mut g = Game::new(17);
        g.monsters.clear();
        let target = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(target).unwrap().terrain = Terrain::Floor;
        let monster_id = g.id();
        g.monsters
            .push(monster::create(monster_id, 0, target, g.depth, &mut g.rng));
        let stick = g.id();
        let mut polymorph = Item::basic(stick, ItemKind::Stick, 5);
        polymorph.charges = 1;
        g.player.inventory.push(polymorph);
        g.rng = GameRng::new(seed);

        g.zap(stick, Direction::Right);

        assert_eq!(g.monsters[0].kind, 15);
        assert!(!g.can_see_monster(&g.monsters[0]));
        assert!(!g.knowledge.sticks[5]);
    }

    #[test]
    fn throwing_from_a_stack_removes_one_item() {
        let mut g = Game::new(11);
        let arrows = g
            .player
            .inventory
            .iter()
            .find(|i| i.kind == ItemKind::Weapon && i.which == 3)
            .unwrap()
            .id;
        let before = g
            .player
            .inventory
            .iter()
            .find(|i| i.id == arrows)
            .unwrap()
            .count;
        g.throw_item(arrows, Direction::Right);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|i| i.id == arrows)
                .unwrap()
                .count,
            before - 1
        );
    }
    #[test]
    fn levels_place_collectable_gold() {
        let mut found = false;
        for seed in 1..100 {
            let g = Game::new(seed);
            if g.floor_items
                .iter()
                .any(|i| i.kind == ItemKind::Gold && i.gold_value >= 2)
            {
                found = true;
                break;
            }
        }
        assert!(found)
    }
    #[test]
    fn remembered_remote_monster_is_not_revealed() {
        let mut g = Game::new(15);
        let p = g.player.pos;
        let remote = g
            .dungeon
            .rooms
            .iter()
            .find(|r| !r.gone && r.id != g.dungeon.map.get(p).and_then(|c| c.room).unwrap_or(255))
            .unwrap()
            .center();
        g.dungeon.map.get_mut(remote).unwrap().seen = true;
        let id = g.id();
        g.monsters = vec![monster::create(id, 0, remote, g.depth, &mut g.rng)];
        assert_ne!(g.glyph_at(remote), 'A');
        g.player.conditions.detect_monsters = true;
        assert_eq!(g.glyph_at(remote), 'A')
    }

    #[test]
    fn detection_shows_true_type_for_unseen_xerocs_and_invisible_monsters() {
        let mut g = Game::new(189);
        g.monsters.clear();
        g.player.pos = Pos::new(10, 10);
        let xeroc_pos = Pos::new(40, 10);
        let phantom_pos = Pos::new(11, 10);
        let xeroc_id = g.id();
        let phantom_id = g.id();
        g.monsters.push(monster::create(
            xeroc_id, 23, xeroc_pos, g.depth, &mut g.rng,
        ));
        g.monsters.push(monster::create(
            phantom_id,
            15,
            phantom_pos,
            g.depth,
            &mut g.rng,
        ));
        g.player.conditions.detect_monsters = true;

        assert_eq!(g.glyph_at(xeroc_pos), 'X');
        assert_eq!(g.glyph_at(phantom_pos), 'P');
    }
    #[test]
    fn identify_scroll_restricts_and_records_item_type() {
        let mut g = Game::new(16);
        let sid = g.id();
        g.player
            .inventory
            .push(Item::basic(sid, ItemKind::Scroll, 5));
        g.read_scroll(sid);
        let food = g
            .player
            .inventory
            .iter()
            .find(|i| i.kind == ItemKind::Food)
            .unwrap()
            .id;
        assert!(!g.identify_item(food).consumed_turn);
        assert!(g.pending_identification.is_some());
        let pid = g.id();
        g.player
            .inventory
            .push(Item::basic(pid, ItemKind::Potion, 3));
        g.identify_item(pid);
        assert!(g.knowledge.potions[3]);
        assert!(g.pending_identification.is_none())
    }
    #[test]
    fn protection_and_strength_rings_apply_and_remove_bonuses() {
        let mut g = Game::new(17);
        let protection = g.id();
        let mut p = Item::basic(protection, ItemKind::Ring, 0);
        p.armor_class = Some(2);
        g.player.inventory.push(p);
        let base = g.player.armor_class();
        g.put_on_ring(protection, 0);
        assert_eq!(g.player.armor_class(), base - 2);
        let strength = g.id();
        let mut s = Item::basic(strength, ItemKind::Ring, 1);
        s.armor_class = Some(2);
        g.player.inventory.push(s);
        let before = g.player.stats.strength;
        g.put_on_ring(strength, 1);
        assert_eq!(g.player.stats.strength, before + 2);
        g.remove_ring(1);
        assert_eq!(g.player.stats.strength, before)
    }
    #[test]
    fn duplicate_searching_rings_search_once_per_hand() {
        let seed = (1..1000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(2) != 0 && rng.rnd(2) == 0
            })
            .unwrap();
        let mut g = Game::new(226);
        g.player.inventory.clear();
        g.player.rings = [None, None];
        let hidden = g.player.pos.offset(-1, -1);
        let cell = g.dungeon.map.get_mut(hidden).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::Bear);
        cell.trap_revealed = false;
        for hand in 0..2 {
            let id = g.id();
            g.player.inventory.push(Item::basic(id, ItemKind::Ring, 3));
            g.player.rings[hand] = Some(id);
        }
        g.rng = GameRng::new(seed);

        g.automatic_ring_checks();

        assert!(g.dungeon.map.get(hidden).unwrap().trap_revealed);
    }

    #[test]
    fn duplicate_teleportation_rings_roll_once_per_hand_in_order() {
        let template = Game::new(227);
        let start = template.player.pos;
        let seed = (1..100_000)
            .find(|seed| {
                let mut probe = template.clone();
                probe.rng = GameRng::new(*seed);
                if probe.rng.rnd(50) == 0 || probe.rng.rnd(50) != 0 {
                    return false;
                }
                probe.random_teleport();
                probe.player.pos != start
            })
            .unwrap();
        let mut actual = template;
        actual.player.inventory.clear();
        actual.player.rings = [None, None];
        for hand in 0..2 {
            let id = actual.id();
            actual
                .player
                .inventory
                .push(Item::basic(id, ItemKind::Ring, 11));
            actual.player.rings[hand] = Some(id);
        }
        actual.rng = GameRng::new(seed);
        let mut expected = actual.clone();
        for _ in 0..2 {
            if expected.rng.rnd(50) == 0 {
                expected.random_teleport();
            }
        }

        actual.automatic_ring_checks();

        assert_eq!(actual.player.pos, expected.player.pos);
        assert_eq!(actual.rng, expected.rng);
        assert_ne!(actual.player.pos, start);
    }
    #[test]
    fn aggravation_ring_wakes_monsters_created_while_it_is_worn() {
        let mut g = Game::new(171);
        let ring = g.id();
        g.player
            .inventory
            .push(Item::basic(ring, ItemKind::Ring, 6));
        g.put_on_ring(ring, 0);

        let monster = g.make_monster(25, g.player.pos.offset(5, 0), g.depth, false, false);

        assert!(monster.awake);
    }
    #[test]
    fn slowed_monster_moves_on_alternate_turns() {
        let mut g = Game::new(18);
        g.monsters.clear();
        let start = g.player.pos.offset(5, 0);
        for dx in 1..=5 {
            g.dungeon
                .map
                .get_mut(g.player.pos.offset(dx, 0))
                .unwrap()
                .terrain = Terrain::Floor
        }
        let id = g.id();
        let mut m = monster::create(id, 25, start, g.depth, &mut g.rng);
        m.awake = true;
        m.flags |= monster::SLOWED;
        m.turn = false;
        g.monsters.push(m);
        g.move_monsters();
        assert_eq!(g.monsters[0].pos, start);
        g.move_monsters();
        assert_ne!(g.monsters[0].pos, start)
    }
    #[test]
    fn slowed_hasted_monster_alternates_one_and_two_scheduled_moves() {
        let mut g = Game::new(181);
        let mut monster = monster::create(g.id(), 25, g.player.pos, g.depth, &mut g.rng);
        monster.flags |= monster::SLOWED | monster::HASTED;
        monster.turn = false;

        assert_eq!(Game::scheduled_monster_steps(&mut monster), 1);
        assert_eq!(Game::scheduled_monster_steps(&mut monster), 2);
        assert_eq!(Game::scheduled_monster_steps(&mut monster), 1);
    }

    #[test]
    fn slowed_flyer_runs_the_full_scheduler_twice_each_world_turn() {
        let mut g = Game::new(1810);
        g.monsters.clear();
        g.floor_items.clear();
        g.player.pos = Pos::new(30, 12);
        let start = Pos::new(40, 12);
        for x in 30..=40 {
            let cell = g.dungeon.map.get_mut(Pos::new(x, 12)).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(0);
            cell.passage = None;
        }
        let mut griffin = monster::create(g.id(), 6, start, g.depth, &mut g.rng);
        griffin.awake = true;
        griffin.flags |= monster::SLOWED;
        griffin.turn = false;
        g.monsters.push(griffin);

        g.move_monsters();
        assert_eq!(g.monsters[0].pos, start.offset(-1, 0));
        assert!(!g.monsters[0].turn);
        g.move_monsters();
        assert_eq!(g.monsters[0].pos, start.offset(-2, 0));
        assert!(!g.monsters[0].turn);
    }

    #[test]
    fn hasted_flyer_can_move_four_times_in_one_world_turn() {
        let mut g = Game::new(1811);
        g.monsters.clear();
        g.floor_items.clear();
        g.player.pos = Pos::new(30, 12);
        let start = Pos::new(40, 12);
        for x in 30..=40 {
            let cell = g.dungeon.map.get_mut(Pos::new(x, 12)).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(0);
            cell.passage = None;
        }
        let mut griffin = monster::create(g.id(), 6, start, g.depth, &mut g.rng);
        griffin.awake = true;
        griffin.flags |= monster::HASTED;
        g.monsters.push(griffin);

        g.move_monsters();

        assert_eq!(g.monsters[0].pos, start.offset(-4, 0));
        assert!(g.monsters[0].turn);
    }

    #[test]
    fn chasing_monster_does_not_move_farther_when_no_nonworsening_step_exists() {
        let mut g = Game::new(182);
        g.monsters.clear();
        g.player.pos = Pos::new(35, 12);
        let from = Pos::new(40, 12);
        for dy in -1..=1 {
            for dx in -1..=1 {
                g.dungeon.map.get_mut(from.offset(dx, dy)).unwrap().terrain = Terrain::Void;
            }
        }
        g.dungeon.map.get_mut(from).unwrap().terrain = Terrain::Floor;
        g.dungeon.map.get_mut(from.offset(1, 0)).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        let mut monster = monster::create(id, 25, from, g.depth, &mut g.rng);
        monster.awake = true;
        g.monsters.push(monster);

        g.move_monster_step(0);

        assert_eq!(g.monsters[0].pos, from);
    }

    #[test]
    fn chase_uses_reference_reservoir_order_for_equally_close_steps() {
        let mut g = Game::new(183);
        g.monsters.clear();
        g.floor_items.clear();
        let from = Pos::new(40, 12);
        let destination = from.offset(2, 2);
        let down = from.offset(0, 1);
        let right = from.offset(1, 0);
        for dy in -1..=2 {
            for dx in -1..=2 {
                let cell = g.dungeon.map.get_mut(from.offset(dx, dy)).unwrap();
                cell.terrain = Terrain::Void;
                cell.room = None;
                cell.passage = None;
            }
        }
        for pos in [from, destination, down, right] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(0);
        }
        g.player.pos = destination;
        let mut monster = monster::create(g.id(), 25, from, g.depth, &mut g.rng);
        monster.awake = true;
        g.monsters.push(monster);
        let mut expected_rng = g.rng;
        let expected = if expected_rng.rnd(2) == 0 {
            right
        } else {
            down
        };

        g.move_monster_step(0);

        assert_eq!(g.monsters[0].pos, expected);
        assert_eq!(g.rng, expected_rng);
    }

    #[test]
    fn chase_routes_to_the_nearest_recorded_room_exit() {
        let mut g = Game::new(1830);
        let room = g.dungeon.rooms.iter().find(|room| !room.gone).unwrap().id;
        let from = Pos::new(40, 12);
        let destination = Pos::new(60, 12);
        let far_exit = Pos::new(39, 10);
        let near_exit = Pos::new(45, 12);
        for (pos, room_id) in [(from, room), (destination, (room + 1) % 9)] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(room_id);
            cell.passage = None;
        }
        g.dungeon.rooms[room as usize].exits = vec![far_exit, near_exit];

        assert_eq!(g.monster_chase_target(from, destination), near_exit);
    }

    #[test]
    fn reference_regen_flag_does_not_heal_monsters() {
        let mut g = Game::new(184);
        g.monsters.clear();
        g.player.pos = Pos::new(35, 12);
        let from = Pos::new(40, 12);
        for x in 35..=40 {
            g.dungeon.map.get_mut(Pos::new(x, 12)).unwrap().terrain = Terrain::Floor;
        }
        let id = g.id();
        let mut troll = monster::create(id, 19, from, g.depth, &mut g.rng);
        troll.awake = true;
        troll.max_hp = 20;
        troll.hp = 10;
        g.monsters.push(troll);

        g.move_monsters();

        assert_eq!(g.monsters[0].hp, 10);
    }
    #[test]
    fn wizard_commands_require_activation_and_power_up() {
        let mut g = Game::new(19);
        let level = g.depth;
        g.execute(Command::Wizard(WizardCommand::Down));
        assert_eq!(g.depth, level);
        g.set_wizard(true);
        g.execute(Command::Wizard(WizardCommand::Down));
        assert_eq!(g.depth, level + 1);
        g.execute(Command::Wizard(WizardCommand::Power));
        assert!(g.player.stats.level >= 10);
        assert!(g.no_score)
    }

    #[test]
    fn wizard_power_keeps_failed_full_pack_additions_equipped_but_outside_the_pack() {
        let mut g = Game::new(1900);
        g.set_wizard(true);
        g.player.inventory.clear();
        let mut potions = Item::basic(g.id(), ItemKind::Potion, 0);
        potions.count = MAX_PACK as u32;
        potions.pack_letter = Some('a');
        g.player.inventory.push(potions);

        g.wizard_power();

        assert_eq!(g.pack_count(), MAX_PACK);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .filter(|item| !item.in_pack)
                .count(),
            2
        );
        assert_eq!(g.player.armor_class(), -5);
        assert!(
            g.player
                .weapon
                .is_some_and(|id| g.player.inventory.iter().any(|item| item.id == id))
        );
        assert_eq!(
            g.messages
                .iter()
                .filter(|message| message.as_str() == "v tvojej torbě ne jest města")
                .count(),
            2
        );
    }

    #[test]
    fn wizard_activation_reports_the_reference_dungeon_number() {
        let mut g = Game::new(0x1_0000_0013);
        g.set_wizard(true);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("naglo znaješ vse, tako kako Ken Arnold, o temnici #19")
        );
        assert!(g.no_score);
        assert!(g.player.conditions.detect_monsters);
    }

    #[test]
    fn startup_wizard_enables_detection_without_the_command_activation_message() {
        let mut g = Game::new(190);
        let message = g.messages.last().cloned();

        g.set_startup_wizard();

        assert!(g.wizard && g.no_score && g.player.conditions.detect_monsters);
        assert_eq!(g.messages.last(), message.as_ref());
    }

    #[test]
    fn enabling_detection_redraws_every_hallucinated_monster_in_rng_order() {
        let mut g = Game::new(191);
        g.monsters.clear();
        for (kind, offset) in [(23, 1), (0, 2), (12, 3)] {
            let pos = g.player.pos.offset(offset, 0);
            let id = g.id();
            g.monsters
                .push(monster::create(id, kind, pos, g.depth, &mut g.rng));
        }
        g.player.conditions.hallucinating = true;
        g.player.conditions.detect_monsters = false;
        let mut expected_rng = g.rng;
        let expected: Vec<_> = g
            .monsters
            .iter()
            .map(|monster| (monster.id, (b'A' + expected_rng.rnd(26) as u8) as char))
            .collect();

        g.set_monster_detection(true);

        assert_eq!(g.hallucinated_monsters, expected);
        assert_eq!(g.rng, expected_rng);
        assert!(
            g.hallucinated_monsters
                .iter()
                .all(|(_, glyph)| glyph.is_ascii_uppercase())
        );
    }

    #[test]
    fn wizard_up_can_generate_level_zero_without_winning() {
        let mut g = Game::new(202);
        g.set_wizard(true);
        g.depth = 1;
        g.execute(Command::Wizard(WizardCommand::Up));
        assert_eq!(g.depth, 0);
        assert_eq!(g.end, EndState::Playing);
        g.execute(Command::Wizard(WizardCommand::Up));
        assert_eq!(g.depth, 0);
        assert_eq!(g.end, EndState::Playing);
    }

    #[test]
    fn wizard_map_command_does_not_change_map_or_memory() {
        let mut g = Game::new(1919);
        g.set_wizard(true);
        let before = g.dungeon.clone();
        g.execute(Command::Wizard(WizardCommand::Map));
        assert_eq!(g.dungeon, before);
    }

    #[test]
    fn wizard_add_passages_reveals_without_opening_secret_terrain() {
        let mut g = Game::new(19190);
        g.set_wizard(true);
        let secret_door = Pos::new(1, 1);
        let secret_passage = Pos::new(78, 21);
        g.dungeon.map.get_mut(secret_door).unwrap().terrain = Terrain::SecretDoorHorizontal;
        g.dungeon.map.get_mut(secret_passage).unwrap().terrain = Terrain::SecretPassage;

        g.execute(Command::Wizard(WizardCommand::AddPassages));

        let door = g.dungeon.map.get(secret_door).unwrap();
        assert_eq!(door.terrain, Terrain::SecretDoorHorizontal);
        assert!(!door.terrain.passable());
        assert!(door.seen && door.wizard_revealed);
        assert_eq!(g.glyph_at(secret_door), '+');
        let passage = g.dungeon.map.get(secret_passage).unwrap();
        assert_eq!(passage.terrain, Terrain::SecretPassage);
        assert!(!passage.terrain.passable());
        assert!(passage.seen && passage.wizard_revealed);
        assert_eq!(g.glyph_at(secret_passage), '#');
    }
    #[test]
    fn wizard_can_create_and_charge_sticks() {
        let mut g = Game::new(20);
        g.set_wizard(true);
        g.wizard_create(ItemKind::Stick, 2);
        let id = g.player.inventory.last().unwrap().id;
        g.wizard_charge(id);
        assert_eq!(g.player.inventory.last().unwrap().charges, 10000)
    }

    #[test]
    fn wizard_creation_supports_gold_and_sets_the_amulet_invariant() {
        let mut g = Game::new(201);
        g.set_wizard(true);
        g.wizard_create_gold(70_000);
        let gold = g.player.inventory.last().unwrap();
        assert_eq!(gold.kind, ItemKind::Gold);
        assert_eq!(gold.gold_value, 70_000);

        g.wizard_create_gold(-25);
        let debt = g.player.inventory.last().unwrap();
        assert_eq!(debt.gold_value, -25);
        assert_eq!(g.inventory_name(debt, false), "-25 zlåtnikov");

        assert!(!g.has_amulet);
        g.wizard_create(ItemKind::Amulet, 0);
        assert!(g.has_amulet);
        assert_eq!(g.player.inventory.last().unwrap().kind, ItemKind::Amulet);
    }

    #[test]
    fn wizard_creation_preserves_unknown_object_glyphs_including_escape() {
        let mut g = Game::new(2010);
        g.set_wizard(true);

        g.wizard_create_bizarre('\u{1b}');

        let item = g.player.inventory.last().unwrap();
        assert_eq!(item.kind, ItemKind::Bizarre('\u{1b}'));
        assert_eq!(item.kind.glyph(), '\u{1b}');
        assert_eq!(g.inventory_name(item, false), "něčto divno ^[");
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("podbiraješ něčto divno ^[ (f)")
        );
    }

    #[test]
    fn wizard_creation_uses_reference_blessing_and_identification_rules() {
        let mut g = Game::new(188);
        g.set_wizard(true);
        g.wizard_create_blessed(ItemKind::Weapon, 3, '-');
        let weapon = g.player.inventory.last().unwrap();
        assert!(weapon.cursed && weapon.hit_plus < 0 && weapon.damage_plus < 0);
        assert!((8..=15).contains(&weapon.count));
        assert_ne!(weapon.group, 0);
        assert!(!weapon.known);

        g.wizard_create_blessed(ItemKind::Ring, 0, '+');
        let ring = g.player.inventory.last().unwrap();
        assert!((1..=2).contains(&ring.armor_class.unwrap()));
        assert!(!ring.cursed);
    }
    #[test]
    fn dropped_scare_scroll_turns_to_dust_when_recovered() {
        let mut g = Game::new(21);
        g.floor_items.clear();
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Scroll, 10));
        g.drop_item(id);
        assert!(g.floor_items[0].dropped_once);
        g.pickup();
        assert!(g.floor_items.is_empty());
        assert!(!g.player.inventory.iter().any(|i| i.id == id));
        assert!(g.messages.last().unwrap().contains("pråh"))
    }

    #[test]
    fn identification_retries_name_the_required_type_and_print_the_item() {
        let mut g = Game::new(203);
        g.pending_identification = Some(IdentifyKind::Potion);
        let armor = g
            .player
            .inventory
            .iter()
            .find(|item| item.kind == ItemKind::Armor)
            .unwrap()
            .id;
        g.identify_item(armor);
        assert_eq!(g.messages.last().unwrap(), "musiš opoznati napitȯk");
        assert_eq!(g.pending_identification, Some(IdentifyKind::Potion));

        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 0));
        g.identify_item(potion);
        assert!(
            g.messages
                .last()
                .unwrap()
                .starts_with(&format!("napitȯk {}", crate::lang::potion_effect_gen(0)))
        );
        assert!(g.knowledge.potions[0]);
        assert!(g.pending_identification.is_none());
    }
    #[test]
    fn flytrap_damage_escalates_and_holds_player() {
        let mut g = Game::new(22);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        let id = g.id();
        let mut flytrap = monster::create(id, 5, pos, g.depth, &mut g.rng);
        flytrap.awake = true;
        flytrap.level = 100;
        g.monsters.push(flytrap);
        let hp = g.player.stats.hp;
        g.monster_attack(0);
        assert_eq!(g.player.stats.hp, hp - 1);
        g.monster_attack(0);
        assert_eq!(g.player.stats.hp, hp - 3);
        assert_eq!(g.flytrap_holder, Some(id));
        assert_eq!(g.player.conditions.held_turns, 0);
    }

    #[test]
    fn flytrap_miss_applies_accumulated_constriction_damage_without_new_hold() {
        let mut g = Game::new(183);
        g.monsters.clear();
        let id = g.id();
        let mut flytrap = monster::create(id, 5, g.player.pos.offset(1, 0), g.depth, &mut g.rng);
        flytrap.level = -100;
        g.monsters.push(flytrap);
        g.flytrap_hits = 4;
        let hp = g.player.stats.hp;

        g.monster_attack(0);

        assert_eq!(g.player.stats.hp, hp - 4);
        assert_eq!(g.flytrap_hits, 4);
        assert_eq!(g.player.conditions.held_turns, 0);
        assert!(g.messages.last().unwrap().starts_with("Muholovka "));
        assert!(g.messages.last().unwrap().ends_with(" tebe"));
    }

    #[test]
    fn unseen_monster_combat_messages_do_not_reveal_its_species() {
        let mut g = Game::new(208);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        let mut phantom = monster::create(g.id(), 15, pos, g.depth, &mut g.rng);
        phantom.level = -100;
        phantom.flags |= monster::INVISIBLE;
        g.monsters.push(phantom);
        g.options.terse = true;

        g.monster_attack(0);

        assert_eq!(g.messages.last().map(String::as_str), Some("Ono hybi"));
        assert!(!g.messages.iter().any(|message| message.contains("phantom")));
    }

    #[test]
    fn combat_hit_and_miss_helpers_use_the_reference_terse_forms() {
        let mut g = Game::new(209);
        g.options.terse = true;
        assert_eq!(g.attack_hit_message(None, Some("the bat")), "udarjaješ");
        assert_eq!(g.attack_miss_message(None, Some("the bat")), "hybiš");
        assert_eq!(
            g.attack_hit_message(Some("the bat"), None),
            "The bat udarjaje"
        );
        assert_eq!(g.attack_miss_message(Some("the bat"), None), "The bat hybi");
    }

    #[test]
    fn random_monster_move_can_fail_despite_an_open_neighbor() {
        let seed = (1..1000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(2) == 0 && (rng.rnd(3), rng.rnd(3)) != (1, 2)
            })
            .unwrap();
        let mut g = Game::new(184);
        g.monsters.clear();
        let start = Pos::new(40, 12);
        g.player.pos = Pos::new(45, 12);
        for dy in -1..=1 {
            for dx in -1..=1 {
                g.dungeon.map.get_mut(start.offset(dx, dy)).unwrap().terrain = Terrain::Void;
            }
        }
        g.dungeon.map.get_mut(start).unwrap().terrain = Terrain::Floor;
        g.dungeon.map.get_mut(start.offset(1, 0)).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        let mut bat = monster::create(id, 1, start, g.depth, &mut g.rng);
        bat.awake = true;
        g.monsters.push(bat);
        g.rng = GameRng::new(seed);
        let mut expected_rng = g.rng;
        assert_eq!(expected_rng.rnd(2), 0);
        let _ = expected_rng.rnd(3);
        let _ = expected_rng.rnd(3);
        let _ = expected_rng.rnd(20);

        g.move_monster_step(0);

        assert_eq!(g.monsters[0].pos, start);
        assert_eq!(g.rng, expected_rng);
    }

    #[test]
    fn random_monster_move_does_not_overwrite_a_disguised_xeroc() {
        let seed = (1..10_000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(2) == 0 && rng.rnd(3) == 1 && rng.rnd(3) == 2
            })
            .unwrap();
        let mut g = Game::new(1840);
        g.monsters.clear();
        let start = Pos::new(40, 12);
        let target = start.offset(1, 0);
        g.player.pos = Pos::new(45, 12);
        for pos in [start, target] {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        }
        let mut bat = monster::create(g.id(), 1, start, g.depth, &mut g.rng);
        bat.awake = true;
        let mut xeroc = monster::create(g.id(), 23, target, g.depth, &mut g.rng);
        xeroc.disguise = '!';
        let bat_id = bat.id;
        let xeroc_id = xeroc.id;
        g.monsters.extend([bat, xeroc]);
        g.rng = GameRng::new(seed);

        g.move_monster_step(0);

        assert_eq!(
            g.monsters.iter().find(|m| m.id == bat_id).unwrap().pos,
            start
        );
        assert_eq!(
            g.monsters.iter().find(|m| m.id == xeroc_id).unwrap().pos,
            target
        );
    }

    #[test]
    fn fatal_wraith_max_hp_drain_records_the_wraith() {
        let mut g = Game::new(185);
        g.player.stats.experience = 100;
        g.player.stats.level = 5;
        g.player.stats.hp = 1;
        g.player.stats.max_hp = 1;

        g.drain_level();

        assert_eq!(g.end, EndState::Dead);
        assert_eq!(g.death_cause.as_deref(), Some("prizraka"));
    }
    #[test]
    fn leather_armor_does_not_rust() {
        let mut g = Game::new(23);
        let id = g.id();
        let mut leather = Item::basic(id, ItemKind::Armor, 0);
        leather.armor_class = Some(8);
        g.player.inventory.push(leather);
        g.player.armor = Some(id);
        g.rust_armor();
        assert_eq!(g.player.armor_class(), 8)
    }

    #[test]
    fn dead_monster_drops_its_carried_pack() {
        let mut g = Game::new(24);
        g.monsters.clear();
        g.floor_items.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut dragon = monster::create(g.id(), 3, pos, g.depth, &mut g.rng);
        let carried = Item::basic(g.id(), ItemKind::Potion, 0);
        let carried_id = carried.id;
        dragon.inventory.push(carried);
        g.monsters.push(dragon);
        g.kill_monster(0);
        assert!(g.floor_items.iter().any(|item| item.id == carried_id));
    }

    #[test]
    fn leprechaun_death_checks_fall_position_before_deepest_level() {
        let mut g = Game::new(240);
        g.monsters.clear();
        g.floor_items.clear();
        g.depth = 1;
        g.max_depth = 2;
        g.player.pos = Pos::new(1, 1);
        let center = Pos::new(40, 12);
        for dy in -1..=1 {
            for dx in -1..=1 {
                g.dungeon
                    .map
                    .get_mut(center.offset(dx, dy))
                    .unwrap()
                    .terrain = Terrain::Floor;
            }
        }
        let leprechaun = monster::create(g.id(), 11, center, g.depth, &mut g.rng);
        g.rng = GameRng::new(241);
        let mut expected = g.clone();
        expected.fall_position(center);

        g.drop_monster_inventory(leprechaun);

        assert_eq!(g.rng, expected.rng);
        assert!(g.floor_items.is_empty());
    }

    #[test]
    fn monster_pack_and_floor_lists_preserve_head_insertion_order() {
        let mut g = Game::new(242);
        g.monsters.clear();
        g.floor_items.clear();
        g.player.pos = Pos::new(1, 1);
        let center = Pos::new(40, 12);
        for dy in -1..=1 {
            for dx in -1..=1 {
                g.dungeon
                    .map
                    .get_mut(center.offset(dx, dy))
                    .unwrap()
                    .terrain = Terrain::Floor;
            }
        }
        let mut dragon = monster::create(g.id(), 3, center, g.depth, &mut g.rng);
        let newest = Item::basic(g.id(), ItemKind::Potion, 0);
        let newest_id = newest.id;
        let older = Item::basic(g.id(), ItemKind::Scroll, 0);
        let older_id = older.id;
        dragon.inventory = vec![newest, older];

        g.drop_monster_inventory(dragon);

        assert_eq!(
            g.floor_items
                .iter()
                .rev()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![older_id, newest_id]
        );
    }

    #[test]
    fn treasure_room_adds_guarded_items() {
        let mut g = Game::new(25);
        g.monsters.clear();
        g.floor_items.clear();
        g.populate_treasure_room();
        assert!(g.floor_items.len() >= 2);
        assert!(g.monsters.len() >= g.floor_items.len() + 2);
        assert!(
            g.monsters
                .iter()
                .all(|monster| monster.flags & monster::MEAN != 0)
        );
    }

    #[test]
    fn appearances_are_seeded_unique_and_complete() {
        let a = Game::new(26);
        let b = Game::new(26);
        assert_eq!(a.appearances, b.appearances);
        assert_eq!(a.appearances.potion_colors.len(), 14);
        assert_eq!(a.appearances.scroll_titles.len(), 18);
        assert_eq!(a.appearances.ring_stones.len(), 14);
        assert_eq!(a.appearances.ring_stone_values.len(), 14);
        assert_eq!(a.appearances.stick_materials.len(), 14);
        let mut colors = a.appearances.potion_colors.clone();
        colors.sort();
        colors.dedup();
        assert_eq!(colors.len(), 14);
    }

    #[test]
    fn passgo_follows_a_single_orthogonal_corridor_turn() {
        let mut g = Game::new(27);
        g.monsters.clear();
        g.floor_items.clear();
        let start = g.player.pos;
        for dy in -2..=2 {
            for dx in -1..=4 {
                let cell = g.dungeon.map.get_mut(start.offset(dx, dy)).unwrap();
                cell.terrain = Terrain::Void;
                cell.room = None;
                cell.passage = None;
                cell.trap = None;
                cell.trap_revealed = false;
            }
        }
        for pos in [
            start,
            start.offset(1, 0),
            start.offset(2, 0),
            start.offset(2, 1),
        ] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Passage;
            cell.passage = Some(0);
        }
        g.options.passgo = false;
        g.run_player(Direction::Right, true);
        assert_eq!(g.player.pos, start.offset(2, 0));

        g.player.pos = start;
        g.options.passgo = true;
        g.run_player(Direction::Right, true);
        assert_eq!(g.player.pos, start.offset(2, 1));
    }

    #[test]
    fn passgo_can_start_on_a_blocked_corner_and_turn_through_a_door() {
        let mut g = Game::new(270);
        g.monsters.clear();
        g.options.passgo = true;
        let start = g.player.pos;
        let current = g.dungeon.map.get_mut(start).unwrap();
        current.terrain = Terrain::Passage;
        current.passage = Some(0);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let cell = g.dungeon.map.get_mut(start.offset(dx, dy)).unwrap();
            cell.terrain = Terrain::Void;
            cell.passage = None;
        }
        let turn = start.offset(0, -1);
        let doorway = g.dungeon.map.get_mut(turn).unwrap();
        doorway.terrain = Terrain::Door;
        doorway.passage = Some(0);

        assert_eq!(g.run_player(Direction::Right, false), CommandResult::TURN);

        assert_eq!(g.player.pos, turn);
    }

    #[test]
    fn blocked_and_confused_stationary_moves_are_free() {
        let mut blocked = Game::new(222);
        let start = blocked.player.pos;
        blocked
            .dungeon
            .map
            .get_mut(start.offset(1, 0))
            .unwrap()
            .terrain = Terrain::Void;
        assert!(!blocked.move_player(Direction::Right).consumed_turn);
        assert_eq!(blocked.player.pos, start);

        let seed = (1..10_000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(5) != 0 && rng.rnd(3) == 1 && rng.rnd(3) == 1
            })
            .unwrap();
        blocked.player.conditions.confused = true;
        blocked.rng = GameRng::new(seed);
        assert!(!blocked.move_player(Direction::Right).consumed_turn);
        assert_eq!(blocked.player.pos, start);
    }

    #[test]
    fn confused_movement_avoids_scare_scrolls_and_monsters() {
        let seed = (1..10_000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(5) != 0 && rng.rnd(3) == 1 && rng.rnd(3) == 2
            })
            .unwrap();
        let mut g = Game::new(223);
        g.floor_items.clear();
        g.monsters.clear();
        let start = g.player.pos;
        let target = start.offset(1, 0);
        g.dungeon.map.get_mut(target).unwrap().terrain = Terrain::Floor;
        let mut scare = Item::basic(g.id(), ItemKind::Scroll, 10);
        scare.pos = Some(target);
        g.floor_items.push(scare);
        g.player.conditions.confused = true;
        g.rng = GameRng::new(seed);

        assert!(!g.move_player(Direction::Left).consumed_turn);
        assert_eq!(g.player.pos, start);

        g.floor_items.clear();
        let monster_id = g.id();
        g.monsters
            .push(monster::create(monster_id, 0, target, g.depth, &mut g.rng));
        g.rng = GameRng::new(seed);
        let monster_hp = g.monsters[0].hp;
        assert!(!g.move_player(Direction::Left).consumed_turn);
        assert_eq!(g.player.pos, start);
        assert_eq!(g.monsters[0].hp, monster_hp);
    }

    #[test]
    fn diagonal_move_requires_both_side_squares_to_be_open() {
        let mut g = Game::new(223);
        let start = g.player.pos;
        for pos in [start.offset(1, 0), start.offset(0, -1), start.offset(1, -1)] {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        }
        g.dungeon.map.get_mut(start.offset(1, 0)).unwrap().terrain = Terrain::Void;
        assert!(!g.move_player(Direction::UpRight).consumed_turn);
        assert_eq!(g.player.pos, start);
    }

    #[test]
    fn visibility_memory_does_not_leak_through_a_blocked_diagonal_corner() {
        let mut g = Game::new(2280);
        let origin = g.player.pos;
        let diagonal = origin.offset(1, 1);
        for pos in [origin.offset(1, 0), origin.offset(0, 1)] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::WallVertical;
            cell.room = None;
            cell.passage = None;
        }
        let cell = g.dungeon.map.get_mut(diagonal).unwrap();
        cell.terrain = Terrain::Floor;
        cell.room = None;
        cell.passage = None;
        cell.seen = false;
        cell.remembered = ' ';

        g.update_visibility();

        let cell = g.dungeon.map.get(diagonal).unwrap();
        assert!(!cell.seen);
        assert_eq!(cell.remembered, ' ');
    }

    #[test]
    fn running_stops_on_and_collects_an_object() {
        let mut g = Game::new(224);
        g.monsters.clear();
        g.floor_items.clear();
        let start = g.player.pos;
        for dx in 0..=4 {
            g.dungeon.map.get_mut(start.offset(dx, 0)).unwrap().terrain = Terrain::Passage;
        }
        let id = g.id();
        let food_before = g.player.inventory[0].count;
        let mut food = Item::basic(id, ItemKind::Food, 0);
        food.pos = Some(start.offset(2, 0));
        g.floor_items.push(food);
        assert!(g.run_player(Direction::Right, false).consumed_turn);
        assert_eq!(g.player.pos, start.offset(2, 0));
        assert!(g.floor_items.iter().all(|item| item.id != id));
        assert_eq!(g.player.inventory[0].count, food_before + 1);
    }

    #[test]
    fn running_does_not_reveal_an_undetected_invisible_monster() {
        let mut g = Game::new(225);
        g.monsters.clear();
        g.floor_items.clear();
        let start = g.player.pos;
        for dy in -1..=1 {
            for dx in 0..=4 {
                g.dungeon.map.get_mut(start.offset(dx, dy)).unwrap().terrain = Terrain::Void;
            }
        }
        for dx in 0..=3 {
            g.dungeon.map.get_mut(start.offset(dx, 0)).unwrap().terrain = Terrain::Passage;
        }
        let monster_id = g.id();
        let mut phantom = monster::create(monster_id, 15, start.offset(1, -1), g.depth, &mut g.rng);
        phantom.flags |= monster::INVISIBLE;
        phantom.awake = false;
        g.monsters.push(phantom);

        g.run_player(Direction::Right, false);

        assert_eq!(g.player.pos, start.offset(3, 0));
    }

    #[test]
    fn uppercase_run_ignores_a_side_monster_but_control_run_stops() {
        let mut base = Game::new(2250);
        base.monsters.clear();
        base.floor_items.clear();
        base.wandering_countdown = 10_000;
        let start = base.player.pos;
        for dy in -1..=1 {
            for dx in 0..=4 {
                let cell = base.dungeon.map.get_mut(start.offset(dx, dy)).unwrap();
                cell.terrain = Terrain::Void;
                cell.room = None;
                cell.passage = None;
            }
        }
        for dx in 0..=3 {
            let cell = base.dungeon.map.get_mut(start.offset(dx, 0)).unwrap();
            cell.terrain = Terrain::Passage;
            cell.passage = Some(0);
        }
        let monster_pos = start.offset(1, -1);
        let cell = base.dungeon.map.get_mut(monster_pos).unwrap();
        cell.terrain = Terrain::Passage;
        cell.passage = Some(0);
        let mut monster = monster::create(base.id(), 0, monster_pos, base.depth, &mut base.rng);
        monster.awake = true;
        monster.flags |= monster::HELD;
        base.monsters.push(monster);
        let mut cautious = base.clone();
        let mut blind = base.clone();
        blind.player.conditions.blind = true;

        base.run_player(Direction::Right, false);
        cautious.run_player(Direction::Right, true);
        blind.run_player(Direction::Right, true);

        assert_eq!(base.player.pos, start.offset(3, 0));
        assert_eq!(cautious.player.pos, start.offset(1, 0));
        assert_eq!(blind.player.pos, start.offset(3, 0));
    }

    #[test]
    fn control_run_stops_before_an_orthogonal_door_but_uppercase_run_enters_it() {
        let (mut momentum, start) = straight_test_passage(2251);
        let door = momentum.dungeon.map.get_mut(start.offset(2, 0)).unwrap();
        door.terrain = Terrain::Door;
        let mut cautious = momentum.clone();

        momentum.run_player(Direction::Right, false);
        cautious.run_player(Direction::Right, true);

        assert_eq!(momentum.player.pos, start.offset(2, 0));
        assert_eq!(cautious.player.pos, start.offset(1, 0));
    }

    #[test]
    fn control_run_scans_forward_diagonal_objects_but_ignores_rear_ones() {
        let (mut forward, start) = straight_test_passage(2252);
        let forward_pos = start.offset(2, -1);
        let cell = forward.dungeon.map.get_mut(forward_pos).unwrap();
        cell.terrain = Terrain::Passage;
        cell.passage = Some(0);
        let id = forward.id();
        let mut potion = Item::basic(id, ItemKind::Potion, 0);
        potion.pos = Some(forward_pos);
        forward.floor_items.push(potion);

        let (mut rear, rear_start) = straight_test_passage(2253);
        let rear_pos = rear_start.offset(0, -1);
        let cell = rear.dungeon.map.get_mut(rear_pos).unwrap();
        cell.terrain = Terrain::Passage;
        cell.passage = Some(0);
        let id = rear.id();
        let mut scroll = Item::basic(id, ItemKind::Scroll, 0);
        scroll.pos = Some(rear_pos);
        rear.floor_items.push(scroll);

        forward.run_player(Direction::Right, true);
        rear.run_player(Direction::Right, true);

        assert_eq!(forward.player.pos, start.offset(1, 0));
        assert_eq!(rear.player.pos, rear_start.offset(3, 0));
    }

    #[test]
    fn detected_invisible_monster_stops_control_run_even_in_the_rear_sector() {
        let (mut game, start) = straight_test_passage(2254);
        game.player.conditions.detect_monsters = true;
        let monster_pos = start.offset(0, -1);
        let cell = game.dungeon.map.get_mut(monster_pos).unwrap();
        cell.terrain = Terrain::Passage;
        cell.passage = Some(0);
        let mut monster = monster::create(game.id(), 15, monster_pos, game.depth, &mut game.rng);
        monster.awake = true;
        monster.flags |= monster::HELD | monster::INVISIBLE;
        game.monsters.push(monster);

        game.run_player(Direction::Right, true);

        assert_eq!(game.player.pos, start.offset(1, 0));
    }

    #[test]
    fn control_run_scans_after_monsters_finish_the_previous_turn() {
        let (mut game, start) = straight_test_passage(2255);
        for pos in [
            start.offset(3, -2),
            start.offset(2, -2),
            start.offset(3, -1),
            start.offset(2, -1),
        ] {
            let cell = game.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Passage;
            cell.passage = Some(0);
            cell.room = None;
            cell.trap = None;
        }
        let monster_start = start.offset(3, -2);
        let mut monster = monster::create(game.id(), 0, monster_start, game.depth, &mut game.rng);
        monster.awake = true;
        game.monsters.push(monster);

        game.run_player(Direction::Right, true);

        assert_eq!(game.monsters[0].pos, start.offset(2, -1));
        assert_eq!(game.player.pos, start.offset(1, 0));
    }

    #[test]
    fn passgo_does_not_turn_after_an_attack_consumes_the_move() {
        let (mut game, start) = straight_test_passage(2256);
        game.options.passgo = true;
        let turn = game.dungeon.map.get_mut(start.offset(0, -1)).unwrap();
        turn.terrain = Terrain::Passage;
        turn.passage = Some(0);
        let mut monster =
            monster::create(game.id(), 5, start.offset(1, 0), game.depth, &mut game.rng);
        monster.awake = true;
        game.monsters.push(monster);

        game.run_player(Direction::Right, false);

        assert_eq!(game.player.pos, start);
    }

    #[test]
    fn passgo_does_not_turn_after_a_stationary_confused_move() {
        let seed = (1..10_000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(5) != 0 && rng.rnd(3) == 1 && rng.rnd(3) == 1
            })
            .unwrap();
        let (mut game, start) = straight_test_passage(2257);
        game.options.passgo = true;
        game.player.conditions.confused = true;
        game.rng = GameRng::new(seed);
        let turn = game.dungeon.map.get_mut(start.offset(0, -1)).unwrap();
        turn.terrain = Terrain::Passage;
        turn.passage = Some(0);

        assert_eq!(
            game.run_player(Direction::Right, false),
            CommandResult::FREE
        );

        assert_eq!(game.player.pos, start);
    }

    #[test]
    fn confused_run_stops_on_the_object_at_its_actual_random_destination() {
        let seed = (1..10_000)
            .find(|seed| {
                let mut rng = GameRng::new(*seed);
                rng.rnd(5) != 0 && rng.rnd(3) == 0 && rng.rnd(3) == 1
            })
            .unwrap();
        let (mut game, start) = straight_test_passage(2258);
        game.player.conditions.confused = true;
        game.rng = GameRng::new(seed);
        let item_pos = start.offset(0, -1);
        let cell = game.dungeon.map.get_mut(item_pos).unwrap();
        cell.terrain = Terrain::Passage;
        cell.passage = Some(0);
        let id = game.id();
        let mut potion = Item::basic(id, ItemKind::Potion, 0);
        potion.pos = Some(item_pos);
        game.floor_items.push(potion);

        assert_eq!(
            game.run_player(Direction::Right, false),
            CommandResult::TURN
        );

        assert_eq!(game.player.pos, item_pos);
        assert!(game.floor_items.is_empty());
    }

    #[test]
    fn running_retries_bear_trap_holds_until_the_player_can_move() {
        let (mut game, start) = straight_test_passage(2259);
        game.player.conditions.held_turns = 2;

        game.run_player(Direction::Right, false);

        assert_eq!(game.player.conditions.held_turns, 0);
        assert_eq!(game.player.pos, start.offset(3, 0));
        assert_eq!(
            game.messages
                .iter()
                .filter(|message| message.as_str() == "ješče ne možeš izlězti iz medvěďjej pasti")
                .count(),
            2
        );
    }

    #[test]
    fn see_floor_controls_lamp_floor_in_a_dark_room() {
        let mut g = Game::new(28);
        let room_id = g.dungeon.map.get(g.player.pos).unwrap().room.unwrap();
        g.dungeon.rooms[room_id as usize].dark = true;
        let pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.seen = true;
        g.floor_items.retain(|item| item.pos != Some(pos));
        g.monsters.retain(|monster| monster.pos != pos);
        g.options.see_floor = true;
        assert_eq!(g.glyph_at(pos), '.');
        g.options.see_floor = false;
        assert_eq!(g.glyph_at(pos), ' ');

        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(pos);
        g.floor_items.push(potion);
        assert_eq!(g.glyph_at(pos), '!');
    }

    #[test]
    fn hallucination_visual_daemon_consumes_rng_and_keeps_frame_glyphs_stable() {
        let mut g = Game::new(185);
        g.floor_items.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(pos);
        g.floor_items.push(potion);
        g.player.conditions.hallucinating = true;
        let rng = g.rng;
        g.refresh_hallucination_visuals();
        let glyph = g.glyph_at(pos);

        assert!(['!', '?', '=', '/', ':', ')', ']', '%', '*'].contains(&glyph));
        assert_eq!(
            (0..30).map(|_| g.glyph_at(pos)).collect::<Vec<_>>(),
            vec![glyph; 30]
        );
        assert_ne!(g.rng, rng);
    }

    #[test]
    fn jump_suppresses_hallucination_redraws_during_an_automatic_run() {
        let (mut base, start) = straight_test_passage(1850);
        let item_pos = start.offset(1, 1);
        let cell = base.dungeon.map.get_mut(item_pos).unwrap();
        cell.terrain = Terrain::Passage;
        cell.passage = Some(0);
        let mut potion = Item::basic(base.id(), ItemKind::Potion, 0);
        potion.pos = Some(item_pos);
        base.floor_items.push(potion);
        base.player.conditions.hallucinating = true;
        base.refresh_hallucination_visuals();

        let initial_frame = base.hallucinated_items.clone();
        let mut jumping = base.clone();
        jumping.options.jump = true;
        let mut animated = base;
        animated.options.jump = false;

        jumping.run_player(Direction::Right, false);
        animated.run_player(Direction::Right, false);

        assert_eq!(jumping.player.pos, start.offset(3, 0));
        assert_eq!(animated.player.pos, start.offset(3, 0));
        assert_eq!(jumping.hallucinated_items, initial_frame);
        assert!(animated.hallucinated_items.is_empty());
        assert_ne!(jumping.rng, animated.rng);
    }

    #[test]
    fn hallucination_preserves_known_stairs_but_randomizes_unknown_stairs() {
        let mut g = Game::new(186);
        g.player.pos = g.dungeon.stairs.offset(1, 0);
        g.dungeon.map.get_mut(g.dungeon.stairs).unwrap().seen = true;
        g.player.conditions.hallucinating = true;
        g.seen_stairs = false;
        g.refresh_hallucination_visuals();
        assert!(g.hallucinated_stairs.is_some());
        assert_eq!(g.glyph_at(g.dungeon.stairs), g.hallucinated_stairs.unwrap());

        g.seen_stairs = true;
        assert_eq!(g.glyph_at(g.dungeon.stairs), '%');
    }

    #[test]
    fn hallucination_randomizes_detected_monsters_even_while_blind() {
        let mut g = Game::new(212);
        g.monsters.clear();
        let pos = g.player.pos.offset(4, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        g.monsters
            .push(monster::create(id, 0, pos, g.depth, &mut g.rng));
        g.player.conditions.hallucinating = true;
        g.player.conditions.blind = true;
        g.player.conditions.detect_monsters = true;

        g.refresh_hallucination_visuals();

        assert_eq!(g.hallucinated_monsters.len(), 1);
        assert!(g.glyph_at(pos).is_ascii_uppercase());
    }

    #[test]
    fn hallucinated_disguised_xeroc_remains_an_object_glyph() {
        let mut g = Game::new(187);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        g.monsters
            .push(monster::create(id, 23, pos, g.depth, &mut g.rng));
        g.player.conditions.hallucinating = true;
        g.refresh_hallucination_visuals();

        let glyph = g.glyph_at(pos);

        assert!(['!', '?', '=', '/', ':', ')', ']', '%', '*'].contains(&glyph));
    }

    #[test]
    fn lamp_visibility_uses_squared_distance_less_than_three() {
        let mut g = Game::new(188);
        g.player.pos = Pos::new(40, 12);
        for pos in [
            g.player.pos,
            Pos::new(41, 12),
            Pos::new(40, 13),
            Pos::new(41, 13),
            Pos::new(42, 12),
        ] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Passage;
            cell.room = None;
        }

        assert!(g.currently_visible(Pos::new(41, 13)));
        assert!(!g.currently_visible(Pos::new(42, 12)));

        g.dungeon.map.get_mut(Pos::new(41, 12)).unwrap().terrain = Terrain::Void;
        g.dungeon.map.get_mut(Pos::new(40, 13)).unwrap().terrain = Terrain::Void;
        assert!(!g.currently_visible(Pos::new(41, 13)));
    }

    #[test]
    fn hold_scroll_prevents_awake_monsters_from_moving() {
        let mut g = Game::new(29);
        g.monsters.clear();
        let pos = g.player.pos.offset(2, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut monster = monster::create(g.id(), 25, pos, g.depth, &mut g.rng);
        monster.awake = true;
        g.monsters.push(monster);
        let scroll = g.id();
        g.player
            .inventory
            .push(Item::basic(scroll, ItemKind::Scroll, 2));
        g.read_scroll(scroll);
        assert_ne!(g.monsters[0].flags & monster::HELD, 0);
        assert!(!g.monsters[0].awake);
        g.move_monsters();
        assert_eq!(g.monsters[0].pos, pos);
    }

    #[test]
    fn invalid_reads_cost_a_turn_and_wielded_scroll_stacks_are_unwielded() {
        let mut g = Game::new(291);
        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));
        assert_eq!(g.read_scroll(food), CommandResult::TURN);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("na tom ničto ne jest napisano")
        );

        let scroll = g.id();
        let mut stack = Item::basic(scroll, ItemKind::Scroll, 0);
        stack.count = 2;
        g.player.inventory.push(stack);
        g.player.weapon = Some(scroll);
        assert_eq!(g.read_scroll(scroll), CommandResult::TURN);
        assert_eq!(g.player.weapon, None);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == scroll)
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn create_monster_scroll_uses_reference_reservoir_rng_even_for_one_square() {
        let mut g = Game::new(292);
        g.monsters.clear();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx != 0 || dy != 0 {
                    g.dungeon
                        .map
                        .get_mut(g.player.pos.offset(dx, dy))
                        .unwrap()
                        .terrain = Terrain::Void;
                }
            }
        }
        let destination = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(destination).unwrap().terrain = Terrain::Floor;
        let scroll = g.id();
        g.player
            .inventory
            .push(Item::basic(scroll, ItemKind::Scroll, 14));
        let mut expected = g.clone();
        let _ = expected.rng.rnd(1);
        let kind = monster::random_kind(&mut expected.rng, expected.depth, false);
        let _ = expected.make_monster(kind, destination, expected.depth, false, false);

        g.read_scroll(scroll);

        assert_eq!(g.monsters[0].kind, kind);
        assert_eq!(g.rng, expected.rng);
    }

    #[test]
    fn create_monster_scroll_scans_adjacent_squares_in_reference_row_major_order() {
        let mut g = Game::new(213);
        g.monsters.clear();
        g.floor_items.clear();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx != 0 || dy != 0 {
                    g.dungeon
                        .map
                        .get_mut(g.player.pos.offset(dx, dy))
                        .unwrap()
                        .terrain = Terrain::Floor;
                }
            }
        }
        let scroll = g.id();
        g.player
            .inventory
            .push(Item::basic(scroll, ItemKind::Scroll, 14));
        let mut expected = g.clone();
        let mut count = 0;
        let mut destination = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                count += 1;
                if expected.rng.rnd(count) == 0 {
                    destination = Some(expected.player.pos.offset(dx, dy));
                }
            }
        }
        let kind = monster::random_kind(&mut expected.rng, expected.depth, false);
        let destination = destination.unwrap();
        let _ = expected.make_monster(kind, destination, expected.depth, false, false);

        g.read_scroll(scroll);

        assert_eq!(g.monsters[0].pos, destination);
        assert_eq!(g.monsters[0].kind, kind);
        assert_eq!(g.rng, expected.rng);
    }

    #[test]
    fn enchant_armor_scroll_is_silent_without_worn_armor() {
        let mut g = Game::new(214);
        g.player.armor = None;
        g.messages.clear();
        let scroll = g.id();
        g.player
            .inventory
            .push(Item::basic(scroll, ItemKind::Scroll, 4));

        g.read_scroll(scroll);

        assert!(g.messages.is_empty());
        assert!(!g.knowledge.scrolls[4]);
    }

    #[test]
    fn confusion_scroll_empowers_one_successful_hit() {
        let mut g = Game::new(30);
        g.monsters.clear();
        let scroll = g.id();
        g.player
            .inventory
            .push(Item::basic(scroll, ItemKind::Scroll, 0));
        g.read_scroll(scroll);
        assert!(g.player.conditions.can_confuse_monster);
        g.player.stats.level = 100;
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut target = monster::create(g.id(), 3, pos, g.depth, &mut g.rng);
        target.hp = 10_000;
        target.max_hp = 10_000;
        g.monsters.push(target);
        g.player_attack(0);
        assert!(!g.player.conditions.can_confuse_monster);
        assert_ne!(g.monsters[0].flags & monster::CONFUSED, 0);
    }

    #[test]
    fn identification_only_occurs_when_reference_effect_reveals_type() {
        let mut g = Game::new(31);
        let see_invisible = g.id();
        g.player
            .inventory
            .push(Item::basic(see_invisible, ItemKind::Potion, 4));
        g.quaff(see_invisible);
        assert!(!g.knowledge.potions[4]);

        let healing = g.id();
        g.player
            .inventory
            .push(Item::basic(healing, ItemKind::Potion, 5));
        g.quaff(healing);
        assert!(g.knowledge.potions[5]);

        let enchant_armor = g.id();
        g.player
            .inventory
            .push(Item::basic(enchant_armor, ItemKind::Scroll, 4));
        g.read_scroll(enchant_armor);
        assert!(!g.knowledge.scrolls[4]);
    }

    #[test]
    fn monster_carried_magic_scan_preserves_floor_object_guard_quirk() {
        let mut g = Game::new(32);
        g.floor_items.clear();
        g.monsters.clear();
        let pos = g.player.pos.offset(5, 0);
        let mut carrier = monster::create(g.id(), 2, pos, g.depth, &mut g.rng);
        carrier
            .inventory
            .push(Item::basic(g.id(), ItemKind::Ring, 0));
        g.monsters.push(carrier);
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 7));
        g.quaff(potion);
        assert!(!g.knowledge.potions[7]);

        let mut food = Item::basic(g.id(), ItemKind::Food, 0);
        food.pos = Some(g.player.pos.offset(1, 0));
        g.floor_items.push(food);
        assert_eq!(g.magic_positions(), vec![pos]);
    }

    #[test]
    fn monster_detection_is_unidentified_when_every_monster_is_already_seen() {
        let mut g = Game::new(191);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        let id = g.id();
        g.monsters
            .push(monster::create(id, 25, pos, g.depth, &mut g.rng));
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 6));

        g.quaff(potion);

        assert!(!g.knowledge.potions[6]);
        assert_eq!(
            g.messages.last().unwrap(),
            "na moment čuješ sę divno, potom to prěhodi"
        );
    }

    #[test]
    fn failed_detection_feeling_is_normal_during_hallucination() {
        let mut g = Game::new(192);
        g.floor_items.clear();
        g.monsters.clear();
        g.player.conditions.hallucinating = true;
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 7));

        g.quaff(potion);

        assert_eq!(
            g.messages.last().unwrap(),
            "na moment čuješ sę normaľno, potom to prěhodi"
        );
    }

    #[test]
    fn failed_drain_life_does_not_consume_a_charge() {
        let mut g = Game::new(33);
        g.player.stats.hp = 1;
        let id = g.id();
        let mut stick = Item::basic(id, ItemKind::Stick, 9);
        stick.charges = 3;
        g.player.inventory.push(stick);
        g.zap(id, Direction::Right);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .charges,
            3
        );
    }

    #[test]
    fn magic_missile_save_leaves_sleep_hold_and_destination_unchanged() {
        let mut g = Game::new(1841);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let monster_id = g.id();
        let mut target = monster::create(monster_id, 25, pos, g.depth, &mut g.rng);
        target.level = 100;
        target.awake = false;
        target.flags |= monster::HELD;
        target.destination = Some(g.dungeon.stairs);
        let hp = target.hp;
        g.monsters.push(target);
        let stick_id = g.id();
        let mut stick = Item::basic(stick_id, ItemKind::Stick, 6);
        stick.charges = 1;
        g.player.inventory.push(stick);

        g.zap(stick_id, Direction::Right);

        assert_eq!(g.monsters[0].hp, hp);
        assert!(!g.monsters[0].awake);
        assert_ne!(g.monsters[0].flags & monster::HELD, 0);
        assert_eq!(g.monsters[0].destination, Some(g.dungeon.stairs));
        assert_eq!(g.messages.last().unwrap(), "strěla izčezaje v oblåku dyma");
    }

    #[test]
    fn successful_magic_missile_uses_reference_runto_transition() {
        let mut g = Game::new(1842);
        g.monsters.clear();
        g.floor_items.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let monster_id = g.id();
        let mut target = monster::create(monster_id, 25, pos, g.depth, &mut g.rng);
        target.level = -100;
        target.hp = 100;
        target.max_hp = 100;
        target.awake = false;
        target.flags |= monster::HELD;
        target.destination = Some(g.dungeon.stairs);
        g.monsters.push(target);
        let stick_id = g.id();
        let mut stick = Item::basic(stick_id, ItemKind::Stick, 6);
        stick.charges = 1;
        g.player.inventory.push(stick);

        g.zap(stick_id, Direction::Right);

        assert!(g.monsters[0].hp < 100);
        assert!(g.monsters[0].awake);
        assert_eq!(g.monsters[0].flags & monster::HELD, 0);
        assert_eq!(g.monsters[0].destination, None);
    }

    #[test]
    fn polymorph_preserves_the_monsters_pack() {
        let mut g = Game::new(34);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut target = monster::create(g.id(), 2, pos, g.depth, &mut g.rng);
        let carried = Item::basic(g.id(), ItemKind::Potion, 0);
        let carried_id = carried.id;
        target.inventory.push(carried);
        g.monsters.push(target);
        let id = g.id();
        let mut stick = Item::basic(id, ItemKind::Stick, 5);
        stick.charges = 1;
        g.player.inventory.push(stick);
        g.zap(id, Direction::Right);
        assert!(
            g.monsters[0]
                .inventory
                .iter()
                .any(|item| item.id == carried_id)
        );
    }

    #[test]
    fn polymorph_resets_running_state_unless_aggravation_is_worn() {
        fn setup(aggravated: bool) -> (Game, u64) {
            let mut g = Game::new(341);
            g.monsters.clear();
            let pos = g.player.pos.offset(1, 0);
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
            let mut target = monster::create(g.id(), 2, pos, g.depth, &mut g.rng);
            target.awake = true;
            g.monsters.push(target);
            if aggravated {
                let ring = g.id();
                g.player
                    .inventory
                    .push(Item::basic(ring, ItemKind::Ring, 6));
                g.player.rings[0] = Some(ring);
            }
            let id = g.id();
            let mut stick = Item::basic(id, ItemKind::Stick, 5);
            stick.charges = 1;
            g.player.inventory.push(stick);
            (g, id)
        }

        let (mut ordinary, stick) = setup(false);
        ordinary.zap(stick, Direction::Right);
        assert!(!ordinary.monsters[0].awake);

        let (mut aggravated, stick) = setup(true);
        aggravated.zap(stick, Direction::Right);
        assert!(aggravated.monsters[0].awake);
    }

    #[test]
    fn sticks_are_only_identified_by_revealing_effects() {
        let mut g = Game::new(35);
        g.monsters.clear();
        let invisible = g.id();
        let mut stick = Item::basic(invisible, ItemKind::Stick, 1);
        stick.charges = 1;
        g.player.inventory.push(stick);
        g.zap(invisible, Direction::Right);
        assert!(!g.knowledge.sticks[1]);

        let light = g.id();
        let mut stick = Item::basic(light, ItemKind::Stick, 0);
        stick.charges = 1;
        g.player.inventory.push(stick);
        g.zap(light, Direction::Right);
        assert!(g.knowledge.sticks[0]);
    }

    #[test]
    fn wielded_nonweapons_use_their_reference_melee_damage() {
        let mut g = Game::new(36);
        g.monsters.clear();
        g.player.stats.level = 100;
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut target = monster::create(g.id(), 3, pos, g.depth, &mut g.rng);
        target.hp = 10_000;
        target.max_hp = 10_000;
        g.monsters.push(target);

        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));
        g.player.weapon = Some(food);
        g.player_attack(0);
        assert_eq!(g.monsters[0].hp, 9_999);

        let staff = g.id();
        let mut item = Item::basic(staff, ItemKind::Stick, 0);
        item.charges = 1;
        g.player.inventory.push(item);
        g.appearances.stick_is_staff[0] = true;
        g.player.weapon = Some(staff);
        g.player_attack(0);
        assert!(g.monsters[0].hp <= 9_996);
    }

    #[test]
    fn pack_capacity_counts_multiplied_items_but_not_missile_quantity() {
        let mut g = Game::new(37);
        g.player.inventory.clear();
        g.floor_items.clear();
        let mut potions = Item::basic(g.id(), ItemKind::Potion, 0);
        potions.count = 25;
        g.player.inventory.push(potions);
        let mut arrows = Item::basic(g.id(), ItemKind::Weapon, 3);
        arrows.count = 15;
        g.player.inventory.push(arrows);
        assert_eq!(g.pack_count(), 26);

        let mut floor_potion = Item::basic(g.id(), ItemKind::Potion, 0);
        floor_potion.pos = Some(g.player.pos);
        g.floor_items.push(floor_potion);
        g.pickup();
        assert_eq!(g.pack_count(), 26);
        assert_eq!(g.floor_items.len(), 1);
    }

    #[test]
    fn recovered_grouped_missile_merges_even_when_pack_is_full() {
        let mut g = Game::new(193);
        g.player.inventory.clear();
        g.floor_items.clear();
        let group = 9000;
        let mut arrows = Item::basic(g.id(), ItemKind::Weapon, 3);
        arrows.count = 10;
        arrows.group = group;
        let arrows_id = arrows.id;
        g.player.inventory.push(arrows);
        for which in 0..25 {
            let id = g.id();
            g.player
                .inventory
                .push(Item::basic(id, ItemKind::Ring, (which % 14) as u8));
        }
        let mut recovered = Item::basic(g.id(), ItemKind::Weapon, 3);
        recovered.group = group;
        recovered.pos = Some(g.player.pos);
        g.floor_items.push(recovered);
        assert_eq!(g.pack_count(), MAX_PACK);

        g.pickup();

        assert!(g.floor_items.is_empty());
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == arrows_id)
                .unwrap()
                .count,
            11
        );
        assert_eq!(g.pack_count(), MAX_PACK);
    }

    #[test]
    fn multiplied_items_merge_by_type_even_if_legacy_item_labels_differ() {
        let mut g = Game::new(1930);
        g.player.inventory.clear();
        g.floor_items.clear();
        let mut carried = Item::basic(g.id(), ItemKind::Potion, 4);
        carried.label = Some("old per-item name".into());
        carried.pack_letter = Some('a');
        let carried_id = carried.id;
        g.player.inventory.push(carried);
        let mut floor = Item::basic(g.id(), ItemKind::Potion, 4);
        floor.label = Some("different old name".into());
        floor.pos = Some(g.player.pos);
        g.floor_items.push(floor);

        g.pickup();

        assert!(g.floor_items.is_empty());
        assert_eq!(g.player.inventory.len(), 1);
        assert_eq!(g.player.inventory[0].id, carried_id);
        assert_eq!(g.player.inventory[0].count, 2);
        assert_eq!(g.pack_count(), 2);
    }

    #[test]
    fn counted_consumable_cannot_merge_into_a_full_pack() {
        let mut g = Game::new(1931);
        g.player.inventory.clear();
        g.floor_items.clear();
        let mut carried = Item::basic(g.id(), ItemKind::Potion, 4);
        carried.count = MAX_PACK as u32;
        carried.pack_letter = Some('a');
        g.player.inventory.push(carried);
        let mut floor = Item::basic(g.id(), ItemKind::Potion, 4);
        floor.pos = Some(g.player.pos);
        g.floor_items.push(floor);

        g.pickup();

        assert_eq!(g.player.inventory[0].count, MAX_PACK as u32);
        assert_eq!(g.floor_items.len(), 1);
        assert_eq!(g.pack_count(), MAX_PACK);
    }

    #[test]
    fn dropping_equipped_noncursed_items_uses_dropcheck_semantics() {
        let mut weapon_game = Game::new(194);
        weapon_game.floor_items.clear();
        weapon_game
            .dungeon
            .map
            .get_mut(weapon_game.player.pos)
            .unwrap()
            .terrain = Terrain::Floor;
        let weapon = weapon_game.player.weapon.unwrap();
        let result = weapon_game.drop_item(weapon);
        assert!(result.consumed_turn);
        assert_eq!(weapon_game.player.weapon, None);

        let mut armor_game = Game::new(195);
        armor_game.floor_items.clear();
        armor_game
            .dungeon
            .map
            .get_mut(armor_game.player.pos)
            .unwrap()
            .terrain = Terrain::Floor;
        let armor = armor_game.player.armor.unwrap();
        let before = armor_game.turn;
        let result = armor_game.drop_item(armor);
        armor_game.finish_action(result);
        assert_eq!(armor_game.player.armor, None);
        assert_eq!(armor_game.turn, before + 2);

        let mut ring_game = Game::new(196);
        ring_game.player.inventory.clear();
        ring_game.floor_items.clear();
        ring_game
            .dungeon
            .map
            .get_mut(ring_game.player.pos)
            .unwrap()
            .terrain = Terrain::Floor;
        let ring = ring_game.id();
        let mut item = Item::basic(ring, ItemKind::Ring, 1);
        item.armor_class = Some(2);
        ring_game.player.inventory.push(item);
        let strength = ring_game.player.stats.strength;
        ring_game.put_on_ring(ring, 0);
        ring_game.drop_item(ring);
        assert_eq!(ring_game.player.rings[0], None);
        assert_eq!(ring_game.player.stats.strength, strength);
    }

    #[test]
    fn dropping_on_a_revealed_trap_is_rejected_as_an_occupied_cell() {
        let mut g = Game::new(1960);
        g.floor_items.clear();
        let cell = g.dungeon.map.get_mut(g.player.pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::Bear);
        cell.trap_revealed = true;
        let food = g.player.inventory[0].id;

        let result = g.drop_item(food);

        assert!(!result.consumed_turn);
        assert!(g.player.inventory.iter().any(|item| item.id == food));
        assert!(g.floor_items.is_empty());
        assert_eq!(g.messages.last().unwrap(), "tam uže něčto jest");
    }

    #[test]
    fn nymph_steals_one_item_from_a_multiplied_stack() {
        let mut g = Game::new(38);
        g.player.inventory.clear();
        g.monsters.clear();
        let mut potions = Item::basic(g.id(), ItemKind::Potion, 0);
        potions.count = 3;
        g.player.inventory.push(potions);
        let pos = g.player.pos.offset(1, 0);
        let monster_id = g.id();
        g.monsters
            .push(monster::create(monster_id, 13, pos, g.depth, &mut g.rng));
        let mut stolen = g.player.inventory[0].clone();
        stolen.count = 1;
        stolen.pack_letter = None;
        let expected_message = format!("ona ukradla {}!", g.inventory_name(&stolen, true));
        g.nymph_steal(0);
        assert_eq!(g.player.inventory[0].count, 2);
        assert!(g.monsters.is_empty());
        assert_eq!(g.messages.last(), Some(&expected_message));
    }

    #[test]
    fn nymph_uses_reference_reservoir_rng_across_eligible_magic_items() {
        let mut g = Game::new(380);
        g.player.inventory.clear();
        g.player.weapon = None;
        g.player.armor = None;
        g.player.rings = [None, None];
        g.monsters.clear();
        for (kind, which) in [
            (ItemKind::Potion, 0),
            (ItemKind::Scroll, 1),
            (ItemKind::Ring, 2),
        ] {
            let id = g.id();
            g.player.inventory.push(Item::basic(id, kind, which));
        }
        let monster_id = g.id();
        g.monsters.push(monster::create(
            monster_id,
            13,
            g.player.pos.offset(1, 0),
            g.depth,
            &mut g.rng,
        ));
        let mut expected_rng = g.rng;
        let mut expected_choice = 0;
        for eligible in 1..=3 {
            if expected_rng.rnd(eligible) == 0 {
                expected_choice = eligible as usize - 1;
            }
        }
        let stolen_id = g.player.inventory[expected_choice].id;
        let stolen_name = g.inventory_name(&g.player.inventory[expected_choice], true);

        g.nymph_steal(0);

        assert_eq!(g.rng, expected_rng);
        assert!(g.player.inventory.iter().all(|item| item.id != stolen_id));
        assert!(g.monsters.is_empty());
        assert_eq!(
            g.messages.last().unwrap(),
            &format!("ona ukradla {stolen_name}!")
        );
    }

    #[test]
    fn every_deep_level_replaces_an_unclaimed_amulet() {
        let mut g = Game::new(39);
        g.depth = AMULET_LEVEL + 3;
        g.has_amulet = false;
        g.new_level();
        assert!(
            g.floor_items
                .iter()
                .any(|item| item.kind == ItemKind::Amulet)
        );
    }

    #[test]
    fn player_starts_each_level_on_a_legal_monster_free_room_square() {
        for seed in 40..80 {
            let g = Game::new(seed);
            assert!(g.monsters.iter().all(|monster| monster.pos != g.player.pos));
            assert!(matches!(
                g.dungeon.map.get(g.player.pos).unwrap().terrain,
                Terrain::Floor | Terrain::Passage | Terrain::Stairs
            ));
        }
    }

    #[test]
    fn haste_grants_two_actions_before_world_advances() {
        let mut g = Game::new(80);
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 10));
        let result = g.quaff(potion);
        g.finish_action(result);
        assert_eq!(g.turn, 0);
        assert!(g.player.conditions.hasted);

        g.execute(Command::Rest);
        assert_eq!(g.turn, 0);
        g.execute(Command::Rest);
        assert_eq!(g.turn, 1);
    }

    #[test]
    fn removing_see_invisible_ring_extinguishes_the_shared_c_effect() {
        let mut g = Game::new(81);
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 4));
        g.quaff(potion);
        let ring = g.id();
        g.player
            .inventory
            .push(Item::basic(ring, ItemKind::Ring, 4));
        g.put_on_ring(ring, 0);
        assert!(g.player.conditions.see_invisible);
        g.remove_ring(0);
        assert!(!g.player.conditions.see_invisible);
        assert!(
            g.scheduler
                .fuses
                .iter()
                .all(|fuse| fuse.effect != Effect::SeeInvisible)
        );
    }

    #[test]
    fn potion_fuse_expiration_disables_see_invisible_even_with_ring() {
        let mut g = Game::new(216);
        let ring = g.id();
        g.player
            .inventory
            .push(Item::basic(ring, ItemKind::Ring, 4));
        g.put_on_ring(ring, 0);
        g.scheduler.add_or_lengthen(Effect::SeeInvisible, 1);

        g.tick_effects();

        assert!(!g.player.conditions.see_invisible);
    }

    #[test]
    fn restore_strength_preserves_add_strength_ring_bonus() {
        let mut g = Game::new(82);
        let ring = g.id();
        let mut item = Item::basic(ring, ItemKind::Ring, 1);
        item.armor_class = Some(2);
        g.player.inventory.push(item);
        g.put_on_ring(ring, 0);
        g.player.stats.strength = 8;
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 11));
        g.quaff(potion);
        assert_eq!(g.player.stats.strength, g.player.max_strength + 2);
    }

    #[test]
    fn elemental_bolt_can_reflect_back_into_the_player() {
        let mut reflected_hit = false;
        for seed in 83..200 {
            let mut g = Game::new(seed);
            g.monsters.clear();
            g.player.stats.hp = 100;
            g.player.stats.max_hp = 100;
            g.player.stats.level = 1;
            let start = g.player.pos;
            for (offset, terrain) in [
                ((1, 0), Terrain::Floor),
                ((2, 0), Terrain::Floor),
                ((3, 0), Terrain::WallVertical),
            ] {
                g.dungeon
                    .map
                    .get_mut(start.offset(offset.0, offset.1))
                    .unwrap()
                    .terrain = terrain;
            }
            g.fire_bolt(Direction::Right, 2);
            if g.player.stats.hp < 100 {
                reflected_hit = true;
                break;
            }
        }
        assert!(reflected_hit);
    }

    #[test]
    fn only_flame_bounces_off_dragons() {
        fn game_with_dragon(seed: u64) -> Game {
            let mut g = Game::new(seed);
            g.monsters.clear();
            let pos = g.player.pos.offset(1, 0);
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
            let id = g.id();
            let mut dragon = monster::create(id, 3, pos, g.depth, &mut g.rng);
            dragon.level = -100;
            dragon.hp = 100;
            dragon.max_hp = 100;
            g.monsters.push(dragon);
            g
        }
        let mut flame = game_with_dragon(200);
        flame.fire_bolt(Direction::Right, 3);
        assert_eq!(flame.monsters[0].hp, 100);

        let mut cold = game_with_dragon(200);
        cold.fire_bolt(Direction::Right, 4);
        assert!(cold.monsters[0].hp < 100);
    }

    #[test]
    fn saved_bolt_starts_monster_pursuit_and_uses_reference_messages() {
        fn game_with_saving_monster(seed: u64) -> Game {
            let mut g = Game::new(seed);
            g.monsters.clear();
            let pos = g.player.pos.offset(1, 0);
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
            let id = g.id();
            let mut monster = monster::create(id, 0, pos, g.depth, &mut g.rng);
            monster.level = 100;
            monster.awake = false;
            monster.flags |= monster::HELD;
            monster.destination = Some(Pos::new(0, 0));
            g.monsters.push(monster);
            g
        }

        let mut verbose = game_with_saving_monster(203);
        verbose.fire_bolt(Direction::Right, 2);
        assert!(verbose.monsters[0].awake);
        assert_eq!(verbose.monsters[0].flags & monster::HELD, 0);
        assert_ne!(verbose.monsters[0].destination, Some(Pos::new(0, 0)));
        assert!(
            verbose
                .messages
                .iter()
                .any(|message| message == "mȯlnja leti mimo akvatora")
        );

        let mut terse = game_with_saving_monster(204);
        terse.options.terse = true;
        terse.fire_bolt(Direction::Right, 2);
        assert!(
            terse
                .messages
                .iter()
                .any(|message| message == "mȯlnja hybi")
        );
    }

    #[test]
    fn saved_bolt_alerts_a_real_medusa_like_any_other_disguise() {
        let mut g = Game::new(205);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        let mut medusa = monster::create(id, 12, pos, g.depth, &mut g.rng);
        medusa.level = 100;
        medusa.awake = false;
        medusa.flags |= monster::HELD;
        g.monsters.push(medusa);
        g.fire_bolt(Direction::Right, 2);
        assert!(g.monsters[0].awake);
        assert_eq!(g.monsters[0].flags & monster::HELD, 0);
        assert!(
            g.messages
                .iter()
                .any(|message| message == "mȯlnja leti mimo meduzy")
        );
    }

    #[test]
    fn dragon_breath_reflects_before_reaching_the_player() {
        let mut g = Game::new(206);
        g.monsters.clear();
        g.player.stats.hp = 100;
        let start = g.player.pos.offset(-2, 0);
        g.dungeon.map.get_mut(start).unwrap().terrain = Terrain::Floor;
        g.dungeon
            .map
            .get_mut(g.player.pos.offset(-1, 0))
            .unwrap()
            .terrain = Terrain::WallVertical;

        g.fire_bolt_from(start, Direction::Right, 3, Some(3));

        assert_eq!(g.player.stats.hp, 100);
        assert!(
            g.messages
                .iter()
                .any(|message| message == "plåmenj odskoči")
        );
    }

    #[test]
    fn monster_fired_bolt_save_does_not_start_pursuit() {
        let mut g = Game::new(207);
        g.monsters.clear();
        g.player.stats.level = 100;
        let start = g.player.pos.offset(-3, 0);
        let target_pos = g.player.pos.offset(-1, 0);
        for pos in [
            start,
            start.offset(1, 0),
            target_pos,
            g.player.pos,
            g.player.pos.offset(1, 0),
        ] {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        }
        g.dungeon
            .map
            .get_mut(g.player.pos.offset(2, 0))
            .unwrap()
            .terrain = Terrain::WallVertical;
        let id = g.id();
        let mut target = monster::create(id, 0, target_pos, g.depth, &mut g.rng);
        target.level = 100;
        target.awake = false;
        target.flags |= monster::HELD;
        target.destination = Some(Pos::new(0, 0));
        g.monsters.push(target);

        g.fire_bolt_from(start, Direction::Right, 3, Some(3));

        assert!(!g.monsters[0].awake);
        assert_ne!(g.monsters[0].flags & monster::HELD, 0);
        assert_eq!(g.monsters[0].destination, Some(Pos::new(0, 0)));
    }

    #[test]
    fn equipment_cannot_be_used_in_two_roles() {
        let mut g = Game::new(201);
        let armor = g.id();
        g.player
            .inventory
            .push(Item::basic(armor, ItemKind::Armor, 0));
        assert!(!g.wield(armor).consumed_turn);

        let ring = g.id();
        g.player
            .inventory
            .push(Item::basic(ring, ItemKind::Ring, 0));
        g.player.weapon = Some(ring);
        assert!(g.put_on_ring(ring, 0).consumed_turn);
        assert_eq!(g.player.weapon, Some(ring));
        assert_eq!(g.player.rings[0], None);
    }

    #[test]
    fn grouped_weapons_drop_as_a_group_but_multiplied_items_drop_one() {
        let mut g = Game::new(202);
        g.player.inventory.clear();
        g.floor_items.clear();
        g.dungeon.map.get_mut(g.player.pos).unwrap().terrain = Terrain::Floor;
        let arrows = g.id();
        let mut arrow_stack = Item::basic(arrows, ItemKind::Weapon, 3);
        arrow_stack.count = 12;
        g.player.inventory.push(arrow_stack);
        g.drop_item(arrows);
        assert_eq!(g.floor_items[0].count, 12);
        assert!(g.player.inventory.is_empty());

        g.floor_items.clear();
        let potions = g.id();
        let mut potion_stack = Item::basic(potions, ItemKind::Potion, 0);
        potion_stack.count = 3;
        g.player.inventory.push(potion_stack);
        g.drop_item(potions);
        assert_eq!(g.floor_items[0].count, 1);
        assert_eq!(g.player.inventory[0].count, 2);
        assert_eq!(
            g.messages.last().unwrap(),
            &format!(
                "ostavjaješ {}",
                g.inventory_name_case(&g.floor_items[0], true, Case::Acc)
            )
        );
    }

    #[test]
    fn levitation_blocks_both_stair_directions() {
        let mut g = Game::new(203);
        g.player.pos = g.dungeon.stairs;
        g.player.conditions.levitating = true;
        let depth = g.depth;
        assert!(!g.descend().consumed_turn);
        assert_eq!(g.depth, depth);
        g.has_amulet = true;
        assert!(!g.ascend().consumed_turn);
        assert_eq!(g.depth, depth);
    }

    #[test]
    fn searching_can_reveal_a_hidden_passage() {
        let mut g = Game::new(204);
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::SecretPassage;
        for _ in 0..100 {
            g.search();
            if g.dungeon.map.get(pos).unwrap().terrain == Terrain::Passage {
                break;
            }
        }
        assert_eq!(g.dungeon.map.get(pos).unwrap().terrain, Terrain::Passage);
    }

    #[test]
    fn greedy_orc_guards_the_room_gold_coordinate_without_collecting_it() {
        let mut g = Game::new(205);
        g.monsters.clear();
        g.floor_items.clear();
        let room = g
            .dungeon
            .map
            .get(g.player.pos)
            .and_then(|cell| cell.room)
            .unwrap();
        g.dungeon.rooms[room as usize].dark = false;
        let orc_pos = g.player.pos.offset(4, 0);
        let gold_pos = g.player.pos.offset(6, 0);
        for dx in 1..=6 {
            let cell = g.dungeon.map.get_mut(g.player.pos.offset(dx, 0)).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(room);
        }
        let id = g.id();
        g.monsters
            .push(monster::create(id, 14, orc_pos, g.depth, &mut g.rng));
        let mut gold = Item::gold(g.id(), 50);
        gold.pos = Some(gold_pos);
        g.floor_items.push(gold);
        g.dungeon.rooms[room as usize].gold = Some(gold_pos);
        g.dungeon.rooms[room as usize].gold_value = 50;
        g.wake_room_monsters(room);
        for _ in 0..3 {
            g.move_monsters();
        }
        assert_eq!(g.monsters[0].pos, gold_pos);
        assert!(!g.monsters[0].awake);
        assert!(g.monsters[0].inventory.is_empty());
        assert!(g.floor_items.iter().any(|item| item.pos == Some(gold_pos)));
    }

    #[test]
    fn greedy_orc_ignores_arbitrary_gold_without_an_active_room_treasure() {
        let mut g = Game::new(2050);
        g.monsters.clear();
        g.floor_items.clear();
        let room = g.dungeon.map.get(g.player.pos).unwrap().room.unwrap();
        let orc_pos = g.player.pos.offset(4, 0);
        let gold_pos = g.player.pos.offset(6, 0);
        for dx in 1..=6 {
            let cell = g.dungeon.map.get_mut(g.player.pos.offset(dx, 0)).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(room);
        }
        let id = g.id();
        g.monsters
            .push(monster::create(id, 14, orc_pos, g.depth, &mut g.rng));
        let mut gold = Item::gold(g.id(), 50);
        gold.pos = Some(gold_pos);
        g.floor_items.push(gold);

        g.wake_room_monsters(room);

        assert!(g.monsters[0].awake);
        assert_eq!(g.monsters[0].destination, None);
        assert!(!g.monsters[0].destination_is_room_gold);
    }

    #[test]
    fn medusa_gaze_is_a_one_time_saving_throw() {
        let mut g = Game::new(206);
        g.monsters.clear();
        g.player.stats.level = -100;
        let room = g.dungeon.map.get(g.player.pos).unwrap().room;
        let pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.room = room;
        let id = g.id();
        let mut medusa = monster::create(id, 12, pos, g.depth, &mut g.rng);
        medusa.awake = true;
        g.monsters.push(medusa);
        g.begin_command();
        assert!(g.player.conditions.confused);
        assert_ne!(g.monsters[0].flags & monster::GAZE_USED, 0);
        g.player.conditions.confused = false;
        g.begin_command();
        assert!(!g.player.conditions.confused);
    }

    #[test]
    fn attacking_reveals_a_xeroc_disguise() {
        let mut g = Game::new(207);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        g.monsters
            .push(monster::create(id, 23, pos, g.depth, &mut g.rng));
        assert_ne!(g.monsters[0].disguise, 'X');
        let hp = g.monsters[0].hp;
        g.player_attack(0);
        assert_eq!(g.monsters[0].disguise, 'X');
        assert_eq!(g.monsters[0].hp, hp);
        assert!(g.messages.last().unwrap().contains("kserok"));
    }

    #[test]
    fn attacking_xeroc_reveals_before_its_hallucinated_attack_name() {
        let mut g = Game::new(2071);
        g.monsters.clear();
        g.player.conditions.hallucinating = true;
        g.options.terse = true;
        let pos = g.player.pos.offset(1, 0);
        let mut xeroc = monster::create(g.id(), 23, pos, g.depth, &mut g.rng);
        xeroc.level = -100;
        let id = xeroc.id;
        g.monsters.push(xeroc);
        g.hallucinated_monsters.push((id, '?'));
        g.rng = GameRng::new(2072);
        let mut expected_rng = g.rng;
        let shown_kind = expected_rng.rnd(26) as usize;

        g.monster_attack(0);

        assert_eq!(g.monsters[0].disguise, 'X');
        assert_eq!(
            g.hallucinated_monsters,
            vec![(id, (b'A' + shown_kind as u8) as char)]
        );
        assert!(
            g.messages
                .last()
                .unwrap()
                .starts_with(&uppercase_first(&crate::lang::phrase(
                    &crate::lang::MONSTER_LEX[shown_kind],
                    Case::Nom,
                    interslavic::Number::Singular
                )))
        );
    }

    #[test]
    fn zero_damage_ice_monster_hit_still_freezes_player() {
        let mut g = Game::new(210);
        g.monsters.clear();
        let id = g.id();
        let mut ice = monster::create(id, 8, g.player.pos.offset(1, 0), g.depth, &mut g.rng);
        ice.level = 100;
        g.monsters.push(ice);
        let hp = g.player.stats.hp;

        g.monster_attack(0);

        assert_eq!(g.player.stats.hp, hp);
        assert!(g.player.conditions.asleep_turns >= 2);
        assert!(!g.player_is_running);
        assert_eq!(g.messages.last().unwrap(), "leděno čudovišče tę zamražaje");
    }

    #[test]
    fn monster_attack_gets_reference_bonus_until_player_is_running() {
        let attack = Attack {
            level: 2,
            strength: 10,
            hit_bonus: 0,
            damage_bonus: 0,
        };
        let seed = (1..1000)
            .find(|seed| {
                let vulnerable =
                    combat::resolve_outcome(&mut GameRng::new(*seed), attack, 6, "1x8", false);
                let running =
                    combat::resolve_outcome(&mut GameRng::new(*seed), attack, 6, "1x8", true);
                vulnerable.hit && !running.hit
            })
            .expect("the four-point defender penalty must affect a seed");

        let mut vulnerable = Game::new(236);
        vulnerable.monsters.clear();
        vulnerable.player.stats.hp = 100;
        vulnerable.player.stats.max_hp = 100;
        let mut zombie = monster::create(
            vulnerable.id(),
            25,
            vulnerable.player.pos.offset(1, 0),
            vulnerable.depth,
            &mut vulnerable.rng,
        );
        zombie.level = 2;
        vulnerable.monsters.push(zombie);
        vulnerable.rng = GameRng::new(seed);
        vulnerable.player_is_running = false;
        let mut running = vulnerable.clone();
        running.player_is_running = true;

        vulnerable.monster_attack(0);
        running.monster_attack(0);

        assert!(vulnerable.player.stats.hp < 100);
        assert_eq!(running.player.stats.hp, 100);
    }

    #[test]
    fn ice_monster_miss_is_silent() {
        let mut g = Game::new(235);
        g.monsters.clear();
        let mut ice = monster::create(g.id(), 8, g.player.pos.offset(1, 0), g.depth, &mut g.rng);
        ice.level = -100;
        g.monsters.push(ice);
        let messages = g.messages.len();

        g.monster_attack(0);

        assert_eq!(g.messages.len(), messages);
        assert_eq!(g.player.conditions.asleep_turns, 0);
    }

    #[test]
    fn fight_command_repeats_until_its_target_is_defeated() {
        let mut g = Game::new(211);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        let mut monster = monster::create(id, 25, pos, g.depth, &mut g.rng);
        monster.hp = 1;
        monster.max_hp = 1;
        monster.armor = 20;
        g.monsters.push(monster);

        let result = g.fight_direction(Direction::Right, false);

        assert!(!result.consumed_turn);
        assert!(g.monsters.is_empty());
        assert!(g.turn >= 1);
    }

    #[test]
    fn fight_command_cannot_target_an_unseen_phantom() {
        let mut g = Game::new(212);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        g.monsters
            .push(monster::create(id, 15, pos, g.depth, &mut g.rng));

        let result = g.fight_direction(Direction::Right, false);

        assert!(!result.consumed_turn);
        assert_eq!(g.monsters.len(), 1);
        assert_eq!(g.messages.last().unwrap(), "ne vidiš tam čudovišče");
    }

    #[test]
    fn fight_command_requires_both_side_squares_for_a_diagonal_target() {
        let mut g = Game::new(2120);
        g.monsters.clear();
        let target = g.player.pos.offset(1, -1);
        for pos in [
            target,
            g.player.pos.offset(1, 0),
            g.player.pos.offset(0, -1),
        ] {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        }
        g.dungeon
            .map
            .get_mut(g.player.pos.offset(1, 0))
            .unwrap()
            .terrain = Terrain::Void;
        let monster = monster::create(g.id(), 0, target, g.depth, &mut g.rng);
        let hp = monster.hp;
        g.monsters.push(monster);

        assert_eq!(
            g.fight_direction(Direction::UpRight, false),
            CommandResult::FREE
        );

        assert_eq!(g.monsters[0].hp, hp);
    }

    #[test]
    fn blind_fight_command_requires_monster_detection_and_uses_the_terse_form() {
        let mut g = Game::new(2121);
        g.monsters.clear();
        g.player.conditions.blind = true;
        g.options.terse = true;
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let monster = monster::create(g.id(), 0, pos, g.depth, &mut g.rng);
        let hp = monster.hp;
        g.monsters.push(monster);

        g.fight_direction(Direction::Right, false);

        assert_eq!(g.monsters[0].hp, hp);
        assert_eq!(g.messages.last().unwrap(), "tam ne jest čudovišča");
    }

    #[test]
    fn fight_to_death_suppresses_ordinary_attack_but_reports_actual_rust() {
        let mut g = Game::new(2122);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        let mut aquator = monster::create(id, 0, pos, g.depth, &mut g.rng);
        aquator.level = 100;
        aquator.awake = true;
        g.monsters.push(aquator);
        g.fight_target = Some(id);
        g.fight_kamikaze = true;
        let armor = g.player.armor.unwrap();
        let before_armor = g
            .player
            .inventory
            .iter()
            .find(|item| item.id == armor)
            .unwrap()
            .armor_class;
        g.monster_attack(0);

        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("tvoja brȯnja sejčas izględaje slaběje. O ne!")
        );
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == armor)
                .unwrap()
                .armor_class,
            before_armor.map(|value| value + 1)
        );
    }

    #[test]
    fn normal_fight_safety_tracks_the_largest_hit_not_aggregate_damage() {
        let mut g = Game::new(2123);
        g.fight_target = Some(1);
        g.player.stats.hp = 7;

        g.record_fight_hit(3);
        g.player.stats.hp = 4;
        g.record_fight_hit(3);
        assert_eq!(g.fight_target, Some(1));

        g.player.stats.hp = 3;
        g.record_fight_hit(1);
        assert_eq!(g.fight_target, None);
    }

    #[test]
    fn defeat_messages_preserve_visibility_hallucination_and_terse_forms() {
        let mut unseen = Game::new(2124);
        unseen.monsters.clear();
        let pos = unseen.player.pos.offset(1, 0);
        let mut phantom = monster::create(unseen.id(), 15, pos, unseen.depth, &mut unseen.rng);
        phantom.flags |= monster::INVISIBLE;
        unseen.monsters.push(phantom);

        unseen.kill_monster(0);
        assert_eq!(
            unseen
                .messages
                .iter()
                .rev()
                .find(|message| message.contains("ubivaješ"))
                .unwrap(),
            "ubivaješ něčto"
        );

        let mut hallucinating = Game::new(2125);
        hallucinating.monsters.clear();
        hallucinating.options.terse = true;
        hallucinating.player.conditions.hallucinating = true;
        let pos = hallucinating.player.pos.offset(1, 0);
        let monster = monster::create(
            hallucinating.id(),
            25,
            pos,
            hallucinating.depth,
            &mut hallucinating.rng,
        );
        let id = monster.id;
        hallucinating.monsters.push(monster);
        hallucinating.hallucinated_monsters.push((id, 'B'));

        hallucinating.kill_monster(0);
        assert_eq!(
            hallucinating
                .messages
                .iter()
                .rev()
                .find(|message| message.contains("ubivaješ"))
                .unwrap(),
            "ubivaješ netopyŕa"
        );
    }

    #[test]
    fn add_hit_ring_does_not_modify_unarmed_combat() {
        let mut with_ring = Game::new(2126);
        with_ring.monsters.clear();
        with_ring.player.weapon = None;
        let ring_id = with_ring.id();
        let mut ring = Item::basic(ring_id, ItemKind::Ring, 7);
        ring.armor_class = Some(100);
        with_ring.player.inventory.push(ring);
        with_ring.player.rings[0] = Some(ring_id);
        let pos = with_ring.player.pos.offset(1, 0);
        let mut monster =
            monster::create(with_ring.id(), 0, pos, with_ring.depth, &mut with_ring.rng);
        monster.hp = 100;
        monster.max_hp = 100;
        with_ring.monsters.push(monster);
        let mut unadorned = with_ring.clone();
        unadorned.player.rings[0] = None;

        with_ring.player_attack(0);
        unadorned.player_attack(0);

        assert_eq!(with_ring.monsters[0].hp, unadorned.monsters[0].hp);
        assert_eq!(with_ring.rng, unadorned.rng);
    }

    #[test]
    fn poison_ends_hallucination_unless_sustain_strength_blocks_it() {
        let mut g = Game::new(213);
        g.player.inventory.clear();
        g.player.conditions.hallucinating = true;
        g.scheduler.add_or_lengthen(Effect::Hallucination, 100);
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Potion, 2));

        g.quaff(id);

        assert!(!g.player.conditions.hallucinating);
        assert!(
            g.scheduler
                .fuses
                .iter()
                .all(|fuse| fuse.effect != Effect::Hallucination)
        );
    }

    #[test]
    fn create_monster_scroll_uses_an_adjacent_legal_square() {
        let mut g = Game::new(214);
        g.monsters.clear();
        g.floor_items.clear();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx != 0 || dy != 0 {
                    g.dungeon
                        .map
                        .get_mut(g.player.pos.offset(dx, dy))
                        .unwrap()
                        .terrain = Terrain::Void;
                }
            }
        }
        let expected = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(expected).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Scroll, 14));

        g.read_scroll(id);

        assert_eq!(g.monsters.len(), 1);
        assert_eq!(g.monsters[0].pos, expected);
    }

    #[test]
    fn magic_mapping_reveals_hidden_structure_but_not_ordinary_floors() {
        let mut g = Game::new(215);
        let ordinary = Pos::new(2, 2);
        let trapped = Pos::new(3, 2);
        for pos in [ordinary, trapped] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Floor;
            cell.seen = false;
            cell.remembered = ' ';
            cell.trap = None;
            cell.trap_revealed = false;
        }
        g.dungeon.map.get_mut(trapped).unwrap().trap = Some(Trap::Arrow);
        let id = g.id();
        g.player
            .inventory
            .push(Item::basic(id, ItemKind::Scroll, 1));

        g.read_scroll(id);

        assert!(!g.dungeon.map.get(ordinary).unwrap().seen);
        let trap = g.dungeon.map.get(trapped).unwrap();
        assert!(trap.seen);
        assert!(trap.trap_revealed);
        assert_eq!(trap.remembered, '^');
    }

    #[test]
    fn wearing_and_removing_armor_each_waste_an_extra_turn() {
        let mut wearing = Game::new(217);
        wearing.monsters.clear();
        wearing.wandering_countdown = 10_000;
        wearing.player.armor = None;
        let armor = wearing.id();
        wearing
            .player
            .inventory
            .push(Item::basic(armor, ItemKind::Armor, 1));
        let result = wearing.wear(armor);
        wearing.finish_action(result);
        assert_eq!(wearing.turn, 2);
        assert!(
            wearing
                .player
                .inventory
                .iter()
                .find(|item| item.id == armor)
                .unwrap()
                .known
        );

        let mut removing = Game::new(218);
        removing.monsters.clear();
        removing.wandering_countdown = 10_000;
        let result = removing.take_off();
        removing.finish_action(result);
        assert_eq!(removing.turn, 2);
        assert!(removing.player.armor.is_none());
    }

    #[test]
    fn wielded_armor_can_be_worn_and_takeoff_clears_both_reference_slots() {
        let mut g = Game::new(220);
        g.monsters.clear();
        g.wandering_countdown = 10_000;
        g.player.armor = None;
        let armor = g.id();
        g.player
            .inventory
            .push(Item::basic(armor, ItemKind::Armor, 1));
        g.player.weapon = Some(armor);

        let wear = g.wear(armor);
        g.finish_action(wear);
        assert_eq!(g.turn, 2);
        assert_eq!(g.player.weapon, Some(armor));
        assert_eq!(g.player.armor, Some(armor));

        let take_off = g.take_off();
        g.finish_action(take_off);
        assert_eq!(g.turn, 3);
        assert!(g.player.weapon.is_none());
        assert!(g.player.armor.is_none());
    }

    #[test]
    fn wearing_armor_while_armored_uses_reference_message_and_no_turn() {
        let mut g = Game::new(2200);
        let armor = g.player.armor.unwrap();

        let result = g.wear(armor);

        assert!(!result.consumed_turn);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("uže nosiš brȯnjų.  Pŕvo musiš sjęti jų")
        );

        g.options.terse = true;
        g.wear(armor);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("uže nosiš brȯnjų")
        );
    }

    #[test]
    fn throwing_shared_slot_armor_clears_weapon_but_refuses_current_armor() {
        let mut g = Game::new(2201);
        g.player.armor = None;
        let armor = g.id();
        g.player
            .inventory
            .push(Item::basic(armor, ItemKind::Armor, 1));
        g.player.weapon = Some(armor);
        g.player.armor = Some(armor);

        let result = g.throw_item(armor, Direction::Right);

        assert!(result.consumed_turn);
        assert_eq!(g.player.weapon, None);
        assert_eq!(g.player.armor, Some(armor));
        assert!(g.player.inventory.iter().any(|item| item.id == armor));
        assert_eq!(g.messages.last().unwrap(), "to uže koristaješ");
    }

    #[test]
    fn armor_waste_time_runs_daemons_but_automatic_ring_search_only_once() {
        let mut actual = Game::new(204);
        actual.player.armor = None;
        let armor_id = actual.id();
        actual
            .player
            .inventory
            .push(Item::basic(armor_id, ItemKind::Armor, 0));
        let ring_id = actual.id();
        let ring = Item::basic(ring_id, ItemKind::Ring, 3);
        actual.player.inventory.push(ring);
        actual.player.rings[0] = Some(ring_id);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx != 0 || dy != 0 {
                    actual
                        .dungeon
                        .map
                        .get_mut(actual.player.pos.offset(dx, dy))
                        .unwrap()
                        .terrain = Terrain::SecretDoor;
                }
            }
        }
        let mut expected = actual.clone();
        expected.after_turn();
        expected.after_turn();
        expected.search();

        let result = actual.wear(armor_id);
        actual.finish_action(result);
        assert_eq!(actual.turn, expected.turn);
        assert_eq!(actual.rng, expected.rng);
        assert_eq!(actual.dungeon.map, expected.dungeon.map);
    }

    #[test]
    fn armor_protection_scroll_does_not_remove_a_curse() {
        let mut g = Game::new(219);
        let armor = g.player.armor.unwrap();
        g.player
            .inventory
            .iter_mut()
            .find(|item| item.id == armor)
            .unwrap()
            .cursed = true;
        let scroll = g.id();
        g.player
            .inventory
            .push(Item::basic(scroll, ItemKind::Scroll, 17));

        g.read_scroll(scroll);

        let armor = g
            .player
            .inventory
            .iter()
            .find(|item| item.id == armor)
            .unwrap();
        assert!(armor.protected);
        assert!(armor.cursed);
    }

    #[test]
    fn starvation_uses_one_food_per_turn_and_the_original_boundary() {
        let mut g = Game::new(220);
        g.has_amulet = true;
        g.player.food_left = -850;

        g.digest();
        assert_eq!(g.player.food_left, -851);
        assert_eq!(g.end, EndState::Playing);

        g.digest();
        assert_eq!(g.player.food_left, -852);
        assert_eq!(g.end, EndState::Dead);
        assert_eq!(g.death_cause.as_deref(), Some("glåda"));
    }

    #[test]
    fn ordinary_food_can_taste_awful_and_grant_experience() {
        let mut found = false;
        for seed in 1..200 {
            let mut g = Game::new(seed);
            g.player.inventory.clear();
            let food = g.id();
            g.player
                .inventory
                .push(Item::basic(food, ItemKind::Food, 0));
            g.eat(food);
            if g.player.stats.experience == 1 {
                assert_eq!(g.messages.last().unwrap(), "fuj, ta jeda imaje užasny vkųs");
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn eating_after_starvation_discards_the_negative_food_deficit() {
        let mut g = Game::new(221);
        g.player.inventory.clear();
        g.player.food_left = -800;
        g.hungry_state = 2;
        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));

        assert_eq!(g.eat(food), CommandResult::TURN);

        assert!((1100..1500).contains(&g.player.food_left));
        assert_eq!(g.hungry_state, 0);
    }

    #[test]
    fn eating_a_wielded_food_stack_clears_the_weapon_slot() {
        let mut g = Game::new(222);
        g.player.inventory.clear();
        let food = g.id();
        let mut stack = Item::basic(food, ItemKind::Food, 0);
        stack.count = 2;
        g.player.inventory.push(stack);
        g.player.weapon = Some(food);

        assert_eq!(g.eat(food), CommandResult::TURN);

        assert_eq!(g.player.weapon, None);
        assert_eq!(g.player.inventory[0].count, 1);
    }

    #[test]
    fn terse_attempt_to_eat_a_nonfood_item_consumes_a_turn_and_uses_reference_message() {
        let mut g = Game::new(223);
        g.monsters.clear();
        g.options.terse = true;
        let weapon = g.player.weapon.unwrap();
        let turn = g.turn;

        let result = g.eat(weapon);
        assert_eq!(result, CommandResult::TURN);
        g.finish_action(result);

        assert_eq!(g.turn, turn + 1);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("to ne jest jedlivo!")
        );
    }

    #[test]
    fn call_names_are_global_for_appearance_types_and_food_is_not_callable() {
        let mut g = Game::new(197);
        g.player.inventory.clear();
        let first = g.id();
        let second = g.id();
        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(first, ItemKind::Potion, 3));
        g.player
            .inventory
            .push(Item::basic(second, ItemKind::Potion, 3));
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));

        g.call_item(first, "muscle juice".into());

        assert_eq!(g.item_guess(&g.player.inventory[0]), Some("muscle juice"));
        assert_eq!(g.item_guess(&g.player.inventory[1]), Some("muscle juice"));
        assert!(g.player.inventory[0].label.is_none());

        g.call_item(food, "lunch".into());
        assert_eq!(g.messages.last().unwrap(), "ne možeš to nikako nazvati");
        assert!(g.player.inventory[2].label.is_none());
    }

    #[test]
    fn unidentified_potions_request_a_call_and_rogomatic_aliases_use_the_true_name() {
        let mut g = Game::new(1970);
        g.monsters.clear();
        g.player.inventory.clear();
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 6));

        assert_eq!(g.quaff(potion), CommandResult::TURN);
        assert_eq!(g.pending_call, Some((ItemKind::Potion, 6)));

        g.finish_pending_call("?".into());
        assert_eq!(g.pending_call, None);
        assert_eq!(g.knowledge.guesses[6].as_deref(), Some("čuťja čudovišč"));
    }

    #[test]
    fn invalid_drinks_cost_a_turn_and_wielded_potion_stacks_are_unwielded() {
        let mut g = Game::new(1972);
        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));
        assert_eq!(g.quaff(food), CommandResult::TURN);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("fuj! Začto hoćeš piti to?")
        );

        let potion = g.id();
        let mut stack = Item::basic(potion, ItemKind::Potion, 3);
        stack.count = 2;
        g.player.inventory.push(stack);
        g.player.weapon = Some(potion);
        assert_eq!(g.quaff(potion), CommandResult::TURN);
        assert_eq!(g.player.weapon, None);
        assert_eq!(
            g.player
                .inventory
                .iter()
                .find(|item| item.id == potion)
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn repeated_monster_detection_potions_create_independent_fuses() {
        let mut g = Game::new(1973);
        g.player.inventory.clear();
        let potion = g.id();
        let mut stack = Item::basic(potion, ItemKind::Potion, 6);
        stack.count = 2;
        g.player.inventory.push(stack);

        g.quaff(potion);
        g.quaff(potion);

        assert_eq!(
            g.scheduler
                .fuses
                .iter()
                .filter(|fuse| fuse.effect == Effect::MonsterDetection)
                .count(),
            2
        );
    }

    #[test]
    fn identified_consumables_clear_obsolete_guesses_without_prompting() {
        let mut g = Game::new(1971);
        g.player.inventory.clear();
        g.knowledge.potions[3] = true;
        g.knowledge.guesses[3] = Some("old guess".into());
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 3));

        g.quaff(potion);

        assert_eq!(g.pending_call, None);
        assert_eq!(g.knowledge.guesses[3], None);
    }

    #[test]
    fn inventory_names_preserve_reference_articles_counts_and_properties() {
        let mut g = Game::new(2290);
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.count = 2;
        let color = crate::lang::COLOR_ADJ[g.appearances.potion_colors[0]];
        assert_eq!(
            g.inventory_name(&potion, false),
            format!(
                "2 {} {}",
                crate::lang::adj_for(
                    color,
                    &crate::lang::POTION,
                    Case::Nom,
                    interslavic::Number::Plural
                ),
                crate::lang::decl(&crate::lang::POTION, Case::Nom, interslavic::Number::Plural)
            )
        );

        g.knowledge.potions[0] = true;
        assert_eq!(
            g.inventory_name(&potion, false),
            format!(
                "2 {} {}({color})",
                crate::lang::decl(&crate::lang::POTION, Case::Nom, interslavic::Number::Plural),
                crate::lang::potion_effect_gen(0)
            )
        );

        let mut weapon = Item::basic(g.id(), ItemKind::Weapon, 3);
        weapon.count = 8;
        weapon.hit_plus = 1;
        weapon.damage_plus = -2;
        weapon.known = true;
        assert_eq!(g.inventory_name(&weapon, false), "+1,-2 8 strěl");
        assert_eq!(g.inventory_name(&weapon, true), "+1,-2 8 strěl");

        let food = Item::basic(g.id(), ItemKind::Food, 0);
        assert_eq!(g.inventory_name(&food, false), "porcija jedy");
        assert_eq!(g.inventory_name(&food, true), "porcija jedy");
    }

    #[test]
    fn failed_healing_at_full_health_preserves_quiet_counter() {
        let mut g = Game::new(221);
        g.player.stats.level = 1;
        g.player.stats.hp = g.player.stats.max_hp;
        g.quiet_turns = 19;

        g.heal();

        assert_eq!(g.player.stats.hp, g.player.stats.max_hp);
        assert_eq!(g.quiet_turns, 20);
    }

    #[test]
    fn doctor_applies_one_natural_gain_plus_each_regeneration_ring() {
        let mut low = Game::new(216);
        low.player.stats.hp = 1;
        low.player.stats.max_hp = 20;
        low.player.stats.level = 1;
        low.quiet_turns = 18;
        low.heal();
        assert_eq!(low.player.stats.hp, 2);
        assert_eq!(low.quiet_turns, 0);

        let mut with_rings = Game::new(217);
        with_rings.player.stats.hp = 1;
        with_rings.player.stats.max_hp = 20;
        with_rings.player.stats.level = 1;
        with_rings.quiet_turns = 18;
        for hand in 0..2 {
            let id = with_rings.id();
            with_rings
                .player
                .inventory
                .push(Item::basic(id, ItemKind::Ring, 9));
            with_rings.player.rings[hand] = Some(id);
        }
        with_rings.heal();
        assert_eq!(with_rings.player.stats.hp, 4);
        assert_eq!(with_rings.quiet_turns, 0);
    }

    #[test]
    fn timed_effects_use_their_reference_expiration_messages() {
        let mut g = Game::new(222);
        g.player.conditions.levitating = true;
        g.scheduler.add_or_lengthen(Effect::Levitation, 1);

        g.tick_effects();

        assert!(!g.player.conditions.levitating);
        assert_eq!(g.messages.last().unwrap(), "legko spušćaješ sę na zemjų");
    }

    #[test]
    fn hallucination_selects_alternate_hunger_and_expiration_messages() {
        let mut confused = Game::new(219);
        confused.player.conditions.hallucinating = true;
        confused.player.conditions.confused = true;
        confused.scheduler.add_or_lengthen(Effect::Confusion, 1);
        confused.tick_effects();
        assert_eq!(
            confused.messages.last().unwrap(),
            "sejčas čuješ sę menje kosmično"
        );

        let mut hungry = Game::new(220);
        hungry.player.conditions.hallucinating = true;
        hungry.player.food_left = 300;
        hungry.digest();
        assert_eq!(hungry.hungry_state, 1);
        assert_eq!(hungry.messages.last().unwrap(), "imaješ veliky apetit");

        let mut landing = Game::new(221);
        landing.player.conditions.hallucinating = true;
        landing.player.conditions.levitating = true;
        landing.scheduler.add_or_lengthen(Effect::Levitation, 1);
        landing.tick_effects();
        assert_eq!(landing.messages.last().unwrap(), "buh!  Padaješ na zemjų");
    }

    #[test]
    fn bear_trap_counter_decrements_only_on_movement_attempts() {
        let mut g = Game::new(223);
        g.monsters.clear();
        g.player.conditions.held_turns = 2;

        g.execute(Command::Rest);
        assert_eq!(g.player.conditions.held_turns, 2);
        g.execute(Command::Move(Direction::Right));
        assert_eq!(g.player.conditions.held_turns, 1);
    }

    #[test]
    fn second_haste_potion_cancels_haste_and_can_faint_for_zero_to_seven_turns() {
        let mut g = Game::new(225);
        g.player.conditions.hasted = true;
        g.scheduler.add_or_lengthen(Effect::Haste, 20);
        let potion = g.id();
        g.player
            .inventory
            .push(Item::basic(potion, ItemKind::Potion, 10));

        g.quaff(potion);

        assert!(!g.player.conditions.hasted);
        assert!(g.player.conditions.asleep_turns <= 7);
        assert!(
            g.scheduler
                .fuses
                .iter()
                .all(|fuse| fuse.effect != Effect::Haste)
        );
    }

    #[test]
    fn held_player_can_still_attack_the_holding_flytrap() {
        let mut g = Game::new(224);
        g.monsters.clear();
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let id = g.id();
        let mut flytrap = monster::create(id, 5, pos, g.depth, &mut g.rng);
        flytrap.hp = 10_000;
        flytrap.max_hp = 10_000;
        g.monsters.push(flytrap);
        g.flytrap_holder = Some(id);
        let hp = g.monsters[0].hp;
        g.player.stats.level = 100;

        g.move_player(Direction::Right);

        assert!(g.monsters[0].hp < hp);
    }

    #[test]
    fn teleport_trap_clears_bear_and_flytrap_holds_without_extra_message() {
        let mut g = Game::new(226);
        g.player.conditions.held_turns = 5;
        g.flytrap_holder = Some(999);
        g.flytrap_hits = 4;
        let cell = g.dungeon.map.get_mut(g.player.pos).unwrap();
        cell.trap = Some(Trap::Teleport);
        cell.trap_revealed = false;

        g.trigger_trap();

        assert_eq!(g.player.conditions.held_turns, 0);
        assert!(g.flytrap_holder.is_none());
        assert_eq!(g.flytrap_hits, 0);
        assert_ne!(
            g.messages.last().map(String::as_str),
            Some("you are teleported")
        );
    }

    #[test]
    fn concealed_floor_trap_precedes_the_flytrap_hold_check() {
        let mut g = Game::new(2260);
        g.monsters.clear();
        g.floor_items.clear();
        let holder_pos = g.player.pos.offset(-1, 0);
        g.dungeon.map.get_mut(holder_pos).unwrap().terrain = Terrain::Floor;
        let holder_id = g.id();
        g.monsters.push(monster::create(
            holder_id, 5, holder_pos, g.depth, &mut g.rng,
        ));
        g.flytrap_holder = Some(holder_id);
        let trap_pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(trap_pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::Bear);
        cell.trap_revealed = false;

        assert_eq!(g.move_player(Direction::Right), CommandResult::TURN);

        assert_eq!(g.player.pos, trap_pos);
        assert_eq!(g.flytrap_holder, Some(holder_id));
        assert!(g.player.conditions.held_turns > 0);
        assert_eq!(g.messages.last().unwrap(), "medvěďja pasť tę lovi");
    }

    #[test]
    fn object_glyph_suppresses_a_concealed_trap_until_the_object_is_gone() {
        let mut g = Game::new(2261);
        g.monsters.clear();
        g.floor_items.clear();
        let start = g.player.pos;
        let trap_pos = start.offset(1, 0);
        let cell = g.dungeon.map.get_mut(trap_pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::Bear);
        cell.trap_revealed = false;
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(trap_pos);
        g.floor_items.push(potion);

        assert_eq!(g.move_player(Direction::Right), CommandResult::TURN);

        assert_eq!(g.player.pos, trap_pos);
        assert_eq!(g.player.conditions.held_turns, 0);
        assert!(!g.dungeon.map.get(trap_pos).unwrap().trap_revealed);
        assert!(g.floor_items.is_empty());

        assert_eq!(g.move_player(Direction::Left), CommandResult::TURN);
        assert_eq!(g.move_player(Direction::Right), CommandResult::TURN);
        assert!(g.player.conditions.held_turns > 0);
        assert!(g.dungeon.map.get(trap_pos).unwrap().trap_revealed);
    }

    #[test]
    fn trap_identification_uses_the_original_player_facing_name() {
        let mut g = Game::new(227);
        let pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::PoisonDart);
        cell.trap_revealed = true;

        g.identify_trap(Direction::Right);

        assert_eq!(g.messages.last().unwrap(), "Nahodiš: jadna pasť");
        assert!(g.dungeon.map.get(pos).unwrap().trap_revealed);
    }

    #[test]
    fn hallucinated_trap_identification_randomizes_an_already_revealed_trap() {
        let mut g = Game::new(225);
        g.player.conditions.hallucinating = true;
        let pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::PoisonDart);
        cell.trap_revealed = true;

        assert_eq!(g.identify_trap(Direction::Right), CommandResult::FREE);

        assert!(g.dungeon.map.get(pos).unwrap().trap_revealed);
        assert!(g.messages.last().unwrap().starts_with("Nahodiš: "));
    }

    #[test]
    fn trap_identification_does_not_reveal_a_hidden_trap() {
        let mut g = Game::new(2250);
        let pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = Some(Trap::PoisonDart);
        cell.trap_revealed = false;

        assert_eq!(g.identify_trap(Direction::Right), CommandResult::FREE);

        assert!(!g.dungeon.map.get(pos).unwrap().trap_revealed);
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("ne nahodiš tam pasť")
        );
    }

    #[test]
    fn verbose_trap_identification_prefixes_the_no_trap_message() {
        let mut g = Game::new(226);
        let pos = g.player.pos.offset(1, 0);
        let cell = g.dungeon.map.get_mut(pos).unwrap();
        cell.terrain = Terrain::Floor;
        cell.trap = None;

        assert_eq!(g.identify_trap(Direction::Right), CommandResult::FREE);

        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("ne nahodiš tam pasť")
        );
    }

    #[test]
    fn missed_trap_arrow_can_land_under_a_monster() {
        let mut g = Game::new(237);
        g.monsters.clear();
        g.floor_items.clear();
        g.player.stats.armor = -100;
        let landing = g.player.pos.offset(1, 0);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let pos = g.player.pos.offset(dx, dy);
                g.dungeon.map.get_mut(pos).unwrap().terrain =
                    if pos == g.player.pos || pos == landing {
                        Terrain::Floor
                    } else {
                        Terrain::Void
                    };
            }
        }
        g.dungeon.map.get_mut(g.player.pos).unwrap().trap = Some(Trap::Arrow);
        let monster_id = g.id();
        g.monsters.push(monster::create(
            monster_id, 25, landing, g.depth, &mut g.rng,
        ));
        let mut expected_rng = g.rng;
        assert!(!combat::swing(
            &mut expected_rng,
            g.player.stats.level - 1,
            g.player.stats.armor,
            1,
        ));
        let _ = expected_rng.rnd(8);
        let _ = expected_rng.rnd(1);

        g.trigger_trap();

        assert!(g.floor_items.iter().any(|item| {
            item.kind == ItemKind::Weapon && item.which == 3 && item.pos == Some(landing)
        }));
        assert_eq!(g.messages.last().unwrap(), "strěla leti mimo tebe");
        assert_eq!(g.rng, expected_rng);
    }

    #[test]
    fn wandering_carrier_targets_and_collects_an_unclaimed_room_item() {
        let mut g = Game::new(208);
        g.monsters.clear();
        g.floor_items.clear();
        let rooms: Vec<_> = g
            .dungeon
            .rooms
            .iter()
            .filter(|room| !room.gone)
            .take(2)
            .cloned()
            .collect();
        assert_eq!(rooms.len(), 2);
        g.player.pos = rooms[0].center();
        let monster_pos = rooms[1].center();
        let item_pos = monster_pos.offset(1, 0);
        for pos in [monster_pos, item_pos] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(rooms[1].id);
        }
        let id = g.id();
        let mut dragon = monster::create(id, 3, monster_pos, g.depth, &mut g.rng);
        dragon.awake = true;
        g.monsters.push(dragon);
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(item_pos);
        g.floor_items.push(potion);

        g.monsters[0].destination = g.find_monster_item_destination(0);
        assert_eq!(g.monsters[0].destination, Some(item_pos));
        g.move_monster_step(0);
        assert!(g.floor_items.is_empty());
        assert_eq!(g.monsters[0].inventory.len(), 1);
        assert!(g.monsters[0].awake);
        assert_eq!(g.monsters[0].destination, None);
    }

    #[test]
    fn item_seeking_monsters_scan_the_newest_floor_object_first() {
        let mut g = Game::new(2080);
        g.monsters.clear();
        g.floor_items.clear();
        let rooms: Vec<_> = g
            .dungeon
            .rooms
            .iter()
            .filter(|room| !room.gone && !room.maze)
            .take(2)
            .cloned()
            .collect();
        assert_eq!(rooms.len(), 2);
        g.player.pos = rooms[0].center();
        let monster_pos = rooms[1].center();
        let older_pos = monster_pos.offset(1, 0);
        let newer_pos = monster_pos.offset(0, 1);
        for pos in [monster_pos, older_pos, newer_pos] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(rooms[1].id);
            cell.passage = None;
        }
        let dragon = monster::create(g.id(), 3, monster_pos, g.depth, &mut g.rng);
        g.monsters.push(dragon);
        for pos in [older_pos, newer_pos] {
            let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
            potion.pos = Some(pos);
            g.floor_items.push(potion);
        }

        assert_eq!(g.find_monster_item_destination(0), Some(newer_pos));
    }

    #[test]
    fn find_destination_treats_the_callers_existing_target_as_reserved() {
        let mut g = Game::new(2081);
        g.monsters.clear();
        g.floor_items.clear();
        let rooms: Vec<_> = g
            .dungeon
            .rooms
            .iter()
            .filter(|room| !room.gone && !room.maze)
            .take(2)
            .cloned()
            .collect();
        assert_eq!(rooms.len(), 2);
        g.player.pos = rooms[0].center();
        let monster_pos = rooms[1].center();
        let older_pos = monster_pos.offset(1, 0);
        let newer_pos = monster_pos.offset(0, 1);
        for pos in [monster_pos, older_pos, newer_pos] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = Some(rooms[1].id);
            cell.passage = None;
        }
        let mut dragon = monster::create(g.id(), 3, monster_pos, g.depth, &mut g.rng);
        dragon.destination = Some(newer_pos);
        g.monsters.push(dragon);
        for pos in [older_pos, newer_pos] {
            let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
            potion.pos = Some(pos);
            g.floor_items.push(potion);
        }

        assert_eq!(g.find_monster_item_destination(0), Some(older_pos));
    }

    #[test]
    fn stale_object_destination_remains_a_coordinate_target() {
        let mut g = Game::new(2082);
        g.monsters.clear();
        g.floor_items.clear();
        let start = Pos::new(40, 12);
        let stale_destination = start.offset(1, 0);
        g.player.pos = Pos::new(45, 12);
        for pos in [start, stale_destination, g.player.pos] {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        }
        let mut zombie = monster::create(g.id(), 25, start, g.depth, &mut g.rng);
        zombie.awake = true;
        zombie.destination = Some(stale_destination);
        g.monsters.push(zombie);

        g.move_monster_step(0);

        assert_eq!(g.monsters[0].pos, stale_destination);
        assert!(!g.monsters[0].awake);
    }

    #[test]
    fn moving_fight_target_cancels_fight_to_death() {
        let mut g = Game::new(2083);
        g.monsters.clear();
        let start = Pos::new(40, 12);
        g.player.pos = Pos::new(45, 12);
        for dx in 0..=5 {
            g.dungeon
                .map
                .get_mut(Pos::new(40 + dx, 12))
                .unwrap()
                .terrain = Terrain::Floor;
        }
        let mut zombie = monster::create(g.id(), 25, start, g.depth, &mut g.rng);
        zombie.awake = true;
        let id = zombie.id;
        g.monsters.push(zombie);
        g.fight_target = Some(id);
        g.fight_kamikaze = true;

        g.move_monsters();

        assert_ne!(g.monsters[0].pos, start);
        assert_eq!(g.fight_target, None);
        assert!(g.fight_kamikaze);
    }

    #[test]
    fn wandering_monsters_never_spawn_in_the_players_room() {
        let mut g = Game::new(227);
        let player_room = g
            .dungeon
            .map
            .get(g.player.pos)
            .and_then(|cell| cell.room)
            .unwrap();
        for _ in 0..100 {
            g.monsters.clear();
            g.spawn_random_monster(true);
            let monster = g.monsters.first().expect("another room is available");
            assert_ne!(
                g.dungeon.map.get(monster.pos).and_then(|cell| cell.room),
                Some(player_room)
            );
        }
    }

    #[test]
    fn wandering_spawn_uses_find_floor_rejection_rng_before_monster_creation() {
        let mut g = Game::new(2270);
        g.monsters.clear();
        g.floor_items.clear();
        g.player.conditions.detect_monsters = true;
        g.player.conditions.hallucinating = true;
        let mut expected = g.clone();
        let position = (0..500)
            .find_map(|_| {
                let pos = expected.reference_monster_floor_position()?;
                (expected.area_key(pos) != expected.area_key(expected.player.pos)).then_some(pos)
            })
            .unwrap();
        let kind = monster::random_kind(&mut expected.rng, expected.depth, true);
        let created = expected.make_monster(kind, position, expected.depth, true, false);
        expected.monsters.insert(0, created);
        let index = 0;
        let id = expected.monsters[index].id;
        let glyph = (b'A' + expected.rng.rnd(26) as u8) as char;
        expected.hallucinated_monsters.push((id, glyph));
        expected.monsters[index].destination = expected.find_monster_item_destination(index);

        g.spawn_random_monster(true);

        assert_eq!(g.monsters, expected.monsters);
        assert_eq!(g.hallucinated_monsters, expected.hallucinated_monsters);
        assert_eq!(g.rng, expected.rng);
    }

    #[test]
    fn newly_created_monsters_are_the_head_of_the_movement_list() {
        let mut g = Game::new(2271);
        g.monsters.clear();
        let room = g.dungeon.rooms.iter().find(|room| !room.gone).unwrap().id;

        g.spawn_monster_in_room(room, false);
        let first = g.monsters[0].id;
        g.spawn_monster_in_room(room, false);

        assert_ne!(g.monsters[0].id, first);
        assert_eq!(g.monsters[1].id, first);
    }

    #[test]
    fn only_reference_give_pack_call_sites_generate_monster_inventory() {
        let mut g = Game::new(236);
        g.depth = 10;
        g.max_depth = 10;
        let pos = g.player.pos.offset(2, 0);

        let wandering_or_created = g.make_monster(3, pos, g.depth, true, false);
        let initial_room_monster = g.make_monster(3, pos, g.depth, false, true);

        assert!(wandering_or_created.inventory.is_empty());
        assert_eq!(initial_room_monster.inventory.len(), 1);
    }

    #[test]
    fn terse_option_selects_the_short_c_messages() {
        let mut g = Game::new(209);
        g.options.terse = true;
        g.player.armor = None;
        g.take_off();
        assert_eq!(g.messages.last().unwrap(), "ne nosiš brȯnjų");

        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));
        g.put_on_ring(food, 0);
        assert_eq!(g.messages.last().unwrap(), "to ne jest pŕstenj");

        g.player.inventory.clear();
        for _ in 0..MAX_PACK {
            let id = g.id();
            g.player.inventory.push(Item::basic(id, ItemKind::Food, 0));
        }
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(g.player.pos);
        g.floor_items.push(potion);
        g.pickup();
        assert_eq!(
            &g.messages[g.messages.len() - 2..],
            ["ne jest města", "tu: srěbrny napitȯk"]
        );

        g.floor_items.clear();
        g.pickup();
        assert_eq!(g.messages.last().unwrap(), "tu ničego ne jest");
    }

    #[test]
    fn manual_pickup_uses_reference_levitation_and_gold_messages() {
        let mut levitating = Game::new(2090);
        let mut food = Item::basic(levitating.id(), ItemKind::Food, 0);
        food.pos = Some(levitating.player.pos);
        levitating.floor_items.push(food);
        levitating.player.conditions.levitating = true;

        assert_eq!(levitating.pickup(), CommandResult::TURN);
        assert_eq!(
            levitating.messages.last().unwrap(),
            "ne možeš.  Letiš nad zemjejų!"
        );

        let mut gold_game = Game::new(2091);
        gold_game.floor_items.clear();
        let mut gold = Item::gold(gold_game.id(), 123);
        gold.pos = Some(gold_game.player.pos);
        gold_game.floor_items.push(gold);
        let room = gold_game
            .dungeon
            .map
            .get(gold_game.player.pos)
            .unwrap()
            .room
            .unwrap();
        gold_game.dungeon.rooms[room as usize].gold = Some(gold_game.player.pos);
        gold_game.dungeon.rooms[room as usize].gold_value = 123;

        assert_eq!(gold_game.pickup(), CommandResult::TURN);
        assert_eq!(gold_game.messages.last().unwrap(), "nahodiš 123 zlåtnikov");
        assert_eq!(gold_game.dungeon.rooms[room as usize].gold_value, 0);

        gold_game.options.terse = true;
        let mut more_gold = Item::gold(gold_game.id(), 7);
        more_gold.pos = Some(gold_game.player.pos);
        gold_game.floor_items.push(more_gold);
        gold_game.pickup();
        assert_eq!(gold_game.messages.last().unwrap(), "7 zlåtnikov");
    }

    #[test]
    fn moving_without_pickup_reports_the_item_and_leaves_it_on_the_floor() {
        let mut g = Game::new(224);
        g.monsters.clear();
        let destination = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(destination).unwrap().terrain = Terrain::Floor;
        let mut food = Item::basic(g.id(), ItemKind::Food, 0);
        food.pos = Some(destination);
        let food_id = food.id;
        g.floor_items.push(food);

        assert_eq!(g.move_without_pickup(Direction::Right), CommandResult::TURN);

        assert_eq!(g.player.pos, destination);
        assert!(g.floor_items.iter().any(|item| item.id == food_id));
        assert_eq!(
            g.messages.last().map(String::as_str),
            Some("stųpaješ na porcijų jedy")
        );
    }

    #[test]
    fn wraith_level_drain_preserves_the_original_threshold_indexing() {
        let mut g = Game::new(228);
        g.player.stats.level = 2;
        g.player.stats.experience = 20;
        g.player.stats.hp = 100;
        g.player.stats.max_hp = 100;

        g.drain_level();

        assert_eq!(g.player.stats.level, 1);
        assert_eq!(
            g.player.stats.experience,
            crate::player::EXPERIENCE_LEVELS[0] + 1
        );
    }

    #[test]
    fn fatal_wraith_drain_records_the_killer() {
        let mut g = Game::new(230);
        g.player.stats.experience = 0;

        g.drain_level();

        assert_eq!(g.end, EndState::Dead);
        assert_eq!(g.death_cause.as_deref(), Some("prizraka"));
    }

    #[test]
    fn experience_jump_reports_only_the_final_level() {
        let mut g = Game::new(229);
        let messages = g.messages.len();
        g.player.stats.experience = 80;

        g.check_experience();

        assert_eq!(g.player.stats.level, 5);
        assert_eq!(&g.messages[messages..], ["dostigaješ stųpene 5"]);
    }

    #[test]
    fn every_experience_threshold_advances_at_the_reference_boundary() {
        for (index, threshold) in crate::player::EXPERIENCE_LEVELS.iter().enumerate() {
            let lower_level = index as i32 + 1;
            let mut below = Game::new(10_000 + index as u64);
            below.player.stats.level = lower_level;
            below.player.stats.experience = threshold - 1;
            below.check_experience();
            assert_eq!(below.player.stats.level, lower_level);

            let mut at = Game::new(20_000 + index as u64);
            at.player.stats.level = lower_level + 1;
            at.player.stats.experience = *threshold;
            at.check_experience();
            assert_eq!(at.player.stats.level, lower_level + 1);
        }

        let mut beyond = Game::new(30_000);
        beyond.player.stats.level = 21;
        beyond.player.stats.experience = crate::player::EXPERIENCE_LEVELS[19];
        beyond.check_experience();
        assert_eq!(beyond.player.stats.level, 21);
    }

    #[test]
    fn losing_experience_lowers_level_without_removing_hit_points() {
        let mut g = Game::new(30_001);
        g.player.stats.level = 8;
        g.player.stats.experience = 39;
        g.player.stats.hp = 47;
        g.player.stats.max_hp = 53;

        g.check_experience();

        assert_eq!(g.player.stats.level, 3);
        assert_eq!(g.player.stats.hp, 47);
        assert_eq!(g.player.stats.max_hp, 53);
    }

    #[test]
    fn successful_zero_damage_melee_hit_can_confuse() {
        let mut g = Game::new(238);
        g.monsters.clear();
        g.player.stats.level = 100;
        g.player.stats.strength = 15;
        g.player.conditions.can_confuse_monster = true;
        let food = g.id();
        g.player
            .inventory
            .push(Item::basic(food, ItemKind::Food, 0));
        g.player.weapon = Some(food);
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut target = monster::create(g.id(), 25, pos, g.depth, &mut g.rng);
        target.hp = 100;
        target.max_hp = 100;
        g.monsters.push(target);

        g.player_attack(0);

        assert_eq!(g.monsters[0].hp, 100);
        assert_ne!(g.monsters[0].flags & monster::CONFUSED, 0);
        assert!(!g.player.conditions.can_confuse_monster);
    }

    #[test]
    fn noncursed_wielded_item_is_unwielded_and_thrown() {
        let mut g = Game::new(239);
        let weapon = g.player.weapon.unwrap();
        let count = g.player.inventory.len();

        let result = g.throw_item(weapon, Direction::Right);

        assert!(result.consumed_turn);
        assert_eq!(g.player.inventory.len(), count - 1);
        assert_eq!(g.player.weapon, None);
    }

    #[test]
    fn missed_thrown_weapon_falls_under_the_target_when_that_is_the_only_square() {
        let mut g = Game::new(240);
        g.monsters.clear();
        g.floor_items.clear();
        let target_pos = g.player.pos.offset(1, 0);
        for dy in -1..=1 {
            for dx in 0..=2 {
                let pos = g.player.pos.offset(dx, dy);
                if let Some(cell) = g.dungeon.map.get_mut(pos) {
                    cell.terrain = if pos == g.player.pos || pos == target_pos {
                        Terrain::Floor
                    } else {
                        Terrain::Void
                    };
                }
            }
        }
        let mut target = monster::create(g.id(), 25, target_pos, g.depth, &mut g.rng);
        target.armor = -100;
        target.hp = 100;
        g.monsters.push(target);
        let dagger = g.id();
        g.player
            .inventory
            .push(Item::basic(dagger, ItemKind::Weapon, 4));

        g.throw_item(dagger, Direction::Right);

        assert_eq!(g.monsters[0].hp, 100);
        assert!(g.floor_items.iter().any(|item| {
            item.kind == ItemKind::Weapon && item.which == 4 && item.pos == Some(target_pos)
        }));
    }

    #[test]
    fn launched_arrow_uses_hurled_damage_and_bow_bonuses() {
        let mut base = Game::new(241);
        base.monsters.clear();
        base.player.stats.level = 100;
        base.player.stats.strength = 15;
        base.player.weapon = None;
        let pos = base.player.pos.offset(1, 0);
        let mut target = monster::create(base.id(), 25, pos, base.depth, &mut base.rng);
        target.hp = 100;
        target.max_hp = 100;
        base.monsters.push(target);
        let arrow = Item::basic(base.id(), ItemKind::Weapon, 3);
        let mut launched = base.clone();
        let bow_id = launched.id();
        let mut bow = Item::basic(bow_id, ItemKind::Weapon, 2);
        bow.damage_plus = 2;
        launched.player.inventory.push(bow);
        launched.player.weapon = Some(bow_id);

        assert!(base.thrown_attack(0, &arrow));
        assert!(launched.thrown_attack(0, &arrow));

        assert_eq!(base.monsters[0].hp, 99);
        assert!(launched.monsters[0].hp <= 96);
    }

    #[test]
    fn thrown_attack_reveals_xeroc_and_continues() {
        let mut g = Game::new(242);
        g.monsters.clear();
        g.player.stats.level = 100;
        let pos = g.player.pos.offset(1, 0);
        let mut xeroc = monster::create(g.id(), 23, pos, g.depth, &mut g.rng);
        xeroc.hp = 100;
        xeroc.max_hp = 100;
        assert_ne!(xeroc.disguise, 'X');
        g.monsters.push(xeroc);
        let dagger = Item::basic(g.id(), ItemKind::Weapon, 4);

        assert!(g.thrown_attack(0, &dagger));

        assert_eq!(g.monsters[0].disguise, 'X');
        assert!(g.monsters[0].hp < 100);
    }

    #[test]
    fn thrown_weapon_messages_name_the_weapon_and_visibility_safe_target() {
        let mut hit = Game::new(2420);
        hit.monsters.clear();
        hit.player.stats.level = 100;
        let pos = hit.player.pos.offset(1, 0);
        let mut zombie = monster::create(hit.id(), 25, pos, hit.depth, &mut hit.rng);
        zombie.hp = 100;
        zombie.max_hp = 100;
        hit.monsters.push(zombie);
        let dagger = Item::basic(hit.id(), ItemKind::Weapon, 4);

        assert!(hit.thrown_attack(0, &dagger));
        assert_eq!(hit.messages.last().unwrap(), "kinžal udarjaje zombi");

        let mut miss = hit.clone();
        miss.monsters[0].armor = -100;
        miss.player.stats.level = 1;
        miss.messages.clear();

        assert!(!miss.thrown_attack(0, &dagger));
        assert_eq!(miss.messages.last().unwrap(), "kinžal leti mimo zombi");
    }

    #[test]
    fn successful_thrown_hit_consumes_the_projectile() {
        let mut g = Game::new(243);
        g.monsters.clear();
        g.floor_items.clear();
        g.player.stats.level = 100;
        let pos = g.player.pos.offset(1, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut target = monster::create(g.id(), 25, pos, g.depth, &mut g.rng);
        target.hp = 100;
        target.max_hp = 100;
        g.monsters.push(target);
        let dagger = g.id();
        g.player
            .inventory
            .push(Item::basic(dagger, ItemKind::Weapon, 4));

        g.throw_item(dagger, Direction::Right);

        assert!(g.monsters[0].hp < 100);
        assert!(g.floor_items.is_empty());
        assert!(!g.player.inventory.iter().any(|item| item.id == dagger));
    }

    #[test]
    fn carried_item_can_fall_under_another_monster() {
        let mut g = Game::new(244);
        g.monsters.clear();
        g.floor_items.clear();
        let center = g.player.pos.offset(3, 0);
        let landing = center.offset(1, 0);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let pos = center.offset(dx, dy);
                g.dungeon.map.get_mut(pos).unwrap().terrain = if pos == landing {
                    Terrain::Floor
                } else {
                    Terrain::Void
                };
            }
        }
        let blocker_id = g.id();
        g.monsters.push(monster::create(
            blocker_id, 25, landing, g.depth, &mut g.rng,
        ));
        let mut dead = monster::create(g.id(), 3, center, g.depth, &mut g.rng);
        let carried_id = g.id();
        dead.inventory
            .push(Item::basic(carried_id, ItemKind::Potion, 0));

        g.drop_monster_inventory(dead);

        assert!(
            g.floor_items
                .iter()
                .any(|item| item.id == carried_id && item.pos == Some(landing))
        );
    }

    #[test]
    fn mean_monster_does_not_wake_from_two_squares_away() {
        let mut g = Game::new(231);
        g.monsters.clear();
        let pos = g.player.pos.offset(2, 0);
        g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        let mut monster = monster::create(g.id(), 0, pos, g.depth, &mut g.rng);
        monster.awake = false;
        g.monsters.push(monster);

        g.move_monsters();

        assert!(!g.monsters[0].awake);
    }

    #[test]
    fn approaching_a_sleeping_mean_monster_delays_waking_until_the_next_look() {
        let seed = (1..1000)
            .find(|seed| GameRng::new(*seed).rnd(3) != 0)
            .unwrap();
        let mut g = Game::new(2310);
        g.monsters.clear();
        g.wandering_countdown = 10_000;
        let start = g.player.pos;
        let room = g.dungeon.map.get(start).unwrap().room;
        // Adjacency-delayed waking only applies where the monster is not yet
        // visible: in a LIT room, look(true) parity wakes room-wide every
        // turn. Darken the room so the lamp-radius path is what's under test.
        if let Some(room) = room {
            g.dungeon.rooms[room as usize].dark = true;
        }
        for dx in 1..=2 {
            let cell = g.dungeon.map.get_mut(start.offset(dx, 0)).unwrap();
            cell.terrain = Terrain::Floor;
            cell.room = room;
            cell.passage = None;
        }
        let pos = start.offset(2, 0);
        let mut monster = monster::create(g.id(), 0, pos, g.depth, &mut g.rng);
        monster.awake = false;
        monster.flags |= monster::MEAN;
        g.monsters.push(monster);
        g.rng = GameRng::new(seed);

        g.execute(Command::Move(Direction::Right));

        assert!(!g.monsters[0].awake);
        g.execute(Command::Rest);
        assert!(g.monsters[0].awake);
    }

    #[test]
    fn entering_a_door_wakes_mean_monsters_across_the_room() {
        let seed = (1..1000)
            .find(|seed| GameRng::new(*seed).rnd(3) != 0)
            .unwrap();
        let mut g = Game::new(2311);
        g.monsters.clear();
        g.wandering_countdown = 10_000;
        let start = g.player.pos;
        let room = 0;
        let start_cell = g.dungeon.map.get_mut(start).unwrap();
        start_cell.terrain = Terrain::Passage;
        start_cell.room = None;
        start_cell.passage = Some(0);
        let door = start.offset(1, 0);
        let door_cell = g.dungeon.map.get_mut(door).unwrap();
        door_cell.terrain = Terrain::Door;
        door_cell.room = Some(room);
        door_cell.passage = Some(0);
        let pos = start.offset(5, 0);
        let monster_cell = g.dungeon.map.get_mut(pos).unwrap();
        monster_cell.terrain = Terrain::Floor;
        monster_cell.room = Some(room);
        monster_cell.passage = None;
        let mut monster = monster::create(g.id(), 0, pos, g.depth, &mut g.rng);
        monster.awake = false;
        monster.flags |= monster::MEAN;
        g.monsters.push(monster);
        g.rng = GameRng::new(seed);

        g.execute(Command::Move(Direction::Right));

        assert!(g.monsters[0].awake);
    }

    #[test]
    fn monster_cannot_attack_diagonally_through_a_blocked_corner() {
        let mut g = Game::new(232);
        g.monsters.clear();
        g.player.stats.hp = 100;
        let from = g.player.pos.offset(1, 1);
        g.dungeon.map.get_mut(from).unwrap().terrain = Terrain::Floor;
        g.dungeon
            .map
            .get_mut(g.player.pos.offset(1, 0))
            .unwrap()
            .terrain = Terrain::Void;
        g.dungeon
            .map
            .get_mut(g.player.pos.offset(0, 1))
            .unwrap()
            .terrain = Terrain::Void;
        let mut monster = monster::create(g.id(), 25, from, g.depth, &mut g.rng);
        monster.level = 100;
        monster.awake = true;
        g.monsters.push(monster);

        g.move_monster_step(0);

        assert_eq!(g.player.stats.hp, 100);
    }

    #[test]
    fn monster_diagonal_requires_both_orthogonal_sides_to_be_open() {
        let mut g = Game::new(2320);
        g.monsters.clear();
        g.player.stats.hp = 100;
        let from = g.player.pos.offset(1, 1);
        let open_side = g.player.pos.offset(0, 1);
        let closed_side = g.player.pos.offset(1, 0);
        for pos in [from, open_side] {
            g.dungeon.map.get_mut(pos).unwrap().terrain = Terrain::Floor;
        }
        g.dungeon.map.get_mut(closed_side).unwrap().terrain = Terrain::Void;
        let mut monster = monster::create(g.id(), 25, from, g.depth, &mut g.rng);
        monster.level = 100;
        monster.awake = true;
        g.monsters.push(monster);

        g.move_monster_step(0);

        assert_eq!(g.player.stats.hp, 100);
    }

    #[test]
    fn dragon_does_not_breathe_between_different_areas() {
        let seed = (1..1000)
            .find(|seed| GameRng::new(*seed).rnd(5) == 0)
            .unwrap();
        let mut g = Game::new(233);
        g.monsters.clear();
        g.player.stats.hp = 100;
        let from = g.player.pos.offset(5, 0);
        for dx in 0..=5 {
            let cell = g.dungeon.map.get_mut(g.player.pos.offset(dx, 0)).unwrap();
            cell.terrain = Terrain::Passage;
            cell.room = None;
            cell.passage = Some(if dx == 0 { 1 } else { 2 });
        }
        let mut dragon = monster::create(g.id(), 3, from, g.depth, &mut g.rng);
        dragon.awake = true;
        g.monsters.push(dragon);
        g.rng = GameRng::new(seed);

        g.move_monster_step(0);

        assert_eq!(g.player.stats.hp, 100);
    }

    #[test]
    fn cancelling_a_repeatable_prompt_restores_the_previous_repeat_tuple() {
        let mut g = Game::new(235);
        g.last_command = Some('t');
        g.last_item = Some(41);
        g.last_direction = Some(Direction::UpLeft);
        g.last_hand = Some(1);

        g.remember_command('q');
        g.last_item = Some(99);
        g.last_direction = Some(Direction::Down);
        g.last_hand = Some(0);
        g.reset_last_command();

        assert_eq!(g.last_command, Some('t'));
        assert_eq!(g.last_item, Some(41));
        assert_eq!(g.last_direction, Some(Direction::UpLeft));
        assert_eq!(g.last_hand, Some(1));
    }

    #[test]
    fn illegal_commands_report_the_reference_unctrl_key() {
        let mut g = Game::new(236);
        g.message("remember this");

        g.execute(Command::Unknown('x'));
        assert_eq!(g.messages.last().unwrap(), "nepraviľna komanda 'x'");
        assert_eq!(g.recall_message, "remember this");

        g.execute(Command::Wizard(WizardCommand::Down));
        assert_eq!(g.messages.last().unwrap(), "nepraviľna komanda '^D'");
        assert_eq!(g.recall_message, "remember this");
    }

    #[test]
    fn unseen_invisible_carrier_may_divert_to_an_item() {
        let mut g = Game::new(234);
        g.monsters.clear();
        g.floor_items.clear();
        let monster_pos = g.player.pos.offset(1, 0);
        let item_pos = g.player.pos.offset(2, 0);
        for pos in [monster_pos, item_pos] {
            let cell = g.dungeon.map.get_mut(pos).unwrap();
            cell.terrain = Terrain::Passage;
            cell.passage = Some(7);
        }
        let mut dragon = monster::create(g.id(), 3, monster_pos, g.depth, &mut g.rng);
        dragon.flags |= monster::INVISIBLE;
        dragon.awake = true;
        g.monsters.push(dragon);
        let mut potion = Item::basic(g.id(), ItemKind::Potion, 0);
        potion.pos = Some(item_pos);
        g.floor_items.push(potion);

        assert_eq!(g.find_monster_item_destination(0), Some(item_pos));
    }
}
