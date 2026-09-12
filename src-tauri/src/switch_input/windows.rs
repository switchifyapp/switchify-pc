//! Windows hotkey presses and Raw Input releases. No low-level hook dependency.
use super::{Driver, Mode};
use anyhow::{bail, Result};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
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
    driver: Driver,
    keys: HashMap<i32, String>,
}
impl Input {
    fn hotkey(&self, id: i32) {
        if let Some(name) = self.keys.get(&id) {
            self.driver.key(name, true);
        }
    }
    fn raw(&self, vk: i32, released: bool) {
        if released {
            if let Some(name) = self.keys.get(&vk) {
                self.driver.key(name, false);
            }
        }
    }
}
thread_local! { static INPUT: RefCell<Option<Input>> = const { RefCell::new(None) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe extern "system" fn window(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_INPUT_DEVICE_CHANGE && wp == GIDC_REMOVAL as usize {
        INPUT.with(|slot| {
            if let Some(input) = slot.borrow().as_ref() {
                input.driver.lost();
                // The removed device may not own a held switch. Off-mode cleanup
                // reconciles actual key state before releasing its reservation.
            }
        });
    }
    if msg == WM_HOTKEY {
        INPUT.with(|slot| {
            if let Some(input) = slot.borrow().as_ref() {
                input.hotkey(wp as i32);
            }
        });
        return 0;
    }
    if msg == WM_INPUT {
        let mut raw: RAWINPUT = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let count = unsafe {
            GetRawInputData(
                lp as HRAWINPUT,
                RID_INPUT,
                &mut raw as *mut _ as *mut _,
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if count == u32::MAX {
            INPUT.with(|slot| {
                if let Some(input) = slot.borrow().as_ref() {
                    input.driver.lost();
                }
            });
        } else if raw.header.dwType == RIM_TYPEKEYBOARD {
            let key = unsafe { raw.data.keyboard };
            // Hotkeys own presses. Raw make events can be repeats or modified keys
            // that we did not reserve, so they must never start a gesture.
            if key.Flags & RI_KEY_BREAK as u16 != 0 {
                INPUT.with(|slot| {
                    if let Some(input) = slot.borrow().as_ref() {
                        input.raw(key.VKey as i32, true);
                    }
                });
            }
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Owns every registration on one dedicated message-loop thread.
pub struct Capture {
    stop: Arc<AtomicBool>,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Capture {
    pub fn start(driver: Driver, mode: Mode) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("switchify-keyboard-input".into())
            .spawn(move || unsafe { run(driver, mode, stopping, tx) })?;
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread_id)) => Ok(Self {
                stop,
                thread_id,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                bail!(error)
            }
            Err(_) => {
                stop.store(true, Ordering::Release);
                bail!("Keyboard capture did not acknowledge startup.")
            }
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
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
    stopping: Arc<AtomicBool>,
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
        let _ = tx.send(Err(
            "Could not register the keyboard input window.".to_string()
        ));
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
        let _ = tx.send(Err(
            "Could not create the keyboard input window.".to_string()
        ));
        return;
    }
    let raw = RAWINPUTDEVICE {
        usUsagePage: 1,
        usUsage: 6,
        dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
        hwndTarget: hwnd,
    };
    let mut registered = HashMap::new();
    let setup = (|| -> Result<()> {
        if RegisterRawInputDevices(&raw, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
            bail!("Could not receive keyboard releases.");
        }
        let keys = if mode == Mode::Learning {
            known_keys()
        } else {
            let c = driver.core.lock().unwrap_or_else(|p| p.into_inner());
            c.mappings
                .keys()
                .chain(std::iter::once(&"Escape".to_string()))
                .map(|n| (code_for_name(n).unwrap(), n.clone()))
                .collect()
        };
        for (vk, name) in keys {
            if RegisterHotKey(hwnd, vk as i32, MOD_NOREPEAT, vk) != 0 {
                registered.insert(vk as i32, name);
            } else if mode == Mode::Active || name == "Escape" {
                bail!("The {name} key could not be reserved. It may already be used by another application.");
            }
        }
        let down = known_keys()
            .into_iter()
            .filter(|(vk, _)| GetAsyncKeyState(*vk as i32) < 0)
            .map(|(_, name)| name)
            .collect();
        driver.ready(down);
        let mut core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
        if !core.down.is_empty() {
            bail!("Release held keys before starting capture.");
        }
        core.begin(mode, driver.now());
        Ok(())
    })();
    let timer = if setup.is_ok() {
        SetTimer(hwnd, 1, 20, None)
    } else {
        0
    };
    let setup = setup.and_then(|()| {
        if timer == 0 {
            bail!("Could not start the keyboard watchdog.");
        }
        Ok(())
    });
    if let Err(e) = setup {
        for id in registered.keys() {
            UnregisterHotKey(hwnd, *id);
        }
        let remove = RAWINPUTDEVICE {
            dwFlags: RIDEV_REMOVE,
            hwndTarget: std::ptr::null_mut(),
            ..raw
        };
        RegisterRawInputDevices(&remove, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        DestroyWindow(hwnd);
        driver.lost();
        let _ = tx.send(Err(e.to_string()));
        return;
    }
    INPUT.with(|slot| {
        *slot.borrow_mut() = Some(Input {
            driver: driver.clone(),
            keys: registered.clone(),
        })
    });
    let mut msg = std::mem::zeroed();
    if tx.send(Ok(GetCurrentThreadId())).is_ok() {
        while !stopping.load(Ordering::Acquire) {
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            driver.tick();
            let mut core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
            if core.status.mode == Mode::Off {
                // Reconcile only after cancellation; a lost raw break must not
                // leave a reservation stuck, and must never execute an action.
                core.physical.retain(|name| {
                    code_for_name(name).is_some_and(|vk| GetAsyncKeyState(vk as i32) < 0)
                });
                // Drain keys already consumed when cancelled, without releasing
                // their autorepeat into the foreground application.
                registered.retain(|id, name| {
                    if core.physical.contains(name) {
                        true
                    } else {
                        UnregisterHotKey(hwnd, *id);
                        false
                    }
                });
                INPUT.with(|slot| {
                    if let Some(input) = slot.borrow_mut().as_mut() {
                        input.keys = registered.clone();
                    }
                });
                if registered.is_empty() {
                    break;
                }
            }
        }
    }
    for id in registered.keys() {
        UnregisterHotKey(hwnd, *id);
    }
    KillTimer(hwnd, timer);
    let remove = RAWINPUTDEVICE {
        dwFlags: RIDEV_REMOVE,
        hwndTarget: std::ptr::null_mut(),
        ..raw
    };
    RegisterRawInputDevices(&remove, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
    DestroyWindow(hwnd);
    INPUT.with(|slot| *slot.borrow_mut() = None);
    let active = driver
        .core
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .status
        .mode
        != Mode::Off;
    if active {
        driver.lost();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::switch_input::{Action, Core, Event, StopReason};
    use std::{sync::Mutex, time::Instant};
    fn input() -> Input {
        let mut core = Core::default();
        core.mappings.insert("Space".into(), "select".into());
        core.begin(Mode::Active, 0);
        Input {
            driver: Driver {
                core: Arc::new(Mutex::new(core)),
                started: Instant::now(),
            },
            keys: HashMap::from([(32, "Space".into()), (27, "Escape".into())]),
        }
    }
    #[test]
    fn raw_make_and_unmatched_break_cannot_start_gestures() {
        let i = input();
        i.raw(32, false);
        i.raw(32, true);
        assert!(i.driver.core.lock().unwrap().events.is_empty());
        i.hotkey(32);
        i.hotkey(32);
        i.raw(32, false);
        i.raw(32, true);
        i.raw(32, true);
        let c = i.driver.core.lock().unwrap();
        assert_eq!(c.events.len(), 2);
        assert!(matches!(
            c.events[0],
            Event::Switch {
                action: Action::Pressed,
                ..
            }
        ));
        assert!(matches!(
            c.events[1],
            Event::Switch {
                action: Action::Released,
                ..
            }
        ));
    }
    #[test]
    fn escape_cancels_pending_action_and_drains_held_switch() {
        let i = input();
        i.hotkey(32);
        i.hotkey(27);
        assert_eq!(
            i.driver.core.lock().unwrap().status.reason,
            Some(StopReason::Escape)
        );
        i.raw(32, true);
        i.raw(27, true);
        let c = i.driver.core.lock().unwrap();
        assert!(c.physical.is_empty());
        assert_eq!(c.events.len(), 1);
        assert!(matches!(c.events[0], Event::Stopped { .. }));
    }
    #[test]
    fn missing_raw_release_cancels_without_inventing_an_action() {
        let i = input();
        i.hotkey(32);
        let mut c = i.driver.core.lock().unwrap();
        c.last_heartbeat = 4000;
        c.tick(4000);
        assert_eq!(c.status.reason, Some(StopReason::HoldEscape));
        assert_eq!(c.events.len(), 1);
    }
    #[test]
    fn learning_uses_complete_hotkey_and_raw_release_pair() {
        let i = input();
        i.driver.core.lock().unwrap().begin(Mode::Learning, 0);
        i.raw(32, true);
        i.hotkey(32);
        i.raw(32, true);
        let c = i.driver.core.lock().unwrap();
        assert_eq!(c.events.len(), 1);
        assert!(matches!(&c.events[0],Event::Learned {code,..} if code=="Space"));
    }
    #[test]
    fn f12_is_not_reservable() {
        assert!(!super::super::supported_key("F12"));
        assert!(!known_keys().iter().any(|(_, name)| name == "F12"));
    }
    #[test]
    fn device_loss_preserves_other_held_keys_until_their_release() {
        let i = input();
        i.hotkey(32);
        i.driver.lost();
        assert!(i.driver.core.lock().unwrap().physical.contains("Space"));
        i.raw(32, true);
        let c = i.driver.core.lock().unwrap();
        assert!(c.physical.is_empty());
        assert_eq!(c.events.len(), 1);
        assert!(matches!(
            c.events[0],
            Event::Stopped {
                reason: StopReason::CaptureLost,
                ..
            }
        ));
    }
}
