//! Menu rows and navigation are independent of native windows and input.
use crate::{
    custom_panels::CustomPanel,
    panel_actions::{Category, PanelAction},
    scan_items::{ItemScanner, Policy},
    scan_panel::{Dock, Panel, PanelKey},
    scanning::{Action, Frame, Rect, TileRole},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Home,
    PointScan,
    Switchify,
    Keyboard,
    MousePanel,
    Group(Kind),
    Command(Command),
    Setting(Setting),
    Display(bool),
    Pause,
    Reverse,
    LeftClick,
    RightClick,
    DoubleClick,
    Scroll,
    Drag,
    NewPoint,
    Cancel,
    Up,
    Down,
    Left,
    Right,
    Back,
    DragHere,
    DestinationAgain,
    CancelDrag,
    /// Opens the list of the user's own panels.
    Panels,
    /// Opens the user's panel at this index in the list page.
    OpenCustom(usize),
    /// A tile of the open custom panel, by index into its cells.
    Custom(usize),
}
impl Item {
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::PointScan => "Point",
            Self::Switchify => "Open Switchify",
            Self::Keyboard => "Keyboard",
            Self::MousePanel => "Mouse",
            Self::Group(kind) => kind.label(),
            Self::Command(command) => command.label(),
            Self::Setting(setting) => setting.label(),
            Self::Display(true) => "Next display",
            Self::Display(false) => "Previous display",
            Self::Pause => "Pause scanning",
            Self::Reverse => "Reverse direction",
            Self::LeftClick => "Left click",
            Self::RightClick => "Right click",
            Self::DoubleClick => "Double click",
            Self::Scroll => "Scroll",
            Self::Drag => "Drag",
            Self::NewPoint => "New point",
            Self::Cancel => "Close menu",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Back => "Back",
            Self::DragHere => "Drag here",
            Self::DestinationAgain => "New destination",
            Self::CancelDrag => "Cancel drag",
            Self::Panels => "My panels",
            Self::OpenCustom(_) | Self::Custom(_) => "Panel",
        }
    }
    /// Keys that leave or close the menu are drawn like the panels' toolbar keys.
    fn role(self) -> TileRole {
        match self {
            Self::Back | Self::Cancel | Self::CancelDrag | Self::Home => TileRole::Toolbar,
            _ => TileRole::Utility,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Home,
    /// Retired: the More actions page that once followed a point. Switches
    /// and saved tiles that still name it open Home, which lists its menus.
    More,
    /// The page of other clicks (middle, triple and modifier clicks), called
    /// More clicks. It follows a point, or the mouse panel's Speed and more page.
    Mouse,
    Editing,
    Windows,
    Browser,
    Tabs,
    Zoom,
    Media,
    Displays,
    Scanning,
    Actions,
    Scroll,
    ConfirmDrag,
    ShutDown,
    Restart,
    /// The list of the user's own panels. Its rows come from the panels.
    Panels,
    /// One of the user's own panels. Its rows come from the panel.
    Custom,
}
/// The pages with fixed rows. `Panels` and `Custom` are built from data.
#[cfg(test)]
pub(crate) const ALL_MENU_KINDS: [Kind; 15] = [
    Kind::Home,
    Kind::Actions,
    Kind::Mouse,
    Kind::Editing,
    Kind::Windows,
    Kind::Browser,
    Kind::Tabs,
    Kind::Zoom,
    Kind::Media,
    Kind::Displays,
    Kind::Scanning,
    Kind::Scroll,
    Kind::ConfirmDrag,
    Kind::ShutDown,
    Kind::Restart,
];
/// A page on the Back stack: a fixed page, or one of the user's panels by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageRef {
    Kind(Kind),
    Custom(String),
}

/// A tile of a custom panel, as the menu scans it.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomCell {
    pub label: String,
    pub span: u8,
    pub action: PanelAction,
}

pub struct Menu {
    pub kind: Kind,
    /// Set while one of the user's panels is open.
    pub custom_id: Option<String>,
    rows: Vec<Vec<Item>>,
    /// The open custom panel's tiles, indexed by `Item::Custom`.
    cells: Vec<CustomCell>,
    /// Panel names and ids on the list page, indexed by `Item::OpenCustom`.
    names: Vec<String>,
    ids: Vec<String>,
    scan: ItemScanner<Item>,
    period: u64,
}
impl Menu {
    pub fn set_panel_size(&mut self, size: crate::scan_preferences::PanelSize) {
        self.scan.options.panel_size = size;
    }
    pub fn set_panel_theme(&mut self, theme: crate::scan_preferences::PanelTheme) {
        self.scan.options.panel_theme = theme;
    }

    #[cfg(test)]
    pub fn new(kind: Kind, period: u64) -> Self {
        Self::configured(
            kind,
            crate::scan_preferences::Resolved {
                interval_ms: period,
                ..Default::default()
            },
        )
    }
    pub fn configured(kind: Kind, options: crate::scan_preferences::Resolved) -> Self {
        Self::with_rows(kind, kind.rows(), options)
    }
    /// A fixed page as the user sees it: without the My panels tile while
    /// there are no panels to list, as Home leaves it out too.
    pub fn configured_for(
        kind: Kind,
        options: crate::scan_preferences::Resolved,
        has_panels: bool,
    ) -> Self {
        let mut rows = kind.rows();
        if !has_panels {
            for row in &mut rows {
                row.retain(|item| *item != Item::Panels);
            }
        }
        Self::with_rows(kind, rows, options)
    }
    fn with_rows(
        kind: Kind,
        rows: Vec<Vec<Item>>,
        options: crate::scan_preferences::Resolved,
    ) -> Self {
        let scan = ItemScanner::configured_rows(&rows, Policy::MENU, options);
        Self {
            kind,
            custom_id: None,
            rows,
            cells: Vec::new(),
            names: Vec::new(),
            ids: Vec::new(),
            scan,
            period: options.interval_ms,
        }
    }
    /// A built-in menu from its document, as shipped or as the user
    /// rearranged it. It keeps its kind, so its menu behaviour stays.
    pub fn builtin(
        kind: Kind,
        document: &CustomPanel,
        options: crate::scan_preferences::Resolved,
    ) -> Self {
        let mut menu = Self::custom(document, options);
        menu.kind = kind;
        menu.custom_id = None;
        menu
    }
    /// The user's panels by name, three to a row, then Back and Close menu.
    pub fn panel_list(panels: &[CustomPanel], options: crate::scan_preferences::Resolved) -> Self {
        let items: Vec<Item> = (0..panels.len()).map(Item::OpenCustom).collect();
        let mut rows: Vec<Vec<Item>> = items.chunks(3).map(|row| row.to_vec()).collect();
        rows.push(vec![Item::Back, Item::Cancel]);
        let mut menu = Self::with_rows(Kind::Panels, rows, options);
        menu.names = panels.iter().map(|panel| panel.name.clone()).collect();
        menu.ids = panels.iter().map(|panel| panel.id.clone()).collect();
        menu
    }
    /// One of the user's panels, tile for tile.
    pub fn custom(panel: &CustomPanel, options: crate::scan_preferences::Resolved) -> Self {
        let mut cells = Vec::new();
        let rows = panel
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| {
                        cells.push(CustomCell {
                            label: cell.label.clone(),
                            span: cell.span,
                            action: cell.action.clone(),
                        });
                        Item::Custom(cells.len() - 1)
                    })
                    .collect()
            })
            .collect();
        let mut menu = Self::with_rows(Kind::Custom, rows, options);
        menu.custom_id = Some(panel.id.clone());
        menu.cells = cells;
        menu
    }
    /// What Back should reopen to come back to this page.
    pub fn page_ref(&self) -> PageRef {
        match &self.custom_id {
            Some(id) => PageRef::Custom(id.clone()),
            None => PageRef::Kind(self.kind),
        }
    }
    /// The labels on each row, as drawn.
    #[cfg(test)]
    pub fn frame_labels(&self) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|row| row.iter().map(|item| self.item_label(*item)).collect())
            .collect()
    }
    /// The id behind a tile of the list page, so the panel is found by id
    /// even if the list changed since the page opened.
    pub fn listed_panel_id(&self, index: usize) -> Option<&str> {
        self.ids.get(index).map(String::as_str)
    }
    pub fn cell(&self, index: usize) -> Option<&CustomCell> {
        self.cells.get(index)
    }
    fn item_label(&self, item: Item) -> String {
        match item {
            Item::Custom(index) => self.cells.get(index).map(|cell| cell.label.clone()),
            Item::OpenCustom(index) => self.names.get(index).cloned(),
            _ => None,
        }
        .unwrap_or_else(|| item.label().into())
    }
    fn item_weight(&self, item: Item) -> f64 {
        match item {
            Item::Custom(index) => self
                .cells
                .get(index)
                .map_or(1.0, |cell| f64::from(cell.span)),
            _ => 1.0,
        }
    }
    fn item_role(&self, item: Item) -> TileRole {
        match item {
            Item::Custom(index) => match self.cells.get(index).map(|cell| &cell.action) {
                Some(PanelAction::Back | PanelAction::Close) => TileRole::Toolbar,
                // Home on an action menu is drawn like Back, as it always was.
                Some(PanelAction::OpenPanel {
                    panel: crate::panel_actions::BuiltInPanel::Menu(Kind::Home),
                }) if self.kind != Kind::Home => TileRole::Toolbar,
                _ => TileRole::Utility,
            },
            _ => item.role(),
        }
    }
    pub fn suspended(&self) -> bool {
        self.scan.suspended
    }
    pub fn suspend(&mut self) {
        self.scan.suspended = true;
    }
    /// The page is waiting for a switch before it scans again.
    pub fn waiting(&self) -> bool {
        self.scan.waiting()
    }
    pub fn wait(&mut self) {
        self.scan.wait();
    }
    pub fn set_period(&mut self, period: u64) {
        self.period = period;
        self.restart_interval();
    }
    pub fn restart_interval(&mut self) {
        self.scan.restart_interval();
    }
    /// An item that leaves the menu open was chosen.
    pub fn continue_after_selection(&mut self) {
        self.scan.continue_after_selection();
    }
    pub fn advance(&mut self, ms: u64) {
        self.scan.advance(ms, self.period);
    }
    pub fn handle(&mut self, action: Action) -> Option<Item> {
        self.scan.handle(action)
    }

    /// Draws the menu as a docked panel. Nothing is written beside it:
    /// waiting, pausing and going back show through the highlight alone, and
    /// while an action has `failed`, nothing is highlighted and the workflow
    /// shows the error itself, with what went wrong, beside the panel.
    pub fn frame(&self, screen: Rect, units: f64, dock: Dock, failed: bool) -> Frame {
        let options = self.scan.options;
        Panel {
            key_width: 176.0,
            size: options.panel_size,
            rows: self
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|item| PanelKey {
                            text: self.item_label(*item),
                            weight: self.item_weight(*item),
                            role: self.item_role(*item),
                            active: false,
                            blank: false,
                            hidden: false,
                        })
                        .collect()
                })
                .collect(),
            message: None,
            dock,
            // Nothing can be chosen while an error or pause waits for Select.
            selected: (!failed && !self.suspended()).then(|| self.scan.highlight(&self.rows)),
            row_scan: self.scan.row_scan() || self.scan.group_scan() || self.scan.nav.escaping(),
            thickness: options.thickness,
            appearance: options.panel_theme.appearance(),
        }
        .frame(screen, units, options.color)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Command {
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
}
impl Command {
    fn spec(self) -> &'static CommandSpec {
        COMMANDS
            .iter()
            .find(|spec| spec.command == self)
            .expect("every command has a row in COMMANDS")
    }
    pub fn label(self) -> &'static str {
        let spec = self.spec();
        match spec.mac_label {
            Some(label) if cfg!(target_os = "macos") => label,
            _ => spec.label,
        }
    }
    /// Where the command sits in the panel action picker.
    pub fn category(self) -> Category {
        self.spec().category
    }
    /// Extra words the panel action picker matches when searching.
    pub fn keywords(self) -> &'static [&'static str] {
        self.spec().keywords
    }
    /// The keys the command presses, if it is a keyboard shortcut on this
    /// platform, with `PRIMARY` resolved to Meta on macOS or Ctrl elsewhere.
    pub fn shortcut(self, mac: bool) -> Option<Vec<&'static str>> {
        let spec = self.spec();
        let primary = if mac { "Meta" } else { "Ctrl" };
        let keys = if mac { spec.mac_keys } else { spec.other_keys }?;
        Some(
            keys.iter()
                .map(|&key| if key == PRIMARY { primary } else { key })
                .collect(),
        )
    }
    /// Clicks with a modifier act at a point; every other command acts on
    /// the window in front.
    pub fn at_point(self) -> bool {
        self.spec().at_point
    }
    pub fn stays_open(self) -> bool {
        self.spec().stays_open
    }
}

/// Stands for Command on macOS and Ctrl elsewhere in a shortcut.
const PRIMARY: &str = "Primary";

/// Everything fixed about one [`Command`]. See [`COMMANDS`].
struct CommandSpec {
    command: Command,
    label: &'static str,
    /// The label on macOS, when it differs.
    mac_label: Option<&'static str>,
    category: Category,
    keywords: &'static [&'static str],
    /// The shortcut on macOS. `None` means the command does something else
    /// there, such as a click or a media key.
    mac_keys: Option<&'static [&'static str]>,
    /// The shortcut on Windows, with the same rule.
    other_keys: Option<&'static [&'static str]>,
    at_point: bool,
    stays_open: bool,
}

impl CommandSpec {
    const fn new(
        command: Command,
        label: &'static str,
        category: Category,
        keywords: &'static [&'static str],
    ) -> Self {
        Self {
            command,
            label,
            mac_label: None,
            category,
            keywords,
            mac_keys: None,
            other_keys: None,
            at_point: false,
            stays_open: false,
        }
    }
    const fn on_mac_called(mut self, label: &'static str) -> Self {
        self.mac_label = Some(label);
        self
    }
    /// The same shortcut on every platform.
    const fn keys(self, keys: &'static [&'static str]) -> Self {
        self.mac_keys(keys).other_keys(keys)
    }
    const fn mac_keys(mut self, keys: &'static [&'static str]) -> Self {
        self.mac_keys = Some(keys);
        self
    }
    const fn other_keys(mut self, keys: &'static [&'static str]) -> Self {
        self.other_keys = Some(keys);
        self
    }
    const fn acts_at_point(mut self) -> Self {
        self.at_point = true;
        self
    }
    const fn keeps_menu_open(mut self) -> Self {
        self.stays_open = true;
        self
    }
}

/// Every scan menu command, listed once in picker order. Adding a command
/// means adding its enum variant, a row here, and its menu placement in
/// `Kind::rows`. A command without a shortcut also needs an arm in
/// `scan_executor::execute_command`.
const COMMANDS: &[CommandSpec] = {
    use Category::{Browser, Editing, Media, Mouse, Power, Windows};
    use Command as C;
    use CommandSpec as S;
    const P: &str = PRIMARY;
    const MODIFIER_CLICK: &[&str] = &["modifier click"];
    const TABS: &[&str] = &["tabs"];
    const ZOOM: &[&str] = &["magnify", "text size"];
    const SOUND: &[&str] = &["sound", "audio"];
    &[
        S::new(C::MiddleClick, "Middle click", Mouse, MODIFIER_CLICK).acts_at_point(),
        S::new(C::TripleClick, "Triple click", Mouse, MODIFIER_CLICK).acts_at_point(),
        S::new(C::ShiftClick, "Shift-click", Mouse, MODIFIER_CLICK).acts_at_point(),
        S::new(C::CtrlClick, "Ctrl-click", Mouse, MODIFIER_CLICK).acts_at_point(),
        S::new(C::AltClick, "Alt-click", Mouse, MODIFIER_CLICK)
            .on_mac_called("Option-click")
            .acts_at_point(),
        S::new(C::MetaClick, "Windows-click", Mouse, MODIFIER_CLICK)
            .on_mac_called("Command-click")
            .acts_at_point(),
        S::new(C::Copy, "Copy", Editing, &["clipboard", "ctrl c"]).keys(&[P, "C"]),
        S::new(C::Cut, "Cut", Editing, &["clipboard", "ctrl x"]).keys(&[P, "X"]),
        S::new(C::Paste, "Paste", Editing, &["clipboard", "ctrl v"]).keys(&[P, "V"]),
        S::new(C::SelectAll, "Select all", Editing, &["ctrl a"]).keys(&[P, "A"]),
        S::new(C::Undo, "Undo", Editing, &["ctrl z"]).keys(&[P, "Z"]),
        S::new(C::Redo, "Redo", Editing, &["ctrl y"])
            .mac_keys(&[P, "Shift", "Z"])
            .other_keys(&[P, "Y"]),
        S::new(C::Save, "Save", Editing, &["ctrl s"]).keys(&[P, "S"]),
        S::new(C::Find, "Find", Editing, &["search", "ctrl f"]).keys(&[P, "F"]),
        S::new(
            C::SwitchNext,
            "Switch app forward",
            Windows,
            &["alt tab", "application"],
        )
        .mac_keys(&["Meta", "Tab"])
        .other_keys(&["Alt", "Tab"]),
        S::new(
            C::SwitchPrevious,
            "Switch app backward",
            Windows,
            &["alt tab", "application"],
        )
        .mac_keys(&["Meta", "Shift", "Tab"])
        .other_keys(&["Alt", "Shift", "Tab"]),
        S::new(
            C::Overview,
            "App overview",
            Windows,
            &["task view", "mission control"],
        )
        .mac_keys(&["Ctrl", "ArrowUp"])
        .other_keys(&["Meta", "Tab"]),
        S::new(C::Desktop, "Show desktop", Windows, &["minimise all"])
            .mac_keys(&["Meta", "F3"])
            .other_keys(&["Meta", "D"]),
        // Windows minimizes through the window adapter instead.
        S::new(
            C::Minimize,
            "Minimize",
            Windows,
            &["minimise", "hide window"],
        )
        .mac_keys(&["Meta", "M"]),
        // Windows maximizes through the window adapter instead.
        S::new(
            C::Maximize,
            "Maximize / restore",
            Windows,
            &["maximise", "full screen", "restore"],
        )
        .on_mac_called("Toggle full screen")
        .mac_keys(&["Ctrl", "Meta", "F"]),
        // macOS closes through the window adapter instead.
        S::new(C::CloseWindow, "Close window", Windows, &["alt f4", "quit"])
            .other_keys(&["Alt", "F4"]),
        S::new(C::BrowserBack, "Back", Browser, &["history", "navigate"])
            .mac_keys(&[P, "["])
            .other_keys(&["Alt", "ArrowLeft"]),
        S::new(
            C::BrowserForward,
            "Forward",
            Browser,
            &["history", "navigate"],
        )
        .mac_keys(&[P, "]"])
        .other_keys(&["Alt", "ArrowRight"]),
        S::new(C::Reload, "Reload", Browser, &["refresh"]).keys(&[P, "R"]),
        S::new(C::NewTab, "New tab", Browser, TABS).keys(&[P, "T"]),
        S::new(C::CloseTab, "Close tab", Browser, TABS).keys(&[P, "W"]),
        S::new(C::ReopenTab, "Reopen closed tab", Browser, TABS).keys(&[P, "Shift", "T"]),
        S::new(C::NextTab, "Next tab", Browser, TABS).keys(&["Ctrl", "Tab"]),
        S::new(C::PreviousTab, "Previous tab", Browser, TABS).keys(&["Ctrl", "Shift", "Tab"]),
        S::new(
            C::Address,
            "Address bar",
            Browser,
            &["url", "location", "address bar"],
        )
        .keys(&[P, "L"]),
        S::new(C::ZoomIn, "Zoom in", Browser, ZOOM).keys(&[P, "="]),
        S::new(C::ZoomOut, "Zoom out", Browser, ZOOM).keys(&[P, "-"]),
        S::new(C::ZoomReset, "Reset zoom", Browser, ZOOM).keys(&[P, "0"]),
        S::new(C::PlayPause, "Play / pause", Media, &["music", "video"]).keeps_menu_open(),
        S::new(C::NextTrack, "Next track", Media, &["music", "skip"]).keeps_menu_open(),
        S::new(
            C::PreviousTrack,
            "Previous track",
            Media,
            &["music", "skip"],
        )
        .keeps_menu_open(),
        S::new(C::VolumeUp, "Volume up", Media, SOUND).keeps_menu_open(),
        S::new(C::VolumeDown, "Volume down", Media, SOUND).keeps_menu_open(),
        S::new(C::Mute, "Mute", Media, SOUND).keeps_menu_open(),
        S::new(
            C::ShutDown,
            "Shut down now",
            Power,
            &["power off", "turn off"],
        ),
        S::new(C::Restart, "Restart now", Power, &["reboot"]),
    ]
};

/// Every command in picker order.
pub fn all_commands() -> impl Iterator<Item = Command> {
    COMMANDS.iter().map(|spec| spec.command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Setting {
    FasterScan,
    SlowerScan,
    FasterLine,
    SlowerLine,
    LineMode,
    GridMode,
    KeyboardLayout(crate::point_scan::KeyboardLayout),
}
impl Setting {
    pub fn label(self) -> &'static str {
        match self {
            Self::FasterScan => "Faster auto scan",
            Self::SlowerScan => "Slower auto scan",
            Self::FasterLine => "Faster lines",
            Self::SlowerLine => "Slower lines",
            Self::LineMode => "Line only",
            Self::GridMode => "Grid then line",
            Self::KeyboardLayout(_) => "Keyboard layout",
        }
    }
    pub fn apply(self, config: &mut crate::point_scan::Config) {
        match self {
            Self::FasterScan | Self::SlowerScan => {
                let current = config
                    .resolved(crate::scan_preferences::Area::Menu)
                    .interval_ms;
                let next = if self == Self::FasterScan {
                    current.saturating_sub(100).max(100)
                } else {
                    current.saturating_add(100).min(10000)
                };
                if config.scan_preferences.menu.interval_ms.is_some() {
                    config.scan_preferences.menu.interval_ms = Some(next);
                } else {
                    config.block_interval_ms = next;
                }
            }
            Self::FasterLine => config.speed = (config.speed + 1).min(4),
            Self::SlowerLine => config.speed = config.speed.saturating_sub(1),
            Self::LineMode => config.mode = crate::point_scan::Mode::Line,
            Self::GridMode => config.mode = crate::point_scan::Mode::Grid,
            Self::KeyboardLayout(layout) => config.keyboard_layout = layout,
        }
    }
}
impl Kind {
    /// The page a kind opens: the retired More actions page opens Home.
    pub fn resolved(self) -> Self {
        match self {
            Self::More => Self::Home,
            kind => kind,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Actions => "Actions",
            Self::Scroll => "Scroll",
            Self::ConfirmDrag => "Confirm drag",
            Self::More => "Home",
            Self::Mouse => "More clicks",
            Self::Editing => "Editing",
            Self::Windows => "Apps and windows",
            Self::Browser => "Browser",
            Self::Tabs => "Tabs",
            Self::Zoom => "Zoom",
            Self::Media => "Media",
            Self::Displays => "Displays",
            Self::Scanning => "Scanning",
            Self::ShutDown => "Shut down",
            Self::Restart => "Restart",
            Self::Panels => "My panels",
            Self::Custom => "Panel",
        }
    }
    pub(crate) fn rows(self) -> Vec<Vec<Item>> {
        use self::Command as C;
        use Item::*;
        let items = match self {
            // The one hub: the modes, then every menu that needs no point, so
            // nothing depends on which window is in front. The other panels
            // reach these menus through their Home key.
            Self::Home | Self::More => {
                return vec![
                    vec![PointScan, MousePanel, Keyboard],
                    vec![
                        Group(Self::Windows),
                        Group(Self::Editing),
                        Group(Self::Browser),
                    ],
                    vec![Group(Self::Media), Group(Self::Zoom), Group(Self::Displays)],
                    vec![Group(Self::Scanning), Cancel],
                ]
            }
            // Only what acts at the chosen point, then the ways on. The app
            // menus are on Home, one choice away.
            Self::Actions => {
                return vec![
                    vec![LeftClick, RightClick, DoubleClick],
                    vec![Scroll, Drag, Group(Self::Mouse)],
                    vec![Keyboard, MousePanel, Home],
                    vec![NewPoint, Panels, Cancel],
                ]
            }
            Self::Scroll => return vec![vec![Up, Down], vec![Left, Right], vec![Back, Cancel]],
            Self::ConfirmDrag => {
                return vec![vec![DragHere, DestinationAgain], vec![CancelDrag, Cancel]]
            }
            Self::Mouse => vec![
                Command(C::MiddleClick),
                Command(C::TripleClick),
                Command(C::ShiftClick),
                Command(C::CtrlClick),
                Command(C::AltClick),
                Command(C::MetaClick),
                Back,
            ],
            Self::Editing => vec![
                Command(C::Copy),
                Command(C::Cut),
                Command(C::Paste),
                Command(C::SelectAll),
                Command(C::Undo),
                Command(C::Redo),
                Command(C::Save),
                Command(C::Find),
                Back,
            ],
            Self::Windows => vec![
                Command(C::SwitchNext),
                Command(C::SwitchPrevious),
                Command(C::Overview),
                Command(C::Desktop),
                Command(C::Minimize),
                Command(C::Maximize),
                Command(C::CloseWindow),
                Group(Self::ShutDown),
                Group(Self::Restart),
                Back,
            ],
            Self::Browser => vec![
                Command(C::BrowserBack),
                Command(C::BrowserForward),
                Command(C::Reload),
                Group(Self::Tabs),
                Command(C::Address),
                Back,
            ],
            Self::Tabs => vec![
                Command(C::NewTab),
                Command(C::CloseTab),
                Command(C::ReopenTab),
                Command(C::NextTab),
                Command(C::PreviousTab),
                Back,
            ],
            // Ctrl or Cmd with =, - and 0, which most apps honour, not only browsers.
            Self::Zoom => vec![
                Command(C::ZoomIn),
                Command(C::ZoomOut),
                Command(C::ZoomReset),
                Back,
            ],
            Self::Media => vec![
                Command(C::PlayPause),
                Command(C::NextTrack),
                Command(C::PreviousTrack),
                Command(C::VolumeUp),
                Command(C::VolumeDown),
                Command(C::Mute),
                Back,
            ],
            Self::Displays => vec![Display(true), Display(false), Back],
            // One row with the action in the middle: whatever the scan pattern or
            // direction, the first highlight on this page is Back or Close menu,
            // and the action needs a deliberate move and a second Select.
            // Placeholders: `Menu::panel_list` and `Menu::custom` supply the rows.
            Self::Panels | Self::Custom => return vec![vec![Back, Cancel]],
            Self::ShutDown => return vec![vec![Back, Command(C::ShutDown), Cancel]],
            Self::Restart => return vec![vec![Back, Command(C::Restart), Cancel]],
            Self::Scanning => vec![
                Switchify,
                Pause,
                Reverse,
                Setting(self::Setting::FasterScan),
                Setting(self::Setting::SlowerScan),
                Setting(self::Setting::FasterLine),
                Setting(self::Setting::SlowerLine),
                Setting(self::Setting::LineMode),
                Setting(self::Setting::GridMode),
                Back,
            ],
        };
        let actions: Vec<_> = items.into_iter().filter(|item| *item != Back).collect();
        let mut rows: Vec<_> = actions.chunks(3).map(|row| row.to_vec()).collect();
        rows.push(vec![Back, Cancel]);
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanning::FrameTile;

    /// Fails to compile when a `Command` is added, as a reminder to give it a
    /// row in `COMMANDS`.
    fn command_count() -> usize {
        use Command::*;
        match Command::Copy {
            MiddleClick | TripleClick | ShiftClick | CtrlClick | AltClick | MetaClick | Copy
            | Cut | Paste | SelectAll | Undo | Redo | Save | Find | SwitchNext | SwitchPrevious
            | Overview | Desktop | Minimize | Maximize | CloseWindow | BrowserBack
            | BrowserForward | Reload | NewTab | CloseTab | ReopenTab | NextTab | PreviousTab
            | Address | ZoomIn | ZoomOut | ZoomReset | PlayPause | NextTrack | PreviousTrack
            | VolumeUp | VolumeDown | Mute | ShutDown | Restart => 41,
        }
    }

    #[test]
    fn every_command_has_exactly_one_row() {
        assert_eq!(COMMANDS.len(), command_count());
        for spec in COMMANDS {
            let rows = COMMANDS
                .iter()
                .filter(|s| s.command == spec.command)
                .count();
            assert_eq!(rows, 1, "{:?}", spec.command);
            assert!(!spec.label.is_empty(), "{:?}", spec.command);
        }
    }

    /// The message floating beside the panel, if there is one.
    fn message(frame: &Frame) -> Option<String> {
        frame.label.as_ref().map(|label| label.text.clone())
    }

    #[test]
    fn every_menu_is_a_docked_panel_that_fits_every_screen_size_and_scale() {
        use crate::scan_preferences::PanelSize;
        let docks = crate::scan_panel::position_rows(|dock| dock, Dock::default());
        for kind in ALL_MENU_KINDS {
            for size in [PanelSize::Small, PanelSize::Medium, PanelSize::Large] {
                for (width, height) in [(320.0, 240.0), (800.0, 600.0), (1920.0, 1080.0)] {
                    let screen = Rect {
                        x: -width,
                        y: -200.0,
                        width,
                        height,
                    };
                    for units in [1.0, 1.5, 2.0] {
                        let mut menu = Menu::new(kind, 250);
                        menu.set_panel_size(size);
                        for &dock in docks[..3].iter().flatten() {
                            let frame = menu.frame(screen, units, dock, false);
                            assert!(frame.label.is_none(), "the status tile is the header");
                            let panel = frame.tiles[0].rect;
                            assert!(frame.tiles[0].is_panel_background());
                            assert_eq!(panel, dock.rect(screen, (panel.width, panel.height)));
                            for tile in &frame.tiles {
                                let rect = tile.rect;
                                assert!(rect.width > 0.0 && rect.height > 0.0);
                                assert!(rect.x >= screen.x - 0.01 && rect.y >= screen.y - 0.01);
                                assert!(rect.x + rect.width <= screen.x + screen.width + 0.01);
                                assert!(rect.y + rect.height <= screen.y + screen.height + 0.01);
                            }
                            // Every item is a text key, Close menu included.
                            let texts: Vec<_> =
                                frame.tiles.iter().map(|tile| tile.text.as_str()).collect();
                            for item in kind.rows().into_iter().flatten() {
                                assert!(texts.contains(&item.label()), "{kind:?} {item:?}");
                            }
                            // The background, the title, Settings and the items; no status line.
                            assert_eq!(
                                frame.tiles.len(),
                                kind.rows().iter().map(Vec::len).sum::<usize>() + 3
                            );
                            assert_eq!(message(&frame), None);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn home_is_the_only_page_that_lists_the_app_menus() {
        let app_menus = [
            Kind::Editing,
            Kind::Windows,
            Kind::Browser,
            Kind::Media,
            Kind::Zoom,
            Kind::Displays,
            Kind::Scanning,
        ];
        for kind in ALL_MENU_KINDS {
            let items = kind.rows().into_iter().flatten().collect::<Vec<_>>();
            for menu in app_menus {
                assert_eq!(
                    items.contains(&Item::Group(menu)),
                    kind == Kind::Home,
                    "{kind:?} {menu:?}"
                );
            }
        }
        // The page after a point holds what acts there, More clicks included.
        let actions = Kind::Actions
            .rows()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(actions.contains(&Item::Group(Kind::Mouse)));
        assert!(actions.contains(&Item::Home));
        assert_eq!(Kind::Mouse.label(), "More clicks");
        // Switches and tiles saved for the retired More actions page open Home.
        assert_eq!(Kind::More.resolved(), Kind::Home);
        assert_eq!(Kind::More.rows(), Kind::Home.rows());
        assert_eq!(
            serde_json::from_str::<Kind>("\"more\"").unwrap(),
            Kind::More
        );
    }
    #[test]
    fn every_page_has_one_close_tile_in_its_final_navigation_row() {
        for kind in ALL_MENU_KINDS {
            let rows = kind.rows();
            let back = match kind {
                // Home is the top of the menus and has nothing to go back to.
                Kind::Home => None,
                Kind::Actions => Some(Item::NewPoint),
                Kind::ConfirmDrag => Some(Item::CancelDrag),
                // Confirmation pages keep Back and Close menu beside the one action.
                Kind::ShutDown | Kind::Restart => None,
                _ => Some(Item::Back),
            };
            if matches!(kind, Kind::ShutDown | Kind::Restart) {
                assert_eq!(rows[0][0], Item::Back);
                assert_eq!(rows[0][2], Item::Cancel);
            } else if let Some(back) = back {
                // The action pages keep My panels between the way back and Close menu.
                let expected = if kind == Kind::Actions {
                    vec![back, Item::Panels, Item::Cancel]
                } else {
                    vec![back, Item::Cancel]
                };
                assert_eq!(rows.last().unwrap(), &expected, "{kind:?}");
            } else {
                assert_eq!(rows.last().unwrap().last(), Some(&Item::Cancel));
                assert!(!rows.iter().flatten().any(|item| *item == Item::Back));
            }
            assert_eq!(
                rows.iter()
                    .flatten()
                    .filter(|item| **item == Item::Cancel)
                    .count(),
                1
            );
            assert!(rows.len() <= 4 && rows.iter().all(|row| row.len() <= 3));
        }
    }

    #[test]
    fn close_tile_is_reachable_by_automatic_row_and_item_scanning_on_every_page() {
        for kind in ALL_MENU_KINDS {
            let mut menu = Menu::new(kind, 250);
            for _ in 1..kind.rows().len() {
                menu.advance(250);
            }
            assert_eq!(menu.handle(Action::Select), None);
            for _ in 1..kind.rows().last().unwrap().len() {
                menu.advance(250);
            }
            assert_eq!(menu.handle(Action::Select), Some(Item::Cancel), "{kind:?}");
        }
    }

    #[test]
    fn every_submenu_is_reachable() {
        let mut pending = vec![Kind::Actions, Kind::Home];
        let mut visited = vec![];
        let mut commands = vec![];
        while let Some(kind) = pending.pop() {
            if visited.contains(&kind) {
                continue;
            }
            visited.push(kind);
            let rows = kind.rows();
            for item in rows.into_iter().flatten() {
                match item {
                    Item::Group(k) => pending.push(k),
                    Item::Command(c) => commands.push(c),
                    _ => {}
                }
            }
        }
        assert_eq!(commands.len(), 41);
        assert!(visited.contains(&Kind::ShutDown));
        assert!(visited.contains(&Kind::Restart));
        assert!(visited.contains(&Kind::Scanning));
        assert!(visited.contains(&Kind::Displays));
        assert!(visited.contains(&Kind::Tabs));
        assert!(visited.contains(&Kind::Zoom));
    }
    #[test]
    fn power_actions_need_a_confirming_page_whose_first_highlight_is_harmless() {
        use crate::scan_preferences::{Direction, Pattern, Resolved};
        let windows = Kind::Windows
            .rows()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(windows.contains(&Item::Group(Kind::ShutDown)));
        assert!(windows.contains(&Item::Group(Kind::Restart)));
        for (kind, command) in [
            (Kind::ShutDown, Command::ShutDown),
            (Kind::Restart, Command::Restart),
        ] {
            assert_eq!(
                kind.rows(),
                [vec![Item::Back, Item::Command(command), Item::Cancel]]
            );
            assert!(!command.stays_open());
            for (pattern, direction) in [
                (Pattern::Grouped, Direction::Forward),
                (Pattern::Grouped, Direction::Reverse),
                (Pattern::Linear, Direction::Forward),
                (Pattern::Linear, Direction::Reverse),
            ] {
                let options = Resolved {
                    interval_ms: 250,
                    pattern,
                    direction,
                    ..Default::default()
                };
                // Scanning starts on Back, or on Close menu when reversed, so the
                // first Select that reaches a tile never acts; the action is one
                // step along from there.
                let harmless = if direction == Direction::Reverse {
                    Item::Cancel
                } else {
                    Item::Back
                };
                let mut menu = Menu::configured(kind, options);
                let row_first = pattern == Pattern::Grouped;
                if row_first {
                    assert_eq!(menu.handle(Action::Select), None, "{kind:?} {direction:?}");
                }
                assert_eq!(
                    menu.handle(Action::Select),
                    Some(harmless),
                    "{kind:?} {pattern:?} {direction:?}"
                );
                let mut menu = Menu::configured(kind, options);
                if row_first {
                    assert_eq!(menu.handle(Action::Select), None);
                }
                menu.advance(250);
                assert_eq!(
                    menu.handle(Action::Select),
                    Some(Item::Command(command)),
                    "{kind:?} {pattern:?} {direction:?}"
                );
            }
            // No other page offers the command.
            for other in ALL_MENU_KINDS {
                let items = other.rows().into_iter().flatten().collect::<Vec<_>>();
                assert_eq!(
                    items.contains(&Item::Command(command)),
                    other == kind,
                    "{other:?} {command:?}"
                );
            }
        }
    }
    #[test]
    fn zoom_sits_beside_the_other_app_menus_not_inside_browser() {
        let items = |kind: Kind| kind.rows().into_iter().flatten().collect::<Vec<_>>();
        // Zoom works in most apps, so it sits on Home beside Editing and Media.
        let home = items(Kind::Home);
        assert!(home.contains(&Item::Group(Kind::Zoom)));
        assert!(home.contains(&Item::Group(Kind::Editing)));
        assert!(home.contains(&Item::Group(Kind::Media)));
        for kind in ALL_MENU_KINDS {
            assert_eq!(
                items(kind).contains(&Item::Group(Kind::Zoom)),
                kind == Kind::Home,
                "{kind:?}"
            );
        }
        assert_eq!(
            Kind::Zoom.rows(),
            [
                vec![
                    Item::Command(Command::ZoomIn),
                    Item::Command(Command::ZoomOut),
                    Item::Command(Command::ZoomReset),
                ],
                vec![Item::Back, Item::Cancel],
            ]
        );
    }

    #[test]
    fn home_offers_modes_and_point_free_actions() {
        let home = Kind::Home.rows().into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(
            home,
            [
                Item::PointScan,
                Item::MousePanel,
                Item::Keyboard,
                Item::Group(Kind::Windows),
                Item::Group(Kind::Editing),
                Item::Group(Kind::Browser),
                Item::Group(Kind::Media),
                Item::Group(Kind::Zoom),
                Item::Group(Kind::Displays),
                Item::Group(Kind::Scanning),
                Item::Cancel,
            ]
        );
        // More clicks and Scroll act at a chosen point, so Home never reaches them.
        // Displays and Scanning act on the scan itself, not on a point.
        let mut pending = home.clone();
        while let Some(item) = pending.pop() {
            if let Item::Group(kind) = item {
                assert!(!matches!(kind, Kind::Mouse | Kind::Scroll));
                pending.extend(kind.rows().into_iter().flatten());
            }
        }
        assert!(Kind::Actions
            .rows()
            .iter()
            .flatten()
            .any(|item| *item == Item::Home));
    }
    #[test]
    fn the_action_page_offers_my_panels_only_once_there_are_panels() {
        let options = crate::scan_preferences::Resolved::default();
        {
            let kind = Kind::Actions;
            let with = Menu::configured_for(kind, options, true);
            let without = Menu::configured_for(kind, options, false);
            assert_eq!(with.rows, kind.rows(), "{kind:?}");
            assert_eq!(with.rows.last().unwrap()[1], Item::Panels, "{kind:?}");
            assert_eq!(without.rows.len(), with.rows.len(), "{kind:?}");
            let last = without.rows.last().unwrap();
            assert_eq!(last.len(), 2, "{kind:?}");
            assert_eq!(last[1], Item::Cancel, "{kind:?}");
            assert!(!without
                .rows
                .iter()
                .flatten()
                .any(|item| *item == Item::Panels));
        }
    }
    #[test]
    fn settings_step_tenths_and_clamp_at_limits() {
        let mut c = crate::point_scan::Config {
            block_interval_ms: 650,
            ..Default::default()
        };
        Setting::FasterScan.apply(&mut c);
        assert_eq!(c.block_interval_ms, 550);
        Setting::SlowerScan.apply(&mut c);
        assert_eq!(c.block_interval_ms, 650);
        for _ in 0..110 {
            Setting::FasterScan.apply(&mut c);
            Setting::SlowerLine.apply(&mut c);
        }
        assert_eq!((c.block_interval_ms, c.speed), (100, 0));
        for _ in 0..110 {
            Setting::SlowerScan.apply(&mut c);
            Setting::FasterLine.apply(&mut c);
        }
        assert_eq!((c.block_interval_ms, c.speed), (10000, 4));
        c.validate().unwrap();
    }
    #[test]
    fn panel_highlights_the_current_row_or_item_without_a_status_line() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let keys = |frame: &Frame| -> Vec<FrameTile> {
            frame
                .tiles
                .iter()
                .filter(|tile| {
                    !tile.is_panel_background()
                        && !tile.is_menu_bar()
                        && tile.style.map(|s| s.role) != Some(TileRole::Title)
                })
                .cloned()
                .collect()
        };
        let mut menu = Menu::new(Kind::Actions, 250);
        let row = menu.frame(screen, 1.0, Dock::default(), false);
        assert!(row.tiles[0].is_panel_background());
        assert_eq!(message(&row), None, "the highlight says it all");
        let selected: Vec<_> = keys(&row).into_iter().filter(|t| t.selected).collect();
        assert_eq!(
            selected.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
            ["Left click", "Right click", "Double click"]
        );
        assert!(selected.iter().all(|t| t.style.unwrap().row_scan));
        menu.handle(Action::Select);
        let item = menu.frame(screen, 1.0, Dock::default(), false);
        assert_eq!(message(&item), None);
        let selected: Vec<_> = keys(&item).into_iter().filter(|t| t.selected).collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].text, "Left click");
        assert!(!selected[0].style.unwrap().row_scan);
        // Keys in a row share its width, in the panels' text style.
        let keys = keys(&item);
        assert_eq!(keys[0].rect.width, keys[1].rect.width);
        assert_eq!(keys[0].rect.y, keys[2].rect.y);
        assert!(keys.iter().all(|t| t.style.is_some()));
        let close = keys.iter().find(|t| t.text == "Close menu").unwrap();
        assert_eq!(close.style.unwrap().role, TileRole::Toolbar);
    }
    #[test]
    fn errors_pauses_and_waits_clear_the_highlight_and_write_nothing() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut menu = Menu::new(Kind::Home, 250);
        // A failed action leaves the error to the workflow, which knows what went wrong.
        let error = menu.frame(screen, 1.0, Dock::default(), true);
        assert_eq!(message(&error), None);
        assert!(error.tiles.iter().all(|tile| !tile.selected));
        assert!(error.tiles.iter().any(|tile| tile.text == "Close menu"));
        menu.suspend();
        let paused = menu.frame(screen, 1.0, Dock::default(), false);
        // A pause shows only as the missing highlight; nothing is written.
        assert_eq!(message(&paused), None);
        assert!(paused.tiles.iter().all(|tile| !tile.selected));
        assert!(!paused.tiles.iter().any(|tile| tile.text.contains("paused")));
    }
    #[test]
    fn rounded_panel_has_clear_corners_and_insets_all_keys() {
        let screen = Rect {
            x: -1280.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        for kind in [Kind::Actions, Kind::Scroll, Kind::ConfirmDrag] {
            for scale in [1.0, 1.5, 2.0] {
                let frame = Menu::new(kind, 250).frame(screen, scale, Dock::default(), false);
                let panel = &frame.tiles[0];
                assert!(panel.is_panel_background());
                let content = &frame.tiles[1..];
                // The menu bar band is flush with the top; everything else is inset.
                for tile in content
                    .iter()
                    .filter(|tile| tile.style.map(|s| s.role) != Some(TileRole::Title))
                {
                    assert!(tile.rect.x > panel.rect.x);
                    assert!(tile.rect.y > panel.rect.y);
                    assert!(tile.rect.x + tile.rect.width < panel.rect.x + panel.rect.width);
                    assert!(tile.rect.y + tile.rect.height < panel.rect.y + panel.rect.height);
                }
                // Every key sits below the menu bar, with equal side margins.
                let bar = crate::scan_panel::menu_bar(&frame).unwrap();
                assert!(content
                    .iter()
                    .filter(|tile| {
                        !tile.is_menu_bar() && tile.style.map(|s| s.role) != Some(TileRole::Title)
                    })
                    .all(|tile| bar.y + bar.height < tile.rect.y));
                let left = content
                    .iter()
                    .map(|t| t.rect.x)
                    .fold(f64::INFINITY, f64::min);
                let right = content
                    .iter()
                    .map(|t| t.rect.x + t.rect.width)
                    .fold(f64::NEG_INFINITY, f64::max);
                assert!(
                    ((left - panel.rect.x) - (panel.rect.x + panel.rect.width - right)).abs()
                        < 0.001
                );
                let pixels = crate::scan_tile::bitmap(panel).unwrap();
                assert_eq!(pixels.data()[3], 0);
                let center =
                    ((pixels.height() / 2 * pixels.width() + pixels.width() / 2) * 4 + 3) as usize;
                assert_eq!(pixels.data()[center], 255);
            }
        }
    }
    #[test]
    fn escape_manual_timing_and_reverse_use_shared_navigator() {
        let mut m = Menu::new(Kind::Actions, 250);
        m.handle(Action::Select);
        m.advance(249);
        m.handle(Action::Back);
        m.advance(1);
        assert!(m.scan.nav.escaping());
        assert_eq!(m.handle(Action::Select), None);
        assert!(m.scan.nav.path().is_empty());
        assert_eq!(m.scan.nav.index(), 0);
        m.handle(Action::Reverse);
        m.advance(250);
        assert_eq!(m.scan.nav.index(), 3);
    }
    #[test]
    fn menu_speed_changes_respect_the_area_override() {
        let mut c = crate::point_scan::Config::default();
        c.scan_preferences.menu.interval_ms = Some(500);
        Setting::SlowerScan.apply(&mut c);
        assert_eq!(c.scan_preferences.menu.interval_ms, Some(600));
        assert_eq!(c.block_interval_ms, 1000);
        assert_eq!(c.point().block_interval_ms, 1000);
        c.scan_preferences.menu.interval_ms = None;
        Setting::FasterScan.apply(&mut c);
        assert_eq!(c.block_interval_ms, 900);
        assert_eq!(c.scan_preferences.menu.interval_ms, None);
    }
}
