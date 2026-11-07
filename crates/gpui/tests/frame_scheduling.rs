#![cfg(feature = "test-support")]

use open_gpui::{
    AppContext as _, AtlasKey, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureLeaseEpoch,
    AtlasTextureLeaseError, AtlasTile, Bounds, Context, DevicePixels, IntoElement,
    ParentElement as _, PlatformAtlas, PlatformWindowPresentOutcome, Render, RenderImage, Size,
    Styled as _, TestAppContext, TileId, VisualTestContext, Window, WindowBounds, WindowHandle,
    WindowInitialPresentationStatus, WindowOptions, WindowPresentationFacts, blue, canvas, div,
    point, px, size,
};
use std::{
    borrow::Cow,
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
};

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    paint_image: Cell<bool>,
}

struct SchedulingView {
    observed: Rc<Observations>,
    image: Arc<RenderImage>,
}

impl Render for SchedulingView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let observed = Rc::clone(&self.observed);
        let image = Arc::clone(&self.image);
        div().size_full().bg(blue()).child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    if observed.paint_image.get() {
                        window
                            .paint_image(bounds, Default::default(), Arc::clone(&image), 0, false)
                            .unwrap();
                    }
                },
            )
            .size_full(),
        )
    }
}

fn presentation_facts(visual: &mut VisualTestContext) -> WindowPresentationFacts {
    visual.update(|window, _| window.presentation_facts())
}

fn assert_submitted(facts: WindowPresentationFacts, generation: u64) {
    assert!(facts.native_created);
    assert!(facts.native_visible);
    assert_eq!(
        facts.initial_presentation,
        WindowInitialPresentationStatus::Completed
    );
    assert_eq!(facts.frame_accepted_generation, Some(generation));
    assert_eq!(facts.present_submitted_generation, Some(generation));
    assert_eq!(facts.non_empty_presented_generation, Some(generation));
    let attempt = facts.latest_present_attempt.unwrap();
    assert_eq!(attempt.generation, generation);
    assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
    assert!(attempt.contained_valid_primitives);
}

fn assert_compositor_callback_parks(
    visual: &mut VisualTestContext,
    observed: &Observations,
    submitted: WindowPresentationFacts,
) {
    let renders = observed.renders.get();
    assert!(
        visual.frame_scheduled(),
        "Submitted must await its compositor callback"
    );
    assert!(visual.simulate_scheduled_frame());
    assert!(
        !visual.frame_scheduled(),
        "a completed idle callback must park the source"
    );
    assert!(
        !visual.simulate_scheduled_frame(),
        "a parked source must not invent an idle tick"
    );
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(presentation_facts(visual), submitted);
}

fn open_parked_window(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<SchedulingView>,
    VisualTestContext,
    Rc<Observations>,
) {
    let observed = Rc::new(Observations::default());
    let image = Arc::new(RenderImage::new([image::Frame::new(
        image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 255])),
    )]));
    cx.defer_next_window_frame_requests();
    let handle = cx.update(|app| {
        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(96.), px(96.)),
                ))),
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            },
            |_, app| {
                app.new(|_| SchedulingView {
                    observed: Rc::clone(&observed),
                    image,
                })
            },
        )
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, app| {
        assert!(window.is_window_active());
        window.draw(app).clear(app);
    });
    assert!(cx.window_frame_scheduled(handle.into()));
    assert!(cx.simulate_window_scheduled_frame(handle.into()));
    let submitted = presentation_facts(&mut visual);
    assert_submitted(submitted, submitted.frame_accepted_generation.unwrap());
    assert_compositor_callback_parks(&mut visual, &observed, submitted);
    (handle, visual, observed)
}

#[open_gpui::test]
fn public_notify_window_refresh_and_async_refresh_wake_parked_windows(cx: &mut TestAppContext) {
    let (handle, mut visual, observed) = open_parked_window(cx);
    let mut baseline = presentation_facts(&mut visual);

    for notify in [true, false] {
        let renders = observed.renders.get();
        let root = handle.entity(cx).unwrap();
        visual.update(|window, app| {
            if notify {
                root.update(app, |_, cx| cx.notify());
            } else {
                window.refresh();
            }
        });
        let pending = presentation_facts(&mut visual);
        assert!(observed.renders.get() > renders);
        assert!(pending.frame_accepted_generation > baseline.frame_accepted_generation);
        assert_eq!(
            pending.present_submitted_generation,
            baseline.present_submitted_generation
        );
        assert_eq!(
            pending.non_empty_presented_generation,
            baseline.non_empty_presented_generation
        );
        assert!(visual.frame_scheduled());
        let renders = observed.renders.get();
        assert!(visual.simulate_scheduled_frame());
        baseline = presentation_facts(&mut visual);
        assert_submitted(baseline, pending.frame_accepted_generation.unwrap());
        assert_eq!(observed.renders.get(), renders);
        assert_compositor_callback_parks(&mut visual, &observed, baseline);
    }

    let (other_handle, mut other_visual, other_observed) = open_parked_window(cx);
    let other_baseline = presentation_facts(&mut other_visual);
    let renders = observed.renders.get();
    let other_renders = other_observed.renders.get();
    cx.to_async().refresh();
    assert!(
        observed.renders.get() > renders,
        "AsyncApp::refresh must flush before returning"
    );
    assert!(other_observed.renders.get() > other_renders);
    assert!(cx.window_frame_scheduled(handle.into()));
    assert!(cx.window_frame_scheduled(other_handle.into()));

    for (visual, observed, baseline) in [
        (&mut visual, observed.as_ref(), baseline),
        (&mut other_visual, other_observed.as_ref(), other_baseline),
    ] {
        let pending = presentation_facts(visual);
        assert!(pending.frame_accepted_generation > baseline.frame_accepted_generation);
        assert_eq!(
            pending.present_submitted_generation,
            baseline.present_submitted_generation
        );
        let renders = observed.renders.get();
        assert!(visual.simulate_scheduled_frame());
        let submitted = presentation_facts(visual);
        assert_submitted(submitted, pending.frame_accepted_generation.unwrap());
        assert_eq!(observed.renders.get(), renders);
        assert_compositor_callback_parks(visual, observed, submitted);
    }
}

#[open_gpui::test]
fn public_noninvalidating_callbacks_keep_midframe_demand_then_park(cx: &mut TestAppContext) {
    let (_, mut visual, observed) = open_parked_window(cx);
    let baseline = presentation_facts(&mut visual);
    let renders = observed.renders.get();
    let callbacks = Rc::new(Cell::new(0));
    visual.update(|window, _| {
        let callbacks = Rc::clone(&callbacks);
        window.on_next_frame(move |window, _| {
            callbacks.set(callbacks.get() + 1);
            window.on_next_frame(move |_, _| {
                callbacks.set(callbacks.get() + 1);
            });
        });
    });
    assert_eq!(callbacks.get(), 0);
    assert!(visual.frame_scheduled());
    assert!(visual.simulate_scheduled_frame());
    assert_eq!(callbacks.get(), 1);
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(presentation_facts(&mut visual), baseline);
    assert!(
        visual.frame_scheduled(),
        "the callback queued mid-frame must retain demand"
    );
    assert!(visual.simulate_scheduled_frame());
    assert_eq!(callbacks.get(), 2);
    assert!(!visual.frame_scheduled());
    assert!(!visual.simulate_scheduled_frame());
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(presentation_facts(&mut visual), baseline);
}

#[open_gpui::test]
fn public_deferred_and_rejected_scenes_keep_scheduled_demand_until_exact_submission(
    cx: &mut TestAppContext,
) {
    for outcome in [
        PlatformWindowPresentOutcome::Deferred,
        PlatformWindowPresentOutcome::Rejected,
    ] {
        let (handle, mut visual, observed) = open_parked_window(cx);
        let baseline = presentation_facts(&mut visual);
        cx.set_window_present_outcome(handle.into(), outcome);
        visual.update(|window, _| window.refresh());
        let pending = presentation_facts(&mut visual);
        let generation = pending.frame_accepted_generation.unwrap();
        assert!(pending.frame_accepted_generation > baseline.frame_accepted_generation);
        let renders = observed.renders.get();

        for _ in 0..2 {
            assert!(visual.frame_scheduled());
            assert!(visual.simulate_scheduled_frame());
            let retry = presentation_facts(&mut visual);
            assert_eq!(retry.frame_accepted_generation, Some(generation));
            assert_eq!(
                retry.present_submitted_generation,
                baseline.present_submitted_generation
            );
            assert_eq!(
                retry.non_empty_presented_generation,
                baseline.non_empty_presented_generation
            );
            assert_eq!(retry.initial_presentation, baseline.initial_presentation);
            assert_eq!(retry.native_created, baseline.native_created);
            assert_eq!(retry.native_visible, baseline.native_visible);
            let attempt = retry.latest_present_attempt.unwrap();
            assert_eq!(attempt.generation, generation);
            assert_eq!(attempt.outcome, outcome);
            assert!(attempt.contained_valid_primitives);
            assert_eq!(observed.renders.get(), renders);
            assert!(
                visual.frame_scheduled(),
                "a failed renderer attempt must retain demand"
            );
        }

        cx.set_window_present_outcome(handle.into(), PlatformWindowPresentOutcome::Submitted);
        assert!(visual.simulate_scheduled_frame());
        let submitted = presentation_facts(&mut visual);
        assert_submitted(submitted, generation);
        assert_eq!(observed.renders.get(), renders);
        assert_compositor_callback_parks(&mut visual, &observed, submitted);
    }
}

#[derive(Default)]
struct RefusingAtlas {
    refuse: AtomicBool,
    lease_attempts: AtomicUsize,
}

impl PlatformAtlas for RefusingAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> open_gpui::Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> open_gpui::Result<Option<AtlasTile>> {
        Ok(build()?.map(|(size, _)| AtlasTile {
            texture_id: AtlasTextureId {
                index: 1,
                kind: key.texture_kind(),
            },
            tile_id: TileId(1),
            padding: 0,
            bounds: Bounds::new(Default::default(), size),
            texture_generation: 1,
            texture_generation_padding: 0,
        }))
    }

    fn remove(&self, _: &AtlasKey) {}

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        AtlasTextureLeaseEpoch::INITIAL
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        self.lease_attempts.fetch_add(1, SeqCst);
        if self.refuse.load(SeqCst) {
            return Err(AtlasTextureLeaseError::TextureUnavailable {
                texture: *textures.first().unwrap(),
                epoch: AtlasTextureLeaseEpoch::INITIAL,
            });
        }
        Ok(AtlasTextureLeaseEpoch::INITIAL)
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        _: AtlasTextureLeaseEpoch,
        _: &[AtlasTextureInstanceId],
    ) {
    }
}

#[open_gpui::test]
fn public_refused_candidate_keeps_scheduled_demand_until_a_new_scene_is_submitted(
    cx: &mut TestAppContext,
) {
    let (_, mut visual, observed) = open_parked_window(cx);
    let baseline = presentation_facts(&mut visual);
    let atlas = Arc::new(RefusingAtlas::default());
    atlas.refuse.store(true, SeqCst);
    observed.paint_image.set(true);
    let renders = observed.renders.get();
    visual.update(|window, _| {
        window.set_sprite_atlas_for_test(atlas.clone());
        window.refresh();
    });
    assert!(observed.renders.get() > renders);
    assert!(atlas.lease_attempts.load(SeqCst) > 0);
    assert_eq!(presentation_facts(&mut visual), baseline);

    for _ in 0..2 {
        let attempts = atlas.lease_attempts.load(SeqCst);
        assert!(visual.frame_scheduled());
        assert!(visual.simulate_scheduled_frame());
        assert!(atlas.lease_attempts.load(SeqCst) > attempts);
        assert_eq!(presentation_facts(&mut visual), baseline);
        assert!(
            visual.frame_scheduled(),
            "refusal must not discard the dirty candidate's demand"
        );
    }

    atlas.refuse.store(false, SeqCst);
    assert!(visual.simulate_scheduled_frame());
    let submitted = presentation_facts(&mut visual);
    assert_submitted(submitted, baseline.frame_accepted_generation.unwrap() + 1);
    assert_compositor_callback_parks(&mut visual, &observed, submitted);
}

#[open_gpui::test]
fn public_close_releases_pending_callbacks_and_root_before_the_held_native_terminal(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(open_gpui::QuitMode::Explicit));
    let (handle, mut visual, observed) = open_parked_window(cx);
    let root = handle.entity(cx).unwrap().downgrade();
    let callback_capture = Rc::new(Cell::new(false));
    let callback_lifetime = Rc::downgrade(&callback_capture);
    visual.update(|window, _| {
        window.on_next_frame(move |_, _| callback_capture.set(true));
    });
    assert!(visual.frame_scheduled());
    assert!(callback_lifetime.upgrade().is_some());
    let renders = observed.renders.get();
    let terminal = cx.hold_window_native_terminal(handle.into());
    assert!(cx.simulate_window_close(handle.into()));
    assert!(!cx.windows().contains(&handle.into()));
    assert!(root.upgrade().is_none());
    assert!(callback_lifetime.upgrade().is_none());
    drop(visual);

    cx.to_async().refresh();
    cx.run_until_parked();
    assert_eq!(observed.renders.get(), renders);
    assert!(terminal.release());
    cx.run_until_parked();
    assert!(!cx.windows().contains(&handle.into()));
    assert!(root.upgrade().is_none());
    assert!(callback_lifetime.upgrade().is_none());
    assert_eq!(observed.renders.get(), renders);
}
