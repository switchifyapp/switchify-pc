use std::sync::Mutex;

use serde::Serialize;
use tauri_plugin_updater::Update;
use tokio::sync::watch;

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
struct RuntimeData {
    active: Option<Operation>,
    available: Option<Update>,
    downloaded: Option<Vec<u8>>,
    cancel_download: Option<watch::Sender<bool>>,
}

#[derive(Default)]
pub struct UpdateManager(Mutex<RuntimeData>);

impl UpdateManager {
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

    pub fn replace_available(&self, update: Option<Update>) {
        let mut data = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.available = update;
        data.downloaded = None;
    }

    pub fn available(&self) -> Option<Update> {
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
    use super::*;

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
        let manager = UpdateManager::default();
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
        let manager = UpdateManager::default();
        let (sender, mut receiver) = watch::channel(false);
        manager.set_download_cancel(sender);
        assert!(manager.cancel_download());
        receiver.changed().await.unwrap();
        assert!(*receiver.borrow());
        manager.finish(Operation::Download);
        assert!(!manager.cancel_download());
    }
}
