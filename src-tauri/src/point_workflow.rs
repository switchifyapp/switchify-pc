//! Point selection policy, menu navigation and execution requests. No OS input.
use crate::{
    custom_panels::CustomPanel,
    panel_actions::{BuiltInPanel, MouseButton, PanelAction, ScrollDirection},
    panel_extras::Target as ExtraTarget,
    point_scan::{Engine, PointSettings},
    scan_menu::{Item, Kind, Menu, PageRef},
    scanning::{Action, Frame, FrameLabel, Rect, Technique, UpdateContext},
};
use serde::Serialize;
pub type Point = (i32, i32);

/// A page the editor asks to try: one of the user's panels, a built-in
/// menu, or a native panel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TrialTarget {
    Panel { id: String },
    Menu { menu: Kind },
    Native { panel: ExtraTarget },
}
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Prediction {
        token: u64,
        index: usize,
    },
    PredictionRetry,
    OpenKeyboard,
    /// Shows the Switchify window. Scanning has ended.
    OpenSwitchify,
    /// Scanning has ended from a panel that may hold input, such as a drag.
    Close,
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
    /// A left click from the Left click at the pointer switch. A held drag
    /// refuses it instead of being dropped.
    PointerClick,
    MouseScroll {
        dy: i32,
    },
    MouseDrag,
    MouseSpeed(i8),
    /// The Blocks tile flips the saved mouse movement.
    MouseMovementToggle,
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
    /// A key chord chosen on one of the user's panels.
    Chord(Vec<String>),
    /// Text chosen on one of the user's panels. Never logged.
    Text(String),
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
fn selection_policy(
    point: Point,
    options: crate::scan_preferences::Resolved,
    has_panels: bool,
) -> (Point, Menu) {
    (
        point,
        Menu::configured_for(Kind::Actions, options, has_panels),
    )
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
/// Which of the saved documents changed, so only the page built from it is rebuilt.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Saved {
    Panels,
    Menus,
    Extras,
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
    /// A direction hopping a block at a time until a switch stops it.
    block_hop: Option<crate::mouse_repeat::ScanBlockHop>,
    mouse_block_size: crate::mouse_repeat::BlockSize,
    /// Milliseconds between repeating block hops, or 0 for the scan speed.
    mouse_block_interval_ms: u32,
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
    /// The menus Back leads through, outermost first. Only their identity is
    /// kept: Back rebuilds the parent so it scans from the top like any other
    /// freshly opened menu.
    parent_menu: Vec<PageRef>,
    /// The user's own panels, as last loaded. Home lists them when there are any.
    panels: Vec<CustomPanel>,
    /// Built-in menus the user rearranged, by menu.
    menu_overrides: std::collections::BTreeMap<Kind, CustomPanel>,
    /// Extra tiles around the mouse panel's and the keyboard's keys.
    panel_extras: std::collections::BTreeMap<ExtraTarget, crate::panel_extras::PanelExtras>,
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
            mouse: crate::scan_mouse::MousePanel::new(mouse_options, 1, 100),
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
            block_hop: None,
            mouse_block_size: Default::default(),
            mouse_block_interval_ms: 0,
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
            menu: Menu::configured_for(Kind::Actions, menu_options, false),
            parent_menu: vec![],
            panels: vec![],
            menu_overrides: Default::default(),
            panel_extras: Default::default(),
            source: (0, 0),
            destination: (0, 0),
            elapsed: 0,
            pending: None,
            error: None,
            following: false,
        })
    }
    /// Replaces the user's panels. An open page keeps its tiles until it is
    /// reopened.
    pub fn set_panels(&mut self, panels: Vec<CustomPanel>) {
        if self.panels == panels {
            return;
        }
        self.panels = panels;
        self.refresh_page(Saved::Panels);
    }
    /// Replaces the rearranged menus. An open menu follows the change.
    pub fn set_menu_overrides(&mut self, overrides: std::collections::BTreeMap<Kind, CustomPanel>) {
        if self.menu_overrides == overrides {
            return;
        }
        self.menu_overrides = overrides;
        self.refresh_page(Saved::Menus);
    }
    /// Replaces the extra tiles on the mouse panel and the keyboard. An open
    /// panel follows the change.
    pub fn set_panel_extras(
        &mut self,
        extras: std::collections::BTreeMap<ExtraTarget, crate::panel_extras::PanelExtras>,
    ) {
        if self.panel_extras == extras {
            return;
        }
        self.panel_extras = extras;
        self.refresh_page(Saved::Extras);
    }
    /// Rebuilds the open page from the documents it was built from, so an
    /// edit saved in the Panels page shows straight away. Scanning starts
    /// again from the top of the page, keeping a pause or a wait for a
    /// switch; a panel that is gone gives way to the page under it. A page
    /// in the middle of a move or a scroll is left alone until it is next
    /// opened. Saving something else leaves the open page alone, so the
    /// scan keeps its place.
    fn refresh_page(&mut self, saved: Saved) {
        match self.stage {
            Stage::Menu if saved != Saved::Extras => {
                let (suspended, waiting) = (self.menu.suspended(), self.menu.waiting());
                match self.menu.page_ref() {
                    PageRef::Custom(_) if saved == Saved::Menus => return,
                    PageRef::Custom(id) => {
                        if !self.open_custom(&id) {
                            self.error = None;
                            self.restore_actions();
                        }
                    }
                    // The list is built from the panels alone.
                    PageRef::Kind(Kind::Panels) if saved == Saved::Menus => return,
                    // The menus and the action pages list the panels, so they follow both.
                    PageRef::Kind(kind)
                        if kind == Kind::Panels
                            || (kind == Kind::Actions && saved == Saved::Panels)
                            || crate::builtin_panels::editable(kind) =>
                    {
                        self.menu = self.build(kind);
                    }
                    PageRef::Kind(_) => return,
                }
                if suspended {
                    self.menu.suspend();
                }
                if waiting {
                    self.menu.wait();
                }
            }
            Stage::Keyboard if saved == Saved::Extras => {
                let extras = self.extras_for(ExtraTarget::Keyboard);
                self.keyboard.set_extras(extras);
            }
            Stage::Mouse if saved == Saved::Extras => {
                let extras = self.extras_for(ExtraTarget::Mouse);
                self.mouse.set_extras(extras);
            }
            _ => {}
        }
    }
    /// Starts on the page the editor asked to try, as if it had just been
    /// chosen from Home, so Back and Close go where they always do.
    pub fn open_for_trial(&mut self, target: &TrialTarget) -> Result<Option<Request>, String> {
        self.reset();
        self.open_home();
        match target {
            TrialTarget::Panel { id } => {
                let panel = self
                    .panels
                    .iter()
                    .find(|panel| panel.id == *id)
                    .ok_or("That panel no longer exists.")?;
                let next = Menu::custom(panel, self.point.config.menu_scan);
                self.push_menu(next);
                Ok(None)
            }
            TrialTarget::Menu {
                menu: Kind::Home | Kind::More,
            } => Ok(None),
            TrialTarget::Menu { menu } => {
                if !crate::builtin_panels::editable(*menu) {
                    return Err("That menu cannot be tried on its own.".into());
                }
                let next = self.build(*menu);
                self.push_menu(next);
                Ok(None)
            }
            TrialTarget::Native {
                panel: ExtraTarget::Mouse,
            } => Ok(self.open_mouse(true)),
            TrialTarget::Native {
                panel: ExtraTarget::Keyboard,
            } => Ok(self.selected(Item::Keyboard)),
        }
    }
    fn extras_for(&self, target: ExtraTarget) -> crate::panel_extras::PanelExtras {
        self.panel_extras.get(&target).cloned().unwrap_or_default()
    }
    fn new_keyboard(&self) -> crate::scan_keyboard::Keyboard {
        let mut keyboard = crate::scan_keyboard::Keyboard::configured(
            cfg!(target_os = "macos"),
            self.point.config.keyboard_scan,
        )
        .with_layout(self.point.config.keyboard_layout)
        .with_wait_after_typing(
            self.point.config.keyboard_scan.next_scan == crate::scan_preferences::NextScan::Wait,
        )
        .with_extras(self.extras_for(ExtraTarget::Keyboard));
        keyboard.dock = self.dock;
        keyboard
    }
    fn new_mouse(&self) -> crate::scan_mouse::MousePanel {
        let mut mouse = crate::scan_mouse::MousePanel::new(
            self.point.config.mouse_scan,
            1,
            self.mouse.speed_percent,
        )
        .with_extras(self.extras_for(ExtraTarget::Mouse));
        mouse.dock = self.dock;
        mouse.blocks = self.mouse.blocks;
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
        let colors_changed =
            self.point.config.menu_scan.panel_colors != config.menu_scan.panel_colors;
        self.point.config = config;
        self.keyboard
            .set_panel_size(self.point.config.keyboard_scan.panel_size);
        self.mouse
            .set_panel_size(self.point.config.mouse_scan.panel_size);
        self.menu
            .set_panel_size(self.point.config.menu_scan.panel_size);
        self.keyboard
            .set_panel_theme(self.point.config.keyboard_scan.panel_theme);
        self.mouse
            .set_panel_theme(self.point.config.mouse_scan.panel_theme);
        self.menu
            .set_panel_theme(self.point.config.menu_scan.panel_theme);
        self.keyboard
            .set_panel_colors(self.point.config.keyboard_scan.panel_colors);
        self.mouse
            .set_panel_colors(self.point.config.mouse_scan.panel_colors);
        self.menu
            .set_panel_colors(self.point.config.menu_scan.panel_colors);
        // Home as shipped takes its tile colours from the setting, so an open
        // built-in menu is rebuilt to follow it, keeping its place in the scan.
        if colors_changed {
            self.refresh_page(Saved::Menus);
        }

        self.keyboard.set_layout(self.point.config.keyboard_layout);
        self.menu
            .set_period(self.point.config.menu_scan.interval_ms);
        // Line or Grid chosen from Home is saved for the next point scan;
        // Home stays open rather than starting one.
        if restart && !self.home_open() {
            self.keyboard = self.new_keyboard();
            self.keyboard_layout_pending = false;
            self.mouse = self.new_mouse();
            self.return_to_mouse = false;
            self.mouse_actions_open = false;
            self.move_repeat = None;
            self.block_hop = None;
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
    /// One of the user's panels is open. Its pointer tiles act at the pointer, or
    /// at the chosen point when it was opened from the point's page, so the
    /// runtime keeps the pointer position current while it is open.
    pub fn custom_panel_open(&self) -> bool {
        self.stage == Stage::Menu && self.menu.custom_id.is_some()
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
                    .is_some_and(|page| *page == PageRef::Kind(Kind::Home)))
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
            self.block_hop = None;
            self.scroll_repeat = None;
            self.radar = None;
            self.pending = None;
            self.mouse.opened();
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
            self.block_hop = None;
            self.scroll_repeat = None;
            self.pending = None;
            self.mouse.restart();
        }
    }
    /// A switch click was refused, for example during a drag. The mouse panel
    /// writes out the failure as it does for its own actions; the drag and
    /// everything else stay as they were.
    pub fn switch_click_refused(&mut self) {
        if self.stage == Stage::Mouse {
            self.mouse.failed();
        }
    }
    /// Takes a new Auto select setting without moving the scan. A countdown
    /// already under way gives way to the action menu when it is turned off.
    pub fn set_auto_select(&mut self, enabled: bool) {
        self.point.config.auto_select_enabled = enabled;
        if !enabled && self.stage == Stage::Countdown {
            self.elapsed = 0;
            self.open(Kind::Actions);
        }
    }
    /// Whether the arrows glide or jump a block, how big a block is and how
    /// often repeating blocks hop (0 for the scan speed). A change ends a
    /// move in flight, as other mouse settings do; a scroll carries on.
    pub fn set_mouse_movement(
        &mut self,
        movement: crate::mouse_repeat::MouseMovement,
        block_size: crate::mouse_repeat::BlockSize,
        block_interval_ms: u32,
    ) {
        let blocks = movement == crate::mouse_repeat::MouseMovement::Blocks;
        let changed = self.mouse.blocks != blocks
            || self.mouse_block_size != block_size
            || self.mouse_block_interval_ms != block_interval_ms;
        self.mouse.blocks = blocks;
        self.mouse_block_size = block_size;
        self.mouse_block_interval_ms = block_interval_ms;
        if changed {
            self.block_hop = None;
        }
        if changed && self.stage == Stage::MouseMoving {
            self.stage = Stage::Mouse;
            self.move_repeat = None;
            self.block_hop = None;
            self.pending = None;
            self.mouse.restart();
        }
    }
    /// One block in a direction on the display under the pointer.
    fn block_delta(&self, dx: i8, dy: i8) -> (i32, i32) {
        let step = crate::mouse_repeat::block_distance(
            self.point.screen.width,
            self.point.screen.height,
            self.mouse_block_size,
        );
        (i32::from(dx) * step, i32::from(dy) * step)
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
            // A refused switch click is the one failure that keeps a drag
            // held, so the pointer still shows it.
            return (self.stage == Stage::Mouse && self.mouse.dragging)
                .then_some(PointerFeedback::Drag);
        }
        match self.stage {
            Stage::MouseMoving => Some(PointerFeedback::RepeatMove {
                // Block hops keep one size and pace.
                accelerated: self.block_hop.is_none() && self.mouse_acceleration_ms > 0,
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
    /// The display the keyboard or Home menu is on changed, or was replaced.
    pub fn set_keyboard_display(&mut self, area: Rect, screen: Rect, scale: f64) {
        self.keyboard_area = area;
        self.point.screen = screen;
        self.point.units_per_logical_pixel = scale;
    }
    pub fn panel_avoids_pointer(&self) -> bool {
        self.point.config.panel_avoids_pointer
    }
    /// Whether `pointer` is over the menu bar of the panel showing now. The
    /// bar takes mouse clicks; the rest of the panel passes them through.
    pub fn menu_bar_contains(&self, (x, y): (f64, f64)) -> bool {
        crate::scan_panel::menu_bar(&self.frame()).is_some_and(|bar| {
            x >= bar.x && x < bar.x + bar.width && y >= bar.y && y < bar.y + bar.height
        })
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
                false,
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
            crate::scan_panel::menu_bar(&frame),
        )
    }
    /// A page as it should open now: the editable menus from their document
    /// (as shipped or rearranged), the panel list from the user's panels,
    /// every other page from its fixed rows.
    fn build(&self, kind: Kind) -> Menu {
        let options = self.point.config.menu_scan;
        if crate::builtin_panels::editable(kind) {
            let mut document =
                crate::builtin_panels::document(kind, &self.menu_overrides, options.panel_colors);
            if self.panels.is_empty() {
                // Nothing to list yet, so the tile that would list it is left out.
                for row in &mut document.rows {
                    row.retain(|cell| cell.action != PanelAction::MyPanels);
                }
                document.rows.retain(|row| !row.is_empty());
            }
            return Menu::builtin(kind, &document, options);
        }
        match kind {
            Kind::Panels => Menu::panel_list(&self.panels, options),
            kind => Menu::configured_for(kind, options, !self.panels.is_empty()),
        }
    }
    fn open(&mut self, kind: Kind) {
        self.menu = self.build(kind);
        self.stage = Stage::Menu;
    }
    /// The point a panel opened from the point's page acts at, as that page's
    /// own clicks do. Opened from anywhere else, no point was chosen.
    fn chosen_point(&self) -> Option<Point> {
        (self.parent_menu.first() == Some(&PageRef::Kind(Kind::Actions))).then_some(self.source)
    }
    /// Where a pointer action on a custom panel lands: at the chosen point
    /// when there is one, otherwise at the pointer, which is not always known.
    fn pointer_point(&mut self) -> Option<Point> {
        if let Some(point) = self.chosen_point() {
            return Some(point);
        }
        match self.pointer {
            Some((x, y)) => Some((x.round() as i32, y.round() as i32)),
            None => {
                self.error = Some("Pointer unavailable. Try again.".into());
                None
            }
        }
    }
    /// Opens one of the user's panels, or reports that it is gone.
    fn open_custom(&mut self, id: &str) -> bool {
        match self.panels.iter().find(|panel| panel.id == id) {
            Some(panel) => {
                self.menu = Menu::custom(panel, self.point.config.menu_scan);
                self.stage = Stage::Menu;
                true
            }
            None => {
                self.error = Some("That panel no longer exists.".into());
                false
            }
        }
    }
    /// Opens `next` with the current page on the Back stack.
    fn push_menu(&mut self, next: Menu) {
        let previous = std::mem::replace(&mut self.menu, next);
        self.parent_menu.push(previous.page_ref());
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
        // A page opened from the mouse panel goes back to it.
        if self.mouse_actions_open && self.parent_menu.is_empty() {
            return self.return_to_mouse_panel();
        }
        match self.parent_menu.pop() {
            Some(PageRef::Kind(kind)) => self.open(kind),
            Some(PageRef::Custom(id)) => {
                if !self.open_custom(&id) {
                    self.open_home();
                }
            }
            // The user's pages are reached from Home; the point menus from a point.
            None if self.menu.custom_id.is_some() || self.menu.kind == Kind::Panels => {
                self.open_home()
            }
            None => self.open(Kind::Actions),
        }
    }
    /// A tile of the open custom panel, or of a built-in menu built from
    /// its document, was chosen. On the user's panels, actions leave the
    /// panel open so the next choice can follow, as on a communication
    /// board. On the built-in menus they behave as the menus always have.
    fn custom_selected(&mut self, index: usize) -> Option<Request> {
        let action = self.menu.cell(index)?.action.clone();
        let builtin = self.menu.custom_id.is_none();
        match action {
            PanelAction::Key { keys } => {
                self.menu.continue_after_selection();
                Some(Request::Chord(keys))
            }
            PanelAction::Text { text } => {
                self.menu.continue_after_selection();
                Some(Request::Text(text))
            }
            PanelAction::Command { command } if builtin => self.selected(Item::Command(command)),
            PanelAction::Command { command } => {
                // Modifier clicks act at the pointer; the rest need no point.
                let point = if command.at_point() {
                    self.pointer_point()?
                } else {
                    self.source
                };
                self.menu.continue_after_selection();
                Some(Request::Command { command, point })
            }
            PanelAction::MyPanels => {
                let next = Menu::panel_list(&self.panels, self.point.config.menu_scan);
                self.push_menu(next);
                None
            }
            PanelAction::Setting { setting } => {
                if !builtin {
                    self.menu.continue_after_selection();
                }
                Some(Request::Setting(setting))
            }
            PanelAction::Click {
                button: MouseButton::Middle,
                ..
            } => {
                let point = self.pointer_point()?;
                self.menu.continue_after_selection();
                Some(Request::Command {
                    command: crate::scan_menu::Command::MiddleClick,
                    point,
                })
            }
            PanelAction::Click { button, count } => {
                self.menu.continue_after_selection();
                let right = button == MouseButton::Right;
                Some(match self.chosen_point() {
                    Some(point) => Request::Click {
                        point,
                        right,
                        count,
                    },
                    None => Request::MouseClick { right, count },
                })
            }
            PanelAction::Scroll { direction } => {
                let point = self.pointer_point()?;
                self.menu.continue_after_selection();
                let (dx, dy) = match direction {
                    ScrollDirection::Up => (0, 3),
                    ScrollDirection::Down => (0, -3),
                    ScrollDirection::Left => (-3, 0),
                    ScrollDirection::Right => (3, 0),
                };
                Some(Request::Scroll { point, dx, dy })
            }
            PanelAction::OpenPanel { panel } => match panel {
                BuiltInPanel::Keyboard => self.selected(Item::Keyboard),
                BuiltInPanel::Mouse => self.selected(Item::MousePanel),
                BuiltInPanel::Point => self.selected(Item::PointScan),
                BuiltInPanel::Switchify => self.selected(Item::Switchify),
                BuiltInPanel::Menu(Kind::Home | Kind::More) => {
                    self.open_home();
                    None
                }
                BuiltInPanel::Menu(kind) => self.selected(Item::Group(kind)),
            },
            PanelAction::OpenCustomPanel { id } => {
                if let Some(panel) = self.panels.iter().find(|panel| panel.id == id) {
                    let next = Menu::custom(panel, self.point.config.menu_scan);
                    self.push_menu(next);
                } else {
                    self.error = Some("That panel no longer exists.".into());
                }
                None
            }
            PanelAction::Pause => {
                if builtin {
                    return self.selected(Item::Pause);
                }
                self.menu.suspend();
                None
            }
            PanelAction::Reverse => {
                self.menu.handle(Action::Reverse);
                None
            }
            PanelAction::Display { next } if builtin => self.selected(Item::Display(next)),
            PanelAction::Display { next } => {
                self.menu.continue_after_selection();
                Some(Request::Display(next))
            }
            PanelAction::Back if builtin => self.selected(Item::Back),
            PanelAction::Back => {
                self.restore_actions();
                None
            }
            PanelAction::Close if builtin => self.selected(Item::Cancel),
            PanelAction::Close => {
                self.reset();
                None
            }
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
        self.block_hop = None;
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
        if matches!(key, Key::Extra(..) | Key::Column(..)) {
            let action = self.mouse.cell(key)?.action.clone();
            // A page opened over a held drag could never end it.
            let opens_page = matches!(
                action,
                PanelAction::OpenPanel {
                    panel: BuiltInPanel::Menu(_)
                } | PanelAction::OpenCustomPanel { .. }
                    | PanelAction::MyPanels
            );
            if opens_page && self.mouse.dragging {
                self.mouse
                    .block_actions("End drag before leaving the panel");
                return None;
            }
            self.mouse.choose(key);
            return self.extra_action(action, ExtraTarget::Mouse);
        }
        if key == Key::Home && self.mouse.dragging {
            self.mouse.block_actions("End drag before opening Home");
            return None;
        }
        if key == Key::MoreClicks {
            if self.mouse.dragging {
                self.mouse
                    .block_actions("End drag before opening More clicks");
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
            Key::Move(dx, dy) if self.mouse.blocks => {
                let (dx, dy) = self.block_delta(dx, dy);
                if self.mouse_repeat_enabled {
                    self.block_hop = Some(crate::mouse_repeat::ScanBlockHop::new(dx, dy));
                    self.stage = Stage::MouseMoving;
                }
                Some(Request::MouseMove { dx, dy })
            }
            Key::Move(dx, dy) => {
                // A hop left behind by a page opened mid-repeat never resumes.
                self.block_hop = None;
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
            Key::Blocks => Some(Request::MouseMovementToggle),
            Key::Monitor(dx, dy) => Some(Request::MouseMonitor(dx, dy)),
            Key::Keyboard => {
                self.return_to_mouse = true;
                self.mouse.dragging = false;
                self.stage = Stage::KeyboardOpening;
                self.keyboard = self.new_keyboard();
                Some(Request::OpenKeyboard)
            }
            Key::Close => {
                self.reset();
                Some(Request::Close)
            }
            Key::Home => {
                self.open_home();
                None
            }
            // Back and each click return to the panel; the clicks land at the pointer.
            Key::MoreClicks => {
                self.mouse_actions_open = true;
                self.parent_menu.clear();
                self.source = self
                    .pointer
                    .map_or((0, 0), |(x, y)| (x.round() as i32, y.round() as i32));
                self.open(Kind::Mouse);
                None
            }
            Key::Radar
            | Key::More
            | Key::Movement
            | Key::Dock
            | Key::Position(_)
            | Key::Back
            | Key::Extra(..)
            | Key::Column(..) => None,
        }
    }
    /// A tile the user added to the mouse panel or the keyboard was chosen.
    /// It acts as the same tile does on one of the user's panels; a way out
    /// leaves the panel as its own Close key does.
    fn extra_action(&mut self, action: PanelAction, origin: ExtraTarget) -> Option<Request> {
        match action {
            PanelAction::Key { keys } => Some(Request::Chord(keys)),
            PanelAction::Text { text } => Some(Request::Text(text)),
            PanelAction::Command { command } => {
                let point = if command.at_point() {
                    self.pointer_point()?
                } else {
                    self.source
                };
                Some(Request::Command { command, point })
            }
            PanelAction::Setting { setting } => Some(Request::Setting(setting)),
            PanelAction::Click {
                button: MouseButton::Middle,
                ..
            } => {
                let point = self.pointer_point()?;
                Some(Request::Command {
                    command: crate::scan_menu::Command::MiddleClick,
                    point,
                })
            }
            PanelAction::Click { button, count } => Some(Request::MouseClick {
                right: button == MouseButton::Right,
                count,
            }),
            PanelAction::Scroll { direction } => {
                let point = self.pointer_point()?;
                let (dx, dy) = match direction {
                    ScrollDirection::Up => (0, 3),
                    ScrollDirection::Down => (0, -3),
                    ScrollDirection::Left => (-3, 0),
                    ScrollDirection::Right => (3, 0),
                };
                Some(Request::Scroll { point, dx, dy })
            }
            PanelAction::Display { next } => Some(Request::Display(next)),
            PanelAction::OpenPanel { panel } => match panel {
                // The keyboard is already open; reopening it would forget
                // where it returns to.
                BuiltInPanel::Keyboard if origin == ExtraTarget::Keyboard => {
                    self.keyboard.cancel_activation();
                    None
                }
                BuiltInPanel::Keyboard => self.selected(Item::Keyboard),
                BuiltInPanel::Mouse => self.open_mouse(true),
                BuiltInPanel::Point => self.open_point(),
                BuiltInPanel::Switchify => self.selected(Item::Switchify),
                BuiltInPanel::Menu(Kind::Home | Kind::More) => {
                    self.open_home();
                    None
                }
                BuiltInPanel::Menu(kind) => {
                    self.leave_for_page(origin);
                    self.open(kind);
                    None
                }
            },
            PanelAction::OpenCustomPanel { id } => {
                if !self.panels.iter().any(|panel| panel.id == id) {
                    match origin {
                        ExtraTarget::Mouse => {
                            self.mouse.block_actions("That panel no longer exists")
                        }
                        ExtraTarget::Keyboard => self.keyboard.cancel_activation(),
                    }
                    return None;
                }
                self.leave_for_page(origin);
                self.open_custom(&id);
                None
            }
            PanelAction::MyPanels => {
                self.leave_for_page(origin);
                self.menu = Menu::panel_list(&self.panels, self.point.config.menu_scan);
                self.stage = Stage::Menu;
                None
            }
            // The keyboard pauses and reverses itself.
            PanelAction::Pause => {
                if origin == ExtraTarget::Mouse {
                    self.mouse.pause();
                }
                None
            }
            PanelAction::Reverse => {
                if origin == ExtraTarget::Mouse {
                    self.mouse.reverse();
                }
                None
            }
            PanelAction::Back | PanelAction::Close => match origin {
                ExtraTarget::Mouse => {
                    self.reset();
                    Some(Request::Close)
                }
                ExtraTarget::Keyboard => {
                    self.keyboard_closed();
                    None
                }
            },
        }
    }
    /// Leaves the mouse panel or the keyboard for a page of tiles. From the
    /// mouse panel, Back on that page returns to it; from the keyboard, Back
    /// goes Home.
    fn leave_for_page(&mut self, origin: ExtraTarget) {
        self.parent_menu.clear();
        self.pending = None;
        self.error = None;
        self.elapsed = 0;
        self.move_repeat = None;
        self.block_hop = None;
        self.scroll_repeat = None;
        self.radar = None;
        match origin {
            ExtraTarget::Mouse => {
                self.mouse_actions_open = true;
                self.mouse.dragging = false;
                self.source = self
                    .pointer
                    .map_or((0, 0), |(x, y)| (x.round() as i32, y.round() as i32));
            }
            ExtraTarget::Keyboard => {
                self.mouse_actions_open = false;
                self.return_to_mouse = false;
                self.return_to_home = false;
                self.parent_menu.push(PageRef::Kind(Kind::Home));
                self.source = (0, 0);
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
            Some(crate::scan_keyboard::Output::Tile(key)) => {
                let action = self.keyboard.cell(key)?.action.clone();
                let request = self.extra_action(action, ExtraTarget::Keyboard);
                // Nothing to wait for: the keyboard scans on.
                if request.is_none() && self.stage == Stage::Keyboard {
                    self.keyboard.cancel_activation();
                    self.error = None;
                }
                request
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
        self.mouse.show_movement();
        self.parent_menu.clear();
        self.mouse_actions_open = false;
        self.stage = Stage::Mouse;
        self.mouse.opened();
    }
    /// Where Close keyboard goes. The keyboard shows it on the key, and
    /// `keyboard_closed` goes there.
    fn keyboard_close_to(&self) -> crate::scan_keyboard::CloseTo {
        use crate::scan_keyboard::CloseTo;
        if self.return_to_mouse {
            CloseTo::MousePanel
        } else if self.return_to_home {
            CloseTo::Home
        } else if self.point.config.control_mode == crate::point_scan::ControlMode::Mouse {
            CloseTo::MousePanel
        } else {
            CloseTo::PointScanning
        }
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
            // Switches and tiles saved before More actions was retired open Home.
            Item::Group(Kind::More) => self.open_home(),
            Item::Group(kind) => {
                let next = self.build(kind);
                self.push_menu(next);
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
                self.push_menu(next);
                if item == Item::Drag {
                    self.stage = Stage::Destination;
                    self.point.start();
                }
            }
            Item::NewPoint => self.new_point(),
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
            Item::Panels => {
                let next = Menu::panel_list(&self.panels, self.point.config.menu_scan);
                self.push_menu(next);
            }
            Item::OpenCustom(index) => {
                // By id, not position: the list may have changed since it opened.
                let found = self
                    .menu
                    .listed_panel_id(index)
                    .and_then(|id| self.panels.iter().find(|panel| panel.id == id));
                match found {
                    Some(panel) => {
                        let next = Menu::custom(panel, self.point.config.menu_scan);
                        self.push_menu(next);
                    }
                    None => self.error = Some("That panel no longer exists.".into()),
                }
            }
            Item::Custom(index) => return self.custom_selected(index),
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
            // A page opened from the panel, such as More clicks, gives way to
            // the Movement page; a failure on the panel's own pages stays put.
            if self.mouse_actions_open {
                self.mouse.show_movement();
            }
            self.stage = Stage::Mouse;
            self.mouse_actions_open = false;
            self.parent_menu.clear();
            self.move_repeat = None;
            self.block_hop = None;
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
        self.open(if home { Kind::Home } else { Kind::Actions });
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
            let close_to = self.keyboard_close_to();
            self.keyboard.set_close_to(close_to);
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
        self.block_hop = None;
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
        self.block_hop = None;
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
        if self.end_key_repeat() && !action.opens_panel() {
            return None;
        }
        self.key_repeat = None;
        let target = match &action {
            Action::OpenMenu(menu) => Some(TrialTarget::Menu { menu: *menu }),
            Action::OpenCustomPanel(id) => Some(TrialTarget::Panel { id: id.clone() }),
            _ => None,
        };
        if let Some(target) = target {
            return match self.open_for_trial(&target) {
                Ok(request) => request,
                Err(error) => {
                    self.error = Some(error);
                    None
                }
            };
        }
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
                        let (target, menu) = selection_policy(
                            point,
                            self.point.config.menu_scan,
                            !self.panels.is_empty(),
                        );
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
        if self.stage == Stage::MouseMoving && self.block_hop.is_some() {
            // Blocks hop at the chosen block speed, or by default at the pace
            // the panel scans, so a switch user has as long to stop the
            // pointer as to choose a tile.
            let interval = match self.mouse_block_interval_ms {
                0 => self.point.config.mouse_scan.interval_ms,
                ms => u64::from(ms),
            };
            if let Some(hop) = self.block_hop.as_mut() {
                if hop.advance(ms, interval) {
                    let (dx, dy) = hop.delta();
                    self.pending = Some(Request::MouseMove { dx, dy });
                }
            }
        } else if self.stage == Stage::MouseMoving {
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
                    .map(|repeat| repeat.stroke.key),
            ),
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling => self.mouse.frame(
                self.mouse_area,
                self.point.units_per_logical_pixel,
                self.point.config.mouse_scan.color,
                matches!(self.stage, Stage::MouseMoving | Stage::MouseScrolling),
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
        if let Some(sketch) = &mut frame.sketch {
            sketch.color = frame.color.rgb();
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
mod tests;
