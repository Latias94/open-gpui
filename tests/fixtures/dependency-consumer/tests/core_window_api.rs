use open_gpui::{ElementId, InteractiveElement as _, Window, div};

#[test]
fn ordinary_u64_element_ids_preserve_all_bits_and_build_interactive_elements() {
    for identifier in [0_u64, 1, u32::MAX as u64 + 1, u64::MAX] {
        assert_eq!(ElementId::from(identifier), ElementId::Integer(identifier));
        let _element = div().id(identifier);
    }
}

#[test]
fn ordinary_touch_prediction_policy_has_a_passive_public_getter() {
    let _: fn(&Window) -> bool = Window::touch_prediction_enabled;
    let _: fn(&mut Window, bool) = Window::set_touch_prediction_enabled;
}
