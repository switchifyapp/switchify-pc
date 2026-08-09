use std::{future::Future, pin::Pin, sync::Mutex};

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

    pub fn add_progress(&mut self, chunk: usize, total: Option<u64>) {
        self.downloaded_bytes = self.downloaded_bytes.saturating_add(chunk as u64);
        if total.is_some() {
            self.total_bytes = total;
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

    #[test]
    fn transitions_expose_progress_retry_and_cancellation() {
        let mut downloading = UpdateView::downloading("2.0.0".into());
        downloading.add_progress(25, Some(100));
        downloading.add_progress(30, None);
        assert_eq!(downloading.downloaded_bytes, 55);
        assert_eq!(downloading.total_bytes, Some(100));
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
