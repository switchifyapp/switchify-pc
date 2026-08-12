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
}
