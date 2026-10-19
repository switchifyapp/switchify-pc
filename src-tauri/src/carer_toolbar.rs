//! A small toolbar beside each scanning panel that a carer can click with a
//! mouse: open Settings, pause or resume scanning, or close the panel. It is
//! not part of scanning, so switch users scan exactly as before. Unlike the
//! panels it accepts clicks, but it never takes focus. Windows only for now.
// Only the Windows window and the tests use the layout.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use crate::scanning::Rect;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Settings,
    Pause,
    Close,
}

pub const BUTTONS: [Button; 3] = [Button::Settings, Button::Pause, Button::Close];

impl Button {
    pub fn label(self, paused: bool) -> &'static str {
        match self {
            Self::Settings => "Settings",
            Self::Pause if paused => "Resume",
            Self::Pause => "Pause",
            Self::Close => "Close panel",
        }
    }
}

/// Sizes in logical pixels, multiplied by the display scale.
const HEIGHT: f64 = 34.0;
const PADDING: f64 = 4.0;
const BUTTON_WIDTH: f64 = 96.0;
const PANEL_GAP: f64 = 6.0;

/// Where the toolbar and its buttons are, in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub rect: Rect,
    /// Relative to `rect`.
    pub buttons: [(Button, Rect); 3],
}

impl Layout {
    /// Places the toolbar on the side of the panel with room in `area`,
    /// above it unless the panel is at the top, aligned to its right edge.
    pub fn new(panel: Rect, area: Rect, scale: f64) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let padding = (PADDING * scale).round();
        let button_width = (BUTTON_WIDTH * scale).round();
        let height = (HEIGHT * scale).round();
        let gap = (PANEL_GAP * scale).round();
        let width = padding * 2.0 + button_width * 3.0 + padding * 2.0;
        let right = area.x + area.width;
        let bottom = area.y + area.height;
        let x = (panel.x + panel.width - width)
            .min(right - width)
            .max(area.x);
        let above = panel.y - gap - height;
        let y = if above >= area.y {
            above
        } else {
            (panel.y + panel.height + gap).min(bottom - height)
        };
        let buttons = std::array::from_fn(|index| {
            (
                BUTTONS[index],
                Rect {
                    x: padding + index as f64 * (button_width + padding),
                    y: padding,
                    width: button_width,
                    height: height - padding * 2.0,
                },
            )
        });
        Self {
            rect: Rect {
                x: x.round(),
                y: y.round(),
                width,
                height,
            },
            buttons,
        }
    }

    /// The button under a point given relative to the toolbar.
    pub fn hit(&self, x: f64, y: f64) -> Option<Button> {
        self.buttons
            .iter()
            .find(|(_, rect)| {
                x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
            })
            .map(|(button, _)| *button)
    }

    /// Whether the toolbar is clear of the panel. A panel that fills the
    /// height of the display leaves no room, and the toolbar must never
    /// cover its keys.
    pub fn clear_of(&self, panel: Rect) -> bool {
        !overlaps(self.rect, panel)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect.x
            && x < self.rect.x + self.rect.width
            && y >= self.rect.y
            && y < self.rect.y + self.rect.height
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

static ENABLED: AtomicBool = AtomicBool::new(true);

/// Follows the "Show carer toolbar" setting. The next scan tick shows or
/// hides the toolbar to match.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Runs a button's action on the main thread, outside the window procedure
/// and the scan tick, so it never re-enters either.
fn perform(app: &tauri::AppHandle, button: Button) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || match button {
        Button::Settings => crate::show_main_window(&handle, Some("settings")),
        Button::Pause => crate::scanning_runtime::toggle_pause(&handle),
        Button::Close => crate::scanning_runtime::close_panel(&handle),
    });
}

#[cfg(target_os = "windows")]
pub use platform::{hide, install, show, step_aside};

#[cfg(not(target_os = "windows"))]
pub fn install(_: &tauri::AppHandle) {}
#[cfg(not(target_os = "windows"))]
pub fn show(_: Rect, _: bool, _: bool) -> Result<(), String> {
    Ok(())
}
#[cfg(not(target_os = "windows"))]
pub fn hide() {}
#[cfg(not(target_os = "windows"))]
pub fn step_aside() {}

#[cfg(target_os = "windows")]
mod platform {
    use super::{perform, Button, Layout};
    use crate::scanning::Rect;
    use std::cell::RefCell;
    use std::sync::OnceLock;
    use tauri::AppHandle;
    use windows::{
        core::w,
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
            Graphics::Gdi::{
                GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
                Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT},
                WindowsAndMessaging::*,
            },
        },
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    /// In `UI::Controls`, a windows feature this crate does not otherwise need.
    const WM_MOUSELEAVE: u32 = 0x02a3;

    struct Toolbar {
        window: HWND,
        /// What is on screen, so an unchanged frame is not presented again.
        shown: Option<(Layout, bool, f64)>,
        hovered: Option<Button>,
        pressed: Option<Button>,
        tracking: bool,
    }

    thread_local! {
        static TOOLBAR: RefCell<Option<Toolbar>> = const { RefCell::new(None) };
        /// Until when the toolbar keeps out of the pointer's way after a scan
        /// click or scroll. SendInput only queues the event, so the toolbar
        /// must not return the moment the action is sent.
        static ASIDE_UNTIL: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
    }

    const ASIDE: std::time::Duration = std::time::Duration::from_millis(250);

    pub fn install(app: &AppHandle) {
        let _ = APP.set(app.clone());
    }

    /// The work area and scale of the display the panel is on.
    fn display(panel: Rect) -> (Rect, f64) {
        let rect = RECT {
            left: panel.x as i32,
            top: panel.y as i32,
            right: (panel.x + panel.width) as i32,
            bottom: (panel.y + panel.height) as i32,
        };
        unsafe {
            let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let work = if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let r = info.rcWork;
                Rect {
                    x: r.left as f64,
                    y: r.top as f64,
                    width: (r.right - r.left) as f64,
                    height: (r.bottom - r.top) as f64,
                }
            } else {
                panel
            };
            let (mut dpi, mut unused) = (96, 96);
            let scale =
                if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused).is_ok() {
                    dpi as f64 / 96.0
                } else {
                    1.0
                };
            (work, scale)
        }
    }

    /// Whether the pointer is over the toolbar at `layout`.
    fn under_pointer(layout: &Layout) -> bool {
        let mut point = windows::Win32::Foundation::POINT::default();
        unsafe { GetCursorPos(&mut point) }.is_ok()
            && layout.contains(point.x.into(), point.y.into())
    }

    /// Shows the toolbar beside the panel with this rectangle. With
    /// `scan_pointer`, scanning is moving the pointer, and the toolbar steps
    /// aside while the pointer is over it so it never catches the scan's
    /// pointer or scrolling.
    pub fn show(panel: Rect, paused: bool, scan_pointer: bool) -> Result<(), String> {
        let (area, scale) = display(panel);
        let layout = Layout::new(panel, area, scale);
        let aside = ASIDE_UNTIL
            .get()
            .is_some_and(|until| std::time::Instant::now() < until);
        if !layout.clear_of(panel) || ((scan_pointer || aside) && under_pointer(&layout)) {
            hide();
            return Ok(());
        }
        TOOLBAR.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(Toolbar {
                    window: create()?,
                    shown: None,
                    hovered: None,
                    pressed: None,
                    tracking: false,
                });
            }
            let toolbar = slot.as_mut().expect("toolbar was just created");
            if toolbar.shown == Some((layout, paused, scale)) {
                return Ok(());
            }
            toolbar.shown = Some((layout, paused, scale));
            present(toolbar)?;
            unsafe {
                SetWindowPos(
                    toolbar.window,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
                .map_err(|error| error.to_string())
            }
        })
    }

    /// Hides the toolbar if the pointer is over it, just before a scan action
    /// is sent there; the next tick shows it again.
    pub fn step_aside() {
        ASIDE_UNTIL.set(Some(std::time::Instant::now() + ASIDE));
        let under = TOOLBAR.with(|slot| {
            slot.try_borrow()
                .ok()
                .and_then(|slot| slot.as_ref().and_then(|toolbar| toolbar.shown))
                .is_some_and(|(layout, ..)| under_pointer(&layout))
        });
        if under {
            hide();
        }
    }

    pub fn hide() {
        TOOLBAR.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                if let Some(toolbar) = slot.as_mut() {
                    if toolbar.shown.take().is_some() {
                        toolbar.hovered = None;
                        toolbar.pressed = None;
                        unsafe {
                            let _ = ShowWindow(toolbar.window, SW_HIDE);
                        }
                    }
                }
            }
        });
    }

    fn present(toolbar: &Toolbar) -> Result<(), String> {
        let Some((layout, paused, scale)) = toolbar.shown else {
            return Ok(());
        };
        crate::modifier_overlay::windows_backend::present_carer_toolbar(
            toolbar.window,
            &layout,
            scale,
            paused,
            toolbar.hovered,
        )
    }

    fn create() -> Result<HWND, String> {
        unsafe {
            let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = w!("SwitchifyCarerToolbar");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: module.into(),
                lpszClassName: class,
                ..Default::default()
            });
            // Layered and topmost like the panels, but not transparent, so it
            // takes clicks; no-activate, so it never takes focus.
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                class,
                w!("Switchify PC carer toolbar"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(module.into()),
                None,
            )
            .map_err(|e| e.to_string())
        }
    }

    fn point(lparam: LPARAM) -> (f64, f64) {
        let x = (lparam.0 & 0xffff) as u16 as i16;
        let y = ((lparam.0 >> 16) & 0xffff) as u16 as i16;
        (x.into(), y.into())
    }

    /// Runs `f` on the toolbar unless it is busy, which only happens if a
    /// message arrives while the toolbar itself is being presented.
    fn with_toolbar<T>(f: impl FnOnce(&mut Toolbar) -> T) -> Option<T> {
        TOOLBAR.with(|slot| {
            let mut slot = slot.try_borrow_mut().ok()?;
            slot.as_mut().map(f)
        })
    }

    fn set_hovered(toolbar: &mut Toolbar, hovered: Option<Button>) {
        if toolbar.hovered != hovered {
            toolbar.hovered = hovered;
            let _ = present(toolbar);
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_SETCURSOR if (lparam.0 & 0xffff) as u32 == HTCLIENT => {
                let over_button =
                    with_toolbar(|toolbar| toolbar.hovered.is_some()).unwrap_or(false);
                let cursor = if over_button { IDC_HAND } else { IDC_ARROW };
                if let Ok(cursor) = unsafe { LoadCursorW(None, cursor) } {
                    unsafe { SetCursor(Some(cursor)) };
                }
                return LRESULT(1);
            }
            WM_MOUSEMOVE => {
                let (x, y) = point(lparam);
                with_toolbar(|toolbar| {
                    if !toolbar.tracking {
                        let mut track = TRACKMOUSEEVENT {
                            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        toolbar.tracking = unsafe { TrackMouseEvent(&mut track) }.is_ok();
                    }
                    let hovered = toolbar.shown.and_then(|(layout, ..)| layout.hit(x, y));
                    set_hovered(toolbar, hovered);
                });
                return LRESULT(0);
            }
            WM_MOUSELEAVE => {
                with_toolbar(|toolbar| {
                    toolbar.tracking = false;
                    toolbar.pressed = None;
                    set_hovered(toolbar, None);
                });
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                let (x, y) = point(lparam);
                with_toolbar(|toolbar| {
                    toolbar.pressed = toolbar.shown.and_then(|(layout, ..)| layout.hit(x, y));
                });
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let (x, y) = point(lparam);
                let clicked = with_toolbar(|toolbar| {
                    let pressed = toolbar.pressed.take();
                    let released = toolbar.shown.and_then(|(layout, ..)| layout.hit(x, y));
                    pressed.filter(|button| Some(*button) == released)
                })
                .flatten();
                if let (Some(button), Some(app)) = (clicked, APP.get()) {
                    perform(app, button);
                }
                return LRESULT(0);
            }
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1040.0,
    };

    fn panel(y: f64) -> Rect {
        Rect {
            x: 600.0,
            y,
            width: 720.0,
            height: 300.0,
        }
    }

    #[test]
    fn toolbar_sits_above_a_bottom_panel_and_below_a_top_one() {
        let layout = Layout::new(panel(700.0), SCREEN, 1.0);
        assert_eq!(layout.rect.y, 700.0 - 6.0 - 34.0);
        // Aligned to the panel's right edge.
        assert_eq!(layout.rect.x + layout.rect.width, 1320.0);
        let layout = Layout::new(panel(10.0), SCREEN, 1.0);
        assert_eq!(layout.rect.y, 10.0 + 300.0 + 6.0);
    }

    #[test]
    fn toolbar_stays_on_the_panels_display() {
        // A display to the left of the main one.
        let left = Rect {
            x: -1280.0,
            y: 0.0,
            width: 1280.0,
            height: 1000.0,
        };
        let layout = Layout::new(
            Rect {
                x: -1280.0,
                y: 0.0,
                width: 200.0,
                height: 1000.0,
            },
            left,
            1.5,
        );
        assert!(layout.rect.x >= left.x);
        assert!(layout.rect.y + layout.rect.height <= left.y + left.height);
        assert_eq!(layout.rect.height, 51.0);
    }

    #[test]
    fn clicks_find_the_button_under_them_and_nothing_between() {
        let layout = Layout::new(panel(700.0), SCREEN, 1.0);
        for (button, rect) in layout.buttons {
            assert_eq!(
                layout.hit(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0),
                Some(button)
            );
        }
        let (_, first) = layout.buttons[0];
        assert_eq!(layout.hit(first.x + first.width + 1.0, first.y + 1.0), None);
        assert_eq!(layout.hit(1.0, 1.0), None);
        assert_eq!(
            layout.buttons.map(|(button, _)| button),
            [Button::Settings, Button::Pause, Button::Close]
        );
    }

    #[test]
    fn toolbar_never_covers_a_panel_that_fills_the_display() {
        let layout = Layout::new(panel(700.0), SCREEN, 1.0);
        assert!(layout.clear_of(panel(700.0)));
        let tall = Rect {
            x: 600.0,
            y: 0.0,
            width: 720.0,
            height: 1040.0,
        };
        assert!(!Layout::new(tall, SCREEN, 1.0).clear_of(tall));
    }

    #[test]
    fn the_pointer_is_over_the_toolbar_only_inside_it() {
        let layout = Layout::new(panel(700.0), SCREEN, 1.0);
        let r = layout.rect;
        assert!(layout.contains(r.x, r.y));
        assert!(layout.contains(r.x + r.width - 1.0, r.y + r.height - 1.0));
        assert!(!layout.contains(r.x + r.width, r.y));
        assert!(!layout.contains(r.x, r.y + r.height));
    }

    #[test]
    fn pause_names_what_it_will_do() {
        assert_eq!(Button::Pause.label(false), "Pause");
        assert_eq!(Button::Pause.label(true), "Resume");
        assert_eq!(Button::Settings.label(true), "Settings");
    }
}
