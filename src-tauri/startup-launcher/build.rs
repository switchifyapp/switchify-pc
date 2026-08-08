fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        tauri_winres::WindowsResource::new()
            .set_manifest(include_str!("startup.manifest"))
            .compile()
            .expect("failed to embed startup launcher manifest");
    }
}
