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
