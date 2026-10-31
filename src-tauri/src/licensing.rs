//! Device entitlements. Separate from pairing, accounts, and synced settings.
use crate::{licence_activation as activation, licence_key, licence_lease};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager};
const DAY: i64 = 86_400;
const TRIAL: i64 = 30 * DAY;
pub const LOCKED: &str = "Your trial has ended. Open Settings → Licence to enter a key.";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub configured: bool,
    pub status: String,
    pub days_remaining: u32,
    pub licence_id: Option<String>,
    pub message: String,
    pub storage_warning: bool,
    pub offline_days_remaining: Option<u32>,
    pub last_checked_at: Option<i64>,
    pub can_transfer: bool,
    pub locked: bool,
    #[serde(skip)]
    pub access_until: Option<i64>,
}
impl View {
    pub fn allowed(&self, now: i64) -> bool {
        self.access_until.is_none_or(|end| now < end)
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    first_run: i64,
    last_seen: i64,
    #[serde(default)]
    lease_seen: i64,
    revision: u64,
    key: Option<String>,
    #[serde(default)]
    pending_key: Option<String>,
    #[serde(default)]
    lease: Option<String>,
    #[serde(default)]
    installation: Option<activation::Installation>,
    #[serde(default)]
    generation: u64,
    #[serde(default)]
    last_checked: Option<i64>,
    #[serde(default)]
    terminal_status: Option<String>,
    #[serde(default)]
    last_outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local_signature: Option<String>,
}
impl Record {
    fn new(now: i64) -> Self {
        Self {
            version: 3,
            first_run: now,
            last_seen: now,
            lease_seen: 0,
            revision: 0,
            key: None,
            pending_key: None,
            lease: None,
            installation: None,
            generation: 0,
            last_checked: None,
            terminal_status: None,
            last_outcome: None,
            local_signature: None,
        }
    }
    fn valid(&self) -> bool {
        matches!(self.version, 1..=3)
            && self.first_run >= 0
            && self.first_run <= self.last_seen
            && self.lease_seen >= 0
            && self.local_signature.as_ref().is_none_or(|s| s.len() <= 128)
            && self
                .key
                .as_ref()
                .is_none_or(|k| k.len() <= licence_key::MAX_KEY_BYTES)
            && self
                .pending_key
                .as_ref()
                .is_none_or(|k| k.len() <= licence_key::MAX_KEY_BYTES)
            && self.lease.as_ref().is_none_or(|k| k.len() <= 4096)
            && self
                .terminal_status
                .as_ref()
                .is_none_or(|s| matches!(s.as_str(), "revoked" | "deactivated"))
            && self
                .installation
                .as_ref()
                .is_none_or(|i| uuid::Uuid::parse_str(&i.id).is_ok() && i.public_key.len() == 43)
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
                // Only the same signed allowance shares a clock epoch. A fresh
                // server reply may legitimately recover from a forward clock jump.
                if a.lease.is_some() && a.lease == b.lease && a.installation == b.installation {
                    chosen.lease_seen = a.lease_seen.max(b.lease_seen);
                }
                Some(chosen)
            }
            (a, b) => a.or(b),
        }
    }
    fn cache_bytes(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.local_signature = None;
        serde_json::to_vec(&unsigned).expect("fixed local licence record serializes")
    }
    fn authenticated(&self, identity: &activation::Identity) -> bool {
        self.installation.as_ref() == Some(&identity.installation)
            && self
                .local_signature
                .as_ref()
                .is_some_and(|seal| identity.verify_cache(&self.cache_bytes(), seal))
    }
    fn seal(&mut self, identity: &activation::Identity) {
        self.local_signature = Some(identity.seal_cache(&self.cache_bytes()));
    }
    fn cached_paid(&self) -> bool {
        self.lease.is_some() || self.terminal_status.is_some()
    }
    fn reconcile(
        file: Option<Self>,
        secure: Option<Self>,
        identity: &activation::Identity,
    ) -> Option<Self> {
        let trusted_file = file.as_ref().filter(|r| r.authenticated(identity)).cloned();
        let trusted_secure = secure
            .as_ref()
            .filter(|r| r.authenticated(identity))
            .cloned();
        let mut chosen = if trusted_file.is_some() || trusted_secure.is_some() {
            Self::merge(trusted_file, trusted_secure)
        } else {
            Self::merge(file.clone(), secure.clone()).map(|mut record| {
                // Old/unsealed records retain purchase and trial history for
                // online recovery, but cannot carry offline paid authority.
                record.lease = None;
                record.revision = 0;
                record.lease_seen = 0;
                record.local_signature = None;
                record
            })
        }?;
        for old in [file, secure].into_iter().flatten() {
            chosen.first_run = chosen.first_run.min(old.first_run);
            chosen.last_seen = chosen.last_seen.max(old.last_seen);
        }
        // Seal only records already authenticated above, never an imported lease.
        if chosen.local_signature.is_some() {
            chosen.seal(identity);
        }
        Some(chosen)
    }
    fn view(&self, keys: &[(&str, [u8; 32])], leases: &licence_lease::LeaseKeys, now: i64) -> View {
        let effective = now.max(self.last_seen);
        let trial_end = self.first_run.saturating_add(TRIAL);
        let trial_left = trial_end.saturating_sub(effective).max(0);
        let mut view = View {
            configured: true,
            status: "trial".into(),
            access_until: Some(trial_end),
            days_remaining: ((trial_left + DAY - 1) / DAY) as u32,
            last_checked_at: self.last_checked,
            can_transfer: self.last_outcome.as_deref() == Some("seat_taken"),
            ..View::default()
        };
        if let Some(key) = &self.key {
            let paid_time = now.max(self.lease_seen);
            match licence_key::verify(key, keys, paid_time) {
                Ok(claims) => {
                    view.licence_id = Some(claims.licence_id.clone());
                    if let Some(status) = &self.terminal_status {
                        view.status = status.clone();
                        view.access_until = Some(0);
                        view.locked = true;
                        view.message=if status=="revoked" { "This licence has been revoked. Contact support if you need help." } else { "This computer is no longer activated. Check your licence or move it back to this computer." }.into();
                        return view;
                    }
                    if let (Some(lease), Some(installation)) = (&self.lease, &self.installation) {
                        let binding = licence_lease::Binding {
                            licence_id: &claims.licence_id,
                            installation_id: &installation.id,
                            installation_key: &installation.public_key,
                        };
                        if let Ok(lease) =
                            licence_lease::verify_lease(lease, leases, &binding, paid_time)
                        {
                            if lease.generation >= self.generation {
                                view.status = "licensed".into();
                                view.access_until = Some(lease.expires_at);
                                let days = ((lease.expires_at - paid_time + DAY - 1) / DAY) as u32;
                                view.offline_days_remaining = Some(days);
                                view.message = if days <= 7 {
                                    format!("Licensed. Connect to the internet within {days} {} to keep using Switchify PC.",if days==1 {"day"} else {"days"})
                                } else {
                                    "Licensed for this computer. Checks automatically while online."
                                        .into()
                                };
                                return view;
                            }
                        }
                        view.status = "lease_expired".into();
                        view.access_until = Some(0);
                        view.locked = true;
                        view.message="Your offline allowance has ended. Connect to the internet, then choose Check licence now.".into();
                        return view;
                    }
                    view.status = "needs_activation".into();
                    view.message="Connect to the internet, then choose Check licence now to activate this computer.".into();
                }
                Err(error) => view.message = error.to_string(),
            }
        }
        if trial_left == 0 {
            if view.status == "trial" {
                view.status = "expired".into();
            }
            view.access_until = Some(0);
            view.locked = true;
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
struct OfflineClock {
    epoch: i64,
    started: std::time::Instant,
}
impl OfflineClock {
    fn new(epoch: i64) -> Self {
        Self {
            epoch,
            started: std::time::Instant::now(),
        }
    }
    fn at(&self, wall: i64, elapsed: std::time::Duration) -> i64 {
        wall.max(
            self.epoch
                .saturating_add(elapsed.as_secs().min(i64::MAX as u64) as i64),
        )
    }
    fn time(&self, wall: i64) -> i64 {
        self.at(wall, self.started.elapsed())
    }
    fn advance(&mut self, minimum: i64) {
        *self = Self::new(minimum.max(self.time(now())));
    }
}
struct Data {
    record: Option<Record>,
    storage_warning: bool,
    recovery: bool,
    recovery_armed: bool,
    cleanup_pending: bool,
    last_allowed: bool,
    /// Cached by `view` so hot paths skip signature verification. Starts
    /// closed; `open` seeds it before the service is managed.
    access_until: Option<i64>,
    /// Consecutive failed keychain retries; the next retry waits
    /// `60 * 2^min(n, 6)` seconds so a locked keychain is not prompted every minute.
    secure_failures: u32,
    next_secure_retry: i64,
    file_dirty: bool,
    file_warning: bool,
    mirror_dirty: bool,
    mirror_loaded: bool,
    identity: Option<Arc<activation::Identity>>,
    clock: OfflineClock,
}
impl Data {
    fn observe_access(&mut self, current: bool) -> bool {
        let expired = self.last_allowed && !current;
        self.last_allowed = current;
        self.cleanup_pending |= expired;
        expired
    }
    /// Whether this checkpoint should also attempt the keychain mirror.
    fn retry_secure_due(&self, time: i64) -> bool {
        self.storage_warning && time >= self.next_secure_retry
    }
    /// Apply a checkpoint `save` outcome. `Err` is a file failure (the keychain
    /// was not tried), `Ok(true)` a keychain failure, `Ok(false)` success.
    /// A warning only clears when the keychain was actually retried, so a
    /// stale mirror keeps its warning across the non-retry minutes.
    fn record_checkpoint(&mut self, time: i64, retried_secure: bool, outcome: Result<bool, ()>) {
        match outcome {
            Err(()) => self.storage_warning = true,
            Ok(true) => {
                self.storage_warning = true;
                self.secure_failures = self.secure_failures.saturating_add(1);
                self.next_secure_retry = time + 60 * (1i64 << self.secure_failures.min(6));
            }
            Ok(false) if retried_secure => {
                self.storage_warning = false;
                self.secure_failures = 0;
                self.next_secure_retry = 0;
            }
            Ok(false) => {}
        }
    }
}
pub struct Licensing {
    data: Arc<Mutex<Data>>,
    path: PathBuf,
    keys: Vec<(String, [u8; 32])>,
    configured: bool,
    lease_keys: licence_lease::LeaseKeys,
    config: Option<activation::Config>,
    transport: Arc<dyn activation::Transport>,
    identity: Arc<dyn activation::IdentityStore>,
    mirror: Arc<dyn activation::MirrorStore>,
    operation: tokio::sync::Mutex<()>,
    file_operation: tokio::sync::Mutex<()>,
    mirror_operation: Arc<tokio::sync::Mutex<()>>,
    mirror_read_operation: tokio::sync::Mutex<()>,
}
fn now() -> i64 {
    crate::state::now_ms() / 1000
}
fn parse_keys(config: &str) -> Vec<(String, [u8; 32])> {
    let Ok(map) = serde_json::from_str::<std::collections::BTreeMap<String, String>>(config) else {
        return vec![];
    };
    let mut keys = Vec::new();
    for (id, value) in map {
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(value) else {
            return vec![];
        };
        let Ok(bytes) = bytes.try_into() else {
            return vec![];
        };
        if !licence_key::valid_key_id(&id) {
            return vec![];
        }
        keys.push((id, bytes));
    }
    keys
}
fn decode_record(text: &str) -> Result<Record, String> {
    if text.len() > 16_384 {
        return Err("Licence storage is unavailable.".into());
    }
    let mut record: Record =
        serde_json::from_str(text).map_err(|_| "Licence storage is unavailable.".to_string())?;
    let valid = record.valid();
    record.version = 3;
    valid
        .then_some(record)
        .ok_or_else(|| "Licence storage is unavailable.".into())
}
/// A missing file with a working keychain, or a missing file with an
/// unavailable keychain, starts the trial: nothing readable says otherwise, and
/// the next launch with the keychain back reconciles to the earliest first run.
/// An unreadable file fails closed until it is repaired.
fn initial_record(
    file: Option<Record>,
    secure: Option<Record>,
    file_error: bool,
    now: i64,
) -> Option<Record> {
    Record::merge(file, secure).or_else(|| (!file_error).then(|| Record::new(now)))
}
/// The keychain mirror, if present. Errors cover a locked or denied keychain
/// and a corrupt record alike; callers treat both as "unavailable for now".
fn read_secure(store: &dyn activation::MirrorStore) -> Result<Option<Record>, String> {
    store.read()?.map(|text| decode_record(&text)).transpose()
}
fn write_record_file(path: &std::path::Path, record: &Record) -> Result<(), String> {
    let text =
        serde_json::to_string(record).map_err(|_| "Could not save the licence.".to_string())?;
    let parent = path.parent().ok_or("Could not save the licence.")?;
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
        crate::storage::replace_file(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return Err("Could not save the licence.".into());
    }
    Ok(())
}
impl Licensing {
    fn open(path: PathBuf) -> Self {
        let config = option_env!("SWITCHIFY_LICENCE_PUBLIC_KEYS").unwrap_or("");
        let keys = parse_keys(config);
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
        // The file is sufficient to open. Read and heal the keychain mirror on
        // a worker after launch: a keychain dialog must never hold input locks.
        let record = initial_record(file, None, file_error, now());
        let service = Self {
            data: Arc::new(Mutex::new(Data {
                record,
                storage_warning: file_error,
                recovery: false,
                recovery_armed: false,
                cleanup_pending: false,
                last_allowed: true,
                access_until: Some(0),
                secure_failures: 0,
                next_secure_retry: 0,
                file_dirty: true,
                file_warning: false,
                mirror_dirty: true,
                mirror_loaded: false,
                identity: None,
                clock: OfflineClock::new(now()),
            })),
            path,
            lease_keys: licence_lease::LeaseKeys::new(
                parse_keys(option_env!("SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS").unwrap_or_default()),
                &keys,
            )
            .unwrap_or_default(),
            config: activation::Config::from_build(),
            transport: Arc::new(activation::HttpTransport),
            identity: Arc::new(activation::OsIdentityStore),
            mirror: Arc::new(activation::OsMirrorStore),
            operation: tokio::sync::Mutex::new(()),
            file_operation: tokio::sync::Mutex::new(()),
            mirror_operation: Arc::new(tokio::sync::Mutex::new(())),
            mirror_read_operation: tokio::sync::Mutex::new(()),
            keys,
            configured,
        };
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
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let allowed = data
            .access_until
            .is_none_or(|end| data.clock.time(now) < end);
        data.cleanup_pending |= data.last_allowed && !allowed;
        allowed
    }
    fn seed_access(&self) {
        let allowed = self.view().allowed(now());
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        data.last_allowed = allowed;
        data.cleanup_pending = false;
    }
    fn trust(&self) -> Vec<(&str, [u8; 32])> {
        self.keys
            .iter()
            .map(|(id, key)| (id.as_str(), *key))
            .collect()
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
        let mut view = if self.keys.is_empty()
            || self.lease_keys.is_empty()
            || self.config.is_none()
        {
            View {
                configured: true,
                status: "unavailable".into(),
                message: "Licence verification is unavailable in this build. Contact support."
                    .into(),
                access_until: Some(0),
                locked: true,
                ..View::default()
            }
        } else {
            data.record.as_ref().map(|r| {
                if r.cached_paid() && (!data.mirror_loaded || !data.identity.as_ref().is_some_and(|id| r.authenticated(id))) {
                    View { configured:true, status:"verification_pending".into(), message:"Unlock your computer's credential store to verify this licence. Your saved licence is unchanged.".into(), licence_id:r.key.as_ref().and_then(|key| licence_key::verify(key, &self.trust(), now()).ok()).map(|claims| claims.licence_id), access_until:Some(0), locked:true, ..View::default() }
                } else { r.view(&self.trust(),&self.lease_keys,data.clock.time(now())) }
            }).unwrap_or_else(|| View { configured:true,status:"unavailable".into(),message:"Unlock your keychain and restart Switchify PC, or enter your licence again.".into(),access_until:Some(0),..View::default() })
        };
        view.locked = !view.allowed(now());
        view.storage_warning = data.storage_warning;
        data.cleanup_pending |= data.last_allowed && view.locked;
        data.access_until = view.access_until;
        view
    }
    fn validate_purchase(&self, key: &str) -> Result<licence_key::Claims, String> {
        if !self.configured {
            return Err("Licensing is not enabled in this build.".into());
        }
        licence_key::verify(key, &self.trust(), now()).map_err(|e| e.to_string())
    }
    async fn check(
        &self,
        requested_action: &str,
        override_key: Option<&str>,
        takeover: bool,
    ) -> Result<(), String> {
        let _operation = self.operation.lock().await;
        let config = self
            .config
            .as_ref()
            .filter(|_| !self.lease_keys.is_empty())
            .ok_or("Licence activation is unavailable in this build. Contact support.")?;
        if let Some(key) = override_key {
            self.validate_purchase(key.trim())?;
        }
        // Establish the recovered revision baseline before committing any fresh
        // server result. This awaits only the first read, never a mirror write.
        self.ensure_mirror_ready().await?;
        let original = {
            let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
            if data.record.is_none() && override_key.is_some() {
                // A corrupt file with no recoverable mirror must not prevent
                // paid recovery, nor manufacture a fresh unpaid trial.
                let mut recovered = Record::new(0);
                recovered.last_seen = now();
                data.record = Some(recovered);
            }
            data.record.clone()
        }
        .ok_or("Licence storage is unavailable.")?;
        let key = override_key
            .map(str::trim)
            .or_else(|| {
                if takeover {
                    original.pending_key.as_deref().or(original.key.as_deref())
                } else {
                    original.key.as_deref().or(original.pending_key.as_deref())
                }
            })
            .ok_or("Enter or import your purchase key first.")?
            .to_owned();
        let claims = self.validate_purchase(&key)?;
        let store = self.identity.clone();
        let identity = Arc::new(
            tauri::async_runtime::spawn_blocking(move || store.load_or_create())
                .await
                .map_err(|_| activation::STORAGE_ERROR)??,
        );
        self.data.lock().unwrap_or_else(|p| p.into_inner()).identity = Some(identity.clone());
        let same_identity = original.installation.as_ref() == Some(&identity.installation);
        let same_purchase = original.key.as_deref() == Some(&key);
        let action = if requested_action == "deactivate" {
            "deactivate"
        } else if same_identity
            && same_purchase
            && original.lease.is_some()
            && original.terminal_status.is_none()
            && !takeover
        {
            "renew"
        } else {
            "activate"
        };
        let timestamp = now();
        let nonce = uuid::Uuid::new_v4().to_string();
        let body = activation::request(
            &identity,
            action,
            &claims.licence_id,
            (action == "activate").then_some(key.as_str()),
            takeover,
            timestamp,
            &nonce,
        );
        let response = self.transport.post(config, body).await?;
        if let Some(outcome) = response.get("outcome").and_then(|v| v.as_str()) {
            if outcome == "invalid" {
                return Err(activation::INVALID_REQUEST_ERROR.into());
            }
            if matches!(outcome, "seat_taken" | "takeover_limited") {
                {
                    let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
                    let mut record = data
                        .record
                        .clone()
                        .ok_or("Licence storage is unavailable.")?;
                    if record.key != original.key
                        || record.lease != original.lease
                        || record.terminal_status != original.terminal_status
                    {
                        return Err("Your licence changed. Try again.".into());
                    }
                    let authenticated = record.authenticated(&identity);
                    record.pending_key = Some(key);
                    record.last_outcome = Some(outcome.into());
                    record.revision = record.revision.saturating_add(1);
                    if authenticated {
                        record.seal(&identity);
                    } else {
                        record.local_signature = None;
                    }
                    data.record = Some(record);
                    data.file_dirty = true;
                    data.mirror_dirty = true;
                }
                self.persist_file().await;
                return Err(if outcome=="seat_taken" { "This licence is active on another computer. Choose Move licence here to replace it." } else { "This licence has already been moved twice in 30 days. Contact support for help." }.into());
            }
            return Err(activation::NETWORK_ERROR.into());
        }
        let proof = response
            .get("proof")
            .and_then(|v| v.as_str())
            .ok_or(activation::NETWORK_ERROR)?;
        let context = licence_lease::RequestContext {
            binding: licence_lease::Binding {
                licence_id: &claims.licence_id,
                installation_id: &identity.installation.id,
                installation_key: &identity.installation.public_key,
            },
            nonce: &nonce,
            action,
            requested_at: timestamp,
            min_generation: if same_identity && same_purchase {
                original.generation
            } else {
                0
            },
        };
        let reply = licence_lease::verify_reply(proof, &self.lease_keys, &context, now())
            .map_err(|_| activation::NETWORK_ERROR)?;
        if matches!(&reply,licence_lease::Reply::Status(s) if s.status=="revoked")
            && !same_purchase
            && original.lease.is_some()
        {
            return Err(
                "That purchase has been revoked. Your existing licence is unchanged.".into(),
            );
        }
        {
            let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
            let mut record = data
                .record
                .clone()
                .ok_or("Licence storage is unavailable.")?;
            if record.key != original.key
                || record.lease != original.lease
                || record.terminal_status != original.terminal_status
            {
                return Err("Your licence changed. Try again.".into());
            }
            record.key = Some(key);
            if action != "renew" {
                record.pending_key = None;
                record.last_outcome = None;
            }
            record.installation = Some(identity.installation.clone());
            match reply {
                licence_lease::Reply::Lease(lease) => {
                    record.lease = Some(proof.into());
                    record.generation = lease.generation;
                    record.terminal_status = None;
                    record.lease_seen = lease.issued_at;
                    record.last_checked = Some(lease.issued_at);
                    record.last_seen = record.last_seen.max(lease.issued_at);
                }
                licence_lease::Reply::Status(status) => {
                    record.lease = None;
                    record.generation = status.generation;
                    record.terminal_status = Some(status.status);
                    record.lease_seen = status.issued_at;
                    record.last_checked = Some(status.issued_at);
                    record.last_seen = record.last_seen.max(status.issued_at);
                    if action == "deactivate" {
                        record.key = None;
                        record.terminal_status = None;
                    }
                }
            }
            record.revision = record.revision.saturating_add(1);
            record.last_seen = record.last_seen.max(now());
            record.seal(&identity);
            data.clock = OfflineClock::new(record.lease_seen);
            data.record = Some(record);
            data.file_dirty = true;
            data.mirror_dirty = true;
        }
        self.view();
        self.persist_file().await;
        let terminal_result = {
            let data = self.data.lock().unwrap_or_else(|p| p.into_inner());
            data.record
                .as_ref()
                .is_some_and(|r| r.terminal_status.is_some() || action == "deactivate")
        };
        if terminal_result {
            self.persist_terminal_mirror().await?;
        }
        Ok(())
    }
    /// Retain terminal results in the OS mirror before reporting success, so
    /// restoring an older file cannot undo them. A stuck OS prompt must not hold the
    /// command or input gate indefinitely; the serialized writer continues.
    async fn persist_terminal_mirror(&self) -> Result<(), String> {
        let data = self.data.clone();
        let snapshot = data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .record
            .clone();
        let store = self.mirror.clone();
        let serial = self.mirror_operation.clone();
        let task = tokio::spawn(async move {
            let _guard = serial.lock_owned().await;
            let written = snapshot.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let record = written.ok_or("Licence storage is unavailable.")?;
                store.write(
                    &serde_json::to_string(&record).map_err(|_| "Could not save the licence.")?,
                )
            })
            .await
            .is_ok_and(|r| r.is_ok());
            let mut data = data.lock().unwrap_or_else(|p| p.into_inner());
            if result && data.record == snapshot {
                data.mirror_dirty = false;
                data.record_checkpoint(now(), true, Ok(false));
            } else if !result {
                data.record_checkpoint(now(), true, Ok(true));
            }
            data.storage_warning |= data.file_warning;
            result
        });
        if tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .is_ok_and(|r| r.is_ok_and(|ok| ok))
        {
            Ok(())
        } else {
            self.data
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .storage_warning = true;
            Err("Your licence status changed, but could not be saved yet. Keep Switchify PC open and check your storage permissions.".into())
        }
    }
    fn checkpoint(&self) {
        if !self.configured {
            return;
        }
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut record) = data.record.clone() {
            // The original seal is the only recoverable authority while the
            // first OS read waits. Never overwrite it with an unsealed edit.
            if (record.local_signature.is_some() || record.cached_paid())
                && (!data.mirror_loaded
                    || !data
                        .identity
                        .as_ref()
                        .is_some_and(|id| record.authenticated(id)))
            {
                return;
            }
            let time = data.clock.time(now());
            if time.saturating_sub(record.last_seen) >= 60
                || time.saturating_sub(record.lease_seen) >= 60
            {
                let identity = data.identity.clone().filter(|id| record.authenticated(id));
                record.last_seen = record.last_seen.max(time);
                record.lease_seen = record.lease_seen.max(time);
                record.revision = record.revision.saturating_add(1);
                if let Some(identity) = identity {
                    record.seal(&identity);
                }
                data.mirror_dirty = true;
                data.record = Some(record);
                data.file_dirty = true;
            }
        }
    }
    async fn persist_file(&self) {
        if !self.configured {
            return;
        }
        let _serial = self.file_operation.lock().await;
        let snapshot = {
            let data = self.data.lock().unwrap_or_else(|p| p.into_inner());
            if !data.file_dirty {
                return;
            }
            data.record.clone()
        };
        let Some(record) = snapshot else {
            return;
        };
        let path = self.path.clone();
        let written = record.clone();
        let success =
            tauri::async_runtime::spawn_blocking(move || write_record_file(&path, &written))
                .await
                .is_ok_and(|r| r.is_ok());
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        if success && data.record.as_ref() == Some(&record) {
            data.file_dirty = false;
        }
        data.file_warning = !success;
        data.storage_warning = data.file_warning || data.secure_failures > 0;
    }
    async fn ensure_mirror_ready(&self) -> Result<(), String> {
        let _read = self.mirror_read_operation.lock().await;
        if self
            .data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .mirror_loaded
        {
            return Ok(());
        }
        let identity_store = self.identity.clone();
        let identity =
            match tauri::async_runtime::spawn_blocking(move || identity_store.load_or_create())
                .await
            {
                Ok(Ok(identity)) => Arc::new(identity),
                _ => {
                    self.data
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .record_checkpoint(now(), true, Ok(true));
                    return Err(activation::STORAGE_ERROR.into());
                }
            };
        let store = self.mirror.clone();
        let result =
            tauri::async_runtime::spawn_blocking(move || read_secure(store.as_ref())).await;
        {
            let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
            match result {
                Ok(Ok(stored)) => {
                    let merged = Record::reconcile(data.record.clone(), stored, &identity);
                    if let Some(record) = &merged {
                        data.clock.advance(record.lease_seen);
                    }
                    data.identity = Some(identity.clone());
                    data.file_dirty |= merged != data.record;
                    data.record = merged;
                    data.mirror_loaded = true;
                    data.mirror_dirty = true;
                    data.record_checkpoint(now(), true, Ok(false));
                    data.storage_warning |= data.file_warning;
                }
                _ => {
                    data.record_checkpoint(now(), true, Ok(true));
                    return Err(activation::STORAGE_ERROR.into());
                }
            }
        }
        self.view();
        Ok(())
    }
    async fn sync_mirror(&self) {
        if !self.configured {
            return;
        }
        let _serial = self.mirror_operation.lock().await;
        {
            let data = self.data.lock().unwrap_or_else(|p| p.into_inner());
            if (data.mirror_loaded && !data.mirror_dirty && !data.storage_warning)
                || (data.storage_warning && !data.retry_secure_due(now()))
            {
                return;
            }
        }
        if self.ensure_mirror_ready().await.is_err() {
            return;
        }
        let snapshot = self
            .data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .record
            .clone();
        let store = self.mirror.clone();
        let written = snapshot.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            if let Some(record) = written {
                store.write(
                    &serde_json::to_string(&record).map_err(|_| "Could not save the licence.")?,
                )?;
            }
            Ok::<_, String>(())
        })
        .await;
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        match result {
            Ok(Ok(())) => {
                // The read established the baseline before any server commit.
                // A late write only acknowledges that snapshot; it never merges
                // its old entitlements back over a newer server response.
                data.mirror_dirty = data.record != snapshot;
                data.record_checkpoint(now(), true, Ok(false));
            }
            _ => data.record_checkpoint(now(), true, Ok(true)),
        }
        data.storage_warning |= data.file_warning;
    }
}
#[tauri::command]
pub async fn paste_licence(app: AppHandle) -> Result<View, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let key = app
        .clipboard()
        .read_text()
        .map_err(|_| "Could not read a licence from the clipboard.".to_string())?;
    redeem_licence(app, key).await
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
pub async fn redeem_licence(app: AppHandle, key: String) -> Result<View, String> {
    let result = app
        .state::<Licensing>()
        .check("activate", Some(&key), false)
        .await;
    publish(&app);
    result?;
    Ok(app.state::<Licensing>().view())
}
#[tauri::command]
pub async fn check_licence(app: AppHandle) -> Result<View, String> {
    let result = app.state::<Licensing>().check("check", None, false).await;
    publish(&app);
    result?;
    Ok(app.state::<Licensing>().view())
}
#[tauri::command]
pub async fn transfer_licence(app: AppHandle) -> Result<View, String> {
    let result = app.state::<Licensing>().check("activate", None, true).await;
    publish(&app);
    result?;
    Ok(app.state::<Licensing>().view())
}
#[tauri::command]
pub async fn remove_licence(app: AppHandle) -> Result<View, String> {
    let result = app
        .state::<Licensing>()
        .check("deactivate", None, false)
        .await;
    publish(&app);
    result?;
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
    let files = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            files.state::<Licensing>().persist_file().await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    let mirrors = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            mirrors.state::<Licensing>().sync_mirror().await;
            publish(&mirrors);
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
    let background = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut first = true;
        let mut retry_after = std::time::Instant::now();
        let mut last_success = std::time::Instant::now();
        loop {
            let service = background.state::<Licensing>();
            let needs_check = {
                let data = service.data.lock().unwrap_or_else(|p| p.into_inner());
                data.record.as_ref().is_some_and(|r| {
                    r.key.is_some()
                        && r.terminal_status.as_deref() != Some("revoked")
                        && (first
                            || last_success.elapsed() >= std::time::Duration::from_secs(DAY as u64)
                            || r.last_checked
                                .is_none_or(|checked| now().saturating_sub(checked) >= 86400))
                })
            };
            if needs_check && std::time::Instant::now() >= retry_after {
                let success = service.check("check", None, false).await.is_ok();
                if success {
                    first = false;
                    last_success = std::time::Instant::now();
                }
                retry_after = std::time::Instant::now()
                    + std::time::Duration::from_secs(if success { 60 } else { 900 });
                publish(&background);
            }
            // Poll signed server check time after sleep/resume; retry failures
            // on a monotonic timer so correcting the clock cannot strand them.
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
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
    use activation::IdentityStore;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::{json, Value};

    struct FakeTransport {
        outcome: Mutex<&'static str>,
    }
    impl activation::Transport for FakeTransport {
        fn post(&self, _config: &activation::Config, body: Value) -> activation::ResponseFuture {
            let outcome = *self.outcome.lock().unwrap();
            Box::pin(async move {
                if outcome == "outage" {
                    return Err(activation::NETWORK_ERROR.into());
                }
                if matches!(outcome, "seat_taken" | "takeover_limited" | "invalid") {
                    return Ok(json!({"outcome":outcome}));
                }
                let request = &body["request"];
                if (now() - request["timestamp"].as_i64().unwrap()).abs() > 300 {
                    return Err(activation::NETWORK_ERROR.into());
                }
                let terminal = matches!(outcome, "revoked" | "deactivated")
                    || request["action"] == "deactivate";
                let server_time = now();
                let mut claims = json!({"version":1,"product":"switchify-pc","licence_id":request["licence_id"],
                    "installation_id":request["installation_id"],"installation_key":request["public_key"],
                    "generation":1,"issued_at":server_time,"request_nonce":request["nonce"],"action":request["action"]});
                if terminal {
                    claims["status"] = json!(if outcome == "revoked" {
                        "revoked"
                    } else {
                        "deactivated"
                    });
                } else {
                    claims["lease_id"] = json!(uuid::Uuid::new_v4().to_string());
                    claims["expires_at"] = json!(server_time + TRIAL);
                }
                if outcome == "wrong_nonce" {
                    claims["request_nonce"] = json!(uuid::Uuid::new_v4().to_string());
                }
                let message = format!(
                    "{}.lease.{}",
                    if terminal { "SWPS1" } else { "SWPL1" },
                    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
                );
                let signature = SigningKey::from_bytes(&[8; 32]).sign(message.as_bytes());
                Ok(
                    json!({"proof":format!("{message}.{}",URL_SAFE_NO_PAD.encode(signature.to_bytes()))}),
                )
            })
        }
    }
    fn purchase() -> String {
        let message=format!("SWPC1.test.{}",URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({"version":1,"licence_id":"6af44000-17b7-4d87-ad1d-32e9a31b331a","product":"switchify-pc","issued_at":1000,"expires_at":null,"seats":1})).unwrap()));
        format!(
            "{message}.{}",
            URL_SAFE_NO_PAD.encode(
                SigningKey::from_bytes(&[7; 32])
                    .sign(message.as_bytes())
                    .to_bytes()
            )
        )
    }
    fn online_service() -> (Licensing, Arc<FakeTransport>, PathBuf) {
        let mut service = configured_service(Record::new(now()));
        let dir = std::env::temp_dir().join(format!(
            "switchify-activation-test-{}",
            uuid::Uuid::new_v4()
        ));
        service.path = dir.join("licence.json");
        service.keys = vec![(
            "test".into(),
            SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
        )];
        service.lease_keys = licence_lease::LeaseKeys::new(
            vec![(
                "lease".into(),
                SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
            )],
            &service.keys,
        )
        .unwrap();
        let transport = Arc::new(FakeTransport {
            outcome: Mutex::new("licensed"),
        });
        service.transport = transport.clone();
        (service, transport, dir)
    }
    #[tokio::test]
    async fn unverifiable_request_guidance_does_not_replace_or_revoke_cached_access() {
        let (service, transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let before = service.data.lock().unwrap().record.clone();
        *transport.outcome.lock().unwrap() = "invalid";
        assert_eq!(
            service.check("check", None, false).await.unwrap_err(),
            activation::INVALID_REQUEST_ERROR
        );
        assert_eq!(service.view().status, "licensed");
        assert!(service.data.lock().unwrap().record == before);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn rejected_initial_activation_preserves_trial_and_suggests_key_and_clock_checks() {
        let (service, transport, dir) = online_service();
        *transport.outcome.lock().unwrap() = "invalid";
        assert_eq!(
            service
                .check("activate", Some(&purchase()), false)
                .await
                .unwrap_err(),
            activation::INVALID_REQUEST_ERROR
        );
        assert_eq!(service.view().status, "trial");
        assert!(service
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .key
            .is_none());
        std::fs::remove_dir_all(dir).ok();
    }
    fn restart(source: &Licensing, record: Record) -> Licensing {
        let mut service = configured_service(record);
        service.keys = source.keys.clone();
        service.lease_keys = source.lease_keys.clone();
        service.path = source.path.clone();
        service.identity = source.identity.clone();
        service.mirror = source.mirror.clone();
        service.transport = source.transport.clone();
        service.seed_access();
        service
    }
    #[tokio::test]
    async fn file_only_transplant_never_grants_paid_access_even_after_seat_conflict() {
        let (source, transport, dir) = online_service();
        source.data.lock().unwrap().record = Some(Record::new(0));
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let record = decode_record(&std::fs::read_to_string(&source.path).unwrap()).unwrap();
        let mut copied = restart(&source, record);
        copied.identity = Arc::new(activation::tests::OtherIdentity);
        copied.mirror = Arc::new(activation::tests::MemoryMirror::default());
        assert!(copied.view().locked, "file alone grants no startup access");
        copied.sync_mirror().await;
        assert_eq!(copied.view().status, "needs_activation");
        assert!(copied.view().locked);
        *transport.outcome.lock().unwrap() = "seat_taken";
        assert!(copied.check("check", None, false).await.is_err());
        assert!(copied.view().locked);
        assert!(copied
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .lease
            .is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn cold_restart_waits_for_os_identity_then_recovers_offline_after_unlock() {
        let (source, transport, dir) = online_service();
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let record = decode_record(&std::fs::read_to_string(&source.path).unwrap()).unwrap();
        let mut cold = restart(&source, record.clone());
        cold.identity = Arc::new(LockedIdentity);
        *transport.outcome.lock().unwrap() = "outage";
        assert_eq!(cold.view().status, "verification_pending");
        assert!(cold.view().locked);
        cold.sync_mirror().await;
        assert!(cold.view().locked);
        assert!(cold.view().storage_warning);
        cold.data.lock().unwrap().clock.started =
            std::time::Instant::now() - std::time::Duration::from_secs(61);
        cold.checkpoint();
        cold.persist_file().await;
        let retained = decode_record(&std::fs::read_to_string(&cold.path).unwrap()).unwrap();
        assert!(
            retained == record,
            "locked-startup checkpoints retain the original seal and record"
        );
        assert!(cold.data.lock().unwrap().record.as_ref().unwrap().lease == record.lease);
        cold.identity = source.identity.clone();
        cold.data.lock().unwrap().next_secure_retry = 0;
        cold.sync_mirror().await;
        assert_eq!(cold.view().status, "licensed");
        assert!(!cold.view().storage_warning);
        assert!(cold
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .authenticated(&cold.identity.load_or_create().unwrap()));
        assert!(cold.check("check", None, false).await.is_err());
        assert_eq!(
            cold.view().status,
            "licensed",
            "outage after verification keeps the allowance"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn locked_startup_checkpoint_cannot_restore_an_older_terminal_mirror() {
        let (source, transport, dir) = online_service();
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        *transport.outcome.lock().unwrap() = "deactivated";
        source.check("check", None, false).await.unwrap(); // Durable old mirror.
        *transport.outcome.lock().unwrap() = "licensed";
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let record = source.data.lock().unwrap().record.clone().unwrap();
        let mut cold = restart(&source, record.clone());
        cold.identity = Arc::new(LockedIdentity);
        cold.data.lock().unwrap().clock.started =
            std::time::Instant::now() - std::time::Duration::from_secs(61);
        cold.checkpoint();
        cold.persist_file().await;
        cold.identity = source.identity.clone();
        cold.sync_mirror().await;
        assert_eq!(cold.view().status, "licensed");
        assert_eq!(
            cold.data.lock().unwrap().record.as_ref().unwrap().lease,
            record.lease
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn authenticated_mirror_defeats_older_and_modified_revocation_rollbacks() {
        let (source, transport, dir) = online_service();
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let old = source.data.lock().unwrap().record.clone().unwrap();
        *transport.outcome.lock().unwrap() = "revoked";
        source.check("check", None, false).await.unwrap();
        source.sync_mirror().await;
        for tampered in [false, true] {
            let mut file = old.clone();
            if tampered {
                file.revision = u64::MAX;
                file.lease_seen = 0;
            }
            let cold = restart(&source, file);
            assert!(
                cold.view().locked,
                "no stale access before mirror reconciliation"
            );
            cold.sync_mirror().await;
            assert_eq!(cold.view().status, "revoked");
            assert!(cold.view().locked);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn frozen_and_rolled_back_wall_clock_cannot_stop_running_lease_expiry() {
        let (service, _transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        service.sync_mirror().await;
        assert!(!service.data.lock().unwrap().mirror_dirty);
        {
            let mut data = service.data.lock().unwrap();
            let issued = data.record.as_ref().unwrap().last_checked.unwrap();
            data.clock = OfflineClock {
                epoch: issued,
                started: std::time::Instant::now() - std::time::Duration::from_secs(TRIAL as u64),
            };
            assert!(data.clock.time(issued - DAY) >= issued + TRIAL);
        }
        assert_eq!(service.view().status, "lease_expired");
        assert!(!service.cached_allowed(now()));
        service.checkpoint();
        let record = service.data.lock().unwrap().record.clone().unwrap();
        assert!(record.authenticated(&service.identity.load_or_create().unwrap()));
        assert!(service.data.lock().unwrap().mirror_dirty);
        service.sync_mirror().await;
        let cold = restart(&service, record);
        cold.sync_mirror().await;
        assert_eq!(cold.view().status, "lease_expired");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn same_lease_reconciliation_keeps_highest_authenticated_time_marker() {
        let (service, _transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let identity = service.identity.load_or_create().unwrap();
        let old = service.data.lock().unwrap().record.clone().unwrap();
        let mut newer = old.clone();
        newer.lease_seen += TRIAL;
        newer.seal(&identity);
        let reconciled = Record::reconcile(Some(old), Some(newer.clone()), &identity).unwrap();
        assert_eq!(reconciled.lease_seen, newer.lease_seen);
        assert!(reconciled.authenticated(&identity));
        assert_eq!(
            reconciled
                .view(&service.trust(), &service.lease_keys, now())
                .status,
            "lease_expired"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn unsigned_v2_cache_keeps_trial_and_purchase_for_online_recovery() {
        let (source, _transport, dir) = online_service();
        source.data.lock().unwrap().record = Some(Record::new(0));
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let mut legacy = source.data.lock().unwrap().record.clone().unwrap();
        legacy.version = 2;
        legacy.local_signature = None;
        let legacy = decode_record(&serde_json::to_string(&legacy).unwrap()).unwrap();
        let mut cold = restart(&source, legacy);
        cold.mirror = Arc::new(activation::tests::MemoryMirror::default());
        cold.sync_mirror().await;
        assert_eq!(cold.view().status, "needs_activation");
        assert!(cold.view().locked);
        assert_eq!(
            cold.data.lock().unwrap().record.as_ref().unwrap().first_run,
            0
        );
        cold.check("check", None, false).await.unwrap();
        assert_eq!(cold.view().status, "licensed");
        assert_eq!(
            cold.data.lock().unwrap().record.as_ref().unwrap().version,
            3
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn online_activation_outage_replay_refund_and_deactivation_use_one_access_gate() {
        let (service, transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        assert_eq!(service.view().status, "licensed");
        assert_eq!(service.view().offline_days_remaining, Some(30));
        let before = service.data.lock().unwrap().record.clone().unwrap();
        for failure in ["outage", "wrong_nonce"] {
            *transport.outcome.lock().unwrap() = failure;
            assert!(service.check("check", None, false).await.is_err());
            assert_eq!(service.view().status, "licensed");
            assert_eq!(
                service.data.lock().unwrap().record.as_ref().unwrap().lease,
                before.lease
            );
        }
        let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
        let expiry = before.last_checked.unwrap() + TRIAL;
        assert!(saved
            .view(&service.trust(), &service.lease_keys, expiry - 1)
            .allowed(expiry - 1));
        assert!(!saved
            .view(&service.trust(), &service.lease_keys, expiry)
            .allowed(expiry));
        *transport.outcome.lock().unwrap() = "revoked";
        service.check("check", None, false).await.unwrap();
        assert_eq!(service.view().status, "revoked");
        assert!(service.view().locked);
        {
            let mut data = service.data.lock().unwrap();
            assert!(
                data.cleanup_pending,
                "verified revocation latches cleanup before the next tick"
            );
            assert!(data.observe_access(false));
            assert!(data.cleanup_pending);
        }
        let revoked = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
        assert_eq!(
            revoked
                .view(&service.trust(), &service.lease_keys, now())
                .status,
            "revoked"
        );
        *transport.outcome.lock().unwrap() = "licensed";
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        service.check("deactivate", None, false).await.unwrap();
        assert_eq!(service.view().status, "trial");
        assert!(service
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .key
            .is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn seat_conflict_preserves_trial_and_requires_explicit_transfer() {
        let (service, transport, dir) = online_service();
        let first_run = service
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .first_run;
        *transport.outcome.lock().unwrap() = "seat_taken";
        assert!(service
            .check("activate", Some(&purchase()), false)
            .await
            .is_err());
        assert!(service.view().can_transfer);
        assert_eq!(service.view().status, "trial");
        *transport.outcome.lock().unwrap() = "licensed";
        service.check("activate", None, true).await.unwrap();
        assert!(!service.view().can_transfer);
        assert_eq!(service.view().status, "licensed");
        assert_eq!(
            service
                .data
                .lock()
                .unwrap()
                .record
                .as_ref()
                .unwrap()
                .first_run,
            first_run
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    struct LockedIdentity;
    impl activation::IdentityStore for LockedIdentity {
        fn load_or_create(&self) -> Result<activation::Identity, String> {
            Err(activation::STORAGE_ERROR.into())
        }
    }
    #[tokio::test]
    async fn cached_lease_opens_with_locked_keychain_and_mirror_heals_after_unlock() {
        let (mut service, _transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        service.identity = Arc::new(LockedIdentity);
        assert!(service.check("check", None, false).await.is_err());
        assert_eq!(service.view().status, "licensed");
        {
            let mut data = service.data.lock().unwrap();
            data.record.as_mut().unwrap().last_seen = now() - 61;
            data.storage_warning = true;
        }
        service.identity = Arc::new(activation::tests::FakeIdentity);
        service.checkpoint();
        service.sync_mirror().await;
        assert!(!service.view().storage_warning);
        service.check("check", None, false).await.unwrap();
        assert_eq!(service.view().status, "licensed");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn verified_revocation_locks_current_session_even_when_record_file_cannot_be_written() {
        let (mut service, transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let old = service.data.lock().unwrap().record.clone().unwrap();
        service.path = dir.clone(); // A directory cannot be replaced by the record file.
        *transport.outcome.lock().unwrap() = "revoked";
        service.check("check", None, false).await.unwrap();
        assert!(service.view().locked);
        assert!(service.view().storage_warning);
        let cold = restart(&service, old);
        assert!(cold.view().locked);
        cold.sync_mirror().await;
        assert_eq!(
            cold.view().status,
            "revoked",
            "terminal mirror was durable before the command succeeded"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn modified_cache_without_mirror_requires_online_recovery() {
        let (source, _transport, dir) = online_service();
        source.data.lock().unwrap().record = Some(Record::new(0));
        source
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let mut modified = source.data.lock().unwrap().record.clone().unwrap();
        modified.revision = u64::MAX;
        modified.lease_seen = 0;
        let mut cold = restart(&source, modified);
        cold.mirror = Arc::new(activation::tests::MemoryMirror::default());
        cold.sync_mirror().await;
        assert!(cold.view().locked);
        assert_eq!(cold.view().status, "needs_activation");
        assert_eq!(
            cold.data.lock().unwrap().record.as_ref().unwrap().revision,
            0
        );
        cold.check("check", None, false).await.unwrap();
        assert_eq!(cold.view().status, "licensed");
        assert_eq!(
            cold.data.lock().unwrap().record.as_ref().unwrap().revision,
            1
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn v1_record_migrates_without_resetting_trial_or_granting_purchase_only_access() {
        let old = json!({"version":1,"first_run":1000,"last_seen":1000+TRIAL,"revision":4,"key":purchase()});
        let record = decode_record(&old.to_string()).unwrap();
        assert_eq!(record.version, 3);
        assert_eq!(record.first_run, 1000);
        assert_eq!(record.revision, 4);
        let keys = [(
            "test",
            SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
        )];
        let view = record.view(&keys, &licence_lease::LeaseKeys::default(), 1000);
        assert_eq!(view.status, "needs_activation");
        assert!(view.locked);
        assert!(!view.allowed(1000));
    }
    #[tokio::test]
    async fn corrected_clock_recovers_paid_access_without_resetting_trial_high_water() {
        let (service, _transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let future = now() + 60 * DAY;
        let stale = {
            let mut data = service.data.lock().unwrap();
            let record = data.record.as_mut().unwrap();
            record.last_seen = future;
            record.lease_seen = future;
            record.clone()
        };
        service
            .mirror
            .write(&serde_json::to_string(&stale).unwrap())
            .unwrap();
        assert!(service.view().locked);
        // The fake endpoint enforces the same +/-300-second request window as
        // Node. A trial high-water timestamp must never become wire freshness.
        service.check("check", None, false).await.unwrap();
        assert_eq!(service.view().status, "licensed");
        service.sync_mirror().await;
        assert_eq!(service.view().status, "licensed");
        assert_eq!(
            service
                .data
                .lock()
                .unwrap()
                .record
                .as_ref()
                .unwrap()
                .last_seen,
            future
        );
        service.check("deactivate", None, false).await.unwrap();
        assert_eq!(
            service.view().status,
            "expired",
            "online recovery cannot restart the trial"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn unconfigured_build_does_not_start_a_persisted_trial() {
        let (mut service, _transport, dir) = online_service();
        service.configured = false;
        service.persist_file().await;
        service.sync_mirror().await;
        assert!(!service.path.exists());
        assert!(!dir.exists());
    }
    #[tokio::test]
    async fn paid_recovery_from_unreadable_record_does_not_restart_trial() {
        let (service, _transport, dir) = online_service();
        service.data.lock().unwrap().record = None;
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        assert_eq!(service.view().status, "licensed");
        service.check("deactivate", None, false).await.unwrap();
        assert_eq!(service.view().status, "expired");
        assert_eq!(
            service
                .data
                .lock()
                .unwrap()
                .record
                .as_ref()
                .unwrap()
                .first_run,
            0
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    struct StalledMirror {
        entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        stored: Option<String>,
    }
    impl activation::MirrorStore for StalledMirror {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.stored.clone())
        }
        fn write(&self, _text: &str) -> Result<(), String> {
            if let Some(entered) = self.entered.lock().unwrap().take() {
                let _ = entered.send(());
                self.release
                    .lock()
                    .unwrap()
                    .recv()
                    .map_err(|_| "Mirror closed".to_string())?;
            }
            Ok(())
        }
    }
    #[tokio::test]
    async fn stalled_keychain_never_holds_access_lock_or_delays_revocation_cleanup() {
        let (mut service, transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let mut old = service.data.lock().unwrap().record.clone().unwrap();
        old.revision = 5;
        old.first_run -= DAY;
        old.seal(&activation::tests::FakeIdentity.load_or_create().unwrap());
        std::fs::remove_file(&service.path).unwrap();
        {
            let mut data = service.data.lock().unwrap();
            data.record = Some(Record::new(now()));
            data.mirror_loaded = false;
        }
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        service.mirror = Arc::new(StalledMirror {
            entered: Mutex::new(Some(entered_tx)),
            release: Mutex::new(release_rx),
            stored: Some(serde_json::to_string(&old).unwrap()),
        });
        let service = Arc::new(service);
        let worker = service.clone();
        let task = tokio::spawn(async move {
            worker.sync_mirror().await;
        });
        entered_rx.await.unwrap();
        assert!(
            service.data.try_lock().is_ok(),
            "keychain I/O holds no entitlement lock"
        );
        assert!(service.cached_allowed(now()));
        assert_eq!(service.view().status, "licensed");
        *transport.outcome.lock().unwrap() = "revoked";
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            service.check("check", None, false),
        )
        .await
        .unwrap()
        .unwrap_err(); // Saving is still pending, but revocation is immediate.
        assert!(service.view().locked);
        assert!(service.data.lock().unwrap().cleanup_pending);
        let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
        assert_eq!(saved.terminal_status.as_deref(), Some("revoked"));
        assert!(saved.revision > old.revision);
        assert_eq!(saved.first_run, old.first_run);
        release_tx.send(()).unwrap();
        task.await.unwrap();
        assert_eq!(
            service.view().status,
            "revoked",
            "a stale mirror completion cannot restore access"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    struct PausedReadMirror {
        entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        stored: String,
    }
    impl activation::MirrorStore for PausedReadMirror {
        fn read(&self) -> Result<Option<String>, String> {
            if let Some(entered) = self.entered.lock().unwrap().take() {
                let _ = entered.send(());
            }
            self.release
                .lock()
                .unwrap()
                .recv()
                .map_err(|_| "Mirror closed")?;
            Ok(Some(self.stored.clone()))
        }
        fn write(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }
    }
    #[tokio::test]
    async fn delayed_initial_read_establishes_old_revision_before_fresh_server_commit() {
        let (mut service, transport, dir) = online_service();
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap();
        let mut old = service.data.lock().unwrap().record.clone().unwrap();
        old.revision = 5;
        old.first_run -= DAY;
        old.seal(&activation::tests::FakeIdentity.load_or_create().unwrap());
        std::fs::remove_file(&service.path).unwrap();
        {
            let mut data = service.data.lock().unwrap();
            data.record = Some(Record::new(now()));
            data.mirror_loaded = false;
        }
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        service.mirror = Arc::new(PausedReadMirror {
            entered: Mutex::new(Some(entered_tx)),
            release: Mutex::new(release_rx),
            stored: serde_json::to_string(&old).unwrap(),
        });
        *transport.outcome.lock().unwrap() = "revoked";
        let service = Arc::new(service);
        let worker = service.clone();
        let task =
            tokio::spawn(async move { worker.check("activate", Some(&purchase()), false).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(service.data.try_lock().is_ok());
        assert_eq!(
            service.view().status,
            "trial",
            "cached launch state remains usable while keychain read waits"
        );
        assert!(
            !task.is_finished(),
            "network state cannot commit before the recovered revision is known"
        );
        release_tx.send(()).unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(service.view().status, "revoked");
        let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
        assert!(saved.revision > old.revision);
        assert_eq!(saved.first_run, old.first_run);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn trial_uses_exact_boundary_and_never_restarts_on_rollback() {
        let record = Record::new(1000);
        assert_eq!(
            record
                .view(&[], &licence_lease::LeaseKeys::default(), 1000)
                .days_remaining,
            30
        );
        assert_eq!(
            record
                .view(&[], &licence_lease::LeaseKeys::default(), 1000)
                .message,
            "30 days left in your full trial."
        );
        assert_eq!(
            record
                .view(&[], &licence_lease::LeaseKeys::default(), 1000 + TRIAL - 1)
                .message,
            "1 day left in your full trial."
        );
        assert!(record
            .view(&[], &licence_lease::LeaseKeys::default(), 1000 + TRIAL - 1)
            .allowed(1000 + TRIAL - 1));
        assert!(!record
            .view(&[], &licence_lease::LeaseKeys::default(), 1000 + TRIAL)
            .allowed(1000 + TRIAL));
        let mut advanced = record.clone();
        advanced.last_seen = 1000 + TRIAL;
        assert_eq!(
            advanced
                .view(&[], &licence_lease::LeaseKeys::default(), 1000)
                .status,
            "expired"
        );
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
            access_until: Some(0),
            secure_failures: 0,
            next_secure_retry: 0,
            file_dirty: true,
            file_warning: false,
            mirror_dirty: true,
            mirror_loaded: false,
            identity: None,
            clock: OfflineClock::new(now()),
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
            data: Arc::new(Mutex::new(Data {
                record: Some(record),
                storage_warning: false,
                recovery: false,
                recovery_armed: false,
                cleanup_pending: false,
                last_allowed: true,
                access_until: Some(0),
                secure_failures: 0,
                next_secure_retry: 0,
                file_dirty: true,
                file_warning: false,
                mirror_dirty: true,
                mirror_loaded: false,
                identity: None,
                clock: OfflineClock::new(now()),
            })),
            path: std::env::temp_dir().join("switchify-licence-test-never-written.json"),
            keys: vec![("test".into(), [0; 32])],
            configured: true,
            lease_keys: licence_lease::LeaseKeys::new(vec![("lease".into(), [1; 32])], &[])
                .unwrap(),
            config: Some(activation::Config::test_config()),
            transport: Arc::new(activation::HttpTransport),
            identity: Arc::new(activation::tests::FakeIdentity),
            mirror: Arc::new(activation::tests::MemoryMirror::default()),
            operation: tokio::sync::Mutex::new(()),
            file_operation: tokio::sync::Mutex::new(()),
            mirror_operation: Arc::new(tokio::sync::Mutex::new(())),
            mirror_read_operation: tokio::sync::Mutex::new(()),
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
    fn fresh_install_starts_the_trial_even_when_the_keychain_is_unavailable() {
        let fresh = initial_record(None, None, false, 5000).unwrap();
        assert_eq!(fresh.first_run, 5000);
        assert!(initial_record(None, None, true, 5000).is_none());
        let older = Record::new(1000);
        let kept = initial_record(Some(fresh), Some(older), false, 9000).unwrap();
        assert_eq!(
            kept.first_run, 1000,
            "the keychain's earlier trial wins once it is back"
        );
    }
    #[test]
    fn unreadable_file_keeps_the_keychain_record_and_fails_closed_without_one() {
        let old = Record::new(1000);
        let kept = initial_record(None, Some(old.clone()), true, 9000).unwrap();
        assert_eq!(kept.first_run, 1000);
        assert!(initial_record(None, None, true, 9000).is_none());
    }
    #[test]
    fn file_only_save_never_reports_a_keychain_failure() {
        let dir = std::env::temp_dir().join(format!("switchify-licence-{}", uuid::Uuid::new_v4()));
        let mut service = configured_service(Record::new(1000));
        service.path = dir.join("licence.json");
        assert_eq!(write_record_file(&service.path, &Record::new(1000)), Ok(()));
        let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
        assert_eq!(saved.first_run, 1000);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn stale_keychain_keeps_its_warning_and_backs_off_until_a_retry_succeeds() {
        let mut data = Data {
            record: None,
            storage_warning: true,
            recovery: false,
            recovery_armed: false,
            cleanup_pending: false,
            last_allowed: true,
            access_until: Some(0),
            secure_failures: 0,
            next_secure_retry: 0,
            file_dirty: true,
            file_warning: false,
            mirror_dirty: true,
            mirror_loaded: false,
            identity: None,
            clock: OfflineClock::new(now()),
        };
        // Retries at 60, then 60 + 120, then + 240 ... capped at 64 minutes.
        let mut time = 60;
        let mut expected = vec![];
        for _ in 0..8 {
            assert!(data.retry_secure_due(time));
            data.record_checkpoint(time, true, Ok(true));
            expected.push(time);
            // Non-retry minutes keep the warning and the schedule untouched.
            assert!(!data.retry_secure_due(time + 60));
            data.record_checkpoint(time + 60, false, Ok(false));
            assert!(data.storage_warning);
            time = data.next_secure_retry;
        }
        assert_eq!(expected, [60, 180, 420, 900, 1860, 3780, 7620, 11460]);
        // A file failure sets the warning without touching the keychain schedule.
        let scheduled = data.next_secure_retry;
        data.record_checkpoint(time, false, Err(()));
        assert!(data.storage_warning);
        assert_eq!(data.next_secure_retry, scheduled);
        // The first successful keychain retry clears everything.
        data.record_checkpoint(time, true, Ok(false));
        assert!(!data.storage_warning);
        assert_eq!(data.secure_failures, 0);
        assert!(!data.retry_secure_due(time + 60));
    }
    #[test]
    fn rejects_corrupt_trial_records() {
        let mut record = Record::new(1000);
        record.first_run = 2000;
        assert!(decode_record(&serde_json::to_string(&record).unwrap()).is_err());
    }
}
