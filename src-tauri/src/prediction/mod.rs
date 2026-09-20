mod activity;
mod context;
mod database;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
pub mod worker;

use crate::scan_keyboard::{Keyboard, Modifier, Page, Stroke};
use std::{
    cell::RefCell,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use worker::{Request, Response};

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
    edit: Option<String>,
    reset: bool,
    accept: Option<(u64, usize)>,
    accepting: bool,
    case: Option<(bool, bool, bool)>,
    tracking: bool,
}
impl Service {
    fn suggestions(
        &mut self,
        keyboard: &mut Keyboard,
        batch: Option<worker::Batch>,
        tracking: bool,
    ) {
        self.tracking = tracking;
        if !tracking {
            self.edit = None;
        }
        keyboard.predictions(batch, false);
    }
    fn fail(&mut self, keyboard: &mut Keyboard) {
        self.client = None;
        self.failed = true;
        self.tracking = false;
        self.edit = None;
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
pub fn stop() {
    SERVICE.with(|s| *s.borrow_mut() = Service::default());
}
pub fn record(stroke: Stroke, success: bool) {
    SERVICE.with(|s| {
        let mut s = s.borrow_mut();
        s.generation = s.generation.wrapping_add(1);
        s.last = None;
        if success && !stroke.shortcut() && s.tracking {
            if let Some(c) = stroke.character() {
                if s.edit
                    .as_ref()
                    .is_some_and(|text| text.chars().count() >= 512)
                {
                    s.edit = None;
                    s.reset = true;
                }
                s.edit.get_or_insert_with(String::new).push(c);
                return;
            }
        }
        s.edit = None;
        s.reset = true;
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
            "resources/WordData2017051601.db",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|_| ());
    let development = cfg!(debug_assertions)
        .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/WordData2017051601.db"));
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
            keyboard.modifiers[0] != Modifier::Off,
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
                        tracking,
                    } if generation == s.generation => {
                        s.suggestions(keyboard, batch, tracking);
                    }
                    Response::Insert { generation, text } => {
                        s.accepting = false;
                        let result = if generation == s.generation {
                            text.ok_or(()).and_then(|text| {
                                crate::point_scan_ready(app).map_err(|_| ())?;
                                crate::scan_executor::prediction_text(&text).map_err(|_| ())?;
                                s.edit = Some(text);
                                s.reset = false;
                                Ok(())
                            })
                        } else {
                            Err(())
                        };
                        s.generation = s.generation.wrapping_add(1);
                        s.last = None;
                        keyboard.predictions(None, false);
                        if result.is_ok() {
                            keyboard.succeeded();
                        } else {
                            s.reset = true;
                            let _ = crate::scan_executor::cleanup();
                            keyboard.failed();
                        }
                    }
                    _ => {}
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
                index,
            }
        } else {
            if keyboard.page != Page::Letters
                || keyboard.modifiers[1..].iter().any(|m| *m != Modifier::Off)
            {
                keyboard.predictions(None, false);
                s.tracking = false;
                s.edit = None;
                s.reset = true;
                return;
            }
            if s.last
                .is_some_and(|t| t.elapsed() < Duration::from_millis(250))
            {
                return;
            }
            Request::Query {
                generation: s.generation,
                edit: s.edit.take(),
                reset: std::mem::take(&mut s.reset),
                shift: keyboard.modifiers[0] != Modifier::Off,
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
