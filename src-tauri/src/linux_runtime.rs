//! Development foundation. No Bluetooth service, input device or permission
//! prompt is opened until the Linux adapters have been implemented and tested.
use tauri::AppHandle;

use crate::state::{emit_state, AccessibilityState, BluetoothState, SharedModel};

const UNAVAILABLE: &str = "Bluetooth control is not implemented in this Linux development build.";

fn reset_session(shared: &SharedModel) {
    let mut data = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    data.engine.reset_transport_session();
    data.state.pending_pairings.clear();
    data.state.connected_device_name = None;
    data.state.bluetooth = BluetoothState::Unsupported;
    data.state.accessibility = AccessibilityState::Unavailable;
}

pub fn install(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    reset_session(&shared);
    emit_state(&app, &shared);
    Ok(())
}

pub fn shutdown(_app: &AppHandle, shared: &SharedModel) {
    reset_session(shared);
}

pub fn check_accessibility(
    app: &AppHandle,
    shared: &SharedModel,
    _prompt: bool,
) -> Result<(), String> {
    reset_session(shared);
    emit_state(app, shared);
    Ok(())
}

pub fn approve_pairing(
    _app: &AppHandle,
    _shared: &SharedModel,
    _request_id: &str,
) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

pub fn reject_pairing(
    _app: &AppHandle,
    _shared: &SharedModel,
    _request_id: &str,
) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

pub fn disconnect_all(app: &AppHandle, shared: &SharedModel) -> Result<(), String> {
    reset_session(shared);
    emit_state(app, shared);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppModel;
    use crate::storage::AppStorage;

    #[test]
    fn unavailable_runtime_clears_session_but_preserves_pairing_credentials() {
        let root = std::env::temp_dir().join(format!("switchify-linux-{}", uuid::Uuid::new_v4()));
        let model = AppModel::with_storage_for_test(AppStorage::at(root.join("state.json")));
        {
            let mut data = model.shared.lock().unwrap();
            data.engine
                .set_paired_token("test-device".into(), "test-token".into());
            data.state.bluetooth = BluetoothState::Connected;
            data.state.accessibility = AccessibilityState::Granted;
            data.state.connected_device_name = Some("Test device".into());
        }
        reset_session(&model.shared);
        reset_session(&model.shared);
        let data = model.shared.lock().unwrap();
        assert_eq!(data.state.bluetooth, BluetoothState::Unsupported);
        assert_eq!(data.state.accessibility, AccessibilityState::Unavailable);
        assert!(data.state.connected_device_name.is_none());
        assert!(data.state.pending_pairings.is_empty());
        assert_eq!(data.engine.token_for("test-device"), Some("test-token"));
        drop(data);
        let _ = std::fs::remove_dir_all(root);
    }
}
