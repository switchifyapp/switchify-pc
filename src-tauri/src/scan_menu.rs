//! Menu rows and navigation are independent of native windows and input.
use crate::{
    scan_items::{ItemScanner, Policy},
    scan_panel::{Dock, Panel, PanelKey},
    scanning::{Action, Frame, Rect, TileRole},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Home,
    PointScan,
    Switchify,
    Keyboard,
    MousePanel,
    More,
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
}
impl Item {
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::PointScan => "Point",
            Self::Switchify => "Switchify",
            Self::Keyboard => "Keyboard",
            Self::MousePanel => "Mouse",
            Self::More => "More",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Home,
    More,
    MouseActions,
    MouseScanning,
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
}
#[cfg(test)]
pub(crate) const ALL_MENU_KINDS: [Kind; 16] = [
    Kind::Home,
    Kind::Actions,
    Kind::More,
    Kind::MouseActions,
    Kind::MouseScanning,
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
];
pub struct Menu {
    pub kind: Kind,
    rows: Vec<Vec<Item>>,
    scan: ItemScanner<Item>,
    period: u64,
}
impl Menu {
    pub fn set_panel_size(&mut self, size: crate::scan_preferences::PanelSize) {
        self.scan.options.panel_size = size;
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
        let rows = kind.rows();
        let scan = ItemScanner::configured_rows(&rows, Policy::MENU, options);
        Self {
            kind,
            rows,
            scan,
            period: options.interval_ms,
        }
    }
    pub fn suspended(&self) -> bool {
        self.scan.suspended
    }
    pub fn suspend(&mut self) {
        self.scan.suspended = true;
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

    /// Draws the menu as a docked panel. While an action has `failed`, the status
    /// stays short so it fits one line; the workflow shows the message beside it.
    pub fn frame(&self, screen: Rect, units: f64, dock: Dock, failed: bool) -> Frame {
        let (row, column) = self.scan.position(&self.rows);
        let escaping = self.scan.nav.escaping();
        let title = self.kind.label();
        let status = if failed {
            format!("{title} · Action failed · Select to return")
        } else if self.scan.waiting() {
            format!("{title} · Select to continue")
        } else if self.suspended() {
            format!("{title} paused · Select to resume")
        } else if escaping {
            crate::scan_panel::back_status(&self.scan).to_owned()
        } else {
            crate::scan_panel::item_status(
                title,
                &self.scan,
                self.rows[row][column.unwrap_or(0)].label(),
            )
        };
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
                            text: item.label().into(),
                            weight: 1.0,
                            role: item.role(),
                            active: false,
                            blank: false,
                            hidden: false,
                        })
                        .collect()
                })
                .collect(),
            status,
            note: None,
            dock,
            // Nothing can be chosen while an error or pause waits for Select.
            selected: (!failed && !self.suspended()).then(|| self.scan.highlight(&self.rows)),
            row_scan: self.scan.row_scan() || self.scan.group_scan() || escaping,
            row_label: None,
            thickness: options.thickness,
        }
        .frame(screen, units, options.color)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
impl Command {
    pub fn label(self) -> &'static str {
        match self {
            Self::MiddleClick => "Middle click",
            Self::TripleClick => "Triple click",
            Self::ShiftClick => "Shift-click",
            Self::CtrlClick => "Ctrl-click",
            Self::AltClick => {
                if cfg!(target_os = "macos") {
                    "Option-click"
                } else {
                    "Alt-click"
                }
            }
            Self::MetaClick => {
                if cfg!(target_os = "macos") {
                    "Command-click"
                } else {
                    "Windows-click"
                }
            }
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::SelectAll => "Select all",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Save => "Save",
            Self::Find => "Find",
            Self::SwitchNext => "Switch app forward",
            Self::SwitchPrevious => "Switch app backward",
            Self::Overview => "App overview",
            Self::Desktop => "Show desktop",
            Self::Minimize => "Minimize",
            Self::Maximize => {
                if cfg!(target_os = "macos") {
                    "Toggle full screen"
                } else {
                    "Maximize / restore"
                }
            }
            Self::CloseWindow => "Close window",
            Self::BrowserBack => "Back",
            Self::BrowserForward => "Forward",
            Self::Reload => "Reload",
            Self::NewTab => "New tab",
            Self::CloseTab => "Close tab",
            Self::ReopenTab => "Reopen closed tab",
            Self::NextTab => "Next tab",
            Self::PreviousTab => "Previous tab",
            Self::Address => "Address bar",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ZoomReset => "Reset zoom",
            Self::PlayPause => "Play / pause",
            Self::NextTrack => "Next track",
            Self::PreviousTrack => "Previous track",
            Self::VolumeUp => "Volume up",
            Self::VolumeDown => "Volume down",
            Self::Mute => "Mute",
        }
    }
    pub fn stays_open(self) -> bool {
        matches!(
            self,
            Self::PlayPause
                | Self::NextTrack
                | Self::PreviousTrack
                | Self::VolumeUp
                | Self::VolumeDown
                | Self::Mute
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Actions => "Actions",
            Self::Scroll => "Scroll",
            Self::ConfirmDrag => "Confirm drag",
            Self::More => "More actions",
            Self::MouseActions => "Actions",
            Self::MouseScanning => "Scanning",
            Self::Mouse => "Mouse",
            Self::Editing => "Editing",
            Self::Windows => "Apps and windows",
            Self::Browser => "Browser",
            Self::Tabs => "Tabs",
            Self::Zoom => "Zoom",
            Self::Media => "Media",
            Self::Displays => "Displays",
            Self::Scanning => "Scanning",
        }
    }
    fn rows(self) -> Vec<Vec<Item>> {
        use self::Command as C;
        use Item::*;
        let items = match self {
            // Only actions that need no point, so nothing depends on which window is in front.
            Self::Home => {
                return vec![
                    vec![PointScan, MousePanel, Keyboard],
                    vec![
                        Group(Self::Windows),
                        Group(Self::Editing),
                        Group(Self::Browser),
                    ],
                    vec![Group(Self::Media), Switchify, Cancel],
                ]
            }
            Self::Actions => {
                return vec![
                    vec![LeftClick, RightClick, DoubleClick],
                    vec![Scroll, Drag, More],
                    vec![Keyboard, MousePanel, Home],
                    vec![NewPoint, Cancel],
                ]
            }
            Self::Scroll => return vec![vec![Up, Down], vec![Left, Right], vec![Back, Cancel]],
            Self::ConfirmDrag => {
                return vec![vec![DragHere, DestinationAgain], vec![CancelDrag, Cancel]]
            }
            Self::More => vec![
                Keyboard,
                MousePanel,
                Group(Self::Mouse),
                Group(Self::Editing),
                Group(Self::Windows),
                Group(Self::Browser),
                Group(Self::Media),
                Group(Self::Displays),
                Group(Self::Scanning),
                Back,
            ],
            Self::MouseActions => vec![
                Group(Self::Mouse),
                Group(Self::Editing),
                Group(Self::Windows),
                Group(Self::Browser),
                Group(Self::Media),
                Group(Self::Displays),
                Group(Self::MouseScanning),
                Keyboard,
                Home,
                Back,
            ],
            Self::MouseScanning => vec![
                Pause,
                Reverse,
                Setting(self::Setting::FasterScan),
                Setting(self::Setting::SlowerScan),
                Back,
            ],
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
                Back,
            ],
            Self::Browser => vec![
                Command(C::BrowserBack),
                Command(C::BrowserForward),
                Command(C::Reload),
                Group(Self::Tabs),
                Group(Self::Zoom),
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
            Self::Scanning => vec![
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

    fn is_status(tile: &FrameTile) -> bool {
        tile.style
            .is_some_and(|style| style.role == TileRole::Status)
    }
    fn status(frame: &Frame) -> String {
        frame
            .tiles
            .iter()
            .find(|tile| is_status(tile))
            .unwrap()
            .text
            .clone()
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
                            assert_eq!(
                                frame.tiles.len(),
                                kind.rows().iter().map(Vec::len).sum::<usize>() + 4
                            );
                            assert!(status(&frame).starts_with(kind.label()));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn mouse_actions_expose_app_commands_without_point_only_controls() {
        let root = Kind::MouseActions
            .rows()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        for kind in [
            Kind::Mouse,
            Kind::Editing,
            Kind::Windows,
            Kind::Browser,
            Kind::Media,
            Kind::Displays,
            Kind::MouseScanning,
        ] {
            assert!(root.contains(&Item::Group(kind)));
        }
        assert!(!root.contains(&Item::NewPoint));
        assert!(!root.contains(&Item::MousePanel));
        assert!(root.contains(&Item::Keyboard));
        let mut menu = Menu::new(Kind::MouseActions, 250);
        menu.advance(250);
        menu.advance(250);
        assert_eq!(menu.handle(Action::Select), None);
        menu.advance(250);
        assert_eq!(menu.handle(Action::Select), Some(Item::Keyboard));
        let scanning = Kind::MouseScanning
            .rows()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(scanning.contains(&Item::Pause));
        assert!(scanning.contains(&Item::Reverse));
        assert!(!scanning.contains(&Item::Setting(Setting::LineMode)));
        assert!(!scanning.contains(&Item::Setting(Setting::GridMode)));
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
                _ => Some(Item::Back),
            };
            if let Some(back) = back {
                assert_eq!(rows.last().unwrap(), &[back, Item::Cancel], "{kind:?}");
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
            assert_eq!(Item::Cancel.label(), "Close menu");
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
    fn every_submenu_is_reachable_and_fits_three_columns_and_four_rows() {
        let mut pending = vec![Kind::Actions, Kind::Home];
        let mut visited = vec![];
        let mut commands = vec![];
        while let Some(kind) = pending.pop() {
            if visited.contains(&kind) {
                continue;
            }
            visited.push(kind);
            let rows = kind.rows();
            assert!(rows.len() <= 4);
            assert!(rows.iter().all(|r| r.len() <= 3));
            if !matches!(kind, Kind::Actions | Kind::Home) {
                assert!(rows.iter().flatten().any(|i| *i == Item::Back));
            }
            for item in rows.into_iter().flatten() {
                match item {
                    Item::More => pending.push(Kind::More),
                    Item::Group(k) => pending.push(k),
                    Item::Command(c) => commands.push(c),
                    _ => {}
                }
            }
        }
        assert_eq!(commands.len(), 39);
        assert!(visited.contains(&Kind::Scanning));
        assert!(visited.contains(&Kind::Displays));
        assert!(visited.contains(&Kind::Tabs));
        assert!(visited.contains(&Kind::Zoom));
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
                Item::Switchify,
                Item::Cancel,
            ]
        );
        // Mouse and Displays act at a point or move it; Scanning settings restart the point scan.
        let mut pending = home.clone();
        while let Some(item) = pending.pop() {
            if let Item::Group(kind) = item {
                assert!(!matches!(
                    kind,
                    Kind::Mouse | Kind::Displays | Kind::Scanning | Kind::Scroll
                ));
                pending.extend(kind.rows().into_iter().flatten());
            }
        }
        for kind in [Kind::Actions, Kind::MouseActions] {
            assert!(kind.rows().iter().flatten().any(|item| *item == Item::Home));
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
    fn panel_highlights_the_current_row_or_item_with_panel_status() {
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
                        && !is_status(tile)
                        && !tile.is_menu_bar()
                        && tile.style.map(|s| s.role) != Some(TileRole::Title)
                })
                .cloned()
                .collect()
        };
        let mut menu = Menu::new(Kind::Actions, 250);
        let row = menu.frame(screen, 1.0, Dock::default(), false);
        assert!(row.tiles[0].is_panel_background());
        assert_eq!(status(&row), "Actions · Select a row");
        let selected: Vec<_> = keys(&row).into_iter().filter(|t| t.selected).collect();
        assert_eq!(
            selected.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
            ["Left click", "Right click", "Double click"]
        );
        assert!(selected.iter().all(|t| t.style.unwrap().row_scan));
        menu.handle(Action::Select);
        let item = menu.frame(screen, 1.0, Dock::default(), false);
        assert_eq!(status(&item), "Actions · Select Left click");
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
    fn errors_pauses_and_waits_show_in_the_status_without_a_highlight() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut menu = Menu::new(Kind::Home, 250);
        let error = menu.frame(screen, 1.0, Dock::default(), true);
        assert_eq!(status(&error), "Home · Action failed · Select to return");
        assert!(error.tiles.iter().all(|tile| !tile.selected));
        assert!(error.tiles.iter().any(|tile| tile.text == "Close menu"));
        menu.suspend();
        let paused = menu.frame(screen, 1.0, Dock::default(), false);
        assert_eq!(status(&paused), "Home paused · Select to resume");
        assert!(paused.tiles.iter().all(|tile| !tile.selected));
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
                // The status header sits above every key, with equal side margins.
                let header = content.iter().find(|tile| is_status(tile)).unwrap();
                assert!(content
                    .iter()
                    .filter(|tile| {
                        !is_status(tile)
                            && !tile.is_menu_bar()
                            && tile.style.map(|s| s.role) != Some(TileRole::Title)
                    })
                    .all(|tile| header.rect.y + header.rect.height < tile.rect.y));
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
