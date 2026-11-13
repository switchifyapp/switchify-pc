use objc2_app_kit::{
    NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWindowLevel,
};

const OVERLAY_COLLECTION_BEHAVIOR: NSWindowCollectionBehavior = NSWindowCollectionBehavior(
    NSWindowCollectionBehavior::CanJoinAllSpaces.0
        | NSWindowCollectionBehavior::Stationary.0
        | NSWindowCollectionBehavior::IgnoresCycle.0
        | NSWindowCollectionBehavior::FullScreenAuxiliary.0,
);

pub(crate) fn configure(window: &NSWindow) {
    window.setLevel(overlay_window_level());
    window.setHidesOnDeactivate(false);
    // Trying a panel hides the app to bring the previous app forward; the
    // scan's own windows must stay on screen through that.
    window.setCanHide(false);
    window.setCollectionBehavior(OVERLAY_COLLECTION_BEHAVIOR);
}

fn overlay_window_level() -> NSWindowLevel {
    NSScreenSaverWindowLevel
}
