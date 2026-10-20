//! Point selection policy, menu navigation and execution requests. No OS input.
use crate::{
    point_scan::{Engine, PointSettings},
    scan_menu::{Item, Kind, Menu},
    scanning::{Action, Frame, FrameLabel, Rect, Technique, UpdateContext},
};
use serde::Serialize;
pub type Point = (i32, i32);
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
    Prediction {
        token: u64,
        index: usize,
    },
    PredictionRetry,
    OpenKeyboard,
    /// Shows the Switchify window. Scanning has ended.
    OpenSwitchify,
    OpenMouse,
    OpenPoint,
    Keyboard(crate::scan_keyboard::Stroke),
    KeyboardPunctuation(crate::scan_keyboard::Punctuation),
    MouseMove {
        dx: i32,
        dy: i32,
    },
    MouseMoveAbsolute {
        x: i32,
        y: i32,
    },
    MouseClick {
        right: bool,
        count: u8,
    },
    MouseScroll {
        dy: i32,
    },
    MouseDrag,
    MouseSpeed(i8),
    MouseMonitor(i8, i8),
    MouseCycleDisplay(bool),
    Click {
        point: Point,
        right: bool,
        count: u8,
    },
    Scroll {
        point: Point,
        dx: i32,
        dy: i32,
    },
    Command {
        command: crate::scan_menu::Command,
        point: Point,
    },
    Setting(crate::scan_menu::Setting),
    Display(bool),
    DragStart(Point),
    DragMove(Point),
    DragEnd(Point),
}
pub fn default_click(point: Point) -> Request {
    Request::Click {
        point,
        right: false,
        count: 1,
    }
}
/// Only navigation keys repeat, never text, Space, Enter or a modifier.
/// Shift and Ctrl may come along (selecting, deleting words), but not Alt or
/// the Windows/Command key, whose shortcuts switch and move windows.
fn repeatable(stroke: crate::scan_keyboard::Stroke) -> bool {
    !stroke.modifiers[2]
        && !stroke.modifiers[3]
        && matches!(
            stroke.key,
            crate::scan_keyboard::Key::Named(name)
                if crate::mouse_repeat::RepeatKey::parse(name).is_some()
        )
}
fn selection_policy(point: Point, options: crate::scan_preferences::Resolved) -> (Point, Menu) {
    (point, Menu::configured(Kind::Actions, options))
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Phase {
    Point(crate::point_scan::Phase),
    Workflow(WorkflowPhase),
}
impl Default for Phase {
    fn default() -> Self {
        Self::Point(crate::point_scan::Phase::Idle)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowPhase {
    Keyboard,
    KeyboardSuspended,
    KeyboardOpening,
    KeyboardRepeating,
    Mouse,
    MouseSuspended,
    MouseMoving,
    MouseScrolling,
    MouseRadar,
    Menu,
    MenuSuspended,
    AutoSelecting,
    DragDestination,
    DragConfirmation,
    Executing,
}
#[derive(PartialEq)]
enum Stage {
    Keyboard,
    KeyboardOpening,
    Mouse,
    MouseMoving,
    MouseScrolling,
    MouseRadar,
    Idle,
    Countdown,
    Point,
    Menu,
    Destination,
    Executing,
}
/// A navigation key chosen on the scanned keyboard. It repeats once choosing
/// it has typed it, until a switch stops it.
struct KeyRepeat {
    stroke: crate::scan_keyboard::Stroke,
    timer: crate::mouse_repeat::ScanKeyRepeat,
    running: bool,
}
pub struct Workflow {
    keyboard: crate::scan_keyboard::Keyboard,
    keyboard_layout_pending: bool,
    keyboard_area: Rect,
    mouse: crate::scan_mouse::MousePanel,
    mouse_area: Rect,
    /// Shared by the keyboard and mouse panels and the menus, and kept when any is rebuilt.
    dock: crate::scan_panel::Dock,
    /// The dock last loaded or saved, so only a newly chosen dock is saved.
    saved_dock: crate::scan_panel::Dock,
    /// Latest pointer position, used to move an open panel out of the pointer's way.
    pointer: Option<(f64, f64)>,
    /// Where the open panel is drawn while it avoids the pointer.
    moved: Option<crate::scan_panel::Dock>,
    return_to_mouse: bool,
    /// The keyboard was opened from Home and returns there when closed.
    return_to_home: bool,
    /// The request just chosen in Home left it, and returns there if it fails.
    leaving_home: bool,
    /// The last scan ended by performing an action, so Select continues in
    /// the same mode instead of opening Home.
    resume: bool,
    mouse_actions_open: bool,
    move_repeat: Option<crate::mouse_repeat::ScanMoveRepeat>,
    scroll_repeat: Option<crate::mouse_repeat::ScanScrollRepeat>,
    /// Chooses a pointer target for the mouse panel, which it hides meanwhile.
    radar: Option<crate::scan_radar::Radar>,
    /// The radar is drawn in the cursor overlay's colour once it is known.
    cursor_color: Option<crate::scanning::ScannerColor>,
    mouse_repeat_enabled: bool,
    mouse_move_interval_ms: u32,
    mouse_scroll_interval_ms: u32,
    mouse_acceleration_ms: u32,
    key_repeat: Option<KeyRepeat>,
    scan_key_repeat_enabled: bool,
    key_repeat_interval_ms: u32,
    key_repeat_initial_delay_ms: u32,
    point: Engine,
    stage: Stage,
    menu: Menu,
    parent_menu: Vec<Menu>,
    source: Point,
    destination: Point,
    elapsed: u64,
    pending: Option<Request>,
    error: Option<String>,
    /// A point scan in which no switch has moved or chosen anything yet.
    following: bool,
}
impl Workflow {
    pub fn new(config: PointSettings, screen: Rect, scale: f64) -> Result<Self, String> {
        let menu_options = config.menu_scan;
        let keyboard_options = config.keyboard_scan;
        let mouse_options = config.mouse_scan;
        Ok(Self {
            keyboard: crate::scan_keyboard::Keyboard::configured(
                cfg!(target_os = "macos"),
                keyboard_options,
            )
            .with_layout(config.keyboard_layout)
            .with_wait_after_typing(
                config.keyboard_scan.next_scan == crate::scan_preferences::NextScan::Wait,
            ),
            keyboard_layout_pending: false,
            keyboard_area: screen,
            mouse: crate::scan_mouse::MousePanel::new(mouse_options, 1, 100)
                .with_repeat_stop_edge(config.mouse_repeat_stop_edge),
            mouse_area: screen,
            dock: Default::default(),
            saved_dock: Default::default(),
            pointer: None,
            moved: None,
            return_to_mouse: false,
            return_to_home: false,
            leaving_home: false,
            resume: false,
            mouse_actions_open: false,
            move_repeat: None,
            scroll_repeat: None,
            radar: None,
            cursor_color: None,
            mouse_repeat_enabled: true,
            mouse_move_interval_ms: 250,
            mouse_scroll_interval_ms: 250,
            mouse_acceleration_ms: 1000,
            key_repeat: None,
            scan_key_repeat_enabled: false,
            key_repeat_interval_ms: 250,
            key_repeat_initial_delay_ms: 500,
            point: Engine::new(config, screen, scale)?,
            stage: Stage::Idle,
            menu: Menu::configured(Kind::Actions, menu_options),
            parent_menu: vec![],
            source: (0, 0),
            destination: (0, 0),
            elapsed: 0,
            pending: None,
            error: None,
            following: false,
        })
    }
    fn new_keyboard(&self) -> crate::scan_keyboard::Keyboard {
        let mut keyboard = crate::scan_keyboard::Keyboard::configured(
            cfg!(target_os = "macos"),
            self.point.config.keyboard_scan,
        )
        .with_layout(self.point.config.keyboard_layout)
        .with_wait_after_typing(
            self.point.config.keyboard_scan.next_scan == crate::scan_preferences::NextScan::Wait,
        );
        keyboard.dock = self.dock;
        keyboard
    }
    fn new_mouse(&self) -> crate::scan_mouse::MousePanel {
        let mut mouse = crate::scan_mouse::MousePanel::new(
            self.point.config.mouse_scan,
            1,
            self.mouse.speed_percent,
        )
        .with_repeat_stop_edge(self.point.config.mouse_repeat_stop_edge);
        mouse.dock = self.dock;
        mouse
    }
    /// Starts with the panels at a previously saved dock.
    pub fn with_dock(mut self, dock: crate::scan_panel::Dock) -> Self {
        self.set_dock(dock);
        self.saved_dock = dock;
        self
    }
    /// The dock chosen since it was last loaded or saved, until it is saved.
    /// A panel moved out of the pointer's way keeps its chosen dock.
    pub fn unsaved_dock(&self) -> Option<crate::scan_panel::Dock> {
        (self.dock != self.saved_dock).then_some(self.dock)
    }
    pub fn dock_saved(&mut self, dock: crate::scan_panel::Dock) {
        self.saved_dock = dock;
    }
    fn set_dock(&mut self, dock: crate::scan_panel::Dock) {
        self.dock = dock;
        self.keyboard.dock = dock;
        self.mouse.dock = dock;
    }
    fn active_preferences(&self) -> crate::scan_preferences::Resolved {
        match self.stage {
            Stage::Keyboard | Stage::KeyboardOpening => self.point.config.keyboard_scan,
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling | Stage::MouseRadar => {
                self.point.config.mouse_scan
            }
            Stage::Menu => self.point.config.menu_scan,
            _ => self.point.config.scan,
        }
    }
    pub fn apply_config(&mut self, config: PointSettings, restart: bool) {
        if self.stage == Stage::Countdown {
            self.pending = None;
            self.elapsed = 0;
            self.open(Kind::Actions);
        }
        self.point.config = config;
        self.keyboard
            .set_panel_size(self.point.config.keyboard_scan.panel_size);
        self.mouse
            .set_panel_size(self.point.config.mouse_scan.panel_size);
        self.menu
            .set_panel_size(self.point.config.menu_scan.panel_size);
        for menu in &mut self.parent_menu {
            menu.set_panel_size(self.point.config.menu_scan.panel_size);
        }

        self.keyboard.set_layout(self.point.config.keyboard_layout);
        self.menu
            .set_period(self.point.config.menu_scan.interval_ms);
        for menu in &mut self.parent_menu {
            menu.set_period(self.point.config.menu_scan.interval_ms);
        }
        if restart {
            self.keyboard = self.new_keyboard();
            self.keyboard_layout_pending = false;
            self.mouse = self.new_mouse();
            self.return_to_mouse = false;
            self.mouse_actions_open = false;
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.radar = None;
            self.key_repeat = None;
            self.parent_menu.clear();
            self.begin_point();
        }
    }
    pub fn prediction_keyboard(&mut self) -> Option<&mut crate::scan_keyboard::Keyboard> {
        if self.stage == Stage::Keyboard {
            self.keyboard
                .enable_predictions(self.point.config.word_prediction);
            Some(&mut self.keyboard)
        } else {
            None
        }
    }
    /// Home or an action menu is showing.
    pub fn menu_open(&self) -> bool {
        self.stage == Stage::Menu
    }
    pub fn keyboard_open(&self) -> bool {
        matches!(self.stage, Stage::Keyboard | Stage::KeyboardOpening)
    }
    pub fn mouse_open(&self) -> bool {
        matches!(
            self.stage,
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling | Stage::MouseRadar
        ) || (self.stage == Stage::Menu && self.mouse_actions_open)
    }
    pub fn control_mode(&self) -> crate::point_scan::ControlMode {
        self.point.config.control_mode
    }
    /// Carries over whether the scan that came before ended with an action,
    /// and the dock it chose.
    pub fn resume_from(&mut self, previous: &Self) {
        self.resume = previous.resume;
        // The chosen dock stays even when it could not be saved.
        self.set_dock(previous.dock);
        self.saved_dock = previous.saved_dock;
    }
    /// Home, or a menu opened from it, is showing. Nothing in it acts at a point.
    pub fn home_open(&self) -> bool {
        self.stage == Stage::Menu
            && (self.menu.kind == Kind::Home
                || self
                    .parent_menu
                    .first()
                    .is_some_and(|menu| menu.kind == Kind::Home))
    }
    fn open_home(&mut self) {
        self.mouse_actions_open = false;
        self.return_to_home = false;
        self.parent_menu.clear();
        self.pending = None;
        self.error = None;
        self.elapsed = 0;
        // Home acts on whatever window is in front, never on a chosen point.
        self.source = (0, 0);
        self.destination = (0, 0);
        self.open(Kind::Home);
    }
    pub fn set_mouse_area(&mut self, area: Rect, screen: Rect, scale: f64, displays: usize) {
        if self.point.screen != screen
            && matches!(
                self.stage,
                Stage::MouseMoving | Stage::MouseScrolling | Stage::MouseRadar
            )
        {
            self.stage = Stage::Mouse;
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.radar = None;
            self.pending = None;
            self.mouse.restart();
        }
        self.mouse_area = area;
        self.point.screen = screen;
        self.point.units_per_logical_pixel = scale;
        self.mouse.set_displays(displays);
    }
    pub fn set_mouse_settings(
        &mut self,
        speed: u16,
        acceleration_ms: u32,
        move_interval_ms: u32,
        scroll_interval_ms: u32,
        repeat_enabled: bool,
    ) {
        let changed = self.mouse.speed_percent != speed
            || self.mouse_acceleration_ms != acceleration_ms
            || self.mouse_move_interval_ms != move_interval_ms
            || self.mouse_scroll_interval_ms != scroll_interval_ms
            || self.mouse_repeat_enabled != repeat_enabled;
        self.mouse.speed_percent = speed;
        self.mouse_acceleration_ms = acceleration_ms;
        self.mouse_move_interval_ms = move_interval_ms;
        self.mouse_scroll_interval_ms = scroll_interval_ms;
        self.mouse_repeat_enabled = repeat_enabled;
        if changed && matches!(self.stage, Stage::MouseMoving | Stage::MouseScrolling) {
            self.stage = Stage::Mouse;
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.pending = None;
            self.mouse.restart();
        }
    }
    pub fn set_cursor_color(&mut self, color: crate::scanning::ScannerColor) {
        self.cursor_color = Some(color);
    }
    pub fn set_key_repeat_settings(
        &mut self,
        enabled: bool,
        interval_ms: u32,
        initial_delay_ms: u32,
    ) {
        let changed = self.scan_key_repeat_enabled != enabled
            || self.key_repeat_interval_ms != interval_ms
            || self.key_repeat_initial_delay_ms != initial_delay_ms;
        self.scan_key_repeat_enabled = enabled;
        self.key_repeat_interval_ms = interval_ms;
        self.key_repeat_initial_delay_ms = initial_delay_ms;
        if changed {
            self.end_key_repeat();
        }
    }
    fn key_repeating(&self) -> bool {
        self.stage == Stage::Keyboard
            && self
                .key_repeat
                .as_ref()
                .is_some_and(|repeat| repeat.running)
    }
    /// Ends a key repeat, returning whether one was running. Each press was
    /// released as it was typed, so nothing is left held down.
    fn end_key_repeat(&mut self) -> bool {
        let running = self.key_repeating();
        self.key_repeat = None;
        if running {
            self.pending = None;
            self.keyboard.repeat_stopped();
        }
        running
    }
    pub fn mouse_feedback(&self) -> Option<crate::input::PointerFeedback> {
        use crate::input::PointerFeedback;
        if self.mouse.error {
            return None;
        }
        match self.stage {
            Stage::MouseMoving => Some(PointerFeedback::RepeatMove {
                accelerated: self.mouse_acceleration_ms > 0,
                dragging: self.mouse.dragging,
            }),
            Stage::MouseScrolling => Some(PointerFeedback::RepeatScroll {
                dx: 0,
                dy: self.scroll_repeat.as_ref()?.dy(),
            }),
            Stage::Mouse => Some(if self.mouse.dragging {
                PointerFeedback::Drag
            } else {
                PointerFeedback::Move
            }),
            Stage::Menu if self.mouse_actions_open => Some(PointerFeedback::Move),
            // A drag stays held while the radar chooses where to drop it.
            Stage::MouseRadar if self.mouse.dragging => Some(PointerFeedback::Drag),
            _ => None,
        }
    }
    pub fn typing_moved(&mut self) {
        self.end_key_repeat();
        self.keyboard.typing_moved();
    }
    pub fn prediction_enabled(&self) -> bool {
        self.point.config.word_prediction
    }

    pub fn set_keyboard_area(&mut self, area: Rect) {
        self.keyboard_area = area;
    }
    /// The display the open keyboard is on changed, or was replaced.
    pub fn set_keyboard_display(&mut self, area: Rect, screen: Rect, scale: f64) {
        self.keyboard_area = area;
        self.point.screen = screen;
        self.point.units_per_logical_pixel = scale;
    }
    pub fn panel_avoids_pointer(&self) -> bool {
        self.point.config.panel_avoids_pointer
    }
    pub fn set_pointer(&mut self, pointer: Option<(f64, f64)>) {
        self.pointer = pointer;
        if self.stage == Stage::Menu && self.mouse_actions_open {
            if let Some((x, y)) = pointer {
                self.source = (x.round() as i32, y.round() as i32);
            }
        }
        self.moved = self.avoiding();
    }
    /// Menus dock in the work area of the display they act on. Each area is kept
    /// current only while its own panel is in use, so one left on another display
    /// is passed over.
    fn menu_area(&self) -> Rect {
        let screen = self.point.screen;
        let areas = if self.mouse_actions_open {
            [self.mouse_area, self.keyboard_area]
        } else {
            [self.keyboard_area, self.mouse_area]
        };
        areas
            .into_iter()
            .find(|area| {
                area.x >= screen.x
                    && area.y >= screen.y
                    && area.x + area.width <= screen.x + screen.width
                    && area.y + area.height <= screen.y + screen.height
            })
            .unwrap_or(screen)
    }
    fn menu_frame(&self) -> Frame {
        self.menu.frame(
            self.menu_area(),
            self.point.units_per_logical_pixel,
            self.dock,
            self.error.is_some(),
        )
    }
    /// Why the open menu's action failed, in a prompt in the space the menu panel
    /// leaves in its work area: above or below it, or else beside it. Windows
    /// always draws a two-line prompt 154 units tall, so that is the height kept
    /// clear. Only when no space fits does the prompt overlap the panel.
    fn error_label(&self, error: &str, panel: Option<Rect>) -> FrameLabel {
        let s = self.point.units_per_logical_pixel;
        let area = self.menu_area();
        let full_width = (640.0 * s).min(area.width);
        let narrowest = 240.0 * s;
        let height = (154.0 * s).min(area.height);
        let margin = 20.0 * s;
        let (right, bottom) = (area.x + area.width, area.y + area.height);
        let centred = area.x + (area.width - full_width) / 2.0;
        let rect = match panel {
            None => Rect {
                x: centred,
                y: area.y + margin,
                width: full_width,
                height,
            },
            Some(panel) => {
                let above = panel.y - area.y;
                let below = bottom - (panel.y + panel.height);
                let left = panel.x - area.x;
                let beside = right - (panel.x + panel.width);
                let vertical = |above_panel: bool| Rect {
                    x: centred,
                    // At the edge of the area, or against the panel when the space is short.
                    y: if above_panel {
                        (area.y + margin).min(panel.y - height)
                    } else {
                        (bottom - margin - height).max(panel.y + panel.height)
                    },
                    width: full_width,
                    height,
                };
                if above.max(below) >= height || left.max(beside) < narrowest {
                    vertical(above > below)
                } else {
                    let space = left.max(beside);
                    let width = (space - 2.0 * margin).max(narrowest).min(full_width);
                    let start = if left > beside {
                        area.x
                    } else {
                        panel.x + panel.width
                    };
                    Rect {
                        x: start + (space - width) / 2.0,
                        y: panel.y + (panel.height - height) / 2.0,
                        width,
                        height,
                    }
                }
            }
        };
        let error = error.trim();
        FrameLabel {
            text: format!(
                "{}\nSelect to return",
                if error.is_empty() {
                    "Action failed."
                } else {
                    error
                }
            ),
            rect: Rect {
                x: rect.x.clamp(area.x, right - rect.width),
                y: rect.y.clamp(area.y, bottom - rect.height),
                ..rect
            },
            scale: s,
            hud: Some(crate::scanning::HudPresentation {
                screen: self.point.screen,
                scale: s,
            }),
        }
    }
    /// Dock to draw the open panel at instead of `dock`, while the pointer is over it.
    fn avoiding(&self) -> Option<crate::scan_panel::Dock> {
        let area = match self.stage {
            Stage::Keyboard => self.keyboard_area,
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling => self.mouse_area,
            Stage::Menu => self.menu_area(),
            _ => return None,
        };
        if !self.point.config.panel_avoids_pointer {
            return None;
        }
        // Use the same measured frame as rendering, never a keyboard-sized proxy.
        let frame = match self.stage {
            Stage::Keyboard => self.keyboard.frame(
                area,
                self.point.units_per_logical_pixel,
                self.point.config.scanner_color,
            ),
            Stage::Menu => self.menu_frame(),
            _ => self.mouse.frame(
                area,
                self.point.units_per_logical_pixel,
                self.point.config.mouse_scan.color,
                None,
            ),
        };
        let bounds = frame
            .tiles
            .iter()
            .find(|tile| tile.is_panel_background())?
            .rect;
        self.dock.avoiding(
            self.pointer?,
            area,
            (bounds.width, bounds.height),
            self.moved,
        )
    }
    fn open(&mut self, kind: Kind) {
        self.menu = Menu::configured(kind, self.point.config.menu_scan);
        self.stage = Stage::Menu;
    }
    /// The point has been used. The next scan waits for Select unless the
    /// user chose to have it start by itself.
    fn used(&mut self) {
        if self.point.config.scan.next_scan == crate::scan_preferences::NextScan::Automatic {
            self.new_point();
        } else {
            self.stage = Stage::Idle;
            self.resume = true;
        }
    }
    fn new_point(&mut self) {
        self.parent_menu.clear();
        self.begin_point();
    }
    fn begin_point(&mut self) {
        self.stage = Stage::Point;
        self.point.start();
        self.following = true;
    }
    /// A window may come forward or go as a point scan begins. Until a
    /// switch is used in the scan, no point depends on which is in front.
    /// Home brings windows forward itself and acts only on the one in front.
    pub fn follows_foreground(&self) -> bool {
        (self.following && self.stage == Stage::Point) || self.home_open()
    }
    fn restore_actions(&mut self) {
        if let Some(menu) = self.parent_menu.pop() {
            self.menu = menu;
            self.menu.restart_interval();
            self.stage = Stage::Menu;
        } else {
            self.open(Kind::Actions);
        }
    }
    /// Opens the mouse panel. Opened by a selection, it follows the user's
    /// choice to wait after a selection.
    fn open_mouse(&mut self, selected: bool) -> Option<Request> {
        self.point.config.control_mode = crate::point_scan::ControlMode::Mouse;
        self.stage = Stage::Mouse;
        self.mouse_actions_open = false;
        self.return_to_home = false;
        self.move_repeat = None;
        self.scroll_repeat = None;
        self.radar = None;
        self.mouse = self.new_mouse();
        if selected {
            self.mouse.opened();
        }
        self.pending = None;
        self.parent_menu.clear();
        self.elapsed = 0;
        self.error = None;
        self.return_to_mouse = false;
        // A point chosen earlier is not kept for a window that may change.
        self.source = (0, 0);
        self.destination = (0, 0);
        Some(Request::OpenMouse)
    }
    fn open_point(&mut self) -> Option<Request> {
        self.point.config.control_mode = crate::point_scan::ControlMode::Point;
        self.reset();
        self.begin_point();
        Some(Request::OpenPoint)
    }
    fn mouse_key(&mut self, key: crate::scan_mouse::Key) -> Option<Request> {
        use crate::scan_mouse::Key;
        if key == Key::Actions {
            if self.mouse.dragging {
                self.mouse.block_actions("End drag before opening Actions");
                return None;
            }
            if self.pointer.is_none() {
                self.mouse.block_actions("Pointer unavailable · Try again");
                return None;
            }
        }
        if key == Key::Radar {
            // Without the pointer, the display it is on may not be known.
            if self.pointer.is_none() {
                self.mouse.block_actions("Pointer unavailable · Try again");
                return None;
            }
            self.mouse.choose(key);
            self.radar = Some(crate::scan_radar::Radar::new(
                self.point.config.mouse_scan,
                self.point.config.radar_speed,
                self.point.screen,
                self.point.units_per_logical_pixel,
            ));
            self.stage = Stage::MouseRadar;
            return None;
        }
        self.mouse.choose(key);
        self.set_dock(self.mouse.dock);
        match key {
            Key::Move(dx, dy) => {
                let step = crate::mouse_repeat::DIRECTION_MOVE_DELTA;
                let delta = (i32::from(dx) * step, i32::from(dy) * step);
                if self.mouse_repeat_enabled {
                    let mut repeat = crate::mouse_repeat::ScanMoveRepeat::new(
                        delta.0,
                        delta.1,
                        std::time::Instant::now(),
                    );
                    let initial = repeat.initial_move();
                    self.move_repeat = Some(repeat);
                    self.stage = Stage::MouseMoving;
                    Some(Request::MouseMove {
                        dx: initial.0,
                        dy: initial.1,
                    })
                } else {
                    let scale = f64::from(self.mouse.speed_percent) / 100.0;
                    Some(Request::MouseMove {
                        dx: (f64::from(delta.0) * scale).round() as i32,
                        dy: (f64::from(delta.1) * scale).round() as i32,
                    })
                }
            }
            Key::Click => Some(Request::MouseClick {
                right: false,
                count: 1,
            }),
            Key::RightClick => Some(Request::MouseClick {
                right: true,
                count: 1,
            }),
            Key::DoubleClick => Some(Request::MouseClick {
                right: false,
                count: 2,
            }),
            Key::Drag => {
                self.mouse.dragging = !self.mouse.dragging;
                Some(Request::MouseDrag)
            }
            Key::Scroll(direction) => {
                if self.mouse_repeat_enabled {
                    let repeat = crate::mouse_repeat::ScanScrollRepeat::new(direction);
                    let dy = repeat.dy();
                    self.scroll_repeat = Some(repeat);
                    self.stage = Stage::MouseScrolling;
                    Some(Request::MouseScroll { dy })
                } else {
                    Some(Request::MouseScroll {
                        dy: i32::from(direction) * 5,
                    })
                }
            }
            Key::Speed(direction) => Some(Request::MouseSpeed(direction)),
            Key::Monitor(dx, dy) => Some(Request::MouseMonitor(dx, dy)),
            Key::Keyboard => {
                self.return_to_mouse = true;
                self.mouse.dragging = false;
                self.stage = Stage::KeyboardOpening;
                self.keyboard = self.new_keyboard();
                Some(Request::OpenKeyboard)
            }
            Key::Close => self.open_point(),
            Key::Actions => {
                self.mouse_actions_open = true;
                self.parent_menu.clear();
                self.source = self
                    .pointer
                    .map_or((0, 0), |(x, y)| (x.round() as i32, y.round() as i32));
                self.open(Kind::MouseActions);
                None
            }
            Key::Radar | Key::More | Key::Movement | Key::Dock | Key::Position(_) | Key::Back => {
                None
            }
        }
    }
    /// Leaves the radar for the mouse panel. A chosen target is an action.
    fn end_radar(&mut self, chosen: bool) {
        self.radar = None;
        self.stage = Stage::Mouse;
        if chosen {
            self.mouse.repeat_stopped();
        } else {
            self.mouse.opened();
        }
    }
    /// Goes back from a direction or distance to choosing an area, and from
    /// choosing an area to the mouse panel, without moving the pointer.
    fn radar_back(&mut self) {
        if !self.radar.as_mut().is_some_and(|radar| radar.back()) {
            self.end_radar(false);
        }
    }
    /// Takes the pointer to the radar's chosen centre, then along with its
    /// marker, only when that has moved. A held drag drags along with it.
    fn radar_follow(&mut self) -> Option<Request> {
        let (x, y) = self.radar.as_mut()?.follow()?;
        Some(Request::MouseMoveAbsolute { x, y })
    }
    /// What the scanned keyboard's choice asks for. A navigation key also
    /// prepares a repeat when that is turned on.
    fn keyboard_output(&mut self, output: Option<crate::scan_keyboard::Output>) -> Option<Request> {
        match output {
            Some(crate::scan_keyboard::Output::Stroke(stroke)) => {
                if self.scan_key_repeat_enabled && repeatable(stroke) {
                    self.key_repeat = Some(KeyRepeat {
                        stroke,
                        timer: crate::mouse_repeat::ScanKeyRepeat::new(),
                        running: false,
                    });
                }
                Some(Request::Keyboard(stroke))
            }
            Some(crate::scan_keyboard::Output::Punctuation(punctuation)) => {
                Some(Request::KeyboardPunctuation(punctuation))
            }
            Some(crate::scan_keyboard::Output::Prediction { token, index }) => {
                Some(Request::Prediction { token, index })
            }
            Some(crate::scan_keyboard::Output::RetryPrediction) => Some(Request::PredictionRetry),
            Some(crate::scan_keyboard::Output::Layout(layout)) => {
                self.keyboard_layout_pending = true;
                Some(Request::Setting(crate::scan_menu::Setting::KeyboardLayout(
                    layout,
                )))
            }
            Some(crate::scan_keyboard::Output::Close) => {
                self.keyboard_closed();
                None
            }
            None => None,
        }
    }
    /// Chooses a key on the open mouse panel without scanning to it.
    #[cfg(test)]
    pub(crate) fn choose_mouse_key(&mut self, key: crate::scan_mouse::Key) -> Option<Request> {
        self.mouse_key(key)
    }
    /// Chooses a key on the open keyboard without scanning to it.
    #[cfg(test)]
    pub(crate) fn choose_key(&mut self, key: crate::scan_keyboard::Key) -> Option<Request> {
        let output = self.keyboard.choose(key);
        self.keyboard_output(output)
    }
    fn return_to_mouse_panel(&mut self) {
        self.radar = None;
        self.parent_menu.clear();
        self.mouse_actions_open = false;
        self.stage = Stage::Mouse;
        self.mouse.opened();
    }
    fn keyboard_closed(&mut self) {
        if self.return_to_mouse {
            self.return_to_mouse = false;
            self.return_to_mouse_panel();
        } else if self.return_to_home {
            self.open_home();
        } else {
            self.resume_mode(true);
        }
    }
    /// Starts the mode used last.
    fn resume_mode(&mut self, selected: bool) {
        let mode = self.point.config.control_mode;
        self.reset();
        if mode == crate::point_scan::ControlMode::Mouse {
            let _ = self.open_mouse(selected);
        } else {
            self.begin_point();
        }
    }
    fn selected(&mut self, item: Item) -> Option<Request> {
        match item {
            Item::Home => self.open_home(),
            // No point exists yet, so a failure goes back to Home, never to an action menu.
            Item::PointScan => {
                let request = self.open_point();
                self.leaving_home = true;
                return request;
            }
            Item::Switchify => {
                self.reset();
                self.leaving_home = true;
                return Some(Request::OpenSwitchify);
            }
            Item::Keyboard => {
                self.return_to_home = self.home_open();
                self.return_to_mouse = self.mouse_open();
                if self.return_to_mouse {
                    self.mouse.dragging = false;
                }
                self.mouse_actions_open = false;
                self.radar = None;
                self.stage = Stage::KeyboardOpening;
                self.keyboard = self.new_keyboard();
                self.pending = None;
                self.parent_menu.clear();
                self.elapsed = 0;
                self.error = None;
                return Some(Request::OpenKeyboard);
            }
            Item::MousePanel => return self.open_mouse(true),
            Item::More | Item::Group(_) => {
                let kind = if let Item::Group(kind) = item {
                    kind
                } else {
                    Kind::More
                };
                let next = Menu::configured(kind, self.point.config.menu_scan);
                self.parent_menu
                    .push(std::mem::replace(&mut self.menu, next));
            }
            Item::Command(command) => {
                if self.mouse_actions_open
                    && matches!(
                        command,
                        crate::scan_menu::Command::MiddleClick
                            | crate::scan_menu::Command::TripleClick
                            | crate::scan_menu::Command::ShiftClick
                            | crate::scan_menu::Command::CtrlClick
                            | crate::scan_menu::Command::AltClick
                            | crate::scan_menu::Command::MetaClick
                    )
                    && self.pointer.is_none()
                {
                    self.error = Some("Pointer unavailable. Try again.".into());
                    return None;
                }
                if command.stays_open() {
                    self.menu.continue_after_selection();
                } else if self.mouse_actions_open {
                    self.return_to_mouse_panel();
                } else if self.home_open() {
                    // Stay in Home so the user can go on, for example to
                    // bring another app forward and then choose a mode.
                    self.open_home();
                } else {
                    self.used();
                }
                return Some(Request::Command {
                    command,
                    point: self.source,
                });
            }
            Item::Setting(setting) => return Some(Request::Setting(setting)),
            Item::Display(next) if self.mouse_actions_open => {
                self.return_to_mouse_panel();
                return Some(Request::MouseCycleDisplay(next));
            }
            Item::Display(next) => return Some(Request::Display(next)),
            Item::Pause => self.menu.suspend(),
            Item::Reverse => {
                self.menu.handle(Action::Reverse);
            }

            Item::LeftClick | Item::RightClick | Item::DoubleClick => {
                self.used();
                return Some(if item == Item::LeftClick {
                    default_click(self.source)
                } else {
                    Request::Click {
                        point: self.source,
                        right: item == Item::RightClick,
                        count: if item == Item::DoubleClick { 2 } else { 1 },
                    }
                });
            }
            Item::Scroll | Item::Drag => {
                let next = Menu::configured(Kind::Scroll, self.point.config.menu_scan);
                self.parent_menu
                    .push(std::mem::replace(&mut self.menu, next));
                if item == Item::Drag {
                    self.stage = Stage::Destination;
                    self.point.start();
                }
            }
            Item::NewPoint => self.new_point(),
            Item::Cancel if self.mouse_actions_open => self.return_to_mouse_panel(),
            Item::Cancel => self.reset(),
            Item::Up | Item::Down | Item::Left | Item::Right => {
                self.menu.continue_after_selection();
                let (dx, dy) = match item {
                    Item::Up => (0, 3),
                    Item::Down => (0, -3),
                    Item::Left => (-3, 0),
                    _ => (3, 0),
                };
                return Some(Request::Scroll {
                    point: self.source,
                    dx,
                    dy,
                });
            }
            Item::Back | Item::CancelDrag
                if self.mouse_actions_open && self.parent_menu.is_empty() =>
            {
                self.return_to_mouse_panel();
            }
            Item::Back | Item::CancelDrag => self.restore_actions(),
            Item::DestinationAgain => {
                self.stage = Stage::Destination;
                self.point.start();
            }
            Item::DragHere => {
                self.stage = Stage::Executing;
                self.elapsed = 0;
                return Some(Request::DragStart(self.source));
            }
        }
        None
    }
}
impl Technique for Workflow {
    type Selection = Request;
    type Phase = Phase;
    fn execution_failed(&mut self, message: String) {
        if self.stage == Stage::Keyboard {
            self.key_repeat = None;
            self.pending = None;
            if self.keyboard_layout_pending {
                self.keyboard_layout_pending = false;
                self.keyboard.layout_change_failed();
            } else {
                self.keyboard.failed();
            }
            return;
        }
        // The keyboard did not open from the mouse panel, which is still there.
        let returning = self.stage == Stage::KeyboardOpening && self.return_to_mouse;
        self.return_to_mouse &= !returning;
        if self.mouse_open() || returning {
            self.stage = Stage::Mouse;
            self.mouse_actions_open = false;
            self.parent_menu.clear();
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.radar = None;
            self.mouse.dragging = false;
            self.mouse.failed();
            self.error = None;
            let _ = message;
            return;
        }
        // A failure in Home, or in a keyboard opened from it, returns to Home.
        let home = self.home_open() || self.return_to_home || self.leaving_home;
        self.return_to_home = false;
        self.leaving_home = false;
        self.pending = None;
        self.following = false;
        self.stage = Stage::Menu;
        self.menu = Menu::configured(
            if home { Kind::Home } else { Kind::Actions },
            self.point.config.menu_scan,
        );
        self.parent_menu.clear();
        self.menu.suspend();
        self.error = Some(message);
    }
    /// Open Mouse takes the radar from a direction or distance back to
    /// picking an area. From picking an area it opens the panel afresh.
    fn steps_back(&self, action: Action) -> bool {
        action == Action::OpenMouse
            && self.stage == Stage::MouseRadar
            && self
                .radar
                .as_ref()
                .is_some_and(|radar| radar.phase != crate::scan_radar::Phase::Area)
    }
    fn execution_succeeded(&mut self) {
        self.leaving_home = false;
        if self.stage == Stage::KeyboardOpening {
            self.stage = Stage::Keyboard;
            self.keyboard.opened();
        } else if self.stage == Stage::Keyboard {
            self.keyboard_layout_pending = false;
            let typed = self
                .keyboard
                .succeeded_with_context(crate::prediction::keyboard_input_context());
            // The chosen key repeats only once it has been typed.
            if self
                .key_repeat
                .as_ref()
                .is_some_and(|repeat| !repeat.running)
            {
                match self.key_repeat.as_mut() {
                    Some(repeat) if typed => repeat.running = true,
                    _ => self.key_repeat = None,
                }
            }
        }
    }
    fn start(&mut self) {
        if self.resume || self.point.config.start_with == crate::point_scan::StartWith::LastMode {
            // The Select that starts scanning is not waited after.
            self.resume_mode(false);
        } else {
            self.reset();
            self.open_home();
        }
    }
    fn reset(&mut self) {
        self.keyboard = self.new_keyboard();
        self.keyboard_layout_pending = false;
        self.mouse = self.new_mouse();
        self.return_to_mouse = false;
        self.return_to_home = false;
        self.leaving_home = false;
        self.resume = false;
        self.mouse_actions_open = false;
        self.move_repeat = None;
        self.scroll_repeat = None;
        self.radar = None;
        self.key_repeat = None;
        self.stage = Stage::Idle;
        self.parent_menu.clear();
        self.pending = None;
        self.error = None;
        self.following = false;
        self.elapsed = 0;
        self.source = (0, 0);
        self.destination = (0, 0);
        self.point.reset();
    }
    fn finished(&self) -> bool {
        self.stage == Stage::Idle
    }
    fn complete_on_selection(&self) -> bool {
        false
    }
    fn automatic(&self, _fallback: bool) -> bool {
        self.active_preferences().automatic
    }
    fn mouse_repeating(&self) -> bool {
        matches!(self.stage, Stage::MouseMoving | Stage::MouseScrolling) || self.key_repeating()
    }
    fn mouse_repeat_stop_edge(&self) -> crate::point_scan::MouseRepeatStopEdge {
        self.point.config.mouse_repeat_stop_edge
    }
    fn stop_mouse_repeat(&mut self) -> bool {
        if self.end_key_repeat() {
            return true;
        }
        if !matches!(self.stage, Stage::MouseMoving | Stage::MouseScrolling) {
            return false;
        }
        self.stage = Stage::Mouse;
        self.move_repeat = None;
        self.scroll_repeat = None;
        self.pending = None;
        self.mouse.repeat_stopped();
        true
    }
    fn auto_selecting(&self) -> bool {
        self.stage == Stage::Countdown
    }
    fn pausable(&self) -> bool {
        self.stage != Stage::Executing
    }
    fn handle(&mut self, action: Action) -> Option<Request> {
        self.following = false;
        self.leaving_home = false;
        // Anything that reaches a repeating key stops it, and only an
        // explicit change of mode goes on to act.
        if self.end_key_repeat()
            && !matches!(
                action,
                Action::OpenPoint | Action::OpenMouse | Action::OpenKeyboard
            )
        {
            return None;
        }
        self.key_repeat = None;
        if action == Action::OpenPoint {
            return self.open_point();
        }
        if action == Action::OpenMouse {
            if self.stage == Stage::MouseRadar {
                self.radar_back();
                return None;
            }
            if self.mouse_open() {
                return None;
            }
            return self.open_mouse(true);
        }
        if action == Action::OpenKeyboard {
            if self.keyboard_open() {
                return None;
            }
            return self.selected(Item::Keyboard);
        }
        match self.stage {
            Stage::Keyboard => {
                let output = self
                    .keyboard
                    .handle_with_context(action, crate::prediction::keyboard_input_context());
                self.set_dock(self.keyboard.dock);
                return self.keyboard_output(output);
            }
            Stage::KeyboardOpening => {}
            Stage::Mouse => {
                if let Some(key) = self.mouse.handle(action) {
                    return self.mouse_key(key);
                }
            }
            Stage::MouseMoving | Stage::MouseScrolling => {}
            Stage::MouseRadar => {
                if let Some((x, y)) = self.radar.as_mut().and_then(|radar| radar.handle(action)) {
                    self.end_radar(true);
                    return Some(Request::MouseMoveAbsolute { x, y });
                }
                return self.radar_follow();
            }
            Stage::Point | Stage::Destination => {
                if let Some(point) = self.point.handle(action) {
                    if self.stage == Stage::Destination {
                        self.destination = point;
                        self.open(Kind::ConfirmDrag);
                    } else {
                        let (target, menu) = selection_policy(point, self.point.config.menu_scan);
                        self.source = target;
                        self.menu = menu;
                        self.elapsed = 0;
                        self.stage = if self.point.config.auto_select_enabled {
                            Stage::Countdown
                        } else {
                            Stage::Menu
                        };
                    }
                }
            }
            Stage::Menu => {
                if self.error.is_some() {
                    if action == Action::Select {
                        self.error = None;
                        self.menu.restart_interval();
                    }
                    return None;
                }
                if let Some(item) = self.menu.handle(action) {
                    return self.selected(item);
                }
            }
            Stage::Countdown => {
                self.elapsed = 0;
                self.open(Kind::Actions);
            }
            Stage::Idle | Stage::Executing => {}
        }
        None
    }
    fn advance(&mut self, ms: u64) {
        match self.stage {
            Stage::Keyboard => self
                .keyboard
                .advance(ms, self.point.config.keyboard_scan.interval_ms),
            Stage::Mouse => self
                .mouse
                .advance(ms, self.point.config.mouse_scan.interval_ms),
            Stage::MouseRadar => {
                if let Some(radar) = self.radar.as_mut() {
                    radar.advance(ms);
                    if radar.exhausted() {
                        self.radar_back();
                    } else if let Some(request) = self.radar_follow() {
                        // At most one move each tick, as the marker moves.
                        self.pending = Some(request);
                    }
                }
            }
            Stage::Point | Stage::Destination => {
                self.point.advance(ms);
                if self.point.exhausted() {
                    self.point.start();
                    if self.stage == Stage::Point {
                        // Nothing was chosen, so Select starts afresh.
                        self.resume = false;
                        self.stage = Stage::Idle;
                    } else {
                        self.restore_actions();
                    }
                }
            }
            Stage::Menu => self.menu.advance(ms),
            _ => {}
        }
    }
    fn update(&mut self, ms: u64, context: UpdateContext) {
        if self.key_repeating() {
            if context.paused {
                self.end_key_repeat();
                return;
            }
            let (delay, interval) = (
                self.key_repeat_initial_delay_ms,
                self.key_repeat_interval_ms,
            );
            if let Some(repeat) = self.key_repeat.as_mut() {
                if repeat.timer.advance(ms, delay, interval) {
                    self.pending = Some(Request::Keyboard(repeat.stroke));
                }
            }
            return;
        }
        if self.stage == Stage::MouseMoving {
            let (dx, dy) = self.move_repeat.as_mut().map_or((0, 0), |repeat| {
                repeat.advance(
                    ms,
                    self.mouse_move_interval_ms,
                    self.mouse.speed_percent,
                    self.mouse_acceleration_ms,
                )
            });
            if dx != 0 || dy != 0 {
                self.pending = Some(Request::MouseMove { dx, dy });
            }
        } else if self.stage == Stage::MouseScrolling {
            if let Some(repeat) = self.scroll_repeat.as_mut() {
                if repeat.advance(ms, self.mouse_scroll_interval_ms) {
                    self.pending = Some(Request::MouseScroll { dy: repeat.dy() });
                }
            }
        } else if self.stage == Stage::Countdown {
            if !context.paused && !context.switch_held {
                self.elapsed = self
                    .elapsed
                    .saturating_add(ms)
                    .min(self.point.config.auto_select_delay_ms);
                if self.elapsed == self.point.config.auto_select_delay_ms {
                    self.pending = Some(default_click(self.source));
                    self.used();
                }
            }
        } else if self.stage == Stage::Executing {
            self.elapsed = (self.elapsed + ms).min(300);
            let t = self.elapsed as f64 / 300.0;
            let p = (
                (self.source.0 as f64 + (self.destination.0 as f64 - self.source.0 as f64) * t)
                    .round() as i32,
                (self.source.1 as f64 + (self.destination.1 as f64 - self.source.1 as f64) * t)
                    .round() as i32,
            );
            self.pending = Some(if self.elapsed == 300 {
                self.used();
                Request::DragEnd(p)
            } else {
                Request::DragMove(p)
            });
        } else if context.movement_enabled {
            self.advance(ms);
        }
    }
    fn take_selection(&mut self) -> Option<Request> {
        self.pending.take()
    }
    fn phase(&self) -> Phase {
        match self.stage {
            Stage::KeyboardOpening => Phase::Workflow(WorkflowPhase::KeyboardOpening),
            Stage::Mouse => Phase::Workflow(if self.mouse.suspended() {
                WorkflowPhase::MouseSuspended
            } else {
                WorkflowPhase::Mouse
            }),
            Stage::MouseMoving => Phase::Workflow(WorkflowPhase::MouseMoving),
            Stage::MouseScrolling => Phase::Workflow(WorkflowPhase::MouseScrolling),
            Stage::MouseRadar => Phase::Workflow(WorkflowPhase::MouseRadar),
            Stage::Keyboard => Phase::Workflow(if self.key_repeating() {
                WorkflowPhase::KeyboardRepeating
            } else if self.keyboard.suspended() {
                WorkflowPhase::KeyboardSuspended
            } else {
                WorkflowPhase::Keyboard
            }),
            Stage::Idle => Phase::default(),
            Stage::Countdown => Phase::Workflow(WorkflowPhase::AutoSelecting),
            Stage::Point => Phase::Point(self.point.phase()),
            Stage::Destination => Phase::Workflow(WorkflowPhase::DragDestination),
            Stage::Executing => Phase::Workflow(WorkflowPhase::Executing),
            Stage::Menu => Phase::Workflow(if self.menu.suspended() {
                WorkflowPhase::MenuSuspended
            } else if self.menu.kind == Kind::ConfirmDrag {
                WorkflowPhase::DragConfirmation
            } else {
                WorkflowPhase::Menu
            }),
        }
    }
    fn frame(&self) -> Frame {
        let panel_area = match self.stage {
            Stage::Keyboard => self.keyboard_area,
            Stage::Menu => self.menu_area(),
            _ => self.mouse_area,
        };
        let mut frame = match self.stage {
            Stage::Keyboard => self.keyboard.frame_repeating(
                self.keyboard_area,
                self.point.units_per_logical_pixel,
                self.point.config.scanner_color,
                self.key_repeat
                    .as_ref()
                    .filter(|_| self.key_repeating())
                    .map(|repeat| {
                        (
                            repeat.stroke,
                            self.point.config.mouse_repeat_stop_edge.instruction(),
                        )
                    }),
            ),
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling => self.mouse.frame(
                self.mouse_area,
                self.point.units_per_logical_pixel,
                self.point.config.mouse_scan.color,
                match self.stage {
                    Stage::MouseMoving => Some(crate::scan_mouse::RepeatPrompt::Moving),
                    Stage::MouseScrolling => Some(crate::scan_mouse::RepeatPrompt::Scrolling(
                        self.scroll_repeat
                            .as_ref()
                            .map_or(1, |repeat| repeat.dy().signum()),
                    )),
                    _ => None,
                },
            ),
            Stage::Countdown => {
                let scale = self.point.units_per_logical_pixel;
                let screen = self.point.screen;
                let width = (480.0 * scale).min(screen.width);
                Frame {
                    countdown: Some(crate::scanning::Countdown {
                        point: self.source,
                        scale,
                        permille: (self.elapsed * 1000 / self.point.config.auto_select_delay_ms)
                            as u16,
                        color: self.point.config.scanner_color,
                    }),
                    label: Some(FrameLabel {
                        text: "Press a switch for the action menu.".into(),
                        rect: Rect {
                            x: screen.x + (screen.width - width) / 2.0,
                            y: screen.y + 20.0 * scale,
                            width,
                            height: (64.0 * scale).min(screen.height),
                        },
                        scale,
                        hud: Some(crate::scanning::HudPresentation { screen, scale }),
                    }),
                    ..Frame::default()
                }
            }
            Stage::MouseRadar => self
                .radar
                .as_ref()
                .map(crate::scan_radar::Radar::frame)
                .unwrap_or_default(),
            Stage::Point | Stage::Destination => self.point.frame(),
            Stage::Menu => self.menu_frame(),
            _ => Frame::default(),
        };
        if let Some(moved) = self.avoiding() {
            crate::scan_panel::move_frame(&mut frame, moved, panel_area);
        }
        if let (Stage::Menu, Some(error)) = (&self.stage, &self.error) {
            let panel = frame
                .tiles
                .iter()
                .find(|tile| tile.is_panel_background())
                .map(|tile| tile.rect);
            frame.label = Some(self.error_label(error, panel));
        }
        if matches!(self.stage, Stage::Menu | Stage::Destination) && !self.home_open() {
            let s = self.point.units_per_logical_pixel;
            frame
                .strips
                .extend(crosshair(self.source, self.point.screen, s));
            // A docked menu is not beside the destination, so it is marked too.
            if self.stage == Stage::Menu && self.menu.kind == Kind::ConfirmDrag {
                frame
                    .strips
                    .extend(crosshair(self.destination, self.point.screen, s));
            }
            if self.stage == Stage::Destination {
                frame.label = Some(FrameLabel {
                    text: "Choose drag destination".into(),
                    hud: Some(crate::scanning::HudPresentation {
                        screen: self.point.screen,
                        scale: s,
                    }),
                    rect: Rect {
                        x: self.point.screen.x,
                        y: self.point.screen.y,
                        width: (480.0 * s).min(self.point.screen.width),
                        height: 64.0 * s,
                    },
                    scale: s,
                });
            }
        }
        let preferences = self.active_preferences();
        frame.color = match (&self.stage, self.cursor_color) {
            (Stage::MouseRadar, Some(color)) => color,
            _ => preferences.color,
        };
        for tile in &mut frame.tiles {
            tile.color = frame.color;
            tile.thickness = preferences.thickness;
        }
        frame
    }
}
/// Four arms around a chosen point, leaving its centre clear so the exact spot
/// stays visible. Arms keep their full thickness on screen, as scan lines do, and
/// are cut short at its edges; one too short to draw is left out.
fn crosshair(point: Point, screen: Rect, s: f64) -> Vec<Rect> {
    let (x, y) = (point.0 as f64, point.1 as f64);
    let (gap, reach, t) = (4.0 * s, 16.0 * s, 2.0 * s);
    let (right, bottom) = (screen.x + screen.width, screen.y + screen.height);
    let across_x = (x - t / 2.0).clamp(screen.x, (right - t).max(screen.x));
    let across_y = (y - t / 2.0).clamp(screen.y, (bottom - t).max(screen.y));
    let horizontal = |from: f64, to: f64| {
        let from = from.max(screen.x);
        Rect {
            x: from,
            y: across_y,
            width: to.min(right) - from,
            height: t,
        }
    };
    let vertical = |from: f64, to: f64| {
        let from = from.max(screen.y);
        Rect {
            x: across_x,
            y: from,
            width: t,
            height: to.min(bottom) - from,
        }
    };
    [
        horizontal(x - reach, x - gap),
        horizontal(x + gap, x + reach),
        vertical(y - reach, y - gap),
        vertical(y + gap, y + reach),
    ]
    .into_iter()
    .filter(|arm| arm.valid())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point_scan::Config, scanning::Session};

    /// Select starts the last mode, as these scans were written for.
    fn last_mode() -> Config {
        Config {
            start_with: crate::point_scan::StartWith::LastMode,
            ..Config::default()
        }
    }
    fn home_session() -> Session<Workflow> {
        Session::new(
            Workflow::new(
                Config {
                    block_interval_ms: 250,
                    ..Config::default()
                }
                .point(),
                Rect {
                    x: -1000.0,
                    y: 20.0,
                    width: 1000.0,
                    height: 800.0,
                },
                1.0,
            )
            .unwrap(),
            false,
        )
    }
    /// The next scan, made the way the runtime makes it when Select starts one.
    fn next_session(previous: &Session<Workflow>) -> Session<Workflow> {
        let mut next = home_session();
        next.technique.resume_from(&previous.technique);
        next
    }
    fn choose_in(s: &mut Session<Workflow>, row: usize, column: usize) -> Option<Request> {
        for _ in 0..row {
            s.action(Action::Next);
        }
        assert_eq!(s.action(Action::Select), None);
        for _ in 0..column {
            s.action(Action::Next);
        }
        s.action(Action::Select)
    }

    #[test]
    fn select_opens_home_until_an_action_lets_the_next_select_continue() {
        let mut s = home_session();
        assert_eq!(s.action(Action::Select), None);
        assert!(s.technique.home_open());
        assert_eq!(s.technique.menu.kind, Kind::Home);
        assert!(s.technique.follows_foreground());
        let frame = s.technique.frame();
        // Home is docked like the panels, at the bottom of the screen by default.
        let panel = frame.tiles[0].rect;
        assert!(frame.tiles[0].is_panel_background());
        assert!((panel.x + panel.width / 2.0 + 500.0).abs() < 0.001);
        assert!((panel.y + panel.height - 820.0).abs() < 0.001);
        assert!(frame.label.is_none());
        // No chosen point is marked, because Home has none.
        assert!(frame.strips.is_empty());

        assert_eq!(choose_in(&mut s, 0, 0), Some(Request::OpenPoint));
        assert!(matches!(s.technique.phase(), Phase::Point(_)));
        assert!(s.technique.follows_foreground());
        s.action(Action::Select);
        s.action(Action::Select);
        assert_eq!(s.technique.menu.kind, Kind::Actions);
        assert!(!s.technique.home_open());
        assert!(matches!(
            choose_in(&mut s, 0, 0),
            Some(Request::Click { right: false, .. })
        ));
        assert!(!s.active());

        // After an action, Select carries on with point scanning.
        let mut s = next_session(&s);
        s.action(Action::Select);
        assert!(matches!(s.technique.phase(), Phase::Point(_)));
        // Stopping means starting afresh from Home.
        s.action(Action::Stop);
        let mut s = next_session(&s);
        s.action(Action::Select);
        assert!(s.technique.home_open());
    }

    #[test]
    fn a_point_scan_that_times_out_starts_afresh_from_home() {
        let mut s = home_session();
        s.action(Action::Select);
        choose_in(&mut s, 0, 0);
        s.technique.resume = true;
        for _ in 0..100_000 {
            if !s.active() {
                break;
            }
            s.tick(100, false);
            assert_eq!(s.take_selection(), None);
        }
        assert!(!s.active());
        let mut s = next_session(&s);
        s.action(Action::Select);
        assert!(s.technique.home_open());
    }

    #[test]
    fn home_commands_return_to_home_and_close_or_switchify_end_scanning() {
        use crate::scan_menu::Command;
        let mut s = home_session();
        s.action(Action::Select);
        assert_eq!(choose_in(&mut s, 1, 0), None);
        assert_eq!(s.technique.menu.kind, Kind::Windows);
        assert!(s.technique.home_open());
        assert!(s.technique.follows_foreground());
        assert_eq!(
            choose_in(&mut s, 0, 0),
            Some(Request::Command {
                command: Command::SwitchNext,
                point: (0, 0),
            })
        );
        assert_eq!(s.technique.menu.kind, Kind::Home);
        assert!(s.technique.parent_menu.is_empty());
        assert!(s.active());

        assert_eq!(choose_in(&mut s, 2, 2), None);
        assert!(!s.active());

        let mut s = next_session(&s);
        s.action(Action::Select);
        assert_eq!(choose_in(&mut s, 2, 1), Some(Request::OpenSwitchify));
        assert!(!s.active());

        let mut s = next_session(&s);
        s.action(Action::Select);
        assert!(s.technique.home_open());
        // Back from a Home group returns to Home.
        choose_in(&mut s, 1, 1);
        assert_eq!(s.technique.menu.kind, Kind::Editing);
        assert_eq!(choose_in(&mut s, 3, 0), None);
        assert_eq!(s.technique.menu.kind, Kind::Home);
        assert!(s.technique.home_open());
    }

    #[test]
    fn keyboard_from_home_returns_home_and_failures_stay_in_home() {
        let mut s = home_session();
        s.action(Action::Select);
        assert_eq!(choose_in(&mut s, 0, 2), Some(Request::OpenKeyboard));
        assert!(!s.technique.home_open());
        s.technique.execution_succeeded();
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::Keyboard)
        );
        s.technique.keyboard_closed();
        assert!(s.technique.home_open());
        assert!(!s.technique.return_to_home);

        assert_eq!(choose_in(&mut s, 0, 2), Some(Request::OpenKeyboard));
        s.execution_failed("Keyboard unavailable.".into());
        assert_eq!(s.technique.menu.kind, Kind::Home);
        assert!(s.technique.error.is_some());
        assert!(!s.technique.return_to_home);

        // A failed Home command is reported in Home too.
        s.action(Action::Select);
        choose_in(&mut s, 1, 0);
        choose_in(&mut s, 0, 0);
        s.execution_failed("Action is unavailable on this platform.".into());
        assert_eq!(s.technique.menu.kind, Kind::Home);
    }

    #[test]
    fn leaving_home_that_fails_returns_to_home_not_to_an_unchosen_point() {
        for (column, request) in [(0, Request::OpenPoint), (1, Request::OpenSwitchify)] {
            let mut s = home_session();
            s.action(Action::Select);
            let row = if request == Request::OpenPoint { 0 } else { 2 };
            assert_eq!(choose_in(&mut s, row, column), Some(request));
            s.execution_failed("Input could not be released.".into());
            assert_eq!(s.technique.menu.kind, Kind::Home, "{request:?}");
            assert!(s.technique.home_open());
            assert!(s.active());
        }
        // Once Point has opened, a failed click offers the action menu at its point as before.
        let mut s = home_session();
        s.action(Action::Select);
        choose_in(&mut s, 0, 0);
        s.technique.execution_succeeded();
        s.action(Action::Select);
        s.action(Action::Select);
        choose_in(&mut s, 0, 0);
        s.execution_failed("Click failed.".into());
        assert_eq!(s.technique.menu.kind, Kind::Actions);
        assert!(!s.technique.home_open());
    }

    #[test]
    fn home_opens_from_the_point_and_mouse_action_menus() {
        let mut s = home_session();
        s.action(Action::Select);
        choose_in(&mut s, 0, 0);
        s.action(Action::Select);
        s.action(Action::Select);
        assert_eq!(choose_in(&mut s, 2, 2), None);
        assert!(s.technique.home_open());
        assert_eq!(s.technique.source, (0, 0));
        assert!(s.technique.follows_foreground());

        assert_eq!(choose_in(&mut s, 0, 1), Some(Request::OpenMouse));
        s.technique.set_pointer(Some((100.0, 200.0)));
        s.technique.mouse_key(crate::scan_mouse::Key::Actions);
        assert_eq!(s.technique.menu.kind, Kind::MouseActions);
        assert_eq!(choose_in(&mut s, 2, 2), None);
        assert!(s.technique.home_open());
        assert!(!s.technique.mouse_actions_open);
        assert_eq!(s.technique.mouse_feedback(), None);
        assert_eq!(
            s.technique.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
    }

    #[test]
    fn radar_moves_at_the_radar_speed_or_else_the_line_speed() {
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        // Very slow is 45 logical pixels a second; very fast is 270.
        for (radar_speed, reach) in [(None, 45), (Some(4), 270), (Some(0), 45)] {
            let config = Config {
                speed: 0,
                radar_speed,
                ..last_mode()
            };
            let mut workflow = Workflow::new(config.point(), screen, 1.0).unwrap();
            workflow.handle(Action::OpenMouse);
            workflow.set_pointer(Some((640.0, 360.0)));
            workflow.mouse_key(Key::Radar);
            // The whole display, then straight up from its centre.
            for _ in 0..4 {
                workflow.handle(Action::Next);
            }
            workflow.handle(Action::Select);
            workflow.handle(Action::Select);
            workflow.advance(1000);
            assert_eq!(
                workflow.radar.as_ref().unwrap().target(),
                (640, 360 - reach),
                "{radar_speed:?}"
            );
        }
    }

    #[test]
    fn radar_sweeps_from_the_chosen_area_and_returns_to_the_mouse_panel() {
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.handle(Action::OpenMouse);
        // Without a pointer the display it is on may not be known.
        assert_eq!(workflow.mouse_key(Key::Radar), None);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(
            workflow
                .mouse
                .frame(screen, 1.0, Default::default(), None)
                .tiles
                .last()
                .unwrap()
                .text,
            "Pointer unavailable · Try again"
        );

        workflow.set_pointer(Some((640.0, 360.0)));
        assert_eq!(workflow.mouse_key(Key::Radar), None);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::MouseRadar));
        assert!(workflow.mouse_open());
        // Drawn in the mouse panel's colour until the cursor colour is known.
        assert_eq!(
            workflow.frame().color,
            workflow.point.config.mouse_scan.color
        );
        workflow.set_cursor_color(crate::scanning::ScannerColor::Yellow);
        let frame = workflow.frame();
        assert_eq!(frame.color, crate::scanning::ScannerColor::Yellow);
        assert!(frame.tiles.is_empty(), "the panel is hidden");
        assert_eq!(frame.label.unwrap().text, "Radar · Select an area");
        assert_eq!(
            frame.fills,
            [Rect {
                width: 640.0,
                height: 360.0,
                ..screen
            }],
            "the top left quarter is highlighted first"
        );
        // A pointer moving the panel out of its way does not move the radar.
        workflow.set_pointer(Some((10.0, 10.0)));

        // The bottom right quarter, around (960, 540).
        for _ in 0..3 {
            assert_eq!(workflow.handle(Action::Next), None);
        }
        // The pointer goes to its centre and stays there while the beam turns.
        assert_eq!(
            workflow.handle(Action::Select),
            Some(Request::MouseMoveAbsolute { x: 960, y: 540 })
        );
        assert_eq!(
            workflow.frame().label.unwrap().text,
            "Radar · Select a direction"
        );
        // Straight up, then four manual steps out, each taking the pointer along.
        assert_eq!(workflow.handle(Action::Select), None);
        for y in [520, 500, 480, 460] {
            assert_eq!(
                workflow.handle(Action::Next),
                Some(Request::MouseMoveAbsolute { x: 960, y })
            );
        }
        assert_eq!(
            workflow.handle(Action::Select),
            Some(Request::MouseMoveAbsolute { x: 960, y: 460 })
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(workflow.radar.is_none());

        // A change of display ends the radar without moving the pointer.
        workflow.mouse_key(Key::Radar);
        workflow.set_mouse_area(
            screen,
            Rect {
                x: 1280.0,
                ..screen
            },
            1.0,
            2,
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(workflow.radar.is_none());

        // A held drag keeps its cue. Open Mouse goes back to choosing an area,
        // then to the panel, without moving the pointer.
        // (The runtime rebuilds the engine for Open Mouse, which also releases the drag.)
        workflow.mouse.dragging = true;
        workflow.mouse_key(Key::Radar);
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Drag)
        );
        assert!(!workflow.steps_back(Action::OpenMouse));
        workflow.handle(Action::Next);
        workflow.handle(Action::Select);
        workflow.handle(Action::Select);
        assert_eq!(
            workflow.frame().label.unwrap().text,
            "Radar · Select a distance"
        );
        // The runtime keeps this scan for Open Mouse instead of starting afresh.
        assert!(workflow.steps_back(Action::OpenMouse));
        assert!(!workflow.steps_back(Action::OpenKeyboard));
        assert!(!workflow.steps_back(Action::OpenPoint));
        assert_eq!(workflow.handle(Action::OpenMouse), None);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::MouseRadar));
        let frame = workflow.frame();
        assert_eq!(frame.label.unwrap().text, "Radar · Select an area");
        // On the display the pointer moved to, from the top left quarter again.
        assert_eq!(
            (frame.fills[0].x, frame.fills[0].width),
            (1280.0, 640.0),
            "areas are highlighted from the start"
        );
        assert_eq!(workflow.handle(Action::OpenMouse), None);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(workflow.radar.is_none());
    }

    #[test]
    fn radar_divides_a_scaled_display_left_of_the_main_one() {
        use crate::scan_mouse::Key;
        let main = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let left = Rect {
            x: -2561.0,
            y: -301.0,
            width: 2561.0,
            height: 1441.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), main, 1.0).unwrap();
        workflow.handle(Action::OpenMouse);
        workflow.set_mouse_area(left, left, 1.5, 2);
        workflow.set_pointer(Some((-100.0, 1000.0)));
        workflow.mouse_key(Key::Radar);
        let frame = workflow.frame();
        assert_eq!(
            frame.fills,
            [Rect {
                width: 1280.0,
                height: 720.0,
                ..left
            }]
        );
        assert!(frame
            .grid
            .iter()
            .all(|rect| rect.x >= left.x && rect.x + rect.width <= left.x + left.width));

        // The bottom left quarter is -2561..-1282 by 419..1139, around (-1921, 779.5).
        workflow.handle(Action::Next);
        workflow.handle(Action::Next);
        workflow.handle(Action::Select);
        // Due left, in steps of 20 logical pixels (30 units). The quarter's
        // edge is 640 units away, so the 22nd step wraps back to the centre.
        for _ in 0..18 {
            workflow.handle(Action::Back);
        }
        workflow.handle(Action::Select);
        for _ in 0..30 {
            workflow.handle(Action::Next);
        }
        let Some(Request::MouseMoveAbsolute { x, y }) = workflow.handle(Action::Select) else {
            panic!("the radar chooses a point");
        };
        assert_eq!(x, -1921 - 8 * 30);
        assert!((419..=1139).contains(&y));
    }

    #[test]
    fn stepping_back_from_a_paused_direction_resumes_picking_an_area() {
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut s = Session::new(
            Workflow::new(last_mode().point(), screen, 1.0).unwrap(),
            false,
        );
        s.action(Action::Select);
        s.technique.handle(Action::OpenMouse);
        s.technique.set_pointer(Some((640.0, 360.0)));
        s.technique.mouse_key(Key::Radar);
        s.action(Action::Select);
        s.action(Action::Pause);
        assert!(s.paused());
        assert_eq!(s.action(Action::OpenMouse), None);
        assert!(!s.paused());
        assert!(s.active());
        let first = s.frame().fills;
        assert_eq!(first[0].x, 0.0, "the top left quarter is highlighted");
        for _ in 0..10 {
            s.tick(100, false);
        }
        let next = s.frame().fills;
        assert_eq!(
            (next[0].x, next[0].y),
            (640.0, 0.0),
            "the highlight moves on to the top right quarter"
        );
    }

    #[test]
    fn radar_returns_to_the_panel_after_unattended_passes() {
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut s = Session::new(
            Workflow::new(last_mode().point(), screen, 1.0).unwrap(),
            false,
        );
        s.action(Action::Select);
        s.technique.handle(Action::OpenMouse);
        s.technique.set_pointer(Some((640.0, 360.0)));
        s.technique.mouse_key(Key::Radar);
        s.action(Action::Select);
        // An unattended direction goes back to choosing an area, and that back to the panel.
        let mut phases = vec![];
        for _ in 0..100_000 {
            let Some(radar) = s.technique.radar.as_ref() else {
                break;
            };
            if phases.last() != Some(&radar.phase) {
                phases.push(radar.phase);
            }
            s.tick(100, false);
            assert_eq!(s.take_selection(), None);
        }
        use crate::scan_radar::Phase::{Angle, Area};
        assert_eq!(phases, [Angle, Area]);
        assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(s.active());
    }

    #[test]
    fn mouse_actions_use_the_current_pointer_and_return_to_mouse() {
        use crate::scan_menu::Command;
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.handle(Action::OpenMouse);
        workflow.set_pointer(Some((321.4, 246.6)));
        assert_eq!(workflow.mouse_key(Key::Actions), None);
        assert_eq!(workflow.menu.kind, Kind::MouseActions);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Menu));
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Move)
        );
        assert_eq!(
            workflow.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
        workflow.selected(Item::Group(Kind::Mouse));
        workflow.set_pointer(Some((400.0, 500.0)));
        assert_eq!(
            workflow.selected(Item::Command(Command::ShiftClick)),
            Some(Request::Command {
                command: Command::ShiftClick,
                point: (400, 500),
            })
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Move)
        );
        assert_eq!(
            workflow.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
        assert!(workflow.parent_menu.is_empty());

        workflow.mouse_key(Key::Actions);
        workflow.selected(Item::Group(Kind::Media));
        assert_eq!(
            workflow.selected(Item::Command(Command::Mute)),
            Some(Request::Command {
                command: Command::Mute,
                point: (400, 500),
            })
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Menu));
        workflow.selected(Item::Back);
        assert_eq!(workflow.menu.kind, Kind::MouseActions);
        workflow.selected(Item::Back);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));

        workflow.mouse_key(Key::Actions);
        workflow.selected(Item::Group(Kind::Displays));
        assert_eq!(
            workflow.selected(Item::Display(true)),
            Some(Request::MouseCycleDisplay(true))
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
    }

    #[test]
    fn mouse_actions_block_active_drag_and_return_from_keyboard_or_failure() {
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.handle(Action::OpenMouse);
        workflow.set_pointer(Some((100.0, 200.0)));
        assert_eq!(workflow.mouse_key(Key::Drag), Some(Request::MouseDrag));
        assert_eq!(workflow.mouse_key(Key::Actions), None);
        assert!(workflow.mouse.dragging);
        assert!(!workflow.mouse_actions_open);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(workflow.mouse_key(Key::Drag), Some(Request::MouseDrag));
        workflow.mouse_key(Key::Actions);
        assert_eq!(
            workflow.selected(Item::Keyboard),
            Some(Request::OpenKeyboard)
        );
        workflow.execution_succeeded();
        workflow.keyboard_closed();
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Move)
        );
        workflow.mouse_key(Key::Actions);
        workflow.execution_failed("failed".into());
        assert_eq!(
            workflow.phase(),
            Phase::Workflow(WorkflowPhase::MouseSuspended)
        );
        assert!(!workflow.mouse_actions_open);
        assert_eq!(workflow.mouse_feedback(), None);
    }

    #[test]
    fn mouse_actions_stop_and_unavailable_pointer_never_reuse_an_old_target() {
        use crate::scan_menu::Command;
        use crate::scan_mouse::Key;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut session = Session::new(
            Workflow::new(last_mode().point(), screen, 1.0).unwrap(),
            true,
        );
        session.action(Action::OpenMouse);
        session.technique.set_pointer(Some((40.0, 50.0)));
        session.technique.mouse_key(Key::Actions);
        session.technique.set_pointer(None);
        assert_eq!(
            session
                .technique
                .selected(Item::Command(Command::MiddleClick)),
            None
        );
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::Menu)
        );
        session.action(Action::Stop);
        assert!(!session.active());
        assert_eq!(session.technique.mouse_feedback(), None);
        assert!(!session.technique.mouse_actions_open);
        assert_eq!(
            session.technique.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
    }

    #[test]
    fn select_resumes_saved_mode_and_switching_updates_it() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let config = Config {
            control_mode: crate::point_scan::ControlMode::Mouse,
            ..last_mode()
        };
        let mut session = Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), true);
        assert_eq!(session.action(Action::Select), None);
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::Mouse)
        );
        assert_eq!(
            session.technique.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
        assert_eq!(session.action(Action::OpenPoint), Some(Request::OpenPoint));
        assert_eq!(
            session.technique.control_mode(),
            crate::point_scan::ControlMode::Point
        );
        assert!(matches!(session.technique.phase(), Phase::Point(_)));
        assert_eq!(session.action(Action::OpenMouse), Some(Request::OpenMouse));
        assert_eq!(
            session.technique.mouse_key(crate::scan_mouse::Key::Close),
            Some(Request::OpenPoint)
        );
        assert_eq!(
            session.technique.control_mode(),
            crate::point_scan::ControlMode::Point
        );
        assert_eq!(session.action(Action::Stop), None);
        assert_eq!(session.action(Action::Select), None);
        assert!(matches!(session.technique.phase(), Phase::Point(_)));
    }

    #[test]
    fn stopped_mouse_motion_waits_at_the_arrow_where_the_user_chose_to() {
        use crate::scan_preferences::{NextScan, StartFrom};
        let mut config = last_mode();
        config.scan_preferences.mouse.next_scan = Some(NextScan::Wait);
        config.scan_preferences.mouse.start_from = Some(StartFrom::Selection);
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut session = Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), true);
        let status = |session: &Session<Workflow>| {
            let mut frame = session.technique.frame();
            frame.tiles.pop().unwrap().text
        };
        session.action(Action::OpenMouse);
        assert_eq!(status(&session), "Mouse waiting · Select to continue");
        for action in [
            Action::Select,
            Action::Next,
            Action::Next,
            Action::Select,
            Action::Next,
            Action::Next,
        ] {
            assert_eq!(session.action(action), None);
        }
        assert!(matches!(
            session.action(Action::Select),
            Some(Request::MouseMove { dy: 0, .. })
        ));
        assert!(session.technique.stop_mouse_repeat());
        assert_eq!(status(&session), "Mouse waiting · Select to continue");
        session.tick(5000, false);
        assert_eq!(session.action(Action::Select), None);
        assert_eq!(status(&session), "Movement · Select →");
        // A move that fails starts again from the top.
        assert!(session.action(Action::Select).is_some());
        session.technique.stop_mouse_repeat();
        session.action(Action::Select);
        session.technique.mouse_repeat_enabled = false;
        assert!(session.action(Action::Select).is_some());
        session.technique.execution_failed(String::new());
        assert_eq!(status(&session), "Mouse action failed · Select to resume");
        session.action(Action::Select);
        assert_eq!(status(&session), "Movement · Select a row");
    }
    #[test]
    fn panels_wait_on_opening_where_the_user_chose_to() {
        use crate::scan_preferences::NextScan;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        for next_scan in [NextScan::Standard, NextScan::Automatic, NextScan::Wait] {
            for automatic in [false, true] {
                let waits = automatic && next_scan == NextScan::Wait;
                let mut config = Config {
                    automatic,
                    control_mode: crate::point_scan::ControlMode::Mouse,
                    ..last_mode()
                };
                config.scan_preferences.mouse.next_scan = Some(next_scan);
                config.scan_preferences.keyboard.next_scan = Some(next_scan);
                let mut s = Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), true);
                // The Select that starts scanning is not waited after.
                s.action(Action::Select);
                assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Mouse));
                let mouse = Phase::Workflow(if waits {
                    WorkflowPhase::MouseSuspended
                } else {
                    WorkflowPhase::Mouse
                });
                let keyboard = Phase::Workflow(if waits {
                    WorkflowPhase::KeyboardSuspended
                } else {
                    WorkflowPhase::Keyboard
                });
                assert_eq!(
                    s.technique
                        .choose_mouse_key(crate::scan_mouse::Key::Keyboard),
                    Some(Request::OpenKeyboard)
                );
                s.technique.execution_succeeded();
                assert_eq!(s.technique.phase(), keyboard, "{next_scan:?} {automatic}");
                // Suggestions turning on, then failing, do not end the wait.
                let prediction = s.technique.prediction_keyboard().unwrap();
                prediction.predictions(None, true);
                s.tick(5000, false);
                assert_eq!(s.technique.phase(), keyboard);
                if waits {
                    let mut frame = s.technique.frame();
                    assert_eq!(
                        frame.tiles.pop().unwrap().text,
                        "Keyboard waiting · Select to start"
                    );
                    // Select starts the scan without typing.
                    assert_eq!(s.action(Action::Select), None);
                    assert_eq!(
                        s.technique.phase(),
                        Phase::Workflow(WorkflowPhase::Keyboard)
                    );
                }
                // Closing the keyboard returns to the mouse panel.
                assert_eq!(
                    s.technique.choose_key(crate::scan_keyboard::Key::Close),
                    None
                );
                assert_eq!(s.technique.phase(), mouse);
                if waits {
                    assert_eq!(s.action(Action::Next), None);
                    assert_eq!(s.technique.phase(), mouse);
                    assert_eq!(s.action(Action::Select), None);
                    assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Mouse));
                }
                // So does leaving the mouse actions menu.
                s.technique.pointer = Some((10.0, 10.0));
                assert_eq!(
                    s.technique
                        .choose_mouse_key(crate::scan_mouse::Key::Actions),
                    None
                );
                s.technique.selected(Item::Back);
                assert_eq!(s.technique.phase(), mouse);
                // And leaving the radar without choosing a target.
                s.technique.mouse.restart();
                s.technique.choose_mouse_key(crate::scan_mouse::Key::Radar);
                assert_eq!(
                    s.technique.phase(),
                    Phase::Workflow(WorkflowPhase::MouseRadar)
                );
                s.action(Action::OpenMouse);
                assert_eq!(s.technique.phase(), mouse);
                // And a switch that opens the mouse panel from point scanning.
                s.action(Action::OpenPoint);
                s.action(Action::OpenMouse);
                assert_eq!(s.technique.phase(), mouse);
            }
        }
    }
    #[test]
    fn the_next_point_scan_starts_by_itself_where_the_user_chose_to() {
        use crate::scan_preferences::NextScan;
        for next_scan in [NextScan::Standard, NextScan::Wait, NextScan::Automatic] {
            for auto_select in [false, true] {
                let mut config = Config {
                    block_interval_ms: 250,
                    auto_select_enabled: auto_select,
                    auto_select_delay_ms: 500,
                    ..last_mode()
                };
                config.scan_preferences.point.next_scan = Some(next_scan);
                // The menu's own choice does not decide this.
                config.scan_preferences.menu.next_scan = Some(NextScan::Automatic);
                let screen = Rect {
                    x: -1000.0,
                    y: 20.0,
                    width: 1000.0,
                    height: 800.0,
                };
                let mut s = Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), true);
                for _ in 0..3 {
                    assert_eq!(s.action(Action::Select), None);
                }
                let click = if auto_select {
                    (0..50).find_map(|_| {
                        s.tick(33, false);
                        s.take_selection()
                    })
                } else {
                    choose(&mut s, 0, 0)
                };
                assert!(
                    matches!(click, Some(Request::Click { .. })),
                    "{next_scan:?} {click:?}"
                );
                let again = next_scan == NextScan::Automatic;
                assert_eq!(s.active(), again, "{next_scan:?}");
                assert_eq!(s.frame() != Frame::default(), again, "{next_scan:?}");
                assert_eq!(s.technique.follows_foreground(), again);
                if again {
                    // Once something is chosen, the window is held to.
                    s.tick(33, false);
                    assert!(s.technique.follows_foreground());
                    s.action(Action::Select);
                    assert!(!s.technique.follows_foreground());
                    s.action(Action::Cancel);
                    // A scan begun with Select follows until a switch is used in it.
                    s.action(Action::Select);
                    assert!(s.technique.follows_foreground());
                    s.action(Action::Next);
                    assert!(!s.technique.follows_foreground());
                    s.action(Action::Cancel);
                    s.action(Action::Select);
                    assert_eq!(
                        s.technique.phase(),
                        Phase::Point(crate::point_scan::Phase::X)
                    );
                    // A click that failed is still reported.
                    s.technique.execution_failed("failed".into());
                    assert_eq!(
                        s.technique.phase(),
                        Phase::Workflow(WorkflowPhase::MenuSuspended)
                    );
                    s.action(Action::Select);
                    s.technique.selected(Item::NewPoint);
                    // And the pass limit still stops it.
                    for _ in 0..400 {
                        s.tick(250, false);
                    }
                    assert_eq!(s.frame(), Frame::default());
                }
            }
        }
    }
    #[test]
    fn a_drag_or_a_closing_command_is_followed_by_the_next_point_scan() {
        use crate::scan_preferences::NextScan;
        for next_scan in [NextScan::Standard, NextScan::Automatic] {
            let again = next_scan == NextScan::Automatic;
            let mut config = Config {
                automatic: false,
                block_interval_ms: 250,
                ..last_mode()
            };
            config.scan_preferences.point.next_scan = Some(next_scan);
            let screen = Rect {
                x: -1000.0,
                y: 20.0,
                width: 1000.0,
                height: 800.0,
            };
            let new = || Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), false);
            let mut s = new();
            open(&mut s);
            choose(&mut s, 1, 1);
            for action in [Action::Next, Action::Select, Action::Next, Action::Select] {
                s.action(action);
            }
            assert!(matches!(choose(&mut s, 0, 0), Some(Request::DragStart(_))));
            s.tick(150, false);
            assert!(matches!(s.take_selection(), Some(Request::DragMove(_))));
            s.tick(150, false);
            assert!(matches!(s.take_selection(), Some(Request::DragEnd(_))));
            assert_eq!(s.active(), again);
            assert_eq!(s.technique.follows_foreground(), again);
            if again {
                assert_eq!(
                    s.technique.phase(),
                    Phase::Point(crate::point_scan::Phase::X)
                );
                // The menus of the drag are not returned to.
                assert!(s.technique.parent_menu.is_empty());
            }
            let mut s = new();
            open(&mut s);
            let copy = Item::Command(crate::scan_menu::Command::Copy);
            assert!(matches!(
                s.technique.selected(copy),
                Some(Request::Command { .. })
            ));
            s.take_selection();
            assert_eq!(s.active(), again);
            // A command that leaves the menu open starts nothing.
            let mut s = new();
            open(&mut s);
            let mute = Item::Command(crate::scan_menu::Command::Mute);
            let before = s.frame();
            assert!(s.technique.selected(mute).is_some());
            assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Menu));
            assert_eq!(s.frame(), before);
        }
    }
    #[test]
    fn scrolling_continues_in_the_menu_as_the_user_chose() {
        use crate::scan_preferences::{NextScan, StartFrom};
        for (next_scan, start_from) in [
            (NextScan::Standard, StartFrom::Standard),
            (NextScan::Wait, StartFrom::Selection),
            (NextScan::Wait, StartFrom::Beginning),
            (NextScan::Automatic, StartFrom::Beginning),
        ] {
            let mut config = Config {
                automatic: true,
                block_interval_ms: 250,
                ..last_mode()
            };
            config.scan_preferences.menu.next_scan = Some(next_scan);
            config.scan_preferences.menu.start_from = Some(start_from);
            let screen = Rect {
                x: -1000.0,
                y: 20.0,
                width: 1000.0,
                height: 800.0,
            };
            let mut s = Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), true);
            open(&mut s);
            choose(&mut s, 1, 0);
            assert!(matches!(choose(&mut s, 0, 1), Some(Request::Scroll { .. })));
            let waits = next_scan == NextScan::Wait;
            assert_eq!(
                s.technique.phase(),
                Phase::Workflow(if waits {
                    WorkflowPhase::MenuSuspended
                } else {
                    WorkflowPhase::Menu
                })
            );
            if waits {
                s.tick(5000, false);
                assert_eq!(s.action(Action::Select), None);
            }
            assert_eq!(
                matches!(s.action(Action::Select), Some(Request::Scroll { .. })),
                start_from != StartFrom::Beginning,
                "{start_from:?}"
            );
        }
    }
    #[test]
    fn mouse_motion_repeats_while_stop_switch_is_held_and_returns_to_first_row_on_release() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut session = Session::new(
            Workflow::new(last_mode().point(), screen, 1.0).unwrap(),
            true,
        );
        assert_eq!(session.action(Action::OpenMouse), Some(Request::OpenMouse));
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::Mouse)
        );
        assert_eq!(
            session
                .technique
                .mouse_key(crate::scan_mouse::Key::Move(1, 0)),
            Some(Request::MouseMove { dx: 1, dy: 0 })
        );
        let mut moved = false;
        for _ in 0..20 {
            session.tick(33, false);
            if matches!(session.take_selection(), Some(Request::MouseMove { dx, dy: 0 }) if dx > 0)
            {
                moved = true;
                break;
            }
        }
        assert!(moved);
        let frame = session.technique.frame();
        assert!(frame.tiles.iter().all(|tile| !tile.selected));
        assert_eq!(
            frame.tiles.last().unwrap().text,
            "Moving pointer · Press and release a switch to stop"
        );
        session.tick(1000, true);
        assert!(
            matches!(session.take_selection(), Some(Request::MouseMove { dx, dy: 0 }) if dx > 0)
        );
        assert!(session.technique.mouse_repeating());
        assert!(session.technique.stop_mouse_repeat());
        assert!(!session.technique.stop_mouse_repeat());
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::Mouse)
        );
        let selected: Vec<_> = session
            .technique
            .mouse
            .frame(screen, 1.0, Default::default(), None)
            .tiles
            .into_iter()
            .filter(|tile| tile.selected)
            .map(|tile| tile.text)
            .collect();
        assert_eq!(
            selected,
            ["Left click", "Right click", "Double click", "Start drag"]
        );
    }

    #[test]
    fn mouse_without_repeat_moves_once_and_keeps_scanning() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.open_mouse(true);
        workflow.set_mouse_settings(50, 1000, 250, 250, false);
        assert_eq!(
            workflow.mouse_key(crate::scan_mouse::Key::Move(-1, 1)),
            Some(Request::MouseMove { dx: -6, dy: 6 })
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Move)
        );
        workflow.update(
            1000,
            UpdateContext {
                movement_enabled: true,
                paused: false,
                switch_held: false,
            },
        );
        assert!(workflow.take_selection().is_none());
        assert!(!workflow.stop_mouse_repeat());
    }

    #[test]
    fn mouse_one_step_uses_the_shared_direction_distance_at_saved_speeds() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        for (speed, distance) in [(5, 1), (100, 12), (225, 27), (1350, 162)] {
            for (x, y) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
                workflow.open_mouse(true);
                workflow.set_mouse_settings(speed, 1000, 250, 250, false);
                assert_eq!(
                    workflow.mouse_key(crate::scan_mouse::Key::Move(x, y)),
                    Some(Request::MouseMove {
                        dx: i32::from(x) * distance,
                        dy: i32::from(y) * distance,
                    })
                );
                assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
            }
        }
    }

    #[test]
    fn scanned_scroll_repeats_while_held_and_stops_on_release() {
        let mut workflow = session(false).technique;
        workflow.open_mouse(true);
        workflow.set_mouse_settings(100, 1000, 250, 120, true);
        for (direction, dy) in [(1, 5), (-1, -5)] {
            assert_eq!(
                workflow.mouse_key(crate::scan_mouse::Key::Scroll(direction)),
                Some(Request::MouseScroll { dy })
            );
            assert_eq!(
                workflow.phase(),
                Phase::Workflow(WorkflowPhase::MouseScrolling)
            );
            assert_eq!(
                workflow.mouse_feedback(),
                Some(crate::input::PointerFeedback::RepeatScroll { dx: 0, dy })
            );
            workflow.update(
                119,
                UpdateContext {
                    movement_enabled: true,
                    paused: false,
                    switch_held: false,
                },
            );
            assert_eq!(workflow.take_selection(), None);
            workflow.update(
                1,
                UpdateContext {
                    movement_enabled: true,
                    paused: false,
                    switch_held: false,
                },
            );
            assert_eq!(workflow.take_selection(), Some(Request::MouseScroll { dy }));
            workflow.update(
                370,
                UpdateContext {
                    movement_enabled: true,
                    paused: false,
                    switch_held: true,
                },
            );
            assert_eq!(workflow.take_selection(), Some(Request::MouseScroll { dy }));
            assert!(workflow.mouse_repeating());
            assert!(workflow.stop_mouse_repeat());
            assert!(!workflow.stop_mouse_repeat());
            assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
            workflow.update(
                120,
                UpdateContext {
                    movement_enabled: true,
                    paused: false,
                    switch_held: false,
                },
            );
            assert_eq!(workflow.take_selection(), None);
        }
    }

    #[test]
    fn scanned_scroll_single_step_and_cleanup_paths() {
        let mut workflow = session(false).technique;
        workflow.open_mouse(true);
        workflow.set_mouse_settings(100, 1000, 250, 80, false);
        assert_eq!(
            workflow.mouse_key(crate::scan_mouse::Key::Scroll(1)),
            Some(Request::MouseScroll { dy: 5 })
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(!workflow.stop_mouse_repeat());
        workflow.set_mouse_settings(100, 1000, 250, 80, true);
        workflow.mouse_key(crate::scan_mouse::Key::Scroll(-1));
        workflow.execution_failed("input failed".into());
        assert_eq!(workflow.mouse_feedback(), None);
        assert_eq!(
            workflow.scroll_repeat.as_ref().map(|repeat| repeat.dy()),
            None
        );
        workflow.handle(Action::Select);
        workflow.mouse_key(crate::scan_mouse::Key::Scroll(1));
        workflow.set_mouse_settings(100, 1000, 250, 80, false);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(!workflow.stop_mouse_repeat());
        workflow.set_mouse_settings(100, 1000, 250, 80, true);
        workflow.mouse_key(crate::scan_mouse::Key::Scroll(1));
        let next_screen = Rect {
            x: 1000.0,
            y: 20.0,
            width: 1000.0,
            height: 800.0,
        };
        workflow.set_mouse_area(next_screen, next_screen, 1.0, 2);
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(
            workflow.scroll_repeat.as_ref().map(|repeat| repeat.dy()),
            None
        );
        workflow.mouse_key(crate::scan_mouse::Key::Scroll(1));
        workflow.reset();
        assert_eq!(
            workflow.scroll_repeat.as_ref().map(|repeat| repeat.dy()),
            None
        );
        assert_eq!(workflow.mouse_feedback(), None);
    }

    #[test]
    fn changing_mouse_settings_or_display_ends_a_held_move_repeat() {
        use crate::scan_mouse::Key;
        let mut workflow = session(false).technique;
        workflow.open_mouse(true);
        workflow.set_mouse_settings(100, 1000, 250, 80, true);
        assert!(matches!(
            workflow.mouse_key(Key::Move(1, 0)),
            Some(Request::MouseMove { .. })
        ));
        assert!(workflow.mouse_repeating());
        workflow.set_mouse_settings(105, 1000, 250, 80, true);
        assert!(!workflow.mouse_repeating());
        assert!(!workflow.stop_mouse_repeat());

        workflow.mouse_key(Key::Move(1, 0));
        let next_screen = Rect {
            x: 1000.0,
            y: 20.0,
            width: 1000.0,
            height: 800.0,
        };
        workflow.set_mouse_area(next_screen, next_screen, 1.0, 2);
        assert!(!workflow.mouse_repeating());
        assert!(workflow.move_repeat.is_none());
        assert!(!workflow.stop_mouse_repeat());
    }

    #[test]
    fn panel_position_is_shared_and_survives_rebuilding_panels() {
        use crate::{scan_mouse::Key, scan_panel::Dock};
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.open_mouse(true);
        let dock = Dock { column: 2, row: 1 };
        assert_eq!(workflow.mouse_key(Key::Dock), None);
        assert_eq!(workflow.mouse_key(Key::Position(dock)), None);
        assert_eq!(workflow.mouse.dock, dock);
        assert_eq!(
            workflow.mouse_key(Key::Keyboard),
            Some(Request::OpenKeyboard)
        );
        assert_eq!(workflow.keyboard.dock, dock);
        workflow.execution_succeeded();
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Keyboard));
        let dock = Dock { column: 0, row: 0 };
        workflow.keyboard.dock = dock;
        workflow.handle(Action::Next);
        workflow.keyboard_closed();
        assert_eq!(workflow.mouse.dock, dock, "keyboard choices move the mouse");
        workflow.reset();
        workflow.open_mouse(true);
        assert_eq!((workflow.keyboard.dock, workflow.mouse.dock), (dock, dock));
    }

    #[test]
    fn a_saved_panel_position_is_restored_and_only_chosen_positions_are_saved() {
        use crate::{scan_mouse::Key, scan_panel::Dock};
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let config = Config {
            panel_avoids_pointer: true,
            ..last_mode()
        };
        let saved = Dock { column: 1, row: 0 };
        let mut workflow = Workflow::new(config.point(), screen, 1.0)
            .unwrap()
            .with_dock(saved);
        assert_eq!(workflow.unsaved_dock(), None, "a loaded dock is saved");
        workflow.open_mouse(true);
        assert_eq!(
            (workflow.keyboard.dock, workflow.mouse.dock),
            (saved, saved)
        );
        let panel = workflow.frame().tiles[0].rect;
        assert_eq!(panel, saved.rect(screen, (panel.width, panel.height)));

        // Moving out of the pointer's way is not a choice.
        workflow.set_pointer(Some((640.0, 20.0)));
        assert!(workflow.moved.is_some());
        assert_eq!(workflow.unsaved_dock(), None);

        let chosen = Dock { column: 2, row: 2 };
        assert_eq!(workflow.mouse_key(Key::Dock), None);
        assert_eq!(workflow.mouse_key(Key::Position(chosen)), None);
        assert_eq!(workflow.unsaved_dock(), Some(chosen));
        // A failed save leaves it to retry after the next action.
        assert_eq!(workflow.unsaved_dock(), Some(chosen));
        workflow.dock_saved(chosen);
        assert_eq!(workflow.unsaved_dock(), None, "saved once");

        // The next scan keeps the chosen dock.
        let mut next = Workflow::new(config.point(), screen, 1.0)
            .unwrap()
            .with_dock(Dock::default());
        next.resume_from(&workflow);
        next.open_mouse(true);
        assert_eq!(next.mouse.dock, chosen);
        assert_eq!(next.unsaved_dock(), None);
        // Home and the action menus open at the same dock, which they never change.
        next.open_home();
        let menu = next.frame().tiles[0].rect;
        assert_eq!(menu, chosen.rect(screen, (menu.width, menu.height)));
        next.set_pointer(Some((menu.x + 1.0, menu.y + 1.0)));
        assert_ne!(next.frame().tiles[0].rect, menu, "Home avoids the pointer");
        assert_eq!(next.unsaved_dock(), None);
    }

    #[test]
    fn size_changes_update_open_panels_without_resetting_the_scan() {
        use crate::scan_preferences::PanelSize;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let mut config = last_mode();
        let mut workflow = Workflow::new(config.point(), screen, 1.0).unwrap();
        workflow.open_mouse(true);
        workflow.handle(Action::Next);
        let selected = workflow
            .frame()
            .tiles
            .iter()
            .filter(|tile| tile.selected)
            .map(|tile| tile.text.clone())
            .collect::<Vec<_>>();
        config.scan_preferences.panel_size = PanelSize::Large;
        workflow.apply_config(config.point(), false);
        let frame = workflow.frame();
        assert!((frame.tiles[0].rect.width - 568.0 * 1.2).abs() < 1e-8);
        assert_eq!(
            selected,
            frame
                .tiles
                .iter()
                .filter(|tile| tile.selected)
                .map(|tile| tile.text.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
    }

    #[test]
    fn open_panels_move_out_of_the_pointers_way_only_when_enabled() {
        use crate::scan_panel::Dock;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let background = |workflow: &Workflow| workflow.frame().tiles[0].rect;
        let bottom = Dock::default().rect(screen, (568.0, 392.0));
        let top = Dock { column: 1, row: 0 }.rect(screen, (568.0, 392.0));
        let over_bottom = Some((640.0, 700.0));
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.open_mouse(true);
        workflow.set_pointer(over_bottom);
        assert_eq!(background(&workflow), bottom, "off by default");

        let config = Config {
            panel_avoids_pointer: true,
            ..last_mode()
        };
        workflow.apply_config(config.point(), false);
        assert!(workflow.panel_avoids_pointer());
        workflow.set_pointer(over_bottom);
        let frame = workflow.frame();
        assert_eq!(frame.tiles[0].rect, top);
        assert!(frame
            .tiles
            .iter()
            .all(|tile| tile.rect.y < top.y + top.height));
        assert_eq!(
            workflow.mouse.dock,
            Dock::default(),
            "the chosen dock is kept"
        );
        workflow.set_pointer(Some((640.0, 20.0)));
        assert_eq!(
            background(&workflow),
            bottom,
            "returns once the pointer leaves"
        );

        workflow.mouse_key(crate::scan_mouse::Key::Keyboard);
        workflow.execution_succeeded();
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Keyboard));
        let keyboard_bounds = background(&workflow);
        let size = (keyboard_bounds.width, keyboard_bounds.height);
        let top = Dock { column: 1, row: 0 }.rect(screen, size);
        let bottom = Dock::default().rect(screen, size);
        workflow.set_pointer(over_bottom);
        assert_eq!(background(&workflow), top, "the keyboard also moves");
        workflow.set_pointer(None);
        assert_eq!(background(&workflow), bottom);

        // A closed panel forgets where it moved, so a middle dock chooses afresh when reopened.
        let middle = Dock { column: 1, row: 1 };
        workflow.set_dock(middle);
        let area = middle.rect(screen, size);
        workflow.set_pointer(Some((640.0, area.y + 10.0)));
        assert_eq!(workflow.moved, Some(Dock::default()));
        workflow.keyboard_closed();
        workflow.open_point();
        workflow.set_pointer(Some((640.0, area.y + 10.0)));
        assert_eq!(workflow.moved, None);
    }
    #[test]
    fn menus_share_the_panel_dock_avoid_the_pointer_and_mark_their_points() {
        use crate::scan_panel::Dock;
        let screen = Rect {
            x: -1280.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let work = Rect {
            height: 680.0,
            ..screen
        };
        let config = Config {
            panel_avoids_pointer: true,
            ..last_mode()
        };
        let mut workflow = Workflow::new(config.point(), screen, 1.0).unwrap();
        workflow.set_keyboard_area(work);
        let top = Dock { column: 1, row: 0 };
        workflow.set_dock(top);
        workflow.source = (-640, 100);
        workflow.open(Kind::Actions);
        let frame = workflow.frame();
        let panel = frame.tiles[0].rect;
        let size = (panel.width, panel.height);
        assert_eq!(panel, top.rect(work, size), "docked in the work area");
        assert_eq!(
            frame.strips,
            crosshair((-640, 100), screen, 1.0),
            "the chosen point stays marked"
        );
        workflow.set_pointer(Some((panel.x + 1.0, panel.y + 1.0)));
        assert_eq!(
            workflow.frame().tiles[0].rect,
            Dock { column: 1, row: 2 }.rect(work, size),
            "the menu moves out of the pointer's way"
        );
        assert_eq!(workflow.dock, top, "the chosen dock is kept");
        workflow.set_pointer(None);
        assert_eq!(workflow.frame().tiles[0].rect, panel);

        // A docked menu is not beside the destination, so both ends of a drag are marked.
        workflow.destination = (-200, 600);
        workflow.open(Kind::ConfirmDrag);
        let mut ends = crosshair((-640, 100), screen, 1.0);
        ends.extend(crosshair((-200, 600), screen, 1.0));
        assert_eq!(workflow.frame().strips, ends);

        // A work area left on another display is passed over for this display.
        workflow.set_keyboard_area(Rect { x: 0.0, ..work });
        workflow.open(Kind::Home);
        let frame = workflow.frame();
        let panel = frame.tiles[0].rect;
        assert_eq!(panel, top.rect(screen, (panel.width, panel.height)));
        assert!(frame.strips.is_empty(), "Home acts at no point");
    }
    #[test]
    fn mouse_keyboard_return_and_drag_state() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(last_mode().point(), screen, 1.0).unwrap();
        workflow.open_mouse(true);
        assert_eq!(
            workflow.mouse_key(crate::scan_mouse::Key::Drag),
            Some(Request::MouseDrag)
        );
        assert!(workflow.mouse.dragging);
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Drag)
        );
        assert_eq!(
            workflow.mouse_key(crate::scan_mouse::Key::Keyboard),
            Some(Request::OpenKeyboard)
        );
        assert!(!workflow.mouse.dragging);
        assert_eq!(workflow.mouse_feedback(), None);
        workflow.execution_succeeded();
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Keyboard));
        assert!(workflow.return_to_mouse);
        workflow.keyboard_closed();
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert_eq!(
            workflow.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Move)
        );
        workflow.reset();
        assert_eq!(
            workflow.control_mode(),
            crate::point_scan::ControlMode::Mouse
        );
        workflow.start();
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
    }

    #[test]
    fn mouse_input_failure_clears_repeat_and_ring_until_resumed() {
        let mut workflow = session(false).technique;
        workflow.open_mouse(true);
        workflow.mouse_key(crate::scan_mouse::Key::Move(1, 0));
        assert!(matches!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::RepeatMove { .. })
        ));
        workflow.execution_failed("input failed".into());
        assert_eq!(workflow.mouse_feedback(), None);
        assert!(!workflow.stop_mouse_repeat());
        workflow.handle(Action::Select);
        assert_eq!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::Move)
        );
    }
    fn session(automatic: bool) -> Session<Workflow> {
        Session::new(
            Workflow::new(
                Config {
                    automatic,
                    block_interval_ms: 250,
                    ..last_mode()
                }
                .point(),
                Rect {
                    x: -1000.0,
                    y: 20.0,
                    width: 1000.0,
                    height: 800.0,
                },
                1.0,
            )
            .unwrap(),
            automatic,
        )
    }
    #[test]
    fn direct_keyboard_opens_from_every_workflow_and_cancels_pending_actions() {
        for stage in [
            Stage::Idle,
            Stage::Point,
            Stage::Menu,
            Stage::Countdown,
            Stage::Destination,
            Stage::Executing,
        ] {
            let mut s = session(false);
            s.action(Action::Select);
            s.action(Action::Pause);
            s.technique.stage = stage;
            s.technique.pending = Some(default_click((10, 20)));
            assert_eq!(s.action(Action::OpenKeyboard), Some(Request::OpenKeyboard));
            assert!(s.active());
            assert!(!s.paused());
            assert!(s.take_selection().is_none());
            s.technique.execution_succeeded();
            assert_eq!(
                s.technique.phase(),
                Phase::Workflow(WorkflowPhase::Keyboard)
            );
        }
        let mut idle = session(false);
        assert_eq!(
            idle.action(Action::OpenKeyboard),
            Some(Request::OpenKeyboard)
        );
        assert!(idle.active());
    }
    #[test]
    fn chosen_keyboard_layout_opens_from_point_and_mouse_and_survives_save_failure() {
        use crate::point_scan::{ControlMode, KeyboardLayout};
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        for mode in [ControlMode::Point, ControlMode::Mouse] {
            for layout in [KeyboardLayout::CommonLetters, KeyboardLayout::SimpleQwerty] {
                let config = Config {
                    control_mode: mode,
                    keyboard_layout: layout,
                    ..last_mode()
                };
                let mut workflow = Workflow::new(config.point(), screen, 1.0).unwrap();
                if mode == ControlMode::Mouse {
                    workflow.handle(Action::OpenMouse);
                }
                assert_eq!(
                    workflow.handle(Action::OpenKeyboard),
                    Some(Request::OpenKeyboard)
                );
                workflow.execution_succeeded();
                assert_eq!(workflow.keyboard.layout, layout);
                workflow.keyboard_layout_pending = true;
                workflow.execution_failed("Cannot save scanning settings.".into());
                assert_eq!(workflow.keyboard.layout, layout);
                assert!(!workflow.keyboard.suspended());
                let next = Config {
                    keyboard_layout: KeyboardLayout::Qwerty,
                    ..config
                };
                workflow.apply_config(next.point(), false);
                assert_eq!(workflow.keyboard.layout, KeyboardLayout::Qwerty);
                workflow.keyboard_closed();
                if mode == ControlMode::Mouse {
                    assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
                }
            }
        }
    }
    #[test]
    fn a_point_scan_follows_the_window_only_while_nothing_is_chosen() {
        let mut s = session(false);
        s.action(Action::Select);
        assert!(s.technique.follows_foreground());
        // Pausing chooses nothing.
        s.action(Action::Pause);
        s.action(Action::Pause);
        assert!(s.technique.follows_foreground());
        s.action(Action::Select);
        assert!(!s.technique.follows_foreground());
        s.action(Action::Select);
        // The source of a drag was chosen over a window.
        s.technique.selected(Item::Drag);
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::DragDestination)
        );
        assert!(!s.technique.follows_foreground());
        // A scan restarted by a change of mode has nothing chosen in it.
        s.technique.apply_config(last_mode().point(), true);
        assert!(s.technique.follows_foreground());
    }
    #[test]
    fn a_keyboard_that_fails_to_open_from_the_mouse_panel_returns_to_it() {
        let mut s = session(false);
        open(&mut s);
        assert_ne!(s.technique.source, (0, 0));
        assert_eq!(
            s.technique.selected(Item::MousePanel),
            Some(Request::OpenMouse)
        );
        // The point chosen before is not kept.
        assert_eq!(s.technique.source, (0, 0));
        assert_eq!(
            s.technique.mouse_key(crate::scan_mouse::Key::Keyboard),
            Some(Request::OpenKeyboard)
        );
        s.technique.execution_failed("Cleanup failed".into());
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::MouseSuspended)
        );
        assert!(!s.technique.return_to_mouse);
        assert_eq!(
            s.frame().tiles.last().unwrap().text,
            "Mouse action failed · Select to resume"
        );
    }
    #[test]
    fn another_window_in_front_takes_the_suggestions_and_nothing_else() {
        let mut s = session(false);
        s.action(Action::OpenKeyboard);
        s.technique.execution_succeeded();
        let keyboard = &mut s.technique.keyboard;
        keyboard.modifiers[1] = crate::scan_keyboard::Modifier::Locked;
        keyboard.caps = true;
        keyboard.predictions(
            Some(crate::prediction::worker::Batch {
                refined: false,
                token: 7,
                words: crate::prediction::slots(vec!["water".to_owned()]),
            }),
            false,
        );
        s.action(Action::Next);
        s.action(Action::Next);
        s.action(Action::Select);
        s.action(Action::Next);
        let selected = |s: &Session<Workflow>| {
            let tiles = s.frame().tiles;
            let selected: Vec<_> = tiles.into_iter().filter(|t| t.selected).collect();
            assert_eq!(selected.len(), 1);
            selected[0].text.clone()
        };
        let key = selected(&s);
        s.technique.typing_moved();
        assert!(s.technique.keyboard_open());
        assert_eq!(
            s.technique.keyboard.modifiers[1],
            crate::scan_keyboard::Modifier::Locked
        );
        assert!(s.technique.keyboard.caps);
        assert_eq!(selected(&s), key);
        assert!(!s.frame().tiles.iter().any(|t| t.text == "water"));
        // The keyboard follows its display without closing.
        let area = Rect {
            x: 1920.0,
            y: 0.0,
            width: 1280.0,
            height: 680.0,
        };
        let before = s.frame().tiles[0].rect;
        s.technique.set_keyboard_display(area, area, 1.0);
        assert!(s.technique.keyboard_open());
        assert_eq!(selected(&s), key);
        let after = s.frame().tiles[0].rect;
        assert_ne!(after, before);
        assert!(after.x >= area.x && after.y + after.height <= area.y + area.height);
    }
    #[test]
    fn keyboard_opening_waits_for_cleanup_and_failure_stays_in_menu() {
        let mut s = session(false);
        let w = &mut s.technique;
        w.source = (120, 230);
        assert_eq!(w.selected(Item::Keyboard), Some(Request::OpenKeyboard));
        assert_eq!(w.phase(), Phase::Workflow(WorkflowPhase::KeyboardOpening));
        assert!(w.handle(Action::Select).is_none());
        w.execution_failed("Cleanup failed".into());
        assert_eq!(w.phase(), Phase::Workflow(WorkflowPhase::MenuSuspended));
        w.error = None;
        assert_eq!(w.selected(Item::Keyboard), Some(Request::OpenKeyboard));
        w.execution_succeeded();
        assert_eq!(w.phase(), Phase::Workflow(WorkflowPhase::Keyboard));
        w.keyboard.modifiers[0] = crate::scan_keyboard::Modifier::Locked;
        w.keyboard.caps = true;
        w.execution_failed("Do not expose provider details".into());
        assert_eq!(w.phase(), Phase::Workflow(WorkflowPhase::KeyboardSuspended));
        assert_eq!(
            w.keyboard.modifiers,
            [crate::scan_keyboard::Modifier::Off; 4]
        );
        w.reset();
        assert!(!w.keyboard.caps);
        assert!(w.frame().tiles.is_empty());
    }
    #[test]
    fn typing_wait_survives_pause_hold_and_predictions_but_not_stop_or_config_reset() {
        let mut s = session(true);
        let config = Config {
            keyboard_wait_after_typing: true,
            block_interval_ms: 250,
            ..last_mode()
        };
        s.technique.apply_config(config.point(), false);
        s.action(Action::Select);
        s.technique.selected(Item::Keyboard);
        s.technique.execution_succeeded();
        // The saved choice also waits when the keyboard opens.
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::KeyboardSuspended)
        );
        assert!(s.action(Action::Select).is_none());
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::Keyboard)
        );
        assert!(s.action(Action::Select).is_none());
        assert!(matches!(
            s.action(Action::Select),
            Some(Request::Keyboard(_))
        ));
        s.technique.execution_succeeded();
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::KeyboardSuspended)
        );
        let waiting = s.frame();
        s.action(Action::Pause);
        s.tick(250, false);
        s.action(Action::Pause);
        s.tick(250, true);
        s.tick(250, false);
        assert_eq!(s.frame(), waiting);
        assert!(s.action(Action::Select).is_none());
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::Keyboard)
        );
        let resumed = s.frame();
        s.tick(249, false);
        assert_eq!(s.frame(), resumed);
        s.tick(1, false);
        assert_ne!(s.frame(), resumed);
        s.action(Action::Stop);
        assert!(!s.active());
        s.technique.execution_succeeded();
        assert_eq!(s.technique.phase(), Phase::default());
        s.technique.apply_config(config.point(), true);
        s.technique.selected(Item::Keyboard);
        s.technique.execution_succeeded();
        // Only the wait to start scanning the newly opened keyboard.
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::KeyboardSuspended)
        );
        let mut frame = s.technique.frame();
        assert_eq!(
            frame.tiles.pop().unwrap().text,
            "Keyboard waiting · Select to start"
        );
    }
    #[test]
    fn keyboard_key_acknowledgement_is_exactly_once_and_reset_discards_state() {
        let mut s = session(false);
        let w = &mut s.technique;
        w.selected(Item::Keyboard);
        w.execution_succeeded();
        assert!(w.handle(Action::Select).is_none());
        assert!(matches!(
            w.handle(Action::Select),
            Some(Request::Keyboard(_))
        ));
        assert!(w.handle(Action::Select).is_none());
        w.execution_succeeded();
        assert!(w.handle(Action::Select).is_none());
        w.reset();
        assert_eq!(w.phase(), Phase::default());
    }
    fn keyboard_session(edge: crate::point_scan::MouseRepeatStopEdge) -> Session<Workflow> {
        let config = Config {
            automatic: false,
            mouse_repeat_stop_edge: edge,
            ..last_mode()
        };
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut s = Session::new(Workflow::new(config.point(), screen, 1.0).unwrap(), false);
        assert_eq!(s.action(Action::OpenKeyboard), Some(Request::OpenKeyboard));
        s.technique.execution_succeeded();
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::Keyboard)
        );
        s.technique.set_key_repeat_settings(true, 250, 500);
        s
    }
    fn typed(request: Option<Request>, name: &'static str) -> bool {
        matches!(
            request,
            Some(Request::Keyboard(stroke))
                if stroke.key == crate::scan_keyboard::Key::Named(name)
        )
    }
    /// Chooses Backspace and lets the first press succeed.
    fn repeating_backspace(edge: crate::point_scan::MouseRepeatStopEdge) -> Session<Workflow> {
        let mut s = keyboard_session(edge);
        let request = s
            .technique
            .choose_key(crate::scan_keyboard::Key::Named("Backspace"));
        assert!(typed(request, "Backspace"));
        // Nothing repeats until the first press has been typed.
        assert!(!s.technique.mouse_repeating());
        s.technique.execution_succeeded();
        assert!(s.technique.mouse_repeating());
        s
    }
    #[test]
    fn chosen_navigation_keys_repeat_until_a_switch_stops_them() {
        use crate::point_scan::MouseRepeatStopEdge::{Press, Release};
        for edge in [Press, Release] {
            let mut s = repeating_backspace(edge);
            assert_eq!(
                s.technique.phase(),
                Phase::Workflow(WorkflowPhase::KeyboardRepeating)
            );
            assert_eq!(s.technique.mouse_repeat_stop_edge(), edge);
            let frame = s.frame();
            assert!(frame.tiles.iter().all(|tile| !tile.selected));
            assert_eq!(
                frame.tiles.last().unwrap().text,
                format!("Repeating Backspace · {}", edge.instruction())
            );

            s.tick(250, false);
            s.tick(249, false);
            assert!(s.take_selection().is_none());
            s.tick(1, false);
            assert!(typed(s.take_selection(), "Backspace"));
            s.technique.execution_succeeded();
            assert!(s.technique.mouse_repeating());
            s.tick(249, false);
            assert!(s.take_selection().is_none());
            s.tick(1, false);
            assert!(typed(s.take_selection(), "Backspace"));
            s.technique.execution_succeeded();

            assert!(s.technique.stop_mouse_repeat());
            assert!(!s.technique.stop_mouse_repeat());
            assert_eq!(
                s.technique.phase(),
                Phase::Workflow(WorkflowPhase::Keyboard)
            );
            for _ in 0..4 {
                s.tick(250, false);
                assert!(s.take_selection().is_none());
            }
            assert!(!s.technique.mouse_repeating());
        }
    }
    #[test]
    fn every_navigation_key_repeats_but_text_enter_and_other_keys_never_do() {
        use crate::scan_keyboard::Key;
        for name in [
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Tab",
            "Backspace",
            "Delete",
            "PageUp",
            "PageDown",
        ] {
            let mut s = keyboard_session(Default::default());
            assert!(typed(s.technique.choose_key(Key::Named(name)), name));
            s.technique.execution_succeeded();
            assert!(s.technique.mouse_repeating(), "{name}");
            let text = s.frame().tiles.last().unwrap().text.clone();
            assert!(text.starts_with("Repeating "), "{text}");
            assert!(!text.contains("Arrow"), "{text}");
        }
        for key in [
            Key::Named("Enter"),
            Key::Named("Escape"),
            Key::Named("Home"),
            Key::Named("End"),
            Key::Named("F5"),
            Key::Character('a', 'A'),
            Key::Character(' ', ' '),
        ] {
            let mut s = keyboard_session(Default::default());
            assert!(matches!(
                s.technique.choose_key(key),
                Some(Request::Keyboard(_))
            ));
            s.technique.execution_succeeded();
            assert!(!s.technique.mouse_repeating(), "{key:?}");
            for _ in 0..8 {
                s.tick(250, false);
                assert!(s.take_selection().is_none(), "{key:?}");
            }
        }

        // Turned off, a navigation key is typed once as before.
        let mut s = keyboard_session(Default::default());
        s.technique.set_key_repeat_settings(false, 250, 500);
        assert!(typed(
            s.technique.choose_key(Key::Named("ArrowDown")),
            "ArrowDown"
        ));
        s.technique.execution_succeeded();
        assert!(!s.technique.mouse_repeating());
        for _ in 0..8 {
            s.tick(250, false);
            assert!(s.take_selection().is_none());
        }

        // A first press that fails never starts repeating.
        let mut s = keyboard_session(Default::default());
        s.technique.choose_key(Key::Named("Delete"));
        s.execution_failed("Input failed.".into());
        assert!(!s.technique.mouse_repeating());
        for _ in 0..8 {
            s.tick(250, false);
            assert!(s.take_selection().is_none());
        }
    }
    #[test]
    fn shift_and_ctrl_repeat_with_the_key_but_window_shortcuts_never_do() {
        use crate::scan_keyboard::Key;
        let mut s = keyboard_session(Default::default());
        assert_eq!(s.technique.choose_key(Key::Modifier(0)), None);
        assert!(matches!(
            s.technique.choose_key(Key::Named("ArrowLeft")),
            Some(Request::Keyboard(stroke)) if stroke.modifiers == [true, false, false, false]
        ));
        s.technique.execution_succeeded();
        assert!(s.technique.mouse_repeating());
        assert!(s
            .frame()
            .tiles
            .last()
            .unwrap()
            .text
            .starts_with("Repeating Shift+Left arrow · "));
        // The one-shot Shift goes with every repeated press.
        s.tick(250, false);
        s.tick(250, false);
        assert!(matches!(
            s.take_selection(),
            Some(Request::Keyboard(stroke)) if stroke.modifiers[0]
        ));

        for modifier in [2, 3] {
            let mut s = keyboard_session(Default::default());
            assert_eq!(s.technique.choose_key(Key::Modifier(modifier)), None);
            assert!(typed(
                s.technique.choose_key(Key::Named("ArrowRight")),
                "ArrowRight"
            ));
            s.technique.execution_succeeded();
            assert!(!s.technique.mouse_repeating(), "{modifier}");
        }
    }
    #[test]
    fn a_key_repeat_ends_with_failure_settings_pause_window_and_scanning_changes() {
        for reason in [
            "failure", "off", "timing", "pause", "window", "select", "stop", "reset", "mouse",
            "config",
        ] {
            let mut s = repeating_backspace(Default::default());
            s.tick(250, false);
            s.tick(250, false);
            assert!(s.take_selection().is_some());
            s.technique.execution_succeeded();
            match reason {
                "failure" => s.execution_failed("Input failed.".into()),
                "off" => s.technique.set_key_repeat_settings(false, 250, 500),
                "timing" => s.technique.set_key_repeat_settings(true, 100, 500),
                "pause" => assert!(s.action(Action::Pause).is_none()),
                "window" => s.technique.typing_moved(),
                "select" => assert!(s.action(Action::Select).is_none()),
                "stop" => assert!(s.action(Action::Stop).is_none()),
                "reset" => s.reset(),
                "mouse" => assert!(s.action(Action::OpenMouse).is_some()),
                _ => s.technique.apply_config(last_mode().point(), true),
            }
            s.tick(250, false);
            assert!(!s.technique.mouse_repeating(), "{reason}");
            assert_ne!(
                s.technique.phase(),
                Phase::Workflow(WorkflowPhase::KeyboardRepeating),
                "{reason}"
            );
            for _ in 0..8 {
                s.tick(250, false);
                assert!(s.take_selection().is_none(), "{reason}");
            }
        }
        // The same settings arriving again leave the repeat running.
        let mut s = repeating_backspace(Default::default());
        s.technique.set_key_repeat_settings(true, 250, 500);
        assert!(s.technique.mouse_repeating());
    }
    fn open(s: &mut Session<Workflow>) {
        for _ in 0..3 {
            assert_eq!(s.action(Action::Select), None);
        }
        assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Menu));
    }
    #[test]
    fn a_chosen_point_is_marked_by_a_crosshair_that_stays_on_screen() {
        let mut s = session(false);
        open(&mut s);
        s.technique.source = (-500, 400);
        let rect = |x, y, width, height| Rect {
            x,
            y,
            width,
            height,
        };
        // The menu draws no strips of its own, so these are the whole marker.
        assert_eq!(
            s.technique.frame().strips,
            vec![
                rect(-516.0, 399.0, 12.0, 2.0),
                rect(-496.0, 399.0, 12.0, 2.0),
                rect(-501.0, 384.0, 2.0, 12.0),
                rect(-501.0, 404.0, 2.0, 12.0),
            ]
        );
        // At the top-left corner only the arms pointing into the screen are
        // drawn, at full thickness along its edges.
        s.technique.source = (-1000, 20);
        assert_eq!(
            s.technique.frame().strips,
            vec![
                rect(-996.0, 20.0, 12.0, 2.0),
                rect(-1000.0, 24.0, 2.0, 12.0)
            ]
        );
        // Home marks no point.
        s.technique.selected(Item::Home);
        assert!(s.technique.frame().strips.is_empty());
    }
    fn choose(s: &mut Session<Workflow>, row: usize, column: usize) -> Option<Request> {
        for _ in 0..row {
            s.action(Action::Next);
        }
        s.action(Action::Select);
        for _ in 0..column {
            s.action(Action::Next);
        }
        s.action(Action::Select)
    }
    #[test]
    fn closing_every_page_clears_nested_workflow_and_waits_for_a_new_select() {
        for automatic in [false, true] {
            for kind in crate::scan_menu::ALL_MENU_KINDS {
                let mut s = session(automatic);
                open(&mut s);
                s.technique.selected(Item::More);
                s.technique.selected(Item::Group(Kind::Browser));
                s.technique.open(kind);
                s.technique.destination = (40, 50);
                s.technique.elapsed = 123;
                s.technique.pending = Some(default_click((10, 20)));
                let keys: Vec<_> = s
                    .frame()
                    .tiles
                    .into_iter()
                    .filter(|tile| {
                        tile.style.is_some_and(|style| {
                            !matches!(
                                style.role,
                                crate::scanning::TileRole::Panel
                                    | crate::scanning::TileRole::Status
                            )
                        })
                    })
                    .collect();
                let first = keys.first().expect("menu actions");
                let rows = keys
                    .iter()
                    .filter(|tile| tile.rect.x == first.rect.x)
                    .count();
                let close = if kind == Kind::Home { 2 } else { 1 };
                assert_eq!(choose(&mut s, rows - 1, close), None, "{kind:?}");
                assert!(!s.active());
                assert!(s.technique.stage == Stage::Idle);
                assert!(s.technique.parent_menu.is_empty());
                assert!(s.technique.pending.is_none());
                assert_eq!(s.technique.source, (0, 0));
                assert_eq!(s.technique.destination, (0, 0));
                assert_eq!(s.technique.elapsed, 0);
                assert!(s.frame().tiles.is_empty());
                s.tick(5000, false);
                assert_eq!(s.take_selection(), None);
                assert!(!s.active());
                assert_eq!(s.action(Action::Select), None);
                assert!(s.active());
                assert!(s.technique.stage == Stage::Point);
            }
        }
    }
    fn auto_session(
        mode: crate::point_scan::Mode,
        automatic: bool,
    ) -> crate::scanning::Session<Workflow> {
        let config = crate::point_scan::Config {
            mode,
            automatic,
            auto_select_enabled: true,
            ..last_mode()
        };
        let workflow = Workflow::new(
            config.point(),
            Rect {
                x: -1000.0,
                y: -100.0,
                width: 800.0,
                height: 600.0,
            },
            1.0,
        )
        .unwrap();
        let mut session = crate::scanning::Session::new(workflow, automatic);
        let presses = if mode == crate::point_scan::Mode::Grid {
            5
        } else {
            3
        };
        for _ in 0..presses {
            assert_eq!(session.action(Action::Select), None);
        }
        assert!(session.technique.auto_selecting());
        session
    }
    fn advance_auto(session: &mut crate::scanning::Session<Workflow>, duration: u64, held: bool) {
        for _ in 0..duration / 100 {
            session.tick(100, held);
        }
        session.tick(duration % 100, held);
    }
    #[test]
    fn auto_click_runs_once_in_every_scan_mode_and_then_waits() {
        for mode in [crate::point_scan::Mode::Line, crate::point_scan::Mode::Grid] {
            for automatic in [false, true] {
                let mut session = auto_session(mode, automatic);
                let target = session.technique.source;
                advance_auto(&mut session, 999, false);
                assert_eq!(session.take_selection(), None);
                assert_eq!(session.frame().countdown.as_ref().unwrap().point, target);
                session.tick(1, false);
                assert_eq!(session.take_selection(), Some(default_click(target)));
                assert!(!session.active());
                advance_auto(&mut session, 2000, false);
                assert_eq!(session.take_selection(), None);
                assert!(session.frame().countdown.is_none());
            }
        }
    }
    #[test]
    fn held_switch_and_pause_freeze_countdown_and_menu_consumes_selection() {
        let mut session = auto_session(crate::point_scan::Mode::Line, false);
        advance_auto(&mut session, 900, false);
        advance_auto(&mut session, 2000, true);
        assert_eq!(session.frame().countdown.unwrap().permille, 900);
        session.action(Action::Pause);
        advance_auto(&mut session, 2000, false);
        assert_eq!(session.frame().countdown.unwrap().permille, 900);
        session.action(Action::Pause);
        assert_eq!(session.action(Action::Select), None);
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::Menu)
        );
        advance_auto(&mut session, 2000, false);
        assert_eq!(session.take_selection(), None);
        assert!(!session.frame().tiles.is_empty());
    }
    #[test]
    fn pause_resumes_remaining_time_and_reset_discards_even_pending_click() {
        let mut session = auto_session(crate::point_scan::Mode::Line, true);
        advance_auto(&mut session, 900, false);
        session.action(Action::Pause);
        advance_auto(&mut session, 500, false);
        session.action(Action::Pause);
        advance_auto(&mut session, 99, false);
        assert_eq!(session.take_selection(), None);
        session.tick(1, false);
        session.reset();
        assert_eq!(session.take_selection(), None);
        for action in [Action::Cancel, Action::Stop] {
            let mut session = auto_session(crate::point_scan::Mode::Line, false);
            session.action(action);
            advance_auto(&mut session, 2000, false);
            assert_eq!(session.take_selection(), None);
        }
    }
    #[test]
    fn config_changes_cancel_countdown_and_auto_click_errors_offer_menu_return() {
        let mut session = auto_session(crate::point_scan::Mode::Line, false);
        let config = session.technique.point.config.clone();
        session.technique.apply_config(config, false);
        advance_auto(&mut session, 2000, false);
        assert_eq!(session.take_selection(), None);
        let mut session = auto_session(crate::point_scan::Mode::Line, false);
        advance_auto(&mut session, 1000, false);
        assert!(session.take_selection().is_some());
        session.execution_failed("Cleanup failed".into());
        let frame = session.frame();
        assert!(frame
            .tiles
            .iter()
            .any(|tile| tile.text.ends_with("Action failed · Select to return")));
        assert!(frame.label.unwrap().text.starts_with("Cleanup failed"));
        assert_eq!(session.action(Action::Select), None);
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::Menu)
        );
        assert_eq!(session.take_selection(), None);
    }
    #[test]
    fn auto_selection_excludes_drag_destinations() {
        let mut session = auto_session(crate::point_scan::Mode::Line, false);
        session.action(Action::Select);
        session.technique.selected(Item::Drag);
        session.action(Action::Select);
        session.action(Action::Select);
        assert_eq!(
            session.technique.phase(),
            Phase::Workflow(WorkflowPhase::DragConfirmation)
        );
        assert!(session.frame().countdown.is_none());
    }
    #[test]
    fn countdown_artwork_has_transparent_center_and_growing_colour() {
        let mut countdown = crate::scanning::Countdown {
            point: (-200, 50),
            scale: 2.0,
            permille: 100,
            color: crate::scanning::ScannerColor::Green,
        };
        let early = countdown.bitmap(2.0).unwrap();
        countdown.permille = 900;
        let late = countdown.bitmap(2.0).unwrap();
        assert_eq!(late.pixel(128, 128).unwrap().alpha(), 0);
        let alpha =
            |p: &tiny_skia::Pixmap| p.pixels().iter().map(|p| p.alpha() as u64).sum::<u64>();
        assert!(alpha(&late) > alpha(&early));
        let rect = countdown.rect();
        assert_eq!(rect.x + rect.width / 2.0, -200.0);
        assert_eq!(rect.y + rect.height / 2.0, 50.0);
    }

    #[test]
    fn nested_back_restores_parent_and_pause_resumes_without_selection() {
        let mut w = Workflow::new(
            last_mode().point(),
            Rect {
                x: 0.,
                y: 0.,
                width: 1280.,
                height: 720.,
            },
            1.,
        )
        .unwrap();
        w.source = (120, 80);
        w.stage = Stage::Menu;
        w.selected(Item::More);
        w.selected(Item::Group(Kind::Browser));
        w.selected(Item::Group(Kind::Tabs));
        assert_eq!(w.parent_menu.len(), 3);
        w.selected(Item::Back);
        assert_eq!(w.menu.kind, Kind::Browser);
        w.selected(Item::Back);
        assert_eq!(w.menu.kind, Kind::More);
        w.selected(Item::Pause);
        let frame = w.frame();
        w.advance(5000);
        assert_eq!(w.frame(), frame);
        assert!(w.handle(Action::Select).is_none());
        assert!(!w.menu.suspended());
        assert_eq!(w.source, (120, 80));
    }
    #[test]
    fn menu_errors_stay_clear_of_the_panel_inside_the_work_area() {
        use crate::{scan_panel::Dock, scan_preferences::PanelSize};
        let message = "Release modifiers and end switch forwarding before selecting an action.";
        let display = |x: f64, y: f64, width: f64, height: f64, scale: f64| {
            (
                Rect {
                    x,
                    y,
                    width: width * scale,
                    height: height * scale,
                },
                scale,
            )
        };
        let mut moved = 0;
        for (screen, scale) in [
            display(-1920.0, -200.0, 1280.0, 720.0, 1.5),
            display(0.0, 0.0, 1280.0, 720.0, 1.0),
            display(0.0, 0.0, 1280.0, 720.0, 2.0),
            display(0.0, 0.0, 1366.0, 768.0, 1.0),
            display(0.0, 0.0, 1920.0, 1080.0, 1.0),
        ] {
            // A taskbar along the bottom.
            let area = Rect {
                height: screen.height - 48.0 * scale,
                ..screen
            };
            for size in [PanelSize::Small, PanelSize::Medium, PanelSize::Large] {
                for dock in crate::scan_panel::position_rows(|dock| dock, Dock::default())[..3]
                    .iter()
                    .flatten()
                    .copied()
                {
                    for pointer in [false, true] {
                        let mut config = Config {
                            panel_avoids_pointer: true,
                            ..last_mode()
                        };
                        config.scan_preferences.panel_size = size;
                        let mut w = Workflow::new(config.point(), screen, scale).unwrap();
                        w.set_keyboard_area(area);
                        w.set_dock(dock);
                        w.stage = Stage::Menu;
                        w.execution_failed(message.into());
                        let docked = w.frame().tiles[0].rect;
                        if pointer {
                            // Over the docked panel, which then moves out of the way.
                            w.set_pointer(Some((docked.x + 1.0, docked.y + docked.height / 2.0)));
                            if w.moved.is_some() {
                                moved += 1;
                                assert_ne!(w.frame().tiles[0].rect, docked);
                            }
                        }
                        let frame = w.frame();
                        let panel = frame.tiles[0].rect;
                        let label = frame.label.unwrap();
                        let rect = label.rect;
                        let case = format!("{screen:?} {size:?} {dock:?} pointer {pointer}");
                        assert!(
                            rect.y >= area.y - 0.001
                                && rect.y + rect.height <= area.y + area.height + 0.001,
                            "{case}"
                        );
                        assert!(
                            rect.x >= area.x - 0.001
                                && rect.x + rect.width <= area.x + area.width + 0.001,
                            "{case}"
                        );
                        // Every one of these displays has room beside or around the panel.
                        let apart = rect.y + rect.height <= panel.y + 0.001
                            || rect.y >= panel.y + panel.height - 0.001
                            || rect.x + rect.width <= panel.x + 0.001
                            || rect.x >= panel.x + panel.width - 0.001;
                        assert!(apart, "{case}: {rect:?} over {panel:?}");
                        assert!(rect.width >= 240.0 * scale - 0.001, "{case}");
                        assert_eq!(label.hud.unwrap().screen, screen);
                    }
                }
            }
        }
        assert!(moved > 0, "some panels moved away from the pointer");
        // An empty message still says what happened.
        let mut w = Workflow::new(
            last_mode().point(),
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            1.0,
        )
        .unwrap();
        w.stage = Stage::Menu;
        w.execution_failed(String::new());
        assert_eq!(
            w.frame().label.unwrap().text,
            "Action failed.\nSelect to return"
        );
    }
    #[test]
    fn command_failure_preserves_point_and_requires_acknowledgement() {
        let mut w = Workflow::new(
            last_mode().point(),
            Rect {
                x: 0.,
                y: 0.,
                width: 1280.,
                height: 720.,
            },
            1.,
        )
        .unwrap();
        w.source = (120, 80);
        w.stage = Stage::Menu;
        assert!(matches!(
            w.selected(Item::Command(crate::scan_menu::Command::Copy)),
            Some(Request::Command { .. })
        ));
        w.point.config.menu_scan.panel_size = crate::scan_preferences::PanelSize::Small;
        // The longest messages an action can fail with.
        for message in [
            "Release modifiers and end switch forwarding before selecting an action.",
            "Typing with the keyboard. Resume switch control in Switches when finished.",
        ] {
            for dock in [
                crate::scan_panel::Dock { column: 1, row: 0 },
                crate::scan_panel::Dock::default(),
            ] {
                w.set_dock(dock);
                w.stage = Stage::Menu;
                w.execution_failed(message.into());
                // The status stays one short line, and the message is shown clear of the panel.
                let frame = w.frame();
                let status = frame
                    .tiles
                    .iter()
                    .find(|tile| {
                        tile.style
                            .is_some_and(|style| style.role == crate::scanning::TileRole::Status)
                    })
                    .unwrap();
                assert_eq!(status.text, "Actions · Action failed · Select to return");
                assert!(frame.tiles.iter().all(|tile| !tile.selected));
                let label = frame.label.unwrap();
                assert_eq!(label.text, format!("{message}\nSelect to return"));
                assert!(label.hud.is_some());
                let panel = frame.tiles[0].rect;
                assert!(
                    label.rect.y + label.rect.height <= panel.y
                        || label.rect.y >= panel.y + panel.height,
                    "{dock:?}"
                );
                assert!(label.rect.y >= 0.0 && label.rect.y + label.rect.height <= 720.0);
                assert!(w.handle(Action::Select).is_none());
                assert!(w.frame().label.is_none());
            }
        }
        assert_eq!(w.selected(Item::LeftClick), Some(default_click((120, 80))));
    }
    #[test]
    fn scanner_colour_follows_action_scroll_and_drag_menus() {
        use crate::scanning::ScannerColor::*;
        for color in [Red, Green, Blue, Yellow, White] {
            let mut s = session(false);
            s.technique.point.config.scanner_color = color;
            s.technique.point.config.scan.color = color;
            s.technique.point.config.menu_scan.color = color;
            s.technique.point.config.keyboard_scan.color = color;
            open(&mut s);
            for kind in [Kind::Actions, Kind::Scroll, Kind::ConfirmDrag] {
                s.technique.open(kind);
                let frame = s.frame();
                assert_eq!(frame.color, color);
                assert!(!frame.tiles.is_empty());
                assert!(frame.tiles.iter().all(|tile| tile.color == color));
            }
        }
    }
    #[test]
    fn point_selection_never_clicks_and_menu_clicks_complete_once() {
        for (column, right, count) in [(0, false, 1), (1, true, 1), (2, false, 2)] {
            let mut s = session(false);
            open(&mut s);
            // The background, eleven actions and the status header.
            assert_eq!(s.frame().tiles.len(), 13);
            assert_eq!(
                choose(&mut s, 0, column),
                Some(Request::Click {
                    point: (-1000, 20),
                    right,
                    count
                })
            );
            assert!(!s.active());
            assert!(s.frame().tiles.is_empty());
            assert_eq!(s.take_selection(), None);
        }
    }
    #[test]
    fn scrolling_retains_direction_and_back_restores_scroll_position() {
        let mut s = session(false);
        open(&mut s);
        choose(&mut s, 1, 0);
        assert_eq!(
            choose(&mut s, 0, 1),
            Some(Request::Scroll {
                point: (-1000, 20),
                dx: 0,
                dy: -3
            })
        );
        assert_eq!(
            s.action(Action::Select),
            Some(Request::Scroll {
                point: (-1000, 20),
                dx: 0,
                dy: -3
            })
        );
        s.action(Action::Next);
        s.action(Action::Select);
        s.action(Action::Next);
        s.action(Action::Next);
        assert_eq!(s.action(Action::Select), None);
        assert_eq!(s.action(Action::Select), None);
        assert_eq!(s.technique.menu.kind, Kind::Actions);
        assert_eq!(s.action(Action::Select), None);
        assert_eq!(s.technique.menu.kind, Kind::Scroll);
    }
    #[test]
    fn drag_waits_for_confirmation_and_ticks_in_manual_mode() {
        let mut s = session(false);
        open(&mut s);
        assert_eq!(choose(&mut s, 1, 1), None);
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::DragDestination)
        );
        s.action(Action::Next);
        s.action(Action::Select);
        s.action(Action::Next);
        s.action(Action::Select);
        assert_eq!(s.take_selection(), None);
        assert_eq!(choose(&mut s, 0, 0), Some(Request::DragStart((-1000, 20))));
        assert_eq!(s.action(Action::Select), None);
        s.tick(150, true);
        assert!(matches!(s.take_selection(), Some(Request::DragMove(_))));
        s.tick(150, false);
        assert!(matches!(s.take_selection(), Some(Request::DragEnd(_))));
        assert!(!s.active());
        assert!(s.frame().strips.is_empty());
    }
    #[test]
    fn cancellation_discards_pending_drag_output() {
        let mut s = session(false);
        open(&mut s);
        choose(&mut s, 1, 1);
        s.action(Action::Select);
        s.action(Action::Select);
        choose(&mut s, 0, 0);
        s.tick(33, false);
        s.action(Action::Cancel);
        assert_eq!(s.take_selection(), None);
        assert!(!s.active());
        assert!(s.frame().tiles.is_empty());
        assert!(s.frame().label.is_none());
    }
    #[test]
    fn automatic_menu_pauses_preserving_target_and_select_only_resumes() {
        let mut s = session(true);
        open(&mut s);
        for _ in 0..12 {
            s.tick(250, false);
            s.take_selection();
        }
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::MenuSuspended)
        );
        assert!(!s.frame().tiles.is_empty());
        assert_eq!(s.action(Action::Select), None);
        assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Menu));
        assert_eq!(choose(&mut s, 0, 0), Some(default_click((-1000, 20))));
    }
    #[test]
    fn existing_phase_strings_are_preserved() {
        assert_eq!(
            serde_json::to_value(Phase::Point(crate::point_scan::Phase::RowEscape)).unwrap(),
            "rowEscape"
        );
        assert_eq!(
            serde_json::to_value(Phase::Workflow(WorkflowPhase::MenuSuspended)).unwrap(),
            "menuSuspended"
        );
    }
    #[test]
    fn confirmed_drag_clears_pause_and_cannot_be_paused_with_button_held() {
        let mut s = session(true);
        open(&mut s);
        choose(&mut s, 1, 1);
        s.action(Action::Select);
        s.action(Action::Select);
        s.action(Action::Pause);
        assert!(s.paused());
        assert!(matches!(choose(&mut s, 0, 0), Some(Request::DragStart(_))));
        assert!(!s.paused());
        s.action(Action::Pause);
        assert!(!s.paused());
        s.tick(250, false);
        s.take_selection();
        s.tick(50, false);
        assert!(matches!(s.take_selection(), Some(Request::DragEnd(_))));
    }
    #[test]
    fn point_reselection_cancel_and_drag_confirmation_back_paths() {
        let mut s = session(false);
        open(&mut s);
        assert_eq!(choose(&mut s, 3, 0), None);
        assert_eq!(
            s.technique.phase(),
            Phase::Point(crate::point_scan::Phase::X)
        );
        s.action(Action::Select);
        s.action(Action::Select);
        assert_eq!(choose(&mut s, 3, 1), None);
        assert!(!s.active());
        open(&mut s);
        choose(&mut s, 1, 1);
        s.action(Action::Select);
        s.action(Action::Select);
        assert_eq!(choose(&mut s, 0, 1), None);
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::DragDestination)
        );
        s.action(Action::Select);
        s.action(Action::Select);
        assert_eq!(choose(&mut s, 1, 0), None);
        assert_eq!(s.technique.menu.kind, Kind::Actions);
        assert_eq!(s.take_selection(), None);
    }
    #[test]
    fn session_uses_each_areas_automatic_setting_without_ignoring_pause_or_hold() {
        use crate::scan_preferences::Area;
        let mut config = crate::point_scan::Config {
            automatic: false,
            ..last_mode()
        };
        config.scan_preferences.menu.automatic = Some(true);
        config.scan_preferences.menu.interval_ms = Some(250);
        config.scan_preferences.keyboard.automatic = Some(false);
        let mut s = session(false);
        s.technique.apply_config(config.point(), false);
        s.action(Action::Select);
        let initial = s.frame();
        s.tick(250, false);
        assert_eq!(s.frame(), initial);
        s.action(Action::Select);
        s.action(Action::Select);
        let menu = s.frame();
        s.tick(250, true);
        assert_eq!(s.frame(), menu);
        s.action(Action::Pause);
        s.tick(250, false);
        assert_eq!(s.frame(), menu);
        s.action(Action::Pause);
        s.tick(250, false);
        assert_ne!(s.frame(), menu);
        assert!(config.resolved(Area::Menu).automatic);
        s.technique.stage = Stage::Keyboard;
        let keyboard = s.frame();
        s.tick(250, false);
        assert_eq!(s.frame(), keyboard);
    }
}
