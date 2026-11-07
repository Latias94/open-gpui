use open_gpui::{
    Context, Div, Entity, InteractiveElement as _, ParentElement as _, Role, SharedString,
    Stateful, StatefulInteractiveElement as _, Subscription, Window, div,
};

#[test]
fn downstream_accessibility_property_builders_are_type_only() {
    let _spin_type_only = |author: String, shortcuts: SharedString| -> Stateful<Div> {
        div()
            .id("independent-font-size-element")
            .role(Role::SpinButton)
            .accessibility_id(author)
            .aria_label("Editor font size")
            .aria_keyshortcuts(shortcuts)
            .aria_numeric_value(14.5)
            .aria_min_numeric_value(6.0)
            .aria_max_numeric_value(72.0)
            .aria_numeric_value_step(0.25)
    };
    let _menu_type_only = |author: SharedString, shortcuts: String| -> Stateful<Div> {
        div()
            .id("independent-font-menu-element")
            .role(Role::Menu)
            .accessibility_id(author)
            .aria_keyshortcuts(shortcuts)
            .child(
                div()
                    .id("independent-font-menu-item-element")
                    .role(Role::MenuItem)
                    .accessibility_id("settings.font-menu.apply")
                    .aria_keyshortcuts("Control+Enter"),
            )
    };
}

struct IndependentMenu {
    selected: Option<usize>,
    _subscription: Option<Subscription>,
}

#[test]
fn downstream_accessibility_configuration_queries_and_composite_subscription_are_type_only() {
    let _: fn(&Window) -> bool = Window::is_a11y_enabled;
    let _: fn(&Window) -> bool = Window::is_a11y_active;
    let _configuration_type_only = |window: &Window| -> (bool, bool, Option<String>) {
        (
            window.is_a11y_enabled(),
            window.is_a11y_active(),
            window.debug_a11y_tree_json(),
        )
    };
    let _subscription_type_only =
        |source: &Entity<usize>, window: &mut Window, cx: &mut Context<IndependentMenu>| {
            let subscription = if window.is_a11y_enabled() {
                Some(cx.observe_in(source, window, |this, source, _, cx| {
                    this.selected = Some(*source.read(cx));
                    cx.notify();
                }))
            } else {
                None
            };
            IndependentMenu {
                selected: None,
                _subscription: subscription,
            }
        };
}
