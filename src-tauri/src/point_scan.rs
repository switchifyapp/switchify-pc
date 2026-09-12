//! Android point scanning, with desktop coordinates and no OS input in the engine.
use crate::scanning::{Action, Cycle, Frame, Interval, Rect, SwitchSettings, Technique, TICK_MS};
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
    pub row: Cycle,
    pub cell: Cycle,
    pub units_per_logical_pixel: f64,
    direction: f64,
    block_elapsed: Interval,
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
        Ok(Self {
            config,
            screen,
            region: screen,
            phase: Phase::Idle,
            x: screen.x,
            y: screen.y,
            row: Cycle::default(),
            cell: Cycle::default(),
            direction: 1.0,
            block_elapsed: Interval::default(),
            units_per_logical_pixel,
        })
    }
    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.region = self.screen;
        self.x = self.screen.x;
        self.y = self.screen.y;
        self.row.reset();
        self.cell.reset();
        self.direction = 1.0;
        self.block_elapsed.reset();
    }
    fn select_action(&mut self, action: Action) -> Option<(i32, i32)> {
        match action {
            Action::Select => {
                self.block_elapsed.reset();
                match self.phase {
                    Phase::Row => {
                        self.phase = Phase::Cell;
                        self.cell.reset();
                    }
                    Phase::Cell => {
                        self.region = self.cell_rect();
                        self.x = self.region.x;
                        self.y = self.region.y;
                        self.phase = Phase::X;
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
                self.direction = if action == Action::Next { 1.0 } else { -1.0 };
                self.step(TICK_MS);
            }
            Action::Pause | Action::Cancel => {}
        }
        None
    }
    fn advance_time(&mut self, elapsed_ms: u64) {
        if matches!(self.phase, Phase::Row | Phase::Cell)
            && !self
                .block_elapsed
                .elapsed(elapsed_ms, self.config.block_interval_ms)
        {
            return;
        }
        self.step(elapsed_ms);
    }
    fn step(&mut self, elapsed_ms: u64) {
        let amount = ([45.0, 75.0, 120.0, 180.0, 270.0][self.config.speed]
            * self.units_per_logical_pixel
            * elapsed_ms as f64
            / 1000.0)
            .max(1.0);
        match self.phase {
            Phase::Row => self.row.step(self.config.grid_size, self.direction > 0.0),
            Phase::Cell => self.cell.step(self.config.grid_size, self.direction > 0.0),
            Phase::X => {
                self.x = advance(
                    self.x,
                    self.region.x,
                    self.region.width,
                    self.direction * amount,
                )
            }
            Phase::Y => {
                self.y = advance(
                    self.y,
                    self.region.y,
                    self.region.height,
                    self.direction * amount,
                )
            }
            Phase::Idle => {}
        }
    }
    pub fn row_rect(&self) -> Rect {
        Rect {
            x: self.screen.x,
            y: self.screen.y
                + self.screen.height * self.row.index() as f64 / self.config.grid_size as f64,
            width: self.screen.width,
            height: self.screen.height / self.config.grid_size as f64,
        }
    }
    pub fn cell_rect(&self) -> Rect {
        let row = self.row_rect();
        Rect {
            x: row.x + row.width * self.cell.index() as f64 / self.config.grid_size as f64,
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
            Phase::Row | Phase::Cell => {
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
                let r = if self.phase == Phase::Row {
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
fn advance(value: f64, start: f64, length: f64, delta: f64) -> f64 {
    let next = value + delta;
    if next > start + length - 1.0 {
        start
    } else if next < start {
        start + length - 1.0
    } else {
        next
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
        }
    }
    fn phase(&self) -> Phase {
        self.phase
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
        assert_eq!(e.technique.row.index(), 2);
        e.action(Action::Select);
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
