use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
#[cfg(test)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;

use crate::diagnostics::sanitize_detail;
use crate::state::{ActivityKind, AppState};
use crate::storage::replace_file;

const MAX_QUEUED_REPORTS: usize = 20;
const SOURCE: &str = "switchify-pc";

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

#[derive(Clone)]
struct TelemetryConfig {
    endpoint: String,
    api_key: String,
}

impl TelemetryConfig {
    fn from_build() -> Option<Self> {
        Self::new(
            option_env!("SWITCHIFY_TELEMETRY_ENDPOINT").unwrap_or_default(),
            option_env!("TIMBERLOGS_API_KEY").unwrap_or_default(),
        )
    }

    fn new(endpoint: impl Into<String>, api_key: impl Into<String>) -> Option<Self> {
        let endpoint = endpoint.into().trim().to_owned();
        let api_key = api_key.into().trim().to_owned();
        (endpoint.starts_with("https://") && !api_key.is_empty())
            .then_some(Self { endpoint, api_key })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TelemetryLog {
    id: String,
    level: String,
    message: String,
    source: String,
    environment: String,
    dataset: String,
    version: String,
    tags: Vec<String>,
    user_id: String,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryDisk {
    install_id: String,
    #[serde(default)]
    queue: Vec<TelemetryLog>,
}

struct TelemetryData {
    consent: TelemetryConsent,
    install_id: Option<String>,
    queue: Vec<TelemetryLog>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendOutcome {
    Sent,
    Retry,
    Drop,
}

type SendFuture = Pin<Box<dyn Future<Output = SendOutcome> + Send>>;

trait TelemetryTransport: Send + Sync {
    fn send(&self, config: TelemetryConfig, logs: Vec<TelemetryLog>) -> SendFuture;
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
        logs: Vec<TelemetryLog>,
    ) -> Result<reqwest::Request, reqwest::Error> {
        self.client
            .post(config.endpoint)
            .bearer_auth(config.api_key)
            .json(&serde_json::json!({ "logs": logs }))
            .build()
    }
}

impl TelemetryTransport for HttpTransport {
    fn send(&self, config: TelemetryConfig, logs: Vec<TelemetryLog>) -> SendFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let request = HttpTransport {
                client: client.clone(),
            }
            .build_request(config, logs);
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
    #[cfg(test)]
    flush_requested: AtomicU64,
    #[cfg(test)]
    flush_completed_tx: tokio::sync::watch::Sender<u64>,
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
        #[cfg(test)]
        let (flush_completed_tx, _) = tokio::sync::watch::channel(0);
        let service = Self {
            inner: Arc::new(TelemetryInner {
                path,
                config,
                data: Mutex::new(TelemetryData {
                    consent,
                    install_id,
                    queue: disk.map_or_else(Vec::new, |disk| disk.queue),
                    last_error: None,
                }),
                flushing: AtomicBool::new(false),
                #[cfg(test)]
                flush_requested: AtomicU64::new(0),
                #[cfg(test)]
                flush_completed_tx,
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
            }
            let _ = persist_locked(&self.inner.path, &data);
        }
        if consent != TelemetryConsent::Enabled {
            self.purge();
        } else {
            self.inner.consent_tx.send_replace(true);
            self.flush();
        }
    }

    pub fn start(&self, version: &str, platform: &str) {
        self.report_health("app.startup.completed", version, platform);
        self.flush();
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
        let Some(log) = self.create_log("error", "runtime", message, version, platform) else {
            return;
        };
        {
            let mut data = self
                .inner
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            data.queue.push(log);
            let excess = data.queue.len().saturating_sub(MAX_QUEUED_REPORTS);
            if excess > 0 {
                data.queue.drain(..excess);
            }
            let _ = persist_locked(&self.inner.path, &data);
        }
        #[cfg(test)]
        self.inner.flush_requested.fetch_add(1, Ordering::AcqRel);
        self.flush();
    }

    pub fn report_health(&self, name: &str, version: &str, platform: &str) {
        let Some(config) = self.inner.config.clone() else {
            return;
        };
        let Some(log) = self.create_log("info", "health", name, version, platform) else {
            return;
        };
        let transport = self.inner.transport.clone();
        let mut consent = self.inner.consent_tx.subscribe();
        tauri::async_runtime::spawn(async move {
            if !*consent.borrow() {
                return;
            }
            tokio::select! {
                biased;
                changed = consent.changed() => {
                    let _ = changed;
                }
                _ = transport.send(config, vec![log]) => {}
            }
        });
    }

    fn create_log(
        &self,
        level: &str,
        dataset: &str,
        message: &str,
        version: &str,
        platform: &str,
    ) -> Option<TelemetryLog> {
        let data = self
            .inner
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if data.consent != TelemetryConsent::Enabled || self.inner.config.is_none() {
            return None;
        }
        let install_id = data.install_id.clone()?;
        Some(TelemetryLog {
            id: uuid::Uuid::new_v4().to_string(),
            level: safe_label(level),
            message: sanitize_detail(message),
            source: SOURCE.into(),
            environment: "production".into(),
            dataset: safe_label(dataset),
            version: safe_label(version),
            tags: vec![safe_label(dataset), safe_label(platform)],
            user_id: install_id,
            timestamp: crate::state::now_ms().to_string(),
        })
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
                    if data.consent != TelemetryConsent::Enabled {
                        None
                    } else {
                        data.queue.first().cloned()
                    }
                };
                let Some(next) = next else {
                    break;
                };
                let mut consent = service.inner.consent_tx.subscribe();
                if !*consent.borrow() {
                    break;
                }
                let outcome = tokio::select! {
                    biased;
                    changed = consent.changed() => {
                        let _ = changed;
                        None
                    }
                    outcome = service.inner.transport.send(config.clone(), vec![next.clone()]) => Some(outcome)
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
                data.queue.retain(|queued| queued.id != next.id);
                let _ = persist_locked(&service.inner.path, &data);
            }
            if service.finish_flush(retry_blocked) {
                service.flush();
            } else {
                #[cfg(test)]
                {
                    let completed = service.inner.flush_requested.load(Ordering::Acquire);
                    service.inner.flush_completed_tx.send_replace(completed);
                }
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
        data.consent == TelemetryConsent::Enabled && !data.queue.is_empty()
    }

    fn purge(&self) {
        let _ = std::fs::remove_file(&self.inner.path);
        let _ = std::fs::remove_file(self.inner.path.with_extension("json.tmp"));
    }
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
    if disk.queue.len() > MAX_QUEUED_REPORTS {
        disk.queue = disk.queue.split_off(disk.queue.len() - MAX_QUEUED_REPORTS);
    }
    for log in &mut disk.queue {
        log.level = safe_label(&log.level);
        log.message = sanitize_detail(&log.message);
        log.source = SOURCE.into();
        log.environment = "production".into();
        log.dataset = safe_label(&log.dataset);
        log.version = safe_label(&log.version);
        log.tags = log.tags.iter().take(8).map(|tag| safe_label(tag)).collect();
        log.user_id = disk.install_id.clone();
        log.timestamp = safe_label(&log.timestamp);
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
    use std::sync::atomic::AtomicUsize;

    struct FakeTransport {
        outcomes: Mutex<VecDeque<SendOutcome>>,
        sent: Mutex<Vec<TelemetryLog>>,
    }

    impl FakeTransport {
        fn new(outcomes: impl IntoIterator<Item = SendOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                sent: Mutex::new(Vec::new()),
            }
        }
    }

    impl TelemetryTransport for FakeTransport {
        fn send(&self, _config: TelemetryConfig, logs: Vec<TelemetryLog>) -> SendFuture {
            self.sent.lock().unwrap().extend(logs);
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
        fn send(&self, _config: TelemetryConfig, _logs: Vec<TelemetryLog>) -> SendFuture {
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

    fn test_service(
        consent: TelemetryConsent,
        transport: Arc<FakeTransport>,
    ) -> (TelemetryService, PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("switchify-telemetry-{}", uuid::Uuid::new_v4()))
            .join("telemetry.json");
        (
            TelemetryService::with_transport(
                path.clone(),
                consent,
                TelemetryConfig::new("https://telemetry.example.test", "test-key"),
                transport,
            ),
            path,
        )
    }

    async fn settle() {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }

    async fn wait_for_flush(service: &TelemetryService) {
        let requested = service.inner.flush_requested.load(Ordering::Acquire);
        let mut completed = service.inner.flush_completed_tx.subscribe();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while *completed.borrow() < requested {
                completed
                    .changed()
                    .await
                    .expect("flush completion sender should remain available");
            }
        })
        .await
        .expect("telemetry flush should finish before the test timeout");
    }

    #[tokio::test]
    async fn nothing_is_stored_or_sent_before_opt_in() {
        let transport = Arc::new(FakeTransport::new([]));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport.clone());
        service.report_exception("failure", "1.0.0", "macos");
        settle().await;
        assert!(!path.exists());
        assert!(transport.sent.lock().unwrap().is_empty());
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
        wait_for_flush(&service).await;
        let first_disk = load_disk(&path).unwrap();
        assert_eq!(first_disk.queue.len(), 1);
        service.report_exception("second failure", "1.0.0", "macos");
        wait_for_flush(&service).await;
        let second_disk = load_disk(&path).unwrap();
        assert_eq!(second_disk.install_id, first_disk.install_id);
        assert!(second_disk.queue.is_empty());
        assert!(transport.sent.lock().unwrap().len() >= 3);
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
        let sent_before = transport.sent.lock().unwrap().len();
        service.report_exception("another failure", "1.0.0", "windows");
        settle().await;
        assert_eq!(transport.sent.lock().unwrap().len(), sent_before);
    }

    #[tokio::test]
    async fn opt_out_cancels_a_prepared_health_request() {
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let delivered = Arc::new(AtomicUsize::new(0));
        let transport = Arc::new(BlockingTransport {
            started: started.clone(),
            release: release.clone(),
            delivered: delivered.clone(),
        });
        let path = std::env::temp_dir()
            .join(format!("switchify-telemetry-{}", uuid::Uuid::new_v4()))
            .join("telemetry.json");
        let service = TelemetryService::with_transport(
            path.clone(),
            TelemetryConsent::Enabled,
            TelemetryConfig::new("https://telemetry.example.test", "test-key"),
            transport,
        );
        let started_signal = started.notified();
        service.report_health("app.healthy", "1.0.0", "macos");
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
    async fn queue_is_bounded_and_health_failures_are_not_queued() {
        let transport = Arc::new(FakeTransport::new(std::iter::repeat_n(
            SendOutcome::Retry,
            MAX_QUEUED_REPORTS + 2,
        )));
        let (service, path) = test_service(TelemetryConsent::Undecided, transport);
        service.set_consent(TelemetryConsent::Enabled);
        for index in 0..(MAX_QUEUED_REPORTS + 2) {
            service.report_exception(&format!("failure {index}"), "1.0.0", "macos");
        }
        service.report_health("app.healthy", "1.0.0", "macos");
        settle().await;
        let disk = load_disk(&path).unwrap();
        assert_eq!(disk.queue.len(), MAX_QUEUED_REPORTS);
        assert_eq!(disk.queue.first().unwrap().message, "failure 2");
        assert!(disk.queue.iter().all(|log| log.dataset == "runtime"));
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
    fn configuration_requires_https_endpoint_and_a_key() {
        assert!(TelemetryConfig::new("", "key").is_none());
        assert!(TelemetryConfig::new("http://example.test", "key").is_none());
        assert!(TelemetryConfig::new("https://example.test", "").is_none());
        assert!(TelemetryConfig::new("https://example.test", "key").is_some());
    }

    #[test]
    fn http_transport_uses_bearer_auth_and_the_logs_envelope() {
        let transport = HttpTransport::default();
        let config = TelemetryConfig::new("https://example.test/logs", "test-key").unwrap();
        let log = TelemetryLog {
            id: "report-id".into(),
            level: "info".into(),
            message: "healthy".into(),
            source: SOURCE.into(),
            environment: "production".into(),
            dataset: "health".into(),
            version: "1.0.0".into(),
            tags: vec!["health".into()],
            user_id: uuid::Uuid::new_v4().to_string(),
            timestamp: "1".into(),
        };
        let request = transport.build_request(config, vec![log]).unwrap();
        assert_eq!(
            request.headers()[reqwest::header::AUTHORIZATION],
            "Bearer test-key"
        );
        let body = request.body().unwrap().as_bytes().unwrap();
        assert!(std::str::from_utf8(body).unwrap().contains("\"logs\""));
    }
}
