use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{
    Emitter, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::input::ModifierKey;
use crate::state::{emit_state, set_activity, ActivityKind, SharedModel};

const OVERLAY_WINDOW_LABEL: &str = "modifier-overlay";
const OVERLAY_WIDTH: f64 = 480.0;
const OVERLAY_HEIGHT: f64 = 70.0;
const OVERLAY_MARGIN: f64 = 16.0;

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
    window: WebviewWindow,
    state: Mutex<ModifierOverlayState>,
}

#[derive(Default)]
struct ModifierOverlayState {
    revision: u64,
    active_modifiers: Vec<ModifierKey>,
    ready: bool,
}

impl ModifierOverlay {
    pub fn install(app: tauri::AppHandle, shared: SharedModel) -> Result<Self, String> {
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
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?;
        window
            .set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            inner: Arc::new(ModifierOverlayInner {
                app,
                shared,
                window,
                state: Mutex::new(ModifierOverlayState::default()),
            }),
        })
    }

    pub fn notifier(&self) -> Arc<dyn ModifierKeyOverlayNotifier> {
        Arc::new(self.clone())
    }

    pub fn ready(&self, window_label: &str) -> Result<ModifierOverlaySnapshot, String> {
        if window_label != OVERLAY_WINDOW_LABEL {
            return Err("Modifier overlay state is only available to its overlay window.".into());
        }
        let snapshot = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.ready = true;
            snapshot(&state)
        };
        if !snapshot.labels.is_empty() {
            self.position_window()?;
        }
        Ok(snapshot)
    }

    pub fn end_session(&self) {
        self.end_control_session();
    }

    fn update(&self, active_modifiers: &[ModifierKey]) {
        let normalized = canonical_modifiers(active_modifiers);
        let (snapshot, ready) = {
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
            (snapshot(&state), state.ready)
        };
        if let Err(error) = self
            .inner
            .window
            .emit("modifier-overlay-changed", &snapshot)
        {
            self.report_failure(&error.to_string());
        }
        if snapshot.labels.is_empty() {
            if let Err(error) = self.inner.window.hide() {
                self.report_failure(&error.to_string());
            }
        } else if ready {
            if let Err(error) = self.position_window().and_then(|_| {
                self.inner
                    .window
                    .show()
                    .map_err(|window_error| window_error.to_string())
            }) {
                self.report_failure(&error);
            }
        }
    }

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

impl ModifierKeyOverlayNotifier for ModifierOverlay {
    fn set_active_modifiers(&self, active_modifiers: &[ModifierKey]) {
        self.update(active_modifiers);
    }

    fn end_control_session(&self) {
        {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !state.active_modifiers.is_empty() {
                state.active_modifiers.clear();
                state.revision = state.revision.wrapping_add(1);
                let current = snapshot(&state);
                if let Err(error) = self.inner.window.emit("modifier-overlay-changed", current) {
                    self.report_failure(&error.to_string());
                }
            }
        }
        if let Err(error) = self.inner.window.hide() {
            self.report_failure(&error.to_string());
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
}
