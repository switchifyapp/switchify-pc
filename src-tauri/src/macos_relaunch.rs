use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use directories::ProjectDirs;
use tauri::{AppHandle, Manager};

use crate::diagnostics::sanitize_detail;

const RELAUNCH_AFTER_PID_ARGUMENT: &str = "--switchify-relaunch-after-pid";
const WAIT_INTERVAL: Duration = Duration::from_millis(100);
const MAX_WAIT_ATTEMPTS: usize = 600;
const FAILURE_MARKER_FILE: &str = "updater-relaunch-failure.txt";

pub fn spawn_after_update(app: &AppHandle) -> Result<(), String> {
    let executable =
        tauri::process::current_binary(&app.env()).map_err(|error| error.to_string())?;
    bundle_path_for_executable(&executable)?;
    Command::new(executable)
        .arg(RELAUNCH_AFTER_PID_ARGUMENT)
        .arg(std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn run_from_args() -> bool {
    let args = std::env::args_os().collect::<Vec<_>>();
    let Some(result) = parse_relaunch_args(&args) else {
        return false;
    };
    let result = result
        .and_then(|parent_pid| {
            std::env::current_exe()
                .map_err(|error| error.to_string())
                .map(|executable| (parent_pid, executable))
        })
        .and_then(|(parent_pid, executable)| {
            bundle_path_for_executable(&executable).map(|bundle| (parent_pid, bundle))
        })
        .and_then(|(parent_pid, bundle)| {
            run_relaunch(parent_pid, &bundle, &mut SystemRelaunchPlatform)
        });
    if let Err(error) = result {
        let _ = write_failure_marker(&failure_marker_path(), &error);
    }
    true
}

pub fn take_failure() -> Option<String> {
    take_failure_marker(&failure_marker_path())
}

fn failure_marker_path() -> PathBuf {
    ProjectDirs::from("com", "Enabo Apps", "Switchify PC")
        .map(|directories| directories.config_dir().join(FAILURE_MARKER_FILE))
        .unwrap_or_else(|| std::env::temp_dir().join(FAILURE_MARKER_FILE))
}

fn write_failure_marker(path: &Path, error: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The updater restart diagnostic directory is unavailable.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let detail = sanitize_detail(error);
    if let Err(error) = writeln!(file, "{detail}").and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

fn take_failure_marker(path: &Path) -> Option<String> {
    let detail = fs::read_to_string(path).ok()?;
    fs::remove_file(path).ok()?;
    let detail = sanitize_detail(detail.trim());
    (!detail.is_empty()).then_some(detail)
}

fn parse_relaunch_args(args: &[OsString]) -> Option<Result<u32, String>> {
    if args.get(1).and_then(|argument| argument.to_str()) != Some(RELAUNCH_AFTER_PID_ARGUMENT) {
        return None;
    }
    if args.len() != 3 {
        return Some(Err("The relaunch helper arguments are invalid.".into()));
    }
    let pid = args[2]
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid > 1 && *pid <= libc::pid_t::MAX as u32)
        .ok_or_else(|| "The relaunch parent process is invalid.".to_string());
    Some(pid)
}

fn bundle_path_for_executable(executable: &Path) -> Result<PathBuf, String> {
    let macos = executable
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "MacOS"))
        .ok_or_else(|| "The relaunch executable is outside a macOS app bundle.".to_string())?;
    let contents = macos
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
        .ok_or_else(|| "The relaunch executable has no Contents directory.".to_string())?;
    let bundle = contents
        .parent()
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| "The relaunch executable has no app bundle.".to_string())?;
    Ok(bundle.to_path_buf())
}

trait RelaunchPlatform {
    fn process_exists(&mut self, pid: u32) -> bool;
    fn sleep(&mut self, duration: Duration);
    fn open_bundle(&mut self, bundle: &Path) -> Result<(), String>;
}

fn run_relaunch(
    parent_pid: u32,
    bundle: &Path,
    platform: &mut impl RelaunchPlatform,
) -> Result<(), String> {
    for _ in 0..MAX_WAIT_ATTEMPTS {
        if !platform.process_exists(parent_pid) {
            return platform.open_bundle(bundle);
        }
        platform.sleep(WAIT_INTERVAL);
    }
    Err("The previous Switchify process did not exit in time.".into())
}

struct SystemRelaunchPlatform;

impl RelaunchPlatform for SystemRelaunchPlatform {
    fn process_exists(&mut self, pid: u32) -> bool {
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn open_bundle(&mut self, bundle: &Path) -> Result<(), String> {
        let status = open_bundle_command(bundle)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("The macOS app launcher exited with {status}."))
        }
    }
}

fn open_bundle_command(bundle: &Path) -> Command {
    let mut command = Command::new("/usr/bin/open");
    command.arg("-n").arg(bundle);
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakePlatform {
        process_states: VecDeque<bool>,
        sleeps: usize,
        opened: Vec<PathBuf>,
        open_error: Option<String>,
    }

    impl RelaunchPlatform for FakePlatform {
        fn process_exists(&mut self, _pid: u32) -> bool {
            self.process_states.pop_front().unwrap_or(true)
        }

        fn sleep(&mut self, _duration: Duration) {
            self.sleeps += 1;
        }

        fn open_bundle(&mut self, bundle: &Path) -> Result<(), String> {
            self.opened.push(bundle.to_path_buf());
            match self.open_error.as_ref() {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }
    }

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_only_a_valid_internal_relaunch_request() {
        assert!(parse_relaunch_args(&args(&["switchify-pc"])).is_none());
        assert_eq!(
            parse_relaunch_args(&args(&["switchify-pc", RELAUNCH_AFTER_PID_ARGUMENT, "42"])),
            Some(Ok(42))
        );
        assert!(
            parse_relaunch_args(&args(&["switchify-pc", RELAUNCH_AFTER_PID_ARGUMENT, "0"]))
                .unwrap()
                .is_err()
        );
        assert!(parse_relaunch_args(&args(&[
            "switchify-pc",
            RELAUNCH_AFTER_PID_ARGUMENT,
            "42",
            "unexpected"
        ]))
        .unwrap()
        .is_err());
    }

    #[test]
    fn derives_the_app_bundle_from_its_executable() {
        assert_eq!(
            bundle_path_for_executable(Path::new(
                "/Applications/Switchify PC.app/Contents/MacOS/switchify-pc"
            )),
            Ok(PathBuf::from("/Applications/Switchify PC.app"))
        );
        assert!(bundle_path_for_executable(Path::new("/tmp/switchify-pc")).is_err());
    }

    #[test]
    fn forces_launch_services_to_start_a_new_instance() {
        let command = open_bundle_command(Path::new("/Applications/Switchify PC.app"));
        assert_eq!(command.get_program(), "/usr/bin/open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-n", "/Applications/Switchify PC.app"]
        );
    }

    #[test]
    fn failure_marker_is_sanitized_consumed_and_user_only() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("switchify-relaunch-{}", uuid::Uuid::new_v4()));
        let marker = root.join(FAILURE_MARKER_FILE);
        write_failure_marker(&marker, "failed at /Users/example/secret").unwrap();

        assert_eq!(
            fs::metadata(&marker).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            take_failure_marker(&marker),
            Some("failed at [redacted]".to_string())
        );
        assert!(!marker.exists());
        assert_eq!(take_failure_marker(&marker), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn waits_for_the_parent_before_opening_the_bundle() {
        let mut platform = FakePlatform {
            process_states: VecDeque::from([true, true, false]),
            ..Default::default()
        };
        let bundle = Path::new("/Applications/Switchify PC.app");

        assert_eq!(run_relaunch(42, bundle, &mut platform), Ok(()));
        assert_eq!(platform.sleeps, 2);
        assert_eq!(platform.opened, [bundle]);
    }

    #[test]
    fn reports_launch_failure() {
        let mut platform = FakePlatform {
            process_states: VecDeque::from([false]),
            open_error: Some("launch failed".into()),
            ..Default::default()
        };

        assert_eq!(
            run_relaunch(
                42,
                Path::new("/Applications/Switchify PC.app"),
                &mut platform
            ),
            Err("launch failed".into())
        );
    }

    #[test]
    fn does_not_open_the_bundle_while_the_parent_is_running() {
        let mut platform = FakePlatform::default();

        assert_eq!(
            run_relaunch(
                42,
                Path::new("/Applications/Switchify PC.app"),
                &mut platform
            ),
            Err("The previous Switchify process did not exit in time.".into())
        );
        assert_eq!(platform.sleeps, MAX_WAIT_ATTEMPTS);
        assert!(platform.opened.is_empty());
    }
}
