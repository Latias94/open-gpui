use super::*;
use crate::{
    Corners, PlatformWindowPresentOutcome, Point, RenderImage, SubtreeClip, SubtreeClipExt as _,
    SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, img, point,
    window::cached_paint_atlas_tests::RejectOnceAtlas,
};
use std::{sync::Arc, time::Duration};

#[crate::test]
fn every_mutation_kind_matches_a_fresh_render_with_and_without_cached_views(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| cx.set_quit_mode(crate::QuitMode::Explicit));
    for cached in [false, true] {
        for (index, kind) in RandomizedElementTreeMutationKind::ALL
            .into_iter()
            .enumerate()
        {
            let snapshot = controlled_snapshot(cached);
            let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
                RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
            });
            let root = window.root(cx).expect("the mutation fixture has a root");
            assert_current_matches(&root, window.into(), cx, "before selected mutation");
            let before = root.read_with(cx, |tree, _| tree.snapshot());
            let mutation = cx.update(|cx| {
                root.update(cx, |tree, cx| {
                    tree.rng = StdRng::seed_from_u64(index as u64 + 100);
                    tree.apply_mutation(kind, cx)
                })
            });
            let after = root.read_with(cx, |tree, _| tree.snapshot());
            assert_requested_mutation(kind, mutation);
            assert_ne!(before, after, "{kind:?} must change the {cached:?} fixture");
            match kind {
                RandomizedElementTreeMutationKind::Insert => {
                    assert_eq!(after.descendant_count(), before.descendant_count() + 1);
                    assert_eq!(after.next_element_id, before.next_element_id + 1);
                }
                RandomizedElementTreeMutationKind::RemoveLeaf => {
                    assert_eq!(after.descendant_count() + 1, before.descendant_count());
                    assert!(matches!(
                        mutation,
                        RandomizedElementTreeMutation::Removed {
                            removed_element_count: 1,
                            ..
                        }
                    ));
                }
                RandomizedElementTreeMutationKind::Remove => {
                    let RandomizedElementTreeMutation::Removed {
                        removed_element_count,
                        ..
                    } = mutation
                    else {
                        unreachable!();
                    };
                    assert_eq!(
                        after.descendant_count() + removed_element_count,
                        before.descendant_count()
                    );
                }
                _ => assert_eq!(after.descendant_count(), before.descendant_count()),
            }
            assert_current_matches(
                &root,
                window.into(),
                cx,
                &format!("cached={cached}, kind={kind:?}, mutation={mutation:?}"),
            );
            close_window(window.into(), cx);
        }
    }
}

fn assert_requested_mutation(
    kind: RandomizedElementTreeMutationKind,
    mutation: RandomizedElementTreeMutation,
) {
    use RandomizedElementTreeMutation as Mutation;
    use RandomizedElementTreeMutationKind as Kind;
    assert!(
        matches!(
            (kind, mutation),
            (Kind::RootColor, Mutation::RootColor)
                | (Kind::RootOpacity, Mutation::RootOpacity)
                | (Kind::RootBorder, Mutation::RootBorder)
                | (Kind::RootBounds, Mutation::RootBounds)
                | (Kind::RootClipping, Mutation::RootClipping)
                | (Kind::RootVisibility, Mutation::RootVisibility)
                | (Kind::ChildColor, Mutation::ChildColor { .. })
                | (Kind::ChildOpacity, Mutation::ChildOpacity { .. })
                | (Kind::ChildBorder, Mutation::ChildBorder { .. })
                | (Kind::ChildBounds, Mutation::ChildBounds { .. })
                | (Kind::ChildClipping, Mutation::ChildClipping { .. })
                | (Kind::ChildVisibility, Mutation::ChildVisibility { .. })
                | (Kind::ChildText, Mutation::ChildText { .. })
                | (Kind::Reorder, Mutation::Reordered { .. })
                | (Kind::Insert, Mutation::Inserted { .. })
                | (Kind::Remove | Kind::RemoveLeaf, Mutation::Removed { .. })
        ),
        "requested {kind:?}, applied {mutation:?}"
    );
}

#[crate::test]
fn cached_boundaries_match_fresh_after_zero_size_reorder_hide_and_restore(cx: &mut TestAppContext) {
    let snapshot = controlled_snapshot(true);
    let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
        RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
    });
    let root = window.root(cx).expect("the cached fixture has a root");
    let original = root.read_with(cx, |tree, _| tree.snapshot());
    let entity_ids = root.read_with(cx, |tree, _| {
        tree.entities
            .borrow()
            .iter()
            .map(|(id, entity)| (*id, entity.entity_id()))
            .collect::<std::collections::BTreeMap<_, _>>()
    });
    assert_current_matches(&root, window.into(), cx, "initial cached frame");
    cx.update(|cx| {
        root.update(cx, |tree, cx| {
            tree.reset_work_counters();
            cx.notify();
        });
    });
    assert_eq!(
        root.read_with(cx, |tree, _| tree.work_counters().entity_render_count()),
        0,
        "an unchanged root notification must reuse cached child boundaries"
    );
    assert_current_matches(&root, window.into(), cx, "replayed cached frame");

    for (width, height) in [(0.0, 160.0), (200.0, 0.0), (0.0, 0.0), (200.0, 160.0)] {
        update_node(&root, 0, cx, |node| {
            node.style.width = width;
            node.style.height = height;
        });
        assert_current_matches(
            &root,
            window.into(),
            cx,
            &format!("cached boundary resized to {width} x {height}"),
        );
    }

    let mutation = cx.update(|cx| {
        root.update(cx, |tree, cx| {
            tree.apply_mutation(RandomizedElementTreeMutationKind::Reorder, cx)
        })
    });
    assert_requested_mutation(RandomizedElementTreeMutationKind::Reorder, mutation);
    assert_current_matches(&root, window.into(), cx, "reordered cached boundaries");

    for visible in [false, true] {
        update_node(&root, 0, cx, |node| node.style.visible = visible);
        assert_current_matches(
            &root,
            window.into(),
            cx,
            &format!("cached descendant visibility={visible}"),
        );
    }

    for (width, height, visible) in [
        (0.0, 700.0, true),
        (1000.0, 0.0, true),
        (1000.0, 700.0, true),
        (1000.0, 700.0, false),
        (1000.0, 700.0, true),
    ] {
        cx.update(|cx| {
            root.update(cx, |tree, cx| {
                {
                    let mut snapshot = tree.snapshot.borrow_mut();
                    snapshot.root.width = width;
                    snapshot.root.height = height;
                    snapshot.root.visible = visible;
                }
                cx.notify();
            });
        });
        assert_current_matches(
            &root,
            window.into(),
            cx,
            &format!("cached root {width} x {height}, visible={visible}"),
        );
    }

    root.read_with(cx, |tree, _| {
        let entities = tree.entities.borrow();
        for (id, entity_id) in entity_ids {
            assert_eq!(entities[&id].entity_id(), entity_id);
        }
        let snapshot = tree.snapshot();
        assert_eq!(snapshot.root, original.root);
        assert_eq!(
            snapshot.node(0).unwrap().style,
            original.node(0).unwrap().style
        );
    });
    close_window(window.into(), cx);
}

#[crate::test]
fn removing_a_cached_subtree_releases_entities_and_owned_scroll_entries(cx: &mut TestAppContext) {
    let snapshot = controlled_snapshot(true);
    let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
        RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
    });
    let root = window.root(cx).expect("the deletion fixture has a root");
    let removed_ids = BTreeSet::from([0, 6, 7]);
    let (removed_entities, surviving_entities, scroll_table) = root.read_with(cx, |tree, _| {
        let entities = tree.entities.borrow();
        let removed = removed_ids
            .iter()
            .map(|id| entities[id].downgrade())
            .collect::<Vec<_>>();
        let survivors = entities
            .iter()
            .filter(|(id, _)| !removed_ids.contains(*id))
            .map(|(id, entity)| (*id, entity.entity_id()))
            .collect::<Vec<_>>();
        let scrolls = tree.scroll_handles.borrow();
        assert!(scrolls.contains_key(&0));
        assert!(scrolls.contains_key(&6));
        (removed, survivors, Rc::downgrade(&tree.scroll_handles))
    });
    let mutation = cx.update(|cx| {
        root.update(cx, |tree, cx| {
            let snapshot = tree.snapshot();
            let seed = (0..1024)
                .find(|seed| {
                    random_node_path(&snapshot.children, &mut StdRng::seed_from_u64(*seed))
                        == Some(vec![0])
                })
                .expect("a bounded seed search must select the first subtree");
            tree.rng = StdRng::seed_from_u64(seed);
            tree.apply_mutation(RandomizedElementTreeMutationKind::Remove, cx)
        })
    });
    assert!(matches!(
        mutation,
        RandomizedElementTreeMutation::Removed {
            element_id: 0,
            removed_element_count: 3,
            parent_id: None,
            index: 0,
        }
    ));
    assert_current_matches(&root, window.into(), cx, "removed cached subtree");
    draw_window(window.into(), cx);
    cx.run_until_parked();
    root.read_with(cx, |tree, _| {
        let snapshot = tree.snapshot();
        let entities = tree.entities.borrow();
        let scrolls = tree.scroll_handles.borrow();
        for id in &removed_ids {
            assert!(snapshot.node(*id).is_none());
            assert!(!entities.contains_key(id));
            assert!(!scrolls.contains_key(id));
        }
        assert!(
            scrolls.contains_key(&3),
            "a surviving scroll owner stays live"
        );
        for (id, entity_id) in surviving_entities {
            assert_eq!(entities[&id].entity_id(), entity_id);
        }
    });
    assert!(
        removed_entities
            .iter()
            .all(|entity| entity.upgrade().is_none())
    );
    let weak_root = root.downgrade();
    close_window(window.into(), cx);
    // Entity values release their owned resources at the end of the App effect cycle.
    cx.update(|_| drop(root));
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    assert!(scroll_table.upgrade().is_none());
}

fn controlled_snapshot(cached: bool) -> RandomizedElementTreeSnapshot {
    let config = RandomizedElementTreeConfig::new(0, 8)
        .with_topology(RandomizedElementTreeTopology::Wide)
        .with_entity_density(1.0)
        .with_handler_density(1.0)
        .with_cached_views(cached);
    let mut snapshot = RandomizedElementTree::from_config(config).snapshot();
    snapshot.root.width = 1000.0;
    snapshot.root.height = 700.0;
    snapshot.root.opacity = 1.0;
    snapshot.root.padding = 12.0;
    snapshot.root.gap = 12.0;
    snapshot.root.border_width = 2.0;
    snapshot.root.corner_radius = 24.0;
    snapshot.root.clipped = true;
    snapshot.root.visible = true;
    snapshot.root.vertical = false;
    snapshot.root.wrap = true;
    for node in &mut snapshot.children {
        node.style.width = 200.0;
        node.style.height = 160.0;
        node.style.padding = 8.0;
        node.style.gap = 4.0;
        node.style.opacity = 1.0;
        node.style.border_width = 2.0;
        node.style.corner_radius = 16.0;
        node.style.visible = true;
        node.style.clipped = true;
        node.style.vertical = true;
        node.style.wrap = false;
    }
    let children = snapshot.children.split_off(6);
    snapshot.children[0].children = children;
    snapshot.rebuild_element_paths();
    snapshot
}

fn update_node(
    root: &Entity<RandomizedElementTree>,
    element_id: u64,
    cx: &TestAppContext,
    update: impl FnOnce(&mut RandomizedElementNode),
) {
    cx.update(|cx| {
        root.update(cx, |tree, cx| {
            let owner = {
                let mut snapshot = tree.snapshot.borrow_mut();
                let path = snapshot.paths_by_element_id[&element_id].clone();
                let owner = nearest_entity_id(&snapshot.children, &path);
                update(node_at_path_mut(&mut snapshot.children, &path).unwrap());
                owner
            };
            tree.notify_owner(owner, cx);
        });
    });
}

fn accepted_scene(window: AnyWindowHandle, cx: &mut TestAppContext) -> SceneSnapshot {
    cx.update_window(window, |_, window, _| SceneSnapshot::from_window(window))
        .expect("the fixture must retain its accepted frame")
}

fn settle_requested_frames(window: AnyWindowHandle, cx: &mut TestAppContext) {
    for _ in 0..4 {
        let callbacks = cx
            .update_window(window, |_, window, cx| {
                window.drain_next_frame_callbacks_for_test(cx)
            })
            .expect("the static fixture must remain open while settling inherited style");
        cx.run_until_parked();
        if callbacks == 0 {
            return;
        }
    }
    panic!("a static randomized tree must settle within four requested frames");
}

fn assert_current_matches(
    root: &Entity<RandomizedElementTree>,
    window: AnyWindowHandle,
    cx: &mut TestAppContext,
    context: &str,
) {
    settle_requested_frames(window, cx);
    let snapshot = root.read_with(cx, |tree, _| tree.snapshot());
    let actual = accepted_scene(window, cx);
    let expected = draw_fresh_reference(snapshot, cx);
    assert_eq!(actual, expected, "{context}");
}

#[crate::test]
fn cached_hover_styles_follow_explicit_pointer_inputs(cx: &mut TestAppContext) {
    let mut snapshot = controlled_snapshot(true);
    snapshot.children.truncate(1);
    snapshot.children[0].children.clear();
    snapshot.hover_styles = true;
    snapshot.rebuild_element_paths();
    let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
        RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
    });
    let root = window.root(cx).expect("the hover fixture has a root");
    let outside = point(px(1100.0), px(800.0));
    let inside = point(px(50.0), px(50.0));
    let mut baseline = None;
    for position in [outside, inside, outside] {
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_mouse_move(position, cx);
        })
        .expect("dispatch the persistent pointer input");
        settle_requested_frames(window.into(), cx);
        let actual = accepted_scene(window.into(), cx);
        let snapshot = root.read_with(cx, |tree, _| tree.snapshot());
        let reference = cx.open_window(WINDOW_SIZE, move |_, cx| {
            RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
        });
        cx.update_window(reference.into(), |_, window, cx| {
            window.simulate_mouse_move(position, cx);
        })
        .expect("dispatch the identical fresh-reference pointer input");
        settle_requested_frames(reference.into(), cx);
        assert_eq!(actual, accepted_scene(reference.into(), cx));
        close_window(reference.into(), cx);
        if let Some(baseline) = &baseline {
            if position == inside {
                assert_ne!(&actual, baseline, "hover must change the accepted drawing");
            } else {
                assert_eq!(
                    &actual, baseline,
                    "leaving must restore the accepted drawing"
                );
            }
        } else {
            baseline = Some(actual);
        }
    }
    close_window(window.into(), cx);
}

#[crate::test]
fn cached_animation_uses_the_explicit_clock_and_retires_with_its_window(cx: &mut TestAppContext) {
    let mut snapshot = controlled_snapshot(true);
    snapshot.children.truncate(1);
    snapshot.children[0].children.clear();
    snapshot.children[0].features.animated = true;
    snapshot.rebuild_element_paths();
    cx.defer_next_window_frame_requests();
    let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
        RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
    });
    let root = window.root(cx).expect("the animation fixture has a root");
    let child = root.read_with(cx, |tree, _| tree.entities.borrow()[&0].downgrade());
    let spinner_alpha = |window: &Window| {
        let spinner_size = size(px(10.0), px(10.0)).scale(window.scale_factor());
        window
            .rendered_frame
            .scene
            .quads
            .iter()
            .find(|quad| quad.bounds.size == spinner_size)
            .expect("the animated entity must paint its ten-pixel spinner")
            .background
            .solid
            .a
    };
    let initial = cx
        .update_window(window.into(), |_, window, _| spinner_alpha(window))
        .unwrap();
    assert!((initial - 0.25).abs() < 0.01, "initial alpha={initial}");
    let initial_renders = root.read_with(cx, |tree, _| tree.work_counters().entity_render_count());
    cx.executor().advance_clock(Duration::from_millis(200));
    let callbacks = cx
        .update_window(window.into(), |_, window, cx| {
            window.drain_next_frame_callbacks_for_test(cx)
        })
        .unwrap();
    assert!(
        callbacks > 0,
        "the accepted spinner must schedule its next frame"
    );
    cx.run_until_parked();
    let advanced = cx
        .update_window(window.into(), |_, window, _| spinner_alpha(window))
        .unwrap();
    assert!((advanced - 0.4375).abs() < 0.01, "200ms alpha={advanced}");
    assert!(
        root.read_with(cx, |tree, _| tree.work_counters().entity_render_count()) > initial_renders,
        "the animation callback must invalidate its cached entity boundary"
    );
    let weak_root = root.downgrade();
    close_window(window.into(), cx);
    cx.update(|_| drop(root));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    assert!(child.upgrade().is_none());
}

#[crate::test]
fn cached_focus_survives_replay_and_unrelated_entity_mutation(cx: &mut TestAppContext) {
    let snapshot = controlled_snapshot(true);
    let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
        RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
    });
    let root = window.root(cx).expect("the focus fixture has a root");
    cx.test_window(window.into())
        .simulate_active_status_change(true);
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| window.focus_next(cx))
        .expect("focus an actual randomized tab stop");
    let focused = cx
        .update_window(window.into(), |_, window, cx| {
            window
                .focused(cx)
                .expect("tab navigation must claim a live focus handle")
                .id
        })
        .unwrap();
    for notify_only in [true, false] {
        cx.update(|cx| {
            root.update(cx, |tree, cx| {
                if notify_only {
                    cx.notify();
                } else {
                    assert!(tree.alternate_element_color(5, cx));
                }
            });
        });
        settle_requested_frames(window.into(), cx);
        cx.update_window(window.into(), |_, window, cx| {
            assert_eq!(window.focused(cx).map(|focus| focus.id), Some(focused));
            assert_eq!(window.rendered_frame.focus, Some(focused));
        })
        .unwrap();
    }
    close_window(window.into(), cx);
}

#[crate::test]
fn cached_scroll_uses_explicit_offsets_in_both_persistent_and_fresh_trees(cx: &mut TestAppContext) {
    let snapshot = controlled_snapshot(true);
    let window = cx.open_window(WINDOW_SIZE, move |_, cx| {
        RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
    });
    let root = window.root(cx).expect("the scroll fixture has a root");
    let handle = root.read_with(cx, |tree, _| tree.scroll_handles.borrow()[&0].clone());
    assert!(
        handle.max_offset().y > Pixels::ZERO,
        "the fixture must actually scroll"
    );
    let offset = point(Pixels::ZERO, px(-24.0));
    for reorder in [false, true] {
        cx.update(|cx| {
            root.update(cx, |tree, cx| {
                if reorder {
                    assert_requested_mutation(
                        RandomizedElementTreeMutationKind::Reorder,
                        tree.apply_mutation(RandomizedElementTreeMutationKind::Reorder, cx),
                    );
                }
                tree.scroll_handles.borrow()[&0].set_offset(offset);
                tree.notify_owner(Some(0), cx);
            });
        });
        settle_requested_frames(window.into(), cx);
        assert_eq!(handle.offset(), offset);
        assert_eq!(
            root.read_with(cx, |tree, _| tree.scroll_handles.borrow()[&0].offset()),
            offset
        );
        let snapshot = root.read_with(cx, |tree, _| tree.snapshot());
        let reference = cx.open_window(WINDOW_SIZE, move |_, cx| {
            RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)
        });
        let reference_root = reference.root(cx).unwrap();
        cx.update(|cx| {
            reference_root.update(cx, |tree, cx| {
                tree.scroll_handles.borrow()[&0].set_offset(offset);
                tree.notify_owner(Some(0), cx);
            });
        });
        settle_requested_frames(reference.into(), cx);
        assert_eq!(
            accepted_scene(window.into(), cx),
            accepted_scene(reference.into(), cx)
        );
        close_window(reference.into(), cx);
    }
    close_window(window.into(), cx);
}

struct AdmissionRoot {
    tree: Entity<RandomizedElementTree>,
    image: Arc<RenderImage>,
}

impl Render for AdmissionRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.tree.clone())
            .child(img(self.image.clone()).w(px(8.0)).h(px(8.0)))
    }
}

#[crate::test]
fn refused_cached_candidate_retains_accepted_frame_then_retries_against_fresh_reference(
    cx: &mut TestAppContext,
) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let snapshot = controlled_snapshot(true);
    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image::ImageBuffer::from_pixel(1, 1, image::Rgba([30, 60, 90, 255])),
    )]));
    let root = cx.new(|cx| RandomizedElementTree::from_snapshot_with_entities(snapshot, cx));
    let window = cx.open_window(WINDOW_SIZE, {
        let root = root.clone();
        let atlas = atlas.clone();
        let image = image.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            AdmissionRoot { tree: root, image }
        }
    });
    cx.test_window(window.into())
        .defer_frame_requests_for_test();
    let accepted = accepted_scene(window.into(), cx);
    let (retry, snapshot) = cx
        .update_window(window.into(), |_, window, cx| {
            let generation = window.rendered_frame_revision();
            let facts = window.presentation_facts();
            let dependencies = cx.tracked_entities[&window.window_handle().window_id()].clone();
            root.update(cx, |tree, cx| {
                tree.apply_mutation(RandomizedElementTreeMutationKind::ChildColor, cx);
                tree.apply_mutation(RandomizedElementTreeMutationKind::Reorder, cx);
            });
            atlas.fail_next_lease();
            window.refresh();
            window.draw(cx).clear(cx);
            assert_eq!(window.rendered_frame_revision(), generation);
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                facts.frame_accepted_generation
            );
            assert_eq!(SceneSnapshot::from_window(window), accepted);
            assert_eq!(
                cx.tracked_entities[&window.window_handle().window_id()],
                dependencies
            );
            assert!(window.refresh_pending_for_test());
            assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
            assert_eq!(
                window.presentation_facts().present_submitted_generation,
                Some(generation)
            );
            window.draw(cx).clear(cx);
            assert_eq!(window.rendered_frame_revision(), generation + 1);
            assert!(!window.refresh_pending_for_test());
            assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
            assert_eq!(
                window.presentation_facts().present_submitted_generation,
                Some(generation + 1)
            );
            (SceneSnapshot::from_window(window), root.read(cx).snapshot())
        })
        .expect("the actual lease admission must refuse one candidate and accept its retry");
    assert_ne!(
        retry, accepted,
        "the accepted retry must contain the requested mutation"
    );
    let reference = cx.open_window(WINDOW_SIZE, move |window, cx| {
        window.set_sprite_atlas_for_test(Arc::new(RejectOnceAtlas::new()));
        AdmissionRoot {
            tree: cx.new(|cx| RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)),
            image,
        }
    });
    assert_eq!(retry, accepted_scene(reference.into(), cx));
    close_window(reference.into(), cx);
    close_window(window.into(), cx);
}

struct GeometryRoot {
    tree: Entity<RandomizedElementTree>,
    radius: f32,
    translation: Point<Pixels>,
}

impl Render for GeometryRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clip = |radius| {
            SubtreeClip::try_own_rounded_border_box(Corners {
                top_left: size(px(radius), px(radius / 2.0)),
                top_right: size(px(radius / 2.0), px(radius)),
                bottom_right: size(px(radius), px(radius)),
                bottom_left: size(px(radius / 2.0), px(radius / 2.0)),
            })
            .unwrap()
        };
        div()
            .w(px(1000.0))
            .h(px(700.0))
            .child(
                div()
                    .w(px(900.0))
                    .h(px(600.0))
                    .child(self.tree.clone())
                    .with_subtree_clip(clip(self.radius / 2.0)),
            )
            .with_subtree_clip(clip(self.radius))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(0.85, 1.1),
                    self.translation,
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

#[crate::test]
fn cached_scene_oracle_preserves_nested_rounded_clips_and_transforms(cx: &mut TestAppContext) {
    let snapshot = controlled_snapshot(true);
    let tree = cx.new(|cx| RandomizedElementTree::from_snapshot_with_entities(snapshot, cx));
    let window = cx.open_window(WINDOW_SIZE, |_, _| GeometryRoot {
        tree: tree.clone(),
        radius: 36.0,
        translation: point(px(10.5), px(20.25)),
    });
    let mut previous = None;
    for (radius, translation) in [
        (36.0, point(px(10.5), px(20.25))),
        (18.0, point(px(10.5), px(20.25))),
        (18.0, point(px(23.5), px(33.25))),
    ] {
        window
            .update(cx, |root, _, cx| {
                root.radius = radius;
                root.translation = translation;
                cx.notify();
            })
            .unwrap();
        settle_requested_frames(window.into(), cx);
        cx.update_window(window.into(), |_, window, _| {
            let scene = &window.rendered_frame.scene;
            assert!(
                scene
                    .clip_shapes()
                    .iter()
                    .any(|shape| shape.radii_x.top_left.0 > 0.0)
            );
            assert!(scene.quads.iter().any(|quad| quad.clip.clip_count >= 2));
            assert!(
                scene
                    .quads
                    .iter()
                    .any(|quad| { quad.transform != crate::PrimitiveTransform::IDENTITY })
            );
        })
        .unwrap();
        let actual = accepted_scene(window.into(), cx);
        let snapshot = tree.read_with(cx, |tree, _| tree.snapshot());
        let reference = cx.open_window(WINDOW_SIZE, move |_, cx| GeometryRoot {
            tree: cx.new(|cx| RandomizedElementTree::from_snapshot_with_entities(snapshot, cx)),
            radius,
            translation,
        });
        assert_eq!(actual, accepted_scene(reference.into(), cx));
        close_window(reference.into(), cx);
        if let Some(previous) = &previous {
            assert_ne!(
                &actual, previous,
                "the oracle must observe exact shape and transform changes"
            );
        }
        previous = Some(actual);
    }
    close_window(window.into(), cx);
}
