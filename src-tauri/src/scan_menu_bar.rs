//! The menu bar along the top of every scanned panel. Its one item, Settings,
//! is clicked with the mouse. The native panel hosts report the click here and
//! the scan tick, which runs on the main thread with the app handle, opens the
//! Settings page. Nothing about the click is stored beyond that it happened.
use std::sync::atomic::{AtomicBool, Ordering};

static CLICKED: AtomicBool = AtomicBool::new(false);

/// A native host saw the mouse released over a panel's menu bar.
pub fn clicked() {
    CLICKED.store(true, Ordering::SeqCst);
}

/// Whether the menu bar was clicked since the last call.
pub fn take_click() -> bool {
    CLICKED.swap(false, Ordering::SeqCst)
}

/// Where the menu bar sends the user: the Settings page of the main window.
pub const DESTINATION: &str = "settings";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_is_reported_once() {
        let _ = take_click();
        assert!(!take_click());
        clicked();
        clicked();
        assert!(take_click(), "two clicks in one tick open settings once");
        assert!(!take_click());
    }
}
