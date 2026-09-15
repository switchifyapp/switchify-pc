//! Point selection policy, menu navigation and execution requests. No OS input.
use crate::{
    point_scan::{Engine, PointSettings},
    scan_menu::{outline, Item, Kind, Menu},
    scanning::{Action, Frame, FrameLabel, Rect, Technique},
};
use serde::Serialize;
pub type Point = (i32, i32);
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
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
/// Future auto-selection policy can return this same request after its countdown.
pub fn default_click(point: Point) -> Request {
    Request::Click {
        point,
        right: false,
        count: 1,
    }
}
fn selection_policy(point: Point, period: u64) -> (Point, Menu) {
    (point, Menu::new(Kind::Actions, period))
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
    Menu,
    MenuSuspended,
    DragDestination,
    DragConfirmation,
    Executing,
}
#[derive(PartialEq)]
enum Stage {
    Idle,
    Point,
    Menu,
    Destination,
    Executing,
}
pub struct Workflow {
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
        let period = config.block_interval_ms;
        Ok(Self {
            point: Engine::new(config, screen, scale)?,
            stage: Stage::Idle,
            menu: Menu::new(Kind::Actions, period),
            parent_menu: vec![],
            source: (0, 0),
            destination: (0, 0),
            elapsed: 0,
            pending: None,
            error: None,
        })
    }
    pub fn apply_config(&mut self, config: PointSettings, restart: bool) {
        self.point.config = config;
        self.menu.set_period(self.point.config.block_interval_ms);
        for menu in &mut self.parent_menu {
            menu.set_period(self.point.config.block_interval_ms);
        }
        if restart {
            self.parent_menu.clear();
            self.stage = Stage::Point;
            self.point.start();
        }
    }
    fn open(&mut self, kind: Kind) {
        self.menu = Menu::new(kind, self.point.config.block_interval_ms);
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
    fn selected(&mut self, item: Item) -> Option<Request> {
        match item {
            Item::More | Item::Group(_) => {
                let kind = if let Item::Group(kind) = item {
                    kind
                } else {
                    Kind::More
                };
                let next = Menu::new(kind, self.point.config.block_interval_ms);
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
            Item::Pause => self.menu.suspended = true,
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
                let next = Menu::new(Kind::Scroll, self.point.config.block_interval_ms);
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
            Item::Cancel => self.stage = Stage::Idle,
            Item::Up | Item::Down | Item::Left | Item::Right => {
                self.menu.restart_interval();
                let (dx, dy) = match item {
                    Item::Up => (0, -3),
                    Item::Down => (0, 3),
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
        self.pending = None;
        self.stage = Stage::Menu;
        self.menu = Menu::new(Kind::Actions, self.point.config.block_interval_ms);
        self.parent_menu.clear();
        self.menu.suspended = true;
        self.error = Some(message);
    }
    fn start(&mut self) {
        self.reset();
        self.stage = Stage::Point;
        self.point.start();
    }
    fn reset(&mut self) {
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
    fn pausable(&self) -> bool {
        self.stage != Stage::Executing
    }
    fn handle(&mut self, action: Action) -> Option<Request> {
        match self.stage {
            Stage::Point | Stage::Destination => {
                if let Some(point) = self.point.handle(action) {
                    if self.stage == Stage::Destination {
                        self.destination = point;
                        self.open(Kind::ConfirmDrag);
                    } else {
                        let (target, menu) =
                            selection_policy(point, self.point.config.block_interval_ms);
                        self.source = target;
                        self.menu = menu;
                        self.stage = Stage::Menu;
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
            Stage::Idle | Stage::Executing => {}
        }
        None
    }
    fn advance(&mut self, ms: u64) {
        match self.stage {
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
    fn update(&mut self, ms: u64, advancing: bool) {
        if self.stage == Stage::Executing {
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
        } else if advancing {
            self.advance(ms);
        }
    }
    fn take_selection(&mut self) -> Option<Request> {
        self.pending.take()
    }
    fn phase(&self) -> Phase {
        match self.stage {
            Stage::Idle => Phase::default(),
            Stage::Point => Phase::Point(self.point.phase()),
            Stage::Destination => Phase::Workflow(WorkflowPhase::DragDestination),
            Stage::Executing => Phase::Workflow(WorkflowPhase::Executing),
            Stage::Menu => Phase::Workflow(if self.menu.suspended {
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
        frame.color = self.point.config.scanner_color;
        for tile in &mut frame.tiles {
            tile.color = frame.color;
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point_scan::Config, scanning::Session};
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
        assert!(!w.menu.suspended);
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
            assert_eq!(s.frame().tiles.len(), 8);
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
                dy: 3
            })
        );
        assert_eq!(
            s.action(Action::Select),
            Some(Request::Scroll {
                point: (-1000, 20),
                dx: 0,
                dy: 3
            })
        );
        s.action(Action::Next);
        s.action(Action::Select);
        s.action(Action::Next);
        s.action(Action::Next);
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
        for _ in 0..9 {
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
        assert_eq!(choose(&mut s, 2, 0), None);
        assert_eq!(
            s.technique.phase(),
            Phase::Point(crate::point_scan::Phase::X)
        );
        s.action(Action::Select);
        s.action(Action::Select);
        assert_eq!(choose(&mut s, 2, 1), None);
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
        assert_eq!(choose(&mut s, 0, 2), None);
        assert_eq!(s.technique.menu.kind, Kind::Actions);
        assert_eq!(s.take_selection(), None);
    }
}
