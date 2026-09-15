//! Menu rows and navigation are independent of native windows and input.
use crate::{
    scan_tree::{Navigator, Node, Selection},
    scanning::{Action, Frame, FrameLabel, FrameTile, Interval, Rect, MAX_SCAN_CYCLES},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
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
            Self::Cancel => "Cancel",
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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    More,
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
pub struct Menu {
    pub kind: Kind,
    rows: Vec<Vec<Item>>,
    nav: Navigator<Item>,
    interval: Interval,
    period: u64,
    cycles: usize,
    forward: bool,
    pub suspended: bool,
}
impl Menu {
    pub fn new(kind: Kind, period: u64) -> Self {
        let rows = kind.rows();
        let nav = Navigator::new(
            rows.iter()
                .map(|row| Node::Branch(row.iter().copied().map(Node::Leaf).collect()))
                .collect(),
        );
        Self {
            kind,
            rows,
            nav,
            interval: Interval::default(),
            period,
            cycles: 0,
            forward: true,
            suspended: false,
        }
    }
    pub fn set_period(&mut self, period: u64) {
        self.period = period;
        self.restart_interval();
    }
    pub fn restart_interval(&mut self) {
        self.interval.reset();
        self.cycles = 0;
        self.suspended = false;
    }
    pub fn advance(&mut self, ms: u64) {
        if !self.suspended && self.interval.elapsed(ms, self.period) && self.nav.step(self.forward)
        {
            self.cycles += 1;
            self.suspended = self.cycles >= MAX_SCAN_CYCLES;
        }
    }
    pub fn handle(&mut self, action: Action) -> Option<Item> {
        if self.suspended {
            if action == Action::Select {
                self.restart_interval();
            }
            return None;
        }
        match action {
            Action::Select => {
                self.restart_interval();
                match self.nav.select() {
                    Selection::Leaf(item) => return Some(item),
                    Selection::Entered | Selection::Escaped => self.forward = true,
                    Selection::None => {}
                }
            }
            Action::Next | Action::Back => {
                self.interval.reset();
                self.forward = action == Action::Next;
                self.nav.step(self.forward);
            }
            Action::Reverse => {
                self.forward = !self.forward;
                self.restart_interval();
            }
            _ => {}
        }
        None
    }
    pub fn frame(&self, point: (i32, i32), screen: Rect, units: f64) -> Frame {
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(1) as f64;
        let logical_width = 16.0 + columns * 180.0;
        let logical_height = 56.0 + 180.0 * self.rows.len() as f64;
        let scale = units
            .min(screen.width / logical_width)
            .min(screen.height / logical_height);
        let width = logical_width * scale;
        let height = logical_height * scale;
        let panel = place(point, screen, width, height, 20.0 * scale);
        let mut frame = Frame::default();
        let active_row = self.nav.path().first().copied().unwrap_or(self.nav.index());
        for (r, row) in self.rows.iter().enumerate() {
            let tile_width = 180.0 * scale;
            for (c, item) in row.iter().enumerate() {
                let rect = Rect {
                    x: panel.x + 8.0 * scale + c as f64 * tile_width,
                    y: panel.y + (56.0 + 180.0 * r as f64) * scale,
                    width: 168.0 * scale,
                    height: 168.0 * scale,
                };
                let selected = r == active_row
                    && (self.nav.path().is_empty() || self.nav.escaping() || c == self.nav.index());
                frame.tiles.push(FrameTile {
                    color: Default::default(),
                    text: item.label().into(),
                    rect,
                    scale,
                    icon: *item,
                    selected,
                });
            }
        }
        let text = if self.suspended {
            "Select to resume"
        } else if self.nav.escaping() {
            "Back to rows"
        } else {
            match self.kind {
                Kind::Actions => "Choose an action",
                Kind::Scroll => "Scroll at selected point",
                Kind::ConfirmDrag => "Confirm drag",
                kind => kind.label(),
            }
        };
        frame.label = Some(FrameLabel {
            text: text.into(),
            rect: Rect {
                height: 48.0 * scale,
                ..panel
            },
            scale: scale * 0.75,
            hud: None,
        });
        frame
    }
}
pub fn outline(r: Rect, t: f64) -> [Rect; 4] {
    [
        Rect { height: t, ..r },
        Rect {
            y: r.y + r.height - t,
            height: t,
            ..r
        },
        Rect { width: t, ..r },
        Rect {
            x: r.x + r.width - t,
            width: t,
            ..r
        },
    ]
}
fn place(point: (i32, i32), screen: Rect, width: f64, height: f64, gap: f64) -> Rect {
    let (x, y) = (point.0 as f64, point.1 as f64);
    let candidates = [
        (x + gap, y - height / 2.0),
        (x - gap - width, y - height / 2.0),
        (x - width / 2.0, y + gap),
        (x - width / 2.0, y - gap - height),
    ];
    let (x, y) = candidates
        .into_iter()
        .find(|(x, y)| {
            *x >= screen.x
                && *y >= screen.y
                && *x + width <= screen.x + screen.width
                && *y + height <= screen.y + screen.height
        })
        .unwrap_or(candidates[0]);
    Rect {
        x: x.clamp(screen.x, screen.x + screen.width - width),
        y: y.clamp(screen.y, screen.y + screen.height - height),
        width,
        height,
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
        }
    }
    pub fn apply(self, config: &mut crate::point_scan::Config) {
        const RATES: &[u64] = &[250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000];
        match self {
            Self::FasterScan => {
                config.block_interval_ms = RATES
                    .iter()
                    .rev()
                    .copied()
                    .find(|v| *v < config.block_interval_ms)
                    .unwrap_or(RATES[0])
            }
            Self::SlowerScan => {
                config.block_interval_ms = RATES
                    .iter()
                    .copied()
                    .find(|v| *v > config.block_interval_ms)
                    .unwrap_or(5000)
            }
            Self::FasterLine => config.speed = (config.speed + 1).min(4),
            Self::SlowerLine => config.speed = config.speed.saturating_sub(1),
            Self::LineMode => config.mode = crate::point_scan::Mode::Line,
            Self::GridMode => config.mode = crate::point_scan::Mode::Grid,
        }
    }
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Actions => "Choose an action",
            Self::Scroll => "Scroll",
            Self::ConfirmDrag => "Confirm drag",
            Self::More => "More actions",
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
            Self::Actions => {
                return vec![
                    vec![LeftClick, RightClick, DoubleClick],
                    vec![Scroll, Drag, More],
                    vec![NewPoint, Cancel],
                ]
            }
            Self::Scroll => return vec![vec![Up, Down], vec![Left, Right], vec![Back]],
            Self::ConfirmDrag => return vec![vec![DragHere, DestinationAgain, CancelDrag]],
            Self::More => vec![
                Group(Self::Mouse),
                Group(Self::Editing),
                Group(Self::Windows),
                Group(Self::Browser),
                Group(Self::Media),
                Group(Self::Displays),
                Group(Self::Scanning),
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
        items.chunks(3).map(|row| row.to_vec()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_submenu_is_reachable_and_fits_a_three_by_three_grid() {
        let mut pending = vec![Kind::Actions];
        let mut visited = vec![];
        let mut commands = vec![];
        while let Some(kind) = pending.pop() {
            if visited.contains(&kind) {
                continue;
            }
            visited.push(kind);
            let rows = kind.rows();
            assert!(rows.len() <= 3);
            assert!(rows.iter().all(|r| r.len() <= 3));
            if kind != Kind::Actions {
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
    fn settings_step_presets_and_clamp_at_limits() {
        let mut c = crate::point_scan::Config {
            block_interval_ms: 650,
            ..Default::default()
        };
        Setting::FasterScan.apply(&mut c);
        assert_eq!(c.block_interval_ms, 500);
        Setting::SlowerScan.apply(&mut c);
        assert_eq!(c.block_interval_ms, 750);
        for _ in 0..20 {
            Setting::FasterScan.apply(&mut c);
            Setting::SlowerLine.apply(&mut c);
        }
        assert_eq!((c.block_interval_ms, c.speed), (250, 0));
        for _ in 0..20 {
            Setting::SlowerScan.apply(&mut c);
            Setting::FasterLine.apply(&mut c);
        }
        assert_eq!((c.block_interval_ms, c.speed), (5000, 4));
        c.validate().unwrap();
    }
    #[test]
    fn grid_keeps_square_columns_and_highlights_the_current_row_or_item() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 240.0,
        };
        let mut menu = Menu::new(Kind::Actions, 250);
        let row = menu.frame((10, 10), screen, 2.0);
        assert_eq!(row.tiles.iter().filter(|t| t.selected).count(), 3);
        assert_eq!(row.tiles[0].rect.x, row.tiles[3].rect.x);
        assert_eq!(row.tiles[1].rect.x, row.tiles[4].rect.x);
        for tile in &row.tiles {
            assert_eq!(tile.rect.width, tile.rect.height);
        }
        menu.handle(Action::Select);
        let item = menu.frame((10, 10), screen, 2.0);
        assert_eq!(item.tiles.iter().filter(|t| t.selected).count(), 1);
        assert!(item.tiles[0].selected);
    }
    #[test]
    fn layouts_fit_edges_negative_coordinates_and_scaling() {
        let screen = Rect {
            x: -1920.0,
            y: -200.0,
            width: 1920.0,
            height: 1080.0,
        };
        for point in [
            (-1920, -200),
            (-1, -200),
            (-1920, 879),
            (-1, 879),
            (-900, 300),
        ] {
            for scale in [1.0, 1.5, 2.0] {
                let frame = Menu::new(Kind::Actions, 250).frame(point, screen, scale);
                for rect in frame
                    .tiles
                    .iter()
                    .map(|tile| tile.rect)
                    .chain(frame.label.iter().map(|label| label.rect))
                {
                    assert!(rect.x >= screen.x && rect.y >= screen.y);
                    assert!(rect.x + rect.width <= screen.x + screen.width + 0.001);
                    assert!(rect.y + rect.height <= screen.y + screen.height + 0.001);
                }
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
        assert!(m.nav.escaping());
        assert_eq!(m.handle(Action::Select), None);
        assert!(m.nav.path().is_empty());
        assert_eq!(m.nav.index(), 0);
        m.handle(Action::Reverse);
        m.advance(250);
        assert_eq!(m.nav.index(), 2);
    }
}
