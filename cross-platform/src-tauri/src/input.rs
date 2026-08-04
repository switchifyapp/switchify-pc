use std::collections::HashSet;

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse};
use serde_json::Value;

use crate::protocol::MouseButton;

pub trait InputInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), String>;
    fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String>;
    fn set_pointer_button(&mut self, button: MouseButton, down: bool) -> Result<(), String>;
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn set_key(&mut self, key: &str, down: bool) -> Result<(), String>;
    fn press_shortcut(&mut self, keys: &[String]) -> Result<(), String>;
    fn media(&mut self, action: &str) -> Result<(), String>;
    fn window(&mut self, action: &str) -> Result<(), String>;
}

fn pointer_button(button: MouseButton) -> Button {
    match button {
        MouseButton::Left => Button::Left,
        MouseButton::Middle => Button::Middle,
        MouseButton::Right => Button::Right,
    }
}

fn named_key(name: &str) -> Option<Key> {
    Some(match name {
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Enter" => Key::Return,
        "Escape" => Key::Escape,
        "Space" => Key::Space,
        "Tab" => Key::Tab,
        "Ctrl" => Key::Control,
        "Alt" => Key::Alt,
        "Shift" => Key::Shift,
        "Meta" => Key::Meta,
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        single if single.chars().count() == 1 => Key::Unicode(single.chars().next()?),
        _ => return None,
    })
}

fn enigo_error(action: &str) -> impl FnOnce(enigo::InputError) -> String + '_ {
    move |_| format!("The operating system could not {action}.")
}

fn send_shortcut(enigo: &mut Enigo, keys: &[&str]) -> Result<(), String> {
    let parsed: Vec<Key> = keys
        .iter()
        .map(|key| named_key(key).ok_or_else(|| format!("Unsupported key: {key}")))
        .collect::<Result<_, _>>()?;
    for key in &parsed {
        enigo
            .key(*key, Direction::Press)
            .map_err(enigo_error("press the shortcut"))?;
    }
    for key in parsed.iter().rev() {
        enigo
            .key(*key, Direction::Release)
            .map_err(enigo_error("release the shortcut"))?;
    }
    Ok(())
}

impl InputInjector for Enigo {
    fn inject_text(&mut self, text: &str) -> Result<(), String> {
        self.text(text)
            .map_err(enigo_error("type the received text"))
    }
    fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.move_mouse(dx, dy, Coordinate::Rel)
            .map_err(enigo_error("move the pointer"))
    }
    fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        for _ in 0..click_count {
            self.button(pointer_button(button), Direction::Click)
                .map_err(enigo_error("click the pointer"))?;
        }
        Ok(())
    }
    fn set_pointer_button(&mut self, button: MouseButton, down: bool) -> Result<(), String> {
        self.button(
            pointer_button(button),
            if down {
                Direction::Press
            } else {
                Direction::Release
            },
        )
        .map_err(enigo_error("change the pointer button"))
    }
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if dx != 0 {
            Mouse::scroll(self, dx, Axis::Horizontal)
                .map_err(enigo_error("scroll horizontally"))?;
        }
        if dy != 0 {
            Mouse::scroll(self, dy, Axis::Vertical).map_err(enigo_error("scroll vertically"))?;
        }
        Ok(())
    }
    fn set_key(&mut self, key: &str, down: bool) -> Result<(), String> {
        let key = named_key(key).ok_or_else(|| format!("Unsupported key: {key}"))?;
        self.key(
            key,
            if down {
                Direction::Press
            } else {
                Direction::Release
            },
        )
        .map_err(enigo_error("send the key"))
    }
    fn press_shortcut(&mut self, keys: &[String]) -> Result<(), String> {
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        send_shortcut(self, &refs)
    }
    fn media(&mut self, action: &str) -> Result<(), String> {
        let key = match action {
            "playPause" => Key::MediaPlayPause,
            "nextTrack" => Key::MediaNextTrack,
            "previousTrack" => Key::MediaPrevTrack,
            "volumeUp" => Key::VolumeUp,
            "volumeDown" => Key::VolumeDown,
            "mute" => Key::VolumeMute,
            _ => return Err("Unsupported media action.".into()),
        };
        self.key(key, Direction::Click)
            .map_err(enigo_error("send the media command"))
    }
    fn window(&mut self, action: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        let keys: &[&str] = match action {
            "switchNext" => &["Alt", "Tab"],
            "switchPrevious" => &["Alt", "Shift", "Tab"],
            "taskView" => &["Meta", "Tab"],
            "showDesktop" => &["Meta", "D"],
            "closeFocused" => &["Alt", "F4"],
            "minimizeFocused" => &["Meta", "ArrowDown"],
            "maximizeFocused" => &["Meta", "ArrowUp"],
            _ => return Err("Unsupported window action.".into()),
        };
        #[cfg(target_os = "macos")]
        let keys: &[&str] = match action {
            "switchNext" => &["Meta", "Tab"],
            "switchPrevious" => &["Meta", "Shift", "Tab"],
            "taskView" => &["Ctrl", "ArrowUp"],
            "showDesktop" => &["Meta", "F3"],
            "closeFocused" => &["Meta", "W"],
            "minimizeFocused" => &["Meta", "M"],
            "maximizeFocused" => &["Ctrl", "Meta", "F"],
            _ => return Err("Unsupported window action.".into()),
        };
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        return Err(format!(
            "Window action is not supported on this platform: {action}"
        ));
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        send_shortcut(self, keys)
    }
}

pub struct DesktopInput<I: InputInjector> {
    pub(crate) injector: I,
    held_modifiers: HashSet<String>,
    held_button: Option<MouseButton>,
}

impl<I: InputInjector> DesktopInput<I> {
    pub fn new(injector: I) -> Self {
        Self {
            injector,
            held_modifiers: HashSet::new(),
            held_button: None,
        }
    }
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.injector.inject_text(text)
    }
    pub fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.injector.move_pointer(dx, dy)
    }
    pub fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        self.injector.click_pointer(button, click_count)
    }

    pub fn execute(&mut self, command_type: &str, payload: &Value) -> Result<(), String> {
        match command_type {
            "mouse.scroll" => self
                .injector
                .scroll(number(payload, "dx", 50)?, number(payload, "dy", 50)?),
            "mouse.dragStart" => {
                let button = payload_button(payload)?;
                if let Some(active) = self.held_button.take() {
                    self.injector.set_pointer_button(active, false)?;
                }
                self.injector.set_pointer_button(button, true)?;
                self.held_button = Some(button);
                Ok(())
            }
            "mouse.dragEnd" => {
                if let Some(active) = self.held_button.take() {
                    self.injector.set_pointer_button(active, false)?;
                }
                Ok(())
            }
            "keyboard.key" | "keyboard.textStream.key" => {
                let key = string(payload, "key")?;
                self.injector.set_key(key, true)?;
                self.injector.set_key(key, false)
            }
            "keyboard.modifierDown" => {
                let key = string(payload, "key")?.to_owned();
                if self.held_modifiers.insert(key.clone()) {
                    self.injector.set_key(&key, true)?;
                }
                Ok(())
            }
            "keyboard.modifierUp" => {
                let key = string(payload, "key")?.to_owned();
                if self.held_modifiers.remove(&key) {
                    self.injector.set_key(&key, false)?;
                }
                Ok(())
            }
            "keyboard.shortcut" => {
                let keys = payload
                    .get("keys")
                    .and_then(Value::as_array)
                    .ok_or("Shortcut keys are required.")?
                    .iter()
                    .map(|key| {
                        key.as_str()
                            .map(str::to_owned)
                            .ok_or("Shortcut key is invalid.")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if keys.is_empty() || keys.len() > 6 {
                    return Err("Shortcut key count is invalid.".into());
                }
                self.injector.press_shortcut(&keys)
            }
            "keyboard.typeText" | "keyboard.textStream.char" | "keyboard.textStream.chunk" => {
                let text = string(payload, "text")?;
                if text.chars().count() > 2000 {
                    return Err("Text payload is too large.".into());
                }
                self.injector.inject_text(text)
            }
            "media.control" => self.injector.media(string(payload, "action")?),
            "window.control" => self.injector.window(string(payload, "action")?),
            "mouse.repeat.start"
            | "mouse.repeat.stop"
            | "keyboard.textStream.open"
            | "keyboard.textStream.close"
            | "grid.switch.set"
            | "grid.switch.sync"
            | "switch.profile.list"
            | "switch.session.start"
            | "switch.edge"
            | "switch.sync"
            | "switch.session.stop"
            | "pointer.display.move"
            | "pointer.speed.set"
            | "connection.disconnecting" => Ok(()),
            _ => Err(format!("Unsupported desktop command: {command_type}")),
        }
    }

    pub fn release_all(&mut self) -> Result<(), String> {
        if let Some(button) = self.held_button.take() {
            self.injector.set_pointer_button(button, false)?;
        }
        for key in self.held_modifiers.drain() {
            self.injector.set_key(&key, false)?;
        }
        Ok(())
    }
}

fn string<'a>(payload: &'a Value, key: &str) -> Result<&'a str, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} is required."))
}
fn number(payload: &Value, key: &str, max: i32) -> Result<i32, String> {
    let value = payload
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("{key} is required."))?;
    if !value.is_finite() || value.abs() > max as f64 {
        return Err(format!("{key} is out of range."));
    }
    Ok(value.round() as i32)
}
fn payload_button(payload: &Value) -> Result<MouseButton, String> {
    match string(payload, "button")? {
        "left" => Ok(MouseButton::Left),
        "middle" => Ok(MouseButton::Middle),
        "right" => Ok(MouseButton::Right),
        _ => Err("Mouse button is invalid.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct FakeInjector {
        text: Vec<String>,
        moves: Vec<(i32, i32)>,
        clicks: Vec<(MouseButton, u8)>,
        keys: Vec<(String, bool)>,
    }
    impl InputInjector for FakeInjector {
        fn inject_text(&mut self, text: &str) -> Result<(), String> {
            self.text.push(text.into());
            Ok(())
        }
        fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.moves.push((dx, dy));
            Ok(())
        }
        fn click_pointer(&mut self, button: MouseButton, count: u8) -> Result<(), String> {
            self.clicks.push((button, count));
            Ok(())
        }
        fn set_pointer_button(&mut self, _button: MouseButton, _down: bool) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
            Ok(())
        }
        fn set_key(&mut self, key: &str, down: bool) -> Result<(), String> {
            self.keys.push((key.into(), down));
            Ok(())
        }
        fn press_shortcut(&mut self, _keys: &[String]) -> Result<(), String> {
            Ok(())
        }
        fn media(&mut self, _action: &str) -> Result<(), String> {
            Ok(())
        }
        fn window(&mut self, _action: &str) -> Result<(), String> {
            Ok(())
        }
    }
    #[test]
    fn command_execution_uses_injected_text_without_system_input() {
        let mut input = DesktopInput::new(FakeInjector::default());
        input.type_text("Hello").unwrap();
        assert_eq!(input.injector.text, vec!["Hello"]);
    }
    #[test]
    fn command_execution_uses_relative_pointer_input_without_system_input() {
        let mut input = DesktopInput::new(FakeInjector::default());
        input.move_pointer(12, -6).unwrap();
        assert_eq!(input.injector.moves, vec![(12, -6)]);
    }
    #[test]
    fn held_modifiers_are_released_at_session_end() {
        let mut input = DesktopInput::new(FakeInjector::default());
        input
            .execute("keyboard.modifierDown", &serde_json::json!({"key":"Ctrl"}))
            .unwrap();
        input.release_all().unwrap();
        assert_eq!(
            input.injector.keys,
            vec![("Ctrl".into(), true), ("Ctrl".into(), false)]
        );
    }
}
