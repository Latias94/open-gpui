use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark,
    NSAppearanceNameVibrantLight,
};
use objc2_foundation::NSString;
use open_gpui::WindowAppearance;

pub(crate) fn window_appearance_from_native(appearance: Option<&NSAppearance>) -> WindowAppearance {
    let Some(appearance) = appearance else {
        return WindowAppearance::Light;
    };
    unsafe {
        let name: Option<Retained<NSString>> = msg_send![appearance, name];
        let Some(name) = name else {
            return WindowAppearance::Light;
        };
        if std::ptr::eq(name.as_ref(), NSAppearanceNameVibrantLight) {
            WindowAppearance::VibrantLight
        } else if std::ptr::eq(name.as_ref(), NSAppearanceNameVibrantDark) {
            WindowAppearance::VibrantDark
        } else if std::ptr::eq(name.as_ref(), NSAppearanceNameAqua) {
            WindowAppearance::Light
        } else if std::ptr::eq(name.as_ref(), NSAppearanceNameDarkAqua) {
            WindowAppearance::Dark
        } else {
            println!("unknown appearance: {name:?}");
            WindowAppearance::Light
        }
    }
}
