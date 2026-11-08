#![cfg(not(target_family = "wasm"))]

#[test]
fn downstream_home_directory_preserves_the_platform_value_with_test_support() {
    let expected = dirs::home_dir().expect("the native platform provides a home directory");
    let first = open_gpui_util::paths::home_dir();
    assert_eq!(first, &expected);
    assert!(std::ptr::eq(first, open_gpui_util::paths::home_dir()));
}
