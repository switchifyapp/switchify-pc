mod activity;
mod context;
mod database;
#[cfg(test)]
mod database_reference;
mod lookup;
pub mod worker;

use crate::scan_keyboard::{Key, Keyboard, Modifier, Page, Stroke};
use std::{
    cell::RefCell,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::atomic::{AtomicU8, Ordering},
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use worker::{Edit, Request, Response};

#[cfg(target_os = "windows")]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(target_os = "windows")]
impl Job {
    fn contain(child: &Child) -> Result<Self, ()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return Err(());
            }
            let job = Self(h);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(h, child.as_raw_handle()) == 0
            {
                return Err(());
            }
            Ok(job)
        }
    }
}
#[cfg(target_os = "windows")]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
struct Client {
    child: Child,
    input: ChildStdin,
    replies: mpsc::Receiver<Result<Response, ()>>,
    reader: Option<JoinHandle<()>>,
    #[cfg(target_os = "windows")]
    _job: Job,
}
impl Client {
    fn start(path: &Path, ignored: Vec<u32>) -> Result<Self, ()> {
        let mut command = Command::new(std::env::current_exe().map_err(|_| ())?);
        command
            .arg(worker::ARG)
            .arg(path)
            .arg(serde_json::to_string(&ignored).map_err(|_| ())?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        Self::spawn(command)
    }
    fn spawn(mut command: Command) -> Result<Self, ()> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| ())?;
        #[cfg(target_os = "windows")]
        let job = match Job::contain(&child) {
            Ok(j) => j,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(());
            }
        };
        let input = child.stdin.take().ok_or(())?;
        let mut output = child.stdout.take().ok_or(())?;
        let (tx, replies) = mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || loop {
            let reply = worker::receive(&mut output);
            let failed = reply.is_err();
            if tx.send(reply).is_err() || failed {
                break;
            }
        });
        Ok(Self {
            child,
            input,
            replies,
            reader: Some(reader),
            #[cfg(target_os = "windows")]
            _job: job,
        })
    }
}
impl Client {
    fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Drain the bounded channel before joining, including a final EOF reply.
        while let Some(reader) = self.reader.as_ref() {
            if reader.is_finished() {
                break;
            }
            let _ = self.replies.recv_timeout(Duration::from_millis(10));
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.terminate();
    }
}
#[derive(Default)]
struct Service {
    client: Option<Client>,
    failed: bool,
    generation: u64,
    outstanding: Option<Instant>,
    last: Option<Instant>,
    edit: Vec<worker::RecordedEdit>,
    edit_revision: u64,
    acknowledged_revision: u64,
    reset: bool,
    accept: Option<(u64, usize)>,
    accepting: bool,
    case: Option<(bool, bool, bool)>,
    tracking: bool,
}
#[derive(Clone, Copy)]
pub struct InputScope {
    foreground: usize,
    time: u64,
}
impl InputScope {
    pub fn capture() -> Self {
        #[cfg(not(test))]
        let foreground = crate::scan_host::foreground().unwrap_or(0);
        #[cfg(test)]
        let foreground = 1;
        Self {
            foreground,
            time: activity::now(),
        }
    }
    fn unchanged(self) -> bool {
        self.foreground != 0 && Self::capture().foreground == self.foreground
    }
    fn record(self, edit: Edit) -> worker::RecordedEdit {
        worker::RecordedEdit {
            edit,
            foreground: self.foreground,
            time: self.time,
        }
    }
}
pub fn reset() {
    SERVICE.with(|s| {
        let mut s = s.borrow_mut();
        s.generation = s.generation.wrapping_add(1);
        s.edit.clear();
        s.reset = true;
        s.last = None;
        s.accept = None;
    });
}
impl Service {
    fn queue_edit(&mut self, edit: Edit, scope: InputScope) {
        self.edit_revision = self.edit_revision.wrapping_add(1);
        if self.edit.len() >= 512 {
            self.edit.clear();
            self.reset = true;
        }
        self.edit.push(scope.record(edit));
    }
    fn take_edits(&mut self) -> Vec<worker::RecordedEdit> {
        let mut edits = std::mem::take(&mut self.edit);
        if std::mem::take(&mut self.reset) {
            self.edit_revision = self.edit_revision.wrapping_add(1);
            edits.insert(0, InputScope::capture().record(Edit::Reset));
        }
        edits
    }

    fn received_suggestions(
        &mut self,
        keyboard: &mut Keyboard,
        generation: u64,
        revision: u64,
        batch: Option<worker::Batch>,
        tracking: bool,
    ) {
        if revision < self.acknowledged_revision || revision > self.edit_revision {
            self.fail(keyboard);
            return;
        }
        self.acknowledged_revision = revision;
        if generation == self.generation && revision == self.edit_revision {
            self.suggestions(keyboard, batch, tracking);
        } else {
            self.tracking = tracking;
            if !tracking {
                self.edit.clear();
                self.reset = true;
            }
        }
    }
    fn suggestions(
        &mut self,
        keyboard: &mut Keyboard,
        batch: Option<worker::Batch>,
        tracking: bool,
    ) {
        self.tracking = tracking;
        if !tracking {
            self.edit.clear();
        }
        let visible = keyboard.page == Page::Letters
            && !keyboard.modifiers[1..].iter().any(|m| *m != Modifier::Off);
        keyboard.predictions(if visible { batch } else { None }, false);
    }
    fn fail(&mut self, keyboard: &mut Keyboard) {
        self.client = None;
        self.failed = true;
        self.tracking = false;
        self.edit.clear();
        self.outstanding = None;
        keyboard.predictions(None, true);
        if self.accepting || self.accept.is_some() {
            keyboard.failed();
        }
        self.accept = None;
        self.accepting = false;
    }
}
thread_local! { static SERVICE: RefCell<Service> = RefCell::new(Service::default()); }
// 0 = idle, 1 = starting, 2 = running. A failed or disabled hook can retry.
static KEYBOARD_ACTIVITY: AtomicU8 = AtomicU8::new(0);

fn claim_keyboard_activity_start(state: &AtomicU8, healthy: bool) -> bool {
    if healthy {
        return false;
    }
    let _ = state.compare_exchange(2, 0, Ordering::SeqCst, Ordering::SeqCst);
    state
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn start_keyboard_activity(ignored: &[String]) {
    activity::set_ignored(
        ignored
            .iter()
            .filter_map(|name| crate::switch_input::prediction_key_code(name))
            .collect(),
    );
    if claim_keyboard_activity_start(&KEYBOARD_ACTIVITY, activity::snapshot().1) {
        std::thread::spawn(move || {
            let started = activity::start();
            if !started {
                // Avoid a busy retry loop while native input access is unavailable.
                std::thread::sleep(Duration::from_secs(1));
            }
            KEYBOARD_ACTIVITY.store(if started { 2 } else { 0 }, Ordering::SeqCst);
        });
    }
}

pub fn keyboard_input_context() -> Option<crate::scan_keyboard::TypingContext> {
    let (epoch, healthy) = activity::snapshot();
    if !healthy {
        return None;
    }
    let foreground = crate::scan_host::foreground().ok()?;
    (foreground != 0).then_some(crate::scan_keyboard::TypingContext {
        foreground,
        activity: epoch,
    })
}
pub fn stop() {
    SERVICE.with(|s| *s.borrow_mut() = Service::default());
}
pub fn record(stroke: Stroke, success: bool, scope: InputScope) {
    SERVICE.with(|s| {
        let mut s = s.borrow_mut();
        s.generation = s.generation.wrapping_add(1);
        s.last = None;
        if success && scope.unchanged() && !stroke.shortcut() {
            let edit = stroke
                .character()
                .map(|c| Edit::Append(c.to_string()))
                .or_else(|| {
                    (stroke.key == Key::Named("Backspace") && !stroke.modifiers[0])
                        .then_some(Edit::Backspace)
                });
            if let Some(edit) = edit {
                s.queue_edit(edit, scope);
                return;
            }
        }
        s.edit.clear();
        s.reset = true;
    });
}
pub fn record_punctuation(mark: char, removed_space: bool, success: bool, scope: InputScope) {
    SERVICE.with(|s| {
        let mut s = s.borrow_mut();
        s.generation = s.generation.wrapping_add(1);
        s.last = None;
        if success && scope.unchanged() {
            if removed_space {
                s.queue_edit(Edit::Backspace, scope);
            }
            s.queue_edit(Edit::Append(format!("{mark} ")), scope);
        } else {
            s.edit.clear();
            s.reset = true;
        }
    });
}
pub fn select(token: u64, index: usize) -> Result<(), String> {
    SERVICE.with(|s| {
        let mut s = s.borrow_mut();
        if s.failed || s.client.is_none() || s.accept.is_some() || s.accepting {
            return Err("Prediction is unavailable.".into());
        }
        s.accept = Some((token, index));
        Ok(())
    })
}
fn resource(app: &AppHandle) -> Result<std::path::PathBuf, ()> {
    let bundled = app
        .path()
        .resolve(
            "resources/word-predictions.lookup",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|_| ());
    let development = cfg!(debug_assertions)
        .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/word-predictions.lookup"));
    database_resource(bundled, development)
}

fn database_resource(
    bundled: Result<std::path::PathBuf, ()>,
    development: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf, ()> {
    bundled
        .ok()
        .filter(|path| path.is_file())
        .or_else(|| development.filter(|path| path.is_file()))
        .ok_or(())
}

pub fn poll(app: &AppHandle, keyboard: Option<&mut Keyboard>, enabled: bool, ignored: &[String]) {
    let Some(keyboard) = keyboard else {
        stop();
        return;
    };
    if !enabled {
        stop();
        keyboard.predictions(None, false);
        return;
    }
    SERVICE.with(|slot| {
        let mut s = slot.borrow_mut();
        let case = (
            keyboard.prediction_shift(),
            keyboard.caps,
            keyboard.modifiers[1..].iter().any(|m| *m != Modifier::Off),
        );
        if s.case != Some(case) {
            s.case = Some(case);
            s.generation = s.generation.wrapping_add(1);
            s.last = None;
            keyboard.predictions(None, false);
        }
        if s.failed {
            keyboard.predictions(None, true);
            return;
        }
        if s.client.is_none() {
            let ignored = ignored
                .iter()
                .filter_map(|name| crate::switch_input::prediction_key_code(name))
                .collect();
            s.client = resource(app)
                .and_then(|path| Client::start(&path, ignored))
                .ok();
            if s.client.is_none() {
                s.failed = true;
                keyboard.predictions(None, true);
                return;
            }
        }
        if s.outstanding
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(2))
        {
            s.fail(keyboard);
            return;
        }
        let reply = s.client.as_ref().unwrap().replies.try_recv();
        match reply {
            Ok(Ok(response)) => {
                s.outstanding = None;
                match response {
                    Response::Suggestions {
                        generation,
                        batch,
                        revision,
                        tracking,
                    } => {
                        s.received_suggestions(keyboard, generation, revision, batch, tracking);
                    }
                    Response::Insert {
                        generation,
                        text,
                        foreground,
                    } => {
                        s.accepting = false;
                        let result = if generation == s.generation {
                            text.ok_or(()).and_then(|text| {
                                crate::point_scan_ready(app).map_err(|_| ())?;
                                let scope = InputScope::capture();
                                if Some(scope.foreground) != foreground || !scope.unchanged() {
                                    return Err(());
                                }
                                crate::scan_executor::prediction_text(&text).map_err(|_| ())?;
                                if !scope.unchanged() {
                                    return Err(());
                                }
                                let trailing_space = text.ends_with(' ');
                                let contains_letter = text.chars().any(char::is_alphabetic);
                                s.queue_edit(Edit::Append(text), scope);
                                Ok((trailing_space, contains_letter))
                            })
                        } else {
                            Err(())
                        };
                        s.generation = s.generation.wrapping_add(1);
                        s.last = None;
                        keyboard.predictions(None, false);
                        if let Ok((trailing_space, contains_letter)) = result {
                            if keyboard.succeeded() {
                                keyboard.prediction_inserted(
                                    trailing_space,
                                    contains_letter,
                                    keyboard_input_context(),
                                );
                            }
                        } else {
                            s.edit.clear();
                            s.reset = true;
                            let _ = crate::scan_executor::cleanup();
                            keyboard.failed();
                        }
                    }
                }
            }
            Ok(Err(())) | Err(mpsc::TryRecvError::Disconnected) => {
                s.fail(keyboard);
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if s.outstanding.is_some() {
            return;
        }
        let request = if let Some((token, index)) = s.accept.take() {
            s.accepting = true;
            Request::Accept {
                generation: s.generation,
                token,
                revision: s.edit_revision,
                index,
            }
        } else {
            if s.last
                .is_some_and(|t| t.elapsed() < Duration::from_millis(250))
            {
                return;
            }
            Request::Query {
                generation: s.generation,
                edits: s.take_edits(),
                revision: s.edit_revision,
                shift: keyboard.prediction_shift(),
                caps: keyboard.caps,
            }
        };
        let now = Instant::now();
        s.last = Some(now);
        s.outstanding = Some(now);
        if worker::send(&mut s.client.as_mut().unwrap().input, &request).is_err() {
            s.fail(keyboard);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activity_observer_retries_after_failure_or_lost_hook() {
        let state = AtomicU8::new(0);
        assert!(claim_keyboard_activity_start(&state, false));
        assert!(!claim_keyboard_activity_start(&state, false));
        state.store(0, Ordering::SeqCst); // first hook attempt failed
        assert!(claim_keyboard_activity_start(&state, false));
        state.store(2, Ordering::SeqCst); // second hook started
        assert!(!claim_keyboard_activity_start(&state, true));
        assert!(claim_keyboard_activity_start(&state, false)); // hook later stopped
    }
    #[test]
    fn punctuation_records_the_backspace_and_insert_as_one_successful_edit() {
        stop();
        record_punctuation('.', true, true, InputScope::capture());
        SERVICE.with(|slot| {
            let service = slot.borrow();
            assert_eq!(service.edit.len(), 2);
            assert!(matches!(service.edit[0].edit, Edit::Backspace));
            assert!(matches!(service.edit[1].edit, Edit::Append(ref text) if text == ". "));
        });
        record_punctuation('?', true, false, InputScope::capture());
        SERVICE.with(|slot| {
            let service = slot.borrow();
            assert!(service.edit.is_empty());
            assert!(service.reset);
        });
        stop();
    }
    #[test]
    fn reopening_discards_failed_workers_and_pending_acceptance() {
        for failed in [false, true] {
            SERVICE.with(|slot| {
                *slot.borrow_mut() = Service {
                    failed,
                    outstanding: Some(Instant::now()),
                    accepting: true,
                    accept: Some((1, 0)),
                    ..Default::default()
                };
            });
            // The OpenKeyboard request starts a fresh service through stop().
            stop();
            SERVICE.with(|slot| {
                let s = slot.borrow();
                assert!(!s.failed);
                assert!(!s.accepting);
                assert!(s.accept.is_none());
                assert!(s.outstanding.is_none());
                assert!(s.client.is_none());
            });
        }
    }
    #[test]
    fn only_successful_supported_edits_enter_the_buffer() {
        stop();
        let stroke = Stroke {
            key: Key::Character('a', 'A'),
            modifiers: [false; 4],
            caps: false,
        };
        record(stroke, true, InputScope::capture());
        SERVICE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.edit_revision, 1);
            assert!(matches!(&s.edit[0].edit, Edit::Append(text) if text == "a"));
        });
        record(stroke, false, InputScope::capture());
        SERVICE.with(|s| {
            let mut s = s.borrow_mut();
            assert!(s.edit.is_empty());
            assert!(s.reset);
            assert!(matches!(s.take_edits()[0].edit, Edit::Reset));
        });
        record(
            Stroke {
                key: Key::Named("ArrowLeft"),
                ..stroke
            },
            true,
            InputScope::capture(),
        );
        SERVICE.with(|s| assert!(s.borrow().reset));
        stop();
    }
    #[test]
    fn invalid_edit_acknowledgements_fail_closed() {
        let mut service = Service {
            edit_revision: 3,
            acknowledged_revision: 2,
            ..Default::default()
        };
        let mut keyboard = Keyboard::new(false);
        service.received_suggestions(&mut keyboard, 0, 4, None, true);
        assert!(service.failed);
    }

    #[test]
    fn database_resolution_supports_transferred_bundles_and_unbundled_development() {
        let root = std::env::temp_dir().join(format!("switchify-resource-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let bundled = root.join("bundled.db");
        let source = root.join("source.db");
        std::fs::write(&bundled, []).unwrap();
        std::fs::write(&source, []).unwrap();
        assert_eq!(
            database_resource(Ok(bundled.clone()), Some(source.clone())),
            Ok(bundled.clone())
        );
        assert_eq!(
            database_resource(Ok(bundled.clone()), None),
            Ok(bundled.clone())
        );
        std::fs::remove_file(&source).unwrap();
        assert_eq!(
            database_resource(Ok(bundled.clone()), Some(source.clone())),
            Ok(bundled.clone())
        );
        std::fs::write(&source, []).unwrap();
        std::fs::remove_file(&bundled).unwrap();
        assert_eq!(
            database_resource(Ok(bundled.clone()), Some(source.clone())),
            Ok(source.clone())
        );
        assert_eq!(
            database_resource(Err(()), Some(source.clone())),
            Ok(source.clone())
        );
        assert_eq!(database_resource(Ok(bundled), None), Err(()));
        assert_eq!(database_resource(Ok(root.clone()), None), Err(()));
        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn cancellation_kills_and_reaps_a_blocked_worker() {
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut c = Command::new("powershell.exe");
            c.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ]);
            c
        };
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut c = Command::new("/bin/sleep");
            c.arg("60");
            c
        };
        command.stdin(Stdio::piped());
        let mut client = Client::spawn(command).expect("fake worker starts");
        let start = Instant::now();
        assert!(client
            .replies
            .recv_timeout(Duration::from_millis(100))
            .is_err());
        client.terminate();
        assert!(client.child.try_wait().unwrap().is_some());
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn bounded_private_frames_reject_invalid_lengths() {
        assert!(
            worker::receive::<Request>(&mut std::io::Cursor::new(u32::MAX.to_le_bytes())).is_err()
        );
        assert!(worker::receive::<Request>(&mut std::io::Cursor::new([0u8; 4])).is_err());
    }
}
