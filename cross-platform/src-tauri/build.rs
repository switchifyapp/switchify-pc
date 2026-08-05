use std::path::PathBuf;
use std::process::Command;

fn main() {
    add_command_line_tools_swift_library_path();
    tauri_build::build()
}

fn add_command_line_tools_swift_library_path() {
    let Ok(output) = Command::new("xcrun").args(["--find", "swift"]).output() else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let swift = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let Some(usr) = swift.parent().and_then(|bin| bin.parent()) else {
        return;
    };
    let libraries = usr.join("lib/swift/macosx");
    if libraries.is_dir() {
        println!("cargo:rustc-link-search=native={}", libraries.display());
    }
}
