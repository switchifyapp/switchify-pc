use super::*;
use crate::protocol::{DesktopCommand, ProtocolEngine, ResponseMode};
use crate::storage::AppStorage;
use std::fs;
use std::sync::Mutex;

#[test]
fn scan_actions_have_exact_fake_input_sequences() {
    use crate::{point_workflow::Request, scan_executor::execute};
    for (right, count, button) in [
        (false, 1, MouseButton::Left),
        (true, 1, MouseButton::Right),
        (false, 2, MouseButton::Left),
    ] {
        let mut input = DesktopInput::new(FakeInjector::default());
        execute(
            &mut input,
            Request::Click {
                point: (10, 20),
                right,
                count,
            },
            true,
        )
        .unwrap();
        assert_eq!(input.injector.events, vec!["move", "click"]);
        assert_eq!(input.injector.clicks, vec![(button, count)]);
    }
    for (dx, dy) in [(0, -3), (0, 3), (-3, 0), (3, 0)] {
        let mut input = DesktopInput::new(FakeInjector::default());
        execute(
            &mut input,
            Request::Scroll {
                point: (10, 20),
                dx,
                dy,
            },
            true,
        )
        .unwrap();
        assert_eq!(input.injector.events, vec!["move", "scroll"]);
        assert_eq!(input.injector.scrolls, vec![(dx, dy)]);
    }
    let mut input = DesktopInput::new(FakeInjector::default());
    execute(&mut input, Request::DragStart((10, 20)), true).unwrap();
    execute(&mut input, Request::DragMove((20, 30)), true).unwrap();
    execute(&mut input, Request::DragEnd((30, 40)), true).unwrap();
    assert_eq!(
        input.injector.events,
        vec!["move", "down", "move", "move", "up"]
    );
    assert!(!input.has_active_drag());
}
#[test]
fn remote_stop_releases_drag_before_direct_input_and_blocks_failed_release() {
    use crate::{point_workflow::Request, remote_scan::input_available, scan_executor::execute};
    for fail in [false, true] {
        let mut input = DesktopInput::new(FakeInjector::default());
        execute(&mut input, Request::DragStart((10, 20)), true).unwrap();
        input.injector.fail_pointer_release = fail;
        let mut cleanup_required = input.release_all().is_err();
        if input_available(false, cleanup_required) {
            input.click_pointer(MouseButton::Left, 1).unwrap();
        }
        if fail {
            assert!(input.injector.clicks.is_empty());
            assert!(input.has_active_drag());
            input.injector.fail_pointer_release = false;
            cleanup_required = input.release_all().is_err();
            assert!(input_available(false, cleanup_required));
            input.click_pointer(MouseButton::Left, 1).unwrap();
            assert_eq!(
                input.injector.events,
                vec!["move", "down", "up", "up", "click"]
            );
        } else {
            assert_eq!(input.injector.events, vec!["move", "down", "up", "click"]);
        }
        assert!(!input.has_active_drag());
    }
}
#[test]
fn scan_drag_failures_remain_owned_until_cleanup_succeeds() {
    use crate::{point_workflow::Request, scan_executor::execute};
    let mut input = DesktopInput::new(FakeInjector::default());
    assert!(execute(&mut input, Request::DragStart((1, 2)), false).is_err());
    assert!(input.injector.events.is_empty());
    execute(&mut input, Request::DragStart((1, 2)), true).unwrap();
    input.injector.fail_absolute = true;
    input.injector.fail_pointer_release = true;
    assert!(execute(&mut input, Request::DragEnd((3, 4)), true).is_err());
    assert!(input.has_active_drag());
    input.injector.fail_pointer_release = false;
    input.release_all().unwrap();
    assert!(!input.has_active_drag());
    assert_eq!(
        input.injector.events,
        vec!["move", "down", "move", "up", "up"]
    );
}
#[derive(Default)]
struct FakeInjector {
    text: Vec<String>,
    moves: Vec<(i32, i32)>,
    absolute_moves: Vec<(i32, i32)>,
    scrolls: Vec<(i32, i32)>,
    clicks: Vec<(MouseButton, u8)>,
    fail_click: bool,
    fail_absolute: bool,
    events: Vec<&'static str>,
    pointer_states: Vec<(MouseButton, bool)>,
    fail_pointer_release: bool,
    keys: Vec<(String, bool)>,
    fail_key_down: bool,
    fail_key_up: Option<String>,
    shortcuts: Vec<Vec<String>>,
    shortcut_shift_held: Vec<bool>,
    #[cfg(target_os = "windows")]
    layout: Option<fn(u16) -> i16>,
}
impl InputInjector for FakeInjector {
    #[cfg(target_os = "windows")]
    fn chord_keys(&mut self, keys: &[&str]) -> Result<Vec<ChordKey>, String> {
        match self.layout {
            Some(layout) => windows_chord_keys(keys, layout),
            None => Ok(keys
                .iter()
                .map(|key| ChordKey::Named((*key).into()))
                .collect()),
        }
    }
    #[cfg(target_os = "windows")]
    fn set_chord_key(&mut self, key: &ChordKey, down: bool) -> Result<(), String> {
        match key {
            ChordKey::Named(name) => self.set_key(name, down),
            ChordKey::Virtual(code) => self.set_key(&format!("vk {code:x}"), down),
        }
    }
    fn inject_text(&mut self, text: &str) -> Result<(), String> {
        self.text.push(text.into());
        Ok(())
    }
    fn move_pointer(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.moves.push((dx, dy));
        Ok(())
    }
    fn move_pointer_absolute(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.events.push("move");
        if self.fail_absolute {
            return Err("move failed".into());
        }
        self.absolute_moves.push((x, y));
        Ok(())
    }
    fn click_pointer(&mut self, button: MouseButton, count: u8) -> Result<(), String> {
        self.events.push("click");
        if self.fail_click {
            return Err("click injection failed".into());
        }
        self.clicks.push((button, count));
        Ok(())
    }
    fn set_pointer_button(&mut self, button: MouseButton, down: bool) -> Result<(), String> {
        self.events.push(if down { "down" } else { "up" });
        if !down && self.fail_pointer_release {
            return Err("pointer release failed".into());
        }
        self.pointer_states.push((button, down));
        Ok(())
    }
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.events.push("scroll");
        self.scrolls.push((dx, dy));
        Ok(())
    }
    fn set_key(&mut self, key: &str, down: bool) -> Result<(), String> {
        self.keys.push((key.into(), down));
        if (down && self.fail_key_down) || (!down && self.fail_key_up.as_deref() == Some(key)) {
            return Err("key injection failed".into());
        }
        Ok(())
    }
    fn press_shortcut(&mut self, keys: &[String], shift_held: bool) -> Result<(), String> {
        self.shortcuts.push(keys.to_vec());
        self.shortcut_shift_held.push(shift_held);
        Ok(())
    }
    fn media(&mut self, _action: &str) -> Result<(), String> {
        Ok(())
    }
    fn window(&mut self, _action: &str) -> Result<(), String> {
        Ok(())
    }
    fn power(&mut self, _action: crate::power::Action) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[derive(Default)]
struct FakeWindowsCursor {
    current: (i32, i32),
    positions: Vec<(i32, i32)>,
    fail_read: bool,
    fail_write: bool,
}

#[cfg(target_os = "windows")]
impl WindowsCursorPosition for FakeWindowsCursor {
    fn position(&mut self) -> Result<(i32, i32), String> {
        if self.fail_read {
            Err("private cursor read failure".into())
        } else {
            Ok(self.current)
        }
    }

    fn set_position(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.positions.push((x, y));
        if self.fail_write {
            Err("private cursor write failure".into())
        } else {
            self.current = (x, y);
            Ok(())
        }
    }
}

#[derive(Default)]
struct FakeModifierOverlay {
    changes: Mutex<Vec<Vec<ModifierKey>>>,
    ended: Mutex<u32>,
}

impl ModifierKeyOverlayNotifier for FakeModifierOverlay {
    fn set_active_modifiers(&self, active_modifiers: &[ModifierKey]) {
        self.changes.lock().unwrap().push(active_modifiers.to_vec());
    }

    fn end_control_session(&self) {
        *self.ended.lock().unwrap() += 1;
    }
}

fn input_with_modifier_overlay() -> (DesktopInput<FakeInjector>, Arc<FakeModifierOverlay>) {
    let overlay = Arc::new(FakeModifierOverlay::default());
    let input = DesktopInput::with_modifier_overlay(FakeInjector::default(), overlay.clone());
    (input, overlay)
}
#[test]
fn command_execution_uses_injected_text_without_system_input() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.type_text("Hello").unwrap();
    assert_eq!(input.injector.text, vec!["Hello"]);
}

#[test]
fn point_scan_clicks_once_at_absolute_point_with_fake_input() {
    let mut input = DesktopInput::new(FakeInjector::default());
    crate::point_scan_activation::click(&mut input, (-640, 512), true).unwrap();
    assert_eq!(input.injector.absolute_moves, vec![(-640, 512)]);
    assert_eq!(input.injector.clicks, vec![(MouseButton::Left, 1)]);
    assert!(input.injector.keys.is_empty());
}

#[test]
fn point_scan_refuses_to_move_during_drag() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.held_button = Some(MouseButton::Left);
    assert!(crate::point_scan_activation::click(&mut input, (30, 40), true).is_err());
    assert!(input.injector.absolute_moves.is_empty());
    assert!(input.injector.clicks.is_empty());
}

#[test]
fn point_scan_does_not_inject_a_modified_click() {
    let mut input = DesktopInput::new(FakeInjector::default());
    assert!(crate::point_scan_activation::click(&mut input, (30, 40), false).is_err());
    assert!(input.injector.absolute_moves.is_empty());
    assert!(input.injector.clicks.is_empty());
}

#[cfg(target_os = "windows")]
#[test]
fn canonical_windows_alphanumeric_keys_use_physical_keys() {
    let expected = [
        ("A", Key::A),
        ("B", Key::B),
        ("C", Key::C),
        ("D", Key::D),
        ("E", Key::E),
        ("F", Key::F),
        ("G", Key::G),
        ("H", Key::H),
        ("I", Key::I),
        ("J", Key::J),
        ("K", Key::K),
        ("L", Key::L),
        ("M", Key::M),
        ("N", Key::N),
        ("O", Key::O),
        ("P", Key::P),
        ("Q", Key::Q),
        ("R", Key::R),
        ("S", Key::S),
        ("T", Key::T),
        ("U", Key::U),
        ("V", Key::V),
        ("W", Key::W),
        ("X", Key::X),
        ("Y", Key::Y),
        ("Z", Key::Z),
        ("0", Key::Num0),
        ("1", Key::Num1),
        ("2", Key::Num2),
        ("3", Key::Num3),
        ("4", Key::Num4),
        ("5", Key::Num5),
        ("6", Key::Num6),
        ("7", Key::Num7),
        ("8", Key::Num8),
        ("9", Key::Num9),
    ];

    for (protocol_name, physical_key) in expected {
        assert_eq!(named_key(protocol_name), Some(physical_key));
    }
}

#[cfg(target_os = "macos")]
#[test]
fn canonical_macos_alphanumeric_keys_remain_layout_dependent() {
    assert_eq!(named_key("A"), Some(Key::Unicode('A')));
    assert_eq!(named_key("0"), Some(Key::Unicode('0')));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_symbol_shortcuts_follow_the_layout_and_refuse_altgr() {
    // VK_OEM_2 (/) alone, as "/" on UK and US.
    assert_eq!(super::windows_symbol_key(0x00bf), Ok((false, 0xbf)));
    // Shift and VK_OEM_2, as "?" on UK and US.
    assert_eq!(super::windows_symbol_key(0x01bf), Ok((true, 0xbf)));
    // Shift and VK_OEM_3, as "@" on UK.
    assert_eq!(super::windows_symbol_key(0x01c0), Ok((true, 0xc0)));
    // Ctrl and Alt (AltGr) and 4, as the euro sign on UK.
    assert!(super::windows_symbol_key(0x0634).is_err());
    // No key types the symbol.
    assert!(super::windows_symbol_key(-1).is_err());
}

#[cfg(target_os = "windows")]
fn uk_layout(unit: u16) -> i16 {
    match char::from_u32(unit.into()) {
        Some('/') => 0x00bf,
        Some('?') => 0x01bf,
        Some('@') => 0x01c0,
        Some('€') => 0x0634,
        _ => -1,
    }
}

#[cfg(target_os = "windows")]
fn chord(keys: &[&str]) -> Result<Vec<ChordKey>, String> {
    super::windows_chord_keys(keys, uk_layout)
}

#[cfg(target_os = "windows")]
fn named(key: &str) -> ChordKey {
    ChordKey::Named(key.into())
}

#[cfg(target_os = "windows")]
#[test]
fn windows_shortcut_symbols_resolve_once_and_add_shift_only_when_missing() {
    assert_eq!(
        chord(&["Ctrl", "?"]),
        Ok(vec![named("Ctrl"), named("Shift"), ChordKey::Virtual(0xbf)])
    );
    // Shift already in the shortcut is not pressed twice.
    assert_eq!(
        chord(&["Shift", "Ctrl", "@"]),
        Ok(vec![named("Shift"), named("Ctrl"), ChordKey::Virtual(0xc0)])
    );
    assert_eq!(
        chord(&["Ctrl", "/"]),
        Ok(vec![named("Ctrl"), ChordKey::Virtual(0xbf)])
    );
    // Named keys and the fixed zoom keys are not looked up.
    assert_eq!(
        chord(&["Ctrl", "=", "Tab"]),
        Ok(vec![named("Ctrl"), named("="), named("Tab")])
    );
    // A symbol left alone because the remote holds the modifiers.
    assert_eq!(
        chord(&["?"]),
        Ok(vec![named("Shift"), ChordKey::Virtual(0xbf)])
    );
    assert!(chord(&["ñ"]).is_err());
    assert!(chord(&["Ctrl", "€"]).is_err());
    assert!(chord(&["Ctrl", "ñ"]).is_err());
}

#[cfg(target_os = "windows")]
#[test]
fn scan_chord_releases_the_symbol_key_it_pressed_after_the_layout_changes() {
    let mut input = DesktopInput::new(FakeInjector {
        layout: Some(uk_layout),
        fail_key_up: Some("vk bf".into()),
        ..FakeInjector::default()
    });
    assert!(input.scan_chord(&["Ctrl", "?"], None).is_err());
    // The new layout has no key for "?", but the retry still releases
    // the key that was pressed rather than looking it up again.
    input.injector.layout = Some(|_| -1);
    input.injector.fail_key_up = None;
    input.injector.keys.clear();
    input.release_all().unwrap();
    assert_eq!(input.injector.keys, [("vk bf".to_string(), false)]);
}

#[cfg(target_os = "windows")]
#[test]
fn shortcut_symbol_leaves_a_remote_held_shift_down() {
    let chord = chord(&["Ctrl", "?"]).unwrap();
    assert_eq!(
        super::without_held_shift(chord.clone(), true),
        [named("Ctrl"), ChordKey::Virtual(0xbf)]
    );
    assert_eq!(super::without_held_shift(chord.clone(), false), chord);
}

#[test]
fn protocol_shortcut_reports_a_remote_held_shift() {
    let mut input = DesktopInput::new(FakeInjector::default());
    let shortcut = serde_json::json!({"keys": ["Ctrl", "?"]});
    input
        .execute("device", "keyboard.shortcut", &shortcut, &[])
        .unwrap();
    input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Shift"}),
            &[],
        )
        .unwrap();
    input
        .execute("device", "keyboard.shortcut", &shortcut, &[])
        .unwrap();
    assert_eq!(input.injector.shortcut_shift_held, [false, true]);
}

#[test]
fn shortcut_press_failure_releases_every_key_pressed_first() {
    let mut events = Vec::new();
    let result = run_shortcut_sequence(&["Ctrl", "A"], |key, direction| {
        events.push((key, direction));
        if key == "A" && direction == Direction::Press {
            Err("A press failed".into())
        } else {
            Ok(())
        }
    });

    assert_eq!(result, Err("A press failed".into()));
    assert_eq!(
        events,
        vec![
            ("Ctrl", Direction::Press),
            ("A", Direction::Press),
            ("Ctrl", Direction::Release),
        ]
    );
}

#[test]
fn shortcut_release_failure_continues_cleanup_and_preserves_the_first_error() {
    let mut events = Vec::new();
    let result = run_shortcut_sequence(&["Ctrl", "A"], |key, direction| {
        events.push((key, direction));
        if direction == Direction::Release {
            Err(format!("{key} release failed"))
        } else {
            Ok(())
        }
    });

    assert_eq!(result, Err("A release failed".into()));
    assert_eq!(
        events,
        vec![
            ("Ctrl", Direction::Press),
            ("A", Direction::Press),
            ("A", Direction::Release),
            ("Ctrl", Direction::Release),
        ]
    );
}

#[test]
fn shortcuts_do_not_repress_modifiers_held_by_the_remote() {
    let mut input = DesktopInput::new(FakeInjector::default());
    for modifier in ["Ctrl", "Shift"] {
        input
            .execute(
                "device",
                "keyboard.modifierDown",
                &serde_json::json!({"key": modifier}),
                &[],
            )
            .unwrap();
    }

    input
        .execute(
            "device",
            "keyboard.shortcut",
            &serde_json::json!({"keys": ["Ctrl", "Shift", "A"]}),
            &[],
        )
        .unwrap();

    assert_eq!(input.injector.shortcuts, vec![vec!["A"]]);
    assert_eq!(
        input.held_modifiers,
        HashSet::from([ModifierKey::Ctrl, ModifierKey::Shift])
    );
    assert_eq!(
        input.injector.keys,
        vec![("Ctrl".into(), true), ("Shift".into(), true)]
    );
}

#[test]
fn standalone_shortcuts_still_press_the_complete_key_list() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input
        .execute(
            "device",
            "keyboard.shortcut",
            &serde_json::json!({"keys": ["Meta", "Alt", "S"]}),
            &[],
        )
        .unwrap();

    assert_eq!(input.injector.shortcuts, vec![vec!["Meta", "Alt", "S"]]);
}

#[test]
fn mobile_typing_commands_report_only_actual_keyboard_injection() {
    let mut input = DesktopInput::new(FakeInjector::default());

    for (command_type, payload) in [
        ("keyboard.key", serde_json::json!({"key": "Enter"})),
        (
            "keyboard.shortcut",
            serde_json::json!({"keys": ["Ctrl", "A"]}),
        ),
        ("keyboard.typeText", serde_json::json!({"text": "Hello"})),
    ] {
        let outcome = input
            .execute("device", command_type, &payload, &[])
            .unwrap();
        assert!(outcome.typing_injected, "{command_type}");
        assert_eq!(outcome.pointer_feedback, None);
    }

    input
        .execute(
            "device",
            "keyboard.textStream.open",
            &serde_json::json!({"streamId": "stream"}),
            &[],
        )
        .unwrap();
    for (sequence, command_type, payload) in [
        (
            0,
            "keyboard.textStream.char",
            serde_json::json!({"streamId": "stream", "seq": 0, "text": "a"}),
        ),
        (
            1,
            "keyboard.textStream.chunk",
            serde_json::json!({"streamId": "stream", "seq": 1, "text": "bc"}),
        ),
        (
            2,
            "keyboard.textStream.key",
            serde_json::json!({"streamId": "stream", "seq": 2, "key": "Enter"}),
        ),
    ] {
        let outcome = input
            .execute("device", command_type, &payload, &[])
            .unwrap();
        assert!(outcome.typing_injected, "sequence {sequence}");
    }

    let duplicate = input
        .execute(
            "device",
            "keyboard.textStream.key",
            &serde_json::json!({"streamId": "stream", "seq": 2, "key": "Enter"}),
            &[],
        )
        .unwrap();
    assert_eq!(duplicate, DesktopCommandOutcome::default());

    for (command_type, payload) in [
        ("keyboard.modifierDown", serde_json::json!({"key": "Ctrl"})),
        ("keyboard.modifierUp", serde_json::json!({"key": "Ctrl"})),
        (
            "keyboard.textStream.close",
            serde_json::json!({"streamId": "stream", "expectedCount": 3}),
        ),
        ("media.control", serde_json::json!({"action": "playPause"})),
        ("window.control", serde_json::json!({"action": "minimize"})),
    ] {
        let outcome = input
            .execute("device", command_type, &payload, &[])
            .unwrap();
        assert!(!outcome.typing_injected, "{command_type}");
    }
}

#[test]
fn failed_mobile_typing_does_not_report_injection() {
    let mut input = DesktopInput::new(FakeInjector {
        fail_key_down: true,
        ..FakeInjector::default()
    });
    assert!(input
        .execute(
            "device",
            "keyboard.key",
            &serde_json::json!({"key": "Enter"}),
            &[],
        )
        .is_err());
}

#[test]
fn typing_command_classification_excludes_lifecycle_and_modifier_commands() {
    for command_type in [
        "keyboard.key",
        "keyboard.shortcut",
        "keyboard.typeText",
        "keyboard.textStream.char",
        "keyboard.textStream.chunk",
        "keyboard.textStream.key",
    ] {
        assert!(
            MobileTypingRoute::for_command(command_type).is_eligible(),
            "{command_type}"
        );
    }
    for command_type in [
        "keyboard.modifierDown",
        "keyboard.modifierUp",
        "keyboard.textStream.open",
        "keyboard.textStream.close",
        "media.control",
        "window.control",
    ] {
        assert!(
            !MobileTypingRoute::for_command(command_type).is_eligible(),
            "{command_type}"
        );
    }
    assert!(MobileTypingRoute::for_text("a").is_eligible());
    assert!(!MobileTypingRoute::for_text("").is_eligible());
}

#[test]
fn typing_route_orders_prepare_and_successful_hide_effects() {
    let events = Mutex::new(Vec::new());
    let route = MobileTypingRoute::for_text("Hello");
    route.prepare(
        || events.lock().unwrap().push("cancel dwell"),
        || events.lock().unwrap().push("stop repeats"),
    );
    route.finish(true, || events.lock().unwrap().push("hide overlay"));
    assert_eq!(
        *events.lock().unwrap(),
        ["cancel dwell", "stop repeats", "hide overlay"]
    );

    let events = Mutex::new(Vec::new());
    MobileTypingRoute::for_text("").prepare(
        || events.lock().unwrap().push("cancel dwell"),
        || events.lock().unwrap().push("stop repeats"),
    );
    MobileTypingRoute::for_text("").finish(true, || events.lock().unwrap().push("hide overlay"));
    MobileTypingRoute::for_text("Hello")
        .finish(false, || events.lock().unwrap().push("hide overlay"));
    assert!(events.lock().unwrap().is_empty());
}

#[cfg(target_os = "windows")]
#[test]
fn windows_relative_pointer_movement_preserves_axis_on_every_monitor_origin() {
    for (current, delta, expected) in [
        ((960, 540), (0, -64), (960, 476)),
        ((3_200, 900), (0, 64), (3_200, 964)),
        ((-1_280, 720), (0, -64), (-1_280, 656)),
        ((2_560, -300), (-64, 0), (2_496, -300)),
        ((-640, 512), (64, -64), (-576, 448)),
    ] {
        let mut cursor = FakeWindowsCursor {
            current,
            ..FakeWindowsCursor::default()
        };

        move_windows_pointer_relative(&mut cursor, delta.0, delta.1).unwrap();

        assert_eq!(cursor.positions, vec![expected]);
    }
}

#[cfg(target_os = "windows")]
#[test]
fn windows_relative_pointer_movement_saturates_coordinate_overflow() {
    let mut cursor = FakeWindowsCursor {
        current: (i32::MAX - 2, i32::MIN + 2),
        ..FakeWindowsCursor::default()
    };

    move_windows_pointer_relative(&mut cursor, 64, -64).unwrap();

    assert_eq!(cursor.positions, vec![(i32::MAX, i32::MIN)]);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_relative_pointer_movement_sanitizes_native_failures() {
    let mut read_failure = FakeWindowsCursor {
        fail_read: true,
        ..FakeWindowsCursor::default()
    };
    assert_eq!(
        move_windows_pointer_relative(&mut read_failure, 0, 64),
        Err(WINDOWS_POINTER_MOVE_ERROR.into())
    );
    assert!(read_failure.positions.is_empty());

    let mut write_failure = FakeWindowsCursor {
        current: (-1_280, 720),
        fail_write: true,
        ..FakeWindowsCursor::default()
    };
    assert_eq!(
        move_windows_pointer_relative(&mut write_failure, 0, -64),
        Err(WINDOWS_POINTER_MOVE_ERROR.into())
    );
    assert_eq!(write_failure.positions, vec![(-1_280, 656)]);
}

#[test]
fn dwell_click_uses_the_runtime_input_adapter_once() {
    let mut input = DesktopInput::new(FakeInjector::default());
    assert_eq!(
        execute_dwell_click(&mut input).unwrap(),
        PointerFeedback::Click {
            button: MouseButton::Left,
            count: 1,
        }
    );
    assert_eq!(input.injector.clicks, vec![(MouseButton::Left, 1)]);

    input.injector.fail_click = true;
    assert!(execute_dwell_click(&mut input).is_err());
    assert_eq!(input.injector.clicks, vec![(MouseButton::Left, 1)]);
}

#[test]
fn enigo_scroll_preserves_horizontal_direction() {
    assert_eq!(enigo_scroll_delta(5, 0), (5, 0));
    assert_eq!(enigo_scroll_delta(-5, 0), (-5, 0));
}

#[test]
fn enigo_scroll_reverses_vertical_direction() {
    assert_eq!(enigo_scroll_delta(0, 5), (0, -5));
    assert_eq!(enigo_scroll_delta(0, -5), (0, 5));
}

#[test]
fn direct_pointer_commands_return_overlay_feedback() {
    let mut input = DesktopInput::new(FakeInjector::default());
    let feedback = input
        .execute(
            "device",
            "mouse.scroll",
            &serde_json::json!({"dx": 2, "dy": -4}),
            &[],
        )
        .unwrap();

    assert_eq!(
        feedback,
        DesktopCommandOutcome {
            pointer_feedback: Some(PointerFeedback::Scroll { dx: 2, dy: -4 }),
            typing_injected: false,
        }
    );
}

#[test]
fn repeat_pixel_movement_bypasses_normal_pointer_scaling() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.set_pointer_scale_percent(50);
    assert_eq!(
        input.move_pointer_pixels(1, -1).unwrap(),
        PointerFeedback::Move
    );
    assert_eq!(input.injector.moves, vec![(1, -1)]);
}

#[test]
fn direct_pointer_movement_uses_the_extended_speed() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.set_pointer_scale_percent(1350);
    input.move_pointer(12, -12).unwrap();
    assert_eq!(input.injector.moves, vec![(162, -162)]);
}

#[test]
fn mobile_pointer_speed_changes_persist_and_restore() {
    let root =
        std::env::temp_dir().join(format!("switchify-pointer-speed-{}", uuid::Uuid::new_v4()));
    let state_path = root.join("state.json");
    let model = AppModel::with_storage_for_test(AppStorage::at(state_path.clone()));
    let mut input = DesktopInput::new(FakeInjector::default());
    let command = DesktopCommand {
        id: "pointer-speed-1".into(),
        device_id: "android-1".into(),
        command_type: "pointer.speed.set".into(),
        payload: serde_json::json!({"scalePercent": 123.0}),
        response_mode: ResponseMode::Ack,
    };
    let result = execute_desktop_command(
        &mut input,
        &model,
        &command.device_id,
        &command.command_type,
        &command.payload,
        &[],
    );
    let engine = ProtocolEngine::new("desktop".into());
    let response: Value = serde_json::from_str(
        &engine
            .complete_desktop_command_with_error(
                &command,
                result
                    .as_ref()
                    .map(|_| ())
                    .map_err(|error| ("input_failed", error.as_str())),
            )
            .unwrap(),
    )
    .unwrap();

    assert!(result.is_ok());
    assert_eq!(response["type"], "ack");
    assert_eq!(response["ok"], true);
    assert_eq!(input.scaled_pointer_delta(100, 0), (125, 0));
    assert_eq!(model.snapshot().settings.pointer_scale_percent, 125);
    let restored = AppModel::with_storage_for_test(AppStorage::at(state_path));
    assert_eq!(restored.snapshot().settings.pointer_scale_percent, 125);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_pointer_speed_save_restores_the_active_scale() {
    let root = std::env::temp_dir().join(format!(
        "switchify-pointer-speed-fail-{}",
        uuid::Uuid::new_v4()
    ));
    let state_path = root.join("state.json");
    let model = AppModel::with_storage_for_test(AppStorage::at(state_path.clone()));
    let mut input = DesktopInput::new(FakeInjector::default());
    fs::remove_file(&state_path).unwrap();
    fs::create_dir(&state_path).unwrap();

    let command = DesktopCommand {
        id: "pointer-speed-1".into(),
        device_id: "android-1".into(),
        command_type: "pointer.speed.set".into(),
        payload: serde_json::json!({"scalePercent": 175.0}),
        response_mode: ResponseMode::Ack,
    };
    let result = execute_desktop_command(
        &mut input,
        &model,
        &command.device_id,
        &command.command_type,
        &command.payload,
        &[],
    );
    assert_eq!(result, Err("Pointer speed could not be saved.".into()));
    let engine = ProtocolEngine::new("desktop".into());
    let response: Value = serde_json::from_str(
        &engine
            .complete_desktop_command_with_error(
                &command,
                result
                    .as_ref()
                    .map(|_| ())
                    .map_err(|error| ("input_failed", error.as_str())),
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(input.scaled_pointer_delta(100, 0), (100, 0));
    assert_eq!(model.snapshot().settings.pointer_scale_percent, 100);
    assert_eq!(response["type"], "error");
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "input_failed");
    assert_eq!(
        response["error"]["message"],
        "Pointer speed could not be saved."
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn display_movement_is_absolute_and_rejects_an_active_drag() {
    let mut input = DesktopInput::new(FakeInjector::default());
    assert_eq!(
        input.move_pointer_absolute(-640, 512).unwrap(),
        PointerFeedback::Move
    );
    assert_eq!(input.injector.absolute_moves, vec![(-640, 512)]);

    input
        .execute(
            "device",
            "mouse.dragStart",
            &serde_json::json!({"button": "left"}),
            &[],
        )
        .unwrap();
    assert_eq!(
        input.move_pointer_absolute(2_880, 540).unwrap_err(),
        "End the active drag before moving to another monitor."
    );
    assert_eq!(input.injector.absolute_moves, vec![(-640, 512)]);
}

#[test]
fn click_releases_an_active_drag_before_clicking() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input
        .execute(
            "device",
            "mouse.dragStart",
            &serde_json::json!({"button": "left"}),
            &[],
        )
        .unwrap();

    input.click_pointer(MouseButton::Right, 1).unwrap();

    assert_eq!(
        input.injector.pointer_states,
        vec![(MouseButton::Left, true), (MouseButton::Left, false)]
    );
    assert_eq!(input.injector.clicks, vec![(MouseButton::Right, 1)]);
    assert_eq!(input.pointer_feedback_for_move(), PointerFeedback::Move);
}

#[test]
fn failed_drag_release_remains_tracked_for_cleanup() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input
        .execute(
            "device",
            "mouse.dragStart",
            &serde_json::json!({"button": "left"}),
            &[],
        )
        .unwrap();
    input.injector.fail_pointer_release = true;

    assert!(input.click_pointer(MouseButton::Right, 1).is_err());
    assert_eq!(input.pointer_feedback_for_move(), PointerFeedback::Drag);

    input.injector.fail_pointer_release = false;
    input.release_all().unwrap();
    assert_eq!(
        input.injector.pointer_states,
        vec![(MouseButton::Left, true), (MouseButton::Left, false)]
    );
}

#[test]
fn mapped_mouse_switch_returns_click_feedback() {
    let profile = SwitchProfile {
        id: "mouse".into(),
        version: 1,
        name: "Mouse".into(),
        provider: "mapped".into(),
        built_in: false,
        bindings: vec![SwitchBinding {
            switch_id: 1,
            binding_type: "mouseClick".into(),
            value: Some("left".into()),
            keys: None,
            click_count: Some(2),
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
                "profileId": "mouse",
                "profileVersion": 1,
                "switchCount": 1
            }),
            std::slice::from_ref(&profile),
        )
        .unwrap();
    let feedback = input
        .execute(
            "device",
            "switch.edge",
            &serde_json::json!({
                "sessionId": session_id,
                "sequence": 1,
                "switchId": 1,
                "state": "down"
            }),
            &[profile],
        )
        .unwrap();

    assert_eq!(
        feedback,
        DesktopCommandOutcome {
            pointer_feedback: Some(PointerFeedback::Click {
                button: MouseButton::Left,
                count: 2,
            }),
            typing_injected: false,
        }
    );
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
fn explicit_modifiers_update_overlay_in_canonical_order() {
    let (mut input, overlay) = input_with_modifier_overlay();
    for key in ["Shift", "Ctrl", "Meta", "Ctrl"] {
        input
            .execute(
                "device",
                "keyboard.modifierDown",
                &serde_json::json!({"key": key}),
                &[],
            )
            .unwrap();
    }
    for key in ["Alt", "Ctrl", "Shift", "Meta"] {
        input
            .execute(
                "device",
                "keyboard.modifierUp",
                &serde_json::json!({"key": key}),
                &[],
            )
            .unwrap();
    }

    assert_eq!(
        *overlay.changes.lock().unwrap(),
        vec![
            vec![ModifierKey::Shift],
            vec![ModifierKey::Ctrl, ModifierKey::Shift],
            vec![ModifierKey::Ctrl, ModifierKey::Shift, ModifierKey::Meta],
            vec![ModifierKey::Shift, ModifierKey::Meta],
            vec![ModifierKey::Meta],
            vec![],
        ]
    );
    assert_eq!(
        input.injector.keys,
        vec![
            ("Shift".into(), true),
            ("Ctrl".into(), true),
            ("Meta".into(), true),
            ("Ctrl".into(), false),
            ("Shift".into(), false),
            ("Meta".into(), false),
        ]
    );
}

#[test]
fn repeat_key_taps_press_and_release_without_holding_the_key() {
    let mut input = DesktopInput::new(FakeInjector::default());
    for _ in 0..3 {
        assert!(input.execute_repeat_key(RepeatKey::ArrowDown).is_ok());
    }
    assert_eq!(
        input.injector.keys,
        vec![
            ("ArrowDown".into(), true),
            ("ArrowDown".into(), false),
            ("ArrowDown".into(), true),
            ("ArrowDown".into(), false),
            ("ArrowDown".into(), true),
            ("ArrowDown".into(), false),
        ]
    );
    // Nothing is held between ticks, so cleanup has nothing to release.
    assert!(input.pending_key_releases.is_empty());
    assert!(input.release_all().is_ok());
    assert_eq!(input.injector.keys.len(), 6);
}

#[test]
fn a_failed_repeat_key_release_is_retried_by_release_all() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.injector.fail_key_up = Some("Tab".into());
    assert!(input.execute_repeat_key(RepeatKey::Tab).is_err());
    assert_eq!(
        input.pending_key_releases,
        HashSet::from([RepeatKey::Tab]),
        "a key whose release failed must be remembered"
    );

    // The retry succeeds once injection recovers, and the key is cleared.
    input.injector.fail_key_up = None;
    assert!(input.release_all().is_ok());
    assert!(input.pending_key_releases.is_empty());
    assert_eq!(
        input.injector.keys,
        vec![
            ("Tab".into(), true),
            ("Tab".into(), false),
            ("Tab".into(), false),
        ]
    );
}

#[test]
fn a_still_failing_repeat_key_release_stays_pending_for_the_next_cleanup() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.injector.fail_key_up = Some("Delete".into());
    assert!(input.execute_repeat_key(RepeatKey::Delete).is_err());
    assert!(input.release_all().is_err());
    assert_eq!(
        input.pending_key_releases,
        HashSet::from([RepeatKey::Delete]),
        "an unrecovered key must not be forgotten"
    );

    input.injector.fail_key_up = None;
    assert!(input.release_all().is_ok());
    assert!(input.pending_key_releases.is_empty());
}

#[test]
fn release_repeat_keys_frees_a_stuck_key_without_waiting_for_a_disconnect() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.injector.fail_key_up = Some("Backspace".into());
    assert!(input.execute_repeat_key(RepeatKey::Backspace).is_err());

    // Stopping the repeat must free the key: while it is held the OS keeps
    // auto-repeating it into the focused application.
    input.injector.fail_key_up = None;
    assert!(input.release_repeat_keys().is_ok());
    assert!(input.pending_key_releases.is_empty());
    assert_eq!(
        input.injector.keys,
        vec![
            ("Backspace".into(), true),
            ("Backspace".into(), false),
            ("Backspace".into(), false),
        ]
    );
}

#[test]
fn a_key_stuck_after_a_failed_retry_is_freed_by_a_later_stop() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.injector.fail_key_up = Some("ArrowLeft".into());
    assert!(input.execute_repeat_key(RepeatKey::ArrowLeft).is_err());
    // The retry at stop time fails too, so the key is still down with no
    // repeat left to hang the recovery off.
    assert!(input.release_repeat_keys().is_err());
    assert_eq!(
        input.pending_key_releases,
        HashSet::from([RepeatKey::ArrowLeft])
    );

    // Stop paths run this unconditionally, so recovery arrives without
    // needing another repeat to be active.
    input.injector.fail_key_up = None;
    assert!(input.release_repeat_keys().is_ok());
    assert!(input.pending_key_releases.is_empty());
    assert_eq!(
        input.injector.keys.last().unwrap(),
        &("ArrowLeft".to_string(), false)
    );
}

#[test]
fn release_repeat_keys_injects_nothing_when_no_key_is_stuck() {
    let mut input = DesktopInput::new(FakeInjector::default());
    assert!(input.execute_repeat_key(RepeatKey::ArrowUp).is_ok());
    let injected = input.injector.keys.len();

    // The common case runs on every repeat stop, so it must not inject.
    assert!(input.release_repeat_keys().is_ok());
    assert_eq!(input.injector.keys.len(), injected);
}

#[test]
fn repeat_keys_are_refused_during_a_switch_session() {
    // The repeat path bypasses execute(), so the runtimes enforce this guard
    // themselves; this pins the message and the underlying predicate.
    let mut input = DesktopInput::new(FakeInjector::default());
    assert!(!input.has_active_switch_session());
    let profile = SwitchProfile {
        id: "profile".into(),
        version: 1,
        name: "Profile".into(),
        provider: "mapped".into(),
        built_in: false,
        bindings: Vec::new(),
    };
    input
        .execute(
            "device",
            "switch.session.start",
            &serde_json::json!({
                "sessionId": uuid::Uuid::new_v4().to_string(),
                "profileId": "profile",
                "profileVersion": 1,
                "switchCount": 1
            }),
            std::slice::from_ref(&profile),
        )
        .unwrap();
    assert!(input.has_active_switch_session());
    assert_eq!(
        input.execute(
            "device",
            "keyboard.key",
            &serde_json::json!({"key": "ArrowDown"}),
            &[],
        ),
        Err("Stop Switch Forwarding before using other PC control commands.".into())
    );
}

#[test]
fn a_later_successful_tap_clears_an_earlier_pending_release() {
    let mut input = DesktopInput::new(FakeInjector::default());
    input.injector.fail_key_up = Some("PageUp".into());
    assert!(input.execute_repeat_key(RepeatKey::PageUp).is_err());
    assert!(!input.pending_key_releases.is_empty());

    input.injector.fail_key_up = None;
    assert!(input.execute_repeat_key(RepeatKey::PageUp).is_ok());
    assert!(input.pending_key_releases.is_empty());
}

#[test]
fn repeat_key_taps_leave_latched_modifiers_held() {
    let mut input = DesktopInput::new(FakeInjector::default());
    assert!(input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Shift"}),
            &[],
        )
        .is_ok());
    for _ in 0..2 {
        assert!(input.execute_repeat_key(RepeatKey::ArrowRight).is_ok());
    }
    // Shift stays down across every tick so a repeating arrow extends a
    // selection rather than moving the caret.
    assert_eq!(
        input.injector.keys,
        vec![
            ("Shift".into(), true),
            ("ArrowRight".into(), true),
            ("ArrowRight".into(), false),
            ("ArrowRight".into(), true),
            ("ArrowRight".into(), false),
        ]
    );
    assert!(input.release_all().is_ok());
    assert_eq!(
        input.injector.keys.last().unwrap(),
        &("Shift".into(), false)
    );
}

#[test]
fn modifier_failures_keep_overlay_consistent() {
    let (mut down_input, down_overlay) = input_with_modifier_overlay();
    down_input.injector.fail_key_down = true;
    assert!(down_input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Ctrl"}),
            &[],
        )
        .is_err());
    assert!(down_overlay.changes.lock().unwrap().is_empty());
    assert!(down_input.held_modifiers.is_empty());

    let (mut up_input, up_overlay) = input_with_modifier_overlay();
    up_input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Ctrl"}),
            &[],
        )
        .unwrap();
    up_input.injector.fail_key_up = Some("Ctrl".into());
    assert!(up_input
        .execute(
            "device",
            "keyboard.modifierUp",
            &serde_json::json!({"key": "Ctrl"}),
            &[],
        )
        .is_err());
    assert_eq!(
        *up_overlay.changes.lock().unwrap(),
        vec![vec![ModifierKey::Ctrl], vec![]]
    );
    assert!(up_input.held_modifiers.is_empty());
    assert_eq!(
        up_input.pending_modifier_releases,
        HashSet::from([ModifierKey::Ctrl])
    );

    up_input.injector.fail_key_up = None;
    up_input
        .execute(
            "device",
            "keyboard.modifierUp",
            &serde_json::json!({"key": "Ctrl"}),
            &[],
        )
        .unwrap();
    assert!(up_input.pending_modifier_releases.is_empty());
    assert_eq!(
        up_input.injector.keys,
        vec![
            ("Ctrl".into(), true),
            ("Ctrl".into(), false),
            ("Ctrl".into(), false),
        ]
    );
}

#[test]
fn cleanup_uses_release_order_and_retains_unattempted_modifiers() {
    let (mut input, overlay) = input_with_modifier_overlay();
    for key in ["Ctrl", "Alt", "Shift", "Meta"] {
        input
            .execute(
                "device",
                "keyboard.modifierDown",
                &serde_json::json!({"key": key}),
                &[],
            )
            .unwrap();
    }
    input.injector.fail_key_up = Some("Shift".into());

    assert!(input.release_all().is_err());
    assert_eq!(
        input.held_modifiers,
        HashSet::from([ModifierKey::Ctrl, ModifierKey::Alt])
    );
    assert_eq!(
        input.pending_modifier_releases,
        HashSet::from([ModifierKey::Shift])
    );
    assert_eq!(
        overlay.changes.lock().unwrap().last().cloned(),
        Some(vec![ModifierKey::Ctrl, ModifierKey::Alt])
    );

    input.end_control_session();
    assert!(input.held_modifiers.is_empty());
    assert_eq!(
        input.pending_modifier_releases,
        HashSet::from([ModifierKey::Ctrl, ModifierKey::Alt, ModifierKey::Shift])
    );

    input.injector.fail_key_up = None;
    input.release_all().unwrap();
    let releases = input
        .injector
        .keys
        .iter()
        .filter(|(_, down)| !down)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        releases,
        vec![
            ("Meta".into(), false),
            ("Shift".into(), false),
            ("Shift".into(), false),
            ("Alt".into(), false),
            ("Ctrl".into(), false),
        ]
    );
    assert!(input.pending_modifier_releases.is_empty());
    assert_eq!(*overlay.ended.lock().unwrap(), 1);
}

#[test]
fn cleanup_releases_modifiers_even_when_drag_release_fails() {
    let (mut input, overlay) = input_with_modifier_overlay();
    input
        .execute(
            "device",
            "mouse.dragStart",
            &serde_json::json!({"button": "left"}),
            &[],
        )
        .unwrap();
    input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Ctrl"}),
            &[],
        )
        .unwrap();
    input.injector.fail_pointer_release = true;

    assert!(input.release_all().is_err());
    assert_eq!(input.pointer_feedback_for_move(), PointerFeedback::Drag);
    assert!(input.held_modifiers.is_empty());
    assert!(overlay.changes.lock().unwrap().last().unwrap().is_empty());
}

#[test]
fn ending_control_session_clears_and_hides_modifier_state() {
    let (mut input, overlay) = input_with_modifier_overlay();
    input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Ctrl"}),
            &[],
        )
        .unwrap();

    input.end_control_session();

    assert!(input.held_modifiers.is_empty());
    assert_eq!(
        input.pending_modifier_releases,
        HashSet::from([ModifierKey::Ctrl])
    );
    assert_eq!(*overlay.ended.lock().unwrap(), 1);
}

#[test]
fn ending_control_session_retains_failed_release_for_later_cleanup() {
    let (mut input, overlay) = input_with_modifier_overlay();
    input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Alt"}),
            &[],
        )
        .unwrap();
    input.injector.fail_key_up = Some("Alt".into());

    assert!(input.release_all().is_err());
    input.end_control_session();

    assert!(input.held_modifiers.is_empty());
    assert_eq!(
        input.pending_modifier_releases,
        HashSet::from([ModifierKey::Alt])
    );
    assert_eq!(*overlay.ended.lock().unwrap(), 1);

    input.injector.fail_key_up = None;
    input.release_all().unwrap();
    assert!(input.pending_modifier_releases.is_empty());
}

#[test]
fn disconnecting_releases_modifiers_and_ends_the_overlay_session() {
    let (mut input, overlay) = input_with_modifier_overlay();
    input
        .execute(
            "device",
            "keyboard.modifierDown",
            &serde_json::json!({"key": "Meta"}),
            &[],
        )
        .unwrap();

    input
        .execute(
            "device",
            "connection.disconnecting",
            &serde_json::json!({}),
            &[],
        )
        .unwrap();

    assert_eq!(
        input.injector.keys,
        vec![("Meta".into(), true), ("Meta".into(), false)]
    );
    assert_eq!(*overlay.ended.lock().unwrap(), 1);
}

#[test]
fn switch_profile_held_keys_do_not_affect_modifier_overlay() {
    let profile = SwitchProfile {
        id: "modifier-output".into(),
        version: 1,
        name: "Modifier output".into(),
        provider: "mapped".into(),
        built_in: false,
        bindings: vec![SwitchBinding {
            switch_id: 1,
            binding_type: "key".into(),
            value: Some("Ctrl".into()),
            keys: None,
            click_count: None,
        }],
    };
    let (mut input, overlay) = input_with_modifier_overlay();
    let session_id = uuid::Uuid::new_v4().to_string();
    input
        .execute(
            "device",
            "switch.session.start",
            &serde_json::json!({
                "sessionId": session_id,
                "profileId": profile.id,
                "profileVersion": 1,
                "switchCount": 1
            }),
            std::slice::from_ref(&profile),
        )
        .unwrap();
    input
        .execute(
            "device",
            "switch.edge",
            &serde_json::json!({
                "sessionId": session_id,
                "sequence": 1,
                "switchId": 1,
                "state": "down"
            }),
            &[profile],
        )
        .unwrap();

    assert!(overlay.changes.lock().unwrap().is_empty());
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
