//! Public window and layout consumers of Zed
//! 635026fabc38440fce5e1767d125026a17b12012, source ccf4058b7a.

#![cfg(feature = "test-support")]

use open_gpui::{
    Context, Empty, InteractiveElement as _, IntoElement, ParentElement as _, Rems, Render,
    Styled as _, TestApp, TestAppContext, VisualTestContext, Window, div, px, rems, size,
};

#[test]
fn public_from_pixels_tracks_window_rem_size_changes_and_window_isolation() {
    let mut app = TestApp::new();
    let mut first = app.open_window(|window, _| {
        window.set_rem_size(px(16.));
        Empty
    });
    let mut second = app.open_window(|window, _| {
        window.set_rem_size(px(24.));
        Empty
    });

    let original = first.update(|_, window, _| Rems::from_pixels(px(48.), window));
    assert_eq!(original, rems(3.));

    for (rem_size, expected) in [(16., 3.), (24., 2.), (32., 1.5), (16., 3.)] {
        first.update(|_, window, _| {
            window.set_rem_size(px(rem_size));
            let converted = Rems::from_pixels(px(48.), window);
            assert_eq!(converted, rems(expected));
            assert_eq!(converted.to_pixels(window.rem_size()), px(48.));
            assert_eq!(Rems::from_pixels(px(0.), window), Rems::ZERO);
            assert_eq!(Rems::from_pixels(px(-24.), window), rems(-24. / rem_size));
            assert_eq!(original.to_pixels(window.rem_size()), px(3. * rem_size));
        });
        second.update(|_, window, _| {
            assert_eq!(window.rem_size(), px(24.));
            assert_eq!(Rems::from_pixels(px(48.), window), rems(2.));
        });
    }

    drop(first);
    drop(second);
    app.update(|cx| cx.shutdown());
}

struct RemLayout {
    original_width: Rems,
}

impl Render for RemLayout {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut converted_width = Rems::ZERO;
        converted_width += Rems::from_pixels(px(48.), window);
        converted_width += rems(1.);

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .debug_selector(|| "converted-rem-width".to_owned())
                    .w(converted_width)
                    .h(rems(1.))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .debug_selector(|| "original-rem-width".to_owned())
                    .w(self.original_width)
                    .h(rems(1.))
                    .flex_shrink_0(),
            )
    }
}

#[open_gpui::test]
fn public_rem_arithmetic_reaches_committed_layout_after_window_rem_size_changes(
    cx: &mut TestAppContext,
) {
    let handle = cx.open_window(size(px(200.), px(200.)), |window, _| {
        window.set_rem_size(px(16.));
        RemLayout {
            original_width: Rems::from_pixels(px(48.), window),
        }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);

    for rem_size in [16., 24., 32., 16.] {
        visual.update(|window, app| {
            window.set_rem_size(px(rem_size));
            window.refresh();
            window.draw(app).clear(app);
        });

        let converted = visual
            .debug_bounds("converted-rem-width")
            .expect("the converted Rems width must reach the committed frame");
        assert_eq!(converted.size, size(px(48. + rem_size), px(rem_size)));

        let original = visual
            .debug_bounds("original-rem-width")
            .expect("the original Rems width must reach the committed frame");
        assert_eq!(original.size, size(px(3. * rem_size), px(rem_size)));
    }
}
