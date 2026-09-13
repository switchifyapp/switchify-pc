//! Typed local scan actions. Input ownership survives failures for cleanup retries.
use crate::{
    input::{DesktopInput, InputInjector},
    point_workflow::Request,
    protocol::MouseButton,
};
pub fn execute<I: InputInjector>(
    input: &mut DesktopInput<I>,
    request: Request,
    modifiers_released: bool,
) -> Result<(), String> {
    if !modifiers_released || input.has_active_switch_session() {
        return Err(
            "Release modifiers and end switch forwarding before selecting an action.".into(),
        );
    }
    match request {
        Request::Click {
            point,
            right,
            count,
        } => {
            if input.has_active_drag() {
                return Err("End the active drag before clicking.".into());
            }
            input.move_pointer_absolute(point.0, point.1)?;
            input.click_pointer(
                if right {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                },
                count,
            )
        }
        Request::Scroll { point, dx, dy } => {
            input.move_pointer_absolute(point.0, point.1)?;
            input.execute_repeat_scroll(dx, dy).map(|_| ())
        }
        Request::DragStart(point) => input.start_scan_drag(point),
        Request::DragMove(point) => input.move_scan_drag(point),
        Request::DragEnd(point) => {
            let result = input.move_scan_drag(point);
            let released = input.release_all();
            result.and(released)
        }
    }
}
thread_local! { static INPUT:std::cell::RefCell<Option<DesktopInput<enigo::Enigo>>>=const{std::cell::RefCell::new(None)}; }
pub fn activate(request: Request) -> Result<(), String> {
    INPUT.with(|slot| {
        let mut input = slot.borrow_mut();
        if input.is_none() {
            *input = Some(DesktopInput::new(
                enigo::Enigo::new(&enigo::Settings::default())
                    .map_err(|_| "Scan input could not be initialized.")?,
            ));
        }
        let input = input.as_mut().unwrap();
        let result = execute(input, request, crate::scan_host::modifiers_released());
        if result.is_err() {
            let _ = input.release_all();
        }
        result
    })
}
pub fn cleanup() -> Result<(), String> {
    INPUT.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map_or(Ok(()), DesktopInput::release_all)
    })
}
