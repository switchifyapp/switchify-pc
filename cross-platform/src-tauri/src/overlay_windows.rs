use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;
use std::sync::mpsc::Receiver;
use std::thread;

use tauri::AppHandle;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW,
    MonitorFromPoint, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetCursorPos, RegisterClassW, ShowWindow, UpdateLayeredWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WINDOW_EX_STYLE,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};

use crate::overlay::{render_marker, run_loop, Command, Frame};
use crate::state::{emit_state, set_activity, ActivityKind, SharedModel};

pub(super) fn spawn(app: AppHandle, shared: SharedModel, receiver: Receiver<Command>) {
    thread::Builder::new()
        .name("Switchify cursor overlay".into())
        .spawn(move || {
            let host = match WindowsOverlayHost::new() {
                Ok(host) => host,
                Err(error) => {
                    report_failure(&app, &shared, &error);
                    return;
                }
            };
            let failure_app = app.clone();
            let failure_shared = shared.clone();
            let host = RefCell::new(host);
            run_loop(
                |frame| {
                    let result = host.borrow_mut().render(frame);
                    if let Err(error) = &result {
                        report_failure(&failure_app, &failure_shared, error);
                    }
                    result
                },
                || host.borrow_mut().hide(),
                receiver,
            );
        })
        .expect("cursor overlay thread should start");
}

fn report_failure(app: &AppHandle, shared: &SharedModel, error: &str) {
    set_activity(
        shared,
        ActivityKind::Error,
        format!("Cursor overlay was disabled: {error}"),
    );
    emit_state(app, shared);
}

struct WindowsOverlayHost {
    marker: HWND,
    horizontal: HWND,
    vertical: HWND,
}

impl WindowsOverlayHost {
    fn new() -> Result<Self, String> {
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
            let class = w!("SwitchifyCursorOverlayNative");
            let registration = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            if RegisterClassW(&registration) == 0 {
                // The class may already exist when a development runtime is restarted.
            }
            Ok(Self {
                marker: create_overlay_window(class, instance.into())?,
                horizontal: create_overlay_window(class, instance.into())?,
                vertical: create_overlay_window(class, instance.into())?,
            })
        }
    }

    fn render(&mut self, frame: &Frame) -> Result<(), String> {
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
            let scale = f64::from(dpi_x) / 96.0;
            let pixmap = render_marker(frame, scale);
            let width = pixmap.width() as i32;
            let height = pixmap.height() as i32;
            present_rgba(
                self.marker,
                cursor.x - width / 2,
                cursor.y - height / 2,
                width,
                height,
                pixmap.data(),
            )?;
            if frame.crosshairs {
                let thickness = ((2.0 * scale).round() as i32).max(1);
                present_solid(
                    self.horizontal,
                    info.rcMonitor.left,
                    cursor.y - thickness / 2,
                    info.rcMonitor.right - info.rcMonitor.left,
                    thickness,
                    frame.color,
                    184,
                )?;
                present_solid(
                    self.vertical,
                    cursor.x - thickness / 2,
                    info.rcMonitor.top,
                    thickness,
                    info.rcMonitor.bottom - info.rcMonitor.top,
                    frame.color,
                    184,
                )?;
            } else {
                let _ = ShowWindow(self.horizontal, SW_HIDE);
                let _ = ShowWindow(self.vertical, SW_HIDE);
            }
            Ok(())
        }
    }

    fn hide(&mut self) {
        unsafe {
            let _ = ShowWindow(self.marker, SW_HIDE);
            let _ = ShowWindow(self.horizontal, SW_HIDE);
            let _ = ShowWindow(self.vertical, SW_HIDE);
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

unsafe fn create_overlay_window(
    class: PCWSTR,
    instance: windows::Win32::Foundation::HINSTANCE,
) -> Result<HWND, String> {
    let styles: WINDOW_EX_STYLE =
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    unsafe {
        CreateWindowExW(
            styles,
            class,
            w!(""),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1,
            1,
            None,
            None,
            Some(instance),
            None,
        )
        .map_err(|error| error.to_string())
    }
}

fn present_solid(
    window: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: [u8; 3],
    alpha: u8,
) -> Result<(), String> {
    let pixel_count = (width.max(1) as usize) * (height.max(1) as usize);
    let mut rgba = vec![0_u8; pixel_count * 4];
    let premultiply = |channel: u8| ((u16::from(channel) * u16::from(alpha)) / 255) as u8;
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[
            premultiply(color[0]),
            premultiply(color[1]),
            premultiply(color[2]),
            alpha,
        ]);
    }
    present_rgba(window, x, y, width.max(1), height.max(1), &rgba)
}

fn present_rgba(
    window: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
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
            return Err("the overlay drawing context is unavailable".into());
        }
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
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
        let old = SelectObject(memory, HGDIOBJ(bitmap.0));
        let output = std::slice::from_raw_parts_mut(bits.cast::<u8>(), rgba.len());
        for (source, target) in rgba.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
            target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
        }
        let destination = POINT { x, y };
        let size = SIZE {
            cx: width,
            cy: height,
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
        SelectObject(memory, old);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory);
        let _ = ReleaseDC(None, screen);
        result.map_err(|error| error.to_string())?;
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        Ok(())
    }
}
