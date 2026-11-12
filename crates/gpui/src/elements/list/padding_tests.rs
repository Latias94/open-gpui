use super::*;
use crate::{AppContext as _, Context, ParentElement as _, Render, TestAppContext, canvas, div};

#[crate::test]
fn test_horizontal_padding_insets_items(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    let state = ListState::new(10, ListAlignment::Top, px(0.)).measure_all();
    let item_bounds = Rc::new(RefCell::new(Vec::new()));

    struct TestView {
        state: ListState,
        item_bounds: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item_bounds = self.item_bounds.clone();
            list(self.state.clone(), move |_, _, _| {
                let item_bounds = item_bounds.clone();
                canvas(
                    move |bounds, _, _| item_bounds.borrow_mut().push(bounds),
                    |_, _, _, _| {},
                )
                .w_full()
                .aspect_ratio(2.)
                .into_any()
            })
            .pl(px(10.))
            .pr(px(30.))
            .w_full()
            .h_full()
        }
    }

    cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, cx| {
        cx.new(|_| TestView {
            state: state.clone(),
            item_bounds: item_bounds.clone(),
        })
        .into_any_element()
    });

    let painted_bounds = item_bounds.borrow();
    assert!(!painted_bounds.is_empty());
    for bounds in painted_bounds.iter() {
        assert_eq!(bounds.origin.x, px(10.));
        assert_eq!(bounds.size.width, px(60.));
    }
    assert_eq!(state.max_offset_for_scrollbar().y, px(200.));
}

#[crate::test]
fn test_inferred_width_includes_horizontal_padding(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    let state = ListState::new(2, ListAlignment::Top, px(10.));
    let full_width_item_bounds = Rc::new(RefCell::new(None));

    struct TestView {
        state: ListState,
        full_width_item_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let full_width_item_bounds = self.full_width_item_bounds.clone();
            div().flex().size_full().child(
                list(self.state.clone(), move |index, _, _| {
                    if index == 0 {
                        div().w(px(50.)).h(px(20.)).into_any()
                    } else {
                        let full_width_item_bounds = full_width_item_bounds.clone();
                        canvas(
                            move |bounds, _, _| *full_width_item_bounds.borrow_mut() = Some(bounds),
                            |_, _, _, _| {},
                        )
                        .h(px(20.))
                        .w_full()
                        .into_any()
                    }
                })
                .with_sizing_behavior(ListSizingBehavior::Infer)
                .pl(px(10.))
                .pr(px(30.))
                .h_full(),
            )
        }
    }

    cx.draw(point(px(0.), px(0.)), size(px(200.), px(100.)), |_, cx| {
        cx.new(|_| TestView {
            state: state.clone(),
            full_width_item_bounds: full_width_item_bounds.clone(),
        })
        .into_any_element()
    });

    let bounds = full_width_item_bounds
        .borrow()
        .expect("item 1 was not painted");
    assert_eq!(bounds.origin.x, px(10.));
    assert_eq!(bounds.size.width, px(50.));
}

#[crate::test]
fn test_autoscroll_above_item_top_with_horizontal_padding(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    struct TestView {
        state: ListState,
        refuse_source: bool,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let refuse_source = self.refuse_source;
            list(self.state.clone(), move |index, _, _| {
                if index == 2 {
                    canvas(
                        |bounds, window, _| {
                            window.request_autoscroll(Bounds::from_corners(
                                point(bounds.left(), bounds.top() - px(30.)),
                                point(bounds.right(), bounds.top() + px(5.)),
                            ));
                        },
                        move |_, _, window, _| {
                            if refuse_source {
                                window.paint_quad(crate::fill(
                                    Bounds::new(
                                        point(px(f32::MAX), px(0.)),
                                        size(px(10.), px(10.)),
                                    ),
                                    crate::red(),
                                ));
                            }
                        },
                    )
                    .w_full()
                    .aspect_ratio(2.)
                    .into_any()
                } else {
                    div().w_full().aspect_ratio(2.).into_any()
                }
            })
            .pl(px(10.))
            .pr(px(30.))
            .w_full()
            .h_full()
        }
    }

    for refuse_source in [false, true] {
        let state = ListState::new(5, ListAlignment::Top, px(0.));
        let initial = ListOffset {
            item_ix: 2,
            offset_in_item: px(0.),
        };
        state.scroll_to(initial);
        let before_scroll = state.direct_scroll_revision();

        cx.draw(point(px(0.), px(0.)), size(px(100.), px(60.)), |_, cx| {
            cx.new(|_| TestView {
                state: state.clone(),
                refuse_source,
            })
            .into_any_element()
        });

        let scroll_top = state.logical_scroll_top();
        let after_scroll = state.direct_scroll_revision();
        if refuse_source {
            assert_eq!(scroll_top, initial);
            assert_eq!(after_scroll, before_scroll);
        } else {
            assert_eq!(scroll_top.item_ix, 1);
            assert_eq!(scroll_top.offset_in_item, px(0.));
            assert!(after_scroll.vertical_changed_since(before_scroll));
            assert!(!after_scroll.horizontal_changed_since(before_scroll));
        }
    }
}

#[crate::test]
fn test_horizontal_padding_change_remeasures_items(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    let state = ListState::new(10, ListAlignment::Top, px(0.)).measure_all();

    struct TestView {
        state: ListState,
        padding_right: Pixels,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            list(self.state.clone(), |_, _, _| {
                div().w_full().aspect_ratio(2.).into_any()
            })
            .pl(px(10.))
            .pr(self.padding_right)
            .w_full()
            .h_full()
        }
    }

    for (padding_right, expected_max_offset) in [(px(30.), px(200.)), (px(10.), px(300.))] {
        cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, cx| {
            cx.new(|_| TestView {
                state: state.clone(),
                padding_right,
            })
            .into_any_element()
        });
        assert_eq!(state.max_offset_for_scrollbar().y, expected_max_offset);
    }
}

#[crate::test]
fn test_border_and_padding_inset_items(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    let state = ListState::new(10, ListAlignment::Top, px(0.)).measure_all();
    let item_bounds = Rc::new(RefCell::new(Vec::new()));

    struct TestView {
        state: ListState,
        item_bounds: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item_bounds = self.item_bounds.clone();
            list(self.state.clone(), move |_, _, _| {
                let item_bounds = item_bounds.clone();
                canvas(
                    move |bounds, _, _| item_bounds.borrow_mut().push(bounds),
                    |_, _, _, _| {},
                )
                .w_full()
                .aspect_ratio(2.)
                .into_any()
            })
            .border_l_4()
            .pl(px(6.))
            .border_r_8()
            .pr(px(22.))
            .w_full()
            .h_full()
        }
    }

    cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, cx| {
        cx.new(|_| TestView {
            state: state.clone(),
            item_bounds: item_bounds.clone(),
        })
        .into_any_element()
    });

    let painted_bounds = item_bounds.borrow();
    assert!(!painted_bounds.is_empty());
    for bounds in painted_bounds.iter() {
        assert_eq!(bounds.origin.x, px(10.));
        assert_eq!(bounds.size.width, px(60.));
    }
    assert_eq!(state.max_offset_for_scrollbar().y, px(200.));

    let reported_bounds = state.bounds_for_item(0).expect("item 0 should be measured");
    assert_eq!(reported_bounds.origin.x, px(10.));
    assert_eq!(reported_bounds.size.width, px(60.));
}

#[crate::test]
fn test_vertical_insets_match_item_bounds_and_reveal_range(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(2, ListAlignment::Top, px(0.)).measure_all();
    let painted = Rc::new(RefCell::new(Vec::new()));

    struct TestView {
        state: ListState,
        painted: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    }
    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let painted = self.painted.clone();
            list(self.state.clone(), move |_, _, _| {
                let painted = painted.clone();
                canvas(
                    move |bounds, _, _| painted.borrow_mut().push(bounds),
                    |_, _, _, _| {},
                )
                .w_full()
                .h(px(50.))
                .into_any()
            })
            .border_t_4()
            .pt(px(6.))
            .border_b_8()
            .pb(px(2.))
            .size_full()
        }
    }

    cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, cx| {
        cx.new(|_| TestView {
            state: state.clone(),
            painted: painted.clone(),
        })
        .into_any_element()
    });

    let first_painted = painted.borrow()[0];
    assert_eq!(first_painted.origin.y, px(10.));
    assert_eq!(
        (state.bounds_for_item(0), state.max_offset_for_scrollbar().y),
        (Some(first_painted), px(20.)),
    );
    assert!(state.apply_bring_into_view_offset(point(px(0.), -px(20.)), true));
    assert_eq!(state.scroll_px_offset_for_scrollbar().y, -px(20.));
    assert_eq!(
        state.bounds_for_item(0).unwrap().top(),
        first_painted.top() - px(20.),
    );
}
