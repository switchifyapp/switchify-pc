use std::path::PathBuf;
use std::process::Command;

fn main() {
    add_command_line_tools_swift_library_path();
    let mut attributes = tauri_build::Attributes::new();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = if std::env::var("PROFILE").as_deref() == Ok("release") {
            include_str!("windows/uiaccess.manifest")
        } else {
            include_str!("windows/default.manifest")
        };
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(manifest));
    }
    tauri_build::try_build(attributes).expect("failed to run Tauri build script");
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
