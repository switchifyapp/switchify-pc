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
    input.move_pointer_absolute(point.0, point.1)?;
    input.click_pointer(crate::protocol::MouseButton::Left, 1)
}
