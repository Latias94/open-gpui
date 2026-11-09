//! Profiler regressions exercised through real windows and the test platform renderer.

mod overlay;
mod refresh;

use super::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    AnyWindowHandle, App, AppCell, AppContext as _, AtlasTextureKind, Context, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke, MonochromeSprite, MouseButton,
    MouseDownEvent, ParentElement as _, PlatformInput, PlatformWindowPresentOutcome,
    PrepaintPublicationId, PrimitiveTransform, Render, Styled as _, TestAppContext, TestWindow,
    Window, WindowHandle, WindowId, WindowPresentAttemptFacts, blue, canvas, div, fill, point,
    profiler::{
        self, FrameEvent, FrameTiming, FrameTimingCollector, InputLatencySnapshot, PresentTiming,
    },
    px, red,
};
use open_gpui_scheduler::Instant;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

const RENDER_PANIC: &str = "injected window profiler render panic";
const PUBLICATION_PANIC: &str = "injected window profiler publication panic";

#[derive(Default)]
struct ProbeControls {
    inputs: Cell<u64>,
    paint_sprite: Cell<bool>,
    panic_render: Cell<bool>,
    panic_publication: Cell<bool>,
    publication_generation: Cell<Option<u64>>,
    following_publication_generation: Cell<Option<u64>>,
    publication_discards: Cell<u64>,
    nested_draw: Cell<Option<AnyWindowHandle>>,
    key_refresh: Cell<bool>,
    keyboard_style: Cell<bool>,
}

struct ProfilerProbe {
    controls: Rc<ProbeControls>,
    publication: PrepaintPublicationId,
    following_publication: PrepaintPublicationId,
    focus: FocusHandle,
}

impl Render for ProfilerProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        assert!(!self.controls.panic_render.replace(false), "{RENDER_PANIC}");
        let input_controls = self.controls.clone();
        let prepaint_controls = self.controls.clone();
        let paint_controls = self.controls.clone();
        let key_controls = self.controls.clone();
        let publication = self.publication;
        let following_publication = self.following_publication;
        let color = if self.controls.inputs.get() % 2 == 0 {
            red()
        } else {
            blue()
        };
        let mut root = div().id("window-profiler-probe").size_full();
        if self.controls.keyboard_style.get() {
            root = root
                .track_focus(&self.focus)
                .focus_visible(|style| style.border_1());
        }
        root.on_mouse_down(MouseButton::Left, move |_, window, _| {
            input_controls.inputs.set(input_controls.inputs.get() + 1);
            window.refresh();
        })
        .on_key_down(move |_, window, _| {
            if key_controls.key_refresh.get() {
                window.refresh();
            }
        })
        .child(
            canvas(
                move |_, window, _| {
                    if prepaint_controls.panic_publication.replace(false) {
                        let controls = prepaint_controls.clone();
                        let discards = prepaint_controls.clone();
                        window.record_prepaint_window_transaction(
                            publication,
                            move |fence, window, _| {
                                assert!(fence.is_satisfied_by(window));
                                controls
                                    .publication_generation
                                    .set(Some(fence.generation()));
                                panic!("{PUBLICATION_PANIC}");
                            },
                            move |_, _, _| {
                                discards
                                    .publication_discards
                                    .set(discards.publication_discards.get() + 1)
                            },
                        );
                        let following = prepaint_controls.clone();
                        window.record_prepaint_window_transaction(
                            following_publication,
                            move |fence, window, _| {
                                assert!(fence.is_satisfied_by(window));
                                following
                                    .following_publication_generation
                                    .set(Some(fence.generation()));
                            },
                            |_, _, _| {},
                        );
                    }
                },
                move |bounds, _, window, cx| {
                    if let Some(inner) = paint_controls.nested_draw.take() {
                        cx.update_window(inner, |_, window, cx| {
                            window.dispatch_event(input_event(), cx);
                            window.draw(cx).clear(cx);
                            assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Deferred);
                        })
                        .unwrap();
                    }
                    window.paint_quad(fill(bounds, color));
                    if paint_controls.paint_sprite.get() {
                        let _ = window.insert_scene_primitive(MonochromeSprite {
                            order: 1,
                            pad: 0,
                            bounds: window.device_local_bounds(bounds),
                            clip: Default::default(),
                            color,
                            tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                            transform: PrimitiveTransform::IDENTITY,
                        });
                    }
                },
            )
            .size_full(),
        )
    }
}

fn input_event() -> PlatformInput {
    PlatformInput::MouseDown(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(8.), px(8.)),
        click_count: 1,
        ..Default::default()
    })
}

struct WindowFixture {
    handle: WindowHandle<ProfilerProbe>,
    platform: TestWindow,
    controls: Rc<ProbeControls>,
    atlas: Arc<RejectOnceAtlas>,
    baseline_generation: u64,
    baseline_draw_samples: u64,
    baseline_present_samples: u64,
    _batch: ManualFrameBatch,
}

struct ManualFrameBatch(Rc<AppCell>);

impl ManualFrameBatch {
    fn new(cx: &TestAppContext) -> Self {
        cx.update(|app| app.start_update());
        Self(cx.app.clone())
    }
}

impl Drop for ManualFrameBatch {
    fn drop(&mut self) {
        let already_unwinding = std::thread::panicking();
        let cleanup = catch_unwind(AssertUnwindSafe(|| self.0.borrow_mut().finish_update()));
        if let Err(panic) = cleanup
            && !already_unwinding
        {
            std::panic::resume_unwind(panic);
        }
    }
}

impl WindowFixture {
    fn new(cx: &mut TestAppContext) -> Self {
        let controls = Rc::new(ProbeControls::default());
        let atlas = Arc::new(RejectOnceAtlas::new());
        let handle = cx.add_window({
            let controls = controls.clone();
            let atlas = atlas.clone();
            move |window, app| {
                window.set_sprite_atlas_for_test(atlas);
                ProfilerProbe {
                    controls,
                    publication: PrepaintPublicationId::new(),
                    following_publication: PrepaintPublicationId::new(),
                    focus: app.focus_handle(),
                }
            }
        });
        let platform = cx.test_window(handle.into());
        platform.defer_frame_requests_for_test();
        cx.run_until_parked();
        let batch = ManualFrameBatch::new(cx);
        let (baseline_generation, baseline_draw_samples, baseline_present_samples) = cx
            .update_window(handle.into(), |_, window, cx| {
                window.draw(cx).clear(cx);
                assert_eq!(window.present(cx), PlatformWindowPresentOutcome::Submitted);
                window.reset_input_latency_measurements();
                let frames = window.frame_duration_snapshot();
                (
                    window.rendered_frame_revision(),
                    frames.draw_duration_histogram.len(),
                    frames.dirty_to_present_histogram.len(),
                )
            })
            .unwrap();
        Self {
            handle,
            platform,
            controls,
            atlas,
            baseline_generation,
            baseline_draw_samples,
            baseline_present_samples,
            _batch: batch,
        }
    }

    fn update<R>(
        &self,
        cx: &mut TestAppContext,
        update: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.handle.into(), |_, window, cx| update(window, cx))
            .unwrap()
    }

    fn input(&self, cx: &mut TestAppContext) {
        let before = self.controls.inputs.get();
        self.update(cx, |window, app| {
            window.dispatch_event(input_event(), app);
        });
        assert_eq!(self.controls.inputs.get(), before + 1);
        self.update(cx, |window, _| assert!(window.refresh_pending_for_test()));
    }

    fn draw(&self, cx: &mut TestAppContext) -> u64 {
        self.update(cx, |window, cx| {
            let previous = window.rendered_frame_revision();
            window.draw(cx).clear(cx);
            let generation = window.rendered_frame_revision();
            assert_eq!(generation, previous + 1);
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                Some(generation)
            );
            generation
        })
    }

    fn present(
        &self,
        cx: &mut TestAppContext,
        outcome: PlatformWindowPresentOutcome,
    ) -> WindowPresentAttemptFacts {
        cx.set_window_present_outcome(self.handle.into(), outcome);
        self.update(cx, |window, cx| {
            assert_eq!(window.present(cx), outcome);
            let facts = window.presentation_facts();
            let attempt = facts.latest_present_attempt.unwrap();
            assert_eq!(attempt.generation, window.rendered_frame_revision());
            assert_eq!(attempt.outcome, outcome);
            assert!(attempt.contained_valid_primitives);
            if outcome == PlatformWindowPresentOutcome::Submitted {
                assert_eq!(facts.present_submitted_generation, Some(attempt.generation));
            }
            attempt
        })
    }

    fn snapshot(&self, cx: &mut TestAppContext) -> InputLatencySnapshot {
        self.update(cx, |window, _| window.input_latency_snapshot())
    }

    fn assert_frame_samples(&self, cx: &mut TestAppContext, draws: u64, presents: u64) {
        self.update(cx, |window, _| {
            let frames = window.frame_duration_snapshot();
            assert_eq!(
                frames.draw_duration_histogram.len(),
                self.baseline_draw_samples + draws
            );
            assert_eq!(
                frames.dirty_to_present_histogram.len(),
                self.baseline_present_samples + presents
            );
            assert_eq!(window.input_latency_snapshot().drawn_frame_count, draws);
        });
    }

    fn scene_color(&self, cx: &mut TestAppContext) -> Hsla {
        self.update(cx, |window, _| {
            assert_eq!(window.rendered_frame.scene.quads.len(), 1);
            window.rendered_frame.scene.quads[0].background.solid
        })
    }
}

fn pending_dirty(window: &Window) -> (Option<Instant>, u64) {
    let dirty = window.invalidator.take_frame_dirty();
    window.invalidator.restore_frame_dirty(dirty);
    (dirty.dirty_at, dirty.invalidations)
}

fn window_events(collector: &mut FrameTimingCollector, window_id: WindowId) -> Vec<FrameEvent> {
    collector
        .collect_unseen()
        .into_iter()
        .filter(|event| match event {
            FrameEvent::Draw(frame) => frame.window_id == window_id,
            FrameEvent::Present(present) => present.window_id == window_id,
        })
        .collect()
}

fn only_draw(events: &[FrameEvent]) -> FrameTiming {
    let [FrameEvent::Draw(frame)] = events else {
        panic!("expected exactly one accepted draw, got {events:?}");
    };
    *frame
}

fn only_present(events: &[FrameEvent]) -> PresentTiming {
    let [FrameEvent::Present(present)] = events else {
        panic!("expected exactly one submitted frame, got {events:?}");
    };
    *present
}

fn assert_input_samples(snapshot: &InputLatencySnapshot, samples: u64, pending: u64) {
    assert_eq!(snapshot.latency_histogram.len(), samples);
    assert_eq!(snapshot.events_per_frame_histogram.len(), samples);
    assert_eq!(snapshot.pending_input_count, pending);
    assert_eq!(snapshot.mid_draw_events_dropped, 0);
}

fn assert_injected_panic(panic: &(dyn std::any::Any + Send), expected: &str) {
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
    assert_eq!(message, Some(expected));
}

#[open_gpui::test]
fn invalidating_input_records_one_submitted_window_frame(cx: &mut TestAppContext) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    fixture.input(cx);
    assert_input_samples(&fixture.snapshot(cx), 0, 1);
    let dirty = fixture.update(cx, |window, _| pending_dirty(window));
    assert!(dirty.0.is_some());
    assert!(dirty.1 > 0);

    let generation = fixture.draw(cx);
    assert_eq!(generation, fixture.baseline_generation + 1);
    let draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert_eq!(draw.dirty_at, dirty.0);
    assert_eq!(draw.invalidations, dirty.1);
    assert!(draw.draw_start <= draw.draw_end);
    assert_input_samples(&fixture.snapshot(cx), 0, 1);
    fixture.assert_frame_samples(cx, 1, 0);

    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        generation
    );
    let present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(draw.draw_end <= present.present_start);
    assert!(present.present_start <= present.present_end);
    let snapshot = fixture.snapshot(cx);
    assert_input_samples(&snapshot, 1, 0);
    assert_eq!(snapshot.events_per_frame_histogram.count_at(1), 1);
    assert_eq!(snapshot.present_attempt_count, 1);
    assert_eq!(snapshot.submitted_present_count, 1);
    assert_eq!(
        snapshot.refresh_request_count, 2,
        "{:?}",
        snapshot.refresh_callsites
    );
    assert_eq!(snapshot.coalesced_refresh_request_count, 1);
    assert_eq!(snapshot.refresh_callsites.len(), 2);
    assert!(snapshot.refresh_callsites.iter().any(|callsite| {
        callsite.file == file!()
            && callsite.request_count == 1
            && callsite.coalesced_request_count == 1
    }));
    assert!(snapshot.refresh_callsites.iter().any(|callsite| {
        callsite.file.ends_with("div.rs")
            && callsite.request_count == 1
            && callsite.coalesced_request_count == 0
    }));
    fixture.assert_frame_samples(cx, 1, 1);

    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        generation
    );
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());
    assert_input_samples(&fixture.snapshot(cx), 1, 0);
    assert_eq!(fixture.snapshot(cx).submitted_present_count, 2);
    fixture.assert_frame_samples(cx, 1, 1);
}

#[open_gpui::test]
fn deferred_and_rejected_attempts_retry_the_exact_scene_without_flushing(cx: &mut TestAppContext) {
    let _trace = profiler::trace_scope();
    for outcome in [
        PlatformWindowPresentOutcome::Deferred,
        PlatformWindowPresentOutcome::Rejected,
    ] {
        let fixture = WindowFixture::new(cx);
        let mut collector = FrameTimingCollector::new();
        fixture.input(cx);
        let generation = fixture.draw(cx);
        let draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
        let color = fixture.scene_color(cx);
        let platform_draws = fixture.platform.draw_count();
        for _ in 0..2 {
            assert_eq!(fixture.present(cx, outcome).generation, generation);
            assert_eq!(fixture.scene_color(cx), color);
            assert_input_samples(&fixture.snapshot(cx), 0, 1);
            assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());
            fixture.assert_frame_samples(cx, 1, 0);
            fixture.update(cx, |window, _| {
                assert_eq!(
                    window.presentation_facts().present_submitted_generation,
                    Some(fixture.baseline_generation)
                );
            });
        }
        assert_eq!(fixture.platform.draw_count(), platform_draws + 2);
        let snapshot = fixture.snapshot(cx);
        assert_eq!(snapshot.present_attempt_count, 2);
        assert_eq!(snapshot.submitted_present_count, 0);
        assert_eq!(
            snapshot.deferred_present_count,
            if outcome == PlatformWindowPresentOutcome::Deferred {
                2
            } else {
                0
            }
        );
        assert_eq!(
            snapshot.rejected_present_count,
            if outcome == PlatformWindowPresentOutcome::Rejected {
                2
            } else {
                0
            }
        );

        assert_eq!(
            fixture
                .present(cx, PlatformWindowPresentOutcome::Submitted)
                .generation,
            generation
        );
        let present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
        assert!(draw.draw_end <= present.present_start);
        assert_input_samples(&fixture.snapshot(cx), 1, 0);
        fixture.assert_frame_samples(cx, 1, 1);

        fixture.input(cx);
        let newer_generation = fixture.draw(cx);
        assert_eq!(newer_generation, generation + 1);
        assert_ne!(fixture.scene_color(cx), color);
        let newer_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
        assert_eq!(
            fixture
                .present(cx, PlatformWindowPresentOutcome::Submitted)
                .generation,
            newer_generation
        );
        let newer_present =
            only_present(&window_events(&mut collector, fixture.handle.window_id()));
        assert!(newer_draw.draw_end <= newer_present.present_start);
        assert_input_samples(&fixture.snapshot(cx), 2, 0);
        fixture.assert_frame_samples(cx, 2, 2);
    }
}

fn assert_late_input_waits_for_new_generation(cx: &mut TestAppContext, early_input: bool) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    if early_input {
        fixture.input(cx);
    } else {
        fixture.update(cx, |window, _| window.refresh());
    }
    let old_generation = fixture.draw(cx);
    let old_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Deferred)
            .generation,
        old_generation
    );
    fixture.input(cx);
    assert_input_samples(&fixture.snapshot(cx), 0, 1 + u64::from(early_input));
    let old_color = fixture.scene_color(cx);

    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        old_generation
    );
    let old_present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(old_draw.draw_end <= old_present.present_start);
    assert_input_samples(&fixture.snapshot(cx), u64::from(early_input), 1);
    assert_eq!(fixture.scene_color(cx), old_color);
    fixture.assert_frame_samples(cx, 1, 1);

    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        old_generation
    );
    assert_input_samples(&fixture.snapshot(cx), u64::from(early_input), 1);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());

    let newer_generation = fixture.draw(cx);
    assert_eq!(newer_generation, old_generation + 1);
    assert_ne!(fixture.scene_color(cx), old_color);
    let newer_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(old_present.present_end <= newer_draw.draw_start);
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        newer_generation
    );
    let newer_present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(newer_draw.draw_end <= newer_present.present_start);
    let snapshot = fixture.snapshot(cx);
    assert_input_samples(&snapshot, 1 + u64::from(early_input), 0);
    assert_eq!(
        snapshot.events_per_frame_histogram.count_at(1),
        1 + u64::from(early_input)
    );
    assert_eq!(snapshot.present_attempt_count, 4);
    assert_eq!(snapshot.deferred_present_count, 1);
    assert_eq!(snapshot.submitted_present_count, 3);
    fixture.assert_frame_samples(cx, 2, 2);
}

#[open_gpui::test]
fn input_after_accepted_draw_is_not_completed_by_that_scene(cx: &mut TestAppContext) {
    assert_late_input_waits_for_new_generation(cx, false);
}

#[open_gpui::test]
fn old_scene_submission_completes_only_input_included_in_its_draw(cx: &mut TestAppContext) {
    assert_late_input_waits_for_new_generation(cx, true);
}

#[open_gpui::test]
fn repaint_required_waits_for_a_new_scene_then_retries_that_exact_generation(
    cx: &mut TestAppContext,
) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    fixture.input(cx);
    let invalidated_generation = fixture.draw(cx);
    only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::RepaintRequired)
            .generation,
        invalidated_generation
    );
    assert_input_samples(&fixture.snapshot(cx), 0, 1);
    fixture.assert_frame_samples(cx, 1, 0);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());

    let platform_draws = fixture.platform.draw_count();
    cx.set_window_present_outcome(
        fixture.handle.into(),
        PlatformWindowPresentOutcome::Submitted,
    );
    fixture.update(cx, |window, cx| {
        assert!(window.renderer_repaint_is_pending());
        assert_eq!(
            window.present(cx),
            PlatformWindowPresentOutcome::RepaintRequired
        );
        assert_eq!(window.rendered_frame_revision(), invalidated_generation);
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            Some(fixture.baseline_generation)
        );
    });
    assert_eq!(fixture.platform.draw_count(), platform_draws);
    assert_input_samples(&fixture.snapshot(cx), 0, 1);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());

    let recovery_generation = fixture.draw(cx);
    assert_eq!(recovery_generation, invalidated_generation + 1);
    let recovery_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Deferred)
            .generation,
        recovery_generation
    );
    assert_input_samples(&fixture.snapshot(cx), 0, 1);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        recovery_generation
    );
    let present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(recovery_draw.draw_end <= present.present_start);
    let snapshot = fixture.snapshot(cx);
    assert_input_samples(&snapshot, 1, 0);
    assert_eq!(snapshot.present_attempt_count, 4);
    assert_eq!(snapshot.repaint_required_present_count, 2);
    assert_eq!(snapshot.deferred_present_count, 1);
    assert_eq!(snapshot.submitted_present_count, 1);
    fixture.assert_frame_samples(cx, 2, 1);
}

#[open_gpui::test]
fn atlas_refusal_preserves_accepted_timing_input_scene_and_first_dirty_timestamp(
    cx: &mut TestAppContext,
) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    fixture.input(cx);
    let accepted_generation = fixture.draw(cx);
    let accepted_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    let accepted_color = fixture.scene_color(cx);
    fixture.input(cx);
    let dirty = fixture.update(cx, |window, _| pending_dirty(window));
    fixture.controls.paint_sprite.set(true);
    fixture.atlas.fail_next_lease();
    let platform_draws = fixture.platform.draw_count();
    fixture.update(cx, |window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), accepted_generation);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(accepted_generation)
        );
        let rejection = window.last_atlas_frame_rejection.as_ref().unwrap();
        assert_eq!(rejection.generation, accepted_generation + 1);
        assert!(matches!(
            rejection.error,
            crate::AtlasTextureLeaseError::TextureUnavailable { .. }
        ));
        let retained_dirty = pending_dirty(window);
        assert_eq!(retained_dirty.0, dirty.0);
        assert!(retained_dirty.1 >= dirty.1);
        assert!(window.refresh_pending_for_test());
        assert!(window.rendered_frame.scene.monochrome_sprites.is_empty());
    });
    assert_eq!(fixture.scene_color(cx), accepted_color);
    assert_eq!(fixture.platform.draw_count(), platform_draws);
    assert_input_samples(&fixture.snapshot(cx), 0, 2);
    fixture.assert_frame_samples(cx, 1, 0);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());

    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        accepted_generation
    );
    let accepted_present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(accepted_draw.draw_end <= accepted_present.present_start);
    assert_input_samples(&fixture.snapshot(cx), 1, 1);
    fixture.assert_frame_samples(cx, 1, 1);

    let recovery_generation = fixture.draw(cx);
    assert_eq!(recovery_generation, accepted_generation + 1);
    assert_ne!(fixture.scene_color(cx), accepted_color);
    let recovery_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert_eq!(recovery_draw.dirty_at, dirty.0);
    assert!(recovery_draw.invalidations >= dirty.1);
    fixture.update(cx, |window, _| {
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        assert!(!window.refresh_pending_for_test());
    });
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        recovery_generation
    );
    let recovery_present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(recovery_draw.draw_end <= recovery_present.present_start);
    let snapshot = fixture.snapshot(cx);
    assert_input_samples(&snapshot, 2, 0);
    assert_eq!(snapshot.events_per_frame_histogram.count_at(1), 2);
    fixture.assert_frame_samples(cx, 2, 2);
}

#[open_gpui::test]
fn render_unwind_preserves_last_scene_pending_input_and_dirty_timestamp_for_recovery(
    cx: &mut TestAppContext,
) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    fixture.input(cx);
    let accepted_generation = fixture.draw(cx);
    only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    let accepted_color = fixture.scene_color(cx);
    fixture.input(cx);
    let dirty = fixture.update(cx, |window, _| pending_dirty(window));
    fixture.controls.panic_render.set(true);
    let platform_draws = fixture.platform.draw_count();
    fixture.update(cx, |window, cx| {
        let panic = catch_unwind(AssertUnwindSafe(|| window.draw(cx).clear(cx))).unwrap_err();
        assert_injected_panic(panic.as_ref(), RENDER_PANIC);
        assert_eq!(window.rendered_frame_revision(), accepted_generation);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(accepted_generation)
        );
        assert_eq!(pending_dirty(window).0, dirty.0);
    });
    assert_eq!(fixture.scene_color(cx), accepted_color);
    assert_eq!(fixture.platform.draw_count(), platform_draws);
    assert_input_samples(&fixture.snapshot(cx), 0, 2);
    fixture.assert_frame_samples(cx, 1, 0);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Deferred)
            .generation,
        accepted_generation
    );
    assert_input_samples(&fixture.snapshot(cx), 0, 2);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());

    let recovery_generation = fixture.draw(cx);
    assert_eq!(recovery_generation, accepted_generation + 1);
    assert_ne!(fixture.scene_color(cx), accepted_color);
    let recovery_draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    assert_eq!(recovery_draw.dirty_at, dirty.0);
    assert!(recovery_draw.invalidations >= dirty.1);
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        recovery_generation
    );
    only_present(&window_events(&mut collector, fixture.handle.window_id()));
    let snapshot = fixture.snapshot(cx);
    assert_input_samples(&snapshot, 1, 0);
    assert_eq!(snapshot.events_per_frame_histogram.count_at(2), 1);
    fixture.assert_frame_samples(cx, 2, 1);
}

#[open_gpui::test]
fn accepted_publication_unwind_discards_previous_scene_timing(cx: &mut TestAppContext) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    fixture.input(cx);
    let previous_generation = fixture.draw(cx);
    only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    fixture.input(cx);
    fixture.controls.panic_publication.set(true);
    fixture.update(cx, |window, cx| {
        let panic = catch_unwind(AssertUnwindSafe(|| window.draw(cx).clear(cx))).unwrap_err();
        assert_injected_panic(panic.as_ref(), PUBLICATION_PANIC);
        assert_eq!(window.rendered_frame_revision(), previous_generation + 1);
        assert_eq!(
            fixture.controls.publication_generation.get(),
            Some(window.rendered_frame_revision())
        );
        assert_eq!(
            fixture.controls.following_publication_generation.get(),
            Some(window.rendered_frame_revision())
        );
    });
    assert_input_samples(&fixture.snapshot(cx), 0, 2);
    fixture.assert_frame_samples(cx, 1, 0);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        previous_generation + 1
    );
    assert_input_samples(&fixture.snapshot(cx), 0, 2);
    fixture.assert_frame_samples(cx, 1, 0);
    assert!(window_events(&mut collector, fixture.handle.window_id()).is_empty());
    fixture.update(cx, |window, _| {
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(previous_generation + 1)
        );
        window.refresh();
    });
    let recovered = fixture.draw(cx);
    assert_eq!(recovered, previous_generation + 2);
    only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    fixture.present(cx, PlatformWindowPresentOutcome::Submitted);
    only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert_input_samples(&fixture.snapshot(cx), 1, 0);
    assert_eq!(fixture.controls.publication_discards.get(), 1);
}

#[open_gpui::test]
fn keyboard_modality_draw_covers_only_input_invalidations_it_has_rendered(cx: &mut TestAppContext) {
    let _trace = profiler::trace_scope();
    for invalidate_after_draw in [false, true] {
        let fixture = WindowFixture::new(cx);
        fixture.controls.keyboard_style.set(true);
        fixture
            .handle
            .update(cx, |root, window, app| window.focus(&root.focus, app))
            .unwrap();
        fixture.input(cx);
        fixture.draw(cx);
        fixture.present(cx, PlatformWindowPresentOutcome::Submitted);
        fixture.update(cx, |window, _| window.reset_input_latency_measurements());
        fixture.controls.key_refresh.set(invalidate_after_draw);
        let before = fixture.update(cx, |window, _| window.rendered_frame_revision());
        let mut collector = FrameTimingCollector::new();
        fixture.update(cx, |window, app| {
            window.dispatch_event(
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke {
                        key: "q".into(),
                        ..Default::default()
                    },
                    is_held: false,
                    prefer_character_input: false,
                }),
                app,
            );
            assert!(window.last_input_was_keyboard());
            assert_eq!(window.rendered_frame_revision(), before + 1);
        });
        only_draw(&window_events(&mut collector, fixture.handle.window_id()));
        assert_input_samples(&fixture.snapshot(cx), 0, 1);
        fixture.present(cx, PlatformWindowPresentOutcome::Submitted);
        only_present(&window_events(&mut collector, fixture.handle.window_id()));
        assert_input_samples(
            &fixture.snapshot(cx),
            u64::from(!invalidate_after_draw),
            u64::from(invalidate_after_draw),
        );
        if invalidate_after_draw {
            fixture.draw(cx);
            fixture.present(cx, PlatformWindowPresentOutcome::Submitted);
            assert_input_samples(&fixture.snapshot(cx), 1, 0);
        }
    }
}

#[open_gpui::test]
fn reset_clears_window_input_diagnostics_without_discarding_accepted_frame_timing(
    cx: &mut TestAppContext,
) {
    let _trace = profiler::trace_scope();
    let fixture = WindowFixture::new(cx);
    let mut collector = FrameTimingCollector::new();
    fixture.input(cx);
    fixture.update(cx, |window, _| window.refresh());
    let generation = fixture.draw(cx);
    let draw = only_draw(&window_events(&mut collector, fixture.handle.window_id()));
    fixture.present(cx, PlatformWindowPresentOutcome::Deferred);
    fixture.update(cx, |window, _| window.reset_input_latency_measurements());
    let snapshot = fixture.snapshot(cx);
    assert_input_samples(&snapshot, 0, 0);
    assert_eq!(snapshot.drawn_frame_count, 0);
    assert_eq!(snapshot.present_attempt_count, 0);
    assert_eq!(snapshot.submitted_present_count, 0);
    assert_eq!(snapshot.deferred_present_count, 0);
    assert_eq!(snapshot.repaint_required_present_count, 0);
    assert_eq!(snapshot.rejected_present_count, 0);
    assert_eq!(snapshot.refresh_request_count, 0);
    assert_eq!(snapshot.coalesced_refresh_request_count, 0);
    assert!(snapshot.refresh_callsites.is_empty());
    fixture.update(cx, |window, _| {
        assert_eq!(
            window
                .frame_duration_snapshot()
                .draw_duration_histogram
                .len(),
            fixture.baseline_draw_samples + 1
        );
    });
    assert_eq!(
        fixture
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        generation
    );
    let present = only_present(&window_events(&mut collector, fixture.handle.window_id()));
    assert!(draw.draw_end <= present.present_start);
    assert_input_samples(&fixture.snapshot(cx), 0, 0);
    fixture.update(cx, |window, _| {
        assert_eq!(
            window
                .frame_duration_snapshot()
                .dirty_to_present_histogram
                .len(),
            fixture.baseline_present_samples + 1
        );
    });
}

#[open_gpui::test]
fn nested_window_draw_keeps_input_and_submission_measurements_isolated(cx: &mut TestAppContext) {
    let _trace = profiler::trace_scope();
    let outer = WindowFixture::new(cx);
    let inner = WindowFixture::new(cx);
    assert_ne!(outer.handle.window_id(), inner.handle.window_id());
    let mut collector = FrameTimingCollector::new();
    cx.set_window_present_outcome(inner.handle.into(), PlatformWindowPresentOutcome::Deferred);
    outer.controls.nested_draw.set(Some(inner.handle.into()));
    outer.input(cx);
    let outer_generation = outer.draw(cx);
    let events = collector.collect_unseen();
    let [FrameEvent::Draw(inner_draw), FrameEvent::Draw(outer_draw)] = events.as_slice() else {
        panic!("expected the inner draw to finish inside the outer draw, got {events:?}");
    };
    assert_eq!(inner_draw.window_id, inner.handle.window_id());
    assert_eq!(outer_draw.window_id, outer.handle.window_id());
    assert!(outer_draw.draw_start <= inner_draw.draw_start);
    assert!(inner_draw.draw_end <= outer_draw.draw_end);
    assert_eq!(inner.controls.inputs.get(), 1);
    assert_input_samples(&inner.snapshot(cx), 0, 1);
    assert_input_samples(&outer.snapshot(cx), 0, 1);
    inner.assert_frame_samples(cx, 1, 0);
    outer.assert_frame_samples(cx, 1, 0);

    assert_eq!(
        outer
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        outer_generation
    );
    only_present(&window_events(&mut collector, outer.handle.window_id()));
    assert_input_samples(&outer.snapshot(cx), 1, 0);
    assert_input_samples(&inner.snapshot(cx), 0, 1);
    assert_eq!(inner.snapshot(cx).deferred_present_count, 1);
    assert_eq!(
        inner
            .present(cx, PlatformWindowPresentOutcome::Submitted)
            .generation,
        inner.baseline_generation + 1
    );
    only_present(&window_events(&mut collector, inner.handle.window_id()));
    assert_input_samples(&inner.snapshot(cx), 1, 0);
    assert_input_samples(&outer.snapshot(cx), 1, 0);
    inner.assert_frame_samples(cx, 1, 1);
    outer.assert_frame_samples(cx, 1, 1);
}
