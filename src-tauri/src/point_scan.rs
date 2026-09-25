//! Mobile point scanning, with desktop coordinates and no OS input in the engine.
use crate::scan_tree::{Navigator, Node, Selection};
use crate::scanning::{
    Action, Frame, FrameLabel, Interval, Rect, SwitchSettings, Technique, TICK_MS,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Line,
    Grid,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlMode {
    #[default]
    Point,
    Mouse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub control_mode: ControlMode,
    #[serde(deserialize_with = "crate::scan_preferences::deserialize_preferences")]
    pub scan_preferences: crate::scan_preferences::Preferences,
    pub word_prediction: bool,
    /// Retained for saved-settings compatibility; the model is always used
    /// when `word_prediction` is enabled.
    pub enhanced_word_prediction: bool,
    pub keyboard_wait_after_typing: bool,
    /// Moves the keyboard or mouse panel away while the pointer is over it.
    pub panel_avoids_pointer: bool,
    pub scanner_color: crate::scanning::ScannerColor,
    pub mode: Mode,
    pub automatic: bool,
    pub speed: usize,
    pub grid_size: usize,
    pub block_interval_ms: u64,
    pub auto_select_enabled: bool,
    pub auto_select_delay_ms: u64,
    pub select_key: String,
    pub next_key: String,
    pub back_key: String,
    pub pause_key: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            control_mode: ControlMode::Point,
            scan_preferences: Default::default(),
            word_prediction: true,
            enhanced_word_prediction: false,
            keyboard_wait_after_typing: false,
            panel_avoids_pointer: false,
            scanner_color: Default::default(),
            mode: Mode::Line,
            automatic: true,
            speed: 2,
            grid_size: 4,
            block_interval_ms: 1000,
            auto_select_enabled: false,
            auto_select_delay_ms: 1000,
            select_key: "Space".into(),
            next_key: "Enter".into(),
            back_key: "Backspace".into(),
            pause_key: "F8".into(),
        }
    }
}
impl Config {
    pub fn switches(&self) -> SwitchSettings {
        SwitchSettings {
            automatic: [
                crate::scan_preferences::Area::Point,
                crate::scan_preferences::Area::Menu,
                crate::scan_preferences::Area::Keyboard,
                crate::scan_preferences::Area::Mouse,
            ]
            .into_iter()
            .all(|area| self.resolved(area).automatic),
            select_key: self.select_key.clone(),
            next_key: self.next_key.clone(),
            back_key: self.back_key.clone(),
            pause_key: self.pause_key.clone(),
        }
    }
    pub fn resolved(
        &self,
        area: crate::scan_preferences::Area,
    ) -> crate::scan_preferences::Resolved {
        let mut resolved = self.scan_preferences.resolve(
            area,
            self.automatic,
            self.block_interval_ms,
            self.scanner_color,
        );
        if area == crate::scan_preferences::Area::Mouse
            && self.scan_preferences.mouse.automatic.is_none()
        {
            resolved.automatic = self
                .resolved(crate::scan_preferences::Area::Keyboard)
                .automatic;
        }
        resolved
    }
    pub fn point(&self) -> PointSettings {
        PointSettings {
            control_mode: self.control_mode,
            scan: self.resolved(crate::scan_preferences::Area::Point),
            menu_scan: self.resolved(crate::scan_preferences::Area::Menu),
            keyboard_scan: self.resolved(crate::scan_preferences::Area::Keyboard),
            mouse_scan: self.resolved(crate::scan_preferences::Area::Mouse),
            word_prediction: self.word_prediction,
            keyboard_wait_after_typing: self.keyboard_wait_after_typing,
            panel_avoids_pointer: self.panel_avoids_pointer,
            scanner_color: self.resolved(crate::scan_preferences::Area::Point).color,
            mode: self.mode,
            speed: self.speed,
            grid_size: self.grid_size,
            block_interval_ms: self
                .resolved(crate::scan_preferences::Area::Point)
                .interval_ms,
            auto_select_enabled: self.auto_select_enabled,
            auto_select_delay_ms: self.auto_select_delay_ms,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.switches().validate()?;
        self.point().validate()
    }
}
#[derive(Clone)]
pub struct PointSettings {
    pub control_mode: ControlMode,
    pub scan: crate::scan_preferences::Resolved,
    pub menu_scan: crate::scan_preferences::Resolved,
    pub keyboard_scan: crate::scan_preferences::Resolved,
    pub mouse_scan: crate::scan_preferences::Resolved,
    pub word_prediction: bool,
    pub keyboard_wait_after_typing: bool,
    pub panel_avoids_pointer: bool,
    pub scanner_color: crate::scanning::ScannerColor,
    pub mode: Mode,
    pub speed: usize,
    pub grid_size: usize,
    pub block_interval_ms: u64,
    pub auto_select_enabled: bool,
    pub auto_select_delay_ms: u64,
}
impl PointSettings {
    fn validate(&self) -> Result<(), String> {
        if self.speed > 4
            || !(2..=10).contains(&self.grid_size)
            || !(100..=10000).contains(&self.block_interval_ms)
            || !(100..=100_000).contains(&self.auto_select_delay_ms)
            || !self.auto_select_delay_ms.is_multiple_of(100)
        {
            return Err("Point scan speed, grid size, or interval is invalid.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    #[default]
    Idle,
    Row,
    Cell,
    RowEscape,
    X,
    Y,
}
pub struct Engine {
    pub config: PointSettings,
    pub screen: Rect,
    pub region: Rect,
    pub phase: Phase,
    pub x: f64,
    pub y: f64,
    grid: Navigator<(usize, usize)>,
    pub units_per_logical_pixel: f64,
    direction: f64,
    block_elapsed: Interval,
    /// Full automatic passes of the current phase since it began.
    cycles: usize,
}
impl Engine {
    pub fn new(
        config: PointSettings,
        screen: Rect,
        units_per_logical_pixel: f64,
    ) -> Result<Self, String> {
        config.validate()?;
        if !screen.valid() || !units_per_logical_pixel.is_finite() || units_per_logical_pixel <= 0.0
        {
            return Err("The scanning display has invalid geometry.".into());
        }
        let grid = Navigator::new(
            (0..config.grid_size)
                .map(|row| {
                    Node::Branch(
                        (0..config.grid_size)
                            .map(|cell| Node::Leaf((row, cell)))
                            .collect(),
                    )
                })
                .collect(),
        );
        Ok(Self {
            config,
            screen,
            region: screen,
            phase: Phase::Idle,
            x: screen.x,
            y: screen.y,
            grid,
            direction: 1.0,
            block_elapsed: Interval::default(),
            cycles: 0,
            units_per_logical_pixel,
        })
    }
    fn initial_direction(&self) -> f64 {
        if self.config.scan.direction == crate::scan_preferences::Direction::Forward {
            1.0
        } else {
            -1.0
        }
    }
    fn reset_line_origin(&mut self) {
        self.x = self.region.x;
        self.y = self.region.y;
        if self.initial_direction() < 0.0 {
            self.x += self.region.width - 1.0;
            self.y += self.region.height - 1.0;
        }
    }
    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.region = self.screen;
        self.reset_line_origin();
        self.grid.reset();
        if self.initial_direction() < 0.0 {
            self.grid.start_at_end();
        }
        self.direction = self.initial_direction();
        self.block_elapsed.reset();
        self.cycles = 0;
    }
    fn select_action(&mut self, action: Action) -> Option<(i32, i32)> {
        match action {
            Action::Select => {
                self.block_elapsed.reset();
                self.cycles = 0;
                match self.phase {
                    Phase::Row | Phase::Cell | Phase::RowEscape => {
                        match self.grid.select() {
                            Selection::Leaf(_) => {
                                self.region = self.cell_rect();
                                self.reset_line_origin();
                                self.phase = Phase::X;
                            }
                            Selection::Entered => {
                                if self.initial_direction() < 0.0 {
                                    self.grid.start_at_end();
                                }
                                self.sync_grid_phase();
                            }
                            Selection::Escaped => self.sync_grid_phase(),
                            Selection::None => {}
                        }
                        self.direction = self.initial_direction();
                    }
                    Phase::X => {
                        self.phase = Phase::Y;
                        self.direction = self.initial_direction();
                    }
                    Phase::Y => {
                        let point = (
                            self.x
                                .round()
                                .clamp(self.region.x, self.region.x + self.region.width - 1.0)
                                as i32,
                            self.y
                                .round()
                                .clamp(self.region.y, self.region.y + self.region.height - 1.0)
                                as i32,
                        );
                        return Some(point);
                    }
                    Phase::Idle => {}
                }
            }
            Action::Next | Action::Back => {
                self.block_elapsed.reset();
                self.direction = if action == Action::Next { 1.0 } else { -1.0 };
                self.step(TICK_MS);
            }
            Action::Reverse => self.direction = -self.direction,
            Action::Pause
            | Action::Stop
            | Action::Cancel
            | Action::OpenKeyboard
            | Action::OpenPoint
            | Action::OpenMouse => {}
        }
        None
    }
    fn advance_time(&mut self, elapsed_ms: u64) {
        if matches!(self.phase, Phase::Row | Phase::Cell | Phase::RowEscape)
            && !self
                .block_elapsed
                .elapsed(elapsed_ms, self.config.block_interval_ms)
        {
            return;
        }
        if self.step(elapsed_ms) {
            self.cycles += 1;
        }
    }
    /// Moves the current phase and reports whether it wrapped past an edge.
    /// Manual Next and Back call this too, but only automatic passes count
    /// towards the cycle limit: a user who is stepping is not unattended.
    fn step(&mut self, elapsed_ms: u64) -> bool {
        let amount = ([45.0, 75.0, 120.0, 180.0, 270.0][self.config.speed]
            * self.units_per_logical_pixel
            * elapsed_ms as f64
            / 1000.0)
            .max(1.0);
        let forward = self.direction > 0.0;
        match self.phase {
            Phase::Row | Phase::Cell | Phase::RowEscape => {
                let wrapped = self.grid.step(forward);
                self.sync_grid_phase();
                wrapped
            }
            Phase::X => {
                let (x, wrapped) = advance(
                    self.x,
                    self.region.x,
                    self.region.width,
                    self.direction * amount,
                );
                self.x = x;
                wrapped
            }
            Phase::Y => {
                let (y, wrapped) = advance(
                    self.y,
                    self.region.y,
                    self.region.height,
                    self.direction * amount,
                );
                self.y = y;
                wrapped
            }
            Phase::Idle => false,
        }
    }
    fn sync_grid_phase(&mut self) {
        self.phase = if self.grid.escaping() {
            Phase::RowEscape
        } else if self.grid.path().is_empty() {
            Phase::Row
        } else {
            Phase::Cell
        };
    }
    pub fn row_rect(&self) -> Rect {
        Rect {
            x: self.screen.x,
            y: self.screen.y
                + self.screen.height
                    * self
                        .grid
                        .path()
                        .first()
                        .copied()
                        .unwrap_or(self.grid.index()) as f64
                    / self.config.grid_size as f64,
            width: self.screen.width,
            height: self.screen.height / self.config.grid_size as f64,
        }
    }
    pub fn cell_rect(&self) -> Rect {
        let row = self.row_rect();
        Rect {
            x: row.x + row.width * self.grid.index() as f64 / self.config.grid_size as f64,
            width: row.width / self.config.grid_size as f64,
            ..row
        }
    }
    /// Thin rectangles let both native hosts render without a full-screen bitmap.
    pub fn lines(&self) -> Vec<Rect> {
        let t = 2.0 * self.units_per_logical_pixel * self.config.scan.thickness.scale();
        let mut result = vec![];
        match self.phase {
            Phase::Idle => return result,
            Phase::Row | Phase::Cell | Phase::RowEscape => {
                let n = self.config.grid_size as f64;
                for i in 0..=self.config.grid_size {
                    result.push(Rect {
                        x: (self.screen.x + self.screen.width * i as f64 / n)
                            .min(self.screen.x + self.screen.width - t),
                        y: self.screen.y,
                        width: t,
                        height: self.screen.height,
                    });
                    result.push(Rect {
                        x: self.screen.x,
                        y: (self.screen.y + self.screen.height * i as f64 / n)
                            .min(self.screen.y + self.screen.height - t),
                        width: self.screen.width,
                        height: t,
                    });
                }
                let r = if matches!(self.phase, Phase::Row | Phase::RowEscape) {
                    self.row_rect()
                } else {
                    self.cell_rect()
                };
                result.extend(outline(r, t * 2.0));
            }
            Phase::X | Phase::Y => {
                if self.config.mode == Mode::Grid {
                    result.extend(outline(self.region, t));
                }
                result.push(Rect {
                    x: self.x.min(self.region.x + self.region.width - t),
                    y: self.region.y,
                    width: t,
                    height: self.region.height,
                });
                if self.phase == Phase::Y {
                    result.push(Rect {
                        x: self.region.x,
                        y: self.y.min(self.region.y + self.region.height - t),
                        width: self.region.width,
                        height: t,
                    });
                }
            }
        }
        result
    }
}
fn outline(r: Rect, t: f64) -> [Rect; 4] {
    let t = t.min(r.width).min(r.height);
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
/// Moves along one axis and reports whether the move wrapped past an edge. The
/// flag comes from the overflow itself, so a step of exactly the region's
/// length still counts as a completed pass even though it lands where it began.
fn advance(value: f64, start: f64, length: f64, delta: f64) -> (f64, bool) {
    let next = value + delta;
    if next > start + length - 1.0 {
        (start, true)
    } else if next < start {
        (start + length - 1.0, true)
    } else {
        (next, false)
    }
}

impl Technique for Engine {
    type Selection = (i32, i32);
    type Phase = Phase;
    fn start(&mut self) {
        self.reset();
        self.phase = if self.config.mode == Mode::Grid {
            Phase::Row
        } else {
            Phase::X
        };
    }
    fn advance(&mut self, ms: u64) {
        self.advance_time(ms);
    }
    fn handle(&mut self, action: Action) -> Option<Self::Selection> {
        self.select_action(action)
    }
    fn reset(&mut self) {
        Engine::reset(self);
    }
    fn frame(&self) -> Frame {
        let mut strips = self.lines();
        let mut grid = vec![];
        let mut fills = vec![];
        if matches!(self.phase, Phase::Row | Phase::Cell | Phase::RowEscape) {
            grid.extend(strips.drain(..2 * (self.config.grid_size + 1)));
            fills.push(if self.phase == Phase::Cell {
                self.cell_rect()
            } else {
                self.row_rect()
            });
        }
        Frame {
            color: self.config.scanner_color,
            fills,
            grid,
            strips,
            tiles: vec![],
            countdown: None,
            label: (self.phase == Phase::RowEscape).then(|| {
                let scale = self.units_per_logical_pixel;
                let width = (360.0 * scale).min(self.screen.width);
                let height = (64.0 * scale).min(self.screen.height);
                FrameLabel {
                    text: "Back to rows".into(),
                    hud: Some(crate::scanning::HudPresentation {
                        screen: self.screen,
                        scale,
                    }),
                    rect: Rect {
                        x: self.screen.x + (self.screen.width - width) / 2.0,
                        y: (self.row_rect().y + 8.0 * scale)
                            .min(self.screen.y + self.screen.height - height),
                        width,
                        height,
                    },
                    scale,
                }
            }),
        }
    }
    fn phase(&self) -> Phase {
        self.phase
    }
    fn exhausted(&self) -> bool {
        self.config.scan.exhausted(self.cycles)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn auto_scan_interval_bounds_and_legacy_values_roundtrip() {
        for interval in [100, 200, 250, 750, 1000, 10000] {
            let mut config = super::Config {
                block_interval_ms: interval,
                ..Default::default()
            };
            config.scan_preferences.point.interval_ms = Some(interval);
            config.scan_preferences.menu.interval_ms = Some(interval);
            config.scan_preferences.keyboard.interval_ms = Some(interval);
            config.validate().unwrap();
            let restored: super::Config =
                serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
            for area in [
                crate::scan_preferences::Area::Point,
                crate::scan_preferences::Area::Menu,
                crate::scan_preferences::Area::Keyboard,
            ] {
                assert_eq!(restored.resolved(area).interval_ms, interval);
            }
        }
        for interval in [0, 99, 10001, u64::MAX] {
            assert!(super::Config {
                block_interval_ms: interval,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
    }

    use super::*;
    use crate::scanning::Session;
    fn engine(config: Config) -> Session<Engine> {
        Session::new(
            Engine::new(
                config.point(),
                Rect {
                    x: -1000.0,
                    y: 50.0,
                    width: 999.0,
                    height: 701.0,
                },
                1.0,
            )
            .unwrap(),
            config.automatic,
        )
    }
    fn grid() -> Session<Engine> {
        engine(Config {
            mode: Mode::Grid,
            grid_size: 2,
            block_interval_ms: 250,
            ..Config::default()
        })
    }
    #[test]
    fn legacy_settings_enable_predictions_and_explicit_disable_roundtrips() {
        let c: Config = serde_json::from_str("{}").unwrap();
        assert!(c.word_prediction);
        let c: Config = serde_json::from_str(r#"{"wordPrediction":false}"#).unwrap();
        assert!(!c.word_prediction);
        let restored: Config = serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap();
        assert!(!restored.word_prediction);
    }
    #[test]
    fn enhanced_prediction_defaults_off_for_saved_settings_and_round_trips() {
        let c: Config = serde_json::from_str(r#"{"wordPrediction":true}"#).unwrap();
        assert!(!c.enhanced_word_prediction);
        let c: Config = serde_json::from_str(r#"{"enhancedWordPrediction":true}"#).unwrap();
        let restored: Config = serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap();
        assert!(restored.enhanced_word_prediction);
    }
    #[test]
    fn keyboard_wait_defaults_off_and_round_trips() {
        let legacy: Config = serde_json::from_str("{}").unwrap();
        assert!(!legacy.keyboard_wait_after_typing);
        for enabled in [false, true] {
            let config = Config {
                keyboard_wait_after_typing: enabled,
                ..Default::default()
            };
            let restored: Config =
                serde_json::from_value(serde_json::to_value(config).unwrap()).unwrap();
            assert_eq!(restored.keyboard_wait_after_typing, enabled);
            assert_eq!(restored.point().keyboard_wait_after_typing, enabled);
        }
    }
    #[test]
    fn panel_pointer_avoidance_defaults_off_and_round_trips() {
        let legacy: Config = serde_json::from_str("{}").unwrap();
        assert!(!legacy.panel_avoids_pointer);
        for enabled in [false, true] {
            let config = Config {
                panel_avoids_pointer: enabled,
                ..Default::default()
            };
            let restored: Config =
                serde_json::from_value(serde_json::to_value(config).unwrap()).unwrap();
            assert_eq!(restored.panel_avoids_pointer, enabled);
            assert_eq!(restored.point().panel_avoids_pointer, enabled);
        }
    }
    #[test]
    fn auto_select_delays_are_bounded_and_round_trip() {
        for delay in [100, 500, 1000, 100_000] {
            let config = Config {
                auto_select_enabled: true,
                auto_select_delay_ms: delay,
                ..Config::default()
            };
            config.validate().unwrap();
            let restored: Config =
                serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
            assert_eq!(restored, config);
        }
        for delay in [0, 99, 101, 100_100, u64::MAX] {
            assert!(Config {
                auto_select_delay_ms: delay,
                ..Config::default()
            }
            .validate()
            .is_err());
        }
    }
    #[test]
    fn grid_highlights_fill_only_the_current_target_then_clear_for_lines() {
        for scale in [1.0, 2.0] {
            let config = Config {
                mode: Mode::Grid,
                scanner_color: crate::scanning::ScannerColor::Green,
                ..Config::default()
            };
            let mut e = Engine::new(
                config.point(),
                Rect {
                    x: -1200.0,
                    y: -300.0,
                    width: 1200.0,
                    height: 900.0,
                },
                scale,
            )
            .unwrap();
            e.start();
            let row = e.frame();
            assert_eq!(row.fills, vec![e.row_rect()]);
            assert_eq!(row.color, crate::scanning::ScannerColor::Green);
            assert_eq!(row.rectangles()[0].opacity, 64);
            assert_eq!(row.rectangles()[0].role, crate::scanning::VisualRole::Fill);
            assert!(!row.grid.is_empty());
            e.handle(Action::Select);
            assert_eq!(e.frame().fills, vec![e.cell_rect()]);
            e.handle(Action::Select);
            assert!(e.frame().fills.is_empty());
            assert!(e.frame().grid.is_empty());
            assert!(!e.frame().strips.is_empty());
            e.reset();
            assert!(e.frame().rectangles().is_empty());
        }
    }
    #[test]
    fn scanner_colours_round_trip_and_reject_unknown_values() {
        use crate::scanning::ScannerColor::*;
        for color in [Red, Green, Blue, Yellow, White] {
            let config = Config {
                scanner_color: color,
                ..Config::default()
            };
            let saved = serde_json::to_string(&config).unwrap();
            assert_eq!(serde_json::from_str::<Config>(&saved).unwrap(), config);
        }
        assert!(serde_json::from_str::<Config>(r#"{"scannerColor":"unknown"}"#).is_err());
    }
    #[test]
    fn manual_entry_into_escape_discards_partial_interval() {
        for action in [Action::Next, Action::Back] {
            let mut e = grid();
            e.action(Action::Select);
            e.action(Action::Select);
            if action == Action::Next {
                e.action(Action::Next);
            }
            e.tick(249, false);
            e.action(action);
            assert_eq!(e.technique.phase, Phase::RowEscape);
            e.tick(1, false);
            assert_eq!(e.technique.phase, Phase::RowEscape);
            e.tick(248, false);
            assert_eq!(e.technique.phase, Phase::RowEscape);
            e.tick(1, false);
            assert_eq!(e.technique.phase, Phase::Cell);
        }
    }
    #[test]
    fn label_yields_only_to_an_actual_hold_prompt_and_returns_after_release() {
        let mut e = grid();
        e.action(Action::Select);
        e.action(Action::Select);
        e.action(Action::Back);
        let frame = e.frame();
        let original_label = frame.label.as_ref().unwrap().clone();
        assert!(original_label.hud.is_some());
        for hold_actions in [vec![], vec![Action::Pause]] {
            let settings = crate::switches::Settings {
                bindings: vec![crate::switches::Binding {
                    id: "test".into(),
                    name: "Switch".into(),
                    key: "Space".into(),
                    press_action: Action::Select,
                    hold_actions: hold_actions.clone(),
                }],
                hold_interval_ms: 1000,
                ..crate::switches::Settings::default()
            };
            let mut gestures = crate::switch_gestures::Gestures::default();
            gestures.pressed("test", 0, &settings);
            assert!(frame
                .label_for_prompt(gestures.prompt(999).is_some())
                .is_some());
            assert_eq!(
                frame
                    .label_for_prompt(gestures.prompt(1000).is_some())
                    .is_some(),
                hold_actions.is_empty()
            );
            gestures.released("test", 1001);
            assert_eq!(
                frame.label_for_prompt(gestures.prompt(1001).is_some()),
                Some(&original_label)
            );
            gestures.cancel();
            e.action(Action::Cancel);
            assert!(e.frame().label_for_prompt(false).is_none());
        }
    }
    #[test]
    fn row_escape_returns_to_the_same_row_without_clicking() {
        let mut e = grid();
        e.action(Action::Select);
        e.action(Action::Next);
        let row = e.technique.row_rect();
        e.action(Action::Select);
        e.action(Action::Next);
        e.action(Action::Next);
        assert_eq!(e.technique.phase, Phase::RowEscape);
        let frame = e.frame();
        assert_eq!(frame.label.as_ref().unwrap().text, "Back to rows");
        assert_eq!(frame.fills, vec![row]);
        assert!(outline(row, 4.0)
            .iter()
            .all(|strip| frame.strips.contains(strip)));
        assert_eq!(e.action(Action::Select), None);
        assert_eq!(e.technique.phase, Phase::Row);
        assert_eq!(e.technique.row_rect(), row);
        assert!(e.frame().label.is_none());
        e.action(Action::Select);
        assert_eq!(e.technique.grid.index(), 0);
        assert_eq!(e.action(Action::Select), None);
        assert_eq!(e.technique.phase, Phase::X);
    }
    #[test]
    fn escape_has_a_full_interval_and_remains_selectable_on_the_final_cycle() {
        let mut e = grid();
        e.action(Action::Select);
        e.action(Action::Select);
        for _ in 0..8 {
            e.tick(250, false);
        }
        assert_eq!(e.technique.phase, Phase::RowEscape);
        assert_eq!(e.technique.cycles, 2);
        e.tick(249, false);
        assert_eq!(e.technique.phase, Phase::RowEscape);
        assert_eq!(e.action(Action::Select), None);
        assert_eq!(e.technique.phase, Phase::Row);
        assert_eq!(e.technique.cycles, 0);
        e.tick(1, false);
        assert_eq!(e.technique.grid.index(), 0);

        e.action(Action::Select);
        for _ in 0..9 {
            e.tick(250, false);
        }
        assert!(!e.active());
        assert!(e.frame().label.is_none());
    }
    #[test]
    fn reverse_escape_ignored_wraps_and_confirming_restores_forward_scan() {
        let mut e = grid();
        e.action(Action::Select);
        e.action(Action::Select);
        e.action(Action::Reverse);
        e.tick(250, false);
        assert_eq!(e.technique.phase, Phase::RowEscape);
        e.tick(250, false);
        assert_eq!(e.technique.phase, Phase::Cell);
        assert_eq!(e.technique.grid.index(), 1);
        e.tick(250, false);
        e.tick(250, false);
        e.action(Action::Select);
        e.tick(250, false);
        assert_eq!(e.technique.grid.index(), 1);
        assert_eq!(e.technique.phase, Phase::Row);
    }
    #[test]
    fn pause_hold_manual_steps_and_cancel_preserve_escape_safety() {
        let mut e = grid();
        e.action(Action::Select);
        e.action(Action::Select);
        e.action(Action::Back);
        e.tick(250, true);
        assert_eq!(e.technique.phase, Phase::RowEscape);
        e.action(Action::Pause);
        e.tick(250, false);
        assert_eq!(e.technique.phase, Phase::RowEscape);
        e.action(Action::Pause);
        for _ in 0..30 {
            e.action(Action::Next);
        }
        assert!(e.active());
        assert_eq!(e.technique.cycles, 0);
        e.action(Action::Cancel);
        assert!(e.frame().strips.is_empty());
        assert!(e.frame().label.is_none());
        e.action(Action::Select);
        assert_eq!(e.technique.phase, Phase::Row);
        assert_eq!(e.technique.grid.index(), 0);
    }
    #[test]
    fn escape_label_stays_inside_scaled_negative_origin_display() {
        let mut e = grid();
        e.technique.units_per_logical_pixel = 2.0;
        e.action(Action::Select);
        e.action(Action::Next);
        e.action(Action::Select);
        e.action(Action::Back);
        let label = e.frame().label.unwrap();
        let screen = e.technique.screen;
        assert_eq!(label.scale, 2.0);
        assert!(label.rect.x >= screen.x && label.rect.y >= screen.y);
        assert!(label.rect.x + label.rect.width <= screen.x + screen.width);
        assert!(label.rect.y + label.rect.height <= screen.y + screen.height);
        assert_eq!(serde_json::to_value(Phase::RowEscape).unwrap(), "rowEscape");
    }
    #[test]
    fn line_selects_x_then_y_and_resets() {
        let mut e = engine(Config::default());
        assert_eq!(e.action(Action::Select), None);
        e.tick(100, false);
        let x = e.technique.x;
        e.action(Action::Select);
        e.tick(100, false);
        assert_eq!(e.technique.x, x);
        assert_eq!(e.action(Action::Select), Some((-988, 62)));
        assert_eq!(e.technique.phase, Phase::Idle);
        assert!(e.frame().strips.is_empty());
    }
    #[test]
    fn grid_uses_row_then_cell_and_covers_remainder() {
        let mut e = engine(Config {
            mode: Mode::Grid,
            grid_size: 3,
            ..Config::default()
        });
        e.action(Action::Select);
        e.action(Action::Back);
        assert_eq!(e.technique.grid.index(), 2);
        e.action(Action::Select);
        e.action(Action::Back);
        e.action(Action::Back);
        e.action(Action::Select);
        assert_eq!(e.technique.region.x + e.technique.region.width, -1.0);
        assert!((e.technique.region.y + e.technique.region.height - 751.0).abs() < 0.001);
        assert_eq!(e.technique.phase, Phase::X);
    }
    #[test]
    fn pause_resume_wrap_and_delayed_ticks() {
        let mut e = engine(Config::default());
        e.action(Action::Select);
        e.action(Action::Pause);
        e.tick(1000, false);
        assert_eq!(e.technique.x, -1000.0);
        e.action(Action::Pause);
        e.tick(10000, false);
        assert_eq!(e.technique.x, -970.0);
        e.technique.x = -1000.0;
        e.action(Action::Back);
        assert_eq!(e.technique.x, -2.0);
        e.action(Action::Cancel);
        assert_eq!(e.technique.phase, Phase::Idle);
    }
    // The 999 px line at speed 2 (120 px/s) wraps roughly every 8.3 s.
    fn run(e: &mut Session<Engine>, ms: u64) {
        for _ in 0..ms / 250 {
            e.tick(250, false);
        }
    }
    #[test]
    fn automatic_scan_stops_after_max_cycles_without_selection() {
        let mut e = engine(Config::default());
        e.action(Action::Select);
        run(&mut e, 20_000);
        assert!(e.active(), "two passes keep scanning");
        run(&mut e, 6_000);
        assert!(!e.active());
        assert_eq!(e.technique.phase, Phase::Idle);
        assert!(e.frame().strips.is_empty());
        e.action(Action::Select);
        assert_eq!(e.technique.phase, Phase::X);
    }
    #[test]
    fn each_selected_phase_starts_a_fresh_cycle_count() {
        let mut e = engine(Config::default());
        e.action(Action::Select);
        run(&mut e, 20_000);
        e.action(Action::Select);
        assert_eq!(e.technique.phase, Phase::Y);
        // The 701 px column wraps every 5.8 s, so 14 s is two passes; had the
        // X passes carried over, this would already have stopped.
        run(&mut e, 14_000);
        assert!(e.active(), "the Y phase counts its own passes");
        run(&mut e, 4_000);
        assert!(!e.active());
        let mut g = engine(Config {
            mode: Mode::Grid,
            grid_size: 2,
            block_interval_ms: 250,
            ..Config::default()
        });
        g.action(Action::Select);
        run(&mut g, 1_250);
        assert!(g.active(), "five row steps are two and a half passes");
        run(&mut g, 500);
        assert!(!g.active());
    }
    #[test]
    fn a_step_of_exactly_the_region_length_counts_as_a_pass() {
        // At speed 4 a 250 ms tick moves 67.5 units, so a 67.5-wide grid cell
        // lands back on its own left edge every tick and must still be counted.
        let mut e = Session::new(
            Engine::new(
                Config {
                    speed: 4,
                    ..Config::default()
                }
                .point(),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 67.5,
                    height: 67.5,
                },
                1.0,
            )
            .unwrap(),
            true,
        );
        e.action(Action::Select);
        e.tick(250, false);
        e.tick(250, false);
        assert!(e.active());
        e.tick(250, false);
        assert!(!e.active(), "three exact-length passes exhaust the scan");
    }
    #[test]
    fn manual_steps_never_exhaust_the_scan() {
        let mut e = engine(Config {
            automatic: false,
            ..Config::default()
        });
        e.action(Action::Select);
        for _ in 0..2000 {
            e.action(Action::Next);
        }
        assert!(e.active());
        assert_eq!(e.technique.phase, Phase::X);
    }
    #[test]
    fn manual_mode_only_moves_on_steps() {
        let mut e = engine(Config {
            automatic: false,
            ..Config::default()
        });
        e.action(Action::Select);
        e.tick(250, false);
        assert_eq!(e.technique.x, -1000.0);
        e.action(Action::Next);
        assert!(e.technique.x > -1000.0);
    }
    #[test]
    fn existing_flat_settings_gain_safe_auto_select_defaults() {
        let mut json = serde_json::json!({"mode":"grid","automatic":false,"speed":4,"gridSize":7,"blockIntervalMs":1500,"selectKey":"F1","nextKey":"F2","backKey":"F3","pauseKey":"F4"});
        let config: Config = serde_json::from_value(json.clone()).unwrap();
        config.validate().unwrap();
        assert_eq!(config.scanner_color, crate::scanning::ScannerColor::Blue);
        assert!(!config.auto_select_enabled);
        assert_eq!(config.auto_select_delay_ms, 1000);
        json["autoSelectEnabled"] = serde_json::json!(false);
        json["autoSelectDelayMs"] = serde_json::json!(1000);
        json["scannerColor"] = serde_json::json!("blue");
        json["controlMode"] = serde_json::json!("point");
        json["wordPrediction"] = serde_json::json!(true);
        json["enhancedWordPrediction"] = serde_json::json!(false);
        json["keyboardWaitAfterTyping"] = serde_json::json!(false);
        json["panelAvoidsPointer"] = serde_json::json!(false);
        json["scanPreferences"] =
            serde_json::to_value(crate::scan_preferences::Preferences::default()).unwrap();
        assert_eq!(serde_json::to_value(&config).unwrap(), json);
        assert!(!config.switches().automatic);
        assert_eq!(config.point().grid_size, 7);
        let view = crate::scanning_runtime::View {
            remote: false,
            config,
            enabled: true,
            phase: Phase::Cell,
            paused: true,
            message: "Ready".to_string(),
            supported: true,
        };
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            serde_json::json!({"config":json,"enabled":true,"phase":"cell","paused":true,"message":"Ready","supported":true,"remote":false})
        );
    }
    #[test]
    fn legacy_empty_config_and_validation() {
        let c: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.control_mode, ControlMode::Point);
        assert!(Config {
            select_key: "Escape".into(),
            ..c.clone()
        }
        .validate()
        .is_err());
        assert!(Config { speed: 5, ..c }.validate().is_err());
        assert!(Engine::new(
            Config::default().point(),
            Rect {
                x: 0.0,
                y: 0.0,
                width: f64::NAN,
                height: 10.0
            },
            1.0
        )
        .is_err());
    }
    #[test]
    fn legacy_settings_and_new_overrides_round_trip_without_changing_shared_values() {
        use crate::scan_preferences::{Area, Direction, Pattern, Thickness};
        let legacy = serde_json::json!({"automatic":false,"blockIntervalMs":1500,"scannerColor":"green","speed":4});
        let mut config: Config = serde_json::from_value(legacy).unwrap();
        for area in [Area::Point, Area::Menu, Area::Keyboard] {
            let resolved = config.resolved(area);
            assert!(!resolved.automatic);
            assert_eq!(resolved.interval_ms, 1500);
            assert_eq!(resolved.color, crate::scanning::ScannerColor::Green);
            assert_eq!(resolved.pass_limit, 3);
        }
        config.scan_preferences.keyboard.automatic = Some(true);
        config.scan_preferences.keyboard.direction = Some(Direction::Reverse);
        config.scan_preferences.keyboard.pattern = Some(Pattern::Linear);
        config.scan_preferences.keyboard.thickness = Some(Thickness::Thick);
        let stored = serde_json::to_vec(&config).unwrap();
        let restored: Config = serde_json::from_slice(&stored).unwrap();
        assert_eq!(restored, config);
        assert!(!restored.automatic);
        assert!(restored.resolved(Area::Keyboard).automatic);
        assert!(!restored.switches().automatic);
    }

    #[test]
    fn obsolete_app_override_does_not_require_manual_switches() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "automatic": false,
            "scanPreferences": {
                "point": {"automatic": true},
                "menu": {"automatic": true},
                "keyboard": {"automatic": true},
                "app": {"automatic": false}
            }
        }))
        .unwrap();
        assert!(config.switches().automatic);
        let stored = serde_json::to_value(&config).unwrap();
        assert!(stored["scanPreferences"].get("app").is_none());
        let restored: Config = serde_json::from_value(stored).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn invalid_preferences_do_not_discard_existing_point_settings() {
        for prefs in [
            serde_json::json!({"direction":"invalid"}),
            serde_json::json!({"passLimit":999,"menu":{"intervalMs":0,"passLimit":9}}),
        ] {
            let config: Config = serde_json::from_value(serde_json::json!({"automatic":false,"speed":4,"blockIntervalMs":1500,"scanPreferences":prefs})).unwrap();
            assert_eq!(config.speed, 4);
            assert!(!config.automatic);
            let options = config.resolved(crate::scan_preferences::Area::Menu);
            assert_eq!(options.pass_limit, 3);
            assert_eq!(options.interval_ms, 1500);
        }
    }

    #[test]
    fn reverse_point_scan_starts_at_the_far_edge_and_uses_its_pass_limit() {
        let mut config = Config::default();
        config.scan_preferences.direction = crate::scan_preferences::Direction::Reverse;
        config.scan_preferences.pass_limit = 1;
        let mut session = engine(config);
        session.action(Action::Select);
        let before = session.technique.x;
        assert_eq!(
            before,
            session.technique.screen.x + session.technique.screen.width - 1.0
        );
        session.tick(100, false);
        assert!(session.technique.x < before);
        for _ in 0..1000 {
            session.tick(250, false);
        }
        assert!(!session.active());
    }
}
