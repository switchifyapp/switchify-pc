use super::{
    windows, windows_hook::await_shutdown, windows_worker_protocol as wire, Core, Driver, Mode,
    HEARTBEAT_TIMEOUT_MS,
};
use anyhow::{bail, Context, Result};
use std::{
    io,
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering::*},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{JobObjects::*, SystemInformation::GetTickCount64, Threading::CREATE_NO_WINDOW},
};

const ARGUMENT: &str = "--switchify-capture-worker";

struct Job(HANDLE);
unsafe impl Send for Job {}
unsafe impl Sync for Job {}
impl Job {
    fn new() -> Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error().into());
            }
            let job = Self(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            Ok(job)
        }
    }
    fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Shared {
    disposing: AtomicBool,
    cancellation: AtomicU8,
    recovering: AtomicBool,
    finished: AtomicBool,
    response_ms: AtomicU64,
    owned: Mutex<Vec<String>>,
}

pub(super) struct Capture {
    driver: Driver,
    shared: Arc<Shared>,
    job: Arc<Job>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn start(driver: Driver, mode: Mode) -> Result<Self> {
        let request = {
            let core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
            if core.worker_failed {
                bail!(wire::WORKER_FAILURE_MESSAGE);
            }
            wire::Start {
                version: wire::VERSION,
                generation: core.status.generation,
                mode,
                mappings: core.mappings.clone(),
                escape_ms: core.escape_ms,
                parent_ms: driver.now(),
                sent_ticks: unsafe { GetTickCount64() },
            }
        };
        let generation = request.generation.wrapping_add(1);
        let job =
            Arc::new(Job::new().context("Could not create the keyboard capture worker job.")?);
        let mut child = Command::new(std::env::current_exe()?)
            .arg(ARGUMENT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .context("Could not start the keyboard capture worker.")?;
        if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0 {
            let error = io::Error::last_os_error();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error).context("Could not contain the keyboard capture worker.");
        }
        let shared = Arc::new(Shared {
            disposing: AtomicBool::new(false),
            cancellation: AtomicU8::new(0),
            recovering: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            response_ms: AtomicU64::new(driver.now()),
            owned: Mutex::new(Vec::new()),
        });
        let (tx, rx) = mpsc::sync_channel(1);
        let running = shared.clone();
        let events = driver.clone();
        let contained = job.clone();
        let thread = std::thread::Builder::new()
            .name("switchify-capture-worker-io".into())
            .spawn(move || {
                let mut rejected = false;
                let exchange = || -> Result<()> {
                    let mut input = child
                        .stdin
                        .take()
                        .context("Missing capture worker input.")?;
                    let mut output = child
                        .stdout
                        .take()
                        .context("Missing capture worker output.")?;
                    wire::write(&mut input, &request)?;
                    let initial: wire::Reply = wire::read(&mut output)?;
                    let snapshot = match initial {
                        wire::Reply::Ready(state) => state,
                        wire::Reply::Failed(error) => {
                            rejected = true;
                            bail!(error)
                        }
                    };
                    {
                        let mut core = events.core.lock().unwrap_or_else(|p| p.into_inner());
                        wire::ready(&mut core, &snapshot, generation, mode, events.now())?;
                    }
                    running.response_ms.store(events.now(), Release);
                    tx.send(Ok(()))
                        .map_err(|_| anyhow::anyhow!("Capture startup was cancelled."))?;
                    loop {
                        let heartbeat_ms = events
                            .core
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .last_heartbeat;
                        wire::write(
                            &mut input,
                            &wire::Poll {
                                generation,
                                heartbeat_ms,
                                cancellation: running.cancellation.load(Acquire),
                            },
                        )?;
                        let snapshot: wire::Snapshot = wire::read(&mut output)?;
                        {
                            let mut core = events.core.lock().unwrap_or_else(|p| p.into_inner());
                            running.recovering.store(snapshot.recovering, Release);
                            *running.owned.lock().unwrap_or_else(|p| p.into_inner()) =
                                snapshot.owned.clone();
                            wire::apply(
                                &mut core,
                                &snapshot,
                                generation,
                                running.cancellation.load(Acquire) != 0,
                            )?;
                        }
                        running.response_ms.store(events.now(), Release);
                        if snapshot.finished {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(())
                };
                let mut exchange = exchange;
                let outcome = exchange();
                let healthy = outcome.is_ok();
                if let Err(error) = outcome {
                    let _ = tx.try_send(Err(error.to_string()));
                    if !rejected && !running.disposing.load(Acquire) {
                        wire::worker_failed(
                            &mut events.core.lock().unwrap_or_else(|p| p.into_inner()),
                        );
                    }
                }
                contained.terminate();
                let _ = child.wait();
                if healthy {
                    running
                        .owned
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .clear();
                    events
                        .core
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .physical
                        .clear();
                }
                running.recovering.store(false, Release);
                running.finished.store(true, Release);
            })?;
        let mut capture = Self {
            driver,
            shared,
            job,
            thread: Some(thread),
        };
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(capture),
            result => {
                capture.cancel();
                capture.job.terminate();
                if let Some(thread) = capture.thread.take() {
                    let _ = thread.join();
                }
                capture.driver.lost();
                match result {
                    Ok(Err(error)) => bail!(error),
                    _ => bail!("Keyboard capture worker did not acknowledge startup."),
                }
            }
        }
    }
    pub fn check_health(&self) {
        if !self.shared.finished.load(Acquire)
            && self
                .driver
                .now()
                .saturating_sub(self.shared.response_ms.load(Acquire))
                >= HEARTBEAT_TIMEOUT_MS
        {
            self.job.terminate();
            wire::worker_failed(&mut self.driver.core.lock().unwrap_or_else(|p| p.into_inner()));
        }
    }
    pub fn cancel(&self) {
        self.shared.cancellation.store(2, Release);
    }
    pub fn cancel_for_recovery(&self) {
        let _ = self
            .shared
            .cancellation
            .compare_exchange(0, 1, AcqRel, Acquire);
    }
    pub fn recovering(&self) -> bool {
        self.shared.recovering.load(Acquire) && self.shared.cancellation.load(Acquire) < 2
    }
    pub fn held_keys(&self) -> Vec<String> {
        self.shared
            .owned
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn await_shutdown(&self) -> Result<()> {
        let started = Instant::now();
        await_shutdown(
            || {
                self.check_health();
                let held = self.held_keys();
                if !held.is_empty() {
                    bail!(
                        "Release held keys before starting capture: {}.",
                        held.join(", ")
                    );
                }
                Ok(self.shared.finished.load(Acquire))
            },
            || std::thread::sleep(Duration::from_millis(1)),
            || started.elapsed().as_millis() as u64,
        )
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.shared.disposing.store(true, Release);
        self.cancel();
        self.job.terminate();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn snapshot(driver: &Driver, native: &windows::Capture) -> wire::Snapshot {
    let mut core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
    wire::Snapshot {
        status: core.status,
        down: core.down.clone(),
        owned: native.held_keys(),
        native_lost: core.native_lost,
        recovering: native.recovering(),
        finished: native.finished(),
        events: core.events.drain(..).collect(),
    }
}

fn run() -> Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let request: wire::Start = wire::read(&mut input)?;
    if request.version != wire::VERSION
        || !matches!(request.mode, Mode::Active | Mode::Learning)
        || request.mappings.len() > 128
        || request.escape_ms < 4000
    {
        bail!("Invalid keyboard capture worker configuration.");
    }
    let age_ms = wire::clock_age(request.parent_ms, request.sent_ticks, unsafe {
        GetTickCount64()
    })?;
    let started = Instant::now()
        .checked_sub(Duration::from_millis(age_ms))
        .context("Invalid capture clock origin.")?;
    let mut core = Core {
        mappings: request.mappings,
        escape_ms: request.escape_ms,
        ..Core::default()
    };
    core.status.generation = request.generation;
    let driver = Driver {
        core: Arc::new(Mutex::new(core)),
        started,
    };
    let native = match windows::Capture::start(driver.clone(), request.mode) {
        Ok(native) => native,
        Err(error) => {
            wire::write(&mut output, &wire::Reply::Failed(error.to_string()))?;
            return Ok(());
        }
    };
    wire::write(&mut output, &wire::Reply::Ready(snapshot(&driver, &native)))?;
    loop {
        let poll: wire::Poll = wire::read(&mut input)?;
        if poll.generation != request.generation.wrapping_add(1) || poll.cancellation > 2 {
            bail!("Invalid capture worker command.");
        }
        match poll.cancellation {
            1 => native.cancel_for_recovery(),
            2 => native.cancel(),
            _ => {}
        }
        driver
            .core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_heartbeat = poll.heartbeat_ms.min(driver.now());
        let state = snapshot(&driver, &native);
        let finished = state.finished;
        wire::write(&mut output, &state)?;
        if finished {
            return Ok(());
        }
    }
}

pub(super) fn run_from_args() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(ARGUMENT)) {
        return false;
    }
    let _ = run();
    true
}
