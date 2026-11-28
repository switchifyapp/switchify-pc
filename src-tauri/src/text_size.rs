//! Text size of the main window. On Windows the webview already scales the
//! whole page by the system text size, so matching the system needs no zoom,
//! and a fixed size zooms by the ratio that cancels the system text size out.

use tauri::{AppHandle, Manager};

use crate::state::AppModel;

/// Largest system text size Windows offers is 225%.
const LARGEST_SYSTEM_SCALE: f64 = 2.25;

/// The webview zoom for a saved text size, given the system text size the
/// webview already applies. 0, or a size this build does not offer, means
/// match the system.
pub fn zoom_factor(text_size_percent: u16, system: f64) -> f64 {
    let text_size_percent = crate::state::offered_text_size(text_size_percent);
    if text_size_percent == 0 {
        return 1.0;
    }
    let system = if system.is_finite() {
        system.clamp(1.0, LARGEST_SYSTEM_SCALE)
    } else {
        1.0
    };
    f64::from(text_size_percent) / 100.0 / system
}

/// The system text size the webview applies, such as 1.5 for 150%.
#[cfg(target_os = "windows")]
pub fn system_scale() -> f64 {
    windows::UI::ViewManagement::UISettings::new()
        .and_then(|settings| settings.TextScaleFactor())
        .unwrap_or(1.0)
}

/// macOS has no system-wide text size that apps or WKWebView follow.
#[cfg(not(target_os = "windows"))]
pub fn system_scale() -> f64 {
    1.0
}

/// Zooms the main window to the saved text size.
pub fn apply(app: &AppHandle) {
    let percent = app
        .state::<AppModel>()
        .snapshot()
        .settings
        .text_size_percent;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_zoom(zoom_factor(percent, system_scale()));
    }
}

/// Applies the text size now and again whenever the system text size changes,
/// so a fixed size stays fixed.
pub fn install(app: &AppHandle) {
    apply(app);
    #[cfg(target_os = "windows")]
    watch(app);
}

#[cfg(target_os = "windows")]
fn watch(app: &AppHandle) {
    use windows::core::IInspectable;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::ViewManagement::UISettings;

    /// Keeps the settings object, and so its change handler, alive.
    struct Watcher(#[allow(dead_code)] UISettings);

    let Ok(settings) = UISettings::new() else {
        return;
    };
    let handle = app.clone();
    let handler = TypedEventHandler::<UISettings, IInspectable>::new(move |_, _| {
        apply(&handle);
        Ok(())
    });
    if settings.TextScaleFactorChanged(&handler).is_ok() {
        app.manage(Watcher(settings));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_the_system_leaves_the_webview_scaling_alone() {
        assert_eq!(zoom_factor(0, 1.5), 1.0);
        assert_eq!(zoom_factor(0, 1.0), 1.0);
    }

    #[test]
    fn a_fixed_size_cancels_the_system_text_size() {
        assert_eq!(zoom_factor(150, 1.0), 1.5);
        assert_eq!(zoom_factor(150, 1.5), 1.0);
        assert_eq!(zoom_factor(100, 2.0), 0.5);
        assert_eq!(zoom_factor(200, 1.25), 1.6);
    }

    #[test]
    fn an_odd_system_scale_is_kept_in_range() {
        assert_eq!(zoom_factor(100, 0.5), 1.0);
        assert_eq!(zoom_factor(200, 9.0), 2.0 / 2.25);
        assert_eq!(zoom_factor(125, f64::NAN), 1.25);
    }

    #[test]
    fn a_size_this_build_does_not_offer_matches_the_system() {
        assert_eq!(zoom_factor(250, 1.0), 1.0);
        assert_eq!(zoom_factor(1000, 1.5), 1.0);
    }
}
