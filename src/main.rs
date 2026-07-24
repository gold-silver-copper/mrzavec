use bevy::{
    ecs::system::SystemParam,
    input::touch::Touches,
    prelude::*,
    text::LineHeight,
    ui::FocusPolicy,
    window::{PrimaryWindow, WindowResizeConstraints, WindowResolution},
};
use mrzavec::help::{
    COMMANDS_LABEL, CONTEXT_OPTIONS_LABEL, CommandCategory, DockImportance, HELP_ENTRIES, HelpEntry,
};
use mrzavec::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, DUNGEON_FIRST_ROW, EVENT_ROWS, Game, STATUS_ROW,
    command::{Command, Direction, WizardCommand, parse},
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

const CELL_W: f32 = 12.0;
const CELL_H: f32 = 24.0;
const FONT_SIZE: f32 = 20.0;
const DOCK_BUTTON_HEIGHT: f32 = 48.0;
const DOCK_NAVIGATION_HEIGHT: f32 = 44.0;
const DOCK_HEADER_HEIGHT: f32 = 32.0;
const DOCK_GAP: f32 = 4.0;
const DOCK_MIN_BUTTON_WIDTH: f32 = 72.0;
const DOCK_MAX_PROMPT_COLUMNS: usize = 5;
const PALETTE_PAGE_SIZE: usize = 4;
const PROMPT_CHOICE_PAGE_SIZE: usize = 6;
const MODAL_MORE_ROW: usize = DISPLAY_HEIGHT - 1;
const MODAL_PAGE_ROWS: usize = MODAL_MORE_ROW;
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
const DOCK_BACKGROUND_COLOR: Color = Color::BLACK;
const DOCK_BUTTON_COLOR: Color = Color::BLACK;
const DOCK_BUTTON_HOVERED_COLOR: Color = Color::srgb(0.16, 0.16, 0.15);
const DOCK_BUTTON_PRESSED_COLOR: Color = GLYPH_COLOR;
const DOCK_BORDER_COLOR: Color = Color::srgb(0.48, 0.48, 0.45);
const DOCK_DIVIDER_COLOR: Color = Color::srgb(0.24, 0.24, 0.22);
const DOCK_DISABLED_COLOR: Color = Color::srgb(0.36, 0.36, 0.34);
const DOCK_DISABLED_BORDER_COLOR: Color = Color::srgb(0.22, 0.22, 0.21);
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

fn remembered_inline_prompt(state: &mut State, text: impl Into<String>) -> Modal {
    let text = text.into();
    state.game.remember_message(&text);
    Modal::inline_prompt(message_display_text(&text))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalPresentation {
    InlinePrompt,
    Overlay,
    FullScreen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Modal {
    text: String,
    presentation: ModalPresentation,
    offset: usize,
}

impl Modal {
    fn inline_prompt(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            presentation: ModalPresentation::InlinePrompt,
            offset: 0,
        }
    }

    fn overlay(text: impl Into<String>) -> Self {
        let text = text.into();
        debug_assert!(
            text.lines().count() <= STATUS_ROW,
            "overlay content must fit above the status row"
        );
        Self {
            text,
            presentation: ModalPresentation::Overlay,
            offset: 0,
        }
    }

    fn event_overlay_or_full_screen(text: impl Into<String>) -> Self {
        let text = text.into();
        if text.lines().count() <= STATUS_ROW {
            Self::overlay(text)
        } else {
            Self::full_screen(text)
        }
    }

    fn full_screen(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            presentation: ModalPresentation::FullScreen,
            offset: 0,
        }
    }
}

impl std::ops::Deref for Modal {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

#[derive(Resource)]
struct State {
    game: Game,
    modal: Option<Modal>,
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
#[derive(Component)]
struct GameRoot;
#[derive(Component)]
struct TerminalViewport;
#[derive(Component)]
struct TerminalGrid;
#[derive(Component)]
struct DockRoot;
#[derive(Component)]
struct PaletteOverlay;

type LayoutNodeQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        Option<&'static TerminalViewport>,
        Option<&'static TerminalGrid>,
        Option<&'static DockRoot>,
    ),
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockAction {
    Command(char),
    OpenPalette,
    OpenContextActions,
    OpenCategory(CommandCategory),
    PreviousPage,
    NextPage,
    PreviousPromptPage,
    NextPromptPage,
    OptionRow(usize),
    BackToCategories,
    ClosePalette,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockButtonLayout {
    Compact,
    FullWidth,
    Navigation,
    Spacer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockButtonTone {
    Normal,
    Urgent,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockButtonSpec {
    action: DockAction,
    label: String,
    layout: DockButtonLayout,
    tone: DockButtonTone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RankedDockCommand {
    command: char,
    importance: DockImportance,
    priority: u8,
    declaration_order: usize,
}

#[derive(Component)]
struct DockButton {
    action: DockAction,
    tone: DockButtonTone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DockMode {
    #[default]
    Gameplay,
    ContextActions {
        page: usize,
    },
    Categories {
        page: usize,
    },
    Category {
        category: CommandCategory,
        page: usize,
    },
}

#[derive(Resource, Debug)]
struct DockUi {
    mode: DockMode,
    prompt_pending: Option<Pending>,
    prompt_page: usize,
}

impl Default for DockUi {
    fn default() -> Self {
        Self {
            mode: DockMode::Gameplay,
            prompt_pending: None,
            prompt_page: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TerminalLayout {
    origin: Vec2,
    scale: f32,
    logical_size: Vec2,
    rendered_size: Vec2,
    dock_height: f32,
}

impl Default for TerminalLayout {
    fn default() -> Self {
        let logical_size = Vec2::new(
            CELL_W * DISPLAY_WIDTH as f32,
            CELL_H * DISPLAY_HEIGHT as f32,
        );
        Self {
            origin: Vec2::ZERO,
            scale: 1.0,
            logical_size,
            rendered_size: logical_size,
            dock_height: dock_height(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockLayout {
    rail_left: f32,
    rail_width: f32,
    rows: usize,
    columns: usize,
    button_width: f32,
    height: f32,
}

impl Default for DockLayout {
    fn default() -> Self {
        let terminal = TerminalLayout::default();
        Self {
            rail_left: terminal.origin.x,
            rail_width: terminal.rendered_size.x,
            rows: 1,
            columns: 4,
            button_width: (terminal.rendered_size.x - DOCK_GAP * 5.0) / 4.0,
            height: dock_height(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DockOverlayLayout {
    heading: String,
    height: f32,
    content: Vec<DockButtonSpec>,
    navigation: Vec<DockButtonSpec>,
}

#[derive(Resource, Debug, Clone, PartialEq, Default)]
struct ScreenLayout {
    terminal: TerminalLayout,
    dock: DockLayout,
    base_specs: Vec<DockButtonSpec>,
    overlay: Option<DockOverlayLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockPointer {
    Mouse,
    Touch(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArmedDockButton {
    entity: Entity,
    action: DockAction,
    pointer: DockPointer,
    canceled: bool,
}

#[derive(Resource, Debug, Default)]
struct DockPress {
    armed: Option<ArmedDockButton>,
}

impl DockPress {
    fn try_arm(&mut self, button: ArmedDockButton) -> bool {
        if self.armed.is_some() {
            return false;
        }
        self.armed = Some(button);
        true
    }
}

#[derive(SystemParam)]
struct DockPointerInput<'w, 's> {
    mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    touches: Res<'w, Touches>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

type DockButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Interaction,
        &'static DockButton,
        &'static ComputedNode,
        &'static UiGlobalTransform,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        Option<&'static Children>,
    ),
>;

#[derive(Resource, Default)]
struct MovementRepeat {
    key: Option<KeyCode>,
    remaining: Duration,
}

#[derive(Resource, Default)]
struct InjectedInput(Option<char>);

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
        "Upotrěba: {program} [-SrdVh] [-s [fajl_rezultatov]] [fajl_shrånjeńja]\n\n\
         \t-S\t\tpri signalu izhod bez shrånjeńja\n\
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
            pointer_input,
            dock_input,
            keyboard,
            advance_pointer_travel,
            finalize_end,
            prepare_messages,
            update_screen_layout,
            layout_terminal,
            sync_dock,
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
            game.message(format!(
                "ne možno obnoviti shrånjeńje iz ⟨n:prěględka:gen⟩: {error}"
            ));
            game
        }
    };
    if game.options.name.is_empty() {
        game.options.name = "igrač".into();
    }
    game_app(game, false)
        .add_systems(
            Update,
            (
                pointer_input,
                dock_input,
                keyboard,
                advance_pointer_travel,
                finalize_end,
                prepare_messages,
                update_screen_layout,
                layout_terminal,
                sync_dock,
                render,
            )
                .chain(),
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
        .insert_resource(InjectedInput::default())
        .insert_resource(DockUi::default())
        .insert_resource(DockPress::default())
        .insert_resource(ScreenLayout::default())
        .insert_resource(State {
            game,
            modal: wizard_prompt
                .then(|| Modal::inline_prompt(password_prompt(Pending::StartupPassword))),
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
        title: "Rogue 5.4.5 — Mŕzavec".into(),
        resolution: WindowResolution::new(984, 744),
        resize_constraints: WindowResizeConstraints {
            min_width: 400.0,
            min_height: 480.0,
            ..default()
        },
        resizable: true,
        prevent_default_event_handling: true,
        ..default()
    };
    #[cfg(target_arch = "wasm32")]
    let window = Window {
        canvas: Some("#mrzavec".into()),
        fit_canvas_to_parent: true,
        resize_constraints: WindowResizeConstraints {
            min_width: 1.0,
            min_height: 1.0,
            ..default()
        },
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
        eprintln!("Avtomatično shrånjeńje ne udalo sę: {error}");
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
        state.modal = Some(Modal::full_screen(format!(
            "{}\n\n{}",
            tombstone_text(&state.game),
            table
        )));
        state.score_recorded = true;
        return;
    }
    state.modal = Some(Modal::full_screen(match state.game.end {
        mrzavec::game::EndState::Won => format!(
            "{}\n\n{}\nKonečny rezultat: {}\n\n{}",
            speak(
                "⟨n:čestitańje:nom:pl:U⟩, ⟨v2:viděti⟩ ⟨a:dnevny:světlo:acc⟩ ⟨n:světlo:acc⟩!\n\nUspěšno ⟨v2:izhoditi⟩ iz ⟨n:temnica:gen:pl:U⟩ ⟨n:pohibel:gen:U⟩."
            ),
            winner_sales_text(&state.game),
            score::amount(&state.game),
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
    }));
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
        .spawn((
            GameRoot,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                TerminalViewport,
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|viewport| {
                viewport
                    .spawn((
                        TerminalGrid,
                        Node {
                            display: Display::Grid,
                            width: px(CELL_W * DISPLAY_WIDTH as f32),
                            height: px(CELL_H * DISPLAY_HEIGHT as f32),
                            grid_template_columns: RepeatedGridTrack::px(
                                DISPLAY_WIDTH as u16,
                                CELL_W,
                            ),
                            grid_template_rows: RepeatedGridTrack::px(
                                DISPLAY_HEIGHT as u16,
                                CELL_H,
                            ),
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
                                BackgroundColor(Color::BLACK),
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
            root.spawn((
                DockRoot,
                Node {
                    width: percent(100),
                    height: px(dock_height(1)),
                    flex_shrink: 0.0,
                    border: UiRect::top(px(1)),
                    ..default()
                },
                BackgroundColor(DOCK_BACKGROUND_COLOR),
                BorderColor::all(DOCK_DIVIDER_COLOR),
            ));
        });
}

fn help_entry(command: char) -> Option<&'static HelpEntry> {
    HELP_ENTRIES
        .iter()
        .find(|entry| entry.command == command && entry.print)
}

fn command_dock_label(command: char) -> String {
    let label = help_entry(command)
        .and_then(|entry| entry.dock_label)
        .map(speak)
        .unwrap_or_else(|| control_label(command));
    format!("{} {label}", control_label(command))
}

fn command_palette_label(entry: &HelpEntry) -> String {
    help_entry_text(entry.command, entry.description)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn pack_has_selection(game: &Game, pending: Pending) -> bool {
    game.player
        .inventory
        .iter()
        .any(|item| item.in_pack && item_matches_selection(game, pending, item.kind))
}

fn ranked_dock_command(command: char) -> Option<RankedDockCommand> {
    HELP_ENTRIES
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.command == command && entry.print)
        .and_then(|(declaration_order, entry)| {
            Some(RankedDockCommand {
                command,
                importance: entry.dock_importance?,
                priority: entry.dock_priority?,
                declaration_order,
            })
        })
}

fn rank_dock_commands(commands: impl IntoIterator<Item = char>) -> Vec<RankedDockCommand> {
    let mut ranked: Vec<_> = commands
        .into_iter()
        .filter_map(ranked_dock_command)
        .collect();
    ranked.sort_by_key(|command| {
        (
            std::cmp::Reverse(command.importance),
            std::cmp::Reverse(command.priority),
            command.declaration_order,
        )
    });
    ranked.dedup_by_key(|command| command.command);
    ranked
}

fn contextual_commands(state: &State) -> Vec<char> {
    if state.pending.is_some()
        || state.modal.is_some()
        || state.game.end != mrzavec::game::EndState::Playing
    {
        return Vec::new();
    }

    let mut contextual = Vec::new();
    if state
        .game
        .floor_items
        .iter()
        .any(|item| item.pos == Some(state.game.player.pos))
    {
        contextual.push(',');
    }
    if state.game.player.pos == state.game.dungeon.stairs {
        if state.game.has_amulet {
            contextual.push('<');
        }
        contextual.push('>');
    }
    for (command, pending) in [
        ('q', Pending::Quaff),
        ('r', Pending::Read),
        ('e', Pending::Eat),
        ('z', Pending::ZapSelect),
        ('w', Pending::Wield),
        ('W', Pending::Wear),
        ('P', Pending::PutRing),
    ] {
        if pack_has_selection(&state.game, pending) {
            contextual.push(command);
        }
    }
    if state.game.player.armor.is_some() {
        contextual.push('T');
    }
    if state.game.player.rings.iter().any(Option::is_some) {
        contextual.push('R');
    }
    rank_dock_commands(contextual)
        .into_iter()
        .map(|command| command.command)
        .collect()
}

fn player_has_inventory(state: &State) -> bool {
    state.game.player.inventory.iter().any(|item| item.in_pack)
}

fn ranked_gameplay_commands(state: &State) -> Vec<RankedDockCommand> {
    let mut commands = contextual_commands(state);
    if commands.is_empty() && player_has_inventory(state) {
        commands.push('i');
    }
    commands.extend(['s', '.']);
    rank_dock_commands(commands)
}

fn dock_command_tone(command: char) -> DockButtonTone {
    if ranked_dock_command(command)
        .is_some_and(|command| command.importance == DockImportance::Urgent)
    {
        DockButtonTone::Urgent
    } else {
        DockButtonTone::Normal
    }
}

fn commands_heading() -> String {
    speak("⟨n:komanda:nom:pl:U⟩")
}

fn context_options_heading() -> String {
    speak(CONTEXT_OPTIONS_LABEL)
}

fn context_options_count_label(count: usize) -> String {
    format!("{} · {count}", context_options_heading())
}

fn dock_spec(
    action: DockAction,
    label: impl Into<String>,
    layout: DockButtonLayout,
    tone: DockButtonTone,
) -> DockButtonSpec {
    DockButtonSpec {
        action,
        label: label.into(),
        layout,
        tone,
    }
}

fn command_spec(command: char, layout: DockButtonLayout, tone: DockButtonTone) -> DockButtonSpec {
    let label = if layout == DockButtonLayout::Compact {
        command_dock_label(command)
    } else {
        help_entry(command)
            .map(command_palette_label)
            .unwrap_or_else(|| modal_command_label(command))
    };
    dock_spec(DockAction::Command(command), label, layout, tone)
}

fn spacer_spec() -> DockButtonSpec {
    dock_spec(
        DockAction::Disabled,
        "",
        DockButtonLayout::Spacer,
        DockButtonTone::Disabled,
    )
}

fn rail_capacity(rail_width: f32, minimum_button_width: f32) -> usize {
    (((rail_width - DOCK_GAP).max(0.0) / (minimum_button_width + DOCK_GAP)).floor() as usize).max(1)
}

fn gameplay_columns(rail_width: f32) -> usize {
    rail_capacity(rail_width, DOCK_MIN_BUTTON_WIDTH).max(2)
}

fn visible_gameplay_command_count(command_count: usize, rail_width: f32) -> usize {
    command_count.min(gameplay_columns(rail_width).saturating_sub(2))
}

fn gameplay_dock_specs(state: &State, rail_width: f32) -> Vec<DockButtonSpec> {
    let commands = ranked_gameplay_commands(state);
    let columns = gameplay_columns(rail_width);
    let direct_capacity = columns.saturating_sub(2);
    let visible = visible_gameplay_command_count(commands.len(), rail_width);
    let mut specs: Vec<DockButtonSpec> = commands
        .iter()
        .take(visible)
        .map(|command| {
            command_spec(
                command.command,
                DockButtonLayout::Compact,
                dock_command_tone(command.command),
            )
        })
        .collect();
    specs.resize_with(direct_capacity, spacer_spec);
    let hidden_count = commands.len() - visible;
    if hidden_count > 0 {
        specs.push(dock_spec(
            DockAction::OpenContextActions,
            context_options_count_label(hidden_count),
            DockButtonLayout::Compact,
            DockButtonTone::Normal,
        ));
    } else {
        specs.push(dock_spec(
            DockAction::Disabled,
            context_options_heading(),
            DockButtonLayout::Compact,
            DockButtonTone::Disabled,
        ));
    }
    specs.push(dock_spec(
        DockAction::OpenPalette,
        speak(COMMANDS_LABEL),
        DockButtonLayout::Compact,
        DockButtonTone::Normal,
    ));
    specs
}

fn context_overlay_commands(state: &State, rail_width: f32) -> Vec<char> {
    let commands = ranked_gameplay_commands(state);
    let visible = visible_gameplay_command_count(commands.len(), rail_width);
    commands
        .into_iter()
        .skip(visible)
        .map(|command| command.command)
        .collect()
}

fn modal_dock_commands(state: &State) -> Vec<char> {
    if state.game.end != mrzavec::game::EndState::Playing {
        return vec!['\u{1b}'];
    }
    let mut commands = match state.pending {
        Some(Pending::SaveConfirm | Pending::SaveOverwrite | Pending::QuitConfirm) => {
            vec!['y', 'n']
        }
        Some(Pending::PutRingHand(_) | Pending::RemoveRingHand) => vec!['l', 'r'],
        Some(Pending::Discoveries) => vec!['!', '?', '=', '/', '*'],
        Some(pending) if direction_pending(pending) => direction_menu_entries()
            .map(|(command, _)| command)
            .collect(),
        _ => Vec::new(),
    };
    let advances = state.modal.as_ref().is_some_and(modal_has_next_page)
        || matches!(
            state.pending,
            Some(
                Pending::MagicDetection
                    | Pending::FoodDetection
                    | Pending::DiscoveryMore
                    | Pending::SlowDiscoveryPrompt
                    | Pending::SlowDiscovery(_)
                    | Pending::SlowInventory(_)
                    | Pending::More
            )
        );
    if advances {
        commands.push(' ');
    }
    if state.pending.is_some() || state.modal.is_some() {
        commands.push('\u{1b}');
    }
    commands
}

fn modal_command_label(command: char) -> String {
    match command {
        ' ' => "Dalje".into(),
        '\u{1b}' => "^[ Escape".into(),
        'y' => "y Da".into(),
        'n' => "n Ne".into(),
        'l' => format!("l {}", speak("⟨a:lěvy:rųka:nom⟩")),
        'r' => format!("r {}", speak("⟨a:pravy:rųka:nom⟩")),
        command if matches!(command, 'h' | 'j' | 'k' | 'l' | 'y' | 'u' | 'b' | 'n') => {
            help_entry(command)
                .map(command_palette_label)
                .unwrap_or_else(|| control_label(command))
        }
        command => control_label(command),
    }
}

fn modal_dock_specs(state: &State) -> Vec<DockButtonSpec> {
    modal_dock_commands(state)
        .into_iter()
        .map(|command| {
            dock_spec(
                DockAction::Command(command),
                modal_command_label(command),
                DockButtonLayout::Compact,
                DockButtonTone::Normal,
            )
        })
        .collect()
}

fn base_dock_specs(state: &State, rail_width: f32) -> Vec<DockButtonSpec> {
    if state.pending.is_some()
        || state.modal.is_some()
        || state.game.end != mrzavec::game::EndState::Playing
    {
        modal_dock_specs(state)
    } else {
        gameplay_dock_specs(state, rail_width)
    }
}

fn prompt_choice_pending(state: &State) -> Option<Pending> {
    state.pending.filter(|pending| {
        *pending == Pending::PickyInventory
            || matches!(pending, Pending::Options(_))
            || is_item_selection(*pending)
    })
}

fn item_choice_specs(state: &State, pending: Pending) -> Vec<DockButtonSpec> {
    state
        .game
        .player
        .inventory
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.in_pack
                && (pending == Pending::PickyInventory
                    || item_matches_selection(&state.game, pending, item.kind))
        })
        .map(|(index, item)| {
            let letter = item.pack_letter.unwrap_or((b'a' + index as u8) as char);
            dock_spec(
                DockAction::Command(letter),
                format!("{letter}) {}", state.game.inventory_name(item, false)),
                DockButtonLayout::FullWidth,
                DockButtonTone::Normal,
            )
        })
        .collect()
}

fn prompt_choice_specs(state: &State) -> Option<(String, Vec<DockButtonSpec>)> {
    let pending = prompt_choice_pending(state)?;
    match pending {
        Pending::PickyInventory => {
            Some((speak("⟨n:torba:nom:U⟩"), item_choice_specs(state, pending)))
        }
        Pending::Options(_) => {
            let modal = state.modal.as_ref()?;
            Some((
                speak("⟨n:opcija:nom:pl:U⟩"),
                modal
                    .text
                    .lines()
                    .take(OPTION_COUNT)
                    .enumerate()
                    .map(|(index, label)| {
                        dock_spec(
                            DockAction::OptionRow(index),
                            label,
                            DockButtonLayout::FullWidth,
                            DockButtonTone::Normal,
                        )
                    })
                    .collect(),
            ))
        }
        pending if is_item_selection(pending) => Some((
            speak("⟨n:prědmet:nom:pl:U⟩"),
            item_choice_specs(state, pending),
        )),
        _ => None,
    }
}

fn palette_content_specs(state: &State, mode: DockMode, rail_width: f32) -> Vec<DockButtonSpec> {
    match mode {
        DockMode::Gameplay => Vec::new(),
        DockMode::ContextActions { page } => context_overlay_commands(state, rail_width)
            .into_iter()
            .skip(page * PALETTE_PAGE_SIZE)
            .take(PALETTE_PAGE_SIZE)
            .map(|command| {
                command_spec(
                    command,
                    DockButtonLayout::FullWidth,
                    dock_command_tone(command),
                )
            })
            .collect(),
        DockMode::Categories { page } => CommandCategory::ALL
            .into_iter()
            .skip(page * PALETTE_PAGE_SIZE)
            .take(PALETTE_PAGE_SIZE)
            .map(|category| {
                dock_spec(
                    DockAction::OpenCategory(category),
                    speak(category.label()),
                    DockButtonLayout::FullWidth,
                    DockButtonTone::Normal,
                )
            })
            .collect(),
        DockMode::Category { category, page } => {
            let entries: Vec<&HelpEntry> = HELP_ENTRIES
                .iter()
                .filter(|entry| entry.print && entry.command != '\0' && entry.category == category)
                .collect();
            let page_count = entries.len().div_ceil(PALETTE_PAGE_SIZE).max(1);
            let page = page.min(page_count - 1);
            entries
                .into_iter()
                .skip(page * PALETTE_PAGE_SIZE)
                .take(PALETTE_PAGE_SIZE)
                .map(|entry| {
                    dock_spec(
                        DockAction::Command(entry.command),
                        command_palette_label(entry),
                        DockButtonLayout::FullWidth,
                        DockButtonTone::Normal,
                    )
                })
                .collect()
        }
    }
}

#[cfg(test)]
fn dock_specs(state: &State, mode: DockMode) -> Vec<DockButtonSpec> {
    let rail_width = CELL_W * DISPLAY_WIDTH as f32;
    if state.pending.is_some()
        || state.modal.is_some()
        || state.game.end != mrzavec::game::EndState::Playing
        || mode == DockMode::Gameplay
    {
        base_dock_specs(state, rail_width)
    } else {
        let mut specs = palette_content_specs(state, mode, rail_width);
        specs.extend(palette_navigation_specs(state, mode, rail_width));
        specs
    }
}

fn mode_page_count(state: &State, mode: DockMode, rail_width: f32) -> usize {
    let entries = match mode {
        DockMode::ContextActions { .. } => context_overlay_commands(state, rail_width).len(),
        DockMode::Category { category, .. } => HELP_ENTRIES
            .iter()
            .filter(|entry| entry.print && entry.command != '\0' && entry.category == category)
            .count(),
        DockMode::Categories { .. } => CommandCategory::ALL.len(),
        DockMode::Gameplay => 0,
    };
    entries.div_ceil(PALETTE_PAGE_SIZE).max(1)
}

fn mode_page(mode: DockMode) -> usize {
    match mode {
        DockMode::Categories { page }
        | DockMode::ContextActions { page }
        | DockMode::Category { page, .. } => page,
        DockMode::Gameplay => 0,
    }
}

fn palette_heading(state: &State, mode: DockMode, rail_width: f32) -> String {
    match mode {
        DockMode::Gameplay => String::new(),
        DockMode::Categories { .. } => commands_heading(),
        DockMode::ContextActions { page } => {
            let count = context_overlay_commands(state, rail_width).len();
            let pages = mode_page_count(state, mode, rail_width);
            format!(
                "{} · {count} · {}/{}",
                context_options_heading(),
                page.min(pages - 1) + 1,
                pages
            )
        }
        DockMode::Category { category, page } => {
            let pages = mode_page_count(state, mode, rail_width);
            format!(
                "{} / {} · {}/{}",
                commands_heading(),
                speak(category.label()),
                page.min(pages - 1) + 1,
                pages
            )
        }
    }
}

fn navigation_spec(action: DockAction, label: impl Into<String>, enabled: bool) -> DockButtonSpec {
    dock_spec(
        if enabled {
            action
        } else {
            DockAction::Disabled
        },
        label,
        DockButtonLayout::Navigation,
        if enabled {
            DockButtonTone::Normal
        } else {
            DockButtonTone::Disabled
        },
    )
}

fn palette_navigation_specs(state: &State, mode: DockMode, rail_width: f32) -> Vec<DockButtonSpec> {
    let page = mode_page(mode);
    let page_count = mode_page_count(state, mode, rail_width);
    vec![
        navigation_spec(
            DockAction::BackToCategories,
            format!("← {}", commands_heading()),
            matches!(mode, DockMode::Category { .. }),
        ),
        navigation_spec(DockAction::PreviousPage, "←", page > 0),
        navigation_spec(DockAction::NextPage, "Dalje →", page + 1 < page_count),
        navigation_spec(DockAction::ClosePalette, "^[ Escape", true),
    ]
}

fn dock_height(rows: usize) -> f32 {
    let rows = rows.max(1);
    DOCK_BUTTON_HEIGHT * rows as f32 + DOCK_GAP * (rows + 1) as f32
}

fn calculate_terminal_layout(window_size: Vec2, dock_height: f32) -> TerminalLayout {
    let available_height = (window_size.y - dock_height).max(1.0);
    let logical_size = Vec2::new(
        CELL_W * DISPLAY_WIDTH as f32,
        CELL_H * DISPLAY_HEIGHT as f32,
    );
    let scale = (window_size.x / logical_size.x)
        .min(available_height / logical_size.y)
        .clamp(0.1, 1.0);
    let rendered_size = logical_size * scale;
    TerminalLayout {
        origin: Vec2::new(
            (window_size.x - rendered_size.x) / 2.0,
            (available_height - rendered_size.y) / 2.0,
        ),
        scale,
        logical_size,
        rendered_size,
        dock_height,
    }
}

fn compact_columns(spec_count: usize, rail_width: f32, ordinary_gameplay: bool) -> usize {
    if spec_count == 0 {
        return 1;
    }
    let width_capacity = rail_capacity(rail_width, DOCK_MIN_BUTTON_WIDTH);
    let maximum = if ordinary_gameplay {
        spec_count
    } else {
        DOCK_MAX_PROMPT_COLUMNS
    };
    spec_count.min(width_capacity).min(maximum).max(1)
}

fn calculate_dock_layout(
    terminal: TerminalLayout,
    spec_count: usize,
    ordinary_gameplay: bool,
) -> DockLayout {
    let columns = compact_columns(spec_count, terminal.rendered_size.x, ordinary_gameplay);
    let rows = spec_count.max(1).div_ceil(columns);
    let rail_width = terminal.rendered_size.x;
    let button_width = ((rail_width - DOCK_GAP * (columns + 1) as f32) / columns as f32).max(1.0);
    DockLayout {
        rail_left: terminal.origin.x,
        rail_width,
        rows,
        columns,
        button_width,
        height: dock_height(rows),
    }
}

fn palette_overlay_height(content_count: usize) -> f32 {
    DOCK_GAP * (content_count + 3) as f32
        + DOCK_HEADER_HEIGHT
        + content_count as f32 * DOCK_NAVIGATION_HEIGHT
        + DOCK_NAVIGATION_HEIGHT
}

fn prompt_choice_overlay_layout(
    state: &State,
    prompt_page: usize,
    window_height: f32,
) -> Option<DockOverlayLayout> {
    let (heading, choices) = prompt_choice_specs(state)?;
    if choices.is_empty() {
        return None;
    }
    let page_count = choices.len().div_ceil(PROMPT_CHOICE_PAGE_SIZE).max(1);
    let page = prompt_page.min(page_count - 1);
    let content: Vec<_> = choices
        .into_iter()
        .skip(page * PROMPT_CHOICE_PAGE_SIZE)
        .take(PROMPT_CHOICE_PAGE_SIZE)
        .collect();
    let navigation = vec![
        navigation_spec(DockAction::PreviousPromptPage, "←", page > 0),
        navigation_spec(DockAction::NextPromptPage, "Dalje →", page + 1 < page_count),
        dock_spec(
            DockAction::Command('\u{1b}'),
            "^[ Escape",
            DockButtonLayout::Navigation,
            DockButtonTone::Normal,
        ),
    ];
    let desired_height = palette_overlay_height(content.len());
    Some(DockOverlayLayout {
        heading: format!("{heading} · {}/{}", page + 1, page_count),
        height: desired_height.min((window_height - DOCK_GAP * 2.0).max(dock_height(1))),
        content,
        navigation,
    })
}

fn overlay_layout(
    state: &State,
    mode: DockMode,
    prompt_page: usize,
    terminal: TerminalLayout,
    window_height: f32,
) -> Option<DockOverlayLayout> {
    if let Some(overlay) = prompt_choice_overlay_layout(state, prompt_page, window_height) {
        return Some(overlay);
    }
    if mode == DockMode::Gameplay {
        return None;
    }
    let rail_width = terminal.rendered_size.x;
    let content = palette_content_specs(state, mode, rail_width);
    let navigation = palette_navigation_specs(state, mode, rail_width);
    let desired_height = palette_overlay_height(content.len());
    Some(DockOverlayLayout {
        heading: palette_heading(state, mode, rail_width),
        height: desired_height.min((window_height - DOCK_GAP * 2.0).max(dock_height(1))),
        content,
        navigation,
    })
}

fn calculate_screen_layout_with_prompt_page(
    window_size: Vec2,
    state: &State,
    mode: DockMode,
    prompt_page: usize,
) -> ScreenLayout {
    let ordinary_gameplay = state.pending.is_none()
        && state.modal.is_none()
        && state.game.end == mrzavec::game::EndState::Playing;
    let mut rows = 1;
    for _ in 0..4 {
        let terminal = calculate_terminal_layout(window_size, dock_height(rows));
        let base_specs = base_dock_specs(state, terminal.rendered_size.x);
        let dock = calculate_dock_layout(terminal, base_specs.len(), ordinary_gameplay);
        if dock.rows == rows {
            return ScreenLayout {
                terminal,
                dock,
                overlay: overlay_layout(state, mode, prompt_page, terminal, window_size.y),
                base_specs,
            };
        }
        rows = dock.rows;
    }
    let terminal = calculate_terminal_layout(window_size, dock_height(rows));
    let base_specs = base_dock_specs(state, terminal.rendered_size.x);
    let dock = calculate_dock_layout(terminal, base_specs.len(), ordinary_gameplay);
    ScreenLayout {
        terminal,
        dock,
        overlay: overlay_layout(state, mode, prompt_page, terminal, window_size.y),
        base_specs,
    }
}

#[cfg(test)]
fn calculate_screen_layout(window_size: Vec2, state: &State, mode: DockMode) -> ScreenLayout {
    calculate_screen_layout_with_prompt_page(window_size, state, mode, 0)
}

fn normalized_dock_mode(state: &State, mode: DockMode, rail_width: f32) -> DockMode {
    if state.pending.is_some()
        || state.modal.is_some()
        || state.game.end != mrzavec::game::EndState::Playing
    {
        return DockMode::Gameplay;
    }
    match mode {
        DockMode::ContextActions { page } => {
            let commands = context_overlay_commands(state, rail_width);
            if commands.is_empty() {
                DockMode::Gameplay
            } else {
                DockMode::ContextActions {
                    page: page.min(commands.len().div_ceil(PALETTE_PAGE_SIZE).max(1) - 1),
                }
            }
        }
        DockMode::Category { category, page } => {
            let entry_count = HELP_ENTRIES
                .iter()
                .filter(|entry| entry.print && entry.command != '\0' && entry.category == category)
                .count();
            DockMode::Category {
                category,
                page: page.min(entry_count.div_ceil(PALETTE_PAGE_SIZE).max(1) - 1),
            }
        }
        DockMode::Categories { page } => {
            let page_count = CommandCategory::ALL
                .len()
                .div_ceil(PALETTE_PAGE_SIZE)
                .max(1);
            DockMode::Categories {
                page: page.min(page_count - 1),
            }
        }
        DockMode::Gameplay => mode,
    }
}

fn normalize_prompt_choice_state(state: &State, dock_ui: &mut DockUi) {
    let prompt_pending = prompt_choice_pending(state);
    if dock_ui.prompt_pending != prompt_pending {
        dock_ui.prompt_pending = prompt_pending;
        dock_ui.prompt_page = match prompt_pending {
            Some(Pending::Options(index)) => index / PROMPT_CHOICE_PAGE_SIZE,
            _ => 0,
        };
    }
    if let Some((_, choices)) = prompt_choice_specs(state) {
        let page_count = choices.len().div_ceil(PROMPT_CHOICE_PAGE_SIZE).max(1);
        dock_ui.prompt_page = dock_ui.prompt_page.min(page_count - 1);
    } else {
        dock_ui.prompt_page = 0;
    }
}

fn update_screen_layout(
    state: Res<State>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut dock_ui: ResMut<DockUi>,
    mut screen: ResMut<ScreenLayout>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let window_size = Vec2::new(window.width(), window.height());
    let one_row_terminal = calculate_terminal_layout(window_size, dock_height(1));
    let mode = normalized_dock_mode(&state, dock_ui.mode, one_row_terminal.rendered_size.x);
    if dock_ui.mode != mode {
        dock_ui.mode = mode;
    }
    normalize_prompt_choice_state(&state, &mut dock_ui);
    let next = calculate_screen_layout_with_prompt_page(
        window_size,
        &state,
        dock_ui.mode,
        dock_ui.prompt_page,
    );
    if *screen != next {
        *screen = next;
    }
}

fn layout_terminal(
    windows: Query<&Window, With<PrimaryWindow>>,
    screen: Res<ScreenLayout>,
    mut nodes: LayoutNodeQuery,
    mut glyphs: Query<(&mut TextFont, &mut LineHeight), With<Glyph>>,
) {
    if !screen.is_changed() {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let available_height = (window.height() - screen.dock.height).max(1.0);
    for (mut node, viewport, grid, dock) in &mut nodes {
        if viewport.is_some() {
            node.height = px(available_height);
            node.flex_grow = 0.0;
        } else if dock.is_some() {
            node.height = px(screen.dock.height);
        } else if grid.is_some() {
            node.width = px(screen.terminal.rendered_size.x);
            node.height = px(screen.terminal.rendered_size.y);
            node.grid_template_columns =
                RepeatedGridTrack::px(DISPLAY_WIDTH as u16, CELL_W * screen.terminal.scale);
            node.grid_template_rows =
                RepeatedGridTrack::px(DISPLAY_HEIGHT as u16, CELL_H * screen.terminal.scale);
        }
    }
    for (mut font, mut line_height) in &mut glyphs {
        font.font_size = FontSize::Px(FONT_SIZE * screen.terminal.scale);
        *line_height = LineHeight::Px(CELL_H * screen.terminal.scale);
    }
}

fn sync_dock(
    mut commands: Commands,
    screen: Res<ScreenLayout>,
    dock_root: Query<Entity, With<DockRoot>>,
    game_root: Query<Entity, With<GameRoot>>,
    overlays: Query<Entity, With<PaletteOverlay>>,
    mut press: ResMut<DockPress>,
) {
    if !screen.is_changed() {
        return;
    }
    press.armed = None;
    for overlay in &overlays {
        commands.entity(overlay).despawn();
    }
    let Ok(dock_root) = dock_root.single() else {
        return;
    };
    commands.entity(dock_root).despawn_children();
    commands.entity(dock_root).with_children(|dock| {
        dock.spawn(Node {
            position_type: PositionType::Absolute,
            left: px(screen.dock.rail_left),
            top: px(0),
            width: px(screen.dock.rail_width),
            height: percent(100),
            flex_direction: FlexDirection::Row,
            flex_wrap: if screen.dock.rows == 1 {
                FlexWrap::NoWrap
            } else {
                FlexWrap::Wrap
            },
            justify_content: JustifyContent::Start,
            align_content: AlignContent::Center,
            align_items: AlignItems::Center,
            column_gap: px(DOCK_GAP),
            row_gap: px(DOCK_GAP),
            padding: UiRect::all(px(DOCK_GAP)),
            ..default()
        })
        .with_children(|rail| {
            for spec in &screen.base_specs {
                if spec.layout == DockButtonLayout::Spacer {
                    rail.spawn(Node {
                        width: px(screen.dock.button_width),
                        height: px(DOCK_BUTTON_HEIGHT),
                        flex_shrink: 0.0,
                        ..default()
                    });
                    continue;
                }
                let (background, text, border) = button_colors(spec.tone, false, false);
                rail.spawn((
                    Button,
                    DockButton {
                        action: spec.action,
                        tone: spec.tone,
                    },
                    Node {
                        width: px(screen.dock.button_width),
                        height: px(DOCK_BUTTON_HEIGHT),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(6)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(background),
                    BorderColor::all(border),
                ))
                .with_child((
                    Text::new(spec.label.clone()),
                    TextFont {
                        font_size: FontSize::Px(if screen.dock.button_width < 92.0 {
                            14.0
                        } else {
                            15.0
                        }),
                        ..default()
                    },
                    TextColor(text),
                    TextLayout::justify(Justify::Center),
                ));
            }
        });
    });

    let (Ok(game_root), Some(overlay)) = (game_root.single(), screen.overlay.as_ref()) else {
        return;
    };
    let navigation_width =
        ((screen.dock.rail_width - DOCK_GAP * 5.0) / overlay.navigation.len() as f32).max(1.0);
    commands.entity(game_root).with_children(|game| {
        game.spawn((
            PaletteOverlay,
            GlobalZIndex(10),
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    FocusPolicy::Block,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(screen.dock.rail_left),
                        bottom: px(0),
                        width: px(screen.dock.rail_width),
                        height: px(overlay.height),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Start,
                        align_items: AlignItems::Stretch,
                        row_gap: px(DOCK_GAP),
                        padding: UiRect::all(px(DOCK_GAP)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(DOCK_BACKGROUND_COLOR),
                    BorderColor::all(DOCK_BORDER_COLOR),
                ))
                .with_children(|panel| {
                    panel
                        .spawn(Node {
                            width: percent(100),
                            height: px(DOCK_HEADER_HEIGHT),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(px(1)),
                            ..default()
                        })
                        .insert(BorderColor::all(DOCK_DIVIDER_COLOR))
                        .with_child((
                            Text::new(overlay.heading.clone()),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(GLYPH_COLOR),
                            TextLayout::justify(Justify::Center),
                        ));
                    for spec in &overlay.content {
                        let (background, text, border) = button_colors(spec.tone, false, false);
                        panel
                            .spawn((
                                Button,
                                DockButton {
                                    action: spec.action,
                                    tone: spec.tone,
                                },
                                Node {
                                    width: percent(100),
                                    height: px(DOCK_NAVIGATION_HEIGHT),
                                    flex_shrink: 0.0,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::horizontal(px(8)),
                                    border: UiRect::all(px(1)),
                                    ..default()
                                },
                                BackgroundColor(background),
                                BorderColor::all(border),
                            ))
                            .with_child((
                                Text::new(spec.label.clone()),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(text),
                                TextLayout::justify(Justify::Center),
                            ));
                    }
                    panel
                        .spawn(Node {
                            width: percent(100),
                            height: px(DOCK_NAVIGATION_HEIGHT),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Row,
                            column_gap: px(DOCK_GAP),
                            ..default()
                        })
                        .with_children(|navigation| {
                            for spec in &overlay.navigation {
                                let (background, text, border) =
                                    button_colors(spec.tone, false, false);
                                navigation
                                    .spawn((
                                        Button,
                                        DockButton {
                                            action: spec.action,
                                            tone: spec.tone,
                                        },
                                        Node {
                                            width: px(navigation_width),
                                            height: px(DOCK_NAVIGATION_HEIGHT),
                                            flex_shrink: 0.0,
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            padding: UiRect::horizontal(px(4)),
                                            border: UiRect::all(px(1)),
                                            ..default()
                                        },
                                        BackgroundColor(background),
                                        BorderColor::all(border),
                                    ))
                                    .with_child((
                                        Text::new(spec.label.clone()),
                                        TextFont {
                                            font_size: FontSize::Px(14.0),
                                            ..default()
                                        },
                                        TextColor(text),
                                        TextLayout::justify(Justify::Center),
                                    ));
                            }
                        });
                });
        });
    });
}

fn button_colors(tone: DockButtonTone, armed: bool, hovered: bool) -> (Color, Color, Color) {
    match (tone, armed, hovered) {
        (DockButtonTone::Disabled, _, _) => (
            DOCK_BUTTON_COLOR,
            DOCK_DISABLED_COLOR,
            DOCK_DISABLED_BORDER_COLOR,
        ),
        (DockButtonTone::Urgent, true, _) => (DOCK_BUTTON_COLOR, GLYPH_COLOR, DOCK_BORDER_COLOR),
        (DockButtonTone::Urgent, false, _) => {
            (DOCK_BUTTON_PRESSED_COLOR, Color::BLACK, GLYPH_COLOR)
        }
        (DockButtonTone::Normal, true, _) => (DOCK_BUTTON_PRESSED_COLOR, Color::BLACK, GLYPH_COLOR),
        (DockButtonTone::Normal, false, true) => {
            (DOCK_BUTTON_HOVERED_COLOR, GLYPH_COLOR, DOCK_BORDER_COLOR)
        }
        (DockButtonTone::Normal, false, false) => {
            (DOCK_BUTTON_COLOR, GLYPH_COLOR, DOCK_BORDER_COLOR)
        }
    }
}

fn apply_dock_action(
    action: DockAction,
    dock_ui: &mut DockUi,
    injected: &mut InjectedInput,
    state: &mut State,
) {
    match action {
        DockAction::Command(command) => {
            if dock_ui.mode != DockMode::Gameplay {
                dock_ui.mode = DockMode::Gameplay;
            }
            injected.0 = Some(command);
        }
        DockAction::OpenPalette => dock_ui.mode = DockMode::Categories { page: 0 },
        DockAction::OpenContextActions => dock_ui.mode = DockMode::ContextActions { page: 0 },
        DockAction::OpenCategory(category) => {
            dock_ui.mode = DockMode::Category { category, page: 0 }
        }
        DockAction::PreviousPage => match dock_ui.mode {
            DockMode::ContextActions { page } => {
                dock_ui.mode = DockMode::ContextActions {
                    page: page.saturating_sub(1),
                }
            }
            DockMode::Category { category, page } => {
                dock_ui.mode = DockMode::Category {
                    category,
                    page: page.saturating_sub(1),
                }
            }
            DockMode::Categories { page } => {
                dock_ui.mode = DockMode::Categories {
                    page: page.saturating_sub(1),
                }
            }
            DockMode::Gameplay => {}
        },
        DockAction::NextPage => match dock_ui.mode {
            DockMode::ContextActions { page } => {
                dock_ui.mode = DockMode::ContextActions { page: page + 1 }
            }
            DockMode::Category { category, page } => {
                dock_ui.mode = DockMode::Category {
                    category,
                    page: page + 1,
                }
            }
            DockMode::Categories { page } => dock_ui.mode = DockMode::Categories { page: page + 1 },
            DockMode::Gameplay => {}
        },
        DockAction::PreviousPromptPage => {
            dock_ui.prompt_page = dock_ui.prompt_page.saturating_sub(1);
        }
        DockAction::NextPromptPage => {
            dock_ui.prompt_page += 1;
        }
        DockAction::OptionRow(index) => apply_option_pointer(state, index),
        DockAction::BackToCategories => dock_ui.mode = DockMode::Categories { page: 0 },
        DockAction::ClosePalette => dock_ui.mode = DockMode::Gameplay,
        DockAction::Disabled => {}
    }
}

fn dock_input(
    input: DockPointerInput,
    mut buttons: DockButtonQuery,
    mut labels: Query<&mut TextColor>,
    mut press: ResMut<DockPress>,
    mut dock_ui: ResMut<DockUi>,
    mut injected: ResMut<InjectedInput>,
    mut state: ResMut<State>,
) {
    let Ok(window) = input.windows.single() else {
        return;
    };
    if press.armed.is_none() {
        let pointer = if input.mouse_buttons.just_pressed(MouseButton::Left) {
            Some(DockPointer::Mouse)
        } else {
            input
                .touches
                .iter_just_pressed()
                .min_by_key(|touch| touch.id())
                .map(|touch| DockPointer::Touch(touch.id()))
        };
        if let Some(pointer) = pointer {
            let position = match pointer {
                DockPointer::Mouse => window.physical_cursor_position(),
                DockPointer::Touch(id) => input
                    .touches
                    .iter_just_pressed()
                    .find(|touch| touch.id() == id)
                    .map(|touch| touch.position() * window.scale_factor()),
            };
            if let Some(position) = position {
                for (entity, interaction, button, node, transform, _, _, _) in &mut buttons {
                    if *interaction == Interaction::Pressed
                        && button.action != DockAction::Disabled
                        && node.contains_point(*transform, position)
                    {
                        press.try_arm(ArmedDockButton {
                            entity,
                            action: button.action,
                            pointer,
                            canceled: false,
                        });
                        break;
                    }
                }
            }
        }
    }

    let mut activation = None;
    if let Some(mut armed) = press.armed {
        let (position, released, canceled) = match armed.pointer {
            DockPointer::Mouse => (
                window.physical_cursor_position(),
                input.mouse_buttons.just_released(MouseButton::Left),
                false,
            ),
            DockPointer::Touch(id) => {
                let held = input
                    .touches
                    .get_pressed(id)
                    .map(|touch| touch.position() * window.scale_factor());
                let released = input
                    .touches
                    .get_released(id)
                    .map(|touch| touch.position() * window.scale_factor());
                (
                    held.or(released),
                    input.touches.just_released(id),
                    input.touches.just_canceled(id),
                )
            }
        };
        if canceled {
            press.armed = None;
        } else {
            let inside = position.is_some_and(|position| {
                buttons
                    .get_mut(armed.entity)
                    .is_ok_and(|(_, _, _, node, transform, _, _, _)| {
                        node.contains_point(*transform, position)
                    })
            });
            if !inside {
                armed.canceled = true;
            }
            if released {
                if !armed.canceled && inside {
                    activation = Some(armed.action);
                }
                press.armed = None;
            } else {
                press.armed = Some(armed);
            }
        }
    }
    if let Some(action) = activation {
        apply_dock_action(action, &mut dock_ui, &mut injected, &mut state);
    }

    for (entity, interaction, button, _, _, mut background, mut border, children) in &mut buttons {
        let armed = press
            .armed
            .is_some_and(|armed| armed.entity == entity && !armed.canceled);
        let (button_color, text_color, border_color) =
            button_colors(button.tone, armed, *interaction == Interaction::Hovered);
        background.0 = button_color;
        border.set_all(border_color);
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut color) = labels.get_mut(child) {
                    color.0 = text_color;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PointerCell {
    column: usize,
    row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerAction {
    Key(char),
    Travel(Pos),
    OptionRow(usize),
}

/// Cursor and touch positions supplied by Bevy/winit are logical pixels.
/// `TerminalLayout` is also used to size the rendered grid, keeping native
/// DPI, responsive wasm canvases, rendering, and hit testing on one transform.
fn pointer_cell(position: Vec2, layout: &TerminalLayout) -> Option<PointerCell> {
    let local = position - layout.origin;
    if local.x < 0.0
        || local.y < 0.0
        || local.x >= layout.rendered_size.x
        || local.y >= layout.rendered_size.y
    {
        return None;
    }
    Some(PointerCell {
        column: (local.x / (CELL_W * layout.scale)).floor() as usize,
        row: (local.y / (CELL_H * layout.scale)).floor() as usize,
    })
}

fn dungeon_pos_at_cell(cell: PointerCell) -> Option<Pos> {
    (DUNGEON_FIRST_ROW..STATUS_ROW)
        .contains(&cell.row)
        .then(|| {
            Pos::new(
                cell.column as i32,
                (cell.row - DUNGEON_FIRST_ROW + 1) as i32,
            )
        })
}

fn direction_command_toward(player: Pos, target: Pos) -> Option<char> {
    let direction = Direction::from_delta(
        (target.x - player.x).signum(),
        (target.y - player.y).signum(),
    )?;
    Some(match direction {
        Direction::Left => 'h',
        Direction::Down => 'j',
        Direction::Up => 'k',
        Direction::Right => 'l',
        Direction::UpLeft => 'y',
        Direction::UpRight => 'u',
        Direction::DownLeft => 'b',
        Direction::DownRight => 'n',
    })
}

fn direction_pending(pending: Pending) -> bool {
    matches!(
        pending,
        Pending::ThrowDirection(_)
            | Pending::ZapDirection(_)
            | Pending::FightDirection(_)
            | Pending::MoveDirection
            | Pending::TrapDirection
    )
}

fn overlay_start_x(modal: &str) -> usize {
    let width = modal
        .lines()
        .take(STATUS_ROW)
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
        .min(DISPLAY_WIDTH - 2);
    (DISPLAY_WIDTH - 1).saturating_sub(width)
}

fn modal_advances_with_map_tap(state: &State) -> bool {
    state.game.end != mrzavec::game::EndState::Playing
        || matches!(
            state.pending,
            Some(
                Pending::MagicDetection
                    | Pending::FoodDetection
                    | Pending::Help
                    | Pending::DiscoveryMore
                    | Pending::SlowDiscoveryPrompt
                    | Pending::SlowDiscovery(_)
                    | Pending::SlowInventory(_)
                    | Pending::More
            )
        )
}

fn item_menu_letters(game: &Game, pending: Pending) -> Vec<char> {
    game.player
        .inventory
        .iter()
        .enumerate()
        .filter(|(_, item)| item.in_pack && item_matches_selection(game, pending, item.kind))
        .map(|(index, item)| item.pack_letter.unwrap_or((b'a' + index as u8) as char))
        .collect()
}

fn inventory_letters(game: &Game) -> Vec<char> {
    game.player
        .inventory
        .iter()
        .enumerate()
        .filter(|(_, item)| item.in_pack)
        .map(|(index, item)| item.pack_letter.unwrap_or((b'a' + index as u8) as char))
        .collect()
}

fn pointer_action_at_cell(state: &State, cell: PointerCell) -> Option<PointerAction> {
    if let Some(pending) = state.pending {
        if direction_pending(pending) {
            if let Some(modal) = &state.modal {
                let start_x = overlay_start_x(&modal.text);
                if cell.column >= start_x && (1..=8).contains(&cell.row) {
                    return direction_menu_entries()
                        .nth(cell.row - 1)
                        .map(|(command, _)| PointerAction::Key(command));
                }
            }
            if let Some(target) = dungeon_pos_at_cell(cell) {
                return direction_command_toward(state.game.player.pos, target)
                    .map(PointerAction::Key);
            }
        }
        if pending == Pending::IdentifyGlyph
            && let Some(target) = dungeon_pos_at_cell(cell)
        {
            return Some(PointerAction::Key(state.game.glyph_at(target)));
        }
        if let Pending::Options(_) = pending
            && cell.row < OPTION_COUNT
        {
            return Some(PointerAction::OptionRow(cell.row));
        }
        if pending == Pending::PickyInventory {
            let letters = inventory_letters(&state.game);
            let leading_rows = state.modal.as_deref().map_or(0, |modal| {
                modal.lines().count().saturating_sub(letters.len())
            });
            let absolute_row = state.modal.as_ref().map_or(0, |modal| modal.offset) + cell.row;
            if let Some(index) = absolute_row.checked_sub(leading_rows)
                && let Some(letter) = letters.get(index)
            {
                return Some(PointerAction::Key(*letter));
            }
        }
        if is_item_selection(pending) {
            let letters = item_menu_letters(&state.game, pending);
            let leading_rows = state.modal.as_deref().map_or(0, |modal| {
                modal.lines().count().saturating_sub(letters.len())
            });
            let absolute_row = state.modal.as_ref().map_or(0, |modal| modal.offset) + cell.row;
            if let Some(index) = absolute_row.checked_sub(leading_rows)
                && let Some(letter) = letters.get(index)
            {
                return Some(PointerAction::Key(*letter));
            }
        }
        if modal_advances_with_map_tap(state) && dungeon_pos_at_cell(cell).is_some() {
            return Some(PointerAction::Key(' '));
        }
        return None;
    }

    if state.modal.is_some() && state.game.end != mrzavec::game::EndState::Playing {
        if dungeon_pos_at_cell(cell).is_some() {
            return Some(PointerAction::Key(' '));
        }
        return None;
    }
    dungeon_pos_at_cell(cell).map(PointerAction::Travel)
}

fn apply_option_pointer(state: &mut State, index: usize) {
    show_option(state, index);
    match index {
        0..=5 => {
            let current = match index {
                0 => state.game.options.terse,
                1 => state.game.options.fight_flush,
                2 => state.game.options.jump,
                3 => state.game.options.see_floor,
                4 => state.game.options.passgo,
                5 => state.game.options.tombstone,
                _ => unreachable!(),
            };
            set_boolean_option(&mut state.game, index, !current);
            state.modal = Some(Modal::full_screen(options_text(
                &state.game,
                Some(index),
                None,
                None,
            )));
        }
        6 => {
            state.game.options.inventory_style = match state.game.options.inventory_style {
                mrzavec::game::InventoryStyle::Overwrite => mrzavec::game::InventoryStyle::Slow,
                mrzavec::game::InventoryStyle::Slow => mrzavec::game::InventoryStyle::Clear,
                mrzavec::game::InventoryStyle::Clear => mrzavec::game::InventoryStyle::Overwrite,
            };
            state.modal = Some(Modal::full_screen(options_text(
                &state.game,
                Some(index),
                None,
                None,
            )));
        }
        7..OPTION_COUNT => {}
        _ => unreachable!("pointer option row is bounded by OPTION_COUNT"),
    }
}

fn pointer_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    screen: Res<ScreenLayout>,
    dock_ui: Res<DockUi>,
    mut injected: ResMut<InjectedInput>,
    mut state: ResMut<State>,
) {
    if dock_ui.mode != DockMode::Gameplay || dock_ui.prompt_pending.is_some() {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let touch_position = touches
        .iter_just_pressed()
        .next()
        .map(|touch| touch.position());
    let activation = touch_position.or_else(|| {
        mouse_buttons
            .just_pressed(MouseButton::Left)
            .then(|| window.cursor_position())
            .flatten()
    });
    let Some(cell) = activation.and_then(|position| pointer_cell(position, &screen.terminal))
    else {
        return;
    };
    if state.game.is_traveling() {
        state.game.cancel_travel();
        return;
    }
    let Some(action) = pointer_action_at_cell(&state, cell) else {
        return;
    };
    match action {
        PointerAction::Key(command) => {
            injected.0 = Some(command);
        }
        PointerAction::Travel(destination) => {
            state.game.start_travel(destination);
        }
        PointerAction::OptionRow(index) => apply_option_pointer(&mut state, index),
    }
}

fn advance_pointer_travel(
    mut state: ResMut<State>,
    dock_ui: Res<DockUi>,
    dock_press: Res<DockPress>,
) {
    if !state.game.is_traveling() {
        return;
    }
    if dock_press.armed.is_some()
        || dock_ui.mode != DockMode::Gameplay
        || state.pending.is_some()
        || state.modal.is_some()
        || state.game.end != mrzavec::game::EndState::Playing
        || state.game.player.conditions.asleep_turns > 0
    {
        state.game.cancel_travel();
        return;
    }
    state.game.advance_travel();
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut movement_repeat: ResMut<MovementRepeat>,
    mut injected: ResMut<InjectedInput>,
    mut dock_ui: ResMut<DockUi>,
    mut state: ResMut<State>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let injected_input = injected.0.take();
    if matches!(
        dock_ui.mode,
        DockMode::ContextActions { .. } | DockMode::Categories { .. } | DockMode::Category { .. }
    ) {
        if keys.just_pressed(KeyCode::Escape) {
            dock_ui.mode = DockMode::Gameplay;
        }
        return;
    }
    let space_pressed = keys.just_pressed(KeyCode::Space) || injected_input == Some(' ');
    let escape_pressed = keys.just_pressed(KeyCode::Escape) || injected_input == Some('\u{1b}');
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
    if keys.get_just_pressed().next().is_some()
        || repeated_input.is_some()
        || injected_input.is_some()
    {
        state.game.cancel_travel();
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
    if state.game.end != mrzavec::game::EndState::Playing && state.score_recorded && escape_pressed
    {
        app_exit.write(AppExit::Success);
        return;
    }
    if matches!(
        state.pending,
        Some(Pending::MagicDetection | Pending::FoodDetection)
    ) && space_pressed
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
        if escape_pressed {
            state.pending = None;
            state.modal = None;
        } else if space_pressed && state.modal.as_ref().is_some_and(modal_has_next_page) {
            state.modal.as_mut().unwrap().offset += MODAL_PAGE_ROWS;
        }
        return;
    }
    if let Some(pending) = state.pending.filter(|pending| is_item_selection(*pending))
        && space_pressed
    {
        if state.modal.as_ref().is_some_and(modal_has_next_page) {
            state.modal.as_mut().unwrap().offset += MODAL_PAGE_ROWS;
        } else {
            retry_invalid_item(&mut state, pending, ' ');
        }
        return;
    }
    if state.pending == Some(Pending::DiscoveryMore) && space_pressed {
        if state.modal.as_ref().is_some_and(modal_has_next_page) {
            state.modal.as_mut().unwrap().offset += MODAL_PAGE_ROWS;
        } else {
            state.pending = None;
            state.modal = None;
            state.game.message_without_recall("");
        }
        return;
    }
    if matches!(
        state.pending,
        Some(Pending::SlowDiscoveryPrompt | Pending::SlowDiscovery(_))
    ) && space_pressed
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
            state.modal = Some(Modal::full_screen(format!(
                "{}  --Dalje--",
                message_display_text(&line)
            )));
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
    if space_pressed
        && let Some(modal) = &state.modal
        && modal.presentation == ModalPresentation::FullScreen
    {
        if modal_has_next_page(modal) {
            state.modal.as_mut().unwrap().offset += MODAL_PAGE_ROWS;
            return;
        }
        if modal.offset > 0 || state.game.end != mrzavec::game::EndState::Playing {
            if state.game.end != mrzavec::game::EndState::Playing {
                app_exit.write(AppExit::Success);
            } else {
                state.modal = None;
                state.pending = None;
            }
            return;
        }
    }
    if state.game.end != mrzavec::game::EndState::Playing && state.score_recorded {
        return;
    }
    if state.pending == Some(Pending::More) {
        if space_pressed || escape_pressed {
            state.pending = None;
            state.modal = None;
        }
        return;
    }
    if matches!(state.pending, Some(Pending::Options(_))) && escape_pressed {
        finish_options(&mut state);
        return;
    }
    if escape_pressed {
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
        state.pending = None;
        if continue_count {
            continue_counted_command(&mut state);
        } else {
            state.counted_command = None;
        }
        return;
    }
    if let Some(Pending::SlowInventory(index)) = state.pending
        && space_pressed
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
            state.modal = Some(Modal::full_screen(slow_inventory_line(&state.game, next)));
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
                state.modal = Some(Modal::full_screen(options_text(
                    &state.game,
                    Some(index),
                    Some(&state.input_buffer),
                    None,
                )));
            } else {
                let error = if index == 6 {
                    "(O, S ili C)"
                } else {
                    "(T ili F)"
                };
                state.modal = Some(Modal::full_screen(options_text(
                    &state.game,
                    Some(index),
                    None,
                    Some(error),
                )));
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
                        state.modal =
                            Some(remembered_inline_prompt(&mut state, "imę ⟨n:fajl:gen⟩: "));
                        return;
                    }
                    state.game.options.save_file = mrzavec::game::normalize_option_string(&input);
                    if save_exists(&state.game).unwrap_or(false) {
                        state.pending = Some(Pending::SaveOverwrite);
                        state.modal = Some(remembered_inline_prompt(
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
            state.modal = Some(Modal::inline_prompt(match pending {
                Pending::Password | Pending::StartupPassword => password_prompt(pending),
                Pending::CallText(_) | Pending::AutoCall => {
                    format!("{}{}", call_prompt(&state.game), state.input_buffer)
                }
                Pending::SaveFileText => {
                    format!("{}{}", speak("imę ⟨n:fajl:gen⟩: "), state.input_buffer)
                }
                Pending::WizardCreateGold => format!("koliko?{}", state.input_buffer),
                _ => unreachable!(),
            }));
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
    let special = injected_input.or(control).or_else(|| {
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
    if injected_input.is_none() && shifted && ch.is_ascii_alphabetic() {
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
                    state.modal = Some(remembered_inline_prompt(&mut state, "imę ⟨n:fajl:gen⟩: "));
                }
                (Pending::SaveOverwrite, 'n') => {
                    state.pending = Some(Pending::SaveConfirm);
                    let prompt = save_confirmation(&state.game);
                    state.modal = Some(remembered_inline_prompt(&mut state, prompt));
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
                    state.modal = Some(Modal::inline_prompt(message_display_text(&prompt)));
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
                state.modal = Some(Modal::inline_prompt(message_display_text(&prompt)));
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
            state.modal = wizard_probability_text(ch).map(Modal::full_screen);
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
                    state.modal = Some(remembered_inline_prompt(&mut state, "koliko?"));
                } else if matches!(kind, ItemKind::Food | ItemKind::Amulet) {
                    state.game.wizard_create(kind, 0);
                    state.pending = None;
                    state.modal = None;
                    continue_counted_command(&mut state);
                } else {
                    state.pending = Some(Pending::WizardCreateWhich(kind));
                    let prompt = wizard_which_prompt(kind);
                    state.modal = Some(remembered_inline_prompt(&mut state, prompt));
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
                        state.modal = Some(Modal::full_screen(options_text(
                            &state.game,
                            Some(index),
                            None,
                            Some("(T ili F)"),
                        )));
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
                        state.modal = Some(Modal::full_screen(options_text(
                            &state.game,
                            Some(index),
                            None,
                            Some("(O, S ili C)"),
                        )));
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
            state.modal = Some(Modal::full_screen(options_text(
                &state.game,
                Some(index),
                Some(&state.input_buffer),
                None,
            )));
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
                state.modal = Some(Modal::inline_prompt(match pending {
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
                        format!("{}{}", speak("Imę ⟨n:fajl:gen⟩: "), state.input_buffer)
                    }
                    Pending::WizardCreateGold => format!("Koliko?{}", state.input_buffer),
                    _ => unreachable!(),
                }));
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
                                    state.modal =
                                        Some(Modal::inline_prompt(message_display_text(&prompt)));
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
                    state.modal = Some(Modal::full_screen(magic_detection_text(&state.game)));
                    return;
                }
                if food_detection && !state.game.food_positions().is_empty() {
                    state.pending = Some(Pending::FoodDetection);
                    state.modal = Some(Modal::full_screen(food_detection_text(&state.game)));
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
            Some(remembered_inline_prompt(
                &mut state,
                "čto ⟨v2:hotěti⟩ opoznati? ",
            ))
        }
        Command::Help => {
            state.game.remember_message("");
            state.pending = Some(Pending::Help);
            Some(Modal::full_screen(help_text()))
        }
        Command::Discoveries => {
            state.pending = Some(Pending::Discoveries);
            let prompt = discoveries_prompt(&state.game);
            Some(remembered_inline_prompt(&mut state, prompt))
        }
        Command::Options => {
            state.pending = Some(Pending::Options(0));
            Some(Modal::full_screen(options_text(
                &state.game,
                Some(0),
                None,
                None,
            )))
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
            state
                .game
                .message("shell ne jest dostųpny v ⟨n:prěględka:loc⟩");
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
            Some(remembered_inline_prompt(
                &mut state,
                "istinno li ⟨v2:izhoditi⟩?",
            ))
        }
        Command::Save => {
            state.pending = Some(Pending::SaveConfirm);
            let prompt = save_confirmation(&state.game);
            Some(remembered_inline_prompt(&mut state, prompt))
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
                state
                    .game
                    .message("ne ⟨v2:mogti⟩.  ⟨v3:izględati:U⟩, že to jest ⟨pp:proklęti:n⟩");
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
                Some(remembered_inline_prompt(
                    &mut state,
                    "parola ⟨n:čarovnik:gen:U⟩: ",
                ))
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
                Some(Modal::inline_prompt(message_display_text(&prompt)))
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
            let message =
                current_message(&state.game, state.game.player.weapon, "⟨v2:dŕžati⟩", None);
            state.game.message(message);
            None
        }
        Command::CurrentArmor => {
            let message =
                current_message(&state.game, state.game.player.armor, "⟨v2:nositi⟩", None);
            state.game.message(message);
            None
        }
        Command::CurrentRings => {
            for (id, verbose_where, terse_where) in [
                (
                    state.game.player.rings[0],
                    "na ⟨a:lěvy:rųka:loc⟩ ⟨n:rųka:loc⟩",
                    "(L)",
                ),
                (
                    state.game.player.rings[1],
                    "na ⟨a:pravy:rųka:loc⟩ ⟨n:rųka:loc⟩",
                    "(R)",
                ),
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
            Some(remembered_inline_prompt(&mut state, prompt))
        }
        Command::Wizard(WizardCommand::Map) if state.game.wizard => {
            state.pending = Some(Pending::More);
            Some(Modal::full_screen(wizard_map_text(&state.game)))
        }
        Command::Wizard(WizardCommand::Identify) if state.game.wizard => {
            wizard_identify_prompt(&mut state)
        }
        Command::Wizard(WizardCommand::Charge) if state.game.wizard => {
            wizard_charge_prompt(&mut state)
        }
        Command::Wizard(WizardCommand::Create) if state.game.wizard => {
            state.pending = Some(Pending::WizardCreateType);
            Some(remembered_inline_prompt(
                &mut state,
                "vid ⟨n:prědmet:gen⟩: ",
            ))
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
    format!("shråniti fajl ({})? ", game.options.save_file)
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
            state
                .game
                .message(format!("igra ⟨pp:shråniti:f⟩: {destination}"));
            app_exit.write(AppExit::Success);
        }
        Err(error) => {
            state
                .game
                .message(format!("shrånjeńje ne udalo sę: {error}"));
            state.pending = Some(Pending::SaveFileText);
            state.input_buffer.clear();
            state.modal = Some(remembered_inline_prompt(state, "imę ⟨n:fajl:gen⟩: "));
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
            state.modal = Some(Modal::full_screen(food_detection_text(&state.game)));
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
    state.modal = Some(Modal::inline_prompt(format!(
        "{}{}",
        message_display_text(&prompt),
        state.input_buffer
    )));
}

fn show_pending_call(state: &mut State) -> bool {
    if state.game.pending_call.is_none() {
        return false;
    }
    state.input_buffer.clear();
    state.pending = Some(Pending::AutoCall);
    let prompt = call_prompt(&state.game);
    state.game.remember_message(&prompt);
    state.modal = Some(Modal::inline_prompt(message_display_text(&prompt)));
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
            Some(remembered_inline_prompt(state, "vid ⟨n:prědmet:gen⟩: "))
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
fn ground_inventory_modal(state: &mut State) -> Option<Modal> {
    if state.game.floor_items.is_empty() {
        state.game.message(if state.game.options.terse {
            "⟨a:pråzdny:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩"
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
    if state.game.options.inventory_style == mrzavec::game::InventoryStyle::Overwrite {
        Some(Modal::event_overlay_or_full_screen(out))
    } else {
        Some(Modal::full_screen(out))
    }
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
        "{} ({}) — ⟨a:kaky:čislo:acc⟩ čislo ⟨v2:hotěti⟩? (0-{highest})",
        kind.glyph(),
        wizard_kind_name(kind)
    ))
}
fn resolve_wizard_which(state: &mut State, kind: ItemKind, which: u8) {
    if which >= wizard_kind_count(kind) {
        let error = format!(
            "⟨a:nepraviľny:čislo:nom⟩ čislo ({}), ješče raz",
            wizard_kind_name(kind)
        );
        state.game.message(&error);
        let prompt = wizard_which_prompt(kind);
        state.game.remember_message(&prompt);
        state.modal = Some(Modal::inline_prompt(message_display_text(&prompt)));
    } else if matches!(kind, ItemKind::Weapon | ItemKind::Armor)
        || (kind == ItemKind::Ring && matches!(which, 0 | 1 | 7 | 8))
    {
        state.pending = Some(Pending::WizardCreateBlessing(kind, which));
        state.modal = Some(remembered_inline_prompt(state, "blagoslovjeńje? (+,-,n) "));
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
    let title = speak(
        "⟨v2:čuti:U⟩ ⟨n:blizkosť:acc⟩ ⟨n:čar:gen:pl⟩ na ⟨toj:loc⟩ ⟨n:stųpenj:loc⟩. --Dalje--",
    );
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
    let title = speak(
        "⟨a:tvoj:nos:nom:U⟩ ⟨n:nos:nom⟩ ⟨v3:svŕběti⟩ i ⟨v2:čuti⟩ ⟨n:zapah:acc⟩ ⟨n:jeda:gen⟩. --Dalje--",
    );
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

fn direction_prompt(state: &mut State, pending: Pending) -> Option<Modal> {
    let prompt = if state.game.options.terse {
        "strana: "
    } else {
        "v ⟨ktory:acc:f⟩ ⟨n:stråna:acc⟩? "
    };
    state.pending = Some(pending);
    state.game.remember_message(prompt);
    let mut lines = vec![message_display_text(prompt)];
    lines.extend(direction_menu_entries().map(|(_, text)| text));
    Some(Modal::overlay(lines.join("\n")))
}

fn direction_menu_entries() -> impl Iterator<Item = (char, String)> {
    ['h', 'j', 'k', 'l', 'y', 'u', 'b', 'n']
        .into_iter()
        .map(|command| {
            let entry = HELP_ENTRIES
                .iter()
                .find(|entry| entry.command == command)
                .expect("every pointer direction is in HELP_ENTRIES");
            (command, help_entry_text(command, entry.description))
        })
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
    state.modal = Some(Modal::inline_prompt(message_display_text(&prompt)));
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
    let mut lines: Vec<String> = game
        .player
        .inventory
        .iter()
        .enumerate()
        .filter(|(_, item)| item.in_pack)
        .filter(|(_, item)| item_matches_selection(game, pending, item.kind))
        .map(|(index, item)| {
            let letter = item.pack_letter.unwrap_or((b'a' + index as u8) as char);
            format!("{letter}) {}", game.inventory_name(item, false))
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    if let Some(feedback) = feedback {
        lines.insert(0, feedback.into());
    }
    Some(lines.join("\n"))
}
fn select_item_menu(state: &mut State, pending: Pending) -> Option<Modal> {
    let Some(text) = item_menu_text(&state.game, pending, None) else {
        state.game.message(if state.game.options.terse {
            "⟨ničto:gen⟩ ⟨a:prigodny:město:gen⟩"
        } else {
            "ne ⟨v2:imati⟩ ⟨ničto:gen⟩ ⟨a:prigodny:město:gen⟩"
        });
        state.pending = None;
        state.modal = None;
        return None;
    };
    state.pending = Some(pending);
    state.game.remember_message("");
    Some(Modal::full_screen(text))
}
fn retry_invalid_item(state: &mut State, pending: Pending, ch: char) {
    let error = format!("'{}' ne jest praviľny prědmet", control_label(ch));
    state.game.message(&error);
    state.modal = item_menu_text(&state.game, pending, Some(&message_display_text(&error)))
        .map(Modal::full_screen);
    if state.modal.is_none() {
        // A selection is only pending while eligible items exist; if that ever
        // stops holding, cancel rather than strand a menu-less pending state.
        state.pending = None;
    }
}
fn wizard_identify_prompt(state: &mut State) -> Option<Modal> {
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
fn wizard_charge_prompt(state: &mut State) -> Option<Modal> {
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
) -> Option<Modal> {
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

fn render(
    state: Res<State>,
    mut cells: Query<(&Cell, &Children, &mut BackgroundColor)>,
    mut glyphs: Query<(&mut Text, &mut TextColor), With<Glyph>>,
) {
    if !state.is_changed() {
        return;
    }
    let buffer = display(&state);
    for (cell, children, mut background) in &mut cells {
        background.0 = Color::BLACK;
        for child in children.iter() {
            if let Ok((mut text, mut color)) = glyphs.get_mut(child) {
                text.0 = buffer[cell.0].to_string();
                color.0 = GLYPH_COLOR;
            }
        }
    }
}
fn display(state: &State) -> Vec<char> {
    let mut out = vec![' '; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    if let Some(modal) = &state.modal
        && modal.presentation == ModalPresentation::FullScreen
    {
        let all_lines: Vec<&str> = modal.text.lines().collect();
        let explicit_more = all_lines.last() == Some(&" --Dalje--");
        let content_count = all_lines.len() - usize::from(explicit_more);
        let remaining = content_count.saturating_sub(modal.offset);
        let has_next_page = remaining > MODAL_PAGE_ROWS;
        let reserve_more = explicit_more || has_next_page;
        let visible = if reserve_more {
            MODAL_PAGE_ROWS
        } else {
            DISPLAY_HEIGHT
        };
        for (y, line) in all_lines
            .into_iter()
            .skip(modal.offset)
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
    if let Some(prompt) = &state.modal
        && prompt.presentation == ModalPresentation::InlinePrompt
    {
        let text = event_text.get_or_insert_with(String::new);
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(prompt.text.trim());
    }
    if let Some(text) = event_text {
        let content_rows: Vec<usize> = (0..EVENT_ROWS).collect();
        write_event_text(&mut out, &text, &content_rows);
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
    if let Some(modal) = &state.modal
        && modal.presentation == ModalPresentation::Overlay
    {
        let lines: Vec<&str> = modal.text.lines().take(STATUS_ROW).collect();
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

fn modal_has_next_page(modal: &Modal) -> bool {
    let mut lines = modal.text.lines();
    let count = lines.by_ref().count();
    let explicit_more = modal.text.lines().last() == Some(" --Dalje--");
    count.saturating_sub(usize::from(explicit_more)) > modal.offset + MODAL_PAGE_ROWS
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

fn write_event_text(out: &mut [char], text: &str, content_rows: &[usize]) {
    let lines = wrap_terminal_text(text, DISPLAY_WIDTH);
    let start = lines.len().saturating_sub(content_rows.len());
    for (&row, line) in content_rows.iter().zip(&lines[start..]) {
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
    let hunger = ["", "Glåd", "Slabosť", "Nemoć"]
        .get(game.hungry_state as usize)
        .copied()
        .unwrap_or("");
    let hp_width = game.player.stats.max_hp.to_string().len();
    format!(
        "Stųp: {}  Zlåto: {:<5}  Zdr: {:>width$}({:>width$})  Sila: {:>2}({})  Brȯn: {:<2}  Izk: {}/{}  {}",
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
fn inventory_modal(state: &mut State) -> Option<Modal> {
    let pack_len = state
        .game
        .player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .count();
    if pack_len == 0 {
        state.game.message(if state.game.options.terse {
            "⟨a:pråzdny:rųka:nom:pl⟩ ⟨n:rųka:nom:pl⟩"
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
            Some(Modal::full_screen(slow_inventory_line(&state.game, 0)))
        }
        mrzavec::game::InventoryStyle::Overwrite => {
            let text = inventory_text(&state.game);
            state.pending = Some(Pending::More);
            Some(Modal::event_overlay_or_full_screen(text))
        }
        mrzavec::game::InventoryStyle::Clear => {
            state.pending = Some(Pending::More);
            Some(Modal::full_screen(inventory_text(&state.game)))
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
            || speak("⟨a:tvoj:torba:nom:U⟩ ⟨n:torba:nom⟩ jest ⟨a:pråzdny:torba:nom⟩."),
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

fn picky_inventory_prompt(state: &mut State) -> Option<Modal> {
    let pack_len = state
        .game
        .player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .count();
    if pack_len == 0 {
        state.game.message("⟨ničto:gen⟩ ne ⟨v2:nositi⟩");
        return None;
    }
    if pack_len == 1 {
        let item = state
            .game
            .player
            .inventory
            .iter()
            .find(|item| item.in_pack)
            .unwrap();
        let description = state.game.inventory_name(item, false);
        let letter = item.pack_letter.unwrap_or('?');
        state.game.message(format!("{letter}) {description}"));
        return None;
    }
    state.pending = Some(Pending::PickyInventory);
    let prompt = if state.game.options.terse {
        "prědmet: "
    } else {
        "kaky prědmet ⟨v2:hotěti⟩ viděti? "
    };
    state.game.remember_message(prompt);
    let items = (0..pack_len)
        .map(|index| inventory_line(&state.game, index))
        .collect::<Vec<_>>()
        .join("\n");
    Some(Modal::full_screen(format!(
        "{}\n{items}",
        message_display_text(prompt)
    )))
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
        ItemKind::Potion => mrzavec::lang::COLOR_ADJ
            [game.appearances.potion_colors[item.which as usize]]
            .to_string(),
        ItemKind::Scroll => game.appearances.scroll_titles[item.which as usize].clone(),
        ItemKind::Ring => mrzavec::lang::STONE_LEX
            [game.appearances.ring_stones[item.which as usize]]
            .lemma
            .to_string(),
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
        (
            '!',
            ItemKind::Potion,
            POTION_NAMES.len(),
            &mrzavec::lang::POTION,
        ),
        (
            '?',
            ItemKind::Scroll,
            SCROLL_NAMES.len(),
            &mrzavec::lang::SCROLL,
        ),
        ('=', ItemKind::Ring, RING_NAMES.len(), &mrzavec::lang::RING),
        (
            '/',
            ItemKind::Stick,
            STICK_NAMES.len(),
            &mrzavec::lang::WAND,
        ),
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
            state.modal = Some(Modal::inline_prompt(format!(
                "{}  --Dalje--",
                message_display_text(&prompt)
            )));
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
            state.pending = Some(Pending::DiscoveryMore);
            state.modal = Some(
                if state.game.options.inventory_style == mrzavec::game::InventoryStyle::Overwrite {
                    Modal::event_overlay_or_full_screen(text)
                } else {
                    Modal::full_screen(text)
                },
            );
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

fn help_text() -> String {
    HELP_ENTRIES
        .iter()
        .filter(|entry| entry.print)
        .map(|entry| help_entry_text(entry.command, entry.description))
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
            '+' => "dvėri",
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
    (
        "⟨a:kråtky:sȯobčeńje:nom:pl:U⟩ ⟨n:sȯobčeńje:nom:pl⟩",
        "terse",
    ),
    ("Ignorovati pisańje podčas boja", "flush"),
    (
        "Pokazati ⟨n:pozicija:acc⟩ jedino na ⟨n:konec:loc⟩ ⟨n:běg:gen⟩",
        "jump",
    ),
    ("Pokazati ⟨pp:osvětliti:n⟩ tlo", "seefloor"),
    ("Slědovati ⟨n:povråt:dat:pl⟩ v ⟨n:prohod:loc:pl⟩", "passgo"),
    ("Pokazati kamenj ⟨n:grob:gen⟩ po ⟨n:smŕť:loc⟩", "tombstone"),
    ("Stiľ ⟨n:torba:gen⟩", "inven"),
    ("Imę", "name"),
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
    state.modal = Some(Modal::full_screen(options_text(
        &state.game,
        Some(index),
        None,
        None,
    )));
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
    state.modal = Some(Modal::full_screen(format!(
        "{} --Dalje--",
        options_text(&state.game, None, None, None)
    )));
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
        app.insert_resource(InjectedInput::default());
        app.insert_resource(DockUi::default());
        app.add_message::<AppExit>();
        app.add_systems(Update, keyboard);
        app
    }

    fn dock_input_app(initial_state: State) -> App {
        let mut app = App::new();
        app.insert_resource(initial_state);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.insert_resource(MovementRepeat::default());
        app.insert_resource(InjectedInput::default());
        app.insert_resource(DockUi::default());
        app.insert_resource(DockPress::default());
        app.add_message::<AppExit>();
        app.world_mut().spawn((game_window(), PrimaryWindow));
        app.add_systems(Update, (dock_input, keyboard).chain());
        app
    }

    fn spawn_test_dock_button(app: &mut App, action: DockAction) -> Entity {
        app.world_mut()
            .spawn((
                Button,
                DockButton {
                    action,
                    tone: DockButtonTone::Normal,
                },
                Interaction::None,
                ComputedNode {
                    size: Vec2::new(100.0, DOCK_BUTTON_HEIGHT),
                    ..default()
                },
                UiGlobalTransform::from_xy(50.0, DOCK_BUTTON_HEIGHT / 2.0),
                BackgroundColor(DOCK_BUTTON_COLOR),
                BorderColor::all(DOCK_BORDER_COLOR),
            ))
            .id()
    }

    fn set_test_cursor(app: &mut App, position: Vec2) {
        let mut windows = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        windows
            .single_mut(app.world_mut())
            .expect("test app has one primary window")
            .set_physical_cursor_position(Some(position.as_dvec2()));
    }

    fn arm_test_dock_button(app: &mut App, entity: Entity) {
        set_test_cursor(app, Vec2::new(50.0, DOCK_BUTTON_HEIGHT / 2.0));
        *app.world_mut()
            .entity_mut(entity)
            .get_mut::<Interaction>()
            .expect("test dock button has interaction") = Interaction::Pressed;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
    }

    fn release_test_dock_button(app: &mut App, entity: Entity) {
        set_test_cursor(app, Vec2::new(50.0, DOCK_BUTTON_HEIGHT / 2.0));
        *app.world_mut()
            .entity_mut(entity)
            .get_mut::<Interaction>()
            .expect("test dock button has interaction") = Interaction::Hovered;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
    }

    fn pointer_keyboard_app(initial_state: State) -> App {
        let mut app = App::new();
        app.insert_resource(initial_state);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(Touches::default());
        app.insert_resource(Time::<()>::default());
        app.insert_resource(MovementRepeat::default());
        app.insert_resource(InjectedInput::default());
        app.insert_resource(DockUi::default());
        let mut screen = ScreenLayout::default();
        screen.terminal.origin = Vec2::new(12.0, 30.0);
        app.insert_resource(screen);
        app.add_message::<AppExit>();
        app.world_mut().spawn((game_window(), PrimaryWindow));
        app.add_systems(Update, (pointer_input, keyboard).chain());
        app
    }

    fn click_cell(app: &mut App, cell: PointerCell) {
        let layout = app.world().resource::<ScreenLayout>().terminal;
        let position = (layout.origin
            + Vec2::new(
                CELL_W * layout.scale * (cell.column as f32 + 0.5),
                CELL_H * layout.scale * (cell.row as f32 + 0.5),
            ))
        .as_dvec2();
        let mut windows = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        windows
            .single_mut(app.world_mut())
            .expect("test app has one primary window")
            .set_physical_cursor_position(Some(position));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
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

    fn seen_passable_neighbor(game: &Game) -> Pos {
        let player = game.player.pos;
        [
            Direction::Left,
            Direction::Down,
            Direction::Up,
            Direction::Right,
            Direction::UpLeft,
            Direction::UpRight,
            Direction::DownLeft,
            Direction::DownRight,
        ]
        .into_iter()
        .map(|direction| {
            let (dx, dy) = direction.delta();
            player.offset(dx, dy)
        })
        .find(|position| {
            game.dungeon
                .map
                .get(*position)
                .is_some_and(|cell| cell.seen && cell.terrain.passable())
        })
        .expect("the player starts with at least one remembered neighboring floor tile")
    }

    #[test]
    fn responsive_terminal_geometry_and_pointer_mapping_share_one_transform() {
        for window_size in [
            Vec2::new(984.0, 744.0),
            Vec2::new(1366.0, 768.0),
            Vec2::new(400.0, 800.0),
        ] {
            let dock_height = dock_height(1);
            let layout = calculate_terminal_layout(window_size, dock_height);
            let position = layout.origin
                + Vec2::new(CELL_W * layout.scale * 17.5, CELL_H * layout.scale * 9.5);
            assert_eq!(
                pointer_cell(position, &layout),
                Some(PointerCell { column: 17, row: 9 })
            );
            assert_eq!(
                pointer_cell(layout.origin, &layout),
                Some(PointerCell { column: 0, row: 0 })
            );
            assert_eq!(
                pointer_cell(
                    layout.origin + layout.rendered_size - Vec2::splat(0.01),
                    &layout,
                ),
                Some(PointerCell {
                    column: DISPLAY_WIDTH - 1,
                    row: DISPLAY_HEIGHT - 1,
                })
            );
            assert_eq!(
                pointer_cell(layout.origin - Vec2::new(0.1, 0.0), &layout),
                None
            );
            assert_eq!(
                pointer_cell(layout.origin - Vec2::new(0.0, 0.1), &layout),
                None
            );
            assert_eq!(
                pointer_cell(
                    layout.origin + Vec2::new(layout.rendered_size.x + 0.1, 0.0),
                    &layout,
                ),
                None
            );
            assert_eq!(
                pointer_cell(
                    layout.origin + Vec2::new(0.0, layout.rendered_size.y + 0.1),
                    &layout,
                ),
                None
            );
            assert!(
                layout.origin.y + layout.rendered_size.y
                    <= window_size.y - dock_height + f32::EPSILON
            );
        }

        let layout = calculate_terminal_layout(Vec2::new(400.0, 800.0), dock_height(1));
        assert!((layout.scale - (400.0 / 960.0)).abs() < 0.001);
        assert_eq!(
            pointer_cell(Vec2::new(200.0, 799.0), &layout),
            None,
            "dock coordinates never map to terminal cells"
        );

        let mut scaled = game_window();
        scaled
            .resolution
            .set_scale_factor_and_apply_to_physical_size(2.0);
        let logical = Vec2::new(17.5 * CELL_W, 9.5 * CELL_H);
        scaled.set_physical_cursor_position(Some(logical.as_dvec2() * 2.0));
        assert_eq!(scaled.cursor_position(), Some(logical));
    }

    #[test]
    fn browser_frame_reserves_all_four_safe_area_insets() {
        let html = include_str!("../web/index.html");
        for edge in ["top", "right", "bottom", "left"] {
            assert!(
                html.contains(&format!("padding-{edge}: env(safe-area-inset-{edge}, 0px)")),
                "the responsive game frame must reserve the {edge} safe-area inset"
            );
        }
    }

    #[test]
    fn every_printable_help_command_is_in_exactly_one_palette_category() {
        for entry in HELP_ENTRIES {
            if !entry.print || entry.command == '\0' {
                continue;
            }
            let occurrences: usize = CommandCategory::ALL
                .into_iter()
                .map(|category| usize::from(entry.category == category))
                .sum();
            assert_eq!(
                occurrences,
                1,
                "{} must occur in exactly one palette category",
                control_label(entry.command)
            );
            assert!(!command_palette_label(entry).trim().is_empty());
            assert_eq!(
                entry.dock_label.is_some(),
                entry.dock_priority.is_some(),
                "{} must define its compact label and priority together",
                control_label(entry.command)
            );
            assert_eq!(
                entry.dock_label.is_some(),
                entry.dock_importance.is_some(),
                "{} must define semantic dock importance with its compact label",
                control_label(entry.command)
            );
        }
        assert!(
            HELP_ENTRIES
                .iter()
                .filter(|entry| entry.command == '\0')
                .all(|entry| entry.print && entry.dock_label.is_none())
        );
    }

    #[test]
    fn command_palette_reaches_every_printable_command_once() {
        let state = state(90_020);
        let expected: std::collections::BTreeSet<char> = HELP_ENTRIES
            .iter()
            .filter_map(|entry| (entry.print && entry.command != '\0').then_some(entry.command))
            .collect();
        let mut actual = std::collections::BTreeSet::new();

        for category in CommandCategory::ALL {
            let category_entries = HELP_ENTRIES
                .iter()
                .filter(|entry| entry.print && entry.command != '\0' && entry.category == category)
                .count();
            for page in 0..category_entries.div_ceil(PALETTE_PAGE_SIZE).max(1) {
                for spec in dock_specs(&state, DockMode::Category { category, page }) {
                    if let DockAction::Command(command) = spec.action {
                        assert!(
                            actual.insert(command),
                            "{} appeared more than once in the palette",
                            control_label(command)
                        );
                    }
                }
            }
        }

        assert_eq!(actual, expected);
        assert!(
            dock_specs(&state, DockMode::Categories { page: 0 })
                .iter()
                .all(|spec| !matches!(spec.action, DockAction::Command(_)))
        );
    }

    #[test]
    fn gameplay_dock_ranks_context_and_exposes_exact_overflow() {
        let mut state = state(90_021);
        state.game.player.inventory.clear();
        state.game.player.weapon = None;
        state.game.player.armor = None;
        state.game.player.rings = [None, None];
        state.game.floor_items.clear();

        assert!(contextual_commands(&state).is_empty());
        let empty = gameplay_dock_specs(&state, 400.0);
        assert_eq!(empty.len(), gameplay_columns(400.0));
        assert_eq!(empty[0].action, DockAction::Command('s'));
        assert_eq!(empty[1].action, DockAction::Command('.'));
        assert_eq!(empty[2].layout, DockButtonLayout::Spacer);
        assert_eq!(empty[3].action, DockAction::Disabled);
        assert_eq!(empty[3].label, context_options_heading());
        assert_eq!(empty[4].action, DockAction::OpenPalette);

        let mut potion = mrzavec::item::Item::basic(990_021, ItemKind::Potion, 0);
        potion.pos = Some(state.game.player.pos);
        state.game.floor_items.push(potion);
        state
            .game
            .player
            .inventory
            .push(pack_item(990_022, ItemKind::Food, 0, 'a'));
        state
            .game
            .player
            .inventory
            .push(pack_item(990_023, ItemKind::Stick, 0, 'b'));

        let actions = contextual_commands(&state);
        assert_eq!(actions[0], ',');
        assert_eq!(actions, vec![',', 'e', 'z', 'w']);
        let narrow = gameplay_dock_specs(&state, 400.0);
        assert_eq!(narrow[0].action, DockAction::Command(','));
        assert_eq!(narrow[0].tone, DockButtonTone::Urgent);
        assert_eq!(narrow[1].action, DockAction::Command('e'));
        assert_eq!(narrow[2].action, DockAction::Command('z'));
        assert_eq!(narrow[3].action, DockAction::OpenContextActions);
        assert_eq!(narrow[3].label, context_options_count_label(3));
        assert_eq!(narrow[4].action, DockAction::OpenPalette);
        assert_eq!(context_overlay_commands(&state, 400.0), vec!['w', 's', '.']);

        let very_narrow = gameplay_dock_specs(&state, 320.0);
        assert_eq!(very_narrow.len(), 4);
        assert_eq!(very_narrow[0].action, DockAction::Command(','));
        assert_eq!(very_narrow[0].tone, DockButtonTone::Urgent);
        assert_eq!(very_narrow[1].action, DockAction::Command('e'));
        assert_eq!(very_narrow[2].action, DockAction::OpenContextActions);
        assert_eq!(very_narrow[2].label, context_options_count_label(4));
        assert_eq!(very_narrow[3].action, DockAction::OpenPalette);
        assert_eq!(
            context_overlay_commands(&state, 320.0),
            vec!['z', 'w', 's', '.']
        );
        let two_columns = gameplay_dock_specs(&state, 150.0);
        assert_eq!(two_columns.len(), 2);
        assert_eq!(
            two_columns[0].action,
            DockAction::OpenContextActions,
            "when no direct slot fits, Možnosti retains every ranked command"
        );
        assert_eq!(
            two_columns[0].label,
            context_options_count_label(ranked_gameplay_commands(&state).len())
        );
        assert_eq!(two_columns[1].action, DockAction::OpenPalette);

        let overlay_specs =
            palette_content_specs(&state, DockMode::ContextActions { page: 0 }, 320.0);
        let overlay_commands: Vec<_> = overlay_specs
            .into_iter()
            .filter_map(|spec| match spec.action {
                DockAction::Command(command) => Some(command),
                _ => None,
            })
            .collect();
        assert_eq!(overlay_commands, vec!['z', 'w', 's', '.']);
        assert!(overlay_commands.iter().all(|command| {
            !very_narrow[..2]
                .iter()
                .any(|spec| spec.action == DockAction::Command(*command))
        }));

        state.game.player.pos = state.game.dungeon.stairs;
        state.game.floor_items.clear();
        state.game.player.inventory.clear();
        let without_amulet = contextual_commands(&state);
        assert_eq!(without_amulet[0], '>');
        assert!(!without_amulet.contains(&'<'));

        state.game.has_amulet = true;
        let with_amulet = contextual_commands(&state);
        assert_eq!(with_amulet, vec!['<', '>']);
        let stairs = gameplay_dock_specs(&state, 320.0);
        assert_eq!(stairs[0].action, DockAction::Command('<'));
        assert_eq!(stairs[1].action, DockAction::Command('>'));
        assert_eq!(stairs[0].tone, DockButtonTone::Urgent);
        assert_eq!(stairs[1].tone, DockButtonTone::Urgent);

        state.game.player.pos = mrzavec::map::Pos::new(0, 0);
        state.game.has_amulet = false;
        state.game.player.inventory.clear();
        state.game.player.armor = Some(990_024);
        state.game.player.rings[0] = Some(990_025);
        let equipped_actions = contextual_commands(&state);
        assert!(equipped_actions.contains(&'T'));
        assert!(equipped_actions.contains(&'R'));

        state.pending = Some(Pending::Help);
        state.modal = Some(Modal::full_screen(help_text()));
        assert!(contextual_commands(&state).is_empty());
    }

    #[test]
    fn metadata_ranking_is_urgent_first_and_stable_for_ties() {
        let ranked = rank_dock_commands(['P', '.', 'W', '>', 's', 'w', ',']);
        let commands: Vec<_> = ranked.iter().map(|command| command.command).collect();
        assert_eq!(commands, vec![',', '>', 'w', 'W', 'P', 's', '.']);
        assert_eq!(ranked[0].importance, DockImportance::Urgent);
        assert_eq!(ranked[1].importance, DockImportance::Urgent);
        assert_eq!(ranked[2].priority, ranked[3].priority);
        assert!(ranked[2].declaration_order < ranked[3].declaration_order);
    }

    #[test]
    fn urgent_tone_is_persistent_inverse_and_arms_as_inverse_of_inverse() {
        assert_eq!(
            button_colors(DockButtonTone::Urgent, false, false),
            (DOCK_BUTTON_PRESSED_COLOR, Color::BLACK, GLYPH_COLOR)
        );
        assert_eq!(
            button_colors(DockButtonTone::Urgent, true, false),
            (DOCK_BUTTON_COLOR, GLYPH_COLOR, DOCK_BORDER_COLOR)
        );
    }

    #[test]
    fn inventory_fallback_is_nonurgent_and_options_disable_without_overflow() {
        let mut state = state(90_029);
        state.game.player.inventory.clear();
        state.game.player.weapon = None;
        state.game.player.armor = None;
        state.game.player.rings = [None, None];
        state.game.floor_items.clear();

        let ranked = rank_dock_commands(['i', 's', '.']);
        let commands: Vec<_> = ranked.iter().map(|command| command.command).collect();
        assert_eq!(commands, vec!['i', 's', '.']);
        assert_eq!(ranked[0].importance, DockImportance::Fallback);
        assert_eq!(dock_command_tone('i'), DockButtonTone::Normal);

        let wide = gameplay_dock_specs(&state, 984.0);
        assert_eq!(wide[0].action, DockAction::Command('s'));
        assert_eq!(wide[1].action, DockAction::Command('.'));
        assert_eq!(wide[wide.len() - 2].action, DockAction::Disabled);
        assert_eq!(wide[wide.len() - 2].label, context_options_heading());
        assert_eq!(wide[wide.len() - 1].action, DockAction::OpenPalette);
        assert!(context_overlay_commands(&state, 984.0).is_empty());
    }

    #[test]
    fn gameplay_dock_exposes_each_matching_inventory_action_in_isolation() {
        for (offset, kind, command) in [
            (0, ItemKind::Potion, 'q'),
            (1, ItemKind::Scroll, 'r'),
            (2, ItemKind::Food, 'e'),
            (3, ItemKind::Stick, 'z'),
            (4, ItemKind::Weapon, 'w'),
            (5, ItemKind::Armor, 'W'),
            (6, ItemKind::Ring, 'P'),
        ] {
            let mut state = state(90_030 + offset);
            state.game.player.inventory = vec![pack_item(990_030 + offset, kind, 0, 'a')];
            state.game.player.weapon = None;
            state.game.player.armor = None;
            state.game.player.rings = [None, None];
            state.game.floor_items.clear();

            let actions = contextual_commands(&state);
            assert!(
                actions.contains(&command),
                "{command} should be offered for {kind:?}"
            );
        }
    }

    #[test]
    fn gameplay_dock_is_one_map_aligned_row_with_fixed_final_anchors() {
        let base_state = state(90_022);
        let mut contextual = state(90_022);
        let mut item = mrzavec::item::Item::basic(990_022, ItemKind::Potion, 0);
        item.pos = Some(contextual.game.player.pos);
        contextual.game.floor_items.push(item);

        for (window_size, current) in [
            (Vec2::new(320.0, 480.0), &base_state),
            (Vec2::new(400.0, 800.0), &base_state),
            (Vec2::new(1366.0, 768.0), &base_state),
            (Vec2::new(320.0, 480.0), &contextual),
            (Vec2::new(400.0, 800.0), &contextual),
            (Vec2::new(1366.0, 768.0), &contextual),
        ] {
            let screen = calculate_screen_layout(window_size, current, DockMode::Gameplay);
            assert_eq!(screen.dock.rows, 1);
            assert_eq!(screen.dock.height, dock_height(1));
            assert_eq!(screen.dock.rail_left, screen.terminal.origin.x);
            assert_eq!(screen.dock.rail_width, screen.terminal.rendered_size.x);
            assert!(screen.dock.button_width >= DOCK_MIN_BUTTON_WIDTH);
            let options = screen.base_specs.len() - 2;
            assert!(
                matches!(
                    screen.base_specs[options].action,
                    DockAction::OpenContextActions | DockAction::Disabled
                ),
                "Možnosti must be the penultimate gameplay control"
            );
            assert_eq!(
                screen.base_specs[options + 1].action,
                DockAction::OpenPalette,
                "Komandy must be the far-right gameplay control"
            );
            assert!(screen.base_specs[options + 1].label.contains("Komandy"));
            assert!(screen.base_specs[options].label.contains("Možnosti"));
            assert_eq!(
                screen
                    .base_specs
                    .iter()
                    .filter(|spec| spec.label.contains("Komandy"))
                    .count(),
                1,
                "normal gameplay exposes exactly one global Komandy control"
            );
        }

        let narrow =
            calculate_screen_layout(Vec2::new(320.0, 480.0), &contextual, DockMode::Gameplay);
        let wide =
            calculate_screen_layout(Vec2::new(1366.0, 768.0), &contextual, DockMode::Gameplay);
        let direct_count = |layout: &ScreenLayout| {
            layout.base_specs[..layout.base_specs.len() - 2]
                .iter()
                .filter(|spec| matches!(spec.action, DockAction::Command(_)))
                .count()
        };
        assert!(direct_count(&wide) > direct_count(&narrow));
    }

    #[test]
    fn responsive_density_uses_rendered_map_width_not_raw_window_width() {
        let state = state(90_023);
        let short_wide_window =
            calculate_screen_layout(Vec2::new(1000.0, 400.0), &state, DockMode::Gameplay);
        assert!(
            gameplay_columns(short_wide_window.terminal.rendered_size.x) < gameplay_columns(1000.0)
        );
        assert_eq!(
            short_wide_window.base_specs.len(),
            gameplay_columns(short_wide_window.terminal.rendered_size.x)
        );
        assert_eq!(
            short_wide_window.dock.rail_width,
            short_wide_window.terminal.rendered_size.x
        );
    }

    #[test]
    fn gameplay_capacity_is_monotonic_as_the_terminal_rail_grows() {
        let mut previous = gameplay_columns(0.0);
        for width in 1..=1_400 {
            let current = gameplay_columns(width as f32);
            assert!(
                current >= previous,
                "capacity fell from {previous} to {current} at {width}px"
            );
            previous = current;
        }
        assert!(gameplay_columns(984.0) > gameplay_columns(400.0));
    }

    #[test]
    fn prompt_rows_are_content_derived_and_keep_minimum_targets() {
        let mut confirmation = state(90_024);
        confirmation.pending = Some(Pending::QuitConfirm);
        confirmation.modal = Some(Modal::inline_prompt("Iziti?"));
        let confirmation_layout =
            calculate_screen_layout(Vec2::new(400.0, 800.0), &confirmation, DockMode::Gameplay);
        assert_eq!(confirmation_layout.base_specs.len(), 3);
        assert_eq!(confirmation_layout.dock.rows, 1);
        assert_eq!(confirmation_layout.dock.height, dock_height(1));

        let mut direction = state(90_025);
        direction.pending = Some(Pending::MoveDirection);
        direction.modal = Some(Modal::overlay("Kamo?"));
        let direction_layout =
            calculate_screen_layout(Vec2::new(400.0, 800.0), &direction, DockMode::Gameplay);
        assert_eq!(direction_layout.base_specs.len(), 9);
        assert_eq!(direction_layout.dock.rows, 2);
        assert!(direction_layout.dock.button_width >= DOCK_MIN_BUTTON_WIDTH);

        let very_narrow =
            calculate_screen_layout(Vec2::new(320.0, 480.0), &direction, DockMode::Gameplay);
        assert_eq!(very_narrow.dock.rows, 3);
        assert!(very_narrow.dock.button_width >= DOCK_MIN_BUTTON_WIDTH);
        assert!(very_narrow.dock.height < 480.0);
    }

    #[test]
    fn item_and_option_choices_use_paginated_unscaled_overlay_controls() {
        let mut picky = state(90_051);
        picky.game.player.inventory = (0..26)
            .map(|index| {
                pack_item(
                    92_000 + index,
                    ItemKind::Potion,
                    0,
                    (b'a' + index as u8) as char,
                )
            })
            .collect();
        picky.modal = picky_inventory_prompt(&mut picky);

        let first = calculate_screen_layout_with_prompt_page(
            Vec2::new(400.0, 800.0),
            &picky,
            DockMode::Gameplay,
            0,
        );
        let first_overlay = first.overlay.expect("item choices use a Bevy overlay");
        assert_eq!(first_overlay.content.len(), PROMPT_CHOICE_PAGE_SIZE);
        assert!(
            first_overlay
                .content
                .iter()
                .all(|spec| spec.layout == DockButtonLayout::FullWidth)
        );
        assert_eq!(
            first_overlay
                .navigation
                .iter()
                .map(|spec| spec.action)
                .collect::<Vec<_>>(),
            vec![
                DockAction::Disabled,
                DockAction::NextPromptPage,
                DockAction::Command('\u{1b}')
            ]
        );

        let last = calculate_screen_layout_with_prompt_page(
            Vec2::new(400.0, 800.0),
            &picky,
            DockMode::Gameplay,
            4,
        );
        let last_overlay = last.overlay.expect("last item page remains available");
        assert_eq!(last.terminal, first.terminal);
        assert_eq!(
            last_overlay
                .content
                .iter()
                .map(|spec| spec.action)
                .collect::<Vec<_>>(),
            vec![DockAction::Command('y'), DockAction::Command('z')]
        );
        assert_eq!(
            last_overlay
                .navigation
                .iter()
                .map(|spec| spec.action)
                .collect::<Vec<_>>(),
            vec![
                DockAction::PreviousPromptPage,
                DockAction::Disabled,
                DockAction::Command('\u{1b}')
            ]
        );

        let mut options = state(90_052);
        show_option(&mut options, 0);
        let option_page = calculate_screen_layout_with_prompt_page(
            Vec2::new(320.0, 480.0),
            &options,
            DockMode::Gameplay,
            1,
        );
        let option_overlay = option_page.overlay.expect("options use a Bevy overlay");
        assert_eq!(
            option_overlay
                .content
                .iter()
                .map(|spec| spec.action)
                .collect::<Vec<_>>(),
            (6..OPTION_COUNT)
                .map(DockAction::OptionRow)
                .collect::<Vec<_>>()
        );

        let old = options.game.options.passgo;
        let starting_turn = options.game.turn;
        let mut dock_ui = DockUi {
            prompt_pending: options.pending,
            ..default()
        };
        let mut injected = InjectedInput::default();
        apply_dock_action(
            DockAction::OptionRow(4),
            &mut dock_ui,
            &mut injected,
            &mut options,
        );
        assert_eq!(options.game.options.passgo, !old);
        assert_eq!(options.game.turn, starting_turn);
        assert_eq!(injected.0, None);

        dock_ui.prompt_page = 1;
        apply_dock_action(
            DockAction::OptionRow(7),
            &mut dock_ui,
            &mut injected,
            &mut options,
        );
        normalize_prompt_choice_state(&options, &mut dock_ui);
        assert_eq!(options.pending, Some(Pending::Options(7)));
        assert_eq!(
            dock_ui.prompt_page, 1,
            "activating a page-two option must keep its edited row visible"
        );
        let edited = calculate_screen_layout_with_prompt_page(
            Vec2::new(320.0, 480.0),
            &options,
            DockMode::Gameplay,
            dock_ui.prompt_page,
        );
        let edited_overlay = edited.overlay.expect("edited option remains overlaid");
        let edited_spec = edited_overlay
            .content
            .iter()
            .find(|spec| spec.action == DockAction::OptionRow(7))
            .expect("active string option remains on the visible page");
        assert_eq!(
            Some(edited_spec.label.as_str()),
            options
                .modal
                .as_ref()
                .and_then(|modal| modal.text.lines().nth(7))
        );
    }

    #[test]
    fn palette_is_a_map_aligned_overlay_that_never_rescales_the_terminal() {
        let state = state(90_026);
        for window_size in [
            Vec2::new(984.0, 760.0),
            Vec2::new(400.0, 800.0),
            Vec2::new(800.0, 360.0),
        ] {
            let gameplay = calculate_screen_layout(window_size, &state, DockMode::Gameplay);
            let categories =
                calculate_screen_layout(window_size, &state, DockMode::Categories { page: 0 });
            assert_eq!(categories.terminal, gameplay.terminal);
            assert_eq!(categories.dock, gameplay.dock);
            assert_eq!(categories.dock.rail_left, categories.terminal.origin.x);
            assert_eq!(
                categories.dock.rail_width,
                categories.terminal.rendered_size.x
            );
            let overlay = categories.overlay.expect("categories use an overlay");
            assert!(overlay.height <= window_size.y - DOCK_GAP * 2.0 + f32::EPSILON);
            assert_eq!(
                overlay.height,
                palette_overlay_height(overlay.content.len())
            );
        }
    }

    #[test]
    fn palette_breadcrumbs_page_counts_and_navigation_slots_are_stable() {
        let state = state(90_027);
        let context_heading = palette_heading(&state, DockMode::ContextActions { page: 0 }, 320.0);
        assert!(context_heading.starts_with(&context_options_heading()));
        assert!(!context_heading.contains(&commands_heading()));

        let category_pages = mode_page_count(&state, DockMode::Categories { page: 0 }, 400.0);
        assert_eq!(
            category_pages,
            CommandCategory::ALL.len().div_ceil(PALETTE_PAGE_SIZE)
        );
        assert_eq!(
            palette_content_specs(&state, DockMode::Categories { page: 0 }, 400.0).len(),
            PALETTE_PAGE_SIZE
        );
        assert_eq!(
            palette_content_specs(&state, DockMode::Categories { page: 1 }, 400.0).len(),
            CommandCategory::ALL.len() - PALETTE_PAGE_SIZE
        );
        let first_categories =
            palette_navigation_specs(&state, DockMode::Categories { page: 0 }, 400.0);
        let last_categories = palette_navigation_specs(
            &state,
            DockMode::Categories {
                page: category_pages - 1,
            },
            400.0,
        );
        assert_eq!(first_categories[1].action, DockAction::Disabled);
        assert_eq!(first_categories[2].action, DockAction::NextPage);
        assert_eq!(last_categories[1].action, DockAction::PreviousPage);
        assert_eq!(last_categories[2].action, DockAction::Disabled);

        let category = CommandCategory::Information;
        let pages = mode_page_count(&state, DockMode::Category { category, page: 0 }, 400.0);
        assert!(pages > 1);
        let heading = palette_heading(&state, DockMode::Category { category, page: 1 }, 400.0);
        assert!(heading.contains('/'));
        assert!(heading.ends_with(&format!("2/{pages}")));

        let first =
            palette_navigation_specs(&state, DockMode::Category { category, page: 0 }, 400.0);
        let last = palette_navigation_specs(
            &state,
            DockMode::Category {
                category,
                page: pages - 1,
            },
            400.0,
        );
        assert_eq!(first.len(), 4);
        assert_eq!(last.len(), 4);
        assert_eq!(first[1].action, DockAction::Disabled);
        assert_eq!(last[2].action, DockAction::Disabled);
        assert_eq!(first[0].action, DockAction::BackToCategories);
        assert_eq!(last[3].action, DockAction::ClosePalette);
    }

    #[test]
    fn injected_dock_input_uses_the_normal_keyboard_path() {
        let mut initial = state(90_002);
        initial.game.player.inventory = vec![pack_item(91_000, ItemKind::Potion, 0, 'a')];
        let mut app = keyboard_app(initial);
        app.world_mut().resource_mut::<InjectedInput>().0 = Some('q');
        press_keys(&mut app, &[KeyCode::ShiftLeft]);

        app.update();

        let state = app.world().resource::<State>();
        assert_eq!(state.pending, Some(Pending::Quaff));
        assert!(
            state
                .modal
                .as_deref()
                .is_some_and(|modal| modal.starts_with("a) "))
        );
    }

    #[test]
    fn dock_button_uses_the_command_path_and_cancels_travel() {
        let mut initial = state(90_010);
        initial.game.player.inventory = vec![pack_item(91_010, ItemKind::Potion, 0, 'a')];
        let target = seen_passable_neighbor(&initial.game);
        assert!(initial.game.start_travel(target));
        let mut app = dock_input_app(initial);
        let button = spawn_test_dock_button(&mut app, DockAction::Command('q'));

        arm_test_dock_button(&mut app, button);
        assert!(app.world().resource::<State>().game.is_traveling());
        assert_eq!(app.world().resource::<State>().pending, None);

        release_test_dock_button(&mut app, button);

        let current = app.world().resource::<State>();
        assert!(!current.game.is_traveling());
        assert_eq!(current.pending, Some(Pending::Quaff));
        assert!(
            current
                .modal
                .as_deref()
                .is_some_and(|modal| modal.starts_with("a) "))
        );
        let turn = current.game.turn;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.update();
        assert_eq!(app.world().resource::<State>().game.turn, turn);
    }

    #[test]
    fn pressing_any_dock_button_cancels_travel_before_another_step() {
        let mut initial = state(90_049);
        let target = seen_passable_neighbor(&initial.game);
        assert!(initial.game.start_travel(target));
        let starting_turn = initial.game.turn;
        let mut app = dock_input_app(initial);
        app.add_systems(Update, advance_pointer_travel.after(keyboard));
        let button = spawn_test_dock_button(&mut app, DockAction::Command('s'));

        arm_test_dock_button(&mut app, button);

        let state = app.world().resource::<State>();
        assert!(!state.game.is_traveling());
        assert_eq!(
            state.game.turn, starting_turn,
            "pointer-down must cancel travel before the release activates a command"
        );
        assert!(app.world().resource::<DockPress>().armed.is_some());
        assert_eq!(app.world().resource::<InjectedInput>().0, None);
    }

    #[test]
    fn dragging_outside_cancels_a_dock_action_even_if_release_returns_inside() {
        let initial_turn = state(90_011).game.turn;
        let mut app = dock_input_app(state(90_011));
        let button = spawn_test_dock_button(&mut app, DockAction::Command('.'));
        arm_test_dock_button(&mut app, button);

        set_test_cursor(&mut app, Vec2::new(200.0, 200.0));
        *app.world_mut()
            .entity_mut(button)
            .get_mut::<Interaction>()
            .expect("test dock button has interaction") = Interaction::None;
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();

        release_test_dock_button(&mut app, button);
        assert_eq!(app.world().resource::<State>().game.turn, initial_turn);
        assert_eq!(app.world().resource::<InjectedInput>().0, None);
        assert!(app.world().resource::<DockPress>().armed.is_none());
    }

    #[test]
    fn disabled_and_secondary_pointers_cannot_replace_an_armed_action() {
        let first = ArmedDockButton {
            entity: Entity::from_bits(1),
            action: DockAction::Command('.'),
            pointer: DockPointer::Touch(7),
            canceled: false,
        };
        let second = ArmedDockButton {
            entity: Entity::from_bits(2),
            action: DockAction::Command('s'),
            pointer: DockPointer::Touch(8),
            canceled: false,
        };
        let mut press = DockPress::default();
        assert!(press.try_arm(first));
        assert!(!press.try_arm(second));
        assert_eq!(press.armed, Some(first));

        let mut dock_ui = DockUi::default();
        let mut injected = InjectedInput::default();
        let mut state = state(90_048);
        apply_dock_action(
            DockAction::Disabled,
            &mut dock_ui,
            &mut injected,
            &mut state,
        );
        assert_eq!(dock_ui.mode, DockMode::Gameplay);
        assert_eq!(injected.0, None);
    }

    #[test]
    fn dock_search_wait_and_pickup_match_keyboard_commands() {
        for (seed, command, key) in [(90_040, 's', KeyCode::KeyS), (90_041, '.', KeyCode::Period)] {
            let initial_turn = state(seed).game.turn;
            let mut dock_app = dock_input_app(state(seed));
            let button = spawn_test_dock_button(&mut dock_app, DockAction::Command(command));
            arm_test_dock_button(&mut dock_app, button);
            assert_eq!(dock_app.world().resource::<State>().game.turn, initial_turn);
            release_test_dock_button(&mut dock_app, button);

            let mut keyboard_app = keyboard_app(state(seed));
            press_keys(&mut keyboard_app, &[key]);
            keyboard_app.update();

            let dock_turn = dock_app.world().resource::<State>().game.turn;
            let keyboard_turn = keyboard_app.world().resource::<State>().game.turn;
            assert_eq!(dock_turn, keyboard_turn);
            assert_eq!(dock_turn, initial_turn + 1);
        }

        fn pickup_state(seed: u64, id: u64) -> State {
            let mut state = state(seed);
            state.game.floor_items.clear();
            let mut item = mrzavec::item::Item::basic(id, ItemKind::Potion, 0);
            item.pos = Some(state.game.player.pos);
            state.game.floor_items.push(item);
            state
        }

        let item_id = 990_042;
        let mut dock_app = dock_input_app(pickup_state(90_042, item_id));
        let button = spawn_test_dock_button(&mut dock_app, DockAction::Command(','));
        arm_test_dock_button(&mut dock_app, button);
        release_test_dock_button(&mut dock_app, button);

        let mut keyboard_app = keyboard_app(pickup_state(90_042, item_id));
        press_keys(&mut keyboard_app, &[KeyCode::Comma]);
        keyboard_app.update();

        for app in [&dock_app, &keyboard_app] {
            let state = app.world().resource::<State>();
            assert!(
                state
                    .game
                    .player
                    .inventory
                    .iter()
                    .any(|item| item.id == item_id && item.in_pack)
            );
            assert!(state.game.floor_items.iter().all(|item| item.id != item_id));
        }
        assert_eq!(
            dock_app.world().resource::<State>().game.turn,
            keyboard_app.world().resource::<State>().game.turn
        );
    }

    #[test]
    fn palette_navigation_is_free_and_selection_executes_exactly_one_command() {
        let initial = state(90_024);
        let starting_turn = initial.game.turn;
        let mut app = dock_input_app(initial);
        let palette = spawn_test_dock_button(&mut app, DockAction::OpenPalette);
        arm_test_dock_button(&mut app, palette);
        release_test_dock_button(&mut app, palette);

        assert_eq!(
            app.world().resource::<DockUi>().mode,
            DockMode::Categories { page: 0 }
        );
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn);
        assert_eq!(app.world().resource::<State>().pending, None);

        press_keys(&mut app, &[KeyCode::Escape]);
        app.update();

        assert_eq!(app.world().resource::<DockUi>().mode, DockMode::Gameplay);
        assert_eq!(app.world().resource::<State>().game.turn, starting_turn);
        assert_eq!(app.world().resource::<State>().pending, None);

        press_keys(&mut app, &[]);
        let palette = spawn_test_dock_button(&mut app, DockAction::OpenPalette);
        arm_test_dock_button(&mut app, palette);
        release_test_dock_button(&mut app, palette);

        let help = spawn_test_dock_button(&mut app, DockAction::Command('?'));
        arm_test_dock_button(&mut app, help);
        release_test_dock_button(&mut app, help);

        let state = app.world().resource::<State>();
        assert_eq!(state.pending, Some(Pending::Help));
        assert_eq!(state.game.turn, starting_turn);
        assert_eq!(app.world().resource::<DockUi>().mode, DockMode::Gameplay);
    }

    #[test]
    fn palette_overlay_intercepts_map_pointer_input() {
        let initial = state(90_028);
        let target = seen_passable_neighbor(&initial.game);
        let player = initial.game.player.pos;
        let target_cell = PointerCell {
            column: target.x as usize,
            row: DUNGEON_FIRST_ROW + target.y as usize - 1,
        };
        let mut app = pointer_keyboard_app(initial);
        app.world_mut().resource_mut::<DockUi>().mode = DockMode::Categories { page: 0 };

        click_cell(&mut app, target_cell);

        let state = app.world().resource::<State>();
        assert_eq!(state.game.player.pos, player);
        assert!(!state.game.is_traveling());
    }

    #[test]
    fn end_screen_escape_button_and_keyboard_escape_exit() {
        for injected in [false, true] {
            let mut ended = state(90_025 + u64::from(injected));
            ended.game.end = mrzavec::game::EndState::Quit;
            ended.score_recorded = true;
            ended.modal = Some(Modal::full_screen("Konec"));
            assert!(
                dock_specs(&ended, DockMode::Gameplay)
                    .iter()
                    .any(|spec| spec.action == DockAction::Command('\u{1b}'))
            );
            let mut app = keyboard_app(ended);
            if injected {
                app.world_mut().resource_mut::<InjectedInput>().0 = Some('\u{1b}');
            } else {
                press_keys(&mut app, &[KeyCode::Escape]);
            }

            app.update();

            assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 1);
        }
    }

    #[test]
    fn identify_glyph_is_inline_without_prior_events_and_pointer_selection_closes_it() {
        let mut initial = state(90_013);
        initial.game.messages.clear();
        initial.game.recall_message.clear();
        assert!(initial.visible_message.is_none());
        let player = initial.game.player.pos;
        let player_row = DUNGEON_FIRST_ROW + player.y as usize - 1;
        let mut app = pointer_keyboard_app(initial);

        press_keys(&mut app, &[KeyCode::Slash]);
        app.update();

        {
            let state = app.world().resource::<State>();
            assert_eq!(state.pending, Some(Pending::IdentifyGlyph));
            assert_eq!(
                state.modal.as_ref().unwrap().presentation,
                ModalPresentation::InlinePrompt
            );
            assert!(state.visible_message.is_none());
            let buffer = display(state);
            let events = (0..EVENT_ROWS)
                .map(|row| display_row(&buffer, row))
                .collect::<String>();
            assert!(events.contains("Čto hoćeš opoznati?"));
            assert_eq!(buffer[player_row * DISPLAY_WIDTH + player.x as usize], '@');
            assert!(display_row(&buffer, STATUS_ROW).starts_with("Stųp:"));
            assert_eq!(buffer.len(), DISPLAY_WIDTH * (STATUS_ROW + 1));
            assert!(
                dock_specs(state, DockMode::Gameplay)
                    .iter()
                    .any(|control| control.action == DockAction::Command('\u{1b}'))
            );
        }

        press_keys(&mut app, &[]);
        click_cell(
            &mut app,
            PointerCell {
                column: player.x as usize,
                row: player_row,
            },
        );

        let state = app.world().resource::<State>();
        assert!(state.pending.is_none());
        assert!(state.modal.is_none());
        assert!(
            state
                .game
                .messages
                .last()
                .is_some_and(|message| message.contains("'@': ty"))
        );
    }

    #[test]
    fn confirmation_and_discovery_prompts_are_inline_without_prior_events() {
        for (keys, pending) in [
            ([KeyCode::ShiftLeft, KeyCode::KeyQ], Pending::QuitConfirm),
            ([KeyCode::ShiftLeft, KeyCode::KeyD], Pending::Discoveries),
        ] {
            let mut initial = state(90_014);
            initial.game.messages.clear();
            initial.game.recall_message.clear();
            let player = initial.game.player.pos;
            let player_row = DUNGEON_FIRST_ROW + player.y as usize - 1;
            let mut app = keyboard_app(initial);

            press_keys(&mut app, &keys);
            app.update();

            let state = app.world().resource::<State>();
            assert_eq!(state.pending, Some(pending));
            assert_eq!(
                state.modal.as_ref().unwrap().presentation,
                ModalPresentation::InlinePrompt
            );
            assert!(state.visible_message.is_none());
            let buffer = display(state);
            assert_eq!(buffer[player_row * DISPLAY_WIDTH + player.x as usize], '@');
            assert!(display_row(&buffer, STATUS_ROW).starts_with("Stųp:"));
            let actions: Vec<_> = dock_specs(state, DockMode::Gameplay)
                .into_iter()
                .map(|spec| spec.action)
                .collect();
            assert!(actions.contains(&DockAction::Command('\u{1b}')));
            if pending == Pending::QuitConfirm {
                assert!(actions.contains(&DockAction::Command('y')));
                assert!(actions.contains(&DockAction::Command('n')));
            }
        }
    }

    #[test]
    fn pointer_routes_item_rows_directions_map_identification_and_cancel() {
        let mut item_state = state(90_003);
        item_state.game.player.inventory = vec![
            pack_item(91_001, ItemKind::Potion, 0, 'a'),
            pack_item(91_002, ItemKind::Potion, 1, 'b'),
        ];
        item_state.modal = select_item_menu(&mut item_state, Pending::Quaff);
        assert_eq!(
            pointer_action_at_cell(&item_state, PointerCell { column: 70, row: 1 }),
            Some(PointerAction::Key('b'))
        );
        assert!(
            dock_specs(&item_state, DockMode::Gameplay)
                .iter()
                .any(|spec| spec.action == DockAction::Command('\u{1b}'))
        );

        let mut picky = state(90_012);
        picky.game.player.inventory = vec![
            pack_item(91_012, ItemKind::Potion, 0, 'a'),
            pack_item(91_013, ItemKind::Food, 0, 'b'),
        ];
        picky.modal = picky_inventory_prompt(&mut picky);
        assert_eq!(
            pointer_action_at_cell(&picky, PointerCell { column: 70, row: 2 }),
            Some(PointerAction::Key('b'))
        );
        let mut app = pointer_keyboard_app(picky);
        click_cell(&mut app, PointerCell { column: 70, row: 2 });
        let picky = app.world().resource::<State>();
        assert_eq!(picky.pending, None);
        assert!(
            picky
                .game
                .messages
                .last()
                .is_some_and(|line| line.starts_with("b) "))
        );

        let mut paged_picky = state(90_013);
        paged_picky.game.player.inventory = (0..26)
            .map(|index| {
                pack_item(
                    91_100 + index,
                    ItemKind::Potion,
                    0,
                    (b'a' + index as u8) as char,
                )
            })
            .collect();
        paged_picky.modal = picky_inventory_prompt(&mut paged_picky);
        paged_picky
            .modal
            .as_mut()
            .expect("full picky inventory uses a modal")
            .offset = MODAL_PAGE_ROWS;
        assert_eq!(
            pointer_action_at_cell(&paged_picky, PointerCell { column: 70, row: 0 }),
            Some(PointerAction::Key('y'))
        );
        assert_eq!(
            pointer_action_at_cell(&paged_picky, PointerCell { column: 70, row: 1 }),
            Some(PointerAction::Key('z'))
        );
        let mut app = pointer_keyboard_app(paged_picky);
        click_cell(&mut app, PointerCell { column: 70, row: 1 });
        let paged_picky = app.world().resource::<State>();
        assert_eq!(paged_picky.pending, None);
        assert!(
            paged_picky
                .game
                .messages
                .last()
                .is_some_and(|line| line.starts_with("z) "))
        );

        let mut direction_state = state(90_004);
        direction_state.modal = direction_prompt(&mut direction_state, Pending::ZapDirection(1));
        assert_eq!(
            direction_state.modal.as_ref().unwrap().presentation,
            ModalPresentation::Overlay
        );
        let direction_controls = dock_specs(&direction_state, DockMode::Gameplay);
        for command in ['h', 'j', 'k', 'l', 'y', 'u', 'b', 'n', '\u{1b}'] {
            assert!(
                direction_controls
                    .iter()
                    .any(|spec| spec.action == DockAction::Command(command))
            );
        }
        let direction_buffer = display(&direction_state);
        let direction_player = direction_state.game.player.pos;
        let direction_player_row = DUNGEON_FIRST_ROW + direction_player.y as usize - 1;
        assert_eq!(
            direction_buffer[direction_player_row * DISPLAY_WIDTH + direction_player.x as usize],
            '@'
        );
        let start_x = overlay_start_x(direction_state.modal.as_deref().unwrap());
        assert_eq!(
            pointer_action_at_cell(
                &direction_state,
                PointerCell {
                    column: start_x,
                    row: 1,
                },
            ),
            Some(PointerAction::Key('h'))
        );
        let player = direction_state.game.player.pos;
        let target_column = if player.x > 0 { 0 } else { 1 };
        let expected = if target_column < player.x { 'h' } else { 'l' };
        assert_eq!(
            pointer_action_at_cell(
                &direction_state,
                PointerCell {
                    column: target_column as usize,
                    row: DUNGEON_FIRST_ROW + player.y as usize - 1,
                },
            ),
            Some(PointerAction::Key(expected))
        );

        let mut identify = state(90_005);
        identify.pending = Some(Pending::IdentifyGlyph);
        identify.modal = Some(Modal::inline_prompt("Čto hoćeš opoznati?"));
        let target = identify.game.player.pos;
        assert_eq!(
            pointer_action_at_cell(
                &identify,
                PointerCell {
                    column: target.x as usize,
                    row: DUNGEON_FIRST_ROW + target.y as usize - 1,
                },
            ),
            Some(PointerAction::Key('@'))
        );
    }

    #[test]
    fn pointer_prompt_answers_options_and_more_are_semantic_actions() {
        let mut confirm = state(90_006);
        confirm.pending = Some(Pending::QuitConfirm);
        confirm.modal = Some(Modal::inline_prompt("Istinno li izhoditi?"));
        let controls = dock_specs(&confirm, DockMode::Gameplay);
        assert_eq!(
            controls
                .iter()
                .map(|control| control.action)
                .collect::<Vec<_>>(),
            vec![
                DockAction::Command('y'),
                DockAction::Command('n'),
                DockAction::Command('\u{1b}')
            ]
        );
        assert!(
            display_row(&display(&confirm), EVENT_ROWS - 1)
                .trim()
                .is_empty()
        );

        let mut options = state(90_007);
        options.pending = Some(Pending::Options(0));
        options.modal = Some(Modal::full_screen(options_text(
            &options.game,
            Some(0),
            None,
            None,
        )));
        assert!(
            dock_specs(&options, DockMode::Gameplay)
                .iter()
                .any(|spec| spec.action == DockAction::Command('\u{1b}'))
        );
        let old = options.game.options.passgo;
        assert_eq!(
            pointer_action_at_cell(&options, PointerCell { column: 40, row: 4 }),
            Some(PointerAction::OptionRow(4))
        );
        apply_option_pointer(&mut options, 4);
        assert_eq!(options.game.options.passgo, !old);
        apply_option_pointer(&mut options, 7);
        assert_eq!(options.pending, Some(Pending::Options(7)));

        let mut more = state(90_008);
        more.pending = Some(Pending::More);
        more.modal = Some(Modal::full_screen("Torba\n --Dalje--"));
        let more_actions: Vec<_> = dock_specs(&more, DockMode::Gameplay)
            .into_iter()
            .map(|spec| spec.action)
            .collect();
        assert!(more_actions.contains(&DockAction::Command(' ')));
        assert!(more_actions.contains(&DockAction::Command('\u{1b}')));
        assert_eq!(
            pointer_action_at_cell(
                &more,
                PointerCell {
                    column: 10,
                    row: DUNGEON_FIRST_ROW,
                },
            ),
            Some(PointerAction::Key(' '))
        );

        let mut app = keyboard_app(more);
        app.world_mut().resource_mut::<InjectedInput>().0 = Some('\u{1b}');
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.pending, None);
        assert_eq!(state.modal, None);
    }

    #[test]
    fn blocking_ui_state_cancels_core_pointer_travel() {
        let mut initial = state(90_009);
        let target = seen_passable_neighbor(&initial.game);
        assert!(initial.game.start_travel(target));
        initial.pending = Some(Pending::Help);
        initial.modal = Some(Modal::full_screen(help_text()));
        let mut app = App::new();
        app.insert_resource(initial);
        app.insert_resource(DockUi::default());
        app.insert_resource(DockPress::default());
        app.add_systems(Update, advance_pointer_travel);

        app.update();

        assert!(!app.world().resource::<State>().game.is_traveling());
    }

    #[test]
    fn opening_either_palette_cancels_core_pointer_travel() {
        for (offset, action, expected_mode) in [
            (0, DockAction::OpenPalette, DockMode::Categories { page: 0 }),
            (
                1,
                DockAction::OpenContextActions,
                DockMode::ContextActions { page: 0 },
            ),
        ] {
            let mut initial = state(90_050 + offset);
            let target = seen_passable_neighbor(&initial.game);
            assert!(initial.game.start_travel(target));
            let starting_turn = initial.game.turn;

            let mut dock_ui = DockUi::default();
            let mut injected = InjectedInput::default();
            apply_dock_action(action, &mut dock_ui, &mut injected, &mut initial);
            assert_eq!(dock_ui.mode, expected_mode);

            let mut app = App::new();
            app.insert_resource(initial);
            app.insert_resource(dock_ui);
            app.insert_resource(DockPress::default());
            app.add_systems(Update, advance_pointer_travel);
            app.update();

            assert!(!app.world().resource::<State>().game.is_traveling());
            assert_eq!(
                app.world().resource::<State>().game.turn,
                starting_turn,
                "opening a palette must cancel before another travel step"
            );
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
        assert_eq!(lines[1], "Ignorovati pisańje podčas boja (\"flush\"): Ne");
        assert_eq!(
            lines[2],
            "Pokazati pozicijų jedino na koncu běga (\"jump\"): Ne"
        );
        assert_eq!(lines[3], "Pokazati osvětljeno tlo (\"seefloor\"): Da");
        assert_eq!(lines[4], "Slědovati povråtam v prohodah (\"passgo\"): Ne");
        assert_eq!(
            lines[5],
            "Pokazati kamenj groba po smŕti (\"tombstone\"): Da"
        );
        assert_eq!(lines[6], "Stiľ torby (\"inven\"): Pomalo");
        assert_eq!(lines[7], "Imę (\"name\"): Rodney");
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
        assert_eq!(erased.lines().nth(7).unwrap(), "Imę (\"name\"): ");
    }

    #[test]
    fn inventory_styles_select_distinct_presentation_paths() {
        let mut state = state(1);
        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Overwrite;
        let overwrite = inventory_modal(&mut state).unwrap();
        assert_eq!(overwrite.presentation, ModalPresentation::Overlay);
        state.modal = Some(overwrite);
        assert!(
            dock_specs(&state, DockMode::Gameplay)
                .iter()
                .any(|spec| spec.action == DockAction::Command('\u{1b}'))
        );
        state.modal = None;

        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Clear;
        let clear = inventory_modal(&mut state).unwrap();
        assert_eq!(clear.presentation, ModalPresentation::FullScreen);

        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Slow;
        let slow = inventory_modal(&mut state).unwrap();
        assert_eq!(slow.presentation, ModalPresentation::FullScreen);
        assert_eq!(state.pending, Some(Pending::SlowInventory(0)));
    }

    #[test]
    fn modal_constructors_require_an_explicit_presentation_and_own_pagination() {
        let cases = [
            (
                Modal::inline_prompt("prompt"),
                ModalPresentation::InlinePrompt,
            ),
            (Modal::overlay("overlay"), ModalPresentation::Overlay),
            (Modal::full_screen("page"), ModalPresentation::FullScreen),
        ];

        for (modal, presentation) in cases {
            assert_eq!(modal.presentation, presentation);
            assert_eq!(modal.offset, 0);
        }

        let capacity = STATUS_ROW;
        let fitting = (0..capacity)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            Modal::event_overlay_or_full_screen(fitting).presentation,
            ModalPresentation::Overlay
        );
        let oversized = (0..=capacity)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            Modal::event_overlay_or_full_screen(oversized).presentation,
            ModalPresentation::FullScreen
        );
    }

    #[test]
    fn modal_controls_live_outside_the_terminal_and_do_not_overwrite_content() {
        let marker = "PROMPT-END-MUST-REMAIN";
        let mut inline = state(90_015);
        inline.visible_message = Some("x".repeat(205));
        inline.pending = Some(Pending::QuitConfirm);
        inline.modal = Some(Modal::inline_prompt(marker));

        let inline_buffer = display(&inline);
        assert!(display_row(&inline_buffer, EVENT_ROWS - 1).contains(marker));
        assert!(
            dock_specs(&inline, DockMode::Gameplay)
                .iter()
                .any(|spec| spec.action == DockAction::Command('\u{1b}'))
        );

        let overlay_marker = "THIRD-CONTENT-LINE-MUST-REMAIN";
        let mut overlay = state(90_016);
        overlay.pending = Some(Pending::More);
        overlay.modal = Some(Modal::overlay(format!("first\nsecond\n{overlay_marker}")));

        let overlay_buffer = display(&overlay);
        assert!(display_row(&overlay_buffer, EVENT_ROWS - 1).contains(overlay_marker));
        assert!(
            dock_specs(&overlay, DockMode::Gameplay)
                .iter()
                .any(|spec| spec.action == DockAction::Command('\u{1b}'))
        );
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
        let prompt = picky_inventory_prompt(&mut multiple).unwrap();
        assert!(prompt.starts_with("Prědmet: \na) "));
        assert_eq!(
            prompt.lines().count(),
            multiple
                .game
                .player
                .inventory
                .iter()
                .filter(|item| item.in_pack)
                .count()
                + 1
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
    fn clear_screen_modal_pages_use_the_compact_terminal() {
        let mut state = state(103);
        state.modal = Some(Modal::full_screen(
            (0..MODAL_PAGE_ROWS + 5)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
        let first = display(&state);
        let first_text = display_row(&first, MODAL_MORE_ROW);
        assert!(first_text.starts_with(" --Dalje--"));

        state.modal.as_mut().unwrap().offset = MODAL_PAGE_ROWS;
        let second = display(&state);
        let second_text = display_row(&second, 0);
        assert!(second_text.starts_with(&format!("line {MODAL_PAGE_ROWS}")));
    }

    #[test]
    fn normal_display_uses_three_event_rows_then_map_and_status() {
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
        assert_eq!(DISPLAY_HEIGHT, 26);
        assert_eq!(STATUS_ROW + 1, DISPLAY_HEIGHT);
    }

    #[test]
    fn question_mark_key_opens_complete_help_immediately() {
        let mut app = keyboard_app(state(105));
        press_keys(&mut app, &[KeyCode::ShiftLeft, KeyCode::Slash]);

        app.update();

        let state = app.world().resource::<State>();
        assert_eq!(state.pending, Some(Pending::Help));
        assert_eq!(
            state.modal.as_ref().unwrap().presentation,
            ModalPresentation::FullScreen
        );
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
            .filter(|entry| entry.print)
            .map(|entry| help_entry_text(entry.command, entry.description))
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
        assert_eq!(state.modal.as_ref().unwrap().offset, MODAL_PAGE_ROWS);
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
        assert_eq!(state.modal.as_ref().unwrap().offset, MODAL_PAGE_ROWS);
        assert_eq!(state.pending, Some(Pending::Help));
        assert!(state.modal.is_some());

        press_keys(&mut app, &[KeyCode::Escape]);
        app.update();
        let state = app.world().resource::<State>();
        assert!(state.pending.is_none());
        assert!(state.modal.is_none());
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
            state.modal = Some(Modal::full_screen("a) bulava"));
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

        game.death_cause = Some("glåda".into());
        let starvation = tombstone_text(&game);
        assert!(starvation.contains("glåda"));

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
        assert!(status.contains("Zdr:  9(12)"));
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
        assert_eq!(
            clear.modal.as_ref().unwrap().presentation,
            ModalPresentation::FullScreen
        );
        assert!(clear.modal.as_deref().unwrap().ends_with(" --Dalje--"));

        let mut overwrite = state(202);
        overwrite.game.options.inventory_style = mrzavec::game::InventoryStyle::Overwrite;
        start_discoveries(&mut overwrite, '*');
        assert_eq!(overwrite.pending, Some(Pending::DiscoveryMore));
        assert_eq!(
            overwrite.modal.as_ref().unwrap().presentation,
            ModalPresentation::Overlay
        );

        let mut slow = state(203);
        slow.game.options.inventory_style = mrzavec::game::InventoryStyle::Slow;
        start_discoveries(&mut slow, '*');
        assert_eq!(slow.pending, Some(Pending::SlowDiscoveryPrompt));
        assert_eq!(slow.slow_discovery_lines.len(), 4);
        assert_eq!(
            slow.modal.as_ref().unwrap().presentation,
            ModalPresentation::InlinePrompt
        );
        assert!(slow.modal.as_deref().unwrap().ends_with("  --Dalje--"));
    }

    #[test]
    fn overwrite_discovery_capacity_includes_the_continuation_line_and_controls() {
        let mut state = state(204);
        state.game.options.inventory_style = mrzavec::game::InventoryStyle::Overwrite;
        state.game.knowledge.potions.fill(false);
        state.game.knowledge.scrolls.fill(false);
        state.game.knowledge.rings.fill(false);
        state.game.knowledge.sticks.fill(false);
        state.game.knowledge.potions[..5].fill(true);
        state.game.knowledge.scrolls[..5].fill(true);
        state.game.knowledge.rings[..5].fill(true);
        state.game.knowledge.sticks[..7].fill(true);

        let mut line_count_game = state.game.clone();
        assert_eq!(
            discovery_lines(&mut line_count_game, Some('*')).len(),
            STATUS_ROW
        );

        start_discoveries(&mut state, '*');

        let modal = state.modal.as_ref().unwrap();
        assert_eq!(modal.text.lines().count(), STATUS_ROW + 1);
        assert_eq!(modal.presentation, ModalPresentation::FullScreen);
        let buffer = display(&state);
        assert!(display_row(&buffer, MODAL_MORE_ROW).starts_with(" --Dalje--"));
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
            .filter(|entry| entry.print)
            .map(|entry| help_entry_text(entry.command, entry.description))
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

        let mut state = state(190);
        state.pending = Some(Pending::MagicDetection);
        state.modal = Some(Modal::full_screen(view));
        let buffer = display(&state);
        assert_eq!(
            state.modal.as_ref().unwrap().presentation,
            ModalPresentation::FullScreen
        );
        assert!(display_row(&buffer, STATUS_ROW).trim().is_empty());
        assert_eq!(buffer.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
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
        assert_eq!(verbose.modal.as_deref(), Some("Lěva rųka ili prava rųka? "));
        collect_messages(&mut verbose);
        assert_eq!(verbose.visible_message.as_deref(), Some("Prošų, L ili R."));
        assert_eq!(verbose.modal.as_deref(), Some("Lěva rųka ili prava rųka? "));

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
        assert!(current_message(&game, Some(weapon), "dŕžiš", None).starts_with("dŕžiš sejčas: "));
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
            current_message(&game, Some(weapon), "dŕžiš", None).starts_with(&format!("{letter}) "))
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
                .contains("'^H' ne jest praviľny prědmet")
        );
        assert!(modal.contains("a) "));
        assert!(!modal.contains("* for list"));
        collect_messages(&mut state);
        assert_eq!(
            state.visible_message.as_deref(),
            Some("'^H' ne jest praviľny prědmet.")
        );
        assert!(state.modal.as_deref().unwrap().contains("a) "));
        assert_eq!(state.game.recall_message, "'^H' ne jest praviľny prědmet");
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
        let displayed = remembered_inline_prompt(&mut state, "v ktorų strånų? ");
        assert_eq!(displayed.text, "V ktorų strånų? ");
        assert_eq!(displayed.presentation, ModalPresentation::InlinePrompt);
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
        assert_eq!(
            state.modal.as_ref().unwrap().presentation,
            ModalPresentation::FullScreen
        );
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
        prompted.modal = Some(Modal::inline_prompt("Čto hoćeš opoznati? "));
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
            assert!(menu.contains(expected), "{pending:?}: {}", menu.text);
            for letter in excluded {
                assert!(!menu.contains(letter), "{pending:?}: {}", menu.text);
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
                .contains("'z' ne jest praviľny prědmet")
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
        assert!(
            throw_direction
                .modal
                .as_deref()
                .is_some_and(|modal| modal.starts_with("V ktorų strånų? \nh       vlěvo"))
        );
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
        assert_eq!(
            ring_result.modal.as_ref().unwrap().presentation,
            ModalPresentation::InlinePrompt
        );
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
            "shråniti fajl (/tmp/rodney.save)? "
        );
    }

    #[test]
    fn quit_confirmation_accepts_only_y_and_every_other_key_cancels() {
        for ch in ['n', 'x', '\u{1b}', ' '] {
            let mut state = state(340);
            state.pending = Some(Pending::QuitConfirm);
            state.modal = Some(Modal::inline_prompt("istinno li ⟨v2:izhoditi⟩?"));
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
        initial_state.modal = Some(Modal::full_screen(
            (0..MODAL_PAGE_ROWS + 5)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
        let mut app = keyboard_app(initial_state);
        press_keys(&mut app, &[KeyCode::Space]);
        app.update();
        assert_eq!(
            app.world()
                .resource::<State>()
                .modal
                .as_ref()
                .unwrap()
                .offset,
            MODAL_PAGE_ROWS
        );

        press_keys(&mut app, &[KeyCode::Space]);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.modal.as_ref().unwrap().offset, 0);
        assert_eq!(state.pending, Some(Pending::Quaff));
        let modal = state.modal.as_deref().unwrap();
        assert!(modal.contains("' ' ne jest praviľny prědmet"));
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
        let verbose = direction_prompt(&mut state, Pending::ZapDirection(1)).unwrap();
        assert!(verbose.starts_with("V ktorų strånų? \nh       vlěvo"));
        assert_eq!(verbose.lines().count(), 9);
        assert_eq!(state.game.recall_message, "v ktorų strånų? ");

        state.game.options.terse = true;
        let terse = direction_prompt(&mut state, Pending::ZapDirection(1)).unwrap();
        assert!(terse.starts_with("Strana: \nh       vlěvo"));
        assert_eq!(terse.lines().count(), 9);
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
