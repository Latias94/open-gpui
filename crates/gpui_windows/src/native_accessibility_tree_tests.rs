//! Synthetic activation and explicit raw inspection through a real Windows Application.
//!
//! The installed production activation handler is invoked directly, outside App borrows.
//! Accepted-tree snapshots are correlated with nonempty renderer Submitted receipts. This
//! does not qualify a real AT client, OS automation, UIA event receipt or compositor scanout.

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use accesskit::{ActivationHandler, Node, Role};
use open_gpui::{
    InteractiveElement, StatefulInteractiveElement, WindowHandle, WindowInitialPresentationStatus,
    private::serde_json::{self, Value},
};

const INITIAL_TEXT: &str = "native raw text: α🦀 <initial>";
const CHANGED_TEXT: &str = "native raw text: β中 <changed>";
const REMOVED_TEXT: &str = "native raw text: γ🎛 <metadata removed>";
const CREATION_TITLE: &str = "Native a11y <creation snapshot>";
const RAW_TITLE: &str = "Native a11y <explicit inspection>";
const OWNER_LABEL: &str = "Native synthetic owner";
const CHILD_LABEL: &str = "Native synthetic text";
const INITIAL_AUTHOR_ID: &str = "native.settings.音量.initial";
const CHANGED_AUTHOR_ID: &str = "native.settings.音量.changed";
const INITIAL_SHORTCUT: &str = "Control+Shift+音量↑";
const CHANGED_SHORTCUT: &str = "Alt+中🦀";

#[derive(Default)]
struct NativeA11yProbe {
    renders: Cell<usize>,
    hooks: Cell<usize>,
}

struct NativeA11yRoot {
    text: &'static str,
    author_id: Option<&'static str>,
    keyshortcuts: Option<&'static str>,
    numeric_value_step: Option<f64>,
    probe: Rc<NativeA11yProbe>,
}

impl Render for NativeA11yRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.probe.renders.set(self.probe.renders.get() + 1);
        let text = self.text;
        let author_id = self.author_id;
        let keyshortcuts = self.keyshortcuts;
        let numeric_value_step = self.numeric_value_step;
        let hook_probe = self.probe.clone();
        let mut owner = div()
            .id("native-synthetic-owner")
            .role(Role::Group)
            .aria_label(OWNER_LABEL)
            .size_full()
            .bg(white());
        if let Some(author_id) = author_id {
            owner = owner.accessibility_id(author_id);
        }
        if let Some(keyshortcuts) = keyshortcuts {
            owner = owner.aria_keyshortcuts(keyshortcuts);
        }
        if let Some(step) = numeric_value_step {
            owner = owner.aria_numeric_value_step(step);
        }
        owner
            .a11y_synthetic_children(move |builder| {
                hook_probe.hooks.set(hook_probe.hooks.get() + 1);
                let id = builder.synthetic_node_id(("text", 0_usize));
                let mut child = Node::new(Role::TextRun);
                child.set_label(CHILD_LABEL);
                child.set_value(text);
                child.set_character_lengths(
                    text.chars()
                        .map(|character| character.len_utf8() as u8)
                        .collect::<Vec<_>>(),
                );
                assert!(builder.push_child(id, child));
                builder
                    .parent_node()
                    .set_description("Raw parent <metadata>");
                let parent = builder.parent_node();
                assert_eq!(parent.author_id(), author_id);
                assert_eq!(parent.keyboard_shortcut(), keyshortcuts);
                assert_eq!(parent.numeric_value_step(), numeric_value_step);
            })
            .child(text)
    }
}

fn assert_native_a11y_initial_tree(update: &accesskit::TreeUpdate) {
    let tree = update.tree.as_ref().expect("initial Window tree");
    assert_eq!(tree.root, accesskit::NodeId(0));
    assert_eq!(update.tree_id, accesskit::TreeId::ROOT);
    assert_eq!(update.focus, tree.root);
    assert_eq!(update.nodes.len(), 1);
    let (id, node) = &update.nodes[0];
    assert_eq!(*id, tree.root);
    assert_eq!(node.role(), Role::Window);
    assert_eq!(node.label(), Some(CREATION_TITLE));
    assert!(node.children().is_empty());
    assert_eq!(node.author_id(), None);
    assert_eq!(node.keyboard_shortcut(), None);
    assert_eq!(node.numeric_value_step(), None);
    assert_eq!(node.value(), None);
}

fn labelled<'a>(snapshot: &'a Value, label: &str) -> (&'a str, &'a Value) {
    let matches = snapshot["nodes"]
        .as_object()
        .expect("snapshot nodes")
        .iter()
        .filter(|(_, node)| node["aria"]["label"] == label)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one node labelled {label:?}"
    );
    (matches[0].0.as_str(), matches[0].1)
}

fn wait_for_native_a11y_submission(
    app: &mut Application,
    handle: WindowHandle<NativeA11yRoot>,
    text: &'static str,
    previous_generation: u64,
) -> Value {
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native a11y accepted nonempty submission",
        Duration::from_millis(5),
        || {
            let (facts, revision, diagnostics, raw, enabled, active) = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        (
                            window.presentation_facts(),
                            window.rendered_frame_revision(),
                            window
                                .submitted_frame_diagnostics_for_test()
                                .map(|receipt| receipt.generation()),
                            window.debug_a11y_tree_json(),
                            window.is_a11y_enabled(),
                            window.is_a11y_active(),
                        )
                    })
                })
                .expect("the exact native a11y window must remain open");
            let (Some(attempt), Some(raw)) = (facts.latest_present_attempt, raw) else {
                return false;
            };
            let snapshot: Value = serde_json::from_str(&raw).expect("official raw inspection JSON");
            if attempt.generation > previous_generation
                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                && attempt.contained_valid_primitives
                && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                && facts.native_visible
                && facts.frame_accepted_generation == Some(attempt.generation)
                && facts.present_submitted_generation == Some(attempt.generation)
                && facts.non_empty_presented_generation == Some(attempt.generation)
                && revision == attempt.generation
                && diagnostics == Some(attempt.generation)
                && snapshot["frame"]["frame_generation"].as_u64() == Some(attempt.generation)
                && labelled(&snapshot, CHILD_LABEL).1["aria"]["value"] == text
                && enabled
                && active
            {
                submitted = Some(snapshot);
                true
            } else {
                false
            }
        },
    );
    let snapshot = submitted.expect("raw inspection must belong to the exact Submitted frame");
    let nodes = snapshot["nodes"].as_object().unwrap();
    assert_eq!(
        snapshot["frame"]["node_count"].as_u64(),
        Some(nodes.len() as u64)
    );
    assert!(snapshot["frame"]["activation_generation"].as_u64().unwrap() > 0);
    assert_eq!(snapshot["frame"]["window_title"], RAW_TITLE);
    let (owner_key, owner) = labelled(&snapshot, OWNER_LABEL);
    let (child_key, child) = labelled(&snapshot, CHILD_LABEL);
    assert_eq!(owner["aria"]["role"], "Group");
    assert_eq!(owner["aria"]["description"], "Raw parent <metadata>");
    assert_eq!(owner["children"], serde_json::json!([child_key]));
    assert_eq!(child["aria"]["role"], "TextRun");
    let root = snapshot["root"].as_str().expect("published window root");
    assert_eq!(nodes[root]["aria"]["role"], "Window");
    assert_eq!(nodes[root]["aria"]["label"], RAW_TITLE);
    assert_eq!(nodes[root]["children"], serde_json::json!([owner_key]));
    child["accesskit_id"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .expect("synthetic AccessKit identity");
    #[cfg(debug_assertions)]
    {
        assert_eq!(child["synthetic"], true);
        assert!(owner.get("synthetic").is_none());
        for property in ["element_id", "view", "source_location"] {
            assert!(
                !owner[property]
                    .as_str()
                    .expect("real creator provenance")
                    .is_empty()
            );
            assert_eq!(child[property], owner[property]);
        }
        assert!(
            child["element_id"]
                .as_str()
                .unwrap()
                .contains("native-synthetic-owner")
        );
        assert!(child["view"].as_str().unwrap().contains("NativeA11yRoot"));
        assert!(
            child["source_location"]
                .as_str()
                .unwrap()
                .contains("native_accessibility_tree_tests.rs:")
        );
    }
    snapshot
}

fn assert_native_a11y_reads_are_passive(
    app: &mut Application,
    handle: WindowHandle<NativeA11yRoot>,
    probe: &NativeA11yProbe,
    frame_requests: &Cell<usize>,
) {
    let read = |app: &mut Application| {
        app.update_for_test(|cx| {
            handle.update(cx, |_, window, _| {
                (
                    window.debug_a11y_tree_json(),
                    window.presentation_facts(),
                    window.is_a11y_enabled(),
                    window.is_a11y_active(),
                )
            })
        })
        .expect("the inspected native window must remain open")
    };
    let before = read(app);
    let counts = (probe.renders.get(), probe.hooks.get());
    for _ in 0..3 {
        assert_eq!(read(app), before);
    }
    let requests_before = frame_requests.get();
    pump_messages_until_with_delay(
        "passive inspection frame opportunities",
        Duration::from_millis(5),
        || frame_requests.get() >= requests_before + 2,
    );
    assert_eq!(read(app), before);
    assert_eq!((probe.renders.get(), probe.hooks.get()), counts);
}

#[test]
fn native_synthetic_a11y_activation_snapshot_update_and_release() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let cursor = observation.cursor();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native a11y Windows backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let text_system_lifetime = Arc::downgrade(&app.text_system());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let probe = Rc::new(NativeA11yProbe::default());
    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(480.), px(240.)), cx)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(CREATION_TITLE.into()),
                        ..TitlebarOptions::default()
                    }),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    window.set_window_title(RAW_TITLE);
                    cx.new(|_| NativeA11yRoot {
                        text: INITIAL_TEXT,
                        author_id: Some(INITIAL_AUTHOR_ID),
                        keyshortcuts: Some(INITIAL_SHORTCUT),
                        numeric_value_step: Some(0.25),
                        probe: probe.clone(),
                    })
                },
            )
        })
        .expect("the native a11y window must open");
    let root = app.update_for_test(|cx| handle.root(cx).unwrap().downgrade());
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|registration| registration.window_id() == handle.window_id())
        .expect("the a11y consumer must own its exact HWND registration");
    let hwnd = registration.as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("registered native a11y HWND");
    assert_eq!(native_window.registration, registration);
    let native_lifetime = Rc::downgrade(&native_window);
    let frame_requests = observe_frame_requests(&native_window);
    let read_submission = |app: &mut Application| {
        app.update_for_test(|cx| {
            handle.update(cx, |_, window, _| {
                (
                    window.presentation_facts(),
                    window.rendered_frame_revision(),
                    window
                        .submitted_frame_diagnostics_for_test()
                        .map(|receipt| receipt.generation()),
                )
            })
        })
        .expect("the exact native a11y window must remain open")
    };
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            for _ in 0..3 {
                assert_eq!(window.debug_a11y_tree_json(), None);
                assert!(window.is_a11y_enabled());
                assert!(!window.is_a11y_active());
            }
        })
    })
    .unwrap();
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    let requests_before = frame_requests.get();
    pump_messages_until_with_delay(
        "inactive a11y frame opportunities",
        Duration::from_millis(5),
        || frame_requests.get() >= requests_before + 2,
    );
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert_eq!(window.debug_a11y_tree_json(), None);
            assert!(window.is_a11y_enabled());
            assert!(!window.is_a11y_active());
        })
    })
    .unwrap();
    assert_eq!(probe.hooks.get(), 0);
    assert_native_a11y_reads_are_passive(&mut app, handle, &probe, &frame_requests);
    let submission_before_activation = read_submission(&mut app);
    let counts_before_activation = (probe.renders.get(), probe.hooks.get());
    let activation_snapshot = {
        let mut a11y = native_window.state.a11y.borrow_mut();
        a11y.as_mut()
            .expect("production a11y state")
            .activation_handler
            .request_initial_tree()
            .expect("installed activation must return the synchronous initial Window tree")
    };
    assert_native_a11y_initial_tree(&activation_snapshot);
    assert_eq!(
        (probe.renders.get(), probe.hooks.get()),
        counts_before_activation
    );
    assert_eq!(read_submission(&mut app), submission_before_activation);
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert_eq!(window.debug_a11y_tree_json(), None);
            assert!(window.is_a11y_enabled());
        })
    })
    .expect("the initial Window snapshot must not publish an inspected frame");
    assert_eq!(
        (probe.renders.get(), probe.hooks.get()),
        counts_before_activation
    );
    let initial = wait_for_native_a11y_submission(
        &mut app,
        handle,
        INITIAL_TEXT,
        submission_before_activation
            .0
            .frame_accepted_generation
            .unwrap_or(0),
    );
    let initial_owner = labelled(&initial, OWNER_LABEL).1;
    assert_eq!(initial_owner["aria"]["author_id"], INITIAL_AUTHOR_ID);
    assert_eq!(initial_owner["aria"]["keyboard_shortcut"], INITIAL_SHORTCUT);
    assert_eq!(initial_owner["aria"]["numeric_value_step"], 0.25);
    assert!(probe.hooks.get() > 0);
    assert_native_a11y_reads_are_passive(&mut app, handle, &probe, &frame_requests);
    let counts_before = (probe.renders.get(), probe.hooks.get());
    let initial_generation = initial["frame"]["frame_generation"].as_u64().unwrap();
    app.update_for_test(|cx| {
        handle.update(cx, |root, _, cx| {
            root.text = CHANGED_TEXT;
            root.author_id = Some(CHANGED_AUTHOR_ID);
            root.keyshortcuts = Some(CHANGED_SHORTCUT);
            root.numeric_value_step = Some(0.5);
            cx.notify();
        })
    })
    .expect("the existing native source must notify its view");
    let changed =
        wait_for_native_a11y_submission(&mut app, handle, CHANGED_TEXT, initial_generation);
    let changed_owner = labelled(&changed, OWNER_LABEL).1;
    assert_eq!(changed_owner["aria"]["author_id"], CHANGED_AUTHOR_ID);
    assert_eq!(changed_owner["aria"]["keyboard_shortcut"], CHANGED_SHORTCUT);
    assert_eq!(changed_owner["aria"]["numeric_value_step"], 0.5);
    assert!(probe.renders.get() > counts_before.0 && probe.hooks.get() > counts_before.1);
    assert_eq!(
        labelled(&initial, CHILD_LABEL).1["accesskit_id"],
        labelled(&changed, CHILD_LABEL).1["accesskit_id"]
    );
    assert_eq!(
        labelled(&initial, OWNER_LABEL).1["accesskit_id"],
        labelled(&changed, OWNER_LABEL).1["accesskit_id"]
    );
    assert_eq!(
        initial["frame"]["activation_generation"],
        changed["frame"]["activation_generation"]
    );
    assert!(changed["frame"]["frame_number"].as_u64() > initial["frame"]["frame_number"].as_u64());
    assert!(!changed.to_string().contains(INITIAL_TEXT));
    assert!(!changed.to_string().contains(INITIAL_AUTHOR_ID));
    assert!(!changed.to_string().contains(INITIAL_SHORTCUT));
    assert_native_a11y_reads_are_passive(&mut app, handle, &probe, &frame_requests);

    let changed_generation = changed["frame"]["frame_generation"].as_u64().unwrap();
    let counts_before_removal = (probe.renders.get(), probe.hooks.get());
    app.update_for_test(|cx| {
        handle.update(cx, |root, _, cx| {
            root.text = REMOVED_TEXT;
            root.author_id = None;
            root.keyshortcuts = None;
            root.numeric_value_step = None;
            cx.notify();
        })
    })
    .expect("the existing native source must notify metadata removal");
    let removed =
        wait_for_native_a11y_submission(&mut app, handle, REMOVED_TEXT, changed_generation);
    let removed_owner = labelled(&removed, OWNER_LABEL).1;
    for property in ["author_id", "keyboard_shortcut", "numeric_value_step"] {
        assert!(removed_owner["aria"].get(property).is_none());
    }
    assert!(probe.renders.get() > counts_before_removal.0);
    assert!(probe.hooks.get() > counts_before_removal.1);
    for label in [OWNER_LABEL, CHILD_LABEL] {
        assert_eq!(
            labelled(&changed, label).1["accesskit_id"],
            labelled(&removed, label).1["accesskit_id"]
        );
    }
    assert_eq!(
        changed["frame"]["activation_generation"],
        removed["frame"]["activation_generation"]
    );
    assert!(removed["frame"]["frame_number"].as_u64() > changed["frame"]["frame_number"].as_u64());
    assert!(!removed.to_string().contains(CHANGED_TEXT));
    assert!(!removed.to_string().contains(CHANGED_AUTHOR_ID));
    assert!(!removed.to_string().contains(CHANGED_SHORTCUT));
    assert_native_a11y_reads_are_passive(&mut app, handle, &probe, &frame_requests);

    let submission_before_reactivation = read_submission(&mut app);
    let counts_before_reactivation = (probe.renders.get(), probe.hooks.get());
    let reactivation_snapshot = app
        .update_for_test(|cx| {
            handle.update(cx, |_, window, _| {
                let facts = window.presentation_facts();
                let revision = window.rendered_frame_revision();
                let initial = native_window
                    .state
                    .a11y
                    .borrow_mut()
                    .as_mut()
                    .expect("production a11y state")
                    .activation_handler
                    .request_initial_tree_on_worker_for_test()
                    .expect(
                        "the original Send activation returns while App and Window are borrowed",
                    );
                assert_eq!(window.presentation_facts(), facts);
                assert_eq!(window.rendered_frame_revision(), revision);
                assert_eq!(window.debug_a11y_tree_json(), None);
                initial
            })
        })
        .expect("worker activation must preserve the existing checked-out Window");
    assert_native_a11y_initial_tree(&reactivation_snapshot);
    assert_eq!(
        (probe.renders.get(), probe.hooks.get()),
        counts_before_reactivation
    );
    assert_eq!(read_submission(&mut app), submission_before_reactivation);
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert_eq!(window.debug_a11y_tree_json(), None);
            assert!(window.is_a11y_enabled());
        })
    })
    .expect("repeated activation must invalidate the previous inspected frame");
    assert_eq!(
        (probe.renders.get(), probe.hooks.get()),
        counts_before_reactivation
    );
    let removed_generation = removed["frame"]["frame_generation"].as_u64().unwrap();
    let reactivated =
        wait_for_native_a11y_submission(&mut app, handle, REMOVED_TEXT, removed_generation);
    assert!(probe.renders.get() > counts_before_reactivation.0);
    assert!(probe.hooks.get() > counts_before_reactivation.1);
    assert!(
        reactivated["frame"]["activation_generation"].as_u64()
            > removed["frame"]["activation_generation"].as_u64()
    );
    assert!(
        reactivated["frame"]["frame_number"].as_u64() > removed["frame"]["frame_number"].as_u64()
    );
    for label in [OWNER_LABEL, CHILD_LABEL] {
        assert_eq!(
            labelled(&removed, label).1["accesskit_id"],
            labelled(&reactivated, label).1["accesskit_id"]
        );
    }
    let reactivated_owner = labelled(&reactivated, OWNER_LABEL).1;
    for property in ["author_id", "keyboard_shortcut", "numeric_value_step"] {
        assert!(reactivated_owner["aria"].get(property).is_none());
    }
    assert_native_a11y_reads_are_passive(&mut app, handle, &probe, &frame_requests);

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, cx| {
            window.remove_window(cx);
            assert_eq!(window.debug_a11y_tree_json(), None);
        })
    })
    .expect("the exact native a11y window must close");
    pump_messages_until_with_delay(
        "exact native a11y registration retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !is_registered(&platform, hwnd)
                && platform.inner.native_retirement_is_settled()
        },
    );
    let lifecycle = observation
        .events_after(cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                assert_ne!(disposition, NativeWindowTestMessageDisposition::Panicked);
                None
            }
            lifecycle => Some(lifecycle),
        })
        .collect::<Vec<_>>();
    let [
        NativeWindowTestEventKind::PresentationQuiesced {
            ticket_generation: quiesced,
        },
        NativeWindowTestEventKind::DestroyEntered {
            ticket_generation: destroyed,
        },
        NativeWindowTestEventKind::NativeTerminal {
            ticket_generation: terminal,
        },
    ] = lifecycle.as_slice()
    else {
        panic!("unexpected native a11y retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(platform.raw_window_handles.read().is_empty());
    assert!(root.upgrade().is_none());
    drop(native_window);
    assert!(native_lifetime.upgrade().is_none());
    assert_eq!(Rc::strong_count(&probe), 1);
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native a11y App and platform release",
        Duration::from_millis(5),
        || {
            platform_lifetime.upgrade().is_none()
                && !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
        },
    );
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(text_system_lifetime.upgrade().is_none());
}
