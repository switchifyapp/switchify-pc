use std::cell::RefCell;
use std::ptr;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBitmapFormat, NSBitmapImageRep, NSCalibratedRGBColorSpace, NSColor,
    NSEvent, NSImage, NSImageRep, NSImageView, NSPanel, NSScreen, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize};
use tauri::AppHandle;

use crate::macos_overlay_window;
use crate::overlay::{render_marker, run_loop, Command, Frame};
use crate::state::{emit_state, set_activity, ActivityKind, SharedModel};

// With AlphaNonpremultiplied and AlphaFirst both absent, AppKit expects the
// Tiny-Skia byte order: premultiplied RGBA.
const PREMULTIPLIED_RGBA_BITMAP_FORMAT: NSBitmapFormat = NSBitmapFormat(0);

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
                || true,
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
        let screen_frames = screens
            .iter()
            .map(|candidate| candidate.frame())
            .collect::<Vec<_>>();
        let screen = screen_at_or_nearest_point(&screens, cursor)
            .ok_or_else(|| "the active display could not be resolved".to_string())?;
        let scale = screen.backingScaleFactor();
        let pixmap = render_marker(frame, scale);
        let logical_size = frame.logical_size as f64;
        let image = image_from_rgba(
            pixmap.data(),
            pixmap.width() as usize,
            pixmap.height() as usize,
            logical_size,
        )?;
        let screen_frame = screen.frame();
        let marker_layout =
            clipped_marker_layout(&screen_frames, cursor, logical_size, logical_size)
                .ok_or_else(|| "the cursor marker is outside the active display".to_string())?;
        self.marker.setFrame_display(marker_layout.panel, false);
        self.marker_view.setFrame(marker_layout.image);
        self.marker_view.setImage(Some(&image));
        self.marker.orderFrontRegardless();

        if frame.crosshairs {
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
                contained_centered_rect(
                    screen_frame,
                    NSPoint {
                        x: screen_frame.origin.x + screen_frame.size.width / 2.0,
                        y: cursor.y,
                    },
                    screen_frame.size.width,
                    thickness,
                ),
                false,
            );
            self.vertical.setFrame_display(
                contained_centered_rect(
                    screen_frame,
                    NSPoint {
                        x: cursor.x,
                        y: screen_frame.origin.y + screen_frame.size.height / 2.0,
                    },
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
    macos_overlay_window::configure(&panel);
    panel
}

fn image_from_rgba(
    rgba: &[u8],
    width: usize,
    height: usize,
    logical_size: f64,
) -> Result<Retained<NSImage>, String> {
    let layout = MacBitmapLayout::new(rgba.len(), width, height, logical_size)?;
    let representation = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            ptr::null_mut(),
            layout.width as isize,
            layout.height as isize,
            8,
            4,
            true,
            false,
            NSCalibratedRGBColorSpace,
            PREMULTIPLIED_RGBA_BITMAP_FORMAT,
            layout.bytes_per_row as isize,
            32,
        )
    }
    .ok_or_else(|| "the overlay bitmap could not be created".to_string())?;
    unsafe {
        ptr::copy_nonoverlapping(rgba.as_ptr(), representation.bitmapData(), rgba.len());
    }
    representation.setSize(NSSize {
        width: layout.logical_size,
        height: layout.logical_size,
    });
    let image = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize {
            width: layout.logical_size,
            height: layout.logical_size,
        },
    );
    image.addRepresentation(&representation as &NSImageRep);
    Ok(image)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MacBitmapLayout {
    width: usize,
    height: usize,
    bytes_per_row: usize,
    logical_size: f64,
}

impl MacBitmapLayout {
    fn new(
        byte_len: usize,
        width: usize,
        height: usize,
        logical_size: f64,
    ) -> Result<Self, String> {
        let bytes_per_row = width
            .checked_mul(4)
            .ok_or_else(|| "the overlay bitmap width is invalid".to_string())?;
        let expected_len = bytes_per_row
            .checked_mul(height)
            .ok_or_else(|| "the overlay bitmap height is invalid".to_string())?;
        if width == 0
            || height == 0
            || byte_len != expected_len
            || !logical_size.is_finite()
            || logical_size <= 0.0
        {
            return Err("the overlay bitmap layout is invalid".into());
        }
        Ok(Self {
            width,
            height,
            bytes_per_row,
            logical_size,
        })
    }
}

fn screen_at_or_nearest_point(
    screens: &NSArray<NSScreen>,
    point: NSPoint,
) -> Option<Retained<NSScreen>> {
    let frames = screens
        .iter()
        .map(|screen| screen.frame())
        .collect::<Vec<_>>();
    screen_index_at_point(&frames, point).and_then(|index| screens.iter().nth(index))
}

fn screen_index_at_point(frames: &[NSRect], point: NSPoint) -> Option<usize> {
    if let Some(index) = frames
        .iter()
        .position(|frame| frame_contains_point(*frame, point))
    {
        return Some(index);
    }

    let mut nearest = None;
    for (index, frame) in frames.iter().enumerate() {
        let distance = squared_distance_to_frame(*frame, point);
        if nearest.is_none_or(|(_, nearest_distance)| distance < nearest_distance) {
            nearest = Some((index, distance));
        }
    }
    nearest.map(|(index, _)| index)
}

fn frame_contains_point(frame: NSRect, point: NSPoint) -> bool {
    point.x >= frame.origin.x
        && point.x < frame.origin.x + frame.size.width
        && point.y >= frame.origin.y
        && point.y < frame.origin.y + frame.size.height
}

fn squared_distance_to_frame(frame: NSRect, point: NSPoint) -> f64 {
    let max_x = frame.origin.x + frame.size.width;
    let max_y = frame.origin.y + frame.size.height;
    let dx = axis_distance(point.x, frame.origin.x, max_x);
    let dy = axis_distance(point.y, frame.origin.y, max_y);
    dx * dx + dy * dy
}

fn axis_distance(value: f64, minimum: f64, maximum: f64) -> f64 {
    if value < minimum {
        minimum - value
    } else if value > maximum {
        value - maximum
    } else {
        0.0
    }
}

fn contained_centered_rect(container: NSRect, center: NSPoint, width: f64, height: f64) -> NSRect {
    let container_width = container.size.width.max(0.0);
    let container_height = container.size.height.max(0.0);
    let width = width.max(0.0).min(container_width);
    let height = height.max(0.0).min(container_height);
    let maximum_x = container.origin.x + container_width - width;
    let maximum_y = container.origin.y + container_height - height;
    rect(
        (center.x - width / 2.0).clamp(container.origin.x, maximum_x),
        (center.y - height / 2.0).clamp(container.origin.y, maximum_y),
        width,
        height,
    )
}

fn centered_rect(center: NSPoint, width: f64, height: f64) -> NSRect {
    rect(
        center.x - width / 2.0,
        center.y - height / 2.0,
        width,
        height,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MarkerLayout {
    panel: NSRect,
    image: NSRect,
}

fn clipped_marker_layout(
    screens: &[NSRect],
    cursor: NSPoint,
    width: f64,
    height: f64,
) -> Option<MarkerLayout> {
    let desired = centered_rect(cursor, width, height);
    let desired_max_x = desired.origin.x + desired.size.width;
    let desired_max_y = desired.origin.y + desired.size.height;
    let intersections = screens.iter().filter_map(|screen| {
        let min_x = desired.origin.x.max(screen.origin.x);
        let min_y = desired.origin.y.max(screen.origin.y);
        let max_x = desired_max_x.min(screen.origin.x + screen.size.width);
        let max_y = desired_max_y.min(screen.origin.y + screen.size.height);
        (max_x > min_x && max_y > min_y).then_some((min_x, min_y, max_x, max_y))
    });
    let (min_x, min_y, max_x, max_y) =
        intersections.fold(None::<(f64, f64, f64, f64)>, |bounds, intersection| {
            Some(match bounds {
                None => intersection,
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(intersection.0),
                    min_y.min(intersection.1),
                    max_x.max(intersection.2),
                    max_y.max(intersection.3),
                ),
            })
        })?;
    let panel = rect(min_x, min_y, max_x - min_x, max_y - min_y);
    Some(MarkerLayout {
        panel,
        image: rect(
            desired.origin.x - panel.origin.x,
            desired.origin.y - panel.origin.y,
            desired.size.width,
            desired.size.height,
        ),
    })
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect {
        origin: NSPoint { x, y },
        size: NSSize { width, height },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_display_resolves_interior_edges_and_corners() {
        let frames = [rect(0.0, 0.0, 1920.0, 1080.0)];
        for point in [
            NSPoint { x: 960.0, y: 540.0 },
            NSPoint { x: 0.0, y: 540.0 },
            NSPoint {
                x: 1920.0,
                y: 540.0,
            },
            NSPoint { x: 960.0, y: 0.0 },
            NSPoint {
                x: 960.0,
                y: 1080.0,
            },
            NSPoint { x: 0.0, y: 0.0 },
            NSPoint { x: 1920.0, y: 0.0 },
            NSPoint { x: 0.0, y: 1080.0 },
            NSPoint {
                x: 1920.0,
                y: 1080.0,
            },
        ] {
            assert_eq!(screen_index_at_point(&frames, point), Some(0));
        }
    }

    #[test]
    fn shared_boundary_keeps_half_open_screen_ownership() {
        let frames = [
            rect(-1920.0, 0.0, 1920.0, 1080.0),
            rect(0.0, 0.0, 1920.0, 1080.0),
        ];
        assert_eq!(
            screen_index_at_point(&frames, NSPoint { x: 0.0, y: 540.0 }),
            Some(1)
        );
    }

    #[test]
    fn negative_coordinate_layout_resolves_its_outer_edges() {
        let frames = [
            rect(-1600.0, -900.0, 1600.0, 900.0),
            rect(0.0, 0.0, 1920.0, 1080.0),
        ];
        assert_eq!(
            screen_index_at_point(
                &frames,
                NSPoint {
                    x: -1600.0,
                    y: -900.0,
                }
            ),
            Some(0)
        );
        assert_eq!(
            screen_index_at_point(
                &frames,
                NSPoint {
                    x: 1920.0,
                    y: 1080.0
                }
            ),
            Some(1)
        );
    }

    #[test]
    fn display_gaps_use_the_nearest_screen_deterministically() {
        let frames = [rect(0.0, 0.0, 100.0, 100.0), rect(200.0, 0.0, 100.0, 100.0)];
        assert_eq!(
            screen_index_at_point(&frames, NSPoint { x: 160.0, y: 50.0 }),
            Some(1)
        );
        assert_eq!(
            screen_index_at_point(&frames, NSPoint { x: 150.0, y: 50.0 }),
            Some(0)
        );
    }

    #[test]
    fn empty_display_list_cannot_resolve_a_screen() {
        assert_eq!(screen_index_at_point(&[], NSPoint { x: 0.0, y: 0.0 }), None);
    }

    #[test]
    fn overlay_rect_stays_centered_away_from_edges() {
        assert_eq!(
            centered_rect(NSPoint { x: 960.0, y: 540.0 }, 128.0, 128.0,),
            rect(896.0, 476.0, 128.0, 128.0)
        );
    }

    #[test]
    fn overlay_rect_stays_pointer_centered_at_every_screen_edge() {
        for (cursor, expected) in [
            (
                NSPoint { x: 0.0, y: 540.0 },
                rect(-64.0, 476.0, 128.0, 128.0),
            ),
            (
                NSPoint {
                    x: 1920.0,
                    y: 540.0,
                },
                rect(1856.0, 476.0, 128.0, 128.0),
            ),
            (
                NSPoint { x: 960.0, y: 0.0 },
                rect(896.0, -64.0, 128.0, 128.0),
            ),
            (
                NSPoint {
                    x: 960.0,
                    y: 1080.0,
                },
                rect(896.0, 1016.0, 128.0, 128.0),
            ),
            (NSPoint { x: 0.0, y: 0.0 }, rect(-64.0, -64.0, 128.0, 128.0)),
            (
                NSPoint {
                    x: 1920.0,
                    y: 1080.0,
                },
                rect(1856.0, 1016.0, 128.0, 128.0),
            ),
        ] {
            assert_eq!(centered_rect(cursor, 128.0, 128.0), expected);
        }
    }

    #[test]
    fn marker_layout_clips_the_ring_at_each_outer_edge() {
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        for (cursor, expected_panel, expected_image) in [
            (
                NSPoint { x: 0.0, y: 540.0 },
                rect(0.0, 476.0, 64.0, 128.0),
                rect(-64.0, 0.0, 128.0, 128.0),
            ),
            (
                NSPoint {
                    x: 1920.0,
                    y: 540.0,
                },
                rect(1856.0, 476.0, 64.0, 128.0),
                rect(0.0, 0.0, 128.0, 128.0),
            ),
            (
                NSPoint { x: 960.0, y: 0.0 },
                rect(896.0, 0.0, 128.0, 64.0),
                rect(0.0, -64.0, 128.0, 128.0),
            ),
            (
                NSPoint {
                    x: 960.0,
                    y: 1080.0,
                },
                rect(896.0, 1016.0, 128.0, 64.0),
                rect(0.0, 0.0, 128.0, 128.0),
            ),
        ] {
            assert_eq!(
                clipped_marker_layout(&[screen], cursor, 128.0, 128.0),
                Some(MarkerLayout {
                    panel: expected_panel,
                    image: expected_image,
                })
            );
        }
    }

    #[test]
    fn marker_layout_shows_a_quarter_ring_at_an_outer_corner() {
        assert_eq!(
            clipped_marker_layout(
                &[rect(0.0, 0.0, 1920.0, 1080.0)],
                NSPoint { x: 0.0, y: 0.0 },
                128.0,
                128.0,
            ),
            Some(MarkerLayout {
                panel: rect(0.0, 0.0, 64.0, 64.0),
                image: rect(-64.0, -64.0, 128.0, 128.0),
            })
        );
    }

    #[test]
    fn marker_layout_remains_centered_across_an_internal_display_seam() {
        assert_eq!(
            clipped_marker_layout(
                &[
                    rect(-1920.0, 0.0, 1920.0, 1080.0),
                    rect(0.0, 0.0, 1920.0, 1080.0),
                ],
                NSPoint { x: 0.0, y: 540.0 },
                128.0,
                128.0,
            ),
            Some(MarkerLayout {
                panel: rect(-64.0, 476.0, 128.0, 128.0),
                image: rect(0.0, 0.0, 128.0, 128.0),
            })
        );
    }

    #[test]
    fn overlay_rect_supports_negative_screen_origins() {
        assert_eq!(
            centered_rect(
                NSPoint {
                    x: -1600.0,
                    y: -900.0
                },
                128.0,
                128.0,
            ),
            rect(-1664.0, -964.0, 128.0, 128.0)
        );
    }

    #[test]
    fn marker_size_is_not_reduced_to_fit_an_undersized_screen() {
        assert_eq!(
            centered_rect(NSPoint { x: 0.0, y: 65.0 }, 128.0, 128.0),
            rect(-64.0, 1.0, 128.0, 128.0)
        );
    }

    #[test]
    fn crosshair_rects_are_clamped_perpendicular_to_screen_edges() {
        let screen = rect(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(
            contained_centered_rect(screen, NSPoint { x: 960.0, y: 0.0 }, 1920.0, 2.0,),
            rect(0.0, 0.0, 1920.0, 2.0)
        );
        assert_eq!(
            contained_centered_rect(
                screen,
                NSPoint {
                    x: 1920.0,
                    y: 540.0
                },
                2.0,
                1080.0,
            ),
            rect(1918.0, 0.0, 2.0, 1080.0)
        );
    }

    #[test]
    fn mac_bitmap_layout_preserves_retina_pixels_and_logical_size() {
        let layout = MacBitmapLayout::new(256 * 256 * 4, 256, 256, 128.0).unwrap();
        assert_eq!(layout.width, 256);
        assert_eq!(layout.height, 256);
        assert_eq!(layout.bytes_per_row, 1024);
        assert_eq!(layout.logical_size, 128.0);
        assert_eq!(PREMULTIPLIED_RGBA_BITMAP_FORMAT.0, 0);
    }

    #[test]
    fn mac_bitmap_layout_rejects_mismatched_pixel_buffers() {
        assert!(MacBitmapLayout::new(15, 2, 2, 2.0).is_err());
        assert!(MacBitmapLayout::new(16, 2, 2, f64::NAN).is_err());
    }
}
