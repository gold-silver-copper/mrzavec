use serde::{Deserialize, Serialize};

pub const MAP_WIDTH: i32 = 80;
pub const MAP_HEIGHT: i32 = 23;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
    pub fn distance2(self, other: Self) -> i32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terrain {
    #[default]
    Void,
    WallHorizontal,
    WallVertical,
    Floor,
    Passage,
    Door,
    SecretDoor,
    SecretPassage,
    Stairs,
    SecretDoorHorizontal,
    SecretDoorVertical,
}

impl Terrain {
    pub fn glyph(self) -> char {
        match self {
            Self::Void => ' ',
            Self::WallHorizontal => '-',
            Self::WallVertical => '|',
            Self::Floor => '.',
            Self::Passage => '#',
            Self::Door => '+',
            Self::SecretDoor | Self::SecretDoorVertical => wall_glyph(),
            Self::SecretDoorHorizontal => '-',
            Self::SecretPassage => ' ',
            Self::Stairs => '%',
        }
    }
    pub fn passable(self) -> bool {
        !matches!(
            self,
            Self::Void
                | Self::WallHorizontal
                | Self::WallVertical
                | Self::SecretDoor
                | Self::SecretPassage
                | Self::SecretDoorHorizontal
                | Self::SecretDoorVertical
        )
    }
}

const fn wall_glyph() -> char {
    '|'
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub terrain: Terrain,
    pub room: Option<u8>,
    pub passage: Option<u8>,
    pub seen: bool,
    pub remembered: char,
    #[serde(default)]
    pub wizard_revealed: bool,
    pub trap: Option<Trap>,
    pub trap_revealed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trap {
    TrapDoor,
    Arrow,
    SleepGas,
    Bear,
    Teleport,
    PoisonDart,
    Rust,
    Mysterious,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Map {
    cells: Vec<Cell>,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            cells: vec![Cell::default(); (MAP_WIDTH * MAP_HEIGHT) as usize],
        }
    }
}

impl Map {
    pub fn in_bounds(pos: Pos) -> bool {
        pos.x >= 0 && pos.x < MAP_WIDTH && pos.y >= 1 && pos.y < MAP_HEIGHT
    }
    fn index(pos: Pos) -> usize {
        (pos.y * MAP_WIDTH + pos.x) as usize
    }
    pub fn get(&self, pos: Pos) -> Option<&Cell> {
        Self::in_bounds(pos).then(|| &self.cells[Self::index(pos)])
    }
    pub fn get_mut(&mut self, pos: Pos) -> Option<&mut Cell> {
        Self::in_bounds(pos).then(|| {
            let i = Self::index(pos);
            &mut self.cells[i]
        })
    }
    pub fn iter(&self) -> impl Iterator<Item = (Pos, &Cell)> {
        self.cells.iter().enumerate().filter_map(|(i, c)| {
            let p = Pos::new(i as i32 % MAP_WIDTH, i as i32 / MAP_WIDTH);
            Self::in_bounds(p).then_some((p, c))
        })
    }
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }
}
