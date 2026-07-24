//! The `?` help screen entries, shared with the language-gate corpus and the
//! pointer command palette. Help text, palette categories, and compact-dock
//! labels deliberately live in one table so a new printable command cannot
//! silently become unreachable by pointer.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Explore,
    Combat,
    Items,
    Equipment,
    Information,
    System,
}

impl CommandCategory {
    pub const ALL: [Self; 6] = [
        Self::Explore,
        Self::Combat,
        Self::Items,
        Self::Equipment,
        Self::Information,
        Self::System,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Explore => "Izslědovańje",
            Self::Combat => "Boj",
            Self::Items => "Prědmet",
            Self::Equipment => "Orųžje i brȯnja",
            Self::Information => "Informacija",
            Self::System => "Igra",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DockImportance {
    Utility,
    Fallback,
    Contextual,
    Urgent,
}

pub const COMMANDS_LABEL: &str = "⟨n:komanda:nom:pl:U⟩…";
pub const CONTEXT_OPTIONS_LABEL: &str = "Možnosti";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpEntry {
    pub command: char,
    pub description: &'static str,
    pub print: bool,
    pub category: CommandCategory,
    pub dock_label: Option<&'static str>,
    pub dock_priority: Option<u8>,
    pub dock_importance: Option<DockImportance>,
}

impl HelpEntry {
    const fn new(
        command: char,
        description: &'static str,
        print: bool,
        category: CommandCategory,
    ) -> Self {
        Self {
            command,
            description,
            print,
            category,
            dock_label: None,
            dock_priority: None,
            dock_importance: None,
        }
    }

    const fn dock(
        command: char,
        description: &'static str,
        category: CommandCategory,
        dock_label: &'static str,
        dock_priority: u8,
        dock_importance: DockImportance,
    ) -> Self {
        Self {
            command,
            description,
            print: true,
            category,
            dock_label: Some(dock_label),
            dock_priority: Some(dock_priority),
            dock_importance: Some(dock_importance),
        }
    }
}

use CommandCategory::{Combat, Equipment, Explore, Information, Items, System};
use DockImportance::{Contextual, Fallback, Urgent, Utility};

pub const HELP_ENTRIES: &[HelpEntry] = &[
    HelpEntry::new('?', "\tpokazati pomoć", true, Information),
    HelpEntry::new('/', "\topoznati znak", true, Information),
    HelpEntry::new('h', "\tvlěvo", true, Explore),
    HelpEntry::new('j', "\tdolu", true, Explore),
    HelpEntry::new('k', "\tgorě", true, Explore),
    HelpEntry::new('l', "\tvpravo", true, Explore),
    HelpEntry::new('y', "\tgorě i vlěvo", true, Explore),
    HelpEntry::new('u', "\tgorě i vpravo", true, Explore),
    HelpEntry::new('b', "\tdolu i vlěvo", true, Explore),
    HelpEntry::new('n', "\tdolu i vpravo", true, Explore),
    HelpEntry::new('H', "\tběgati vlěvo", false, Explore),
    HelpEntry::new('J', "\tběgati dolu", false, Explore),
    HelpEntry::new('K', "\tběgati gorě", false, Explore),
    HelpEntry::new('L', "\tběgati vpravo", false, Explore),
    HelpEntry::new('Y', "\tběgati gorě i vlěvo", false, Explore),
    HelpEntry::new('U', "\tběgati gorě i vpravo", false, Explore),
    HelpEntry::new('B', "\tběgati dolu i vlěvo", false, Explore),
    HelpEntry::new('N', "\tběgati dolu i vpravo", false, Explore),
    HelpEntry::new(
        '\u{8}',
        "\tběgati vlěvo do ⟨n:prěškoda:gen⟩",
        false,
        Explore,
    ),
    HelpEntry::new('\u{a}', "\tběgati dolu do ⟨n:prěškoda:gen⟩", false, Explore),
    HelpEntry::new('\u{b}', "\tběgati gorě do ⟨n:prěškoda:gen⟩", false, Explore),
    HelpEntry::new(
        '\u{c}',
        "\tběgati vpravo do ⟨n:prěškoda:gen⟩",
        false,
        Explore,
    ),
    HelpEntry::new(
        '\u{19}',
        "\tběgati gorě i vlěvo do ⟨n:prěškoda:gen⟩",
        false,
        Explore,
    ),
    HelpEntry::new(
        '\u{15}',
        "\tběgati gorě i vpravo do ⟨n:prěškoda:gen⟩",
        false,
        Explore,
    ),
    HelpEntry::new(
        '\u{2}',
        "\tběgati dolu i vlěvo do ⟨n:prěškoda:gen⟩",
        false,
        Explore,
    ),
    HelpEntry::new(
        '\u{e}',
        "\tběgati dolu i vpravo do ⟨n:prěškoda:gen⟩",
        false,
        Explore,
    ),
    HelpEntry::new(
        '\0',
        "\t<SHIFT><dir>: běgati v ⟨toj:acc:f⟩ ⟨n:stråna:acc⟩",
        true,
        Explore,
    ),
    HelpEntry::new(
        '\0',
        "\t<CTRL><dir>: běgati do ⟨n:prěškoda:gen⟩",
        true,
        Explore,
    ),
    HelpEntry::new(
        'f',
        "<dir>\tboriti sę do ⟨n:smŕť:gen⟩ ili skoro do ⟨n:smŕť:gen⟩",
        true,
        Combat,
    ),
    HelpEntry::new('t', "<dir>\tmetnųti něčto", true, Combat),
    HelpEntry::new('m', "<dir>\tidti i ⟨ničto:gen⟩ ne vzęti", true, Explore),
    HelpEntry::dock('z', "<dir>\tužiti žezlo", Combat, "žezlo", 40, Contextual),
    HelpEntry::new('^', "<dir>\topoznati vid ⟨n:pasť:gen⟩", true, Explore),
    HelpEntry::dock(
        's',
        "\tiskati pasť/⟨a:tajny:dvėri:acc:pl⟩ ⟨n:dvėri:acc:pl⟩",
        Explore,
        "iskati",
        20,
        Utility,
    ),
    HelpEntry::dock('>', "\tidti dolu", Explore, "dolu", 90, Urgent),
    HelpEntry::dock('<', "\tidti gorě", Explore, "gorě", 100, Urgent),
    HelpEntry::dock('.', "\tčekati jedin hod", Explore, "čekati", 10, Utility),
    HelpEntry::dock(',', "\tvzęti něčto", Explore, "vzęti", 95, Urgent),
    HelpEntry::dock(
        'i',
        "\tpokazati ⟨n:torba:acc⟩",
        Items,
        "⟨n:torba:acc⟩",
        30,
        Fallback,
    ),
    HelpEntry::new('I', "\tpokazati jedin prědmet", true, Information),
    HelpEntry::dock('q', "\tpiti napitȯk", Items, "piti", 60, Contextual),
    HelpEntry::dock('r', "\tčitati svitȯk", Items, "čitati", 55, Contextual),
    HelpEntry::dock('e', "\tjesti ⟨n:jeda:acc⟩", Items, "jesti", 50, Contextual),
    HelpEntry::dock('w', "\tdŕžati orųžje", Equipment, "orųžje", 35, Contextual),
    HelpEntry::dock(
        'W',
        "\tnositi ⟨n:brȯnja:acc⟩",
        Equipment,
        "⟨n:brȯnja:acc⟩",
        35,
        Contextual,
    ),
    HelpEntry::dock(
        'T',
        "\tsjęti ⟨n:brȯnja:acc⟩",
        Equipment,
        "sjęti ⟨n:brȯnja:acc⟩",
        45,
        Contextual,
    ),
    HelpEntry::dock(
        'P',
        "\tnaděti pŕstenj",
        Equipment,
        "pŕstenj",
        35,
        Contextual,
    ),
    HelpEntry::dock(
        'R',
        "\tsjęti pŕstenj",
        Equipment,
        "sjęti pŕstenj",
        45,
        Contextual,
    ),
    HelpEntry::new('d', "\tostaviti prědmet", true, Items),
    HelpEntry::new('c', "\tnazvati prědmet", true, Items),
    HelpEntry::new(
        'a',
        "\tpovtoriti ⟨a:poslědnji:komanda:acc⟩ ⟨n:komanda:acc⟩",
        true,
        System,
    ),
    HelpEntry::new(')', "\tpokazati orųžje v ⟨n:rųka:loc⟩", true, Information),
    HelpEntry::new(']', "\tpokazati ⟨n:brȯnja:acc⟩", true, Information),
    HelpEntry::new('=', "\tpokazati ⟨n:pŕstenj:acc:pl⟩", true, Information),
    HelpEntry::new(
        '@',
        "\tpokazati ⟨a:tvoj:stańje:acc⟩ ⟨n:stańje:acc⟩",
        true,
        Information,
    ),
    HelpEntry::new('D', "\tpokazati, čto uže ⟨v2:znati⟩", true, Information),
    HelpEntry::new('o', "\tpokazati/měnjati ⟨n:opcija:acc:pl⟩", true, System),
    HelpEntry::new('\u{12}', "\tobnoviti ekran", true, System),
    HelpEntry::new(
        '\u{10}',
        "\tpovtoriti ⟨a:poslědnji:sȯobčeńje:acc⟩ ⟨n:sȯobčeńje:acc⟩",
        true,
        Information,
    ),
    HelpEntry::new(
        '\u{1b}',
        "\tanulovati ⟨n:komanda:acc⟩, ^[ jest knopka escape",
        true,
        System,
    ),
    HelpEntry::new('S', "\tshråniti ⟨n:igra:acc⟩", true, System),
    HelpEntry::new('Q', "\tizhod", true, System),
    HelpEntry::new('!', "\totvoriti shell", true, System),
    HelpEntry::new(
        'F',
        "<dir>\tboriti sę dokolě někto ne ⟨v3:umrěti⟩",
        true,
        Combat,
    ),
    HelpEntry::new(
        'v',
        "\tpokazati ⟨n:verzija:acc⟩, izdańje, čislo ⟨n:temnica:gen⟩",
        true,
        Information,
    ),
];
