use super::*;
use crate::{ParentElement as _, StyledText, TestAppContext, TextStyleRefinement, div};

#[crate::test]
fn unknown_uniform_list_width_measures_truncating_rows_at_max_content(cx: &mut TestAppContext) {
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
                            let ranges = Rc::new(RefCell::new(Vec::new()));
                            let list = uniform_list("text-measurement", 1_000, {
                                let layouts = layouts.clone();
                                let ranges = ranges.clone();
                                move |range, _, _| {
                                    ranges.borrow_mut().push(range.clone());
                                    range
                                        .map(|_| {
                                            let label = StyledText::new(source);
                                            layouts.borrow_mut().push(label.layout().clone());
                                            div()
                                                .flex()
                                                .w_full()
                                                .min_w_0()
                                                .child(div().min_w_0().child(label))
                                        })
                                        .collect()
                                }
                            })
                            .with_width_from_item(Some(999));
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
                                .expect("uniform list label must shape")[0]
                                .size(px(24.))
                                .width
                                .ceil();
                            let mut widths = [Some(px(1.)), None, Some(px(240.))];
                            if reverse {
                                widths.reverse();
                            }
                            for width in widths {
                                layouts.borrow_mut().clear();
                                ranges.borrow_mut().clear();
                                let measured = list.measure_item(width, window, cx);
                                assert_eq!(ranges.borrow().len(), 1);
                                assert_eq!(ranges.borrow()[0], 999..1_000);
                                assert_eq!(layouts.borrow().len(), 1);
                                let layout = &layouts.borrow()[0];
                                if width.is_none() {
                                    assert_eq!(measured.width, intrinsic);
                                    assert_eq!(layout.text(), source);
                                } else {
                                    assert!(
                                        measured.width < intrinsic,
                                        "{width:?}: measured {:?}, intrinsic {intrinsic:?}",
                                        measured.width
                                    );
                                    assert!(layout.text().contains('…'));
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
