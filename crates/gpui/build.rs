#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    println!("cargo::rustc-check-cfg=cfg(gles)");
    println!("cargo::rustc-check-cfg=cfg(gpui_leak_detection)");
    println!("cargo::rerun-if-env-changed=GPUI_LEAK_DETECTION");
    let requested_by_environment =
        std::env::var_os("GPUI_LEAK_DETECTION").is_some_and(|value| !value.is_empty());
    if requested_by_environment || std::env::var_os("CARGO_FEATURE_LEAK_DETECTION").is_some() {
        println!("cargo::rustc-cfg=gpui_leak_detection");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    println!("cargo::rustc-check-cfg=cfg(gpui_hot_reload)");
    if std::env::var_os("CARGO_FEATURE_HOT_RELOAD").is_some()
        && std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some()
        && matches!(target_os.as_str(), "macos" | "linux" | "windows" | "ios")
    {
        println!("cargo::rustc-cfg=gpui_hot_reload");
    }

    if target_os == "windows" {
        #[cfg(feature = "windows-manifest")]
        embed_resource();
    }
}

#[cfg(feature = "windows-manifest")]
fn embed_resource() {
    let resource_dir = std::path::Path::new("resources/windows");
    let manifest = resource_dir.join("gpui.manifest.xml");
    let rc_file = resource_dir.join("gpui.rc");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", rc_file.display());
    embed_resource::compile(rc_file, embed_resource::ParamsIncludeDirs([resource_dir]))
        .manifest_required()
        .unwrap();
}
