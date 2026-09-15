use super::{
    windows_hook::{Installation, NativeResources, Resource, Shared, State},
    Driver, Mode, StopReason,
};
use anyhow::{bail, Result};
use std::{
    cell::RefCell,
    sync::{atomic::Ordering::*, mpsc, Arc},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{
        Input::{KeyboardAndMouse::*, *},
        WindowsAndMessaging::*,
    },
};

pub fn code_for_name(name: &str) -> Option<u32> {
    Some(match name {
        "Space" => 32,
        "Enter" => 13,
        "Backspace" => 8,
        "Tab" => 9,
        "Escape" => 27,
        "ArrowUp" => 38,
        "ArrowDown" => 40,
        "ArrowLeft" => 37,
        "ArrowRight" => 39,
        "Home" => 36,
        "End" => 35,
        "PageUp" => 33,
        "PageDown" => 34,
        "Insert" => 45,
        "Delete" => 46,
        n if n.len() == 1
            && n.bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) =>
        {
            n.as_bytes()[0] as u32
        }
        n => {
            let f = n.strip_prefix('F')?.parse::<u32>().ok()?;
            if !(1..=24).contains(&f) {
                return None;
            }
            0x70 + f - 1
        }
    })
}
fn known_keys() -> Vec<(u32, String)> {
    let mut names: Vec<String> = [
        "Space",
        "Enter",
        "Backspace",
        "Tab",
        "Escape",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Insert",
        "Delete",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    names.extend(('A'..='Z').chain('0'..='9').map(|c| c.to_string()));
    names.extend((1..=24).filter(|f| *f != 12).map(|f| format!("F{f}")));
    names
        .into_iter()
        .map(|name| (code_for_name(&name).unwrap(), name))
        .collect()
}

struct Input {
    state: State,
    shared: Arc<Shared>,
    started: std::time::Instant,
}
thread_local! { static INPUT: RefCell<Option<Input>> = const { RefCell::new(None) }; }
unsafe extern "system" fn keyboard(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0
        && matches!(
            wp as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
    {
        let key = &*(lp as *const KBDLLHOOKSTRUCT);
        if crate::input::own_input(key.dwExtraInfo as i64) {
            return CallNextHookEx(std::ptr::null_mut(), code, wp, lp);
        }
        let consumed = INPUT.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                return false;
            };
            let Some(input) = slot.as_mut() else {
                return false;
            };
            let now = input.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            input.state.key(
                &input.shared,
                key.vkCode,
                matches!(wp as u32, WM_KEYDOWN | WM_SYSKEYDOWN),
                false,
                now,
            )
        });
        if consumed {
            return 1;
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wp, lp)
}
unsafe extern "system" fn window(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_INPUT_DEVICE_CHANGE && wp == GIDC_REMOVAL as usize {
        INPUT.with(|slot| {
            if let Ok(slot) = slot.try_borrow() {
                if let Some(input) = slot.as_ref() {
                    input.shared.fail(StopReason::CaptureLost);
                }
            }
        });
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

struct Resources {
    hwnd: HWND,
    module: HINSTANCE,
    hook: HHOOK,
}
impl NativeResources for Resources {
    fn acquire(&mut self, resource: Resource) -> bool {
        unsafe {
            match resource {
                Resource::RawInput => {
                    RegisterRawInputDevices(
                        &RAWINPUTDEVICE {
                            usUsagePage: 1,
                            usUsage: 6,
                            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                            hwndTarget: self.hwnd,
                        },
                        1,
                        std::mem::size_of::<RAWINPUTDEVICE>() as u32,
                    ) != 0
                }
                Resource::Hook => {
                    self.hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), self.module, 0);
                    !self.hook.is_null()
                }
                Resource::Timer => SetTimer(self.hwnd, 1, 20, None) != 0,
            }
        }
    }
    fn release(&mut self, resource: Resource) -> bool {
        unsafe {
            match resource {
                Resource::Timer => KillTimer(self.hwnd, 1) != 0,
                Resource::Hook => UnhookWindowsHookEx(self.hook) != 0,
                Resource::RawInput => {
                    RegisterRawInputDevices(
                        &RAWINPUTDEVICE {
                            usUsagePage: 1,
                            usUsage: 6,
                            dwFlags: RIDEV_REMOVE,
                            hwndTarget: std::ptr::null_mut(),
                        },
                        1,
                        std::mem::size_of::<RAWINPUTDEVICE>() as u32,
                    ) != 0
                }
            }
        }
    }
}

pub struct Capture {
    shared: Arc<Shared>,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Capture {
    pub fn start(driver: Driver, mode: Mode) -> Result<Self> {
        let mut names: [Option<String>; 256] = std::array::from_fn(|_| None);
        let mut down = [false; 256];
        for (vk, name) in known_keys() {
            down[vk as usize] = unsafe { GetAsyncKeyState(vk as i32) < 0 };
            names[vk as usize] = Some(name);
        }
        let pressed = names
            .iter()
            .enumerate()
            .filter_map(|(vk, name)| if down[vk] { name.clone() } else { None })
            .collect();
        driver.ready(pressed);
        let (generation, active) = {
            let mut core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
            let pressed = core.down.clone();
            let generation = core.begin_with_pressed_keys(mode, driver.now(), pressed)?;
            let active = std::array::from_fn(|vk| {
                names[vk]
                    .as_ref()
                    .is_some_and(|name| mode == Mode::Learning || core.mappings.contains_key(name))
            });
            (generation, active)
        };
        let shared = Arc::new(Shared::new(generation, driver.now()));
        let running = shared.clone();
        let capture_driver = driver.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let thread = match std::thread::Builder::new()
            .name("switchify-keyboard-hook".into())
            .spawn(move || unsafe { run(capture_driver, mode, active, down, names, running, tx) })
        {
            Ok(thread) => thread,
            Err(error) => {
                driver.lost();
                return Err(error.into());
            }
        };
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread_id)) => Ok(Self {
                shared,
                thread_id,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                shared.shutdown.store(true, Release);
                let _ = thread.join();
                driver.lost();
                bail!(error)
            }
            Err(_) => {
                shared.shutdown.store(true, Release);
                shared.cancel();
                driver.lost();
                bail!("Keyboard capture did not acknowledge startup.")
            }
        }
    }
    pub fn cancel(&self) {
        self.shared.cancel();
    }
    pub fn held_keys(&self) -> Vec<String> {
        known_keys()
            .into_iter()
            .filter_map(|(vk, name)| self.shared.owned[vk as usize].load(Acquire).then_some(name))
            .collect()
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.shared.cancel();
        self.shared.shutdown.store(true, Release);
        unsafe {
            PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe fn run(
    driver: Driver,
    mode: Mode,
    active: [bool; 256],
    down: [bool; 256],
    names: [Option<String>; 256],
    shared: Arc<Shared>,
    tx: mpsc::SyncSender<Result<u32, String>>,
) {
    let module = GetModuleHandleW(std::ptr::null());
    let class = wide("SwitchifyLocalKeyboardInput");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(window),
        hInstance: module,
        lpszClassName: class.as_ptr(),
        ..std::mem::zeroed()
    };
    if RegisterClassW(&wc) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
        let _ = tx.send(Err("Could not register the keyboard input window.".into()));
        return;
    }
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        class.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        std::ptr::null_mut(),
        module,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        let _ = tx.send(Err("Could not create the keyboard input window.".into()));
        return;
    }
    INPUT.with(|slot| {
        *slot.borrow_mut() = Some(Input {
            state: State::new(mode, active, down),
            shared: shared.clone(),
            started: driver.started,
        })
    });
    let mut resources = Resources {
        hwnd,
        module,
        hook: std::ptr::null_mut(),
    };
    let mut installation = Installation::start(&mut resources);
    let startup_ready = installation.is_some()
        && INPUT.with(|slot| {
            let mut slot = slot.borrow_mut();
            let input = slot.as_mut().unwrap();
            let mut pressed = std::collections::HashSet::new();
            for (code, name) in names.iter().enumerate() {
                let down = GetAsyncKeyState(code as i32) < 0;
                input.state.refresh_key(code, down);
                if let Some(name) = name {
                    if down {
                        pressed.insert(name.clone());
                    }
                }
            }
            let mut core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
            let blocked = pressed.iter().any(|name| {
                name == "Escape" || (mode == Mode::Active && core.mappings.contains_key(name))
            });
            core.reconcile_pressed_keys(pressed);
            !blocked
        });
    let worker = if startup_ready {
        let state = shared.clone();
        let events = driver.clone();
        let id = GetCurrentThreadId();
        std::thread::Builder::new()
            .name("switchify-keyboard-events".into())
            .spawn(move || {
                while !state.shutdown.load(Acquire) {
                    state.dispatch(&events, &names);
                    if !state.enabled.load(Acquire) && !state.has_owned() {
                        state.dispatch(&events, &names);
                        unsafe {
                            PostThreadMessageW(id, WM_QUIT, 0, 0);
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            })
            .ok()
    } else {
        None
    };
    if worker.is_none() || shared.shutdown.load(Acquire) {
        let _ = tx.send(Err("Could not start native keyboard suppression.".into()));
    } else if tx.send(Ok(GetCurrentThreadId())).is_ok() {
        let mut msg = std::mem::zeroed();
        while !shared.shutdown.load(Acquire) {
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    shared.shutdown.store(true, Release);
    let cleaned = installation.as_mut().is_none_or(Installation::stop);
    DestroyWindow(hwnd);
    INPUT.with(|slot| *slot.borrow_mut() = None);
    if let Some(worker) = worker {
        let _ = worker.join();
    }
    if shared.enabled.swap(false, AcqRel) || !cleaned {
        driver.lost();
    }
}
