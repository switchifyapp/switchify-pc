use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::{AppSettings, PairedDeviceView, SwitchProfile};

const SCHEMA_VERSION: u32 = 1;
const KEYRING_SERVICE: &str = "com.enaboapps.switchify.pc.preview.pairing";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub schema_version: u32,
    pub desktop_id: Option<String>,
    pub paired_devices: Vec<PairedDeviceView>,
    pub settings: AppSettings,
    pub profiles: Vec<SwitchProfile>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            desktop_id: None,
            paired_devices: vec![],
            settings: AppSettings::default(),
            profiles: vec![],
        }
    }
}

#[derive(Debug)]
pub struct AppStorage {
    path: PathBuf,
}

impl AppStorage {
    pub fn new() -> Self {
        let path = ProjectDirs::from("com", "Enabo Apps", "Switchify PC Preview")
            .map(|dirs| dirs.config_dir().join("preview-state.json"))
            .unwrap_or_else(|| PathBuf::from("switchify-pc-preview-state.json"));
        Self { path }
    }

    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<PersistedState, String> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }
        let raw = fs::read_to_string(&self.path).map_err(|error| error.to_string())?;
        let state: PersistedState =
            serde_json::from_str(&raw).map_err(|error| error.to_string())?;
        if state.schema_version != SCHEMA_VERSION {
            return Err("Unsupported preview state schema.".into());
        }
        Ok(state)
    }

    pub fn save(&self, state: &PersistedState) -> Result<(), String> {
        let mut next = state.clone();
        next.schema_version = SCHEMA_VERSION;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let temp = self.path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(&next).map_err(|error| error.to_string())? + "\n";
        fs::write(&temp, json).map_err(|error| error.to_string())?;
        replace_file(&temp, &self.path)
    }

    pub fn save_pairing_token(&self, device_id: &str, token: &str) -> Result<(), String> {
        keyring::Entry::new(KEYRING_SERVICE, device_id)
            .map_err(|error| error.to_string())?
            .set_password(token)
            .map_err(|error| error.to_string())
    }

    pub fn load_pairing_token(&self, device_id: &str) -> Option<String> {
        keyring::Entry::new(KEYRING_SERVICE, device_id)
            .ok()?
            .get_password()
            .ok()
    }

    pub fn delete_pairing_token(&self, device_id: &str) -> Result<(), String> {
        let entry =
            keyring::Entry::new(KEYRING_SERVICE, device_id).map_err(|error| error.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn export_diagnostics(&self, diagnostics: &Value) -> Result<PathBuf, String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Diagnostics directory is unavailable.".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let path = parent.join("switchify-preview-diagnostics.json");
        let json =
            serde_json::to_string_pretty(diagnostics).map_err(|error| error.to_string())? + "\n";
        fs::write(&path, json).map_err(|error| error.to_string())?;
        Ok(path)
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_round_trips_in_an_isolated_directory() {
        let root = std::env::temp_dir().join(format!("switchify-preview-{}", uuid::Uuid::new_v4()));
        let store = AppStorage::at(root.join("state.json"));
        let state = PersistedState {
            desktop_id: Some("desktop-1".into()),
            ..PersistedState::default()
        };
        store.save(&state).unwrap();
        assert_eq!(
            store.load().unwrap().desktop_id.as_deref(),
            Some("desktop-1")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn state_without_overlay_visibility_uses_the_persistent_default() {
        let mut value = serde_json::to_value(PersistedState::default()).unwrap();
        value["settings"]
            .as_object_mut()
            .unwrap()
            .remove("cursorOverlayVisibility");
        let state: PersistedState = serde_json::from_value(value).unwrap();
        assert_eq!(state.settings.cursor_overlay_visibility, "whileControlling");
    }

    #[test]
    fn state_preserves_an_explicit_transient_overlay_preference() {
        let mut value = serde_json::to_value(PersistedState::default()).unwrap();
        value["settings"]["cursorOverlayVisibility"] = serde_json::json!("onInput");
        let state: PersistedState = serde_json::from_value(value).unwrap();
        assert_eq!(state.settings.cursor_overlay_visibility, "onInput");
    }

    #[test]
    fn state_without_repeat_acceleration_uses_the_shipping_default() {
        let mut value = serde_json::to_value(PersistedState::default()).unwrap();
        value["settings"]
            .as_object_mut()
            .unwrap()
            .remove("mouseRepeatAccelerationDurationMs");
        let state: PersistedState = serde_json::from_value(value).unwrap();
        assert_eq!(state.settings.mouse_repeat_acceleration_duration_ms, 1000);
    }

    #[test]
    fn state_preserves_an_explicit_repeat_acceleration() {
        let mut value = serde_json::to_value(PersistedState::default()).unwrap();
        value["settings"]["mouseRepeatAccelerationDurationMs"] = serde_json::json!(2000);
        let state: PersistedState = serde_json::from_value(value).unwrap();
        assert_eq!(state.settings.mouse_repeat_acceleration_duration_ms, 2000);
    }
}
