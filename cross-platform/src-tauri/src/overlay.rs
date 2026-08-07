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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CursorOverlayVisualTokens {
    pub window_size: u32,
    pub unit: f32,
    pub ring_diameter: f32,
    pub ring_stroke: f32,
    pub glow_stroke: f32,
    pub drag_dot_diameter: f32,
    pub ring_alpha: u8,
    pub glow_alpha: u8,
    pub drag_glow_alpha: u8,
    pub drag_dot_alpha: u8,
    pub drag_ring_extra_stroke: f32,
    pub drag_glow_scale: f32,
}

impl CursorOverlayVisualTokens {
    fn create(logical_size: u32, scale: f64) -> Self {
        let logical_size = if logical_size > 0 { logical_size } else { 128 };
        let scale = normalized_scale(scale);
        let logical = logical_size as f32;
        let physical_window_size = (logical_size as f64 * scale).round();
        let scale = scale as f32;
        let ring_diameter = logical * 0.5625 * scale;
        Self {
            window_size: (physical_window_size as u32).max(1),
            unit: logical * scale,
            ring_diameter,
            ring_stroke: (4.0 * scale).max(logical * 0.039 * scale),
            glow_stroke: (18.0 * scale).max(logical * 0.1875 * scale),
            drag_dot_diameter: ring_diameter * 0.22,
            ring_alpha: 250,
            glow_alpha: 62,
            drag_glow_alpha: 66,
            drag_dot_alpha: 240,
            drag_ring_extra_stroke: 1.0,
            drag_glow_scale: 1.08,
        }
    }
}

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
        dragging: bool,
        settings: AppSettings,
    ) {
        let _ = self.sender.send(Command::BeginRepeat(
            generation,
            command,
            accelerated,
            dragging,
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
    BeginRepeat(u64, RepeatCommand, bool, bool, AppSettings),
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
            Command::BeginRepeat(generation, command, accelerated, dragging, settings) => {
                self.settings = settings;
                if !self.settings.cursor_overlay_enabled {
                    return self.hide();
                }
                self.control_active = true;
                self.drag_active = dragging;
                self.repeat_generation = Some(generation);
                let feedback = match command {
                    RepeatCommand::Move { .. } => PointerFeedback::RepeatMove {
                        accelerated,
                        dragging,
                    },
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
                    && (self.drag_active
                        || self.settings.cursor_overlay_visibility == "whileControlling")
                {
                    let feedback = if self.drag_active {
                        PointerFeedback::Drag
                    } else {
                        PointerFeedback::Move
                    };
                    self.feedback = Some(feedback);
                    self.next_follow = now + FOLLOW_INTERVAL;
                    self.visible = true;
                    Update::Render(self.frame(feedback))
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
            let feedback = if self.drag_active && self.repeat_generation.is_none() {
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
    let scale = normalized_scale(scale);
    let tokens = CursorOverlayVisualTokens::create(frame.logical_size, scale);
    let mut pixmap =
        Pixmap::new(tokens.window_size, tokens.window_size).expect("valid overlay pixmap size");
    let unit = tokens.unit;
    let center = tokens.window_size as f32 / 2.0;
    match frame.feedback {
        PointerFeedback::Click { .. } => draw_landing(&mut pixmap, center, unit, frame.color),
        PointerFeedback::Scroll { dx, dy } => {
            draw_ring(&mut pixmap, center, tokens, frame.color, false);
            draw_scroll(&mut pixmap, center, unit, frame.color, dx, dy);
        }
        PointerFeedback::RepeatScroll { dx, dy } => {
            draw_ring(&mut pixmap, center, tokens, frame.color, false);
            draw_scroll(&mut pixmap, center, unit, frame.color, dx, dy);
        }
        PointerFeedback::RepeatMove {
            accelerated,
            dragging,
        } => {
            draw_ring(&mut pixmap, center, tokens, frame.color, dragging);
            if accelerated {
                draw_repeat_progress(&mut pixmap, center, unit, frame.color);
            }
        }
        PointerFeedback::Drag => draw_ring(&mut pixmap, center, tokens, frame.color, true),
        PointerFeedback::Move => draw_ring(&mut pixmap, center, tokens, frame.color, false),
    }
    pixmap
}

fn normalized_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
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

fn draw_ring(
    pixmap: &mut Pixmap,
    center: f32,
    tokens: CursorOverlayVisualTokens,
    color: [u8; 3],
    drag: bool,
) {
    circle(
        pixmap,
        center,
        center,
        tokens.ring_diameter / 2.0,
        color,
        if drag {
            tokens.drag_glow_alpha
        } else {
            tokens.glow_alpha
        },
        Some(if drag {
            tokens.glow_stroke * tokens.drag_glow_scale
        } else {
            tokens.glow_stroke
        }),
    );
    circle(
        pixmap,
        center,
        center,
        tokens.ring_diameter / 2.0,
        color,
        tokens.ring_alpha,
        Some(if drag {
            tokens.ring_stroke + tokens.drag_ring_extra_stroke
        } else {
            tokens.ring_stroke
        }),
    );
    if drag {
        circle(
            pixmap,
            center,
            center,
            tokens.drag_dot_diameter / 2.0,
            color,
            tokens.drag_dot_alpha,
            None,
        );
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

    fn ring_frame(feedback: PointerFeedback) -> Frame {
        Frame {
            feedback,
            logical_size: 128,
            color: [211, 47, 47],
            crosshairs: false,
        }
    }

    fn alpha_at(pixmap: &Pixmap, x: u32, y: u32) -> u8 {
        pixmap.pixel(x, y).unwrap().alpha()
    }

    #[test]
    fn visual_tokens_match_csharp_geometry_across_sizes_and_scales() {
        for (logical_size, expected) in [
            (96, [(96, 54.0), (144, 81.0), (192, 108.0)]),
            (128, [(128, 72.0), (192, 108.0), (256, 144.0)]),
            (176, [(176, 99.0), (264, 148.5), (352, 198.0)]),
        ] {
            for (index, scale) in [1.0, 1.5, 2.0].into_iter().enumerate() {
                let tokens = CursorOverlayVisualTokens::create(logical_size, scale);
                let (expected_window, expected_diameter) = expected[index];
                assert_eq!(tokens.window_size, expected_window);
                assert_eq!(tokens.ring_diameter, expected_diameter);
                assert_eq!(tokens.drag_dot_diameter, expected_diameter * 0.22);
                assert_eq!(tokens.ring_alpha, 250);
                assert_eq!(tokens.glow_alpha, 62);
                assert_eq!(tokens.drag_glow_alpha, 66);
                assert_eq!(tokens.drag_dot_alpha, 240);
                assert_eq!(tokens.drag_ring_extra_stroke, 1.0);
                assert_eq!(tokens.drag_glow_scale, 1.08);
            }
        }

        let small_retina = CursorOverlayVisualTokens::create(96, 2.0);
        assert_eq!(small_retina.ring_stroke, 8.0);
        assert_eq!(small_retina.glow_stroke, 36.0);
    }

    #[test]
    fn invalid_visual_scale_falls_back_to_one() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                CursorOverlayVisualTokens::create(128, scale),
                CursorOverlayVisualTokens::create(128, 1.0)
            );
        }
    }

    #[test]
    fn movement_ring_has_csharp_solid_and_glow_alpha_falloff() {
        let pixmap = render_marker(&ring_frame(PointerFeedback::Move), 1.0);
        let center = pixmap.width() / 2;
        let solid = alpha_at(&pixmap, center + 36, center);
        let glow = alpha_at(&pixmap, center + 43, center);

        assert!(solid > 240);
        assert!((1..=80).contains(&glow));
        assert_eq!(alpha_at(&pixmap, center, center), 0);
    }

    #[test]
    fn drag_ring_uses_the_csharp_center_dot() {
        for feedback in [
            PointerFeedback::Drag,
            PointerFeedback::RepeatMove {
                accelerated: true,
                dragging: true,
            },
        ] {
            let pixmap = render_marker(&ring_frame(feedback), 1.0);
            let center = pixmap.width() / 2;
            assert!(alpha_at(&pixmap, center, center) >= 240);
            assert_eq!(alpha_at(&pixmap, center + 10, center), 0);
        }
    }

    #[test]
    fn every_ring_feedback_state_renders_the_shared_base_ring() {
        for feedback in [
            PointerFeedback::Move,
            PointerFeedback::RepeatMove {
                accelerated: false,
                dragging: false,
            },
            PointerFeedback::RepeatMove {
                accelerated: true,
                dragging: false,
            },
            PointerFeedback::Drag,
            PointerFeedback::Scroll { dx: 0, dy: 4 },
            PointerFeedback::RepeatScroll { dx: 4, dy: 0 },
        ] {
            let pixmap = render_marker(&ring_frame(feedback), 1.0);
            let center = pixmap.width() / 2;
            assert!(alpha_at(&pixmap, center + 36, center) > 200);
        }
    }

    #[test]
    fn raster_output_is_premultiplied_rgba() {
        for color in [
            [211, 47, 47],
            [132, 255, 145],
            [100, 166, 255],
            [255, 209, 102],
            [255, 255, 255],
        ] {
            let mut frame = ring_frame(PointerFeedback::Move);
            frame.color = color;
            let pixmap = render_marker(&frame, 1.0);
            for pixel in pixmap.data().chunks_exact(4) {
                assert!(pixel[0] <= pixel[3]);
                assert!(pixel[1] <= pixel[3]);
                assert!(pixel[2] <= pixel[3]);
            }
        }
    }

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
                false,
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
            Command::BeginRepeat(
                7,
                RepeatCommand::Scroll { dx: 0, dy: 5 },
                false,
                false,
                settings,
            ),
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

    #[test]
    fn repeat_movement_preserves_drag_until_drag_end() {
        let now = Instant::now();
        let mut engine = OverlayEngine::new(now);
        let settings = AppSettings {
            cursor_overlay_visibility: "onInput".into(),
            ..AppSettings::default()
        };

        engine.handle(Command::Show(PointerFeedback::Drag, settings.clone()), now);
        let Update::Render(started) = engine.handle(
            Command::BeginRepeat(
                8,
                RepeatCommand::Move { dx: 10, dy: 0 },
                true,
                true,
                settings.clone(),
            ),
            now,
        ) else {
            panic!("expected drag repeat frame");
        };
        assert_eq!(
            started.feedback,
            PointerFeedback::RepeatMove {
                accelerated: true,
                dragging: true,
            }
        );

        let Update::Render(followed) = engine.tick(now + FOLLOW_INTERVAL) else {
            panic!("expected followed drag repeat frame");
        };
        assert_eq!(followed.feedback, started.feedback);

        let Update::Render(stopped) = engine.handle(Command::EndRepeat(8), now) else {
            panic!("expected restored drag frame");
        };
        assert_eq!(stopped.feedback, PointerFeedback::Drag);

        let Update::Render(released) =
            engine.handle(Command::Show(PointerFeedback::Move, settings), now)
        else {
            panic!("expected released movement frame");
        };
        assert_eq!(released.feedback, PointerFeedback::Move);
    }
}
