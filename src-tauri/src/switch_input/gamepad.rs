//! Buttons 1 to 8 across every connected controller, so button 1 on any of
//! them is one switch. A button is down while any controller holds it.
use super::GAMEPAD_BUTTONS;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Gamepads {
    devices: HashMap<isize, u8>,
    held: u8,
}
impl Gamepads {
    /// A whole-state report: bit `n - 1` of `buttons` is button `n`.
    /// Returns the button edges it produces, as (button, pressed).
    pub fn report(&mut self, device: isize, buttons: u8) -> Vec<(u8, bool)> {
        if buttons == 0 {
            self.devices.remove(&device);
        } else {
            self.devices.insert(device, buttons);
        }
        self.settle()
    }
    /// One button of one controller changed.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn button(&mut self, device: isize, button: u8, pressed: bool) -> Vec<(u8, bool)> {
        if !(1..=GAMEPAD_BUTTONS).contains(&button) {
            return Vec::new();
        }
        let bit = 1 << (button - 1);
        let current = self.devices.get(&device).copied().unwrap_or(0);
        let buttons = if pressed {
            current | bit
        } else {
            current & !bit
        };
        self.report(device, buttons)
    }
    /// A controller went away: its held buttons are released.
    pub fn removed(&mut self, device: isize) -> Vec<(u8, bool)> {
        self.devices.remove(&device);
        self.settle()
    }
    fn settle(&mut self) -> Vec<(u8, bool)> {
        let held = self.devices.values().fold(0u8, |acc, bits| acc | bits);
        let changed = held ^ self.held;
        self.held = held;
        (1..=GAMEPAD_BUTTONS)
            .filter(|button| changed & (1 << (button - 1)) != 0)
            .map(|button| (button, held & (1 << (button - 1)) != 0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controllers_merge_and_release_on_removal() {
        let mut pads = Gamepads::default();
        assert_eq!(pads.report(1, 0b0000_0001), vec![(1, true)]);
        // A second controller holding the same button changes nothing.
        assert_eq!(pads.report(2, 0b0000_0001), vec![]);
        assert_eq!(pads.report(2, 0b1000_0001), vec![(8, true)]);
        assert_eq!(pads.report(1, 0), vec![]);
        assert_eq!(pads.removed(2), vec![(1, false), (8, false)]);
        assert_eq!(pads.removed(2), vec![]);
    }
    #[test]
    fn single_button_changes_build_the_same_state() {
        let mut pads = Gamepads::default();
        assert_eq!(pads.button(7, 2, true), vec![(2, true)]);
        assert_eq!(pads.button(7, 2, true), vec![]);
        assert_eq!(pads.button(7, 5, true), vec![(5, true)]);
        assert_eq!(pads.button(7, 2, false), vec![(2, false)]);
        assert_eq!(pads.button(7, 9, true), vec![]);
        assert_eq!(pads.button(7, 0, true), vec![]);
        assert_eq!(pads.removed(7), vec![(5, false)]);
    }
}
