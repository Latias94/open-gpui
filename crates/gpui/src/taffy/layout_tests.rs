use super::*;
use crate::{AppContext as _, Empty, TestAppContext, fast::layout_key::LayoutKeys, px, relative};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn measured_leaf_applies_padding_border_and_logical_content_constraints_at_each_scale() {
    let mut app = TestAppContext::single();
    let handle = app.open_window(size(px(320.), px(200.)), |_, _| Empty);
    for (scale, expected_content_width, expected_height) in [
        (1., 110., 30.),
        (1.5, 110.666_664, 29.333_334),
        (2., 110., 30.),
    ] {
        app.set_platform_window_physical_client_geometry(handle.into(), None, scale);
        let mut platform_window = app
            .update_window(handle.into(), |_, window, _| {
                window.platform_window.as_test().unwrap().clone()
            })
            .unwrap();
        platform_window.simulate_resize(size(px(320.), px(200.)));
        app.run_until_parked();
        app.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.scale_factor(), scale);
            let observations = Rc::new(RefCell::new(Vec::new()));
            let observed = observations.clone();
            let mut engine = TaffyLayoutEngine::new();
            let node = engine.request_measured_layout(
                Style {
                    size: size(px(120.).into(), Length::Auto),
                    padding: Edges::all(px(4.).into()),
                    border_widths: Edges::all(px(1.).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                move |known, available, _, _| {
                    observed.borrow_mut().push((known, available));
                    let width = known.width.unwrap_or(match available.width {
                        AvailableSpace::Definite(width) => width,
                        _ => px(80.),
                    });
                    size(width, px(20.))
                },
            );
            engine.compute_layout(
                node,
                size(AvailableSpace::Definite(px(160.)), AvailableSpace::MaxContent),
                window,
                cx,
            );
            let actual = engine.layout_bounds(node, scale);
            assert_eq!(actual.size.width, px(120.));
            assert!((actual.size.height - px(expected_height)).abs() < px(0.001));
            let observations = observations.borrow();
            assert!(!observations.is_empty());
            assert!(observations.iter().any(|(known, available)| {
                known.width.is_none()
                    && matches!(available.width, AvailableSpace::Definite(width)
                        if (width - px(expected_content_width)).abs() < px(0.001))
            }), "{scale}: {observations:?}");
            assert!(observations.iter().all(|(_, available)| {
                available.width == AvailableSpace::MaxContent
                    || matches!(available.width, AvailableSpace::Definite(width) if width <= px(160.))
            }));
        })
        .unwrap();
    }
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))
        .unwrap();
}

#[test]
fn remeasurement_changes_constraints_without_reusing_an_old_leaf_size() {
    let mut app = TestAppContext::single();
    let handle = app.open_window(size(px(320.), px(200.)), |_, _| Empty);
    app.update_window(handle.into(), |_, window, cx| {
        let scale = window.scale_factor();
        let observations = Rc::new(RefCell::new(Vec::new()));
        let observed = observations.clone();
        let mut engine = TaffyLayoutEngine::new();
        let node = engine.request_measured_layout(
            Style::default(),
            px(16.),
            scale,
            move |known, available, _, _| {
                observed.borrow_mut().push(available.width);
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::MinContent => px(25.),
                    AvailableSpace::MaxContent => px(100.),
                    AvailableSpace::Definite(width) => width.min(px(100.)),
                });
                size(width, px(20.))
            },
        );
        for (constraint, expected) in [
            (AvailableSpace::Definite(px(40.)), px(40.)),
            (AvailableSpace::MaxContent, px(100.)),
            (AvailableSpace::MinContent, px(25.)),
            (AvailableSpace::Definite(px(80.)), px(80.)),
            (AvailableSpace::MaxContent, px(100.)),
        ] {
            engine.compute_layout(
                node,
                size(constraint, AvailableSpace::MaxContent),
                window,
                cx,
            );
            assert_eq!(
                engine.layout_bounds(node, scale).size,
                size(expected, px(20.))
            );
        }
        let observations = observations.borrow();
        for expected in [AvailableSpace::MinContent, AvailableSpace::MaxContent] {
            assert!(observations.contains(&expected));
        }
        window.remove_window(cx);
    })
    .unwrap();
}

fn with_layout_window(test: impl FnOnce(&mut Window, &mut App)) {
    let mut app = TestAppContext::single();
    let window = app.open_window(size(px(320.), px(200.)), |_, _| Empty);
    app.update_window(window.into(), |_, window, cx| {
        test(window, cx);
        window.remove_window(cx);
    })
    .unwrap();
}

#[test]
fn retained_subtree_claims_descendants_once_and_expires_orphans() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let child_key = keys.next_request();
        let root_key = keys.next_request();
        let scale = window.scale_factor();
        let request = |engine: &mut TaffyLayoutEngine, key, children: &[LayoutId]| {
            engine.request_layout_keyed(key, Style::default(), px(16.), scale, children)
        };

        engine.begin_retained_frame(true);
        let child = request(&mut engine, child_key, &[]);
        let duplicate = request(&mut engine, child_key, &[]);
        assert_ne!(child, duplicate);
        let root = request(&mut engine, root_key, &[child, duplicate]);
        engine.compute_layout(root, size(px(160.), px(80.)).into(), window, cx);
        engine.finish_retained_frame();

        engine.begin_retained_frame(true);
        let revision = engine.layout_revision();
        assert!(engine.try_retain_subtree(root));
        assert!(!engine.try_retain_subtree(root));
        assert_eq!(engine.layout_revision(), revision);
        let unrelated = request(&mut engine, child_key, &[]);
        assert_ne!(unrelated, child);
        assert_ne!(unrelated, duplicate);
        assert_ne!(engine.layout_revision(), revision);
        engine.finish_retained_frame();

        engine.begin_retained_frame(true);
        assert_eq!(request(&mut engine, child_key, &[]), child);
        assert!(!engine.try_retain_subtree(root));
        assert_eq!(request(&mut engine, root_key, &[child]), root);
        engine.compute_layout(root, size(px(160.), px(80.)).into(), window, cx);
        engine.finish_retained_frame();
        assert_eq!(engine.taffy.total_node_count(), 2);

        engine.begin_retained_frame(false);
        assert!(!engine.try_retain_subtree(root));
        assert_eq!(engine.taffy.total_node_count(), 0);
        let fresh = request(&mut engine, root_key, &[]);
        assert_ne!(fresh, root);
        engine.finish_retained_frame();
        assert_eq!(engine.taffy.total_node_count(), 0);
    });
}

#[test]
fn parent_changes_use_current_children_and_recompute_logical_bounds() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let child_key = keys.next_request();
        let first_root_key = keys.next_request();
        let second_root_key = keys.next_request();
        let scale = window.scale_factor();
        let request_child = |engine: &mut TaffyLayoutEngine| {
            engine.request_layout_keyed(
                child_key,
                Style {
                    size: size(px(20.).into(), px(12.).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                &[],
            )
        };
        let request_root = |engine: &mut TaffyLayoutEngine, key, padding, child| {
            engine.request_layout_keyed(
                key,
                Style {
                    padding: Edges::all(px(padding).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                &[child],
            )
        };
        let available = size(px(100.), px(80.)).into();
        engine.begin_retained_frame(true);
        let child = request_child(&mut engine);
        let root = request_root(&mut engine, first_root_key, 3., child);
        engine.stretch_auto_size_to_fill(root, size(px(100.), px(80.)), scale);
        engine.compute_layout(root, available, window, cx);
        assert_eq!(engine.layout_bounds(child, scale).origin.x, px(3.));
        engine.finish_retained_frame();

        engine.begin_retained_frame(true);
        assert_eq!(request_child(&mut engine), child);
        let second = request_root(&mut engine, second_root_key, 7., child);
        assert!(!engine.try_retain_subtree(root));
        engine.compute_layout(second, available, window, cx);
        let actual = engine.layout_bounds(child, scale);
        assert_eq!(actual.origin.x, px(7.));
        assert_eq!(actual.size, size(px(20.), px(12.)));
        assert_eq!(engine.layout_bounds(child, scale * 2.).origin.x, px(3.5));
        engine.finish_retained_frame();

        engine.begin_retained_frame(true);
        assert!(!engine.try_retain_subtree(root));
        assert!(engine.try_retain_subtree(second));
        engine.clear();
        engine.begin_retained_frame(true);
        assert!(!engine.try_retain_subtree(second));
    });
}

#[test]
fn cached_measurements_restore_fresh_state_and_remeasure_changed_constraints() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let key = keys.next_request();
        let scale = window.scale_factor();
        let work = Rc::new(Cell::new(0));
        let callback_owner = Rc::new(());
        let mut previous = None;

        for (width, rem, revision, expected_work) in [
            (70., 16., 1, 1),
            (70., 16., 1, 1),
            (40., 16., 1, 2),
            (40., 18., 1, 3),
            (40., 18., 2, 4),
        ] {
            engine.begin_retained_frame(true);
            let state = Rc::new(Cell::new(None));
            let node = engine.request_cached_measured_layout(
                key,
                Style::default(),
                px(rem),
                scale,
                revision,
                {
                    let work = work.clone();
                    let callback_owner = callback_owner.clone();
                    move |_, available, _, _| {
                        let _owner = &callback_owner;
                        work.set(work.get() + 1);
                        let AvailableSpace::Definite(width) = available.width else {
                            panic!("expected the test's definite width");
                        };
                        (size(width, px(15.)), width)
                    }
                },
                {
                    let state = state.clone();
                    move |width| state.set(Some(*width))
                },
            );
            if let Some(previous) = previous.replace(node) {
                assert_eq!(node, previous);
            }
            engine.compute_layout(
                node,
                size(
                    AvailableSpace::Definite(px(width)),
                    AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            assert_eq!(state.get(), Some(px(width)));
            assert_eq!(
                engine.layout_bounds(node, scale).size,
                size(px(width), px(15.))
            );
            assert_eq!(work.get(), expected_work);
            engine.finish_retained_frame();
            assert_eq!(Rc::strong_count(&callback_owner), 1);
            assert_eq!(Rc::strong_count(&state), 1);
        }
    });
}

#[test]
fn dynamic_measurements_install_this_frames_closure() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let key = keys.next_request();
        let scale = window.scale_factor();
        let mut previous = None;
        for width in [px(20.), px(45.)] {
            engine.begin_retained_frame(true);
            let node = engine.request_measured_layout_keyed(
                key,
                Style::default(),
                px(16.),
                scale,
                move |_, _, _, _| size(width, px(15.)),
            );
            if let Some(previous) = previous.replace(node) {
                assert_eq!(previous, node);
            }
            engine.compute_layout(
                node,
                size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                window,
                cx,
            );
            assert_eq!(engine.layout_bounds(node, scale).size.width, width);
            engine.finish_retained_frame();
        }
    });
}

#[test]
fn repeated_root_fill_keeps_revision_until_the_viewport_changes() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let child_key = keys.next_request();
        let root_key = keys.next_request();
        let scale = window.scale_factor();
        let mut previous = None;
        for (width, changed) in [(100., true), (100., false), (80., true), (80., false)] {
            engine.begin_retained_frame(true);
            let revision = engine.layout_revision();
            let child = engine.request_layout_keyed(
                child_key,
                Style {
                    size: size(relative(0.5).into(), px(12.).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                &[],
            );
            let root =
                engine.request_layout_keyed(root_key, Style::default(), px(16.), scale, &[child]);
            if let Some(previous) = previous.replace((root, child)) {
                assert_eq!(previous, (root, child));
            }
            let viewport = size(px(width), px(80.));
            engine.stretch_auto_size_to_fill(root, viewport, scale);
            assert_eq!(engine.layout_revision() != revision, changed);
            engine.compute_layout(root, viewport.into(), window, cx);
            assert_eq!(
                engine.layout_bounds(child, scale).size.width,
                px(width / 2.)
            );
            engine.finish_retained_frame();
        }
    });
}

#[test]
fn retained_measurement_matches_fresh_layout_under_flex_parent_constraints() {
    with_layout_window(|window, cx| {
        let mut retained = TaffyLayoutEngine::new();
        let mut fresh = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let text_key = keys.next_request();
        let parent_key = keys.next_request();
        let scale = window.scale_factor();
        let mut layout = |engine: &mut TaffyLayoutEngine, reuse, width| {
            engine.begin_retained_frame(reuse);
            let state = Rc::new(Cell::new(None));
            let text = engine.request_cached_measured_layout(
                text_key,
                Style {
                    flex_grow: 1.,
                    flex_basis: px(0.).into(),
                    min_size: size(px(0.).into(), Length::Auto),
                    ..Style::default()
                },
                px(16.),
                scale,
                (),
                |known, available, _, _| {
                    let limit = known.width.or(match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        _ => None,
                    });
                    let measured_width = limit.unwrap_or(px(120.)).max(px(0.)).min(px(120.));
                    let lines = if measured_width < px(80.) { 3 } else { 1 };
                    (size(measured_width, px(14. * lines as f32)), (limit, lines))
                },
                {
                    let state = state.clone();
                    move |snapshot| state.set(Some(*snapshot))
                },
            );
            let parent = engine.request_layout_keyed(
                parent_key,
                Style {
                    display: crate::Display::Flex,
                    flex_direction: crate::FlexDirection::Row,
                    size: size(px(width).into(), Length::Auto),
                    padding: Edges::all(px(5.).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                &[text],
            );
            engine.compute_layout(
                parent,
                size(
                    AvailableSpace::Definite(px(width)),
                    AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            let result = (engine.layout_bounds(text, scale), state.get().unwrap());
            engine.finish_retained_frame();
            result
        };

        for (width, lines) in [(170., 1), (170., 1), (55., 3), (55., 3), (170., 1)] {
            let warm = layout(&mut retained, true, width);
            assert_eq!(warm, layout(&mut fresh, false, width));
            let (_, (_, measured_lines)) = warm;
            assert_eq!(measured_lines, lines);
        }
    });
}

#[test]
fn subtree_exclusions_are_atomic_and_rollback_preserves_fresh_requests() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let measured_key = keys.next_request();
        let branch_key = keys.next_request();
        let sibling_key = keys.next_request();
        let root_key = keys.next_request();
        let scale = window.scale_factor();
        let request = |engine: &mut TaffyLayoutEngine, key, children: &[LayoutId]| {
            engine.request_layout_keyed(key, Style::default(), px(16.), scale, children)
        };
        let measure = |engine: &mut TaffyLayoutEngine| {
            engine.request_measured_layout_keyed(
                measured_key,
                Style::default(),
                px(16.),
                scale,
                |_, _, _, _| size(px(25.), px(12.)),
            )
        };
        let available = size(px(160.), px(80.)).into();
        engine.begin_retained_frame(true);
        let measured = measure(&mut engine);
        let branch = request(&mut engine, branch_key, &[measured]);
        let sibling = request(&mut engine, sibling_key, &[]);
        let root = request(&mut engine, root_key, &[branch, sibling]);
        let unrelated = request(&mut engine, None, &[]);
        engine.compute_layout(root, available, window, cx);
        engine.compute_layout(unrelated, available, window, cx);
        engine.finish_retained_frame();

        engine.begin_retained_frame(true);
        assert!(!engine.try_retain_subtree(root));
        assert!(!engine.try_retain_subtree_except(root, &[root]));
        assert!(!engine.try_retain_subtree_except(root, &[branch, measured]));
        assert!(!engine.try_retain_subtree_except(root, &[branch, branch]));
        assert!(!engine.try_retain_subtree_except(root, &[branch, unrelated]));

        let revision = engine.layout_revision();
        assert!(engine.try_retain_subtree_except(root, &[branch]));
        assert_eq!(engine.layout_revision(), revision);
        assert_eq!(measure(&mut engine), measured);
        assert_eq!(request(&mut engine, branch_key, &[measured]), branch);
        assert_ne!(engine.layout_revision(), revision);

        engine.release_subtree_claims(root);
        assert_ne!(measure(&mut engine), measured);
        assert_eq!(request(&mut engine, sibling_key, &[]), sibling);
        assert_eq!(request(&mut engine, root_key, &[branch, sibling]), root);
        engine.compute_layout(root, available, window, cx);
        engine.finish_retained_frame();
    });
}

#[test]
fn unchanged_keyed_branch_can_rejoin_the_claimed_root() {
    with_layout_window(|window, cx| {
        let mut engine = TaffyLayoutEngine::new();
        let mut keys = LayoutKeys::default();
        keys.push(None);
        let child_key = keys.next_request();
        let root_key = keys.next_request();
        let scale = window.scale_factor();
        let available = size(px(100.), px(60.)).into();
        let request_child = |engine: &mut TaffyLayoutEngine| {
            engine.request_layout_keyed(
                child_key,
                Style {
                    size: size(px(30.).into(), px(12.).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                &[],
            )
        };

        engine.begin_retained_frame(true);
        let child = request_child(&mut engine);
        let root =
            engine.request_layout_keyed(root_key, Style::default(), px(16.), scale, &[child]);
        engine.compute_layout(root, available, window, cx);
        let original = engine.layout_bounds(child, scale);
        engine.finish_retained_frame();

        engine.begin_retained_frame(true);
        let revision = engine.layout_revision();
        assert!(engine.try_retain_subtree_except(root, &[child]));
        assert_eq!(request_child(&mut engine), child);
        assert_eq!(engine.layout_revision(), revision);
        engine.compute_layout(root, available, window, cx);
        assert_eq!(engine.layout_bounds(child, scale), original);
        engine.finish_retained_frame();
    });
}
