use std::path::{Path, PathBuf};
use std::process::Command;

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, RegValue};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const CANONICAL_VALUE_NAME: &str = "app.switchify.pc";
const TAURI_VALUE_NAME: &str = "Switchify PC";
const LEGACY_TASK_NAME: &str = "Switchify PC";
const LAUNCHER_NAME: &str = "switchify-pc-startup.exe";
const STARTUP_APPROVED_ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub fn apply(enabled: bool) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    apply_for_executable(&executable, enabled)
}

pub fn repair(configured_enabled: bool) -> Result<bool, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let launcher = launcher_for(&executable)?;
    if !launcher.is_file() {
        return Ok(configured_enabled);
    }
    let enabled = detected_registration_state()?.unwrap_or(configured_enabled);
    apply_for_executable(&executable, enabled)?;
    Ok(enabled)
}

fn detected_registration_state() -> Result<Option<bool>, String> {
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let run = current_user
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .map_err(|error| error.to_string())?;
    let approved = current_user
        .open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_READ)
        .ok();

    for name in [CANONICAL_VALUE_NAME, TAURI_VALUE_NAME] {
        if let Ok(command) = run.get_value::<String, _>(name) {
            if is_recognized_launcher_command(&command) || is_recognized_legacy_command(&command) {
                return Ok(Some(startup_approved(approved.as_ref(), name)));
            }
        }
    }
    Ok(legacy_task_state())
}

fn apply_for_executable(executable: &Path, enabled: bool) -> Result<(), String> {
    let launcher = launcher_for(executable)?;
    if enabled && !launcher.is_file() {
        return Err("The installed startup launcher is missing.".into());
    }

    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let run = current_user
        .open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_SET_VALUE)
        .map_err(|error| error.to_string())?;
    let approved = current_user
        .open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_READ | KEY_SET_VALUE)
        .ok();

    if enabled {
        run.set_value(CANONICAL_VALUE_NAME, &quoted_path(&launcher))
            .map_err(|error| error.to_string())?;
        if let Some(approved) = &approved {
            approved
                .set_raw_value(
                    CANONICAL_VALUE_NAME,
                    &RegValue {
                        bytes: STARTUP_APPROVED_ENABLED.to_vec(),
                        vtype: winreg::enums::RegType::REG_BINARY,
                    },
                )
                .map_err(|error| error.to_string())?;
        }
    } else {
        delete_value_if_present(&run, CANONICAL_VALUE_NAME)?;
        if let Some(approved) = &approved {
            delete_value_if_present(approved, CANONICAL_VALUE_NAME)?;
        }
    }

    remove_recognized_legacy_value(&run, approved.as_ref(), TAURI_VALUE_NAME)?;
    remove_legacy_task_if_owned();
    Ok(())
}

fn launcher_for(executable: &Path) -> Result<PathBuf, String> {
    executable
        .parent()
        .map(|directory| directory.join(LAUNCHER_NAME))
        .ok_or_else(|| "The application directory could not be determined.".into())
}

fn quoted_path(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn remove_recognized_legacy_value(
    run: &RegKey,
    approved: Option<&RegKey>,
    name: &str,
) -> Result<(), String> {
    let command = run.get_value::<String, _>(name).ok();
    if command.as_deref().is_some_and(is_recognized_legacy_command) {
        delete_value_if_present(run, name)?;
        if let Some(approved) = approved {
            delete_value_if_present(approved, name)?;
        }
    }
    Ok(())
}

fn is_recognized_legacy_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    command.contains("--start-hidden")
        && (command.contains("switchify-pc.exe") || command.contains("switchify pc.exe"))
}

fn is_recognized_launcher_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    command.contains("switchify-pc-startup.exe") || command.contains("switchify pc startup.exe")
}

fn startup_approved(key: Option<&RegKey>, name: &str) -> bool {
    key.and_then(|key| key.get_raw_value(name).ok())
        .and_then(|value| value.bytes.first().copied())
        != Some(3)
}

fn delete_value_if_present(key: &RegKey, name: &str) -> Result<(), String> {
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn legacy_task_state() -> Option<bool> {
    let output = Command::new("schtasks.exe")
        .args(["/Query", "/TN", LEGACY_TASK_NAME, "/XML"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let xml = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    xml.contains("--start-hidden")
        .then(|| !xml.contains("<enabled>false</enabled>"))
}

fn remove_legacy_task_if_owned() {
    if legacy_task_state().is_some() {
        let _ = Command::new("schtasks.exe")
            .args(["/Delete", "/TN", LEGACY_TASK_NAME, "/F"])
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_switchify_hidden_start_commands() {
        assert!(is_recognized_legacy_command(
            r#"\"C:\Program Files\Switchify PC\Switchify PC.exe\" --start-hidden"#
        ));
        assert!(is_recognized_legacy_command(
            r#"C:\Program Files\Switchify PC\switchify-pc.exe --start-hidden"#
        ));
        assert!(!is_recognized_legacy_command(
            r#"C:\Other\App.exe --start-hidden"#
        ));
        assert!(!is_recognized_legacy_command(
            r#"C:\Program Files\Switchify PC\switchify-pc.exe"#
        ));
    }

    #[test]
    fn recognizes_current_and_legacy_launchers() {
        assert!(is_recognized_launcher_command(
            r#"\"C:\Program Files\Switchify PC\switchify-pc-startup.exe\""#
        ));
        assert!(is_recognized_launcher_command(
            r#"\"C:\Program Files\Switchify PC\Switchify PC Startup.exe\""#
        ));
        assert!(!is_recognized_launcher_command(
            r#"\"C:\Program Files\Other App\startup.exe\""#
        ));
    }

    #[test]
    fn launcher_is_a_sibling_of_the_main_executable() {
        assert_eq!(
            launcher_for(Path::new(r"C:\Program Files\Switchify PC\switchify-pc.exe")).unwrap(),
            PathBuf::from(r"C:\Program Files\Switchify PC\switchify-pc-startup.exe")
        );
    }
}
