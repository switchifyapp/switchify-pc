//! The tray and menu bar icon: the Switchify mark on its own, in one colour,
//! so it sits with the system's other status icons.
//!
//! macOS tints a template image itself. Windows does not, so the mark is drawn
//! white for a dark taskbar and black for a light one, and redrawn when the
//! Windows mode changes.

use tauri::image::Image;

// The mark in a unit square: two dots joined by a curve that leaves the lower
// dot sideways and arrives at the upper dot from below.
const LOWER_DOT: (f32, f32) = (0.22, 0.78);
const UPPER_DOT: (f32, f32) = (0.78, 0.22);
const CURVE_CONTROL: (f32, f32) = (0.78, 0.78);
const DOT_RADIUS: f32 = 0.15;
const CURVE_HALF_WIDTH: f32 = 0.055;
const CURVE_SEGMENTS: usize = 24;

/// Draws the mark as straight-alpha RGBA, `size` pixels square, in one colour
/// on a transparent background.
pub fn render(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let scale = size as f32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let point = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            // Distance to the edge of the mark in pixels, negative inside it;
            // half a pixel either side of the edge blends for a smooth outline.
            let coverage = (0.5 - distance_to_mark(point) * scale).clamp(0.0, 1.0);
            rgba.extend_from_slice(&rgb);
            rgba.push((coverage * 255.0).round() as u8);
        }
    }
    rgba
}

fn distance_to_mark(point: (f32, f32)) -> f32 {
    let dots =
        (distance(point, LOWER_DOT) - DOT_RADIUS).min(distance(point, UPPER_DOT) - DOT_RADIUS);
    let mut curve = f32::MAX;
    let mut from = LOWER_DOT;
    for step in 1..=CURVE_SEGMENTS {
        let to = curve_point(step as f32 / CURVE_SEGMENTS as f32);
        curve = curve.min(distance_to_segment(point, from, to));
        from = to;
    }
    dots.min(curve - CURVE_HALF_WIDTH)
}

fn curve_point(t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    (
        u * u * LOWER_DOT.0 + 2.0 * u * t * CURVE_CONTROL.0 + t * t * UPPER_DOT.0,
        u * u * LOWER_DOT.1 + 2.0 * u * t * CURVE_CONTROL.1 + t * t * UPPER_DOT.1,
    )
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn distance_to_segment(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let along = (to.0 - from.0, to.1 - from.1);
    let length_squared = along.0 * along.0 + along.1 * along.1;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((point.0 - from.0) * along.0 + (point.1 - from.1) * along.1) / length_squared)
            .clamp(0.0, 1.0)
    };
    distance(point, (from.0 + along.0 * t, from.1 + along.1 * t))
}

/// White on a dark taskbar, black on a light one.
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn colour_for_taskbar(light: bool) -> [u8; 3] {
    if light {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

/// The icon to start the tray with.
pub fn image() -> Image<'static> {
    #[cfg(target_os = "windows")]
    {
        windows::image(windows::taskbar_is_light())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 18 points at 2x: the height the menu bar draws a status icon. The
        // colour is ignored for a template image; only the alpha is used.
        const SIZE: u32 = 36;
        Image::new_owned(render(SIZE, [0, 0, 0]), SIZE, SIZE)
    }
}

/// Whether the system recolours the icon itself.
pub fn is_template() -> bool {
    cfg!(target_os = "macos")
}

/// Keeps the icon matched to the taskbar while the app runs. macOS needs
/// nothing here: the menu bar retints a template image on its own.
pub fn follow_system(app: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    windows::follow_taskbar(app.clone());
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{colour_for_taskbar, render};
    use std::time::Duration;
    use tauri::image::Image;
    use windows::Win32::System::Registry::{
        RegNotifyChangeKeyValue, HKEY, REG_NOTIFY_CHANGE_LAST_SET,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    // The taskbar follows the Windows mode, not the app mode.
    const SYSTEM_LIGHT: &str = "SystemUsesLightTheme";

    fn personalize() -> std::io::Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(PERSONALIZE, KEY_READ)
    }

    /// A missing value means a Windows that predates the light taskbar.
    pub fn taskbar_is_light() -> bool {
        personalize()
            .and_then(|key| key.get_value::<u32, _>(SYSTEM_LIGHT))
            .map(|value| value != 0)
            .unwrap_or(false)
    }

    pub fn image(light: bool) -> Image<'static> {
        // SAFETY: GetSystemMetrics takes an index and only reads system state.
        let small_icon = unsafe { GetSystemMetrics(SM_CXSMICON) };
        let size = small_icon.clamp(16, 64) as u32;
        Image::new_owned(render(size, colour_for_taskbar(light)), size, size)
    }

    pub fn follow_taskbar(app: tauri::AppHandle) {
        let spawned = std::thread::Builder::new()
            .name("tray-icon-theme".into())
            .spawn(move || {
                let Ok(key) = personalize() else {
                    return;
                };
                let mut light = taskbar_is_light();
                loop {
                    // SAFETY: `key` is an open registry key that outlives the
                    // call, which blocks until a value under it changes.
                    let waited = unsafe {
                        RegNotifyChangeKeyValue(
                            HKEY(key.raw_handle() as _),
                            false,
                            REG_NOTIFY_CHANGE_LAST_SET,
                            None,
                            false,
                        )
                    };
                    if waited.is_err() {
                        return;
                    }
                    // Switching mode writes several values in a row.
                    std::thread::sleep(Duration::from_millis(250));
                    let now = taskbar_is_light();
                    if now == light {
                        continue;
                    }
                    light = now;
                    if let Some(tray) = app.tray_by_id(crate::TRAY_ID) {
                        let _ = tray.set_icon(Some(image(light)));
                    }
                }
            });
        // Without the watcher the icon keeps the colour it started with.
        drop(spawned);
    }
}

#[cfg(test)]
mod tests {
    use super::{colour_for_taskbar, render, LOWER_DOT, UPPER_DOT};

    fn alpha(rgba: &[u8], size: u32, point: (f32, f32)) -> u8 {
        let x = (point.0 * size as f32) as u32;
        let y = (point.1 * size as f32) as u32;
        rgba[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn mark_is_solid_on_the_dots_and_clear_in_the_empty_corners() {
        for size in [16, 20, 24, 32, 36, 64] {
            let rgba = render(size, [255, 255, 255]);
            assert_eq!(rgba.len(), (size * size * 4) as usize);
            assert_eq!(alpha(&rgba, size, LOWER_DOT), 255, "lower dot at {size}");
            assert_eq!(alpha(&rgba, size, UPPER_DOT), 255, "upper dot at {size}");
            assert_eq!(alpha(&rgba, size, (0.1, 0.1)), 0, "top left at {size}");
            assert_eq!(
                alpha(&rgba, size, (0.95, 0.95)),
                0,
                "bottom right at {size}"
            );
        }
    }

    #[test]
    fn curve_joins_the_dots() {
        // The middle of the curve, well clear of both dots.
        let size = 32;
        let rgba = render(size, [0, 0, 0]);
        assert_eq!(alpha(&rgba, size, (0.64, 0.64)), 255);
    }

    #[test]
    fn mark_stays_inside_the_canvas() {
        let size = 32;
        let rgba = render(size, [0, 0, 0]);
        for i in 0..size {
            for (x, y) in [(i, 0), (i, size - 1), (0, i), (size - 1, i)] {
                assert_eq!(rgba[((y * size + x) * 4 + 3) as usize], 0, "edge {x},{y}");
            }
        }
    }

    #[test]
    fn every_pixel_carries_the_one_colour() {
        let rgba = render(24, [12, 34, 56]);
        assert!(rgba.chunks_exact(4).all(|pixel| pixel[..3] == [12, 34, 56]));
    }

    #[test]
    fn colour_contrasts_with_the_taskbar() {
        assert_eq!(colour_for_taskbar(false), [255, 255, 255]);
        assert_eq!(colour_for_taskbar(true), [0, 0, 0]);
    }
}
