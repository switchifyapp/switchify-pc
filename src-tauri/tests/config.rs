#[test]
fn updater_configuration_deserializes() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
    let updater = config
        .get("plugins")
        .and_then(|plugins| plugins.get("updater"))
        .cloned()
        .expect("updater configuration");

    serde_json::from_value::<tauri_plugin_updater::Config>(updater)
        .expect("valid updater configuration");
}

#[test]
fn application_configuration_uses_the_promoted_identity() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
    assert_eq!(config["productName"], "Switchify PC");
    assert_eq!(config["identifier"], "com.enaboapps.switchify.pc");
    assert_eq!(config["version"], "1.0.0-rc.1");
    assert_eq!(config["app"]["windows"][0]["title"], "Switchify PC");
    assert_eq!(
        config["plugins"]["updater"]["endpoints"],
        serde_json::json!([])
    );

    let macos: serde_json::Value = serde_json::from_str(include_str!("../tauri.macos.conf.json"))
        .expect("valid macOS Tauri config");
    assert_eq!(
        macos["bundle"]["macOS"]["signingIdentity"],
        "Switchify PC Development"
    );
    assert_eq!(config["bundle"]["macOS"]["hardenedRuntime"], true);
}

fn capability(path: &str) -> serde_json::Value {
    serde_json::from_str(path).expect("valid capability configuration")
}

fn permission_names(capability: &serde_json::Value) -> Vec<&str> {
    capability["permissions"]
        .as_array()
        .expect("capability permissions")
        .iter()
        .map(|permission| permission.as_str().expect("permission name"))
        .collect()
}

fn command_names(source: &str, start: &str, end: &str) -> std::collections::BTreeSet<String> {
    source
        .split_once(start)
        .expect("command list start")
        .1
        .split_once(end)
        .expect("command list end")
        .0
        .split(',')
        .map(|command| command.trim().trim_matches('"').to_owned())
        .filter(|command| !command.is_empty())
        .collect()
}

#[test]
fn every_invoke_handler_command_is_registered_in_the_app_manifest() {
    let manifest_commands = command_names(include_str!("../build.rs"), ".commands(&[", "]);");
    let handler_commands = command_names(
        include_str!("../src/lib.rs"),
        "tauri::generate_handler![",
        "])",
    );
    assert_eq!(manifest_commands, handler_commands);
}

#[test]
fn main_window_has_application_commands_and_core_defaults() {
    let main = capability(include_str!("../capabilities/main.json"));
    assert_eq!(main["windows"], serde_json::json!(["main"]));

    let permissions = permission_names(&main);
    assert!(permissions.contains(&"core:default"));
    for permission in [
        "allow-get-app-state",
        "allow-approve-pairing",
        "allow-save-settings",
        "allow-list-switch-profiles",
        "allow-check-for-updates",
        "allow-export-diagnostics",
    ] {
        assert!(permissions.contains(&permission), "missing {permission}");
    }
}

#[test]
fn modifier_overlay_has_only_its_minimal_event_and_command_permissions() {
    let overlay = capability(include_str!("../capabilities/modifier-overlay.json"));
    assert_eq!(overlay["windows"], serde_json::json!(["modifier-overlay"]));
    assert_eq!(
        permission_names(&overlay),
        [
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "allow-modifier-overlay-ready",
            "allow-modifier-overlay-present",
        ]
    );
}

#[test]
fn modifier_overlay_cannot_invoke_sensitive_application_commands() {
    let overlay = capability(include_str!("../capabilities/modifier-overlay.json"));
    let permissions = permission_names(&overlay);
    for denied in [
        "allow-save-settings",
        "allow-approve-pairing",
        "allow-reject-pairing",
        "allow-check-for-updates",
        "allow-download-update",
        "allow-install-update",
        "allow-export-diagnostics",
        "allow-list-switch-profiles",
        "allow-save-switch-profile",
        "allow-delete-switch-profile",
    ] {
        assert!(!permissions.contains(&denied), "overlay grants {denied}");
    }
}

#[test]
fn production_csp_allows_only_local_assets_and_tauri_ipc() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
    let security = &config["app"]["security"];
    assert_eq!(security["devCsp"], "");
    assert_ne!(security["devCsp"], security["csp"]);
    assert_eq!(
        security["csp"],
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; frame-src 'none'; form-action 'none'"
    );
}
