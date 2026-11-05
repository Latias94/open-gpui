use super::AnyViewState;
use crate::{
    AnyView, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, StyleRefinement, Styled as _, TestAppContext, Window,
    WindowHandle, canvas, div, point, px,
};
use std::{cell::Cell, rc::Rc};

struct PaintModel {
    value: usize,
    output: Rc<Cell<usize>>,
}

struct PaintReadView {
    model: Entity<PaintModel>,
    renders: Rc<Cell<usize>>,
}

impl Render for PaintReadView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let model = self.model.clone();
        canvas(
            |_, _, _| {},
            move |_, _, _, cx| {
                let model = model.read(cx);
                model.output.set(model.value);
            },
        )
        .size(px(50.))
    }
}

struct PaintReadRoot {
    child: Entity<PaintReadView>,
    renders: Rc<Cell<usize>>,
}

impl Render for PaintReadRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        AnyView::from(self.child.clone()).cached(StyleRefinement::default().size(px(100.)))
    }
}

#[open_gpui::test]
fn cached_view_paint_reads_keep_subscriptions_until_rebuilt(cx: &mut TestAppContext) {
    let output = Rc::new(Cell::new(0));
    let child_renders = Rc::new(Cell::new(0));
    let root_renders = Rc::new(Cell::new(0));
    let first = cx.new(|_| PaintModel {
        value: 1,
        output: output.clone(),
    });
    let second = cx.new(|_| PaintModel {
        value: 7,
        output: output.clone(),
    });
    let child = cx.new(|_| PaintReadView {
        model: first.clone(),
        renders: child_renders.clone(),
    });
    let window = cx.add_window(|_, _| PaintReadRoot {
        child: child.clone(),
        renders: root_renders.clone(),
    });
    assert_eq!(output.get(), 1);
    let initial_renders = child_renders.get();

    for _ in 0..2 {
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("notify root");
        assert_eq!(child_renders.get(), initial_renders, "reuse cached paint");
    }

    let before_notify = root_renders.get();
    first.update(cx, |model, cx| {
        model.value = 2;
        cx.notify();
    });
    assert!(
        root_renders.get() > before_notify,
        "a paint-only read must keep the window subscribed after cache reuse"
    );
    assert_eq!(child_renders.get(), initial_renders);
    assert_eq!(output.get(), 1);
    window
        .update(cx, |_, window, _| window.refresh())
        .expect("refresh canvas");
    assert_eq!(output.get(), 2);

    child.update(cx, |child, cx| {
        child.model = second.clone();
        cx.notify();
    });
    assert_eq!(output.get(), 7);
    let rebuilt_renders = child_renders.get();
    for _ in 0..2 {
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("notify root after rebuild");
        assert_eq!(child_renders.get(), rebuilt_renders);
    }
    let before_old_notify = root_renders.get();
    first.update(cx, |_, cx| cx.notify());
    assert_eq!(
        root_renders.get(),
        before_old_notify,
        "rebuilding must unsubscribe the window from obsolete paint dependencies"
    );
    second.update(cx, |_, cx| cx.notify());
    assert!(root_renders.get() > before_old_notify);
    assert_eq!(child_renders.get(), rebuilt_renders);
}

struct OpacityChild {
    renders: usize,
}

impl Render for OpacityChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        div().size_full().bg(crate::rgb(0xff0000))
    }
}

struct OpacityRoot {
    opacity: f32,
    width: Pixels,
    child: Entity<OpacityChild>,
}

impl Render for OpacityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("opacity-parent")
            .w(self.width)
            .h(px(200.))
            .opacity(self.opacity)
            .hover(|style| style.opacity(1.))
            .child(
                AnyView::from(self.child.clone()).cached(StyleRefinement::default().size(px(100.))),
            )
    }
}

fn frame_revision(window: WindowHandle<OpacityRoot>, cx: &mut TestAppContext) -> u64 {
    cx.update_window(window.into(), |_, window, _| {
        window.rendered_frame_revision()
    })
    .expect("read committed revision")
}

/// The test App draws notifications synchronously. Drain the requested animation
/// callbacks between those draws, counting accepted frames from before the update.
fn settle_opacity(
    window: WindowHandle<OpacityRoot>,
    child: &Entity<OpacityChild>,
    cx: &mut TestAppContext,
    start_revision: u64,
) -> (f32, usize) {
    for _ in 0..3 {
        let (alpha, renders, requested_frames) = cx
            .update_window(window.into(), |_, window, cx| {
                let frames = window.rendered_frame_revision() - start_revision;
                assert!((1..=3).contains(&frames), "opacity took {frames} frames");
                let quads = &window.rendered_frame.scene.quads;
                assert_eq!(quads.len(), 1, "only the cached child paints a quad");
                let alpha = quads[0].background.solid.a;
                let renders = child.read(cx).renders;
                let requested_frames = window.drain_next_frame_callbacks_for_test(cx);
                if requested_frames == 0 {
                    assert!(
                        !window.refresh_pending_for_test(),
                        "settled opacity must leave no dirty follow-up"
                    );
                }
                (alpha, renders, requested_frames)
            })
            .expect("read painted opacity and advance requested frames");
        if requested_frames == 0 {
            return (alpha, renders);
        }
    }
    panic!("cached child kept requesting frames");
}

fn update_opacity(
    window: WindowHandle<OpacityRoot>,
    child: &Entity<OpacityChild>,
    cx: &mut TestAppContext,
    update: impl FnOnce(&mut OpacityRoot, &mut Window, &mut Context<OpacityRoot>),
) -> (f32, usize) {
    let start_revision = frame_revision(window, cx);
    window
        .update(cx, |root, window, cx| {
            update(root, window, cx);
            cx.notify();
        })
        .expect("update opacity root");
    settle_opacity(window, child, cx, start_revision)
}

#[open_gpui::test]
fn cached_view_follows_inherited_opacity(cx: &mut TestAppContext) {
    let child = cx.new(|_| OpacityChild { renders: 0 });
    let window = cx.add_window(|_, _| OpacityRoot {
        opacity: 1.,
        width: px(200.),
        child: child.clone(),
    });
    let outside = point(px(500.), px(500.));
    let inside = point(px(50.), px(50.));
    let beside = point(px(250.), px(50.));

    assert_eq!(
        update_opacity(window, &child, cx, |_, window, cx| {
            window.simulate_mouse_move(outside, cx)
        }),
        (1., 1)
    );
    assert_eq!(update_opacity(window, &child, cx, |_, _, _| {}), (1., 1));
    assert_eq!(
        update_opacity(window, &child, cx, |root, _, _| root.opacity = 0.5),
        (0.5, 2)
    );
    assert_eq!(update_opacity(window, &child, cx, |_, _, _| {}), (0.5, 2));

    let hovered = update_opacity(window, &child, cx, |_, window, cx| {
        window.simulate_mouse_move(inside, cx)
    });
    assert_eq!(hovered.0, 1.);
    assert!(hovered.1 > 2);
    assert_eq!(update_opacity(window, &child, cx, |_, _, _| {}), hovered);
    let unhovered = update_opacity(window, &child, cx, |_, window, cx| {
        window.simulate_mouse_move(beside, cx)
    });
    assert_eq!(unhovered.0, 0.5);
    assert!(unhovered.1 > hovered.1);
    assert_eq!(update_opacity(window, &child, cx, |_, _, _| {}), unhovered);

    let start_revision = frame_revision(window, cx);
    window
        .update(cx, |root, _, cx| {
            root.width = px(300.);
            cx.notify();
        })
        .expect("expand parent under stationary pointer");
    cx.update_window(window.into(), |_, window, cx| {
        assert_eq!(window.rendered_frame_revision(), start_revision + 1);
        assert_eq!(child.read(cx).renders, unhovered.1, "replay before rebuild");
        let quads = &window.rendered_frame.scene.quads;
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].background.solid.a, 0.5);
        let mut cached_states = window
            .rendered_frame
            .element_states
            .values()
            .filter_map(|state| state.inner.downcast_ref::<Option<AnyViewState>>())
            .flatten();
        let state = cached_states.next().expect("cached child state");
        assert!(cached_states.next().is_none());
        assert_eq!(state.cache_key.opacity.to_bits(), 0.5_f32.to_bits());
        assert_eq!(
            window.drain_next_frame_callbacks_for_test(cx),
            1,
            "opacity mismatch schedules one follow-up"
        );
    })
    .expect("inspect opacity replay");
    assert_eq!(
        settle_opacity(window, &child, cx, start_revision),
        (1., unhovered.1 + 1)
    );
    cx.update_window(window.into(), |_, window, _| {
        let state = window
            .rendered_frame
            .element_states
            .values()
            .find_map(|state| {
                state
                    .inner
                    .downcast_ref::<Option<AnyViewState>>()
                    .and_then(Option::as_ref)
            })
            .expect("rebuilt cached child state");
        assert_eq!(state.cache_key.opacity.to_bits(), 1.0_f32.to_bits());
    })
    .expect("fresh key records the opacity actually painted");
}

#[open_gpui::test]
fn cached_view_reuses_same_bit_nan_inherited_opacity(cx: &mut TestAppContext) {
    let child = cx.new(|_| OpacityChild { renders: 0 });
    let opacity = f32::from_bits(0x7fc0_1234);
    let window = cx.add_window(|_, _| OpacityRoot {
        opacity,
        width: px(200.),
        child: child.clone(),
    });
    let (alpha, renders) = update_opacity(window, &child, cx, |_, window, cx| {
        window.simulate_mouse_move(point(px(500.), px(500.)), cx)
    });
    assert!(alpha.is_nan());
    for _ in 0..2 {
        let (alpha, reused_renders) = update_opacity(window, &child, cx, |root, _, _| {
            root.opacity = f32::from_bits(opacity.to_bits());
        });
        assert!(alpha.is_nan());
        assert_eq!(reused_renders, renders, "same-bit NaN must reuse paint");
    }
    assert_eq!(
        update_opacity(window, &child, cx, |root, _, _| root.opacity = 0.5),
        (0.5, renders + 1)
    );
}
