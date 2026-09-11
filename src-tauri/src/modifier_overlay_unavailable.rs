//! Linux deliberately creates no feedback window until overlay support is qualified.
use serde::Serialize;

use crate::input::ModifierKey;
use crate::state::SharedModel;

// Keep the input seam usable by the shared fake-injector tests on Linux.
#[allow(dead_code)]
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

pub struct ModifierOverlay;

impl ModifierOverlay {
    pub fn install(_app: tauri::AppHandle, _shared: SharedModel) -> Result<Self, String> {
        Ok(Self)
    }

    pub fn ready(&self, _window_label: &str) -> Result<ModifierOverlaySnapshot, String> {
        Err("Modifier feedback is unavailable in this Linux development build.".into())
    }

    pub fn present(&self, _window_label: &str, _revision: u64) -> Result<(), String> {
        Err("Modifier feedback is unavailable in this Linux development build.".into())
    }

    pub fn end_session(&self) {}
}
