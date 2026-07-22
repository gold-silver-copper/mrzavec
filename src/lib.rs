pub mod combat;
pub mod command;
pub mod effects;
pub mod game;
pub mod generation;
pub mod help;
pub mod item;
pub mod lang;
pub mod map;
pub mod monster;
pub mod platform;
pub mod player;
pub mod rng;
pub mod save;
pub mod score;

pub use game::Game;

pub const DISPLAY_WIDTH: usize = 80;
pub const EVENT_ROWS: usize = 3;
pub const DUNGEON_FIRST_ROW: usize = EVENT_ROWS;
pub const DUNGEON_ROWS: usize = 22;
pub const STATUS_ROW: usize = DUNGEON_FIRST_ROW + DUNGEON_ROWS;
pub const KEYBINDING_FIRST_ROW: usize = STATUS_ROW + 1;
pub const COMMAND_BAR_ROWS: usize = 15;
pub const DISPLAY_HEIGHT: usize = KEYBINDING_FIRST_ROW + COMMAND_BAR_ROWS;
pub const AMULET_LEVEL: u32 = 26;
pub const MAX_PACK: usize = 26;
