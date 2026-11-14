use super::*;
use crate::{TestAppContext, TitlebarOptions};
use accesskit::{NodeId, Role, TreeUpdate};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Activation = Box<dyn Fn() -> Option<TreeUpdate> + Send + 'static>;

struct ActivationProbe {
    renders: Rc<Cell<usize>>,
    leased_paint: bool,
}

impl Render for ActivationProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let mut root = crate::div()
            .id("activation-probe")
            .role(Role::Button)
            .aria_label("Complete control");
        if self.leased_paint {
            root = root.child(
                crate::canvas(
                    |_, _, _| {},
                    |bounds, (), window, _| {
                        let _ = window.insert_scene_primitive(MonochromeSprite {
                            order: 0,
                            animation: Default::default(),
                            transformation: Default::default(),
                            bounds: window.device_local_bounds(bounds),
                            clip: Default::default(),
                            color: crate::red(),
                            tile: super::cached_paint_atlas_tests::RejectOnceAtlas::tile(
                                crate::AtlasTextureKind::Monochrome,
                            ),
                            transform: PrimitiveTransform::IDENTITY,
                        });
                    },
                )
                .size(px(20.)),
            );
        }
        root
    }
}

fn options() -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("Creation title 文本".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn open(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<ActivationProbe>,
    Rc<Cell<usize>>,
    Option<Activation>,
) {
    let renders = Rc::new(Cell::new(0));
    let handle = cx.update(|app| {
        app.set_quit_mode(crate::QuitMode::Explicit);
        app.open_window(options(), |_, app| {
            app.new(|_| ActivationProbe {
                renders: renders.clone(),
                leased_paint: false,
            })
        })
        .unwrap()
    });
    cx.run_until_parked();
    let callback = cx
        .test_window(handle.into())
        .take_accessibility_activation_for_test();
    (handle, renders, Some(callback))
}

fn activate_on_thread(callback: &mut Option<Activation>) -> Option<TreeUpdate> {
    let owned = callback.take().expect("retained original activation");
    let (owned, result) = std::thread::spawn(move || {
        let result = owned();
        (owned, result)
    })
    .join()
    .expect("production callback completes without borrowing the UI thread");
    *callback = Some(owned);
    result
}

fn assert_initial(update: &TreeUpdate) {
    assert_eq!(update.nodes.len(), 1);
    assert_eq!(update.tree.as_ref().unwrap().root, NodeId(0));
    assert_eq!(update.focus, NodeId(0));
    let (id, node) = &update.nodes[0];
    assert_eq!(*id, NodeId(0));
    assert_eq!(node.role(), Role::Window);
    assert_eq!(node.label(), Some("Creation title 文本"));
    assert!(node.children().is_empty());
}

#[crate::test]
fn initial_tree_is_synchronous_while_borrowed_and_full_publication_remains_deferred(
    cx: &mut TestAppContext,
) {
    let (handle, renders, callback) = open(cx);
    cx.update_window(handle.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        let count = renders.get();
        let facts = window.presentation_facts();
        assert!(window.debug_a11y_tree_json().is_none());
        assert_initial(&callback.as_ref().unwrap()().expect("synchronous official initial tree"));
        assert_eq!(window.rendered_frame_revision(), revision);
        assert_eq!(window.presentation_facts(), facts);
        assert_eq!(renders.get(), count);
        assert!(window.debug_a11y_tree_json().is_none());
    })
    .unwrap();
    cx.run_until_parked();
    let full = cx
        .latest_accessibility_tree_update(handle.into())
        .expect("sequenced full tree");
    assert!(
        full.nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Complete control"))
    );
    cx.update_window(handle.into(), |_, window, _| {
        window.set_window_title("Live title updated");
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        assert_initial(&callback.as_ref().unwrap()().expect("repeat creation snapshot"));
        assert_eq!(window.rendered_frame_revision(), revision);
        assert!(window.debug_a11y_tree_json().is_none());
    })
    .unwrap();
    cx.run_until_parked();
    let current = cx.latest_accessibility_tree_update(handle.into()).unwrap();
    assert_eq!(
        current
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(0))
            .unwrap()
            .1
            .label(),
        Some("Live title updated")
    );
}

#[crate::test]
fn retained_activation_cannot_reopen_quiesced_closed_or_replaced_windows(cx: &mut TestAppContext) {
    let (handle, _, mut callback) = open(cx);
    cx.update_window(handle.into(), |_, window, app| {
        assert!(window.quiesce_interaction(app));
        let generation = window.a11y.activation_generation();
        assert!(activate_on_thread(&mut callback).is_none());
        window.a11y.sync_active_flag();
        assert_eq!(window.a11y.activation_generation(), generation);
        window.remove_window(app);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(activate_on_thread(&mut callback).is_none());
    let (replacement, _, _) = open(cx);
    assert_ne!(replacement.window_id(), handle.window_id());
    cx.update_window(replacement.into(), |_, window, _| {
        assert!(window.is_a11y_enabled());
        assert!(!window.is_a11y_active());
        assert!(activate_on_thread(&mut callback).is_none());
        assert!(!window.is_a11y_active());
        assert!(window.debug_a11y_tree_json().is_none());
    })
    .unwrap();
}

#[crate::test]
fn native_close_revokes_retained_activation_before_queued_logical_close(cx: &mut TestAppContext) {
    let (handle, _, mut callback) = open(cx);
    let platform = cx.test_window(handle.into());
    cx.update_window(handle.into(), |_, window, _| {
        assert!(platform.simulate_close());
        assert!(window.native_closed.load(SeqCst));
        assert!(window.interaction_authority.is_admitted());
        assert!(activate_on_thread(&mut callback).is_none());
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}

#[crate::test]
fn reserved_activation_is_allowed_but_creation_rollback_retires_the_original_callback(
    cx: &mut TestAppContext,
) {
    let mut callback = None;
    cx.update(|app| {
        app.set_quit_mode(crate::QuitMode::Explicit);
        let failure = catch_unwind(AssertUnwindSafe(|| {
            let _ = app.open_window(options(), |window, _| -> Entity<ActivationProbe> {
                callback = Some(
                    window
                        .platform_window
                        .as_test()
                        .unwrap()
                        .take_accessibility_activation_for_test(),
                );
                assert_initial(
                    &activate_on_thread(&mut callback).expect("reserved initial response"),
                );
                panic!("injected root construction failure");
            });
        }))
        .expect_err("actual root construction must fail");
        assert_eq!(
            failure.downcast_ref::<&str>(),
            Some(&"injected root construction failure")
        );
        assert!(activate_on_thread(&mut callback).is_none());
    });
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    assert!(activate_on_thread(&mut callback).is_none());
}

#[crate::test]
fn shutdown_fence_rejects_retained_activation_before_native_termination(cx: &mut TestAppContext) {
    let (_, _, callback) = open(cx);
    let callback = Rc::new(RefCell::new(callback));
    let observed = Rc::new(Cell::new(false));
    let _observer = cx.update(|app| {
        let callback = callback.clone();
        let observed = observed.clone();
        app.on_app_quit(move |_| {
            assert!(activate_on_thread(&mut callback.borrow_mut()).is_none());
            observed.set(true);
            async {}
        })
    });
    cx.update(|app| app.shutdown());
    assert!(observed.get());
    assert!(activate_on_thread(&mut callback.borrow_mut()).is_none());
}

#[crate::test]
fn initial_root_does_not_publish_a_candidate_rejected_by_the_actual_atlas(cx: &mut TestAppContext) {
    let atlas = Arc::new(super::cached_paint_atlas_tests::RejectOnceAtlas::new());
    cx.defer_next_window_frame_requests();
    let handle = cx.update(|app| {
        app.open_window(options(), |window, app| {
            window.sprite_atlas = atlas.clone();
            app.new(|_| ActivationProbe {
                renders: Rc::new(Cell::new(0)),
                leased_paint: true,
            })
        })
        .unwrap()
    });
    let platform = cx.test_window(handle.into());
    let callback = platform.take_accessibility_activation_for_test();
    cx.update_window(handle.into(), |_, window, app| {
        let revision = window.rendered_frame_revision();
        assert!(window.debug_a11y_tree_json().is_none());
        let initial = callback().expect("root response precedes the refused candidate");
        assert_initial(&initial);
        atlas.fail_next_lease();
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), revision);
        let rejection = window
            .last_atlas_frame_rejection
            .as_ref()
            .expect("actual atlas refusal");
        assert_eq!(rejection.generation, revision + 1);
        assert!(matches!(
            &rejection.error,
            AtlasTextureLeaseError::TextureUnavailable { .. }
        ));
        assert!(window.debug_a11y_tree_json().is_none());
        assert_eq!(window.a11y.published_revision_for_test(), None);
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), revision + 1);
        let json = window
            .debug_a11y_tree_json()
            .expect("only accepted retry publishes inspection");
        let snapshot: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot["frame"]["frame_generation"], revision + 1);
        assert!(window.a11y.published_revision_for_test().is_some());
    })
    .unwrap();
    let full = platform
        .latest_accessibility_tree_update()
        .expect("accepted complete tree");
    assert!(
        full.nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Complete control"))
    );
}
