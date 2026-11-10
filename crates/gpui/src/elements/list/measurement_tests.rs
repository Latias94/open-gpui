use super::*;
use crate::{AppContext, Context, Render, TestAppContext, VisualTestContext, canvas, div};

#[crate::test]
fn test_provisional_item_heights_keep_scroll_anchor(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(20, crate::ListAlignment::Top, px(100.));
    let painted = Rc::new(RefCell::new(Vec::new()));
    let first_rendered = Rc::new(Cell::new(None::<usize>));
    let ready = item_readiness(&state, false);
    state.set_follow_mode(FollowMode::Tail);
    state.scroll_to(list_offset(10, 40.));

    for (height, measurement_ready) in [(0., false), (100., false), (100., true)] {
        ready.set(measurement_ready);
        painted.borrow_mut().clear();
        first_rendered.set(None);
        draw_list(
            &state,
            200.,
            {
                let painted = painted.clone();
                let first_rendered = first_rendered.clone();
                move |index| {
                    first_rendered.set(Some(first_rendered.get().unwrap_or(index).min(index)));
                    painted_item(index, height, &painted)
                }
            },
            cx,
        );
        assert_eq!(state.logical_scroll_top(), list_offset(10, 40.));
        assert!(!state.is_following_tail());
        assert_eq!(
            first_rendered.get(),
            Some(if measurement_ready { 9 } else { 10 })
        );
        if height > 0. {
            assert_eq!(
                painted.borrow().as_slice(),
                &[
                    (10, px(-40.), px(60.)),
                    (11, px(60.), px(160.)),
                    (12, px(160.), px(260.)),
                ]
            );
        }
    }
}

#[crate::test]
fn test_unready_measurements_preserve_pending_fraction(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for (provisional_height, destination, final_height, expected_offset) in [
        (Some(200.), None, 200., 100.),
        (None, Some(40.), 200., 80.),
        (Some(200.), Some(50.), 200., 50.),
        (Some(0.), Some(40.), 20., 20.),
        (Some(10.), Some(40.), 100., 40.),
        (Some(200.), Some(50.), 300., 50.),
    ] {
        let state = ListState::new(10, crate::ListAlignment::Top, px(0.));
        let painted = Rc::new(RefCell::new(Vec::new()));
        let draw = |height, cx: &mut VisualTestContext| {
            painted.borrow_mut().clear();
            let painted = painted.clone();
            draw_list(
                &state,
                200.,
                move |index| painted_item(index, if index == 5 { height } else { 100. }, &painted),
                cx,
            );
        };
        let ready = item_readiness(&state, true);
        state.scroll_to(list_offset(5, 50.));
        draw(100., cx);
        ready.set(false);
        state.remeasure();
        if let Some(height) = provisional_height {
            for height in [0., height, height] {
                draw(height, cx);
                assert_eq!(state.logical_scroll_top(), list_offset(5, 50.));
            }
        }
        if let Some(offset) = destination {
            state.scroll_to(list_offset(5, offset));
        }
        ready.set(true);
        for _ in 0..2 {
            draw(final_height, cx);
            assert_eq!(state.logical_scroll_top(), list_offset(5, expected_offset));
            assert_eq!(
                painted.borrow().first().copied(),
                Some((5, px(-expected_offset), px(final_height - expected_offset)))
            );
        }
    }
}

#[crate::test]
fn test_navigation_cancels_provisional_and_paints_destination(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for measured in [false, true] {
        let navigations: [(&str, fn(&ListState), usize, f32); 7] = [
            (
                "scroll_to",
                |state| state.scroll_to(list_offset(12, 0.)),
                12,
                0.,
            ),
            ("end", ListState::scroll_to_end, 19, 150.),
            ("down", |state| state.scroll_by(px(5000.)), 19, 150.),
            (
                "up",
                |state| state.scroll_by(px(-5000.)),
                if measured { 0 } else { 6 },
                0.,
            ),
            ("reveal_below", |state| state.scroll_to_item(10), 10, 150.),
            ("reveal_above", |state| state.scroll_to_item(0), 0, 0.),
            (
                "follow_tail",
                |state| state.set_follow_mode(FollowMode::Tail),
                19,
                150.,
            ),
        ];
        for (name, navigate, destination, top) in navigations {
            let state = ListState::new(20, crate::ListAlignment::Top, px(0.));
            let state = if measured { state.measure_all() } else { state };
            let painted = Rc::new(RefCell::new(Vec::new()));
            let draw = |cx: &mut VisualTestContext| {
                painted.borrow_mut().clear();
                let painted = painted.clone();
                draw_list(
                    &state,
                    200.,
                    move |index| painted_item(index, if index == 5 { 0. } else { 50. }, &painted),
                    cx,
                );
            };
            state.set_item_measurement_readiness(|index, _| index != 5);
            state.scroll_to(list_offset(5, 20.));
            state.remeasure_items(5..6);
            draw(cx);
            assert!(state.0.borrow().is_provisional(), "{name}");
            let before_navigation = state.direct_scroll_revision();
            navigate(&state);
            let after_navigation = state.direct_scroll_revision();
            assert!(
                after_navigation.vertical_changed_since(before_navigation),
                "{name}"
            );
            assert!(
                !after_navigation.horizontal_changed_since(before_navigation),
                "{name}"
            );
            assert!(!state.0.borrow().is_provisional(), "{name}");
            draw(cx);
            assert_eq!(state.direct_scroll_revision(), after_navigation, "{name}");
            assert!(!state.0.borrow().is_provisional(), "{name}");
            assert_eq!(
                painted
                    .borrow()
                    .iter()
                    .find(|(index, _, _)| *index == destination)
                    .copied(),
                Some((destination, px(top), px(top + 50.))),
                "{name}, measured={measured}",
            );
        }
    }

    let navigations: [(&str, fn(&ListState)); 6] = [
        ("end_sentinel", |state| state.scroll_to(list_offset(3, 0.))),
        ("scroll_to_end", ListState::scroll_to_end),
        ("follow_tail", |state| {
            state.set_follow_mode(FollowMode::Tail)
        }),
        ("horizontal", ListState::scroll_to_end),
        ("pause_tail", |state| {
            state.set_follow_mode(FollowMode::Tail)
        }),
        ("disable_tail", |state| {
            state.set_follow_mode(FollowMode::Tail)
        }),
    ];
    for (name, navigate) in navigations {
        let state = ListState::new(3, crate::ListAlignment::Top, px(0.)).measure_all();
        let ready = item_readiness(&state, false);
        let painted = Rc::new(RefCell::new(Vec::new()));
        let draw = |height, cx: &mut VisualTestContext| {
            painted.borrow_mut().clear();
            let painted = painted.clone();
            draw_list(
                &state,
                200.,
                move |index| painted_item(index, height, &painted),
                cx,
            );
        };
        navigate(&state);
        for (height, measurement_ready, expected) in [
            (
                100.,
                false,
                [(1, px(0.), px(100.)), (2, px(100.), px(200.))],
            ),
            (
                150.,
                true,
                [(1, px(-100.), px(50.)), (2, px(50.), px(200.))],
            ),
        ] {
            if measurement_ready {
                match name {
                    "horizontal" => cx.simulate_event(ScrollWheelEvent {
                        position: point(px(50.), px(100.)),
                        delta: ScrollDelta::Pixels(point(px(20.), px(0.))),
                        ..ScrollWheelEvent::default()
                    }),
                    "pause_tail" => state.pause_following_tail(),
                    "disable_tail" => state.set_follow_mode(FollowMode::Normal),
                    _ => {}
                }
            }
            ready.set(measurement_ready);
            draw(height, cx);
            if !measurement_ready {
                assert_eq!(
                    state.scroll_px_offset_for_scrollbar().y,
                    px(-100.),
                    "{name}"
                );
                assert_eq!(state.logical_scroll_top(), list_offset(1, 0.), "{name}");
                assert_eq!(state.item_is_above_viewport(2), None, "{name}");
            }
            let paused = name == "pause_tail" || name == "disable_tail";
            let expected = if measurement_ready && paused {
                [(1, px(0.), px(150.)), (2, px(150.), px(300.))]
            } else {
                expected
            };
            assert_eq!(
                state.is_following_tail(),
                name == "follow_tail" || (paused && !measurement_ready),
                "{name}"
            );
            assert_eq!(painted.borrow().as_slice(), &expected, "{name}");
        }
        ready.set(false);
        navigate(&state);
        draw(150., cx);
        state.scroll_by(px(-20.));
        draw(150., cx);
        assert_eq!(state.logical_scroll_top(), list_offset(1, 80.), "{name}");
        assert!(!state.is_following_tail(), "{name}");
        assert_eq!(
            painted.borrow().as_slice(),
            &[(1, px(-80.), px(70.)), (2, px(70.), px(220.)),],
            "{name}"
        );
    }
}

#[crate::test]
fn test_unready_overdraw_and_backfill_preserve_scroll_anchor(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for (overdraw, anchor, unready_item) in
        [(100., list_offset(0, 0.), 2), (0., list_offset(2, 40.), 1)]
    {
        let state = ListState::new(3, crate::ListAlignment::Top, px(overdraw));
        let ready = Rc::new(Cell::new(false));
        state.set_item_measurement_readiness({
            let ready = ready.clone();
            move |index, _| index != unready_item || ready.get()
        });
        let painted = Rc::new(RefCell::new(Vec::new()));
        let draw = |height, cx: &mut VisualTestContext| {
            painted.borrow_mut().clear();
            let painted = painted.clone();
            draw_list(
                &state,
                200.,
                move |index| {
                    let height = if index == unready_item { height } else { 100. };
                    painted_item(index, height, &painted)
                },
                cx,
            );
        };
        state.set_follow_mode(FollowMode::Tail);
        state.scroll_to(anchor);
        draw(0., cx);
        assert_eq!(state.logical_scroll_top(), anchor);
        assert!(!state.is_following_tail());
        assert_eq!(
            *painted.borrow(),
            if unready_item == 2 {
                vec![(0, px(0.), px(100.)), (1, px(100.), px(200.))]
            } else {
                vec![(2, px(-40.), px(60.))]
            }
        );
        ready.set(true);
        draw(500., cx);
        if unready_item == 2 {
            assert_eq!(state.logical_scroll_top(), anchor);
            assert!(!state.is_following_tail());
            assert_eq!(state.max_offset_for_scrollbar().y, px(500.));
            assert_eq!(
                painted.borrow().as_slice(),
                &[(0, px(0.), px(100.)), (1, px(100.), px(200.))]
            );
        } else {
            assert_eq!(state.logical_scroll_top(), list_offset(1, 400.));
            assert!(state.is_following_tail());
            assert_eq!(
                painted.borrow().as_slice(),
                &[(1, px(-400.), px(100.)), (2, px(100.), px(200.))]
            );
        }
    }
}

#[crate::test]
fn test_unready_overdraw_stops_at_first_pending_item(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for anchor in [0, 10] {
        let state = ListState::new(13, crate::ListAlignment::Top, px(100.));
        let ready = Rc::new(Cell::new(false));
        let initially_ready = move |index| {
            if anchor == 0 { index < 2 } else { index >= 10 }
        };
        state.set_item_measurement_readiness({
            let ready = ready.clone();
            move |index, _| initially_ready(index) || ready.get()
        });
        state.scroll_to(list_offset(anchor, 0.));
        let rendered = Rc::new(RefCell::new(Vec::new()));
        let painted = Rc::new(RefCell::new(Vec::new()));
        let expected_frames = if anchor == 0 {
            [vec![0, 1, 2], vec![0, 1, 2], vec![0, 1, 2], vec![0, 1]]
        } else {
            [
                vec![10, 11, 12, 9],
                vec![10, 11, 9],
                vec![10, 11, 9],
                vec![10, 11],
            ]
        };
        for (frame, expected_rendered) in expected_frames.into_iter().enumerate() {
            let measurement_ready = frame >= 2;
            ready.set(measurement_ready);
            rendered.borrow_mut().clear();
            painted.borrow_mut().clear();
            draw_list(
                &state,
                200.,
                {
                    let rendered = rendered.clone();
                    let painted = painted.clone();
                    move |index| {
                        rendered.borrow_mut().push(index);
                        let height = if initially_ready(index) || measurement_ready {
                            100.
                        } else {
                            0.
                        };
                        painted_item(index, height, &painted)
                    }
                },
                cx,
            );
            assert_eq!(*rendered.borrow(), expected_rendered);
            assert_eq!(state.item_count(), 13);
            assert_eq!(state.logical_scroll_top(), list_offset(anchor, 0.));
            assert_eq!(
                painted.borrow().as_slice(),
                &[(anchor, px(0.), px(100.)), (anchor + 1, px(100.), px(200.))]
            );
        }
    }
}

#[crate::test]
fn test_scrollbar_and_reset_do_not_resurrect_provisional(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(20, crate::ListAlignment::Top, px(0.));
    let painted = Rc::new(RefCell::new(Vec::new()));
    let draw = |cx: &mut VisualTestContext| {
        painted.borrow_mut().clear();
        let painted = painted.clone();
        draw_list(
            &state,
            200.,
            move |index| painted_item(index, if index == 5 { 0. } else { 50. }, &painted),
            cx,
        );
    };
    state.set_item_measurement_readiness(|index, _| index != 5);
    assert!(!state.0.borrow().is_provisional());
    state.set_follow_mode(FollowMode::Tail);
    for intervening_frame in [false, true] {
        state.scroll_to(list_offset(5, 20.));
        state.remeasure_items(5..6);
        draw(cx);
        state.scrollbar_drag_started();
        if intervening_frame {
            draw(cx);
            assert!(state.0.borrow().is_provisional());
        }
        state.set_offset_from_scrollbar(point(px(0.), px(-10_000.)));
        assert!(!state.0.borrow().is_provisional());
        state.scrollbar_drag_ended();
        assert!(!state.is_scrollbar_dragging());
        draw(cx);
        assert_eq!(
            painted.borrow().last().copied(),
            Some((19, px(150.), px(200.)))
        );
        assert!(!state.0.borrow().is_provisional());
        state.scroll_to(list_offset(5, 20.));
        assert!(!state.0.borrow().is_provisional());
        draw(cx);
        assert!(state.0.borrow().is_provisional());
        assert_eq!(state.logical_scroll_top(), list_offset(5, 20.));
    }
    state.remeasure_items(5..6);
    state.reset(20);
    assert!(!state.0.borrow().is_provisional());
    draw(cx);
    assert!(!state.0.borrow().is_provisional());
    assert_eq!(state.logical_scroll_top(), ListOffset::default());
    assert_eq!(
        painted.borrow().first().copied(),
        Some((0, px(0.), px(50.)))
    );
}

#[crate::test]
fn test_splice_preserves_only_surviving_scroll_offsets(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let painted = Rc::new(RefCell::new(Vec::new()));
    let draw = |state: &ListState, height, cx: &mut VisualTestContext| {
        painted.borrow_mut().clear();
        let painted = painted.clone();
        draw_list(
            state,
            200.,
            move |index| painted_item(index, height, &painted),
            cx,
        );
    };
    for (provisional, replacement_count) in [(true, 0), (false, 1)] {
        let state = ListState::new(20, crate::ListAlignment::Top, px(0.));
        let ready = item_readiness(&state, !provisional);
        state.scroll_to(list_offset(8, 50.));
        state.remeasure_items(8..9);
        if provisional {
            draw(&state, 100., cx);
        }
        state.splice(8..9, replacement_count);
        assert!(!state.0.borrow().is_provisional());
        ready.set(true);
        draw(&state, 100., cx);
        assert_eq!(state.logical_scroll_top(), list_offset(8, 0.));
        assert_eq!(
            painted.borrow().first().copied(),
            Some((8, px(0.), px(100.)))
        );
    }
    let state = ListState::new(20, crate::ListAlignment::Top, px(0.)).measure_all();
    state.scroll_to(list_offset(8, 50.));
    draw(&state, 100., cx);
    state.remeasure();
    state.splice(0..2, 0);
    draw(&state, 80., cx);
    assert_eq!(state.logical_scroll_top(), list_offset(6, 40.));
    assert_eq!(
        painted.borrow().first().copied(),
        Some((6, px(-40.), px(40.)))
    );
    let ready = item_readiness(&state, false);
    state.remeasure_items(6..7);
    state.splice(8..10, 3);
    draw(&state, 100., cx);
    assert!(state.0.borrow().is_provisional());
    assert_eq!(state.logical_scroll_top(), list_offset(6, 40.));
    state.splice(0..0, 2);
    assert!(!state.0.borrow().is_provisional());
    draw(&state, 100., cx);
    assert!(state.0.borrow().is_provisional());
    assert_eq!(state.logical_scroll_top(), list_offset(8, 40.));
    ready.set(true);
    draw(&state, 100., cx);
    assert_eq!(state.logical_scroll_top(), list_offset(8, 40.));
    assert_eq!(
        painted.borrow().first().copied(),
        Some((8, px(-40.), px(60.)))
    );
}

#[crate::test]
fn test_child_autoscroll_overrides_pending_and_release_fills_viewport(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for (count, child, request, expected, released) in [
        (10, 2, 40.0..45.0, list_offset(2, 40.), list_offset(2, 40.)),
        (3, 2, 40.0..45.0, list_offset(2, 40.), list_offset(1, 0.)),
        (10, 4, 80.0..90.0, list_offset(2, 90.), list_offset(2, 90.)),
        (10, 2, -130.0..5.0, list_offset(0, 70.), list_offset(0, 70.)),
        (10, 2, -250.0..5.0, list_offset(0, 0.), list_offset(0, 0.)),
    ] {
        let state = ListState::new(count, crate::ListAlignment::Top, px(0.));
        let requested = Rc::new(Cell::new(false));
        let painted = Rc::new(RefCell::new(Vec::new()));
        let draw = |cx: &mut VisualTestContext| {
            painted.borrow_mut().clear();
            let painted = painted.clone();
            let requested = requested.clone();
            let request = request.clone();
            draw_list(
                &state,
                200.,
                move |index| {
                    let painted = painted.clone();
                    let requested = requested.clone();
                    let request = request.clone();
                    canvas(
                        move |bounds, window, _| {
                            if index == child && !requested.replace(true) {
                                window.request_autoscroll(Bounds::from_corners(
                                    point(bounds.left(), bounds.top() + px(request.start)),
                                    point(bounds.right(), bounds.top() + px(request.end)),
                                ));
                            }
                        },
                        move |bounds, _, _, _| {
                            painted
                                .borrow_mut()
                                .push((index, bounds.top(), bounds.bottom()));
                        },
                    )
                    .h(px(100.))
                    .w_full()
                    .into_any()
                },
                cx,
            );
        };
        let ready = item_readiness(&state, false);
        state.scroll_to(list_offset(2, 50.));
        state.remeasure_items(2..3);
        draw(cx);
        assert!(requested.get());
        for _ in 0..2 {
            draw(cx);
            assert!(requested.get());
            assert!(state.0.borrow().is_provisional());
            assert_eq!(state.logical_scroll_top(), expected);
            assert_eq!(
                painted.borrow().first().copied(),
                Some((
                    expected.item_ix,
                    -expected.offset_in_item,
                    px(100.) - expected.offset_in_item,
                ))
            );
        }
        ready.set(true);
        draw(cx);
        assert!(!state.0.borrow().is_provisional());
        assert_eq!(state.logical_scroll_top(), released);
        let visible_count = if released.offset_in_item == px(0.) {
            2
        } else {
            3
        };
        assert_eq!(
            painted.borrow().as_slice(),
            (0..visible_count)
                .map(|index| {
                    let top = px(index as f32 * 100.) - released.offset_in_item;
                    (released.item_ix + index, top, top + px(100.))
                })
                .collect::<Vec<_>>()
                .as_slice()
        );
    }
}

#[crate::test]
fn test_pause_following_tail_reengages_when_still_at_bottom(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    // 10 items × 50px = 500px total, 200px viewport.
    let state = ListState::new(10, crate::ListAlignment::Top, px(0.));

    struct TestView(ListState);
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            list(self.0.clone(), |_, _, _| {
                div().h(px(50.)).w_full().into_any()
            })
            .w_full()
            .h_full()
        }
    }

    let view = cx.update(|_, cx| cx.new(|_| TestView(state.clone())));
    state.set_follow_mode(FollowMode::Tail);

    cx.draw(point(px(0.), px(0.)), size(px(100.), px(200.)), |_, _| {
        view.clone().into_any_element()
    });
    assert!(state.is_following_tail());

    // Pausing while the view is still at the bottom (e.g. a no-op zoom)
    // must not strand follow-tail: the next layout has to re-engage so the
    // invariant "at the bottom + new content => visible" is preserved.
    state.pause_following_tail();
    assert!(!state.is_following_tail());

    cx.draw(point(px(0.), px(0.)), size(px(100.), px(200.)), |_, _| {
        view.into_any_element()
    });
    assert!(
        state.is_following_tail(),
        "pausing while at the bottom must re-engage follow-tail on the next layout"
    );
}

#[crate::test]
fn test_pause_following_tail_freezes_off_bottom(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    // 10 items, 200px viewport. Item height is adjustable to simulate a
    // diagram block growing/shrinking on zoom.
    let item_height = Rc::new(Cell::new(50usize));
    let state = ListState::new(10, crate::ListAlignment::Top, px(0.));

    struct TestView {
        state: ListState,
        item_height: Rc<Cell<usize>>,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let height = self.item_height.get();
            list(self.state.clone(), move |_, _, _| {
                div().h(px(height as f32)).w_full().into_any()
            })
            .w_full()
            .h_full()
        }
    }

    let view = cx.update(|_, cx| {
        cx.new(|_| TestView {
            state: state.clone(),
            item_height: item_height.clone(),
        })
    });
    state.set_follow_mode(FollowMode::Tail);

    // At the bottom: 500px content, 200px viewport → top at item 6.
    cx.draw(point(px(0.), px(0.)), size(px(100.), px(200.)), |_, _| {
        view.clone().into_any_element()
    });
    assert_eq!(state.logical_scroll_top().item_ix, 6);
    assert!(state.is_following_tail());

    // Pause, then grow items (a zoom-in that pushes content below the fold).
    // The frozen top must stay put rather than snapping to the new end, and
    // following stays paused since we're no longer at the bottom.
    state.pause_following_tail();
    item_height.set(80);
    state.remeasure();
    cx.draw(point(px(0.), px(0.)), size(px(100.), px(200.)), |_, _| {
        view.clone().into_any_element()
    });
    let offset = state.logical_scroll_top();
    assert_eq!(offset.item_ix, 6);
    assert_eq!(offset.offset_in_item, px(0.));
    assert!(
        !state.is_following_tail(),
        "a paused list must not re-engage while the frozen top is off the bottom"
    );

    // Shrink back (zoom-out) so the frozen top once again reaches the
    // bottom: follow-tail must re-engage on its own.
    item_height.set(50);
    state.remeasure();
    cx.draw(point(px(0.), px(0.)), size(px(100.), px(200.)), |_, _| {
        view.into_any_element()
    });
    assert!(
        state.is_following_tail(),
        "returning to the bottom must restore follow-tail"
    );
}

#[crate::test]
fn test_scroll_after_remeasure_clamps_to_shrunk_item_height(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for provisional in [false, true] {
        let state = ListState::new(20, crate::ListAlignment::Top, px(10.));
        let painted = Rc::new(RefCell::new(Vec::new()));
        let draw = |height, cx: &mut VisualTestContext| {
            painted.borrow_mut().clear();
            let painted = painted.clone();
            draw_list(
                &state,
                200.,
                move |index| painted_item(index, if index == 5 { height } else { 100. }, &painted),
                cx,
            );
        };
        let ready = item_readiness(&state, true);
        state.scroll_to(list_offset(5, 40.));
        draw(100., cx);
        ready.set(!provisional);
        state.remeasure_items(5..6);
        if provisional {
            draw(100., cx);
        }
        ready.set(true);
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(50.), px(100.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-30.))),
            ..ScrollWheelEvent::default()
        });
        assert_eq!(state.logical_scroll_top(), list_offset(5, 70.));
        for _ in 0..2 {
            draw(50., cx);
            assert_eq!(state.logical_scroll_top(), list_offset(5, 50.));
            assert_eq!(
                painted
                    .borrow()
                    .iter()
                    .find(|(index, _, _)| *index == 6)
                    .copied(),
                Some((6, px(0.), px(100.)))
            );
        }
    }
}

#[crate::test]
fn child_autoscroll_admission_preserves_or_rebases_provisional_pending(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for refuse_source in [false, true] {
        let state = ListState::new(10, ListAlignment::Top, px(0.));
        let ready = item_readiness(&state, true);
        state.scroll_to(list_offset(5, 50.));
        draw_list(&state, 200., |_| div().h(px(100.)).w_full().into_any(), cx);

        ready.set(false);
        state.remeasure();
        let before_autoscroll = state.direct_scroll_revision();
        draw_list(
            &state,
            200.,
            move |index| {
                canvas(
                    move |bounds, window, _| {
                        if index == 5 {
                            window.request_autoscroll(Bounds::from_corners(
                                point(bounds.left(), bounds.top() + px(20.)),
                                point(bounds.right(), bounds.top() + px(25.)),
                            ));
                        }
                    },
                    move |_, _, window, _| {
                        if index == 5 && refuse_source {
                            window.paint_quad(crate::fill(
                                Bounds::new(point(px(f32::MAX), px(0.)), size(px(10.), px(10.))),
                                crate::red(),
                            ));
                        }
                    },
                )
                .h(px(200.))
                .w_full()
                .into_any()
            },
            cx,
        );

        let anchor = state.0.borrow().scroll_anchor.unwrap();
        let after_autoscroll = state.direct_scroll_revision();
        if refuse_source {
            assert_eq!(anchor.position, list_offset(5, 50.));
            assert!(anchor.provisional);
            assert!(matches!(
                anchor.pending,
                Some(PendingScroll::Proportional(fraction)) if fraction == 0.5
            ));
            assert_eq!(after_autoscroll, before_autoscroll);
        } else {
            assert_eq!(anchor.position, list_offset(5, 20.));
            assert!(!anchor.provisional);
            assert!(matches!(
                anchor.pending,
                Some(PendingScroll::Absolute(offset)) if offset == px(20.)
            ));
            assert!(after_autoscroll.vertical_changed_since(before_autoscroll));
            assert!(!after_autoscroll.horizontal_changed_since(before_autoscroll));
        }

        ready.set(true);
        draw_list(&state, 200., |_| div().h(px(200.)).w_full().into_any(), cx);
        assert_eq!(
            state.logical_scroll_top(),
            list_offset(5, if refuse_source { 100. } else { 20. })
        );
        assert!(!state.0.borrow().is_provisional());
        assert_eq!(state.direct_scroll_revision(), after_autoscroll);
    }
}

#[crate::test]
fn bring_into_view_rebases_provisional_pending_without_a_direct_scroll_revision(
    cx: &mut TestAppContext,
) {
    let cx = cx.add_empty_window();
    let state = ListState::new(10, ListAlignment::Top, px(0.)).measure_all();
    let ready = item_readiness(&state, true);
    state.scroll_to(list_offset(5, 50.));
    draw_list(&state, 200., |_| div().h(px(100.)).w_full().into_any(), cx);

    ready.set(false);
    state.remeasure();
    draw_list(&state, 200., |_| div().h(px(200.)).w_full().into_any(), cx);
    assert!(state.0.borrow().is_provisional());
    let before_bring_into_view = state.direct_scroll_revision();
    let offset = state.scroll_px_offset_for_scrollbar();
    assert!(state.apply_bring_into_view_offset(point(px(0.), offset.y + px(20.)), true));
    assert_eq!(state.logical_scroll_top(), list_offset(5, 30.));
    assert!(!state.0.borrow().is_provisional());
    assert_eq!(state.direct_scroll_revision(), before_bring_into_view);

    ready.set(true);
    draw_list(&state, 200., |_| div().h(px(400.)).w_full().into_any(), cx);
    assert_eq!(state.logical_scroll_top(), list_offset(5, 30.));
    assert_eq!(state.direct_scroll_revision(), before_bring_into_view);
    assert!(state.0.borrow().scroll_anchor.unwrap().pending.is_none());
}

fn draw_list(
    state: &ListState,
    viewport_height: f32,
    render_item: impl Fn(usize) -> AnyElement + 'static,
    cx: &mut VisualTestContext,
) {
    cx.draw(
        point(px(0.), px(0.)),
        size(px(100.), px(viewport_height)),
        |_, cx| {
            cx.new(|_| TestView {
                state: state.clone(),
                render_item: Rc::new(render_item),
            })
            .into_any_element()
        },
    );
}

fn painted_item(
    index: usize,
    height: f32,
    painted: &Rc<RefCell<Vec<(usize, Pixels, Pixels)>>>,
) -> AnyElement {
    let painted = painted.clone();
    canvas(
        |_, _, _| {},
        move |bounds, _, _, _| {
            painted
                .borrow_mut()
                .push((index, bounds.top(), bounds.bottom()));
        },
    )
    .h(px(height))
    .w_full()
    .into_any()
}

fn item_readiness(state: &ListState, ready: bool) -> Rc<Cell<bool>> {
    let ready = Rc::new(Cell::new(ready));
    state.set_item_measurement_readiness({
        let ready = ready.clone();
        move |_, _| ready.get()
    });
    ready
}

fn list_offset(item_ix: usize, offset: f32) -> ListOffset {
    ListOffset {
        item_ix,
        offset_in_item: px(offset),
    }
}

struct TestView {
    state: ListState,
    render_item: Rc<dyn Fn(usize) -> AnyElement>,
}

impl Render for TestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let render_item = self.render_item.clone();
        list(self.state.clone(), move |index, _, _| render_item(index))
            .w_full()
            .h_full()
    }
}
