use super::animation_tests::AnimationFrameBatch;
use super::cached_paint_atlas_tests::RejectOnceAtlas;
use super::*;
use crate::{
    AnimationExt, AtlasTextureKind, SpringAnimation, SpringConfig, SpringPlayback, SpringState,
    TestAppContext, canvas, div, fill,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Controls {
    enabled: Cell<bool>,
    target: Cell<f32>,
    playback: Cell<SpringPlayback>,
    sprite: Cell<bool>,
    panic_paint: Cell<bool>,
    samples: RefCell<Vec<(Instant, f32)>>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            enabled: Cell::new(true),
            target: Cell::new(1.0),
            playback: Cell::new(SpringPlayback::Running),
            sprite: Cell::new(false),
            panic_paint: Cell::new(false),
            samples: RefCell::new(Vec::new()),
        }
    }
}

fn config() -> SpringConfig {
    SpringConfig::new(100.0, 10.0, 1.0)
}

struct Probe(Rc<Controls>);

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paint = self.0.clone();
        let element = div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    assert!(
                        !paint.panic_paint.replace(false),
                        "injected spring paint unwind"
                    );
                    window.paint_quad(fill(bounds, crate::blue()));
                    if paint.sprite.get() {
                        let _ = window.insert_scene_primitive(MonochromeSprite {
                            order: 0,
                            animation: Default::default(),
                            transformation: Default::default(),
                            bounds: window.device_local_bounds(bounds),
                            clip: Default::default(),
                            color: crate::red(),
                            tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                            transform: PrimitiveTransform::IDENTITY,
                        });
                    }
                },
            )
            .size_full(),
        );
        if self.0.enabled.get() {
            let controls = self.0.clone();
            let now = cx.background_executor().now();
            element
                .with_spring(
                    "candidate-spring",
                    SpringAnimation::new(config())
                        .to(self.0.target.get())
                        .from(0.0)
                        .playback(self.0.playback.get()),
                    move |element, value| {
                        controls.samples.borrow_mut().push((now, value));
                        element
                    },
                )
                .into_any_element()
        } else {
            element.into_any_element()
        }
    }
}

fn open(
    cx: &mut TestAppContext,
    controls: &Rc<Controls>,
) -> (WindowHandle<Probe>, Arc<RejectOnceAtlas>) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window({
        let controls = controls.clone();
        let atlas = atlas.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            Probe(controls)
        }
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.run_until_parked();
    (handle, atlas)
}

fn assert_retry_ignores_refused_inputs(
    cx: &mut TestAppContext,
    rejected_playback: SpringPlayback,
    unwind: bool,
) {
    let controls = Rc::new(Controls::default());
    let (handle, atlas) = open(cx, &controls);
    let initial = controls.samples.borrow()[0].0;
    let batch = AnimationFrameBatch::new(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    controls.target.set(2.0);
    controls.playback.set(rejected_playback);
    controls.sprite.set(!unwind);
    controls.panic_paint.set(unwind);
    if !unwind {
        atlas.fail_next_lease();
    }
    let generation = cx
        .update_window(handle.into(), |_, window, app| {
            let generation = window.rendered_frame_revision();
            let old_scene = window.rendered_frame.scene.quads[0].background;
            window.refresh();
            let attempt = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app)));
            assert_eq!(attempt.is_err(), unwind);
            assert_eq!(window.rendered_frame_revision(), generation);
            assert!(window.rendered_frame.scene.quads[0].background == old_scene);
            assert!(window.refresh_pending_for_test());
            generation
        })
        .expect("the rejected candidate must retain its original window");
    controls.target.set(1.0);
    controls.playback.set(SpringPlayback::Running);
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update_window(handle.into(), |_, window, app| {
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert!(!window.refresh_pending_for_test());
    })
    .expect("the original target must remain available for accepted retry");
    let (now, actual) = *controls.samples.borrow().last().unwrap();
    let expected = config().step(
        SpringState {
            position: 0.0,
            velocity: 0.0,
        },
        1.0,
        now.duration_since(initial).as_secs_f32(),
    );
    assert!(
        (actual - expected.position).abs() < 0.00001,
        "rejected target/playback must not seed accepted retry: {actual} != {expected:?}"
    );
    drop(batch);
    let root = handle.entity(cx).unwrap().downgrade();
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    assert!(root.upgrade().is_none());
}

#[crate::test]
fn refused_retarget_preserves_the_original_spring_trajectory(cx: &mut TestAppContext) {
    assert_retry_ignores_refused_inputs(cx, SpringPlayback::Running, false);
}

#[crate::test]
fn refused_completed_playback_preserves_the_original_spring_trajectory(cx: &mut TestAppContext) {
    assert_retry_ignores_refused_inputs(cx, SpringPlayback::Completed, false);
}

#[crate::test]
fn refused_cancelled_playback_preserves_the_original_spring_trajectory(cx: &mut TestAppContext) {
    assert_retry_ignores_refused_inputs(cx, SpringPlayback::Cancelled, false);
}

#[crate::test]
fn spring_paint_unwind_preserves_the_original_spring_trajectory(cx: &mut TestAppContext) {
    assert_retry_ignores_refused_inputs(cx, SpringPlayback::Running, true);
}

#[crate::test]
fn refused_first_spring_mount_restarts_from_its_new_initial_state(cx: &mut TestAppContext) {
    let controls = Rc::new(Controls::default());
    controls.enabled.set(false);
    let (handle, atlas) = open(cx, &controls);
    let batch = AnimationFrameBatch::new(cx);
    controls.enabled.set(true);
    controls.sprite.set(true);
    atlas.fail_next_lease();
    cx.update_window(handle.into(), |_, window, app| {
        let generation = window.rendered_frame_revision();
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), generation);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, app| window.draw(app).clear(app))
        .unwrap();
    assert_eq!(controls.samples.borrow().last().unwrap().1, 0.0);
    drop(batch);
}
