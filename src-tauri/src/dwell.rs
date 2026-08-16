use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::display_navigation;
use crate::input::PointerFeedback;
use crate::overlay::CursorOverlay;
use crate::protocol::MouseButton;
use crate::state::{emit_state, set_activity, ActivityKind, AppModel, BluetoothState};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const TOLERANCE_LOGICAL_PIXELS: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PointerSample {
    x: f64,
    y: f64,
    native_units_per_logical_pixel: f64,
    button_pressed: bool,
}

#[derive(Debug, Clone, Copy)]
struct ArmedDwell {
    generation: u64,
    anchor: PointerSample,
    started_at: Instant,
    deadline: Instant,
}

#[derive(Debug, Default)]
struct DwellState {
    generation: u64,
    armed: Option<ArmedDwell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickResult {
    Progress(u16),
    Cancel,
    Click,
    Stale,
}

impl DwellState {
    fn arm(&mut self, sample: PointerSample, now: Instant, delay: Duration) -> u64 {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.armed = Some(ArmedDwell {
            generation: self.generation,
            anchor: sample,
            started_at: now,
            deadline: now + delay,
        });
        self.generation
    }

    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.armed = None;
    }

    fn tick(&mut self, generation: u64, sample: PointerSample, now: Instant) -> TickResult {
        let Some(armed) = self.armed else {
            return TickResult::Stale;
        };
        if armed.generation != generation {
            return TickResult::Stale;
        }
        let dx = sample.x - armed.anchor.x;
        let dy = sample.y - armed.anchor.y;
        let tolerance = TOLERANCE_LOGICAL_PIXELS
            * armed
                .anchor
                .native_units_per_logical_pixel
                .max(sample.native_units_per_logical_pixel);
        if sample.button_pressed || dx * dx + dy * dy > tolerance * tolerance {
            self.cancel();
            return TickResult::Cancel;
        }
        if now >= armed.deadline {
            self.cancel();
            return TickResult::Click;
        }
        let elapsed = now
            .checked_duration_since(armed.started_at)
            .unwrap_or_default()
            .as_secs_f64();
        let total = armed
            .deadline
            .checked_duration_since(armed.started_at)
            .unwrap_or(Duration::from_millis(1))
            .as_secs_f64();
        TickResult::Progress(((elapsed / total) * 1000.0).round().clamp(0.0, 999.0) as u16)
    }
}

#[derive(Default)]
pub struct DwellController {
    state: Mutex<DwellState>,
}

impl DwellController {
    pub fn arm(&self, app: &AppHandle) {
        let snapshot = app.state::<AppModel>().snapshot();
        if !snapshot.settings.dwell_click_enabled || snapshot.bluetooth != BluetoothState::Connected
        {
            self.cancel(app);
            return;
        }
        let sample = match sample_pointer(app) {
            Ok(sample) if !sample.button_pressed => sample,
            Ok(_) => {
                self.cancel(app);
                return;
            }
            Err(error) => {
                self.fail(app, error);
                return;
            }
        };
        let generation = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .arm(
                sample,
                Instant::now(),
                Duration::from_millis(u64::from(snapshot.settings.dwell_click_delay_ms)),
            );
        app.state::<CursorOverlay>()
            .show_dwell(0, snapshot.settings);
        schedule_tick(app.clone(), generation);
    }

    pub fn cancel(&self, app: &AppHandle) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel();
        if let Some(overlay) = app.try_state::<CursorOverlay>() {
            overlay.end_dwell();
        }
    }

    fn tick(&self, app: &AppHandle, generation: u64) {
        let snapshot = app.state::<AppModel>().snapshot();
        if !snapshot.settings.dwell_click_enabled || snapshot.bluetooth != BluetoothState::Connected
        {
            self.cancel(app);
            return;
        }
        let sample = match sample_pointer(app) {
            Ok(sample) => sample,
            Err(error) => {
                self.fail(app, error);
                return;
            }
        };
        let result = tick_and_publish_progress(
            &self.state,
            generation,
            sample,
            Instant::now(),
            |permille| {
                app.state::<CursorOverlay>()
                    .show_dwell(permille, snapshot.settings.clone());
            },
        );
        match result {
            TickResult::Progress(_) => {
                schedule_tick(app.clone(), generation);
            }
            TickResult::Cancel => app.state::<CursorOverlay>().end_dwell(),
            TickResult::Click => {
                app.state::<CursorOverlay>().end_dwell();
                match crate::platform_dwell_click(app) {
                    Ok(()) => app.state::<CursorOverlay>().show(
                        PointerFeedback::Click {
                            button: MouseButton::Left,
                            count: 1,
                        },
                        snapshot.settings,
                    ),
                    Err(_) => self.fail(
                        app,
                        "The operating system could not perform the dwell click.",
                    ),
                }
            }
            TickResult::Stale => {}
        }
    }

    fn fail(&self, app: &AppHandle, message: impl Into<String>) {
        self.cancel(app);
        let model = app.state::<AppModel>();
        set_activity(&model.shared, ActivityKind::Error, message.into());
        emit_state(app, &model.shared);
    }
}

fn tick_and_publish_progress(
    state: &Mutex<DwellState>,
    generation: u64,
    sample: PointerSample,
    now: Instant,
    publish_progress: impl FnOnce(u16),
) -> TickResult {
    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let result = state.tick(generation, sample, now);
    if let TickResult::Progress(permille) = result {
        publish_progress(permille);
    }
    result
}

fn schedule_tick(app: AppHandle, generation: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(POLL_INTERVAL).await;
        let callback_app = app.clone();
        let _ = app.run_on_main_thread(move || {
            callback_app
                .state::<DwellController>()
                .tick(&callback_app, generation);
        });
    });
}

fn sample_pointer(app: &AppHandle) -> Result<PointerSample, String> {
    let (position, displays) = display_navigation::displays(app)
        .map_err(|_| "The dwell pointer position could not be read.".to_string())?;
    let _display = display_navigation::current_display(position, &displays)
        .ok_or_else(|| "The dwell pointer display could not be resolved.".to_string())?;
    #[cfg(target_os = "windows")]
    let native_units_per_logical_pixel = _display.scale_factor;
    #[cfg(not(target_os = "windows"))]
    let native_units_per_logical_pixel = 1.0;
    Ok(PointerSample {
        x: position.0,
        y: position.1,
        native_units_per_logical_pixel,
        button_pressed: pointer_button_pressed(),
    })
}

#[cfg(target_os = "windows")]
fn pointer_button_pressed() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
    };
    [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
        .into_iter()
        .any(|button| unsafe { GetAsyncKeyState(i32::from(button.0)) } < 0)
}

#[cfg(target_os = "macos")]
fn pointer_button_pressed() -> bool {
    use objc2_core_graphics::{CGEventSource, CGEventSourceStateID, CGMouseButton};
    [
        CGMouseButton::Left,
        CGMouseButton::Center,
        CGMouseButton::Right,
    ]
    .into_iter()
    .any(|button| CGEventSource::button_state(CGEventSourceStateID::HIDSystemState, button))
}

#[cfg(test)]
mod tests {
    use super::{tick_and_publish_progress, DwellState, PointerSample, TickResult};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    fn sample(x: f64, y: f64, scale: f64) -> PointerSample {
        PointerSample {
            x,
            y,
            native_units_per_logical_pixel: scale,
            button_pressed: false,
        }
    }

    #[test]
    fn progress_publication_finishes_before_concurrent_cancellation() {
        let now = Instant::now();
        let state = Arc::new(Mutex::new(DwellState::default()));
        let generation =
            state
                .lock()
                .unwrap()
                .arm(sample(0.0, 0.0, 1.0), now, Duration::from_secs(1));
        let (event_tx, event_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let tick_state = state.clone();
        let tick_events = event_tx.clone();
        let tick = thread::spawn(move || {
            tick_and_publish_progress(
                &tick_state,
                generation,
                sample(0.0, 0.0, 1.0),
                now + Duration::from_millis(250),
                |_| {
                    tick_events.send("progress").unwrap();
                    release_rx.recv().unwrap();
                },
            )
        });
        assert_eq!(event_rx.recv().unwrap(), "progress");

        let cancel_state = state.clone();
        let cancel_events = event_tx;
        let cancel = thread::spawn(move || {
            attempt_tx.send(()).unwrap();
            cancel_state.lock().unwrap().cancel();
            cancel_events.send("cancel").unwrap();
        });
        attempt_rx.recv().unwrap();
        assert!(state.try_lock().is_err());
        assert!(event_rx.recv_timeout(Duration::from_millis(25)).is_err());
        release_tx.send(()).unwrap();
        assert_eq!(event_rx.recv().unwrap(), "cancel");
        assert_eq!(tick.join().unwrap(), TickResult::Progress(250));
        cancel.join().unwrap();
    }

    #[test]
    fn dwell_progresses_clicks_once_and_requires_rearming() {
        let now = Instant::now();
        let mut state = DwellState::default();
        let generation = state.arm(sample(-100.0, 20.0, 1.0), now, Duration::from_secs(1));
        assert_eq!(
            state.tick(
                generation,
                sample(-100.0, 20.0, 1.0),
                now + Duration::from_millis(500)
            ),
            TickResult::Progress(500)
        );
        assert_eq!(
            state.tick(
                generation,
                sample(-100.0, 20.0, 1.0),
                now + Duration::from_secs(1)
            ),
            TickResult::Click
        );
        assert_eq!(
            state.tick(
                generation,
                sample(-100.0, 20.0, 1.0),
                now + Duration::from_secs(2)
            ),
            TickResult::Stale
        );
    }

    #[test]
    fn dwell_tolerates_eight_logical_pixels_at_display_scale() {
        let now = Instant::now();
        let mut state = DwellState::default();
        let generation = state.arm(sample(-200.0, -40.0, 2.0), now, Duration::from_secs(1));
        assert!(matches!(
            state.tick(
                generation,
                sample(-184.0, -40.0, 2.0),
                now + Duration::from_millis(100)
            ),
            TickResult::Progress(_)
        ));
        assert_eq!(
            state.tick(
                generation,
                sample(-183.0, -40.0, 2.0),
                now + Duration::from_millis(200)
            ),
            TickResult::Cancel
        );
    }

    #[test]
    fn pointer_button_cancels_dwell() {
        let now = Instant::now();
        let mut state = DwellState::default();
        let generation = state.arm(sample(0.0, 0.0, 1.0), now, Duration::from_secs(1));
        let mut pressed = sample(0.0, 0.0, 1.0);
        pressed.button_pressed = true;
        assert_eq!(state.tick(generation, pressed, now), TickResult::Cancel);
    }

    #[test]
    fn newer_arm_invalidates_an_older_tick() {
        let now = Instant::now();
        let mut state = DwellState::default();
        let first = state.arm(sample(0.0, 0.0, 1.0), now, Duration::from_secs(1));
        let second = state.arm(sample(20.0, 20.0, 1.0), now, Duration::from_secs(1));
        assert_eq!(
            state.tick(first, sample(20.0, 20.0, 1.0), now),
            TickResult::Stale
        );
        assert!(matches!(
            state.tick(second, sample(20.0, 20.0, 1.0), now),
            TickResult::Progress(_)
        ));
        state.cancel();
        assert_eq!(
            state.tick(second, sample(20.0, 20.0, 1.0), now),
            TickResult::Stale
        );
    }
}
