//! Opt-in diagnostics reported to Sentry.
//!
//! Envelopes are built here from an allowlist rather than by the Sentry SDK,
//! so nothing the SDK collects by default (host name, device model, IP-derived
//! context, breadcrumbs, module lists, minidumps of process memory) can reach
//! a report. Nothing is stored or sent until the user opts in.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(test)]
use serde_json::Value;

use crate::diagnostics::sanitize_detail;
use crate::state::{ActivityKind, AppState};
use crate::storage::replace_file;

const MAX_QUEUED_REPORTS: usize = 20;
/// Delays before flushing again after a retryable failure; the last repeats.
const RETRY_DELAYS: [Duration; 4] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
    Duration::from_secs(1800),
];
const SDK_NAME: &str = "switchify-pc";
const RELEASE_PREFIX: &str = "switchify-pc@";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TelemetryConsent {
    Undecided,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryView {
    pub consent: TelemetryConsent,
    pub available: bool,
}

/// Where envelopes go, derived from a Sentry DSN of the form
/// `https://<public key>@<host>[/<path>]/<project id>`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TelemetryConfig {
    envelope_url: String,
    public_key: String,
}

impl TelemetryConfig {
    fn from_build() -> Option<Self> {
        Self::from_dsn(option_env!("SENTRY_DSN").unwrap_or_default())
    }

    fn from_dsn(dsn: &str) -> Option<Self> {
        let rest = dsn.trim().strip_prefix("https://")?;
        let (credentials, location) = rest.split_once('@')?;
        // Legacy DSNs carry `key:secret`; only the public key is ever sent.
        let public_key = credentials.split(':').next()?;
        let (host, path) = location.split_once('/')?;
        let path = path.trim_end_matches('/');
        let (prefix, project) = path.rsplit_once('/').unwrap_or(("", path));
        let valid_key = !public_key.is_empty()
            && public_key
                .chars()
                .all(|character| character.is_ascii_alphanumeric());
        let valid_host = !host.is_empty()
            && host
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-:".contains(character));
        let valid_prefix = prefix
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character));
        let valid_project =
            !project.is_empty() && project.chars().all(|character| character.is_ascii_digit());
        if !(valid_key && valid_host && valid_prefix && valid_project) {
            return None;
        }
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("/{prefix}")
        };
        Some(Self {
            envelope_url: format!("https://{host}{prefix}/api/{project}/envelope/"),
            public_key: public_key.to_owned(),
        })
    }
}

/// A report waiting to be delivered. Fields from the earlier Timberlogs queue
/// format that are not listed here are ignored, so a queue written by an older
/// build still loads and is delivered to Sentry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct QueuedReport {
    id: String,
    level: String,
    message: String,
    dataset: String,
    version: String,
    tags: Vec<String>,
    timestamp: String,
    /// `file.rs:line` of a panic: the file name only, never a directory.
    #[serde(default)]
    location: Option<String>,
    /// The release-health session this report ended as crashed.
    #[serde(default)]
    crashed_session: Option<SessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SessionRecord {
    id: String,
    started_ms: i64,
    /// True until the session's opening update is known to be delivered, so
    /// a crashed update sent without it opens the session as well.
    #[serde(default)]
    init: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryDisk {
    install_id: String,
    #[serde(default)]
    queue: Vec<QueuedReport>,
}

/// The running build, recorded by `start` so later reports, and a session
/// opened by an opt-in partway through a launch, carry it.
#[derive(Debug, Clone)]
struct BuildInfo {
    version: String,
    platform: String,
}

struct TelemetryData {
    consent: TelemetryConsent,
    install_id: Option<String>,
    queue: Vec<QueuedReport>,
    last_error: Option<String>,
    build: Option<BuildInfo>,
    session: Option<SessionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendOutcome {
    Sent,
    Retry,
    Drop,
}

type SendFuture = Pin<Box<dyn Future<Output = SendOutcome> + Send>>;

trait TelemetryTransport: Send + Sync {
    fn send(&self, config: TelemetryConfig, envelope: String) -> SendFuture;
}

struct HttpTransport {
    client: reqwest::Client,
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl HttpTransport {
    fn build_request(
        &self,
        config: TelemetryConfig,
        envelope: String,
    ) -> Result<reqwest::Request, reqwest::Error> {
        self.client
            .post(config.envelope_url)
            .header(
                "X-Sentry-Auth",
                format!(
                    "Sentry sentry_version=7, sentry_key={}, sentry_client={SDK_NAME}/{}",
                    config.public_key,
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-sentry-envelope",
            )
            .body(envelope)
            .build()
    }
}

impl TelemetryTransport for HttpTransport {
    fn send(&self, config: TelemetryConfig, envelope: String) -> SendFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let request = HttpTransport {
                client: client.clone(),
            }
            .build_request(config, envelope);
            let response = match request {
                Ok(request) => client.execute(request).await,
                Err(_) => return SendOutcome::Drop,
            };
            match response {
                Ok(response) if response.status().is_success() => SendOutcome::Sent,
                Ok(response)
                    if response.status().as_u16() == 408
                        || response.status().as_u16() == 429
                        || response.status().is_server_error() =>
                {
                    SendOutcome::Retry
                }
                Ok(_) => SendOutcome::Drop,
                Err(_) => SendOutcome::Retry,
            }
        })
    }
}

#[derive(Clone)]
pub struct TelemetryService {
    inner: Arc<TelemetryInner>,
}

struct TelemetryInner {
    path: PathBuf,
    config: Option<TelemetryConfig>,
    data: Mutex<TelemetryData>,
    flushing: AtomicBool,
    retry_scheduled: AtomicBool,
    retry_attempt: AtomicUsize,
    retry_delays: Vec<Duration>,
    consent_tx: tokio::sync::watch::Sender<bool>,
    transport: Arc<dyn TelemetryTransport>,
}

impl std::fmt::Debug for TelemetryService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TelemetryService")
            .field("path", &self.inner.path)
            .field("available", &self.inner.config.is_some())
            .finish_non_exhaustive()
    }
}

static PANIC_HOOK: Once = Once::new();

impl TelemetryService {
    pub fn new(path: PathBuf, consent: TelemetryConsent) -> Self {
        Self::with_transport(
            path,
            consent,
            TelemetryConfig::from_build(),
            Arc::new(HttpTransport::default()),
        )
    }

    fn with_transport(
        path: PathBuf,
        consent: TelemetryConsent,
        config: Option<TelemetryConfig>,
        transport: Arc<dyn TelemetryTransport>,
    ) -> Self {
        Self::with_retry_delays(path, consent, config, transport, RETRY_DELAYS.to_vec())
    }

    fn with_retry_delays(
        path: PathBuf,
        consent: TelemetryConsent,
        config: Option<TelemetryConfig>,
        transport: Arc<dyn TelemetryTransport>,
        retry_delays: Vec<Duration>,
    ) -> Self {
        let disk = (consent == TelemetryConsent::Enabled)
            .then(|| load_disk(&path))
            .flatten();
        let install_id = disk
            .as_ref()
            .map(|disk| disk.install_id.clone())
            .or_else(|| {
                (consent == TelemetryConsent::Enabled).then(|| uuid::Uuid::new_v4().to_string())
            });
        let (consent_tx, _) = tokio::sync::watch::channel(consent == TelemetryConsent::Enabled);
        let service = Self {
            inner: Arc::new(TelemetryInner {
                path,
                config,
                data: Mutex::new(TelemetryData {
                    consent,
                    install_id,
                    queue: disk.map_or_else(Vec::new, |disk| disk.queue),
                    last_error: None,
                    build: None,
                    session: None,
                }),
                flushing: AtomicBool::new(false),
                retry_scheduled: AtomicBool::new(false),
                retry_attempt: AtomicUsize::new(0),
                retry_delays,
                consent_tx,
                transport,
            }),
        };
        if consent != TelemetryConsent::Enabled {
            service.purge();
        } else {
            let data = service
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let _ = persist_locked(&service.inner.path, &data);
        }
        service
    }

    pub fn view(&self) -> TelemetryView {
        let data = self
            .inner
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        TelemetryView {
            consent: data.consent,
            available: self.inner.config.is_some(),
        }
    }

    pub fn set_consent(&self, consent: TelemetryConsent) {
        if consent != TelemetryConsent::Enabled {
            self.inner.consent_tx.send_replace(false);
        }
        {
            let mut data = self
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            data.consent = consent;
            data.last_error = None;
            if consent == TelemetryConsent::Enabled && data.install_id.is_none() {
                data.install_id = Some(uuid::Uuid::new_v4().to_string());
            }
            if consent != TelemetryConsent::Enabled {
                data.install_id = None;
                data.queue.clear();
                data.session = None;
            }
            let _ = persist_locked(&self.inner.path, &data);
        }
        if consent != TelemetryConsent::Enabled {
            self.purge();
        } else {
            self.inner.consent_tx.send_replace(true);
            self.start_session();
            self.flush();
        }
    }

    /// Records the running build, opens this launch's release-health session
    /// and delivers anything queued earlier, including a panic recorded in
    /// the previous launch.
    pub fn start(&self, version: &str, platform: &str) {
        {
            let mut data = self
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            data.build = Some(BuildInfo {
                version: safe_label(version),
                platform: safe_label(platform),
            });
        }
        self.start_session();
        self.flush();
    }

    /// Records panics from now on, then runs the previous hook so the
    /// standard panic output is unchanged. Installed once per process.
    pub fn install_panic_hook(&self) {
        let service = self.clone();
        PANIC_HOOK.call_once(move || {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let kind = panic_kind(info.payload_as_str().unwrap_or_default());
                let location = info
                    .location()
                    .map(|location| (location.file(), location.line()));
                service.record_panic(kind, location);
                previous(info);
            }));
        });
    }

    /// Queues and persists a panic, described only by its `panic_kind` and
    /// location, as a fatal event that also ends this launch's session as
    /// crashed. It runs inside the panic hook, so it never waits for the state
    /// lock (the panic may have happened while holding it) and never sends:
    /// the process may be about to end, and the next flush or launch delivers
    /// the persisted report.
    fn record_panic(&self, kind: &'static str, location: Option<(&str, u32)>) {
        if self.inner.config.is_none() {
            return;
        }
        // Another thread may hold the lock for a moment (a flush or a disk
        // write), so try a bounded number of times; a poisoned lock still
        // holds valid data. If this thread holds it, give up rather than wait.
        let mut data = None;
        for _ in 0..200 {
            match self.inner.data.try_lock() {
                Ok(guard) => data = Some(guard),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    data = Some(poisoned.into_inner());
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
            }
            break;
        }
        let Some(mut data) = data else {
            return;
        };
        let Some(mut report) = create_report(&data, "fatal", "panic", kind) else {
            return;
        };
        report.location = location.map(|(file, line)| safe_location(file, line));
        report.crashed_session = data.session.take();
        push_report(&mut data, report);
        let _ = persist_locked(&self.inner.path, &data);
    }

    pub fn observe(&self, state: &AppState) {
        let Some(activity) = state
            .last_activity
            .as_ref()
            .filter(|activity| activity.kind == ActivityKind::Error)
        else {
            return;
        };
        let message = sanitize_detail(&activity.message);
        {
            let mut data = self
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if data.last_error.as_deref() == Some(&message) {
                return;
            }
            data.last_error = Some(message.clone());
        }
        self.report_exception(&message, &state.version, &state.capabilities.platform);
    }

    pub fn report_exception(&self, message: &str, version: &str, platform: &str) {
        if self.inner.config.is_none() {
            return;
        }
        {
            let mut data = self
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if data.build.is_none() {
                data.build = Some(BuildInfo {
                    version: safe_label(version),
                    platform: safe_label(platform),
                });
            }
            let Some(report) = create_report(&data, "error", "runtime", message) else {
                return;
            };
            push_report(&mut data, report);
            let _ = persist_locked(&self.inner.path, &data);
        }
        self.flush();
    }

    /// Opens a release-health session for this launch once consent and the
    /// build are both known. The next flush, which callers start, delivers
    /// its opening update ahead of queued reports and retries it like them,
    /// so a launch that starts offline is still counted. It is never written
    /// to disk.
    fn start_session(&self) {
        if self.inner.config.is_none() {
            return;
        }
        {
            let mut data = self
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if data.consent != TelemetryConsent::Enabled
                || data.session.is_some()
                || data.build.is_none()
                || data.install_id.is_none()
            {
                return;
            }
            data.session = Some(SessionRecord {
                id: uuid::Uuid::new_v4().to_string(),
                started_ms: crate::state::now_ms(),
                init: true,
            });
        }
    }

    fn flush(&self) {
        let Some(config) = self.inner.config.clone() else {
            return;
        };
        if self
            .inner
            .flushing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut retry_blocked = false;
            loop {
                let next = {
                    let data = service
                        .inner
                        .data
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    next_delivery(&data)
                };
                let Some(next) = next else {
                    break;
                };
                let mut consent = service.inner.consent_tx.subscribe();
                if !*consent.borrow() {
                    break;
                }
                let envelope = match &next {
                    Delivery::SessionOpen {
                        session,
                        install_id,
                        version,
                    } => session_envelope(session, install_id, version),
                    Delivery::Report { report, install_id } => report_envelope(report, install_id),
                };
                let outcome = tokio::select! {
                    biased;
                    changed = consent.changed() => {
                        let _ = changed;
                        None
                    }
                    outcome = service.inner.transport.send(config.clone(), envelope) => Some(outcome)
                };
                let Some(outcome) = outcome else {
                    break;
                };
                if outcome == SendOutcome::Retry {
                    retry_blocked = true;
                    break;
                }
                let mut data = service
                    .inner
                    .data
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if data.consent != TelemetryConsent::Enabled {
                    break;
                }
                match next {
                    // Sent or rejected, the opening is not sent again. A panic
                    // may have moved the session into a queued crash report
                    // while the opening was in flight; that report must not
                    // open it a second time.
                    Delivery::SessionOpen { session, .. } => {
                        if let Some(current) = data
                            .session
                            .as_mut()
                            .filter(|current| current.id == session.id)
                        {
                            current.init = false;
                        } else if let Some(crashed) = data
                            .queue
                            .iter_mut()
                            .filter_map(|report| report.crashed_session.as_mut())
                            .find(|crashed| crashed.id == session.id)
                        {
                            crashed.init = false;
                        }
                    }
                    Delivery::Report { report, .. } => {
                        data.queue.retain(|queued| queued.id != report.id);
                    }
                }
                let _ = persist_locked(&service.inner.path, &data);
                service.inner.retry_attempt.store(0, Ordering::Release);
            }
            if retry_blocked {
                service.finish_flush(true);
                service.schedule_retry();
            } else if service.finish_flush(false) {
                service.flush();
            }
        });
    }

    /// After a retryable failure, flushes again after a growing delay, so a
    /// launch that starts before the network is up still delivers its session
    /// opening and queued reports. One retry is pending at a time, and opting
    /// out cancels it.
    fn schedule_retry(&self) {
        if self
            .inner
            .retry_scheduled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let attempt = self.inner.retry_attempt.fetch_add(1, Ordering::AcqRel);
        let delays = &self.inner.retry_delays;
        let delay = delays[attempt.min(delays.len() - 1)];
        let service = self.clone();
        let mut consent = self.inner.consent_tx.subscribe();
        tauri::async_runtime::spawn(async move {
            let opted_out = tokio::select! {
                biased;
                _ = consent.wait_for(|enabled| !*enabled) => true,
                _ = tokio::time::sleep(delay) => false,
            };
            service
                .inner
                .retry_scheduled
                .store(false, Ordering::Release);
            if !opted_out {
                service.flush();
            }
        });
    }

    fn finish_flush(&self, retry_blocked: bool) -> bool {
        self.inner.flushing.store(false, Ordering::Release);
        if retry_blocked {
            return false;
        }
        let data = self
            .inner
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        next_delivery(&data).is_some()
    }

    fn purge(&self) {
        let _ = std::fs::remove_file(&self.inner.path);
        let _ = std::fs::remove_file(self.inner.path.with_extension("json.tmp"));
    }
}

/// The next envelope a flush delivers: this launch's session opening first,
/// then queued reports oldest first.
enum Delivery {
    SessionOpen {
        session: SessionRecord,
        install_id: String,
        version: String,
    },
    Report {
        report: QueuedReport,
        install_id: String,
    },
}

fn next_delivery(data: &TelemetryData) -> Option<Delivery> {
    if data.consent != TelemetryConsent::Enabled {
        return None;
    }
    let install_id = data.install_id.clone()?;
    if let (Some(session), Some(build)) = (
        data.session.as_ref().filter(|session| session.init),
        data.build.as_ref(),
    ) {
        return Some(Delivery::SessionOpen {
            session: session.clone(),
            install_id,
            version: build.version.clone(),
        });
    }
    data.queue
        .first()
        .cloned()
        .map(|report| Delivery::Report { report, install_id })
}

/// A fixed description of a panic. The panic's own message is never sent:
/// standard library panics quote the values involved (a string slicing panic
/// quotes the whole string, which may be typed text), and so can an `expect`
/// or `panic!` message built with `format!`.
fn panic_kind(message: &str) -> &'static str {
    // Standard library panic prefixes only, matched at the start so a message
    // quoting typed text cannot choose the label. Specific entries first.
    const KINDS: [(&str, &str); 11] = [
        (
            "called `Option::unwrap()` on a `None` value",
            "unwrap on None",
        ),
        (
            "called `Result::unwrap()` on an `Err` value",
            "unwrap on Err",
        ),
        ("index out of bounds", "index out of bounds"),
        ("byte index", "string index out of range"),
        ("start byte index", "string index out of range"),
        ("end byte index", "string index out of range"),
        ("attempt to divide by zero", "division by zero"),
        (
            "attempt to calculate the remainder with a divisor of zero",
            "division by zero",
        ),
        ("attempt to", "arithmetic overflow"),
        (
            "already mutably borrowed",
            "RefCell already mutably borrowed",
        ),
        ("already borrowed", "RefCell already borrowed"),
    ];
    KINDS
        .iter()
        .find(|(prefix, _)| message.starts_with(prefix))
        .map_or("panic", |(_, kind)| kind)
}

fn create_report(
    data: &TelemetryData,
    level: &str,
    dataset: &str,
    message: &str,
) -> Option<QueuedReport> {
    if data.consent != TelemetryConsent::Enabled {
        return None;
    }
    data.install_id.as_ref()?;
    let build = data.build.clone()?;
    Some(QueuedReport {
        id: uuid::Uuid::new_v4().to_string(),
        level: safe_label(level),
        message: sanitize_detail(message),
        dataset: safe_label(dataset),
        version: build.version,
        tags: vec![safe_label(dataset), build.platform],
        timestamp: crate::state::now_ms().to_string(),
        location: None,
        crashed_session: None,
    })
}

fn push_report(data: &mut TelemetryData, report: QueuedReport) {
    data.queue.push(report);
    let excess = data.queue.len().saturating_sub(MAX_QUEUED_REPORTS);
    if excess > 0 {
        data.queue.drain(..excess);
    }
}

fn environment(version: &str) -> &'static str {
    if version.contains('-') {
        "prerelease"
    } else {
        "production"
    }
}

/// One Sentry envelope holding the report as an event, followed by the
/// crashed session update when the report ended one. Only the fields listed
/// here are sent.
fn report_envelope(report: &QueuedReport, install_id: &str) -> String {
    let event_id = report.id.replace('-', "");
    let timestamp_ms = report.timestamp.parse::<i64>().unwrap_or(0);
    let platform = report
        .tags
        .get(1)
        .cloned()
        .unwrap_or_else(|| "unknown".into());
    let mut event = json!({
        "event_id": event_id,
        "timestamp": timestamp_ms as f64 / 1000.0,
        "platform": "native",
        "level": report.level,
        "logger": report.dataset,
        "release": format!("{RELEASE_PREFIX}{}", report.version),
        "environment": environment(&report.version),
        "user": { "id": install_id },
        "tags": { "os": platform, "dataset": report.dataset },
        "contexts": { "os": { "name": platform } },
        "sdk": { "name": SDK_NAME, "version": env!("CARGO_PKG_VERSION") },
    });
    if report.dataset == "panic" {
        let mut exception = json!({
            "type": "panic",
            "value": report.message,
            "mechanism": { "type": "panic", "handled": false },
        });
        if let Some((file, line)) = report
            .location
            .as_deref()
            .and_then(|location| location.rsplit_once(':'))
        {
            exception["stacktrace"] = json!({
                "frames": [{
                    "filename": file,
                    "lineno": line.parse::<u32>().unwrap_or(0),
                    "in_app": true,
                }],
            });
        }
        event["exception"] = json!({ "values": [exception] });
    } else {
        event["logentry"] = json!({ "formatted": report.message });
    }
    let mut lines = vec![
        json!({ "event_id": event_id, "sent_at": rfc3339(crate::state::now_ms()) }).to_string(),
        json!({ "type": "event" }).to_string(),
        event.to_string(),
    ];
    if let Some(session) = &report.crashed_session {
        lines.push(json!({ "type": "session" }).to_string());
        lines.push(
            session_payload(
                session,
                "crashed",
                timestamp_ms,
                install_id,
                &report.version,
            )
            .to_string(),
        );
    }
    lines.join("\n") + "\n"
}

/// The update that opens a session as healthy.
fn session_envelope(session: &SessionRecord, install_id: &str, version: &str) -> String {
    let now = crate::state::now_ms();
    [
        json!({ "sent_at": rfc3339(now) }).to_string(),
        json!({ "type": "session" }).to_string(),
        session_payload(session, "ok", session.started_ms, install_id, version).to_string(),
    ]
    .join("\n")
        + "\n"
}

/// `at_ms` is when the session reached `status`: its start for the opening
/// update, the crash time for a crashed one delivered on a later launch.
fn session_payload(
    session: &SessionRecord,
    status: &str,
    at_ms: i64,
    install_id: &str,
    version: &str,
) -> serde_json::Value {
    json!({
        "sid": session.id,
        "did": install_id,
        "init": session.init,
        "started": rfc3339(session.started_ms),
        "timestamp": rfc3339(at_ms.max(session.started_ms)),
        "status": status,
        "errors": u32::from(status == "crashed"),
        "attrs": {
            "release": format!("{RELEASE_PREFIX}{version}"),
            "environment": environment(version),
        },
    })
}

/// Formats Unix milliseconds as an RFC 3339 UTC timestamp.
fn rfc3339(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    // Civil date from days since the epoch (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60
    )
}

/// The file name and line of a panic. Directories are dropped so neither a
/// build machine path nor a Cargo registry path under a home folder leaves.
fn safe_location(file: &str, line: u32) -> String {
    let name = file.rsplit(['/', '\\']).next().unwrap_or_default();
    format!("{}:{line}", safe_label(name))
}

fn safe_label(value: &str) -> String {
    let value = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || "._-".contains(*character))
        .take(64)
        .collect::<String>();
    if value.is_empty() {
        "unknown".into()
    } else {
        value
    }
}

#[cfg(test)]
pub(crate) fn sanitize_source(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let normalized = key.to_ascii_lowercase();
                    let forbidden = [
                        "text",
                        "command",
                        "token",
                        "nonce",
                        "signature",
                        "device",
                        "name",
                        "path",
                        "password",
                        "secret",
                        "pairing",
                    ]
                    .iter()
                    .any(|term| normalized.contains(term));
                    (
                        key.clone(),
                        if forbidden {
                            Value::String("[redacted]".into())
                        } else {
                            sanitize_source(value)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().take(20).map(sanitize_source).collect()),
        Value::String(value) => Value::String(sanitize_detail(value)),
        primitive => primitive.clone(),
    }
}

fn load_disk(path: &Path) -> Option<TelemetryDisk> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut disk: TelemetryDisk = serde_json::from_str(&raw).ok()?;
    if uuid::Uuid::parse_str(&disk.install_id).is_err() {
        return None;
    }
    disk.queue
        .retain(|report| uuid::Uuid::parse_str(&report.id).is_ok());
    if disk.queue.len() > MAX_QUEUED_REPORTS {
        disk.queue = disk.queue.split_off(disk.queue.len() - MAX_QUEUED_REPORTS);
    }
    for report in &mut disk.queue {
        report.level = safe_label(&report.level);
        report.message = sanitize_detail(&report.message);
        report.dataset = safe_label(&report.dataset);
        report.version = safe_label(&report.version);
        report.tags = report
            .tags
            .iter()
            .take(8)
            .map(|tag| safe_label(tag))
            .collect();
        report.timestamp = safe_label(&report.timestamp);
        report.location = report.location.as_deref().map(|location| {
            let (file, line) = location.rsplit_once(':').unwrap_or((location, "0"));
            safe_location(file, line.parse().unwrap_or(0))
        });
        report.crashed_session = report
            .crashed_session
            .take()
            .filter(|session| uuid::Uuid::parse_str(&session.id).is_ok());
    }
    Some(disk)
}

fn persist_locked(path: &Path, data: &TelemetryData) -> Result<(), String> {
    if data.consent != TelemetryConsent::Enabled {
        return Ok(());
    }
    let install_id = data
        .install_id
        .as_ref()
        .ok_or_else(|| "Telemetry install identifier is unavailable.".to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| "Telemetry directory is unavailable.".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let disk = TelemetryDisk {
        install_id: install_id.clone(),
        queue: data.queue.clone(),
    };
    let temp = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(&disk).map_err(|error| error.to_string())? + "\n";
    std::fs::write(&temp, contents).map_err(|error| error.to_string())?;
    replace_file(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    const TEST_DSN: &str = "https://abc123@o1.ingest.de.sentry.io/4500000000000001";

    struct FakeTransport {
        outcomes: Mutex<VecDeque<SendOutcome>>,
        sent: Mutex<Vec<String>>,
    }

    impl FakeTransport {
        fn new(outcomes: impl IntoIterator<Item = SendOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                sent: Mutex::new(Vec::new()),
            }
        }

        fn sent(&self) -> Vec<String> {
            self.sent.lock().unwrap().clone()
        }
    }

    impl TelemetryTransport for FakeTransport {
        fn send(&self, _config: TelemetryConfig, envelope: String) -> SendFuture {
            self.sent.lock().unwrap().push(envelope);
            let outcome = self
                .outcomes
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(SendOutcome::Sent);
            Box::pin(async move { outcome })
        }
    }

    struct BlockingTransport {
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        delivered: Arc<AtomicUsize>,
    }

    impl TelemetryTransport for BlockingTransport {
        fn send(&self, _config: TelemetryConfig, _envelope: String) -> SendFuture {
            let started = self.started.clone();
            let release = self.release.clone();
            let delivered = self.delivered.clone();
            Box::pin(async move {
                started.notify_one();
                release.notified().await;
                delivered.fetch_add(1, Ordering::SeqCst);
                SendOutcome::Sent
            })
        }
    }

    fn temp_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("switchify-telemetry-{}", uuid::Uuid::new_v4()))
            .join("telemetry.json")
    }

    fn test_service(
        consent: TelemetryConsent,
        transport: Arc<dyn TelemetryTransport>,
    ) -> (TelemetryService, PathBuf) {
        let path = temp_path();
        (
            TelemetryService::with_transport(
                path.clone(),
                consent,
                TelemetryConfig::from_dsn(TEST_DSN),
                transport,
            ),
            path,
        )
    }

    /// Parses an envelope into its JSON lines.
    fn lines(envelope: &str) -> Vec<Value> {
        envelope
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    async fn settle() {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }

    /// Polls `condition` until it holds. The flush work runs on Tauri's own
    /// async runtime, not the test runtime, so paused tokio time cannot drive
    /// it; a generous wall-clock budget keeps loaded release runners from
    /// failing a test that passes everywhere else.
    async fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while !condition() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .is_ok()
    }

    #[tokio::test]
    async fn nothing_is_stored_or_sent_before_opt_in() {
        let transport = Arc::new(FakeTransport::new([]));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport.clone());
        service.start("1.0.0", "macos");
        service.report_exception("failure", "1.0.0", "macos");
        service.record_panic("panic", Some(("src/lib.rs", 1)));
        settle().await;
        assert!(!path.exists());
        assert!(transport.sent().is_empty());
    }

    #[tokio::test]
    async fn opting_in_partway_through_a_launch_opens_one_session() {
        let transport = Arc::new(FakeTransport::new([]));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport.clone());
        service.start("1.6.0", "windows");
        service.set_consent(TelemetryConsent::Enabled);
        service.set_consent(TelemetryConsent::Enabled);
        assert!(wait_until(|| transport.sent().len() == 1).await);
        settle().await;
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        let envelope = lines(&sent[0]);
        assert_eq!(envelope[1]["type"], "session");
        assert_eq!(envelope[2]["status"], "ok");
        assert_eq!(envelope[2]["init"], true);
        assert_eq!(envelope[2]["attrs"]["release"], "switchify-pc@1.6.0");
        assert_eq!(envelope[2]["attrs"]["environment"], "production");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn failed_reports_queue_flush_and_keep_a_stable_opaque_id() {
        let transport = Arc::new(FakeTransport::new([
            SendOutcome::Retry,
            SendOutcome::Sent,
            SendOutcome::Sent,
        ]));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport.clone());
        service.set_consent(TelemetryConsent::Enabled);
        service.report_exception("first failure", "1.0.0", "macos");
        assert!(
            wait_until(|| {
                !service.inner.flushing.load(Ordering::Acquire)
                    && transport.sent().len() == 1
                    && load_disk(&path).is_some_and(|disk| disk.queue.len() == 1)
            })
            .await,
            "the retrying flush should finish"
        );
        let first_disk = load_disk(&path).unwrap();
        assert_eq!(first_disk.queue.len(), 1);
        service.report_exception("second failure", "1.0.0", "macos");
        let flushed = wait_until(|| {
            let queue_empty = service
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .queue
                .is_empty();
            queue_empty
                && !service.inner.flushing.load(Ordering::Acquire)
                && transport.sent().len() >= 3
        })
        .await;
        assert!(
            flushed,
            "the successful flush should persist an empty queue (sent={}, flushing={}, queued={:?})",
            transport.sent().len(),
            service.inner.flushing.load(Ordering::Acquire),
            load_disk(&path).map(|disk| disk.queue.len())
        );
        let second_disk = load_disk(&path).unwrap();
        assert_eq!(second_disk.install_id, first_disk.install_id);
        assert!(second_disk.queue.is_empty());
        for envelope in transport.sent() {
            assert_eq!(lines(&envelope)[2]["user"]["id"], first_disk.install_id);
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn opt_out_purges_and_prevents_reporting() {
        let transport = Arc::new(FakeTransport::new([SendOutcome::Retry]));
        let (service, path) = test_service(TelemetryConsent::Enabled, transport.clone());
        service.set_consent(TelemetryConsent::Enabled);
        service.report_exception("failure", "1.0.0", "windows");
        settle().await;
        assert!(path.exists());
        service.set_consent(TelemetryConsent::Disabled);
        assert!(!path.exists());
        let sent_before = transport.sent().len();
        service.report_exception("another failure", "1.0.0", "windows");
        service.record_panic("panic", None);
        settle().await;
        assert_eq!(transport.sent().len(), sent_before);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn opt_out_cancels_a_prepared_session_request() {
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let delivered = Arc::new(AtomicUsize::new(0));
        let transport = Arc::new(BlockingTransport {
            started: started.clone(),
            release: release.clone(),
            delivered: delivered.clone(),
        });
        let (service, path) = test_service(TelemetryConsent::Enabled, transport);
        let started_signal = started.notified();
        service.start("1.0.0", "macos");
        started_signal.await;
        service.set_consent(TelemetryConsent::Disabled);
        release.notify_waiters();
        settle().await;
        assert_eq!(delivered.load(Ordering::SeqCst), 0);
        assert!(!path.exists());
    }

    #[test]
    fn flush_shutdown_detects_reports_queued_during_handoff() {
        let transport = Arc::new(FakeTransport::new([SendOutcome::Retry]));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport);
        service.set_consent(TelemetryConsent::Enabled);
        service.inner.flushing.store(true, Ordering::Release);
        service.report_exception("handoff failure", "1.0.0", "macos");

        assert!(service.finish_flush(false));
        assert!(!service.inner.flushing.load(Ordering::Acquire));
        assert_eq!(load_disk(&path).unwrap().queue.len(), 1);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn queue_is_bounded_and_session_failures_are_not_queued() {
        // One Retry per exception flush plus one for the session, so the fake
        // can never run dry and start answering Sent mid-test.
        let transport = Arc::new(FakeTransport::new(std::iter::repeat_n(
            SendOutcome::Retry,
            MAX_QUEUED_REPORTS + 3,
        )));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport);
        service.set_consent(TelemetryConsent::Enabled);
        for index in 0..(MAX_QUEUED_REPORTS + 2) {
            service.report_exception(&format!("failure {index}"), "1.0.0", "macos");
        }
        service.start("1.0.0", "macos");
        settle().await;
        let disk = load_disk(&path).unwrap();
        assert_eq!(disk.queue.len(), MAX_QUEUED_REPORTS);
        assert_eq!(disk.queue.first().unwrap().message, "failure 2");
        assert!(disk.queue.iter().all(|report| report.dataset == "runtime"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn a_panic_is_persisted_and_sent_with_the_crashed_session() {
        // The session opening fails, so the crashed update must open it.
        let transport = Arc::new(FakeTransport::new([SendOutcome::Retry]));
        let (service, path) = test_service(TelemetryConsent::Enabled, transport.clone());
        service.start("1.6.0-rc.1", "windows");
        assert!(
            wait_until(
                || transport.sent().len() == 1 && !service.inner.flushing.load(Ordering::Acquire)
            )
            .await
        );
        service.record_panic(
            panic_kind("index out of bounds: the len is 3 but the index is 5"),
            Some((r"C:\Users\Owen\.cargo\registry\src\scan_menu.rs", 42)),
        );
        let disk = load_disk(&path).unwrap();
        let report = disk.queue.first().unwrap();
        assert_eq!(report.level, "fatal");
        assert_eq!(report.message, "index out of bounds");
        assert_eq!(report.location.as_deref(), Some("scan_menu.rs:42"));
        assert!(report.crashed_session.as_ref().unwrap().init);

        // The next launch delivers it.
        let next = Arc::new(FakeTransport::new([]));
        let relaunched = TelemetryService::with_transport(
            path.clone(),
            TelemetryConsent::Enabled,
            TelemetryConfig::from_dsn(TEST_DSN),
            next.clone(),
        );
        relaunched.start("1.6.0-rc.1", "windows");
        assert!(wait_until(|| next.sent().len() == 2).await);
        let envelope = next
            .sent()
            .into_iter()
            .find(|envelope| envelope.contains("\"exception\""))
            .unwrap();
        assert!(!envelope.contains("Owen"));
        assert!(!envelope.contains("Users"));
        let parsed = lines(&envelope);
        assert_eq!(parsed[1]["type"], "event");
        let exception = &parsed[2]["exception"]["values"][0];
        assert_eq!(exception["value"], "index out of bounds");
        assert_eq!(exception["mechanism"]["handled"], false);
        assert_eq!(
            exception["stacktrace"]["frames"][0]["filename"],
            "scan_menu.rs"
        );
        assert_eq!(parsed[2]["environment"], "prerelease");
        assert_eq!(parsed[3]["type"], "session");
        assert_eq!(parsed[4]["status"], "crashed");
        assert_eq!(parsed[4]["init"], true);
        assert_eq!(
            parsed[4]["timestamp"],
            rfc3339(report.timestamp.parse().unwrap())
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn a_session_opening_that_fails_is_retried_before_reports() {
        let transport = Arc::new(FakeTransport::new([SendOutcome::Retry]));
        let (service, path) = test_service(TelemetryConsent::Enabled, transport.clone());
        service.start("1.6.0", "macos");
        assert!(
            wait_until(
                || transport.sent().len() == 1 && !service.inner.flushing.load(Ordering::Acquire)
            )
            .await
        );
        service.report_exception("Bluetooth stopped", "1.6.0", "macos");
        assert!(wait_until(|| transport.sent().len() == 3).await);
        let sent = transport.sent();
        let (first, retried) = (lines(&sent[0]), lines(&sent[1]));
        assert_eq!(retried[1]["type"], "session");
        assert_eq!(retried[2]["status"], "ok");
        assert_eq!(retried[2]["init"], true);
        assert_eq!(retried[2]["sid"], first[2]["sid"]);
        assert_eq!(lines(&sent[2])[1]["type"], "event");

        // Once the opening is delivered, a crash no longer reopens the session.
        assert!(wait_until(|| !service.inner.flushing.load(Ordering::Acquire)).await);
        service.record_panic("panic", None);
        let disk = load_disk(&path).unwrap();
        assert!(!disk.queue[0].crashed_session.as_ref().unwrap().init);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn panic_messages_are_reduced_to_a_fixed_kind() {
        let typed = std::panic::catch_unwind(|| {
            let text = String::from("my private messagé");
            let _ = &text[..18];
        })
        .unwrap_err();
        let message = typed
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or_default();
        // Older toolchains quote the whole string here; either way only the
        // kind is kept.
        assert!(message.contains("byte index"), "{message}");
        assert_eq!(panic_kind(message), "string index out of range");
        assert_eq!(
            panic_kind("byte index 18 is not a char boundary; it is inside 'é' (bytes 17..19) of `my private messagé`"),
            "string index out of range"
        );
        assert_eq!(
            panic_kind("called `Result::unwrap()` on an `Err` value: \"secret words\""),
            "unwrap on Err"
        );
        assert_eq!(panic_kind("failed to open my private file"), "panic");
        assert_eq!(panic_kind("word index out of bounds"), "panic");
        assert_eq!(panic_kind("attempt to divide by zero"), "division by zero");
        assert_eq!(
            panic_kind("attempt to add with overflow"),
            "arithmetic overflow"
        );
        assert_eq!(panic_kind(""), "panic");
    }

    #[tokio::test]
    async fn an_offline_launch_retries_its_session_opening_by_itself() {
        let transport = Arc::new(FakeTransport::new([SendOutcome::Retry, SendOutcome::Retry]));
        let path = temp_path();
        let service = TelemetryService::with_retry_delays(
            path.clone(),
            TelemetryConsent::Enabled,
            TelemetryConfig::from_dsn(TEST_DSN),
            transport.clone(),
            vec![Duration::from_millis(10)],
        );
        service.start("1.6.0", "windows");
        // Two failures, then the retry timer delivers it with nothing else
        // starting a flush.
        assert!(wait_until(|| transport.sent().len() == 3).await);
        let sent = transport.sent();
        assert!(sent
            .iter()
            .all(|envelope| lines(envelope)[2]["status"] == "ok"));
        assert!(
            wait_until(|| {
                service
                    .inner
                    .data
                    .lock()
                    .unwrap()
                    .session
                    .as_ref()
                    .is_some_and(|session| !session.init)
            })
            .await
        );
        settle().await;
        assert_eq!(transport.sent().len(), 3);
        assert_eq!(service.inner.retry_attempt.load(Ordering::Acquire), 0);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn opting_out_cancels_a_pending_retry() {
        let transport = Arc::new(FakeTransport::new([SendOutcome::Retry]));
        let path = temp_path();
        let service = TelemetryService::with_retry_delays(
            path.clone(),
            TelemetryConsent::Enabled,
            TelemetryConfig::from_dsn(TEST_DSN),
            transport.clone(),
            vec![Duration::from_millis(200)],
        );
        service.start("1.6.0", "windows");
        assert!(wait_until(|| service.inner.retry_scheduled.load(Ordering::Acquire)).await);
        service.set_consent(TelemetryConsent::Disabled);
        assert!(wait_until(|| !service.inner.retry_scheduled.load(Ordering::Acquire)).await);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(transport.sent().len(), 1);
        assert!(!path.exists());
    }

    #[test]
    fn envelopes_carry_only_the_allowlisted_fields() {
        let report = QueuedReport {
            id: uuid::Uuid::new_v4().to_string(),
            level: "error".into(),
            message: "Bluetooth stopped".into(),
            dataset: "runtime".into(),
            version: "1.6.0".into(),
            tags: vec!["runtime".into(), "macos".into()],
            timestamp: "1700000000123".into(),
            location: None,
            crashed_session: None,
        };
        let parsed = lines(&report_envelope(&report, "install-id"));
        assert_eq!(parsed.len(), 3);
        let event = parsed[2].as_object().unwrap();
        let mut keys = event.keys().map(String::as_str).collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "contexts",
                "environment",
                "event_id",
                "level",
                "logentry",
                "logger",
                "platform",
                "release",
                "sdk",
                "tags",
                "timestamp",
                "user"
            ]
        );
        assert_eq!(event["user"], json!({ "id": "install-id" }));
        assert_eq!(event["contexts"], json!({ "os": { "name": "macos" } }));
        assert_eq!(event["logentry"]["formatted"], "Bluetooth stopped");
        assert_eq!(event["timestamp"], 1_700_000_000.123);
        assert_eq!(parsed[0]["event_id"], event["event_id"]);
    }

    #[test]
    fn a_timberlogs_queue_from_an_earlier_build_still_loads() {
        let path = temp_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let install_id = uuid::Uuid::new_v4().to_string();
        let old = json!({
            "installId": install_id,
            "queue": [{
                "id": uuid::Uuid::new_v4().to_string(),
                "level": "error",
                "message": "Bluetooth stopped",
                "source": "switchify-pc",
                "environment": "production",
                "dataset": "runtime",
                "version": "1.5.2",
                "tags": ["runtime", "windows"],
                "userId": install_id,
                "timestamp": "1700000000000",
            }],
        });
        std::fs::write(&path, old.to_string()).unwrap();
        let disk = load_disk(&path).unwrap();
        assert_eq!(disk.install_id, install_id);
        assert_eq!(disk.queue.len(), 1);
        assert_eq!(disk.queue[0].message, "Bluetooth stopped");
        assert!(disk.queue[0].crashed_session.is_none());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn sanitizer_redacts_secrets_paths_names_commands_and_typed_content() {
        let source = serde_json::json!({
            "token": "secret",
            "deviceName": "Owen's phone",
            "command": "keyboard.typeText",
            "typedText": "private words",
            "error": "failed /Users/Owen/private token=value",
        });
        let serialized = sanitize_source(&source).to_string();
        for forbidden in [
            "secret",
            "Owen",
            "keyboard.typeText",
            "private words",
            "/Users",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert!(serialized.contains("[redacted]"));
    }

    #[test]
    fn dsn_parsing_requires_https_a_key_and_a_numeric_project() {
        assert_eq!(
            TelemetryConfig::from_dsn(TEST_DSN),
            Some(TelemetryConfig {
                envelope_url: "https://o1.ingest.de.sentry.io/api/4500000000000001/envelope/"
                    .into(),
                public_key: "abc123".into(),
            })
        );
        assert_eq!(
            TelemetryConfig::from_dsn("https://key:secret@sentry.example.test/base/7")
                .unwrap()
                .envelope_url,
            "https://sentry.example.test/base/api/7/envelope/"
        );
        assert_eq!(
            TelemetryConfig::from_dsn("https://key:secret@sentry.example.test/7")
                .unwrap()
                .public_key,
            "key"
        );
        for invalid in [
            "",
            "http://key@sentry.example.test/7",
            "https://sentry.example.test/7",
            "https://@sentry.example.test/7",
            "https://key@sentry.example.test/",
            "https://key@sentry.example.test/project",
            "https://key@sentry.example.test?x=/7",
        ] {
            assert!(TelemetryConfig::from_dsn(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn http_transport_uses_the_sentry_auth_header_and_envelope_body() {
        let transport = HttpTransport::default();
        let config = TelemetryConfig::from_dsn(TEST_DSN).unwrap();
        let request = transport
            .build_request(config, "{}\n{\"type\":\"session\"}\n{}\n".into())
            .unwrap();
        assert_eq!(
            request.url().as_str(),
            "https://o1.ingest.de.sentry.io/api/4500000000000001/envelope/"
        );
        let auth = request.headers()["X-Sentry-Auth"].to_str().unwrap();
        assert!(auth.starts_with("Sentry sentry_version=7, sentry_key=abc123,"));
        assert_eq!(
            request.headers()[reqwest::header::CONTENT_TYPE],
            "application/x-sentry-envelope"
        );
    }

    #[test]
    fn timestamps_are_rfc3339_utc() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn panic_locations_keep_only_the_file_name() {
        assert_eq!(safe_location("src/scan_menu.rs", 12), "scan_menu.rs:12");
        assert_eq!(
            safe_location("C:\\Users\\Someone\\a b\\lib.rs", 3),
            "lib.rs:3"
        );
        assert_eq!(safe_location("", 3), "unknown:3");
    }
}
