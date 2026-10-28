//! Offline entitlements. Separate from pairing, accounts, and synced settings.
use crate::licence_key;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager};
const DAY: i64 = 86_400;
const TRIAL: i64 = 30 * DAY;
pub const LOCKED: &str = "Your trial has ended. Open Settings → Licence to enter a key.";
const SERVICE: &str = "com.enaboapps.switchify.pc.licence";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub configured: bool,
    pub status: String,
    pub days_remaining: u32,
    pub licence_id: Option<String>,
    pub message: String,
    pub storage_warning: bool,
    #[serde(skip)]
    pub access_until: Option<i64>,
}
impl View {
    pub fn allowed(&self, now: i64) -> bool {
        self.access_until.is_none_or(|end| now < end)
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    first_run: i64,
    last_seen: i64,
    revision: u64,
    key: Option<String>,
}
impl Record {
    fn new(now: i64) -> Self {
        Self {
            version: 1,
            first_run: now,
            last_seen: now,
            revision: 0,
            key: None,
        }
    }
    fn valid(&self) -> bool {
        self.version == 1
            && self.first_run >= 0
            && self.first_run <= self.last_seen
            && self
                .key
                .as_ref()
                .is_none_or(|k| k.len() <= licence_key::MAX_KEY_BYTES)
    }
    fn merge(file: Option<Self>, secure: Option<Self>) -> Option<Self> {
        match (file, secure) {
            (Some(a), Some(b)) => {
                let mut chosen = if a.revision >= b.revision {
                    a.clone()
                } else {
                    b.clone()
                };
                chosen.first_run = a.first_run.min(b.first_run);
                chosen.last_seen = a.last_seen.max(b.last_seen);
                Some(chosen)
            }
            (a, b) => a.or(b),
        }
    }
    fn view(&self, keys: &[(&str, [u8; 32])], now: i64) -> View {
        let effective = now.max(self.last_seen);
        let mut view = View {
            configured: true,
            status: "trial".into(),
            access_until: Some(self.first_run.saturating_add(TRIAL)),
            ..View::default()
        };
        if let Some(key) = &self.key {
            match licence_key::verify(key, keys, effective) {
                Ok(claims) => {
                    view.status = "licensed".into();
                    view.licence_id = Some(claims.licence_id);
                    view.access_until = claims.expires_at;
                    view.message = "Licensed for this computer. No sign-in or internet connection is required.".into();
                    return view;
                }
                Err(error) => view.message = error.to_string(),
            }
        }
        let left = view.access_until.unwrap().saturating_sub(effective).max(0);
        view.days_remaining = ((left + DAY - 1) / DAY) as u32;
        if left == 0 {
            view.status = "expired".into();
            view.access_until = Some(0);
            if view.message.is_empty() {
                view.message = LOCKED.into();
            }
        } else if view.message.is_empty() {
            view.message = match view.days_remaining {
                1 => "1 day left in your full trial.".into(),
                days => format!("{days} days left in your full trial."),
            };
        }
        view
    }
}
struct Data {
    record: Option<Record>,
    storage_warning: bool,
    recovery: bool,
    recovery_armed: bool,
    cleanup_pending: bool,
    last_allowed: bool,
    /// Cached by `view` so hot paths skip signature verification.
    access_until: Option<i64>,
}
impl Data {
    fn observe_access(&mut self, current: bool) -> bool {
        let expired = self.last_allowed && !current;
        self.last_allowed = current;
        self.cleanup_pending |= expired;
        expired
    }
}
pub struct Licensing {
    data: Mutex<Data>,
    path: PathBuf,
    keys: Vec<(String, [u8; 32])>,
    configured: bool,
}
fn now() -> i64 {
    crate::state::now_ms() / 1000
}
fn decode_record(text: &str) -> Result<Record, String> {
    if text.len() > 16_384 {
        return Err("Licence storage is unavailable.".into());
    }
    let record: Record =
        serde_json::from_str(text).map_err(|_| "Licence storage is unavailable.".to_string())?;
    record
        .valid()
        .then_some(record)
        .ok_or_else(|| "Licence storage is unavailable.".into())
}
fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, "record").map_err(|_| "Licence storage is unavailable.".into())
}
impl Licensing {
    fn open(path: PathBuf) -> Self {
        let config = option_env!("SWITCHIFY_LICENCE_PUBLIC_KEYS").unwrap_or("");
        let keys = serde_json::from_str::<std::collections::BTreeMap<String, String>>(config)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, value)| {
                let bytes: [u8; 32] = URL_SAFE_NO_PAD.decode(value).ok()?.try_into().ok()?;
                licence_key::valid_key_id(&id).then_some((id, bytes))
            })
            .collect::<Vec<_>>();
        // No production trust anchors in development/CI. A nonempty invalid
        // configuration fails closed, rather than granting an unlimited trial.
        let configured = !config.is_empty();
        let mut file_error = false;
        let file = match std::fs::read_to_string(&path) {
            Ok(text) => match decode_record(&text) {
                Ok(record) => Some(record),
                Err(_) => {
                    file_error = true;
                    None
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                file_error = true;
                None
            }
        };
        let mut secure_error = false;
        let secure = if configured {
            match entry().and_then(|entry| match entry.get_password() {
                Ok(text) => decode_record(&text).map(Some),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err("Licence storage is unavailable.".into()),
            }) {
                Ok(record) => record,
                Err(_) => {
                    secure_error = true;
                    None
                }
            }
        } else {
            None
        };
        let record = Record::merge(file, secure)
            .or_else(|| (!file_error && !secure_error).then(|| Record::new(now())));
        let service = Self {
            data: Mutex::new(Data {
                record,
                storage_warning: file_error || secure_error,
                recovery: false,
                recovery_armed: false,
                cleanup_pending: false,
                last_allowed: true,
                access_until: None,
            }),
            path,
            keys,
            configured,
        };
        if configured {
            let mut data = service.data.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(record) = &data.record {
                data.storage_warning |= service.save(record).unwrap_or(true);
            }
        }
        service.seed_access();
        service
    }
    /// Observe from the real state at launch. An install that is already
    /// expired has no active input to clean up and must not raise the window
    /// on every start, including hidden login starts.
    fn cached_allowed(&self, now: i64) -> bool {
        if !self.configured {
            return true;
        }
        self.data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .access_until
            .is_none_or(|end| now < end)
    }
    fn seed_access(&self) {
        let allowed = self.view().allowed(now());
        self.data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_allowed = allowed;
    }
    fn trust(&self) -> Vec<(&str, [u8; 32])> {
        self.keys
            .iter()
            .map(|(id, key)| (id.as_str(), *key))
            .collect()
    }
    fn save(&self, record: &Record) -> Result<bool, String> {
        let text =
            serde_json::to_string(record).map_err(|_| "Could not save the licence.".to_string())?;
        let parent = self.path.parent().ok_or("Could not save the licence.")?;
        std::fs::create_dir_all(parent).map_err(|_| "Could not save the licence.".to_string())?;
        let temp = parent.join(format!("licence-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp)
                .map_err(|_| "Could not save the licence.".to_string())?;
            file.write_all(text.as_bytes())
                .map_err(|_| "Could not save the licence.".to_string())?;
            file.sync_all()
                .map_err(|_| "Could not save the licence.".to_string())?;
            crate::storage::replace_file(&temp, &self.path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
            return Err("Could not save the licence.".into());
        }
        // File revision commits removal even if the keychain is temporarily locked.
        Ok(entry()
            .and_then(|entry| {
                entry
                    .set_password(&text)
                    .map_err(|_| "Keychain unavailable.".into())
            })
            .is_err())
    }
    pub fn view(&self) -> View {
        if !self.configured {
            return View {
                status: "unconfigured".into(),
                message: "Licensing is not enabled in this build.".into(),
                ..View::default()
            };
        }
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let mut view = if self.keys.is_empty() {
            View {
                configured: true,
                status: "unavailable".into(),
                message: "Licence verification is unavailable in this build. Contact support."
                    .into(),
                access_until: Some(0),
                ..View::default()
            }
        } else {
            data.record.as_ref().map(|r|r.view(&self.trust(),now())).unwrap_or_else(|| View { configured:true,status:"unavailable".into(),message:"Unlock your keychain and restart Switchify PC, or enter your licence again.".into(),access_until:Some(0),..View::default() })
        };
        view.storage_warning = data.storage_warning;
        data.access_until = view.access_until;
        view
    }
    fn redeem(&self, key: &str) -> Result<(), String> {
        if !self.configured {
            return Err("Licensing is not enabled in this build.".into());
        }
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let effective = now().max(data.record.as_ref().map_or(0, |r| r.last_seen));
        licence_key::verify(key, &self.trust(), effective).map_err(|e| e.to_string())?;
        let mut record = data
            .record
            .clone()
            .unwrap_or_else(|| Record::new(effective));
        record.key = Some(key.trim().to_owned());
        record.revision = record.revision.saturating_add(1);
        record.last_seen = effective;
        let warning = self.save(&record)?;
        data.record = Some(record);
        data.storage_warning = warning;
        drop(data);
        self.view();
        Ok(())
    }
    fn checkpoint(&self) {
        if !self.configured {
            return;
        }
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut record) = data.record.clone() {
            let time = now();
            if time.saturating_sub(record.last_seen) >= 60 {
                record.last_seen = time;
                data.storage_warning = self.save(&record).unwrap_or(true);
                data.record = Some(record);
            }
        }
    }
    fn remove(&self) -> Result<(), String> {
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let mut record = data
            .record
            .clone()
            .ok_or("Licence storage is unavailable.")?;
        record.key = None;
        record.revision = record.revision.saturating_add(1);
        record.last_seen = now().max(record.last_seen);
        let warning = self.save(&record)?;
        data.record = Some(record);
        data.storage_warning = warning;
        drop(data);
        self.view();
        Ok(())
    }
}
#[tauri::command]
pub fn paste_licence(app: AppHandle) -> Result<View, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let key = app
        .clipboard()
        .read_text()
        .map_err(|_| "Could not read a licence from the clipboard.".to_string())?;
    redeem_licence(app, key)
}
fn publish(app: &AppHandle) {
    let view = app.state::<Licensing>().view();
    let model = app.state::<crate::state::AppModel>();
    let changed = {
        let mut data = model.shared.lock().unwrap_or_else(|p| p.into_inner());
        let changed = data.state.licence != view;
        data.engine.set_access_deadline(view.access_until);
        data.state.licence = view;
        changed
    };
    if changed {
        crate::sync_tray_state(app, &model.snapshot());
        let _ = app.emit(crate::state::APP_STATE_EVENT, model.snapshot());
    }
}
/// Cheap check for scanning ticks and input dispatch. The cached deadline is
/// refreshed by every `view` call: at launch, each second from `publish`, and
/// after a key is redeemed or removed.
pub fn allowed(app: &AppHandle) -> bool {
    app.try_state::<Licensing>()
        .is_none_or(|l| l.cached_allowed(now()))
}
#[tauri::command]
pub fn redeem_licence(app: AppHandle, key: String) -> Result<View, String> {
    app.state::<Licensing>().redeem(&key)?;
    publish(&app);
    Ok(app.state::<Licensing>().view())
}
#[tauri::command]
pub fn remove_licence(app: AppHandle) -> Result<View, String> {
    app.state::<Licensing>().remove()?;
    publish(&app);
    Ok(app.state::<Licensing>().view())
}
#[tauri::command]
pub fn set_licence_recovery(app: AppHandle, active: bool) {
    let service = app.state::<Licensing>();
    let mut data = service.data.lock().unwrap_or_else(|p| p.into_inner());
    data.recovery = active;
}
/// Called on the scanner's main-thread tick. Expiry cleanup runs before any
/// further scan action. While locked, local switches only navigate our own UI.
pub fn recovery_tick(app: &AppHandle) -> bool {
    if allowed(app) {
        if let Some(licence) = app.try_state::<Licensing>() {
            let mut data = licence.data.lock().unwrap_or_else(|p| p.into_inner());
            if data.recovery_armed {
                app.state::<crate::switch_runtime::Controller>().stop();
                data.recovery_armed = false;
            }
        }
        return false;
    }
    let licence = app.state::<Licensing>();
    let mut data = licence.data.lock().unwrap_or_else(|p| p.into_inner());
    let focused = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_focused().unwrap_or(false));
    let controller = app.state::<crate::switch_runtime::Controller>();
    if !data.recovery || !focused {
        if data.recovery_armed {
            controller.stop();
            data.recovery_armed = false;
        }
        return true;
    }
    if data.recovery_armed && !controller.active_generation(controller.generation()) {
        data.recovery_armed = false;
    }
    if !data.recovery_armed {
        if controller.enable(true).is_ok() {
            data.recovery_armed = true;
        }
        return true;
    }
    let (events, _, generation) = controller.poll(app);
    for event in events {
        if let crate::switch_input::Event::Switch {
            generation: g,
            action: crate::switch_input::Action::Released,
            ..
        } = event
        {
            if g == generation {
                let _ = app.emit("licence-recovery-select", ());
            }
        }
    }
    true
}
pub fn install(app: &AppHandle) {
    let path = app.state::<crate::state::AppModel>().storage.licence_path();
    app.manage(Licensing::open(path));
    publish(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let handle = app.clone();
            if app
                .run_on_main_thread(move || {
                    handle.state::<Licensing>().checkpoint();
                    publish(&handle);
                    let current = allowed(&handle);
                    let service = handle.state::<Licensing>();
                    let expired = service
                        .data
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .observe_access(current);
                    if expired {
                        crate::show_main_window(&handle, Some("settings"));
                    }
                    let pending = service
                        .data
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .cleanup_pending;
                    if pending {
                        let model = handle.state::<crate::state::AppModel>();
                        let result = crate::platform_disconnect_all(&handle, &model.shared);
                        if result.is_ok() {
                            service
                                .data
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .cleanup_pending = false;
                        }
                    }
                })
                .is_err()
            {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trial_uses_exact_boundary_and_never_restarts_on_rollback() {
        let record = Record::new(1000);
        assert_eq!(record.view(&[], 1000).days_remaining, 30);
        assert_eq!(
            record.view(&[], 1000).message,
            "30 days left in your full trial."
        );
        assert_eq!(
            record.view(&[], 1000 + TRIAL - 1).message,
            "1 day left in your full trial."
        );
        assert!(record.view(&[], 1000 + TRIAL - 1).allowed(1000 + TRIAL - 1));
        assert!(!record.view(&[], 1000 + TRIAL).allowed(1000 + TRIAL));
        let mut advanced = record.clone();
        advanced.last_seen = 1000 + TRIAL;
        assert_eq!(advanced.view(&[], 1000).status, "expired");
    }
    #[test]
    fn mirror_reconciliation_preserves_trial_and_removal() {
        let old = Record {
            key: Some("old".into()),
            ..Record::new(1000)
        };
        let new = Record {
            key: None,
            revision: 1,
            ..Record::new(2000)
        };
        let merged = Record::merge(Some(new), Some(old)).unwrap();
        assert_eq!(merged.first_run, 1000);
        assert_eq!(merged.last_seen, 2000);
        assert!(merged.key.is_none());
    }
    #[test]
    fn main_thread_observations_cannot_skip_cleanup_between_allowed_and_expired() {
        let mut data = Data {
            record: None,
            storage_warning: false,
            recovery: false,
            recovery_armed: false,
            cleanup_pending: false,
            last_allowed: true,
            access_until: None,
        };
        assert!(!data.observe_access(true));
        // An expiry or removal between callbacks cannot update last_allowed.
        assert!(data.observe_access(false));
        assert!(data.cleanup_pending);
        assert!(!data.observe_access(false));
        assert!(data.cleanup_pending); // failed cleanup stays pending
        data.cleanup_pending = false;
        assert!(!data.observe_access(true));
        assert!(data.observe_access(false));
    }
    fn configured_service(record: Record) -> Licensing {
        Licensing {
            data: Mutex::new(Data {
                record: Some(record),
                storage_warning: false,
                recovery: false,
                recovery_armed: false,
                cleanup_pending: false,
                last_allowed: true,
                access_until: None,
            }),
            path: std::env::temp_dir().join("switchify-licence-test-never-written.json"),
            keys: vec![("test".into(), [0; 32])],
            configured: true,
        }
    }
    #[test]
    fn launching_an_expired_install_does_not_queue_cleanup() {
        let mut expired = Record::new(0);
        expired.last_seen = TRIAL;
        let service = configured_service(expired);
        service.seed_access();
        let mut data = service.data.lock().unwrap();
        assert!(!data.last_allowed);
        assert!(!data.observe_access(false));
        assert!(!data.cleanup_pending);
        drop(data);
        let service = configured_service(Record::new(now()));
        service.seed_access();
        let mut data = service.data.lock().unwrap();
        assert!(data.last_allowed);
        assert!(data.observe_access(false));
    }
    #[test]
    fn cached_access_follows_the_last_computed_view() {
        let service = configured_service(Record::new(now()));
        service.view();
        assert!(service.cached_allowed(now()));
        let mut expired = Record::new(0);
        expired.last_seen = TRIAL;
        service.data.lock().unwrap().record = Some(expired);
        assert!(service.cached_allowed(now()), "stale until refreshed");
        service.view();
        assert!(!service.cached_allowed(now()));
    }
    #[test]
    fn rejects_corrupt_trial_records() {
        let mut record = Record::new(1000);
        record.first_run = 2000;
        assert!(decode_record(&serde_json::to_string(&record).unwrap()).is_err());
    }
}
