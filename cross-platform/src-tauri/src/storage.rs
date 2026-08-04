use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

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
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|error| error.to_string())?;
        }
        fs::rename(temp, &self.path).map_err(|error| error.to_string())
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
}
