#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod windows_launcher {
    use std::ffi::OsStr;
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    const MAIN_EXECUTABLE: &str = "switchify-pc.exe";
    const MAX_DIAGNOSTIC_LINES: usize = 50;

    #[derive(Debug, PartialEq, Eq)]
    struct LaunchRequest {
        executable: PathBuf,
        working_directory: PathBuf,
        arguments: &'static str,
    }

    pub fn run() -> i32 {
        let launcher = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                append_diagnostic("launcher_path_unavailable", error.raw_os_error());
                return 2;
            }
        };
        let request = match launch_request(&launcher) {
            Ok(request) => request,
            Err(reason) => {
                append_diagnostic(reason, None);
                return 2;
            }
        };
        match shell_execute(&request) {
            Ok(()) => {
                append_diagnostic("startup_launcher_requested", None);
                0
            }
            Err(code) => {
                append_diagnostic("shell_launch_failed", Some(code));
                3
            }
        }
    }

    fn launch_request(launcher: &Path) -> Result<LaunchRequest, &'static str> {
        let directory = launcher.parent().ok_or("install_directory_missing")?;
        let executable = directory.join(MAIN_EXECUTABLE);
        if !executable.is_file() {
            return Err("main_executable_missing");
        }
        Ok(LaunchRequest {
            executable,
            working_directory: directory.to_path_buf(),
            arguments: "--start-hidden",
        })
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    fn shell_execute(request: &LaunchRequest) -> Result<(), i32> {
        let verb = wide(OsStr::new("open"));
        let executable = wide(request.executable.as_os_str());
        let arguments = wide(OsStr::new(request.arguments));
        let directory = wide(request.working_directory.as_os_str());
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(executable.as_ptr()),
                PCWSTR(arguments.as_ptr()),
                PCWSTR(directory.as_ptr()),
                SW_HIDE,
            )
        };
        let code = result.0 as isize as i32;
        (code > 32).then_some(()).ok_or(code)
    }

    fn append_diagnostic(event: &str, native_error: Option<i32>) {
        let Some(app_data) = std::env::var_os("APPDATA") else {
            return;
        };
        let directory = PathBuf::from(app_data).join("switchify-pc");
        let path = directory.join("startup-launcher-diagnostics.jsonl");
        let line = format!(
            "{{\"event\":\"{event}\",\"nativeErrorCode\":{}}}",
            native_error.map_or_else(|| "null".into(), |code| code.to_string())
        );
        let _ = (|| -> std::io::Result<()> {
            fs::create_dir_all(&directory)?;
            let mut lines = fs::read_to_string(&path)
                .unwrap_or_default()
                .lines()
                .filter(|existing| existing.starts_with('{') && existing.ends_with('}'))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            lines.push(line);
            if lines.len() > MAX_DIAGNOSTIC_LINES {
                lines.drain(..lines.len() - MAX_DIAGNOSTIC_LINES);
            }
            fs::write(path, format!("{}\n", lines.join("\n")))
        })();
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn launch_request_uses_sibling_main_executable() {
            let root =
                std::env::temp_dir().join(format!("switchify-launcher-{}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join(MAIN_EXECUTABLE), []).unwrap();
            let request = launch_request(&root.join("switchify-pc-startup.exe")).unwrap();
            assert_eq!(request.executable, root.join(MAIN_EXECUTABLE));
            assert_eq!(request.working_directory, root);
            assert_eq!(request.arguments, "--start-hidden");
            let _ = fs::remove_dir_all(request.working_directory);
        }

        #[test]
        fn missing_main_executable_is_rejected() {
            let root = std::env::temp_dir()
                .join(format!("switchify-launcher-missing-{}", std::process::id()));
            assert_eq!(
                launch_request(&root.join("switchify-pc-startup.exe")),
                Err("main_executable_missing")
            );
        }
    }
}

fn main() {
    #[cfg(windows)]
    std::process::exit(windows_launcher::run());
}
