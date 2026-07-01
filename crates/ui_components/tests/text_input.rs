mod support;

use open_gpui::{AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, div, px};
use open_gpui_ui_components::{
    Button, DEFAULT_FOCUS_RING_WIDTH, FocusRing, TextInput, TextInputDisplayMode, Textarea,
    gpui_adapter::{TextInputController, focus_ring_shadow, init_text_input},
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, semantic, ui_px};
use std::cell::RefCell;
use std::rc::Rc;

use support::tokens::custom_tokens;

#[test]
fn default_text_input_state_uses_text_input_role_and_placeholder_display() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.input_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.input_px());
    assert!(!state.has_value());
    assert_eq!(state.display_text().as_ref(), "Email address");
    assert!(state.displaying_placeholder());
    assert!(state.editable());
}

#[test]
fn filled_text_input_reports_value_state() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .value("hello@example.com")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "hello@example.com");
    assert_eq!(state.display_text().as_ref(), "hello@example.com");
    assert!(!state.displaying_placeholder());
}

#[test]
fn text_input_state_normalizes_static_values_with_single_line_policy() {
    let state = TextInput::new("query", "Search")
        .value("alpha\r\nbeta\ngamma")
        .state();

    assert_eq!(state.value(), "alpha  beta gamma");
    assert_eq!(state.display_text().as_ref(), "alpha  beta gamma");
}

#[test]
fn password_text_input_masks_display_without_hiding_value() {
    let state = TextInput::new("password", "Password")
        .placeholder("Password")
        .value("a🙂中")
        .display_mode(TextInputDisplayMode::Password)
        .state();

    assert_eq!(state.value(), "a🙂中");
    assert_eq!(state.display_mode(), TextInputDisplayMode::Password);
    assert_eq!(state.display_text().as_ref(), "•••");
    assert!(state.display_mode().masks_value());
    assert!(!state.displaying_placeholder());
}

#[test]
fn controlled_text_input_on_change_marks_input_controller_driven() {
    let state = TextInput::new("email", "Email")
        .value("hello@example.com")
        .on_change(|_, _, _| {})
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
    assert_eq!(state.value(), "hello@example.com");
}

#[test]
fn disabled_and_read_only_text_inputs_block_editability() {
    let tokens = custom_tokens();
    let disabled = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = TextInput::new("readonly", "Read only")
        .read_only(true)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(!disabled.activation_enabled());
    assert_eq!(disabled.colors().background().token(), tokens.surface_muted);
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(!read_only.activation_enabled());
    assert_eq!(
        read_only.colors().background().token(),
        ThemeTokens::default().surface_muted
    );
    assert_eq!(read_only.role(), Role::TextInput);
}

#[test]
fn invalid_text_input_uses_destructive_border_token() {
    let tokens = custom_tokens();
    let state = TextInput::new("email", "Email")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
    assert_eq!(state.colors().placeholder().token(), tokens.text_muted);
}

#[test]
fn focus_ring_preserves_token_intent_without_layout_shift() {
    let ring = FocusRing::from_color(Button::new("save", "Save").state().colors().focus_ring());
    let shadow = focus_ring_shadow(ring);

    assert_eq!(ring.color().token(), semantic::FOCUS_RING);
    assert_eq!(ring.width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!ring.changes_layout());
    assert_eq!(shadow[0].spread_radius, px(2.0));
    assert_eq!(shadow[0].blur_radius, px(0.0));
    assert!(!shadow[0].inset);
}

#[test]
fn text_input_size_helpers_apply_input_metrics() {
    let state = TextInput::new("query", "Search").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[open_gpui::test]
fn text_input_controller_converts_utf16_ranges_and_replaces_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a🙂中", cx));

    cx.update_entity(&controller, |controller, cx| {
        let mut adjusted = None;

        assert_eq!(
            controller
                .text_for_range_utf16(1..3, &mut adjusted)
                .as_deref(),
            Some("🙂")
        );
        assert_eq!(adjusted, Some(1..3));

        controller.select_range(1.."a🙂".len(), cx);
        controller.replace_text_in_range_utf16(None, "b\nc", cx);

        assert_eq!(controller.value(), "ab c中");
        assert_eq!(controller.selected_range(), 4..4);
        assert_eq!(controller.selected_range_utf16(), 4..4);
    });
}

#[open_gpui::test]
fn text_input_controller_updates_marked_text_and_commits_composition(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(TextInputController::new);

    cx.update_entity(&controller, |controller, cx| {
        controller.replace_and_mark_text_in_range_utf16(None, "ni", Some(1..2), cx);

        assert_eq!(controller.value(), "ni");
        assert_eq!(controller.marked_range_utf16(), Some(0..2));
        assert_eq!(controller.selected_range_utf16(), 1..2);

        controller.replace_text_in_range_utf16(None, "你", cx);

        assert_eq!(controller.value(), "你");
        assert_eq!(controller.marked_range_utf16(), None);
        assert_eq!(controller.selected_range_utf16(), 1..1);
    });
}

#[open_gpui::test]
fn text_input_controller_delete_commands_respect_grapheme_boundaries(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a👨‍👩‍👧‍👦b", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.move_to_offset("a👨‍👩‍👧‍👦".len(), cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "ab");

        controller.move_to_offset(1, cx);
        controller.delete_forward(cx);

        assert_eq!(controller.value(), "a");
    });
}

#[open_gpui::test]
fn text_input_controller_rejects_editing_when_disabled_or_read_only(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("locked", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.set_read_only(true, cx);
        controller.select_range(0..controller.value().len(), cx);
        controller.replace_text_in_range_utf16(None, "changed", cx);

        assert_eq!(controller.value(), "locked");

        controller.set_read_only(false, cx);
        controller.set_disabled(true, cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "locked");
        assert!(!controller.accepts_editing());
    });
}

#[open_gpui::test]
fn text_input_runtime_accepts_controller_backed_simulated_input(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controller: open_gpui::Entity<TextInputController>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                TextInput::new("runtime-text-input", "Runtime text input")
                    .controller(self.controller.clone())
                    .placeholder("Type here"),
            )
        }
    }

    cx.update(init_text_input);
    let controller = cx.new(TextInputController::new);
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        controller: controller.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-text-input:root")
        .expect("standalone text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.update_entity(&controller, |controller, _| {
        assert_eq!(controller.value(), "hello world");
        assert_eq!(
            controller.selected_range(),
            controller.value().len()..controller.value().len()
        );
    });
}

#[open_gpui::test]
fn controlled_text_input_on_change_accepts_input_without_supplied_controller(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                TextInput::new("controlled-text-input", "Controlled text input")
                    .value(value)
                    .placeholder("Type here")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-text-input:root")
        .expect("controlled text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "hello world");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("hello world")
    );
}

#[open_gpui::test]
fn text_input_state_marks_controller_driven_inputs(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("editable", "Editable")
        .controller(controller)
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
}

#[open_gpui::test]
fn controller_driven_text_input_state_marks_disabled_editing(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("disabled", "Disabled")
        .controller(controller)
        .disabled(true)
        .state();

    assert!(state.controller_driven());
    assert!(state.disabled());
    assert!(!state.editable());
}

#[test]
fn default_textarea_state_uses_text_input_role_and_rows() {
    let state = Textarea::new("notes", "Notes")
        .placeholder("Release notes")
        .rows(4)
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.rows(), 4);
    assert_eq!(state.metrics().rows(), 4);
    assert!(state.placeholder_visible());
    assert_eq!(state.display_text(), "Release notes");
    assert!(state.editable());
    assert!(!state.controller_driven());
}

#[test]
fn filled_textarea_preserves_newlines_in_state() {
    let state = Textarea::new("notes", "Notes")
        .value("Line 1\r\nLine 2")
        .placeholder("Release notes")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "Line 1\nLine 2");
    assert_eq!(state.display_text(), "Line 1\nLine 2");
    assert!(!state.displaying_placeholder());
}

#[test]
fn disabled_read_only_and_invalid_textareas_expose_control_state() {
    let tokens = custom_tokens();
    let disabled = Textarea::new("disabled-notes", "Disabled notes")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = Textarea::new("readonly-notes", "Read-only notes")
        .read_only(true)
        .state();
    let invalid = Textarea::new("invalid-notes", "Invalid notes")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(invalid.invalid());
    assert_eq!(invalid.colors().border().token(), tokens.destructive);
}

#[open_gpui::test]
fn controlled_textarea_on_change_preserves_newline_input(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Textarea::new("controlled-textarea", "Controlled textarea")
                    .value(value)
                    .placeholder("Type notes")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("textarea:controlled-textarea:root")
        .expect("controlled textarea should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("Line 1\nLine 2");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "Line 1\nLine 2");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("Line 1\nLine 2")
    );
}
