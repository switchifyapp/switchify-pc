//! Android point scanning, with desktop coordinates and no OS input in the engine.
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
    pub fn keys(&self) -> [&str; 5] {
        [
            &self.select_key,
            &self.next_key,
            &self.back_key,
            &self.pause_key,
            "Escape",
        ]
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.speed > 4
            || !(2..=10).contains(&self.grid_size)
            || !(250..=5000).contains(&self.block_interval_ms)
        {
            return Err("Point scan speed, grid size, or interval is invalid.".into());
        }
        let keys = self.keys();
        for (index, key) in keys.iter().enumerate() {
            let function = key
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
            if !function
                && ![
                    "Space",
                    "Enter",
                    "Backspace",
                    "Escape",
                    "ArrowUp",
                    "ArrowDown",
                    "ArrowLeft",
                    "ArrowRight",
                ]
                .contains(key)
            {
                return Err("Choose a supported switch key.".into());
            }
            if keys[..index].contains(key) {
                return Err(
                    "Each switch action must use a different key. Escape is reserved for cancel."
                        .into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|n| n.is_finite())
            && self.width >= 2.0
            && self.height >= 2.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Row,
    Cell,
    X,
    Y,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Select,
    Next,
    Back,
    Pause,
    Cancel,
}
pub const ACTIONS: [Action; 5] = [
    Action::Select,
    Action::Next,
    Action::Back,
    Action::Pause,
    Action::Cancel,
];

pub struct Engine {
    pub config: Config,
    pub screen: Rect,
    pub region: Rect,
    pub phase: Phase,
    pub paused: bool,
    pub x: f64,
    pub y: f64,
    pub row: usize,
    pub cell: usize,
    pub units_per_logical_pixel: f64,
    direction: f64,
    block_elapsed: u64,
}
impl Engine {
    pub fn new(config: Config, screen: Rect, units_per_logical_pixel: f64) -> Result<Self, String> {
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
            paused: false,
            x: screen.x,
            y: screen.y,
            row: 0,
            cell: 0,
            direction: 1.0,
            block_elapsed: 0,
            units_per_logical_pixel,
        })
    }
    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.paused = false;
        self.region = self.screen;
        self.x = self.screen.x;
        self.y = self.screen.y;
        self.row = 0;
        self.cell = 0;
        self.direction = 1.0;
        self.block_elapsed = 0;
    }
    pub fn action(&mut self, action: Action) -> Option<(i32, i32)> {
        if action == Action::Cancel {
            self.reset();
            return None;
        }
        if self.phase == Phase::Idle {
            if action == Action::Select {
                self.phase = if self.config.mode == Mode::Grid {
                    Phase::Row
                } else {
                    Phase::X
                };
            }
            return None;
        }
        match action {
            Action::Select => {
                self.block_elapsed = 0;
                match self.phase {
                    Phase::Row => {
                        self.phase = Phase::Cell;
                        self.cell = 0;
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
                        self.reset();
                        return Some(point);
                    }
                    Phase::Idle => {}
                }
            }
            Action::Next | Action::Back => {
                self.direction = if action == Action::Next { 1.0 } else { -1.0 };
                self.step(33);
            }
            Action::Pause => self.paused = !self.paused,
            Action::Cancel => {}
        }
        None
    }
    pub fn tick(&mut self, elapsed_ms: u64) {
        if !self.config.automatic || self.paused || self.phase == Phase::Idle {
            return;
        }
        // Android caps delayed timer callbacks at 250 ms to avoid large jumps.
        let elapsed_ms = elapsed_ms.min(250);
        if matches!(self.phase, Phase::Row | Phase::Cell) {
            self.block_elapsed += elapsed_ms;
            if self.block_elapsed < self.config.block_interval_ms {
                return;
            }
            self.block_elapsed %= self.config.block_interval_ms;
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
            Phase::Row => {
                self.row = (self.row as i32 + self.direction as i32)
                    .rem_euclid(self.config.grid_size as i32) as usize
            }
            Phase::Cell => {
                self.cell = (self.cell as i32 + self.direction as i32)
                    .rem_euclid(self.config.grid_size as i32) as usize
            }
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
            y: self.screen.y + self.screen.height * self.row as f64 / self.config.grid_size as f64,
            width: self.screen.width,
            height: self.screen.height / self.config.grid_size as f64,
        }
    }
    pub fn cell_rect(&self) -> Rect {
        let row = self.row_rect();
        Rect {
            x: row.x + row.width * self.cell as f64 / self.config.grid_size as f64,
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

/// Keep injection behind the existing adapter; no tests use the system adapter.
pub fn click<I: crate::input::InputInjector>(
    input: &mut crate::input::DesktopInput<I>,
    point: (i32, i32),
    modifiers_released: bool,
) -> Result<(), String> {
    if !modifiers_released {
        return Err("Release modifier keys before selecting a point.".into());
    }
    if input.has_active_switch_session() || input.has_active_drag() {
        return Err("End switch forwarding or dragging before using point scan.".into());
    }
    input.move_pointer_absolute(point.0, point.1)?;
    input.click_pointer(crate::protocol::MouseButton::Left, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine(config: Config) -> Engine {
        Engine::new(
            config,
            Rect {
                x: -1000.0,
                y: 50.0,
                width: 999.0,
                height: 701.0,
            },
            1.0,
        )
        .unwrap()
    }
    #[test]
    fn line_selects_x_then_y_and_resets() {
        let mut e = engine(Config::default());
        assert_eq!(e.action(Action::Select), None);
        e.tick(100);
        let x = e.x;
        e.action(Action::Select);
        e.tick(100);
        assert_eq!(e.x, x);
        assert_eq!(e.action(Action::Select), Some((-988, 62)));
        assert_eq!(e.phase, Phase::Idle);
        assert!(e.lines().is_empty());
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
        assert_eq!(e.row, 2);
        e.action(Action::Select);
        e.action(Action::Back);
        e.action(Action::Select);
        assert_eq!(e.region.x + e.region.width, -1.0);
        assert!((e.region.y + e.region.height - 751.0).abs() < 0.001);
        assert_eq!(e.phase, Phase::X);
    }
    #[test]
    fn pause_resume_wrap_and_delayed_ticks() {
        let mut e = engine(Config::default());
        e.action(Action::Select);
        e.action(Action::Pause);
        e.tick(1000);
        assert_eq!(e.x, -1000.0);
        e.action(Action::Pause);
        e.tick(10000);
        assert_eq!(e.x, -970.0);
        e.x = -1000.0;
        e.action(Action::Back);
        assert_eq!(e.x, -2.0);
        e.action(Action::Cancel);
        assert_eq!(e.phase, Phase::Idle);
    }
    #[test]
    fn manual_mode_only_moves_on_steps() {
        let mut e = engine(Config {
            automatic: false,
            ..Config::default()
        });
        e.action(Action::Select);
        e.tick(250);
        assert_eq!(e.x, -1000.0);
        e.action(Action::Next);
        assert!(e.x > -1000.0);
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
            Config::default(),
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
