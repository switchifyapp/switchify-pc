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
use crate::state::SwitchProfile;
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
pub const PANELS_EVENT: &str = crate::custom_panels::EVENT;
const STATE_FILE: &str = "settings-sync.json";
/// How often local settings are compared with the last synced copy.
const POLL: Duration = Duration::from_secs(3);
/// How often the account's copy is checked for changes from other computers.
const REMOTE_CHECK: Duration = Duration::from_secs(300);
const MAX_BACKOFF: Duration = Duration::from_secs(300);
const MAX_ATTEMPTS: usize = 3;
/// Well under the server's 256 KiB limit.
const MAX_PAYLOAD_BYTES: usize = 200 * 1024;
/// The custom profile limit enforced when settings are applied.
const MAX_SYNCED_PROFILES: usize = 32;
const TOO_MANY_PROFILES: &str = "Your computers have more than 32 switch profiles between them. Delete some on this computer, then sync again.";
const TOO_MANY_PANELS: &str = "Your computers have more than 32 panels between them. Delete some on this computer, then sync again.";
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

/// The last sync with an account: the merge base. The account's copy and
/// this install's copy are kept apart because applying can normalize values
/// (e.g. profile versions), so the two need not be equal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Base {
    user_id: String,
    revision: i64,
    /// The account's document at `revision`.
    cloud: Document,
    /// This install's document right after that sync.
    local: Document,
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
    /// Applies `document` if this install still matches `expected` (the
    /// settings the merge was computed from), returning the resulting local
    /// document, which can differ (e.g. profile versions). `Ok(None)` means
    /// the user changed something meanwhile; the sync re-reads and retries.
    /// Switch sync is checked again at the moment of applying (see
    /// `to_apply`), since it may have been turned off since the sync began.
    fn apply(
        &self,
        expected: Document,
        document: Document,
    ) -> BoxFuture<'_, Result<Option<Document>, String>>;
    /// The document of an install whose settings were never changed.
    fn untouched(&self) -> Document;
    /// Whether this install shares its switches. Off when this computer has
    /// its own switch interface: its switches then neither upload nor change.
    fn sync_switches(&self) -> bool;
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
    /// Apply `document` locally, then upload the result if it differs from
    /// the account's copy.
    Apply {
        document: Document,
        upload: bool,
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
                upload: false,
            },
        };
    }
    match base {
        Some(base) if base.revision == row.revision => {
            if *local == base.local {
                Step::InSync
            } else {
                Step::Upload {
                    revision: row.revision,
                    document: local.clone(),
                }
            }
        }
        Some(base) => {
            let merged = merge(base, local, cloud);
            if merged == *local && merged == *cloud {
                Step::Adopt
            } else {
                Step::Apply {
                    upload: merged != *cloud,
                    document: merged,
                }
            }
        }
        None if local == cloud => Step::Adopt,
        // Nothing to lose on a computer that was never set up differently.
        None if local == untouched => Step::Apply {
            document: cloud.clone(),
            upload: false,
        },
        None => Step::NeedsChoice,
    }
}

/// The settings this install shares. With switch sync off, its own switches
/// are left out and `account`'s stand in, so they never upload and never
/// look like a change on either side.
fn shared(own: &Document, sync_switches: bool, account: &Document) -> Document {
    if sync_switches {
        return own.clone();
    }
    with_switches_of(own.clone(), account)
}

/// `document` with `from`'s switches: the keys, actions and timing of this
/// computer's own switches, and the remote switch slots.
fn with_switches_of(mut document: Document, from: &Document) -> Document {
    document.switches = from.switches.clone();
    document.remote_switches = from.remote_switches.clone();
    document
}

/// What applying `document` over `expected` writes, given whether switch
/// sync is on right now: with it off, this install's switches stay.
fn to_apply(document: Document, expected: &Document, sync_switches: bool) -> Document {
    if sync_switches {
        document
    } else {
        with_switches_of(document, expected)
    }
}

/// Picks between this install's and the account's version of one value.
/// Local changes are measured against the local base and account changes
/// against the account base, so normalization on one side (e.g. a profile
/// version bump on apply) never looks like a change on the other. Changed
/// on both sides, the account's copy wins.
fn pick<T: Clone + PartialEq>(base_local: &T, base_cloud: &T, local: &T, cloud: &T) -> T {
    if local == base_local {
        cloud.clone()
    } else if cloud == base_cloud {
        local.clone()
    } else {
        cloud.clone()
    }
}

/// Three-way merge by section; profiles merge one by one.
fn merge(base: &Base, local: &Document, cloud: &Document) -> Document {
    let (bl, bc) = (&base.local, &base.cloud);
    Document {
        schema_version: SCHEMA_VERSION,
        app: pick(&bl.app, &bc.app, &local.app, &cloud.app),
        profiles: merge_profiles(&bl.profiles, &bc.profiles, &local.profiles, &cloud.profiles),
        switches: pick(&bl.switches, &bc.switches, &local.switches, &cloud.switches),
        point_scan: pick(
            &bl.point_scan,
            &bc.point_scan,
            &local.point_scan,
            &cloud.point_scan,
        ),
        remote_switches: pick(
            &bl.remote_switches,
            &bc.remote_switches,
            &local.remote_switches,
            &cloud.remote_switches,
        ),
        panels: crate::custom_panels::merge(&bl.panels, &bc.panels, &local.panels, &cloud.panels),
        menu_overrides: crate::custom_panels::merge_overrides(
            &bl.menu_overrides,
            &bc.menu_overrides,
            &local.menu_overrides,
            &cloud.menu_overrides,
        ),
        panel_extras: crate::panel_extras::merge(
            &bl.panel_extras,
            &bc.panel_extras,
            &local.panel_extras,
            &cloud.panel_extras,
        ),
    }
}

/// Merges custom profiles by id, so profiles added or edited on different
/// computers at the same time are all kept. A profile deleted on one side
/// stays deleted unless the other side changed it since the last sync.
/// Names made equal by the merge get a numbered suffix, since names must
/// stay unique.
fn merge_profiles(
    base_local: &[SwitchProfile],
    base_cloud: &[SwitchProfile],
    local: &[SwitchProfile],
    cloud: &[SwitchProfile],
) -> Vec<SwitchProfile> {
    let find = |list: &[SwitchProfile], id: &str| list.iter().find(|p| p.id == id).cloned();
    let mut ids: Vec<String> = cloud.iter().map(|p| p.id.clone()).collect();
    ids.extend(
        local
            .iter()
            .filter(|p| !cloud.iter().any(|c| c.id == p.id))
            .map(|p| p.id.clone()),
    );
    // Versions can differ between computers (e.g. after a lost race bumps one
    // copy), so only content counts as a change.
    let content = |p: &Option<SwitchProfile>| {
        p.as_ref()
            .map(|p| (p.name.clone(), p.provider.clone(), p.bindings.clone()))
    };
    let mut merged: Vec<SwitchProfile> = ids
        .iter()
        .filter_map(|id| {
            let (bl, bc) = (find(base_local, id), find(base_cloud, id));
            let (l, c) = (find(local, id), find(cloud, id));
            let local_changed = content(&l) != content(&bl);
            let cloud_changed = content(&c) != content(&bc);
            let mut chosen = match (local_changed, cloud_changed) {
                (false, _) => c.clone(),
                (true, false) => l.clone(),
                // Both changed: the account wins, but an edit is never lost
                // to a deletion on the other side.
                (true, true) => c.clone().or(l.clone()),
            }?;
            // Never move a version backwards, so connected phones notice.
            chosen.version = [&l, &c]
                .into_iter()
                .flatten()
                .map(|p| p.version)
                .max()
                .unwrap_or(chosen.version);
            Some(chosen)
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    for profile in &mut merged {
        if !seen.insert(profile.name.to_ascii_lowercase()) {
            let base: String = profile.name.chars().take(44).collect();
            let name = (2..)
                .map(|n| format!("{base} ({n})"))
                .find(|candidate| !seen.contains(&candidate.to_ascii_lowercase()))
                .expect("an unused suffix always exists");
            seen.insert(name.to_ascii_lowercase());
            profile.name = name;
        }
    }
    merged
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
    // Only one sync (or forget) runs at a time.
    running: tokio::sync::Mutex<()>,
    // Mirrors the state file, so polling never re-reads it.
    state: Mutex<State>,
    view: Mutex<SyncView>,
    wake: tokio::sync::Notify,
}

impl Engine {
    pub fn new(remote: Arc<dyn Remote>, host: Arc<dyn Host>) -> Self {
        let mut state = host.load_state();
        // A base saved by an older build has already been read into this
        // version's settings, so it only differs by its version number.
        // Left alone, that alone would upload a newer document and stop
        // computers still on the older build syncing.
        if let Some(base) = state.base.as_mut() {
            base.cloud.schema_version = SCHEMA_VERSION;
            base.local.schema_version = SCHEMA_VERSION;
        }
        Self {
            remote,
            host,
            running: tokio::sync::Mutex::new(()),
            view: Mutex::new(SyncView {
                last_synced_at: state.last_synced_at,
                ..SyncView::off()
            }),
            state: Mutex::new(state),
            wake: tokio::sync::Notify::new(),
        }
    }

    pub fn view(&self) -> SyncView {
        self.view.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn state(&self) -> State {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn save(&self, state: &State) -> Result<(), String> {
        self.host.save_state(state)?;
        *self.state.lock().unwrap_or_else(|p| p.into_inner()) = state.clone();
        Ok(())
    }

    fn set(&self, status: Status, message: Option<String>) {
        let view = {
            let mut view = self.view.lock().unwrap_or_else(|p| p.into_inner());
            // Repeated identical states (e.g. staying signed out) are not
            // re-announced.
            if view.status == status && view.message == message && status != Status::UpToDate {
                return;
            }
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

    /// Forgets the merge base, e.g. after the account is deleted. Waits for
    /// any running sync so it cannot write the base back.
    pub async fn forget(&self) {
        let _running = self.running.lock().await;
        let mut state = self.state();
        state.base = None;
        state.last_synced_at = None;
        let _ = self.save(&state);
        self.set(Status::Off, None);
    }

    /// Runs one sync. `choice` answers a `NeedsChoice`.
    pub async fn sync(&self, choice: Option<Choice>) -> SyncView {
        let _running = self.running.lock().await;
        match self.host.authorize().await {
            Ok(None) => self.set(Status::Off, None),
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
        let mut state = self.state();
        if state.install_id.is_empty() {
            state.install_id = uuid::Uuid::new_v4().to_string();
            self.save(&state)?;
        }
        for _ in 0..MAX_ATTEMPTS {
            // A base from another account is never merged with this one.
            let base = state
                .base
                .clone()
                .filter(|base| base.user_id == auth.user_id);
            let own = self.host.local().await?;
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
            let untouched = self.host.untouched();
            let sync_switches = self.host.sync_switches();
            // Without an account copy, a computer that keeps its switches
            // shares untouched ones.
            let account = cloud.as_ref().unwrap_or(&untouched);
            let local = shared(&own, sync_switches, account);
            let step = decide(
                &local,
                &shared(&untouched, sync_switches, account),
                base.as_ref(),
                remote,
                choice,
            );
            let synced = match step {
                Step::InSync => true,
                Step::NeedsChoice => return Ok(Outcome::NeedsChoice),
                Step::Adopt => {
                    let (row, cloud) = remote.expect("adopt needs a row");
                    self.record(&mut state, auth, row.revision, cloud.clone(), local)?;
                    true
                }
                // Switch sync was turned off since this attempt began, so the
                // document may hold this computer's switches: start over.
                Step::Create(_) | Step::Upload { .. } if self.turned_off(sync_switches) => false,
                Step::Create(document) => {
                    match self
                        .remote
                        .create(auth, payload(&document)?, &state.install_id)
                        .await?
                    {
                        Some(row) => {
                            self.record(
                                &mut state,
                                auth,
                                row.revision,
                                document.clone(),
                                document,
                            )?;
                            true
                        }
                        None => false,
                    }
                }
                Step::Upload { revision, document } => {
                    self.upload(&mut state, auth, revision, document).await?
                }
                Step::Apply { document, upload } => {
                    let (row, cloud) = remote.expect("apply needs a row");
                    if document.profiles.len() > MAX_SYNCED_PROFILES {
                        return Err(TOO_MANY_PROFILES.into());
                    }
                    if document.panels.len() > crate::custom_panels::MAX_PANELS {
                        return Err(TOO_MANY_PANELS.into());
                    }
                    // This computer's own switches stay as they are.
                    let document = to_apply(document, &own, sync_switches);
                    let applied = if document == own {
                        Some(own.clone())
                    } else {
                        self.host.apply(own.clone(), document).await?
                    };
                    // Read again: switch sync may have been turned off during
                    // the apply, and then this computer's switches must not
                    // upload.
                    let sync_switches = sync_switches && self.host.sync_switches();
                    let applied = applied.map(|applied| shared(&applied, sync_switches, cloud));
                    match applied {
                        // The user changed something meanwhile: start over.
                        None => false,
                        Some(applied) if upload && applied != *cloud => {
                            // Recorded before uploading with the account's copy
                            // on both sides: if the upload fails, sections taken
                            // from the account do not look like local changes,
                            // while this install's own changes still do and are
                            // uploaded on the next sync.
                            self.record(
                                &mut state,
                                auth,
                                row.revision,
                                cloud.clone(),
                                cloud.clone(),
                            )?;
                            self.upload(&mut state, auth, row.revision, applied).await?
                        }
                        Some(applied) => {
                            self.record(&mut state, auth, row.revision, cloud.clone(), applied)?;
                            true
                        }
                    }
                }
            };
            if synced {
                return Ok(Outcome::Synced);
            }
            // Another computer wrote first, or the user changed something:
            // re-read and merge again.
        }
        Err("Settings changed on another computer at the same time. They will sync shortly.".into())
    }

    /// Whether switch sync was on (`was`) and has since been turned off.
    fn turned_off(&self, was: bool) -> bool {
        was && !self.host.sync_switches()
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
                self.record(state, auth, row.revision, document.clone(), document)?;
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
        cloud: Document,
        local: Document,
    ) -> Result<(), String> {
        state.base = Some(Base {
            user_id: auth.user_id.clone(),
            revision,
            cloud,
            local,
        });
        state.last_synced_at = Some(now_ms());
        self.save(state)
    }

    /// Whether this install has changed since its last sync.
    async fn changed_since_sync(&self) -> bool {
        let Some(base) = self.state().base else {
            return false;
        };
        let sync_switches = self.host.sync_switches();
        matches!(
            self.host.local().await,
            Ok(local) if shared(&local, sync_switches, &base.cloud) != base.local
        )
    }

    /// Background loop: syncs at start, after `wake`, when local settings
    /// have changed and settled, when a failed sync is due a retry, and
    /// periodically for other computers' changes. Failures back off.
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
            // Signed out, or waiting for the user: only a wake or the
            // periodic check (which notices a sign-in) runs a sync.
            let idle = matches!(
                status,
                Status::Off | Status::NeedsChoice | Status::UpdateRequired
            );
            let local = if idle {
                None
            } else {
                self.host.local().await.ok()
            };
            // Upload once local changes have settled for one poll.
            let settled_change =
                local.is_some() && local == previous && self.changed_since_sync().await;
            previous = local;
            let retry_due = status == Status::Error && retry_at.is_some_and(|at| now >= at);
            let due = woken
                || now >= next_remote_check
                || retry_due
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
        return Err(
            "Settings are too large to sync. Remove some switch profiles or panels.".into(),
        );
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

    async fn rows(request: reqwest::RequestBuilder) -> Result<(u16, Vec<Row>, Value), String> {
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
        Ok((status, rows, body))
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
            let (status, mut rows, _) =
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
            let (status, mut rows, body) = Self::rows(request).await?;
            match status {
                200 | 201 => rows.pop().map(Some).ok_or_else(|| http_error(status)),
                // 23503: the account no longer exists (deleted elsewhere).
                409 if body["code"] == "23503" => {
                    Err("This account no longer exists. Sign in again.".into())
                }
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
            let (status, mut rows, _) = Self::rows(request).await?;
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

    fn apply(
        &self,
        expected: Document,
        document: Document,
    ) -> BoxFuture<'_, Result<Option<Document>, String>> {
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let handle = self.app.clone();
            self.app
                .run_on_main_thread(move || {
                    // Checked on the main thread, where settings commands run,
                    // so no edit can slip in between the check and the apply.
                    // The result is read here too, before any later command
                    // can change settings and have its edit taken as synced.
                    let current = Local::read(&handle);
                    let result = if current.document() == expected {
                        let document =
                            to_apply(document, &expected, current.settings.sync_switches);
                        portable_settings::apply(&handle, document)
                            .map(|applied| Some((applied, Local::read(&handle).document())))
                    } else {
                        Ok(None)
                    };
                    let _ = tx.send(result);
                })
                .map_err(|error| error.to_string())?;
            let Some((applied, result)) = rx
                .await
                .map_err(|_| "Applying synced settings was cancelled.".to_string())??
            else {
                return Ok(None);
            };
            if applied.profiles {
                let _ = self.app.emit(PROFILES_EVENT, ());
            }
            if applied.remote {
                let _ = self.app.emit(REMOTE_SWITCHES_EVENT, ());
            }
            if applied.panels {
                let _ = self.app.emit(PANELS_EVENT, ());
            }
            Ok(Some(result))
        })
    }

    fn untouched(&self) -> Document {
        Local {
            settings: crate::state::AppSettings::default(),
            profiles: Vec::new(),
            switches: crate::switches::Settings::default(),
            point_scan: crate::point_scan::Config::default(),
            remote: crate::remote_scan::Config::default(),
            panels: Vec::new(),
            menu_overrides: Default::default(),
            panel_extras: Default::default(),
        }
        .document()
    }

    fn sync_switches(&self) -> bool {
        let model = self.app.state::<crate::state::AppModel>();
        let data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.state.settings.sync_switches
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
mod tests;
