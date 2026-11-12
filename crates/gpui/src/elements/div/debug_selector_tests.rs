use super::*;
use crate::{
    AppContext as _, IntoElement, TestAppContext, canvas,
    window::cached_paint_atlas_tests::RejectOnceAtlas,
};

struct SelectorJournalChild {
    selector: &'static str,
    focus: FocusHandle,
    renders: Rc<Cell<usize>>,
}

impl Render for SelectorJournalChild {
    fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let selector = self.selector;
        div()
            .id("selector-journal-child")
            .size_full()
            .track_focus(&self.focus)
            .debug_selector(|| selector.to_owned())
    }
}

struct SelectorJournalRoot {
    child: Entity<SelectorJournalChild>,
}

impl Render for SelectorJournalRoot {
    fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                self.child
                    .clone()
                    .cached(StyleRefinement::default().size(px(20.))),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    |bounds, (), window, _| {
                        let image = Arc::new(crate::RenderImage::new(smallvec::smallvec![
                            image::Frame::new(image::RgbaImage::from_pixel(
                                1,
                                1,
                                image::Rgba([255, 255, 255, 255]),
                            )),
                        ]));
                        window
                            .paint_image(bounds, bounds, Default::default(), image, 0, false)
                            .unwrap();
                    },
                )
                .size(px(1.)),
            )
    }
}

#[crate::test]
fn debug_selectors_replay_only_accepted_bounds_and_focus(cx: &mut TestAppContext) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let renders = Rc::new(Cell::new(0));
    let child = cx.new(|cx| SelectorJournalChild {
        selector: "selector:accepted",
        focus: cx.focus_handle(),
        renders: renders.clone(),
    });
    let handle = cx.add_window({
        let atlas = atlas.clone();
        let child = child.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            SelectorJournalRoot { child }
        }
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.update_window(handle.into(), |_, window, cx| {
        let focus = child.read(cx).focus.clone();
        focus.focus(window, cx);
        window.draw(cx).clear(cx);
        let accepted_bounds = window.rendered_frame.debug_bounds["selector:accepted"];
        let accepted_focus = window.rendered_frame.debug_focus_handles["selector:accepted"];
        assert_eq!(accepted_bounds.size, size(px(20.), px(20.)));
        assert_eq!(window.focus, Some(accepted_focus));
        let accepted_renders = renders.get();

        window.draw(cx).clear(cx);
        assert_eq!(renders.get(), accepted_renders);
        assert_eq!(window.rendered_frame.debug_bounds.len(), 1);
        assert_eq!(window.rendered_frame.debug_bounds_entries.len(), 1);
        assert_eq!(window.rendered_frame.debug_focus_handles.len(), 1);
        assert_eq!(window.rendered_frame.debug_focus_entries.len(), 1);
        assert_eq!(
            window.rendered_frame.debug_bounds["selector:accepted"],
            accepted_bounds
        );
        assert_eq!(
            window.rendered_frame.debug_focus_handles["selector:accepted"],
            accepted_focus
        );

        let accepted_revision = window.rendered_frame_revision();
        child.update(cx, |child, cx| {
            child.selector = "selector:candidate";
            cx.notify();
        });
        atlas.fail_next_lease();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), accepted_revision);
        assert_eq!(
            window.rendered_frame.debug_bounds["selector:accepted"],
            accepted_bounds
        );
        assert_eq!(
            window.rendered_frame.debug_focus_handles["selector:accepted"],
            accepted_focus
        );
        assert!(
            !window
                .rendered_frame
                .debug_bounds
                .contains_key("selector:candidate")
        );
        assert!(
            !window
                .rendered_frame
                .debug_focus_handles
                .contains_key("selector:candidate")
        );
        assert_eq!(window.rendered_frame.debug_bounds_entries.len(), 1);
        assert_eq!(window.rendered_frame.debug_focus_entries.len(), 1);

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), accepted_revision + 1);
        assert!(
            !window
                .rendered_frame
                .debug_bounds
                .contains_key("selector:accepted")
        );
        assert!(
            !window
                .rendered_frame
                .debug_focus_handles
                .contains_key("selector:accepted")
        );
        assert_eq!(
            window.rendered_frame.debug_bounds["selector:candidate"],
            accepted_bounds
        );
        assert_eq!(
            window.rendered_frame.debug_focus_handles["selector:candidate"],
            accepted_focus
        );
        let retried_renders = renders.get();

        window.draw(cx).clear(cx);
        assert_eq!(renders.get(), retried_renders);
        assert_eq!(window.rendered_frame.debug_bounds_entries.len(), 1);
        assert_eq!(window.rendered_frame.debug_focus_entries.len(), 1);
        assert_eq!(
            window.rendered_frame.debug_bounds["selector:candidate"],
            accepted_bounds
        );
        assert_eq!(
            window.rendered_frame.debug_focus_handles["selector:candidate"],
            accepted_focus
        );
    })
    .unwrap();
}
