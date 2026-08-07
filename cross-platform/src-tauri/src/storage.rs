use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::{AppSettings, PairedDeviceView, SwitchProfile};

const SCHEMA_VERSION: u32 = 1;
#[cfg(not(target_os = "macos"))]
const KEYRING_SERVICE: &str = "com.enaboapps.switchify.pc.preview.pairing";

trait PairingTokenStore: std::fmt::Debug + Send + Sync {
    fn save(&self, device_id: &str, token: &str) -> Result<(), String>;
    fn load(&self, device_id: &str) -> Option<String>;
    fn delete(&self, device_id: &str) -> Result<(), String>;
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct PlatformPairingTokenStore {
    path: PathBuf,
    lock: std::sync::Mutex<()>,
}

#[cfg(target_os = "macos")]
impl PlatformPairingTokenStore {
    fn new(state_path: &Path) -> Self {
        Self {
            path: state_path.with_file_name("pairing-tokens.json"),
            lock: std::sync::Mutex::new(()),
        }
    }

    fn read(&self) -> Result<std::collections::HashMap<String, String>, String> {
        if !self.path.exists() {
            return Ok(std::collections::HashMap::new());
        }
        let raw = fs::read_to_string(&self.path).map_err(|error| error.to_string())?;
        serde_json::from_str(&raw).map_err(|error| error.to_string())
    }

    fn write(&self, tokens: &std::collections::HashMap<String, String>) -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Pairing-token directory is unavailable.".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
        let temp = self.path.with_extension("json.tmp");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|error| error.to_string())?;
        serde_json::to_writer_pretty(&mut file, tokens).map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temp, &self.path).map_err(|error| error.to_string())?;
        fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "macos")]
impl PairingTokenStore for PlatformPairingTokenStore {
    fn save(&self, device_id: &str, token: &str) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut tokens = self.read()?;
        tokens.insert(device_id.to_owned(), token.to_owned());
        self.write(&tokens)
    }

    fn load(&self, device_id: &str) -> Option<String> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.read().ok()?.get(device_id).cloned()
    }

    fn delete(&self, device_id: &str) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut tokens = self.read()?;
        if tokens.remove(device_id).is_none() {
            return Ok(());
        }
        self.write(&tokens)
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default)]
struct PlatformPairingTokenStore;

#[cfg(not(target_os = "macos"))]
impl PlatformPairingTokenStore {
    fn entry(device_id: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, device_id).map_err(|error| error.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
impl PairingTokenStore for PlatformPairingTokenStore {
    fn save(&self, device_id: &str, token: &str) -> Result<(), String> {
        Self::entry(device_id)?
            .set_password(token)
            .map_err(|error| error.to_string())
    }

    fn load(&self, device_id: &str) -> Option<String> {
        Self::entry(device_id).ok()?.get_password().ok()
    }

    fn delete(&self, device_id: &str) -> Result<(), String> {
        let entry = Self::entry(device_id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(target_os = "macos")]
fn platform_pairing_token_store(state_path: &Path) -> Box<dyn PairingTokenStore> {
    Box::new(PlatformPairingTokenStore::new(state_path))
}

#[cfg(not(target_os = "macos"))]
fn platform_pairing_token_store(_state_path: &Path) -> Box<dyn PairingTokenStore> {
    Box::new(PlatformPairingTokenStore)
}

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
    pairing_tokens: Box<dyn PairingTokenStore>,
}

impl AppStorage {
    pub fn new() -> Self {
        let path = ProjectDirs::from("com", "Enabo Apps", "Switchify PC Preview")
            .map(|dirs| dirs.config_dir().join("preview-state.json"))
            .unwrap_or_else(|| PathBuf::from("switchify-pc-preview-state.json"));
        let pairing_tokens = platform_pairing_token_store(&path);
        Self {
            path,
            pairing_tokens,
        }
    }

    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        let pairing_tokens = platform_pairing_token_store(&path);
        Self {
            path,
            pairing_tokens,
        }
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
        self.pairing_tokens.save(device_id, token)
    }

    pub fn load_pairing_token(&self, device_id: &str) -> Option<String> {
        self.pairing_tokens.load(device_id)
    }

    pub fn delete_pairing_token(&self, device_id: &str) -> Result<(), String> {
        self.pairing_tokens.delete(device_id)
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
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct MemoryPairingTokenStore {
        tokens: Mutex<HashMap<String, String>>,
    }

    impl PairingTokenStore for MemoryPairingTokenStore {
        fn save(&self, device_id: &str, token: &str) -> Result<(), String> {
            self.tokens
                .lock()
                .unwrap()
                .insert(device_id.to_owned(), token.to_owned());
            Ok(())
        }

        fn load(&self, device_id: &str) -> Option<String> {
            self.tokens.lock().unwrap().get(device_id).cloned()
        }

        fn delete(&self, device_id: &str) -> Result<(), String> {
            self.tokens.lock().unwrap().remove(device_id);
            Ok(())
        }
    }

    fn isolated_storage(pairing_tokens: Box<dyn PairingTokenStore>) -> AppStorage {
        AppStorage {
            path: std::env::temp_dir()
                .join(format!("switchify-preview-{}.json", uuid::Uuid::new_v4())),
            pairing_tokens,
        }
    }

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
    fn pairing_token_operations_use_the_injected_secure_store() {
        let store = isolated_storage(Box::<MemoryPairingTokenStore>::default());

        assert_eq!(store.load_pairing_token("android-1"), None);
        store
            .save_pairing_token("android-1", "secret-token")
            .unwrap();
        assert_eq!(
            store.load_pairing_token("android-1").as_deref(),
            Some("secret-token")
        );
        store.delete_pairing_token("android-1").unwrap();
        assert_eq!(store.load_pairing_token("android-1"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_pairing_tokens_persist_with_user_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "switchify-preview-pairing-tokens-{}",
            uuid::Uuid::new_v4()
        ));
        let state_path = root.join("preview-state.json");
        let first = AppStorage::at(state_path.clone());
        first
            .save_pairing_token("android-1", "persistent-token")
            .unwrap();

        let token_path = root.join("pairing-tokens.json");
        assert_eq!(
            fs::metadata(&token_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );

        let rebuilt = AppStorage::at(state_path);
        assert_eq!(
            rebuilt.load_pairing_token("android-1").as_deref(),
            Some("persistent-token")
        );
        rebuilt.delete_pairing_token("android-1").unwrap();
        assert_eq!(rebuilt.load_pairing_token("android-1"), None);
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
