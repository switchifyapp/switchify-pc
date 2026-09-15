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
    #[derive(Default)]
    struct Fake {
        events: Vec<String>,
        fail_release: bool,
        fail_click: bool,
    }
    impl InputInjector for Fake {
        fn inject_text(&mut self, _: &str) -> Result<(), String> {
            panic!("No typing keyboard")
        }
        fn move_pointer(&mut self, _: i32, _: i32) -> Result<(), String> {
            panic!("No relative movement")
        }
        fn move_pointer_absolute(&mut self, x: i32, y: i32) -> Result<(), String> {
            self.events.push(format!("move {x} {y}"));
            Ok(())
        }
        fn click_pointer(&mut self, button: MouseButton, count: u8) -> Result<(), String> {
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
