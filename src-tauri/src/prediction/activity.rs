//! A content-free input epoch. Never suppresses or records external input.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
static EPOCH: AtomicU64 = AtomicU64::new(0);
static HEALTHY: AtomicBool = AtomicBool::new(false);
static IGNORED: OnceLock<RwLock<Vec<u32>>> = OnceLock::new();
fn ignored() -> &'static RwLock<Vec<u32>> {
    IGNORED.get_or_init(|| RwLock::new(Vec::new()))
}
pub fn set_ignored(mut keys: Vec<u32>) {
    keys.sort_unstable();
    keys.dedup();
    let mut current = ignored().write().unwrap_or_else(|error| error.into_inner());
    if *current != keys {
        *current = keys;
        // An assignment change makes earlier ownership claims unsafe.
        changed();
    }
}
fn ignored_key(code: u32) -> bool {
    ignored()
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .binary_search(&code)
        .is_ok()
}
#[cfg(any(target_os = "macos", test))]
fn first_disable(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::SeqCst)
}
#[cfg(any(target_os = "macos", test))]
fn needs_exit_invalidation(flag: &AtomicBool) -> bool {
    !flag.load(Ordering::SeqCst)
}
pub fn snapshot() -> (u64, bool) {
    (EPOCH.load(Ordering::SeqCst), HEALTHY.load(Ordering::SeqCst))
}
fn unavailable() {
    HEALTHY.store(false, Ordering::SeqCst);
    changed();
}
static LAST_CHANGE: AtomicU64 = AtomicU64::new(0);
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros().min(u64::MAX as u128) as u64)
}
pub fn last_change() -> u64 {
    LAST_CHANGE.load(Ordering::SeqCst)
}
fn changed() {
    LAST_CHANGE.store(now(), Ordering::SeqCst);
    EPOCH.fetch_add(1, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
pub fn start() -> bool {
    use windows_sys::Win32::{System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*};
    unsafe extern "system" fn keyboard(code: i32, wp: usize, lp: isize) -> isize {
        if code >= 0 && matches!(wp as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
            let key = unsafe { &*(lp as *const KBDLLHOOKSTRUCT) };
            if !crate::input::own_input(key.dwExtraInfo as i64) && !ignored_key(key.vkCode) {
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
        // Never trust edits queued before observation was established.
        changed();
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
pub fn start() -> bool {
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        CallbackResult,
    };
    let disabled = std::sync::Arc::new(AtomicBool::new(false));
    let callback_disabled = disabled.clone();
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
                    if first_disable(&callback_disabled) {
                        unavailable();
                    }
                    // Returning from the run loop drops this tap before a retry.
                    core_foundation::runloop::CFRunLoop::get_current().stop();
                    return CallbackResult::Keep;
                }
                if !crate::input::own_input(event.get_integer_value_field(42))
                    && (!matches!(kind, CGEventType::KeyDown)
                        || !ignored_key(event.get_integer_value_field(9) as u32))
                {
                    changed();
                }
                CallbackResult::Keep
            },
            || {
                changed();
                HEALTHY.store(true, Ordering::SeqCst);
                let _ = ready.send(true);
                core_foundation::runloop::CFRunLoop::run_current();
            },
        );
        let _ = tx.send(false);
        if needs_exit_invalidation(&disabled) {
            unavailable();
        }
    });
    rx.recv_timeout(std::time::Duration::from_millis(500))
        .unwrap_or(false)
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn start() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassignment_updates_ignored_keys_and_invalidates_old_context() {
        set_ignored(vec![32]);
        assert!(ignored_key(32));
        let before = snapshot().0;
        set_ignored(vec![120]);
        assert!(snapshot().0 > before);
        assert!(!ignored_key(32));
        assert!(ignored_key(120));
        set_ignored(Vec::new());
    }

    #[test]
    fn disabled_tap_invalidates_once_before_its_run_loop_exits() {
        let disabled = AtomicBool::new(false);
        assert!(needs_exit_invalidation(&disabled));
        assert!(first_disable(&disabled));
        assert!(!first_disable(&disabled));
        assert!(!needs_exit_invalidation(&disabled));
    }
}
