use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(not(target_os = "windows"))]
use enigo::Coordinate;
use enigo::{Axis, Button, Direction, Enigo, Key, Keyboard, Mouse};
use serde_json::Value;

use crate::modifier_overlay::ModifierKeyOverlayNotifier;
use crate::mouse_repeat::RepeatKey;
use crate::protocol::MouseButton;
use crate::state::{normalize_pointer_scale_percent, AppModel, SwitchBinding, SwitchProfile};

pub const SCAN_EVENT_MARKER: i64 = 0x53575043;
pub fn injection_settings() -> enigo::Settings {
    enigo::Settings {
        event_source_user_data: Some(SCAN_EVENT_MARKER),
        windows_dw_extra_info: Some(SCAN_EVENT_MARKER as usize),
        ..Default::default()
    }
}
pub fn own_input(marker: i64) -> bool {
    marker == SCAN_EVENT_MARKER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerFeedback {
    Move,
    Drag,
    DwellProgress { permille: u16 },
    RepeatMove { accelerated: bool, dragging: bool },
    RepeatScroll { dx: i32, dy: i32 },
    Click { button: MouseButton, count: u8 },
    Scroll { dx: i32, dy: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DesktopCommandOutcome {
    pub pointer_feedback: Option<PointerFeedback>,
    pub typing_injected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobileTypingRoute {
    eligible: bool,
}

impl MobileTypingRoute {
    pub fn for_text(text: &str) -> Self {
        Self {
            eligible: !text.is_empty(),
        }
    }

    pub fn for_command(command_type: &str) -> Self {
        Self {
            eligible: matches!(
                command_type,
                "keyboard.key"
                    | "keyboard.shortcut"
                    | "keyboard.typeText"
                    | "keyboard.textStream.char"
                    | "keyboard.textStream.chunk"
                    | "keyboard.textStream.key"
            ),
        }
    }

    pub fn prepare(self, cancel_dwell: impl FnOnce(), stop_repeats: impl FnOnce()) {
        if self.eligible {
            cancel_dwell();
            stop_repeats();
        }
    }

    pub fn finish(self, typing_injected: bool, hide_overlay: impl FnOnce()) {
        if self.eligible && typing_injected {
            hide_overlay();
        }
    }

    pub fn is_eligible(self) -> bool {
        self.eligible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

impl ModifierKey {
    pub const DISPLAY_ORDER: [Self; 4] = [Self::Ctrl, Self::Alt, Self::Shift, Self::Meta];
    const RELEASE_ORDER: [Self; 4] = [Self::Meta, Self::Shift, Self::Alt, Self::Ctrl];

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Ctrl" => Ok(Self::Ctrl),
            "Alt" => Ok(Self::Alt),
            "Shift" => Ok(Self::Shift),
            "Meta" => Ok(Self::Meta),
            _ => Err("Modifier key is invalid.".into()),
        }
    }

    fn protocol_name(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
            Self::Meta => "Meta",
        }
    }
}

/// A key in a shortcut, after Windows has found the key for each symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordKey {
    Named(String),
    #[cfg(target_os = "windows")]
    Virtual(u16),
}

pub trait InputInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), String>;
    #[cfg_attr(
        target_os = "macos",
        allow(dead_code, reason = "Windows keeps relative pointer injection")
    )]
    fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn move_pointer_absolute(&mut self, x: i32, y: i32) -> Result<(), String>;
    fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String>;
    fn click_pointer_at(
        &mut self,
        point: (i32, i32),
        button: MouseButton,
        count: u8,
        _modifiers: &[&str],
    ) -> Result<(), String> {
        self.move_pointer_absolute(point.0, point.1)?;
        self.click_pointer(button, count)
    }
    fn set_pointer_button(&mut self, button: MouseButton, down: bool) -> Result<(), String>;
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn set_key(&mut self, key: &str, down: bool) -> Result<(), String>;
    /// The keys a shortcut presses, in order. Resolved once, so the release
    /// lifts the same keys even if focus or the layout changes in between.
    fn chord_keys(&mut self, keys: &[&str]) -> Result<Vec<ChordKey>, String> {
        Ok(keys
            .iter()
            .map(|key| ChordKey::Named((*key).into()))
            .collect())
    }
    fn set_chord_key(&mut self, key: &ChordKey, down: bool) -> Result<(), String> {
        match key {
            ChordKey::Named(name) => self.set_key(name, down),
            #[cfg(target_os = "windows")]
            ChordKey::Virtual(_) => Err("Unsupported key.".into()),
        }
    }
    /// Presses and releases a shortcut. `shift_held` says Shift is already
    /// down for the remote, so a symbol that needs Shift does not press and
    /// release it again.
    fn press_shortcut(&mut self, keys: &[String], shift_held: bool) -> Result<(), String>;
    fn media(&mut self, action: &str) -> Result<(), String>;
    fn window(&mut self, action: &str) -> Result<(), String>;
    /// Asks the operating system to shut down or restart; it closes applications itself.
    fn power(&mut self, action: crate::power::Action) -> Result<(), String>;
}

fn pointer_button(button: MouseButton) -> Button {
    match button {
        MouseButton::Left => Button::Left,
        MouseButton::Middle => Button::Middle,
        MouseButton::Right => Button::Right,
    }
}

#[cfg(target_os = "windows")]
fn windows_alphanumeric_key(name: &str) -> Option<Key> {
    Some(match name {
        #[cfg(target_os = "windows")]
        "Insert" => Key::Insert,
        #[cfg(target_os = "windows")]
        "PrintScreen" => Key::PrintScr,
        #[cfg(target_os = "windows")]
        "ScrollLock" => Key::Scroll,
        #[cfg(target_os = "windows")]
        "Pause" => Key::Pause,
        // Fixed keys, not looked up on the layout like other symbols: the
        // zoom shortcuts (Ctrl and = or -) need the key code apps listen for.
        "=" => Key::OEMPlus,
        "-" => Key::OEMMinus,
        "[" => Key::OEM4,
        "]" => Key::OEM6,
        "A" => Key::A,
        "B" => Key::B,
        "C" => Key::C,
        "D" => Key::D,
        "E" => Key::E,
        "F" => Key::F,
        "G" => Key::G,
        "H" => Key::H,
        "I" => Key::I,
        "J" => Key::J,
        "K" => Key::K,
        "L" => Key::L,
        "M" => Key::M,
        "N" => Key::N,
        "O" => Key::O,
        "P" => Key::P,
        "Q" => Key::Q,
        "R" => Key::R,
        "S" => Key::S,
        "T" => Key::T,
        "U" => Key::U,
        "V" => Key::V,
        "W" => Key::W,
        "X" => Key::X,
        "Y" => Key::Y,
        "Z" => Key::Z,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        _ => return None,
    })
}

#[cfg(target_os = "windows")]
const SYMBOL_SHORTCUT_ERROR: &str =
    "This symbol cannot be used in a shortcut on this keyboard layout.";

/// The key for a symbol on the foreground window's keyboard layout, and
/// whether it needs Shift. `scan` is the `VkKeyScanExW` result. Symbols that
/// need Ctrl or Alt (AltGr) are refused, so a shortcut never sends its
/// modifiers alone.
#[cfg(target_os = "windows")]
fn windows_symbol_key(scan: i16) -> Result<(bool, u16), String> {
    if scan < 0 {
        return Err(SYMBOL_SHORTCUT_ERROR.into());
    }
    let shift_state = (scan as u16) >> 8;
    if shift_state & !1 != 0 {
        return Err(SYMBOL_SHORTCUT_ERROR.into());
    }
    Ok((shift_state == 1, scan as u16 & 0xff))
}

/// The `VkKeyScanExW` result for a UTF-16 unit on the foreground window's
/// keyboard layout.
#[cfg(target_os = "windows")]
fn foreground_layout_scan(unit: u16) -> i16 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, VkKeyScanExW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    // SAFETY: These calls only read the foreground thread's keyboard layout.
    unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        VkKeyScanExW(unit, GetKeyboardLayout(thread))
    }
}

/// The keys a shortcut presses on Windows. Each symbol in a shortcut becomes
/// the key that types it on the layout `scan` reads, with Shift when the
/// layout needs it and the shortcut does not already hold Shift. A symbol is
/// looked up even when it is the only key left, as when the remote already
/// holds the shortcut's modifiers.
#[cfg(target_os = "windows")]
fn windows_chord_keys(keys: &[&str], scan: impl Fn(u16) -> i16) -> Result<Vec<ChordKey>, String> {
    let shift = ChordKey::Named("Shift".into());
    let mut chord = Vec::with_capacity(keys.len() + 1);
    for key in keys {
        let symbol = match named_key(key) {
            Some(Key::Unicode(symbol)) => symbol,
            _ => {
                chord.push(ChordKey::Named((*key).into()));
                continue;
            }
        };
        let mut unit = [0; 2];
        let [unit] = symbol.encode_utf16(&mut unit) else {
            return Err(SYMBOL_SHORTCUT_ERROR.into());
        };
        let (needs_shift, virtual_key) = windows_symbol_key(scan(*unit))?;
        if needs_shift && !keys.contains(&"Shift") && !chord.contains(&shift) {
            chord.push(shift.clone());
        }
        chord.push(ChordKey::Virtual(virtual_key));
    }
    Ok(chord)
}

/// The enigo key for a resolved shortcut key.
fn chord_enigo_key(key: &ChordKey) -> Result<Key, String> {
    match key {
        ChordKey::Named(name) => named_key(name).ok_or_else(|| format!("Unsupported key: {name}")),
        #[cfg(target_os = "windows")]
        ChordKey::Virtual(code) => Ok(Key::Other((*code).into())),
    }
}

fn named_key(name: &str) -> Option<Key> {
    #[cfg(target_os = "windows")]
    if let Some(key) = windows_alphanumeric_key(name) {
        return Some(key);
    }

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

fn enigo_scroll_delta(dx: i32, dy: i32) -> (i32, i32) {
    (dx, -dy)
}

#[cfg(target_os = "windows")]
const WINDOWS_POINTER_MOVE_ERROR: &str = "The operating system could not move the pointer.";

#[cfg(target_os = "windows")]
trait WindowsCursorPosition {
    fn position(&mut self) -> Result<(i32, i32), String>;
    fn set_position(&mut self, x: i32, y: i32) -> Result<(), String>;
}

#[cfg(target_os = "windows")]
struct SystemWindowsCursor;

#[cfg(target_os = "windows")]
impl WindowsCursorPosition for SystemWindowsCursor {
    fn position(&mut self) -> Result<(i32, i32), String> {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) }
            .map(|()| (point.x, point.y))
            .map_err(|_| WINDOWS_POINTER_MOVE_ERROR.into())
    }

    fn set_position(&mut self, x: i32, y: i32) -> Result<(), String> {
        unsafe { windows::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y) }
            .map_err(|_| WINDOWS_POINTER_MOVE_ERROR.into())
    }
}

#[cfg(target_os = "windows")]
fn move_windows_pointer_relative(
    cursor: &mut impl WindowsCursorPosition,
    dx: i32,
    dy: i32,
) -> Result<(), String> {
    let (x, y) = cursor
        .position()
        .map_err(|_| WINDOWS_POINTER_MOVE_ERROR.to_string())?;
    cursor
        .set_position(x.saturating_add(dx), y.saturating_add(dy))
        .map_err(|_| WINDOWS_POINTER_MOVE_ERROR.to_string())
}

fn run_shortcut_sequence<K: Copy>(
    keys: &[K],
    mut send: impl FnMut(K, Direction) -> Result<(), String>,
) -> Result<(), String> {
    let mut pressed = Vec::with_capacity(keys.len());
    let mut first_error = None;

    for key in keys {
        match send(*key, Direction::Press) {
            Ok(()) => pressed.push(*key),
            Err(error) => {
                first_error = Some(error);
                break;
            }
        }
    }

    for key in pressed.into_iter().rev() {
        if let Err(error) = send(key, Direction::Release) {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Leaves out Shift when the remote already holds it, so the shortcut does
/// not release it. Shift stays down either way.
fn without_held_shift(chord: Vec<ChordKey>, shift_held: bool) -> Vec<ChordKey> {
    let shift = ChordKey::Named("Shift".into());
    chord
        .into_iter()
        .filter(|key| !(shift_held && *key == shift))
        .collect()
}

fn send_shortcut(enigo: &mut Enigo, keys: &[&str], shift_held: bool) -> Result<(), String> {
    let chord = enigo.chord_keys(keys)?;
    let parsed: Vec<Key> = without_held_shift(chord, shift_held)
        .iter()
        .map(chord_enigo_key)
        .collect::<Result<_, _>>()?;
    run_shortcut_sequence(&parsed, |key, direction| {
        let action = if direction == Direction::Press {
            "press the shortcut"
        } else {
            "release the shortcut"
        };
        enigo.key(key, direction).map_err(enigo_error(action))
    })
}

impl InputInjector for Enigo {
    fn inject_text(&mut self, text: &str) -> Result<(), String> {
        self.text(text)
            .map_err(enigo_error("type the received text"))
    }
    fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            move_windows_pointer_relative(&mut SystemWindowsCursor, dx, dy)
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.move_mouse(dx, dy, Coordinate::Rel)
                .map_err(enigo_error("move the pointer"))
        }
    }
    fn move_pointer_absolute(&mut self, x: i32, y: i32) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y).map_err(|_| {
                "The operating system could not move the pointer to another monitor.".into()
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.move_mouse(x, y, Coordinate::Abs)
                .map_err(enigo_error("move the pointer to another monitor"))
        }
    }
    fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        for _ in 0..click_count {
            self.button(pointer_button(button), Direction::Click)
                .map_err(enigo_error("click the pointer"))?;
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    fn click_pointer_at(
        &mut self,
        point: (i32, i32),
        button: MouseButton,
        count: u8,
        modifiers: &[&str],
    ) -> Result<(), String> {
        crate::macos_point_click::post(point, button, count, modifiers)
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
        let (dx, dy) = enigo_scroll_delta(dx, dy);
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
    #[cfg(target_os = "windows")]
    fn chord_keys(&mut self, keys: &[&str]) -> Result<Vec<ChordKey>, String> {
        windows_chord_keys(keys, foreground_layout_scan)
    }
    fn set_chord_key(&mut self, key: &ChordKey, down: bool) -> Result<(), String> {
        self.key(
            chord_enigo_key(key)?,
            if down {
                Direction::Press
            } else {
                Direction::Release
            },
        )
        .map_err(enigo_error("send the key"))
    }
    fn press_shortcut(&mut self, keys: &[String], shift_held: bool) -> Result<(), String> {
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        send_shortcut(self, &refs, shift_held)
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
        #[cfg(target_os = "macos")]
        if action == "closeFocused" {
            return crate::scan_host::close_foreground_window();
        }
        #[cfg(target_os = "windows")]
        if matches!(action, "minimizeFocused" | "maximizeFocused") {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let window = unsafe { GetForegroundWindow() };
            if window.is_null() {
                return Err("No foreground window.".into());
            }
            let command = if action == "minimizeFocused" {
                SW_MINIMIZE
            } else if unsafe { IsZoomed(window) } != 0 {
                SW_RESTORE
            } else {
                SW_MAXIMIZE
            };
            unsafe {
                ShowWindow(window, command);
            }
            return Ok(());
        }
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
        send_shortcut(self, keys, false)
    }
    fn power(&mut self, action: crate::power::Action) -> Result<(), String> {
        crate::power::request(action)
    }
}

pub struct DesktopInput<I: InputInjector> {
    pub(crate) injector: I,
    scan_keys: Vec<ChordKey>,
    held_modifiers: HashSet<ModifierKey>,
    pending_modifier_releases: HashSet<ModifierKey>,
    pending_key_releases: HashSet<RepeatKey>,
    modifier_overlay: Option<Arc<dyn ModifierKeyOverlayNotifier>>,
    held_button: Option<MouseButton>,
    pointer_scale_percent: u32,
    text_streams: HashMap<String, TextStream>,
    switch_session: Option<SwitchSession>,
    pointer_feedback: Option<PointerFeedback>,
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
            scan_keys: vec![],
            held_modifiers: HashSet::new(),
            pending_modifier_releases: HashSet::new(),
            pending_key_releases: HashSet::new(),
            modifier_overlay: None,
            held_button: None,
            pointer_scale_percent: 100,
            text_streams: HashMap::new(),
            switch_session: None,
            pointer_feedback: None,
        }
    }

    pub fn with_modifier_overlay(
        injector: I,
        modifier_overlay: Arc<dyn ModifierKeyOverlayNotifier>,
    ) -> Self {
        let mut input = Self::new(injector);
        input.modifier_overlay = Some(modifier_overlay);
        input
    }
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.injector.inject_text(text)
    }
    #[cfg_attr(
        target_os = "macos",
        allow(dead_code, reason = "Windows keeps relative pointer injection")
    )]
    pub fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        let (dx, dy) = self.scaled_pointer_delta(dx, dy);
        self.injector.move_pointer(dx, dy)
    }
    pub fn scaled_pointer_delta(&self, dx: i32, dy: i32) -> (i32, i32) {
        let scale = self.pointer_scale_percent as f64 / 100.0;
        (
            (dx as f64 * scale).round() as i32,
            (dy as f64 * scale).round() as i32,
        )
    }
    pub fn set_pointer_scale_percent(&mut self, scale_percent: u16) {
        self.pointer_scale_percent = u32::from(scale_percent.clamp(5, 1350));
    }
    #[cfg_attr(
        target_os = "macos",
        allow(dead_code, reason = "Windows keeps relative repeat injection")
    )]
    pub fn move_pointer_pixels(&mut self, dx: i32, dy: i32) -> Result<PointerFeedback, String> {
        self.injector.move_pointer(dx, dy)?;
        Ok(self.pointer_feedback_for_move())
    }
    #[cfg_attr(
        not(target_os = "macos"),
        allow(dead_code, reason = "macOS uses bounded absolute pointer injection")
    )]
    pub fn move_pointer_pixels_absolute(
        &mut self,
        x: i32,
        y: i32,
    ) -> Result<PointerFeedback, String> {
        self.injector.move_pointer_absolute(x, y)?;
        Ok(self.pointer_feedback_for_move())
    }
    pub fn has_active_drag(&self) -> bool {
        self.held_button.is_some()
    }
    pub fn has_active_switch_session(&self) -> bool {
        self.switch_session.is_some()
    }
    pub fn move_pointer_absolute(&mut self, x: i32, y: i32) -> Result<PointerFeedback, String> {
        if self.has_active_drag() {
            return Err("End the active drag before moving to another monitor.".into());
        }
        self.injector.move_pointer_absolute(x, y)?;
        self.pointer_feedback = Some(PointerFeedback::Move);
        Ok(PointerFeedback::Move)
    }
    pub fn execute_repeat_scroll(&mut self, dx: i32, dy: i32) -> Result<PointerFeedback, String> {
        self.injector.scroll(dx, dy)?;
        Ok(PointerFeedback::Scroll { dx, dy })
    }
    pub fn start_scan_drag(&mut self, point: (i32, i32)) -> Result<(), String> {
        if self.has_active_drag() || self.has_active_switch_session() {
            return Err("Another input session is active.".into());
        }
        self.injector.move_pointer_absolute(point.0, point.1)?;
        self.held_button = Some(MouseButton::Left);
        self.injector.set_pointer_button(MouseButton::Left, true)
    }
    pub fn move_scan_drag(&mut self, point: (i32, i32)) -> Result<(), String> {
        if self.held_button != Some(MouseButton::Left) {
            return Err("No local drag is active.".into());
        }
        self.injector.move_pointer_absolute(point.0, point.1)
    }
    pub fn click_pointer_at(
        &mut self,
        point: (i32, i32),
        button: MouseButton,
        count: u8,
        modifiers: &[&str],
    ) -> Result<(), String> {
        if self.has_active_drag() || self.has_active_switch_session() {
            return Err("End switch forwarding or dragging before clicking.".into());
        }
        self.held_button = Some(button);
        self.injector
            .click_pointer_at(point, button, count, modifiers)?;
        self.held_button = None;
        self.pointer_feedback = Some(PointerFeedback::Click { button, count });
        Ok(())
    }
    pub fn click_pointer(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        self.release_held_button()?;
        self.injector.click_pointer(button, click_count)
    }

    pub fn pointer_feedback_for_move(&self) -> PointerFeedback {
        if self.held_button.is_some() {
            PointerFeedback::Drag
        } else {
            PointerFeedback::Move
        }
    }

    pub fn execute(
        &mut self,
        device_id: &str,
        command_type: &str,
        payload: &Value,
        profiles: &[SwitchProfile],
    ) -> Result<DesktopCommandOutcome, String> {
        self.pointer_feedback = None;
        let mut typing_injected = false;
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
            return Err("Stop Switch Forwarding before using other PC control commands.".into());
        }
        let result = match command_type {
            "mouse.scroll" => {
                let dx = number(payload, "dx", 50)?;
                let dy = number(payload, "dy", 50)?;
                self.injector.scroll(dx, dy)?;
                self.pointer_feedback = Some(PointerFeedback::Scroll { dx, dy });
                Ok(())
            }
            "mouse.dragStart" => {
                let button = payload_button(payload)?;
                self.release_held_button()?;
                self.injector.set_pointer_button(button, true)?;
                self.held_button = Some(button);
                self.pointer_feedback = Some(PointerFeedback::Drag);
                Ok(())
            }
            "mouse.dragEnd" => {
                self.release_held_button()?;
                self.pointer_feedback = Some(PointerFeedback::Move);
                Ok(())
            }
            "mouse.click" | "mouse.doubleClick" | "mouse.rightClick" => {
                let button = if command_type == "mouse.rightClick" {
                    MouseButton::Right
                } else {
                    payload_button(payload)?
                };
                let count = if command_type == "mouse.doubleClick" {
                    2
                } else {
                    1
                };
                self.click_pointer(button, count)?;
                self.pointer_feedback = Some(PointerFeedback::Click { button, count });
                Ok(())
            }
            "keyboard.key" => {
                let key = string(payload, "key")?;
                self.injector.set_key(key, true)?;
                self.injector.set_key(key, false)?;
                typing_injected = true;
                Ok(())
            }
            "keyboard.modifierDown" => {
                let key = ModifierKey::parse(string(payload, "key")?)?;
                if !self.held_modifiers.contains(&key) {
                    self.injector.set_key(key.protocol_name(), true)?;
                    self.pending_modifier_releases.remove(&key);
                    self.held_modifiers.insert(key);
                    self.update_modifier_overlay();
                }
                Ok(())
            }
            "keyboard.modifierUp" => {
                let key = ModifierKey::parse(string(payload, "key")?)?;
                let was_held = self.held_modifiers.remove(&key);
                let was_pending = self.pending_modifier_releases.remove(&key);
                if was_held || was_pending {
                    if was_held {
                        self.update_modifier_overlay();
                    }
                    let result = self.injector.set_key(key.protocol_name(), false);
                    if result.is_err() {
                        self.pending_modifier_releases.insert(key);
                    }
                    result?;
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
                let keys_to_press = keys
                    .into_iter()
                    .filter(|key| {
                        ModifierKey::parse(key)
                            .map(|modifier| !self.held_modifiers.contains(&modifier))
                            .unwrap_or(true)
                    })
                    .collect::<Vec<_>>();
                let shift_held = self.held_modifiers.contains(&ModifierKey::Shift);
                self.injector.press_shortcut(&keys_to_press, shift_held)?;
                typing_injected = true;
                Ok(())
            }
            "keyboard.typeText" => {
                let text = string(payload, "text")?;
                if text.chars().count() > 2000 {
                    return Err("Text payload is too large.".into());
                }
                self.injector.inject_text(text)?;
                typing_injected = true;
                Ok(())
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
                    return Ok(DesktopCommandOutcome::default());
                }
                let text = string(payload, "text")?;
                if text.chars().count() > 2_000 {
                    return Err("Text payload is too large.".into());
                }
                let result = self.injector.inject_text(text);
                if result.is_err() {
                    self.mark_text_stream_failed(&key);
                }
                result?;
                typing_injected = true;
                Ok(())
            }
            "keyboard.textStream.key" => {
                let stream_key = text_stream_key(device_id, string(payload, "streamId")?);
                if !self.begin_text_stream_item(&stream_key, integer(payload, "seq")?)? {
                    return Ok(DesktopCommandOutcome::default());
                }
                let key = string(payload, "key")?;
                let result = self
                    .injector
                    .set_key(key, true)
                    .and_then(|_| self.injector.set_key(key, false));
                if result.is_err() {
                    self.mark_text_stream_failed(&stream_key);
                }
                result?;
                typing_injected = true;
                Ok(())
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
                    .ok_or_else(|| "Pointer speed is invalid.".to_string())?;
                self.pointer_scale_percent = u32::from(normalize_pointer_scale_percent(scale)?);
                Ok(())
            }
            "connection.disconnecting" => {
                let result = self.release_all();
                self.end_control_session();
                result
            }
            "mouse.repeat.start" | "mouse.repeat.stop" => {
                Err("Mouse repeat is managed by the platform runtime.".into())
            }
            "grid.switch.set" => self.apply_legacy_grid_edge(device_id, payload, profiles),
            "grid.switch.sync" => self.sync_legacy_grid(device_id, payload, profiles),
            "pointer.display.move" => {
                Err("Display navigation is not available in this build.".into())
            }
            _ => Err(format!("Unsupported desktop command: {command_type}")),
        };
        result?;
        Ok(DesktopCommandOutcome {
            pointer_feedback: self.pointer_feedback.take(),
            typing_injected,
        })
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
                Err("The Switch Forwarding session is not active.".into())
            }
            None => Err("The Switch Forwarding session is not active.".into()),
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
                let shift_held = self.held_modifiers.contains(&ModifierKey::Shift);
                self.injector.press_shortcut(&keys, shift_held)
            }
            "mouseClick" => self
                .injector
                .click_pointer(
                    parse_button(binding.value.as_deref().unwrap_or("left"))?,
                    binding.click_count.unwrap_or(1).clamp(1, 2),
                )
                .map(|()| {
                    self.pointer_feedback = Some(PointerFeedback::Click {
                        button: parse_button(binding.value.as_deref().unwrap_or("left"))
                            .unwrap_or(MouseButton::Left),
                        count: binding.click_count.unwrap_or(1).clamp(1, 2),
                    });
                }),
            "scroll" => {
                let (dx, dy) = match binding.value.as_deref() {
                    Some("up") => (0, 1),
                    Some("down") => (0, -1),
                    Some("left") => (-1, 0),
                    Some("right") => (1, 0),
                    _ => return Err("Scroll direction is invalid.".into()),
                };
                self.injector.scroll(dx, dy).map(|()| {
                    self.pointer_feedback = Some(PointerFeedback::Scroll { dx, dy });
                })
            }
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
            if binding.binding_type == "mouseButton" {
                self.pointer_feedback = Some(PointerFeedback::Drag);
            }
        }
        if count == 1 && next == 0 {
            self.set_output(&output, false)?;
            if binding.binding_type == "mouseButton" {
                self.pointer_feedback = Some(PointerFeedback::Move);
            }
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

    /// Injects one discrete key tap for a repeat tick.
    ///
    /// A repeat never holds a key down between ticks, so a loop that dies for
    /// any reason cannot leave a key stuck. The one remaining gap is a press
    /// that succeeds followed by a release that fails; that key is recorded and
    /// retried by `release_all`.
    pub fn execute_repeat_key(&mut self, key: RepeatKey) -> Result<(), String> {
        let name = key.protocol_name();
        self.injector.set_key(name, true)?;
        match self.injector.set_key(name, false) {
            Ok(()) => {
                self.pending_key_releases.remove(&key);
                Ok(())
            }
            Err(error) => {
                self.pending_key_releases.insert(key);
                Err(error)
            }
        }
    }

    pub fn scan_chord(
        &mut self,
        keys: &[&str],
        click: Option<((i32, i32), MouseButton, u8)>,
    ) -> Result<(), String> {
        self.release_scan_keys()?;
        let result = (|| {
            for key in self.injector.chord_keys(keys)? {
                self.scan_keys.push(key.clone());
                self.injector.set_chord_key(&key, true)?;
            }
            if let Some((point, button, count)) = click {
                self.click_pointer_at(point, button, count, keys)?;
            }
            Ok(())
        })();
        let released = self.release_scan_keys();
        result.and(released)
    }
    fn release_scan_keys(&mut self) -> Result<(), String> {
        let mut error = None;
        for index in (0..self.scan_keys.len()).rev() {
            match self.injector.set_chord_key(&self.scan_keys[index], false) {
                Ok(()) => {
                    self.scan_keys.remove(index);
                }
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
    pub fn release_all(&mut self) -> Result<(), String> {
        let mut first_error = self.stop_switch_session().err();
        self.text_streams.clear();
        if let Err(error) = self.release_scan_keys() {
            first_error.get_or_insert(error);
        }
        if let Err(error) = self.release_held_button() {
            first_error.get_or_insert(error);
        }
        if let Err(error) = self.release_held_modifiers() {
            first_error.get_or_insert(error);
        }
        if let Err(error) = self.release_pending_keys() {
            first_error.get_or_insert(error);
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }

    /// Releases any key left down by a failed repeat-tick release.
    ///
    /// Called when a repeat ends rather than only on the terminal cleanup
    /// paths: a key stuck down keeps the OS's own auto-repeat firing into the
    /// focused application, so waiting for a disconnect is far too late.
    pub fn release_repeat_keys(&mut self) -> Result<(), String> {
        if self.pending_key_releases.is_empty() {
            return Ok(());
        }
        self.release_pending_keys()
    }

    /// Retries key releases that failed during a repeat tick. Every key is
    /// attempted even when one fails, so a single bad release cannot strand the
    /// others.
    fn release_pending_keys(&mut self) -> Result<(), String> {
        let mut first_error = None;
        for key in RepeatKey::ALL {
            if !self.pending_key_releases.remove(&key) {
                continue;
            }
            if let Err(error) = self.injector.set_key(key.protocol_name(), false) {
                self.pending_key_releases.insert(key);
                first_error.get_or_insert(error);
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn release_held_modifiers(&mut self) -> Result<(), String> {
        for key in ModifierKey::RELEASE_ORDER {
            let was_held = self.held_modifiers.remove(&key);
            let was_pending = self.pending_modifier_releases.remove(&key);
            if !was_held && !was_pending {
                continue;
            }
            if was_held {
                self.update_modifier_overlay();
            }
            let result = self.injector.set_key(key.protocol_name(), false);
            if result.is_err() {
                self.pending_modifier_releases.insert(key);
            }
            result?;
        }
        Ok(())
    }

    pub fn end_control_session(&mut self) {
        self.text_streams.clear();
        self.switch_session = None;
        self.held_button = None;
        self.pending_modifier_releases
            .extend(self.held_modifiers.drain());
        if let Some(overlay) = &self.modifier_overlay {
            overlay.end_control_session();
        }
    }

    fn update_modifier_overlay(&self) {
        let Some(overlay) = &self.modifier_overlay else {
            return;
        };
        let active_modifiers = ModifierKey::DISPLAY_ORDER
            .into_iter()
            .filter(|key| self.held_modifiers.contains(key))
            .collect::<Vec<_>>();
        overlay.set_active_modifiers(&active_modifiers);
    }

    fn release_held_button(&mut self) -> Result<(), String> {
        if let Some(button) = self.held_button {
            self.injector.set_pointer_button(button, false)?;
            self.held_button = None;
        }
        Ok(())
    }
}

pub fn persist_pointer_scale_change<I: InputInjector>(
    input: &mut DesktopInput<I>,
    model: &AppModel,
    scale_percent: f64,
) -> Result<(), String> {
    let scale_percent = normalize_pointer_scale_percent(scale_percent)?;
    let previous_scale = model.snapshot().settings.pointer_scale_percent;
    input.set_pointer_scale_percent(scale_percent);
    if model.apply_pointer_scale_percent(scale_percent).is_err() {
        input.set_pointer_scale_percent(previous_scale);
        return Err("Pointer speed could not be saved.".into());
    }
    Ok(())
}

pub fn execute_desktop_command<I: InputInjector>(
    input: &mut DesktopInput<I>,
    model: &AppModel,
    device_id: &str,
    command_type: &str,
    payload: &Value,
    profiles: &[SwitchProfile],
) -> Result<DesktopCommandOutcome, String> {
    let result = input.execute(device_id, command_type, payload, profiles)?;
    if command_type == "pointer.speed.set" {
        let scale_percent = payload["scalePercent"].as_f64().unwrap_or_default();
        persist_pointer_scale_change(input, model, scale_percent)?;
    }
    Ok(result)
}

pub fn execute_dwell_click<I: InputInjector>(
    input: &mut DesktopInput<I>,
) -> Result<PointerFeedback, String> {
    input.click_pointer(MouseButton::Left, 1)?;
    Ok(PointerFeedback::Click {
        button: MouseButton::Left,
        count: 1,
    })
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
mod tests;
