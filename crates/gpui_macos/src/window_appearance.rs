use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark,
    NSAppearanceNameVibrantLight,
};
use objc2_foundation::NSArray;
use open_gpui::WindowAppearance;

pub(crate) fn window_appearance_from_native(appearance: Option<&NSAppearance>) -> WindowAppearance {
    let Some(appearance) = appearance else {
        return WindowAppearance::Light;
    };
    let supported = unsafe {
        [
            (NSAppearanceNameAqua, WindowAppearance::Light),
            (NSAppearanceNameDarkAqua, WindowAppearance::Dark),
            (NSAppearanceNameVibrantLight, WindowAppearance::VibrantLight),
            (NSAppearanceNameVibrantDark, WindowAppearance::VibrantDark),
        ]
    };
    let names = NSArray::from_slice(&supported.map(|(name, _)| name));
    // AppKit resolves inherited and accessibility appearances, whose names may be aliases.
    let Some(name) = appearance.bestMatchFromAppearancesWithNames(&names) else {
        return WindowAppearance::Light;
    };
    supported
        .into_iter()
        .find_map(|(supported_name, appearance)| (&*name == supported_name).then_some(appearance))
        .unwrap_or(WindowAppearance::Light)
}
