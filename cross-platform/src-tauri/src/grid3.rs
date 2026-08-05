use std::sync::OnceLock;

use windows::core::w;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, RegisterWindowMessageW, HWND_BROADCAST,
};

const MESSAGE_NAME: windows::core::PCWSTR = w!("Sensory_SwitchInput");
static MESSAGE_ID: OnceLock<Result<u32, String>> = OnceLock::new();

pub fn set_switch_state(switch_id: u8, pressed: bool) -> Result<(), String> {
    if !(1..=8).contains(&switch_id) {
        return Err("Grid switch ID is invalid.".into());
    }
    let message = MESSAGE_ID
        .get_or_init(|| {
            let id = unsafe { RegisterWindowMessageW(MESSAGE_NAME) };
            (id != 0)
                .then_some(id)
                .ok_or_else(|| "Could not register the Grid 3 switch message.".to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    unsafe {
        PostMessageW(
            Some(HWND_BROADCAST),
            *message,
            WPARAM(usize::from(switch_id)),
            LPARAM(isize::from(pressed)),
        )
    }
    .map_err(|_| format!("Could not queue Grid switch {switch_id}."))
}
