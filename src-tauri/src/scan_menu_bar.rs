//! The menu bar along the top of every scanned panel. Its one item, Settings,
//! is clicked with the mouse. The native panel hosts report the press and the
//! release here, and the scan tick, which runs on the main thread with the app
//! handle, opens the Settings page. Nothing about the click is stored beyond
//! that it happened.
use std::sync::atomic::{AtomicBool, Ordering};

static PRESSED: AtomicBool = AtomicBool::new(false);
static CLICKED: AtomicBool = AtomicBool::new(false);

/// A native host saw the mouse pressed over a panel's menu bar.
pub fn pressed() {
    PRESSED.store(true, Ordering::SeqCst);
}

/// A native host saw the mouse released over a panel's menu bar. Only a
/// release that follows a press on the bar is a click; a drag that ends over
/// the bar is not.
pub fn released() {
    if PRESSED.swap(false, Ordering::SeqCst) {
        CLICKED.store(true, Ordering::SeqCst);
    }
}

/// Whether the menu bar was clicked since the last call.
pub fn take_click() -> bool {
    CLICKED.swap(false, Ordering::SeqCst)
}

/// Where the menu bar sends the user: the Settings page of the main window.
pub const DESTINATION: &str = "settings";

/// Tests that drive the shared click state take this so they do not see each
/// other's presses.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_after_a_press_is_one_click() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let _ = take_click();
        assert!(!take_click());
        released();
        assert!(!take_click(), "a drag released over the bar is not a click");
        pressed();
        released();
        released();
        assert!(take_click(), "two releases in one tick open settings once");
        assert!(!take_click());
        pressed();
        pressed();
        released();
        assert!(take_click());
    }
}
