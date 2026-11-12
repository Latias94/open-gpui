use open_gpui::{InteractiveElement as _, div};
use std::cell::Cell;

#[test]
fn debug_selector_evaluates_closures_only_when_recording_is_enabled() {
    let evaluations = Cell::new(0);
    let element = div().debug_selector(|| {
        evaluations.set(evaluations.get() + 1);
        "selector:closure".to_owned()
    });
    assert_eq!(
        evaluations.get(),
        usize::from(cfg!(all(debug_assertions, feature = "test-support")))
    );
    drop(element);
}

#[cfg(feature = "test-support")]
mod recording {
    use super::*;
    use open_gpui::{
        Bounds, Context, FocusHandle, IntoElement, Point, Render, Styled as _, TestAppContext,
        Window, px, size,
    };
    use std::rc::Rc;

    struct SelectorProbe {
        focus: FocusHandle,
        evaluations: Rc<Cell<usize>>,
    }

    impl Render for SelectorProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let evaluations = self.evaluations.clone();
            div()
                .id("selector-probe")
                .size(px(24.))
                .track_focus(&self.focus)
                .debug_selector(|| {
                    evaluations.set(evaluations.get() + 1);
                    "selector:probe".to_owned()
                })
        }
    }

    #[open_gpui::test]
    fn debug_selectors_record_bounds_and_focus_only_in_debug_builds(cx: &mut TestAppContext) {
        let evaluations = Rc::new(Cell::new(0));
        let (view, cx) = cx.add_window_view({
            let evaluations = evaluations.clone();
            move |_, cx| SelectorProbe {
                focus: cx.focus_handle(),
                evaluations,
            }
        });
        let focus = view.read_with(cx, |view, _| view.focus.clone());
        cx.update(|window, cx| {
            focus.focus(window, cx);
            window.draw(cx).clear(cx);
        });
        let enabled = cfg!(debug_assertions);
        assert_eq!(evaluations.get() > 0, enabled);
        assert_eq!(
            cx.debug_bounds("selector:probe"),
            enabled.then(|| Bounds::new(Point::default(), size(px(24.), px(24.))))
        );
        assert_eq!(cx.debug_selector_is_focused("selector:probe"), enabled);
        assert_eq!(
            cx.focused_debug_selector(),
            enabled.then(|| "selector:probe".to_owned())
        );
        assert_eq!(
            cx.debug_selectors_with_prefix("selector:"),
            if enabled {
                vec!["selector:probe".to_owned()]
            } else {
                Vec::new()
            }
        );
    }
}
