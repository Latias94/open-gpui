use super::cached_paint_atlas_tests::RejectOnceAtlas;
use super::*;
use crate::{TestAppContext, TimedTransformation, Transformation, radians, red, svg};

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="8" height="16"/></svg>"#;

// CPU-only atlas fault injection needs a nonempty tile after SVG supersampling and raster snap.
struct SvgAtlas(Arc<RejectOnceAtlas>);

impl PlatformAtlas for SvgAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: crate::AtlasKey,
        _: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let mut tile = RejectOnceAtlas::tile(key.texture_kind());
        tile.bounds.size = size(DevicePixels(96), DevicePixels(96));
        Ok(Some(tile))
    }

    fn remove(&self, _: &crate::AtlasKey) {}

    fn atlas_texture_lease_epoch(&self) -> crate::AtlasTextureLeaseEpoch {
        self.0.atlas_texture_lease_epoch()
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> std::result::Result<crate::AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        // SAFETY: The original atlas owns acquisition and matching retirement for these instances.
        unsafe { self.0.acquire_atlas_texture_leases(textures) }
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        epoch: crate::AtlasTextureLeaseEpoch,
        textures: &[AtlasTextureInstanceId],
    ) {
        // SAFETY: Acquisition above forwards the same epoch and texture identities unchanged.
        unsafe { self.0.release_atlas_texture_leases(epoch, textures) }
    }
}

struct Controls {
    renders: Cell<usize>,
    animated: Cell<bool>,
}

struct Probe(Rc<Controls>);

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        let icon = svg().data(SVG).size(px(32.)).text_color(red());
        let initial = Transformation::alpha(0.4);
        if self.0.animated.get() {
            icon.with_timed_transformation(
                TimedTransformation::new(Duration::from_secs(1))
                    .from(initial)
                    .to(initial.with_rotation(radians(std::f32::consts::TAU)))
                    .with_max_fps(10.),
            )
        } else {
            icon.with_transformation(initial)
        }
    }
}

fn open(
    cx: &mut TestAppContext,
    animated: bool,
) -> (WindowHandle<Probe>, Rc<Controls>, Arc<RejectOnceAtlas>) {
    cx.defer_next_window_frame_requests();
    let controls = Rc::new(Controls {
        renders: Cell::new(0),
        animated: Cell::new(animated),
    });
    let atlas = Arc::new(RejectOnceAtlas::new());
    let window = cx.add_window({
        let controls = controls.clone();
        let atlas = atlas.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(Arc::new(SvgAtlas(atlas)));
            Probe(controls)
        }
    });
    cx.run_until_parked();
    (window, controls, atlas)
}

#[crate::test]
fn timed_svg_native_frames_present_without_render_and_forced_submission_bypasses_cap(
    cx: &mut TestAppContext,
) {
    let (handle, controls, _) = open(cx, true);
    let native = cx.test_window(handle.into());
    cx.update_window(handle.into(), |_, window, app| {
        window.active.set(true);
        assert_eq!(
            window.scene_animation_interval,
            Some(Duration::from_millis(100))
        );
        window.present(app);
        let generation = window.rendered_frame_revision();
        let renders = controls.renders.get();
        let draws = native.draw_count();
        window.frame_pacer.last_frame = Some(Instant::now());
        window.native_frame_requested(RequestFrameOptions::default(), app);
        assert_eq!(native.draw_count(), draws);

        window.native_frame_requested(
            RequestFrameOptions {
                require_presentation: true,
                ..Default::default()
            },
            app,
        );
        assert_eq!(native.draw_count(), draws + 1);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(controls.renders.get(), renders);
        assert!(window.has_scene_animations());
    })
    .unwrap();
}

#[crate::test]
fn rejected_svg_candidates_preserve_the_accepted_animation_demand(cx: &mut TestAppContext) {
    let (handle, controls, atlas) = open(cx, false);
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    cx.update_window(handle.into(), |_, window, app| {
        let original = window.rendered_frame_revision();
        assert!(!window.has_scene_animations());
        controls.animated.set(true);
        atlas.fail_next_lease();
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), original);
        assert!(!window.has_scene_animations());
        window.draw(app).clear(app);
        assert!(window.has_scene_animations());
        let animated = window.rendered_frame_revision();

        controls.animated.set(false);
        atlas.fail_next_lease();
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), animated);
        assert!(window.has_scene_animations());
        window.draw(app).clear(app);
        assert!(!window.has_scene_animations());
    })
    .unwrap();
}

#[crate::test]
fn reduced_motion_retires_svg_animation_demand_and_preserves_initial_alpha(
    cx: &mut TestAppContext,
) {
    let (handle, _, _) = open(cx, true);
    cx.update_window(handle.into(), |_, window, app| {
        assert!(window.has_scene_animations());
        app.set_reduce_motion(true);
        window.refresh();
        window.draw(app).clear(app);
        assert!(!window.has_scene_animations());
        let sprite = &window.rendered_frame.scene.monochrome_sprites[0];
        assert_eq!(sprite.animation, SpriteAnimationId::NONE);
        assert!((sprite.color.a - 0.4).abs() < 1e-6);
        assert_eq!(sprite.transformation, TransformationMatrix::unit());
    })
    .unwrap();
}

#[crate::test]
fn deferred_svg_animation_keeps_delivered_pose_when_invalid_sibling_is_filtered(
    cx: &mut TestAppContext,
) {
    let (handle, controls, _) = open(cx, false);
    let native = cx.test_window(handle.into());
    native.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.update_window(handle.into(), |_, window, app| {
        controls.animated.set(true);
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.deferred_candidate.is_some());
        assert!(!window.has_scene_animations());
        let invalid = SubtreeGeometryValidity::new(None);
        let sibling = window.next_frame.scene.monochrome_sprites[0];
        let clip = ClipStackSnapshot::root(Bounds::new(Point::default(), window.viewport_size()));
        let scale_factor = window.scale_factor();
        window
            .next_frame
            .scene
            .insert_primitive_scoped(sibling, &clip, scale_factor, Some(invalid.clone()))
            .unwrap();
        invalid.invalidate(SubtreeClipError::UnrepresentableResult);
        window.next_frame.scene.finish();
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        let pose = window.next_frame.scene.monochrome_sprites[0].transformation;
        let alpha = window.next_frame.scene.monochrome_sprites[0].color.a;
        assert!((alpha - 0.4).abs() < 1e-6);
        assert!((pose.rotation_scale[0][0]).abs() < 1e-5);
        assert!((pose.rotation_scale[1][0] - 1.).abs() < 1e-5);
        let submission = native.pending_frame_submission().unwrap();
        assert!(submission.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(window.complete_deferred_frame_submission(
            &submission,
            PlatformWindowPresentOutcome::Submitted,
            app,
        ));
        assert!(window.has_scene_animations());
        assert_eq!(
            window.rendered_frame.scene.monochrome_sprites[0].transformation,
            pose
        );
        assert_eq!(
            window.rendered_frame.scene.monochrome_sprites[0].color.a,
            alpha
        );
        let mut capture = window.rendered_frame.scene.clone();
        capture.finish();
        assert_eq!(capture.monochrome_sprites[0].transformation, pose);
        assert_eq!(capture.monochrome_sprites[0].color.a, alpha);
    })
    .unwrap();
}

#[crate::test]
fn retained_deferred_svg_frames_publish_only_on_submission_without_rendering(
    cx: &mut TestAppContext,
) {
    let (handle, controls, _) = open(cx, true);
    let native = cx.test_window(handle.into());
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    let generation = cx
        .update_window(handle.into(), |_, window, app| {
            window.active.set(true);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            assert!(!window.invalidator.is_dirty());
            window.rendered_frame_revision()
        })
        .unwrap();
    let renders = controls.renders.get();
    native.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
    let mut previous_submission: Option<crate::PlatformFrameSubmission> = None;

    for quarter in 1..=3 {
        cx.executor()
            .advance_clock(Duration::from_millis(if quarter == 1 { 250 } else { 200 }));
        let (submission, before, draws) = cx
            .update_window(handle.into(), |_, window, app| {
                let before = window.rendered_frame.scene.monochrome_sprites[0];
                window.native_frame_requested(
                    RequestFrameOptions {
                        require_presentation: true,
                        ..Default::default()
                    },
                    app,
                );
                assert!(window.deferred_scene.is_some());
                assert!(window.deferred_candidate.is_none());
                assert!(window.deferred_frame_delivery_is_parked());
                assert!(!window.invalidator.is_dirty());
                assert_eq!(controls.renders.get(), renders);
                assert_eq!(window.rendered_frame_revision(), generation);
                let submission = native.pending_frame_submission().unwrap();
                assert_eq!(submission.frame_generation(), generation);
                if let Some(previous) = previous_submission.as_ref() {
                    assert!(submission.attempt_id() > previous.attempt_id());
                    assert!(!window.complete_deferred_frame_submission(
                        previous,
                        PlatformWindowPresentOutcome::Submitted,
                        app,
                    ));
                }
                assert!(!window.complete_deferred_frame_submission(
                    &submission,
                    PlatformWindowPresentOutcome::Submitted,
                    app,
                ));
                let accepted = window.rendered_frame.scene.monochrome_sprites[0];
                assert_eq!(accepted.transformation, before.transformation);
                assert_eq!(accepted.color, before.color);
                (submission, before, native.draw_count())
            })
            .unwrap();

        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, app| {
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
            assert_eq!(native.draw_count(), draws);
            assert!(
                native
                    .pending_frame_submission()
                    .unwrap()
                    .same_authority(&submission)
            );
            let accepted = window.rendered_frame.scene.monochrome_sprites[0];
            assert_eq!(accepted.transformation, before.transformation);
            assert_eq!(accepted.color, before.color);
            assert!(submission.complete(PlatformWindowPresentOutcome::Submitted));
            assert!(window.complete_deferred_frame_submission(
                &submission,
                PlatformWindowPresentOutcome::Submitted,
                app,
            ));
            assert!(window.deferred_scene.is_none());
            assert!(window.deferred_candidate.is_none());
            assert!(!window.invalidator.is_dirty());
            assert_eq!(controls.renders.get(), renders);
            assert_eq!(window.rendered_frame_revision(), generation);
            let facts = window.presentation_facts();
            assert_eq!(facts.frame_accepted_generation, Some(generation));
            assert_eq!(facts.present_submitted_generation, Some(generation));
            let sprite = window.rendered_frame.scene.monochrome_sprites[0];
            let (sin, cos) = (quarter as f32 * std::f32::consts::FRAC_PI_2).sin_cos();
            for (actual, expected) in sprite
                .transformation
                .rotation_scale
                .as_flattened()
                .iter()
                .zip([cos, -sin, sin, cos])
            {
                assert!((actual - expected).abs() < 1e-5);
            }
            assert!((sprite.color.a - 0.4).abs() < 1e-6);
            assert!(!submission.complete(PlatformWindowPresentOutcome::Submitted));
            assert!(!window.complete_deferred_frame_submission(
                &submission,
                PlatformWindowPresentOutcome::Submitted,
                app,
            ));
        })
        .unwrap();
        previous_submission = Some(submission);
    }
}

#[crate::test]
fn a_new_svg_candidate_cancels_the_retained_deferred_submission(cx: &mut TestAppContext) {
    let (handle, controls, _) = open(cx, true);
    let native = cx.test_window(handle.into());
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
        let generation = window.rendered_frame_revision();
        native.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        let original = native.pending_frame_submission().unwrap();
        assert!(window.deferred_scene.is_some());

        controls.animated.set(false);
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.deferred_scene.is_none());
        assert!(window.deferred_candidate.is_some());
        assert_eq!(
            original.status(),
            crate::PlatformFrameSubmissionStatus::Cancelled
        );
        assert!(!original.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        let replacement = native.pending_frame_submission().unwrap();
        assert_eq!(replacement.frame_generation(), generation + 1);
        assert!(replacement.attempt_id() > original.attempt_id());
        assert!(!window.complete_deferred_frame_submission(
            &original,
            PlatformWindowPresentOutcome::Submitted,
            app,
        ));
        assert!(window.deferred_candidate.is_some());
        assert!(replacement.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(window.complete_deferred_frame_submission(
            &replacement,
            PlatformWindowPresentOutcome::Submitted,
            app,
        ));
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert!(!window.has_scene_animations());
        let sprite = window.rendered_frame.scene.monochrome_sprites[0];
        assert_eq!(sprite.transformation, TransformationMatrix::unit());
        assert!((sprite.color.a - 0.4).abs() < 1e-6);
    })
    .unwrap();
}

#[crate::test]
fn stale_native_geometry_cannot_publish_a_retained_deferred_svg_sample(cx: &mut TestAppContext) {
    let (handle, controls, _) = open(cx, true);
    let native = cx.test_window(handle.into());
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
        let generation = window.rendered_frame_revision();
        let renders = controls.renders.get();
        let before = window.rendered_frame.scene.monochrome_sprites[0];
        native.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        let submission = native.pending_frame_submission().unwrap();
        assert!(window.deferred_scene.is_some());

        native.set_physical_client_geometry(None, window.scale_factor() * 1.25);
        assert!(submission.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(!window.complete_deferred_frame_submission(
            &submission,
            PlatformWindowPresentOutcome::Submitted,
            app,
        ));
        assert!(window.deferred_scene.is_none());
        assert!(window.deferred_candidate.is_none());
        assert!(window.invalidator.is_dirty());
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(controls.renders.get(), renders);
        let accepted = window.rendered_frame.scene.monochrome_sprites[0];
        assert_eq!(accepted.transformation, before.transformation);
        assert_eq!(accepted.color, before.color);
        assert!(!submission.complete(PlatformWindowPresentOutcome::Submitted));
    })
    .unwrap();
}

#[crate::test]
fn retained_deferred_svg_requires_an_independent_atlas_lease(cx: &mut TestAppContext) {
    let (handle, controls, atlas) = open(cx, true);
    let native = cx.test_window(handle.into());
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
        let generation = window.rendered_frame_revision();
        let renders = controls.renders.get();
        let before = window.rendered_frame.scene.monochrome_sprites[0];
        let draws = native.draw_count();
        native.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
        atlas.fail_next_lease();

        assert_eq!(
            window.present(app),
            PlatformWindowPresentOutcome::RepaintRequired
        );
        assert!(window.deferred_scene.is_none());
        assert!(window.deferred_candidate.is_none());
        assert!(native.pending_frame_submission().is_none());
        assert_eq!(native.draw_count(), draws);
        assert!(window.invalidator.is_dirty());
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(controls.renders.get(), renders);
        let accepted = window.rendered_frame.scene.monochrome_sprites[0];
        assert_eq!(accepted.transformation, before.transformation);
        assert_eq!(accepted.color, before.color);
        let leases = &window.rendered_frame.atlas_texture_leases_by_instance;
        assert!(leases.contains_key(&accepted.tile.texture_instance()));
        assert!(leases.values().all(|lease| lease.validate().is_ok()));
    })
    .unwrap();
}

#[crate::test]
fn presentation_shutdown_claim_retires_the_retained_svg_snapshot_and_token(
    cx: &mut TestAppContext,
) {
    let (handle, controls, _) = open(cx, true);
    let native = cx.test_window(handle.into());
    let _batch = super::animation_tests::AnimationFrameBatch::new(cx);
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.update_window(handle.into(), |_, window, app| {
        let generation = window.rendered_frame_revision();
        let renders = controls.renders.get();
        let before = window.rendered_frame.scene.monochrome_sprites[0];
        let atlas_owners = Arc::strong_count(&window.sprite_atlas);
        native.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        let submission = native.pending_frame_submission().unwrap();
        assert!(window.deferred_scene.is_some());
        assert_eq!(Arc::strong_count(&window.sprite_atlas), atlas_owners + 1);
        let draws = native.draw_count();

        let shutdown = window.claim_presentation_shutdown();
        assert!(window.presentation_shutdown.is_some());
        assert!(window.deferred_scene.is_none());
        assert!(window.deferred_candidate.is_none());
        assert_eq!(Arc::strong_count(&window.sprite_atlas), atlas_owners);
        assert_eq!(
            submission.status(),
            crate::PlatformFrameSubmissionStatus::Cancelled
        );
        assert!(!submission.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(!window.complete_deferred_frame_submission(
            &submission,
            PlatformWindowPresentOutcome::Submitted,
            app,
        ));
        assert!(native.pending_frame_submission().is_none());
        assert_eq!(native.draw_count(), draws);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(controls.renders.get(), renders);
        let accepted = window.rendered_frame.scene.monochrome_sprites[0];
        assert_eq!(accepted.transformation, before.transformation);
        assert_eq!(accepted.color, before.color);
        let leases = &window.rendered_frame.atlas_texture_leases_by_instance;
        assert!(leases.contains_key(&accepted.tile.texture_instance()));
        assert!(leases.values().all(|lease| lease.validate().is_ok()));
        assert!(shutdown.same_authority(&window.claim_presentation_shutdown()));
        assert_eq!(Arc::strong_count(&window.sprite_atlas), atlas_owners);
        assert!(!submission.complete(PlatformWindowPresentOutcome::Submitted));
    })
    .unwrap();
}
