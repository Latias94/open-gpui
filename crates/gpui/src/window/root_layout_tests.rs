use crate::{
    App, AppContext as _, Bounds, Context, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Pixels, PromptLevel, PromptResponse, Render, Size, Styled as _,
    TestAppContext, Window, canvas, div, px, relative, size,
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
enum RootSize {
    Auto,
    Explicit,
    Width,
    Height,
    Percentage,
    Constrained,
}

struct RootProbe {
    sizing: RootSize,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Render for RootProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.bounds.clone();
        let root = div()
            .flex()
            .flex_col()
            .child(canvas(move |value, _, _| bounds.set(value), |_, _, _, _| {}).size_full());
        match self.sizing {
            RootSize::Auto => root,
            RootSize::Explicit => root.w(px(120.)).h(px(80.)),
            RootSize::Width => root.w(px(120.)),
            RootSize::Height => root.h(px(80.)),
            RootSize::Percentage => root.w(relative(0.5)).h(relative(0.25)),
            RootSize::Constrained => root.min_w(px(400.)).max_h(px(90.)),
        }
    }
}

#[test]
fn window_roots_fill_only_auto_axes_and_recompute_after_notify_and_resize() {
    let mut app = TestAppContext::single();
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let handle = app.open_window(size(px(320.), px(200.)), {
        let bounds = bounds.clone();
        move |_, _| RootProbe {
            sizing: RootSize::Auto,
            bounds,
        }
    });
    app.run_until_parked();
    assert_eq!(bounds.get().size, size(px(320.), px(200.)));
    for (sizing, expected) in [
        (RootSize::Explicit, size(px(120.), px(80.))),
        (RootSize::Width, size(px(120.), px(200.))),
        (RootSize::Height, size(px(320.), px(80.))),
        (RootSize::Percentage, size(px(160.), px(50.))),
        (RootSize::Constrained, size(px(400.), px(90.))),
        (RootSize::Auto, size(px(320.), px(200.))),
    ] {
        let before = app
            .update_window(handle.into(), |_, window, _| {
                window.rendered_frame_revision()
            })
            .unwrap();
        handle
            .update(&mut app, |view, _, cx| {
                view.sizing = sizing;
                cx.notify();
            })
            .unwrap();
        app.run_until_parked();
        assert_eq!(bounds.get().size, expected);
        app.update_window(handle.into(), |_, window, _| {
            let revision = window.rendered_frame_revision();
            assert!(revision > before);
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                Some(revision)
            );
        })
        .unwrap();
    }
    let mut platform_window = app
        .update_window(handle.into(), |_, window, _| {
            window.platform_window.as_test().unwrap().clone()
        })
        .unwrap();
    platform_window.simulate_resize(size(px(480.), px(260.)));
    app.run_until_parked();
    assert_eq!(bounds.get().size, size(px(480.), px(260.)));
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))
        .unwrap();
    assert!(app.read(|cx| cx.windows().is_empty()));
}

struct PromptProbe {
    root: RootProbe,
    focus: FocusHandle,
}

impl EventEmitter<PromptResponse> for PromptProbe {}

impl Focusable for PromptProbe {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PromptProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.root.bounds.clone();
        let root = div()
            .flex()
            .child(canvas(move |value, _, _| bounds.set(value), |_, _, _, _| {}).size_full());
        match self.root.sizing {
            RootSize::Explicit => root.w(px(120.)).h(px(80.)),
            _ => root,
        }
    }
}

#[test]
fn custom_prompt_roots_fill_auto_dimensions_and_preserve_explicit_sizes() {
    for sizing in [RootSize::Auto, RootSize::Explicit] {
        let mut app = TestAppContext::single();
        let bounds = Rc::new(Cell::new(Bounds::default()));
        app.update({
            let bounds = bounds.clone();
            move |cx| {
                cx.set_prompt_builder(move |_, _, _, _, handle, window, cx| {
                    let bounds = bounds.clone();
                    let prompt = cx.new(|cx| PromptProbe {
                        root: RootProbe { sizing, bounds },
                        focus: cx.focus_handle(),
                    });
                    handle.with_view(prompt, window, cx)
                });
            }
        });
        let handle = app.open_window(size(px(320.), px(200.)), |_, _| crate::Empty);
        let _response = app
            .update_window(handle.into(), |_, window, cx| {
                window.prompt(PromptLevel::Info, "Sizing", None, &["OK"], cx)
            })
            .unwrap();
        app.run_until_parked();
        let expected = match sizing {
            RootSize::Explicit => size(px(120.), px(80.)),
            _ => size(px(320.), px(200.)),
        };
        assert_eq!(bounds.get().size, expected);
        app.update_window(handle.into(), |_, window, cx| {
            assert!(window.prompt.is_some());
            window.remove_window(cx);
        })
        .unwrap();
        assert!(app.read(|cx| cx.windows().is_empty()));
    }
}

#[test]
fn separately_measured_roots_retain_shrink_wrap_sizing() {
    let mut app = TestAppContext::single();
    let mut visual = app.add_empty_window();
    visual.draw(
        crate::Point::default(),
        size(px(320.), px(200.)),
        |window, cx| {
            let mut element = div()
                .flex()
                .child(div().w(px(37.)).h(px(19.)))
                .into_any_element();
            let measured: Size<Pixels> = element.layout_as_root(
                size(
                    crate::AvailableSpace::MaxContent,
                    crate::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            assert_eq!(measured, size(px(37.), px(19.)));
            div()
        },
    );
}
