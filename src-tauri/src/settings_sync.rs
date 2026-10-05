//! Syncs the portable settings document with the signed-in account's
//! `desktop_preferences` row.
//!
//! The row's `revision` is owned by the server (1 on insert, +1 per update),
//! so every write is conditional on the revision last seen: a write that
//! loses a race matches nothing and the engine re-reads and merges. The
//! last document synced with each account is kept locally as the merge base.
//! A document from a newer app is never overwritten.

use crate::account::{Account, Authorized};
use crate::portable_settings::{self, Document, Local, ParseError, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const SYNC_EVENT: &str = "settings-sync-changed";
pub const PROFILES_EVENT: &str = "switch-profiles-changed";
pub const REMOTE_SWITCHES_EVENT: &str = "remote-switches-changed";
const STATE_FILE: &str = "settings-sync.json";
/// How often local settings are compared with the last synced copy.
const POLL: Duration = Duration::from_secs(3);
/// How often the account's copy is checked for changes from other computers.
const REMOTE_CHECK: Duration = Duration::from_secs(300);
const MAX_BACKOFF: Duration = Duration::from_secs(300);
const MAX_ATTEMPTS: usize = 3;
/// Well under the server's 256 KiB limit.
const MAX_PAYLOAD_BYTES: usize = 200 * 1024;
const UPDATE_REQUIRED: &str =
    "Your synced settings were saved by a newer Switchify PC. Update Switchify PC on this computer to keep syncing.";

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The account's row as last read or written.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub revision: i64,
    pub schema_version: u32,
    pub payload: Value,
}

/// The last document synced with an account: the merge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Base {
    user_id: String,
    revision: i64,
    document: Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Choice {
    /// Upload this computer's settings over the account's copy.
    Local,
    /// Replace this computer's settings with the account's copy.
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Off,
    Syncing,
    UpToDate,
    /// This computer and the account both have different settings and no
    /// previous sync to merge from; the user picks which to keep.
    NeedsChoice,
    /// The account's copy came from a newer Switchify PC.
    UpdateRequired,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncView {
    pub status: Status,
    /// Milliseconds since the Unix epoch.
    pub last_synced_at: Option<i64>,
    pub message: Option<String>,
}

impl SyncView {
    /// For builds without accounts.
    pub fn unavailable() -> Self {
        Self::off()
    }

    fn off() -> Self {
        Self {
            status: Status::Off,
            last_synced_at: None,
            message: None,
        }
    }
}

/// The account's `desktop_preferences` row over PostgREST.
pub trait Remote: Send + Sync {
    fn fetch<'a>(&'a self, auth: &'a Authorized) -> BoxFuture<'a, Result<Option<Row>, String>>;
    /// `None` when another computer created the row first.
    fn create<'a>(
        &'a self,
        auth: &'a Authorized,
        payload: Value,
        updated_by: &'a str,
    ) -> BoxFuture<'a, Result<Option<Row>, String>>;
    /// `None` when the row is no longer at `revision`.
    fn update<'a>(
        &'a self,
        auth: &'a Authorized,
        revision: i64,
        payload: Value,
        updated_by: &'a str,
    ) -> BoxFuture<'a, Result<Option<Row>, String>>;
}

/// This install: its account, settings and sync state.
pub trait Host: Send + Sync {
    /// `Ok(None)` when no one is signed in.
    fn authorize(&self) -> BoxFuture<'_, Result<Option<Authorized>, String>>;
    fn local(&self) -> BoxFuture<'_, Result<Document, String>>;
    /// Applies a document and returns the resulting local document, which
    /// can differ (e.g. profile versions) and becomes the merge base.
    fn apply(&self, document: Document) -> BoxFuture<'_, Result<Document, String>>;
    /// The document of an install whose settings were never changed.
    fn untouched(&self) -> Document;
    fn load_state(&self) -> State;
    fn save_state(&self, state: &State) -> Result<(), String>;
    fn publish(&self, view: &SyncView);
}

/// Persisted next to the other settings files.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    /// Random per install, sent as `updated_by`. Never the BLE desktop id.
    #[serde(default)]
    install_id: String,
    #[serde(default)]
    base: Option<Base>,
    #[serde(default)]
    last_synced_at: Option<i64>,
}

/// What to do after comparing this install, the merge base and the account.
#[derive(Debug, PartialEq)]
enum Step {
    InSync,
    /// Local matches the account; record it as the base.
    Adopt,
    Create(Document),
    Upload {
        revision: i64,
        document: Document,
    },
    /// Apply `document` locally, then upload it if the account differs.
    Apply {
        document: Document,
        upload_over: Option<i64>,
    },
    NeedsChoice,
}

fn decide(
    local: &Document,
    untouched: &Document,
    base: Option<&Base>,
    remote: Option<(&Row, &Document)>,
    choice: Option<Choice>,
) -> Step {
    let Some((row, cloud)) = remote else {
        return Step::Create(local.clone());
    };
    if let Some(choice) = choice {
        return match choice {
            Choice::Local if local == cloud => Step::Adopt,
            Choice::Local => Step::Upload {
                revision: row.revision,
                document: local.clone(),
            },
            Choice::Cloud => Step::Apply {
                document: cloud.clone(),
                upload_over: None,
            },
        };
    }
    match base {
        Some(base) if base.revision == row.revision => {
            if *local == base.document {
                Step::InSync
            } else {
                Step::Upload {
                    revision: row.revision,
                    document: local.clone(),
                }
            }
        }
        Some(base) => {
            let merged = merge(&base.document, local, cloud);
            if merged == *local && merged == *cloud {
                Step::Adopt
            } else {
                Step::Apply {
                    upload_over: (merged != *cloud).then_some(row.revision),
                    document: merged,
                }
            }
        }
        None if local == cloud => Step::Adopt,
        // Nothing to lose on a computer that was never set up differently.
        None if local == untouched => Step::Apply {
            document: cloud.clone(),
            upload_over: None,
        },
        None => Step::NeedsChoice,
    }
}

/// Three-way merge by section. A section changed on only one side takes
/// that side; changed on both, the account's copy wins.
fn merge(base: &Document, local: &Document, cloud: &Document) -> Document {
    fn pick<T: Clone + PartialEq>(base: &T, local: &T, cloud: &T) -> T {
        if local == base {
            cloud.clone()
        } else if cloud == base {
            local.clone()
        } else {
            cloud.clone()
        }
    }
    Document {
        schema_version: SCHEMA_VERSION,
        app: pick(&base.app, &local.app, &cloud.app),
        profiles: pick(&base.profiles, &local.profiles, &cloud.profiles),
        switches: pick(&base.switches, &local.switches, &cloud.switches),
        point_scan: pick(&base.point_scan, &local.point_scan, &cloud.point_scan),
        remote_switches: pick(
            &base.remote_switches,
            &local.remote_switches,
            &cloud.remote_switches,
        ),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

enum Outcome {
    Synced,
    NeedsChoice,
    UpdateRequired,
}

pub struct Engine {
    remote: Arc<dyn Remote>,
    host: Arc<dyn Host>,
    // Only one sync runs at a time.
    running: tokio::sync::Mutex<()>,
    view: Mutex<SyncView>,
    wake: tokio::sync::Notify,
}

impl Engine {
    pub fn new(remote: Arc<dyn Remote>, host: Arc<dyn Host>) -> Self {
        let last_synced_at = host.load_state().last_synced_at;
        Self {
            remote,
            host,
            running: tokio::sync::Mutex::new(()),
            view: Mutex::new(SyncView {
                last_synced_at,
                ..SyncView::off()
            }),
            wake: tokio::sync::Notify::new(),
        }
    }

    pub fn view(&self) -> SyncView {
        self.view.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn set(&self, status: Status, message: Option<String>) {
        let view = {
            let mut view = self.view.lock().unwrap_or_else(|p| p.into_inner());
            view.status = status;
            view.message = message;
            if status == Status::UpToDate {
                view.last_synced_at = Some(now_ms());
            }
            if status == Status::Off {
                view.last_synced_at = None;
            }
            view.clone()
        };
        self.host.publish(&view);
    }

    /// Asks the background loop to sync now (e.g. after signing in).
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Forgets the merge base, e.g. after the account is deleted.
    pub fn forget(&self) {
        let mut state = self.host.load_state();
        state.base = None;
        state.last_synced_at = None;
        let _ = self.host.save_state(&state);
        self.set(Status::Off, None);
    }

    /// Runs one sync. `choice` answers a `NeedsChoice`.
    pub async fn sync(&self, choice: Option<Choice>) -> SyncView {
        let _running = self.running.lock().await;
        match self.host.authorize().await {
            Ok(None) => {
                self.set(Status::Off, None);
                return self.view();
            }
            Ok(Some(auth)) => {
                self.set(Status::Syncing, None);
                match self.sync_as(&auth, choice).await {
                    Ok(Outcome::Synced) => self.set(Status::UpToDate, None),
                    Ok(Outcome::NeedsChoice) => self.set(Status::NeedsChoice, None),
                    Ok(Outcome::UpdateRequired) => {
                        self.set(Status::UpdateRequired, Some(UPDATE_REQUIRED.into()))
                    }
                    Err(error) => self.set(Status::Error, Some(error)),
                }
            }
            Err(error) => self.set(Status::Error, Some(error)),
        }
        self.view()
    }

    async fn sync_as(&self, auth: &Authorized, choice: Option<Choice>) -> Result<Outcome, String> {
        let mut state = self.host.load_state();
        if state.install_id.is_empty() {
            state.install_id = uuid::Uuid::new_v4().to_string();
            self.host.save_state(&state)?;
        }
        for _ in 0..MAX_ATTEMPTS {
            // A base from another account is never merged with this one.
            let base = state
                .base
                .clone()
                .filter(|base| base.user_id == auth.user_id);
            let local = self.host.local().await?;
            let row = self.remote.fetch(auth).await?;
            let cloud = match &row {
                None => None,
                Some(row) => {
                    match portable_settings::parse(row.payload.clone(), row.schema_version) {
                        Ok(document) => Some(document),
                        // Never upload over a newer app's settings.
                        Err(ParseError::Newer(_)) => return Ok(Outcome::UpdateRequired),
                        Err(error @ ParseError::Invalid(_)) => return Err(error.to_string()),
                    }
                }
            };
            let remote = row.as_ref().zip(cloud.as_ref());
            let step = decide(
                &local,
                &self.host.untouched(),
                base.as_ref(),
                remote,
                choice,
            );
            let synced = match step {
                Step::InSync => true,
                Step::NeedsChoice => return Ok(Outcome::NeedsChoice),
                Step::Adopt => {
                    let (row, _) = remote.expect("adopt needs a row");
                    self.record(&mut state, auth, row.revision, local)?;
                    true
                }
                Step::Create(document) => {
                    match self
                        .remote
                        .create(auth, payload(&document)?, &state.install_id)
                        .await?
                    {
                        Some(row) => {
                            self.record(&mut state, auth, row.revision, document)?;
                            true
                        }
                        None => false,
                    }
                }
                Step::Upload { revision, document } => {
                    self.upload(&mut state, auth, revision, document).await?
                }
                Step::Apply {
                    document,
                    upload_over,
                } => {
                    let applied = if document == local {
                        local
                    } else {
                        self.host.apply(document).await?
                    };
                    match upload_over {
                        Some(revision) => self.upload(&mut state, auth, revision, applied).await?,
                        None => {
                            let (row, _) = remote.expect("apply needs a row");
                            self.record(&mut state, auth, row.revision, applied)?;
                            true
                        }
                    }
                }
            };
            if synced {
                return Ok(Outcome::Synced);
            }
            // Another computer wrote first: re-read and merge again.
        }
        Err("Settings changed on another computer at the same time. They will sync shortly.".into())
    }

    async fn upload(
        &self,
        state: &mut State,
        auth: &Authorized,
        revision: i64,
        document: Document,
    ) -> Result<bool, String> {
        match self
            .remote
            .update(auth, revision, payload(&document)?, &state.install_id)
            .await?
        {
            Some(row) => {
                self.record(state, auth, row.revision, document)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn record(
        &self,
        state: &mut State,
        auth: &Authorized,
        revision: i64,
        document: Document,
    ) -> Result<(), String> {
        state.base = Some(Base {
            user_id: auth.user_id.clone(),
            revision,
            document,
        });
        state.last_synced_at = Some(now_ms());
        self.host.save_state(state)
    }

    /// Whether this install has changed since the last sync with `user_id`.
    async fn changed_since_sync(&self) -> bool {
        let Some(base) = self.host.load_state().base else {
            return false;
        };
        matches!(self.host.local().await, Ok(local) if local != base.document)
    }

    /// Background loop: syncs at start, after `wake`, when local settings
    /// have changed and settled, and periodically for other computers'
    /// changes. Failures back off.
    pub async fn run(self: Arc<Self>) {
        let mut next_remote_check = Instant::now();
        let mut retry_at: Option<Instant> = None;
        let mut backoff = POLL;
        let mut previous: Option<Document> = None;
        loop {
            let woken = tokio::select! {
                () = self.wake.notified() => true,
                () = tokio::time::sleep(POLL) => false,
            };
            let now = Instant::now();
            let status = self.view().status;
            let waiting_for_user = matches!(status, Status::NeedsChoice | Status::UpdateRequired);
            let local = self.host.local().await.ok();
            // Upload once local changes have settled for one poll.
            let settled_change = local.is_some()
                && local == previous
                && !waiting_for_user
                && self.changed_since_sync().await;
            previous = local;
            let due = woken
                || now >= next_remote_check
                || (settled_change && retry_at.is_none_or(|at| now >= at));
            if !due {
                continue;
            }
            next_remote_check = now + REMOTE_CHECK;
            let view = self.sync(None).await;
            if view.status == Status::Error {
                retry_at = Some(Instant::now() + backoff);
                backoff = (backoff * 2).min(MAX_BACKOFF);
            } else {
                retry_at = None;
                backoff = POLL;
            }
        }
    }
}

fn payload(document: &Document) -> Result<Value, String> {
    let value = serde_json::to_value(document).map_err(|error| error.to_string())?;
    if value.to_string().len() > MAX_PAYLOAD_BYTES {
        return Err("Settings are too large to sync. Remove some switch profiles.".into());
    }
    Ok(value)
}

/// PostgREST access with the signed-in user's token.
pub struct HttpRemote {
    client: reqwest::Client,
    url: String,
    key: String,
}

impl HttpRemote {
    pub fn new(url: String, key: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            url,
            key,
        }
    }

    fn request(
        &self,
        method: reqwest::Method,
        query: &str,
        auth: &Authorized,
    ) -> reqwest::RequestBuilder {
        self.client
            .request(
                method,
                format!("{}/rest/v1/desktop_preferences?{query}", self.url),
            )
            .header("apikey", &self.key)
            .bearer_auth(&auth.access_token)
            .header("Prefer", "return=representation")
    }

    async fn rows(request: reqwest::RequestBuilder) -> Result<(u16, Vec<Row>), String> {
        let unreachable = |_| "Could not reach Switchify to sync settings.".to_string();
        let response = request.send().await.map_err(unreachable)?;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        let rows = body
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        Some(Row {
                            revision: row["revision"].as_i64()?,
                            schema_version: u32::try_from(row["schema_version"].as_u64()?).ok()?,
                            payload: row["payload"].clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok((status, rows))
    }
}

const COLUMNS: &str = "select=revision,schema_version,payload";

fn http_error(status: u16) -> String {
    match status {
        401 | 403 => "Sign in again to sync settings.".into(),
        _ => format!("Settings sync failed (HTTP {status}). It will retry."),
    }
}

impl Remote for HttpRemote {
    fn fetch<'a>(&'a self, auth: &'a Authorized) -> BoxFuture<'a, Result<Option<Row>, String>> {
        Box::pin(async move {
            let query = format!("{COLUMNS}&user_id=eq.{}", auth.user_id);
            let (status, mut rows) =
                Self::rows(self.request(reqwest::Method::GET, &query, auth)).await?;
            match status {
                200 => Ok(rows.pop()),
                _ => Err(http_error(status)),
            }
        })
    }

    fn create<'a>(
        &'a self,
        auth: &'a Authorized,
        payload: Value,
        updated_by: &'a str,
    ) -> BoxFuture<'a, Result<Option<Row>, String>> {
        Box::pin(async move {
            let body = json!({
                "user_id": auth.user_id,
                "schema_version": SCHEMA_VERSION,
                "payload": payload,
                "updated_by": updated_by,
            });
            let request = self
                .request(reqwest::Method::POST, COLUMNS, auth)
                .json(&body);
            let (status, mut rows) = Self::rows(request).await?;
            match status {
                200 | 201 => rows.pop().map(Some).ok_or_else(|| http_error(status)),
                409 => Ok(None),
                _ => Err(http_error(status)),
            }
        })
    }

    fn update<'a>(
        &'a self,
        auth: &'a Authorized,
        revision: i64,
        payload: Value,
        updated_by: &'a str,
    ) -> BoxFuture<'a, Result<Option<Row>, String>> {
        Box::pin(async move {
            let query = format!(
                "{COLUMNS}&user_id=eq.{}&revision=eq.{revision}",
                auth.user_id
            );
            let body = json!({
                "schema_version": SCHEMA_VERSION,
                "payload": payload,
                "updated_by": updated_by,
            });
            let request = self
                .request(reqwest::Method::PATCH, &query, auth)
                .json(&body);
            let (status, mut rows) = Self::rows(request).await?;
            match status {
                200 => Ok(rows.pop()),
                _ => Err(http_error(status)),
            }
        })
    }
}

/// The running app as the sync host.
pub struct AppHost {
    app: AppHandle,
    path: Option<PathBuf>,
}

impl AppHost {
    pub fn new(app: AppHandle) -> Self {
        let path = app
            .path()
            .app_config_dir()
            .ok()
            .map(|dir| dir.join(STATE_FILE));
        Self { app, path }
    }
}

impl Host for AppHost {
    fn authorize(&self) -> BoxFuture<'_, Result<Option<Authorized>, String>> {
        Box::pin(async move {
            let account = self.app.state::<Account>();
            if !account.current_view().await.signed_in {
                return Ok(None);
            }
            match account.authorize().await {
                Ok(auth) => Ok(Some(auth)),
                // A rejected session has just signed this install out.
                Err(_) if !account.current_view().await.signed_in => {
                    crate::account::emit(&self.app, &account.current_view().await);
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        })
    }

    fn local(&self) -> BoxFuture<'_, Result<Document, String>> {
        Box::pin(async move { Ok(Local::read(&self.app).document()) })
    }

    fn apply(&self, document: Document) -> BoxFuture<'_, Result<Document, String>> {
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let handle = self.app.clone();
            self.app
                .run_on_main_thread(move || {
                    let _ = tx.send(portable_settings::apply(&handle, document));
                })
                .map_err(|error| error.to_string())?;
            let applied = rx
                .await
                .map_err(|_| "Applying synced settings was cancelled.".to_string())??;
            if applied.profiles {
                let _ = self.app.emit(PROFILES_EVENT, ());
            }
            if applied.remote {
                let _ = self.app.emit(REMOTE_SWITCHES_EVENT, ());
            }
            Ok(Local::read(&self.app).document())
        })
    }

    fn untouched(&self) -> Document {
        Local {
            settings: crate::state::AppSettings::default(),
            profiles: Vec::new(),
            switches: crate::switches::Settings::default(),
            point_scan: crate::point_scan::Config::default(),
            remote: crate::remote_scan::Config::default(),
        }
        .document()
    }

    fn load_state(&self) -> State {
        self.path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &State) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or("Settings sync cannot find the settings folder.")?;
        let failed = |_| "Settings sync could not save its progress.".to_string();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(failed)?;
        }
        let bytes = serde_json::to_vec_pretty(state).map_err(|_| "Encoding failed".to_string())?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, bytes).map_err(failed)?;
        std::fs::rename(&temp, path).map_err(failed)
    }

    fn publish(&self, view: &SyncView) {
        let _ = self.app.emit(SYNC_EVENT, view);
    }
}

/// Installs the engine when accounts are configured and starts its loop.
pub fn install(app: &AppHandle) {
    let Some((url, key)) = app.state::<Account>().endpoint() else {
        return;
    };
    let engine = Arc::new(Engine::new(
        Arc::new(HttpRemote::new(url, key)),
        Arc::new(AppHost::new(app.clone())),
    ));
    app.manage(engine.clone());
    tauri::async_runtime::spawn(engine.run());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppSettings;
    use std::collections::VecDeque;

    fn untouched() -> Document {
        Local {
            settings: AppSettings::default(),
            profiles: Vec::new(),
            switches: crate::switches::Settings::default(),
            point_scan: crate::point_scan::Config::default(),
            remote: crate::remote_scan::Config::default(),
        }
        .document()
    }

    fn with_dwell(mut document: Document, enabled: bool) -> Document {
        document.app.dwell_click_enabled = enabled;
        document
    }

    fn with_speed(mut document: Document, speed: usize) -> Document {
        document.point_scan.speed = speed;
        document
    }

    fn auth(user: &str) -> Authorized {
        Authorized {
            user_id: user.into(),
            access_token: "token".into(),
        }
    }

    /// (kind, expected revision, payload, updated_by)
    type Write = (&'static str, Option<i64>, Value, String);

    #[derive(Default)]
    struct FakeRemote {
        row: Mutex<Option<Row>>,
        writes: Mutex<Vec<Write>>,
        /// Simulates another computer writing just before our next write.
        interleave: Mutex<VecDeque<Value>>,
        fail: Mutex<bool>,
    }

    impl FakeRemote {
        fn holding(document: &Document, revision: i64) -> Self {
            let remote = Self::default();
            *remote.row.lock().unwrap() = Some(Row {
                revision,
                schema_version: SCHEMA_VERSION,
                payload: serde_json::to_value(document).unwrap(),
            });
            remote
        }
        fn document(&self) -> Option<Document> {
            self.row
                .lock()
                .unwrap()
                .as_ref()
                .map(|row| serde_json::from_value(row.payload.clone()).unwrap())
        }
        fn revision(&self) -> Option<i64> {
            self.row.lock().unwrap().as_ref().map(|row| row.revision)
        }
        fn interleave(&self) {
            if let Some(payload) = self.interleave.lock().unwrap().pop_front() {
                let mut row = self.row.lock().unwrap();
                let revision = row.as_ref().map_or(1, |row| row.revision + 1);
                *row = Some(Row {
                    revision,
                    schema_version: SCHEMA_VERSION,
                    payload,
                });
            }
        }
    }

    impl Remote for FakeRemote {
        fn fetch<'a>(
            &'a self,
            _auth: &'a Authorized,
        ) -> BoxFuture<'a, Result<Option<Row>, String>> {
            Box::pin(async move {
                if *self.fail.lock().unwrap() {
                    return Err("offline".into());
                }
                Ok(self.row.lock().unwrap().clone())
            })
        }
        fn create<'a>(
            &'a self,
            _auth: &'a Authorized,
            payload: Value,
            updated_by: &'a str,
        ) -> BoxFuture<'a, Result<Option<Row>, String>> {
            Box::pin(async move {
                self.interleave();
                self.writes.lock().unwrap().push((
                    "create",
                    None,
                    payload.clone(),
                    updated_by.into(),
                ));
                let mut row = self.row.lock().unwrap();
                if row.is_some() {
                    return Ok(None);
                }
                *row = Some(Row {
                    revision: 1,
                    schema_version: SCHEMA_VERSION,
                    payload,
                });
                Ok(row.clone())
            })
        }
        fn update<'a>(
            &'a self,
            _auth: &'a Authorized,
            revision: i64,
            payload: Value,
            updated_by: &'a str,
        ) -> BoxFuture<'a, Result<Option<Row>, String>> {
            Box::pin(async move {
                self.interleave();
                self.writes.lock().unwrap().push((
                    "update",
                    Some(revision),
                    payload.clone(),
                    updated_by.into(),
                ));
                let mut row = self.row.lock().unwrap();
                match row.as_mut() {
                    Some(current) if current.revision == revision => {
                        current.revision += 1;
                        current.payload = payload;
                        Ok(Some(current.clone()))
                    }
                    _ => Ok(None),
                }
            })
        }
    }

    struct FakeHost {
        user: Mutex<Option<String>>,
        local: Mutex<Document>,
        state: Mutex<State>,
        applied: Mutex<Vec<Document>>,
        published: Mutex<Vec<SyncView>>,
        /// Mimics apply normalizing what it stores (e.g. profile versions).
        transform: fn(Document) -> Document,
    }

    impl FakeHost {
        fn new(local: Document) -> Self {
            Self {
                user: Mutex::new(Some("user-1".into())),
                local: Mutex::new(local),
                state: Mutex::default(),
                applied: Mutex::default(),
                published: Mutex::default(),
                transform: |document| document,
            }
        }
        fn local_now(&self) -> Document {
            self.local.lock().unwrap().clone()
        }
        fn set_local(&self, document: Document) {
            *self.local.lock().unwrap() = document;
        }
        fn base(&self) -> Option<Base> {
            self.state.lock().unwrap().base.clone()
        }
    }

    impl Host for FakeHost {
        fn authorize(&self) -> BoxFuture<'_, Result<Option<Authorized>, String>> {
            Box::pin(async move { Ok(self.user.lock().unwrap().as_deref().map(auth)) })
        }
        fn local(&self) -> BoxFuture<'_, Result<Document, String>> {
            Box::pin(async move { Ok(self.local_now()) })
        }
        fn apply(&self, document: Document) -> BoxFuture<'_, Result<Document, String>> {
            Box::pin(async move {
                self.applied.lock().unwrap().push(document.clone());
                let stored = (self.transform)(document);
                self.set_local(stored.clone());
                Ok(stored)
            })
        }
        fn untouched(&self) -> Document {
            untouched()
        }
        fn load_state(&self) -> State {
            self.state.lock().unwrap().clone()
        }
        fn save_state(&self, state: &State) -> Result<(), String> {
            *self.state.lock().unwrap() = state.clone();
            Ok(())
        }
        fn publish(&self, view: &SyncView) {
            self.published.lock().unwrap().push(view.clone());
        }
    }

    fn engine(remote: &Arc<FakeRemote>, host: &Arc<FakeHost>) -> Engine {
        Engine::new(remote.clone(), host.clone())
    }

    #[tokio::test]
    async fn signed_out_is_off_and_touches_nothing() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        *host.user.lock().unwrap() = None;
        let view = engine(&remote, &host).sync(None).await;
        assert_eq!(view.status, Status::Off);
        assert!(remote.writes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn first_computer_creates_the_row() {
        let remote = Arc::new(FakeRemote::default());
        let local = with_dwell(untouched(), true);
        let host = Arc::new(FakeHost::new(local.clone()));
        let view = engine(&remote, &host).sync(None).await;
        assert_eq!(view.status, Status::UpToDate);
        assert!(view.last_synced_at.is_some());
        assert_eq!(remote.document(), Some(local.clone()));
        let base = host.base().unwrap();
        assert_eq!((base.user_id.as_str(), base.revision), ("user-1", 1));
        let writes = remote.writes.lock().unwrap();
        let install_id = &writes[0].3;
        assert!(
            uuid::Uuid::parse_str(install_id).is_ok(),
            "random install id"
        );
    }

    #[tokio::test]
    async fn untouched_computer_takes_the_account_settings() {
        let cloud = with_dwell(untouched(), true);
        let remote = Arc::new(FakeRemote::holding(&cloud, 4));
        let host = Arc::new(FakeHost::new(untouched()));
        let view = engine(&remote, &host).sync(None).await;
        assert_eq!(view.status, Status::UpToDate);
        assert_eq!(host.local_now(), cloud);
        assert!(remote.writes.lock().unwrap().is_empty());
        assert_eq!(host.base().unwrap().revision, 4);
    }

    #[tokio::test]
    async fn customised_computer_asks_before_replacing_anything() {
        let cloud = with_dwell(untouched(), true);
        let local = with_speed(untouched(), 4);
        let remote = Arc::new(FakeRemote::holding(&cloud, 2));
        let host = Arc::new(FakeHost::new(local.clone()));
        let engine = engine(&remote, &host);
        assert_eq!(engine.sync(None).await.status, Status::NeedsChoice);
        assert_eq!(host.local_now(), local);
        assert!(remote.writes.lock().unwrap().is_empty());
        assert!(host.base().is_none());

        // Keep this computer's settings: uploaded over the account's copy.
        assert_eq!(
            engine.sync(Some(Choice::Local)).await.status,
            Status::UpToDate
        );
        assert_eq!(remote.document(), Some(local));
        assert_eq!(remote.revision(), Some(3));
    }

    #[tokio::test]
    async fn choosing_cloud_replaces_this_computers_settings() {
        let cloud = with_dwell(untouched(), true);
        let remote = Arc::new(FakeRemote::holding(&cloud, 2));
        let host = Arc::new(FakeHost::new(with_speed(untouched(), 4)));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        assert_eq!(
            engine.sync(Some(Choice::Cloud)).await.status,
            Status::UpToDate
        );
        assert_eq!(host.local_now(), cloud);
        assert!(remote.writes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn identical_settings_need_no_choice() {
        let local = with_speed(untouched(), 4);
        let remote = Arc::new(FakeRemote::holding(&local, 2));
        let host = Arc::new(FakeHost::new(local));
        assert_eq!(
            engine(&remote, &host).sync(None).await.status,
            Status::UpToDate
        );
        assert!(remote.writes.lock().unwrap().is_empty());
        assert_eq!(host.base().unwrap().revision, 2);
    }

    #[tokio::test]
    async fn local_change_uploads_on_the_last_seen_revision() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        host.set_local(with_dwell(untouched(), true));
        assert!(engine.changed_since_sync().await);
        engine.sync(None).await;
        let last = remote.writes.lock().unwrap().last().cloned().unwrap();
        assert_eq!((last.0, last.1), ("update", Some(1)));
        assert_eq!(remote.revision(), Some(2));
        assert!(!engine.changed_since_sync().await);
    }

    #[tokio::test]
    async fn remote_change_is_applied_and_not_echoed() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        // Another computer changes the account's copy.
        let other = with_dwell(untouched(), true);
        *remote.row.lock().unwrap() = Some(Row {
            revision: 2,
            schema_version: SCHEMA_VERSION,
            payload: serde_json::to_value(&other).unwrap(),
        });
        engine.sync(None).await;
        assert_eq!(host.local_now(), other);
        assert_eq!(remote.writes.lock().unwrap().len(), 1, "only the create");
        assert_eq!(host.base().unwrap().revision, 2);
    }

    #[tokio::test]
    async fn changes_on_both_sides_merge_by_section() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        *remote.row.lock().unwrap() = Some(Row {
            revision: 2,
            schema_version: SCHEMA_VERSION,
            payload: serde_json::to_value(with_dwell(untouched(), true)).unwrap(),
        });
        host.set_local(with_speed(untouched(), 4));
        engine.sync(None).await;
        let merged = with_speed(with_dwell(untouched(), true), 4);
        assert_eq!(host.local_now(), merged);
        assert_eq!(remote.document(), Some(merged));
        assert_eq!(remote.revision(), Some(3));
    }

    #[test]
    fn the_account_wins_a_section_changed_on_both_sides() {
        let base = untouched();
        let local = with_speed(base.clone(), 4);
        let cloud = with_speed(base.clone(), 1);
        assert_eq!(merge(&base, &local, &cloud).point_scan.speed, 1);
    }

    #[tokio::test]
    async fn losing_a_write_race_re_reads_and_merges() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        host.set_local(with_speed(untouched(), 4));
        // Another computer writes between our read and our write.
        remote
            .interleave
            .lock()
            .unwrap()
            .push_back(serde_json::to_value(with_dwell(untouched(), true)).unwrap());
        assert_eq!(engine.sync(None).await.status, Status::UpToDate);
        let merged = with_speed(with_dwell(untouched(), true), 4);
        assert_eq!(remote.document(), Some(merged.clone()));
        assert_eq!(host.local_now(), merged);
    }

    #[tokio::test]
    async fn create_race_falls_back_to_the_other_computers_row() {
        let remote = Arc::new(FakeRemote::default());
        let other = with_dwell(untouched(), true);
        remote
            .interleave
            .lock()
            .unwrap()
            .push_back(serde_json::to_value(&other).unwrap());
        let host = Arc::new(FakeHost::new(untouched()));
        assert_eq!(
            engine(&remote, &host).sync(None).await.status,
            Status::UpToDate
        );
        assert_eq!(
            host.local_now(),
            other,
            "untouched install takes the winner's"
        );
    }

    #[tokio::test]
    async fn newer_app_settings_are_never_overwritten() {
        let remote = Arc::new(FakeRemote::default());
        *remote.row.lock().unwrap() = Some(Row {
            revision: 9,
            schema_version: SCHEMA_VERSION + 1,
            payload: json!({"futureShape": true}),
        });
        let host = Arc::new(FakeHost::new(with_speed(untouched(), 4)));
        let engine = engine(&remote, &host);
        let view = engine.sync(Some(Choice::Local)).await;
        assert_eq!(view.status, Status::UpdateRequired);
        assert!(view.message.unwrap().contains("Update Switchify PC"));
        assert!(remote.writes.lock().unwrap().is_empty());
        assert!(host.applied.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_account_copy_is_an_error_and_not_overwritten() {
        let remote = Arc::new(FakeRemote::default());
        *remote.row.lock().unwrap() = Some(Row {
            revision: 3,
            schema_version: SCHEMA_VERSION,
            payload: json!({"schemaVersion": 1}),
        });
        let host = Arc::new(FakeHost::new(untouched()));
        let view = engine(&remote, &host).sync(None).await;
        assert_eq!(view.status, Status::Error);
        assert!(remote.writes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn another_accounts_base_is_never_merged() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        // A different account (e.g. deleted and recreated) with its own row.
        *host.user.lock().unwrap() = Some("user-2".into());
        let cloud = with_dwell(untouched(), true);
        *remote.row.lock().unwrap() = Some(Row {
            revision: 1,
            schema_version: SCHEMA_VERSION,
            payload: serde_json::to_value(&cloud).unwrap(),
        });
        host.set_local(with_speed(untouched(), 4));
        // Same revision number as the old base, but it must not count.
        assert_eq!(engine.sync(None).await.status, Status::NeedsChoice);
    }

    #[tokio::test]
    async fn normalized_apply_becomes_the_base_so_nothing_ping_pongs() {
        let cloud = with_dwell(untouched(), true);
        let remote = Arc::new(FakeRemote::holding(&cloud, 2));
        let mut host = FakeHost::new(untouched());
        // Applying stores something slightly different (like a profile
        // version bump); that stored copy must become the base.
        host.transform = |document| with_speed(document, 3);
        let host = Arc::new(host);
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        assert_eq!(host.base().unwrap().document, host.local_now());
        assert!(!engine.changed_since_sync().await);
        engine.sync(None).await;
        assert!(remote.writes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn network_failure_reports_an_error() {
        let remote = Arc::new(FakeRemote::default());
        *remote.fail.lock().unwrap() = true;
        let host = Arc::new(FakeHost::new(untouched()));
        let view = engine(&remote, &host).sync(None).await;
        assert_eq!(view.status, Status::Error);
        assert_eq!(view.message.as_deref(), Some("offline"));
    }

    #[tokio::test]
    async fn forget_clears_the_base_and_turns_off() {
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(untouched()));
        let engine = engine(&remote, &host);
        engine.sync(None).await;
        engine.forget();
        assert!(host.base().is_none());
        assert_eq!(engine.view().status, Status::Off);
        assert!(
            !host.state.lock().unwrap().install_id.is_empty(),
            "install id kept"
        );
    }

    #[tokio::test]
    async fn oversized_documents_are_not_uploaded() {
        let mut local = untouched();
        local.remote_switches[0].name = Some("x".repeat(MAX_PAYLOAD_BYTES));
        assert!(payload(&local).is_err());
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(local));
        assert_eq!(
            engine(&remote, &host).sync(None).await.status,
            Status::Error
        );
        assert!(remote.writes.lock().unwrap().is_empty());
    }

    /// Against a local `supabase start` stack (switchify-supabase). Run with:
    /// SWITCHIFY_LOCAL_SUPABASE_URL=http://127.0.0.1:54321
    /// SWITCHIFY_LOCAL_SUPABASE_KEY=<local publishable key>
    /// cargo test settings_sync::tests::local_stack -- --ignored
    #[tokio::test]
    #[ignore = "needs a local Supabase stack"]
    async fn local_stack_round_trips_the_document_and_enforces_revisions() {
        let url = std::env::var("SWITCHIFY_LOCAL_SUPABASE_URL").unwrap();
        let key = std::env::var("SWITCHIFY_LOCAL_SUPABASE_KEY").unwrap();
        let client = reqwest::Client::new();
        let mut users = Vec::new();
        for _ in 0..2 {
            let signup: Value = client
                .post(format!("{url}/auth/v1/signup"))
                .header("apikey", &key)
                .json(&json!({
                    "email": format!("sync-{}@example.test", uuid::Uuid::new_v4()),
                    "password": "local-only-password",
                }))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            users.push(Authorized {
                user_id: signup["user"]["id"].as_str().unwrap().into(),
                access_token: signup["access_token"].as_str().unwrap().into(),
            });
        }
        let (me, other) = (&users[0], &users[1]);
        let remote = HttpRemote::new(url, key);

        assert_eq!(remote.fetch(me).await.unwrap(), None);
        // The untouched document has explicit nulls (e.g. unassigned remote
        // slots); jsonb must keep them for the strict parser.
        let document = untouched();
        let value = payload(&document).unwrap();
        assert!(value.to_string().contains("null"));
        let created = remote
            .create(me, value.clone(), "install")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(created.revision, 1);
        let fetched = remote.fetch(me).await.unwrap().unwrap();
        assert_eq!(
            portable_settings::parse(fetched.payload, fetched.schema_version).unwrap(),
            document
        );
        assert_eq!(
            remote.create(me, value.clone(), "install").await.unwrap(),
            None,
            "409"
        );

        let changed = payload(&with_dwell(untouched(), true)).unwrap();
        let updated = remote
            .update(me, 1, changed.clone(), "install")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.revision, 2);
        assert_eq!(
            remote.update(me, 1, value, "install").await.unwrap(),
            None,
            "stale"
        );

        // Another account sees nothing of mine and cannot write to it.
        assert_eq!(remote.fetch(other).await.unwrap(), None);
        let forged = Authorized {
            user_id: me.user_id.clone(),
            access_token: other.access_token.clone(),
        };
        assert_eq!(
            remote.update(&forged, 2, json!({}), "x").await.unwrap(),
            None
        );
        assert_eq!(remote.fetch(me).await.unwrap().unwrap().payload, changed);
    }
}
