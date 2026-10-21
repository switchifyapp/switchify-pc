use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri_plugin_updater::Update;
use tokio::sync::watch;

type DownloadFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send + 'a>>;

pub trait UpdateArtifact: Clone + Send + Sync + 'static {
    fn version(&self) -> &str;
    fn download<'a>(
        &'a self,
        on_chunk: Box<dyn FnMut(usize, Option<u64>) + Send + 'a>,
    ) -> DownloadFuture<'a>;
    fn install(&self, bytes: &[u8]) -> Result<(), String>;
}

impl UpdateArtifact for Update {
    fn version(&self) -> &str {
        &self.version
    }

    fn download<'a>(
        &'a self,
        on_chunk: Box<dyn FnMut(usize, Option<u64>) + Send + 'a>,
    ) -> DownloadFuture<'a> {
        Box::pin(async move {
            Update::download(self, on_chunk, || {})
                .await
                .map_err(|error| error.to_string())
        })
    }

    fn install(&self, bytes: &[u8]) -> Result<(), String> {
        Update::install(self, bytes).map_err(|error| error.to_string())
    }
}

/// Whether a build may install the version a feed offers. A stable build never
/// installs a pre-release, whatever the feed holds. Otherwise the offered
/// version must be newer than the running one, which is the updater's own rule.
pub fn accepts_update(current: &semver::Version, offered: &semver::Version) -> bool {
    if current.pre.is_empty() && !offered.pre.is_empty() {
        return false;
    }
    offered > current
}

#[derive(Debug, PartialEq, Eq)]
pub enum DownloadResult {
    Complete(Result<Vec<u8>, String>),
    Cancelled,
}

pub async fn download_with_cancel<U, F>(
    update: &U,
    mut cancel_receiver: watch::Receiver<bool>,
    on_chunk: F,
) -> DownloadResult
where
    U: UpdateArtifact,
    F: FnMut(usize, Option<u64>) + Send,
{
    let mut download = update.download(Box::new(on_chunk));
    tokio::select! {
        result = &mut download => DownloadResult::Complete(result),
        changed = cancel_receiver.changed() => {
            if changed.is_ok() && *cancel_receiver.borrow() {
                DownloadResult::Cancelled
            } else {
                DownloadResult::Complete(download.await)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UpdateStatus {
    Unconfigured,
    Idle,
    Checking,
    Available,
    Downloading,
    ReadyToInstall,
    Applying,
    Current,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RetryAction {
    Check,
    Download,
    Install,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub status: UpdateStatus,
    pub version: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub retry_action: Option<RetryAction>,
}

impl Default for UpdateView {
    fn default() -> Self {
        Self::unconfigured()
    }
}

impl UpdateView {
    pub fn unconfigured() -> Self {
        Self::new(UpdateStatus::Unconfigured)
    }

    pub fn idle() -> Self {
        Self::new(UpdateStatus::Idle)
    }

    pub fn checking() -> Self {
        Self::new(UpdateStatus::Checking)
    }

    pub fn current() -> Self {
        Self::new(UpdateStatus::Current)
    }

    pub fn available(version: String) -> Self {
        Self {
            status: UpdateStatus::Available,
            version: Some(version),
            ..Self::new(UpdateStatus::Available)
        }
    }

    pub fn downloading(version: String) -> Self {
        Self {
            status: UpdateStatus::Downloading,
            version: Some(version),
            ..Self::new(UpdateStatus::Downloading)
        }
    }

    pub fn ready(version: String, downloaded_bytes: u64, total_bytes: Option<u64>) -> Self {
        Self {
            status: UpdateStatus::ReadyToInstall,
            version: Some(version),
            downloaded_bytes,
            total_bytes,
            error: None,
            retry_action: None,
        }
    }

    pub fn applying(version: String, downloaded_bytes: u64, total_bytes: Option<u64>) -> Self {
        Self {
            status: UpdateStatus::Applying,
            version: Some(version),
            downloaded_bytes,
            total_bytes,
            error: None,
            retry_action: None,
        }
    }

    pub fn failed(version: Option<String>, error: String, retry_action: RetryAction) -> Self {
        Self {
            status: UpdateStatus::Failed,
            version,
            downloaded_bytes: 0,
            total_bytes: None,
            error: Some(error),
            retry_action: Some(retry_action),
        }
    }

    pub fn cancelled(version: String) -> Self {
        Self {
            status: UpdateStatus::Cancelled,
            version: Some(version),
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            retry_action: Some(RetryAction::Download),
        }
    }

    fn new(status: UpdateStatus) -> Self {
        Self {
            status,
            version: None,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            retry_action: None,
        }
    }
}

/// Tauri event carrying download progress. Progress travels on this small
/// event instead of the full `AppState` so a large download cannot flood
/// state emission, the tray and the webview.
pub const PROGRESS_EVENT: &str = "update-progress";

/// Minimum spacing between progress events.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

impl UpdateProgress {
    fn percent(&self) -> Option<u64> {
        self.total_bytes
            .filter(|total| *total > 0)
            .map(|total| (self.downloaded_bytes.min(total) * 100) / total)
    }

    fn complete(&self) -> bool {
        self.total_bytes
            .is_some_and(|total| total > 0 && self.downloaded_bytes >= total)
    }
}

/// Byte counter shared with the download callback without touching the
/// application state mutex. A total of zero means the size is unknown.
#[derive(Debug, Default)]
pub struct ProgressCounter {
    downloaded: AtomicU64,
    total: AtomicU64,
}

impl ProgressCounter {
    pub fn add(&self, chunk: usize, total: Option<u64>) -> UpdateProgress {
        if let Some(total) = total {
            self.total.store(total, Ordering::Relaxed);
        }
        self.downloaded.fetch_add(chunk as u64, Ordering::Relaxed);
        self.current()
    }

    pub fn current(&self) -> UpdateProgress {
        let total = self.total.load(Ordering::Relaxed);
        UpdateProgress {
            downloaded_bytes: self.downloaded.load(Ordering::Relaxed),
            total_bytes: (total > 0).then_some(total),
        }
    }
}

/// Decides which progress samples are worth emitting. After the first sample,
/// one is emitted only when the interval has elapsed since the last emission
/// and, when the total is known, the whole percentage has changed, so
/// whichever gate is less frequent wins. The completing sample is always
/// emitted, once, so the bar finishes promptly.
#[derive(Debug)]
pub struct ProgressThrottle {
    interval: Duration,
    last_at: Option<Instant>,
    last_percent: Option<u64>,
    finished: bool,
}

impl Default for ProgressThrottle {
    fn default() -> Self {
        Self::new(PROGRESS_INTERVAL)
    }
}

impl ProgressThrottle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_at: None,
            last_percent: None,
            finished: false,
        }
    }

    pub fn observe(&mut self, now: Instant, progress: UpdateProgress) -> bool {
        if self.finished {
            return false;
        }
        let percent = progress.percent();
        let emit = if progress.complete() {
            self.finished = true;
            true
        } else {
            match self.last_at {
                None => true,
                Some(last) => {
                    now.saturating_duration_since(last) >= self.interval
                        && (percent.is_none() || percent != self.last_percent)
                }
            }
        };
        if emit {
            self.last_at = Some(now);
            self.last_percent = percent;
        }
        emit
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Check,
    Download,
    Install,
}

#[derive(Default)]
struct RuntimeData<U> {
    active: Option<Operation>,
    available: Option<U>,
    downloaded: Option<Vec<u8>>,
    cancel_download: Option<watch::Sender<bool>>,
}

pub struct UpdateManager<U = Update>(Mutex<RuntimeData<U>>);

impl<U> Default for UpdateManager<U> {
    fn default() -> Self {
        Self(Mutex::new(RuntimeData {
            active: None,
            available: None,
            downloaded: None,
            cancel_download: None,
        }))
    }
}

impl<U: Clone> UpdateManager<U> {
    pub fn begin(&self, operation: Operation) -> bool {
        let mut data = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if data.active.is_some() {
            return false;
        }
        data.active = Some(operation);
        true
    }

    pub fn finish(&self, operation: Operation) {
        let mut data = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if data.active == Some(operation) {
            data.active = None;
        }
        if operation == Operation::Download {
            data.cancel_download = None;
        }
    }

    pub fn replace_available(&self, update: Option<U>) {
        let mut data = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.available = update;
        data.downloaded = None;
    }

    pub fn available(&self) -> Option<U> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .available
            .clone()
    }

    pub fn store_download(&self, bytes: Vec<u8>) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .downloaded = Some(bytes);
    }

    pub fn take_download(&self) -> Option<Vec<u8>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .downloaded
            .take()
    }

    pub fn has_download(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .downloaded
            .is_some()
    }

    pub fn set_download_cancel(&self, sender: watch::Sender<bool>) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel_download = Some(sender);
    }

    pub fn cancel_download(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel_download
            .as_ref()
            .is_some_and(|sender| sender.send(true).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::Notify;

    use super::*;

    fn v(text: &str) -> semver::Version {
        semver::Version::parse(text).unwrap()
    }

    #[test]
    fn a_stable_build_never_installs_a_pre_release() {
        assert!(!accepts_update(&v("1.0.0"), &v("1.0.1-rc.1")));
        assert!(!accepts_update(&v("1.0.0"), &v("2.0.0-beta.1")));
        assert!(accepts_update(&v("1.0.0"), &v("1.0.1")));
        assert!(accepts_update(&v("1.0.0"), &v("2.0.0")));
    }

    #[test]
    fn a_pre_release_build_installs_anything_newer() {
        assert!(accepts_update(&v("1.0.0-rc.32"), &v("1.0.0-rc.33")));
        assert!(accepts_update(&v("1.0.0-rc.32"), &v("1.0.0")));
        assert!(accepts_update(&v("1.0.0-rc.32"), &v("1.0.1-rc.1")));
    }

    #[test]
    fn no_build_installs_the_same_or_an_older_version() {
        assert!(!accepts_update(&v("1.0.0"), &v("1.0.0")));
        assert!(!accepts_update(&v("1.0.0"), &v("0.10.0")));
        assert!(!accepts_update(&v("1.0.0-rc.32"), &v("1.0.0-rc.32")));
        assert!(!accepts_update(&v("1.0.0-rc.32"), &v("1.0.0-rc.31")));
        assert!(!accepts_update(&v("1.0.1"), &v("1.0.1-rc.1")));
    }

    #[derive(Clone)]
    struct FakeUpdate {
        version: String,
        chunks: Vec<Vec<u8>>,
        download_error: Option<String>,
        install_error: Option<String>,
        installed: Arc<Mutex<Vec<Vec<u8>>>>,
        gate: Option<Arc<Notify>>,
    }

    impl FakeUpdate {
        fn successful(chunks: Vec<Vec<u8>>) -> Self {
            Self {
                version: "2.0.0".into(),
                chunks,
                download_error: None,
                install_error: None,
                installed: Arc::new(Mutex::new(Vec::new())),
                gate: None,
            }
        }
    }

    impl UpdateArtifact for FakeUpdate {
        fn version(&self) -> &str {
            &self.version
        }

        fn download<'a>(
            &'a self,
            mut on_chunk: Box<dyn FnMut(usize, Option<u64>) + Send + 'a>,
        ) -> DownloadFuture<'a> {
            Box::pin(async move {
                if let Some(gate) = &self.gate {
                    gate.notified().await;
                }
                if let Some(error) = &self.download_error {
                    return Err(error.clone());
                }
                let total = self.chunks.iter().map(Vec::len).sum::<usize>() as u64;
                let mut bytes = Vec::new();
                for chunk in &self.chunks {
                    on_chunk(chunk.len(), Some(total));
                    bytes.extend_from_slice(chunk);
                }
                Ok(bytes)
            })
        }

        fn install(&self, bytes: &[u8]) -> Result<(), String> {
            if let Some(error) = &self.install_error {
                return Err(error.clone());
            }
            self.installed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(bytes.to_vec());
            Ok(())
        }
    }

    fn progress(downloaded_bytes: u64, total_bytes: Option<u64>) -> UpdateProgress {
        UpdateProgress {
            downloaded_bytes,
            total_bytes,
        }
    }

    #[test]
    fn progress_counter_accumulates_outside_the_state() {
        let counter = ProgressCounter::default();
        assert_eq!(counter.current(), progress(0, None));
        assert_eq!(counter.add(25, Some(100)), progress(25, Some(100)));
        assert_eq!(counter.add(30, None), progress(55, Some(100)));
    }

    #[test]
    fn progress_counter_reports_an_unknown_total() {
        let counter = ProgressCounter::default();
        assert_eq!(counter.add(10, None), progress(10, None));
    }

    #[test]
    fn throttle_waits_for_the_interval_even_when_the_percent_changes() {
        let start = Instant::now();
        let ms = |value| start + Duration::from_millis(value);
        let mut throttle = ProgressThrottle::default();
        assert!(throttle.observe(ms(0), progress(1, Some(100))));
        assert!(!throttle.observe(ms(100), progress(20, Some(100))));
        assert!(!throttle.observe(ms(249), progress(40, Some(100))));
        assert!(throttle.observe(ms(250), progress(41, Some(100))));
        assert!(!throttle.observe(ms(300), progress(60, Some(100))));
    }

    #[test]
    fn throttle_waits_for_a_whole_percent_even_after_the_interval() {
        let start = Instant::now();
        let ms = |value| start + Duration::from_millis(value);
        let total = Some(10_000);
        let mut throttle = ProgressThrottle::default();
        assert!(throttle.observe(ms(0), progress(100, total)));
        assert!(!throttle.observe(ms(1_000), progress(150, total)));
        assert!(!throttle.observe(ms(2_000), progress(199, total)));
        assert!(throttle.observe(ms(3_000), progress(200, total)));
        assert!(!throttle.observe(ms(4_000), progress(250, total)));
    }

    #[test]
    fn throttle_flushes_completion_once_regardless_of_timing() {
        let start = Instant::now();
        let ms = |value| start + Duration::from_millis(value);
        let mut throttle = ProgressThrottle::default();
        assert!(throttle.observe(ms(0), progress(99, Some(100))));
        assert!(throttle.observe(ms(10), progress(100, Some(100))));
        assert!(!throttle.observe(ms(1_000), progress(100, Some(100))));
    }

    #[test]
    fn throttle_uses_time_alone_when_the_total_is_unknown() {
        let start = Instant::now();
        let ms = |value| start + Duration::from_millis(value);
        let mut throttle = ProgressThrottle::default();
        assert!(throttle.observe(ms(0), progress(10, None)));
        assert!(!throttle.observe(ms(200), progress(20, None)));
        assert!(throttle.observe(ms(250), progress(20, None)));
        assert!(throttle.observe(ms(500), progress(30, None)));
        assert!(!throttle.observe(ms(600), progress(40, None)));
    }

    #[test]
    fn throttle_bounds_events_for_a_large_download() {
        let start = Instant::now();
        let total = 155 * 1024 * 1024_u64;
        let chunk = 16 * 1024_u64;
        let mut throttle = ProgressThrottle::default();
        let (mut downloaded, mut emitted, mut elapsed) = (0, 0, 0);
        while downloaded < total {
            downloaded = (downloaded + chunk).min(total);
            elapsed += 1;
            if throttle.observe(
                start + Duration::from_millis(elapsed),
                progress(downloaded, Some(total)),
            ) {
                emitted += 1;
            }
        }
        assert!(emitted > 1);
        assert!(emitted <= 101, "emitted {emitted} progress events");
    }

    #[tokio::test]
    async fn fake_download_emits_throttled_progress_and_final_count() {
        let update = FakeUpdate::successful(vec![vec![0; 10]; 50]);
        let (_cancel, receiver) = watch::channel(false);
        let counter = Arc::new(ProgressCounter::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let (chunk_counter, chunk_events) = (counter.clone(), events.clone());
        let start = Instant::now();
        let mut throttle = ProgressThrottle::default();
        let result = download_with_cancel(&update, receiver, move |chunk, total| {
            let sample = chunk_counter.add(chunk, total);
            // Every chunk arrives at the same injected instant, so only the
            // first and the completing samples pass.
            if throttle.observe(start, sample) {
                chunk_events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(sample);
            }
        })
        .await;
        assert!(matches!(result, DownloadResult::Complete(Ok(bytes)) if bytes.len() == 500));
        assert_eq!(counter.current(), progress(500, Some(500)));
        assert_eq!(
            *events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![progress(10, Some(500)), progress(500, Some(500))]
        );
    }

    #[test]
    fn progress_event_is_small_and_camel_cased() {
        assert_eq!(PROGRESS_EVENT, "update-progress");
        assert_eq!(
            serde_json::to_value(progress(5, Some(10))).unwrap(),
            serde_json::json!({ "downloadedBytes": 5, "totalBytes": 10 })
        );
    }

    #[test]
    fn transitions_expose_progress_retry_and_cancellation() {
        assert_eq!(
            UpdateView::ready("2.0.0".into(), 100, Some(100)).status,
            UpdateStatus::ReadyToInstall
        );
        assert_eq!(
            UpdateView::cancelled("2.0.0".into()).retry_action,
            Some(RetryAction::Download)
        );
        assert_eq!(
            UpdateView::failed(None, "offline".into(), RetryAction::Check).retry_action,
            Some(RetryAction::Check)
        );
    }

    #[test]
    fn operation_gate_deduplicates_concurrent_work() {
        let manager = UpdateManager::<FakeUpdate>::default();
        assert!(manager.begin(Operation::Check));
        assert!(!manager.begin(Operation::Check));
        assert!(!manager.begin(Operation::Install));
        manager.finish(Operation::Check);
        assert!(manager.begin(Operation::Install));
        manager.store_download(vec![1, 2, 3]);
        assert!(manager.has_download());
    }

    #[tokio::test]
    async fn cancellation_signal_is_one_shot_and_non_blocking() {
        let manager = UpdateManager::<FakeUpdate>::default();
        let (sender, mut receiver) = watch::channel(false);
        manager.set_download_cancel(sender);
        assert!(manager.cancel_download());
        receiver.changed().await.unwrap();
        assert!(*receiver.borrow());
        manager.finish(Operation::Download);
        assert!(!manager.cancel_download());
    }

    #[tokio::test]
    async fn fake_artifact_download_reports_progress_and_installs_exact_bytes() {
        let update = FakeUpdate::successful(vec![vec![1, 2], vec![3, 4, 5]]);
        let (_cancel, receiver) = watch::channel(false);
        let progress = Arc::new(Mutex::new(Vec::new()));
        let progress_events = progress.clone();
        let result = download_with_cancel(&update, receiver, move |chunk, total| {
            progress_events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((chunk, total));
        })
        .await;
        assert_eq!(result, DownloadResult::Complete(Ok(vec![1, 2, 3, 4, 5])));
        assert_eq!(
            *progress
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![(2, Some(5)), (3, Some(5))]
        );
        update.install(&[1, 2, 3]).unwrap();
        assert_eq!(
            *update
                .installed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![vec![1, 2, 3]]
        );
    }

    #[tokio::test]
    async fn fake_artifact_exposes_download_and_install_failures() {
        let mut update = FakeUpdate::successful(Vec::new());
        update.download_error = Some("network unavailable".into());
        update.install_error = Some("installer rejected".into());
        let (_cancel, receiver) = watch::channel(false);
        assert_eq!(
            download_with_cancel(&update, receiver, |_, _| {}).await,
            DownloadResult::Complete(Err("network unavailable".into()))
        );
        assert_eq!(update.install(&[1]), Err("installer rejected".into()));
    }

    #[tokio::test]
    async fn cancellation_drops_an_incomplete_fake_download() {
        let gate = Arc::new(Notify::new());
        let mut update = FakeUpdate::successful(vec![vec![1]]);
        update.gate = Some(gate);
        let (cancel, receiver) = watch::channel(false);
        let download = download_with_cancel(&update, receiver, |_, _| {});
        tokio::pin!(download);
        tokio::select! {
            result = &mut download => panic!("download finished before cancellation: {result:?}"),
            () = tokio::task::yield_now() => {}
        }
        cancel.send(true).unwrap();
        assert_eq!(download.await, DownloadResult::Cancelled);
    }
}
