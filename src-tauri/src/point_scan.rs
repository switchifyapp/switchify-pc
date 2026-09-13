//! Android point scanning, with desktop coordinates and no OS input in the engine.
use crate::scan_tree::{Navigator, Node, Selection};
use crate::scanning::{
    Action, Frame, FrameLabel, Interval, Rect, SwitchSettings, Technique, MAX_SCAN_CYCLES, TICK_MS,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Line,
    Grid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub mode: Mode,
    pub automatic: bool,
    pub speed: usize,
    pub grid_size: usize,
    pub block_interval_ms: u64,
    pub select_key: String,
    pub next_key: String,
    pub back_key: String,
    pub pause_key: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Line,
            automatic: true,
            speed: 2,
            grid_size: 4,
            block_interval_ms: 1000,
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
            automatic: self.automatic,
            select_key: self.select_key.clone(),
            next_key: self.next_key.clone(),
            back_key: self.back_key.clone(),
            pause_key: self.pause_key.clone(),
        }
    }
    pub fn point(&self) -> PointSettings {
        PointSettings {
            mode: self.mode,
            speed: self.speed,
            grid_size: self.grid_size,
            block_interval_ms: self.block_interval_ms,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.switches().validate()?;
        self.point().validate()
    }
}
#[derive(Clone)]
pub struct PointSettings {
    pub mode: Mode,
    pub speed: usize,
    pub grid_size: usize,
    pub block_interval_ms: u64,
}
impl PointSettings {
    fn validate(&self) -> Result<(), String> {
        if self.speed > 4
            || !(2..=10).contains(&self.grid_size)
            || !(250..=5000).contains(&self.block_interval_ms)
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
    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.region = self.screen;
        self.x = self.screen.x;
        self.y = self.screen.y;
        self.grid.reset();
        self.direction = 1.0;
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
                                self.x = self.region.x;
                                self.y = self.region.y;
                                self.phase = Phase::X;
                            }
                            Selection::Entered | Selection::Escaped => self.sync_grid_phase(),
                            Selection::None => {}
                        }
                        self.direction = 1.0;
                    }
                    Phase::X => {
                        self.phase = Phase::Y;
                        self.direction = 1.0;
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
            Action::Pause | Action::Stop | Action::Cancel => {}
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
        let t = 2.0 * self.units_per_logical_pixel;
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
        Frame {
            strips: self.lines(),
            tiles: vec![],
            label: (self.phase == Phase::RowEscape).then(|| {
                let scale = self.units_per_logical_pixel;
                let width = (360.0 * scale).min(self.screen.width);
                let height = (64.0 * scale).min(self.screen.height);
                FrameLabel {
                    text: "Back to rows".into(),
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
        self.cycles >= MAX_SCAN_CYCLES
    }
}

#[cfg(test)]
mod tests {
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
            assert!(frame
                .label_for_prompt(gestures.prompt(1001).is_some())
                .is_some());
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
    fn existing_flat_settings_round_trip_without_schema_changes() {
        let json = serde_json::json!({"mode":"grid","automatic":false,"speed":4,"gridSize":7,"blockIntervalMs":1500,"selectKey":"F1","nextKey":"F2","backKey":"F3","pauseKey":"F4"});
        let config: Config = serde_json::from_value(json.clone()).unwrap();
        config.validate().unwrap();
        assert_eq!(serde_json::to_value(&config).unwrap(), json);
        assert!(!config.switches().automatic);
        assert_eq!(config.point().grid_size, 7);
        let view = crate::scanning_runtime::View {
            config,
            enabled: true,
            phase: Phase::Cell,
            paused: true,
            message: "Ready".to_string(),
            supported: true,
        };
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            serde_json::json!({"config":json,"enabled":true,"phase":"cell","paused":true,"message":"Ready","supported":true})
        );
    }
    #[test]
    fn legacy_empty_config_and_validation() {
        let c: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(c, Config::default());
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
}
