use crate::input::PointerFeedback;
use tauri::AppHandle;

#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    pub name: String,
    pub scale_factor: f64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Display {
    fn center(&self) -> (i64, i64) {
        (
            i64::from(self.x) + i64::from(self.width) / 2,
            i64::from(self.y) + i64::from(self.height) / 2,
        )
    }

    fn same_bounds(&self, other: &Self) -> bool {
        (self.x, self.y, self.width, self.height) == (other.x, other.y, other.width, other.height)
    }

    fn contains(&self, point: (f64, f64)) -> bool {
        point.0 >= f64::from(self.x)
            && point.0 < f64::from(self.x) + f64::from(self.width)
            && point.1 >= f64::from(self.y)
            && point.1 < f64::from(self.y) + f64::from(self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationError {
    pub code: &'static str,
    pub message: String,
}

impl NavigationError {
    fn adapter(message: &str) -> Self {
        Self {
            code: "adapter_failure",
            message: message.into(),
        }
    }
}

#[cfg(target_os = "macos")]
pub fn displays(app: &AppHandle) -> Result<((f64, f64), Vec<Display>), NavigationError> {
    let _ = app;
    macos_displays()
}

#[cfg(target_os = "macos")]
pub fn cursor_position() -> Result<(f64, f64), NavigationError> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| NavigationError::adapter("The pointer position could not be read."))?;
    let cursor = CGEvent::new(source)
        .map_err(|_| NavigationError::adapter("The pointer position could not be read."))?
        .location();
    Ok((cursor.x, cursor.y))
}

#[cfg(not(target_os = "macos"))]
pub fn displays(app: &AppHandle) -> Result<((f64, f64), Vec<Display>), NavigationError> {
    let cursor = app
        .cursor_position()
        .map_err(|_| NavigationError::adapter("The pointer position could not be read."))?;
    let displays = app
        .available_monitors()
        .map_err(|_| NavigationError::adapter("The connected monitors could not be read."))?
        .into_iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Display {
                name: monitor.name().cloned().unwrap_or_else(|| "display".into()),
                scale_factor: monitor.scale_factor(),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect::<Vec<_>>();
    if displays.is_empty() {
        return Err(NavigationError::adapter(
            "No connected monitor could be resolved.",
        ));
    }
    Ok(((cursor.x, cursor.y), displays))
}

#[cfg(target_os = "macos")]
fn macos_displays() -> Result<((f64, f64), Vec<Display>), NavigationError> {
    use core_graphics::display::CGDisplay;

    let cursor = cursor_position()?;
    let displays = CGDisplay::active_displays()
        .map_err(|_| NavigationError::adapter("The connected monitors could not be read."))?
        .into_iter()
        .map(|id| {
            let display = CGDisplay::new(id);
            let bounds = display.bounds();
            display_from_native_bounds(
                format!("display-{id}"),
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
                display.pixels_wide(),
            )
        })
        .collect::<Vec<_>>();
    if displays.is_empty() {
        return Err(NavigationError::adapter(
            "No connected monitor could be resolved.",
        ));
    }
    Ok((cursor, displays))
}

#[cfg(any(target_os = "macos", test))]
fn display_from_native_bounds(
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    physical_width: u64,
) -> Display {
    let scale_factor = if width.is_finite() && width > 0.0 {
        physical_width as f64 / width
    } else {
        1.0
    };
    Display {
        name,
        scale_factor: if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        },
        x: x.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        y: y.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        width: width.round().clamp(1.0, f64::from(u32::MAX)) as u32,
        height: height.round().clamp(1.0, f64::from(u32::MAX)) as u32,
    }
}

pub fn display_count(count: usize) -> u8 {
    count.clamp(1, 64) as u8
}

pub fn map_injection<T>(result: Result<T, String>) -> Result<T, NavigationError> {
    result.map_err(|_| NavigationError {
        code: "adapter_failure",
        message: "The operating system could not move the pointer to another monitor.".into(),
    })
}

pub fn run_navigation_command<C>(
    context: &mut C,
    direction: &str,
    stop_repeats: impl FnOnce(&mut C),
    navigate: impl FnOnce(&mut C, &str) -> Result<PointerFeedback, NavigationError>,
    show_feedback: impl FnOnce(&mut C, PointerFeedback),
) -> Result<PointerFeedback, NavigationError> {
    stop_repeats(context);
    let feedback = navigate(context, direction)?;
    show_feedback(context, feedback);
    Ok(feedback)
}

pub fn current_display(cursor: (f64, f64), displays: &[Display]) -> Option<&Display> {
    displays
        .iter()
        .find(|display| display.contains(cursor))
        .or_else(|| {
            let distance = |display: &Display| {
                let right = f64::from(display.x) + f64::from(display.width);
                let bottom = f64::from(display.y) + f64::from(display.height);
                let dx = if cursor.0 < f64::from(display.x) {
                    f64::from(display.x) - cursor.0
                } else if cursor.0 > right {
                    cursor.0 - right
                } else {
                    0.0
                };
                let dy = if cursor.1 < f64::from(display.y) {
                    f64::from(display.y) - cursor.1
                } else if cursor.1 > bottom {
                    cursor.1 - bottom
                } else {
                    0.0
                };
                dx * dx + dy * dy
            };
            displays.iter().min_by(|first, second| {
                distance(first)
                    .total_cmp(&distance(second))
                    .then_with(|| first.x.cmp(&second.x))
                    .then_with(|| first.y.cmp(&second.y))
                    .then_with(|| first.width.cmp(&second.width))
                    .then_with(|| first.height.cmp(&second.height))
            })
        })
}

pub fn clamped_pointer_target(
    cursor: (f64, f64),
    dx: i32,
    dy: i32,
    displays: &[Display],
) -> Option<(i32, i32)> {
    let current = nearest_display_point(cursor, displays)?;
    nearest_display_point(
        (
            f64::from(current.0) + f64::from(dx),
            f64::from(current.1) + f64::from(dy),
        ),
        displays,
    )
}

fn nearest_display_point(point: (f64, f64), displays: &[Display]) -> Option<(i32, i32)> {
    displays
        .iter()
        .map(|display| {
            let min_x = i64::from(display.x);
            let min_y = i64::from(display.y);
            let max_x = min_x + i64::from(display.width.saturating_sub(1));
            let max_y = min_y + i64::from(display.height.saturating_sub(1));
            let rounded_x = point.0.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64;
            let rounded_y = point.1.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64;
            let x = rounded_x.clamp(min_x, max_x);
            let y = rounded_y.clamp(min_y, max_y);
            let dx = point.0 - x as f64;
            let dy = point.1 - y as f64;
            ((dx * dx + dy * dy), x, y)
        })
        .min_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.cmp(&second.1))
                .then_with(|| first.2.cmp(&second.2))
        })
        .map(|(_, x, y)| {
            (
                x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            )
        })
}

pub fn target_center(
    source: &Display,
    displays: &[Display],
    direction: &str,
) -> Result<(i32, i32), NavigationError> {
    let source_center = source.center();
    let target = displays
        .iter()
        .filter(|display| !display.same_bounds(source))
        .filter(|display| {
            let center = display.center();
            match direction {
                "left" => center.0 < source_center.0,
                "right" => center.0 > source_center.0,
                "up" => center.1 < source_center.1,
                "down" => center.1 > source_center.1,
                _ => false,
            }
        })
        .min_by_key(|display| {
            let center = display.center();
            let dx = i128::from(center.0 - source_center.0);
            let dy = i128::from(center.1 - source_center.1);
            (
                dx * dx + dy * dy,
                display.x,
                display.y,
                display.width,
                display.height,
            )
        })
        .ok_or_else(|| NavigationError {
            code: "no_display_in_direction",
            message: match direction {
                "up" => "No monitor above.".into(),
                "down" => "No monitor below.".into(),
                _ => format!("No monitor to the {direction}."),
            },
        })?;
    let center = target.center();
    Ok((
        center.0.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        center.1.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(x: i32, y: i32, width: u32, height: u32) -> Display {
        Display {
            name: "test".into(),
            scale_factor: 1.0,
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn finds_each_direction_and_centers_mixed_resolutions() {
        let source = display(0, 0, 1920, 1080);
        for (direction, target, expected) in [
            ("left", display(-1280, 0, 1280, 1024), (-640, 512)),
            ("right", display(1920, 0, 2560, 1440), (3200, 720)),
            ("up", display(0, -900, 1600, 900), (800, -450)),
            ("down", display(0, 1080, 1366, 768), (683, 1464)),
        ] {
            assert_eq!(
                target_center(&source, &[source.clone(), target], direction).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn chooses_nearest_center_and_breaks_ties_by_bounds() {
        let source = display(0, 0, 1920, 1080);
        let near_diagonal = display(1920, 1080, 1280, 720);
        let far_right = display(4000, 0, 1920, 1080);
        assert_eq!(
            target_center(
                &source,
                &[source.clone(), far_right, near_diagonal],
                "right"
            )
            .unwrap(),
            (2560, 1440)
        );

        let upper = display(1920, -1080, 1920, 1080);
        let lower = display(1920, 1080, 1920, 1080);
        assert_eq!(
            target_center(&source, &[source.clone(), lower, upper], "right").unwrap(),
            (2880, -540)
        );
    }

    #[test]
    fn returns_structured_error_without_a_directional_target() {
        let source = display(0, 0, 1920, 1080);
        let error = target_center(
            &source,
            &[source.clone(), display(-1920, 0, 1920, 1080)],
            "right",
        )
        .unwrap_err();
        assert_eq!(error.code, "no_display_in_direction");
        assert_eq!(error.message, "No monitor to the right.");
    }

    #[test]
    fn resolves_the_containing_or_nearest_display() {
        let left = display(-1920, 0, 1920, 1080);
        let right = display(100, 0, 1920, 1080);
        assert_eq!(
            current_display((-10.0, 100.0), &[left.clone(), right.clone()]),
            Some(&left)
        );
        assert_eq!(
            current_display((50.0, 100.0), &[left.clone(), right.clone()]),
            Some(&left)
        );
    }

    #[test]
    fn clamps_pointer_targets_to_all_outer_edges_and_corners() {
        let displays = [display(0, 0, 1920, 1080)];
        for (cursor, delta, expected) in [
            ((0.0, 540.0), (-20, 0), (0, 540)),
            ((1919.0, 540.0), (20, 0), (1919, 540)),
            ((960.0, 0.0), (0, -20), (960, 0)),
            ((960.0, 1079.0), (0, 20), (960, 1079)),
            ((0.0, 0.0), (-20, -20), (0, 0)),
            ((1919.0, 1079.0), (20, 20), (1919, 1079)),
        ] {
            assert_eq!(
                clamped_pointer_target(cursor, delta.0, delta.1, &displays),
                Some(expected)
            );
        }
    }

    #[test]
    fn clamping_preserves_a_free_diagonal_axis() {
        let displays = [display(0, 0, 1920, 1080)];
        assert_eq!(
            clamped_pointer_target((1919.0, 500.0), 10, 4, &displays),
            Some((1919, 504))
        );
    }

    #[test]
    fn clamping_crosses_reachable_adjacent_displays() {
        let displays = [display(0, 0, 1920, 1080), display(1920, 0, 2560, 1440)];
        assert_eq!(
            clamped_pointer_target((1919.0, 500.0), 10, 0, &displays),
            Some((1929, 500))
        );
    }

    #[test]
    fn clamping_recovers_an_existing_out_of_bounds_event_position() {
        let displays = [display(0, 0, 1920, 1080)];
        assert_eq!(
            clamped_pointer_target((2500.0, 540.0), 10, 0, &displays),
            Some((1919, 540))
        );
    }

    #[test]
    fn native_bounds_keep_core_graphics_coordinates_and_derive_scale() {
        let retina =
            display_from_native_bounds("retina".into(), -1920.0, -900.0, 1920.0, 1080.0, 3840);
        assert_eq!(
            (retina.x, retina.y, retina.width, retina.height),
            (-1920, -900, 1920, 1080)
        );
        assert_eq!(retina.scale_factor, 2.0);
        assert_eq!(
            target_center(
                &display(0, 0, 1920, 1080),
                &[display(0, 0, 1920, 1080), retina],
                "left"
            )
            .unwrap(),
            (-960, -360)
        );
    }

    #[test]
    fn capability_count_is_bounded_and_native_failures_are_structured() {
        assert_eq!(display_count(0), 1);
        assert_eq!(display_count(3), 3);
        assert_eq!(display_count(100), 64);
        let error = map_injection::<()>(Err("native failure".into())).unwrap_err();
        assert_eq!(error.code, "adapter_failure");
        assert_eq!(
            error.message,
            "The operating system could not move the pointer to another monitor."
        );
    }

    #[test]
    fn command_stops_repeat_before_navigation_and_forwards_overlay_feedback() {
        #[derive(Default)]
        struct FakeRuntime {
            repeat_active: bool,
            events: Vec<&'static str>,
        }

        let mut runtime = FakeRuntime {
            repeat_active: true,
            ..Default::default()
        };
        let feedback = run_navigation_command(
            &mut runtime,
            "right",
            |runtime| {
                runtime.repeat_active = false;
                runtime.events.push("repeat stopped");
            },
            |runtime, direction| {
                assert!(!runtime.repeat_active);
                assert_eq!(direction, "right");
                runtime.events.push("pointer moved");
                Ok(PointerFeedback::Move)
            },
            |runtime, feedback| {
                assert_eq!(feedback, PointerFeedback::Move);
                runtime.events.push("overlay shown");
            },
        )
        .unwrap();

        assert_eq!(feedback, PointerFeedback::Move);
        assert_eq!(
            runtime.events,
            vec!["repeat stopped", "pointer moved", "overlay shown"]
        );
    }
}
