//! Hands a copy started by the elevated installer back to Explorer.
//!
//! The installer's finish page ("Run Switchify PC") and its `/R` restart
//! start the app with `nsis_tauri_utils::RunAsUser`, which tries the
//! signed-in user's token first. Windows will not create a UIAccess
//! executable that way (`ERROR_ELEVATION_REQUIRED`), so it falls back to
//! `ShellExecute` from the installer and the app runs elevated (#1564).
//! The template defines that launch itself, so the installer hooks cannot
//! replace it.
//!
//! Instead the post-install hook sets [`MARKER`] in the installer's
//! environment. Only a process the installer starts itself inherits it: a
//! process created with a user token gets that user's own environment. A
//! copy that sees the marker and is elevated starts Explorer with the
//! desktop's token, as the update relaunch in the hooks does (#1560), and
//! exits before it opens anything. Explorer starts the app the way Start
//! does: unelevated, with UIAccess granted by the Shell. If that fails, the
//! copy keeps running elevated, which is better than not running. Explorer
//! cannot pass arguments, so any `/ARGS` given to the installer are dropped.
use std::{
    ffi::{OsStr, OsString},
    mem,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FALSE, HANDLE, LUID},
    Security::{
        AdjustTokenPrivileges, DuplicateTokenEx, GetTokenInformation, LookupPrivilegeValueW,
        SecurityAnonymous, TokenElevation, TokenPrimary, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_PRIVILEGES, TOKEN_ADJUST_SESSIONID,
        TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_ELEVATION, TOKEN_PRIVILEGES, TOKEN_QUERY,
    },
    System::{
        SystemInformation::GetSystemWindowsDirectoryW,
        Threading::{
            CreateProcessWithTokenW, GetCurrentProcess, OpenProcess, OpenProcessToken,
            PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, STARTUPINFOW,
        },
    },
    UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId},
};

/// Set by the installer hooks; see `installer-hooks.nsh`.
const MARKER: &str = "SWITCHIFY_STARTED_BY_INSTALLER";

/// Returns true when this copy was handed to Explorer and should exit.
pub fn hand_off_from_installer() -> bool {
    if std::env::var_os(MARKER).is_none() {
        return false;
    }
    // Nothing this copy starts (workers, a later relaunch) should see it.
    std::env::remove_var(MARKER);
    if !is_elevated() {
        return false;
    }
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    start_through_explorer(&executable)
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

fn is_elevated() -> bool {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == FALSE {
            return false;
        }
        let token = Handle(token);
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0;
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        ) != FALSE
            && elevation.TokenIsElevated != 0
    }
}

/// `CreateProcessWithTokenW` needs SeImpersonatePrivilege, which an
/// elevated administrator holds but may not have enabled. It is left
/// enabled: this copy exits right after.
fn enable_impersonation() -> bool {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &mut token) == FALSE {
            return false;
        }
        let token = Handle(token);
        let mut luid = LUID {
            LowPart: 0,
            HighPart: 0,
        };
        let name = wide(OsStr::new("SeImpersonatePrivilege"));
        if LookupPrivilegeValueW(ptr::null(), name.as_ptr(), &mut luid) == FALSE {
            return false;
        }
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        // Succeeds without assigning a privilege the token lacks; that case
        // shows up as CreateProcessWithTokenW failing.
        AdjustTokenPrivileges(
            token.0,
            FALSE,
            &privileges,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        ) != FALSE
    }
}

fn start_through_explorer(executable: &Path) -> bool {
    unsafe {
        let shell = GetShellWindow();
        if shell.is_null() {
            return false;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(shell, &mut process_id);
        if process_id == 0 {
            return false;
        }
        let process = Handle(OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            FALSE,
            process_id,
        ));
        if process.0.is_null() {
            return false;
        }
        let mut token = ptr::null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY | TOKEN_DUPLICATE, &mut token) == FALSE {
            return false;
        }
        let token = Handle(token);
        let mut primary = ptr::null_mut();
        if DuplicateTokenEx(
            token.0,
            TOKEN_QUERY
                | TOKEN_ASSIGN_PRIMARY
                | TOKEN_DUPLICATE
                | TOKEN_ADJUST_DEFAULT
                | TOKEN_ADJUST_SESSIONID,
            ptr::null(),
            SecurityAnonymous,
            TokenPrimary,
            &mut primary,
        ) == FALSE
        {
            return false;
        }
        let primary = Handle(primary);
        if !enable_impersonation() {
            return false;
        }
        let mut windows = [0u16; 260];
        let length =
            GetSystemWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) as usize;
        if length == 0 || length >= windows.len() {
            return false;
        }
        let explorer = PathBuf::from(OsString::from_wide(&windows[..length])).join("explorer.exe");
        let application = wide(explorer.as_os_str());
        let mut command_line = wide(&explorer_command_line(&explorer, executable));
        let startup = STARTUPINFOW {
            cb: mem::size_of::<STARTUPINFOW>() as u32,
            ..mem::zeroed()
        };
        let mut started: PROCESS_INFORMATION = mem::zeroed();
        if CreateProcessWithTokenW(
            primary.0,
            0,
            application.as_ptr(),
            command_line.as_mut_ptr(),
            0,
            ptr::null(),
            ptr::null(),
            &startup,
            &mut started,
        ) == FALSE
        {
            return false;
        }
        CloseHandle(started.hThread);
        CloseHandle(started.hProcess);
        true
    }
}

/// Paths cannot contain `"`, so quoting each one is enough.
fn explorer_command_line(explorer: &Path, executable: &Path) -> OsString {
    let mut line = OsString::from("\"");
    line.push(explorer);
    line.push("\" \"");
    line.push(executable);
    line.push("\"");
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_is_given_the_quoted_executable() {
        assert_eq!(
            explorer_command_line(
                Path::new(r"C:\Windows\explorer.exe"),
                Path::new(r"C:\Program Files\Switchify PC\switchify-pc.exe"),
            ),
            OsString::from(
                r#""C:\Windows\explorer.exe" "C:\Program Files\Switchify PC\switchify-pc.exe""#
            )
        );
    }
}
