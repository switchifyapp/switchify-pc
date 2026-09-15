#![cfg(test)]
/// Keep injection behind the existing adapter; no tests use the system adapter.
pub fn click<I: crate::input::InputInjector>(
    input: &mut crate::input::DesktopInput<I>,
    point: (i32, i32),
    modifiers_released: bool,
) -> Result<(), String> {
    if !modifiers_released {
        return Err("Release modifier keys before selecting a point.".into());
    }
    if input.has_active_switch_session() || input.has_active_drag() {
        return Err("End switch forwarding or dragging before using point scan.".into());
    }
    input.click_pointer_at(point, crate::protocol::MouseButton::Left, 1, &[])
}
