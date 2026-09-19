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
        Request::OpenKeyboard { point } => {
            if input.has_active_drag() {
                return Err("End the active drag before typing.".into());
            }
            match point {
                Some(point) => input.click_pointer_at(point, MouseButton::Left, 1, &[]),
                None => Ok(()),
            }
        }
        Request::Keyboard(stroke) => {
            if input.has_active_drag() {
                return Err("End the active drag before typing.".into());
            }
            input.release_all()?;
            if let Some(character) = stroke.character().filter(|_| !stroke.shortcut()) {
                return input.type_text(&character.to_string());
            }
            let mut modifiers = stroke.modifiers;
            let name = match stroke.key {
                crate::scan_keyboard::Key::Character('+', '+') => {
                    modifiers[0] = true;
                    "=".into()
                }
                crate::scan_keyboard::Key::Character('*', '*') => {
                    modifiers[0] = true;
                    "8".into()
                }
                crate::scan_keyboard::Key::Character(base, _) => {
                    base.to_ascii_uppercase().to_string()
                }
                crate::scan_keyboard::Key::Named(name) => name.into(),
                _ => return Err("Invalid keyboard action.".into()),
            };
            let mut keys: Vec<&str> = ["Shift", "Ctrl", "Alt", "Meta"]
                .into_iter()
                .zip(modifiers)
                .filter_map(|(key, on)| on.then_some(key))
                .collect();
            keys.push(&name);
            input.scan_chord(&keys, None)
        }
        Request::Command { command, point } => execute_command(input, command, point),
        Request::Setting(_) | Request::Display(_) => {
            Err("Scanning action requires the scan controller.".into())
        }
        Request::Click {
            point,
            right,
            count,
        } => {
            if input.has_active_drag() {
                return Err("End the active drag before clicking.".into());
            }
            input.click_pointer_at(
                point,
                if right {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                },
                count,
                &[],
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
                enigo::Enigo::new(&crate::input::injection_settings())
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

pub fn shortcut(command: crate::scan_menu::Command, mac: bool) -> Option<Vec<&'static str>> {
    use crate::scan_menu::Command::*;
    let primary = if mac { "Meta" } else { "Ctrl" };
    Some(match command {
        Copy => vec![primary, "C"],
        Cut => vec![primary, "X"],
        Paste => vec![primary, "V"],
        SelectAll => vec![primary, "A"],
        Undo => vec![primary, "Z"],
        Redo => {
            if mac {
                vec![primary, "Shift", "Z"]
            } else {
                vec![primary, "Y"]
            }
        }
        Save => vec![primary, "S"],
        Find => vec![primary, "F"],
        BrowserBack => {
            if mac {
                vec![primary, "["]
            } else {
                vec!["Alt", "ArrowLeft"]
            }
        }
        BrowserForward => {
            if mac {
                vec![primary, "]"]
            } else {
                vec!["Alt", "ArrowRight"]
            }
        }
        Reload => vec![primary, "R"],
        NewTab => vec![primary, "T"],
        CloseTab => vec![primary, "W"],
        ReopenTab => vec![primary, "Shift", "T"],
        NextTab => vec!["Ctrl", "Tab"],
        PreviousTab => vec!["Ctrl", "Shift", "Tab"],
        Address => vec![primary, "L"],
        ZoomIn => vec![primary, "="],
        ZoomOut => vec![primary, "-"],
        ZoomReset => vec![primary, "0"],
        SwitchNext => vec![if mac { "Meta" } else { "Alt" }, "Tab"],
        SwitchPrevious => vec![if mac { "Meta" } else { "Alt" }, "Shift", "Tab"],
        Overview => {
            if mac {
                vec!["Ctrl", "ArrowUp"]
            } else {
                vec!["Meta", "Tab"]
            }
        }
        Desktop => {
            if mac {
                vec!["Meta", "F3"]
            } else {
                vec!["Meta", "D"]
            }
        }
        CloseWindow if !mac => vec!["Alt", "F4"],
        Minimize if mac => vec!["Meta", "M"],
        Maximize if mac => vec!["Ctrl", "Meta", "F"],
        _ => return None,
    })
}
fn execute_command<I: InputInjector>(
    input: &mut DesktopInput<I>,
    command: crate::scan_menu::Command,
    point: (i32, i32),
) -> Result<(), String> {
    use crate::scan_menu::Command::*;
    if input.has_active_drag() {
        return Err("End the active drag before choosing an action.".into());
    }
    if let Some(keys) = shortcut(command, cfg!(target_os = "macos")) {
        return input.scan_chord(&keys, None);
    }
    let modifier = match command {
        ShiftClick => Some("Shift"),
        CtrlClick => Some("Ctrl"),
        AltClick => Some("Alt"),
        MetaClick => Some("Meta"),
        _ => None,
    };
    if matches!(command, MiddleClick | TripleClick) || modifier.is_some() {
        let keys = modifier.into_iter().collect::<Vec<_>>();
        return input.scan_chord(
            &keys,
            Some((
                point,
                if command == MiddleClick {
                    MouseButton::Middle
                } else {
                    MouseButton::Left
                },
                if command == TripleClick { 3 } else { 1 },
            )),
        );
    }
    match command {
        PlayPause => input.injector.media("playPause"),
        NextTrack => input.injector.media("nextTrack"),
        PreviousTrack => input.injector.media("previousTrack"),
        VolumeUp => input.injector.media("volumeUp"),
        VolumeDown => input.injector.media("volumeDown"),
        Mute => input.injector.media("mute"),
        CloseWindow => input.injector.window("closeFocused"),
        Minimize => input.injector.window("minimizeFocused"),
        Maximize => input.injector.window("maximizeFocused"),
        _ => Err("Action is unavailable on this platform.".into()),
    }
}
pub fn move_to(point: (i32, i32)) -> Result<(), String> {
    INPUT.with(|slot| {
        let mut input = slot.borrow_mut();
        if input.is_none() {
            *input = Some(DesktopInput::new(
                enigo::Enigo::new(&crate::input::injection_settings())
                    .map_err(|_| "Scan input could not be initialized.")?,
            ));
        }
        input
            .as_mut()
            .unwrap()
            .move_pointer_absolute(point.0, point.1)
            .map(|_| ())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_menu::Command;
    type TargetClick = ((i32, i32), MouseButton, u8, Vec<String>);
    #[derive(Default)]
    struct Fake {
        events: Vec<String>,
        fail_release: bool,
        fail_click: bool,
        fail_move: bool,
        cursor: (i32, i32),
        target_clicks: Vec<TargetClick>,
        legacy_clicks: usize,
    }
    impl InputInjector for Fake {
        fn inject_text(&mut self, text: &str) -> Result<(), String> {
            self.events.push(format!("text {text}"));
            Ok(())
        }
        fn move_pointer(&mut self, _: i32, _: i32) -> Result<(), String> {
            panic!("No relative movement")
        }
        fn move_pointer_absolute(&mut self, x: i32, y: i32) -> Result<(), String> {
            self.events.push(format!("move {x} {y}"));
            if self.fail_move {
                return Err("move failed".into());
            }
            Ok(())
        }
        fn click_pointer_at(
            &mut self,
            point: (i32, i32),
            button: MouseButton,
            count: u8,
            modifiers: &[&str],
        ) -> Result<(), String> {
            self.move_pointer_absolute(point.0, point.1)?;
            if self.fail_click {
                return Err("click failed".into());
            }
            self.events.push(format!("click {button:?} {count}"));
            self.target_clicks.push((
                point,
                button,
                count,
                modifiers.iter().map(|key| (*key).into()).collect(),
            ));
            Ok(())
        }
        fn click_pointer(&mut self, button: MouseButton, count: u8) -> Result<(), String> {
            self.legacy_clicks += 1;
            self.events.push(format!("click {button:?} {count}"));
            if self.fail_click {
                Err("click failed".into())
            } else {
                Ok(())
            }
        }
        fn set_pointer_button(&mut self, button: MouseButton, down: bool) -> Result<(), String> {
            self.events.push(format!("button {button:?} {down}"));
            if !down && self.fail_release {
                Err("release failed".into())
            } else {
                Ok(())
            }
        }
        fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.events.push(format!("scroll {dx} {dy}"));
            Ok(())
        }
        fn set_key(&mut self, key: &str, down: bool) -> Result<(), String> {
            self.events.push(format!("key {key} {down}"));
            if !down && self.fail_release {
                Err("release failed".into())
            } else {
                Ok(())
            }
        }
        fn press_shortcut(&mut self, _: &[String]) -> Result<(), String> {
            panic!("Scan chords must retain failed releases")
        }
        fn media(&mut self, action: &str) -> Result<(), String> {
            self.events.push(format!("media {action}"));
            Ok(())
        }
        fn window(&mut self, action: &str) -> Result<(), String> {
            self.events.push(format!("window {action}"));
            Ok(())
        }
    }
    #[test]
    fn keyboard_text_chords_and_current_focus_use_fake_input() {
        use crate::scan_keyboard::{Key, Stroke};
        let mut input = DesktopInput::new(Fake::default());
        execute(&mut input, Request::OpenKeyboard { point: None }, true).unwrap();
        assert!(input.injector.events.is_empty());
        execute(
            &mut input,
            Request::Keyboard(Stroke {
                key: Key::Character('3', '£'),
                modifiers: [true, false, false, false],
                caps: false,
            }),
            true,
        )
        .unwrap();
        assert_eq!(input.injector.events, ["text £"]);
        input.injector.events.clear();
        execute(
            &mut input,
            Request::Keyboard(Stroke {
                key: Key::Character('a', 'A'),
                modifiers: [false, true, false, false],
                caps: false,
            }),
            true,
        )
        .unwrap();
        assert_eq!(
            input.injector.events,
            [
                "key Ctrl true",
                "key A true",
                "key A false",
                "key Ctrl false"
            ]
        );
        input.injector.events.clear();
        execute(
            &mut input,
            Request::Keyboard(Stroke {
                key: Key::Named("Escape"),
                modifiers: [false; 4],
                caps: false,
            }),
            true,
        )
        .unwrap();
        assert_eq!(
            input.injector.events,
            ["key Escape true", "key Escape false"]
        );
    }
    #[test]
    fn numeric_operator_shortcuts_include_required_shift_and_release_every_key() {
        use crate::scan_keyboard::{Key, Stroke};
        for (operator, native_key) in [('+', "="), ('*', "8")] {
            let mut input = DesktopInput::new(Fake::default());
            execute(
                &mut input,
                Request::Keyboard(Stroke {
                    key: Key::Character(operator, operator),
                    modifiers: [false, true, false, false],
                    caps: false,
                }),
                true,
            )
            .unwrap();
            assert_eq!(
                input.injector.events,
                [
                    "key Shift true".to_string(),
                    "key Ctrl true".to_string(),
                    format!("key {native_key} true"),
                    format!("key {native_key} false"),
                    "key Ctrl false".to_string(),
                    "key Shift false".to_string(),
                ]
            );
        }
    }
    #[test]
    fn keyboard_releases_failed_keys_and_rejects_external_modifiers() {
        use crate::scan_keyboard::{Key, Stroke};
        let request = Request::Keyboard(Stroke {
            key: Key::Named("Tab"),
            modifiers: [true, false, false, false],
            caps: false,
        });
        let mut input = DesktopInput::new(Fake {
            fail_release: true,
            ..Default::default()
        });
        assert!(execute(&mut input, request, true).is_err());
        input.injector.fail_release = false;
        input.release_all().unwrap();
        assert!(input
            .injector
            .events
            .ends_with(&["key Tab false".into(), "key Shift false".into()]));
        input.injector.events.clear();
        assert!(execute(&mut input, request, false).is_err());
        assert!(input.injector.events.is_empty());
    }
    #[test]
    fn type_here_clicks_exactly_once_and_propagates_failure() {
        let mut input = DesktopInput::new(Fake::default());
        execute(
            &mut input,
            Request::OpenKeyboard {
                point: Some((42, 70)),
            },
            true,
        )
        .unwrap();
        assert_eq!(input.injector.target_clicks.len(), 1);
        assert_eq!(input.injector.target_clicks[0].0, (42, 70));
        input.injector.fail_click = true;
        assert!(execute(
            &mut input,
            Request::OpenKeyboard {
                point: Some((42, 70))
            },
            true
        )
        .is_err());
        assert_eq!(input.injector.target_clicks.len(), 1);
    }
    #[test]
    fn every_scan_click_uses_target_even_while_cursor_readback_is_stale() {
        for point in [(-1600, -200), (3600, 1200)] {
            let mut cases = vec![
                (
                    crate::point_workflow::default_click(point),
                    MouseButton::Left,
                    1,
                    vec![],
                ),
                (
                    Request::Click {
                        point,
                        right: true,
                        count: 1,
                    },
                    MouseButton::Right,
                    1,
                    vec![],
                ),
                (
                    Request::Click {
                        point,
                        right: false,
                        count: 2,
                    },
                    MouseButton::Left,
                    2,
                    vec![],
                ),
            ];
            for (command, button, count, keys) in [
                (Command::MiddleClick, MouseButton::Middle, 1, vec![]),
                (Command::TripleClick, MouseButton::Left, 3, vec![]),
                (
                    Command::ShiftClick,
                    MouseButton::Left,
                    1,
                    vec!["Shift".into()],
                ),
                (
                    Command::CtrlClick,
                    MouseButton::Left,
                    1,
                    vec!["Ctrl".into()],
                ),
                (Command::AltClick, MouseButton::Left, 1, vec!["Alt".into()]),
                (
                    Command::MetaClick,
                    MouseButton::Left,
                    1,
                    vec!["Meta".into()],
                ),
            ] {
                cases.push((Request::Command { command, point }, button, count, keys));
            }
            for (request, button, count, keys) in cases {
                let mut input = DesktopInput::new(Fake {
                    cursor: (15, 20),
                    ..Default::default()
                });
                execute(&mut input, request, true).unwrap();
                assert_eq!(input.injector.cursor, (15, 20));
                assert_eq!(input.injector.legacy_clicks, 0);
                assert_eq!(input.injector.target_clicks, [(point, button, count, keys)]);
                assert!(!input.has_active_drag());
            }
        }
    }

    #[test]
    fn failed_point_move_never_clicks_and_cleanup_releases_owned_input() {
        for command in [None, Some(Command::ShiftClick)] {
            let mut input = DesktopInput::new(Fake {
                fail_move: true,
                ..Default::default()
            });
            let point = (-900, 400);
            let request = command.map_or(crate::point_workflow::default_click(point), |command| {
                Request::Command { command, point }
            });
            assert!(execute(&mut input, request, true).is_err());
            assert!(input.injector.target_clicks.is_empty());
            assert_eq!(input.injector.legacy_clicks, 0);
            input.release_all().unwrap();
            assert!(!input.has_active_drag());
        }
    }

    #[test]
    fn scanning_menu_scrolls_all_four_directions_and_repeats_without_leaving_menu() {
        use crate::point_scan::Config;
        use crate::point_workflow::{Phase, Workflow, WorkflowPhase};
        use crate::scanning::{Action, Rect, Session, Technique};

        for automatic in [false, true] {
            for (row, column, dx, dy) in [(0, 0, 0, 3), (0, 1, 0, -3), (1, 0, -3, 0), (1, 1, 3, 0)]
            {
                let workflow = Workflow::new(
                    Config {
                        block_interval_ms: 250,
                        ..Config::default()
                    }
                    .point(),
                    Rect {
                        x: -500.0,
                        y: 50.0,
                        width: 1000.0,
                        height: 800.0,
                    },
                    1.0,
                )
                .unwrap();
                let mut session = Session::new(workflow, automatic);
                for action in [
                    Action::Select,
                    Action::Select,
                    Action::Select,
                    Action::Next,
                    Action::Select,
                    Action::Select,
                ] {
                    assert_eq!(session.action(action), None);
                }
                for _ in 0..row {
                    assert_eq!(session.action(Action::Next), None);
                }
                assert_eq!(session.action(Action::Select), None);
                for _ in 0..column {
                    assert_eq!(session.action(Action::Next), None);
                }
                let mut input = DesktopInput::new(Fake::default());
                for _ in 0..2 {
                    let before = session.frame().tiles;
                    session.tick(100, false);
                    let request = session.action(Action::Select).unwrap();
                    assert_eq!(
                        request,
                        Request::Scroll {
                            point: (-500, 50),
                            dx,
                            dy
                        }
                    );
                    execute(&mut input, request, true).unwrap();
                    assert_eq!(
                        session.technique.phase(),
                        Phase::Workflow(WorkflowPhase::Menu)
                    );
                    session.tick(100, false);
                    assert_eq!(session.frame().tiles, before);
                    assert_eq!(session.take_selection(), None);
                }
                assert_eq!(
                    input.injector.events,
                    vec![
                        "move -500 50".to_string(),
                        format!("scroll {dx} {dy}"),
                        "move -500 50".to_string(),
                        format!("scroll {dx} {dy}"),
                    ]
                );
            }
        }
    }
    #[test]
    fn resetting_an_executing_drag_releases_input_and_retains_failed_cleanup_for_retry() {
        let mut input = DesktopInput::new(Fake::default());
        execute(&mut input, Request::DragStart((100, 200)), true).unwrap();
        execute(&mut input, Request::DragMove((150, 250)), true).unwrap();
        input.injector.fail_release = true;
        assert!(input.release_all().is_err());
        assert!(input.has_active_drag());
        input.injector.fail_release = false;
        input.release_all().unwrap();
        assert!(!input.has_active_drag());
        assert!(input
            .injector
            .events
            .ends_with(&["button Left false".into()]));
    }
    #[test]
    fn editing_shortcuts_preserve_focus_and_release_keys_in_reverse_order() {
        let mut input = DesktopInput::new(Fake::default());
        execute(
            &mut input,
            Request::Command {
                command: Command::Copy,
                point: (100, 200),
            },
            true,
        )
        .unwrap();
        let primary = if cfg!(target_os = "macos") {
            "Meta"
        } else {
            "Ctrl"
        };
        assert_eq!(
            input.injector.events,
            vec![
                format!("key {primary} true"),
                "key C true".into(),
                "key C false".into(),
                format!("key {primary} false")
            ]
        );
    }
    #[test]
    fn modified_click_and_failures_release_every_owned_key_and_button() {
        let mut input = DesktopInput::new(Fake {
            fail_click: true,
            fail_release: true,
            ..Default::default()
        });
        assert!(execute(
            &mut input,
            Request::Command {
                command: Command::ShiftClick,
                point: (100, 200)
            },
            true
        )
        .is_err());
        assert!(input.has_active_drag());
        input.injector.fail_release = false;
        input.release_all().unwrap();
        assert!(!input.has_active_drag());
        assert!(input
            .injector
            .events
            .ends_with(&["key Shift false".into(), "button Left false".into()]));
    }
    #[test]
    fn commands_reject_held_physical_modifiers_without_injection() {
        let mut input = DesktopInput::new(Fake::default());
        assert!(execute(
            &mut input,
            Request::Command {
                command: Command::Copy,
                point: (0, 0)
            },
            false
        )
        .is_err());
        assert!(input.injector.events.is_empty());
    }
    #[test]
    fn platform_mappings_and_input_marker_are_explicit() {
        assert_eq!(
            shortcut(Command::Redo, true).unwrap(),
            ["Meta", "Shift", "Z"]
        );
        assert_eq!(shortcut(Command::Redo, false).unwrap(), ["Ctrl", "Y"]);
        assert_eq!(
            shortcut(Command::CloseWindow, false).unwrap(),
            ["Alt", "F4"]
        );
        assert_eq!(shortcut(Command::CloseWindow, true), None);
        assert!(crate::input::own_input(crate::input::SCAN_EVENT_MARKER));
        assert!(!crate::input::own_input(0));
        assert!(!crate::input::own_input(1234));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn close_window_uses_native_window_action_without_typing_or_clicking() {
        let mut input = DesktopInput::new(Fake::default());
        execute(
            &mut input,
            Request::Command {
                command: Command::CloseWindow,
                point: (100, 200),
            },
            true,
        )
        .unwrap();
        assert_eq!(input.injector.events, ["window closeFocused"]);
    }
    #[test]
    fn media_and_extra_clicks_execute_once() {
        let mut input = DesktopInput::new(Fake::default());
        execute(
            &mut input,
            Request::Command {
                command: Command::VolumeUp,
                point: (0, 0),
            },
            true,
        )
        .unwrap();
        assert_eq!(input.injector.events, ["media volumeUp"]);
        input.injector.events.clear();
        execute(
            &mut input,
            Request::Command {
                command: Command::TripleClick,
                point: (100, 200),
            },
            true,
        )
        .unwrap();
        assert_eq!(input.injector.events, ["move 100 200", "click Left 3"]);
    }
}
