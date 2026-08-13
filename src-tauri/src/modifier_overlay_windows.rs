use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Transform};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GetDC,
    GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_QUALITY, DIB_RGB_COLORS, DT_CENTER,
    DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, HGDIOBJ, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, PeekMessageW,
    RegisterClassW, ShowWindow, TranslateMessage, UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW, MSG,
    PM_REMOVE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WINDOW_EX_STYLE, WM_QUIT, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::state::{emit_state, set_activity, ActivityKind, SharedModel};

const PANEL_HEIGHT: f64 = 70.0;
const PADDING: f64 = 16.0;
const CHIP_HEIGHT: f64 = 38.0;
const CHIP_GAP: f64 = 10.0;
const CHIP_RADIUS: f64 = 8.0;
const MARGIN: f64 = 16.0;
const FONT_SIZE: f64 = 13.0;
const PANEL_COLOR: [u8; 3] = [0x1f, 0x1f, 0x23];
const CHIP_COLOR: [u8; 3] = [0xd3, 0x2f, 0x2f];

enum Command {
    SetSnapshot { revision: u64, labels: Vec<String> },
    Shutdown,
}

pub(super) struct WindowsModifierOverlay {
    sender: Sender<Command>,
}

impl WindowsModifierOverlay {
    pub(super) fn spawn(app: AppHandle, shared: SharedModel) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("Switchify modifier overlay".into())
            .spawn(move || {
                let host = NativeHost::new();
                match host {
                    Ok(mut host) => {
                        let _ = ready_sender.send(Ok(()));
                        run_loop(&mut host, receiver, |error| {
                            report_failure(&app, &shared, error);
                        });
                    }
                    Err(error) => {
                        let _ = ready_sender.send(Err(error.clone()));
                        report_failure(&app, &shared, &error);
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        ready_receiver.recv().map_err(|_| {
            "the native modifier overlay thread stopped during startup".to_string()
        })??;
        Ok(Self { sender })
    }

    pub(super) fn set_snapshot(&self, revision: u64, labels: Vec<String>) -> Result<(), String> {
        self.send(Command::SetSnapshot { revision, labels })
    }

    fn send(&self, command: Command) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|_| "the native modifier overlay is unavailable".to_string())
    }
}

impl Drop for WindowsModifierOverlay {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
    }
}

fn report_failure(app: &AppHandle, shared: &SharedModel, error: &str) {
    set_activity(
        shared,
        ActivityKind::Error,
        format!("Modifier overlay was disabled: {error}"),
    );
    emit_state(app, shared);
}

trait OverlayHost {
    fn render(&mut self, labels: &[String]) -> Result<(), String>;
    fn hide(&mut self);
    fn pump_messages(&mut self) -> bool;
}

fn run_loop<H: OverlayHost>(
    host: &mut H,
    receiver: Receiver<Command>,
    mut report: impl FnMut(&str),
) {
    let mut latest_revision = 0;
    loop {
        if !host.pump_messages() {
            host.hide();
            return;
        }
        match receiver.recv_timeout(Duration::from_millis(16)) {
            Ok(Command::SetSnapshot { revision, labels }) => {
                if revision < latest_revision {
                    continue;
                }
                latest_revision = revision;
                let result = if labels.is_empty() {
                    host.hide();
                    Ok(())
                } else {
                    host.render(&labels)
                };
                if let Err(error) = result {
                    host.hide();
                    report(&error);
                }
            }
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                host.hide();
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

struct NativeHost {
    window: HWND,
}

impl NativeHost {
    fn new() -> Result<Self, String> {
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
            let class = w!("SwitchifyModifierOverlayNative");
            let registration = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            if RegisterClassW(&registration) == 0 {
                // The class can already exist after a development runtime restart.
            }
            let styles = native_window_styles();
            let window = CreateWindowExW(
                styles,
                class,
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|error| error.to_string())?;
            Ok(Self { window })
        }
    }
}

impl OverlayHost for NativeHost {
    fn render(&mut self, labels: &[String]) -> Result<(), String> {
        unsafe {
            let mut cursor = POINT::default();
            GetCursorPos(&mut cursor).map_err(|error| error.to_string())?;
            let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return Err("the active display could not be resolved".into());
            }
            let mut dpi_x = 96;
            let mut dpi_y = 96;
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            let scale = valid_scale(f64::from(dpi_x) / 96.0);
            let layout = layout(labels, scale, info.rcWork)?;
            present(self.window, labels, &layout)?;
            Ok(())
        }
    }

    fn hide(&mut self) {
        unsafe {
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }

    fn pump_messages(&mut self) -> bool {
        pump_window_messages()
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn pump_window_messages() -> bool {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            if message.message == WM_QUIT {
                return false;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    true
}

fn native_window_styles() -> WINDOW_EX_STYLE {
    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
}

#[derive(Debug, Clone, PartialEq)]
struct Layout {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale: f64,
    chips: Vec<RECT>,
}

fn valid_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn logical_chip_width(label: &str) -> f64 {
    match label {
        "Ctrl" => 68.0,
        "Alt" => 60.0,
        "Shift" | "Start" => 74.0,
        _ => 68.0,
    }
}

fn layout(labels: &[String], scale: f64, work_area: RECT) -> Result<Layout, String> {
    if labels.is_empty() {
        return Err("modifier labels are required".into());
    }
    let scale = valid_scale(scale);
    let px = |value: f64| (value * scale).round() as i32;
    let padding = px(PADDING);
    let gap = px(CHIP_GAP);
    let height = px(PANEL_HEIGHT);
    let mut x = padding;
    let mut chips = Vec::with_capacity(labels.len());
    for label in labels {
        let width = px(logical_chip_width(label));
        chips.push(RECT {
            left: x,
            top: padding,
            right: x + width,
            bottom: padding + px(CHIP_HEIGHT),
        });
        x += width + gap;
    }
    let width = x - gap + padding;
    let margin = px(MARGIN);
    Ok(Layout {
        x: work_area.right - width - margin,
        y: work_area.top + margin,
        width,
        height,
        scale,
        chips,
    })
}

fn present(window: HWND, labels: &[String], layout: &Layout) -> Result<(), String> {
    let mut pixmap = Pixmap::new(layout.width as u32, layout.height as u32)
        .ok_or_else(|| "the modifier overlay bitmap could not be created".to_string())?;
    pixmap.fill(Color::from_rgba8(
        PANEL_COLOR[0],
        PANEL_COLOR[1],
        PANEL_COLOR[2],
        255,
    ));
    let mut paint = Paint::default();
    paint.set_color_rgba8(CHIP_COLOR[0], CHIP_COLOR[1], CHIP_COLOR[2], 255);
    for chip in &layout.chips {
        let rect = Rect::from_ltrb(
            chip.left as f32,
            chip.top as f32,
            chip.right as f32,
            chip.bottom as f32,
        )
        .ok_or_else(|| "the modifier overlay chip geometry is invalid".to_string())?;
        let radius = (CHIP_RADIUS * layout.scale) as f32;
        let path = rounded_rect(rect, radius)?;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    present_pixmap_with_text(window, labels, layout, pixmap.data())
}

fn rounded_rect(rect: Rect, radius: f32) -> Result<tiny_skia::Path, String> {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let k = radius * 0.552_284_8;
    let mut path = PathBuilder::new();
    path.move_to(rect.left() + radius, rect.top());
    path.line_to(rect.right() - radius, rect.top());
    path.cubic_to(
        rect.right() - radius + k,
        rect.top(),
        rect.right(),
        rect.top() + radius - k,
        rect.right(),
        rect.top() + radius,
    );
    path.line_to(rect.right(), rect.bottom() - radius);
    path.cubic_to(
        rect.right(),
        rect.bottom() - radius + k,
        rect.right() - radius + k,
        rect.bottom(),
        rect.right() - radius,
        rect.bottom(),
    );
    path.line_to(rect.left() + radius, rect.bottom());
    path.cubic_to(
        rect.left() + radius - k,
        rect.bottom(),
        rect.left(),
        rect.bottom() - radius + k,
        rect.left(),
        rect.bottom() - radius,
    );
    path.line_to(rect.left(), rect.top() + radius);
    path.cubic_to(
        rect.left(),
        rect.top() + radius - k,
        rect.left() + radius - k,
        rect.top(),
        rect.left() + radius,
        rect.top(),
    );
    path.close();
    path.finish()
        .ok_or_else(|| "the modifier overlay chip path is invalid".into())
}

fn present_pixmap_with_text(
    window: HWND,
    labels: &[String],
    layout: &Layout,
    rgba: &[u8],
) -> Result<(), String> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err("the screen drawing context is unavailable".into());
        }
        let memory = CreateCompatibleDC(Some(screen));
        if memory.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return Err("the modifier overlay drawing context is unavailable".into());
        }
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: layout.width,
                biHeight: -layout.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let bitmap = CreateDIBSection(
            Some(screen),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
        .map_err(|error| error.to_string())?;
        let old_bitmap = SelectObject(memory, HGDIOBJ(bitmap.0));
        let output = std::slice::from_raw_parts_mut(bits.cast::<u8>(), rgba.len());
        copy_rgba_to_bgra(rgba, output)?;

        let font_height = -((FONT_SIZE * layout.scale).round() as i32).max(1);
        let font = CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        );
        if font.is_invalid() {
            SelectObject(memory, old_bitmap);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory);
            let _ = ReleaseDC(None, screen);
            return Err("the modifier overlay font could not be created".into());
        }
        let old_font = SelectObject(memory, HGDIOBJ(font.0));
        SetBkMode(memory, TRANSPARENT);
        SetTextColor(memory, COLORREF(0x00ff_ffff));
        for (label, chip) in labels.iter().zip(&layout.chips) {
            let mut text = label.encode_utf16().collect::<Vec<_>>();
            let mut text_rect = *chip;
            let _ = DrawTextW(
                memory,
                &mut text,
                &mut text_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }

        let destination = POINT {
            x: layout.x,
            y: layout.y,
        };
        let size = SIZE {
            cx: layout.width,
            cy: layout.height,
        };
        let source = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let result = UpdateLayeredWindow(
            window,
            Some(screen),
            Some(&destination),
            Some(&size),
            Some(memory),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        SelectObject(memory, old_font);
        SelectObject(memory, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory);
        let _ = ReleaseDC(None, screen);
        result.map_err(|error| error.to_string())?;
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        Ok(())
    }
}

fn copy_rgba_to_bgra(source: &[u8], target: &mut [u8]) -> Result<(), String> {
    if source.len() != target.len() || !source.len().is_multiple_of(4) {
        return Err("the modifier overlay pixel buffer has an invalid length".into());
    }
    for (source, target) in source.chunks_exact(4).zip(target.chunks_exact_mut(4)) {
        target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, IsWindowVisible, SendMessageTimeoutW, GWL_EXSTYLE, SMTO_ABORTIFHUNG,
        WM_NULL,
    };

    #[derive(Default)]
    struct FakeHost {
        events: Vec<String>,
        pumping: bool,
    }

    impl OverlayHost for FakeHost {
        fn render(&mut self, labels: &[String]) -> Result<(), String> {
            self.events.push(format!("show:{}", labels.join(",")));
            Ok(())
        }

        fn hide(&mut self) {
            self.events.push("hide".into());
        }

        fn pump_messages(&mut self) -> bool {
            self.pumping
        }
    }

    #[test]
    fn command_loop_renders_latest_updates_and_hides_on_cleanup() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Command::SetSnapshot {
                revision: 1,
                labels: vec!["Ctrl".into(), "Shift".into()],
            })
            .unwrap();
        sender
            .send(Command::SetSnapshot {
                revision: 3,
                labels: vec!["Start".into()],
            })
            .unwrap();
        sender
            .send(Command::SetSnapshot {
                revision: 2,
                labels: vec!["Alt".into()],
            })
            .unwrap();
        sender
            .send(Command::SetSnapshot {
                revision: 4,
                labels: Vec::new(),
            })
            .unwrap();
        sender.send(Command::Shutdown).unwrap();
        let mut host = FakeHost {
            pumping: true,
            ..FakeHost::default()
        };
        run_loop(&mut host, receiver, |_| {});
        assert_eq!(
            host.events,
            ["show:Ctrl,Shift", "show:Start", "hide", "hide"]
        );
    }

    #[test]
    fn layout_scales_and_supports_negative_monitor_origins() {
        let labels = vec!["Ctrl".into(), "Shift".into()];
        let normal = layout(
            &labels,
            1.0,
            RECT {
                left: -1920,
                top: 24,
                right: 0,
                bottom: 1080,
            },
        )
        .unwrap();
        assert_eq!((normal.width, normal.height), (184, 70));
        assert_eq!((normal.x, normal.y), (-200, 40));
        assert_eq!(
            normal.chips[0],
            RECT {
                left: 16,
                top: 16,
                right: 84,
                bottom: 54
            }
        );

        let scaled = layout(
            &labels,
            2.0,
            RECT {
                left: 0,
                top: 48,
                right: 3024,
                bottom: 1964,
            },
        )
        .unwrap();
        assert_eq!((scaled.width, scaled.height), (368, 140));
        assert_eq!((scaled.x, scaled.y), (2624, 80));
    }

    #[test]
    fn native_window_is_hidden_and_uses_overlay_styles() {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (stop_sender, stop_receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut host = NativeHost::new().unwrap();
            ready_sender.send(host.window.0 as isize).unwrap();
            while stop_receiver.try_recv().is_err() {
                assert!(host.pump_messages());
                thread::sleep(Duration::from_millis(2));
            }
            host.hide();
        });
        let raw = ready_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let window = HWND(raw as *mut c_void);
        unsafe {
            assert!(!IsWindowVisible(window).as_bool());
            let styles = WINDOW_EX_STYLE(GetWindowLongPtrW(window, GWL_EXSTYLE) as u32);
            for required in [
                WS_EX_LAYERED,
                WS_EX_TRANSPARENT,
                WS_EX_TOPMOST,
                WS_EX_TOOLWINDOW,
                WS_EX_NOACTIVATE,
            ] {
                assert!(styles.contains(required));
            }
            let mut response = 0;
            SendMessageTimeoutW(
                window,
                WM_NULL,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                1_000,
                Some(&mut response),
            );
        }
        stop_sender.send(()).unwrap();
        thread.join().unwrap();
    }

    #[test]
    fn pixel_conversion_preserves_channels_and_alpha() {
        let source = [0xd3, 0x2f, 0x2f, 0xff, 0x1f, 0x1f, 0x23, 0xff];
        let mut target = [0; 8];
        copy_rgba_to_bgra(&source, &mut target).unwrap();
        assert_eq!(target, [0x2f, 0x2f, 0xd3, 0xff, 0x23, 0x1f, 0x1f, 0xff]);
    }
}
