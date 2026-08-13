use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "windows"))]
use std::time::Duration;

use serde::Serialize;
#[cfg(not(target_os = "windows"))]
use tauri::{
    utils::config::Color, Emitter, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
#[cfg(target_os = "windows")]
use tauri::{PhysicalPosition, PhysicalSize};

use crate::input::ModifierKey;
#[cfg(target_os = "macos")]
use crate::macos_overlay_window;
use crate::state::{emit_state, set_activity, ActivityKind, SharedModel};
#[cfg(target_os = "windows")]
#[path = "modifier_overlay_windows.rs"]
mod windows_backend;

const OVERLAY_WINDOW_LABEL: &str = "modifier-overlay";
#[cfg_attr(target_os = "windows", allow(dead_code))]
const OVERLAY_WIDTH: f64 = 480.0;
#[cfg_attr(target_os = "windows", allow(dead_code))]
const OVERLAY_HEIGHT: f64 = 70.0;
#[cfg_attr(target_os = "windows", allow(dead_code))]
const OVERLAY_MARGIN: f64 = 16.0;
#[cfg(not(target_os = "windows"))]
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);

pub trait ModifierKeyOverlayNotifier: Send + Sync {
    fn set_active_modifiers(&self, active_modifiers: &[ModifierKey]);
    fn end_control_session(&self);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierOverlaySnapshot {
    pub revision: u64,
    pub labels: Vec<String>,
}

#[derive(Clone)]
pub struct ModifierOverlay {
    inner: Arc<ModifierOverlayInner>,
}

struct ModifierOverlayInner {
    app: tauri::AppHandle,
    shared: SharedModel,
    #[cfg(not(target_os = "windows"))]
    window: WebviewWindow,
    #[cfg(target_os = "windows")]
    window: windows_backend::WindowsModifierOverlay,
    state: Mutex<ModifierOverlayState>,
    #[cfg(not(target_os = "windows"))]
    window_actions: Mutex<()>,
}

#[derive(Default)]
struct ModifierOverlayState {
    revision: u64,
    active_modifiers: Vec<ModifierKey>,
    #[cfg(not(target_os = "windows"))]
    ready: bool,
    #[cfg(not(target_os = "windows"))]
    readiness_failure_reported: bool,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PresentationAction {
    Hide,
    Ignore,
    Show,
}

impl ModifierOverlay {
    pub fn install(app: tauri::AppHandle, shared: SharedModel) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        let window = windows_backend::WindowsModifierOverlay::spawn(app.clone(), shared.clone())?;

        #[cfg(not(target_os = "windows"))]
        let window = {
            let window = WebviewWindowBuilder::new(
                &app,
                OVERLAY_WINDOW_LABEL,
                WebviewUrl::App("index.html?view=modifier-overlay".into()),
            )
            .title("Switchify active modifiers")
            .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            .background_color(Color(0, 0, 0, 0))
            .visible(false)
            .build()
            .map_err(|error| error.to_string())?;
            configure_platform_window(&window)?;
            window
                .set_ignore_cursor_events(true)
                .map_err(|error| error.to_string())?;
            window
        };
        let overlay = Self {
            inner: Arc::new(ModifierOverlayInner {
                app,
                shared,
                window,
                state: Mutex::new(ModifierOverlayState::default()),
                #[cfg(not(target_os = "windows"))]
                window_actions: Mutex::new(()),
            }),
        };
        #[cfg(not(target_os = "windows"))]
        overlay.start_readiness_watchdog();
        Ok(overlay)
    }

    pub fn notifier(&self) -> Arc<dyn ModifierKeyOverlayNotifier> {
        Arc::new(self.clone())
    }

    pub fn ready(&self, window_label: &str) -> Result<ModifierOverlaySnapshot, String> {
        if window_label != OVERLAY_WINDOW_LABEL {
            return Err("Modifier overlay state is only available to its overlay window.".into());
        }
        #[cfg(target_os = "windows")]
        return Err("The Windows modifier overlay does not use a WebView.".into());

        #[cfg(not(target_os = "windows"))]
        {
            let snapshot = {
                let mut state = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.ready = true;
                snapshot(&state)
            };
            if snapshot.labels.is_empty() {
                self.hide_if_current_empty(snapshot.revision)?;
            }
            Ok(snapshot)
        }
    }

    pub fn present(&self, window_label: &str, revision: u64) -> Result<(), String> {
        if window_label != OVERLAY_WINDOW_LABEL {
            return Err(
                "Modifier overlay presentation is only available to its overlay window.".into(),
            );
        }
        #[cfg(target_os = "windows")]
        {
            let _ = revision;
            Err("The Windows modifier overlay does not use a WebView.".into())
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _window_action = self
                .inner
                .window_actions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let action = {
                let state = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                presentation_action(&state, revision)
            };
            if action == PresentationAction::Show {
                self.position_window()?;
                self.inner
                    .window
                    .unminimize()
                    .map_err(|error| error.to_string())?;
                self.inner
                    .window
                    .show()
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }
    }

    pub fn end_session(&self) {
        self.end_control_session();
    }

    fn update(&self, active_modifiers: &[ModifierKey]) {
        let normalized = canonical_modifiers(active_modifiers);
        let snapshot = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.active_modifiers == normalized {
                return;
            }
            state.active_modifiers = normalized;
            state.revision = state.revision.wrapping_add(1);
            snapshot(&state)
        };
        #[cfg(not(target_os = "windows"))]
        if let Err(error) = self
            .inner
            .window
            .emit("modifier-overlay-changed", &snapshot)
        {
            self.report_failure(&error.to_string());
        }
        #[cfg(target_os = "windows")]
        if let Err(error) = self
            .inner
            .window
            .set_snapshot(snapshot.revision, snapshot.labels.clone())
        {
            self.report_failure(&error);
        }
        #[cfg(not(target_os = "windows"))]
        if snapshot.labels.is_empty() {
            if let Err(error) = self.hide_if_current_empty(snapshot.revision) {
                self.report_failure(&error);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn hide_if_current_empty(&self, revision: u64) -> Result<(), String> {
        let _window_action = self
            .inner
            .window_actions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let action = {
            let state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            presentation_action(&state, revision)
        };
        if action == PresentationAction::Hide {
            self.inner
                .window
                .hide()
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn start_readiness_watchdog(&self) {
        let overlay = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(READINESS_TIMEOUT).await;
            let should_report = {
                let mut state = overlay
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let should_report = readiness_watchdog_should_report(&state);
                if should_report {
                    state.readiness_failure_reported = true;
                }
                should_report
            };
            if should_report {
                overlay.report_failure("the overlay page did not initialize in time");
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    fn position_window(&self) -> Result<(), String> {
        let cursor = self
            .inner
            .window
            .cursor_position()
            .map_err(|error| error.to_string())?;
        let monitor = self
            .inner
            .window
            .monitor_from_point(cursor.x, cursor.y)
            .map_err(|error| error.to_string())?
            .or_else(|| self.inner.window.primary_monitor().ok().flatten())
            .ok_or_else(|| "the active display could not be resolved".to_string())?;
        let window_size = self
            .inner
            .window
            .outer_size()
            .map_err(|error| error.to_string())?;
        let position = overlay_position(
            monitor.work_area().position.x,
            monitor.work_area().position.y,
            monitor.work_area().size.width,
            window_size,
            monitor.scale_factor(),
        );
        self.inner
            .window
            .set_position(position)
            .map_err(|error| error.to_string())
    }

    fn report_failure(&self, error: &str) {
        set_activity(
            &self.inner.shared,
            ActivityKind::Error,
            format!("Modifier overlay was disabled: {error}"),
        );
        emit_state(&self.inner.app, &self.inner.shared);
    }
}

#[cfg(not(target_os = "windows"))]
fn presentation_action(state: &ModifierOverlayState, revision: u64) -> PresentationAction {
    if revision != state.revision {
        return PresentationAction::Ignore;
    }
    if state.active_modifiers.is_empty() {
        PresentationAction::Hide
    } else if state.ready {
        PresentationAction::Show
    } else {
        PresentationAction::Ignore
    }
}

#[cfg(not(target_os = "windows"))]
fn readiness_watchdog_should_report(state: &ModifierOverlayState) -> bool {
    !state.ready && !state.readiness_failure_reported
}

#[cfg(target_os = "macos")]
fn configure_platform_window(window: &WebviewWindow) -> Result<(), String> {
    window
        .with_webview(|webview| unsafe {
            let window: &objc2_app_kit::NSWindow = &*webview.ns_window().cast();
            macos_overlay_window::configure(window);
        })
        .map_err(|error| error.to_string())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn configure_platform_window(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

impl ModifierKeyOverlayNotifier for ModifierOverlay {
    fn set_active_modifiers(&self, active_modifiers: &[ModifierKey]) {
        self.update(active_modifiers);
    }

    fn end_control_session(&self) {
        let revision = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !state.active_modifiers.is_empty() {
                state.active_modifiers.clear();
                state.revision = state.revision.wrapping_add(1);
                #[cfg(not(target_os = "windows"))]
                {
                    let current = snapshot(&state);
                    if let Err(error) = self.inner.window.emit("modifier-overlay-changed", current)
                    {
                        self.report_failure(&error.to_string());
                    }
                }
            }
            state.revision
        };
        #[cfg(not(target_os = "windows"))]
        let _ = revision;
        #[cfg(target_os = "windows")]
        if let Err(error) = self.inner.window.set_snapshot(revision, Vec::new()) {
            self.report_failure(&error);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _window_action = self
                .inner
                .window_actions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(error) = self.inner.window.hide() {
                self.report_failure(&error.to_string());
            }
        }
    }
}

fn snapshot(state: &ModifierOverlayState) -> ModifierOverlaySnapshot {
    ModifierOverlaySnapshot {
        revision: state.revision,
        labels: labels_for_platform(&state.active_modifiers, current_label_style()),
    }
}

fn canonical_modifiers(active_modifiers: &[ModifierKey]) -> Vec<ModifierKey> {
    ModifierKey::DISPLAY_ORDER
        .into_iter()
        .filter(|key| active_modifiers.contains(key))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelStyle {
    Windows,
    Macos,
}

fn current_label_style() -> LabelStyle {
    if cfg!(target_os = "macos") {
        LabelStyle::Macos
    } else {
        LabelStyle::Windows
    }
}

fn labels_for_platform(active_modifiers: &[ModifierKey], style: LabelStyle) -> Vec<String> {
    canonical_modifiers(active_modifiers)
        .into_iter()
        .map(|key| {
            match (style, key) {
                (LabelStyle::Windows, ModifierKey::Ctrl) => "Ctrl",
                (LabelStyle::Windows, ModifierKey::Alt) => "Alt",
                (LabelStyle::Windows, ModifierKey::Shift) => "Shift",
                (LabelStyle::Windows, ModifierKey::Meta) => "Start",
                (LabelStyle::Macos, ModifierKey::Ctrl) => "Control",
                (LabelStyle::Macos, ModifierKey::Alt) => "Option",
                (LabelStyle::Macos, ModifierKey::Shift) => "Shift",
                (LabelStyle::Macos, ModifierKey::Meta) => "Command",
            }
            .to_string()
        })
        .collect()
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
fn overlay_position(
    work_x: i32,
    work_y: i32,
    work_width: u32,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> PhysicalPosition<i32> {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let margin = (OVERLAY_MARGIN * scale_factor).round() as i32;
    PhysicalPosition::new(
        work_x + work_width as i32 - window_size.width as i32 - margin,
        work_y + margin,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_modifier_order_and_removes_duplicates() {
        assert_eq!(
            canonical_modifiers(&[
                ModifierKey::Meta,
                ModifierKey::Shift,
                ModifierKey::Ctrl,
                ModifierKey::Shift,
            ]),
            vec![ModifierKey::Ctrl, ModifierKey::Shift, ModifierKey::Meta]
        );
    }

    #[test]
    fn maps_platform_native_labels() {
        let modifiers = ModifierKey::DISPLAY_ORDER;
        assert_eq!(
            labels_for_platform(&modifiers, LabelStyle::Windows),
            ["Ctrl", "Alt", "Shift", "Start"]
        );
        assert_eq!(
            labels_for_platform(&modifiers, LabelStyle::Macos),
            ["Control", "Option", "Shift", "Command"]
        );
    }

    #[test]
    fn positions_overlay_inside_scaled_and_negative_work_areas() {
        assert_eq!(
            overlay_position(-1920, 24, 1920, PhysicalSize::new(480, 70), 1.0),
            PhysicalPosition::new(-496, 40)
        );
        assert_eq!(
            overlay_position(0, 48, 3024, PhysicalSize::new(960, 140), 2.0),
            PhysicalPosition::new(2032, 80)
        );
    }

    #[test]
    fn invalid_scale_uses_one() {
        assert_eq!(
            overlay_position(0, 0, 1920, PhysicalSize::new(480, 70), f64::NAN),
            PhysicalPosition::new(1424, 16)
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn presentation_requires_ready_current_nonempty_state() {
        let mut state = ModifierOverlayState {
            revision: 4,
            active_modifiers: vec![ModifierKey::Ctrl],
            ..ModifierOverlayState::default()
        };
        assert_eq!(presentation_action(&state, 4), PresentationAction::Ignore);

        state.ready = true;
        assert_eq!(presentation_action(&state, 3), PresentationAction::Ignore);
        assert_eq!(presentation_action(&state, 4), PresentationAction::Show);

        state.active_modifiers.clear();
        assert_eq!(presentation_action(&state, 4), PresentationAction::Hide);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn stale_empty_revision_cannot_hide_newer_nonempty_state() {
        let state = ModifierOverlayState {
            revision: 5,
            active_modifiers: vec![ModifierKey::Shift],
            ready: true,
            ..ModifierOverlayState::default()
        };
        assert_eq!(presentation_action(&state, 4), PresentationAction::Ignore);
        assert_eq!(presentation_action(&state, 5), PresentationAction::Show);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn readiness_watchdog_reports_only_once_before_ready() {
        let mut state = ModifierOverlayState::default();
        assert!(readiness_watchdog_should_report(&state));
        state.readiness_failure_reported = true;
        assert!(!readiness_watchdog_should_report(&state));
        state.readiness_failure_reported = false;
        state.ready = true;
        assert!(!readiness_watchdog_should_report(&state));
    }
}
