use std::collections::{HashMap, HashSet};

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse};
use serde_json::Value;

use crate::protocol::MouseButton;
use crate::state::{SwitchBinding, SwitchProfile};

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
    pointer_scale_percent: u32,
    text_streams: HashMap<String, TextStream>,
    switch_session: Option<SwitchSession>,
}

struct SwitchSession {
    device_id: String,
    session_id: String,
    profile: SwitchProfile,
    last_sequence: i64,
    pressed_switches: HashSet<u8>,
    held_outputs: HashMap<String, u8>,
}

struct TextStream {
    next_sequence: i64,
    failed: bool,
}

impl<I: InputInjector> DesktopInput<I> {
    pub fn new(injector: I) -> Self {
        Self {
            injector,
            held_modifiers: HashSet::new(),
            held_button: None,
            pointer_scale_percent: 100,
            text_streams: HashMap::new(),
            switch_session: None,
        }
    }
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.injector.inject_text(text)
    }
    pub fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        let scale = self.pointer_scale_percent as f64 / 100.0;
        self.injector.move_pointer(
            (dx as f64 * scale).round() as i32,
            (dy as f64 * scale).round() as i32,
        )
    }
    pub fn set_pointer_scale_percent(&mut self, scale_percent: u8) {
        self.pointer_scale_percent = u32::from(scale_percent.clamp(5, 225));
    }
    pub fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        self.injector.click_pointer(button, click_count)
    }

    pub fn execute(
        &mut self,
        device_id: &str,
        command_type: &str,
        payload: &Value,
        profiles: &[SwitchProfile],
    ) -> Result<(), String> {
        if self.switch_session.is_some()
            && !matches!(
                command_type,
                "grid.switch.set"
                    | "grid.switch.sync"
                    | "switch.profile.list"
                    | "switch.session.start"
                    | "switch.edge"
                    | "switch.sync"
                    | "switch.session.stop"
                    | "connection.disconnecting"
            )
        {
            return Err("Stop PC Switch Control before using other PC control commands.".into());
        }
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
            "keyboard.key" => {
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
            "keyboard.typeText" => {
                let text = string(payload, "text")?;
                if text.chars().count() > 2000 {
                    return Err("Text payload is too large.".into());
                }
                self.injector.inject_text(text)
            }
            "media.control" => self.injector.media(string(payload, "action")?),
            "window.control" => self.injector.window(string(payload, "action")?),
            "keyboard.textStream.open" => {
                let key = text_stream_key(device_id, string(payload, "streamId")?);
                self.text_streams.insert(
                    key,
                    TextStream {
                        next_sequence: 0,
                        failed: false,
                    },
                );
                Ok(())
            }
            "keyboard.textStream.char" | "keyboard.textStream.chunk" => {
                let key = text_stream_key(device_id, string(payload, "streamId")?);
                if !self.begin_text_stream_item(&key, integer(payload, "seq")?)? {
                    return Ok(());
                }
                let text = string(payload, "text")?;
                if text.chars().count() > 2_000 {
                    return Err("Text payload is too large.".into());
                }
                let result = self.injector.inject_text(text);
                if result.is_err() {
                    self.mark_text_stream_failed(&key);
                }
                result
            }
            "keyboard.textStream.key" => {
                let stream_key = text_stream_key(device_id, string(payload, "streamId")?);
                if !self.begin_text_stream_item(&stream_key, integer(payload, "seq")?)? {
                    return Ok(());
                }
                let key = string(payload, "key")?;
                let result = self
                    .injector
                    .set_key(key, true)
                    .and_then(|_| self.injector.set_key(key, false));
                if result.is_err() {
                    self.mark_text_stream_failed(&stream_key);
                }
                result
            }
            "keyboard.textStream.close" => {
                let key = text_stream_key(device_id, string(payload, "streamId")?);
                let expected = integer(payload, "expectedCount")?;
                let stream = self
                    .text_streams
                    .remove(&key)
                    .ok_or_else(|| "Text stream is not open.".to_string())?;
                if stream.failed || stream.next_sequence != expected {
                    Err("Text stream did not receive every item.".into())
                } else {
                    Ok(())
                }
            }
            "switch.profile.list" => Ok(()),
            "switch.session.start" => self.start_switch_session(device_id, payload, profiles),
            "switch.edge" => self.apply_switch_edge(device_id, payload),
            "switch.sync" => self.sync_switches(device_id, payload),
            "switch.session.stop" => self.stop_switch_session_command(device_id, payload),
            "pointer.speed.set" => {
                let scale = payload
                    .get("scalePercent")
                    .and_then(Value::as_f64)
                    .filter(|scale| scale.is_finite() && *scale > 0.0)
                    .ok_or_else(|| "Pointer speed is invalid.".to_string())?;
                self.pointer_scale_percent = ((scale / 5.0).round() * 5.0).clamp(5.0, 225.0) as u32;
                Ok(())
            }
            "connection.disconnecting" => self.release_all(),
            "mouse.repeat.start" | "mouse.repeat.stop" => {
                Err("Mouse repeat is not available in this preview build.".into())
            }
            "grid.switch.set" => self.apply_legacy_grid_edge(device_id, payload, profiles),
            "grid.switch.sync" => self.sync_legacy_grid(device_id, payload, profiles),
            "pointer.display.move" => {
                Err("Display navigation is not available in this preview build.".into())
            }
            _ => Err(format!("Unsupported desktop command: {command_type}")),
        }
    }

    fn begin_text_stream_item(&mut self, key: &str, sequence: i64) -> Result<bool, String> {
        let stream = self
            .text_streams
            .get_mut(key)
            .ok_or_else(|| "Text stream is not open.".to_string())?;
        if stream.failed {
            return Err("Text stream has failed.".into());
        }
        if sequence < stream.next_sequence {
            return Ok(false);
        }
        if sequence > stream.next_sequence {
            stream.failed = true;
            stream.next_sequence = sequence + 1;
            return Err("Text stream sequence mismatch.".into());
        }
        stream.next_sequence += 1;
        Ok(true)
    }

    fn mark_text_stream_failed(&mut self, key: &str) {
        if let Some(stream) = self.text_streams.get_mut(key) {
            stream.failed = true;
        }
    }

    fn start_switch_session(
        &mut self,
        device_id: &str,
        payload: &Value,
        profiles: &[SwitchProfile],
    ) -> Result<(), String> {
        let session_id = string(payload, "sessionId")?;
        uuid::Uuid::parse_str(session_id).map_err(|_| "Session ID must be a UUID.".to_string())?;
        let profile_id = string(payload, "profileId")?;
        let profile_version = integer(payload, "profileVersion")? as u32;
        let switch_count = integer(payload, "switchCount")?;
        if !(1..=8).contains(&switch_count) {
            return Err("Switch count is invalid.".into());
        }
        let profile = profiles
            .iter()
            .find(|profile| profile.id == profile_id && profile.version == profile_version)
            .cloned()
            .ok_or_else(|| "The selected profile changed or is no longer available.".to_string())?;
        if profile.provider != "mapped" && profile.provider != "grid3" {
            return Err("The selected output provider is unavailable.".into());
        }
        #[cfg(not(target_os = "windows"))]
        if profile.provider == "grid3" {
            return Err("The selected output provider is unavailable.".into());
        }
        self.release_all()?;
        self.switch_session = Some(SwitchSession {
            device_id: device_id.into(),
            session_id: session_id.into(),
            profile,
            last_sequence: 0,
            pressed_switches: HashSet::new(),
            held_outputs: HashMap::new(),
        });
        Ok(())
    }

    fn ensure_grid_session(
        &mut self,
        device_id: &str,
        session_id: &str,
        _profiles: &[SwitchProfile],
    ) -> Result<(), String> {
        if self.switch_session.as_ref().is_some_and(|session| {
            session.device_id == device_id
                && session.session_id == session_id
                && session.profile.provider == "grid3"
        }) {
            return Ok(());
        }
        #[cfg(not(target_os = "windows"))]
        return Err("Grid 3 output is unavailable.".into());
        #[cfg(target_os = "windows")]
        {
            let profile = _profiles
                .iter()
                .find(|profile| profile.provider == "grid3")
                .cloned()
                .ok_or_else(|| "Grid 3 output is unavailable.".to_string())?;
            self.release_all()?;
            self.switch_session = Some(SwitchSession {
                device_id: device_id.into(),
                session_id: session_id.into(),
                profile,
                last_sequence: 0,
                pressed_switches: HashSet::new(),
                held_outputs: HashMap::new(),
            });
            Ok(())
        }
    }

    fn apply_legacy_grid_edge(
        &mut self,
        device_id: &str,
        payload: &Value,
        profiles: &[SwitchProfile],
    ) -> Result<(), String> {
        let owned_session_id;
        let session_id = if let Some(session_id) = payload.get("sessionId").and_then(Value::as_str)
        {
            session_id
        } else {
            owned_session_id = format!("legacy:{device_id}");
            &owned_session_id
        };
        self.ensure_grid_session(device_id, session_id, profiles)?;
        let mut session = self.take_matching_session(device_id, session_id)?;
        let sequence = payload
            .get("sequence")
            .and_then(Value::as_i64)
            .unwrap_or(session.last_sequence + 1);
        let switch_id = integer(payload, "switchId")? as u8;
        let pressed = string(payload, "state")? == "down";
        let result = if sequence > session.last_sequence {
            let result = self.apply_binding(&mut session, switch_id, pressed);
            if result.is_ok() {
                session.last_sequence = sequence;
            }
            result
        } else {
            Ok(())
        };
        self.switch_session = Some(session);
        result
    }

    fn sync_legacy_grid(
        &mut self,
        device_id: &str,
        payload: &Value,
        profiles: &[SwitchProfile],
    ) -> Result<(), String> {
        let session_id = string(payload, "sessionId")?;
        self.ensure_grid_session(device_id, session_id, profiles)?;
        self.sync_switches(device_id, payload)
    }

    fn apply_switch_edge(&mut self, device_id: &str, payload: &Value) -> Result<(), String> {
        let session_id = string(payload, "sessionId")?;
        let sequence = integer(payload, "sequence")?;
        let switch_id = integer(payload, "switchId")? as u8;
        let pressed = match string(payload, "state")? {
            "down" => true,
            "up" => false,
            _ => return Err("Switch state is invalid.".into()),
        };
        if !(1..=8).contains(&switch_id) || sequence < 1 {
            return Err("Switch edge payload is invalid.".into());
        }
        let mut session = self.take_matching_session(device_id, session_id)?;
        let result = if sequence > session.last_sequence {
            let result = self.apply_binding(&mut session, switch_id, pressed);
            if result.is_ok() {
                session.last_sequence = sequence;
            }
            result
        } else {
            Ok(())
        };
        self.switch_session = Some(session);
        result
    }

    fn sync_switches(&mut self, device_id: &str, payload: &Value) -> Result<(), String> {
        let session_id = string(payload, "sessionId")?;
        let sequence = integer(payload, "sequence")?;
        let pressed: HashSet<u8> = payload
            .get("pressedSwitchIds")
            .and_then(Value::as_array)
            .ok_or("Pressed switch IDs are required.")?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .filter(|id| (1..=8).contains(id))
                    .map(|id| id as u8)
                    .ok_or("Pressed switch ID is invalid.")
            })
            .collect::<Result<_, _>>()?;
        let mut session = self.take_matching_session(device_id, session_id)?;
        let result = if sequence > session.last_sequence {
            let current = session.pressed_switches.clone();
            let mut result = Ok(());
            for switch_id in current.difference(&pressed).copied().collect::<Vec<_>>() {
                if let Err(error) = self.apply_binding(&mut session, switch_id, false) {
                    result = Err(error);
                    break;
                }
            }
            if result.is_ok() {
                for switch_id in pressed.difference(&current).copied().collect::<Vec<_>>() {
                    if let Err(error) = self.apply_binding(&mut session, switch_id, true) {
                        result = Err(error);
                        break;
                    }
                }
            }
            if result.is_ok() {
                session.last_sequence = sequence;
            }
            result
        } else {
            Ok(())
        };
        self.switch_session = Some(session);
        result
    }

    fn stop_switch_session_command(
        &mut self,
        device_id: &str,
        payload: &Value,
    ) -> Result<(), String> {
        let session_id = string(payload, "sessionId")?;
        let sequence = integer(payload, "sequence")?;
        if self.switch_session.as_ref().is_some_and(|session| {
            session.device_id == device_id
                && session.session_id == session_id
                && sequence > session.last_sequence
        }) {
            self.stop_switch_session()?;
        }
        Ok(())
    }

    fn take_matching_session(
        &mut self,
        device_id: &str,
        session_id: &str,
    ) -> Result<SwitchSession, String> {
        match self.switch_session.take() {
            Some(session) if session.device_id == device_id && session.session_id == session_id => {
                Ok(session)
            }
            Some(session) => {
                self.switch_session = Some(session);
                Err("The switch-control session is not active.".into())
            }
            None => Err("The switch-control session is not active.".into()),
        }
    }

    fn apply_binding(
        &mut self,
        session: &mut SwitchSession,
        switch_id: u8,
        pressed: bool,
    ) -> Result<(), String> {
        let changed = if pressed {
            session.pressed_switches.insert(switch_id)
        } else {
            session.pressed_switches.remove(&switch_id)
        };
        if !changed {
            return Ok(());
        }
        if session.profile.provider == "grid3" {
            #[cfg(target_os = "windows")]
            {
                let result = crate::grid3::set_switch_state(switch_id, pressed);
                if result.is_err() {
                    if pressed {
                        session.pressed_switches.remove(&switch_id);
                    } else {
                        session.pressed_switches.insert(switch_id);
                    }
                }
                return result;
            }
            #[cfg(not(target_os = "windows"))]
            return Err("Grid 3 output is unavailable.".into());
        }
        let Some(binding) = session
            .profile
            .bindings
            .iter()
            .find(|binding| binding.switch_id == switch_id)
            .cloned()
        else {
            return Ok(());
        };
        let result = match binding.binding_type.as_str() {
            "key" | "mouseButton" => self.apply_stateful_output(session, &binding, pressed),
            "none" => Ok(()),
            _ if !pressed => Ok(()),
            "shortcut" => {
                let keys = binding.keys.clone().unwrap_or_else(|| {
                    binding
                        .value
                        .as_deref()
                        .unwrap_or("")
                        .split('+')
                        .filter(|key| !key.is_empty())
                        .map(str::to_owned)
                        .collect()
                });
                self.injector.press_shortcut(&keys)
            }
            "mouseClick" => self.injector.click_pointer(
                parse_button(binding.value.as_deref().unwrap_or("left"))?,
                binding.click_count.unwrap_or(1).clamp(1, 2),
            ),
            "scroll" => match binding.value.as_deref() {
                Some("up") => self.injector.scroll(0, 1),
                Some("down") => self.injector.scroll(0, -1),
                Some("left") => self.injector.scroll(-1, 0),
                Some("right") => self.injector.scroll(1, 0),
                _ => Err("Scroll direction is invalid.".into()),
            },
            "media" => self.injector.media(binding.value.as_deref().unwrap_or("")),
            _ => Err("Switch binding is invalid.".into()),
        };
        if result.is_err() {
            if pressed {
                session.pressed_switches.remove(&switch_id);
            } else {
                session.pressed_switches.insert(switch_id);
            }
        }
        result
    }

    fn apply_stateful_output(
        &mut self,
        session: &mut SwitchSession,
        binding: &SwitchBinding,
        pressed: bool,
    ) -> Result<(), String> {
        let value = binding
            .value
            .as_deref()
            .ok_or("Switch binding value is required.")?;
        let output = format!(
            "{}:{value}",
            if binding.binding_type == "key" {
                "key"
            } else {
                "mouse"
            }
        );
        let count = session.held_outputs.get(&output).copied().unwrap_or(0);
        let next = if pressed {
            count.saturating_add(1)
        } else {
            count.saturating_sub(1)
        };
        if count == 0 && next == 1 {
            self.set_output(&output, true)?;
        }
        if count == 1 && next == 0 {
            self.set_output(&output, false)?;
        }
        if next == 0 {
            session.held_outputs.remove(&output);
        } else {
            session.held_outputs.insert(output, next);
        }
        Ok(())
    }

    fn set_output(&mut self, output: &str, down: bool) -> Result<(), String> {
        let (kind, value) = output.split_once(':').ok_or("Switch output is invalid.")?;
        match kind {
            "key" => self.injector.set_key(value, down),
            "mouse" => self.injector.set_pointer_button(parse_button(value)?, down),
            _ => Err("Switch output is invalid.".into()),
        }
    }

    fn stop_switch_session(&mut self) -> Result<(), String> {
        if let Some(session) = self.switch_session.take() {
            let mut first_error = None;
            #[cfg(target_os = "windows")]
            if session.profile.provider == "grid3" {
                for switch_id in session.pressed_switches {
                    if let Err(error) = crate::grid3::set_switch_state(switch_id, false) {
                        first_error.get_or_insert(error);
                    }
                }
            }
            for output in session.held_outputs.keys() {
                if let Err(error) = self.set_output(output, false) {
                    first_error.get_or_insert(error);
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn release_all(&mut self) -> Result<(), String> {
        self.stop_switch_session()?;
        self.text_streams.clear();
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
fn integer(payload: &Value, key: &str) -> Result<i64, String> {
    payload
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("{key} is required."))
}
fn text_stream_key(device_id: &str, stream_id: &str) -> String {
    format!("{device_id}\0{stream_id}")
}
fn parse_button(value: &str) -> Result<MouseButton, String> {
    match value {
        "left" => Ok(MouseButton::Left),
        "middle" => Ok(MouseButton::Middle),
        "right" => Ok(MouseButton::Right),
        _ => Err("Mouse button is invalid.".into()),
    }
}
fn payload_button(payload: &Value) -> Result<MouseButton, String> {
    parse_button(string(payload, "button")?)
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
            .execute(
                "device",
                "keyboard.modifierDown",
                &serde_json::json!({"key":"Ctrl"}),
                &[],
            )
            .unwrap();
        input.release_all().unwrap();
        assert_eq!(
            input.injector.keys,
            vec![("Ctrl".into(), true), ("Ctrl".into(), false)]
        );
    }

    #[test]
    fn switch_session_ignores_duplicate_sequences_and_releases_stateful_keys() {
        let profile = SwitchProfile {
            id: "custom".into(),
            version: 3,
            name: "Custom".into(),
            provider: "mapped".into(),
            built_in: false,
            bindings: vec![SwitchBinding {
                switch_id: 1,
                binding_type: "key".into(),
                value: Some("Space".into()),
                keys: None,
                click_count: None,
            }],
        };
        let mut input = DesktopInput::new(FakeInjector::default());
        let session_id = uuid::Uuid::new_v4().to_string();
        input
            .execute(
                "device",
                "switch.session.start",
                &serde_json::json!({
                    "sessionId": session_id,
                    "profileId": "custom",
                    "profileVersion": 3,
                    "switchCount": 1
                }),
                std::slice::from_ref(&profile),
            )
            .unwrap();
        for sequence in [1, 1] {
            input
                .execute(
                    "device",
                    "switch.edge",
                    &serde_json::json!({
                        "sessionId": session_id,
                        "sequence": sequence,
                        "switchId": 1,
                        "state": "down"
                    }),
                    std::slice::from_ref(&profile),
                )
                .unwrap();
        }
        input
            .execute(
                "device",
                "switch.session.stop",
                &serde_json::json!({"sessionId": session_id, "sequence": 2}),
                &[profile],
            )
            .unwrap();
        assert_eq!(
            input.injector.keys,
            vec![("Space".into(), true), ("Space".into(), false)]
        );
    }
}
