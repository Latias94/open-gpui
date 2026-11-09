use super::*;
use crate::{DebugFrameOverlayMode, Entity, StyleRefinement, rgba};

struct RestoreOverlayTrace(bool);

impl Drop for RestoreOverlayTrace {
    fn drop(&mut self) {
        profiler::set_trace_enabled(self.0);
    }
}

fn stats(fixture: &WindowFixture, cx: &mut TestAppContext) -> (usize, u64) {
    fixture.update(cx, |window, _| window.debug_frame_overlay.stats_for_test())
}

fn panel_count(window: &Window) -> usize {
    let color: Hsla = rgba(0x000000aa).into();
    window
        .rendered_frame
        .scene
        .quads
        .iter()
        .filter(|quad| quad.background.solid == color)
        .count()
}

#[open_gpui::test]
fn hidden_history_and_explicit_controls_do_not_create_a_redraw_loop(cx: &mut TestAppContext) {
    let _restore = RestoreOverlayTrace(profiler::trace_enabled());
    profiler::set_trace_enabled(false);
    let fixture = WindowFixture::new(cx);
    let baseline = stats(&fixture, cx);
    fixture.draw(cx);
    assert_eq!(stats(&fixture, cx), (baseline.0 + 1, baseline.1 + 1));
    fixture.update(cx, |window, _| {
        assert_eq!(
            window.debug_frame_overlay_mode(),
            DebugFrameOverlayMode::Hidden
        );
        assert_eq!(panel_count(window), 0);
        assert!(!window.refresh_pending_for_test());
        window.cycle_debug_frame_overlay_mode();
        assert!(window.refresh_pending_for_test());
    });
    fixture.draw(cx);
    fixture.update(cx, |window, _| {
        assert_eq!(
            window.debug_frame_overlay_mode(),
            DebugFrameOverlayMode::Minimal
        );
        assert_eq!(panel_count(window), 1);
        assert!(window.rendered_frame.scene.quads.len() > 2);
        assert!(!window.refresh_pending_for_test());
        assert!(window.next_frame_callbacks.borrow().is_empty());
        window.reset_debug_frame_overlay_stats();
        assert_eq!(
            window.debug_frame_overlay.stats_for_test(),
            (0, baseline.1 + 2)
        );
        assert!(window.refresh_pending_for_test());
    });
    fixture.draw(cx);
    assert_eq!(stats(&fixture, cx), (1, baseline.1 + 3));
    fixture.update(cx, |window, _| {
        assert_eq!(panel_count(window), 1);
        assert!(!window.refresh_pending_for_test());
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Hidden);
    });
    fixture.draw(cx);
    fixture.update(cx, |window, _| {
        assert_eq!(panel_count(window), 0);
        assert_eq!(window.rendered_frame.scene.quads.len(), 1);
        assert!(!window.refresh_pending_for_test());
    });
}

#[open_gpui::test]
fn old_scene_present_and_idle_callback_preserve_overlay_draw_history(cx: &mut TestAppContext) {
    let fixture = WindowFixture::new(cx);
    fixture.update(cx, |window, _| {
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Full);
    });
    let generation = fixture.draw(cx);
    let baseline = stats(&fixture, cx);
    for outcome in [
        PlatformWindowPresentOutcome::Deferred,
        PlatformWindowPresentOutcome::Rejected,
        PlatformWindowPresentOutcome::Submitted,
        PlatformWindowPresentOutcome::Submitted,
    ] {
        assert_eq!(fixture.present(cx, outcome).generation, generation);
        assert_eq!(stats(&fixture, cx), baseline);
    }
    fixture.update(cx, |window, app| {
        window.native_frame_requested(crate::RequestFrameOptions::default(), app);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(window.debug_frame_overlay.stats_for_test(), baseline);
        assert_eq!(panel_count(window), 1);
    });
}

#[open_gpui::test]
fn refused_overlay_candidate_preserves_previous_scene_and_history_until_retry(
    cx: &mut TestAppContext,
) {
    let fixture = WindowFixture::new(cx);
    let baseline = stats(&fixture, cx);
    let generation = fixture.update(cx, |window, _| {
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Full);
        window.rendered_frame_revision()
    });
    fixture.controls.paint_sprite.set(true);
    fixture.atlas.fail_next_lease();
    fixture.update(cx, |window, app| {
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(window.debug_frame_overlay.stats_for_test(), baseline);
        assert_eq!(window.rendered_frame.scene.quads.len(), 1);
        assert_eq!(panel_count(window), 0);
        assert!(window.refresh_pending_for_test());
    });
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        generation,
    );
    assert_eq!(stats(&fixture, cx), baseline);
    fixture.controls.paint_sprite.set(false);
    fixture.draw(cx);
    fixture.update(cx, |window, _| {
        assert_eq!(
            window.debug_frame_overlay.stats_for_test(),
            (baseline.0 + 1, baseline.1 + 1)
        );
        assert_eq!(panel_count(window), 1);
        assert!(!window.refresh_pending_for_test());
    });
}

#[open_gpui::test]
fn render_and_accepted_cleanup_unwind_do_not_add_overlay_samples(cx: &mut TestAppContext) {
    let fixture = WindowFixture::new(cx);
    fixture.update(cx, |window, _| {
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Full);
    });
    let generation = fixture.draw(cx);
    let baseline = stats(&fixture, cx);
    fixture.controls.panic_render.set(true);
    fixture.update(cx, |window, app| {
        window.refresh();
        let panic = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app))).unwrap_err();
        assert_injected_panic(panic.as_ref(), RENDER_PANIC);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(window.debug_frame_overlay.stats_for_test(), baseline);
    });
    fixture.draw(cx);
    let recovered = stats(&fixture, cx);
    assert_eq!(recovered, (baseline.0 + 1, baseline.1 + 1));
    fixture.controls.panic_publication.set(true);
    fixture.update(cx, |window, app| {
        let previous = window.rendered_frame_revision();
        window.refresh();
        let panic = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app))).unwrap_err();
        assert_injected_panic(panic.as_ref(), PUBLICATION_PANIC);
        assert_eq!(window.rendered_frame_revision(), previous + 1);
        assert_eq!(window.debug_frame_overlay.stats_for_test(), recovered);
        assert_eq!(panel_count(window), 1);
    });
    fixture.draw(cx);
    assert_eq!(stats(&fixture, cx), (recovered.0 + 1, recovered.1 + 1));
}

struct CachedOverlayLeaf(Rc<Cell<usize>>);

impl Render for CachedOverlayLeaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.set(self.0.get() + 1);
        canvas(
            |_, _, _| (),
            |bounds, (), window, _| window.paint_quad(fill(bounds, red())),
        )
        .size_full()
    }
}

struct CachedOverlayRoot {
    leaf: Entity<CachedOverlayLeaf>,
    renders: Rc<Cell<usize>>,
}

impl Render for CachedOverlayRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().child(
            self.leaf
                .clone()
                .cached(StyleRefinement::default().size(px(100.))),
        )
    }
}

#[open_gpui::test]
fn cached_view_replay_neither_retains_nor_duplicates_overlay_primitives(cx: &mut TestAppContext) {
    let leaf_renders = Rc::new(Cell::new(0));
    let root_renders = Rc::new(Cell::new(0));
    let handle = cx.add_window({
        let leaf_renders = leaf_renders.clone();
        let renders = root_renders.clone();
        move |_, app| CachedOverlayRoot {
            leaf: app.new(|_| CachedOverlayLeaf(leaf_renders)),
            renders,
        }
    });
    let platform = cx.test_window(handle.into());
    platform.defer_frame_requests_for_test();
    cx.run_until_parked();
    let _batch = ManualFrameBatch::new(cx);
    cx.update_window(handle.into(), |_, window, app| {
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Full);
        window.draw(app).clear(app);
        assert_eq!(panel_count(window), 1);
    })
    .unwrap();
    let leaf_baseline = leaf_renders.get();
    let root_baseline = root_renders.get();
    for _ in 0..3 {
        handle.update(cx, |_, _, app| app.notify()).unwrap();
        cx.update_window(handle.into(), |_, window, app| {
            window.draw(app).clear(app);
            assert_eq!(panel_count(window), 1);
            assert!(!window.refresh_pending_for_test());
        })
        .unwrap();
    }
    assert_eq!(leaf_renders.get(), leaf_baseline);
    assert_eq!(root_renders.get(), root_baseline + 3);
    cx.update_window(handle.into(), |_, window, app| {
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Hidden);
        window.draw(app).clear(app);
        assert_eq!(panel_count(window), 0);
        assert_eq!(window.rendered_frame.scene.quads.len(), 1);
    })
    .unwrap();
}

struct OverlayGeometryRoot;

impl Render for OverlayGeometryRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(100.))
    }
}

#[open_gpui::test]
fn overlay_geometry_refusal_rejects_the_candidate_and_keeps_history_until_valid_retry(
    cx: &mut TestAppContext,
) {
    let handle = cx.add_window(|_, _| OverlayGeometryRoot);
    let platform = cx.test_window(handle.into());
    platform.defer_frame_requests_for_test();
    cx.run_until_parked();
    let _batch = ManualFrameBatch::new(cx);
    cx.update_window(handle.into(), |_, window, app| {
        let generation = window.rendered_frame_revision();
        let viewport = window.viewport_size;
        let scale_factor = window.scale_factor;
        let baseline = window.debug_frame_overlay.stats_for_test();
        window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Full);
        window.viewport_size = crate::size(px(f32::MAX), px(200.));
        window.scale_factor = 2.0;
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(window.debug_frame_overlay.stats_for_test(), baseline);
        assert_eq!(panel_count(window), 0);
        assert!(window.refresh_pending_for_test());
        window.viewport_size = viewport;
        window.scale_factor = scale_factor;
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert_eq!(
            window.debug_frame_overlay.stats_for_test(),
            (baseline.0 + 1, baseline.1 + 1),
        );
        assert_eq!(panel_count(window), 1);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
}
