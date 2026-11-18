//! Shared handling for remote desktop commands from a paired phone.
//!
//! The macOS and Windows runtimes keep input in different places and check
//! permissions differently, but the order of work for a desktop command is the
//! same on both. That order lives here, and each runtime supplies its
//! platform-specific steps through [`DesktopHost`].

use enigo::Enigo;
use tauri::{AppHandle, Manager};

use crate::display_navigation::{self, NavigationError};
use crate::dwell::DwellController;
use crate::input::{
    execute_desktop_command, DesktopCommandOutcome, DesktopInput, MobileTypingRoute,
    PointerFeedback,
};
use crate::overlay::CursorOverlay;
use crate::protocol::{switch_profile_catalog_response, DesktopCommand};
use crate::state::{AppModel, AppSettings, SharedModel};

/// The platform-specific steps a runtime provides to [`dispatch`].
///
/// Host methods may lock the runtime's own state. [`dispatch`] never calls a
/// host method from inside a [`DesktopHost::with_input`] closure, and never
/// holds the shared model lock across a host call.
pub trait DesktopHost {
    fn app(&self) -> &AppHandle;
    fn shared(&self) -> &SharedModel;
    fn settings(&self) -> AppSettings;
    fn stop_repeats(&mut self);
    /// Releases held input before a switch session starts.
    fn release_input(&mut self) -> Result<(), String>;
    /// Handles `mouse.repeat.start`. Returns a reply, or `None` when the host
    /// sent it itself.
    fn start_repeat(&mut self, command: DesktopCommand) -> Option<String>;
    /// Handles `mouse.repeat.stop`, with the same reply rule as `start_repeat`.
    fn stop_repeat(&mut self, command: DesktopCommand) -> Option<String>;
    fn move_to_display(&mut self, direction: &str) -> Result<PointerFeedback, NavigationError>;
    /// Runs `operation` against the desktop input, or fails with the
    /// platform's message when input is not available.
    fn with_input<T>(
        &mut self,
        operation: impl FnOnce(&mut DesktopInput<Enigo>) -> Result<T, String>,
    ) -> Result<T, String>;
}

/// What [`dispatch`] produced for one command.
pub struct Dispatched {
    /// The reply to send to the phone, if any.
    pub response: Option<String>,
    /// Whether the command ran. Set only when the command reached input
    /// execution or display navigation, not for routed or early-handled ones.
    /// Only macOS records it, as activity.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub outcome: Option<Result<(), String>>,
}

impl Dispatched {
    fn reply(response: Option<String>) -> Self {
        Self {
            response,
            outcome: None,
        }
    }
}

/// Commands that end any dwell countdown before they run.
fn cancels_dwell(command_type: &str) -> bool {
    matches!(
        command_type,
        "mouse.scroll"
            | "mouse.dragStart"
            | "mouse.dragEnd"
            | "mouse.click"
            | "mouse.doubleClick"
            | "mouse.rightClick"
            | "switch.session.start"
            | "connection.disconnecting"
    )
}

pub fn dispatch<H: DesktopHost>(host: &mut H, command: DesktopCommand) -> Dispatched {
    let app = host.app().clone();
    if command.command_type == "switch.session.start" {
        host.stop_repeats();
        if let Err(error) = host.release_input() {
            return Dispatched::reply(complete(
                host.shared(),
                &command,
                Err(("input_failed", error.as_str())),
            ));
        }
    }
    if let Some(result) = crate::remote_scan::route(
        &app,
        &command.device_id,
        &command.command_type,
        &command.payload,
    ) {
        return Dispatched::reply(complete(
            host.shared(),
            &command,
            result
                .as_ref()
                .map(|_| ())
                .map_err(|e| ("input_failed", e.as_str())),
        ));
    }
    if command.command_type == "mouse.repeat.start" {
        return Dispatched::reply(host.start_repeat(command));
    }
    if command.command_type == "mouse.repeat.stop" {
        return Dispatched::reply(host.stop_repeat(command));
    }
    if cancels_dwell(&command.command_type) {
        app.state::<DwellController>().cancel(&app);
    }
    let typing_route = MobileTypingRoute::for_command(&command.command_type);
    let profiles = host
        .shared()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .profiles
        .clone();
    if command.command_type == "switch.profile.list" {
        host.stop_repeats();
        return Dispatched::reply(Some(crate::remote_scan::catalog(
            &app,
            &command.payload,
            switch_profile_catalog_response(&command.id, &profiles),
        )));
    }
    let moves_display = command.command_type == "pointer.display.move";
    let (result, error_code) = if moves_display {
        let direction = command.payload["direction"].as_str().unwrap_or_default();
        match display_navigation::run_navigation_command(
            host,
            direction,
            |host| host.stop_repeats(),
            |host, direction| host.move_to_display(direction),
            |host, feedback| {
                let settings = host.settings();
                host.app().state::<CursorOverlay>().show(feedback, settings);
            },
        ) {
            Ok(feedback) => (
                Ok(DesktopCommandOutcome {
                    pointer_feedback: Some(feedback),
                    typing_injected: false,
                }),
                "input_failed",
            ),
            Err(error) => (Err(error.message), error.code),
        }
    } else {
        if typing_route.is_eligible() {
            typing_route.prepare(
                || app.state::<DwellController>().cancel(&app),
                || host.stop_repeats(),
            );
        } else {
            host.stop_repeats();
        }
        let model = app.state::<AppModel>();
        (
            host.with_input(|input| {
                execute_desktop_command(
                    input,
                    &model,
                    &command.device_id,
                    &command.command_type,
                    &command.payload,
                    &profiles,
                )
            }),
            "input_failed",
        )
    };
    if !moves_display {
        if let Ok(outcome) = &result {
            let settings = host.settings();
            let overlay = app.state::<CursorOverlay>();
            typing_route.finish(outcome.typing_injected, || overlay.hide_for_typing());
            if !outcome.typing_injected {
                if let Some(feedback) = &outcome.pointer_feedback {
                    overlay.show(*feedback, settings);
                } else {
                    overlay.mark_control_active(settings);
                }
            }
            if command.command_type == "connection.disconnecting" {
                crate::scanning_runtime::cancel(&app);
                overlay.end_session();
            }
        }
    }
    if moves_display && result.is_ok() {
        app.state::<DwellController>().arm(&app);
    }
    let response = complete(
        host.shared(),
        &command,
        result
            .as_ref()
            .map(|_| ())
            .map_err(|error| (error_code, error.as_str())),
    );
    Dispatched {
        response,
        outcome: Some(result.map(|_| ())),
    }
}

fn complete(
    shared: &SharedModel,
    command: &DesktopCommand,
    result: Result<(), (&str, &str)>,
) -> Option<String> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_desktop_command_with_error(command, result)
}

#[cfg(test)]
mod tests {
    use super::cancels_dwell;

    #[test]
    fn pointer_actions_and_session_changes_cancel_dwell() {
        for command in [
            "mouse.scroll",
            "mouse.dragStart",
            "mouse.dragEnd",
            "mouse.click",
            "mouse.doubleClick",
            "mouse.rightClick",
            "switch.session.start",
            "connection.disconnecting",
        ] {
            assert!(cancels_dwell(command), "{command}");
        }
    }

    #[test]
    fn other_commands_leave_dwell_running() {
        for command in [
            "keyboard.key",
            "pointer.display.move",
            "pointer.speed.set",
            "switch.profile.list",
            "mouse.repeat.start",
        ] {
            assert!(!cancels_dwell(command), "{command}");
        }
    }
}
