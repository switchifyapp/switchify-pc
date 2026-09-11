#[cfg(target_os = "linux")]
#[path = "../../../src-tauri/src/ble_wire.rs"]
mod ble_wire;
#[cfg(target_os = "linux")]
mod probe;
#[cfg(target_os = "linux")]
mod verify;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] {
        println!("Linux Bluetooth transport probe (no pairing or input).\nUsage: switchify-linux-ble-probe --serve hci0\n       switchify-linux-ble-probe --verify hci1 ADDRESS\nServe stops after five minutes. Verify requires a separate powered adapter and an already discovered probe; stops within 30 seconds plus cleanup. No radio power or pairing changes.");
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
    #[cfg(target_os = "linux")]
    if args.len() == 3 && args[0] == "--verify" && valid_adapter(&args[1]) {
        if let Ok(address) = args[2].parse::<bluer::Address>() {
            return match verify::run(&args[1], address) {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("{message}");
                    std::process::ExitCode::FAILURE
                }
            };
        }
    }
    eprintln!("Use --help. Probe operations require Linux and explicit arguments.");
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
