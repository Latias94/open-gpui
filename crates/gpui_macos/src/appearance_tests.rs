//! Conversion controls use thread-safe `NSAppearance` objects without mutating application state.
//! The application/window trajectory lives in the `native_window_appearance` example, whose
//! real entry point runs on the process main thread.

use crate::window_appearance::window_appearance_from_native;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAccessibilityHighContrastAqua,
    NSAppearanceNameAccessibilityHighContrastDarkAqua,
    NSAppearanceNameAccessibilityHighContrastVibrantDark,
    NSAppearanceNameAccessibilityHighContrastVibrantLight, NSAppearanceNameAqua,
    NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark, NSAppearanceNameVibrantLight,
};
use open_gpui::WindowAppearance;

#[test]
fn native_conversion_preserves_named_appearances_and_official_fallbacks() {
    objc2::rc::autoreleasepool(|_| {
        assert_eq!(window_appearance_from_native(None), WindowAppearance::Light);
        let cases = unsafe {
            [
                (NSAppearanceNameAqua, WindowAppearance::Light),
                (NSAppearanceNameDarkAqua, WindowAppearance::Dark),
                (NSAppearanceNameVibrantLight, WindowAppearance::VibrantLight),
                (NSAppearanceNameVibrantDark, WindowAppearance::VibrantDark),
                (
                    NSAppearanceNameAccessibilityHighContrastAqua,
                    WindowAppearance::Light,
                ),
                (
                    NSAppearanceNameAccessibilityHighContrastDarkAqua,
                    WindowAppearance::Light,
                ),
                (
                    NSAppearanceNameAccessibilityHighContrastVibrantLight,
                    WindowAppearance::Light,
                ),
                (
                    NSAppearanceNameAccessibilityHighContrastVibrantDark,
                    WindowAppearance::Light,
                ),
            ]
        };
        for (name, expected) in cases {
            let appearance = NSAppearance::appearanceNamed(name)
                .expect("macOS must provide this built-in appearance");
            assert_eq!(
                window_appearance_from_native(Some(&appearance)),
                expected,
                "{name:?}"
            );
        }
    });
}
