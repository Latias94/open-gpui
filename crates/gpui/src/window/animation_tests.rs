use super::cached_paint_atlas_tests::RejectOnceAtlas;
use super::*;
use crate::{Animation, AnimationExt, TestAppContext, canvas, div, fill, rgb};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Default)]
struct AnimationControls {
    renders: Cell<usize>,
    phase: Cell<f32>,
    paint_sprite: Cell<bool>,
    animate: Cell<bool>,
    max_fps: Cell<f32>,
}

struct AnimationProbe(Rc<AnimationControls>);

impl Render for AnimationProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        let animation_controls = self.0.clone();
        let paint_controls = self.0.clone();
        let element = div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let color = Hsla {
                        h: 0.1,
                        s: 0.8,
                        l: 0.3 + paint_controls.phase.get() * 0.3,
                        a: 1.0,
                    };
                    window.paint_quad(fill(bounds, color));
                    if paint_controls.paint_sprite.get() {
                        let _ = window.insert_scene_primitive(MonochromeSprite {
                            order: 0,
                            animation: Default::default(),
                            transformation: Default::default(),
                            bounds: window.device_local_bounds(bounds),
                            clip: Default::default(),
                            color: rgb(0xffffff).into(),
                            tile: RejectOnceAtlas::tile(crate::AtlasTextureKind::Monochrome),
                            transform: PrimitiveTransform::IDENTITY,
                        });
                    }
                },
            )
            .size_full(),
        );
        if self.0.animate.get() {
            element
                .with_animation(
                    "limited-candidate-animation",
                    Animation::new(Duration::from_secs(20))
                        .repeat_synced()
                        .with_max_fps(self.0.max_fps.get()),
                    move |element, phase| {
                        animation_controls.phase.set(phase);
                        element
                    },
                )
                .into_any_element()
        } else {
            element.into_any_element()
        }
    }
}

pub(super) struct AnimationFrameBatch(Rc<AppCell>);

impl AnimationFrameBatch {
    pub(super) fn new(cx: &TestAppContext) -> Self {
        cx.update(|app| app.start_update());
        Self(cx.app.clone())
    }
}

impl Drop for AnimationFrameBatch {
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

#[crate::test]
fn capped_animation_refusal_preserves_scene_retry_and_existing_timer(cx: &mut TestAppContext) {
    let controls = Rc::new(AnimationControls::default());
    controls.animate.set(true);
    controls.max_fps.set(1.0);
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window({
        let controls = controls.clone();
        let atlas = atlas.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            AnimationProbe(controls)
        }
    });
    let platform = cx.test_window(handle.into());
    platform.defer_frame_requests_for_test();
    cx.run_until_parked();
    let weak_root = handle
        .entity(cx)
        .expect("the accepted animation root must remain live")
        .downgrade();
    cx.executor().advance_clock(Duration::from_millis(100));
    let batch = AnimationFrameBatch::new(cx);
    controls.paint_sprite.set(true);
    atlas.fail_next_lease();
    cx.update_window(handle.into(), |_, window, app| {
        let generation = window.rendered_frame_revision();
        let color = window.rendered_frame.scene.quads[0].background;
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert!(window.rendered_frame.scene.quads[0].background == color);
        assert!(window.refresh_pending_for_test());
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            Some(generation)
        );
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert!(window.rendered_frame.scene.quads[0].background != color);
        assert!(!window.refresh_pending_for_test());
        assert_eq!(window.next_frame_callbacks.borrow().len(), 0);
    })
    .expect("refusal and retry must preserve the same animation owner");
    drop(batch);

    let renders_after_retry = controls.renders.get();
    cx.executor().advance_clock(Duration::from_millis(950));
    cx.run_until_parked();
    assert_eq!(controls.renders.get(), renders_after_retry + 1);
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(
        controls.renders.get(),
        renders_after_retry + 1,
        "overlapping refused/retried layouts must not add another delayed frame"
    );
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))
        .expect("the root must close while its next animation timer is pending");
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    let closed_renders = controls.renders.get();
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(controls.renders.get(), closed_renders);
}

#[crate::test]
fn refused_first_animation_mount_cannot_notify_the_accepted_retry(cx: &mut TestAppContext) {
    assert_refused_first_animation_timer_is_retired(cx, true);
}

#[crate::test]
fn refused_first_animation_mount_cannot_notify_a_static_retry(cx: &mut TestAppContext) {
    assert_refused_first_animation_timer_is_retired(cx, false);
}

fn assert_refused_first_animation_timer_is_retired(
    cx: &mut TestAppContext,
    retain_animation: bool,
) {
    let controls = Rc::new(AnimationControls::default());
    controls.max_fps.set(10.0);
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window({
        let controls = controls.clone();
        let atlas = atlas.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            AnimationProbe(controls)
        }
    });
    let platform = cx.test_window(handle.into());
    platform.defer_frame_requests_for_test();
    cx.run_until_parked();
    let batch = AnimationFrameBatch::new(cx);
    controls.animate.set(true);
    controls.paint_sprite.set(true);
    atlas.fail_next_lease();
    let generation = cx
        .update_window(handle.into(), |_, window, app| {
            let generation = window.rendered_frame_revision();
            window.refresh();
            window.draw(app).clear(app);
            assert_eq!(window.rendered_frame_revision(), generation);
            assert!(window.refresh_pending_for_test());
            generation
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    controls.animate.set(retain_animation);
    cx.update_window(handle.into(), |_, window, app| {
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
    drop(batch);
    cx.run_until_parked();
    let renders = controls.renders.get();
    cx.executor().advance_clock(Duration::from_millis(51));
    cx.run_until_parked();
    assert_eq!(
        controls.renders.get(),
        renders,
        "the rejected mount's timer must not notify the accepted view"
    );
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    assert_eq!(
        controls.renders.get(),
        renders + usize::from(retain_animation)
    );
}
