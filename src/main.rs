use bevy::{prelude::*, text::LineHeight, window::WindowResolution};
use mrzavec::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, DUNGEON_FIRST_ROW, EVENT_ROWS, Game, KEYBINDING_FIRST_ROW,
    KEYBINDING_SECOND_ROW, STATUS_ROW,
    command::{Command, WizardCommand, parse},
    item::{
        ARMOR_NAMES, ARMOR_WEIGHTS, ItemKind, POTION_NAMES, POTION_WEIGHTS, RING_NAMES,
        RING_WEIGHTS, SCROLL_NAMES, SCROLL_WEIGHTS, STICK_NAMES, STICK_WEIGHTS, WEAPON_NAMES,
        WEAPON_WEIGHTS,
    },
    lang::speak,
    map::Pos,
    save, score,
};
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
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
    "Iti h/j/k/l  Torba i  Piti q  Čitati r  Jesti e  Orųž. w  Ostav. d";
const KEYBINDING_SECOND_TEXT: &str =
    "Nositi W  Sjęti T  Metnųti t  Žezlo z  Iskati s  Čekati .  Pomoć ?";
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
const GLYPH_COLOR: Color = Color::srgb(0.82, 0.82, 0.78);
const KEYBINDING_DIM_COLOR: Color = Color::srgb(0.55, 0.55, 0.52);
const ROGUE_RELEASE: &str = "2026-07-17";

fn held_repeat_keys() -> impl Iterator<Item = (KeyCode, char)> {
    MOVEMENT_KEYS.into_iter().chain([(KeyCode::Period, '.')])
}

fn version_message(game: &Game) -> String {
    format!(
        "rogue verzija 5.4.5 izdańje {ROGUE_RELEASE} temnica {} (chongo was here)",
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
/// retained by the simulation for Ctrl-R recall. Prompt templates reach the
/// screen through here, so ⟨…⟩ markers are rendered first (`speak` is
/// idempotent on already-rendered text).
fn message_display_text(text: &str) -> String {
    let mut displayed = speak(text);
    let mut chars = displayed.char_indices();
    let Some((_, first)) = chars.next() else {
        return displayed;
    };
    let second = chars.next().map(|(_, ch)| ch);
    if first.is_lowercase() && second != Some(')') {
        // Unicode-aware: ž/č/š-initial messages must capitalize too.
        displayed.replace_range(0..first.len_utf8(), &first.to_uppercase().to_string());
    }
    displayed
}

fn wizard_password_matches(input: &str) -> bool {
    input == "bathtub"
}

fn password_prompt(pending: Pending) -> String {
    if pending == Pending::StartupPassword {
        speak("parola ⟨n:čarovnik:gen⟩: ")
    } else {
        speak("parola ⟨n:čarovnik:gen:U⟩: ")
    }
}

fn call_prompt(game: &Game) -> String {
    if game.options.terse {
        "nazvati: ".into()
    } else {
        speak("kako ⟨v2:hotěti⟩ to nazvati? ")
    }
}

#[derive(Resource)]
struct State {
    game: Game,
    modal: Option<String>,
    modal_overlay: bool,
    modal_offset: usize,
    preserve_message_case: bool,
    slow_discovery_lines: Vec<String>,
    message_serial_seen: u64,
    visible_message: Option<String>,
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
    ThrowDirection(u64),
    ZapDirection(u64),
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

        if let Some((key, _)) = held_repeat_keys().find(|(key, _)| keys.just_pressed(*key)) {
            self.key = Some(key);
            self.remaining = MOVEMENT_REPEAT_DELAY;
            return None;
        }

        let held = self
            .key
            .filter(|key| keys.pressed(*key))
            .and_then(|key| held_repeat_keys().find(|(candidate, _)| *candidate == key));
        let Some((key, ch)) = held else {
            let Some((key, _)) = held_repeat_keys().find(|(key, _)| keys.pressed(*key)) else {
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
        "Upotrěba: {program} [-SrdVh] [-s [fajl_rezultatov]] [fajl_shranjeńja]\n\n\
         \t-S\t\tpri signalu izhod bez shranjeńja\n\
         \t-r\t\tignoruje sę (kompatibilnosť)\n\
         \t-s [fajl]\tpokazati spisȯk rezultatov\n\
         \t-d\t\tubiti igrača i råzsčitati rezultat\n\
         \t-h\t\tpokazati tutų pomoć\n\
         \t-V\t\tpokazati verzijų\n\
         \t[fajl]\tobnoviti igrų (standardno: {})\n\n\
         Standardny fajl rezultatov: {}\n\
         rogue verzija: 5.4.5 {ROGUE_RELEASE} (chongo was here)",
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
            eprintln!(
                "{program}: BLŲD: {} -- {flag}",
                speak("⟨a:nepraviľny:opcija:nom⟩ ⟨n:opcija:nom⟩")
            );
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
                eprintln!(
                    "{}",
                    speak(
                        "BLŲD: pųť ⟨n:fajl:gen⟩ ⟨n:rezultat:gen:pl⟩ ⟨v3:imati⟩ vyše 80 ⟨n:znak:gen:pl⟩"
                    )
                );
                std::process::exit(4);
            }
            let path = path.map_or_else(|| PathBuf::from(&options.score_file), PathBuf::from);
            let scores = match score::read(&path) {
                Ok(scores) => scores,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(error) => {
                    eprintln!(
                        "{} {}: {error}",
                        speak("Ne možno čitati spisȯk ⟨n:rezultat:gen:pl⟩"),
                        path.display()
                    );
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
            game.death_cause = Some(mrzavec::lang::phrase(
                &mrzavec::lang::MONSTER_LEX[1],
                interslavic::Case::Gen,
                interslavic::Number::Singular,
            ));
            game.end = mrzavec::game::EndState::Dead;
            let table = match score::record_locked(
                &game,
                std::path::Path::new(&game.options.score_file),
                std::path::Path::new(&game.options.lock_file),
            ) {
                Ok(scores) => score::format(&scores),
                Err(error) => format!(
                    "{}: {error}",
                    speak("Ne možno čitati ili obnoviti spisȯk ⟨n:rezultat:gen:pl⟩")
                ),
            };
            if game.options.tombstone {
                println!("{}\n\n{table}", tombstone_text(&game));
            } else {
                println!(
                    "Smŕť od {}, s {}\n\n{table}",
                    death_cause_gen(&game),
                    gold_with(score::amount(&game) as u64)
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
                    "{}",
                    speak(&format!(
                        "Ne možno obnoviti {}: {error}",
                        std::path::Path::new(&path).display()
                    ))
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
        eprintln!(
            "{}: {error}",
            speak("Ne možno instalovati ⟨n:obsluga:acc⟩ ⟨n:signal:gen⟩")
        );
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
            game.message(format!("ne možno obnoviti shranjeńje iz browsera: {error}"));
            game
        }
    };
    if game.options.name.is_empty() {
        game.options.name = "igrač".into();
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
            preserve_message_case: false,
            slow_discovery_lines: Vec::new(),
            message_serial_seen,
            visible_message: None,
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
        eprintln!("Avtomatično shranjeńje ne udalo sę: {error}");
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
        Err(error) => format!(
            "{}: {error}",
            speak("Ne možno čitati ili obnoviti spisȯk ⟨n:rezultat:gen:pl⟩")
        ),
    };
    if state.game.end == mrzavec::game::EndState::Dead && state.game.options.tombstone {
        state.modal = Some(format!("{}\n\n{}", tombstone_text(&state.game), table));
        state.score_recorded = true;
        return;
    }
    state.modal = Some(match state.game.end {
        mrzavec::game::EndState::Won => format!(
            "{}\n\n{}\nKonečny rezultat: {}\n\n{}",
            speak(
                "⟨n:čestitańje:nom:pl:U⟩, ⟨v2:viděti⟩ ⟨a:dnevny:světlo:acc⟩ ⟨n:světlo:acc⟩!\n\nUspěšno ⟨v2:izhoditi⟩ iz ⟨n:temnica:gen:pl:U⟩ ⟨n:pohibel:gen:U⟩."
            ),
            winner_sales_text(&state.game),
            score::amount(&state.game),
            table
        ),
        mrzavec::game::EndState::Dead if state.game.options.tombstone => format!(
            "                       __________\n                      /  POČIVAJ   \\\n                     /      V       \\\n                    /      MIRU      \\\n\n                 Smŕť od {}\n                  Zlåto: {}\n                 Stųpenj: {}\n\n{}",
            death_cause_gen(&state.game),
            score::amount(&state.game),
            state.game.depth,
            table
        ),
        mrzavec::game::EndState::Dead => format!(
            "Smŕť od {}, s {}\n\n{}",
            death_cause_gen(&state.game),
            gold_with(score::amount(&state.game) as u64),
            table
        ),
        mrzavec::game::EndState::Quit => format!(
            "Izhod s {}\n\n{}",
            gold_with(state.game.player.gold as u64),
            table
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
    let cause = death_cause_gen(game);
    // The epitaph imperative is inflected at runtime like everything else;
    // the stone traditionally carves it in capitals.
    let epitaph = speak("⟨vim:počivati⟩").to_uppercase();
    let mut lines: Vec<Vec<char>> = [
        "                       __________",
        "                      /          \\",
        "                     /            \\",
        "                    /      V       \\",
        "                   /      MIRU      \\",
        "                  /                  \\",
        "                  |                  |",
        "                  |                  |",
        "                  |     smŕť od      |",
        "                  |                  |",
        "                  |       1980       |",
        "                 *|     *  *  *      | *",
        r"         ________)/\_//(\/(/\)/\//\/|_)_______",
    ]
    .into_iter()
    .map(|line| line.chars().collect())
    .collect();
    overlay(&mut lines[2], 24, &epitaph);
    overlay(
        &mut lines[6],
        center(&game.options.name),
        &game.options.name,
    );
    let gold = format!("{} Au", score::amount(game));
    overlay(&mut lines[7], center(&gold), &gold);
    overlay(&mut lines[9], center(&cause), &cause);
    overlay(&mut lines[10], 26, &format!("{:4}", current_year()));
    lines
        .into_iter()
        .map(|line| line.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Death cause for display after "od". Causes are stored as complete
/// genitive strings (game.rs `die()`); only the internal "signal" key —
/// which score.rs compares literally — still needs a genitive rendering.
/// "N zlåtnikom/zlåtnikami" — instrumental counted phrase for "s {gold}"
/// (interslavic::quantified handles the n==1 singular the old fixed
/// ins:pl marker got wrong).
fn gold_with(amount: u64) -> String {
    format!(
        "{} {}",
        amount,
        interslavic::quantified(
            amount,
            mrzavec::lang::GOLD_COIN.lemma,
            interslavic::Case::Ins,
            mrzavec::lang::GOLD_COIN.gender,
            mrzavec::lang::GOLD_COIN.animacy,
        )
    )
}

fn death_cause_gen(game: &Game) -> String {
    match game.death_cause.as_deref() {
        Some("signal") => speak("⟨n:signal:gen⟩"),
        Some(cause) => cause.into(),
        None => speak("⟨n:bog:gen:U⟩"),
    }
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
    let mut out = String::from("    Cěna  Prědmet\n");
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
    out.push_str(&format!(
        "   {:5}  {}",
        game.player.gold,
        speak("⟨n:zlåtnik:nom:pl:U⟩")
    ));
    out
}

fn setup(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    // Bevy's built-in default font covers only ASCII; the Interslavic
    // orthography (ě ų ȯ č ...) needs full Latin-Extended coverage.
    fonts
        .insert(
            &Handle::default(),
            Font::from_bytes(include_bytes!("../assets/DejaVuSansMono.ttf").to_vec()),
        )
        .expect("install full-coverage default font");
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
                        TextColor(GLYPH_COLOR),
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
    let repeated_input = movement_repeat.update(
        &keys,
        time.delta(),
        !shifted
            && !controlled
            && !alt_or_super
            && state.pending.is_none()
            && state.modal.is_none()
            && state.game.end == mrzavec::game::EndState::Playing
            && state.game.player.conditions.asleep_turns == 0,
    );
    if keys.get_just_pressed().next().is_some() || repeated_input.is_some() {
        state.preserve_message_case = false;
        state.visible_message = None;
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
    if state.pending == Some(Pending::Help) {
        if keys.just_pressed(KeyCode::Escape) {
            state.pending = None;
            state.modal = None;
            state.modal_offset = 0;
        } else if keys.just_pressed(KeyCode::Space)
            && state
                .modal
                .as_deref()
                .is_some_and(|modal| modal_has_next_page(modal, state.modal_offset))
        {
            state.modal_offset += MODAL_PAGE_ROWS;
        }
        return;
    }
    if let Some(pending) = state.pending.filter(|pending| is_item_selection(*pending))
        && keys.just_pressed(KeyCode::Space)
    {
        if state
            .modal
            .as_deref()
            .is_some_and(|modal| modal_has_next_page(modal, state.modal_offset))
        {
            state.modal_offset += MODAL_PAGE_ROWS;
        } else {
            retry_invalid_item(&mut state, pending, ' ');
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
            state.modal = Some(format!("{}  --Dalje--", message_display_text(&line)));
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
        if state.pending == Some(Pending::Password) {
            state.game.message("žalj");
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
                    "(O, S ili C)"
                } else {
                    "(T ili F)"
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
                        state.game.message("žalj")
                    }
                }
                Pending::SaveFileText => {
                    if input.is_empty() {
                        state.modal = Some(remembered_prompt(&mut state, "ime ⟨n:fajl:gen⟩: "));
                        return;
                    }
                    state.game.options.save_file = mrzavec::game::normalize_option_string(&input);
                    if save_exists(&state.game).unwrap_or(false) {
                        state.pending = Some(Pending::SaveOverwrite);
                        state.modal = Some(remembered_prompt(
                            &mut state,
                            "Fajl uže jest.  ⟨v2:hotěti:U⟩ li prěpisati jego?",
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
                Pending::Password | Pending::StartupPassword => password_prompt(pending),
                Pending::CallText(_) | Pending::AutoCall => {
                    format!("{}{}", call_prompt(&state.game), state.input_buffer)
                }
                Pending::SaveFileText => {
                    format!("{}{}", speak("ime ⟨n:fajl:gen⟩: "), state.input_buffer)
                }
                Pending::WizardCreateGold => format!("koliko?{}", state.input_buffer),
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
        .or(repeated_input);
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
    if let Some(pending) = state.pending {
        if matches!(pending, Pending::SaveConfirm | Pending::SaveOverwrite) {
            match (pending, ch.to_ascii_lowercase()) {
                (Pending::SaveConfirm, 'y') | (Pending::SaveOverwrite, 'y') => {
                    save_and_exit(&mut state, &mut app_exit);
                }
                (Pending::SaveConfirm, 'n') => {
                    state.input_buffer.clear();
                    state.pending = Some(Pending::SaveFileText);
                    state.modal = Some(remembered_prompt(&mut state, "ime ⟨n:fajl:gen⟩: "));
                }
                (Pending::SaveOverwrite, 'n') => {
                    state.pending = Some(Pending::SaveConfirm);
                    let prompt = save_confirmation(&state.game);
                    state.modal = Some(remembered_prompt(&mut state, prompt));
                }
                _ => {
                    let error = if pending == Pending::SaveConfirm {
                        "prošų, ⟨vim:odgovoriti⟩ Y ili N"
                    } else {
                        "Prošų, ⟨vim:odgovoriti⟩ Y ili N"
                    };
                    state.game.message(error);
                    let prompt = if pending == Pending::SaveConfirm {
                        save_confirmation(&state.game)
                    } else {
                        "Fajl uže jest.  ⟨v2:hotěti:U⟩ li prěpisati jego?".into()
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
                    "Ne vid"
                } else {
                    "Prošų, ⟨vim:pisati⟩ jedin iz !?=/ (ESCAPE za izhod)"
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
                    .message(format!("'{}' ne jest v ⟨n:torba:loc⟩", control_label(ch)));
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
                    state.modal = Some(remembered_prompt(&mut state, "koliko?"));
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
                            Some("(T ili F)"),
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
                            Some("(O, S ili C)"),
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
                    Pending::StartupPassword => password_prompt(pending),
                    Pending::Password => message_display_text(&password_prompt(pending)),
                    Pending::CallText(_) | Pending::AutoCall => {
                        format!(
                            "{}{}",
                            message_display_text(&call_prompt(&state.game)),
                            state.input_buffer
                        )
                    }
                    Pending::SaveFileText => {
                        format!("{}{}", speak("Ime ⟨n:fajl:gen⟩: "), state.input_buffer)
                    }
                    Pending::WizardCreateGold => format!("Koliko?{}", state.input_buffer),
                    _ => unreachable!(),
                });
            }
            return;
        }
        if matches!(
            pending,
            Pending::ThrowDirection(_)
                | Pending::ZapDirection(_)
                | Pending::FightDirection(_)
                | Pending::MoveDirection
                | Pending::TrapDirection
        ) {
            if let Command::Move(direction) = parse(ch.to_ascii_lowercase()) {
                state.game.remember_message("");
                let result = match pending {
                    Pending::ThrowDirection(id) => {
                        state.game.last_direction = Some(direction);
                        state.game.throw_item(id, direction)
                    }
                    Pending::ZapDirection(id) => {
                        state.game.last_direction = Some(direction);
                        state.game.zap(id, direction)
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
        if is_item_selection(pending) && !ch.is_ascii_lowercase() {
            retry_invalid_item(&mut state, pending, ch);
            return;
        }
        if ch.is_ascii_lowercase() {
            let Some(index) = state.game.inventory_index_for_letter(ch) else {
                if is_item_selection(pending) {
                    retry_invalid_item(&mut state, pending, ch);
                }
                return;
            };
            if let Some(item) = state.game.player.inventory.get(index)
                && item.in_pack
                && item_matches_selection(&state.game, pending, item.kind)
            {
                let id = item.id;
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
                                    "uže ⟨v2:nositi⟩ dva ⟨n:pŕstenj:nom:pl⟩"
                                } else {
                                    "uže ⟨v2:imati⟩ pŕstenj na ⟨a:každy:rųka:loc⟩ ⟨n:rųka:loc⟩"
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
                        state.modal = direction_prompt(&mut state, Pending::ThrowDirection(id));
                        return;
                    }
                    Pending::ZapSelect => {
                        state.modal = direction_prompt(&mut state, Pending::ZapDirection(id));
                        return;
                    }
                    Pending::ThrowDirection(_) | Pending::ZapDirection(_) => unreachable!(),
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
            } else if is_item_selection(pending) {
                retry_invalid_item(&mut state, pending, ch);
                return;
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
            state.game.message("ješče ne jest ⟨n:komanda:gen⟩");
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
            Some(remembered_prompt(&mut state, "čto ⟨v2:hotěti⟩ opoznati? "))
        }
        Command::Help => {
            state.game.remember_message("");
            state.pending = Some(Pending::Help);
            state.modal_offset = 0;
            Some(help_text())
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
                        .message(format!("ne možno otvoriti shell: {error}"));
                }
            }
            #[cfg(target_arch = "wasm32")]
            state.game.message("shell ne jest dostųpny v browseru");
            None
        }
        Command::Suspend => {
            state
                .game
                .message("suspend ne jest dostųpny v ⟨a:grafičny:režim:loc⟩ ⟨n:režim:loc⟩");
            None
        }
        Command::Quit => {
            state.pending = Some(Pending::QuitConfirm);
            Some(remembered_prompt(&mut state, "istinno li ⟨v2:izhoditi⟩?"))
        }
        Command::Save => {
            state.pending = Some(Pending::SaveConfirm);
            let prompt = save_confirmation(&state.game);
            Some(remembered_prompt(&mut state, prompt))
        }
        Command::Quaff => select_action_menu(&mut state, Pending::Quaff, true),
        Command::Read => select_action_menu(&mut state, Pending::Read, true),
        Command::Eat => select_action_menu(&mut state, Pending::Eat, true),
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
                state.game.message("ne ⟨v2:mogti⟩.  ⟨v3:izględati:U⟩, že to jest prokleto");
                state
                    .game
                    .finish_action(mrzavec::command::CommandResult::TURN);
                None
            } else {
                select_action_menu(&mut state, Pending::Wield, false)
            }
        }
        Command::Wear => select_action_menu(&mut state, Pending::Wear, true),
        Command::PutOnRing => select_action_menu(&mut state, Pending::PutRing, true),
        Command::Drop => select_action_menu(&mut state, Pending::Drop, true),
        Command::Throw => select_action_menu(&mut state, Pending::ThrowSelect, true),
        Command::Zap => select_action_menu(&mut state, Pending::ZapSelect, true),
        Command::Fight { kamikaze } => {
            direction_prompt(&mut state, Pending::FightDirection(kamikaze))
        }
        Command::MoveWithoutPickup => direction_prompt(&mut state, Pending::MoveDirection),
        Command::IdentifyTrap => direction_prompt(&mut state, Pending::TrapDirection),
        Command::Call => select_action_menu(&mut state, Pending::CallSelect, false),
        Command::ToggleWizard => {
            if state.game.wizard {
                state.game.set_wizard(false);
                None
            } else {
                state.input_buffer.clear();
                state.pending = Some(Pending::Password);
                Some(remembered_prompt(&mut state, "parola ⟨n:čarovnik:gen:U⟩: "))
            }
        }
        Command::RemoveRing => match state.game.player.rings {
            [None, None] => {
                let terse = state.game.options.terse;
                state.game.message(if terse {
                    "ne jest ⟨n:pŕstenj:gen⟩"
                } else {
                    "ne ⟨v2:nositi⟩ ⟨nikaky:gen⟩ ⟨n:pŕstenj:gen⟩"
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
            let message = current_message(&state.game, state.game.player.weapon, "⟨v2:dŕžati⟩", None);
            state.game.message(message);
            None
        }
        Command::CurrentArmor => {
            let message = current_message(&state.game, state.game.player.armor, "⟨v2:nositi⟩", None);
            state.game.message(message);
            None
        }
        Command::CurrentRings => {
            for (id, verbose_where, terse_where) in [
                (state.game.player.rings[0], "na ⟨a:lěvy:rųka:loc⟩ ⟨n:rųka:loc⟩", "(L)"),
                (state.game.player.rings[1], "na ⟨a:pravy:rųka:loc⟩ ⟨n:rųka:loc⟩", "(R)"),
            ] {
                let location = if state.game.options.terse {
                    terse_where
                } else {
                    verbose_where
                };
                let message = current_message(&state.game, id, "⟨v2:nositi⟩", Some(location));
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
            Some(remembered_prompt(&mut state, "vid ⟨n:prědmet:gen⟩: "))
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
            | Pending::ThrowDirection(_)
            | Pending::ZapDirection(_)
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
    format!("shraniti fajl ({})? ", game.options.save_file)
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
    Ok(format!("slot {}", game.options.save_file))
}

fn save_and_exit(state: &mut State, app_exit: &mut MessageWriter<AppExit>) {
    match persist_game(&state.game) {
        Ok(destination) => {
            state.game.message(format!("igra ⟨pp:shråniti:f⟩: {destination}"));
            app_exit.write(AppExit::Success);
        }
        Err(error) => {
            state.game.message(format!("shranjeńje ne udalo sę: {error}"));
            state.pending = Some(Pending::SaveFileText);
            state.input_buffer.clear();
            state.modal = Some(remembered_prompt(state, "ime ⟨n:fajl:gen⟩: "));
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
                state.game.message("uže ne ⟨v2:imati⟩ ⟨toj:gen⟩");
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
        state.game.message("uže ne ⟨v2:imati⟩ ⟨toj:gen⟩");
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
        state.game.message("uže ne ⟨v2:imati⟩ ⟨toj:gen⟩");
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
            format!("⟨pp:nazvati:n⟩ «{guess}»")
        } else {
            format!("⟨lp:byti:n:U⟩ ⟨pp:nazvati:n⟩ «{guess}»")
        });
    }
    state.pending = Some(Pending::CallText(id));
    let prompt = call_prompt(&state.game);
    state.game.remember_message(&prompt);
    state.modal = Some(format!(
        "{}{}",
        message_display_text(&prompt),
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
    state.game.remember_message(&prompt);
    state.modal = Some(message_display_text(&prompt));
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
            .message("ne ⟨v2:imati⟩ v ⟨n:torba:loc⟩ ⟨ničto:gen⟩ za ⟨n:opoznańje:acc⟩");
        return false;
    }
    state.modal = select_item_menu(state, Pending::Identify);
    if state.modal.is_none() {
        state.game.pending_identification = None;
        return false;
    }
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
        Command::Quaff => select_action_menu(state, Pending::Quaff, true),
        Command::Read => select_action_menu(state, Pending::Read, true),
        Command::Throw => select_action_menu(state, Pending::ThrowSelect, true),
        Command::Zap => select_action_menu(state, Pending::ZapSelect, true),
        Command::MoveWithoutPickup => direction_prompt(state, Pending::MoveDirection),
        Command::PickyInventory => picky_inventory_prompt(state),
        Command::Wizard(WizardCommand::Create) if state.game.wizard => {
            state.pending = Some(Pending::WizardCreateType);
            Some(remembered_prompt(state, "vid ⟨n:prědmet:gen⟩: "))
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
            "⟨a:prazdny:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩"
        } else {
            "ne ⟨v2:imati⟩ ⟨ničto:gen⟩"
        });
        return None;
    }
    let mut out = String::new();
    for item in &state.game.floor_items {
        out.push_str(&state.game.inventory_name(item, false));
        out.push('\n');
    }
    out.push_str(" --Dalje--");
    state.modal_overlay = state.game.options.inventory_style
        == mrzavec::game::InventoryStyle::Overwrite
        && out.lines().count() <= STATUS_ROW;
    Some(out)
}
fn wizard_list_prompt(game: &Game) -> String {
    if game.options.terse {
        "kaky vid? ".into()
    } else {
        speak("za kaky vid ⟨v2:hotěti⟩ spisȯk? ")
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
    out.push_str(" --Dalje--");
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
        ItemKind::Potion => "napitȯk",
        ItemKind::Scroll => "svitȯk",
        ItemKind::Weapon => "orųžje",
        ItemKind::Armor => "brȯnja",
        ItemKind::Ring => "pŕstenj",
        ItemKind::Stick => "posoh",
        _ => "prědmet",
    }
}
fn wizard_which_prompt(kind: ItemKind) -> String {
    let highest = wizard_kind_count(kind) - 1;
    let highest = if highest < 10 {
        (b'0' + highest) as char
    } else {
        (b'a' + highest - 10) as char
    };
    speak(&format!(
        "{} ({}) — ⟨a:kaky:čislo:nom⟩ čislo ⟨v2:hotěti⟩? (0-{highest})",
        kind.glyph(),
        wizard_kind_name(kind)
    ))
}
fn resolve_wizard_which(state: &mut State, kind: ItemKind, which: u8) {
    if which >= wizard_kind_count(kind) {
        let error = format!("⟨a:nepraviľny:čislo:nom⟩ čislo ({}), ješče raz", wizard_kind_name(kind));
        state.game.message(&error);
        let prompt = wizard_which_prompt(kind);
        state.game.remember_message(&prompt);
        state.modal = Some(message_display_text(&prompt));
    } else if matches!(kind, ItemKind::Weapon | ItemKind::Armor)
        || (kind == ItemKind::Ring && matches!(which, 0 | 1 | 7 | 8))
    {
        state.pending = Some(Pending::WizardCreateBlessing(kind, which));
        state.modal = Some(remembered_prompt(state, "blagoslovjeńje? (+,-,n) "));
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
    for (x, ch) in " --Dalje--".chars().enumerate() {
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
    let title =
        speak("⟨v2:čuti:U⟩ ⟨n:blizkosť:acc⟩ ⟨n:čar:gen:pl⟩ na ⟨toj:loc⟩ ⟨n:stųpenj:loc⟩. --Dalje--");
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
    let title =
        speak("⟨a:tvoj:nos:nom:U⟩ ⟨n:nos:nom⟩ ⟨v3:svŕběti⟩ i ⟨v2:čuti⟩ ⟨n:zapah:acc⟩ ⟨n:jeda:gen⟩. --Dalje--");
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

fn direction_prompt(state: &mut State, pending: Pending) -> Option<String> {
    let prompt = if state.game.options.terse {
        "strana: "
    } else {
        "v ⟨ktory:acc:f⟩ ⟨n:stråna:acc⟩? "
    };
    state.pending = Some(pending);
    state.game.remember_message(prompt);
    Some(message_display_text(prompt))
}
fn ring_hand_prompt(game: &Game) -> String {
    if game.options.terse {
        "lěvy ili pravy pŕstenj? ".into()
    } else {
        speak("⟨a:lěvy:rųka:nom⟩ ⟨n:rųka:nom⟩ ili ⟨a:pravy:rųka:nom⟩ ⟨n:rųka:nom⟩? ")
    }
}
fn retry_ring_hand(state: &mut State) {
    let error = if state.game.options.terse {
        "L ili R"
    } else {
        "prošų, L ili R"
    };
    state.game.message(error);
    let prompt = ring_hand_prompt(&state.game);
    state.game.remember_message(&prompt);
    state.modal = Some(message_display_text(&prompt));
}
fn is_item_selection(pending: Pending) -> bool {
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
            | Pending::Identify
            | Pending::CallSelect
            | Pending::WizardCharge
    )
}
fn item_matches_selection(game: &Game, pending: Pending, kind: ItemKind) -> bool {
    match pending {
        Pending::Quaff => kind == ItemKind::Potion,
        Pending::Read => kind == ItemKind::Scroll,
        Pending::Eat => kind == ItemKind::Food,
        Pending::Wield => kind != ItemKind::Armor,
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
        Pending::Drop | Pending::ThrowSelect => true,
        _ => false,
    }
}
fn item_menu_text(game: &Game, pending: Pending, feedback: Option<&str>) -> Option<String> {
    let mut lines = Vec::new();
    for (index, item) in game
        .player
        .inventory
        .iter()
        .enumerate()
        .filter(|(_, item)| item.in_pack)
    {
        if !item_matches_selection(game, pending, item.kind) {
            continue;
        }
        let letter = item.pack_letter.unwrap_or((b'a' + index as u8) as char);
        lines.push(format!("{letter}) {}", game.inventory_name(item, false)));
    }
    if lines.is_empty() {
        return None;
    }
    if let Some(feedback) = feedback {
        lines.insert(0, feedback.into());
    }
    Some(lines.join("\n"))
}
fn select_item_menu(state: &mut State, pending: Pending) -> Option<String> {
    let Some(text) = item_menu_text(&state.game, pending, None) else {
        state.game.message(if state.game.options.terse {
            "⟨ničto:gen⟩ ⟨a:prigodny:město:gen⟩"
        } else {
            "ne ⟨v2:imati⟩ ⟨ničto:gen⟩ ⟨a:prigodny:město:gen⟩"
        });
        state.pending = None;
        state.modal = None;
        state.modal_offset = 0;
        return None;
    };
    state.pending = Some(pending);
    state.game.remember_message("");
    state.modal_offset = 0;
    Some(text)
}
fn retry_invalid_item(state: &mut State, pending: Pending, ch: char) {
    let error = format!("'{}' ne jest pravilny prědmet", control_label(ch));
    state.game.message(&error);
    state.modal = item_menu_text(&state.game, pending, Some(&message_display_text(&error)));
    if state.modal.is_none() {
        // A selection is only pending while eligible items exist; if that ever
        // stops holding, cancel rather than strand a menu-less pending state.
        state.pending = None;
    }
    state.modal_offset = 0;
}
fn wizard_identify_prompt(state: &mut State) -> Option<String> {
    if state.game.player.inventory.is_empty() {
        state
            .game
            .message("ne ⟨v2:imati⟩ v ⟨n:torba:loc⟩ ⟨ničto:gen⟩ za ⟨n:opoznańje:acc⟩");
        None
    } else {
        state.game.pending_identification = None;
        select_item_menu(state, Pending::Identify)
    }
}
fn wizard_charge_prompt(state: &mut State) -> Option<String> {
    if state.game.player.inventory.is_empty() {
        state.game.message("⟨ničto:gen⟩ ne ⟨v2:nositi⟩");
        None
    } else {
        select_item_menu(state, Pending::WizardCharge)
    }
}
fn select_action_menu(
    state: &mut State,
    pending: Pending,
    empty_consumes_turn: bool,
) -> Option<String> {
    if state.game.player.inventory.is_empty() {
        state.game.message("⟨ničto:gen⟩ ne ⟨v2:nositi⟩");
        if empty_consumes_turn {
            state
                .game
                .finish_action(mrzavec::command::CommandResult::TURN);
        }
        None
    } else {
        select_item_menu(state, pending)
    }
}
#[cfg(test)]
fn equipment_text(game: &Game, title: &str, id: Option<u64>) -> String {
    let value = id
        .and_then(|id| game.player.inventory.iter().find(|i| i.id == id))
        .map_or_else(|| speak("⟨ničto:gen⟩"), |item| game.item_name(item));
    format!("{title}\n\n{value}\n\nEscape za izhod")
}
fn current_message(game: &Game, id: Option<u64>, how: &str, where_: Option<&str>) -> String {
    let location = where_
        .map(|where_| format!(" {}", speak(where_)))
        .unwrap_or_default();
    let how = speak(how);
    if let Some(item) = id.and_then(|id| game.player.inventory.iter().find(|item| item.id == id)) {
        let letter = item.pack_letter.unwrap_or('?');
        let prefix = if game.options.terse {
            String::new()
        } else {
            format!("{how} sejčas: ")
        };
        format!(
            "{prefix}{letter}) {}{location}",
            game.inventory_name(item, true)
        )
    } else if game.options.terse {
        format!("{}{location}", speak("⟨ničto:gen⟩"))
    } else {
        format!("ne {how} {}{location}", speak("⟨ničto:gen⟩"))
    }
}
#[cfg(test)]
fn rings_text(game: &Game) -> String {
    let ring = |id: Option<u64>| {
        id.and_then(|id| game.player.inventory.iter().find(|item| item.id == id))
            .map_or_else(|| speak("⟨ničto:gen⟩"), |item| game.item_name(item))
    };
    format!(
        "{}\n\nlěvy: {}\npravy: {}\n\nEscape za izhod",
        speak("⟨n:pŕstenj:nom:pl:U⟩"),
        ring(game.player.rings[0]),
        ring(game.player.rings[1])
    )
}

/// One-line question prompts whose modal text stays visible in the event area
/// (appended after the combined events) instead of replacing the display.
/// Item menus, pagers, and full-screen views are never rendered inline.
fn pending_is_inline_prompt(pending: Pending) -> bool {
    matches!(
        pending,
        Pending::PutRingHand(_)
            | Pending::RemoveRingHand
            | Pending::ThrowDirection(_)
            | Pending::ZapDirection(_)
            | Pending::FightDirection(_)
            | Pending::MoveDirection
            | Pending::TrapDirection
            | Pending::CallText(_)
            | Pending::AutoCall
            | Pending::SaveConfirm
            | Pending::SaveFileText
            | Pending::SaveOverwrite
            | Pending::QuitConfirm
            | Pending::Password
            | Pending::StartupPassword
            | Pending::IdentifyGlyph
            | Pending::Discoveries
            | Pending::SlowDiscoveryPrompt
            | Pending::WizardListType
            | Pending::WizardCreateType
            | Pending::WizardCreateWhich(_)
            | Pending::WizardCreateBlessing(_, _)
            | Pending::WizardCreateGold
    )
}

fn has_inline_event_prompt(state: &State) -> bool {
    state.modal.is_some()
        && state.visible_message.is_some()
        && !state.modal_overlay
        && state.pending.is_some_and(pending_is_inline_prompt)
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
    let footer_visible =
        state.modal.is_none() || state.modal_overlay || has_inline_event_prompt(&state);
    for (cell, children) in cells {
        for child in children.iter() {
            if let Ok((mut text, mut color)) = glyphs.get_mut(child) {
                text.0 = buffer[cell.0].to_string();
                let row = cell.0 / DISPLAY_WIDTH;
                color.0 = if footer_visible
                    && matches!(row, KEYBINDING_FIRST_ROW | KEYBINDING_SECOND_ROW)
                {
                    KEYBINDING_DIM_COLOR
                } else {
                    GLYPH_COLOR
                };
            }
        }
    }
}
fn display(state: &State) -> Vec<char> {
    let mut out = vec![' '; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    let inline_prompt = has_inline_event_prompt(state);
    if let Some(modal) = &state.modal
        && !state.modal_overlay
        && !inline_prompt
    {
        let all_lines: Vec<&str> = modal.lines().collect();
        let explicit_more = all_lines.last() == Some(&" --Dalje--");
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
            write_terminal_text(&mut out, MODAL_MORE_ROW, 0, " --Dalje--", DISPLAY_WIDTH);
        }
        return out;
    }
    let mut event_text = state.visible_message.clone().or_else(|| {
        state
            .game
            .messages
            .last()
            .and_then(|message| event_sentence(message, state.preserve_message_case))
    });
    if inline_prompt && let Some(prompt) = &state.modal {
        let text = event_text.get_or_insert_with(String::new);
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(prompt.trim());
    }
    if let Some(text) = event_text {
        write_event_text(&mut out, &text);
    }
    for screen_y in DUNGEON_FIRST_ROW..STATUS_ROW {
        let dungeon_y = screen_y - DUNGEON_FIRST_ROW + 1;
        for x in 0..DISPLAY_WIDTH {
            out[screen_y * DISPLAY_WIDTH + x] =
                state.game.glyph_at(Pos::new(x as i32, dungeon_y as i32))
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
    let explicit_more = modal.lines().last() == Some(" --Dalje--");
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

fn event_sentence(message: &str, preserve_case: bool) -> Option<String> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    if preserve_case {
        return Some(message.to_owned());
    }
    let mut sentence = message_display_text(message);
    if !sentence
        .chars()
        .last()
        .is_some_and(|ch| matches!(ch, '.' | '!' | '?' | ')' | '"' | '\''))
    {
        sentence.push('.');
    }
    Some(sentence)
}

fn append_event_message(stream: &mut Option<String>, message: &str, preserve_case: bool) {
    let Some(sentence) = event_sentence(message, preserve_case) else {
        *stream = None;
        return;
    };
    let stream = stream.get_or_insert_with(String::new);
    if !stream.is_empty() {
        stream.push(' ');
    }
    stream.push_str(&sentence);
}

fn wrap_terminal_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut x = 0;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(String::new());
            x = 0;
            continue;
        }
        if x >= width {
            if ch == ' ' {
                lines.push(String::new());
                x = 0;
                continue;
            }
            let current = lines.last_mut().unwrap();
            if let Some(space) = current.rfind(' ') {
                let carried = current[space + 1..].to_owned();
                current.truncate(space);
                x = terminal_text_width(&carried);
                lines.push(carried);
            } else {
                lines.push(String::new());
                x = 0;
            }
        }
        lines.last_mut().unwrap().push(ch);
        x = if ch == '\t' { ((x / 8) + 1) * 8 } else { x + 1 };
    }
    lines
}

fn terminal_text_width(text: &str) -> usize {
    text.chars().fold(
        0,
        |x, ch| {
            if ch == '\t' { ((x / 8) + 1) * 8 } else { x + 1 }
        },
    )
}

fn write_event_text(out: &mut [char], text: &str) {
    let lines = wrap_terminal_text(text, DISPLAY_WIDTH);
    let start = lines.len().saturating_sub(EVENT_ROWS);
    for (row, line) in lines[start..].iter().enumerate() {
        write_terminal_text(out, row, 0, line, DISPLAY_WIDTH);
    }
}

fn prepare_messages(mut state: ResMut<State>) {
    collect_messages(&mut state);
}

fn collect_messages(state: &mut State) {
    if state.game.end != mrzavec::game::EndState::Playing && state.modal.is_some() {
        state.message_serial_seen = state.game.message_serial;
        state.visible_message = None;
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
    let messages = state.game.messages[start..].to_vec();
    state.message_serial_seen = state.game.message_serial;
    let preserve_message_case = state.preserve_message_case;
    for message in messages {
        append_event_message(&mut state.visible_message, &message, preserve_message_case);
    }
}

fn status_text(game: &Game) -> String {
    let hunger = ["", "Glad", "Slabosť", "Nemoć"]
        .get(game.hungry_state as usize)
        .copied()
        .unwrap_or("");
    let hp_width = game.player.stats.max_hp.to_string().len();
    format!(
        "Stųp: {}  Zlåto: {:<5}  Hp: {:>width$}({:>width$})  Sila: {:>2}({})  Brȯn: {:<2}  Exp: {}/{}  {}",
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
            "⟨a:prazdny:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩"
        } else {
            "ne ⟨v2:imati⟩ ⟨ničto:gen⟩"
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
            state.modal_overlay = text.lines().count() <= STATUS_ROW;
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
            || speak("⟨a:tvoj:torba:nom:U⟩ ⟨n:torba:nom⟩ jest ⟨a:prazdny:torba:nom⟩."),
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
    format!("{}  --Dalje--", inventory_line(game, index))
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
        state.game.message("⟨ničto:gen⟩ ne ⟨v2:nositi⟩");
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
        "prědmet: "
    } else {
        "kaky prědmet ⟨v2:hotěti⟩ viděti: "
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
    text.push_str(" --Dalje--");
    text
}
fn call_default(game: &Game, item: &mrzavec::item::Item) -> String {
    if let Some(existing) = game.item_guess(item).or(item.label.as_deref()) {
        return existing.into();
    }
    match item.kind {
        ItemKind::Potion => {
            mrzavec::lang::COLOR_ADJ[game.appearances.potion_colors[item.which as usize]].to_string()
        }
        ItemKind::Scroll => game.appearances.scroll_titles[item.which as usize].clone(),
        ItemKind::Ring => {
            mrzavec::lang::STONE_LEX[game.appearances.ring_stones[item.which as usize]]
                .lemma
                .to_string()
        }
        ItemKind::Stick => mrzavec::lang::stick_material_lex(
            game.appearances.stick_is_staff[item.which as usize],
            game.appearances.stick_materials[item.which as usize],
        )
        .lemma
        .to_string(),
        _ => String::new(),
    }
}
fn discovery_lines(game: &mut Game, kind: Option<char>) -> Vec<String> {
    let mut lines = Vec::new();
    let categories = [
        ('!', ItemKind::Potion, POTION_NAMES.len(), &mrzavec::lang::POTION),
        ('?', ItemKind::Scroll, SCROLL_NAMES.len(), &mrzavec::lang::SCROLL),
        ('=', ItemKind::Ring, RING_NAMES.len(), &mrzavec::lang::RING),
        ('/', ItemKind::Stick, STICK_NAMES.len(), &mrzavec::lang::WAND),
    ];
    let mut first = true;
    for (glyph, item_kind, count, category) in categories {
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
            // "o" governs the locative; the plural form comes from lang::decl.
            let loc_pl = mrzavec::lang::decl(
                category,
                interslavic::Case::Loc,
                interslavic::Number::Plural,
            );
            lines.push(if game.options.terse {
                speak(&format!("⟨ničto:gen:U⟩ o {loc_pl}"))
            } else {
                speak(&format!("⟨ničto:gen:U⟩ ne ⟨v2:znati⟩ o {loc_pl}"))
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
    out.push_str(" --Dalje--");
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
            state.modal = Some(format!("{}  --Dalje--", message_display_text(&prompt)));
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
            text.push_str("\n --Dalje--");
            state.modal_overlay = state.game.options.inventory_style
                == mrzavec::game::InventoryStyle::Overwrite
                && lines.len() <= STATUS_ROW;
            state.modal_offset = 0;
            state.pending = Some(Pending::DiscoveryMore);
            state.modal = Some(text);
        }
    }
}
fn discoveries_prompt(game: &Game) -> String {
    if game.options.terse {
        "kaky vid? (* za vse)".into()
    } else {
        speak("za kaky vid ⟨v2:hotěti⟩ spisȯk? (* za vse)")
    }
}
const HELP_ENTRIES: &[(char, &str, bool)] = &[
    ('?', "\tpokazati pomoć", true),
    ('/', "\topoznati znak", true),
    ('h', "\tvlěvo", true),
    ('j', "\tdolu", true),
    ('k', "\tgorě", true),
    ('l', "\tvpravo", true),
    ('y', "\tgorě i vlěvo", true),
    ('u', "\tgorě i vpravo", true),
    ('b', "\tdolu i vlěvo", true),
    ('n', "\tdolu i vpravo", true),
    ('H', "\tběgati vlěvo", false),
    ('J', "\tběgati dolu", false),
    ('K', "\tběgati gorě", false),
    ('L', "\tběgati vpravo", false),
    ('Y', "\tběgati gorě i vlěvo", false),
    ('U', "\tběgati gorě i vpravo", false),
    ('B', "\tběgati dolu i vlěvo", false),
    ('N', "\tběgati dolu i vpravo", false),
    ('\u{8}', "\tběgati vlěvo do ⟨n:prěškoda:gen⟩", false),
    ('\u{a}', "\tběgati dolu do ⟨n:prěškoda:gen⟩", false),
    ('\u{b}', "\tběgati gorě do ⟨n:prěškoda:gen⟩", false),
    ('\u{c}', "\tběgati vpravo do ⟨n:prěškoda:gen⟩", false),
    ('\u{19}', "\tběgati gorě i vlěvo do ⟨n:prěškoda:gen⟩", false),
    ('\u{15}', "\tběgati gorě i vpravo do ⟨n:prěškoda:gen⟩", false),
    ('\u{2}', "\tběgati dolu i vlěvo do ⟨n:prěškoda:gen⟩", false),
    ('\u{e}', "\tběgati dolu i vpravo do ⟨n:prěškoda:gen⟩", false),
    ('\0', "\t<SHIFT><dir>: běgati v ⟨toj:acc:f⟩ ⟨n:stråna:acc⟩", true),
    ('\0', "\t<CTRL><dir>: běgati do ⟨n:prěškoda:gen⟩", true),
    ('f', "<dir>\tboriti sę do ⟨n:smŕť:gen⟩ ili skoro do ⟨n:smŕť:gen⟩", true),
    ('t', "<dir>\tmetnųti něčto", true),
    ('m', "<dir>\titi bez ⟨n:vzęťje:gen⟩ ⟨n:prědmet:gen⟩", true),
    ('z', "<dir>\tužiti žezlo", true),
    ('^', "<dir>\topoznati vid pasti", true),
    ('s', "\tiskati pasť/⟨a:tajny:dveri:nom:pl⟩ ⟨n:dveri:nom:pl⟩", true),
    ('>', "\titi dolu", true),
    ('<', "\titi gorě", true),
    ('.', "\tčekati jedin hod", true),
    (',', "\tvzęti něčto", true),
    ('i', "\tpokazati ⟨n:torba:acc⟩", true),
    ('I', "\tpokazati jedin prědmet", true),
    ('q', "\tpiti napitȯk", true),
    ('r', "\tčitati svitȯk", true),
    ('e', "\tjesti ⟨n:jeda:acc⟩", true),
    ('w', "\tdŕžati orųžje", true),
    ('W', "\tnositi ⟨n:brȯnja:acc⟩", true),
    ('T', "\tsjęti ⟨n:brȯnja:acc⟩", true),
    ('P', "\tnaděti pŕstenj", true),
    ('R', "\tsjęti pŕstenj", true),
    ('d', "\tostaviti prědmet", true),
    ('c', "\tnazvati prědmet", true),
    ('a', "\tpovtoriti ⟨a:poslědnji:komanda:acc⟩ ⟨n:komanda:acc⟩", true),
    (')', "\tpokazati orųžje v ⟨n:rųka:loc⟩", true),
    (']', "\tpokazati ⟨n:brȯnja:acc⟩", true),
    ('=', "\tpokazati ⟨n:pŕstenj:acc:pl⟩", true),
    ('@', "\tpokazati ⟨a:tvoj:stańje:acc⟩ ⟨n:stańje:acc⟩", true),
    ('D', "\tpokazati, čto uže ⟨v2:znati⟩", true),
    ('o', "\tpokazati/měnjati ⟨n:opcija:acc:pl⟩", true),
    ('\u{12}', "\tobnoviti ekran", true),
    ('\u{10}', "\tpovtoriti ⟨a:poslědnji:sȯobčeńje:acc⟩ ⟨n:sȯobčeńje:acc⟩", true),
    ('\u{1b}', "\tanulovati ⟨n:komanda:acc⟩, ^[ jest knopka escape", true),
    ('S', "\tshraniti ⟨n:igra:acc⟩", true),
    ('Q', "\tizhod", true),
    ('!', "\totvoriti shell", true),
    ('F', "<dir>\tboriti sę dokolě někto ne ⟨v3:umrěti⟩", true),
    ('v', "\tpokazati ⟨n:verzija:acc⟩, izdańje, čislo ⟨n:temnica:gen⟩", true),
];

fn help_text() -> String {
    HELP_ENTRIES
        .iter()
        .filter(|(_, _, print)| *print)
        .map(|(ch, description, _)| help_entry_text(*ch, description))
        .collect::<Vec<_>>()
        .join("\n")
}

fn help_entry_text(ch: char, description: &str) -> String {
    let label = if ch == '\0' {
        String::new()
    } else {
        control_label(ch)
    };
    let mut out = String::new();
    let mut column = 0;
    for value in format!("{label}{}", speak(description)).chars() {
        if value == '\t' {
            let next_tab = ((column / 8) + 1) * 8;
            out.extend(std::iter::repeat_n(' ', next_tab - column));
            column = next_tab;
        } else {
            out.push(value);
            column += 1;
        }
    }
    out
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
        mrzavec::lang::phrase(
            &mrzavec::lang::MONSTER_LEX[(ch as u8 - b'A') as usize],
            interslavic::Case::Nom,
            interslavic::Number::Singular,
        )
    } else {
        match ch {
            '|' | '-' => "stěna ⟨n:komnata:gen⟩",
            '*' => "zlåto",
            '%' => "stųpenišče",
            '+' => "dveri",
            '.' => "tlo ⟨n:komnata:gen⟩",
            '@' => "ty",
            '#' => "prohod",
            '^' => "pasť",
            '!' => "napitȯk",
            '?' => "svitȯk",
            ':' => "jeda",
            ')' => "orųžje",
            ' ' => "⟨a:tvŕdy:skala:nom⟩ ⟨n:skala:nom⟩",
            ']' => "brȯnja",
            ',' => "Amulet ⟨n:Jendor:gen⟩",
            '=' => "pŕstenj",
            '/' => "žezlo ili posoh",
            _ => "neznany znak",
        }
        .to_string()
    };
    speak(&format!("'{}': {description}", control_label(ch)))
}
const OPTION_COUNT: usize = 12;
const OPTION_LABELS: [(&str, &str); OPTION_COUNT] = [
    ("⟨a:kråtky:sȯobčeńje:nom:pl:U⟩ ⟨n:sȯobčeńje:nom:pl⟩", "terse"),
    ("Ignorovati pisańje podčas boja", "flush"),
    ("Pokazati ⟨n:pozicija:acc⟩ jedino na ⟨n:konec:loc⟩ ⟨n:běg:gen⟩", "jump"),
    ("Pokazati ⟨pp:osvětliti:n⟩ tlo", "seefloor"),
    ("Slědovati ⟨n:povråt:dat:pl⟩ v ⟨n:prohod:loc:pl⟩", "passgo"),
    ("Pokazati kamenj ⟨n:grob:gen⟩ po ⟨n:smŕť:loc⟩", "tombstone"),
    ("Stiľ ⟨n:torba:gen⟩", "inven"),
    ("Ime", "name"),
    ("Ovoć", "fruit"),
    ("Fajl ⟨n:shrånjeńje:gen⟩", "file"),
    ("Fajl ⟨n:rezultat:gen:pl⟩", "score"),
    ("Fajl ⟨n:zamȯk:gen⟩", "lock"),
];
fn option_value(game: &Game, index: usize) -> String {
    match index {
        0 => if game.options.terse { "Da" } else { "Ne" }.into(),
        1 => if game.options.fight_flush { "Da" } else { "Ne" }.into(),
        2 => if game.options.jump { "Da" } else { "Ne" }.into(),
        3 => if game.options.see_floor { "Da" } else { "Ne" }.into(),
        4 => if game.options.passgo { "Da" } else { "Ne" }.into(),
        5 => if game.options.tombstone { "Da" } else { "Ne" }.into(),
        6 => match game.options.inventory_style {
            mrzavec::game::InventoryStyle::Overwrite => "Prěpisati",
            mrzavec::game::InventoryStyle::Slow => "Pomalo",
            mrzavec::game::InventoryStyle::Clear => "Očistiti",
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
        out.push_str(&format!("{} (\"{name}\"): {value}", speak(prompt)));
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
        "{} --Dalje--",
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
            preserve_message_case: false,
            slow_discovery_lines: Vec::new(),
            message_serial_seen,
            visible_message: None,
            pending: None,
            score_recorded: false,
            input_buffer: String::new(),
            count_prefix: String::new(),
            counted_command: None,
        }
    }

    fn pack_item(id: u64, kind: ItemKind, which: u8, letter: char) -> mrzavec::item::Item {
        let mut item = mrzavec::item::Item::basic(id, kind, which);
        item.pack_letter = Some(letter);
        item
    }

    fn keyboard_app(initial_state: State) -> App {
        let mut app = App::new();
        app.insert_resource(initial_state);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Time::<()>::default());
        app.insert_resource(MovementRepeat::default());
        app.add_message::<AppExit>();
        app.add_systems(Update, keyboard);
        app
    }

    fn press_keys(app: &mut App, keys_to_press: &[KeyCode]) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        let pressed: Vec<_> = keys.get_pressed().copied().collect();
        for key in pressed {
            keys.release(key);
        }
        keys.clear();
        for key in keys_to_press {
            keys.press(*key);
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
        assert_eq!(lines[0], "Kråtke sȯobčeńja (\"terse\"): Da");
        assert_eq!(
            lines[1],
            "Ignorovati pisańje podčas boja (\"flush\"): Ne"
        );
        assert_eq!(
            lines[2],
            "Pokazati pozicijų jedino na koncu běga (\"jump\"): Ne"
        );
        assert_eq!(lines[3], "Pokazati osvětljeno tlo (\"seefloor\"): Da");
        assert_eq!(
            lines[4],
            "Slědovati povråtam v prohodah (\"passgo\"): Ne"
        );
        assert_eq!(
            lines[5],
            "Pokazati kamenj groba po smŕti (\"tombstone\"): Da"
        );
        assert_eq!(lines[6], "Stiľ torby (\"inven\"): Pomalo");
        assert_eq!(lines[7], "Ime (\"name\"): Rodney");
        assert_eq!(lines[8], "Ovoć (\"fruit\"): mango");
        assert!(lines[9].starts_with("Fajl shrånjeńja (\"file\"): "));
        assert!(lines[10].starts_with("Fajl rezultatov (\"score\"): "));
        assert!(lines[11].starts_with("Fajl zamka (\"lock\"): "));
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
        assert!(state.modal.as_deref().unwrap().ends_with(" --Dalje--"));
    }

    #[test]
    fn options_string_entry_can_display_an_empty_replacement_buffer() {
        let mut game = Game::new(102);
        game.options.name = "Old Name".into();

        let initial = options_text(&game, Some(7), None, None);
        let erased = options_text(&game, Some(7), Some(""), None);

        assert!(initial.lines().nth(7).unwrap().ends_with("Old Name"));
        assert_eq!(erased.lines().nth(7).unwrap(), "Ime (\"name\"): ");
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
            "rogue verzija 5.4.5 izdańje 2026-07-17 temnica 3 (chongo was here)"
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
        assert!(text.contains("+0,+0 7 strěl"));
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
            Some("ne imaješ ničego")
        );
        assert!(picky_inventory_prompt(&mut empty).is_none());
        assert_eq!(
            empty.game.messages.last().map(String::as_str),
            Some("ničego ne nosiš")
        );

        let mut single = state(111);
        single.game.player.inventory.truncate(1);
        assert!(picky_inventory_prompt(&mut single).is_none());
        assert!(single.game.messages.last().unwrap().starts_with("a) "));

        let mut multiple = state(112);
        multiple.game.options.terse = true;
        assert_eq!(
            picky_inventory_prompt(&mut multiple).as_deref(),
            Some("Prědmet: ")
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
            Some("ne imaješ ničego")
        );
    }

    #[test]
    fn clear_screen_modal_pages_use_all_twenty_seven_content_rows() {
        let mut state = state(103);
        state.modal = Some(
            (0..32)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let first = display(&state);
        let first_text = display_row(&first, MODAL_MORE_ROW);
        assert!(first_text.starts_with(" --Dalje--"));

        state.modal_offset = MODAL_PAGE_ROWS;
        let second = display(&state);
        let second_text = display_row(&second, 0);
        assert!(second_text.starts_with("line 27"));
    }

    #[test]
    fn normal_display_uses_three_event_rows_then_map_status_and_footer() {
        let mut state = state(104);
        state.visible_message = Some("Hello.".into());
        let player = state.game.player.pos;
        let expected_player_glyph = state.game.glyph_at(player);

        let buffer = display(&state);

        assert_eq!(buffer.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
        assert!(display_row(&buffer, 0).starts_with("Hello"));
        assert!(display_row(&buffer, 1).trim().is_empty());
        assert!(display_row(&buffer, 2).trim().is_empty());
        let player_screen_row = DUNGEON_FIRST_ROW + player.y as usize - 1;
        assert_eq!(
            buffer[player_screen_row * DISPLAY_WIDTH + player.x as usize],
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
                .ends_with("Pomoć ?")
        );
    }

    #[test]
    fn question_mark_key_opens_complete_help_immediately() {
        let mut app = keyboard_app(state(105));
        press_keys(&mut app, &[KeyCode::ShiftLeft, KeyCode::Slash]);

        app.update();

        let state = app.world().resource::<State>();
        assert_eq!(state.pending, Some(Pending::Help));
        assert_eq!(state.modal.as_deref(), Some(help_text().as_str()));
        assert!(!state.modal.as_deref().unwrap().contains("help for"));
        assert!(!state.modal.as_deref().unwrap().contains("* for all"));
        assert!(display_row(&display(state), MODAL_MORE_ROW).starts_with(" --Dalje--"));
    }

    #[test]
    fn help_space_pages_only_when_more_content_exists_and_escape_closes() {
        let initial_state = state(1051);
        let starting_turn = initial_state.game.turn;
        let expected_lines: Vec<_> = HELP_ENTRIES
            .iter()
            .filter(|(_, _, print)| *print)
            .map(|(ch, description, _)| help_entry_text(*ch, description))
            .collect();
        assert!(expected_lines.len() > MODAL_PAGE_ROWS);

        let mut app = keyboard_app(initial_state);
        press_keys(&mut app, &[KeyCode::ShiftLeft, KeyCode::Slash]);
        app.update();

        let first = display(app.world().resource::<State>());
        assert_eq!(
            display_row(&first, 0).trim_end(),
            expected_lines[0].as_str()
        );
        assert_eq!(
            display_row(&first, MODAL_PAGE_ROWS - 1).trim_end(),
            expected_lines[MODAL_PAGE_ROWS - 1].as_str()
        );
        assert!(display_row(&first, MODAL_MORE_ROW).starts_with(" --Dalje--"));

        press_keys(&mut app, &[KeyCode::Space]);
        app.update();

        let state = app.world().resource::<State>();
        assert_eq!(state.modal_offset, MODAL_PAGE_ROWS);
        assert_eq!(state.pending, Some(Pending::Help));
        let second = display(state);
        assert_eq!(
            display_row(&second, 0).trim_end(),
            expected_lines[MODAL_PAGE_ROWS].as_str()
        );
        assert_eq!(
            display_row(&second, expected_lines.len() - MODAL_PAGE_ROWS - 1).trim_end(),
            expected_lines.last().unwrap().as_str()
        );
        assert!(!display_row(&second, MODAL_MORE_ROW).contains("--Dalje--"));

        press_keys(&mut app, &[KeyCode::Space]);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.modal_offset, MODAL_PAGE_ROWS);
        assert_eq!(state.pending, Some(Pending::Help));
        assert!(state.modal.is_some());

        press_keys(&mut app, &[KeyCode::Escape]);
        app.update();
        let state = app.world().resource::<State>();
        assert!(state.pending.is_none());
        assert!(state.modal.is_none());
        assert_eq!(state.modal_offset, 0);
        assert_eq!(state.game.turn, starting_turn);
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
    fn held_wait_uses_the_movement_delay_and_cadence_without_bursts() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut repeat = MovementRepeat::default();
        keys.press(KeyCode::Period);

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
            repeat.update(&keys, Duration::from_millis(1), true),
            Some('.')
        );
        assert_eq!(
            repeat.update(&keys, Duration::from_secs(5), true),
            Some('.')
        );
        assert_eq!(
            repeat.update(
                &keys,
                MOVEMENT_REPEAT_INTERVAL - Duration::from_millis(1),
                true,
            ),
            None
        );
        assert_eq!(
            repeat.update(&keys, Duration::from_millis(1), true),
            Some('.')
        );

        keys.release(KeyCode::Period);
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_INTERVAL, true), None);
        assert_eq!(repeat.key, None);
    }

    #[test]
    fn held_wait_and_movement_switch_without_competing_repeat_state() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut repeat = MovementRepeat::default();
        keys.press(KeyCode::KeyH);
        assert_eq!(repeat.update(&keys, Duration::ZERO, true), None);
        keys.clear();

        keys.press(KeyCode::Period);
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_INTERVAL, true), None);
        assert_eq!(repeat.key, Some(KeyCode::Period));
        keys.clear();
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_DELAY, true), Some('.'));

        keys.press(KeyCode::KeyL);
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_INTERVAL, true), None);
        assert_eq!(repeat.key, Some(KeyCode::KeyL));
        keys.clear();
        assert_eq!(repeat.update(&keys, MOVEMENT_REPEAT_DELAY, true), Some('l'));
    }

    #[test]
    fn keyboard_holds_wait_and_resets_it_for_modifiers_menus_and_release() {
        let initial_state = state(1061);
        let starting_turn = initial_state.game.turn;
        let mut app = keyboard_app(initial_state);
        press_keys(&mut app, &[KeyCode::Period]);

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

        {
            let mut state = app.world_mut().resource_mut::<State>();
            state.pending = Some(Pending::Drop);
            state.modal = Some("a) bulava".into());
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_INTERVAL);
        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 4);
        {
            let mut state = app.world_mut().resource_mut::<State>();
            state.pending = None;
            state.modal = None;
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_INTERVAL);
        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 4);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(MOVEMENT_REPEAT_DELAY - Duration::from_millis(1));
        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 4);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1));
        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 5);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Period);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));
        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 5);
    }

    #[test]
    fn held_wait_repeat_state_is_disabled_during_sleep_and_game_over() {
        let mut sleeping = state(1062);
        sleeping.game.player.conditions.asleep_turns = 2;
        let mut sleeping_app = keyboard_app(sleeping);
        press_keys(&mut sleeping_app, &[KeyCode::Period]);
        sleeping_app.update();
        assert_eq!(sleeping_app.world().resource::<MovementRepeat>().key, None);

        let mut ended = state(1063);
        ended.game.end = mrzavec::game::EndState::Quit;
        ended.score_recorded = true;
        let turn = ended.game.turn;
        let mut ended_app = keyboard_app(ended);
        press_keys(&mut ended_app, &[KeyCode::Period]);
        ended_app.update();
        assert_eq!(ended_app.world().resource::<MovementRepeat>().key, None);
        assert_eq!(ended_app.world().resource::<State>().game.turn, turn);
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
        let mut app = keyboard_app(initial_state);
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
        game.death_cause = Some("drakona".into());
        let text = tombstone_text(&game);
        assert!(text.contains("POČIVAJ"));
        assert!(text.contains("MIRU"));
        assert!(text.contains("Rodney"));
        assert!(text.contains("90 Au"));
        assert!(text.contains("smŕť od"));
        assert!(text.contains("drakona"));
        assert!(text.contains(&current_year().to_string()));
    }

    #[test]
    fn tombstone_renders_genitive_causes_without_article_machinery() {
        let mut game = Game::new(104);
        game.end = mrzavec::game::EndState::Dead;

        game.death_cause = Some("akvatora".into());
        let monster = tombstone_text(&game);
        assert!(monster.contains("smŕť od"));
        assert!(monster.contains("akvatora"));

        game.death_cause = Some("glada".into());
        let starvation = tombstone_text(&game);
        assert!(starvation.contains("glada"));

        // The internal "signal" key stays machine-readable for score.rs but
        // renders as a genitive.
        game.death_cause = Some("signal".into());
        let signal = tombstone_text(&game);
        assert!(signal.contains("signala"));
        assert_eq!(death_cause_gen(&game), "signala");

        game.death_cause = None;
        assert_eq!(death_cause_gen(&game), "Boga");
    }

    #[test]
    fn tombstone_preserves_long_names_instead_of_clipping_them_to_the_art() {
        let mut game = mrzavec::Game::new(2026);
        game.end = mrzavec::game::EndState::Dead;
        game.options.name = "abcdefghijklmnopqrstuvwxyz".into();
        let cause = mrzavec::lang::phrase(
            &mrzavec::lang::MONSTER_LEX[17],
            interslavic::Case::Gen,
            interslavic::Number::Singular,
        );
        game.death_cause = Some(cause.clone());

        let text = tombstone_text(&game);

        assert!(text.contains("abcdefghijklmnopqrstuvwxyz"));
        assert!(text.contains(&cause));
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
        assert!(status.contains("Stųp: 7  Zlåto: 42   "));
        assert!(status.contains("Hp:  9(12)"));
        assert!(status.contains("Sila: 14(16)"));
        assert!(status.ends_with("Slabosť"));
    }

    #[test]
    fn winner_sale_screen_lists_item_worth_and_gold() {
        let mut game = Game::new(100);
        game.player.gold = 321;
        let text = winner_sales_text(&game);
        assert!(text.starts_with("    Cěna  Prědmet"));
        assert!(text.contains("321  Zlåtniky"));
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
        assert!(text.contains("+0,+0 7 strěl"));
        assert!(!text.contains("x7"));
    }

    #[test]
    fn discoveries_can_be_filtered_by_original_object_glyph() {
        let mut game = Game::new(2);
        game.knowledge.potions[0] = true;
        game.knowledge.scrolls[0] = true;
        let potions = discoveries_text(&mut game, Some('!'));
        assert!(potions.contains("napitȯk"));
        assert!(!potions.contains("svitȯk"));
    }

    #[test]
    fn discovery_prompt_has_reference_verbose_and_terse_forms() {
        let mut game = Game::new(108);
        assert_eq!(
            discoveries_prompt(&game),
            "za kaky vid hoćeš spisȯk? (* za vse)"
        );
        game.options.terse = true;
        assert_eq!(discoveries_prompt(&game), "kaky vid? (* za vse)");
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

        assert!(text.contains("«fizzy»"));
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
            format!(
                "Ničego ne znaješ o {}",
                mrzavec::lang::decl(
                    &mrzavec::lang::POTION,
                    interslavic::Case::Loc,
                    interslavic::Number::Plural
                )
            )
        );

        let mut clear = state(201);
        clear.game.options.inventory_style = mrzavec::game::InventoryStyle::Clear;
        clear.game.knowledge.potions[0] = true;
        clear.game.knowledge.potions[1] = true;
        start_discoveries(&mut clear, '!');
        assert_eq!(clear.pending, Some(Pending::DiscoveryMore));
        assert!(!clear.modal_overlay);
        assert!(clear.modal.as_deref().unwrap().ends_with(" --Dalje--"));

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
        assert!(slow.modal.as_deref().unwrap().ends_with("  --Dalje--"));
    }

    #[test]
    fn call_prompt_defaults_to_the_reference_appearance_or_existing_guess() {
        let mut game = Game::new(21);
        let potion = mrzavec::item::Item::basic(1, ItemKind::Potion, 0);
        assert_eq!(
            call_default(&game, &potion),
            mrzavec::lang::COLOR_ADJ[game.appearances.potion_colors[0]]
        );

        game.knowledge.guesses[0] = Some("bubbly".into());
        assert_eq!(call_default(&game, &potion), "bubbly");
    }

    #[test]
    fn automatic_call_prompts_use_the_reference_case_and_terse_wording() {
        let mut state = state(210);
        state.game.pending_call = Some((ItemKind::Potion, 0));
        assert!(show_pending_call(&mut state));
        assert_eq!(state.modal.as_deref(), Some("Kako hoćeš to nazvati? "));

        state.game.options.terse = true;
        state.game.pending_call = Some((ItemKind::Potion, 0));
        assert!(show_pending_call(&mut state));
        assert_eq!(state.modal.as_deref(), Some("Nazvati: "));
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
        let expected_modal = format!("Kako hoćeš to nazvati? {expected}");
        assert_eq!(state.modal.as_deref(), Some(expected_modal.as_str()));
    }

    #[test]
    fn glyph_identification_names_monsters_and_terrain() {
        assert!(identify_glyph_text('D').contains("drakon"));
        assert!(identify_glyph_text('#').contains("prohod"));
    }

    #[test]
    fn complete_help_lists_every_printable_entry_without_truncation() {
        let expected: Vec<_> = HELP_ENTRIES
            .iter()
            .filter(|(_, _, print)| *print)
            .map(|(ch, description, _)| help_entry_text(*ch, description))
            .collect();
        let full = help_text();
        let lines: Vec<_> = full.lines().collect();

        assert_eq!(lines, expected);
        assert_eq!(lines.len(), 49);
        assert!(full.contains("<SHIFT><dir>: běgati v tų strånų"));
        assert!(!full.contains('\t'));
        assert!(full.contains("boriti sę do smŕti ili skoro do smŕti"));
        assert!(full.contains("boriti sę dokolě někto ne umre"));
        assert!(full.contains("otvoriti shell"));
        assert!(full.contains("pokazati verzijų, izdańje, čislo temnice"));
        assert!(!full.contains("Ctrl-Z"));
        assert!(!full.contains("legal no-op"));
        assert!(!full.contains("--Dalje--"));
    }

    #[test]
    fn glyph_identification_uses_unctrl_for_control_characters() {
        assert!(identify_glyph_text('\u{8}').starts_with("'^H': neznany znak"));
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

        assert!(lines[0].starts_with(" --Dalje--"));
        assert_eq!(lines[12].chars().nth(37), Some('+'));
        assert_eq!(lines[12].chars().nth(38), Some('#'));
        assert_eq!(lines[12].chars().nth(39), Some('^'));
        assert_eq!(game.dungeon, before);
    }

    #[test]
    fn wizard_creation_prompts_use_each_reference_subtype_range() {
        assert_eq!(
            wizard_which_prompt(ItemKind::Potion),
            "! (napitȯk) — kako čislo hoćeš? (0-d)"
        );
        assert_eq!(
            wizard_which_prompt(ItemKind::Scroll),
            "? (svitȯk) — kako čislo hoćeš? (0-h)"
        );
        assert_eq!(
            wizard_which_prompt(ItemKind::Weapon),
            ") (orųžje) — kako čislo hoćeš? (0-8)"
        );
        assert_eq!(wizard_kind_count(ItemKind::Ring), 14);
    }

    #[test]
    fn wizard_star_lists_reference_object_probabilities() {
        let mut game = Game::new(108);
        assert_eq!(wizard_list_prompt(&game), "za kaky vid hoćeš spisȯk? ");
        game.options.terse = true;
        assert_eq!(wizard_list_prompt(&game), "kaky vid? ");

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
            Some("ne imaješ v torbě ničego za opoznańje")
        );
        assert!(identify.pending.is_none());

        let mut charge = state(107);
        charge.game.player.inventory.clear();
        assert!(wizard_charge_prompt(&mut charge).is_none());
        assert_eq!(
            charge.game.messages.last().map(String::as_str),
            Some("ničego ne nosiš")
        );
        assert!(charge.pending.is_none());

        let mut identify_menu = state(108);
        identify_menu.game.wizard = true;
        identify_menu.game.player.inventory = vec![pack_item(50_100, ItemKind::Armor, 0, 'k')];
        let menu = wizard_identify_prompt(&mut identify_menu).unwrap();
        assert!(menu.starts_with("k) "));
        assert_eq!(identify_menu.pending, Some(Pending::Identify));
    }

    #[test]
    fn invalid_ring_hand_retries_with_reference_feedback() {
        let mut verbose = state(116);
        retry_ring_hand(&mut verbose);
        assert_eq!(
            verbose.modal.as_deref(),
            Some("Lěva rųka ili prava rųka? ")
        );
        collect_messages(&mut verbose);
        assert_eq!(verbose.visible_message.as_deref(), Some("Prošų, L ili R."));
        assert_eq!(
            verbose.modal.as_deref(),
            Some("Lěva rųka ili prava rųka? ")
        );

        let mut terse = state(117);
        terse.game.options.terse = true;
        retry_ring_hand(&mut terse);
        assert_eq!(terse.modal.as_deref(), Some("Lěvy ili pravy pŕstenj? "));
        collect_messages(&mut terse);
        assert_eq!(terse.visible_message.as_deref(), Some("L ili R."));
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
        assert_eq!(game.item_name(&sword), "dȯlgy meč");
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
        let weapon_text = equipment_text(&game, "Orųžje", Some(weapon_id));
        assert!(weapon_text.contains("dvorųčny meč"));
        assert!(!weapon_text.contains("+3"));

        let ring_id = 90_002;
        let ring = mrzavec::item::Item::basic(ring_id, ItemKind::Ring, 0);
        let stone = mrzavec::lang::STONE_LEX[game.appearances.ring_stones[0]];
        game.player.inventory.push(ring);
        game.player.rings[0] = Some(ring_id);
        let ring_text = rings_text(&game);
        let ring_head = mrzavec::lang::decl(
            &mrzavec::lang::RING,
            interslavic::Case::Nom,
            interslavic::Number::Singular,
        );
        assert!(ring_text.contains(&format!(
            "{ring_head} {}",
            mrzavec::lang::material_of(&stone)
        )));
        assert!(!ring_text.contains(&ring_id.to_string()));
    }

    #[test]
    fn current_equipment_messages_match_the_nonblocking_reference_forms() {
        let mut game = Game::new(103);
        let weapon = game.player.weapon.unwrap();
        assert!(
            current_message(&game, Some(weapon), "dŕžiš", None).starts_with("dŕžiš sejčas: ")
        );
        assert_eq!(
            current_message(&game, None, "nosiš", Some("na lěvoj rųkě")),
            "ne nosiš ničego na lěvoj rųkě"
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
            current_message(&game, Some(weapon), "dŕžiš", None)
                .starts_with(&format!("{letter}) "))
        );
        assert_eq!(
            current_message(&game, None, "nosiš", Some("(R)")),
            "ničego (R)"
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
            Some("uže ne imaješ togo")
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
            Some("ne imaješ v torbě ničego za opoznańje")
        );
    }

    #[test]
    fn counted_multistep_command_prompts_for_each_iteration() {
        let mut state = state(4);
        state.game.player.inventory = vec![pack_item(53_000, ItemKind::Potion, 0, 'a')];
        state.counted_command = Some((Command::Quaff, 2));

        continue_counted_command(&mut state);

        assert_eq!(state.pending, Some(Pending::Quaff));
        assert_eq!(state.counted_command, Some((Command::Quaff, 1)));
        assert!(state.modal.as_deref().unwrap().starts_with("a) "));
        assert!(!state.modal.as_deref().unwrap().contains("* for list"));
    }

    #[test]
    fn final_counted_prompt_becomes_the_repeat_command_at_reference_time() {
        let mut state = state(40);
        state.game.player.inventory = vec![pack_item(53_001, ItemKind::Potion, 0, 'a')];
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
                .filter(|message| message.as_str() == "nepraviľna komanda 'C'")
                .count(),
            1
        );
    }

    #[test]
    fn empty_pack_item_menus_preserve_reference_turn_rules() {
        let mut consuming = state(30);
        consuming.game.player.inventory.clear();
        let turn = consuming.game.turn;
        assert!(select_action_menu(&mut consuming, Pending::Read, true).is_none());
        assert_eq!(consuming.game.turn, turn + 1);
        assert_eq!(
            consuming.game.messages.last().map(String::as_str),
            Some("ničego ne nosiš")
        );

        let mut free = state(31);
        free.game.player.inventory.clear();
        let turn = free.game.turn;
        assert!(select_action_menu(&mut free, Pending::Wield, false).is_none());
        assert_eq!(free.game.turn, turn);
    }

    #[test]
    fn invalid_item_selection_keeps_the_menu_with_unctrl_feedback() {
        let mut state = state(114);
        state.game.player.inventory = vec![pack_item(52_000, ItemKind::Potion, 0, 'a')];
        state.modal = select_item_menu(&mut state, Pending::Quaff);
        retry_invalid_item(&mut state, Pending::Quaff, '\u{8}');
        let modal = state.modal.as_deref().unwrap();
        assert!(
            modal
                .lines()
                .next()
                .unwrap()
                .contains("'^H' ne jest pravilny prědmet")
        );
        assert!(modal.contains("a) "));
        assert!(!modal.contains("* for list"));
        collect_messages(&mut state);
        assert_eq!(
            state.visible_message.as_deref(),
            Some("'^H' ne jest pravilny prědmet.")
        );
        assert!(state.modal.as_deref().unwrap().contains("a) "));
        assert_eq!(state.game.recall_message, "'^H' ne jest pravilny prědmet");
        assert!(!is_item_selection(Pending::Options(0)));
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
        assert_eq!(message_display_text("nahodiš zlåto"), "Nahodiš zlåto");
        assert_eq!(message_display_text("a) napitȯk"), "a) napitȯk");
        assert_eq!(message_display_text("'^H': neznany"), "'^H': neznany");

        let mut state = state(1160);
        let displayed = remembered_prompt(&mut state, "v ktorų strånų? ");
        assert_eq!(displayed, "V ktorų strånų? ");
        assert_eq!(state.game.recall_message, "v ktorų strånų? ");

        state.game.message("h\tvlěvo");
        state.preserve_message_case = true;
        let buffer = display(&state);
        assert_eq!(
            buffer.into_iter().take(13).collect::<String>(),
            "h       vlěvo"
        );
    }

    #[test]
    fn event_sentences_normalize_capitalization_punctuation_and_spacing() {
        let mut stream = None;
        append_event_message(&mut stream, "  udarjaješ orka  ", false);
        append_event_message(&mut stream, "ork tę ne udarjaje!", false);
        append_event_message(&mut stream, "to uže ima tȯčkų.", false);

        assert_eq!(
            stream.as_deref(),
            Some("Udarjaješ orka. Ork tę ne udarjaje! To uže ima tȯčkų.")
        );
    }

    #[test]
    fn event_sentences_leave_parenthesized_and_preserved_case_endings_alone() {
        assert_eq!(
            event_sentence("imaješ sejčas 25 zlåtnikov (cěna 25)", false).as_deref(),
            Some("Imaješ sejčas 25 zlåtnikov (cěna 25)")
        );
        assert_eq!(
            event_sentence("rogue verzija 5.4.5 izdańje", true).as_deref(),
            Some("rogue verzija 5.4.5 izdańje")
        );
    }

    #[test]
    fn event_wrapping_breaks_at_word_boundaries() {
        assert_eq!(
            wrap_terminal_text("You hit the orc. The orc misses!", 12),
            ["You hit the", "orc. The orc", "misses!"]
        );
        assert_eq!(
            wrap_terminal_text(&"A".repeat(30), 12),
            ["A".repeat(12), "A".repeat(12), "A".repeat(6)]
        );
    }

    #[test]
    fn retry_with_no_eligible_items_cancels_instead_of_stranding_the_selection() {
        let mut state = state(324);
        state.game.player.inventory.clear();
        state.pending = Some(Pending::Quaff);
        retry_invalid_item(&mut state, Pending::Quaff, 'x');
        assert!(state.pending.is_none());
        assert!(state.modal.is_none());
    }

    #[test]
    fn single_line_item_menus_render_full_screen_rather_than_inline() {
        let mut state = state(325);
        state.game.player.inventory = vec![pack_item(60_000, ItemKind::Potion, 0, 'a')];
        state.modal = select_item_menu(&mut state, Pending::Quaff);
        state.visible_message = Some("Vse sę vrti.".into());
        assert!(!has_inline_event_prompt(&state));
        let buffer = display(&state);
        assert!(display_row(&buffer, 0).starts_with("a) "));
    }

    #[test]
    fn consecutive_messages_render_together_without_more_or_space_waiting() {
        let mut sequence = state(1161);
        sequence.game.message("prva věsť");
        sequence.game.message("druga věsť!");
        collect_messages(&mut sequence);

        assert_eq!(
            sequence.visible_message.as_deref(),
            Some("Prva věsť. Druga věsť!")
        );
        let top = (0..EVENT_ROWS)
            .map(|row| display_row(&display(&sequence), row))
            .collect::<String>();
        assert!(top.starts_with("Prva věsť. Druga věsť!"));
        assert!(!top.contains("--Dalje--"));
    }

    #[test]
    fn combined_events_do_not_block_the_next_gameplay_command() {
        let mut initial_state = state(1162);
        initial_state.game.monsters.clear();
        initial_state.game.message("first event");
        initial_state.game.message("second event");
        collect_messages(&mut initial_state);
        let starting_turn = initial_state.game.turn;
        let mut app = keyboard_app(initial_state);
        press_keys(&mut app, &[KeyCode::Period]);

        app.update();

        assert_eq!(app.world().resource::<State>().game.turn, starting_turn + 1);
    }

    #[test]
    fn three_line_event_area_wraps_and_retains_the_newest_content() {
        let mut state = state(1162);
        state.visible_message = Some(format!(
            "{}{}{}{}!",
            "A".repeat(DISPLAY_WIDTH),
            "B".repeat(DISPLAY_WIDTH),
            "C".repeat(DISPLAY_WIDTH),
            "D".repeat(DISPLAY_WIDTH - 1),
        ));

        let buffer = display(&state);
        assert_eq!(display_row(&buffer, 0), "B".repeat(DISPLAY_WIDTH));
        assert_eq!(display_row(&buffer, 1), "C".repeat(DISPLAY_WIDTH));
        assert_eq!(
            display_row(&buffer, 2),
            format!("{}!", "D".repeat(DISPLAY_WIDTH - 1))
        );
    }

    #[test]
    fn pending_prompt_stays_visible_after_combined_events() {
        let mut prompted = state(1163);
        prompted.pending = Some(Pending::IdentifyGlyph);
        prompted.modal = Some("Čto hoćeš opoznati? ".into());
        prompted.game.message("něčto sę stalo");
        prompted.game.message("vse sę vrti");
        collect_messages(&mut prompted);

        assert_eq!(prompted.modal.as_deref(), Some("Čto hoćeš opoznati? "));
        assert_eq!(prompted.pending, Some(Pending::IdentifyGlyph));
        let buffer = display(&prompted);
        let top = (0..EVENT_ROWS)
            .map(|row| display_row(&buffer, row))
            .collect::<String>();
        assert!(top.starts_with("Něčto sę stalo. Vse sę vrti. Čto hoćeš opoznati?"));
        assert!(!top.contains("--Dalje--"));

        let mut app = keyboard_app(prompted);
        press_keys(&mut app, &[KeyCode::ShiftLeft, KeyCode::KeyD]);

        app.update();

        let prompted = app.world().resource::<State>();
        assert!(prompted.pending.is_none());
        assert!(prompted.modal.is_none());
        assert!(prompted.game.messages.last().unwrap().contains("drakon"));
    }

    #[test]
    fn every_item_action_menu_is_immediate_and_correctly_filtered() {
        let mut base = state(115);
        base.game.player.inventory = vec![
            pack_item(50_000, ItemKind::Potion, 0, 'a'),
            pack_item(50_001, ItemKind::Scroll, 0, 'b'),
            pack_item(50_002, ItemKind::Food, 0, 'c'),
            pack_item(50_003, ItemKind::Weapon, 0, 'd'),
            pack_item(50_004, ItemKind::Armor, 0, 'e'),
            pack_item(50_005, ItemKind::Ring, 0, 'f'),
            pack_item(50_006, ItemKind::Stick, 0, 'g'),
            pack_item(50_007, ItemKind::Amulet, 0, 'h'),
        ];
        let cases = [
            (Pending::Quaff, "a)", vec!["b)", "g)"]),
            (Pending::Read, "b)", vec!["a)", "g)"]),
            (Pending::Eat, "c)", vec!["a)", "g)"]),
            (Pending::Wield, "a)", vec!["e)"]),
            (Pending::Wear, "e)", vec!["d)", "f)"]),
            (Pending::PutRing, "f)", vec!["e)", "g)"]),
            (Pending::ThrowSelect, "a)", vec![]),
            (Pending::ZapSelect, "g)", vec!["a)", "d)"]),
            (Pending::WizardCharge, "g)", vec!["a)", "d)"]),
        ];

        for (pending, expected, excluded) in cases {
            let mut state = state(1151);
            state.game = base.game.clone();
            let menu = select_item_menu(&mut state, pending).unwrap();
            assert!(menu.contains(expected), "{pending:?}: {menu}");
            for letter in excluded {
                assert!(!menu.contains(letter), "{pending:?}: {menu}");
            }
            assert_eq!(state.pending, Some(pending));
            assert!(!menu.contains("* for list"));
            assert!(!menu.contains("--Dalje--"));
        }

        let mut drop_state = state(1152);
        drop_state.game = base.game.clone();
        let drop_menu = select_item_menu(&mut drop_state, Pending::Drop).unwrap();
        assert_eq!(drop_menu.lines().count(), 8);

        let mut wield_state = state(11521);
        wield_state.game = base.game.clone();
        let wield_menu = select_item_menu(&mut wield_state, Pending::Wield).unwrap();
        assert_eq!(wield_menu.lines().count(), 7);
        assert!(!wield_menu.contains("e)"));

        let mut throw_state = state(11522);
        throw_state.game = base.game.clone();
        let throw_menu = select_item_menu(&mut throw_state, Pending::ThrowSelect).unwrap();
        assert_eq!(throw_menu.lines().count(), 8);

        let mut call_state = state(1153);
        call_state.game = base.game.clone();
        let call_menu = select_item_menu(&mut call_state, Pending::CallSelect).unwrap();
        assert!(call_menu.contains("a)"));
        assert!(call_menu.contains("g)"));
        assert!(!call_menu.contains("c)"));
        assert!(!call_menu.contains("h)"));

        let mut identify_state = state(1154);
        identify_state.game = base.game;
        identify_state.game.pending_identification = Some(mrzavec::game::IdentifyKind::Potion);
        let identify_menu = select_item_menu(&mut identify_state, Pending::Identify).unwrap();
        assert!(identify_menu.contains("a)"));
        assert_eq!(identify_menu.lines().count(), 1);
    }

    #[test]
    fn single_item_menu_stays_open_and_inappropriate_nonempty_pack_is_free() {
        let mut single = state(1155);
        single.game.player.inventory = vec![pack_item(51_000, ItemKind::Potion, 0, 'm')];
        let menu = select_action_menu(&mut single, Pending::Quaff, true).unwrap();
        assert_eq!(menu.lines().count(), 1);
        assert!(menu.starts_with("m) "));
        assert_eq!(single.pending, Some(Pending::Quaff));

        let mut inappropriate = state(1156);
        inappropriate.game.player.inventory = vec![pack_item(51_001, ItemKind::Stick, 0, 's')];
        let turn = inappropriate.game.turn;
        assert!(select_action_menu(&mut inappropriate, Pending::Quaff, true).is_none());
        assert_eq!(inappropriate.game.turn, turn);
        assert_eq!(
            inappropriate.game.messages.last().map(String::as_str),
            Some("ne imaješ ničego prigodnogo")
        );
        assert!(inappropriate.pending.is_none());
    }

    #[test]
    fn normal_item_commands_open_their_filtered_menus_on_the_first_keypress() {
        let cases: &[(&[KeyCode], Pending, &str)] = &[
            (&[KeyCode::KeyQ], Pending::Quaff, "a)"),
            (&[KeyCode::KeyR], Pending::Read, "b)"),
            (&[KeyCode::KeyE], Pending::Eat, "c)"),
            (&[KeyCode::KeyW], Pending::Wield, "d)"),
            (&[KeyCode::ShiftLeft, KeyCode::KeyW], Pending::Wear, "e)"),
            (&[KeyCode::ShiftLeft, KeyCode::KeyP], Pending::PutRing, "f)"),
            (&[KeyCode::KeyD], Pending::Drop, "a)"),
            (&[KeyCode::KeyT], Pending::ThrowSelect, "d)"),
            (&[KeyCode::KeyZ], Pending::ZapSelect, "g)"),
            (&[KeyCode::KeyC], Pending::CallSelect, "a)"),
        ];

        for (keys, expected_pending, expected_item) in cases {
            let mut initial_state = state(1157);
            initial_state.game.player.inventory = vec![
                pack_item(55_000, ItemKind::Potion, 0, 'a'),
                pack_item(55_001, ItemKind::Scroll, 0, 'b'),
                pack_item(55_002, ItemKind::Food, 0, 'c'),
                pack_item(55_003, ItemKind::Weapon, 0, 'd'),
                pack_item(55_004, ItemKind::Armor, 0, 'e'),
                pack_item(55_005, ItemKind::Ring, 0, 'f'),
                pack_item(55_006, ItemKind::Stick, 0, 'g'),
            ];
            let mut app = keyboard_app(initial_state);
            press_keys(&mut app, keys);

            app.update();

            let state = app.world().resource::<State>();
            assert_eq!(state.pending, Some(*expected_pending), "{keys:?}");
            let menu = state.modal.as_deref().unwrap();
            assert!(menu.contains(expected_item), "{keys:?}: {menu}");
            assert!(!menu.contains("* for list"), "{keys:?}: {menu}");
        }
    }

    #[test]
    fn item_menu_letters_select_and_escape_or_invalid_letters_do_not_act() {
        let id = 56_000;
        let mut initial_state = state(1158);
        let mut potion = pack_item(id, ItemKind::Potion, 0, 'a');
        potion.count = 2;
        initial_state.game.player.inventory = vec![potion];
        let starting_turn = initial_state.game.turn;
        let mut app = keyboard_app(initial_state);
        press_keys(&mut app, &[KeyCode::KeyQ]);
        app.update();

        press_keys(&mut app, &[KeyCode::KeyZ]);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.pending, Some(Pending::Quaff));
        assert!(
            state
                .modal
                .as_deref()
                .unwrap()
                .contains("'z' ne jest pravilny prědmet")
        );
        assert!(state.modal.as_deref().unwrap().contains("a) "));
        assert_eq!(state.game.turn, starting_turn);

        press_keys(&mut app, &[KeyCode::Escape]);
        app.update();
        let state = app.world().resource::<State>();
        assert!(state.pending.is_none());
        assert!(state.modal.is_none());
        assert_eq!(state.game.player.inventory[0].count, 2);
        assert_eq!(state.game.turn, starting_turn);

        press_keys(&mut app, &[KeyCode::KeyQ]);
        app.update();
        press_keys(&mut app, &[KeyCode::KeyA]);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.game.player.inventory[0].count, 1);
        assert_eq!(state.game.last_item, Some(id));
        assert_eq!(state.game.turn, starting_turn + 1);
    }

    #[test]
    fn throw_and_zap_select_items_before_requesting_and_using_a_direction() {
        let weapon_id = 57_000;
        let mut throw_state = state(1159);
        throw_state.game.monsters.clear();
        throw_state.game.player.inventory = vec![pack_item(weapon_id, ItemKind::Weapon, 0, 'a')];
        let throw_turn = throw_state.game.turn;
        let mut throw_app = keyboard_app(throw_state);
        press_keys(&mut throw_app, &[KeyCode::KeyT]);
        throw_app.update();
        assert_eq!(
            throw_app.world().resource::<State>().pending,
            Some(Pending::ThrowSelect)
        );
        press_keys(&mut throw_app, &[KeyCode::KeyA]);
        throw_app.update();
        let throw_direction = throw_app.world().resource::<State>();
        assert_eq!(
            throw_direction.pending,
            Some(Pending::ThrowDirection(weapon_id))
        );
        assert_eq!(throw_direction.modal.as_deref(), Some("V ktorų strånų? "));
        press_keys(&mut throw_app, &[KeyCode::KeyH]);
        throw_app.update();
        let throw_result = throw_app.world().resource::<State>();
        assert!(throw_result.pending.is_none());
        assert_eq!(
            throw_result.game.last_direction,
            Some(mrzavec::command::Direction::Left)
        );
        assert_eq!(throw_result.game.turn, throw_turn + 1);

        let stick_id = 57_001;
        let mut zap_state = state(1160);
        let mut stick = pack_item(stick_id, ItemKind::Stick, 0, 'b');
        stick.charges = 2;
        zap_state.game.player.inventory = vec![stick];
        let zap_turn = zap_state.game.turn;
        let mut zap_app = keyboard_app(zap_state);
        press_keys(&mut zap_app, &[KeyCode::KeyZ]);
        zap_app.update();
        assert_eq!(
            zap_app.world().resource::<State>().pending,
            Some(Pending::ZapSelect)
        );
        press_keys(&mut zap_app, &[KeyCode::KeyB]);
        zap_app.update();
        assert_eq!(
            zap_app.world().resource::<State>().pending,
            Some(Pending::ZapDirection(stick_id))
        );
        press_keys(&mut zap_app, &[KeyCode::KeyH]);
        zap_app.update();
        let state = zap_app.world().resource::<State>();
        assert!(state.pending.is_none());
        assert_eq!(state.game.turn, zap_turn + 1);
        assert_eq!(state.game.player.inventory[0].charges, 1);
    }

    #[test]
    fn ring_call_identify_and_wizard_charge_menus_enter_their_next_states() {
        let ring_id = 58_000;
        let mut ring_state = state(1161);
        ring_state.game.player.inventory = vec![pack_item(ring_id, ItemKind::Ring, 0, 'a')];
        ring_state.game.player.rings = [None, None];
        ring_state.modal = select_item_menu(&mut ring_state, Pending::PutRing);
        let mut ring_app = keyboard_app(ring_state);
        press_keys(&mut ring_app, &[KeyCode::KeyA]);
        ring_app.update();
        let ring_result = ring_app.world().resource::<State>();
        assert_eq!(ring_result.pending, Some(Pending::PutRingHand(ring_id)));
        assert!(ring_result.modal.as_deref().unwrap().contains("rųka"));

        let call_id = 58_001;
        let mut call_state = state(1162);
        call_state.game.player.inventory = vec![pack_item(call_id, ItemKind::Potion, 0, 'a')];
        call_state.modal = select_item_menu(&mut call_state, Pending::CallSelect);
        let mut call_app = keyboard_app(call_state);
        press_keys(&mut call_app, &[KeyCode::KeyA]);
        call_app.update();
        let call_result = call_app.world().resource::<State>();
        assert_eq!(call_result.pending, Some(Pending::CallText(call_id)));
        assert!(call_result.modal.as_deref().unwrap().contains("nazvati"));

        let identify_id = 58_002;
        let mut identify_state = state(1163);
        identify_state.game.player.inventory =
            vec![pack_item(identify_id, ItemKind::Potion, 0, 'a')];
        identify_state.game.pending_identification = Some(mrzavec::game::IdentifyKind::Potion);
        assert!(show_pending_identification(&mut identify_state));
        let mut identify_app = keyboard_app(identify_state);
        press_keys(&mut identify_app, &[KeyCode::KeyA]);
        identify_app.update();
        let identify_result = identify_app.world().resource::<State>();
        assert!(identify_result.pending.is_none());
        assert!(identify_result.game.player.inventory[0].known);
        assert!(identify_result.game.pending_identification.is_none());

        let stick_id = 58_003;
        let mut charge_state = state(1164);
        charge_state.game.wizard = true;
        charge_state.game.player.inventory = vec![pack_item(stick_id, ItemKind::Stick, 0, 'a')];
        charge_state.modal = wizard_charge_prompt(&mut charge_state);
        let mut charge_app = keyboard_app(charge_state);
        press_keys(&mut charge_app, &[KeyCode::KeyA]);
        charge_app.update();
        let state = charge_app.world().resource::<State>();
        assert!(state.pending.is_none());
        assert_eq!(state.game.player.inventory[0].charges, 10_000);
    }

    #[test]
    fn save_confirmation_uses_the_configured_reference_filename() {
        let mut game = Game::new(34);
        game.options.save_file = "/tmp/rodney.save".into();
        assert_eq!(
            save_confirmation(&game),
            "shraniti fajl (/tmp/rodney.save)? "
        );
    }

    #[test]
    fn quit_confirmation_accepts_only_y_and_every_other_key_cancels() {
        for ch in ['n', 'x', '\u{1b}', ' '] {
            let mut state = state(340);
            state.pending = Some(Pending::QuitConfirm);
            state.modal = Some("istinno li ⟨v2:izhoditi⟩?".into());
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
    fn counted_throw_starts_with_the_item_menu() {
        let mut state = state(32);
        state.game.player.inventory = vec![pack_item(54_000, ItemKind::Weapon, 0, 'a')];
        state.counted_command = Some((Command::Throw, 1));

        continue_counted_command(&mut state);

        assert_eq!(state.pending, Some(Pending::ThrowSelect));
        assert!(state.modal.as_deref().unwrap().starts_with("a) "));
        assert_eq!(state.game.recall_message, "");
    }

    #[test]
    fn counted_and_repeated_throws_preserve_saved_item_and_direction_state() {
        let id = 59_000;
        let mut weapon = pack_item(id, ItemKind::Weapon, 0, 'a');
        weapon.count = 2;
        let mut counted = state(321);
        counted.game.monsters.clear();
        counted.game.player.inventory = vec![weapon.clone()];
        counted.counted_command = Some((Command::Throw, 2));
        continue_counted_command(&mut counted);
        let mut app = keyboard_app(counted);
        press_keys(&mut app, &[KeyCode::KeyA]);
        app.update();
        press_keys(&mut app, &[KeyCode::KeyH]);
        app.update();
        let counted_result = app.world().resource::<State>();
        assert_eq!(counted_result.pending, Some(Pending::ThrowSelect));
        assert!(counted_result.counted_command.is_none());
        assert_eq!(counted_result.game.player.inventory[0].count, 1);
        assert_eq!(counted_result.game.last_item, None);
        assert_eq!(counted_result.game.previous_item, Some(id));
        assert_eq!(counted_result.game.last_direction, None);
        assert_eq!(
            counted_result.game.previous_direction,
            Some(mrzavec::command::Direction::Left)
        );

        let mut repeated = state(322);
        repeated.game.monsters.clear();
        repeated.game.player.inventory = vec![weapon];
        repeated.game.last_item = Some(id);
        repeated.game.last_direction = Some(mrzavec::command::Direction::Left);
        let turn = repeated.game.turn;
        assert!(repeat_selected_command(&mut repeated, Command::Throw));
        assert_eq!(repeated.game.player.inventory[0].count, 1);
        assert_eq!(repeated.game.turn, turn + 1);
        assert!(repeated.pending.is_none());
    }

    #[test]
    fn item_menu_space_pages_then_retries_like_any_invalid_key() {
        let id = 59_001;
        let mut potion = pack_item(id, ItemKind::Potion, 0, 'a');
        potion.count = 2;
        let mut initial_state = state(323);
        initial_state.game.player.inventory = vec![potion];
        initial_state.pending = Some(Pending::Quaff);
        initial_state.modal = Some(
            (0..32)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut app = keyboard_app(initial_state);
        press_keys(&mut app, &[KeyCode::Space]);
        app.update();
        assert_eq!(
            app.world().resource::<State>().modal_offset,
            MODAL_PAGE_ROWS
        );

        press_keys(&mut app, &[KeyCode::Space]);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.modal_offset, 0);
        assert_eq!(state.pending, Some(Pending::Quaff));
        let modal = state.modal.as_deref().unwrap();
        assert!(modal.contains("' ' ne jest pravilny prědmet"));
        assert!(modal.contains("a) "));

        press_keys(&mut app, &[KeyCode::KeyA]);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.game.player.inventory[0].count, 1);
        assert_eq!(state.game.last_item, Some(id));
        assert!(state.pending.is_none());
    }

    #[test]
    fn direction_prompts_preserve_the_reference_forms_with_the_terse_typo_fixed() {
        let mut state = state(2070);
        assert_eq!(
            direction_prompt(&mut state, Pending::ZapDirection(1)).as_deref(),
            Some("V ktorų strånų? ")
        );
        assert_eq!(state.game.recall_message, "v ktorų strånų? ");

        state.game.options.terse = true;
        assert_eq!(
            direction_prompt(&mut state, Pending::ZapDirection(1)).as_deref(),
            Some("Strana: ")
        );
        assert_eq!(state.game.recall_message, "strana: ");
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
            Pending::ThrowDirection(1),
            Pending::ZapDirection(1),
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
