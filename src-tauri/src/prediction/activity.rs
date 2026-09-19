//! A content-free input epoch. Never suppresses or records external input.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
static EPOCH: AtomicU64 = AtomicU64::new(0);
static HEALTHY: AtomicBool = AtomicBool::new(false);
pub fn snapshot() -> (u64, bool) {
    (EPOCH.load(Ordering::SeqCst), HEALTHY.load(Ordering::SeqCst))
}
fn unavailable() {
    HEALTHY.store(false, Ordering::SeqCst);
    changed();
}
fn changed() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
pub fn start(ignored: Vec<u32>) -> bool {
    use std::cell::RefCell;
    use windows_sys::Win32::{System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*};
    thread_local! { static IGNORE: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) }; }
    unsafe extern "system" fn keyboard(code: i32, wp: usize, lp: isize) -> isize {
        if code >= 0 && matches!(wp as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
            let key = unsafe { &*(lp as *const KBDLLHOOKSTRUCT) };
            if !crate::input::own_input(key.dwExtraInfo as i64)
                && !IGNORE.with(|i| i.borrow().contains(&key.vkCode))
            {
                changed();
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
    }
    unsafe extern "system" fn mouse(code: i32, wp: usize, lp: isize) -> isize {
        if code >= 0
            && matches!(
                wp as u32,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEWHEEL
            )
        {
            let event = unsafe { &*(lp as *const MSLLHOOKSTRUCT) };
            if !crate::input::own_input(event.dwExtraInfo as i64) {
                changed();
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || unsafe {
        IGNORE.with(|i| *i.borrow_mut() = ignored);
        let k = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard),
            GetModuleHandleW(std::ptr::null()),
            0,
        );
        let m = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse),
            GetModuleHandleW(std::ptr::null()),
            0,
        );
        let ok = !k.is_null() && !m.is_null();
        HEALTHY.store(ok, Ordering::SeqCst);
        let _ = tx.send(ok);
        if ok {
            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if !k.is_null() {
            UnhookWindowsHookEx(k);
        }
        if !m.is_null() {
            UnhookWindowsHookEx(m);
        }
        unavailable();
    });
    rx.recv_timeout(std::time::Duration::from_millis(500))
        .unwrap_or(false)
}
#[cfg(target_os = "macos")]
pub fn start(ignored: Vec<u32>) -> bool {
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        CallbackResult,
    };
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let ready = tx.clone();
        let _ = CGEventTap::with_enabled(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![
                CGEventType::KeyDown,
                CGEventType::LeftMouseDown,
                CGEventType::RightMouseDown,
                CGEventType::OtherMouseDown,
                CGEventType::ScrollWheel,
            ],
            move |_, kind, event| {
                if matches!(
                    kind,
                    CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
                ) {
                    unavailable();
                    return CallbackResult::Keep;
                }
                if !crate::input::own_input(event.get_integer_value_field(42))
                    && (!matches!(kind, CGEventType::KeyDown)
                        || !ignored.contains(&(event.get_integer_value_field(9) as u32)))
                {
                    changed();
                }
                CallbackResult::Keep
            },
            || {
                HEALTHY.store(true, Ordering::SeqCst);
                let _ = ready.send(true);
                core_foundation::runloop::CFRunLoop::run_current();
            },
        );
        let _ = tx.send(false);
        unavailable();
    });
    rx.recv_timeout(std::time::Duration::from_millis(500))
        .unwrap_or(false)
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn start(_: Vec<u32>) -> bool {
    false
}
