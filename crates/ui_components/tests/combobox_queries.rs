use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use image::{Frame, ImageBuffer, Rgba};
use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    Context, InteractiveElement, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    ParentElement, Render, RenderImage, Styled, Window, accesskit, div, img, px,
};
use open_gpui_ui_components::{
    ActivationKey, ActivationSource, Button,
    combobox::{
        Combobox, ComboboxFilterMode, ComboboxGroupDescriptor, ComboboxOption,
        ComboboxOptionDescriptor, ComboboxQuerySession, ComboboxResults, ComboboxResultsStatus,
        ComboboxSelection, ComboboxState,
    },
    gpui_adapter::{OverlayLayerPhase, WindowOverlayRuntime, init_text_input},
    listbox::{ListboxOptionState, ListboxRowAction},
};

fn key_down(key: &str, prefer_character_input: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input,
    }
}

fn rendered_input_value(cx: &open_gpui::VisualTestContext, label: &str) -> Option<String> {
    cx.accessibility_tree_update_history()
        .iter()
        .rev()
        .flat_map(|update| update.nodes.iter())
        .find(|(_, node)| node.role() == accesskit::Role::TextInput && node.label() == Some(label))
        .and_then(|(_, node)| node.value().map(str::to_owned))
}

#[test]
fn controlled_query_and_external_results_are_independent_of_selection_and_local_matching() {
    let state = Combobox::new("external-results", "Resources")
        .default_query("ignored")
        .query("server\nquery")
        .selected_item(Some(ComboboxSelection::new(
            "selected",
            "Selected resource",
        )))
        .results(
            ComboboxResults::new()
                .option(ComboboxOptionDescriptor::new(
                    "ranked",
                    "Provider ranked result",
                ))
                .group(
                    ComboboxGroupDescriptor::new("remote", "Remote results")
                        .option(ComboboxOptionDescriptor::new("grouped", "Grouped result")),
                ),
        )
        .state();

    assert_eq!(state.query(), "server query");
    assert_eq!(state.input().value(), "server query");
    assert_eq!(state.filter_mode(), ComboboxFilterMode::External);
    assert_eq!(state.results_status(), &ComboboxResultsStatus::Ready);
    assert_eq!(state.total_option_count(), 2);
    assert_eq!(state.filtered_option_count(), 2);
    assert!(!state.filtered());
    assert_eq!(state.selected_value(), Some("selected"));
    assert_eq!(state.selected_label(), Some("Selected resource"));
    assert_eq!(state.listbox().selected_value(), None);
    assert_eq!(state.active_value(), Some("ranked"));
    assert_eq!(state.listbox().groups()[0].label(), "Remote results");
}

#[test]
fn explicit_external_filtering_keeps_option_builders_and_local_filtering_still_works() {
    let external = Combobox::new("external", "Resources")
        .query("unrelated")
        .filter_mode(ComboboxFilterMode::External)
        .option(ComboboxOption::new("alpha", "Alpha"))
        .state();
    let local = Combobox::new("local", "Resources")
        .query("unrelated")
        .option(ComboboxOption::new("alpha", "Alpha"))
        .state();

    assert_eq!(external.filtered_option_count(), 1);
    assert_eq!(local.filtered_option_count(), 0);
}

#[test]
fn latest_query_reply_wins_even_when_the_old_reply_arrives_last() {
    let mut provider = ComboboxQuerySession::new();
    let request_a = provider.begin("A");
    let request_b = provider.begin("B");
    let results_b = ComboboxResults::new().option(ComboboxOptionDescriptor::new("b", "Result B"));

    assert_eq!(request_a.query(), "A");
    assert_eq!(request_b.query(), "B");
    assert_ne!(request_a, request_b);
    assert_eq!(provider.pending_request(), Some(&request_b));
    assert!(provider.complete(&request_b, results_b.clone()));
    assert!(!provider.complete(
        &request_a,
        ComboboxResults::new().option(ComboboxOptionDescriptor::new("a", "Result A")),
    ));
    assert!(!provider.complete(&request_a, ComboboxResults::failed("Late failure")));

    let state = Combobox::new("provider", "Resources")
        .selected_item(Some(ComboboxSelection::new(
            "selected",
            "Selected resource",
        )))
        .query(provider.query())
        .results(provider.results().clone())
        .state();

    assert_eq!(provider.results(), &results_b);
    assert!(provider.pending_request().is_none());
    assert_eq!(state.query(), "B");
    assert_eq!(state.listbox().options()[0].value(), "b");
    assert_eq!(state.selected_value(), Some("selected"));
    assert_eq!(state.selected_label(), Some("Selected resource"));
}

#[test]
fn request_identity_rejects_repeated_text_duplicate_foreign_and_retired_replies() {
    let mut provider = ComboboxQuerySession::new();
    let first = provider.begin("same\nquery");
    let current = provider.begin("same\nquery");
    let mut other_host = ComboboxQuerySession::new();
    let foreign = other_host.begin("same query");

    assert_eq!(first.query(), "same query");
    assert_ne!(first, current);
    assert_ne!(current, foreign);
    assert!(!provider.complete(&first, ComboboxResults::new()));
    assert!(!provider.complete(&foreign, ComboboxResults::new()));
    assert!(!provider.complete(&current, ComboboxResults::loading()));
    assert_eq!(provider.pending_request(), Some(&current));
    assert!(provider.complete(&current.clone(), ComboboxResults::failed("Unavailable")));
    assert!(!provider.complete(&current, ComboboxResults::new()));
    assert_eq!(
        provider.results().status(),
        &ComboboxResultsStatus::Error("Unavailable".into())
    );

    let retired = provider.begin("same query");
    provider.cancel();
    assert!(!provider.complete(&retired, ComboboxResults::new()));
    assert_eq!(provider.results().status(), &ComboboxResultsStatus::Idle);
    assert!(provider.pending_request().is_none());

    let reopened = provider.begin("same query");
    assert!(!provider.complete(&retired, ComboboxResults::failed("Retired host")));
    assert_eq!(provider.pending_request(), Some(&reopened));
    assert!(provider.complete(&reopened, ComboboxResults::new()));
}

#[test]
fn loading_error_empty_and_renamed_pages_keep_selection_separate_from_navigation() {
    for results in [
        ComboboxResults::loading(),
        ComboboxResults::failed("Unavailable"),
        ComboboxResults::new(),
    ] {
        let state = Combobox::new("status", "Resources")
            .selected_item(Some(ComboboxSelection::new(
                "selected",
                "Selected resource",
            )))
            .query("query")
            .results(results.clone())
            .state();
        assert_eq!(state.selected_value(), Some("selected"));
        assert_eq!(state.selected_label(), Some("Selected resource"));
        assert_eq!(state.active_value(), None);
        assert!(state.listbox().activation_for_key("enter").is_none());
        assert_eq!(state.results_status(), results.status());
        assert_eq!(
            state.empty_label(),
            results.status().message().unwrap_or("No results")
        );
    }

    for results in [
        ComboboxResults::loading(),
        ComboboxResults::failed("Retry needed"),
    ] {
        let state = Combobox::new("retained-page", "Resources")
            .query("query")
            .results(
                results
                    .option(ComboboxOptionDescriptor::new("disabled", "Disabled").disabled(true))
                    .option(ComboboxOptionDescriptor::new("alpha", "Alpha"))
                    .option(ComboboxOptionDescriptor::new("bravo", "Bravo")),
            )
            .state();
        assert_eq!(state.active_value(), Some("alpha"));
        assert_eq!(
            state
                .listbox()
                .navigation_target("down")
                .map(|item| item.value()),
            Some("bravo")
        );
    }

    let renamed = Combobox::new("renamed", "Resources")
        .selected_item(Some(ComboboxSelection::new("selected", "Previous label")))
        .results(
            ComboboxResults::new().option(ComboboxOptionDescriptor::new("selected", "New label")),
        )
        .state();
    assert_eq!(renamed.selected_value(), Some("selected"));
    assert_eq!(renamed.selected_label(), Some("New label"));
}

#[test]
fn duplicate_result_identities_fail_closed_before_local_filtering() {
    let state = Combobox::new("duplicates", "Resources")
        .selected_item(Some(ComboboxSelection::new("duplicate", "Fallback")))
        .results(
            ComboboxResults::new()
                .option(ComboboxOptionDescriptor::new("duplicate", "Alpha"))
                .group(
                    ComboboxGroupDescriptor::new("group", "Group")
                        .option(ComboboxOptionDescriptor::new("duplicate", "Beta")),
                ),
        )
        .filter_mode(ComboboxFilterMode::Local)
        .query("Alpha")
        .state();

    assert_eq!(state.selected_value(), None);
    assert_eq!(state.selected_label(), None);
    assert_eq!(state.filtered_option_count(), 1);
    assert!(state.listbox().options()[0].disabled());
    assert!(state.listbox().activation_for_key("enter").is_none());
}

#[open_gpui::test]
fn external_pages_retain_selected_label_and_results_composition_keeps_listbox_authority(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        results: ComboboxResults,
        states: Rc<RefCell<Vec<ComboboxState>>>,
        selections: Rc<RefCell<Vec<ComboboxSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let states = self.states.clone();
            let selections = self.selections.clone();
            Combobox::new("composed", "Resources")
                .default_open(true)
                .selected(Some("selected".to_owned()))
                .results(self.results.clone())
                .results_content(move |state, listbox, _, _| {
                    states.borrow_mut().push(state.clone());
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(
                            div()
                                .id("provider-heading")
                                .debug_selector(|| "provider:heading".into())
                                .child("Application results"),
                        )
                        .child(listbox)
                })
                .on_select(move |selection, _, _| selections.borrow_mut().push(selection))
        }
    }

    cx.update(init_text_input);
    let states = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        results: ComboboxResults::new()
            .option(ComboboxOptionDescriptor::new("selected", "Original label")),
        states: states.clone(),
        selections: selections.clone(),
    });
    assert!(cx.activate_accessibility());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        rendered_input_value(cx, "Resources").as_deref(),
        Some("Original label")
    );
    assert!(cx.debug_bounds("provider:heading").is_some());

    for (results, expected_label) in [
        (ComboboxResults::loading(), "Original label"),
        (ComboboxResults::failed("Offline"), "Original label"),
        (
            ComboboxResults::new()
                .option(ComboboxOptionDescriptor::new("selected", "Renamed label")),
            "Renamed label",
        ),
        (
            ComboboxResults::new().option(ComboboxOptionDescriptor::new("other", "Other resource")),
            "Renamed label",
        ),
    ] {
        view.update(cx, |view, cx| {
            view.results = results;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let states = states.borrow();
        let state = states
            .last()
            .expect("composed results should receive resolved state");
        assert_eq!(state.selected_value(), Some("selected"));
        assert_eq!(state.selected_label(), Some(expected_label));
        assert_eq!(
            rendered_input_value(cx, "Resources").as_deref(),
            Some(expected_label)
        );
    }

    let input = cx
        .debug_bounds("text-input:composed-input:root")
        .expect("editor should render");
    cx.simulate_click(input.center(), Modifiers::none());
    cx.simulate_event(key_down("enter", false));
    assert_eq!(
        selections.borrow().as_slice(),
        &[ComboboxSelection::new("other", "Other resource")]
    );
    assert!(cx.debug_selector_is_focused("text-input:composed-input:root"));
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("overlay snapshot should resolve")
    });
    assert_eq!(
        snapshot
            .layers()
            .iter()
            .filter(|layer| layer.id().as_str() == "combobox:composed")
            .count(),
        1
    );
}

#[open_gpui::test]
fn controlled_query_can_accept_transform_and_reject_proposals_without_selection_projection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        query: String,
        accept: bool,
        proposals: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity().downgrade();
            let proposals = self.proposals.clone();
            Combobox::new("controlled-query", "Controlled query")
                .query(self.query.clone())
                .default_selected("selected")
                .results(ComboboxResults::new().option(ComboboxOptionDescriptor::new(
                    "selected",
                    "Selected resource",
                )))
                .on_query_change(move |query, _, cx| {
                    proposals.borrow_mut().push(query.clone());
                    let _ = view.update(cx, |view, cx| {
                        if view.accept {
                            view.query = query.to_uppercase();
                        }
                        cx.notify();
                    });
                })
        }
    }

    cx.update(init_text_input);
    let proposals = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        query: "seed".to_owned(),
        accept: true,
        proposals: proposals.clone(),
    });
    assert!(cx.activate_accessibility());
    let input = cx
        .debug_bounds("text-input:controlled-query-input:root")
        .expect("editor should render");
    cx.simulate_click(input.center(), Modifiers::none());
    cx.simulate_input("a");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(proposals.borrow().as_slice(), ["seeda"]);
    assert_eq!(
        rendered_input_value(cx, "Controlled query").as_deref(),
        Some("SEEDA")
    );

    view.update(cx, |view, cx| {
        view.accept = false;
        cx.notify();
    });
    cx.simulate_input("b");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(proposals.borrow().as_slice(), ["seeda", "SEEDAb"]);
    assert_eq!(
        rendered_input_value(cx, "Controlled query").as_deref(),
        Some("SEEDA")
    );
    assert!(cx.debug_selector_is_focused("text-input:controlled-query-input:root"));
}

#[open_gpui::test]
fn controlled_query_survives_uncontrolled_choice_commit(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<ComboboxSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            Combobox::new("independent-query", "Independent query")
                .query("search")
                .default_open(true)
                .results(
                    ComboboxResults::new()
                        .option(ComboboxOptionDescriptor::new("result", "Result label")),
                )
                .on_select(move |selection, _, _| selections.borrow_mut().push(selection))
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    assert!(cx.activate_accessibility());
    let input = cx
        .debug_bounds("text-input:independent-query-input:root")
        .expect("editor should render");
    cx.simulate_click(input.center(), Modifiers::none());
    cx.simulate_event(key_down("enter", false));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        selections.borrow().as_slice(),
        &[ComboboxSelection::new("result", "Result label")]
    );
    assert_eq!(
        rendered_input_value(cx, "Independent query").as_deref(),
        Some("search")
    );
}

#[open_gpui::test]
fn marked_composition_and_character_input_enter_do_not_commit_or_dismiss(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        query: String,
        queries: Rc<RefCell<Vec<String>>>,
        selections: Rc<RefCell<Vec<ComboboxSelection>>>,
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity().downgrade();
            let queries = self.queries.clone();
            let selections = self.selections.clone();
            let open_events = self.open_events.clone();
            Combobox::new("composition", "Composing query")
                .default_open(true)
                .query(self.query.clone())
                .results(
                    ComboboxResults::new()
                        .option(ComboboxOptionDescriptor::new("resource", "Resource")),
                )
                .on_query_change(move |query, _, cx| {
                    queries.borrow_mut().push(query.clone());
                    let _ = view.update(cx, |view, cx| {
                        view.query = query;
                        cx.notify();
                    });
                })
                .on_select(move |selection, _, _| selections.borrow_mut().push(selection))
                .on_open_change(move |intent, _, _| {
                    open_events.borrow_mut().push(intent.desired_open())
                })
        }
    }

    cx.update(init_text_input);
    let queries = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        query: String::new(),
        queries: queries.clone(),
        selections: selections.clone(),
        open_events: open_events.clone(),
    });
    assert!(cx.activate_accessibility());
    let input = cx
        .debug_bounds("text-input:composition-input:root")
        .expect("editor should render");
    cx.simulate_click(input.center(), Modifiers::none());
    cx.simulate_marked_text(None, "ni", Some(2..2));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(queries.borrow().is_empty());
    assert_eq!(
        rendered_input_value(cx, "Composing query").as_deref(),
        Some("ni")
    );

    cx.simulate_event(key_down("enter", false));
    cx.simulate_event(key_down("enter", true));
    cx.simulate_event(key_down("escape", false));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(selections.borrow().is_empty());
    assert!(open_events.borrow().is_empty());
    assert!(cx.debug_bounds("combobox:composition:content").is_some());
    assert!(cx.debug_selector_is_focused("text-input:composition-input:root"));

    cx.simulate_marked_text(None, "", None);
    cx.simulate_input("你");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(queries.borrow().last().map(String::as_str), Some("你"));
    assert!(!queries.borrow().iter().any(|query| query == "ni"));
    cx.simulate_event(key_down("enter", true));
    assert!(selections.borrow().is_empty());
    cx.simulate_event(key_down("enter", false));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        selections.borrow().as_slice(),
        &[ComboboxSelection::new("resource", "Resource")]
    );
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("overlay snapshot should resolve")
    });
    let layer = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "combobox:composition")
        .expect("popup should keep one reusable registration");
    assert_eq!(layer.phase(), OverlayLayerPhase::Hidden);
}

#[open_gpui::test]
fn mock_provider_reply_order_and_host_retirement_preserve_the_owned_popup(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        mounted: bool,
        provider: ComboboxQuerySession,
        states: Rc<RefCell<Vec<ComboboxState>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let states = self.states.clone();
            div().when(self.mounted, |this| {
                this.child(
                    Combobox::new("mock-provider", "Resources")
                        .default_open(true)
                        .query(self.provider.query())
                        .selected_item(Some(ComboboxSelection::new(
                            "selected",
                            "Selected resource",
                        )))
                        .results(self.provider.results().clone())
                        .results_content(move |state, listbox, _, _| {
                            states.borrow_mut().push(state.clone());
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .child("Provider results")
                                .child(listbox)
                        }),
                )
            })
        }
    }

    cx.update(init_text_input);
    let states = Rc::new(RefCell::new(Vec::new()));
    let mut provider = ComboboxQuerySession::new();
    let request_a = provider.begin("A");
    let request_b = provider.begin("B");
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        mounted: true,
        provider,
        states: states.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    view.update(cx, |view, cx| {
        assert!(view.provider.complete(
            &request_b,
            ComboboxResults::new().option(ComboboxOptionDescriptor::new("b", "Result B")),
        ));
        assert!(!view.provider.complete(
            &request_a,
            ComboboxResults::new().option(ComboboxOptionDescriptor::new("a", "Result A")),
        ));
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    {
        let states = states.borrow();
        let state = states.last().expect("current provider page should compose");
        assert_eq!(state.query(), "B");
        assert_eq!(state.selected_value(), Some("selected"));
        assert_eq!(state.selected_label(), Some("Selected resource"));
        assert_eq!(state.listbox().options()[0].value(), "b");
    }

    let retired = view.update(cx, |view, cx| {
        let request = view.provider.begin("B");
        view.mounted = false;
        view.provider.cancel();
        cx.notify();
        request
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("combobox:mock-provider:content").is_none());

    view.update(cx, |view, cx| {
        view.mounted = true;
        let reopened = view.provider.begin("B");
        assert!(
            !view
                .provider
                .complete(&retired, ComboboxResults::failed("Retired request"))
        );
        assert!(view.provider.complete(
            &reopened,
            ComboboxResults::new().option(ComboboxOptionDescriptor::new("fresh", "Fresh result")),
        ));
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let states = states.borrow();
    let state = states
        .last()
        .expect("reopened provider page should compose");
    assert_eq!(state.listbox().options()[0].value(), "fresh");
    assert_eq!(state.selected_value(), Some("selected"));
}

#[open_gpui::test]
fn thumbnail_option_content_keeps_combobox_selection_focus_and_popup_ownership(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        thumbnail: Arc<RenderImage>,
        rows: Rc<RefCell<Vec<ListboxOptionState>>>,
        actions: Rc<RefCell<Vec<ActivationSource>>>,
        selections: Rc<RefCell<Vec<ComboboxSelection>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let thumbnail = self.thumbnail.clone();
            let rows = self.rows.clone();
            let actions = self.actions.clone();
            let selections = self.selections.clone();
            Combobox::new("resource-thumbnails", "Resources")
                .default_open(true)
                .query("server query")
                .selected_item(Some(ComboboxSelection::new(
                    "selected",
                    "Selected elsewhere",
                )))
                .results(
                    ComboboxResults::new()
                        .option(ComboboxOptionDescriptor::new("alpha", "Alpha texture"))
                        .group(
                            ComboboxGroupDescriptor::new("images", "Images")
                                .option(ComboboxOptionDescriptor::new("beta", "Beta texture")),
                        ),
                )
                .option_content(move |option, _, _| {
                    rows.borrow_mut().push(option.clone());
                    let actions = actions.clone();
                    let selector = format!("resource-thumbnail:{}", option.value());
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
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(option.label().to_owned())
                                .child("PNG · 64 × 64"),
                        )
                        .child(ListboxRowAction::new(
                            "inspect-action",
                            Button::new(
                                format!("inspect-resource:{}", option.value()),
                                format!("Inspect {}", option.label()),
                            )
                            .disabled(option.disabled())
                            .on_activate(move |activation, _, _| {
                                actions.borrow_mut().push(activation.source())
                            }),
                        ))
                })
                .on_select(move |selection, _, _| selections.borrow_mut().push(selection))
        }
    }

    cx.update(init_text_input);
    let rows = Rc::new(RefCell::new(Vec::new()));
    let actions = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let thumbnail = Arc::new(RenderImage::new([Frame::new(ImageBuffer::from_pixel(
        2,
        2,
        Rgba([208, 112, 48, 255]),
    ))]));
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        thumbnail,
        rows: rows.clone(),
        actions: actions.clone(),
        selections: selections.clone(),
    });
    assert!(cx.activate_accessibility());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        rows.borrow()
            .iter()
            .any(|option| option.value() == "beta" && option.group_index().is_some())
    );
    let editor = cx
        .debug_bounds("text-input:resource-thumbnails-input:root")
        .expect("the original editor should render");
    cx.simulate_click(editor.center(), Default::default());
    let thumbnail = cx
        .debug_bounds("resource-thumbnail:alpha")
        .expect("thumbnail should render inside the original row");
    cx.simulate_click(thumbnail.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_scheduled_frame();
    assert_eq!(
        selections.borrow().as_slice(),
        &[ComboboxSelection::new("alpha", "Alpha texture")]
    );
    assert!(actions.borrow().is_empty());
    assert!(
        cx.debug_bounds("combobox:resource-thumbnails:content")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("text-input:resource-thumbnails-input:root"));
    assert_eq!(
        rendered_input_value(cx, "Resources").as_deref(),
        Some("server query")
    );

    let toggle = cx
        .debug_bounds("combobox:resource-thumbnails:toggle")
        .expect("toggle should remain reusable");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let inspect = cx
        .debug_bounds("button:inspect-resource:beta:root")
        .expect("nested resource action should render");
    cx.simulate_click(inspect.center(), Default::default());
    cx.simulate_event(key_down("enter", false));
    cx.simulate_event(KeyUpEvent {
        keystroke: key_down("enter", false).keystroke,
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        actions.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter)
        ]
    );
    assert_eq!(selections.borrow().len(), 1);
    assert!(
        cx.debug_bounds("combobox:resource-thumbnails:content")
            .is_some()
    );
    assert!(cx.debug_selector_is_focused("button:inspect-resource:beta:root"));

    let input = cx
        .debug_bounds("text-input:resource-thumbnails-input:root")
        .expect("original editor should render");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_event(key_down("down", false));
    cx.simulate_event(key_down("enter", false));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_scheduled_frame();
    assert_eq!(
        selections.borrow().last(),
        Some(&ComboboxSelection::new("beta", "Beta texture"))
    );
    assert!(cx.debug_selector_is_focused("text-input:resource-thumbnails-input:root"));
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("popup snapshot should resolve")
    });
    let layers = snapshot
        .layers()
        .iter()
        .filter(|layer| layer.id().as_str() == "combobox:resource-thumbnails")
        .collect::<Vec<_>>();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].phase(), OverlayLayerPhase::Hidden);
}
