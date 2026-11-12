fn main() {
    // Tauri's own build step only embeds its manifest into the app binary, but the unit
    // test executable links Tauri's dialog code too, and Windows refuses to start it
    // without the Common Controls v6 manifest (STATUS_ENTRYPOINT_NOT_FOUND). So the
    // linker embeds the same manifest (windows-app.manifest) into every executable.
    let mut attrs = tauri_build::Attributes::new();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app.manifest");
        println!("cargo:rerun-if-changed=windows-app.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
