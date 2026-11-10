#![cfg(any(not(debug_assertions), feature = "debug-embed"))]

open_gpui_util::fs_embed! {
    struct Assets,
    crate_relative = "src",
    root_relative = "this-checkout-path-must-not-be-read",
    include = ["*.rs"],
    exclude = ["redact.rs"],
}

#[test]
fn embedded_assets_use_the_public_macro_and_trait_without_runtime_files() {
    let names: Vec<_> = Assets::iter().collect();
    let trait_names: Vec<_> = <Assets as open_gpui_util::__rust_embed::RustEmbed>::iter().collect();
    assert_eq!(names, trait_names);
    assert!(names.iter().any(|name| name == "markdown.rs"));
    assert!(!names.iter().any(|name| name == "redact.rs"));
    assert!(Assets::get("redact.rs").is_none());

    let markdown = Assets::get("markdown.rs").expect("the selected source is embedded");
    assert_eq!(markdown.data.as_ref(), include_bytes!("../src/markdown.rs"));
    assert_eq!(
        markdown.data.as_ref(),
        open_gpui_util::asset_str::<Assets>("markdown.rs").as_bytes()
    );
}
