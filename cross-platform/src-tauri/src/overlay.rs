use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::input::PointerFeedback;
use crate::mouse_repeat::RepeatCommand;
use crate::state::{AppSettings, SharedModel};

#[cfg(target_os = "macos")]
#[path = "overlay_macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "overlay_windows.rs"]
mod platform;

const FOLLOW_INTERVAL: Duration = Duration::from_millis(75);
const DEFAULT_DURATION: Duration = Duration::from_millis(900);
const LANDING_DURATION: Duration = Duration::from_millis(300);
const DOUBLE_CLICK_DURATION: Duration = Duration::from_millis(550);

#[derive(Clone)]
pub struct CursorOverlay {
    sender: Sender<Command>,
}

impl CursorOverlay {
    pub fn install(app: AppHandle, shared: SharedModel) -> Self {
        let (sender, receiver) = mpsc::channel();
        platform::spawn(app, shared, receiver);
        Self { sender }
    }

    pub fn show(&self, feedback: PointerFeedback, settings: AppSettings) {
        let _ = self.sender.send(Command::Show(feedback, settings));
    }

    pub fn mark_control_active(&self, settings: AppSettings) {
        let _ = self.sender.send(Command::MarkControlActive(settings));
    }

    pub fn apply_settings(&self, settings: AppSettings) {
        let _ = self.sender.send(Command::ApplySettings(settings));
    }

    pub fn begin_repeat(
        &self,
        generation: u64,
        command: RepeatCommand,
        accelerated: bool,
        settings: AppSettings,
    ) {
        let _ = self.sender.send(Command::BeginRepeat(
            generation,
            command,
            accelerated,
            settings,
        ));
    }

    pub fn end_repeat(&self, generation: u64) {
        let _ = self.sender.send(Command::EndRepeat(generation));
    }

    pub fn end_session(&self) {
        let _ = self.sender.send(Command::EndSession);
    }
}

pub(crate) enum Command {
    Show(PointerFeedback, AppSettings),
    MarkControlActive(AppSettings),
    ApplySettings(AppSettings),
    BeginRepeat(u64, RepeatCommand, bool, AppSettings),
    EndRepeat(u64),
    EndSession,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Frame {
    pub feedback: PointerFeedback,
    pub logical_size: u32,
    pub color: [u8; 3],
    pub crosshairs: bool,
}

pub(crate) enum Update {
    Render(Frame),
    Hide,
    None,
    Shutdown,
}

pub(crate) struct OverlayEngine {
    settings: AppSettings,
    feedback: Option<PointerFeedback>,
    control_active: bool,
    drag_active: bool,
    repeat_generation: Option<u64>,
    deadline: Option<Instant>,
    next_follow: Instant,
    visible: bool,
}

impl OverlayEngine {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            settings: AppSettings::default(),
            feedback: None,
            control_active: false,
            drag_active: false,
            repeat_generation: None,
            deadline: None,
            next_follow: now,
            visible: false,
        }
    }

    pub(crate) fn handle(&mut self, command: Command, now: Instant) -> Update {
        match command {
            Command::Show(feedback, settings) => {
                self.settings = settings;
                if !self.settings.cursor_overlay_enabled {
                    return self.hide();
                }
                self.control_active = true;
                self.drag_active = matches!(feedback, PointerFeedback::Drag);
                self.deadline = deadline_for(feedback, self.persistent(), now);
                self.feedback = Some(feedback);
                self.next_follow = now + FOLLOW_INTERVAL;
                self.visible = true;
                Update::Render(self.frame(feedback))
            }
            Command::MarkControlActive(settings) => {
                self.settings = settings;
                self.control_active = true;
                if !self.settings.cursor_overlay_enabled {
                    return self.hide();
                }
                if self.settings.cursor_overlay_visibility == "whileControlling"
                    && self.feedback.is_none()
                {
                    self.feedback = Some(PointerFeedback::Move);
                    self.next_follow = now + FOLLOW_INTERVAL;
                    self.visible = true;
                    return Update::Render(self.frame(PointerFeedback::Move));
                }
                Update::None
            }
            Command::ApplySettings(settings) => {
                self.settings = settings;
                if !self.settings.cursor_overlay_enabled {
                    return self.hide();
                }
                if self.visible || self.repeat_generation.is_some() {
                    let feedback = self.feedback.unwrap_or(PointerFeedback::Move);
                    self.deadline = deadline_for(feedback, self.persistent(), now);
                    self.visible = true;
                    return Update::Render(self.frame(feedback));
                }
                Update::None
            }
            Command::BeginRepeat(generation, command, accelerated, settings) => {
                self.settings = settings;
                if !self.settings.cursor_overlay_enabled {
                    return self.hide();
                }
                self.control_active = true;
                self.drag_active = false;
                self.repeat_generation = Some(generation);
                let feedback = match command {
                    RepeatCommand::Move { .. } => PointerFeedback::RepeatMove { accelerated },
                    RepeatCommand::Scroll { dx, dy } => PointerFeedback::RepeatScroll { dx, dy },
                };
                self.feedback = Some(feedback);
                self.deadline = None;
                self.next_follow = now + FOLLOW_INTERVAL;
                self.visible = true;
                Update::Render(self.frame(feedback))
            }
            Command::EndRepeat(generation) => {
                if self.repeat_generation != Some(generation) {
                    return Update::None;
                }
                self.repeat_generation = None;
                if self.control_active
                    && self.settings.cursor_overlay_enabled
                    && self.settings.cursor_overlay_visibility == "whileControlling"
                {
                    self.feedback = Some(PointerFeedback::Move);
                    self.next_follow = now + FOLLOW_INTERVAL;
                    self.visible = true;
                    Update::Render(self.frame(PointerFeedback::Move))
                } else {
                    self.feedback = None;
                    self.hide()
                }
            }
            Command::EndSession => {
                self.control_active = false;
                self.drag_active = false;
                self.repeat_generation = None;
                self.feedback = None;
                self.deadline = None;
                self.hide()
            }
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) -> Update {
        if let Some(deadline) = self.deadline {
            if now >= deadline {
                self.deadline = None;
                if self.persistent() {
                    self.feedback = Some(PointerFeedback::Move);
                } else {
                    self.feedback = None;
                    return self.hide();
                }
            }
        }
        if self.visible && self.persistent() && now >= self.next_follow {
            self.next_follow = now + FOLLOW_INTERVAL;
            let feedback = if self.drag_active {
                PointerFeedback::Drag
            } else {
                self.feedback.unwrap_or(PointerFeedback::Move)
            };
            return Update::Render(self.frame(feedback));
        }
        Update::None
    }

    fn persistent(&self) -> bool {
        self.drag_active
            || self.repeat_generation.is_some()
            || (self.control_active
                && self.settings.cursor_overlay_visibility == "whileControlling")
    }

    fn frame(&self, feedback: PointerFeedback) -> Frame {
        Frame {
            feedback,
            logical_size: match self.settings.cursor_overlay_size.as_str() {
                "small" => 96,
                "large" => 176,
                _ => 128,
            },
            color: color_for(&self.settings.cursor_overlay_color),
            crosshairs: self.settings.cursor_crosshairs,
        }
    }

    fn hide(&mut self) -> Update {
        let was_visible = self.visible;
        self.visible = false;
        if was_visible {
            Update::Hide
        } else {
            Update::None
        }
    }
}

fn duration_for(feedback: PointerFeedback) -> Duration {
    match feedback {
        PointerFeedback::Click { count: 2, .. } => DOUBLE_CLICK_DURATION,
        PointerFeedback::Click { .. } | PointerFeedback::Scroll { .. } => LANDING_DURATION,
        PointerFeedback::Move
        | PointerFeedback::Drag
        | PointerFeedback::RepeatMove { .. }
        | PointerFeedback::RepeatScroll { .. } => DEFAULT_DURATION,
    }
}

fn deadline_for(feedback: PointerFeedback, persistent: bool, now: Instant) -> Option<Instant> {
    if persistent && matches!(feedback, PointerFeedback::Move | PointerFeedback::Drag) {
        None
    } else {
        Some(now + duration_for(feedback))
    }
}

fn color_for(value: &str) -> [u8; 3] {
    match value {
        "green" => [132, 255, 145],
        "blue" => [100, 166, 255],
        "yellow" => [255, 209, 102],
        "white" => [255, 255, 255],
        _ => [211, 47, 47],
    }
}

pub(crate) fn render_marker(frame: &Frame, scale: f64) -> Pixmap {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let size = ((frame.logical_size as f64 * scale).round() as u32).max(1);
    let mut pixmap = Pixmap::new(size, size).expect("valid overlay pixmap size");
    let unit = frame.logical_size as f32 * scale as f32;
    let center = size as f32 / 2.0;
    match frame.feedback {
        PointerFeedback::Click { .. } => draw_landing(&mut pixmap, center, unit, frame.color),
        PointerFeedback::Scroll { dx, dy } => {
            draw_ring(&mut pixmap, center, unit, frame.color, false);
            draw_scroll(&mut pixmap, center, unit, frame.color, dx, dy);
        }
        PointerFeedback::RepeatScroll { dx, dy } => {
            draw_ring(&mut pixmap, center, unit, frame.color, false);
            draw_scroll(&mut pixmap, center, unit, frame.color, dx, dy);
        }
        PointerFeedback::RepeatMove { accelerated } => {
            draw_ring(&mut pixmap, center, unit, frame.color, false);
            if accelerated {
                draw_repeat_progress(&mut pixmap, center, unit, frame.color);
            }
        }
        PointerFeedback::Drag => draw_ring(&mut pixmap, center, unit, frame.color, true),
        PointerFeedback::Move => draw_ring(&mut pixmap, center, unit, frame.color, false),
    }
    pixmap
}

fn draw_repeat_progress(pixmap: &mut Pixmap, center: f32, unit: f32, color: [u8; 3]) {
    let radius = unit * 0.39;
    let dot = (unit * 0.015).max(1.5);
    for index in 0..12 {
        let angle = std::f32::consts::TAU * index as f32 / 12.0;
        circle(
            pixmap,
            center + radius * angle.cos(),
            center + radius * angle.sin(),
            dot,
            color,
            190,
            None,
        );
    }
}

fn paint(color: [u8; 3], alpha: u8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(color[0], color[1], color[2], alpha));
    paint.anti_alias = true;
    paint
}

fn circle(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    radius: f32,
    color: [u8; 3],
    alpha: u8,
    width: Option<f32>,
) {
    let Some(path) = PathBuilder::from_circle(cx, cy, radius) else {
        return;
    };
    if let Some(width) = width {
        pixmap.stroke_path(
            &path,
            &paint(color, alpha),
            &Stroke {
                width,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    } else {
        pixmap.fill_path(
            &path,
            &paint(color, alpha),
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_ring(pixmap: &mut Pixmap, center: f32, unit: f32, color: [u8; 3], drag: bool) {
    let diameter = unit * 0.5625;
    let ring = (unit * 0.039).max(4.0);
    let glow = (unit * 0.1875).max(18.0);
    circle(
        pixmap,
        center,
        center,
        diameter / 2.0,
        color,
        if drag { 66 } else { 62 },
        Some(if drag { glow * 1.08 } else { glow }),
    );
    circle(
        pixmap,
        center,
        center,
        diameter / 2.0,
        color,
        250,
        Some(if drag { ring + 1.0 } else { ring }),
    );
    if drag {
        circle(pixmap, center, center, diameter * 0.11, color, 240, None);
    }
}

fn draw_landing(pixmap: &mut Pixmap, center: f32, unit: f32, color: [u8; 3]) {
    let core = unit * 0.2625;
    let start = unit * 0.375 / 2.0;
    let stroke = (unit * 0.0234375).max(2.0);
    let max = (unit * 0.6875 - stroke) / 2.0;
    let halo = start + (max - start) * 0.7;
    circle(pixmap, center, center, halo, color, 180, Some(stroke));
    circle(
        pixmap,
        center + (2.0_f32).max(unit / 64.0),
        center + (2.0_f32).max(unit / 64.0),
        core / 2.0,
        [0, 0, 0],
        65,
        None,
    );
    circle(pixmap, center, center, core / 2.0, color, 255, None);
}

fn draw_scroll(pixmap: &mut Pixmap, center: f32, unit: f32, color: [u8; 3], dx: i32, dy: i32) {
    let magnitude = ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt();
    let (x, y) = if magnitude > 0.0 {
        (-(dx as f64) / magnitude, -(dy as f64) / magnitude)
    } else {
        (0.0, -1.0)
    };
    let (x, y) = (x as f32, y as f32);
    let (px, py) = (-y, x);
    let half = unit * 0.4375 / 2.0;
    let head = unit * 0.09375;
    let wing = head * 0.55;
    let start = (center - x * half, center - y * half);
    let end = (center + x * half, center + y * half);
    let back = (end.0 - x * head, end.1 - y * head);
    let mut path = PathBuilder::new();
    path.move_to(start.0, start.1);
    path.line_to(end.0, end.1);
    path.move_to(end.0, end.1);
    path.line_to(back.0 + px * wing, back.1 + py * wing);
    path.move_to(end.0, end.1);
    path.line_to(back.0 - px * wing, back.1 - py * wing);
    let Some(path) = path.finish() else {
        return;
    };
    let width = (unit * 0.046875).max(3.0);
    let stroke = |width| Stroke {
        width,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    pixmap.stroke_path(
        &path,
        &paint([0, 0, 0], 80),
        &stroke(width + (2.0_f32).max(unit / 64.0)),
        Transform::identity(),
        None,
    );
    pixmap.stroke_path(
        &path,
        &paint(color, 230),
        &stroke(width),
        Transform::identity(),
        None,
    );
}

pub(crate) fn run_loop(
    mut render: impl FnMut(&Frame) -> Result<(), String>,
    mut hide: impl FnMut(),
    receiver: Receiver<Command>,
) {
    let mut engine = OverlayEngine::new(Instant::now());
    loop {
        let update = match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(command) => engine.handle(command, Instant::now()),
            Err(mpsc::RecvTimeoutError::Timeout) => engine.tick(Instant::now()),
            Err(mpsc::RecvTimeoutError::Disconnected) => Update::Shutdown,
        };
        match update {
            Update::Render(frame) => {
                if render(&frame).is_err() {
                    hide();
                    break;
                }
            }
            Update::Hide => hide(),
            Update::None => {}
            Update::Shutdown => {
                hide();
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_feedback_hides_after_its_deadline() {
        let now = Instant::now();
        let mut engine = OverlayEngine::new(now);
        let settings = AppSettings {
            cursor_overlay_visibility: "onInput".into(),
            ..AppSettings::default()
        };
        assert!(matches!(
            engine.handle(Command::Show(PointerFeedback::Move, settings), now),
            Update::Render(_)
        ));
        assert!(matches!(engine.tick(now + DEFAULT_DURATION), Update::Hide));
    }

    #[test]
    fn default_feedback_stays_visible_while_controlling() {
        let now = Instant::now();
        let mut engine = OverlayEngine::new(now);
        engine.handle(
            Command::Show(PointerFeedback::Move, AppSettings::default()),
            now,
        );
        let Update::Render(frame) = engine.tick(now + DEFAULT_DURATION) else {
            panic!("expected persistent frame");
        };
        assert_eq!(frame.feedback, PointerFeedback::Move);
    }

    #[test]
    fn while_controlling_restores_the_pointer_after_timed_feedback() {
        let now = Instant::now();
        for (feedback, duration) in [
            (
                PointerFeedback::Click {
                    button: crate::protocol::MouseButton::Left,
                    count: 1,
                },
                LANDING_DURATION,
            ),
            (
                PointerFeedback::Click {
                    button: crate::protocol::MouseButton::Left,
                    count: 2,
                },
                DOUBLE_CLICK_DURATION,
            ),
            (PointerFeedback::Scroll { dx: 0, dy: 10 }, LANDING_DURATION),
        ] {
            let settings = AppSettings::default();
            let mut engine = OverlayEngine::new(now);
            engine.handle(Command::MarkControlActive(settings.clone()), now);
            engine.handle(Command::Show(feedback, settings), now);
            let Update::Render(frame) = engine.tick(now + duration) else {
                panic!("expected persistent frame");
            };
            assert_eq!(frame.feedback, PointerFeedback::Move);
        }
    }

    #[test]
    fn drag_feedback_stays_visible_in_transient_mode() {
        let now = Instant::now();
        let settings = AppSettings {
            cursor_overlay_visibility: "onInput".into(),
            ..AppSettings::default()
        };
        let mut engine = OverlayEngine::new(now);
        engine.handle(Command::Show(PointerFeedback::Drag, settings), now);
        let Update::Render(frame) = engine.tick(now + DEFAULT_DURATION) else {
            panic!("expected persistent drag frame");
        };
        assert_eq!(frame.feedback, PointerFeedback::Drag);
    }

    #[test]
    fn ending_a_session_hides_the_overlay() {
        let now = Instant::now();
        let mut engine = OverlayEngine::new(now);
        engine.handle(
            Command::Show(PointerFeedback::Move, AppSettings::default()),
            now,
        );
        assert!(matches!(
            engine.handle(Command::EndSession, now),
            Update::Hide
        ));
    }

    #[test]
    fn crosshairs_are_included_in_persistent_frames() {
        let now = Instant::now();
        let settings = AppSettings {
            cursor_crosshairs: true,
            ..AppSettings::default()
        };
        let mut engine = OverlayEngine::new(now);
        let Update::Render(frame) =
            engine.handle(Command::Show(PointerFeedback::Move, settings), now)
        else {
            panic!("expected overlay frame");
        };
        assert!(frame.crosshairs);
    }

    #[test]
    fn disabling_settings_hides_a_visible_overlay() {
        let now = Instant::now();
        let mut engine = OverlayEngine::new(now);
        engine.handle(
            Command::Show(PointerFeedback::Move, AppSettings::default()),
            now,
        );
        let settings = AppSettings {
            cursor_overlay_enabled: false,
            ..AppSettings::default()
        };
        assert!(matches!(
            engine.handle(Command::ApplySettings(settings), now),
            Update::Hide
        ));
    }

    #[test]
    fn renderer_uses_size_and_direction_without_panicking() {
        let frame = Frame {
            feedback: PointerFeedback::Scroll { dx: 0, dy: 10 },
            logical_size: 128,
            color: [211, 47, 47],
            crosshairs: true,
        };
        let pixmap = render_marker(&frame, 1.5);
        assert_eq!((pixmap.width(), pixmap.height()), (192, 192));
        assert!(pixmap.data().iter().any(|channel| *channel != 0));
    }

    #[test]
    fn repeat_end_requires_generation_ownership_and_restores_default_marker() {
        let now = Instant::now();
        let mut engine = OverlayEngine::new(now);
        engine.handle(
            Command::BeginRepeat(
                4,
                RepeatCommand::Move { dx: 10, dy: 0 },
                true,
                AppSettings::default(),
            ),
            now,
        );
        assert!(matches!(
            engine.handle(Command::EndRepeat(3), now),
            Update::None
        ));
        let Update::Render(frame) = engine.handle(Command::EndRepeat(4), now) else {
            panic!("expected restored marker");
        };
        assert_eq!(frame.feedback, PointerFeedback::Move);
    }

    #[test]
    fn repeat_is_persistent_but_hides_when_transient_mode_ends() {
        let now = Instant::now();
        let settings = AppSettings {
            cursor_overlay_visibility: "onInput".into(),
            ..AppSettings::default()
        };
        let mut engine = OverlayEngine::new(now);
        engine.handle(
            Command::BeginRepeat(7, RepeatCommand::Scroll { dx: 0, dy: 5 }, false, settings),
            now,
        );
        assert!(matches!(
            engine.tick(now + DEFAULT_DURATION),
            Update::Render(_)
        ));
        assert!(matches!(
            engine.handle(Command::EndRepeat(7), now + DEFAULT_DURATION),
            Update::Hide
        ));
    }
}
