use super::*;
use crate::{ParentElement as _, StyledText, TestAppContext, TextStyleRefinement, div};

#[crate::test]
fn unknown_list_width_measures_truncating_rows_at_max_content(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    for overflow in [
        crate::TextOverflow::TruncateStart("…".into()),
        crate::TextOverflow::TruncateMiddle("…".into()),
        crate::TextOverflow::Truncate("…".into()),
    ] {
        for reverse in [false, true] {
            let overflow = overflow.clone();
            cx.draw(
                Point::default(),
                size(px(800.), px(400.)),
                move |window, cx| {
                    window.with_text_style(
                        Some(TextStyleRefinement {
                            font_size: Some(px(20.).into()),
                            line_height: Some(px(24.).into()),
                            white_space: Some(crate::WhiteSpace::Nowrap),
                            text_overflow: Some(overflow),
                            ..Default::default()
                        }),
                        |window| {
                            let source = "LEFT alpha beta gamma delta epsilon omega RIGHT";
                            let layouts = Rc::new(RefCell::new(Vec::new()));
                            let mut render_item: Box<RenderItemFn> = Box::new({
                                let layouts = layouts.clone();
                                move |_, _, _| {
                                    let label = StyledText::new(source);
                                    layouts.borrow_mut().push(label.layout().clone());
                                    div()
                                        .flex()
                                        .w_full()
                                        .min_w_0()
                                        .child(div().min_w_0().child(label))
                                        .into_any_element()
                                }
                            });
                            let state = ListState::new(1_000, ListAlignment::Top, px(0.));
                            let style = window.text_style();
                            let intrinsic = window
                                .text_system()
                                .shape_text(
                                    source.into(),
                                    px(20.),
                                    &[style.to_run(source.len())],
                                    None,
                                    None,
                                )
                                .expect("list label must shape")[0]
                                .size(px(24.))
                                .width
                                .ceil();
                            let mut widths = [Some(px(1.)), None, Some(px(240.))];
                            if reverse {
                                widths.reverse();
                            }
                            for width in widths {
                                layouts.borrow_mut().clear();
                                let response = state.0.borrow_mut().layout_items(
                                    width,
                                    px(48.),
                                    &Edges::default(),
                                    &mut *render_item,
                                    window,
                                    cx,
                                );
                                assert_eq!(response.item_layouts.len(), 2);
                                assert_eq!(layouts.borrow().len(), 2);
                                if width.is_none() {
                                    assert_eq!(response.max_item_width, intrinsic);
                                    for layout in layouts.borrow().iter() {
                                        assert_eq!(layout.text(), source);
                                    }
                                } else {
                                    assert!(
                                        response.max_item_width < intrinsic,
                                        "{width:?}: measured {:?}, intrinsic {intrinsic:?}",
                                        response.max_item_width
                                    );
                                    for layout in layouts.borrow().iter() {
                                        assert!(layout.text().contains('…'));
                                    }
                                }
                            }
                        },
                    );
                    div()
                },
            );
        }
    }
}
