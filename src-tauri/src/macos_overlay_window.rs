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
    window.setCollectionBehavior(OVERLAY_COLLECTION_BEHAVIOR);
}

fn overlay_window_level() -> NSWindowLevel {
    NSScreenSaverWindowLevel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlays_use_the_screen_saver_window_level() {
        assert_eq!(overlay_window_level(), NSScreenSaverWindowLevel);
    }

    #[test]
    fn overlay_collection_behavior_is_complete_and_exact() {
        let expected = NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        assert_eq!(OVERLAY_COLLECTION_BEHAVIOR, expected);
    }
}
