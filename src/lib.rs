pub mod combat;
pub mod command;
pub mod effects;
pub mod game;
pub mod generation;
pub mod item;
pub mod map;
pub mod monster;
pub mod platform;
pub mod player;
pub mod rng;
pub mod save;
pub mod score;

pub use game::Game;

pub const DISPLAY_WIDTH: usize = 80;
pub const DISPLAY_HEIGHT: usize = 24;
pub const STATUS_ROW: usize = 23;
pub const AMULET_LEVEL: u32 = 26;
pub const MAX_PACK: usize = 26;
