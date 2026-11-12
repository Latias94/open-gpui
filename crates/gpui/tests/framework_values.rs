use open_gpui::{ClickEvent, Hsla, ImageFormat, TouchClickEvent, hsla, opaque_grey, point, px};

const ACCENT: Hsla = hsla(0.4, 0.5, 0.6, 0.7);
const GREY: Hsla = opaque_grey(0.3, 0.8);
const JPEG_EXTENSION: &str = ImageFormat::Jpeg.extension();

#[cfg(feature = "test-support")]
#[test]
fn test_support_exposes_the_exact_scene_for_painted_output_inspection() {
    let _: fn(&open_gpui::Window) -> &open_gpui::Scene = open_gpui::Window::rendered_scene;
}

#[test]
fn colors_and_image_extensions_can_initialize_static_values() {
    assert_eq!(ACCENT, hsla(0.4, 0.5, 0.6, 0.7));
    assert_eq!(GREY, opaque_grey(0.3, 0.8));
    assert_eq!(JPEG_EXTENSION, "jpg");
}

#[test]
fn touch_clicks_preserve_primary_and_secondary_activation_semantics() {
    let position = point(px(12.), px(24.));
    for long_press in [false, true] {
        let event = ClickEvent::Touch(TouchClickEvent {
            position,
            tap_count: 2,
            long_press,
        });
        assert_eq!(event.position(), position);
        assert_eq!(event.mouse_position(), None);
        assert_eq!(event.click_count(), 2);
        assert_eq!(event.is_secondary(), long_press);
        assert_eq!(event.standard_click(), !long_press);
        assert!(!event.is_keyboard());
        assert!(!event.is_right_click());
        assert!(!event.first_focus());
        assert_eq!(event.modifiers(), Default::default());
    }
}
