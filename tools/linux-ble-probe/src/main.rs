#[cfg(target_os = "linux")]
#[path = "../../../src-tauri/src/ble_wire.rs"]
mod ble_wire;
#[cfg(target_os = "linux")]
mod probe;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] {
        println!("Linux Bluetooth transport probe (no pairing or input).\nUsage: switchify-linux-ble-probe --serve hci0\nRequires an already powered adapter. Stops after five minutes or Ctrl-C.");
        return std::process::ExitCode::SUCCESS;
    }
    #[cfg(target_os = "linux")]
    if args.len() == 2 && args[0] == "--serve" && valid_adapter(&args[1]) {
        return match probe::run(&args[1]) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    eprintln!("Use --help. Serving is available only on Linux with --serve hciN.");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "linux")]
fn valid_adapter(name: &str) -> bool {
    name.strip_prefix("hci").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn adapter_argument_is_an_explicit_adapter_not_a_path() {
        for name in ["hci0", "hci12"] {
            assert!(super::valid_adapter(name));
        }
        for name in ["", "hci", "hci0/../hci1", "hci-1", "all"] {
            assert!(!super::valid_adapter(name));
        }
    }
}
