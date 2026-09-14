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
        last_rects: Vec<crate::scanning::PaintedRect>,
    }
    impl Host {
        pub fn new() -> Result<Self, String> {
            Ok(Self {
                windows: vec![],
                last_rects: vec![],
            })
        }
        fn ensure_windows(&mut self, count: usize) -> Result<(), String> {
            unsafe {
                while self.windows.len() < count {
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
            }
            Ok(())
        }
        pub fn render(&mut self, rects: &[crate::scanning::PaintedRect]) -> Result<(), String> {
            if self.last_rects == rects {
                return Ok(());
            }
            self.ensure_windows(rects.len())?;
            unsafe {
                for (index, window) in self.windows.iter().enumerate() {
                    if let Some(paint) = rects.get(index) {
                        let r = &paint.rect;
                        crate::overlay::platform::present_solid(
                            *window,
                            r.x.round() as i32,
                            r.y.round() as i32,
                            r.width.round().max(1.0) as i32,
                            r.height.round().max(1.0) as i32,
                            paint.color,
                            paint.opacity,
                        )?;
                    } else {
                        let _ = ShowWindow(*window, SW_HIDE);
                    }
                }
            }
            self.last_rects = rects.to_vec();
            Ok(())
        }
        pub fn prompt(&mut self, text: &str, rect: Rect, scale: f64) -> Result<(), String> {
            self.ensure_windows(1)?;
            crate::modifier_overlay::windows_backend::present_scan_prompt(
                self.windows[0],
                text,
                rect.x as i32,
                rect.y as i32,
                rect.width as i32,
                scale,
            )
        }
        pub fn tile(&mut self, tile: &crate::scanning::FrameTile) -> Result<(), String> {
            self.ensure_windows(1)?;
            crate::modifier_overlay::windows_backend::present_scan_tile(self.windows[0], tile)
        }
        pub fn hide(&mut self) {
            self.last_rects.clear();
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
    use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSColor, NSPanel, NSScreen};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    pub struct Host {
        panels: Vec<Retained<NSPanel>>,
        last_rects: Vec<crate::scanning::PaintedRect>,
        last_title: Option<(String, Rect, f64)>,
        title: Option<(
            Retained<objc2_app_kit::NSView>,
            Retained<objc2_app_kit::NSTextField>,
        )>,
    }
    impl Host {
        pub fn new() -> Result<Self, String> {
            Ok(Self {
                panels: vec![],
                last_rects: vec![],
                title: None,
                last_title: None,
            })
        }
        pub fn render(&mut self, rects: &[crate::scanning::PaintedRect]) -> Result<(), String> {
            if self.last_rects == rects {
                return Ok(());
            }
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
                if let Some(paint) = rects.get(index) {
                    let r = &paint.rect;
                    panel.setBackgroundColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
                        paint.color[0] as f64 / 255.0,
                        paint.color[1] as f64 / 255.0,
                        paint.color[2] as f64 / 255.0,
                        paint.opacity as f64 / 255.0,
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
            self.last_rects = rects.to_vec();
            Ok(())
        }
        pub fn menu_title(&mut self, text: &str, rect: Rect, scale: f64) -> Result<(), String> {
            use objc2_app_kit::{
                NSFont, NSFontWeightSemibold, NSTextAlignment, NSTextField, NSView,
            };
            use objc2_foundation::NSString;
            if self
                .last_title
                .as_ref()
                .is_some_and(|(old_text, old_rect, old_scale)| {
                    old_text == text && *old_rect == rect && *old_scale == scale
                })
            {
                return Ok(());
            }
            let mtm = MainThreadMarker::new().ok_or("Menu title requires the main thread.")?;
            self.render(&[crate::scanning::PaintedRect {
                rect,
                color: [30, 35, 46],
                opacity: 0,
                role: crate::scanning::VisualRole::Accent,
            }])?;
            let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(rect.width, rect.height));
            if self.title.is_none() {
                let view = NSView::initWithFrame(NSView::alloc(mtm), bounds);
                view.setWantsLayer(true);
                let label = NSTextField::wrappingLabelWithString(&NSString::from_str(text), mtm);
                label.setTextColor(Some(&NSColor::whiteColor()));
                label.setAlignment(NSTextAlignment::Center);
                view.addSubview(&label);
                self.title = Some((view, label));
            }
            let (view, label) = self.title.as_ref().unwrap();
            view.setFrame(bounds);
            let layer = view
                .layer()
                .ok_or("Menu title background is unavailable.")?;
            layer.setBackgroundColor(Some(
                &NSColor::colorWithSRGBRed_green_blue_alpha(
                    30.0 / 255.0,
                    35.0 / 255.0,
                    46.0 / 255.0,
                    1.0,
                )
                .CGColor(),
            ));
            layer.setCornerRadius(10.0 * scale);
            let value = NSString::from_str(text);
            if label.stringValue() != value {
                label.setStringValue(&value);
            }
            label.setFont(Some(&NSFont::systemFontOfSize_weight(
                18.0 * scale,
                unsafe { NSFontWeightSemibold },
            )));
            let width = (rect.width - 24.0 * scale).max(1.0);
            let measured = label
                .cell()
                .ok_or("Menu title text is unavailable.")?
                .cellSizeForBounds(NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(width, rect.height),
                ));
            let height = measured.height.min(rect.height);
            label.setFrame(NSRect::new(
                NSPoint::new(12.0 * scale, (rect.height - height) / 2.0),
                NSSize::new(width, height),
            ));
            if self.panels[0].contentView().as_deref() != Some(view.as_ref()) {
                self.panels[0].setContentView(Some(view));
            }
            self.last_title = Some((text.to_owned(), rect, scale));
            Ok(())
        }
        pub fn prompt(&mut self, text: &str, rect: Rect, scale: f64) -> Result<(), String> {
            use objc2_app_kit::{NSFont, NSTextField};
            use objc2_foundation::NSString;
            self.last_title = None;
            let mtm = MainThreadMarker::new().ok_or("Prompt requires the main thread.")?;
            self.render(&[crate::scanning::PaintedRect {
                rect,
                color: [30, 35, 46],
                opacity: 255,
                role: crate::scanning::VisualRole::Accent,
            }])?;
            let label = NSTextField::wrappingLabelWithString(&NSString::from_str(text), mtm);
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
            self.panels[0].orderFrontRegardless();
            Ok(())
        }
        pub fn tile(&mut self, tile: &crate::scanning::FrameTile) -> Result<(), String> {
            use objc2_app_kit::{NSFont, NSImageView, NSTextAlignment, NSTextField, NSView};
            use objc2_foundation::NSString;
            let mtm = MainThreadMarker::new().ok_or("Action tile requires the main thread")?;
            let pixels = crate::scan_tile::bitmap(tile)?;
            let image = crate::overlay::platform::image_from_rgba(
                pixels.data(),
                pixels.width() as usize,
                pixels.height() as usize,
                tile.rect.width,
            )?;
            self.render(&[crate::scanning::PaintedRect {
                rect: tile.rect,
                color: [30, 35, 46],
                opacity: 255,
                role: crate::scanning::VisualRole::Accent,
            }])?;
            let bounds = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(tile.rect.width, tile.rect.height),
            );
            let view = NSView::initWithFrame(NSView::alloc(mtm), bounds);
            let artwork = NSImageView::initWithFrame(NSImageView::alloc(mtm), bounds);
            artwork.setImage(Some(&image));
            view.addSubview(&artwork);
            let label = NSTextField::wrappingLabelWithString(&NSString::from_str(&tile.text), mtm);
            label.setFont(Some(&NSFont::boldSystemFontOfSize(15.0 * tile.scale)));
            label.setTextColor(Some(&NSColor::whiteColor()));
            label.setAlignment(NSTextAlignment::Center);
            label.setFrame(NSRect::new(
                NSPoint::new(6.0 * tile.scale, 12.0 * tile.scale),
                NSSize::new(tile.rect.width - 12.0 * tile.scale, 40.0 * tile.scale),
            ));
            view.addSubview(&label);
            self.panels[0].setContentView(Some(&view));
            Ok(())
        }
        pub fn hide(&mut self) {
            self.last_title = None;
            self.last_rects.clear();
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
        pub fn render(&mut self, _: &[crate::scanning::PaintedRect]) -> Result<(), String> {
            Err("Point scan is unavailable.".into())
        }
        pub fn prompt(&mut self, _: &str, _: Rect, _: f64) -> Result<(), String> {
            Err("Scanning is unavailable.".into())
        }
        pub fn tile(&mut self, _: &crate::scanning::FrameTile) -> Result<(), String> {
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

#[cfg(target_os = "macos")]
pub fn close_foreground_window() -> Result<(), String> {
    use core_foundation::{
        base::{CFType, CFTypeRef, TCFType},
        string::{CFString, CFStringRef},
    };
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> i32;
    }
    fn attribute(element: &CFType, name: &str) -> Result<CFType, String> {
        let name = CFString::new(name);
        let mut value = std::ptr::null();
        let result = unsafe {
            AXUIElementCopyAttributeValue(
                element.as_CFTypeRef(),
                name.as_concrete_TypeRef(),
                &mut value,
            )
        };
        if result != 0 || value.is_null() {
            return Err("The foreground window's close button is unavailable.".into());
        }
        Ok(unsafe { CFType::wrap_under_create_rule(value) })
    }
    let application = unsafe { AXUIElementCreateApplication(foreground()? as i32) };
    if application.is_null() {
        return Err("No foreground application is available.".into());
    }
    let application = unsafe { CFType::wrap_under_create_rule(application) };
    let window = attribute(&application, "AXFocusedWindow")?;
    let button = attribute(&window, "AXCloseButton")?;
    let action = CFString::new("AXPress");
    if unsafe { AXUIElementPerformAction(button.as_CFTypeRef(), action.as_concrete_TypeRef()) } != 0
    {
        return Err("The foreground window could not be closed.".into());
    }
    Ok(())
}

impl Host {
    pub fn label(
        &mut self,
        label: &crate::scanning::FrameLabel,
        menu_title: Option<MenuTitle>,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        if let Some(MenuTitle { rect, scale }) = menu_title {
            return self.menu_title(&label.text, rect, scale);
        }
        #[cfg(not(target_os = "macos"))]
        if let Some(MenuTitle { rect, scale }) = menu_title {
            let _ = (rect, scale);
        }
        self.prompt(&label.text, label.rect, label.scale)
    }
}

#[derive(Clone, Copy)]
pub struct MenuTitle {
    rect: Rect,
    scale: f64,
}

pub fn menu_title_geometry(
    label: &crate::scanning::FrameLabel,
    tiles: &[crate::scanning::FrameTile],
) -> Option<MenuTitle> {
    let first = tiles.first()?;
    let left = tiles
        .iter()
        .map(|tile| tile.rect.x)
        .fold(f64::INFINITY, f64::min);
    let right = tiles
        .iter()
        .map(|tile| tile.rect.x + tile.rect.width)
        .fold(f64::NEG_INFINITY, f64::max);
    Some(MenuTitle {
        rect: Rect {
            x: left,
            width: right - left,
            ..label.rect
        },
        scale: first.scale,
    })
}

#[cfg(test)]
mod title_tests {
    use super::*;
    use crate::scan_menu::{Kind, Menu};

    #[test]
    fn titles_follow_tile_edges_without_changing_header_spacing() {
        for screen in [
            Rect {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 240.0,
            },
            Rect {
                x: -2048.0,
                y: -100.0,
                width: 2048.0,
                height: 1109.0,
            },
        ] {
            for units in [1.0, 2.0] {
                let mut menu = Menu::new(Kind::Actions, 500);
                for paused in [false, true] {
                    menu.suspended = paused;
                    let frame = menu.frame((0, 0), screen, units);
                    let label = frame.label.as_ref().unwrap();
                    let MenuTitle { rect, scale } =
                        menu_title_geometry(label, &frame.tiles).unwrap();
                    assert_eq!(rect.x, frame.tiles[0].rect.x);
                    let last = &frame.tiles[2].rect;
                    assert!((rect.x + rect.width - last.x - last.width).abs() < 0.001);
                    assert_eq!(rect.y, label.rect.y);
                    assert_eq!(rect.height, label.rect.height);
                    assert!(
                        (frame.tiles[0].rect.y - rect.y - rect.height - 8.0 * scale).abs() < 0.001
                    );
                    assert!(rect.x >= screen.x && rect.x + rect.width <= screen.x + screen.width);
                    assert!(rect.y >= screen.y && rect.y + rect.height <= screen.y + screen.height);
                    if paused {
                        assert_eq!(label.text, "Select to resume");
                    }
                    assert!(menu_title_geometry(label, &[]).is_none());
                }
            }
        }
    }
}
