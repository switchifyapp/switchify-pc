//! Native, nonactivating strips, using the same display units as input injection.
use crate::scanning::Rect;

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::{
        core::w,
        Win32::{
            Foundation::HWND, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
        },
    };
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: windows::Win32::Foundation::WPARAM,
        lparam: windows::Win32::Foundation::LPARAM,
    ) -> windows::Win32::Foundation::LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
    pub struct Host {
        windows: Vec<HWND>,
    }
    impl Host {
        pub fn new() -> Result<Self, String> {
            Ok(Self { windows: vec![] })
        }
        pub fn render(&mut self, rects: &[Rect]) -> Result<(), String> {
            unsafe {
                while self.windows.len() < rects.len() {
                    let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
                    let class = w!("SwitchifyPointScanStrip");
                    RegisterClassW(&WNDCLASSW {
                        lpfnWndProc: Some(window_proc),
                        hInstance: module.into(),
                        lpszClassName: class,
                        ..Default::default()
                    });
                    self.windows.push(
                        CreateWindowExW(
                            WS_EX_LAYERED
                                | WS_EX_TRANSPARENT
                                | WS_EX_NOACTIVATE
                                | WS_EX_TOOLWINDOW
                                | WS_EX_TOPMOST,
                            class,
                            w!(""),
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
                        .map_err(|e| e.to_string())?,
                    );
                }
                for (index, window) in self.windows.iter().enumerate() {
                    if let Some(r) = rects.get(index) {
                        crate::overlay::platform::present_solid(
                            *window,
                            r.x.round() as i32,
                            r.y.round() as i32,
                            r.width.round().max(1.0) as i32,
                            r.height.round().max(1.0) as i32,
                            [255, 196, 0],
                            230,
                        )?;
                    } else {
                        let _ = ShowWindow(*window, SW_HIDE);
                    }
                }
            }
            Ok(())
        }
        pub fn prompt(&mut self, text: &str, rect: Rect, scale: f64) -> Result<(), String> {
            self.render(&[rect])?;
            crate::modifier_overlay::windows_backend::present_scan_prompt(
                self.windows[0],
                text,
                rect.x as i32,
                rect.y as i32,
                rect.width as i32,
                scale,
            )
        }
        pub fn hide(&mut self) {
            for window in &self.windows {
                unsafe {
                    let _ = ShowWindow(*window, SW_HIDE);
                }
            }
        }
    }
    impl Drop for Host {
        fn drop(&mut self) {
            for window in &self.windows {
                unsafe {
                    let _ = DestroyWindow(*window);
                }
            }
        }
    }
}
#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use objc2::{rc::Retained, MainThreadMarker};
    use objc2_app_kit::{NSColor, NSPanel, NSScreen};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    pub struct Host {
        panels: Vec<Retained<NSPanel>>,
    }
    impl Host {
        pub fn new() -> Result<Self, String> {
            Ok(Self { panels: vec![] })
        }
        pub fn render(&mut self, rects: &[Rect]) -> Result<(), String> {
            let mtm =
                MainThreadMarker::new().ok_or("Point scan requires the AppKit main thread.")?;
            let screens = NSScreen::screens(mtm);
            let primary = screens
                .firstObject()
                .ok_or("No scanning display is available.")?;
            let top = primary.frame().origin.y + primary.frame().size.height;
            while self.panels.len() < rects.len() {
                self.panels.push(crate::overlay::platform::make_panel(mtm));
            }
            for (index, panel) in self.panels.iter().enumerate() {
                if let Some(r) = rects.get(index) {
                    panel.setBackgroundColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
                        1.0, 0.77, 0.0, 0.90,
                    )));
                    panel.setFrame_display(
                        NSRect::new(
                            NSPoint::new(r.x, top - r.y - r.height),
                            NSSize::new(r.width, r.height),
                        ),
                        false,
                    );
                    panel.orderFrontRegardless();
                } else {
                    panel.orderOut(None);
                }
            }
            Ok(())
        }
        pub fn prompt(&mut self, text: &str, rect: Rect, scale: f64) -> Result<(), String> {
            use objc2_app_kit::{NSFont, NSTextField};
            use objc2_foundation::NSString;
            let mtm = MainThreadMarker::new().ok_or("Prompt requires the main thread.")?;
            self.render(&[rect])?;
            let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
            label.setFont(Some(&NSFont::systemFontOfSize(20.0 * scale)));
            label.setTextColor(Some(&NSColor::whiteColor()));
            label.setFrame(NSRect::new(
                NSPoint::new(12.0 * scale, 12.0 * scale),
                NSSize::new(
                    (rect.width - 24.0 * scale).max(1.0),
                    (rect.height - 24.0 * scale).max(1.0),
                ),
            ));
            self.panels[0].setBackgroundColor(Some(&NSColor::blackColor()));
            self.panels[0].setContentView(Some(&label));
            Ok(())
        }
        pub fn hide(&mut self) {
            for panel in &self.panels {
                panel.orderOut(None);
            }
        }
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    use super::*;
    pub struct Host;
    impl Host {
        pub fn new() -> Result<Self, String> {
            Err("Point scan is supported on Windows and macOS.".into())
        }
        pub fn render(&mut self, _: &[Rect]) -> Result<(), String> {
            Err("Point scan is unavailable.".into())
        }
        pub fn prompt(&mut self, _: &str, _: Rect, _: f64) -> Result<(), String> {
            Err("Scanning is unavailable.".into())
        }
        pub fn hide(&mut self) {}
    }
}
pub use platform::Host;

pub fn modifiers_released() -> bool {
    #[cfg(target_os = "windows")]
    {
        [0x10, 0x11, 0x12, 0x5B, 0x5C].iter().all(|key| unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(*key) >= 0
        })
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSEvent, NSEventModifierFlags as Flags};
        !NSEvent::modifierFlags_class()
            .intersects(Flags::Shift | Flags::Control | Flags::Option | Flags::Command)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

/// Opaque foreground identity; never emitted or persisted.
pub fn foreground() -> Result<usize, String> {
    #[cfg(target_os = "windows")]
    {
        let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        if hwnd.0.is_null() {
            Err("No foreground window is available.".into())
        } else {
            Ok(hwnd.0 as usize)
        }
    }
    #[cfg(target_os = "macos")]
    {
        objc2_app_kit::NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier() as usize)
            .ok_or("No foreground application is available.".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Local scanning is unavailable.".into())
    }
}
