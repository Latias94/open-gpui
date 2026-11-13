use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use image::{Frame, ImageBuffer, Rgba};
use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AppContext as _, Context, Entity, EntityId, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, Keystroke, Modifiers, ParentElement, Render, RenderImage, RequestFrameOptions,
    Styled, VisualTestContext, Window, accesskit, div, img, px,
};
use open_gpui_ui_components::{
    ActivationHandle, ActivationKey, ActivationRequestResult, ActivationSource, Button, TextInput,
    gpui_adapter::{TextInputController, init_text_input},
    listbox::{Listbox, ListboxGroup, ListboxOption, ListboxOptionState, ListboxRowAction},
};

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    assert!(cx.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let bounds = cx
        .debug_bounds(selector)
        .expect("composed content should render");
    cx.simulate_click(bounds.center(), Default::default());
    draw(cx);
}

fn press_key(cx: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke {
        modifiers: Modifiers::none(),
        key: key.to_owned(),
        key_char: None,
    };
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
    draw(cx);
}

fn node(
    cx: &VisualTestContext,
    label: &str,
    role: accesskit::Role,
) -> (accesskit::NodeId, accesskit::Node) {
    cx.accessibility_tree_update_history()
        .iter()
        .rev()
        .flat_map(|update| update.nodes.iter())
        .find(|(_, node)| node.label() == Some(label) && node.role() == role)
        .map(|(node_id, node)| (*node_id, node.clone()))
        .expect("owned row semantics should publish")
}

#[test]
fn content_callbacks_do_not_enter_renderer_neutral_state_resolution() {
    let calls = Rc::new(Cell::new(0));
    let observed_calls = calls.clone();
    let state = Listbox::new("descriptors", "Resources")
        .selected(Some("alpha".to_owned()))
        .option(ListboxOption::new("alpha", "Alpha"))
        .row_content(move |_, _, _| {
            observed_calls.set(observed_calls.get() + 1);
            div().child("Application content")
        })
        .state();

    assert_eq!(calls.get(), 0);
    assert_eq!(state.selected_value(), Some("alpha"));
    assert_eq!(state.options()[0].label(), "Alpha");
}

#[open_gpui::test]
fn thumbnails_keep_row_identity_semantics_controlled_selection_and_activation_routes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        renamed: bool,
        handle: ActivationHandle,
        thumbnail: Arc<RenderImage>,
        rows: Rc<RefCell<BTreeMap<String, (ListboxOptionState, EntityId)>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let thumbnail = self.thumbnail.clone();
            let rows = self.rows.clone();
            let selections = self.selections.clone();
            let beta =
                ListboxOption::new("beta", if self.renamed { "Renamed Beta" } else { "Beta" });
            let gamma = ListboxOption::new("gamma", "Gamma");
            let grouped = if self.renamed {
                vec![gamma, beta]
            } else {
                vec![beta, gamma]
            };
            Listbox::new("composed-resources", "Resources")
                .selected(Some("alpha".to_owned()))
                .option(ListboxOption::new("alpha", "Alpha"))
                .option(ListboxOption::new("disabled", "Disabled").disabled(true))
                .option(ListboxOption::separator("separator"))
                .group(ListboxGroup::new("images", "Images").options(grouped))
                .activation_handle("beta", &self.handle)
                .row_content(move |option, window, cx| {
                    let local = window.use_keyed_state("thumbnail-state", cx, |_, _| ());
                    rows.borrow_mut().insert(
                        option.value().to_owned(),
                        (option.clone(), local.entity_id()),
                    );
                    let selector = format!("thumbnail:{}", option.value());
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("thumbnail")
                                .debug_selector(move || selector.clone())
                                .size(px(24.0))
                                .child(img(thumbnail.clone()).size(px(24.0))),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(format!("Preview: {}", option.label()))
                                .child("PNG · 64 × 64"),
                        )
                })
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned())
                })
        }
    }

    let rows = Rc::new(RefCell::new(BTreeMap::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let thumbnail = Arc::new(RenderImage::new([Frame::new(ImageBuffer::from_pixel(
        2,
        2,
        Rgba([48, 112, 208, 255]),
    ))]));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        renamed: false,
        handle: handle.clone(),
        thumbnail,
        rows: rows.clone(),
        selections: selections.clone(),
    });
    assert!(cx.activate_accessibility());
    draw(cx);
    assert_eq!(
        rows.borrow().len(),
        4,
        "group headings and separators do not render option content"
    );
    assert!(rows.borrow()["alpha"].0.selected());
    assert!(rows.borrow()["disabled"].0.disabled());
    assert!(rows.borrow()["beta"].0.group_index().is_some());
    let beta_local_identity = rows.borrow()["beta"].1;
    let (beta_node, beta_semantics) = node(cx, "Beta", accesskit::Role::ListBoxOption);
    assert_eq!(beta_semantics.is_selected(), Some(false));
    assert!(beta_semantics.supports_action(accesskit::Action::Click));
    let (_, disabled_semantics) = node(cx, "Disabled", accesskit::Role::ListBoxOption);
    assert!(disabled_semantics.is_disabled());
    assert!(!disabled_semantics.supports_action(accesskit::Action::Click));

    click(cx, "thumbnail:disabled");
    assert!(selections.borrow().is_empty());
    click(cx, "thumbnail:beta");
    assert_eq!(selections.borrow().as_slice(), ["beta"]);
    assert!(rows.borrow()["alpha"].0.selected());
    assert!(!rows.borrow()["beta"].0.selected());
    assert!(cx.debug_selector_is_focused("listbox:composed-resources:option:beta"));
    press_key(cx, "enter");
    assert_eq!(selections.borrow().len(), 2);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: beta_node,
        data: None,
    }));
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        )
    });
    draw(cx);
    assert_eq!(
        selections.borrow().as_slice(),
        ["beta", "beta", "beta", "beta"]
    );

    view.update(cx, |view, cx| {
        view.renamed = true;
        cx.notify();
    });
    draw(cx);
    assert_eq!(rows.borrow()["beta"].1, beta_local_identity);
    assert_eq!(rows.borrow()["beta"].0.label(), "Renamed Beta");
    assert_eq!(
        node(cx, "Renamed Beta", accesskit::Role::ListBoxOption).0,
        beta_node
    );
    assert!(cx.debug_selector_is_focused("listbox:composed-resources:option:beta"));
}

#[open_gpui::test]
fn nested_buttons_and_editors_keep_their_actions_and_text_without_selecting_or_navigating(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        editor: Entity<TextInputController>,
        actions: Rc<RefCell<Vec<ActivationSource>>>,
        selections: Rc<RefCell<Vec<String>>>,
        rows: Rc<RefCell<BTreeMap<String, ListboxOptionState>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let editor = self.editor.clone();
            let actions = self.actions.clone();
            let selections = self.selections.clone();
            let rows = self.rows.clone();
            Listbox::new("row-actions", "Resources")
                .default_active("alpha")
                .option(ListboxOption::new("alpha", "Alpha"))
                .option(ListboxOption::new("beta", "Beta"))
                .row_content(move |option, _, _| {
                    rows.borrow_mut()
                        .insert(option.value().to_owned(), option.clone());
                    let actions = actions.clone();
                    let label_selector = format!("row-label:{}", option.value());
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("label")
                                .debug_selector(move || label_selector.clone())
                                .child(option.label().to_owned()),
                        )
                        .child(ListboxRowAction::new(
                            "inspect-action",
                            Button::new(
                                format!("inspect:{}", option.value()),
                                format!("Inspect {}", option.label()),
                            )
                            .disabled(option.disabled())
                            .on_activate(move |activation, _, _| {
                                actions.borrow_mut().push(activation.source())
                            }),
                        ))
                        .when(option.value() == "alpha", |this| {
                            this.child(ListboxRowAction::new(
                                "notes-action",
                                TextInput::new("notes", "Notes").controller(editor.clone()),
                            ))
                        })
                })
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned())
                })
        }
    }

    cx.update(init_text_input);
    let actions = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let rows = Rc::new(RefCell::new(BTreeMap::new()));
    let (view, cx) = cx.add_window_view(|_, cx| Probe {
        editor: cx.new(TextInputController::new),
        actions: actions.clone(),
        selections: selections.clone(),
        rows: rows.clone(),
    });
    assert!(cx.activate_accessibility());
    draw(cx);
    click(cx, "button:inspect:alpha:root");
    assert_eq!(actions.borrow().as_slice(), &[ActivationSource::Pointer]);
    assert!(cx.debug_selector_is_focused("button:inspect:alpha:root"));
    press_key(cx, "enter");
    press_key(cx, "space");
    press_key(cx, "down");
    assert_eq!(
        actions.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
            ActivationSource::Keyboard(ActivationKey::Space)
        ]
    );
    assert!(rows.borrow()["alpha"].active());
    assert!(selections.borrow().is_empty());
    let inspect_node = node(cx, "Inspect Alpha", accesskit::Role::Button).0;
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: inspect_node,
        data: None,
    }));
    draw(cx);
    assert_eq!(
        actions.borrow().last(),
        Some(&ActivationSource::Accessibility)
    );
    assert!(selections.borrow().is_empty());

    click(cx, "text-input:notes:root");
    cx.simulate_input("beta notes");
    cx.simulate_keystrokes("down enter");
    draw(cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).editor.read(cx).value(), "beta notes"));
    assert!(rows.borrow()["alpha"].active());
    assert!(!rows.borrow()["beta"].active());
    assert!(selections.borrow().is_empty());
    assert!(cx.debug_selector_is_focused("text-input:notes:root"));

    click(cx, "row-label:beta");
    press_key(cx, "enter");
    assert_eq!(selections.borrow().as_slice(), ["beta", "beta"]);
}

#[open_gpui::test]
fn custom_content_keeps_duplicate_rows_disabled_and_their_local_state_distinct(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        identities: Rc<RefCell<BTreeMap<usize, EntityId>>>,
        selections: Rc<Cell<usize>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let identities = self.identities.clone();
            let selections = self.selections.clone();
            Listbox::new("duplicate-content", "Resources")
                .option(ListboxOption::new("shared", "First"))
                .option(ListboxOption::new("shared", "Second"))
                .row_content(move |option, window, cx| {
                    assert!(option.disabled());
                    let local = window.use_keyed_state("content-state", cx, |_, _| ());
                    identities
                        .borrow_mut()
                        .insert(option.index(), local.entity_id());
                    let selector = format!("duplicate-content:{}", option.index());
                    div()
                        .id("content")
                        .debug_selector(move || selector.clone())
                        .child(option.label().to_owned())
                })
                .on_select(move |_, _, _| selections.set(selections.get() + 1))
        }
    }

    let identities = Rc::new(RefCell::new(BTreeMap::new()));
    let selections = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        identities: identities.clone(),
        selections: selections.clone(),
    });
    assert!(cx.activate_accessibility());
    draw(cx);
    assert_ne!(identities.borrow()[&0], identities.borrow()[&1]);
    click(cx, "duplicate-content:0");
    click(cx, "duplicate-content:1");
    assert_eq!(selections.get(), 0);
    let first = node(cx, "First", accesskit::Role::ListBoxOption);
    let second = node(cx, "Second", accesskit::Role::ListBoxOption);
    assert_ne!(first.0, second.0);
    assert!(first.1.is_disabled());
    assert!(second.1.is_disabled());
}

#[open_gpui::test]
fn taller_content_keeps_keyboard_reveal_inside_the_original_scroll_surface(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Listbox::new("tall-content", "Resources")
                .options((0..20).map(|index| {
                    ListboxOption::new(format!("resource-{index:02}"), format!("Resource {index}"))
                }))
                .row_content(|option, _, _| div().h(px(48.0)).child(option.label().to_owned()))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| Probe);
    draw(cx);
    click(cx, "listbox:tall-content:option:resource-00");
    press_key(cx, "end");
    draw(cx);
    let viewport = cx
        .debug_bounds("listbox:tall-content")
        .expect("listbox viewport should render");
    let last = cx
        .debug_bounds("listbox:tall-content:option:resource-19")
        .expect("last row should remain mounted");
    assert!(last.center().y >= viewport.top());
    assert!(
        last.center().y <= viewport.bottom(),
        "last row {last:?} must be visible in {viewport:?}"
    );
    assert!(cx.debug_selector_is_focused("listbox:tall-content:option:resource-19"));
}
