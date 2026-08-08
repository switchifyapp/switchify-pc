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
    assert_eq!(config["version"], "0.1.0");
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
}
