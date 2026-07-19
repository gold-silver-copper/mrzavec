use bevy::{prelude::*, text::LineHeight, window::WindowResolution};
use mrzavec::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, Game, KEYBINDING_FIRST_ROW, KEYBINDING_SECOND_ROW, STATUS_ROW,
    command::{Command, WizardCommand, parse},
    item::{
        ARMOR_NAMES, ARMOR_WEIGHTS, ItemKind, POTION_NAMES, POTION_WEIGHTS, RING_NAMES,
        RING_WEIGHTS, SCROLL_NAMES, SCROLL_WEIGHTS, STICK_NAMES, STICK_WEIGHTS, WEAPON_NAMES,
        WEAPON_WEIGHTS,
    },
    map::Pos,
    save, score,
};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::VecDeque, time::Duration};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const CELL_W: f32 = 10.0;
const CELL_H: f32 = 19.0;
const FONT_SIZE: f32 = 16.0;
const MODAL_MORE_ROW: usize = DISPLAY_HEIGHT - 1;
const MODAL_PAGE_ROWS: usize = MODAL_MORE_ROW;
const KEYBINDING_FIRST_TEXT: &str =
    "Move h/j/k/l  Inventory i  Quaff q  Read r  Eat e  Wield w  Drop d";
const KEYBINDING_SECOND_TEXT: &str = "Wear W  Take off T  Throw t  Zap z  Search s  Rest .  Help ?";
const MOVEMENT_REPEAT_DELAY: Duration = Duration::from_millis(300);
const MOVEMENT_REPEAT_INTERVAL: Duration = Duration::from_millis(100);
const MOVEMENT_KEYS: [(KeyCode, char); 8] = [
    (KeyCode::KeyH, 'h'),
    (KeyCode::KeyJ, 'j'),
    (KeyCode::KeyK, 'k'),
    (KeyCode::KeyL, 'l'),
    (KeyCode::KeyY, 'y'),
    (KeyCode::KeyU, 'u'),
    (KeyCode::KeyB, 'b'),
    (KeyCode::KeyN, 'n'),
];
const ROGUE_RELEASE: &str = "2026-07-17";

fn version_message(game: &Game) -> String {
    format!(
        "rogue version 5.4.5 release {ROGUE_RELEASE} dungeon {} (chongo was here)",
        game.dungeon_number
    )
}

fn recall_last_message(game: &mut Game) {
    let message = game.recall_message.clone();
    game.message(message);
}

fn remembered_prompt(state: &mut State, text: impl Into<String>) -> String {
    let text = text.into();
    state.game.remember_message(&text);
    message_display_text(&text)
}

/// Apply `io.c::endmsg`'s presentation rule without changing the raw text
/// retained by the simulation for Ctrl-R recall.
fn message_display_text(text: &str) -> String {
    let mut displayed = text.to_owned();
    let mut chars = displayed.char_indices();
    let Some((_, first)) = chars.next() else {
        return displayed;
    };
    let second = chars.next().map(|(_, ch)| ch);
    if first.is_ascii_lowercase() && second != Some(')') {
        displayed.replace_range(0..first.len_utf8(), &first.to_ascii_uppercase().to_string());
    }
    displayed
}

fn wizard_password_matches(input: &str) -> bool {
    input == "bathtub"
}

fn password_prompt(pending: Pending) -> &'static str {
    if pending == Pending::StartupPassword {
        "wizard's password: "
    } else {
        "wizard's Password: "
    }
}

fn call_prompt(game: &Game) -> &'static str {
    if game.options.terse {
        "call it: "
    } else {
        "what do you want to call it? "
    }
}

#[derive(Resource)]
struct State {
    game: Game,
    modal: Option<String>,
    modal_overlay: bool,
    modal_offset: usize,
    item_inventory_open: bool,
    preserve_message_case: bool,
    slow_discovery_lines: Vec<String>,
    message_serial_seen: u64,
    message_queue: VecDeque<String>,
    visible_message: Option<String>,
    message_wait: bool,
    deferred_modal: Option<(String, bool, usize)>,
    pending: Option<Pending>,
    score_recorded: bool,
    input_buffer: String,
    count_prefix: String,
    counted_command: Option<(Command, u16)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Quaff,
    Read,
    Eat,
    Wield,
    Wear,
    PutRing,
    PutRingHand(u64),
    RemoveRingHand,
    Drop,
    ThrowSelect,
    ZapSelect,
    ThrowDirection,
    ZapDirection,
    FightDirection(bool),
    MoveDirection,
    TrapDirection,
    Identify,
    CallSelect,
    CallText(u64),
    AutoCall,
    Options(usize),
    SaveConfirm,
    SaveFileText,
    SaveOverwrite,
    MagicDetection,
    FoodDetection,
    Help,
    IdentifyGlyph,
    Discoveries,
    DiscoveryMore,
    SlowDiscoveryPrompt,
    SlowDiscovery(usize),
    PickyInventory,
    SlowInventory(usize),
    More,
    Password,
    StartupPassword,
    QuitConfirm,
    WizardCharge,
    WizardListType,
    WizardCreateType,
    WizardCreateWhich(ItemKind),
    WizardCreateBlessing(ItemKind, u8),
    WizardCreateGold,
}
#[derive(Component)]
struct Cell(usize);
#[derive(Component)]
struct Glyph;

#[derive(Resource, Default)]
struct MovementRepeat {
    key: Option<KeyCode>,
    remaining: Duration,
}

impl MovementRepeat {
    fn reset(&mut self) {
        self.key = None;
        self.remaining = Duration::ZERO;
    }

    fn update(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        delta: Duration,
        enabled: bool,
    ) -> Option<char> {
        if !enabled {
            self.reset();
            return None;
        }

        if let Some((key, _)) = MOVEMENT_KEYS
            .iter()
            .copied()
            .find(|(key, _)| keys.just_pressed(*key))
        {
            self.key = Some(key);
            self.remaining = MOVEMENT_REPEAT_DELAY;
            return None;
        }

        let held = self.key.filter(|key| keys.pressed(*key)).and_then(|key| {
            MOVEMENT_KEYS
                .iter()
                .copied()
                .find(|(candidate, _)| *candidate == key)
        });
        let Some((key, ch)) = held else {
            let Some((key, _)) = MOVEMENT_KEYS
                .iter()
                .copied()
                .find(|(key, _)| keys.pressed(*key))
            else {
                self.reset();
                return None;
            };
            self.key = Some(key);
            self.remaining = MOVEMENT_REPEAT_DELAY;
            return None;
        };

        self.key = Some(key);
        if delta < self.remaining {
            self.remaining -= delta;
            return None;
        }
        let overrun = delta - self.remaining;
        let phase_nanos = overrun.as_nanos() % MOVEMENT_REPEAT_INTERVAL.as_nanos();
        self.remaining = if phase_nanos == 0 {
            MOVEMENT_REPEAT_INTERVAL
        } else {
            MOVEMENT_REPEAT_INTERVAL - Duration::from_nanos(phase_nanos as u64)
        };
        Some(ch)
    }
}

#[derive(Resource)]
#[cfg(not(target_arch = "wasm32"))]
struct TerminationSignal {
    pending: Arc<AtomicBool>,
    signal_quit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(not(target_arch = "wasm32"))]
enum Startup {
    Play {
        restore: Option<OsString>,
        signal_quit: bool,
        wizard_prompt: bool,
    },
    Scores(Option<OsString>),
    Version,
    Help,
    Die,
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_startup(args: impl IntoIterator<Item = OsString>) -> Result<Startup, char> {
    let mut args = args.into_iter().peekable();
    let mut restore = None;
    let mut signal_quit = false;
    let mut wizard_prompt = false;
    let mut options = true;
    let mut first = true;
    while let Some(argument) = args.next() {
        if first && argument.is_empty() {
            wizard_prompt = true;
            first = false;
            continue;
        }
        first = false;
        let text = argument.to_string_lossy();
        if options && text == "--" {
            options = false;
            continue;
        }
        if options && text.starts_with('-') && text.len() > 1 {
            for (offset, flag) in text[1..].char_indices() {
                match flag {
                    'S' => signal_quit = true,
                    'r' => {}
                    'V' => return Ok(Startup::Version),
                    'h' => return Ok(Startup::Help),
                    'd' => return Ok(Startup::Die),
                    's' => {
                        let value_offset = 1 + offset + flag.len_utf8();
                        let attached = &text[value_offset..];
                        let path = if !attached.is_empty() {
                            Some(OsString::from(attached))
                        } else if args.peek().is_some_and(|next| {
                            let next = next.to_string_lossy();
                            next != "--" && !next.starts_with('-')
                        }) {
                            args.next()
                        } else {
                            None
                        };
                        return Ok(Startup::Scores(path));
                    }
                    other => return Err(other),
                }
            }
            continue;
        }
        if restore.is_none() {
            restore = Some(argument);
        }
    }
    Ok(Startup::Play {
        restore,
        signal_quit,
        wizard_prompt,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_c_number(value: &str) -> Option<u32> {
    let (negative, value) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    let (radix, digits) = if let Some(rest) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        (16, rest)
    } else if value.len() > 1 && value.starts_with('0') {
        (8, &value[1..])
    } else {
        (10, value)
    };
    let parsed = u32::from_str_radix(digits, radix).ok()?;
    Some(if negative {
        parsed.wrapping_neg()
    } else {
        parsed
    })
}

fn parse_c_integer(value: &str) -> i32 {
    let value = value.trim_start();
    let (negative, digits) = if let Some(rest) = value.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = value.strip_prefix('+') {
        (false, rest)
    } else {
        (false, value)
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return 0;
    }
    let magnitude = digits.parse::<i64>().unwrap_or(i64::MAX);
    let value = if negative { -magnitude } else { magnitude };
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(not(target_arch = "wasm32"))]
fn startup_wizard_seed() -> Option<u64> {
    std::env::var("SEED")
        .ok()
        .and_then(|value| parse_c_number(&value))
        .map(u64::from)
}

#[cfg(target_arch = "wasm32")]
fn startup_wizard_seed() -> Option<u64> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn selected_seed(fallback: u64, options: &mrzavec::game::Options) -> u64 {
    if options.name.starts_with("rogo-")
        && let Ok(value) = std::env::var("ROGOSEED")
        && let Some(seed) = parse_c_number(&value)
    {
        return u64::from(seed);
    }
    fallback
}

#[cfg(not(target_arch = "wasm32"))]
fn usage(program: &str, options: &mrzavec::game::Options) -> String {
    format!(
        "Usage: {program} [-SrdVh] [-s [score_file]] [save_file]\n\n\
         \t-S\t\tquit instead of saving on a terminating signal\n\
         \t-r\t\tignored for backward compatibility\n\
         \t-s [file]\tprint the score list\n\
         \t-d\t\tkill the rogue and score the result\n\
         \t-h\t\tprint this help\n\
         \t-V\t\tprint version information\n\
         \t[save_file]\trestore a game (default: {})\n\n\
         Default score file: {}\n\
         rogue version: 5.4.5 {ROGUE_RELEASE} (chongo was here)",
        options.save_file, options.score_file
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let options = mrzavec::game::Options::default();
    let program = std::env::args().next().unwrap_or_else(|| "mrzavec".into());
    let startup = match parse_startup(std::env::args_os().skip(1)) {
        Ok(startup) => startup,
        Err(flag) => {
            eprintln!("{program}: ERROR: illegal option -- {flag}");
            eprintln!("{}", usage(&program, &options));
            std::process::exit(3);
        }
    };
    let seed = mrzavec::platform::random_seed();
    let seed = selected_seed(seed, &options);
    let (restore, signal_quit, wizard_prompt) = match startup {
        Startup::Version => {
            println!("5.4.5 {ROGUE_RELEASE}");
            std::process::exit(2);
        }
        Startup::Help => {
            eprintln!("{}", usage(&program, &options));
            std::process::exit(2);
        }
        Startup::Scores(path) => {
            if path
                .as_ref()
                .is_some_and(|path| path.to_string_lossy().len() > 80)
            {
                eprintln!("ERROR: score path length exceeds 80 characters");
                std::process::exit(4);
            }
            let path = path.map_or_else(|| PathBuf::from(&options.score_file), PathBuf::from);
            let scores = match score::read(&path) {
                Ok(scores) => scores,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(error) => {
                    eprintln!("Unable to read score table {}: {error}", path.display());
                    std::process::exit(10);
                }
            };
            print!("{}", score::format(&scores));
            return;
        }
        Startup::Die => {
            let mut game = Game::new(seed);
            game.depth = 1;
            game.player.gold = (game.rng.rnd(60) + 2) as i32;
            game.death_cause = Some("a bat".into());
            game.end = mrzavec::game::EndState::Dead;
            let table = match score::record_locked(
                &game,
                std::path::Path::new(&game.options.score_file),
                std::path::Path::new(&game.options.lock_file),
            ) {
                Ok(scores) => score::format(&scores),
                Err(error) => format!("Unable to read or update score table: {error}"),
            };
            if game.options.tombstone {
                println!("{}\n\n{table}", tombstone_text(&game));
            } else {
                println!(
                    "Killed by a bat with {} gold\n\n{table}",
                    score::amount(&game)
                );
            }
            return;
        }
        Startup::Play {
            restore,
            signal_quit,
            wizard_prompt,
        } => (restore, signal_quit, wizard_prompt),
    };
    let mut game = match restore {
        Some(path) => match save::restore(std::path::Path::new(&path)) {
            Ok(game) => game,
            Err(error) => {
                eprintln!(
                    "Unable to restore {}: {error}",
                    std::path::Path::new(&path).display()
                );
                std::process::exit(5);
            }
        },
        None => Game::new(seed),
    };
    if wizard_prompt {
        game.remember_message(password_prompt(Pending::StartupPassword));
    }
    let termination_pending = Arc::new(AtomicBool::new(false));
    let signal_flag = Arc::clone(&termination_pending);
    if let Err(error) = ctrlc::set_handler(move || signal_flag.store(true, Ordering::SeqCst)) {
        eprintln!("Unable to install terminating-signal handler: {error}");
    }
    let mut app = game_app(game, wizard_prompt);
    app.insert_resource(TerminationSignal {
        pending: termination_pending,
        signal_quit,
    })
    .add_systems(
        Update,
        (
            handle_termination_signal,
            keyboard,
            finalize_end,
            prepare_messages,
            render,
        )
            .chain(),
    )
    .run();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let seed = mrzavec::platform::random_seed();
    let options = mrzavec::game::Options::default();
    let default_slot = options.save_file.clone();
    let mut game = match restore_browser_game(&default_slot) {
        Ok(Some(game)) => game,
        Ok(None) => Game::new(seed),
        Err(error) => {
            let mut game = Game::new(seed);
            game.message(format!("unable to restore browser save: {error}"));
            game
        }
    };
    if game.options.name.is_empty() {
        game.options.name = "player".into();
    }
    game_app(game, false)
        .add_systems(
            Update,
            (keyboard, finalize_end, prepare_messages, render).chain(),
        )
        .run();
}

#[cfg(target_arch = "wasm32")]
fn restore_browser_game(
    default_slot: &str,
) -> Result<Option<Game>, mrzavec::platform::StorageError> {
    let storage = mrzavec::platform::LocalStorage::open()?;
    save::restore_browser_game(default_slot, &storage)
}

fn game_app(game: Game, wizard_prompt: bool) -> App {
    let message_serial_seen = game.message_serial;
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(MovementRepeat::default())
        .insert_resource(State {
            game,
            modal: wizard_prompt.then(|| password_prompt(Pending::StartupPassword).into()),
            modal_overlay: false,
            modal_offset: 0,
            item_inventory_open: false,
            preserve_message_case: false,
            slow_discovery_lines: Vec::new(),
            message_serial_seen,
            message_queue: VecDeque::new(),
            visible_message: None,
            message_wait: false,
            deferred_modal: None,
            pending: wizard_prompt.then_some(Pending::StartupPassword),
            score_recorded: false,
            input_buffer: String::new(),
            count_prefix: String::new(),
            counted_command: None,
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(game_window()),
            ..default()
        }))
        .add_systems(Startup, setup);
    app
}

fn game_window() -> Window {
    let window = Window {
        title: "Rogue 5.4.5 — Mrzavec".into(),
        resolution: WindowResolution::new(
            (CELL_W * DISPLAY_WIDTH as f32 + 24.0) as u32,
            (CELL_H * DISPLAY_HEIGHT as f32 + 24.0) as u32,
        ),
        resizable: false,
        prevent_default_event_handling: true,
        ..default()
    };
    #[cfg(target_arch = "wasm32")]
    let window = Window {
        canvas: Some("#mrzavec".into()),
        fit_canvas_to_parent: false,
        ..window
    };
    window
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_termination_signal(game: &mut Game, signal_quit: bool) -> std::io::Result<()> {
    if signal_quit {
        game.death_cause = Some("signal".into());
        game.end = mrzavec::game::EndState::Dead;
        Ok(())
    } else {
        save::save(game, std::path::Path::new(&game.options.save_file))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_termination_signal(
    mut state: ResMut<State>,
    signal: Res<TerminationSignal>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !signal.pending.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Err(error) = apply_termination_signal(&mut state.game, signal.signal_quit) {
        eprintln!("Automatic save failed: {error}");
    }
    app_exit.write(AppExit::Success);
}

#[cfg(not(target_arch = "wasm32"))]
fn record_game_score(game: &Game) -> Result<Vec<score::ScoreEntry>, String> {
    score::record_locked(
        game,
        std::path::Path::new(&game.options.score_file),
        std::path::Path::new(&game.options.lock_file),
    )
    .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn record_game_score(game: &Game) -> Result<Vec<score::ScoreEntry>, String> {
    let storage = mrzavec::platform::LocalStorage::open().map_err(|error| error.to_string())?;
    score::record_in_storage(game, &game.options.score_file, &storage)
        .map_err(|error| error.to_string())
}

fn finalize_end(mut state: ResMut<State>) {
    if state.score_recorded || state.game.end == mrzavec::game::EndState::Playing {
        return;
    }
    let table = match record_game_score(&state.game) {
        Ok(scores) => score::format(&scores),
        Err(error) => format!("Unable to read or update score table: {error}"),
    };
    if state.game.end == mrzavec::game::EndState::Dead && state.game.options.tombstone {
        state.modal = Some(format!("{}\n\n{}", tombstone_text(&state.game), table));
        state.score_recorded = true;
        return;
    }
    state.modal = Some(match state.game.end {
        mrzavec::game::EndState::Won => format!(
            "Congratulations, you have made it to the light of day!\n\nYou escaped the Dungeons of Doom alive.\n\n{}\nFinal score: {}\n\n{}",
            winner_sales_text(&state.game),
            score::amount(&state.game),
            table
        ),
        mrzavec::game::EndState::Dead if state.game.options.tombstone => format!(
            "                       __________\n                      /    REST    \\\n                     /      IN      \\\n                    /     PEACE      \\\n\n                 Killed by {}\n                  Gold: {}\n                 Level: {}\n\n{}",
            state
                .game
                .death_cause
                .as_deref()
                .unwrap_or("unknown causes"),
            score::amount(&state.game),
            state.game.depth,
            table
        ),
        mrzavec::game::EndState::Dead => format!(
            "Killed by {} with {} gold\n\n{}",
            death_cause_with_article(&state.game),
            score::amount(&state.game),
            table
        ),
        mrzavec::game::EndState::Quit => format!(
            "You quit with {} gold pieces\n\n{}",
            state.game.player.gold, table
        ),
        mrzavec::game::EndState::Playing => unreachable!(),
    });
    state.score_recorded = true;
}

fn tombstone_text(game: &Game) -> String {
    fn center(value: &str) -> usize {
        28_usize.saturating_sub(value.chars().count().div_ceil(2))
    }
    fn overlay(line: &mut Vec<char>, column: usize, value: &str) {
        let value: Vec<char> = value.chars().collect();
        if line.len() < column + value.len() {
            line.resize(column + value.len(), ' ');
        }
        line[column..column + value.len()].copy_from_slice(&value);
    }
    let full_cause = death_cause_with_article(game);
    let (article, cause) = if let Some(cause) = full_cause.strip_prefix("an ") {
        ("an", cause)
    } else if let Some(cause) = full_cause.strip_prefix("a ") {
        ("a", cause)
    } else {
        ("", full_cause.as_str())
    };
    let mut lines: Vec<Vec<char>> = [
        "                       __________",
        "                      /          \\",
        "                     /    REST    \\",
        "                    /      IN      \\",
        "                   /     PEACE      \\",
        "                  /                  \\",
        "                  |                  |",
        "                  |                  |",
        "                  |   killed by a    |",
        "                  |                  |",
        "                  |       1980       |",
        "                 *|     *  *  *      | *",
        r"         ________)/\_//(\/(/\)/\//\/|_)_______",
    ]
    .into_iter()
    .map(|line| line.chars().collect())
    .collect();
    overlay(
        &mut lines[6],
        center(&game.options.name),
        &game.options.name,
    );
    let gold = format!("{} Au", score::amount(game));
    overlay(&mut lines[7], center(&gold), &gold);
    match article {
        "" => overlay(&mut lines[8], 32, " "),
        "an" => overlay(&mut lines[8], 33, "n"),
        _ => {}
    }
    overlay(&mut lines[9], center(cause), cause);
    overlay(&mut lines[10], 26, &format!("{:4}", current_year()));
    lines
        .into_iter()
        .map(|line| line.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn death_cause_with_article(game: &Game) -> String {
    let cause = game.death_cause.as_deref().unwrap_or("God");
    if cause.starts_with("a ")
        || cause.starts_with("an ")
        || matches!(cause, "starvation" | "hypothermia")
    {
        return cause.into();
    }
    let article = if cause.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {cause}")
}

fn current_year() -> i64 {
    let days = (mrzavec::platform::unix_time_seconds() / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    year + i64::from(month_prime >= 10)
}

fn winner_sales_text(game: &Game) -> String {
    let mut named = game.clone();
    named.knowledge.potions.fill(true);
    named.knowledge.scrolls.fill(true);
    named.knowledge.rings.fill(true);
    named.knowledge.sticks.fill(true);
    for item in &mut named.player.inventory {
        item.known = true;
    }
    let mut out = String::from("   Worth  Item\n");
    let mut previous_worth = 0;
    for item in game.player.inventory.iter().filter(|item| item.in_pack) {
        let named_item = named
            .player
            .inventory
            .iter()
            .find(|named_item| named_item.id == item.id)
            .unwrap();
        let letter = item.pack_letter.unwrap_or('?');
        previous_worth = score::item_worth_after(game, item, previous_worth);
        out.push_str(&format!(
            "{letter}) {:5}  {}\n",
            previous_worth,
            named.inventory_name(named_item, false),
        ));
    }
    out.push_str(&format!("   {:5}  Gold Pieces", game.player.gold));
    out
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|root| {
            root.spawn((
                Node {
                    display: Display::Grid,
                    width: px(CELL_W * DISPLAY_WIDTH as f32),
                    height: px(CELL_H * DISPLAY_HEIGHT as f32),
                    grid_template_columns: RepeatedGridTrack::px(DISPLAY_WIDTH as u16, CELL_W),
                    grid_template_rows: RepeatedGridTrack::px(DISPLAY_HEIGHT as u16, CELL_H),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
            ))
            .with_children(|grid| {
                for i in 0..DISPLAY_WIDTH * DISPLAY_HEIGHT {
                    grid.spawn((
                        Cell(i),
                        Node {
                            min_width: px(0),
                            min_height: px(0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ))
                    .with_child((
                        Glyph,
                        Text::new(" "),
                        TextFont {
                            font_size: FontSize::Px(FONT_SIZE),
                            ..default()
                        },
                        LineHeight::Px(CELL_H),
                        TextColor(Color::srgb(0.82, 0.82, 0.78)),
                        TextLayout::justify(Justify::Center),
                    ));
                }
            });
        });
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut movement_repeat: ResMut<MovementRepeat>,
    mut state: ResMut<State>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let controlled = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let alt_or_super = [
        KeyCode::AltLeft,
        KeyCode::AltRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]
    .into_iter()
    .any(|key| keys.pressed(key));
    let repeated_movement = movement_repeat.update(
        &keys,
        time.delta(),
        !shifted
            && !controlled
            && !alt_or_super
            && !state.message_wait
            && state.pending.is_none()
            && state.modal.is_none()
            && !state.item_inventory_open
            && state.game.end == mrzavec::game::EndState::Playing
            && state.game.player.conditions.asleep_turns == 0,
    );
    if keys.get_just_pressed().next().is_some() || repeated_movement.is_some() {
        state.preserve_message_case = false;
        if !state.message_wait {
            state.visible_message = None;
        }
    }
    if state.message_wait {
        if keys.just_pressed(KeyCode::Space) {
            advance_message(&mut state);
        }
        return;
    }
    if state.pending.is_none()
        && state.modal.is_none()
        && state.game.end == mrzavec::game::EndState::Playing
        && state.game.player.conditions.asleep_turns > 0
    {
        while state.game.end == mrzavec::game::EndState::Playing
            && state.game.player.conditions.asleep_turns > 0
        {
            state.game.execute(Command::Rest);
        }
        return;
    }
    if matches!(
        state.pending,
        Some(Pending::MagicDetection | Pending::FoodDetection)
    ) && keys.just_pressed(KeyCode::Space)
    {
        state.pending = None;
        state.modal = None;
        if show_pending_call(&mut state) {
            return;
        }
        continue_counted_command(&mut state);
        return;
    }
    if state.item_inventory_open && keys.just_pressed(KeyCode::Space) {
        if state
            .modal
            .as_deref()
            .is_some_and(|modal| modal_has_next_page(modal, state.modal_offset))
        {
            state.modal_offset += MODAL_PAGE_ROWS;
        } else if let Some(pending) = state.pending {
            restore_item_prompt(&mut state, pending);
        }
        return;
    }
    if state.pending == Some(Pending::DiscoveryMore) && keys.just_pressed(KeyCode::Space) {
        if state
            .modal
            .as_deref()
            .is_some_and(|modal| modal_has_next_page(modal, state.modal_offset))
        {
            state.modal_offset += MODAL_PAGE_ROWS;
        } else {
            state.pending = None;
            state.modal = None;
            state.modal_offset = 0;
            state.modal_overlay = false;
            state.game.message_without_recall("");
        }
        return;
    }
    if matches!(
        state.pending,
        Some(Pending::SlowDiscoveryPrompt | Pending::SlowDiscovery(_))
    ) && keys.just_pressed(KeyCode::Space)
    {
        let next = match state.pending {
            Some(Pending::SlowDiscoveryPrompt) => 0,
            Some(Pending::SlowDiscovery(index)) => index + 1,
            _ => unreachable!(),
        };
        if next + 1 < state.slow_discovery_lines.len() {
            let line = state.slow_discovery_lines[next].clone();
            state.game.remember_message(&line);
            state.pending = Some(Pending::SlowDiscovery(next));
            state.modal = Some(format!("{}  --More--", message_display_text(&line)));
        } else {
            if let Some(last) = state.slow_discovery_lines.last().cloned() {
                state.game.remember_message(last);
            }
            state.slow_discovery_lines.clear();
            state.pending = None;
            state.modal = None;
            state.game.message_without_recall("");
        }
        return;
    }
    if keys.just_pressed(KeyCode::Space)
        && let Some(modal) = &state.modal
        && !state.modal_overlay
    {
        if modal_has_next_page(modal, state.modal_offset) {
            state.modal_offset += MODAL_PAGE_ROWS;
            return;
        }
        if state.modal_offset > 0 || state.game.end != mrzavec::game::EndState::Playing {
            if state.game.end != mrzavec::game::EndState::Playing {
                app_exit.write(AppExit::Success);
            } else {
                state.modal = None;
                state.pending = None;
                state.modal_offset = 0;
            }
            return;
        }
    }
    if state.game.end != mrzavec::game::EndState::Playing && state.score_recorded {
        return;
    }
    if state.pending == Some(Pending::More) {
        if keys.just_pressed(KeyCode::Space) {
            state.pending = None;
            state.modal = None;
            state.modal_offset = 0;
            state.modal_overlay = false;
        }
        return;
    }
    if matches!(state.pending, Some(Pending::Options(_))) && keys.just_pressed(KeyCode::Escape) {
        finish_options(&mut state);
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        if state.game.end != mrzavec::game::EndState::Playing {
            app_exit.write(AppExit::Success);
            return;
        }
        if matches!(
            state.pending,
            Some(Pending::PutRingHand(_) | Pending::RemoveRingHand)
        ) {
            state
                .game
                .finish_action(mrzavec::command::CommandResult::TURN);
        }
        if state.item_inventory_open
            && let Some(pending) = state.pending
        {
            restore_item_prompt(&mut state, pending);
            return;
        }
        if state.pending == Some(Pending::Help) {
            state.game.message(help_for('\u{1b}'));
            state.preserve_message_case = true;
            state.pending = None;
            state.modal = None;
            return;
        }
        if state.pending == Some(Pending::Password) {
            state.game.message("sorry");
            state.pending = None;
            state.modal = None;
            return;
        }
        if let Some(Pending::WizardCreateWhich(kind)) = state.pending {
            resolve_wizard_which(&mut state, kind, 0);
            return;
        }
        if let Some(Pending::WizardCreateBlessing(kind, which)) = state.pending {
            state.game.wizard_create_blessed(kind, which, '\u{1b}');
            state.pending = None;
            state.modal = None;
            continue_counted_command(&mut state);
            return;
        }
        if state.pending == Some(Pending::WizardCreateGold) {
            state.game.wizard_create_gold(0);
            state.pending = None;
            state.modal = None;
            continue_counted_command(&mut state);
            return;
        }
        if state.pending == Some(Pending::WizardCreateType) {
            state.game.wizard_create_bizarre('\u{1b}');
            state.pending = None;
            state.modal = None;
            continue_counted_command(&mut state);
            return;
        }
        if state.pending.is_some_and(prompt_resets_last) {
            state.game.reset_last_command();
        }
        if state.pending.is_some_and(prompt_clears_recall_on_escape) {
            state.game.remember_message("");
        }
        let continue_count = state.counted_command.is_some() && state.pending.is_some();
        state.modal = None;
        state.modal_overlay = false;
        state.modal_offset = 0;
        state.item_inventory_open = false;
        state.pending = None;
        if continue_count {
            continue_counted_command(&mut state);
        } else {
            state.counted_command = None;
        }
        return;
    }
    if let Some(Pending::SlowInventory(index)) = state.pending
        && keys.just_pressed(KeyCode::Space)
    {
        let next = index + 1;
        let pack_len = state
            .game
            .player
            .inventory
            .iter()
            .filter(|item| item.in_pack)
            .count();
        if next + 1 < pack_len {
            state.pending = Some(Pending::SlowInventory(next));
            state.modal = Some(slow_inventory_line(&state.game, next));
        } else if next < pack_len {
            let line = inventory_line(&state.game, next);
            state.game.message(line);
            state.pending = None;
            state.modal = None;
        } else {
            state.pending = None;
            state.modal = None;
        }
        return;
    }
    if let Some(Pending::Options(index)) = state.pending {
        if keys.just_pressed(KeyCode::Enter) {
            if index >= 7 {
                let input = std::mem::take(&mut state.input_buffer);
                set_string_option(&mut state.game, index, &input);
            }
            advance_option(&mut state, index);
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            if index >= 7 {
                state.input_buffer.pop();
                state.modal = Some(options_text(
                    &state.game,
                    Some(index),
                    Some(&state.input_buffer),
                    None,
                ));
            } else {
                let error = if index == 6 {
                    "(O, S, or C)"
                } else {
                    "(T or F)"
                };
                state.modal = Some(options_text(&state.game, Some(index), None, Some(error)));
            }
            return;
        }
    }
    if let Some(
        pending @ (Pending::CallText(_)
        | Pending::AutoCall
        | Pending::Password
        | Pending::StartupPassword
        | Pending::SaveFileText
        | Pending::WizardCreateGold),
    ) = state.pending
    {
        if keys.just_pressed(KeyCode::Enter) {
            let input = std::mem::take(&mut state.input_buffer);
            match pending {
                Pending::CallText(id) => {
                    state.game.call_item(id, input);
                }
                Pending::AutoCall => state.game.finish_pending_call(input),
                Pending::Password | Pending::StartupPassword => {
                    if wizard_password_matches(&input) {
                        if pending == Pending::StartupPassword
                            && let Some(seed) = startup_wizard_seed()
                        {
                            state.game = Game::new(seed);
                        }
                        if pending == Pending::StartupPassword {
                            state.game.set_startup_wizard()
                        } else {
                            state.game.set_wizard(true)
                        }
                    } else if pending == Pending::Password {
                        state.game.message("sorry")
                    }
                }
                Pending::SaveFileText => {
                    if input.is_empty() {
                        state.modal = Some(remembered_prompt(&mut state, "file name: "));
                        return;
                    }
                    state.game.options.save_file = mrzavec::game::normalize_option_string(&input);
                    if save_exists(&state.game).unwrap_or(false) {
                        state.pending = Some(Pending::SaveOverwrite);
                        state.modal = Some(remembered_prompt(
                            &mut state,
                            "File exists.  Do you wish to overwrite it?",
                        ));
                    } else {
                        save_and_exit(&mut state, &mut app_exit);
                    }
                    return;
                }
                Pending::WizardCreateGold => {
                    state.game.wizard_create_gold(parse_c_integer(&input));
                }
                _ => unreachable!(),
            }
            state.pending = None;
            state.modal = None;
            if matches!(pending, Pending::AutoCall | Pending::WizardCreateGold) {
                continue_counted_command(&mut state);
            }
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.input_buffer.pop();
            state.modal = Some(match pending {
                Pending::Password | Pending::StartupPassword => password_prompt(pending).into(),
                Pending::CallText(_) | Pending::AutoCall => {
                    format!("{}{}", call_prompt(&state.game), state.input_buffer)
                }
                Pending::SaveFileText => format!("file name: {}", state.input_buffer),
                Pending::WizardCreateGold => format!("how much?{}", state.input_buffer),
                _ => unreachable!(),
            });
            return;
        }
    }
    let control = if controlled {
        [
            (KeyCode::KeyH, '\u{8}'),
            (KeyCode::KeyJ, '\u{a}'),
            (KeyCode::KeyK, '\u{b}'),
            (KeyCode::KeyL, '\u{c}'),
            (KeyCode::KeyY, '\u{19}'),
            (KeyCode::KeyU, '\u{15}'),
            (KeyCode::KeyB, '\u{2}'),
            (KeyCode::KeyN, '\u{e}'),
            (KeyCode::KeyG, '\u{7}'),
            (KeyCode::KeyW, '\u{17}'),
            (KeyCode::KeyD, '\u{4}'),
            (KeyCode::KeyA, '\u{1}'),
            (KeyCode::KeyF, '\u{6}'),
            (KeyCode::KeyT, '\u{14}'),
            (KeyCode::KeyE, '\u{5}'),
            (KeyCode::KeyQ, '\u{11}'),
            (KeyCode::KeyX, '\u{18}'),
            (KeyCode::KeyI, '\u{9}'),
            (KeyCode::KeyP, '\u{10}'),
            (KeyCode::KeyR, '\u{12}'),
            (KeyCode::KeyZ, '\u{1a}'),
        ]
        .into_iter()
        .find_map(|(key, ch)| keys.just_pressed(key).then_some(ch))
    } else {
        None
    };
    let special = control.or_else(|| {
        if keys.just_pressed(KeyCode::Period) {
            Some(if shifted { '>' } else { '.' })
        } else if keys.just_pressed(KeyCode::Comma) {
            Some(if shifted { '<' } else { ',' })
        } else if keys.just_pressed(KeyCode::Slash) {
            Some(if shifted { '?' } else { '/' })
        } else if keys.just_pressed(KeyCode::Digit6) && shifted {
            Some('^')
        } else if keys.just_pressed(KeyCode::Digit0) && shifted {
            Some(')')
        } else if keys.just_pressed(KeyCode::BracketRight) {
            Some(']')
        } else if keys.just_pressed(KeyCode::Equal) {
            Some(if shifted { '+' } else { '=' })
        } else if keys.just_pressed(KeyCode::Backslash) && shifted {
            Some('|')
        } else if keys.just_pressed(KeyCode::Digit4) && shifted {
            Some('$')
        } else if keys.just_pressed(KeyCode::Backquote) && shifted {
            Some('~')
        } else if keys.just_pressed(KeyCode::Digit8) && shifted {
            Some('*')
        } else if keys.just_pressed(KeyCode::Digit5) && shifted {
            Some('%')
        } else if keys.just_pressed(KeyCode::Digit3) && shifted {
            Some('#')
        } else if keys.just_pressed(KeyCode::Minus) {
            Some(if shifted { '_' } else { '-' })
        } else if keys.just_pressed(KeyCode::Space) {
            Some(' ')
        } else if keys.just_pressed(KeyCode::Digit1) && shifted {
            Some('!')
        } else if keys.just_pressed(KeyCode::Semicolon) && shifted {
            Some(':')
        } else if keys.just_pressed(KeyCode::Digit2) && shifted {
            Some('@')
        } else if !shifted {
            [
                (KeyCode::Digit0, '0'),
                (KeyCode::Digit1, '1'),
                (KeyCode::Digit2, '2'),
                (KeyCode::Digit3, '3'),
                (KeyCode::Digit4, '4'),
                (KeyCode::Digit5, '5'),
                (KeyCode::Digit6, '6'),
                (KeyCode::Digit7, '7'),
                (KeyCode::Digit8, '8'),
                (KeyCode::Digit9, '9'),
            ]
            .into_iter()
            .find_map(|(key, ch)| keys.just_pressed(key).then_some(ch))
        } else {
            None
        }
    });
    let ch = special
        .or_else(|| {
            MOVEMENT_KEYS
                .into_iter()
                .chain([
                    (KeyCode::KeyS, 's'),
                    (KeyCode::KeyI, 'i'),
                    (KeyCode::KeyQ, 'q'),
                    (KeyCode::KeyR, 'r'),
                    (KeyCode::KeyE, 'e'),
                    (KeyCode::KeyW, 'w'),
                    (KeyCode::KeyT, 't'),
                    (KeyCode::KeyP, 'p'),
                    (KeyCode::KeyD, 'd'),
                    (KeyCode::KeyZ, 'z'),
                    (KeyCode::KeyF, 'f'),
                    (KeyCode::KeyM, 'm'),
                    (KeyCode::KeyA, 'a'),
                    (KeyCode::KeyC, 'c'),
                    (KeyCode::KeyG, 'g'),
                    (KeyCode::KeyO, 'o'),
                    (KeyCode::KeyV, 'v'),
                    (KeyCode::KeyX, 'x'),
                ])
                .find_map(|(k, c)| keys.just_pressed(k).then_some(c))
        })
        .or(repeated_movement);
    let Some(mut ch) = ch else { return };
    if shifted && ch.is_ascii_alphabetic() {
        ch = ch.to_ascii_uppercase()
    }
    if state.game.player.conditions.asleep_turns > 0 {
        state.pending = None;
        state.modal = None;
        state.count_prefix.clear();
        state.counted_command = None;
        state.game.execute(Command::Rest);
        return;
    }
    if state.item_inventory_open {
        return;
    }
    if let Some(pending) = state.pending {
        if matches!(pending, Pending::SaveConfirm | Pending::SaveOverwrite) {
            match (pending, ch.to_ascii_lowercase()) {
                (Pending::SaveConfirm, 'y') | (Pending::SaveOverwrite, 'y') => {
                    save_and_exit(&mut state, &mut app_exit);
                }
                (Pending::SaveConfirm, 'n') => {
                    state.input_buffer.clear();
                    state.pending = Some(Pending::SaveFileText);
                    state.modal = Some(remembered_prompt(&mut state, "file name: "));
                }
                (Pending::SaveOverwrite, 'n') => {
                    state.pending = Some(Pending::SaveConfirm);
                    let prompt = save_confirmation(&state.game);
                    state.modal = Some(remembered_prompt(&mut state, prompt));
                }
                _ => {
                    let error = if pending == Pending::SaveConfirm {
                        "please answer Y or N"
                    } else {
                        "Please answer Y or N"
                    };
                    state.game.message(error);
                    let prompt = if pending == Pending::SaveConfirm {
                        save_confirmation(&state.game)
                    } else {
                        "File exists.  Do you wish to overwrite it?".into()
                    };
                    state.game.remember_message(&prompt);
                    state.modal = Some(message_display_text(&prompt));
                }
            }
            return;
        }
        if pending == Pending::QuitConfirm {
            resolve_quit_confirmation(&mut state, ch);
            return;
        }
        if let Pending::PutRingHand(id) = pending {
            let hand = match ch.to_ascii_lowercase() {
                'l' => Some(0),
                'r' => Some(1),
                _ => None,
            };
            if let Some(hand) = hand {
                state.game.last_hand = Some(hand);
                let result = state.game.put_on_ring(id, hand);
                state.game.finish_action(result);
                state.pending = None;
                state.modal = None;
            } else {
                retry_ring_hand(&mut state);
            }
            return;
        }
        if pending == Pending::RemoveRingHand {
            let hand = match ch.to_ascii_lowercase() {
                'l' => Some(0),
                'r' => Some(1),
                _ => None,
            };
            if let Some(hand) = hand {
                state.game.last_hand = Some(hand);
                let result = state.game.remove_ring(hand);
                state.game.finish_action(result);
                state.pending = None;
                state.modal = None;
            } else {
                retry_ring_hand(&mut state);
            }
            return;
        }
        if pending == Pending::Help {
            state.pending = None;
            if ch == '*' {
                state.game.remember_message("");
                state.pending = Some(Pending::More);
                state.modal = Some(help_text());
            } else {
                state.game.message(help_for(ch));
                state.preserve_message_case = true;
                state.modal = None;
            }
            return;
        }
        if pending == Pending::IdentifyGlyph {
            state.pending = None;
            let text = identify_glyph_text(ch);
            state.game.message(text);
            state.modal = None;
            return;
        }
        if pending == Pending::Discoveries {
            if matches!(ch, '!' | '?' | '=' | '/' | '*') {
                start_discoveries(&mut state, ch);
            } else {
                let error = if state.game.options.terse {
                    "Not a type"
                } else {
                    "Please type one of !?=/ (ESCAPE to quit)"
                };
                state.game.message(error);
                let prompt = discoveries_prompt(&state.game);
                state.game.remember_message(&prompt);
                state.modal = Some(message_display_text(&prompt));
            }
            return;
        }
        if pending == Pending::PickyInventory {
            state.pending = None;
            state.modal = None;
            if let Some(index) = state.game.inventory_index_for_letter(ch) {
                let item = &state.game.player.inventory[index];
                let message = format!("{ch}) {}", state.game.inventory_name(item, false));
                state.game.message(message);
            } else {
                state
                    .game
                    .message(format!("'{}' not in pack", control_label(ch)));
            }
            continue_counted_command(&mut state);
            return;
        }
        if pending == Pending::WizardListType {
            state.game.remember_message("");
            state.modal = wizard_probability_text(ch);
            state.pending = state.modal.is_some().then_some(Pending::More);
            return;
        }
        if pending == Pending::WizardCreateType {
            if let Some(kind) = match ch {
                '!' => Some(ItemKind::Potion),
                '?' => Some(ItemKind::Scroll),
                ')' => Some(ItemKind::Weapon),
                ']' => Some(ItemKind::Armor),
                '=' => Some(ItemKind::Ring),
                '/' => Some(ItemKind::Stick),
                ':' => Some(ItemKind::Food),
                ',' => Some(ItemKind::Amulet),
                '*' => Some(ItemKind::Gold),
                _ => None,
            } {
                if kind == ItemKind::Gold {
                    state.input_buffer.clear();
                    state.pending = Some(Pending::WizardCreateGold);
                    state.modal = Some(remembered_prompt(&mut state, "how much?"));
                } else if matches!(kind, ItemKind::Food | ItemKind::Amulet) {
                    state.game.wizard_create(kind, 0);
                    state.pending = None;
                    state.modal = None;
                    continue_counted_command(&mut state);
                } else {
                    state.pending = Some(Pending::WizardCreateWhich(kind));
                    let prompt = wizard_which_prompt(kind);
                    state.modal = Some(remembered_prompt(&mut state, prompt));
                }
            } else {
                state.game.wizard_create_bizarre(ch);
                state.pending = None;
                state.modal = None;
                continue_counted_command(&mut state);
            }
            return;
        }
        if let Pending::WizardCreateWhich(kind) = pending {
            let which = if ch.is_ascii_digit() {
                ch as u8 - b'0'
            } else if ch.is_ascii_lowercase() {
                ch as u8 - b'a' + 10
            } else {
                0
            };
            resolve_wizard_which(&mut state, kind, which);
            return;
        }
        if let Pending::WizardCreateBlessing(kind, which) = pending {
            state
                .game
                .wizard_create_blessed(kind, which, ch.to_ascii_lowercase());
            state.pending = None;
            state.modal = None;
            continue_counted_command(&mut state);
            return;
        }
        if let Pending::Options(index) = pending {
            if index < 6 {
                match ch {
                    't' | 'T' => set_boolean_option(&mut state.game, index, true),
                    'f' | 'F' => set_boolean_option(&mut state.game, index, false),
                    '-' => {
                        if index > 0 {
                            show_option(&mut state, index - 1);
                        }
                        return;
                    }
                    _ => {
                        state.modal = Some(options_text(
                            &state.game,
                            Some(index),
                            None,
                            Some("(T or F)"),
                        ));
                        return;
                    }
                }
                advance_option(&mut state, index);
                return;
            }
            if index == 6 {
                state.game.options.inventory_style = match ch {
                    'o' | 'O' => mrzavec::game::InventoryStyle::Overwrite,
                    's' | 'S' => mrzavec::game::InventoryStyle::Slow,
                    'c' | 'C' => mrzavec::game::InventoryStyle::Clear,
                    '-' => {
                        show_option(&mut state, index - 1);
                        return;
                    }
                    _ => {
                        state.modal = Some(options_text(
                            &state.game,
                            Some(index),
                            None,
                            Some("(O, S, or C)"),
                        ));
                        return;
                    }
                };
                advance_option(&mut state, index);
                return;
            }
            if ch == '-' && state.input_buffer.is_empty() {
                show_option(&mut state, index - 1);
                return;
            }
            if ch == '\u{15}' {
                state.input_buffer.clear();
            } else if matches!(ch, '\u{8}' | '\u{7f}') {
                state.input_buffer.pop();
            } else if state.input_buffer.is_empty() && ch == '~' {
                state.input_buffer = option_home_directory();
            } else if !ch.is_control() && state.input_buffer.len() < 50 {
                state.input_buffer.push(ch);
            } else {
                return;
            }
            if state.input_buffer.len() > 50 {
                state.input_buffer.truncate(50);
            }
            state.modal = Some(options_text(
                &state.game,
                Some(index),
                Some(&state.input_buffer),
                None,
            ));
            return;
        }
        if matches!(
            pending,
            Pending::CallText(_)
                | Pending::AutoCall
                | Pending::Password
                | Pending::StartupPassword
                | Pending::SaveFileText
                | Pending::WizardCreateGold
        ) {
            if !ch.is_control() && state.input_buffer.len() < 50 {
                state.input_buffer.push(ch);
                state.modal = Some(match pending {
                    Pending::StartupPassword => password_prompt(pending).into(),
                    Pending::Password => message_display_text(password_prompt(pending)),
                    Pending::CallText(_) | Pending::AutoCall => {
                        format!(
                            "{}{}",
                            message_display_text(call_prompt(&state.game)),
                            state.input_buffer
                        )
                    }
                    Pending::SaveFileText => format!("File name: {}", state.input_buffer),
                    Pending::WizardCreateGold => format!("How much?{}", state.input_buffer),
                    _ => unreachable!(),
                });
            }
            return;
        }
        if matches!(
            pending,
            Pending::ThrowDirection
                | Pending::ZapDirection
                | Pending::FightDirection(_)
                | Pending::MoveDirection
                | Pending::TrapDirection
        ) {
            if state.item_inventory_open {
                return;
            }
            if let Command::Move(direction) = parse(ch.to_ascii_lowercase()) {
                state.game.remember_message("");
                let result = match pending {
                    Pending::ThrowDirection => {
                        state.game.last_direction = Some(direction);
                        if state.game.player.inventory.is_empty() {
                            state.game.message("you aren't carrying anything");
                            state
                                .game
                                .finish_action(mrzavec::command::CommandResult::TURN);
                            state.pending = None;
                            state.modal = None;
                            continue_counted_command(&mut state);
                            return;
                        }
                        state.modal = select_prompt(&mut state, Pending::ThrowSelect);
                        return;
                    }
                    Pending::ZapDirection => {
                        state.game.last_direction = Some(direction);
                        if state.game.player.inventory.is_empty() {
                            state.game.message("you aren't carrying anything");
                            state
                                .game
                                .finish_action(mrzavec::command::CommandResult::TURN);
                            state.pending = None;
                            state.modal = None;
                            continue_counted_command(&mut state);
                            return;
                        }
                        state.modal = select_prompt(&mut state, Pending::ZapSelect);
                        return;
                    }
                    Pending::FightDirection(kamikaze) => {
                        state.game.last_direction = Some(direction);
                        state.game.fight_direction(direction, kamikaze)
                    }
                    Pending::MoveDirection => {
                        state.game.last_direction = Some(direction);
                        state.game.move_without_pickup(direction)
                    }
                    Pending::TrapDirection => {
                        state.game.last_direction = Some(direction);
                        state.game.identify_trap(direction)
                    }
                    _ => unreachable!(),
                };
                state.game.finish_action(result);
                state.pending = None;
                state.modal = None;
                continue_counted_command(&mut state);
            }
            return;
        }
        if item_selection_title(pending).is_some() && ch == '*' {
            show_item_inventory(&mut state, pending);
            return;
        }
        if item_selection_title(pending).is_some() && !ch.is_ascii_lowercase() {
            retry_invalid_item(&mut state, pending, ch);
            return;
        }
        if ch.is_ascii_lowercase() {
            let Some(index) = state.game.inventory_index_for_letter(ch) else {
                if item_selection_title(pending).is_some() {
                    retry_invalid_item(&mut state, pending, ch);
                }
                return;
            };
            if let Some(id) = state.game.player.inventory.get(index).map(|i| i.id) {
                state.game.remember_message("");
                let magic_detection = pending == Pending::Quaff
                    && state.game.player.inventory.iter().any(|item| {
                        item.id == id && item.kind == ItemKind::Potion && item.which == 7
                    });
                let food_detection = pending == Pending::Read
                    && state.game.player.inventory.iter().any(|item| {
                        item.id == id && item.kind == ItemKind::Scroll && item.which == 11
                    });
                state.game.last_item = Some(id);
                let result = match pending {
                    Pending::Quaff => state.game.quaff(id),
                    Pending::Read => state.game.read_scroll(id),
                    Pending::Eat => state.game.eat(id),
                    Pending::Wield => state.game.wield(id),
                    Pending::Wear => state.game.wear(id),
                    Pending::PutRing => {
                        if !state
                            .game
                            .player
                            .inventory
                            .iter()
                            .any(|item| item.id == id && item.kind == ItemKind::Ring)
                        {
                            state.game.put_on_ring(id, 0)
                        } else {
                            let [left, right] = state.game.player.rings;
                            if left.is_some() && right.is_some() {
                                let terse = state.game.options.terse;
                                state.game.message(if terse {
                                    "wearing two"
                                } else {
                                    "you already have a ring on each hand"
                                });
                                mrzavec::command::CommandResult::TURN
                            } else {
                                if left.is_none() && right.is_none() {
                                    state.pending = Some(Pending::PutRingHand(id));
                                    let prompt = ring_hand_prompt(&state.game);
                                    state.game.remember_message(&prompt);
                                    state.modal = Some(message_display_text(&prompt));
                                    return;
                                }
                                let hand = usize::from(left.is_some());
                                state.game.last_hand = Some(hand);
                                state.game.put_on_ring(id, hand)
                            }
                        }
                    }
                    Pending::Drop => state.game.drop_item(id),
                    Pending::Identify => state.game.identify_item(id),
                    Pending::WizardCharge => {
                        state.game.wizard_charge(id);
                        mrzavec::command::CommandResult::FREE
                    }
                    Pending::CallSelect => {
                        begin_manual_call(&mut state, id);
                        return;
                    }
                    Pending::ThrowSelect => {
                        let direction = state.game.last_direction.unwrap();
                        state.game.throw_item(id, direction)
                    }
                    Pending::ZapSelect => {
                        let direction = state.game.last_direction.unwrap();
                        state.game.zap(id, direction)
                    }
                    Pending::ThrowDirection | Pending::ZapDirection => unreachable!(),
                    Pending::FightDirection(_)
                    | Pending::MoveDirection
                    | Pending::TrapDirection => {
                        unreachable!()
                    }
                    Pending::CallText(_) => unreachable!(),
                    Pending::AutoCall => unreachable!(),
                    Pending::Options(_) => unreachable!(),
                    Pending::MagicDetection | Pending::FoodDetection => unreachable!(),
                    Pending::Password | Pending::StartupPassword => unreachable!(),
                    Pending::QuitConfirm => unreachable!(),
                    Pending::SaveConfirm
                    | Pending::SaveFileText
                    | Pending::SaveOverwrite
                    | Pending::WizardCreateGold => {
                        unreachable!()
                    }
                    Pending::PutRingHand(_) => unreachable!(),
                    Pending::RemoveRingHand => unreachable!(),
                    Pending::WizardCreateType
                    | Pending::WizardCreateWhich(_)
                    | Pending::WizardCreateBlessing(_, _) => unreachable!(),
                    Pending::Help
                    | Pending::IdentifyGlyph
                    | Pending::Discoveries
                    | Pending::WizardListType
                    | Pending::PickyInventory
                    | Pending::SlowInventory(_)
                    | Pending::DiscoveryMore
                    | Pending::SlowDiscoveryPrompt
                    | Pending::SlowDiscovery(_)
                    | Pending::More => unreachable!(),
                };
                state.game.finish_action(result);
                if magic_detection && !state.game.magic_positions().is_empty() {
                    state.pending = Some(Pending::MagicDetection);
                    state.modal = Some(magic_detection_text(&state.game));
                    return;
                }
                if food_detection && !state.game.food_positions().is_empty() {
                    state.pending = Some(Pending::FoodDetection);
                    state.modal = Some(food_detection_text(&state.game));
                    return;
                }
                if state.game.pending_identification.is_some()
                    && show_pending_identification(&mut state)
                {
                    return;
                }
                if show_pending_call(&mut state) {
                    return;
                }
            }
            state.pending = None;
            state.modal = None;
            continue_counted_command(&mut state);
        }
        return;
    }
    if ch.is_ascii_digit() {
        if state.count_prefix.len() < 3 {
            state.count_prefix.push(ch);
            let value = state.count_prefix.parse::<u16>().unwrap_or(0).min(255);
            state.count_prefix = value.to_string();
        }
        return;
    }
    let requested_repeats = state.count_prefix.parse::<u16>().unwrap_or(1).clamp(1, 255);
    let repeats = if countable_command(ch) {
        requested_repeats
    } else {
        1
    };
    state.count_prefix.clear();
    state.modal_overlay = false;
    let parsed = parse(ch);
    let repeated = parsed == Command::Repeat;
    let command = if repeated {
        let Some(last) = state.game.last_command else {
            state.game.message("you haven't typed a command yet");
            return;
        };
        parse(last)
    } else {
        parsed
    };
    let effective_repeats =
        if repeats > 1 && matches!(command, Command::Wizard(_)) && !state.game.wizard {
            1
        } else {
            repeats
        };
    if repeats == 1 && ch != 'a' && !matches!(command, Command::Cancel) {
        state.game.remember_command(ch);
    }
    if repeated {
        let mut handled = false;
        for _ in 0..repeats {
            if !repeat_selected_command(&mut state, command) {
                break;
            }
            handled = true;
            if state.game.end != mrzavec::game::EndState::Playing {
                break;
            }
        }
        if handled {
            return;
        }
    }
    if effective_repeats > 1
        && matches!(
            command,
            Command::Quaff
                | Command::Read
                | Command::Throw
                | Command::Zap
                | Command::MoveWithoutPickup
                | Command::PickyInventory
                | Command::Wizard(WizardCommand::Create)
        )
    {
        state.counted_command = Some((command, effective_repeats - 1));
    }
    state.game.begin_command();
    if state.game.player.conditions.asleep_turns > 0 {
        state.game.execute_after_begin(command);
        return;
    }
    let modal = match command {
        Command::Inventory => inventory_modal(&mut state),
        Command::PickyInventory => picky_inventory_prompt(&mut state),
        Command::IdentifyObject => {
            state.pending = Some(Pending::IdentifyGlyph);
            Some(remembered_prompt(
                &mut state,
                "what do you want identified? ",
            ))
        }
        Command::Help => {
            state.pending = Some(Pending::Help);
            Some(remembered_prompt(
                &mut state,
                "character you want help for (* for all): ",
            ))
        }
        Command::Discoveries => {
            state.pending = Some(Pending::Discoveries);
            let prompt = discoveries_prompt(&state.game);
            Some(remembered_prompt(&mut state, prompt))
        }
        Command::Options => {
            state.pending = Some(Pending::Options(0));
            Some(options_text(&state.game, Some(0), None, None))
        }
        Command::Recall => {
            recall_last_message(&mut state.game);
            None
        }
        Command::Version => {
            let message = version_message(&state.game);
            state.game.message(message);
            None
        }
        Command::Shell => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
                if let Err(error) = std::process::Command::new(shell).status() {
                    state
                        .game
                        .message(format!("could not start shell: {error}"));
                }
            }
            #[cfg(target_arch = "wasm32")]
            state.game.message("shell is unavailable in a web browser");
            None
        }
        Command::Suspend => {
            state
                .game
                .message("suspend is unavailable in the windowed interface");
            None
        }
        Command::Quit => {
            state.pending = Some(Pending::QuitConfirm);
            Some(remembered_prompt(&mut state, "really quit?"))
        }
        Command::Save => {
            state.pending = Some(Pending::SaveConfirm);
            let prompt = save_confirmation(&state.game);
            Some(remembered_prompt(&mut state, prompt))
        }
        Command::Quaff => select_action_prompt(&mut state, Pending::Quaff, true),
        Command::Read => select_action_prompt(&mut state, Pending::Read, true),
        Command::Eat => select_action_prompt(&mut state, Pending::Eat, true),
        Command::Wield => {
            let cursed_weapon = state.game.player.weapon.is_some_and(|id| {
                state
                    .game
                    .player
                    .inventory
                    .iter()
                    .any(|item| item.id == id && item.cursed)
            });
            if cursed_weapon {
                state.game.message("you can't.  It appears to be cursed");
                state
                    .game
                    .finish_action(mrzavec::command::CommandResult::TURN);
                None
            } else {
                select_action_prompt(&mut state, Pending::Wield, false)
            }
        }
        Command::Wear => select_action_prompt(&mut state, Pending::Wear, true),
        Command::PutOnRing => select_action_prompt(&mut state, Pending::PutRing, true),
        Command::Drop => select_action_prompt(&mut state, Pending::Drop, true),
        Command::Throw => direction_prompt(&mut state, Pending::ThrowDirection),
        Command::Zap => direction_prompt(&mut state, Pending::ZapDirection),
        Command::Fight { kamikaze } => {
            direction_prompt(&mut state, Pending::FightDirection(kamikaze))
        }
        Command::MoveWithoutPickup => direction_prompt(&mut state, Pending::MoveDirection),
        Command::IdentifyTrap => direction_prompt(&mut state, Pending::TrapDirection),
        Command::Call => select_action_prompt(&mut state, Pending::CallSelect, false),
        Command::ToggleWizard => {
            if state.game.wizard {
                state.game.set_wizard(false);
                None
            } else {
                state.input_buffer.clear();
                state.pending = Some(Pending::Password);
                Some(remembered_prompt(&mut state, "wizard's Password: "))
            }
        }
        Command::RemoveRing => match state.game.player.rings {
            [None, None] => {
                let terse = state.game.options.terse;
                state.game.message(if terse {
                    "no rings"
                } else {
                    "you aren't wearing any rings"
                });
                state
                    .game
                    .finish_action(mrzavec::command::CommandResult::TURN);
                None
            }
            [Some(_), Some(_)] => {
                state.pending = Some(Pending::RemoveRingHand);
                let prompt = ring_hand_prompt(&state.game);
                state.game.remember_message(&prompt);
                Some(message_display_text(&prompt))
            }
            [Some(_), None] => {
                let result = state.game.remove_ring(0);
                state.game.finish_action(result);
                None
            }
            [None, Some(_)] => {
                let result = state.game.remove_ring(1);
                state.game.finish_action(result);
                None
            }
        },
        Command::CurrentWeapon => {
            let message = current_message(&state.game, state.game.player.weapon, "wielding", None);
            state.game.message(message);
            None
        }
        Command::CurrentArmor => {
            let message = current_message(&state.game, state.game.player.armor, "wearing", None);
            state.game.message(message);
            None
        }
        Command::CurrentRings => {
            for (id, verbose_where, terse_where) in [
                (state.game.player.rings[0], "on left hand", "(L)"),
                (state.game.player.rings[1], "on right hand", "(R)"),
            ] {
                let location = if state.game.options.terse {
                    terse_where
                } else {
                    verbose_where
                };
                let message = current_message(&state.game, id, "wearing", Some(location));
                state.game.message(message);
            }
            None
        }
        Command::CurrentStats => {
            let message = status_text(&state.game);
            state.game.message(message);
            None
        }
        Command::Wizard(WizardCommand::GroundInventory) if state.game.wizard => {
            let modal = ground_inventory_modal(&mut state);
            if modal.is_some() {
                state.pending = Some(Pending::More);
            }
            modal
        }
        Command::Wizard(WizardCommand::List) if state.game.wizard => {
            state.pending = Some(Pending::WizardListType);
            let prompt = wizard_list_prompt(&state.game);
            Some(remembered_prompt(&mut state, prompt))
        }
        Command::Wizard(WizardCommand::Map) if state.game.wizard => {
            state.pending = Some(Pending::More);
            Some(wizard_map_text(&state.game))
        }
        Command::Wizard(WizardCommand::Identify) if state.game.wizard => {
            wizard_identify_prompt(&mut state)
        }
        Command::Wizard(WizardCommand::Charge) if state.game.wizard => {
            wizard_charge_prompt(&mut state)
        }
        Command::Wizard(WizardCommand::Create) if state.game.wizard => {
            state.pending = Some(Pending::WizardCreateType);
            Some(remembered_prompt(&mut state, "type of item: "))
        }
        _ => None,
    };
    state.modal = modal;
    if !matches!(command, Command::Quit | Command::Save) {
        state.game.execute_after_begin(command);
        if state.pending.is_none() && state.counted_command.is_none() {
            for iteration in 1..effective_repeats {
                if iteration + 1 == effective_repeats && ch != 'a' {
                    state.game.remember_command(ch);
                }
                state.game.execute(command);
                if state.game.end != mrzavec::game::EndState::Playing {
                    break;
                }
            }
        } else if state.pending.is_none()
            && state.modal.is_none()
            && state.counted_command.is_some()
        {
            continue_counted_command(&mut state);
        }
    }
}

fn countable_command(ch: char) -> bool {
    matches!(
        ch,
        '\u{2}'
            | '\u{8}'
            | '\u{a}'
            | '\u{b}'
            | '\u{c}'
            | '\u{e}'
            | '\u{15}'
            | '\u{19}'
            | '\u{1a}'
            | '.'
            | 'a'
            | 'b'
            | 'h'
            | 'j'
            | 'k'
            | 'l'
            | 'm'
            | 'n'
            | 'q'
            | 'r'
            | 's'
            | 't'
            | 'u'
            | 'y'
            | 'z'
            | 'B'
            | 'C'
            | 'H'
            | 'I'
            | 'J'
            | 'K'
            | 'L'
            | 'N'
            | 'U'
            | 'Y'
            | '\u{1}'
            | '\u{4}'
    )
}

fn prompt_resets_last(pending: Pending) -> bool {
    matches!(
        pending,
        Pending::Quaff
            | Pending::Read
            | Pending::Eat
            | Pending::Wield
            | Pending::Wear
            | Pending::PutRing
            | Pending::Drop
            | Pending::ThrowSelect
            | Pending::ZapSelect
            | Pending::ThrowDirection
            | Pending::ZapDirection
            | Pending::FightDirection(_)
            | Pending::MoveDirection
            | Pending::TrapDirection
            | Pending::Identify
            | Pending::CallSelect
            | Pending::WizardCharge
    )
}

fn prompt_clears_recall_on_escape(pending: Pending) -> bool {
    prompt_resets_last(pending)
        || matches!(
            pending,
            Pending::IdentifyGlyph
                | Pending::Discoveries
                | Pending::PickyInventory
                | Pending::CallText(_)
                | Pending::AutoCall
                | Pending::SaveConfirm
                | Pending::SaveFileText
                | Pending::SaveOverwrite
                | Pending::WizardListType
        )
}

fn counted_command_char(command: Command) -> Option<char> {
    match command {
        Command::Quaff => Some('q'),
        Command::Read => Some('r'),
        Command::Throw => Some('t'),
        Command::Zap => Some('z'),
        Command::MoveWithoutPickup => Some('m'),
        Command::PickyInventory => Some('I'),
        Command::Wizard(WizardCommand::Create) => Some('C'),
        _ => None,
    }
}

fn resolve_quit_confirmation(state: &mut State, ch: char) {
    if matches!(ch, 'y' | 'Y') {
        state.game.end = mrzavec::game::EndState::Quit;
    } else {
        state.pending = None;
        state.modal = None;
    }
}

fn save_confirmation(game: &Game) -> String {
    format!("save file ({})? ", game.options.save_file)
}

#[cfg(not(target_arch = "wasm32"))]
fn save_exists(game: &Game) -> Result<bool, String> {
    Ok(std::path::Path::new(&game.options.save_file).exists())
}

#[cfg(target_arch = "wasm32")]
fn save_exists(game: &Game) -> Result<bool, String> {
    let storage = mrzavec::platform::LocalStorage::open().map_err(|error| error.to_string())?;
    save::storage_has_save(&game.options.save_file, &storage).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn persist_game(game: &Game) -> Result<String, String> {
    let path = std::path::PathBuf::from(&game.options.save_file);
    save::save(game, &path).map_err(|error| error.to_string())?;
    Ok(path.display().to_string())
}

#[cfg(target_arch = "wasm32")]
fn persist_game(game: &Game) -> Result<String, String> {
    let storage = mrzavec::platform::LocalStorage::open().map_err(|error| error.to_string())?;
    save::save_to_storage(game, &game.options.save_file, &storage)
        .map_err(|error| error.to_string())?;
    Ok(format!("browser slot {}", game.options.save_file))
}

fn save_and_exit(state: &mut State, app_exit: &mut MessageWriter<AppExit>) {
    match persist_game(&state.game) {
        Ok(destination) => {
            state.game.message(format!("game saved to {destination}"));
            app_exit.write(AppExit::Success);
        }
        Err(error) => {
            state.game.message(format!("save failed: {error}"));
            state.pending = Some(Pending::SaveFileText);
            state.input_buffer.clear();
            state.modal = Some(remembered_prompt(state, "file name: "));
        }
    }
}

fn repeat_selected_command(state: &mut State, command: Command) -> bool {
    if !matches!(
        command,
        Command::Quaff
            | Command::Read
            | Command::Eat
            | Command::Wield
            | Command::Wear
            | Command::Drop
            | Command::PutOnRing
            | Command::Throw
            | Command::Zap
            | Command::Fight { .. }
            | Command::MoveWithoutPickup
            | Command::IdentifyTrap
            | Command::Call
    ) {
        return false;
    }
    state.game.begin_command();
    let item = || {
        state.game.last_item.filter(|id| {
            state
                .game
                .player
                .inventory
                .iter()
                .any(|item| item.id == *id)
        })
    };
    let food_detection = command == Command::Read
        && item().is_some_and(|id| {
            state.game.player.inventory.iter().any(|candidate| {
                candidate.id == id && candidate.kind == ItemKind::Scroll && candidate.which == 11
            })
        });
    let result = match command {
        Command::Quaff => item().map(|id| state.game.quaff(id)),
        Command::Read => item().map(|id| state.game.read_scroll(id)),
        Command::Eat => item().map(|id| state.game.eat(id)),
        Command::Wield => item().map(|id| state.game.wield(id)),
        Command::Wear => item().map(|id| state.game.wear(id)),
        Command::Drop => item().map(|id| state.game.drop_item(id)),
        Command::PutOnRing => item().map(|id| {
            state
                .game
                .put_on_ring(id, state.game.last_hand.unwrap_or(0))
        }),
        Command::Throw => item()
            .zip(state.game.last_direction)
            .map(|(id, direction)| state.game.throw_item(id, direction)),
        Command::Zap => item()
            .zip(state.game.last_direction)
            .map(|(id, direction)| state.game.zap(id, direction)),
        Command::Fight { kamikaze } => state
            .game
            .last_direction
            .map(|direction| state.game.fight_direction(direction, kamikaze)),
        Command::MoveWithoutPickup => state
            .game
            .last_direction
            .map(|direction| state.game.move_without_pickup(direction)),
        Command::IdentifyTrap => state
            .game
            .last_direction
            .map(|direction| state.game.identify_trap(direction)),
        Command::Call => {
            if let Some(id) = item() {
                begin_manual_call(state, id);
            } else {
                state.game.message("you ran out");
            }
            return true;
        }
        _ => unreachable!("unsupported repeat commands returned above"),
    };
    if let Some(result) = result {
        state.game.finish_action(result);
        if food_detection && !state.game.food_positions().is_empty() {
            state.pending = Some(Pending::FoodDetection);
            state.modal = Some(food_detection_text(&state.game));
        } else if show_pending_identification(state) {
            return true;
        } else {
            show_pending_call(state);
        }
    } else {
        state.game.message("you ran out");
        if matches!(
            command,
            Command::Quaff
                | Command::Read
                | Command::Eat
                | Command::Wear
                | Command::PutOnRing
                | Command::Drop
                | Command::Throw
                | Command::Zap
        ) {
            state
                .game
                .finish_action(mrzavec::command::CommandResult::TURN);
        }
    }
    true
}

fn begin_manual_call(state: &mut State, id: u64) {
    let Some(item) = state
        .game
        .player
        .inventory
        .iter()
        .find(|item| item.id == id)
        .cloned()
    else {
        state.game.message("you ran out");
        state.pending = None;
        state.modal = None;
        return;
    };
    let known = match item.kind {
        ItemKind::Potion => state.game.knowledge.potions[item.which as usize],
        ItemKind::Scroll => state.game.knowledge.scrolls[item.which as usize],
        ItemKind::Ring => state.game.knowledge.rings[item.which as usize],
        ItemKind::Stick => state.game.knowledge.sticks[item.which as usize],
        _ => false,
    };
    if item.kind == ItemKind::Food || known {
        state.game.call_item(id, String::new());
        state.pending = None;
        state.modal = None;
        return;
    }
    state.input_buffer = call_default(&state.game, &item);
    let guess = state
        .game
        .item_guess(&item)
        .or(item.label.as_deref())
        .map(str::to_owned);
    if let Some(guess) = guess {
        state.game.message(if state.game.options.terse {
            format!("called \"{guess}\"")
        } else {
            format!("Was called \"{guess}\"")
        });
    }
    state.pending = Some(Pending::CallText(id));
    let prompt = call_prompt(&state.game);
    state.game.remember_message(prompt);
    state.modal = Some(format!(
        "{}{}",
        message_display_text(prompt),
        state.input_buffer
    ));
}

fn show_pending_call(state: &mut State) -> bool {
    if state.game.pending_call.is_none() {
        return false;
    }
    state.input_buffer.clear();
    state.pending = Some(Pending::AutoCall);
    let prompt = call_prompt(&state.game);
    state.game.remember_message(prompt);
    state.modal = Some(message_display_text(prompt));
    true
}

fn show_pending_identification(state: &mut State) -> bool {
    if state.game.pending_identification.is_none() {
        return false;
    }
    if state.game.player.inventory.is_empty() {
        state.game.pending_identification = None;
        state
            .game
            .message("you don't have anything in your pack to identify");
        return false;
    }
    state.modal = select_prompt(state, Pending::Identify);
    true
}

fn continue_counted_command(state: &mut State) {
    if state.game.end != mrzavec::game::EndState::Playing {
        state.counted_command = None;
        return;
    }
    let Some((command, remaining)) = state.counted_command else {
        return;
    };
    if remaining == 0 {
        state.counted_command = None;
        return;
    }
    state.game.begin_command();
    while state.game.end == mrzavec::game::EndState::Playing
        && state.game.player.conditions.asleep_turns > 0
    {
        state.game.execute_after_begin(command);
        if state.game.player.conditions.asleep_turns > 0 {
            state.game.begin_command();
        }
    }
    if state.game.end != mrzavec::game::EndState::Playing {
        state.counted_command = None;
        return;
    }
    if matches!(command, Command::Wizard(_)) && !state.game.wizard {
        state.counted_command = None;
        state.game.execute_after_begin(command);
        return;
    }
    if remaining == 1
        && let Some(ch) = counted_command_char(command)
    {
        state.game.remember_command(ch);
    }
    state.counted_command = (remaining > 1).then_some((command, remaining - 1));
    state.modal = match command {
        Command::Quaff => select_action_prompt(state, Pending::Quaff, true),
        Command::Read => select_action_prompt(state, Pending::Read, true),
        Command::Throw => direction_prompt(state, Pending::ThrowDirection),
        Command::Zap => direction_prompt(state, Pending::ZapDirection),
        Command::MoveWithoutPickup => direction_prompt(state, Pending::MoveDirection),
        Command::PickyInventory => picky_inventory_prompt(state),
        Command::Wizard(WizardCommand::Create) if state.game.wizard => {
            state.pending = Some(Pending::WizardCreateType);
            Some(remembered_prompt(state, "type of item: "))
        }
        _ => {
            state.counted_command = None;
            None
        }
    };
    if state.pending.is_none() && state.modal.is_none() && state.counted_command.is_some() {
        continue_counted_command(state);
    }
}
fn ground_inventory_modal(state: &mut State) -> Option<String> {
    if state.game.floor_items.is_empty() {
        state.game.message(if state.game.options.terse {
            "empty handed"
        } else {
            "you are empty handed"
        });
        return None;
    }
    let mut out = String::new();
    for item in &state.game.floor_items {
        out.push_str(&state.game.inventory_name(item, false));
        out.push('\n');
    }
    out.push_str(" --More--");
    state.modal_overlay = state.game.options.inventory_style
        == mrzavec::game::InventoryStyle::Overwrite
        && out.lines().count() <= 23;
    Some(out)
}
fn wizard_list_prompt(game: &Game) -> String {
    if game.options.terse {
        "what type? ".into()
    } else {
        "for what type of object do you want a list? ".into()
    }
}
fn wizard_probability_text(glyph: char) -> Option<String> {
    let (names, weights): (&[&str], &[u32]) = match glyph {
        '!' => (&POTION_NAMES, &POTION_WEIGHTS),
        '?' => (&SCROLL_NAMES, &SCROLL_WEIGHTS),
        '=' => (&RING_NAMES, &RING_WEIGHTS),
        '/' => (&STICK_NAMES, &STICK_WEIGHTS),
        ']' => (&ARMOR_NAMES, &ARMOR_WEIGHTS),
        ')' => (&WEAPON_NAMES, &WEAPON_WEIGHTS),
        _ => return None,
    };
    let mut out = String::new();
    for (index, (name, probability)) in names.iter().zip(weights).enumerate() {
        let label = if index < 10 {
            (b'0' + index as u8) as char
        } else {
            (b'a' + (index - 10) as u8) as char
        };
        out.push_str(&format!("{label}: {name} ({probability}%)\n"));
    }
    out.push_str(" --More--");
    Some(out)
}
fn wizard_kind_count(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Potion => 14,
        ItemKind::Scroll => 18,
        ItemKind::Weapon => 9,
        ItemKind::Armor => 8,
        ItemKind::Ring | ItemKind::Stick => 14,
        _ => 1,
    }
}
fn wizard_kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Potion => "potion",
        ItemKind::Scroll => "scroll",
        ItemKind::Weapon => "weapon",
        ItemKind::Armor => "armor",
        ItemKind::Ring => "ring",
        ItemKind::Stick => "staff",
        _ => "item",
    }
}
fn wizard_which_prompt(kind: ItemKind) -> String {
    let highest = wizard_kind_count(kind) - 1;
    let highest = if highest < 10 {
        (b'0' + highest) as char
    } else {
        (b'a' + highest - 10) as char
    };
    format!(
        "which {} ({}) do you want? (0-{highest})",
        kind.glyph(),
        wizard_kind_name(kind)
    )
}
fn resolve_wizard_which(state: &mut State, kind: ItemKind, which: u8) {
    if which >= wizard_kind_count(kind) {
        let error = format!("invalid {}, try again", wizard_kind_name(kind));
        state.game.message(&error);
        let prompt = wizard_which_prompt(kind);
        state.game.remember_message(&prompt);
        state.modal = Some(message_display_text(&prompt));
    } else if matches!(kind, ItemKind::Weapon | ItemKind::Armor)
        || (kind == ItemKind::Ring && matches!(which, 0 | 1 | 7 | 8))
    {
        state.pending = Some(Pending::WizardCreateBlessing(kind, which));
        state.modal = Some(remembered_prompt(state, "blessing? (+,-,n) "));
    } else {
        state.game.wizard_create(kind, which);
        state.pending = None;
        state.modal = None;
        continue_counted_command(state);
    }
}
fn wizard_map_text(game: &Game) -> String {
    use mrzavec::map::Terrain;

    let mut rows = vec![vec![' '; DISPLAY_WIDTH]; 23];
    for (x, ch) in " --More--".chars().enumerate() {
        rows[0][x] = ch;
    }
    for (pos, cell) in game.dungeon.map.iter() {
        if !(1..23).contains(&pos.y) || !(0..80).contains(&pos.x) {
            continue;
        }
        let glyph = if cell.trap.is_some() {
            '^'
        } else {
            match cell.terrain {
                Terrain::SecretDoor
                | Terrain::SecretDoorHorizontal
                | Terrain::SecretDoorVertical => '+',
                Terrain::SecretPassage => '#',
                terrain => terrain.glyph(),
            }
        };
        rows[pos.y as usize][pos.x as usize] = glyph;
    }
    for item in &game.floor_items {
        if let Some(pos) = item.pos
            && (1..23).contains(&pos.y)
            && (0..80).contains(&pos.x)
        {
            rows[pos.y as usize][pos.x as usize] = item.kind.glyph();
        }
    }
    rows.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn magic_detection_text(game: &Game) -> String {
    let mut rows = vec![vec![' '; DISPLAY_WIDTH]; 23];
    let title = "You sense the presence of magic on this level. --More--";
    for (x, ch) in title.chars().take(DISPLAY_WIDTH).enumerate() {
        rows[0][x] = ch;
    }
    for pos in game.magic_positions() {
        if (1..23).contains(&pos.y) && (0..80).contains(&pos.x) {
            rows[pos.y as usize][pos.x as usize] = '$';
        }
    }
    rows.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn food_detection_text(game: &Game) -> String {
    let mut rows = vec![vec![' '; DISPLAY_WIDTH]; 23];
    let title = "Your nose tingles and you smell food. --More--";
    for (x, ch) in title.chars().take(DISPLAY_WIDTH).enumerate() {
        rows[0][x] = ch;
    }
    for pos in game.food_positions() {
        if (1..23).contains(&pos.y) && (0..80).contains(&pos.x) {
            rows[pos.y as usize][pos.x as usize] = ':';
        }
    }
    rows.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn select_prompt(state: &mut State, pending: Pending) -> Option<String> {
    let prompt = get_item_prompt(&state.game, pending);
    state.pending = Some(pending);
    state.item_inventory_open = false;
    state.modal_offset = 0;
    state.game.remember_message(&prompt);
    Some(message_display_text(&prompt))
}
fn restore_item_prompt(state: &mut State, pending: Pending) {
    state.item_inventory_open = false;
    state.modal_offset = 0;
    state.modal = select_prompt(state, pending);
}
fn direction_prompt(state: &mut State, pending: Pending) -> Option<String> {
    let prompt = if state.game.options.terse {
        "direction: "
    } else {
        "which direction? "
    };
    state.pending = Some(pending);
    state.game.remember_message(prompt);
    Some(message_display_text(prompt))
}
fn ring_hand_prompt(game: &Game) -> String {
    if game.options.terse {
        "left or right ring? ".into()
    } else {
        "left hand or right hand? ".into()
    }
}
fn retry_ring_hand(state: &mut State) {
    let error = if state.game.options.terse {
        "L or R"
    } else {
        "please type L or R"
    };
    state.game.message(error);
    let prompt = ring_hand_prompt(&state.game);
    state.game.remember_message(&prompt);
    state.modal = Some(message_display_text(&prompt));
}
fn item_selection_title(pending: Pending) -> Option<&'static str> {
    match pending {
        Pending::Quaff => Some("quaff"),
        Pending::Read => Some("read"),
        Pending::Eat => Some("eat"),
        Pending::Wield => Some("wield"),
        Pending::Wear => Some("wear"),
        Pending::PutRing => Some("put on"),
        Pending::Drop => Some("drop"),
        Pending::ThrowSelect => Some("throw"),
        Pending::ZapSelect => Some("zap with"),
        Pending::Identify => Some("identify"),
        Pending::CallSelect => Some("call"),
        Pending::WizardCharge => Some("charge"),
        _ => None,
    }
}
fn get_item_prompt(game: &Game, pending: Pending) -> String {
    let purpose = item_selection_title(pending).expect("only item selections have a purpose");
    if game.options.terse {
        format!("{purpose} what? (* for list): ")
    } else {
        format!("which object do you want to {purpose}? (* for list): ")
    }
}
fn item_matches_prompt(game: &Game, pending: Pending, kind: ItemKind) -> bool {
    match pending {
        Pending::Quaff => kind == ItemKind::Potion,
        Pending::Read => kind == ItemKind::Scroll,
        Pending::Eat => kind == ItemKind::Food,
        Pending::Wield | Pending::ThrowSelect => kind == ItemKind::Weapon,
        Pending::Wear => kind == ItemKind::Armor,
        Pending::PutRing => kind == ItemKind::Ring,
        Pending::ZapSelect | Pending::WizardCharge => kind == ItemKind::Stick,
        Pending::CallSelect => !matches!(kind, ItemKind::Food | ItemKind::Amulet),
        Pending::Identify => game
            .pending_identification
            .is_none_or(|required| match required {
                mrzavec::game::IdentifyKind::Potion => kind == ItemKind::Potion,
                mrzavec::game::IdentifyKind::Scroll => kind == ItemKind::Scroll,
                mrzavec::game::IdentifyKind::Weapon => kind == ItemKind::Weapon,
                mrzavec::game::IdentifyKind::Armor => kind == ItemKind::Armor,
                mrzavec::game::IdentifyKind::RingOrStick => {
                    matches!(kind, ItemKind::Ring | ItemKind::Stick)
                }
            }),
        Pending::Drop => true,
        _ => false,
    }
}
fn show_item_inventory(state: &mut State, pending: Pending) {
    let mut text = String::new();
    for (index, item) in state
        .game
        .player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .enumerate()
    {
        if !item_matches_prompt(&state.game, pending, item.kind) {
            continue;
        }
        let letter = item.pack_letter.unwrap_or((b'a' + index as u8) as char);
        text.push_str(&format!(
            "{letter}) {}\n",
            state.game.inventory_name(item, false)
        ));
    }
    if text.is_empty() {
        state.game.message(if state.game.options.terse {
            "nothing appropriate"
        } else {
            "you don't have anything appropriate"
        });
        state.pending = None;
        state.modal = None;
        state.item_inventory_open = false;
        return;
    }
    text.push_str(" --More--");
    state.game.remember_message("");
    state.modal = Some(text);
    state.modal_offset = 0;
    state.item_inventory_open = true;
}
fn retry_invalid_item(state: &mut State, pending: Pending, ch: char) {
    let error = format!("'{}' is not a valid item", control_label(ch));
    state.game.message(&error);
    let prompt = get_item_prompt(&state.game, pending);
    state.game.remember_message(&prompt);
    state.modal = Some(message_display_text(&prompt));
}
fn wizard_identify_prompt(state: &mut State) -> Option<String> {
    if state.game.player.inventory.is_empty() {
        state
            .game
            .message("you don't have anything in your pack to identify");
        None
    } else {
        state.game.pending_identification = None;
        select_prompt(state, Pending::Identify)
    }
}
fn wizard_charge_prompt(state: &mut State) -> Option<String> {
    if state.game.player.inventory.is_empty() {
        state.game.message("you aren't carrying anything");
        None
    } else {
        select_prompt(state, Pending::WizardCharge)
    }
}
fn select_action_prompt(
    state: &mut State,
    pending: Pending,
    empty_consumes_turn: bool,
) -> Option<String> {
    if state.game.player.inventory.is_empty() {
        state.game.message("you aren't carrying anything");
        if empty_consumes_turn {
            state
                .game
                .finish_action(mrzavec::command::CommandResult::TURN);
        }
        None
    } else {
        select_prompt(state, pending)
    }
}
#[cfg(test)]
fn equipment_text(game: &Game, title: &str, id: Option<u64>) -> String {
    let value = id
        .and_then(|id| game.player.inventory.iter().find(|i| i.id == id))
        .map_or("nothing".into(), |item| game.item_name(item));
    format!("{title}\n\n{value}\n\nPress Escape")
}
fn current_message(game: &Game, id: Option<u64>, how: &str, where_: Option<&str>) -> String {
    let location = where_
        .map(|where_| format!(" {where_}"))
        .unwrap_or_default();
    if let Some(item) = id.and_then(|id| game.player.inventory.iter().find(|item| item.id == id)) {
        let letter = item.pack_letter.unwrap_or('?');
        let prefix = if game.options.terse {
            String::new()
        } else {
            format!("you are {how} (")
        };
        format!(
            "{prefix}{letter}) {}{location}",
            game.inventory_name(item, true)
        )
    } else if game.options.terse {
        format!("{how} nothing{location}")
    } else {
        format!("you are {how} nothing{location}")
    }
}
#[cfg(test)]
fn rings_text(game: &Game) -> String {
    let ring = |id: Option<u64>| {
        id.and_then(|id| game.player.inventory.iter().find(|item| item.id == id))
            .map_or("nothing".into(), |item| game.item_name(item))
    };
    format!(
        "Rings\n\nleft: {}\nright: {}\n\nPress Escape",
        ring(game.player.rings[0]),
        ring(game.player.rings[1])
    )
}

fn render(
    state: Res<State>,
    cells: Query<(&Cell, &Children)>,
    mut glyphs: Query<(&mut Text, &mut TextColor), With<Glyph>>,
) {
    if !state.is_changed() {
        return;
    }
    let buffer = display(&state);
    let footer_visible = state.modal.is_none() || state.modal_overlay;
    for (cell, children) in cells {
        for child in children.iter() {
            if let Ok((mut text, mut color)) = glyphs.get_mut(child) {
                text.0 = buffer[cell.0].to_string();
                let row = cell.0 / DISPLAY_WIDTH;
                color.0 = if footer_visible
                    && matches!(row, KEYBINDING_FIRST_ROW | KEYBINDING_SECOND_ROW)
                {
                    Color::srgb(0.55, 0.55, 0.52)
                } else {
                    Color::srgb(0.82, 0.82, 0.78)
                };
            }
        }
    }
}
fn display(state: &State) -> Vec<char> {
    let mut out = vec![' '; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    if let Some(modal) = &state.modal
        && !state.modal_overlay
    {
        let all_lines: Vec<&str> = modal.lines().collect();
        let explicit_more = all_lines.last() == Some(&" --More--");
        let content_count = all_lines.len() - usize::from(explicit_more);
        let remaining = content_count.saturating_sub(state.modal_offset);
        let has_next_page = remaining > MODAL_PAGE_ROWS;
        let reserve_more = explicit_more || has_next_page;
        let visible = if reserve_more {
            MODAL_PAGE_ROWS
        } else {
            DISPLAY_HEIGHT
        };
        for (y, line) in all_lines
            .into_iter()
            .skip(state.modal_offset)
            .take(visible.min(remaining))
            .enumerate()
        {
            write_terminal_text(&mut out, y, 0, line, DISPLAY_WIDTH);
        }
        if reserve_more {
            write_terminal_text(&mut out, MODAL_MORE_ROW, 0, " --More--", DISPLAY_WIDTH);
        }
        return out;
    }
    if let Some(msg) = state
        .visible_message
        .as_ref()
        .or_else(|| state.game.messages.last())
    {
        let displayed = if state.preserve_message_case {
            msg.clone()
        } else {
            message_display_text(msg)
        };
        write_terminal_text(&mut out, 0, 0, &displayed, DISPLAY_WIDTH);
        if state.message_wait {
            let x = terminal_text_width(&displayed, 0).min(DISPLAY_WIDTH);
            write_terminal_text(&mut out, 0, x, " --More--", DISPLAY_WIDTH);
        }
    }
    for y in 1..STATUS_ROW {
        for x in 0..DISPLAY_WIDTH {
            out[y * DISPLAY_WIDTH + x] = state.game.glyph_at(Pos::new(x as i32, y as i32))
        }
    }
    let status = status_text(&state.game);
    write_terminal_text(&mut out, STATUS_ROW, 0, &status, DISPLAY_WIDTH);
    write_terminal_text(
        &mut out,
        KEYBINDING_FIRST_ROW,
        0,
        KEYBINDING_FIRST_TEXT,
        DISPLAY_WIDTH,
    );
    write_terminal_text(
        &mut out,
        KEYBINDING_SECOND_ROW,
        0,
        KEYBINDING_SECOND_TEXT,
        DISPLAY_WIDTH,
    );
    if let Some(modal) = &state.modal
        && state.modal_overlay
    {
        let lines: Vec<&str> = modal.lines().take(STATUS_ROW).collect();
        let width = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
            .min(DISPLAY_WIDTH - 2);
        let start_x = (DISPLAY_WIDTH - 1).saturating_sub(width);
        for (y, line) in lines.into_iter().enumerate() {
            for x in start_x.saturating_sub(1)..DISPLAY_WIDTH {
                out[y * DISPLAY_WIDTH + x] = ' ';
            }
            write_terminal_text(&mut out, y, start_x, line, DISPLAY_WIDTH);
        }
    }
    out
}

fn modal_has_next_page(modal: &str, offset: usize) -> bool {
    let mut lines = modal.lines();
    let count = lines.by_ref().count();
    let explicit_more = modal.lines().last() == Some(" --More--");
    count.saturating_sub(usize::from(explicit_more)) > offset + MODAL_PAGE_ROWS
}

fn write_terminal_text(out: &mut [char], row: usize, start_x: usize, text: &str, max_x: usize) {
    let mut x = start_x;
    for ch in text.chars() {
        if ch == '\t' {
            x = ((x / 8) + 1) * 8;
            continue;
        }
        if x >= max_x || row >= DISPLAY_HEIGHT {
            break;
        }
        out[row * DISPLAY_WIDTH + x] = ch;
        x += 1;
    }
}

fn terminal_text_width(text: &str, start_x: usize) -> usize {
    text.chars().fold(
        start_x,
        |x, ch| {
            if ch == '\t' { ((x / 8) + 1) * 8 } else { x + 1 }
        },
    )
}

fn prepare_messages(mut state: ResMut<State>) {
    collect_messages(&mut state);
}

fn collect_messages(state: &mut State) {
    if state.game.end != mrzavec::game::EndState::Playing && state.modal.is_some() {
        state.message_serial_seen = state.game.message_serial;
        state.message_queue.clear();
        state.visible_message = None;
        state.message_wait = false;
        state.deferred_modal = None;
        return;
    }
    if state.game.message_serial < state.message_serial_seen {
        state.message_serial_seen = state.game.message_serial;
    }
    let added = state
        .game
        .message_serial
        .saturating_sub(state.message_serial_seen) as usize;
    if added == 0 {
        return;
    }
    let start = state.game.messages.len().saturating_sub(added);
    state
        .message_queue
        .extend(state.game.messages[start..].iter().cloned());
    state.message_serial_seen = state.game.message_serial;
    if !state.message_wait {
        begin_message_sequence(state);
    }
}

fn begin_message_sequence(state: &mut State) {
    let Some(message) = state.message_queue.pop_front() else {
        return;
    };
    state.visible_message = Some(message);
    if let Some(modal) = state.modal.take() {
        state.deferred_modal = Some((modal, state.modal_overlay, state.modal_offset));
        state.modal_overlay = false;
        state.modal_offset = 0;
    }
    state.message_wait = !state.message_queue.is_empty() || state.deferred_modal.is_some();
}

fn advance_message(state: &mut State) {
    if let Some(message) = state.message_queue.pop_front() {
        state.visible_message = Some(message);
        state.message_wait = !state.message_queue.is_empty() || state.deferred_modal.is_some();
        return;
    }
    state.message_wait = false;
    state.visible_message = None;
    if let Some((modal, overlay, offset)) = state.deferred_modal.take() {
        state.modal = Some(modal);
        state.modal_overlay = overlay;
        state.modal_offset = offset;
    }
}

fn status_text(game: &Game) -> String {
    let hunger = ["", "Hungry", "Weak", "Faint"]
        .get(game.hungry_state as usize)
        .copied()
        .unwrap_or("");
    let hp_width = game.player.stats.max_hp.to_string().len();
    format!(
        "Level: {}  Gold: {:<5}  Hp: {:>width$}({:>width$})  Str: {:>2}({})  Arm: {:<2}  Exp: {}/{}  {}",
        game.depth,
        game.player.gold,
        game.player.stats.hp,
        game.player.stats.max_hp,
        game.player.stats.strength,
        game.player.max_strength,
        10 - game.player.armor_class(),
        game.player.stats.level,
        game.player.stats.experience,
        hunger,
        width = hp_width,
    )
}
fn inventory_modal(state: &mut State) -> Option<String> {
    let pack_len = state
        .game
        .player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .count();
    if pack_len == 0 {
        state.game.message(if state.game.options.terse {
            "empty handed"
        } else {
            "you are empty handed"
        });
        return None;
    }
    if pack_len == 1 {
        state.game.message(inventory_line(&state.game, 0));
        return None;
    }
    match state.game.options.inventory_style {
        mrzavec::game::InventoryStyle::Slow => {
            state.pending = Some(Pending::SlowInventory(0));
            Some(slow_inventory_line(&state.game, 0))
        }
        mrzavec::game::InventoryStyle::Overwrite => {
            let text = inventory_text(&state.game);
            state.modal_overlay = text.lines().count() <= 23;
            state.pending = Some(Pending::More);
            Some(text)
        }
        mrzavec::game::InventoryStyle::Clear => {
            state.pending = Some(Pending::More);
            Some(inventory_text(&state.game))
        }
    }
}

fn inventory_line(game: &Game, index: usize) -> String {
    game.player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .nth(index)
        .map_or_else(
            || "Your pack is empty.".into(),
            |item| {
                format!(
                    "{}) {}",
                    item.pack_letter.unwrap_or((b'a' + index as u8) as char),
                    game.inventory_name(item, false),
                )
            },
        )
}

fn slow_inventory_line(game: &Game, index: usize) -> String {
    format!("{}  --More--", inventory_line(game, index))
}

fn picky_inventory_prompt(state: &mut State) -> Option<String> {
    let pack: Vec<_> = state
        .game
        .player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .collect();
    if pack.is_empty() {
        state.game.message("you aren't carrying anything");
        return None;
    }
    if pack.len() == 1 {
        let item = pack[0];
        let description = state.game.inventory_name(item, false);
        let letter = item.pack_letter.unwrap_or('?');
        state.game.message(format!("{letter}) {description}"));
        return None;
    }
    state.pending = Some(Pending::PickyInventory);
    let prompt = if state.game.options.terse {
        "item: "
    } else {
        "which item do you wish to inventory: "
    };
    state.game.remember_message(prompt);
    Some(message_display_text(prompt))
}
fn inventory_text(game: &Game) -> String {
    let mut text = String::new();
    for index in 0..game
        .player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .count()
    {
        text.push_str(&inventory_line(game, index));
        text.push('\n');
    }
    text.push_str(" --More--");
    text
}
fn call_default(game: &Game, item: &mrzavec::item::Item) -> String {
    if let Some(existing) = game.item_guess(item).or(item.label.as_deref()) {
        return existing.into();
    }
    match item.kind {
        ItemKind::Potion => game.appearances.potion_colors[item.which as usize].clone(),
        ItemKind::Scroll => game.appearances.scroll_titles[item.which as usize].clone(),
        ItemKind::Ring => game.appearances.ring_stones[item.which as usize].clone(),
        ItemKind::Stick => game.appearances.stick_materials[item.which as usize].clone(),
        _ => String::new(),
    }
}
fn discovery_lines(game: &mut Game, kind: Option<char>) -> Vec<String> {
    let mut lines = Vec::new();
    let categories = [
        ('!', ItemKind::Potion, POTION_NAMES.len(), "potion"),
        ('?', ItemKind::Scroll, SCROLL_NAMES.len(), "scroll"),
        ('=', ItemKind::Ring, RING_NAMES.len(), "ring"),
        ('/', ItemKind::Stick, STICK_NAMES.len(), "stick"),
    ];
    let mut first = true;
    for (glyph, item_kind, count, singular) in categories {
        if !kind.is_none_or(|requested| requested == glyph || requested == '*') {
            continue;
        }
        if !first {
            lines.push(String::new());
        }
        first = false;
        let mut order: Vec<usize> = (0..count).collect();
        for remaining in (1..=count).rev() {
            let selected = game.rng.rnd(remaining as u32) as usize;
            order.swap(remaining - 1, selected);
        }
        let mut found = false;
        for which in order {
            let item = mrzavec::item::Item::basic(0, item_kind, which as u8);
            let known = match item_kind {
                ItemKind::Potion => game.knowledge.potions[which],
                ItemKind::Scroll => game.knowledge.scrolls[which],
                ItemKind::Ring => game.knowledge.rings[which],
                ItemKind::Stick => game.knowledge.sticks[which],
                _ => unreachable!(),
            };
            if known || game.item_guess(&item).is_some() {
                lines.push(game.inventory_name(&item, false));
                found = true;
            }
        }
        if !found {
            lines.push(if game.options.terse {
                format!("Nothing about any {singular}s")
            } else {
                format!("Haven't discovered anything about any {singular}s")
            });
        }
    }
    lines
}

#[cfg(test)]
fn discoveries_text(game: &mut Game, kind: Option<char>) -> String {
    let mut out = discovery_lines(game, kind).join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(" --More--");
    out
}

fn start_discoveries(state: &mut State, kind: char) {
    let lines = discovery_lines(&mut state.game, Some(kind));
    match state.game.options.inventory_style {
        mrzavec::game::InventoryStyle::Slow => {
            state.slow_discovery_lines =
                lines.into_iter().filter(|line| !line.is_empty()).collect();
            state.pending = Some(Pending::SlowDiscoveryPrompt);
            let prompt = discoveries_prompt(&state.game);
            state.modal = Some(format!("{}  --More--", message_display_text(&prompt)));
        }
        mrzavec::game::InventoryStyle::Overwrite | mrzavec::game::InventoryStyle::Clear
            if lines.len() == 1 =>
        {
            state.game.remember_message(&lines[0]);
            state.game.message_without_recall("");
            state.pending = None;
            state.modal = None;
        }
        mrzavec::game::InventoryStyle::Overwrite | mrzavec::game::InventoryStyle::Clear => {
            let mut text = lines.join("\n");
            text.push_str("\n --More--");
            state.modal_overlay = state.game.options.inventory_style
                == mrzavec::game::InventoryStyle::Overwrite
                && lines.len() <= 23;
            state.modal_offset = 0;
            state.pending = Some(Pending::DiscoveryMore);
            state.modal = Some(text);
        }
    }
}
fn discoveries_prompt(game: &Game) -> String {
    if game.options.terse {
        "what type? (* for all)".into()
    } else {
        "for what type of object do you want a list? (* for all)".into()
    }
}
const HELP_ENTRIES: &[(char, &str, bool)] = &[
    ('?', "\tprints help", true),
    ('/', "\tidentify object", true),
    ('h', "\tleft", true),
    ('j', "\tdown", true),
    ('k', "\tup", true),
    ('l', "\tright", true),
    ('y', "\tup & left", true),
    ('u', "\tup & right", true),
    ('b', "\tdown & left", true),
    ('n', "\tdown & right", true),
    ('H', "\trun left", false),
    ('J', "\trun down", false),
    ('K', "\trun up", false),
    ('L', "\trun right", false),
    ('Y', "\trun up & left", false),
    ('U', "\trun up & right", false),
    ('B', "\trun down & left", false),
    ('N', "\trun down & right", false),
    ('\u{8}', "\trun left until adjacent", false),
    ('\u{a}', "\trun down until adjacent", false),
    ('\u{b}', "\trun up until adjacent", false),
    ('\u{c}', "\trun right until adjacent", false),
    ('\u{19}', "\trun up & left until adjacent", false),
    ('\u{15}', "\trun up & right until adjacent", false),
    ('\u{2}', "\trun down & left until adjacent", false),
    ('\u{e}', "\trun down & right until adjacent", false),
    ('\0', "\t<SHIFT><dir>: run that way", true),
    ('\0', "\t<CTRL><dir>: run till adjacent", true),
    ('f', "<dir>\tfight till death or near death", true),
    ('t', "<dir>\tthrow something", true),
    ('m', "<dir>\tmove onto without picking up", true),
    ('z', "<dir>\tzap a wand in a direction", true),
    ('^', "<dir>\tidentify trap type", true),
    ('s', "\tsearch for trap/secret door", true),
    ('>', "\tgo down a staircase", true),
    ('<', "\tgo up a staircase", true),
    ('.', "\trest for a turn", true),
    (',', "\tpick something up", true),
    ('i', "\tinventory", true),
    ('I', "\tinventory single item", true),
    ('q', "\tquaff potion", true),
    ('r', "\tread scroll", true),
    ('e', "\teat food", true),
    ('w', "\twield a weapon", true),
    ('W', "\twear armor", true),
    ('T', "\ttake armor off", true),
    ('P', "\tput on ring", true),
    ('R', "\tremove ring", true),
    ('d', "\tdrop object", true),
    ('c', "\tcall object", true),
    ('a', "\trepeat last command", true),
    (')', "\tprint current weapon", true),
    (']', "\tprint current armor", true),
    ('=', "\tprint current rings", true),
    ('@', "\tprint current stats", true),
    ('D', "\trecall what's been discovered", true),
    ('o', "\texamine/set options", true),
    ('\u{12}', "\tredraw screen", true),
    ('\u{10}', "\trepeat last message", true),
    ('\u{1b}', "\tcancel command, ^[ is the escape key", true),
    ('S', "\tsave game", true),
    ('Q', "\tquit", true),
    ('!', "\tshell escape", true),
    ('F', "<dir>\tfight till either of you dies", true),
    ('v', "\tprint version, release, dungeon number", true),
];

fn help_text() -> String {
    let entries: Vec<_> = HELP_ENTRIES.iter().filter(|(_, _, print)| *print).collect();
    let rows_count = entries.len().div_ceil(2).min(23);
    let mut rows = vec![vec![' '; DISPLAY_WIDTH]; rows_count];
    for (index, (ch, description, _)) in entries.into_iter().take(rows_count * 2).enumerate() {
        let x = if index >= rows_count { 40 } else { 0 };
        let y = index % rows_count;
        let label = if *ch == '\0' {
            String::new()
        } else {
            control_label(*ch)
        };
        let mut cursor = x;
        for value in format!("{label}{description}").chars() {
            if value == '\t' {
                cursor = ((cursor / 8) + 1) * 8;
            } else {
                if cursor < DISPLAY_WIDTH {
                    rows[y][cursor] = value;
                }
                cursor += 1;
            }
        }
    }
    let mut text = rows
        .into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    text.push_str("\n --More--");
    text
}

fn help_for(ch: char) -> String {
    HELP_ENTRIES
        .iter()
        .find(|(key, _, _)| *key == ch)
        .map_or_else(
            || format!("unknown character '{}'", control_label(ch)),
            |(_, description, _)| format!("{}{description}", control_label(ch)),
        )
}

fn control_label(ch: char) -> String {
    match ch {
        '\u{1b}' => "^[".into(),
        ch if ch <= '\u{1f}' => format!("^{}", (b'@' + ch as u8) as char),
        '\u{7f}' => "^?".into(),
        ch => ch.to_string(),
    }
}

fn identify_glyph_text(ch: char) -> String {
    let description = if ch.is_ascii_uppercase() {
        mrzavec::monster::MONSTERS[(ch as u8 - b'A') as usize].name
    } else {
        match ch {
            '|' | '-' => "wall of a room",
            '*' => "gold",
            '%' => "a staircase",
            '+' => "door",
            '.' => "room floor",
            '@' => "you",
            '#' => "passage",
            '^' => "trap",
            '!' => "potion",
            '?' => "scroll",
            ':' => "food",
            ')' => "weapon",
            ' ' => "solid rock",
            ']' => "armor",
            ',' => "the Amulet of Yendor",
            '=' => "ring",
            '/' => "wand or staff",
            _ => "unknown character",
        }
    };
    format!("'{}': {description}", control_label(ch))
}
const OPTION_COUNT: usize = 12;
const OPTION_LABELS: [(&str, &str); OPTION_COUNT] = [
    ("Terse output", "terse"),
    ("Flush typeahead during battle", "flush"),
    ("Show position only at end of run", "jump"),
    ("Show the lamp-illuminated floor", "seefloor"),
    ("Follow turnings in passageways", "passgo"),
    ("Print out tombstone when killed", "tombstone"),
    ("Inventory style", "inven"),
    ("Name", "name"),
    ("Fruit", "fruit"),
    ("Save file", "file"),
    ("Score file", "score"),
    ("Lock file", "lock"),
];
fn option_value(game: &Game, index: usize) -> String {
    match index {
        0 => if game.options.terse { "True" } else { "False" }.into(),
        1 => if game.options.fight_flush {
            "True"
        } else {
            "False"
        }
        .into(),
        2 => if game.options.jump { "True" } else { "False" }.into(),
        3 => if game.options.see_floor {
            "True"
        } else {
            "False"
        }
        .into(),
        4 => if game.options.passgo { "True" } else { "False" }.into(),
        5 => if game.options.tombstone {
            "True"
        } else {
            "False"
        }
        .into(),
        6 => match game.options.inventory_style {
            mrzavec::game::InventoryStyle::Overwrite => "Overwrite",
            mrzavec::game::InventoryStyle::Slow => "Slow",
            mrzavec::game::InventoryStyle::Clear => "Clear",
        }
        .into(),
        7 => game.options.name.clone(),
        8 => game.options.fruit.clone(),
        9 => game.options.save_file.clone(),
        10 => game.options.score_file.clone(),
        11 => game.options.lock_file.clone(),
        _ => unreachable!("option index is bounded by OPTION_COUNT"),
    }
}
fn options_text(
    game: &Game,
    active: Option<usize>,
    input: Option<&str>,
    error: Option<&str>,
) -> String {
    let mut out = String::new();
    for (index, (prompt, name)) in OPTION_LABELS.into_iter().enumerate() {
        let value = if active == Some(index)
            && let Some(input) = input
        {
            input.into()
        } else {
            option_value(game, index)
        };
        out.push_str(&format!("{prompt} (\"{name}\"): {value}"));
        if active == Some(index)
            && let Some(error) = error
        {
            out.push_str(&format!("  {error}"));
        }
        out.push('\n');
    }
    out
}
fn show_option(state: &mut State, index: usize) {
    state.input_buffer.clear();
    state.pending = Some(Pending::Options(index));
    state.modal = Some(options_text(&state.game, Some(index), None, None));
}
fn advance_option(state: &mut State, index: usize) {
    if index + 1 < OPTION_COUNT {
        show_option(state, index + 1);
    } else {
        finish_options(state);
    }
}
fn finish_options(state: &mut State) {
    state.input_buffer.clear();
    state.pending = Some(Pending::More);
    state.modal = Some(format!(
        "{} --More--",
        options_text(&state.game, None, None, None)
    ));
}
fn set_boolean_option(game: &mut Game, index: usize, value: bool) {
    match index {
        0 => game.options.terse = value,
        1 => game.options.fight_flush = value,
        2 => game.options.jump = value,
        3 => game.options.see_floor = value,
        4 => game.options.passgo = value,
        5 => game.options.tombstone = value,
        _ => unreachable!("only boolean options accept true or false"),
    }
}
fn option_home_directory() -> String {
    #[cfg(target_arch = "wasm32")]
    return String::new();
    #[cfg(not(target_arch = "wasm32"))]
    std::env::var("HOME")
        .unwrap_or_else(|_| ".".into())
        .chars()
        .take(50)
        .collect()
}
fn set_string_option(game: &mut Game, index: usize, input: &str) {
    if input.is_empty() {
        return;
    }
    let input = mrzavec::game::normalize_option_string(input);
    match index {
        7 => game.options.name = input,
        8 => game.options.fruit = input,
        9 => game.options.save_file = input,
        10 => game.options.score_file = input,
        11 => game.options.lock_file = input,
        _ => unreachable!("only string options use text entry"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display_row(buffer: &[char], row: usize) -> String {
        buffer[row * DISPLAY_WIDTH..(row + 1) * DISPLAY_WIDTH]
            .iter()
            .collect()
    }

    fn state(seed: u64) -> State {
        let game = Game::new(seed);
        let message_serial_seen = game.message_serial;
        State {
            game,
            modal: None,
            modal_overlay: false,
            modal_offset: 0,
            item_inventory_open: false,
            preserve_message_case: false,
            slow_discovery_lines: Vec::new(),
            message_serial_seen,
            message_queue: VecDeque::new(),
            visible_message: None,
            message_wait: false,
            deferred_modal: None,
            pending: None,
            score_recorded: false,
            input_buffer: String::new(),
            count_prefix: String::new(),
            counted_command: None,
        }
    }

    #[test]
    fn options_screen_uses_the_reference_order_labels_and_values() {
        let mut game = Game::new(100);
        game.options.terse = true;
        game.options.inventory_style = mrzavec::game::InventoryStyle::Slow;
        game.options.name = "Rodney".into();
        game.options.fruit = "mango".into();

        let text = options_text(&game, Some(0), None, None);
        let lines: Vec<_> = text.lines().collect();

        assert_eq!(lines.len(), OPTION_COUNT);
        assert_eq!(lines[0], "Terse output (\"terse\"): True");
        assert_eq!(lines[1], "Flush typeahead during battle (\"flush\"): False");
        assert_eq!(
            lines[2],
            "Show position only at end of run (\"jump\"): False"
        );
        assert_eq!(
            lines[3],
            "Show the lamp-illuminated floor (\"seefloor\"): True"
        );
        assert_eq!(
            lines[4],
            "Follow turnings in passageways (\"passgo\"): False"
        );
        assert_eq!(
            lines[5],
            "Print out tombstone when killed (\"tombstone\"): True"
        );
        assert_eq!(lines[6], "Inventory style (\"inven\"): Slow");
        assert_eq!(lines[7], "Name (\"name\"): Rodney");
        assert_eq!(lines[8], "Fruit (\"fruit\"): mango");
        assert!(lines[9].starts_with("Save file (\"file\"): "));
        assert!(lines[10].starts_with("Score file (\"score\"): "));
        assert!(lines[11].starts_with("Lock file (\"lock\"): "));
    }

    #[test]
    fn options_editor_advances_backtracks_and_waits_for_space_at_the_end() {
        let mut state = state(101);
        show_option(&mut state, 0);
        assert_eq!(state.pending, Some(Pending::Options(0)));

        set_boolean_option(&mut state.game, 0, true);
        advance_option(&mut state, 0);
        assert!(state.game.options.terse);
        assert_eq!(state.pending, Some(Pending::Options(1)));

        show_option(&mut state, 0);
        assert_eq!(state.pending, Some(Pending::Options(0)));

        set_string_option(&mut state.game, 7, "Alice");
        assert_eq!(state.game.options.name, "Alice");
        set_string_option(&mut state.game, 7, "");
        assert_eq!(state.game.options.name, "Alice");

        advance_option(&mut state, OPTION_COUNT - 1);
        assert_eq!(state.pending, Some(Pending::More));
        assert!(state.modal.as_deref().unwrap().ends_with(" --More--"));
    }

    #[test]
    fn options_string_entry_can_display_an_empty_replacement_buffer() {
        let mut game = Game::new(102);
        game.options.name = "Old Name".into();

        let initial = options_text(&game, Some(7), None, None);
        let erased = options_text(&game, Some(7), Some(""), None);

        assert!(initial.lines().nth(7).unwrap().ends_with("Old Name"));
        assert_eq!(erased.lines().nth(7).unwrap(), "Name (\"name\"): ");
    }

    #[test]
    fn inventory_styles_select_distinct_presentation_paths() {
        let mut state = state(1);
        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Overwrite;
        assert!(inventory_modal(&mut state).is_some());
        assert!(state.modal_overlay);

        state.modal_overlay = false;
        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Clear;
        assert!(inventory_modal(&mut state).is_some());
        assert!(!state.modal_overlay);

        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Slow;
        assert!(inventory_modal(&mut state).is_some());
        assert_eq!(state.pending, Some(Pending::SlowInventory(0)));
    }

    #[test]
    fn startup_parser_matches_reference_short_option_modes() {
        assert_eq!(parse_startup(["-V".into()]), Ok(Startup::Version));
        assert_eq!(parse_startup(["-h".into()]), Ok(Startup::Help));
        assert_eq!(parse_startup(["-d".into()]), Ok(Startup::Die));
        assert_eq!(parse_startup(["-s".into()]), Ok(Startup::Scores(None)));
        assert_eq!(
            parse_startup(["-sother.scores".into()]),
            Ok(Startup::Scores(Some("other.scores".into())))
        );
        assert_eq!(
            parse_startup(["-s".into(), "other.scores".into()]),
            Ok(Startup::Scores(Some("other.scores".into())))
        );
        assert_eq!(parse_startup(["-x".into()]), Err('x'));
    }

    #[test]
    fn startup_parser_preserves_nonunicode_restore_paths_and_legacy_flags() {
        let restore = OsString::from("saved-game");
        assert_eq!(
            parse_startup(["-Sr".into(), restore.clone()]),
            Ok(Startup::Play {
                restore: Some(restore),
                signal_quit: true,
                wizard_prompt: false,
            })
        );
        assert_eq!(
            parse_startup(["--".into(), "-odd-save-name".into()]),
            Ok(Startup::Play {
                restore: Some("-odd-save-name".into()),
                signal_quit: false,
                wizard_prompt: false,
            })
        );
        assert_eq!(
            parse_startup(["".into(), "-r".into()]),
            Ok(Startup::Play {
                restore: None,
                signal_quit: false,
                wizard_prompt: true,
            })
        );
        assert_eq!(parse_c_number("42"), Some(42));
        assert_eq!(parse_c_number("052"), Some(42));
        assert_eq!(parse_c_number("0x2a"), Some(42));
        assert_eq!(parse_c_number("-1"), Some(u32::MAX));
        assert_eq!(parse_c_integer(" -42 trailing"), -42);
        assert_eq!(parse_c_integer("+17"), 17);
        assert_eq!(parse_c_integer("nonsense"), 0);
    }

    #[test]
    fn terminating_signal_autosaves_without_finishing_the_game() {
        let path = std::env::temp_dir().join(format!(
            "mrzavec-signal-save-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut game = Game::new(420);
        game.options.save_file = path.to_string_lossy().into_owned();
        game.turn = 1234;

        apply_termination_signal(&mut game, false).unwrap();

        assert_eq!(game.end, mrzavec::game::EndState::Playing);
        assert_eq!(save::load(&path).unwrap().turn, 1234);
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o600);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn signal_quit_mode_is_an_untaxed_signal_death() {
        let mut game = Game::new(421);
        game.player.gold = 101;

        apply_termination_signal(&mut game, true).unwrap();

        assert_eq!(game.end, mrzavec::game::EndState::Dead);
        assert_eq!(game.death_cause.as_deref(), Some("signal"));
        assert_eq!(score::amount(&game), 101);
    }

    #[test]
    fn version_message_matches_reference_and_reports_the_dungeon_number() {
        let game = Game::new(4_294_967_299);
        assert_eq!(
            version_message(&game),
            "rogue version 5.4.5 release 2026-07-17 dungeon 3 (chongo was here)"
        );
    }

    #[test]
    fn wizard_password_is_the_reference_case_sensitive_value() {
        assert!(wizard_password_matches("bathtub"));
        assert!(!wizard_password_matches("Bathtub"));
        assert!(!wizard_password_matches("BATHTUB"));
    }

    #[test]
    fn inventory_views_use_reference_quantity_descriptions_without_x_counts() {
        let mut game = Game::new(109);
        let arrows = game
            .player
            .inventory
            .iter_mut()
            .find(|item| item.kind == ItemKind::Weapon && item.which == 3)
            .unwrap();
        arrows.count = 7;
        let text = inventory_text(&game);
        assert!(text.contains("7 +0,+0 arrows"));
        assert!(!text.contains("x7"));
        assert!(!text.contains(") ) "));
    }

    #[test]
    fn empty_and_picky_inventory_follow_reference_prompt_branches() {
        let mut empty = state(110);
        empty.game.player.inventory.clear();
        assert!(inventory_modal(&mut empty).is_none());
        assert_eq!(
            empty.game.messages.last().map(String::as_str),
            Some("you are empty handed")
        );
        assert!(picky_inventory_prompt(&mut empty).is_none());
        assert_eq!(
            empty.game.messages.last().map(String::as_str),
            Some("you aren't carrying anything")
        );

        let mut single = state(111);
        single.game.player.inventory.truncate(1);
        assert!(picky_inventory_prompt(&mut single).is_none());
        assert!(single.game.messages.last().unwrap().starts_with("a) "));

        let mut multiple = state(112);
        multiple.game.options.terse = true;
        assert_eq!(
            picky_inventory_prompt(&mut multiple).as_deref(),
            Some("Item: ")
        );
        assert_eq!(multiple.pending, Some(Pending::PickyInventory));
    }

    #[test]
    fn wizard_ground_inventory_uses_inventory_names_without_coordinates() {
        let mut state = state(113);
        state.game.floor_items.clear();
        state
            .game
            .floor_items
            .push(mrzavec::item::Item::basic(999_113, ItemKind::Potion, 0));
        let text = ground_inventory_modal(&mut state).unwrap();
        assert!(!text.contains(" at "));
        assert!(text.contains(&state.game.inventory_name(&state.game.floor_items[0], false)));

        state.game.floor_items.clear();
        assert!(ground_inventory_modal(&mut state).is_none());
        assert_eq!(
            state.game.messages.last().map(String::as_str),
            Some("you are empty handed")
        );
    }

    #[test]
    fn clear_screen_modal_pages_use_all_twenty_five_content_rows() {
        let mut state = state(103);
        state.modal = Some(
            (0..30)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let first = display(&state);
        let first_text = display_row(&first, MODAL_MORE_ROW);
        assert!(first_text.starts_with(" --More--"));

        state.modal_offset = MODAL_PAGE_ROWS;
        let second = display(&state);
        let second_text = display_row(&second, 0);
        assert!(second_text.starts_with("line 25"));
    }

    #[test]
    fn normal_display_preserves_game_rows_and_adds_keybinding_footer() {
        let mut state = state(104);
        state.visible_message = Some("hello".into());
        let player = state.game.player.pos;
        let expected_player_glyph = state.game.glyph_at(player);

        let buffer = display(&state);

        assert_eq!(buffer.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
        assert!(display_row(&buffer, 0).starts_with("Hello"));
        assert_eq!(
            buffer[player.y as usize * DISPLAY_WIDTH + player.x as usize],
            expected_player_glyph
        );
        let expected_status = status_text(&state.game);
        assert_eq!(
            display_row(&buffer, STATUS_ROW).trim_end(),
            expected_status.trim_end()
        );
        assert_eq!(
            display_row(&buffer, KEYBINDING_FIRST_ROW).trim_end(),
            KEYBINDING_FIRST_TEXT
        );
        assert_eq!(
            display_row(&buffer, KEYBINDING_SECOND_ROW).trim_end(),
            KEYBINDING_SECOND_TEXT
        );
        assert!(
            display_row(&buffer, KEYBINDING_SECOND_ROW)
                .trim_end()
                .ends_with("Help ?")
        );
    }

    #[test]
    fn question_mark_key_opens_the_help_prompt() {
        let mut app = App::new();
        app.insert_resource(state(105));
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Time::<()>::default());
        app.insert_resource(MovementRepeat::default());
        app.add_message::<AppExit>();
        app.add_systems(Update, keyboard);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::Slash);
        }

        app.update();

        let state = app.world().resource::<State>();
        assert_eq!(state.pending, Some(Pending::Help));
        assert_eq!(
            state.modal.as_deref(),
            Some("Character you want help for (* for all): ")
        );
    }

    #[test]
    fn held_movement_waits_then_repeats_at_a_steady_interval() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut repeat = MovementRepeat::default();
        keys.press(KeyCode::KeyH);

        assert_eq!(repeat.update(&keys, Duration::ZERO, true), None);
        keys.clear();
        assert_eq!(
            repeat.update(
                &keys,
                MOVEMENT_REPEAT_DELAY - Duration::from_millis(1),
                true,
            ),
            None
        );
        assert_eq!(
            repeat.update(&keys, Duration::from_millis(2), true),
            Some('h')
        );
        assert_eq!(
            repeat.update(
                &keys,
                MOVEMENT_REPEAT_INTERVAL - Duration::from_millis(2),
                true,
            ),
            None
        );
        assert_eq!(
            repeat.update(&keys, Duration::from_millis(1), true),
            Some('h')
        );

        keys.release(KeyCode::KeyH);
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_INTERVAL, true), None);
        assert_eq!(repeat.key, None);
    }

    #[test]
    fn held_movement_restarts_its_delay_after_input_is_blocked() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut repeat = MovementRepeat::default();
        keys.press(KeyCode::KeyL);
        assert_eq!(repeat.update(&keys, Duration::ZERO, true), None);
        keys.clear();
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_DELAY, true), Some('l'));

        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_INTERVAL, false), None);
        assert_eq!(repeat.key, None);
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_INTERVAL, true), None);
        assert_eq!(
            repeat.update(
                &keys,
                MOVEMENT_REPEAT_DELAY - Duration::from_millis(1),
                true,
            ),
            None
        );
        assert_eq!(
            repeat.update(&keys, Duration::from_millis(1), true),
            Some('l')
        );
    }

    #[test]
    fn keyboard_repeats_held_movement_and_resets_for_modifiers() {
        let mut initial_state = state(106);
        initial_state.game.monsters.clear();
        let starting_turn = initial_state.game.turn;
        let movement_key = MOVEMENT_KEYS
            .iter()
            .find_map(|(key, ch)| {
                let mut probe = initial_state.game.clone();
                for _ in 0..4 {
                    probe.execute(parse(*ch));
                }
                (probe.turn == starting_turn + 4 && probe.end == mrzavec::game::EndState::Playing)
                    .then_some(*key)
            })
            .expect("generated level has a direction with four valid movement steps");
        let mut app = App::new();
        app.insert_resource(initial_state);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Time::<()>::default());
        app.insert_resource(MovementRepeat::default());
        app.add_message::<AppExit>();
        app.add_systems(Update, keyboard);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(movement_key);

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_DELAY - Duration::from_millis(1));

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 1);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1));

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 2);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_INTERVAL);

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 3);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::AltLeft);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_INTERVAL);

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 3);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::AltLeft);
            keys.clear();
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_INTERVAL);

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 3);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_DELAY);

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 4);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(movement_key);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));

        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 4);
    }

    #[test]
    fn tombstone_contains_reference_fields_and_current_year() {
        let mut game = Game::new(99);
        game.options.name = "Rodney".into();
        game.player.gold = 100;
        game.end = mrzavec::game::EndState::Dead;
        game.death_cause = Some("a dragon".into());
        let text = tombstone_text(&game);
        assert!(text.contains("REST"));
        assert!(text.contains("Rodney"));
        assert!(text.contains("90 Au"));
        assert!(text.contains("killed by a"));
        assert!(text.contains("dragon"));
        assert!(!text.contains("a dragon"));
        assert!(text.contains(&current_year().to_string()));
    }

    #[test]
    fn tombstone_places_articles_in_the_reference_heading() {
        let mut game = Game::new(104);
        game.end = mrzavec::game::EndState::Dead;

        game.death_cause = Some("an aquator".into());
        let vowel = tombstone_text(&game);
        assert!(vowel.contains("killed by an"));
        assert!(!vowel.contains("an aquator"));
        assert!(vowel.contains("aquator"));

        game.death_cause = Some("starvation".into());
        let articleless = tombstone_text(&game);
        assert!(!articleless.contains("killed by a "));
        assert!(articleless.contains("starvation"));

        game.death_cause = Some("signal".into());
        let signal = tombstone_text(&game);
        assert!(signal.contains("killed by a"));
        assert!(signal.contains("signal"));
        assert_eq!(death_cause_with_article(&game), "a signal");
    }

    #[test]
    fn tombstone_preserves_long_names_instead_of_clipping_them_to_the_art() {
        let mut game = mrzavec::Game::new(2026);
        game.end = mrzavec::game::EndState::Dead;
        game.options.name = "abcdefghijklmnopqrstuvwxyz".into();
        game.death_cause = Some("a venus flytrap".into());

        let text = tombstone_text(&game);

        assert!(text.contains("abcdefghijklmnopqrstuvwxyz"));
        assert!(text.contains("venus flytrap"));
    }

    #[test]
    fn status_line_uses_reference_stats_and_hunger_format() {
        let mut game = Game::new(101);
        game.depth = 7;
        game.player.gold = 42;
        game.player.stats.hp = 9;
        game.player.stats.max_hp = 12;
        game.player.stats.strength = 14;
        game.player.max_strength = 16;
        game.hungry_state = 2;
        let status = status_text(&game);
        assert!(status.contains("Level: 7  Gold: 42   "));
        assert!(status.contains("Hp:  9(12)"));
        assert!(status.contains("Str: 14(16)"));
        assert!(status.ends_with("Weak"));
    }

    #[test]
    fn winner_sale_screen_lists_item_worth_and_gold() {
        let mut game = Game::new(100);
        game.player.gold = 321;
        let text = winner_sales_text(&game);
        assert!(text.starts_with("   Worth  Item"));
        assert!(text.contains("321  Gold Pieces"));
        assert!(text.contains("a)"));
    }

    #[test]
    fn winner_sale_screen_uses_identified_inventory_counts() {
        let mut game = Game::new(105);
        let arrows = game
            .player
            .inventory
            .iter_mut()
            .find(|item| item.kind == ItemKind::Weapon && item.which == 3)
            .unwrap();
        arrows.count = 7;
        let text = winner_sales_text(&game);
        assert!(text.contains("7 +0,+0 arrows"));
        assert!(!text.contains("x7"));
    }

    #[test]
    fn discoveries_can_be_filtered_by_original_object_glyph() {
        let mut game = Game::new(2);
        game.knowledge.potions[0] = true;
        game.knowledge.scrolls[0] = true;
        let potions = discoveries_text(&mut game, Some('!'));
        assert!(potions.contains("potion of"));
        assert!(!potions.contains("scroll of"));
    }

    #[test]
    fn discovery_prompt_has_reference_verbose_and_terse_forms() {
        let mut game = Game::new(108);
        assert_eq!(
            discoveries_prompt(&game),
            "for what type of object do you want a list? (* for all)"
        );
        game.options.terse = true;
        assert_eq!(discoveries_prompt(&game), "what type? (* for all)");
    }

    #[test]
    fn discoveries_include_guesses_and_shuffle_with_the_game_rng() {
        let mut game = Game::new(20);
        game.knowledge.guesses[0] = Some("fizzy".into());
        let mut expected_rng = game.rng;
        for remaining in (1..=POTION_NAMES.len()).rev() {
            let _ = expected_rng.rnd(remaining as u32);
        }

        let text = discoveries_text(&mut game, Some('!'));

        assert!(text.contains("called fizzy"));
        assert_eq!(game.rng, expected_rng);
    }

    #[test]
    fn discovery_output_uses_the_reference_inventory_style_branches() {
        let mut single = state(200);
        single.game.options.inventory_style = mrzavec::game::InventoryStyle::Clear;
        start_discoveries(&mut single, '!');
        assert!(single.modal.is_none());
        assert!(single.pending.is_none());
        assert_eq!(single.game.messages.last().map(String::as_str), Some(""));
        assert_eq!(
            single.game.recall_message,
            "Haven't discovered anything about any potions"
        );

        let mut clear = state(201);
        clear.game.options.inventory_style = mrzavec::game::InventoryStyle::Clear;
        clear.game.knowledge.potions[0] = true;
        clear.game.knowledge.potions[1] = true;
        start_discoveries(&mut clear, '!');
        assert_eq!(clear.pending, Some(Pending::DiscoveryMore));
        assert!(!clear.modal_overlay);
        assert!(clear.modal.as_deref().unwrap().ends_with(" --More--"));

        let mut overwrite = state(202);
        overwrite.game.options.inventory_style = mrzavec::game::InventoryStyle::Overwrite;
        start_discoveries(&mut overwrite, '*');
        assert_eq!(overwrite.pending, Some(Pending::DiscoveryMore));
        assert!(overwrite.modal_overlay);

        let mut slow = state(203);
        slow.game.options.inventory_style = mrzavec::game::InventoryStyle::Slow;
        start_discoveries(&mut slow, '*');
        assert_eq!(slow.pending, Some(Pending::SlowDiscoveryPrompt));
        assert_eq!(slow.slow_discovery_lines.len(), 4);
        assert!(slow.modal.as_deref().unwrap().ends_with("  --More--"));
    }

    #[test]
    fn call_prompt_defaults_to_the_reference_appearance_or_existing_guess() {
        let mut game = Game::new(21);
        let potion = mrzavec::item::Item::basic(1, ItemKind::Potion, 0);
        assert_eq!(
            call_default(&game, &potion),
            game.appearances.potion_colors[0]
        );

        game.knowledge.guesses[0] = Some("bubbly".into());
        assert_eq!(call_default(&game, &potion), "bubbly");
    }

    #[test]
    fn automatic_call_prompts_use_the_reference_case_and_terse_wording() {
        let mut state = state(210);
        state.game.pending_call = Some((ItemKind::Potion, 0));
        assert!(show_pending_call(&mut state));
        assert_eq!(
            state.modal.as_deref(),
            Some("What do you want to call it? ")
        );

        state.game.options.terse = true;
        state.game.pending_call = Some((ItemKind::Potion, 0));
        assert!(show_pending_call(&mut state));
        assert_eq!(state.modal.as_deref(), Some("Call it: "));
    }

    #[test]
    fn repeated_call_reuses_the_normal_prefilled_call_flow() {
        let mut state = state(211);
        let item = mrzavec::item::Item::basic(u64::MAX, ItemKind::Scroll, 0);
        state.game.player.inventory.push(item.clone());
        state.game.last_item = Some(item.id);
        let expected = call_default(&state.game, &item);

        assert!(repeat_selected_command(&mut state, Command::Call));

        assert_eq!(state.pending, Some(Pending::CallText(item.id)));
        assert_eq!(state.input_buffer, expected);
        let expected_modal = format!("What do you want to call it? {expected}");
        assert_eq!(state.modal.as_deref(), Some(expected_modal.as_str()));
    }

    #[test]
    fn glyph_identification_names_monsters_and_terrain() {
        assert!(identify_glyph_text('D').contains("dragon"));
        assert!(identify_glyph_text('#').contains("passage"));
        assert!(help_for('q').contains("quaff"));
    }

    #[test]
    fn single_key_help_preserves_fight_modes_controls_and_unknown_form() {
        assert!(help_for('f').contains("near death"));
        assert!(help_for('F').contains("either of you dies"));
        assert!(help_for('\u{8}').starts_with("^H\t"));
        assert_eq!(help_for(' '), "unknown character ' '");
        let full = help_text();
        let lines: Vec<_> = full.lines().collect();
        assert_eq!(lines.len(), 24);
        assert!(full.ends_with(" --More--"));
        assert!(full.contains("<SHIFT><dir>: run that way"));
        assert!(!full.contains('\t'));
        assert_eq!(lines[0].chars().nth(40), Some('I'));
        assert!(!full.contains("shell escape"));
        assert!(!full.contains("Ctrl-Z"));
        assert!(!full.contains("legal no-op"));
    }

    #[test]
    fn glyph_identification_uses_unctrl_for_control_characters() {
        assert!(identify_glyph_text('\u{8}').starts_with("'^H': unknown character"));
    }

    #[test]
    fn magic_detection_view_marks_magic_at_map_coordinates() {
        let mut game = Game::new(190);
        game.floor_items.clear();
        let pos = Pos::new(37, 12);
        let mut ring = mrzavec::item::Item::basic(999, ItemKind::Ring, 0);
        ring.pos = Some(pos);
        game.floor_items.push(ring);

        let view = magic_detection_text(&game);
        let lines: Vec<_> = view.lines().collect();

        assert_eq!(lines[pos.y as usize].chars().nth(pos.x as usize), Some('$'));
    }

    #[test]
    fn wizard_map_view_reveals_without_mutating_secret_features() {
        let mut game = Game::new(189);
        let door = Pos::new(37, 12);
        let passage = Pos::new(38, 12);
        let trap = Pos::new(39, 12);
        game.dungeon.map.get_mut(door).unwrap().terrain = mrzavec::map::Terrain::SecretDoor;
        game.dungeon.map.get_mut(passage).unwrap().terrain = mrzavec::map::Terrain::SecretPassage;
        game.dungeon.map.get_mut(trap).unwrap().trap = Some(mrzavec::map::Trap::Bear);
        let before = game.dungeon.clone();

        let view = wizard_map_text(&game);
        let lines: Vec<_> = view.lines().collect();

        assert!(lines[0].starts_with(" --More--"));
        assert_eq!(lines[12].chars().nth(37), Some('+'));
        assert_eq!(lines[12].chars().nth(38), Some('#'));
        assert_eq!(lines[12].chars().nth(39), Some('^'));
        assert_eq!(game.dungeon, before);
    }

    #[test]
    fn wizard_creation_prompts_use_each_reference_subtype_range() {
        assert_eq!(
            wizard_which_prompt(ItemKind::Potion),
            "which ! (potion) do you want? (0-d)"
        );
        assert_eq!(
            wizard_which_prompt(ItemKind::Scroll),
            "which ? (scroll) do you want? (0-h)"
        );
        assert_eq!(
            wizard_which_prompt(ItemKind::Weapon),
            "which ) (weapon) do you want? (0-8)"
        );
        assert_eq!(wizard_kind_count(ItemKind::Ring), 14);
    }

    #[test]
    fn wizard_star_lists_reference_object_probabilities() {
        let mut game = Game::new(108);
        assert_eq!(
            wizard_list_prompt(&game),
            "for what type of object do you want a list? "
        );
        game.options.terse = true;
        assert_eq!(wizard_list_prompt(&game), "what type? ");

        let potions = wizard_probability_text('!').unwrap();
        assert!(potions.starts_with("0: confusion (7%)\n1: hallucination (8%)"));
        assert!(potions.contains("d: levitation (6%)"));
        assert!(wizard_probability_text('D').is_none());
    }

    #[test]
    fn empty_pack_suppresses_wizard_identify_and_charge_prompts() {
        let mut identify = state(106);
        identify.game.player.inventory.clear();
        assert!(wizard_identify_prompt(&mut identify).is_none());
        assert_eq!(
            identify.game.messages.last().map(String::as_str),
            Some("you don't have anything in your pack to identify")
        );
        assert!(identify.pending.is_none());

        let mut charge = state(107);
        charge.game.player.inventory.clear();
        assert!(wizard_charge_prompt(&mut charge).is_none());
        assert_eq!(
            charge.game.messages.last().map(String::as_str),
            Some("you aren't carrying anything")
        );
        assert!(charge.pending.is_none());
    }

    #[test]
    fn invalid_ring_hand_retries_with_reference_feedback() {
        let mut verbose = state(116);
        retry_ring_hand(&mut verbose);
        assert_eq!(verbose.modal.as_deref(), Some("Left hand or right hand? "));
        collect_messages(&mut verbose);
        assert_eq!(
            verbose.visible_message.as_deref(),
            Some("please type L or R")
        );
        assert!(verbose.message_wait);
        assert!(verbose.modal.is_none());

        let mut terse = state(117);
        terse.game.options.terse = true;
        retry_ring_hand(&mut terse);
        assert_eq!(terse.modal.as_deref(), Some("Left or right ring? "));
        collect_messages(&mut terse);
        assert_eq!(terse.visible_message.as_deref(), Some("L or R"));
        assert!(terse.message_wait);
    }

    #[test]
    fn food_detection_view_marks_food_at_map_coordinates() {
        let mut game = Game::new(191);
        game.floor_items.clear();
        let pos = Pos::new(38, 13);
        let mut food = mrzavec::item::Item::basic(1000, ItemKind::Food, 0);
        food.pos = Some(pos);
        game.floor_items.push(food);

        let view = food_detection_text(&game);
        let lines: Vec<_> = view.lines().collect();

        assert_eq!(lines[pos.y as usize].chars().nth(pos.x as usize), Some(':'));
    }

    #[test]
    fn inventory_hides_individual_numeric_properties_until_known() {
        let mut game = Game::new(198);
        let mut sword = mrzavec::item::Item::basic(1000, ItemKind::Weapon, 1);
        sword.hit_plus = 2;
        sword.damage_plus = 1;
        assert_eq!(game.item_name(&sword), "long sword");
        sword.known = true;
        assert!(game.item_name(&sword).contains("+2/+1"));

        let mut stick = mrzavec::item::Item::basic(1001, ItemKind::Stick, 0);
        stick.charges = 17;
        game.knowledge.sticks[0] = true;
        assert!(!game.item_name(&stick).contains("17"));
        stick.known = true;
        assert!(game.item_name(&stick).contains("[17]"));
    }

    #[test]
    fn current_equipment_views_hide_unknown_bonuses_and_show_ring_names() {
        let mut game = Game::new(102);
        let weapon_id = 90_001;
        let mut weapon = mrzavec::item::Item::basic(weapon_id, ItemKind::Weapon, 5);
        weapon.hit_plus = 3;
        weapon.damage_plus = 2;
        game.player.inventory.push(weapon);
        game.player.weapon = Some(weapon_id);
        let weapon_text = equipment_text(&game, "Weapon", Some(weapon_id));
        assert!(weapon_text.contains("two handed sword"));
        assert!(!weapon_text.contains("+3"));

        let ring_id = 90_002;
        let ring = mrzavec::item::Item::basic(ring_id, ItemKind::Ring, 0);
        let stone = game.appearances.ring_stones[0].clone();
        game.player.inventory.push(ring);
        game.player.rings[0] = Some(ring_id);
        let ring_text = rings_text(&game);
        assert!(ring_text.contains(&format!("{stone} ring")));
        assert!(!ring_text.contains(&ring_id.to_string()));
    }

    #[test]
    fn current_equipment_messages_match_the_nonblocking_reference_forms() {
        let mut game = Game::new(103);
        let weapon = game.player.weapon.unwrap();
        assert!(
            current_message(&game, Some(weapon), "wielding", None)
                .starts_with("you are wielding (")
        );
        assert_eq!(
            current_message(&game, None, "wearing", Some("on left hand")),
            "you are wearing nothing on left hand"
        );

        game.options.terse = true;
        let letter = game
            .player
            .inventory
            .iter()
            .find(|item| item.id == weapon)
            .unwrap()
            .pack_letter
            .unwrap();
        assert!(
            current_message(&game, Some(weapon), "wielding", None)
                .starts_with(&format!("{letter}) "))
        );
        assert_eq!(
            current_message(&game, None, "wearing", Some("(R)")),
            "wearing nothing (R)"
        );
    }

    #[test]
    fn repeat_reuses_the_previous_item_without_prompting() {
        let mut state = state(3);
        state.game.player.inventory.clear();
        let id = state.game.next_id;
        state.game.next_id += 1;
        let mut potion = mrzavec::item::Item::basic(id, ItemKind::Potion, 0);
        potion.count = 2;
        state.game.player.inventory.push(potion);
        state.game.last_item = Some(id);

        assert!(repeat_selected_command(&mut state, Command::Quaff));

        assert_eq!(state.game.player.inventory[0].count, 1);
        assert!(state.pending.is_none());
    }

    #[test]
    fn repeating_a_consumable_after_it_runs_out_costs_a_turn() {
        let mut state = state(33);
        state.game.player.inventory.clear();
        state.game.last_item = Some(999_999);
        let turn = state.game.turn;

        assert!(repeat_selected_command(&mut state, Command::Quaff));

        assert_eq!(state.game.turn, turn + 1);
        assert_eq!(
            state.game.messages.last().map(String::as_str),
            Some("you ran out")
        );
    }

    #[test]
    fn repeated_identify_scroll_with_no_remaining_pack_ends_the_prompt() {
        let mut state = state(115);
        state.game.player.inventory.clear();
        let id = state.game.next_id;
        state.game.next_id += 1;
        state
            .game
            .player
            .inventory
            .push(mrzavec::item::Item::basic(id, ItemKind::Scroll, 5));
        state.game.last_item = Some(id);

        assert!(repeat_selected_command(&mut state, Command::Read));

        assert!(state.pending.is_none());
        assert!(state.game.pending_identification.is_none());
        assert_eq!(
            state.game.messages.last().map(String::as_str),
            Some("you don't have anything in your pack to identify")
        );
    }

    #[test]
    fn counted_multistep_command_prompts_for_each_iteration() {
        let mut state = state(4);
        state.counted_command = Some((Command::Quaff, 2));

        continue_counted_command(&mut state);

        assert_eq!(state.pending, Some(Pending::Quaff));
        assert_eq!(state.counted_command, Some((Command::Quaff, 1)));
        assert_eq!(
            state.modal.as_deref(),
            Some("Which object do you want to quaff? (* for list): ")
        );
    }

    #[test]
    fn final_counted_prompt_becomes_the_repeat_command_at_reference_time() {
        let mut state = state(40);
        state.game.last_command = Some('s');
        state.game.last_item = Some(123);
        state.counted_command = Some((Command::Quaff, 1));

        continue_counted_command(&mut state);

        assert_eq!(state.pending, Some(Pending::Quaff));
        assert_eq!(state.counted_command, None);
        assert_eq!(state.game.last_command, Some('q'));
        assert_eq!(state.game.previous_command, Some('s'));
        assert_eq!(state.game.previous_item, Some(123));
        assert_eq!(state.game.last_item, None);
    }

    #[test]
    fn empty_pack_counted_actions_finish_every_remaining_iteration() {
        let mut state = state(41);
        state.game.player.inventory.clear();
        state.counted_command = Some((Command::Quaff, 2));
        let turn = state.game.turn;

        continue_counted_command(&mut state);

        assert_eq!(state.counted_command, None);
        assert_eq!(state.pending, None);
        assert_eq!(state.game.turn, turn + 2);
        assert_eq!(state.game.last_command, Some('q'));
    }

    #[test]
    fn illegal_wizard_command_clears_a_count_before_the_final_iteration() {
        let mut state = state(42);
        state.game.wizard = false;
        state.game.last_command = Some('s');
        state.counted_command = Some((Command::Wizard(WizardCommand::Create), 2));

        continue_counted_command(&mut state);

        assert_eq!(state.counted_command, None);
        assert_eq!(state.game.last_command, Some('s'));
        assert_eq!(
            state
                .game
                .messages
                .iter()
                .filter(|message| message.as_str() == "illegal command 'C'")
                .count(),
            1
        );
    }

    #[test]
    fn empty_pack_item_prompts_preserve_reference_turn_rules() {
        let mut consuming = state(30);
        consuming.game.player.inventory.clear();
        let turn = consuming.game.turn;
        assert!(select_action_prompt(&mut consuming, Pending::Read, true).is_none());
        assert_eq!(consuming.game.turn, turn + 1);
        assert_eq!(
            consuming.game.messages.last().map(String::as_str),
            Some("you aren't carrying anything")
        );

        let mut free = state(31);
        free.game.player.inventory.clear();
        let turn = free.game.turn;
        assert!(select_action_prompt(&mut free, Pending::Wield, false).is_none());
        assert_eq!(free.game.turn, turn);
    }

    #[test]
    fn invalid_item_selection_rebuilds_the_prompt_with_unctrl_feedback() {
        let mut state = state(114);
        state.pending = Some(Pending::Quaff);
        retry_invalid_item(&mut state, Pending::Quaff, '\u{8}');
        assert_eq!(
            state.modal.as_deref(),
            Some("Which object do you want to quaff? (* for list): ")
        );
        collect_messages(&mut state);
        assert_eq!(
            state.visible_message.as_deref(),
            Some("'^H' is not a valid item")
        );
        assert!(state.message_wait);
        assert!(state.modal.is_none());
        assert_eq!(
            state.game.recall_message,
            "which object do you want to quaff? (* for list): "
        );
        assert!(item_selection_title(Pending::Options(0)).is_none());
    }

    #[test]
    fn recall_uses_the_dedicated_reference_message_buffer() {
        let mut game = Game::new(116);
        game.message("message to recall");
        game.message_without_recall("illegal command 'x'");

        recall_last_message(&mut game);

        assert_eq!(
            game.messages.last().map(String::as_str),
            Some("message to recall")
        );
        assert_eq!(game.recall_message, "message to recall");
    }

    #[test]
    fn endmsg_capitalization_is_presentation_only_with_reference_exceptions() {
        assert_eq!(message_display_text("you found gold"), "You found gold");
        assert_eq!(message_display_text("a) a potion"), "a) a potion");
        assert_eq!(message_display_text("'^H': unknown"), "'^H': unknown");

        let mut state = state(1160);
        let displayed = remembered_prompt(&mut state, "which direction? ");
        assert_eq!(displayed, "Which direction? ");
        assert_eq!(state.game.recall_message, "which direction? ");

        state.game.message("h\tleft");
        state.preserve_message_case = true;
        let buffer = display(&state);
        assert_eq!(
            buffer.into_iter().take(12).collect::<String>(),
            "h       left"
        );
    }

    #[test]
    fn consecutive_messages_and_following_prompts_wait_in_reference_order() {
        let mut sequence = state(1161);
        sequence.game.message("first message");
        sequence.game.message("second message");
        collect_messages(&mut sequence);

        assert_eq!(sequence.visible_message.as_deref(), Some("first message"));
        assert!(sequence.message_wait);
        let first_row: String = display(&sequence).into_iter().take(80).collect();
        assert!(first_row.starts_with("First message --More--"));

        advance_message(&mut sequence);
        assert_eq!(sequence.visible_message.as_deref(), Some("second message"));
        assert!(!sequence.message_wait);

        let mut prompted = state(1162);
        prompted.pending = Some(Pending::Quaff);
        prompted.modal = Some("Which object? ".into());
        prompted.game.message("an effect happened");
        collect_messages(&mut prompted);
        assert!(prompted.modal.is_none());
        assert!(prompted.deferred_modal.is_some());
        assert!(prompted.message_wait);

        advance_message(&mut prompted);
        assert_eq!(prompted.modal.as_deref(), Some("Which object? "));
        assert_eq!(prompted.pending, Some(Pending::Quaff));
    }

    #[test]
    fn get_item_prompt_hides_the_inventory_until_star_and_filters_the_list() {
        let mut state = state(115);
        state.game.player.inventory.clear();
        state
            .game
            .player
            .inventory
            .push(mrzavec::item::Item::basic(50_000, ItemKind::Potion, 0));
        state
            .game
            .player
            .inventory
            .push(mrzavec::item::Item::basic(50_001, ItemKind::Stick, 0));

        let prompt = select_prompt(&mut state, Pending::Quaff).unwrap();
        assert_eq!(prompt, "Which object do you want to quaff? (* for list): ");
        assert!(!prompt.contains("Inventory"));

        show_item_inventory(&mut state, Pending::Quaff);
        let modal = state.modal.as_deref().unwrap();
        assert!(state.item_inventory_open);
        assert!(modal.contains("potion"));
        assert!(!modal.contains("staff"));

        restore_item_prompt(&mut state, Pending::Quaff);
        state.game.options.terse = true;
        assert_eq!(
            select_prompt(&mut state, Pending::Quaff).as_deref(),
            Some("Quaff what? (* for list): ")
        );
    }

    #[test]
    fn save_confirmation_uses_the_configured_reference_filename() {
        let mut game = Game::new(34);
        game.options.save_file = "/tmp/rodney.save".into();
        assert_eq!(save_confirmation(&game), "save file (/tmp/rodney.save)? ");
    }

    #[test]
    fn quit_confirmation_accepts_only_y_and_every_other_key_cancels() {
        for ch in ['n', 'x', '\u{1b}', ' '] {
            let mut state = state(340);
            state.pending = Some(Pending::QuitConfirm);
            state.modal = Some("really quit?".into());
            resolve_quit_confirmation(&mut state, ch);
            assert_eq!(state.game.end, mrzavec::game::EndState::Playing);
            assert_eq!(state.pending, None);
            assert_eq!(state.modal, None);
        }

        for ch in ['y', 'Y'] {
            let mut state = state(341);
            state.pending = Some(Pending::QuitConfirm);
            resolve_quit_confirmation(&mut state, ch);
            assert_eq!(state.game.end, mrzavec::game::EndState::Quit);
        }
    }

    #[test]
    fn counted_throw_starts_with_the_reference_direction_prompt() {
        let mut state = state(32);
        state.counted_command = Some((Command::Throw, 1));

        continue_counted_command(&mut state);

        assert_eq!(state.pending, Some(Pending::ThrowDirection));
        assert_eq!(state.modal.as_deref(), Some("Which direction? "));
        assert_eq!(state.game.recall_message, "which direction? ");
    }

    #[test]
    fn direction_prompts_preserve_the_reference_forms_with_the_terse_typo_fixed() {
        let mut state = state(2070);
        assert_eq!(
            direction_prompt(&mut state, Pending::ZapDirection).as_deref(),
            Some("Which direction? ")
        );
        assert_eq!(state.game.recall_message, "which direction? ");

        state.game.options.terse = true;
        assert_eq!(
            direction_prompt(&mut state, Pending::ZapDirection).as_deref(),
            Some("Direction: ")
        );
        assert_eq!(state.game.recall_message, "direction: ");
    }

    #[test]
    fn decimal_count_whitelist_matches_the_reference_command_switch() {
        for command in [
            'h', 'L', '\u{8}', '.', 'a', 'q', 'r', 's', 't', 'z', 'm', 'I', 'C', '\u{1}', '\u{4}',
            '\u{1a}',
        ] {
            assert!(
                countable_command(command),
                "{command:?} should accept a count"
            );
        }
        for command in [
            'e', 'w', 'W', 'T', 'P', 'R', 'd', 'i', 'f', 'F', '?', 'Q', 'S',
        ] {
            assert!(
                !countable_command(command),
                "{command:?} should discard a count"
            );
        }
    }

    #[test]
    fn only_get_item_and_get_direction_prompts_reset_the_repeat_command() {
        for pending in [
            Pending::Quaff,
            Pending::Read,
            Pending::Eat,
            Pending::Wield,
            Pending::Wear,
            Pending::PutRing,
            Pending::Drop,
            Pending::ThrowSelect,
            Pending::ZapSelect,
            Pending::ThrowDirection,
            Pending::ZapDirection,
            Pending::FightDirection(false),
            Pending::MoveDirection,
            Pending::TrapDirection,
            Pending::Identify,
            Pending::CallSelect,
            Pending::WizardCharge,
        ] {
            assert!(prompt_resets_last(pending), "{pending:?} should reset");
        }
        for pending in [
            Pending::PutRingHand(1),
            Pending::RemoveRingHand,
            Pending::PickyInventory,
            Pending::Help,
            Pending::WizardCreateType,
            Pending::QuitConfirm,
        ] {
            assert!(
                !prompt_resets_last(pending),
                "{pending:?} should keep the current repeat command"
            );
        }
    }
}
