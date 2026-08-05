use std::cell::RefCell;
use std::ptr;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSEvent, NSImage,
    NSImageRep, NSImageView, NSPanel, NSScreen, NSStatusWindowLevel, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize};
use tauri::AppHandle;

use crate::overlay::{render_marker, run_loop, Command, Frame};
use crate::state::{emit_state, set_activity, ActivityKind, SharedModel};

thread_local! {
    static HOST: RefCell<Option<MacOverlayHost>> = const { RefCell::new(None) };
}

pub(super) fn spawn(app: AppHandle, shared: SharedModel, receiver: Receiver<Command>) {
    let Some(mtm) = MainThreadMarker::new() else {
        report_failure(&app, &shared, "the AppKit main thread is unavailable");
        return;
    };
    match MacOverlayHost::new(mtm) {
        Ok(host) => HOST.with(|slot| *slot.borrow_mut() = Some(host)),
        Err(error) => {
            report_failure(&app, &shared, &error);
            return;
        }
    }
    thread::Builder::new()
        .name("Switchify cursor overlay".into())
        .spawn(move || {
            let render_app = app.clone();
            let failure_app = app.clone();
            let failure_shared = shared.clone();
            let hide_app = app.clone();
            run_loop(
                move |frame| {
                    let (sender, result) = mpsc::sync_channel(1);
                    let frame = frame.clone();
                    render_app
                        .run_on_main_thread(move || {
                            let value = HOST.with(|slot| {
                                slot.borrow_mut()
                                    .as_mut()
                                    .ok_or_else(|| "the AppKit overlay is unavailable".to_string())?
                                    .render(&frame)
                            });
                            let _ = sender.send(value);
                        })
                        .map_err(|error| error.to_string())?;
                    let value = result
                        .recv()
                        .map_err(|_| "the AppKit overlay render was cancelled".to_string())?;
                    if let Err(error) = &value {
                        report_failure(&failure_app, &failure_shared, error);
                    }
                    value
                },
                move || {
                    let _ = hide_app.run_on_main_thread(|| {
                        HOST.with(|slot| {
                            if let Some(host) = slot.borrow_mut().as_mut() {
                                host.hide();
                            }
                        });
                    });
                },
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

struct MacOverlayHost {
    marker: Retained<NSPanel>,
    marker_view: Retained<NSImageView>,
    horizontal: Retained<NSPanel>,
    vertical: Retained<NSPanel>,
}

impl MacOverlayHost {
    fn new(mtm: MainThreadMarker) -> Result<Self, String> {
        let marker = make_panel(mtm);
        let marker_view =
            NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
        marker.setContentView(Some(&marker_view));
        Ok(Self {
            marker,
            marker_view,
            horizontal: make_panel(mtm),
            vertical: make_panel(mtm),
        })
    }

    fn render(&mut self, frame: &Frame) -> Result<(), String> {
        let cursor = NSEvent::mouseLocation();
        let mtm = MainThreadMarker::new().ok_or("the AppKit main thread is unavailable")?;
        let screens = NSScreen::screens(mtm);
        let screen = screen_at_point(&screens, cursor)
            .ok_or_else(|| "the active display could not be resolved".to_string())?;
        let scale = screen.backingScaleFactor();
        let pixmap = render_marker(frame, scale);
        let logical_size = frame.logical_size as f64;
        let image = image_from_rgba(
            pixmap.data(),
            pixmap.width() as usize,
            pixmap.height() as usize,
            logical_size,
            mtm,
        )?;
        let marker_frame = rect(
            cursor.x - logical_size / 2.0,
            cursor.y - logical_size / 2.0,
            logical_size,
            logical_size,
        );
        self.marker.setFrame_display(marker_frame, false);
        self.marker_view
            .setFrame(rect(0.0, 0.0, logical_size, logical_size));
        self.marker_view.setImage(Some(&image));
        self.marker.orderFrontRegardless();

        if frame.crosshairs {
            let screen_frame = screen.frame();
            let thickness = 2.0;
            let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                f64::from(frame.color[0]) / 255.0,
                f64::from(frame.color[1]) / 255.0,
                f64::from(frame.color[2]) / 255.0,
                0.72,
            );
            self.horizontal.setBackgroundColor(Some(&color));
            self.vertical.setBackgroundColor(Some(&color));
            self.horizontal.setFrame_display(
                rect(
                    screen_frame.origin.x,
                    cursor.y - thickness / 2.0,
                    screen_frame.size.width,
                    thickness,
                ),
                false,
            );
            self.vertical.setFrame_display(
                rect(
                    cursor.x - thickness / 2.0,
                    screen_frame.origin.y,
                    thickness,
                    screen_frame.size.height,
                ),
                false,
            );
            self.horizontal.orderFrontRegardless();
            self.vertical.orderFrontRegardless();
        } else {
            self.horizontal.orderOut(None);
            self.vertical.orderOut(None);
        }
        Ok(())
    }

    fn hide(&mut self) {
        self.marker.orderOut(None);
        self.horizontal.orderOut(None);
        self.vertical.orderOut(None);
    }
}

fn make_panel(mtm: MainThreadMarker) -> Retained<NSPanel> {
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        rect(0.0, 0.0, 1.0, 1.0),
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(false);
    panel.setIgnoresMouseEvents(true);
    panel.setLevel(NSStatusWindowLevel);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel
}

fn image_from_rgba(
    rgba: &[u8],
    width: usize,
    height: usize,
    logical_size: f64,
    mtm: MainThreadMarker,
) -> Result<Retained<NSImage>, String> {
    let representation = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            ptr::null_mut(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            (width * 4) as isize,
            32,
        )
    }
    .ok_or_else(|| "the overlay bitmap could not be created".to_string())?;
    unsafe {
        ptr::copy_nonoverlapping(rgba.as_ptr(), representation.bitmapData(), rgba.len());
    }
    let image = NSImage::initWithSize(
        NSImage::alloc(mtm),
        NSSize {
            width: logical_size,
            height: logical_size,
        },
    );
    image.addRepresentation(&representation as &NSImageRep);
    Ok(image)
}

fn screen_at_point(screens: &NSArray<NSScreen>, point: NSPoint) -> Option<Retained<NSScreen>> {
    screens.iter().find(|screen| {
        let frame = screen.frame();
        point.x >= frame.origin.x
            && point.x < frame.origin.x + frame.size.width
            && point.y >= frame.origin.y
            && point.y < frame.origin.y + frame.size.height
    })
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect {
        origin: NSPoint { x, y },
        size: NSSize { width, height },
    }
}
