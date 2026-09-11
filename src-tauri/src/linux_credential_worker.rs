//! One blocking credential worker; no native operation is started until requested.
//! Caller cancellation invalidates results, not side effects of an in-flight write.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{sync_channel, SyncSender},
    Arc,
};
use std::time::Duration;

use tokio::sync::oneshot;

use crate::storage::AppStorage;

const MAX_DEVICE_ID_BYTES: usize = 128;
const MAX_TOKEN_BYTES: usize = 4096;

pub(crate) trait CredentialBackend: Send + Sync + 'static {
    fn load(&self, id: &str) -> Result<Option<String>, String>;
    fn save(&self, id: &str, token: &str) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}

impl CredentialBackend for AppStorage {
    fn load(&self, id: &str) -> Result<Option<String>, String> {
        self.load_pairing_token(id)
    }
    fn save(&self, id: &str, token: &str) -> Result<(), String> {
        self.save_pairing_token(id, token)
    }
    fn delete(&self, id: &str) -> Result<(), String> {
        self.delete_pairing_token(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerError {
    Busy,
    TimedOut,
    Cancelled,
    Stopped,
    InvalidRequest,
    StorageUnavailable,
}

enum Operation {
    Load(String),
    Save(String, String),
    Delete(String),
}
// Intentionally no Debug: replies and operations may hold credential material.
enum Reply {
    Loaded(Option<String>),
    Done,
}

struct State {
    busy: AtomicBool,
    stopped: AtomicBool,
    generation: AtomicU64,
}

struct Request {
    operation: Operation,
    generation: u64,
    cancelled: Arc<AtomicBool>,
    reply: oneshot::Sender<Result<Reply, WorkerError>>,
}

struct BusyGuard(Arc<State>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

struct CancellationGuard(Arc<AtomicBool>);
impl Drop for CancellationGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

pub(crate) struct CredentialWorker {
    sender: Option<SyncSender<Request>>,
    state: Arc<State>,
    wait: Duration,
}

impl CredentialWorker {
    pub(crate) fn start(
        backend: Arc<dyn CredentialBackend>,
        wait: Duration,
    ) -> Result<Self, WorkerError> {
        if wait.is_zero() || wait > Duration::from_secs(30) {
            return Err(WorkerError::InvalidRequest);
        }
        let (sender, receiver) = sync_channel::<Request>(1);
        let state = Arc::new(State {
            busy: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            generation: AtomicU64::new(0),
        });
        let worker_state = state.clone();
        std::thread::Builder::new()
            .name("linux-credentials".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let _busy = BusyGuard(worker_state.clone());
                    let current = || {
                        !worker_state.stopped.load(Ordering::SeqCst)
                            && worker_state.generation.load(Ordering::SeqCst) == request.generation
                            && !request.cancelled.load(Ordering::SeqCst)
                    };
                    if !current() {
                        drop(_busy);
                        let _ = request.reply.send(Err(WorkerError::Cancelled));
                        continue;
                    }
                    let result = match &request.operation {
                        Operation::Load(id) => backend.load(id).map(Reply::Loaded),
                        Operation::Save(id, token) => backend.save(id, token).map(|()| Reply::Done),
                        Operation::Delete(id) => backend.delete(id).map(|()| Reply::Done),
                    }
                    .map_err(|_| WorkerError::StorageUnavailable);
                    let result = match result {
                        Ok(Reply::Loaded(Some(ref token)))
                            if token.is_empty() || token.len() > MAX_TOKEN_BYTES =>
                        {
                            Err(WorkerError::StorageUnavailable)
                        }
                        other => other,
                    };
                    let result = if current() {
                        result
                    } else {
                        Err(WorkerError::Cancelled)
                    };
                    // Release admission before notifying the next caller.
                    drop(_busy);
                    let _ = request.reply.send(result);
                }
                worker_state.stopped.store(true, Ordering::SeqCst);
            })
            .map_err(|_| WorkerError::Stopped)?;
        Ok(Self {
            sender: Some(sender),
            state,
            wait,
        })
    }

    pub(crate) fn invalidate(&self) {
        self.state.generation.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn shutdown(&mut self) {
        self.state.stopped.store(true, Ordering::SeqCst);
        self.invalidate();
        self.sender.take();
        // Never join here: a synchronous Secret Service call may not be cancellable.
    }

    async fn request(&self, operation: Operation) -> Result<Reply, WorkerError> {
        if self.state.stopped.load(Ordering::SeqCst) {
            return Err(WorkerError::Stopped);
        }
        if self
            .state
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(WorkerError::Busy);
        }
        let generation = self.state.generation.load(Ordering::SeqCst);
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel = CancellationGuard(cancelled.clone());
        let (reply, receive) = oneshot::channel();
        let request = Request {
            operation,
            generation,
            cancelled,
            reply,
        };
        if self
            .sender
            .as_ref()
            .is_none_or(|sender| sender.try_send(request).is_err())
        {
            self.state.busy.store(false, Ordering::SeqCst);
            return Err(WorkerError::Stopped);
        }
        let result = tokio::time::timeout(self.wait, receive)
            .await
            .map_err(|_| WorkerError::TimedOut)?
            .map_err(|_| WorkerError::Stopped)?;
        if self.state.stopped.load(Ordering::SeqCst)
            || self.state.generation.load(Ordering::SeqCst) != generation
        {
            return Err(WorkerError::Cancelled);
        }
        result
    }

    fn valid_id(id: &str) -> bool {
        !id.is_empty() && id.len() <= MAX_DEVICE_ID_BYTES
    }

    pub(crate) async fn load(&self, id: &str) -> Result<Option<String>, WorkerError> {
        if !Self::valid_id(id) {
            return Err(WorkerError::InvalidRequest);
        }
        match self.request(Operation::Load(id.into())).await? {
            Reply::Loaded(token) => Ok(token),
            Reply::Done => Err(WorkerError::Stopped),
        }
    }

    pub(crate) async fn save(&self, id: &str, token: &str) -> Result<(), WorkerError> {
        if !Self::valid_id(id) || token.is_empty() || token.len() > MAX_TOKEN_BYTES {
            return Err(WorkerError::InvalidRequest);
        }
        self.request(Operation::Save(id.into(), token.into()))
            .await
            .map(|_| ())
    }

    pub(crate) async fn delete(&self, id: &str) -> Result<(), WorkerError> {
        if !Self::valid_id(id) {
            return Err(WorkerError::InvalidRequest);
        }
        self.request(Operation::Delete(id.into())).await.map(|_| ())
    }
}

impl Drop for CredentialWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Condvar, Mutex};

    #[derive(Default)]
    struct Fake {
        token: Mutex<Option<String>>,
        gate: (Mutex<bool>, Condvar),
        entered: AtomicBool,
        fail: AtomicBool,
    }
    impl Fake {
        fn release(&self) {
            *self.gate.0.lock().unwrap() = false;
            self.gate.1.notify_all();
        }
    }
    impl CredentialBackend for Fake {
        fn load(&self, _: &str) -> Result<Option<String>, String> {
            self.entered.store(true, Ordering::SeqCst);
            let mut blocked = self.gate.0.lock().unwrap();
            while *blocked {
                blocked = self.gate.1.wait(blocked).unwrap();
            }
            if self.fail.load(Ordering::SeqCst) {
                return Err("private native detail".into());
            }
            Ok(self.token.lock().unwrap().clone())
        }
        fn save(&self, _: &str, token: &str) -> Result<(), String> {
            *self.token.lock().unwrap() = Some(token.into());
            Ok(())
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            *self.token.lock().unwrap() = None;
            Ok(())
        }
    }
    async fn wait_until(mut predicate: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !predicate() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn success_validation_and_sanitized_errors() {
        let fake = Arc::new(Fake::default());
        let worker = CredentialWorker::start(fake.clone(), Duration::from_secs(1)).unwrap();
        worker.save("id", "test-token").await.unwrap();
        assert_eq!(
            worker.load("id").await.unwrap().as_deref(),
            Some("test-token")
        );
        worker.delete("id").await.unwrap();
        assert_eq!(worker.load("id").await.unwrap(), None);
        assert_eq!(worker.load("").await, Err(WorkerError::InvalidRequest));
        assert_eq!(
            worker.delete(&"x".repeat(MAX_DEVICE_ID_BYTES + 1)).await,
            Err(WorkerError::InvalidRequest)
        );
        assert_eq!(
            worker.save("id", &"x".repeat(MAX_TOKEN_BYTES + 1)).await,
            Err(WorkerError::InvalidRequest)
        );
        for token in [String::new(), "x".repeat(MAX_TOKEN_BYTES + 1)] {
            *fake.token.lock().unwrap() = Some(token);
            assert_eq!(
                worker.load("id").await,
                Err(WorkerError::StorageUnavailable)
            );
        }
        fake.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            worker.load("id").await,
            Err(WorkerError::StorageUnavailable)
        );
    }
    #[tokio::test(start_paused = true)]
    async fn timeout_keeps_single_worker_busy_until_backend_returns() {
        let fake = Arc::new(Fake::default());
        *fake.gate.0.lock().unwrap() = true;
        let worker = CredentialWorker::start(fake.clone(), Duration::from_millis(50)).unwrap();
        let request = worker.load("id");
        tokio::pin!(request);
        tokio::select! {
            _ = &mut request => panic!("request must remain blocked"),
            _ = wait_until(|| fake.entered.load(Ordering::SeqCst)) => (),
        }
        tokio::time::advance(Duration::from_millis(51)).await;
        assert_eq!(request.await, Err(WorkerError::TimedOut));
        assert_eq!(worker.load("id").await, Err(WorkerError::Busy));
        tokio::time::resume();
        fake.release();
        wait_until(|| !worker.state.busy.load(Ordering::SeqCst)).await;
        assert_eq!(worker.load("id").await.unwrap(), None);
    }
    #[tokio::test]
    async fn invalidation_discards_inflight_result_and_allows_recovery() {
        let fake = Arc::new(Fake::default());
        *fake.gate.0.lock().unwrap() = true;
        let worker = CredentialWorker::start(fake.clone(), Duration::from_secs(1)).unwrap();
        let request = worker.load("id");
        tokio::pin!(request);
        tokio::select! {
            _ = &mut request => panic!("request must remain blocked"),
            _ = wait_until(|| fake.entered.load(Ordering::SeqCst)) => (),
        }
        worker.invalidate();
        fake.release();
        assert_eq!(request.await, Err(WorkerError::Cancelled));
        assert_eq!(worker.load("id").await.unwrap(), None);
    }
    #[tokio::test]
    async fn dropped_future_and_shutdown_do_not_wait_for_blocked_backend() {
        let fake = Arc::new(Fake::default());
        *fake.gate.0.lock().unwrap() = true;
        let mut worker = CredentialWorker::start(fake.clone(), Duration::from_secs(1)).unwrap();
        {
            let request = worker.load("id");
            tokio::pin!(request);
            tokio::select! {
                _ = &mut request => panic!("request must remain blocked"),
                _ = wait_until(|| fake.entered.load(Ordering::SeqCst)) => (),
            }
        }
        assert_eq!(worker.load("id").await, Err(WorkerError::Busy));
        worker.shutdown();
        assert_eq!(worker.load("id").await, Err(WorkerError::Stopped));
        fake.release();
    }
}
