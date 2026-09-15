use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField},
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::CGPoint,
};

use crate::{input::SCAN_EVENT_MARKER, protocol::MouseButton};

fn prepare(
    point: (i32, i32),
    button: MouseButton,
    count: u8,
    modifiers: &[&str],
) -> Result<Vec<CGEvent>, String> {
    if !(1..=3).contains(&count) {
        return Err("Invalid scan click count.".into());
    }
    let mut flags = CGEventFlags::empty();
    for modifier in modifiers {
        flags |= match *modifier {
            "Shift" => CGEventFlags::CGEventFlagShift,
            "Ctrl" => CGEventFlags::CGEventFlagControl,
            "Alt" => CGEventFlags::CGEventFlagAlternate,
            "Meta" => CGEventFlags::CGEventFlagCommand,
            _ => return Err("Invalid scan click modifier.".into()),
        };
    }
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Scan click event source could not be initialized.")?;
    let (button, down, up) = match button {
        MouseButton::Left => (
            CGMouseButton::Left,
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
        ),
        MouseButton::Right => (
            CGMouseButton::Right,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
        ),
        MouseButton::Middle => (
            CGMouseButton::Center,
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
        ),
    };
    let target = CGPoint::new(point.0 as f64, point.1 as f64);
    let create = |kind, click_count| {
        let event = CGEvent::new_mouse_event(source.clone(), kind, target, button)
            .map_err(|_| "Scan click events could not be prepared.".to_string())?;
        event.set_flags(flags);
        event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, SCAN_EVENT_MARKER);
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
        Ok::<_, String>(event)
    };
    let mut events = vec![create(CGEventType::MouseMoved, 0)?];
    for click_count in 1..=count {
        events.push(create(down, click_count.into())?);
        events.push(create(up, click_count.into())?);
    }
    Ok(events)
}

pub fn post(
    point: (i32, i32),
    button: MouseButton,
    count: u8,
    modifiers: &[&str],
) -> Result<(), String> {
    let events = prepare(point, button, count, modifiers)?;
    for event in events {
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_events_keep_explicit_target_order_count_flags_and_marker() {
        for point in [(-1800, -400), (3200, 900)] {
            for (button, down, up) in [
                (
                    MouseButton::Left,
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                ),
                (
                    MouseButton::Right,
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                ),
                (
                    MouseButton::Middle,
                    CGEventType::OtherMouseDown,
                    CGEventType::OtherMouseUp,
                ),
            ] {
                for count in 1..=3 {
                    let events =
                        prepare(point, button, count, &["Shift", "Ctrl", "Alt", "Meta"]).unwrap();
                    assert_eq!(events.len(), 1 + 2 * count as usize);
                    assert_eq!(events[0].get_type() as u32, CGEventType::MouseMoved as u32);
                    for (index, event) in events.iter().enumerate() {
                        assert_eq!(
                            (event.location().x, event.location().y),
                            (point.0 as f64, point.1 as f64)
                        );
                        assert_eq!(
                            event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA),
                            SCAN_EVENT_MARKER
                        );
                        assert_eq!(
                            event.get_flags(),
                            CGEventFlags::CGEventFlagShift
                                | CGEventFlags::CGEventFlagControl
                                | CGEventFlags::CGEventFlagAlternate
                                | CGEventFlags::CGEventFlagCommand
                        );
                        if index > 0 {
                            assert_eq!(
                                event.get_type() as u32,
                                if index % 2 == 1 { down } else { up } as u32
                            );
                            assert_eq!(
                                event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
                                index.div_ceil(2) as i64
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_requests_fail_before_any_events_can_be_posted() {
        assert!(prepare((0, 0), MouseButton::Left, 0, &[]).is_err());
        assert!(prepare((0, 0), MouseButton::Left, 4, &[]).is_err());
        assert!(prepare((0, 0), MouseButton::Left, 1, &["Space"]).is_err());
    }
}
