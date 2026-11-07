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
const RAW_TITLE: &str = "Native a11y <explicit inspection>";
const OWNER_LABEL: &str = "Native synthetic owner";
const CHILD_LABEL: &str = "Native synthetic text";

#[derive(Default)]
struct NativeA11yProbe {
    renders: Cell<usize>,
    hooks: Cell<usize>,
}

struct NativeA11yRoot {
    text: &'static str,
    probe: Rc<NativeA11yProbe>,
}

impl Render for NativeA11yRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.probe.renders.set(self.probe.renders.get() + 1);
        let text = self.text;
        let hook_probe = self.probe.clone();
        div()
            .id("native-synthetic-owner")
            .role(Role::Group)
            .aria_label(OWNER_LABEL)
            .size_full()
            .bg(white())
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
            })
            .child(text)
    }
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
            let (facts, revision, diagnostics, raw) = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        (
                            window.presentation_facts(),
                            window.rendered_frame_revision(),
                            window
                                .submitted_frame_diagnostics_for_test()
                                .map(|receipt| receipt.generation()),
                            window.debug_a11y_tree_json(),
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
                (window.debug_a11y_tree_json(), window.presentation_facts())
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
    let platform = Rc::new(WindowsPlatform::new(false).expect("native a11y Windows backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let text_system_lifetime = Arc::downgrade(&app.text_system());
    platform.begin_vsync_thread();
    let probe = Rc::new(NativeA11yProbe::default());
    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(480.), px(240.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    window.set_window_title(RAW_TITLE);
                    cx.new(|_| NativeA11yRoot {
                        text: INITIAL_TEXT,
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
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            for _ in 0..3 {
                assert_eq!(window.debug_a11y_tree_json(), None);
                assert!(!window.is_accessibility_active());
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
            assert!(!window.is_accessibility_active());
        })
    })
    .unwrap();
    assert_eq!(probe.hooks.get(), 0);
    {
        let mut a11y = native_window.state.a11y.borrow_mut();
        assert!(
            a11y.as_mut()
                .expect("production a11y state")
                .activation_handler
                .request_initial_tree()
                .is_none()
        );
    }
    let initial = wait_for_native_a11y_submission(&mut app, handle, INITIAL_TEXT, 0);
    assert!(probe.hooks.get() > 0);
    assert_native_a11y_reads_are_passive(&mut app, handle, &probe, &frame_requests);
    let counts_before = (probe.renders.get(), probe.hooks.get());
    let initial_generation = initial["frame"]["frame_generation"].as_u64().unwrap();
    app.update_for_test(|cx| {
        handle.update(cx, |root, _, cx| {
            root.text = CHANGED_TEXT;
            cx.notify();
        })
    })
    .expect("the existing native source must notify its view");
    let changed =
        wait_for_native_a11y_submission(&mut app, handle, CHANGED_TEXT, initial_generation);
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
