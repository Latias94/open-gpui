#[path = "support/a11y.rs"]
mod a11y_support;
mod support;

use open_gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, accesskit, div};
use open_gpui_ui_components::{
    Avatar, AvatarGroup, AvatarGroupCount, Badge, BadgeVariant, Button, ButtonVariant, Checkbox,
    ColorState, DEFAULT_FOCUS_RING_WIDTH, IconButton, Kbd, Label, Link, Progress,
    ProgressVisualMode, Separator, Skeleton, Switch, Toggle, ToggleVariant, Tooltip,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, Toggled, semantic, ui_px};
use std::cell::RefCell;
use std::rc::Rc;

use support::tokens::custom_tokens;

#[test]
fn default_button_state_uses_button_role_and_medium_metrics() {
    let state = Button::new("save", "Save").state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Default);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.button_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.button_px());
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert_eq!(state.focus_ring().width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn destructive_button_uses_destructive_token_intent() {
    let state = Button::new("delete", "Delete")
        .variant(ButtonVariant::Destructive)
        .state();

    assert_eq!(state.colors().background().token(), semantic::DESTRUCTIVE);
    assert_eq!(
        state.colors().foreground().token(),
        semantic::DESTRUCTIVE_FOREGROUND
    );
}

#[test]
fn disabled_button_blocks_activation_metadata() {
    let state = Button::new("disabled", "Disabled").disabled(true).state();

    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn button_size_helpers_apply_foundation_size_metrics() {
    let state = Button::new("large", "Large").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[test]
fn toggle_pressed_state_maps_to_button_role_and_toggled_state() {
    let state = Toggle::new("notifications", "Notifications")
        .variant(ToggleVariant::Outline)
        .pressed(true)
        .small()
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::True);
    assert!(state.pressed());
    assert_eq!(state.variant(), ToggleVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn disabled_toggle_blocks_activation_without_checkbox_semantics() {
    let state = Toggle::new("locked", "Locked")
        .pressed(false)
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::False);
    assert!(!state.pressed());
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn badge_variants_resolve_display_only_token_intents() {
    let default = Badge::new("status", "Live").state();
    let secondary = Badge::new("beta", "Beta")
        .variant(BadgeVariant::Secondary)
        .small()
        .state();
    let destructive = Badge::new("risk", "Risk")
        .variant(BadgeVariant::Destructive)
        .state();
    let outline = Badge::new("neutral", "Neutral")
        .variant(BadgeVariant::Outline)
        .state();

    assert_eq!(default.variant(), BadgeVariant::Default);
    assert!(default.display_only());
    assert_eq!(default.role(), None);
    assert_eq!(default.colors().background().token(), semantic::ACCENT);
    assert_eq!(secondary.size(), Size::Small);
    assert_eq!(
        secondary.colors().background().token(),
        semantic::SURFACE_MUTED
    );
    assert_eq!(
        destructive.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(outline.colors().border().token(), semantic::BORDER);
}

#[test]
fn icon_button_requires_accessible_label_and_reuses_button_primitives() {
    let button = IconButton::new("search", "?", "Search")
        .variant(ButtonVariant::Outline)
        .small();
    let state = button.state();

    assert_eq!(button.accessible_label(), "Search");
    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.metrics().size(), Size::Small.icon_button_size());
    assert_eq!(state.metrics().icon_size(), Size::Small.icon_size());
    assert_eq!(state.colors().border().token(), semantic::BORDER);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn selected_icon_button_reuses_selected_button_colors() {
    let state = IconButton::new("active-search", "?", "Search")
        .variant(ButtonVariant::Ghost)
        .selected(true)
        .state();

    assert!(state.selected());
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.colors().background().state(), ColorState::Selected);
}

#[test]
fn buttons_accept_tooltip_builders() {
    let _button = Button::new("save", "Save").tooltip(Tooltip::text("Save changes"));
    let _icon_button =
        IconButton::new("search", "?", "Search").tooltip(Tooltip::text("Search workspace"));
}

#[test]
fn disabled_icon_button_blocks_activation_metadata() {
    let state = IconButton::new("locked", "x", "Locked")
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn avatar_fallback_initials_derive_from_display_names_and_empty_names() {
    let ada = Avatar::new("ada", "Ada Lovelace").state();
    let single = Avatar::new("single", "Grace").state();
    let trio = Avatar::new("trio", "Foo Bar Dar").state();
    let empty = Avatar::new("empty", "  ").state();

    assert_eq!(ada.name(), "Ada Lovelace");
    assert_eq!(ada.fallback(), "AL");
    assert_eq!(ada.accessible_label(), "Ada Lovelace");
    assert_eq!(ada.role(), Role::Image);

    assert_eq!(single.fallback(), "GR");
    assert_eq!(trio.fallback(), "FB");
    assert_eq!(empty.fallback(), "?");
    assert_eq!(empty.accessible_label(), "Avatar");
}

#[test]
fn avatar_explicit_fallback_overrides_derived_initials() {
    let state = Avatar::new("current-user", "Ada Lovelace")
        .fallback("ME")
        .state();

    assert_eq!(state.name(), "Ada Lovelace");
    assert_eq!(state.fallback(), "ME");
}

#[test]
fn avatar_source_metadata_does_not_own_loading_state() {
    let state = Avatar::new("profile", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .state();

    assert!(state.has_source());
    assert_eq!(
        state.source().map(|source| source.uri()),
        Some("asset://avatars/ada.png")
    );
    assert_eq!(state.fallback(), "AL");
    assert_eq!(state.accessible_label(), "Ada Lovelace");
}

#[test]
fn avatar_accessible_label_can_be_explicit_for_source_and_fallback_avatars() {
    let fallback = Avatar::new("fallback-avatar", "Ada Lovelace")
        .accessible_label("Current user")
        .state();
    let source = Avatar::new("source-avatar", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .accessible_label("Ada profile photo")
        .state();

    assert_eq!(fallback.accessible_label(), "Current user");
    assert_eq!(source.accessible_label(), "Ada profile photo");
}

#[test]
fn avatar_size_metrics_and_token_intents_are_stable() {
    let tokens = custom_tokens();
    let small = Avatar::new("small-avatar", "Ada")
        .small()
        .tokens(tokens)
        .state();
    let medium = Avatar::new("medium-avatar", "Ada").tokens(tokens).state();
    let large = Avatar::new("large-avatar", "Ada")
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(small.size(), Size::Small);
    assert_eq!(small.metrics().diameter(), ui_px(28.0));
    assert_eq!(small.metrics().text_size(), ui_px(11.0));

    assert_eq!(medium.metrics().diameter(), ui_px(32.0));
    assert_eq!(medium.metrics().radius(), ui_px(16.0));

    assert_eq!(large.metrics().diameter(), ui_px(40.0));
    assert_eq!(large.metrics().text_size(), ui_px(14.0));
    assert_eq!(large.colors().background().token(), tokens.surface_muted);
    assert_eq!(large.colors().foreground().token(), tokens.text);
    assert_eq!(large.colors().border().token(), tokens.border);
}

#[test]
fn avatar_group_state_tracks_visible_and_hidden_counts() {
    let group = AvatarGroup::new("team")
        .avatar(Avatar::new("ada", "Ada Lovelace"))
        .avatar(Avatar::new("grace", "Grace Hopper"))
        .avatar(Avatar::new("katherine", "Katherine Johnson"))
        .avatar(Avatar::new("margaret", "Margaret Hamilton"))
        .max_visible(3)
        .tokens(custom_tokens());
    let state = group.state();

    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.total_count(), 4);
    assert_eq!(state.visible_count(), 3);
    assert_eq!(state.hidden_count(), 1);

    let count = AvatarGroupCount::new("team-count", state.hidden_count())
        .with_size(state.size())
        .tokens(custom_tokens());
    let count_state = count.state();

    assert_eq!(count_state.count(), 1);
    assert_eq!(count_state.size(), Size::Medium);
    assert_eq!(count_state.role(), Role::Label);
}

#[open_gpui::test]
fn avatar_renders_stable_debug_selector(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Avatar::new("runtime-avatar", "Ada Lovelace"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(cx.debug_bounds("avatar:runtime-avatar:root").is_some());
}

#[test]
fn separator_state_exposes_orientation_role_and_decorative_mode() {
    let horizontal = Separator::new("section-separator").state();
    let vertical = Separator::new("panel-separator").vertical().large().state();
    let decorative = Separator::new("decorative-separator")
        .decorative(true)
        .state();

    assert_eq!(horizontal.orientation(), Orientation::Horizontal);
    assert_eq!(horizontal.role(), Some(Role::Separator));
    assert_eq!(horizontal.metrics().thickness(), ui_px(1.0));
    assert_eq!(horizontal.colors().line().token(), semantic::BORDER);

    assert_eq!(vertical.orientation(), Orientation::Vertical);
    assert_eq!(vertical.role(), Some(Role::Separator));
    assert_eq!(vertical.metrics().thickness(), ui_px(2.0));

    assert!(decorative.decorative());
    assert_eq!(decorative.role(), None);
}

#[open_gpui::test]
fn separator_final_tree_downgrades_to_group_preserves_orientation_and_omits_decorative_semantics(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Separator::new("semantic-horizontal-separator"))
                .child(Separator::new("semantic-vertical-separator").vertical())
                .child(Separator::new("decorative-separator").decorative(true))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("separator accessibility tree should publish");
    let separators = update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == accesskit::Role::Group && node.orientation().is_some())
        .map(|(_, node)| node)
        .collect::<Vec<_>>();

    assert_eq!(
        separators.len(),
        2,
        "the decorative separator must not publish separator semantics"
    );
    assert!(
        separators
            .iter()
            .any(|node| node.orientation() == Some(accesskit::Orientation::Horizontal))
    );
    assert!(
        separators
            .iter()
            .any(|node| node.orientation() == Some(accesskit::Orientation::Vertical))
    );
    for separator in separators {
        assert_eq!(separator.numeric_value(), None);
        assert_eq!(separator.min_numeric_value(), None);
        assert_eq!(separator.max_numeric_value(), None);
        a11y_support::assert_exact_actions(separator, &[]);
    }
}

#[open_gpui::test]
fn disabled_link_final_tree_suppresses_actions_and_dispatch_is_a_no_op(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        activations: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let enabled_activations = self.activations.clone();
            let disabled_activations = self.activations.clone();

            div()
                .size_full()
                .child(Link::new("enabled-link", "Open docs", "/docs").on_activate(
                    move |_, _, _, _| enabled_activations.borrow_mut().push("enabled"),
                ))
                .child(
                    Link::new("disabled-link", "Unavailable docs", "/unavailable")
                        .disabled(true)
                        .on_activate(move |_, _, _, _| {
                            disabled_activations.borrow_mut().push("disabled");
                        }),
                )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
    });
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("link accessibility tree should publish");
    let (enabled_id, enabled) = a11y_support::node_with_label(&update, "Open docs");
    let (disabled_id, disabled) = a11y_support::node_with_label(&update, "Unavailable docs");

    assert_eq!(enabled.role(), accesskit::Role::Link);
    assert!(enabled.supports_action(accesskit::Action::Click));
    assert!(enabled.supports_action(accesskit::Action::Focus));
    assert!(!enabled.is_disabled());
    assert!(disabled.is_disabled());
    assert!(!disabled.supports_action(accesskit::Action::Click));
    assert!(!disabled.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: enabled_id,
        data: None,
    }));
    assert_eq!(activations.borrow().as_slice(), &["enabled"]);

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: disabled_id,
        data: None,
    }));
    assert_eq!(
        activations.borrow().as_slice(),
        &["enabled"],
        "disabled accessibility dispatch must not invoke the link handler"
    );
}

#[test]
fn kbd_state_is_display_only_with_muted_token_intents() {
    let tokens = custom_tokens();
    let state = Kbd::new("command-shortcut", "Ctrl+K")
        .small()
        .tokens(tokens)
        .state();

    assert_eq!(state.label(), "Ctrl+K");
    assert_eq!(state.size(), Size::Small);
    assert!(state.display_only());
    assert_eq!(state.metrics().min_width(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
    assert_eq!(state.colors().foreground().token(), tokens.text_muted);
    assert_eq!(state.colors().border().token(), tokens.border);
}

#[test]
fn progress_state_clamps_values_and_preserves_indeterminate_mode() {
    let full = Progress::new("upload-progress", "Upload")
        .value(142.0)
        .large()
        .state();
    let empty = Progress::new("empty-progress", "Empty")
        .value(f32::NAN)
        .state();
    let indeterminate = Progress::new("pending-progress", "Pending")
        .indeterminate()
        .state();

    assert_eq!(full.role(), Role::ProgressIndicator);
    assert_eq!(full.value_percent(), Some(100.0));
    assert_eq!(full.normalized_value(), Some(1.0));
    assert_eq!(
        full.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 1.0
        }
    );
    assert_eq!(full.indicator_start_fraction(), 0.0);
    assert_eq!(full.indicator_fraction(), 1.0);
    assert_eq!(full.metrics().height(), ui_px(10.0));
    assert_eq!(full.colors().track().token(), semantic::SURFACE_MUTED);
    assert_eq!(full.colors().indicator().token(), semantic::ACCENT);

    assert_eq!(empty.value_percent(), Some(0.0));
    assert_eq!(empty.normalized_value(), Some(0.0));
    assert_eq!(
        empty.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 0.0
        }
    );
    assert!(indeterminate.indeterminate());
    assert_eq!(indeterminate.value_percent(), None);
    assert_eq!(indeterminate.normalized_value(), None);
    assert_eq!(
        indeterminate.visual_mode(),
        ProgressVisualMode::Indeterminate
    );
    assert!(
        indeterminate.indicator_start_fraction() > 0.0,
        "indeterminate progress should not look like a left-anchored determinate fill"
    );
    assert!(
        indeterminate.indicator_fraction() > 0.0 && indeterminate.indicator_fraction() < 0.5,
        "indeterminate progress should render as a short segment, not as a fixed percentage value"
    );
}

#[test]
fn skeleton_state_is_noninteractive_placeholder_with_stable_metrics() {
    let tokens = custom_tokens();
    let state = Skeleton::new("loading-line")
        .subtle(true)
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(state.size(), Size::Large);
    assert!(state.subtle());
    assert!(state.display_only());
    assert_eq!(state.metrics().width(), ui_px(224.0));
    assert_eq!(state.metrics().height(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
}

#[open_gpui::test]
fn low_state_primitives_render_stable_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(Separator::new("runtime-separator"))
                .child(Kbd::new("runtime-kbd", "Ctrl+K"))
                .child(Progress::new("runtime-progress", "Loading").value(40.0))
                .child(Progress::new("runtime-progress-indeterminate", "Indexing").indeterminate())
                .child(Skeleton::new("runtime-skeleton"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for selector in [
        "separator:runtime-separator:root",
        "kbd:runtime-kbd:root",
        "progress:runtime-progress:root",
        "progress:runtime-progress:indicator",
        "progress:runtime-progress-indeterminate:root",
        "progress:runtime-progress-indeterminate:indicator",
        "skeleton:runtime-skeleton:root",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} should be rendered"
        );
    }

    let determinate_root = cx
        .debug_bounds("progress:runtime-progress:root")
        .expect("determinate progress root should render");
    let determinate_indicator = cx
        .debug_bounds("progress:runtime-progress:indicator")
        .expect("determinate progress indicator should render");
    let indeterminate_root = cx
        .debug_bounds("progress:runtime-progress-indeterminate:root")
        .expect("indeterminate progress root should render");
    let indeterminate_indicator = cx
        .debug_bounds("progress:runtime-progress-indeterminate:indicator")
        .expect("indeterminate progress indicator should render");

    let determinate_width =
        determinate_indicator.size.width.as_f32() / determinate_root.size.width.as_f32();
    let indeterminate_start = (indeterminate_indicator.left().as_f32()
        - indeterminate_root.left().as_f32())
        / indeterminate_root.size.width.as_f32();
    let indeterminate_width =
        indeterminate_indicator.size.width.as_f32() / indeterminate_root.size.width.as_f32();

    assert!(
        (determinate_width - 0.4).abs() < 0.02,
        "determinate progress indicator should match the provided value"
    );
    assert!(
        indeterminate_start > 0.25,
        "indeterminate progress indicator should not be left-anchored"
    );
    assert!(
        indeterminate_width > 0.25 && indeterminate_width < 0.45,
        "indeterminate progress indicator should be a short segment"
    );
}

#[test]
fn switch_label_uses_theme_text_token() {
    let tokens = custom_tokens();
    let state = Switch::new("feature").tokens(tokens).state();

    assert_eq!(state.colors().label().token(), tokens.text);
}

#[test]
fn checked_switch_maps_to_true_toggled_state() {
    let state = Switch::new("feature").checked(true).state();

    assert!(state.checked());
    assert_eq!(state.role(), Role::Switch);
    assert_eq!(state.toggled(), Toggled::True);
    assert_eq!(state.colors().track().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn unchecked_switch_maps_to_false_toggled_state() {
    let state = Switch::new("feature").state();

    assert!(!state.checked());
    assert_eq!(state.toggled(), Toggled::False);
    assert_eq!(state.colors().track().token(), semantic::SURFACE_MUTED);
}

#[test]
fn disabled_switch_keeps_role_but_blocks_activation_metadata() {
    let state = Switch::new("feature").disabled(true).state();

    assert_eq!(state.role(), Role::Switch);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn switch_size_metrics_are_deterministic() {
    let state = Switch::new("feature").small().state();
    let metrics = state.metrics();

    assert_eq!(state.size(), Size::Small);
    assert_eq!(metrics.track_width(), ui_px(32.0));
    assert_eq!(metrics.track_height(), ui_px(18.0));
    assert_eq!(metrics.thumb_size(), ui_px(14.0));
    assert_eq!(metrics.checked_thumb_x(), ui_px(16.0));
}

#[open_gpui::test]
fn switch_runtime_click_emits_on_change_with_next_checked(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        checked: Rc<RefCell<bool>>,
        changes: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let checked = *self.checked.borrow();
            let next_checked = self.checked.clone();
            let changes = self.changes.clone();
            let disabled_changes = self.changes.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Switch::new("runtime-switch")
                        .label("Runtime switch")
                        .checked(checked)
                        .on_change(move |checked, _, _| {
                            *next_checked.borrow_mut() = checked;
                            changes.borrow_mut().push(checked);
                        }),
                )
                .child(
                    Switch::new("disabled-runtime-switch")
                        .label("Disabled runtime switch")
                        .disabled(true)
                        .on_change(move |checked, _, _| {
                            disabled_changes.borrow_mut().push(checked);
                        }),
                )
        }
    }

    let checked = Rc::new(RefCell::new(false));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        checked: checked.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let disabled_switch = cx
        .debug_bounds("switch:disabled-runtime-switch:root")
        .expect("disabled switch should expose a stable debug selector");
    cx.simulate_click(disabled_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        changes.borrow().is_empty(),
        "disabled switch click should not emit on_change"
    );

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should expose a stable debug selector");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), true);
    assert_eq!(changes.borrow().as_slice(), &[true]);

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should remain rendered after controlled update");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), false);
    assert_eq!(changes.borrow().as_slice(), &[true, false]);
}

#[test]
fn checkbox_states_map_to_checkbox_role_and_toggled_values() {
    let unchecked = Checkbox::new("unchecked").state();
    let checked = Checkbox::new("checked").checked(true).state();
    let mixed = Checkbox::new("mixed").indeterminate(true).state();

    assert_eq!(unchecked.role(), Role::CheckBox);
    assert_eq!(unchecked.toggled(), Toggled::False);
    assert!(!unchecked.checked());
    assert!(!unchecked.indeterminate());

    assert_eq!(checked.role(), Role::CheckBox);
    assert_eq!(checked.toggled(), Toggled::True);
    assert!(checked.checked());
    assert!(!checked.indeterminate());

    assert_eq!(mixed.role(), Role::CheckBox);
    assert_eq!(mixed.toggled(), Toggled::Mixed);
    assert!(!mixed.checked());
    assert!(mixed.indeterminate());
}

#[test]
fn disabled_checkbox_blocks_activation_metadata() {
    let state = Checkbox::new("disabled").disabled(true).state();

    assert_eq!(state.role(), Role::CheckBox);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
    assert!(!state.tab_stop_enabled());
    assert_eq!(state.colors().background().state(), ColorState::Disabled);
}

#[test]
fn invalid_and_required_checkbox_expose_state_and_token_intents() {
    let tokens = custom_tokens();
    let state = Checkbox::new("terms")
        .checked(true)
        .required(true)
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.required());
    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().border().state(), ColorState::Invalid);
    assert_eq!(state.colors().background().token(), tokens.accent);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
}

#[test]
fn checkbox_checked_state_builder_accepts_mixed() {
    let state = Checkbox::new("bulk").checked_state(Toggled::Mixed).state();

    assert_eq!(state.toggled(), Toggled::Mixed);
    assert!(state.indeterminate());
    assert!(!state.checked());
}

#[test]
fn label_state_derives_visible_text_and_required_marker() {
    let tokens = custom_tokens();
    let state = Label::new("email-label", "Email")
        .required(true)
        .tokens(tokens)
        .state();

    assert_eq!(state.role(), Role::Label);
    assert_eq!(state.text(), "Email");
    assert!(state.required());
    assert_eq!(state.colors().text().token(), tokens.text);
    assert_eq!(state.colors().required_marker().token(), tokens.destructive);
}

#[test]
fn disabled_label_uses_muted_text_intent() {
    let tokens = custom_tokens();
    let state = Label::new("disabled-label", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().text().token(), tokens.text_muted);
    assert_eq!(state.colors().text().state(), ColorState::Disabled);
}
