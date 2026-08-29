use std::path::PathBuf;
use std::process::Command;

fn main() {
    add_command_line_tools_swift_library_path();
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "get_app_state",
        "check_accessibility",
        "approve_pairing",
        "reject_pairing",
        "disconnect_all",
        "modifier_overlay_ready",
        "modifier_overlay_present",
        "forget_device",
        "save_settings",
        "set_telemetry_consent",
        "mark_setup_shown",
        "complete_setup",
        "list_switch_profiles",
        "save_switch_profile",
        "delete_switch_profile",
        "complete_profile_exit",
        "cancel_profile_exit",
        "take_navigation_request",
        "check_for_updates",
        "download_update",
        "cancel_update_download",
        "install_update",
        "export_diagnostics",
    ]);
    let mut attributes = tauri_build::Attributes::new().app_manifest(app_manifest);
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
