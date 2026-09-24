//! Point selection policy, menu navigation and execution requests. No OS input.
use crate::{
    point_scan::{Engine, PointSettings},
    scan_menu::{outline, Item, Kind, Menu},
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
    OpenKeyboard,
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
    Mouse,
    MouseSuspended,
    MouseMoving,
    MouseScrolling,
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
    Idle,
    Countdown,
    Point,
    Menu,
    Destination,
    Executing,
}
pub struct Workflow {
    keyboard: crate::scan_keyboard::Keyboard,
    keyboard_area: Rect,
    mouse: crate::scan_mouse::MousePanel,
    mouse_area: Rect,
    return_to_mouse: bool,
    move_repeat: Option<crate::mouse_repeat::ScanMoveRepeat>,
    scroll_repeat: Option<crate::mouse_repeat::ScanScrollRepeat>,
    mouse_repeat_enabled: bool,
    mouse_move_interval_ms: u32,
    mouse_scroll_interval_ms: u32,
    mouse_acceleration_ms: u32,
    point: Engine,
    stage: Stage,
    menu: Menu,
    parent_menu: Vec<Menu>,
    source: Point,
    destination: Point,
    elapsed: u64,
    pending: Option<Request>,
    error: Option<String>,
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
            .with_wait_after_typing(config.keyboard_wait_after_typing),
            keyboard_area: screen,
            mouse: crate::scan_mouse::MousePanel::new(mouse_options, 1, 100),
            mouse_area: screen,
            return_to_mouse: false,
            move_repeat: None,
            scroll_repeat: None,
            mouse_repeat_enabled: true,
            mouse_move_interval_ms: 250,
            mouse_scroll_interval_ms: 250,
            mouse_acceleration_ms: 1000,
            point: Engine::new(config, screen, scale)?,
            stage: Stage::Idle,
            menu: Menu::configured(Kind::Actions, menu_options),
            parent_menu: vec![],
            source: (0, 0),
            destination: (0, 0),
            elapsed: 0,
            pending: None,
            error: None,
        })
    }
    fn active_preferences(&self) -> crate::scan_preferences::Resolved {
        match self.stage {
            Stage::Keyboard | Stage::KeyboardOpening => self.point.config.keyboard_scan,
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling => {
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
        self.menu
            .set_period(self.point.config.menu_scan.interval_ms);
        for menu in &mut self.parent_menu {
            menu.set_period(self.point.config.menu_scan.interval_ms);
        }
        if restart {
            self.keyboard = crate::scan_keyboard::Keyboard::configured(
                cfg!(target_os = "macos"),
                self.point.config.keyboard_scan,
            )
            .with_wait_after_typing(self.point.config.keyboard_wait_after_typing);
            self.mouse = crate::scan_mouse::MousePanel::new(
                self.point.config.mouse_scan,
                1,
                self.mouse.speed_percent,
            );
            self.return_to_mouse = false;
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.parent_menu.clear();
            self.stage = Stage::Point;
            self.point.start();
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
    pub fn keyboard_open(&self) -> bool {
        matches!(self.stage, Stage::Keyboard | Stage::KeyboardOpening)
    }
    pub fn mouse_open(&self) -> bool {
        matches!(
            self.stage,
            Stage::Mouse | Stage::MouseMoving | Stage::MouseScrolling
        )
    }
    pub fn control_mode(&self) -> crate::point_scan::ControlMode {
        self.point.config.control_mode
    }
    pub fn set_mouse_area(&mut self, area: Rect, screen: Rect, scale: f64, displays: usize) {
        if self.point.screen != screen && self.stage == Stage::MouseScrolling {
            self.stage = Stage::Mouse;
            self.scroll_repeat = None;
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
        self.mouse.speed_percent = speed;
        self.mouse_acceleration_ms = acceleration_ms;
        self.mouse_move_interval_ms = move_interval_ms;
        self.mouse_scroll_interval_ms = scroll_interval_ms;
        self.mouse_repeat_enabled = repeat_enabled;
        if !repeat_enabled && matches!(self.stage, Stage::MouseMoving | Stage::MouseScrolling) {
            self.stage = Stage::Mouse;
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.pending = None;
            self.mouse.restart();
        }
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
            _ => None,
        }
    }
    pub fn foreground_changed(&mut self) {
        self.keyboard.reset_context();
    }
    pub fn prediction_enabled(&self) -> bool {
        self.point.config.word_prediction
    }
    pub fn set_keyboard_area(&mut self, area: Rect) {
        self.keyboard_area = area;
    }
    fn open(&mut self, kind: Kind) {
        self.menu = Menu::configured(kind, self.point.config.menu_scan);
        self.stage = Stage::Menu;
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
    fn open_mouse(&mut self) -> Option<Request> {
        self.point.config.control_mode = crate::point_scan::ControlMode::Mouse;
        self.stage = Stage::Mouse;
        self.move_repeat = None;
        self.scroll_repeat = None;
        self.mouse = crate::scan_mouse::MousePanel::new(
            self.point.config.mouse_scan,
            1,
            self.mouse.speed_percent,
        );
        self.pending = None;
        self.parent_menu.clear();
        self.elapsed = 0;
        self.error = None;
        self.return_to_mouse = false;
        Some(Request::OpenMouse)
    }
    fn open_point(&mut self) -> Option<Request> {
        self.point.config.control_mode = crate::point_scan::ControlMode::Point;
        self.reset();
        self.stage = Stage::Point;
        self.point.start();
        Some(Request::OpenPoint)
    }
    fn mouse_key(&mut self, key: crate::scan_mouse::Key) -> Option<Request> {
        use crate::scan_mouse::Key;
        self.mouse.choose(key);
        match key {
            Key::Move(dx, dy) => {
                let delta = (i32::from(dx) * 12, i32::from(dy) * 12);
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
                self.keyboard = crate::scan_keyboard::Keyboard::configured(
                    cfg!(target_os = "macos"),
                    self.point.config.keyboard_scan,
                )
                .with_wait_after_typing(self.point.config.keyboard_wait_after_typing);
                Some(Request::OpenKeyboard)
            }
            Key::Close => self.open_point(),
            Key::More | Key::Movement | Key::Dock => None,
        }
    }
    fn keyboard_closed(&mut self) {
        if self.return_to_mouse {
            self.return_to_mouse = false;
            self.stage = Stage::Mouse;
            self.mouse.restart();
        } else {
            self.start();
        }
    }
    fn selected(&mut self, item: Item) -> Option<Request> {
        match item {
            Item::Keyboard => {
                self.return_to_mouse = self.mouse_open();
                if self.return_to_mouse {
                    self.mouse.dragging = false;
                }
                self.stage = Stage::KeyboardOpening;
                self.keyboard = crate::scan_keyboard::Keyboard::configured(
                    cfg!(target_os = "macos"),
                    self.point.config.keyboard_scan,
                )
                .with_wait_after_typing(self.point.config.keyboard_wait_after_typing);
                self.pending = None;
                self.parent_menu.clear();
                self.elapsed = 0;
                self.error = None;
                return Some(Request::OpenKeyboard);
            }
            Item::KeyboardKey => {}
            Item::MousePanel => return self.open_mouse(),
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
                if !command.stays_open() {
                    self.stage = Stage::Idle;
                }
                return Some(Request::Command {
                    command,
                    point: self.source,
                });
            }
            Item::Setting(setting) => return Some(Request::Setting(setting)),
            Item::Display(next) => return Some(Request::Display(next)),
            Item::Pause => self.menu.suspend(),
            Item::Reverse => {
                self.menu.handle(Action::Reverse);
            }

            Item::LeftClick | Item::RightClick | Item::DoubleClick => {
                self.stage = Stage::Idle;
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
            Item::NewPoint => {
                self.parent_menu.clear();
                self.stage = Stage::Point;
                self.point.start();
            }
            Item::Cancel => self.reset(),
            Item::Up | Item::Down | Item::Left | Item::Right => {
                self.menu.restart_interval();
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
            self.keyboard.failed();
            return;
        }
        if self.mouse_open() {
            self.stage = Stage::Mouse;
            self.move_repeat = None;
            self.scroll_repeat = None;
            self.mouse.dragging = false;
            self.mouse.failed();
            self.error = None;
            let _ = message;
            return;
        }
        self.pending = None;
        self.stage = Stage::Menu;
        self.menu = Menu::configured(Kind::Actions, self.point.config.menu_scan);
        self.parent_menu.clear();
        self.menu.suspend();
        self.error = Some(message);
    }
    fn execution_succeeded(&mut self) {
        if self.stage == Stage::KeyboardOpening {
            self.stage = Stage::Keyboard;
        } else if self.stage == Stage::Keyboard {
            self.keyboard
                .succeeded_with_context(crate::prediction::keyboard_input_context());
        }
    }
    fn start(&mut self) {
        let mode = self.point.config.control_mode;
        self.reset();
        if mode == crate::point_scan::ControlMode::Mouse {
            let _ = self.open_mouse();
        } else {
            self.stage = Stage::Point;
            self.point.start();
        }
    }
    fn reset(&mut self) {
        self.keyboard = crate::scan_keyboard::Keyboard::configured(
            cfg!(target_os = "macos"),
            self.point.config.keyboard_scan,
        )
        .with_wait_after_typing(self.point.config.keyboard_wait_after_typing);
        self.mouse = crate::scan_mouse::MousePanel::new(
            self.point.config.mouse_scan,
            1,
            self.mouse.speed_percent,
        );
        self.return_to_mouse = false;
        self.move_repeat = None;
        self.scroll_repeat = None;
        self.stage = Stage::Idle;
        self.parent_menu.clear();
        self.pending = None;
        self.error = None;
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
    fn switch_pressed(&mut self) -> bool {
        if !matches!(self.stage, Stage::MouseMoving | Stage::MouseScrolling) {
            return false;
        }
        self.stage = Stage::Mouse;
        self.move_repeat = None;
        self.scroll_repeat = None;
        self.pending = None;
        self.mouse.restart();
        true
    }
    fn auto_selecting(&self) -> bool {
        self.stage == Stage::Countdown
    }
    fn pausable(&self) -> bool {
        self.stage != Stage::Executing
    }
    fn handle(&mut self, action: Action) -> Option<Request> {
        if action == Action::OpenPoint {
            return self.open_point();
        }
        if action == Action::OpenMouse {
            if self.mouse_open() {
                return None;
            }
            return self.open_mouse();
        }
        if action == Action::OpenKeyboard {
            if self.keyboard_open() {
                return None;
            }
            return self.selected(Item::Keyboard);
        }
        match self.stage {
            Stage::Keyboard => match self
                .keyboard
                .handle_with_context(action, crate::prediction::keyboard_input_context())
            {
                Some(crate::scan_keyboard::Output::Stroke(stroke)) => {
                    return Some(Request::Keyboard(stroke))
                }
                Some(crate::scan_keyboard::Output::Punctuation(punctuation)) => {
                    return Some(Request::KeyboardPunctuation(punctuation))
                }
                Some(crate::scan_keyboard::Output::Prediction { token, index }) => {
                    return Some(Request::Prediction { token, index })
                }
                Some(crate::scan_keyboard::Output::Close) => self.keyboard_closed(),
                None => {}
            },
            Stage::KeyboardOpening => {}
            Stage::Mouse => {
                if let Some(key) = self.mouse.handle(action) {
                    return self.mouse_key(key);
                }
            }
            Stage::MouseMoving | Stage::MouseScrolling => {}
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
            Stage::Point | Stage::Destination => {
                self.point.advance(ms);
                if self.point.exhausted() {
                    self.point.start();
                    if self.stage == Stage::Point {
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
                    self.stage = Stage::Idle;
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
                self.stage = Stage::Idle;
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
            Stage::Keyboard => Phase::Workflow(if self.keyboard.suspended() {
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
        let mut frame = match self.stage {
            Stage::Keyboard => self.keyboard.frame(
                self.keyboard_area,
                self.point.units_per_logical_pixel,
                self.point.config.scanner_color,
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
            Stage::Point | Stage::Destination => self.point.frame(),
            Stage::Menu => self.menu.frame(
                if self.menu.kind == Kind::ConfirmDrag {
                    self.destination
                } else {
                    self.source
                },
                self.point.screen,
                self.point.units_per_logical_pixel,
            ),
            _ => Frame::default(),
        };
        if let Some(error) = &self.error {
            frame.tiles.clear();
            if let Some(label) = frame.label.as_mut() {
                label.text = format!("{error}\nSelect to return");
                label.rect.height = 100.0 * self.point.units_per_logical_pixel;
                label.scale *= 0.65;
                label.hud = Some(crate::scanning::HudPresentation {
                    screen: self.point.screen,
                    scale: self.point.units_per_logical_pixel,
                });
            }
        }
        if matches!(self.stage, Stage::Menu | Stage::Destination) {
            let s = self.point.units_per_logical_pixel;
            frame.strips.extend(outline(
                Rect {
                    x: self.source.0 as f64 - 5.0 * s,
                    y: self.source.1 as f64 - 5.0 * s,
                    width: 10.0 * s,
                    height: 10.0 * s,
                },
                2.0 * s,
            ));
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
        frame.color = preferences.color;
        for tile in &mut frame.tiles {
            tile.color = frame.color;
            tile.thickness = preferences.thickness;
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point_scan::Config, scanning::Session};

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
            ..Config::default()
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
    fn mouse_motion_stops_on_press_and_returns_to_first_row() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut session = Session::new(
            Workflow::new(Config::default().point(), screen, 1.0).unwrap(),
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
            "Moving pointer · Press any switch to stop"
        );
        assert!(session.technique.switch_pressed());
        assert!(!session.technique.switch_pressed());
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
        let mut workflow = Workflow::new(Config::default().point(), screen, 1.0).unwrap();
        workflow.open_mouse();
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
        assert!(!workflow.switch_pressed());
    }

    #[test]
    fn scanned_scroll_repeats_at_saved_interval_and_stops_on_press() {
        let mut workflow = session(false).technique;
        workflow.open_mouse();
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
                    switch_held: false,
                },
            );
            assert_eq!(workflow.take_selection(), Some(Request::MouseScroll { dy }));
            assert!(workflow.switch_pressed());
            assert!(!workflow.switch_pressed());
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
        workflow.open_mouse();
        workflow.set_mouse_settings(100, 1000, 250, 80, false);
        assert_eq!(
            workflow.mouse_key(crate::scan_mouse::Key::Scroll(1)),
            Some(Request::MouseScroll { dy: 5 })
        );
        assert_eq!(workflow.phase(), Phase::Workflow(WorkflowPhase::Mouse));
        assert!(!workflow.switch_pressed());
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
        assert!(!workflow.switch_pressed());
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
    fn mouse_keyboard_return_and_drag_state() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let mut workflow = Workflow::new(Config::default().point(), screen, 1.0).unwrap();
        workflow.open_mouse();
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
        workflow.open_mouse();
        workflow.mouse_key(crate::scan_mouse::Key::Move(1, 0));
        assert!(matches!(
            workflow.mouse_feedback(),
            Some(crate::input::PointerFeedback::RepeatMove { .. })
        ));
        workflow.execution_failed("input failed".into());
        assert_eq!(workflow.mouse_feedback(), None);
        assert!(!workflow.switch_pressed());
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
    fn foreground_change_keeps_keyboard_layout_but_clears_context_and_modifiers() {
        let mut s = session(false);
        s.action(Action::OpenKeyboard);
        s.technique.execution_succeeded();
        let keyboard = &mut s.technique.keyboard;
        keyboard.modifiers[1] = crate::scan_keyboard::Modifier::Locked;
        keyboard.caps = true;
        keyboard.predictions(
            Some(crate::prediction::worker::Batch {
                token: 7,
                words: vec!["water".into()],
            }),
            false,
        );
        s.technique.foreground_changed();
        assert!(s.technique.keyboard_open());
        assert_eq!(
            s.technique.keyboard.modifiers,
            [crate::scan_keyboard::Modifier::Off; 4]
        );
        assert!(!s.technique.keyboard.caps);
        assert!(!s.frame().tiles.iter().any(|t| t.text == "water"));
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
            ..Default::default()
        };
        s.technique.apply_config(config.point(), false);
        s.action(Action::Select);
        s.technique.selected(Item::Keyboard);
        s.technique.execution_succeeded();
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
        assert_eq!(
            s.technique.phase(),
            Phase::Workflow(WorkflowPhase::Keyboard)
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
    fn open(s: &mut Session<Workflow>) {
        for _ in 0..3 {
            assert_eq!(s.action(Action::Select), None);
        }
        assert_eq!(s.technique.phase(), Phase::Workflow(WorkflowPhase::Menu));
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
                let tiles = s.frame().tiles;
                let first = tiles
                    .iter()
                    .find(|tile| !tile.is_panel_background())
                    .expect("menu actions");
                let rows = tiles
                    .iter()
                    .filter(|tile| !tile.is_panel_background() && tile.rect.x == first.rect.x)
                    .count();
                assert_eq!(choose(&mut s, rows - 1, 1), None, "{kind:?}");
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
            ..Default::default()
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
        assert!(session
            .frame()
            .label
            .unwrap()
            .text
            .contains("Select to return"));
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
            crate::point_scan::Config::default().point(),
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
    fn command_failure_preserves_point_and_requires_acknowledgement() {
        let mut w = Workflow::new(
            crate::point_scan::Config::default().point(),
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
        w.execution_failed("Action failed.".into());
        let label = w.frame().label.unwrap();
        assert!(label.text.contains("Select to return"));
        assert_eq!(label.hud.as_ref().unwrap().scale, 1.0);
        assert_eq!(label.hud.as_ref().unwrap().screen, w.point.screen);
        assert!(label.scale < label.hud.as_ref().unwrap().scale);
        assert!(w.handle(Action::Select).is_none());
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
            assert_eq!(s.frame().tiles.len(), 11);
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
            ..Default::default()
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
