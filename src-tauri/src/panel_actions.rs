//! Everything a tile on a user-built panel can do, and the searchable
//! catalogue the editor picks from. Serialisable and persisted, so every
//! variant name and field is part of the saved panel format. No OS input.
use crate::{
    point_scan::KeyboardLayout,
    scan_menu::{Command, Kind, Setting},
};
use serde::{Deserialize, Serialize};

/// Longest text a single tile may type.
pub const MAX_TEXT_CHARS: usize = 200;
/// Most keys in one chord, matching switch shortcut bindings.
pub const MAX_CHORD_KEYS: usize = 4;
/// Longest id of a custom panel a tile may link to.
pub const MAX_PANEL_ID_LEN: usize = 64;

/// A built-in panel a tile can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuiltInPanel {
    Keyboard,
    Mouse,
    Point,
    /// Shows the Switchify window and ends scanning.
    Switchify,
    Menu(Kind),
}

/// Mouse button a tile clicks at the chosen point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Direction a tile scrolls at the chosen point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// What selecting a tile does. The `type` tag and field names are persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum PanelAction {
    /// Press a key chord such as Ctrl and S.
    Key { keys: Vec<String> },
    /// Type text. Never logged.
    Text { text: String },
    /// A command from the built-in menus (editing, windows, browser, media, power).
    Command { command: Command },
    /// Change a scanning setting.
    Setting { setting: Setting },
    /// Click at the chosen point.
    Click { button: MouseButton, count: u8 },
    /// Scroll at the chosen point.
    Scroll { direction: ScrollDirection },
    /// Open a built-in panel or menu.
    OpenPanel { panel: BuiltInPanel },
    /// Open another custom panel by its id.
    OpenCustomPanel { id: String },
    /// Pause or resume scanning.
    Pause,
    /// Reverse the scan direction.
    Reverse,
    /// Move scanning to the next or previous display.
    Display { next: bool },
    /// Return to the panel this one was opened from.
    Back,
    /// Close the panel and stop scanning.
    Close,
}

impl PanelAction {
    /// Short default label for a tile, which the editor lets people change.
    pub fn label(&self) -> String {
        match self {
            Self::Key { keys } => keys.join("+"),
            Self::Text { text } => text
                .chars()
                .map(|c| if c.is_whitespace() { ' ' } else { c })
                .take(12)
                .collect(),
            Self::Command { command } => command.label().into(),
            Self::Setting {
                setting: Setting::KeyboardLayout(layout),
            } => match layout {
                KeyboardLayout::Qwerty => "QWERTY",
                KeyboardLayout::SimpleQwerty => "Simple QWERTY",
                KeyboardLayout::CommonLetters => "Common letters",
            }
            .into(),
            Self::Setting { setting } => setting.label().into(),
            Self::Click { button, count } => match (button, count) {
                (MouseButton::Left, 1) => "Left click",
                (MouseButton::Left, 2) => "Double click",
                (MouseButton::Left, _) => "Triple click",
                (MouseButton::Right, _) => "Right click",
                (MouseButton::Middle, _) => "Middle click",
            }
            .into(),
            Self::Scroll { direction } => match direction {
                ScrollDirection::Up => "Scroll up",
                ScrollDirection::Down => "Scroll down",
                ScrollDirection::Left => "Scroll left",
                ScrollDirection::Right => "Scroll right",
            }
            .into(),
            Self::OpenPanel { panel } => match panel {
                BuiltInPanel::Keyboard => "Keyboard".into(),
                BuiltInPanel::Mouse => "Mouse".into(),
                BuiltInPanel::Point => "Point".into(),
                BuiltInPanel::Switchify => "Switchify".into(),
                BuiltInPanel::Menu(kind) => kind.label().into(),
            },
            Self::OpenCustomPanel { .. } => "Panel".into(),
            Self::Pause => "Pause scanning".into(),
            Self::Reverse => "Reverse direction".into(),
            Self::Display { next: true } => "Next display".into(),
            Self::Display { next: false } => "Previous display".into(),
            Self::Back => "Back".into(),
            Self::Close => "Close".into(),
        }
    }

    /// Checks the action can be stored and run. Text content is never quoted
    /// in the error.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Key { keys } => validate_chord(keys),
            Self::Text { text } => {
                if text.is_empty() {
                    return Err("Type some text for the tile to type.".into());
                }
                if text.chars().count() > MAX_TEXT_CHARS {
                    return Err(format!(
                        "Text must use {MAX_TEXT_CHARS} characters or fewer."
                    ));
                }
                if text
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
                {
                    return Err("Text may not contain control characters.".into());
                }
                Ok(())
            }
            Self::Click { count, .. } => {
                if !(1..=3).contains(count) {
                    return Err("A click may be single, double or triple.".into());
                }
                Ok(())
            }
            Self::OpenPanel {
                panel: BuiltInPanel::Menu(kind),
            } => {
                if !openable_menus().contains(kind) {
                    return Err("That menu can only be reached from its own panel.".into());
                }
                Ok(())
            }
            Self::OpenCustomPanel { id } => validate_panel_id(id),
            _ => Ok(()),
        }
    }
}

/// Keys a chord or switch binding may name: the named keys the injector
/// knows plus any single ASCII letter or digit.
pub fn valid_key(key: &str) -> bool {
    matches!(
        key,
        "Space"
            | "Enter"
            | "Escape"
            | "Tab"
            | "Backspace"
            | "Delete"
            | "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "Ctrl"
            | "Alt"
            | "Shift"
            | "Meta"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F8"
            | "F9"
            | "F10"
            | "F11"
            | "F12"
    ) || (key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub fn is_modifier(key: &str) -> bool {
    matches!(key, "Ctrl" | "Alt" | "Shift" | "Meta")
}

/// One to four distinct known keys, at least one of which is not a modifier.
pub fn validate_chord(keys: &[String]) -> Result<(), String> {
    if keys.is_empty() || keys.len() > MAX_CHORD_KEYS {
        return Err(format!("A key press uses 1 to {MAX_CHORD_KEYS} keys."));
    }
    if keys.iter().any(|key| !valid_key(key)) {
        return Err("One of the keys is not a key the app can press.".into());
    }
    if keys.iter().all(|key| is_modifier(key)) {
        return Err("Add a key to press with the modifiers.".into());
    }
    if keys.iter().collect::<std::collections::HashSet<_>>().len() != keys.len() {
        return Err("Each key may appear once in a key press.".into());
    }
    Ok(())
}

/// Custom panel ids are lower-case slugs or UUIDs: safe in file names and sync.
pub fn validate_panel_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > MAX_PANEL_ID_LEN {
        return Err(format!(
            "A panel id uses 1 to {MAX_PANEL_ID_LEN} characters."
        ));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("A panel id uses lower-case letters, digits and hyphens.".into());
    }
    Ok(())
}

/// Menus a custom tile may open. The point-scan action pages (Actions,
/// Scroll, Confirm drag) and the mouse panel's own pages need their context.
pub fn openable_menus() -> &'static [Kind] {
    &[
        Kind::Home,
        Kind::More,
        Kind::Mouse,
        Kind::Editing,
        Kind::Windows,
        Kind::Browser,
        Kind::Tabs,
        Kind::Zoom,
        Kind::Media,
        Kind::Displays,
        Kind::Scanning,
        Kind::ShutDown,
        Kind::Restart,
    ]
}

/// Where an entry sits in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Typing,
    Editing,
    Windows,
    Browser,
    Media,
    Mouse,
    Panels,
    Scanning,
    Power,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Typing => "Typing",
            Self::Editing => "Editing",
            Self::Windows => "Apps and windows",
            Self::Browser => "Browser",
            Self::Media => "Media",
            Self::Mouse => "Mouse",
            Self::Panels => "Panels and menus",
            Self::Scanning => "Scanning",
            Self::Power => "Power",
        }
    }
    #[cfg(test)]
    pub const ALL: [Category; 9] = [
        Self::Typing,
        Self::Editing,
        Self::Windows,
        Self::Browser,
        Self::Media,
        Self::Mouse,
        Self::Panels,
        Self::Scanning,
        Self::Power,
    ];
}

/// A value the picker asks for before an entry becomes an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Parameter {
    /// A key chord of `min` to `max` keys.
    Keys { min: usize, max: usize },
    /// Free text up to `max_chars`.
    Text { max_chars: usize },
    /// The id of another custom panel.
    PanelId,
}

/// One row of the picker: a ready-made action, or a template that needs a
/// parameter filled in before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEntry {
    /// Stable across releases, for tests and the editor's recent list.
    pub id: String,
    pub name: String,
    pub category: Category,
    pub category_label: &'static str,
    /// Extra words a search should match besides the name.
    pub keywords: Vec<&'static str>,
    /// The action, complete when `parameter` is `None`.
    pub action: PanelAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter: Option<Parameter>,
    /// True when the action needs a chosen point (point scanning) to make sense.
    pub needs_point: bool,
}

fn entry(
    id: &str,
    name: impl Into<String>,
    category: Category,
    keywords: &[&'static str],
    action: PanelAction,
) -> CatalogueEntry {
    CatalogueEntry {
        id: id.into(),
        name: name.into(),
        category,
        category_label: category.label(),
        keywords: keywords.to_vec(),
        action,
        parameter: None,
        needs_point: false,
    }
}

fn command_category(command: Command) -> Category {
    use Command::*;
    match command {
        MiddleClick | TripleClick | ShiftClick | CtrlClick | AltClick | MetaClick => {
            Category::Mouse
        }
        Copy | Cut | Paste | SelectAll | Undo | Redo | Save | Find => Category::Editing,
        SwitchNext | SwitchPrevious | Overview | Desktop | Minimize | Maximize | CloseWindow => {
            Category::Windows
        }
        BrowserBack | BrowserForward | Reload | NewTab | CloseTab | ReopenTab | NextTab
        | PreviousTab | Address | ZoomIn | ZoomOut | ZoomReset => Category::Browser,
        PlayPause | NextTrack | PreviousTrack | VolumeUp | VolumeDown | Mute => Category::Media,
        ShutDown | Restart => Category::Power,
    }
}

fn command_keywords(command: Command) -> &'static [&'static str] {
    use Command::*;
    match command {
        Copy => &["clipboard", "ctrl c"],
        Cut => &["clipboard", "ctrl x"],
        Paste => &["clipboard", "ctrl v"],
        SelectAll => &["ctrl a"],
        Undo => &["ctrl z"],
        Redo => &["ctrl y"],
        Save => &["ctrl s"],
        Find => &["search", "ctrl f"],
        SwitchNext | SwitchPrevious => &["alt tab", "application"],
        Overview => &["task view", "mission control"],
        Desktop => &["minimise all"],
        Minimize => &["minimise", "hide window"],
        Maximize => &["maximise", "full screen", "restore"],
        CloseWindow => &["alt f4", "quit"],
        BrowserBack | BrowserForward => &["history", "navigate"],
        Reload => &["refresh"],
        NewTab | CloseTab | ReopenTab | NextTab | PreviousTab => &["tabs"],
        Address => &["url", "location", "address bar"],
        ZoomIn | ZoomOut | ZoomReset => &["magnify", "text size"],
        PlayPause => &["music", "video"],
        NextTrack | PreviousTrack => &["music", "skip"],
        VolumeUp | VolumeDown | Mute => &["sound", "audio"],
        ShutDown => &["power off", "turn off"],
        Restart => &["reboot"],
        MiddleClick | TripleClick | ShiftClick | CtrlClick | AltClick | MetaClick => {
            &["modifier click"]
        }
    }
}

const ALL_COMMANDS: [Command; 41] = {
    use Command::*;
    [
        MiddleClick,
        TripleClick,
        ShiftClick,
        CtrlClick,
        AltClick,
        MetaClick,
        Copy,
        Cut,
        Paste,
        SelectAll,
        Undo,
        Redo,
        Save,
        Find,
        SwitchNext,
        SwitchPrevious,
        Overview,
        Desktop,
        Minimize,
        Maximize,
        CloseWindow,
        BrowserBack,
        BrowserForward,
        Reload,
        NewTab,
        CloseTab,
        ReopenTab,
        NextTab,
        PreviousTab,
        Address,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        PlayPause,
        NextTrack,
        PreviousTrack,
        VolumeUp,
        VolumeDown,
        Mute,
        ShutDown,
        Restart,
    ]
};

/// Fails to compile when a `Command` is added, so `ALL_COMMANDS` is updated too.
#[cfg(test)]
fn command_count() -> usize {
    use Command::*;
    match Command::Copy {
        MiddleClick | TripleClick | ShiftClick | CtrlClick | AltClick | MetaClick | Copy | Cut
        | Paste | SelectAll | Undo | Redo | Save | Find | SwitchNext | SwitchPrevious
        | Overview | Desktop | Minimize | Maximize | CloseWindow | BrowserBack | BrowserForward
        | Reload | NewTab | CloseTab | ReopenTab | NextTab | PreviousTab | Address | ZoomIn
        | ZoomOut | ZoomReset | PlayPause | NextTrack | PreviousTrack | VolumeUp | VolumeDown
        | Mute | ShutDown | Restart => 41,
    }
}

#[cfg(test)]
const ALL_SETTINGS: [Setting; 9] = [
    Setting::FasterScan,
    Setting::SlowerScan,
    Setting::FasterLine,
    Setting::SlowerLine,
    Setting::LineMode,
    Setting::GridMode,
    Setting::KeyboardLayout(KeyboardLayout::Qwerty),
    Setting::KeyboardLayout(KeyboardLayout::SimpleQwerty),
    Setting::KeyboardLayout(KeyboardLayout::CommonLetters),
];

fn command_id(command: Command) -> String {
    let json = serde_json::to_value(command).expect("commands serialise");
    format!("command.{}", json.as_str().expect("command is a string"))
}

fn setting_entry(id: &str, setting: Setting, keywords: &[&'static str]) -> CatalogueEntry {
    entry(
        id,
        setting.label(),
        Category::Scanning,
        keywords,
        PanelAction::Setting { setting },
    )
}

/// Every action a tile can be given, in picker order. Commands, settings and
/// built-in panels appear once each; key presses, typed text and panel links
/// are templates with a parameter.
pub fn catalogue() -> Vec<CatalogueEntry> {
    let mut entries = vec![
        CatalogueEntry {
            parameter: Some(Parameter::Keys {
                min: 1,
                max: MAX_CHORD_KEYS,
            }),
            ..entry(
                "key",
                "Press keys",
                Category::Typing,
                &["shortcut", "chord", "keyboard", "hotkey"],
                PanelAction::Key { keys: vec![] },
            )
        },
        CatalogueEntry {
            parameter: Some(Parameter::Text {
                max_chars: MAX_TEXT_CHARS,
            }),
            ..entry(
                "text",
                "Type text",
                Category::Typing,
                &["phrase", "message", "say", "sentence"],
                PanelAction::Text {
                    text: String::new(),
                },
            )
        },
    ];
    for (name, keys, keywords) in [
        ("Enter", "Enter", &["return", "new line"][..]),
        ("Space", "Space", &["space bar"]),
        ("Backspace", "Backspace", &["delete back", "erase"]),
        ("Delete", "Delete", &["delete forward"]),
        ("Tab", "Tab", &["next field"]),
        ("Escape", "Escape", &["esc", "cancel dialog"]),
        ("Up arrow", "ArrowUp", &["cursor", "navigate"]),
        ("Down arrow", "ArrowDown", &["cursor", "navigate"]),
        ("Left arrow", "ArrowLeft", &["cursor", "navigate"]),
        ("Right arrow", "ArrowRight", &["cursor", "navigate"]),
        ("Home", "Home", &["start of line"]),
        ("End", "End", &["end of line"]),
        ("Page up", "PageUp", &["scroll"]),
        ("Page down", "PageDown", &["scroll"]),
    ] {
        entries.push(entry(
            &format!("key.{keys}"),
            name,
            Category::Typing,
            keywords,
            PanelAction::Key {
                keys: vec![keys.into()],
            },
        ));
    }
    for command in ALL_COMMANDS {
        let mut row = entry(
            &command_id(command),
            command.label(),
            command_category(command),
            command_keywords(command),
            PanelAction::Command { command },
        );
        row.needs_point = matches!(
            command,
            Command::MiddleClick
                | Command::TripleClick
                | Command::ShiftClick
                | Command::CtrlClick
                | Command::AltClick
                | Command::MetaClick
        );
        entries.push(row);
    }
    for (id, name, button, count, keywords) in [
        (
            "click.left",
            "Left click",
            MouseButton::Left,
            1,
            &["select", "tap"][..],
        ),
        (
            "click.double",
            "Double click",
            MouseButton::Left,
            2,
            &["open"],
        ),
        (
            "click.right",
            "Right click",
            MouseButton::Right,
            1,
            &["context menu"],
        ),
    ] {
        let mut row = entry(
            id,
            name,
            Category::Mouse,
            keywords,
            PanelAction::Click { button, count },
        );
        row.needs_point = true;
        entries.push(row);
    }
    for (id, name, direction) in [
        ("scroll.up", "Scroll up", ScrollDirection::Up),
        ("scroll.down", "Scroll down", ScrollDirection::Down),
        ("scroll.left", "Scroll left", ScrollDirection::Left),
        ("scroll.right", "Scroll right", ScrollDirection::Right),
    ] {
        let mut row = entry(
            id,
            name,
            Category::Mouse,
            &["wheel", "page"],
            PanelAction::Scroll { direction },
        );
        row.needs_point = true;
        entries.push(row);
    }
    for (id, name, panel, keywords) in [
        (
            "panel.keyboard",
            "Open keyboard",
            BuiltInPanel::Keyboard,
            &["type", "letters"][..],
        ),
        (
            "panel.mouse",
            "Open mouse panel",
            BuiltInPanel::Mouse,
            &["pointer", "move"],
        ),
        (
            "panel.point",
            "Open point scanning",
            BuiltInPanel::Point,
            &["crosshair", "click anywhere"],
        ),
        (
            "panel.switchify",
            "Show Switchify window",
            BuiltInPanel::Switchify,
            &["settings", "app"],
        ),
    ] {
        entries.push(entry(
            id,
            name,
            Category::Panels,
            keywords,
            PanelAction::OpenPanel { panel },
        ));
    }
    for kind in openable_menus() {
        let json = serde_json::to_value(kind).expect("kinds serialise");
        entries.push(entry(
            &format!("menu.{}", json.as_str().expect("kind is a string")),
            format!("Open {} menu", kind.label()),
            Category::Panels,
            &["menu", "built in"],
            PanelAction::OpenPanel {
                panel: BuiltInPanel::Menu(*kind),
            },
        ));
    }
    entries.push(CatalogueEntry {
        parameter: Some(Parameter::PanelId),
        ..entry(
            "panel.custom",
            "Open one of my panels",
            Category::Panels,
            &["link", "go to", "jump"],
            PanelAction::OpenCustomPanel { id: String::new() },
        )
    });
    entries.push(entry(
        "back",
        "Back",
        Category::Panels,
        &["previous panel", "return"],
        PanelAction::Back,
    ));
    entries.push(entry(
        "close",
        "Close panel",
        Category::Panels,
        &["stop scanning", "exit", "cancel"],
        PanelAction::Close,
    ));
    entries.push(entry(
        "pause",
        "Pause scanning",
        Category::Scanning,
        &["resume", "hold"],
        PanelAction::Pause,
    ));
    entries.push(entry(
        "reverse",
        "Reverse direction",
        Category::Scanning,
        &["backwards"],
        PanelAction::Reverse,
    ));
    entries.push(setting_entry(
        "setting.fasterScan",
        Setting::FasterScan,
        &["speed", "quicker"],
    ));
    entries.push(setting_entry(
        "setting.slowerScan",
        Setting::SlowerScan,
        &["speed"],
    ));
    entries.push(setting_entry(
        "setting.fasterLine",
        Setting::FasterLine,
        &["speed", "point"],
    ));
    entries.push(setting_entry(
        "setting.slowerLine",
        Setting::SlowerLine,
        &["speed", "point"],
    ));
    entries.push(setting_entry(
        "setting.lineMode",
        Setting::LineMode,
        &["point scanning", "mode"],
    ));
    entries.push(setting_entry(
        "setting.gridMode",
        Setting::GridMode,
        &["point scanning", "mode"],
    ));
    for (layout, name) in [
        (KeyboardLayout::Qwerty, "Keyboard layout: QWERTY"),
        (
            KeyboardLayout::SimpleQwerty,
            "Keyboard layout: simple QWERTY",
        ),
        (
            KeyboardLayout::CommonLetters,
            "Keyboard layout: common letters",
        ),
    ] {
        let json = serde_json::to_value(layout).expect("layouts serialise");
        entries.push(entry(
            &format!(
                "setting.keyboardLayout.{}",
                json.as_str().expect("layout is a string")
            ),
            name,
            Category::Scanning,
            &["keyboard", "layout", "letters"],
            PanelAction::Setting {
                setting: Setting::KeyboardLayout(layout),
            },
        ));
    }
    entries.push(entry(
        "display.next",
        "Next display",
        Category::Scanning,
        &["monitor", "screen"],
        PanelAction::Display { next: true },
    ));
    entries.push(entry(
        "display.previous",
        "Previous display",
        Category::Scanning,
        &["monitor", "screen"],
        PanelAction::Display { next: false },
    ));
    entries
}

/// Case-insensitive search over names, categories and keywords. Every word of
/// the query must match somewhere. An empty query returns everything.
pub fn search(query: &str) -> Vec<CatalogueEntry> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|word| word.to_lowercase())
        .collect();
    catalogue()
        .into_iter()
        .filter(|entry| {
            let haystack = format!(
                "{} {} {} {}",
                entry.name,
                entry.category_label,
                entry.keywords.join(" "),
                entry.id
            )
            .to_lowercase();
            words.iter().all(|word| haystack.contains(word.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_entry_has_a_unique_id_and_a_valid_or_templated_action() {
        let entries = catalogue();
        let ids: HashSet<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids.len(), entries.len(), "catalogue ids clash");
        for entry in &entries {
            assert!(!entry.name.trim().is_empty(), "{} has no name", entry.id);
            if entry.parameter.is_none() {
                assert_eq!(entry.action.validate(), Ok(()), "{} is invalid", entry.id);
            } else {
                assert!(
                    entry.action.validate().is_err(),
                    "{} template is already complete",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn every_command_setting_and_openable_menu_is_listed() {
        let entries = catalogue();
        assert_eq!(ALL_COMMANDS.len(), command_count());
        let distinct: HashSet<_> = ALL_COMMANDS.iter().map(|c| format!("{c:?}")).collect();
        assert_eq!(distinct.len(), ALL_COMMANDS.len());
        for command in ALL_COMMANDS {
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.action == PanelAction::Command { command }),
                "{command:?} missing"
            );
        }
        for setting in ALL_SETTINGS {
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.action == PanelAction::Setting { setting }),
                "{setting:?} missing"
            );
        }
        for kind in openable_menus() {
            assert!(entries.iter().any(|entry| entry.action
                == PanelAction::OpenPanel {
                    panel: BuiltInPanel::Menu(*kind)
                }));
        }
        for category in Category::ALL {
            assert!(
                entries.iter().any(|entry| entry.category == category),
                "{category:?} empty"
            );
        }
    }

    #[test]
    fn actions_round_trip_through_json_with_stable_tags() {
        let action = PanelAction::Key {
            keys: vec!["Ctrl".into(), "S".into()],
        };
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, r#"{"type":"key","keys":["Ctrl","S"]}"#);
        assert_eq!(serde_json::from_str::<PanelAction>(&json).unwrap(), action);
        let menu = PanelAction::OpenPanel {
            panel: BuiltInPanel::Menu(Kind::Editing),
        };
        let json = serde_json::to_string(&menu).unwrap();
        assert_eq!(json, r#"{"type":"openPanel","panel":{"menu":"editing"}}"#);
        let setting = PanelAction::Setting {
            setting: Setting::KeyboardLayout(KeyboardLayout::CommonLetters),
        };
        let json = serde_json::to_string(&setting).unwrap();
        assert_eq!(
            json,
            r#"{"type":"setting","setting":{"keyboardLayout":"commonLetters"}}"#
        );
        assert_eq!(serde_json::from_str::<PanelAction>(&json).unwrap(), setting);
        let template = catalogue()
            .into_iter()
            .find(|entry| entry.id == "text")
            .unwrap();
        assert_eq!(
            serde_json::to_value(template.parameter.unwrap()).unwrap(),
            serde_json::json!({"kind": "text", "maxChars": MAX_TEXT_CHARS})
        );
        assert!(
            serde_json::from_str::<PanelAction>(r#"{"type":"key","keys":["A"],"extra":1}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<PanelAction>(r#"{"type":"launch","path":"x"}"#).is_err());
    }

    #[test]
    fn validation_rejects_bad_chords_text_slots_and_ids() {
        let key = |keys: &[&str]| PanelAction::Key {
            keys: keys.iter().map(|k| k.to_string()).collect(),
        };
        assert!(key(&[]).validate().is_err());
        assert!(key(&["Ctrl", "Shift"]).validate().is_err());
        assert!(key(&["Ctrl", "Ctrl", "S"]).validate().is_err());
        assert!(key(&["Hyper"]).validate().is_err());
        assert!(key(&["Ctrl", "Alt", "Shift", "Meta", "S"])
            .validate()
            .is_err());
        assert_eq!(key(&["Ctrl", "S"]).validate(), Ok(()));
        assert!(key(&["é"]).validate().is_err());
        assert!(key(&["="]).validate().is_err());
        assert_eq!(key(&["7"]).validate(), Ok(()));
        let text = |text: &str| PanelAction::Text { text: text.into() };
        assert!(text("").validate().is_err());
        assert!(text(&"a".repeat(MAX_TEXT_CHARS + 1)).validate().is_err());
        assert!(text("a\u{7}b").validate().is_err());
        let error = text("secret\u{7}").validate().unwrap_err();
        assert!(!error.contains("secret"), "errors must not echo typed text");
        assert_eq!(text("Hello there\n").validate(), Ok(()));
        assert!(PanelAction::Click {
            button: MouseButton::Left,
            count: 4
        }
        .validate()
        .is_err());
        assert!(PanelAction::OpenPanel {
            panel: BuiltInPanel::Menu(Kind::ConfirmDrag)
        }
        .validate()
        .is_err());
        let link = |id: &str| PanelAction::OpenCustomPanel { id: id.into() };
        assert!(link("").validate().is_err());
        assert!(link("My Panel").validate().is_err());
        assert!(link(&"a".repeat(MAX_PANEL_ID_LEN + 1)).validate().is_err());
        assert_eq!(link("my-panel-2").validate(), Ok(()));
        assert_eq!(
            link("0f4b9c2e-1a2b-4c3d-8e9f-0a1b2c3d4e5f").validate(),
            Ok(())
        );
    }

    #[test]
    fn search_matches_names_keywords_and_categories_case_insensitively() {
        let ids = |query: &str| {
            search(query)
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>()
        };
        assert!(ids("CLIPBOARD").contains(&"command.copy".to_string()));
        assert!(ids("alt tab").contains(&"command.switchNext".to_string()));
        assert!(ids("media").contains(&"command.mute".to_string()));
        assert!(ids("menu editing").contains(&"menu.editing".to_string()));
        assert_eq!(ids("zzzz-nothing"), Vec::<String>::new());
        assert_eq!(search("").len(), catalogue().len());
    }

    #[test]
    fn default_labels_fit_a_tile() {
        assert_eq!(
            PanelAction::Key {
                keys: vec!["Ctrl".into(), "S".into()]
            }
            .label(),
            "Ctrl+S"
        );
        assert_eq!(
            PanelAction::Text {
                text: "Good morning everyone".into()
            }
            .label(),
            "Good morning"
        );
        assert_eq!(
            PanelAction::Text {
                text: "Hi
there"
                    .into()
            }
            .label(),
            "Hi there"
        );
        assert_eq!(
            PanelAction::Setting {
                setting: Setting::KeyboardLayout(KeyboardLayout::SimpleQwerty)
            }
            .label(),
            "Simple QWERTY"
        );
        assert_eq!(
            PanelAction::OpenPanel {
                panel: BuiltInPanel::Menu(Kind::Windows)
            }
            .label(),
            "Apps and windows"
        );
    }
}
