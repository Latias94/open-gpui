use super::*;
use crate::{AppContext as _, Context, Render, TestAppContext, canvas, div, fill, red};
use std::cell::RefCell;

struct OvershootList {
    state: ListState,
    heights: Rc<[Pixels]>,
    target: usize,
    overshoot: Pixels,
    measured: Rc<RefCell<Vec<usize>>>,
    fail_during_paint: bool,
}

impl Render for OvershootList {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let heights = self.heights.clone();
        let target = self.target;
        let overshoot = self.overshoot;
        let measured = self.measured.clone();
        let fail_during_paint = self.fail_during_paint;
        list(self.state.clone(), move |index, _, _| {
            measured.borrow_mut().push(index);
            if index == target {
                canvas(
                    move |bounds, window, _| {
                        window.request_autoscroll(Bounds::from_corners(
                            point(bounds.left(), bounds.top() - overshoot),
                            point(bounds.right(), bounds.top() + px(5.)),
                        ));
                    },
                    move |_, _, window, _| {
                        if fail_during_paint {
                            window.paint_quad(fill(
                                Bounds::new(point(px(f32::MAX), px(0.)), size(px(10.), px(10.))),
                                red(),
                            ));
                        }
                    },
                )
                .h(heights[index])
                .w_full()
                .into_any_element()
            } else {
                div().h(heights[index]).w_full().into_any_element()
            }
        })
        .w_full()
        .h_full()
    }
}

#[crate::test]
fn test_autoscroll_above_item_top_renders_items_above(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(5, ListAlignment::Top, px(10.));
    state.scroll_to(ListOffset {
        item_ix: 2,
        offset_in_item: px(0.),
    });
    let before_scroll = state.direct_scroll_revision();
    let rendered_state = state.clone();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(100.), px(60.)),
        move |_, cx| {
            cx.new(move |_| OvershootList {
                state: rendered_state,
                heights: vec![px(20.); 5].into(),
                target: 2,
                overshoot: px(30.),
                measured: Rc::default(),
                fail_during_paint: false,
            })
            .into_any_element()
        },
    );
    let scroll_top = state.logical_scroll_top();
    assert!(scroll_top.offset_in_item >= px(0.));
    assert_eq!(scroll_top.item_ix, 0);
    assert_eq!(scroll_top.offset_in_item, px(10.));
    let after_scroll = state.direct_scroll_revision();
    assert!(after_scroll.vertical_changed_since(before_scroll));
    assert!(!after_scroll.horizontal_changed_since(before_scroll));
}

#[crate::test]
fn autoscroll_measures_unseen_variable_rows_before_the_source(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(6, ListAlignment::Top, px(0.));
    state.scroll_to(ListOffset {
        item_ix: 3,
        offset_in_item: px(0.),
    });
    let rendered_state = state.clone();
    let measured = Rc::<RefCell<Vec<usize>>>::default();
    let rendered_measured = measured.clone();
    cx.draw(Point::default(), size(px(100.), px(60.)), move |_, cx| {
        cx.new(move |_| OvershootList {
            state: rendered_state,
            heights: vec![px(13.), px(27.), px(19.), px(23.), px(31.), px(17.)].into(),
            target: 3,
            overshoot: px(55.),
            measured: rendered_measured,
            fail_during_paint: false,
        })
        .into_any_element()
    });
    let scroll_top = state.logical_scroll_top();
    assert_eq!(scroll_top.item_ix, 0);
    assert_eq!(scroll_top.offset_in_item, px(4.));
    let measured = measured.borrow();
    let source = measured.iter().position(|index| *index == 3).unwrap();
    for index in 0..3 {
        let first = measured.iter().position(|entry| *entry == index).unwrap();
        assert!(first > source, "preceding row {index} was already measured");
    }
}

#[crate::test]
fn autoscroll_before_the_first_row_clamps_at_the_start(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(5, ListAlignment::Top, px(0.));
    state.scroll_to(ListOffset {
        item_ix: 2,
        offset_in_item: px(0.),
    });
    let rendered_state = state.clone();
    cx.draw(Point::default(), size(px(100.), px(60.)), move |_, cx| {
        cx.new(move |_| OvershootList {
            state: rendered_state,
            heights: vec![px(20.); 5].into(),
            target: 2,
            overshoot: px(500.),
            measured: Rc::default(),
            fail_during_paint: false,
        })
        .into_any_element()
    });
    assert_eq!(state.logical_scroll_top(), ListOffset::default());
}

#[crate::test]
fn invalid_overshoot_source_keeps_scroll_and_direct_revision(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let state = ListState::new(5, ListAlignment::Top, px(0.));
    let initial = ListOffset {
        item_ix: 2,
        offset_in_item: px(0.),
    };
    state.scroll_to(initial);
    let before_scroll = state.direct_scroll_revision();
    let rendered_state = state.clone();
    cx.draw(Point::default(), size(px(100.), px(60.)), move |_, cx| {
        cx.new(move |_| OvershootList {
            state: rendered_state,
            heights: vec![px(20.); 5].into(),
            target: 2,
            overshoot: px(30.),
            measured: Rc::default(),
            fail_during_paint: true,
        })
        .into_any_element()
    });
    assert_eq!(state.logical_scroll_top(), initial);
    assert_eq!(state.direct_scroll_revision(), before_scroll);
}

#[crate::test]
fn autoscroll_skips_zero_height_preceding_rows_and_clamps_when_all_are_zero(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(crate::QuitMode::Explicit));
    for (heights, expected_offset) in [
        ([13., 0., 27., 23., 31., 17.], px(10.)),
        ([0., 0., 0., 23., 31., 17.], px(0.)),
    ] {
        let visual = cx.add_empty_window();
        let state = ListState::new(heights.len(), ListAlignment::Top, px(0.));
        state.scroll_to(ListOffset {
            item_ix: 3,
            offset_in_item: px(0.),
        });
        let rendered_state = state.clone();
        let measured = Rc::<RefCell<Vec<usize>>>::default();
        let rendered_measured = measured.clone();
        visual.draw(Point::default(), size(px(100.), px(60.)), move |_, cx| {
            cx.new(move |_| OvershootList {
                state: rendered_state,
                heights: heights.map(px).into(),
                target: 3,
                overshoot: px(30.),
                measured: rendered_measured,
                fail_during_paint: false,
            })
            .into_any_element()
        });
        assert_eq!(state.logical_scroll_top().item_ix, 0);
        assert_eq!(state.logical_scroll_top().offset_in_item, expected_offset);
        let measured = measured.borrow();
        let source = measured.iter().position(|index| *index == 3).unwrap();
        for index in 0..3 {
            assert!(measured.iter().position(|entry| *entry == index).unwrap() > source);
        }
        visual.update(|window, cx| window.remove_window(cx));
    }
}
