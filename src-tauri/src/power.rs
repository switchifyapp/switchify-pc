//! Shut down or restart the computer on behalf of a scanning menu selection.
//!
//! Both platforms ask the operating system for an ordinary, unforced shutdown
//! so other applications can still save or object. Nothing here touches input.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ShutDown,
    Restart,
}

impl Action {
    /// The verb as it reads in messages and in the System Events script.
    pub fn verb(self) -> &'static str {
        match self {
            Self::ShutDown => "shut down",
            Self::Restart => "restart",
        }
    }
}

/// Asks the operating system to shut down or restart. Returns once the request
/// has been accepted; the system then closes applications, including this one.
pub fn request(action: Action) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return windows::request(action);
    #[cfg(target_os = "macos")]
    return macos::request(action);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    Err(format!("Cannot {} from this platform.", action.verb()))
}

#[cfg(target_os = "windows")]
mod windows {
    use super::Action;
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE, LUID,
    };
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Shutdown::{
        ExitWindowsEx, EWX_POWEROFF, EWX_REBOOT, EWX_SHUTDOWN, SHTDN_REASON_FLAG_PLANNED,
        SHTDN_REASON_MAJOR_OTHER, SHTDN_REASON_MINOR_OTHER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    pub fn request(action: Action) -> Result<(), String> {
        enable_shutdown_privilege()?;
        let flags = match action {
            Action::ShutDown => EWX_SHUTDOWN | EWX_POWEROFF,
            Action::Restart => EWX_REBOOT,
        };
        let reason =
            SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER | SHTDN_REASON_FLAG_PLANNED;
        // No EWX_FORCE: applications keep the chance to save or block the request.
        unsafe { ExitWindowsEx(flags, reason) }
            .map_err(|error| format!("Windows refused the {} request: {error}", action.verb()))
    }

    /// `ExitWindowsEx` needs SeShutdownPrivilege enabled on the calling process token.
    /// Interactive users hold it disabled by default.
    pub(super) fn enable_shutdown_privilege() -> Result<(), String> {
        let mut token = HANDLE::default();
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            )
        }
        .map_err(|error| format!("Could not open the process token: {error}"))?;
        let result = enable_privilege_on(token);
        unsafe {
            let _ = CloseHandle(token);
        }
        result
    }

    fn enable_privilege_on(token: HANDLE) -> Result<(), String> {
        let mut luid = LUID::default();
        unsafe { LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid) }
            .map_err(|error| format!("Could not look up the shutdown privilege: {error}"))?;
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        unsafe { AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None) }
            .map_err(|error| format!("Could not enable the shutdown privilege: {error}"))?;
        // AdjustTokenPrivileges succeeds even when it assigned nothing.
        if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
            return Err("This account is not allowed to shut down the computer.".into());
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::Action;
    use std::process::{Command, Output, Stdio};
    use std::time::{Duration, Instant};

    /// How long an immediate refusal (a denied Automation permission, a script
    /// error) is waited for so it can show in the menu like a Windows failure.
    /// This wait happens on the caller's thread, the main thread, so it is kept
    /// short; a refusal comes back well inside it.
    const PROMPT_WAIT: Duration = Duration::from_millis(300);

    /// System Events performs the same unforced shut down or restart as the Apple
    /// menu. A quick failure is reported to the caller. The first use may show the
    /// Automation permission prompt, and a blocked shutdown keeps osascript waiting,
    /// so a longer wait moves to its own thread; the scan loop and the overlays
    /// must keep running meanwhile.
    pub fn request(action: Action) -> Result<(), String> {
        let verb = action.verb();
        let script = format!("tell application \"System Events\" to {verb}");
        let mut child = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&script)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not run osascript: {error}"))?;
        let started = Instant::now();
        while started.elapsed() < PROMPT_WAIT {
            match child.try_wait() {
                Ok(Some(_)) => return report(child.wait_with_output(), verb),
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(error) => {
                    let _ = child.kill();
                    return Err(format!("Could not wait for osascript: {error}"));
                }
            }
        }
        std::thread::Builder::new()
            .name("switchify-power-request".into())
            .spawn(move || {
                if let Err(message) = report(child.wait_with_output(), verb) {
                    eprintln!("Switchify PC: {message}");
                }
            })
            .map_err(|error| format!("Could not wait for osascript: {error}"))?;
        Ok(())
    }

    /// osascript's stderr carries only its own error text, never typed input.
    fn report(output: std::io::Result<Output>, verb: &str) -> Result<(), String> {
        let output = output.map_err(|error| format!("Could not wait for osascript: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if detail.is_empty() {
            format!("macOS refused the {verb} request.")
        } else {
            format!("macOS refused the {verb} request: {detail}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbs_read_as_plain_words() {
        assert_eq!(Action::ShutDown.verb(), "shut down");
        assert_eq!(Action::Restart.verb(), "restart");
    }

    /// Only adjusts this process's own token; nothing is shut down.
    #[cfg(target_os = "windows")]
    #[test]
    fn interactive_accounts_can_enable_the_shutdown_privilege() {
        super::windows::enable_shutdown_privilege().unwrap();
    }
}
