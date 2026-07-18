use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}
impl Direction {
    pub const fn from_delta(dx: i32, dy: i32) -> Option<Self> {
        match (dx, dy) {
            (-1, 0) => Some(Self::Left),
            (0, 1) => Some(Self::Down),
            (0, -1) => Some(Self::Up),
            (1, 0) => Some(Self::Right),
            (-1, -1) => Some(Self::UpLeft),
            (1, -1) => Some(Self::UpRight),
            (-1, 1) => Some(Self::DownLeft),
            (1, 1) => Some(Self::DownRight),
            _ => None,
        }
    }

    pub const fn delta(self) -> (i32, i32) {
        match self {
            Self::Left => (-1, 0),
            Self::Down => (0, 1),
            Self::Up => (0, -1),
            Self::Right => (1, 0),
            Self::UpLeft => (-1, -1),
            Self::UpRight => (1, -1),
            Self::DownLeft => (-1, 1),
            Self::DownRight => (1, 1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Move(Direction),
    Run(Direction),
    RunUntilInteresting(Direction),
    Rest,
    Search,
    Pickup,
    Quaff,
    Read,
    Eat,
    Wield,
    Wear,
    TakeOff,
    PutOnRing,
    RemoveRing,
    Drop,
    Zap,
    Throw,
    Fight { kamikaze: bool },
    MoveWithoutPickup,
    IdentifyTrap,
    Repeat,
    Call,
    CurrentWeapon,
    CurrentArmor,
    CurrentRings,
    CurrentStats,
    ToggleWizard,
    Wizard(WizardCommand),
    Down,
    Up,
    Inventory,
    PickyInventory,
    IdentifyObject,
    Help,
    Discoveries,
    Options,
    Recall,
    Redraw,
    Version,
    LegalSpace,
    Shell,
    Suspend,
    Quit,
    Save,
    Cancel,
    Unknown(char),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardCommand {
    Coordinates,
    Create,
    PackCount,
    GroundInventory,
    Identify,
    Down,
    Up,
    Map,
    Teleport,
    Food,
    AddPassages,
    Detect,
    Charge,
    Power,
    List,
}

pub fn parse(ch: char) -> Command {
    use Direction::*;
    match ch {
        'h' => Command::Move(Left),
        'j' => Command::Move(Down),
        'k' => Command::Move(Up),
        'l' => Command::Move(Right),
        'y' => Command::Move(UpLeft),
        'u' => Command::Move(UpRight),
        'b' => Command::Move(DownLeft),
        'n' => Command::Move(DownRight),
        'H' => Command::Run(Left),
        'J' => Command::Run(Down),
        'K' => Command::Run(Up),
        'L' => Command::Run(Right),
        'Y' => Command::Run(UpLeft),
        'U' => Command::Run(UpRight),
        'B' => Command::Run(DownLeft),
        'N' => Command::Run(DownRight),
        '\u{8}' => Command::RunUntilInteresting(Left),
        '\u{a}' => Command::RunUntilInteresting(Down),
        '\u{b}' => Command::RunUntilInteresting(Up),
        '\u{c}' => Command::RunUntilInteresting(Right),
        '\u{19}' => Command::RunUntilInteresting(UpLeft),
        '\u{15}' => Command::RunUntilInteresting(UpRight),
        '\u{2}' => Command::RunUntilInteresting(DownLeft),
        '\u{e}' => Command::RunUntilInteresting(DownRight),
        '.' => Command::Rest,
        's' => Command::Search,
        ',' => Command::Pickup,
        'q' => Command::Quaff,
        'r' => Command::Read,
        'e' => Command::Eat,
        'w' => Command::Wield,
        'W' => Command::Wear,
        'T' => Command::TakeOff,
        'P' => Command::PutOnRing,
        'R' => Command::RemoveRing,
        'd' => Command::Drop,
        'z' => Command::Zap,
        't' => Command::Throw,
        'f' => Command::Fight { kamikaze: false },
        'F' => Command::Fight { kamikaze: true },
        'm' => Command::MoveWithoutPickup,
        '^' => Command::IdentifyTrap,
        'a' => Command::Repeat,
        'c' => Command::Call,
        ')' => Command::CurrentWeapon,
        ']' => Command::CurrentArmor,
        '=' => Command::CurrentRings,
        '@' => Command::CurrentStats,
        '+' => Command::ToggleWizard,
        'C' => Command::Wizard(WizardCommand::Create),
        '|' => Command::Wizard(WizardCommand::Coordinates),
        '$' => Command::Wizard(WizardCommand::PackCount),
        '~' => Command::Wizard(WizardCommand::Charge),
        '*' => Command::Wizard(WizardCommand::List),
        '\u{7}' => Command::Wizard(WizardCommand::GroundInventory),
        '\u{17}' => Command::Wizard(WizardCommand::Identify),
        '\u{4}' => Command::Wizard(WizardCommand::Down),
        '\u{1}' => Command::Wizard(WizardCommand::Up),
        '\u{6}' => Command::Wizard(WizardCommand::Map),
        '\u{14}' => Command::Wizard(WizardCommand::Teleport),
        '\u{5}' => Command::Wizard(WizardCommand::Food),
        '\u{11}' => Command::Wizard(WizardCommand::AddPassages),
        '\u{18}' => Command::Wizard(WizardCommand::Detect),
        '\u{9}' => Command::Wizard(WizardCommand::Power),
        '>' => Command::Down,
        '<' => Command::Up,
        'i' => Command::Inventory,
        'I' => Command::PickyInventory,
        '/' => Command::IdentifyObject,
        '?' => Command::Help,
        'D' => Command::Discoveries,
        'o' => Command::Options,
        '\u{10}' => Command::Recall,
        '\u{12}' => Command::Redraw,
        'v' => Command::Version,
        ' ' => Command::LegalSpace,
        '!' => Command::Shell,
        '\u{1a}' => Command::Suspend,
        'Q' => Command::Quit,
        'S' => Command::Save,
        '\u{1b}' => Command::Cancel,
        c => Command::Unknown(c),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandResult {
    pub consumed_turn: bool,
    pub redraw: bool,
}
impl CommandResult {
    pub const FREE: Self = Self {
        consumed_turn: false,
        redraw: true,
    };
    pub const TURN: Self = Self {
        consumed_turn: true,
        redraw: true,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eight_directions_match_rogue_keys() {
        assert_eq!(parse('y'), Command::Move(Direction::UpLeft));
        assert_eq!(parse('n'), Command::Move(Direction::DownRight));
    }
    #[test]
    fn informational_commands_are_recognized() {
        assert_eq!(parse('?'), Command::Help);
        assert_eq!(parse('i'), Command::Inventory);
        assert_eq!(parse('\u{12}'), Command::Redraw);
        assert_eq!(parse(' '), Command::LegalSpace);
        assert_eq!(parse('!'), Command::Shell);
        assert_eq!(parse('\u{1a}'), Command::Suspend);
    }
    #[test]
    fn wizard_control_keys_match_master_build() {
        assert_eq!(parse('\u{4}'), Command::Wizard(WizardCommand::Down));
        assert_eq!(parse('\u{14}'), Command::Wizard(WizardCommand::Teleport));
        assert_eq!(parse('~'), Command::Wizard(WizardCommand::Charge));
        assert_eq!(parse('*'), Command::Wizard(WizardCommand::List));
    }
    #[test]
    fn control_directions_request_cautious_running() {
        assert_eq!(
            parse('\u{8}'),
            Command::RunUntilInteresting(Direction::Left)
        );
        assert_eq!(
            parse('\u{e}'),
            Command::RunUntilInteresting(Direction::DownRight)
        );
    }

    #[test]
    fn uppercase_f_selects_original_kamikaze_fight_mode() {
        assert_eq!(parse('f'), Command::Fight { kamikaze: false });
        assert_eq!(parse('F'), Command::Fight { kamikaze: true });
    }
}
