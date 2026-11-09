#![cfg(feature = "test-support")]

//! Crate-external consumers of the complete official Spring API at Zed
//! 635026fabc38440fce5e1767d125026a17b12012. TestApp workflows execute real element state;
//! the headless renderer records CPU Scenes without GPU, native or performance evidence.

use anyhow::Result;
use open_gpui::{
    AnimationExt as _, AnimationPhase, App, AppContext, Bounds, Context, DevicePixels, Entity,
    Global, HeadlessAppContext, HeadlessAtlas, InteractiveElement as _, IntoElement,
    NoopTextSystem, ParentElement as _, Pixels, PlatformAtlas, PlatformHeadlessRenderer,
    PlatformWindowPresentOutcome, QuitMode, Render, Rgba, Scene, Size, SpringAnimation,
    SpringConfig, SpringPlayback, SpringState, SpringTarget, StyleRefinement, Styled as _,
    TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions, div, point, px, rgb,
    sampled_easing, size,
};
use open_gpui_scheduler::Instant;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Weak},
    time::Duration,
};

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.0005,
        "expected {expected}, got {actual}",
    );
}

#[test]
fn public_gpui_and_motion_exports_are_the_same_solver_types() {
    let motion_config: open_gpui_motion::SpringConfig = SpringConfig::new(100., 4., 1.);
    let gpui_config: SpringConfig = motion_config;
    let motion_state: open_gpui_motion::SpringState = SpringState {
        position: 0.2,
        velocity: 1.,
    };
    let gpui_state: SpringState = motion_state;
    assert_eq!(
        gpui_config.step(gpui_state, 1., 0.05),
        motion_config.step(motion_state, 1., 0.05),
    );
    let (gpui_duration, gpui_easing) = sampled_easing(gpui_config, 0.001);
    let (motion_duration, motion_easing) = open_gpui_motion::sampled_easing(motion_config, 0.001);
    assert_eq!(gpui_duration, motion_duration);
    for progress in [0., 0.1, 0.5, 0.9, 1.] {
        assert_eq!(gpui_easing(progress), motion_easing(progress));
    }
    let _: SpringAnimation<Pixels> = SpringAnimation::new(motion_config)
        .to(px(100.))
        .from(px(motion_state.position));
}

#[test]
fn public_builtin_pixels_and_bool_targets_render_unbounded_phases_and_clamped_colors() -> Result<()>
{
    struct BuiltinTargets {
        width: Pixels,
        enabled: bool,
        widths: Rc<RefCell<Vec<Pixels>>>,
        phases: Rc<RefCell<Vec<AnimationPhase>>>,
        colors: Rc<RefCell<Vec<Rgba>>>,
    }

    impl Render for BuiltinTargets {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let config = SpringConfig::new(100., 2., 1.);
            let widths = Rc::clone(&self.widths);
            let phases = Rc::clone(&self.phases);
            let colors = Rc::clone(&self.colors);
            div()
                .size_full()
                .child(div().h(px(20.)).with_spring(
                    "pixel-target",
                    SpringAnimation::new(config).to(self.width).from(px(20.)),
                    move |element, width: Pixels| {
                        widths.borrow_mut().push(width);
                        element.w(width)
                    },
                ))
                .child(div().size(px(20.)).with_spring(
                    "bool-target",
                    SpringAnimation::new(config).to(self.enabled).from(false),
                    move |element, phase: AnimationPhase| {
                        let color = phase.interpolate_clamped(rgb(0x224466), rgb(0x6688aa));
                        phases.borrow_mut().push(phase);
                        colors.borrow_mut().push(color);
                        element.bg(color)
                    },
                ))
        }
    }

    let mut cx = TestAppContext::single();
    cx.defer_next_window_frame_requests();
    let widths = Rc::new(RefCell::new(Vec::new()));
    let phases = Rc::new(RefCell::new(Vec::new()));
    let colors = Rc::new(RefCell::new(Vec::new()));
    let window = cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.open_window(options(), |_, app| {
            app.new(|_| BuiltinTargets {
                width: px(100.),
                enabled: true,
                widths: Rc::clone(&widths),
                phases: Rc::clone(&phases),
                colors: Rc::clone(&colors),
            })
        })
    })?;
    cx.update_window(window.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    assert_eq!(*widths.borrow().last().unwrap(), px(20.));
    assert_eq!(*phases.borrow().last().unwrap(), AnimationPhase(0.));
    cx.executor().advance_clock(Duration::from_millis(300));
    assert!(cx.simulate_window_scheduled_frame(window.into()));
    let phase = *phases.borrow().last().unwrap();
    assert!(
        phase.0 > 1.1,
        "the bool target must preserve overshooting phases"
    );
    close(
        widths.borrow().last().unwrap().as_f32(),
        20. + 80. * phase.0,
    );
    let actual = *colors.borrow().last().unwrap();
    let expected = rgb(0x6688aa);
    for (actual, expected) in [
        (actual.r, expected.r),
        (actual.g, expected.g),
        (actual.b, expected.b),
        (actual.a, expected.a),
    ] {
        close(actual, expected);
    }
    window.update(&mut cx, |root, _, app| {
        root.width = px(40.);
        root.enabled = false;
        app.notify();
    })?;
    cx.update(|app| app.set_reduce_motion(true));
    assert_eq!(*widths.borrow().last().unwrap(), px(40.));
    assert_eq!(*phases.borrow().last().unwrap(), AnimationPhase(0.));
    assert_eq!(*colors.borrow().last().unwrap(), rgb(0x224466));
    let weak_root = window.entity(&cx)?.downgrade();
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    cx.quit();
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct TileTarget {
    coordinate: f32,
    color: u32,
}

#[derive(Clone, Copy, Debug)]
struct TilePose {
    coordinate: f32,
    width: Pixels,
    color: Rgba,
}

impl SpringTarget for TileTarget {
    type Output = TilePose;

    fn target(&self) -> f32 {
        self.coordinate
    }

    fn resolve(&self, value: f32) -> Self::Output {
        let phase = AnimationPhase(value);
        TilePose {
            coordinate: value,
            width: phase.interpolate(px(20.), px(100.)),
            color: phase.interpolate_clamped(rgb(self.color), rgb(0xddeeff)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    at: Instant,
    pose: TilePose,
}

#[derive(Default)]
struct Trace {
    samples: RefCell<Vec<Sample>>,
    renders: Cell<usize>,
    drops: Cell<usize>,
}

impl Trace {
    fn latest(&self) -> Sample {
        *self
            .samples
            .borrow()
            .last()
            .expect("the real Spring animator must run")
    }

    fn len(&self) -> usize {
        self.samples.borrow().len()
    }
}

#[derive(Clone)]
struct Spec {
    target: f32,
    initial: Option<f32>,
    config: SpringConfig,
    playback: SpringPlayback,
    ancestor: &'static str,
    visible: bool,
    color: u32,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            target: 1.,
            initial: Some(0.),
            config: SpringConfig::new(100., 4., 1.),
            playback: SpringPlayback::Running,
            ancestor: "original-ancestor",
            visible: true,
            color: 0x224466,
        }
    }
}

struct SpringTile {
    spec: Spec,
    trace: Rc<Trace>,
}

impl Render for SpringTile {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trace.renders.set(self.trace.renders.get() + 1);
        let at = cx.background_executor().now();
        let trace = Rc::clone(&self.trace);
        let target = TileTarget {
            coordinate: self.spec.target,
            color: self.spec.color,
        };
        let mut animation = SpringAnimation::new(self.spec.config)
            .to(target)
            .with_epsilon(0.0001)
            .playback(self.spec.playback);
        if let Some(initial) = self.spec.initial {
            animation = animation.from(TileTarget {
                coordinate: initial,
                color: self.spec.color,
            });
        }
        div()
            .id(self.spec.ancestor)
            .size_full()
            .children(self.spec.visible.then(|| {
                div()
                    .id("painted-spring-parent")
                    .with_spring("public-spring", animation, move |element, pose| {
                        assert!(pose.coordinate.is_finite());
                        trace.samples.borrow_mut().push(Sample { at, pose });
                        element.w(pose.width).bg(pose.color)
                    })
                    .child(div().size(px(4.)).bg(rgb(0x77ee55)))
                    .map_element(|element| element.h(px(20.)))
            }))
    }
}

impl Drop for SpringTile {
    fn drop(&mut self) {
        self.trace.drops.set(self.trace.drops.get() + 1);
    }
}

struct SpringRoot {
    tiles: Vec<Entity<SpringTile>>,
}

impl Render for SpringRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101820))
            .children(
                self.tiles
                    .iter()
                    .cloned()
                    .map(|tile| tile.cached(StyleRefinement::default().w(px(140.)).h(px(36.)))),
            )
    }
}

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(180.), px(180.)),
        ))),
        inactive_frame_interval: None,
        low_power_frame_interval: None,
        thermal_frame_interval: None,
        ..Default::default()
    }
}

fn build_root(app: &mut App, specs: &[(Spec, Rc<Trace>)]) -> Entity<SpringRoot> {
    let tiles = specs
        .iter()
        .map(|(spec, trace)| {
            app.new(|_| SpringTile {
                spec: spec.clone(),
                trace: Rc::clone(trace),
            })
        })
        .collect();
    app.new(|_| SpringRoot { tiles })
}

fn open_test(
    cx: &mut TestAppContext,
    specs: &[(Spec, Rc<Trace>)],
) -> Result<WindowHandle<SpringRoot>> {
    cx.defer_next_window_frame_requests();
    let window = cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.open_window(options(), |_, app| build_root(app, specs))
    })?;
    cx.update_window(window.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    Ok(window)
}

fn update_spec(
    cx: &mut impl AppContext,
    window: WindowHandle<SpringRoot>,
    index: usize,
    update: impl FnOnce(&mut Spec),
) -> Result<()> {
    window.update(cx, |root, _, app| {
        root.tiles[index].update(app, |tile, app| {
            update(&mut tile.spec);
            app.notify();
        });
    })
}

fn tick(cx: &mut TestAppContext, window: WindowHandle<SpringRoot>, duration: Duration) {
    cx.executor().advance_clock(duration);
    assert!(cx.simulate_window_scheduled_frame(window.into()));
}

fn park(cx: &TestAppContext, window: WindowHandle<SpringRoot>) {
    for _ in 0..4 {
        if !cx.simulate_window_scheduled_frame(window.into()) {
            return;
        }
    }
    panic!("a static Spring must stop requesting animation frames");
}

fn assert_step(
    config: SpringConfig,
    state: SpringState,
    target: f32,
    previous: Sample,
    next: Sample,
) -> SpringState {
    let state = config.step(
        state,
        target,
        next.at.duration_since(previous.at).as_secs_f32(),
    );
    close(next.pose.coordinate, state.position);
    close(next.pose.width.as_f32(), 20. + state.position * 80.);
    state
}

#[test]
fn public_stateful_spring_retargets_with_old_configuration_and_preserves_playback_state()
-> Result<()> {
    let mut cx = TestAppContext::single();
    let trace = Rc::new(Trace::default());
    let config = SpringConfig::new(100., 4., 1.);
    let window = open_test(&mut cx, &[(Spec::default(), Rc::clone(&trace))])?;
    let mounted = trace.latest();
    close(mounted.pose.coordinate, 0.);
    tick(&mut cx, window, Duration::from_millis(50));
    let before = trace.latest();
    let state = assert_step(config, SpringState::default(), 1., mounted, before);
    assert!((0.05..0.5).contains(&state.position));
    assert!(state.velocity > 0.);

    cx.executor().advance_clock(Duration::from_millis(25));
    let replacement_config = SpringConfig::new(64., 12., 1.);
    update_spec(&mut cx, window, 0, |spec| {
        spec.target = 0.;
        spec.initial = Some(0.8);
        spec.config = replacement_config;
    })?;
    let retargeted = trace.latest();
    let state = assert_step(config, state, 1., before, retargeted);
    assert!(retargeted.pose.coordinate > before.pose.coordinate);
    assert!(retargeted.pose.coordinate < 0.8);
    tick(&mut cx, window, Duration::from_millis(5));
    let advancing = trace.latest();
    let state = assert_step(replacement_config, state, 0., retargeted, advancing);
    assert!(advancing.pose.coordinate > retargeted.pose.coordinate);

    update_spec(&mut cx, window, 0, |spec| {
        spec.playback = SpringPlayback::Paused
    })?;
    let paused = trace.latest();
    let state = assert_step(replacement_config, state, 0., advancing, paused);
    park(&cx, window);
    let paused_count = trace.len();
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(!cx.simulate_window_scheduled_frame(window.into()));
    assert_eq!(trace.len(), paused_count);
    update_spec(&mut cx, window, 0, |spec| {
        spec.playback = SpringPlayback::Running
    })?;
    let resumed = trace.latest();
    close(resumed.pose.coordinate, paused.pose.coordinate);
    tick(&mut cx, window, Duration::from_millis(5));
    let running = trace.latest();
    let state = assert_step(replacement_config, state, 0., resumed, running);
    assert!(running.pose.coordinate > resumed.pose.coordinate);

    update_spec(&mut cx, window, 0, |spec| {
        spec.playback = SpringPlayback::Stopped
    })?;
    let stopped = trace.latest();
    let mut state = assert_step(replacement_config, state, 0., running, stopped);
    state.velocity = 0.;
    park(&cx, window);
    cx.executor().advance_clock(Duration::from_secs(1));
    update_spec(&mut cx, window, 0, |spec| {
        spec.playback = SpringPlayback::Running
    })?;
    let restarted = trace.latest();
    close(restarted.pose.coordinate, stopped.pose.coordinate);
    tick(&mut cx, window, Duration::from_millis(20));
    let after_stop = trace.latest();
    assert_step(replacement_config, state, 0., restarted, after_stop);
    assert!(after_stop.pose.coordinate < restarted.pose.coordinate);

    update_spec(&mut cx, window, 0, |spec| {
        spec.target = 0.75;
        spec.playback = SpringPlayback::Completed;
    })?;
    close(trace.latest().pose.coordinate, 0.75);
    park(&cx, window);
    update_spec(&mut cx, window, 0, |spec| {
        spec.playback = SpringPlayback::Cancelled
    })?;
    close(trace.latest().pose.coordinate, 0.);
    park(&cx, window);
    update_spec(&mut cx, window, 0, |spec| {
        spec.target = 1.;
        spec.playback = SpringPlayback::Running;
    })?;
    let reset = trace.latest();
    close(reset.pose.coordinate, 0.);
    tick(&mut cx, window, Duration::from_millis(30));
    assert_step(
        replacement_config,
        SpringState::default(),
        1.,
        reset,
        trace.latest(),
    );
    let weak_root = window.entity(&cx)?.downgrade();
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    assert_eq!(trace.drops.get(), 1);
    cx.quit();
    Ok(())
}

#[test]
fn public_default_mount_from_and_ancestor_identity_define_new_spring_state() -> Result<()> {
    let mut cx = TestAppContext::single();
    let trace = Rc::new(Trace::default());
    let window = open_test(
        &mut cx,
        &[(
            Spec {
                target: 0.7,
                initial: None,
                ..Default::default()
            },
            Rc::clone(&trace),
        )],
    )?;
    close(trace.latest().pose.coordinate, 0.7);
    park(&cx, window);
    update_spec(&mut cx, window, 0, |spec| {
        spec.target = 1.;
        spec.initial = Some(0.2);
    })?;
    close(trace.latest().pose.coordinate, 0.7);
    tick(&mut cx, window, Duration::from_millis(60));
    assert!(trace.latest().pose.coordinate > 0.7);
    update_spec(&mut cx, window, 0, |spec| {
        spec.ancestor = "replacement-ancestor";
        spec.initial = Some(0.3);
    })?;
    close(trace.latest().pose.coordinate, 0.3);
    update_spec(&mut cx, window, 0, |spec| spec.visible = false)?;
    let hidden = trace.len();
    park(&cx, window);
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(trace.len(), hidden);
    update_spec(&mut cx, window, 0, |spec| {
        spec.visible = true;
        spec.initial = Some(0.4);
        spec.target = 0.9;
    })?;
    close(trace.latest().pose.coordinate, 0.4);
    let weak_root = window.entity(&cx)?.downgrade();
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    assert_eq!(trace.drops.get(), 1);
    let old_samples = trace.len();
    let replacement_trace = Rc::new(Trace::default());
    let replacement = open_test(
        &mut cx,
        &[(
            Spec {
                target: 0.8,
                initial: Some(0.1),
                ..Default::default()
            },
            Rc::clone(&replacement_trace),
        )],
    )?;
    close(replacement_trace.latest().pose.coordinate, 0.1);
    tick(&mut cx, replacement, Duration::from_millis(30));
    assert!(replacement_trace.latest().pose.coordinate > 0.1);
    assert_eq!(trace.len(), old_samples);
    cx.update_window(replacement.into(), |_, window, app| {
        window.remove_window(app)
    })?;
    cx.run_until_parked();
    assert_eq!(replacement_trace.drops.get(), 1);
    assert!(cx.read(|app| app.windows().is_empty()));
    cx.quit();
    Ok(())
}

#[test]
fn public_reduced_motion_updates_cached_springs_and_resumes_from_the_snapped_endpoint() -> Result<()>
{
    let mut cx = TestAppContext::single();
    let moving = Rc::new(Trace::default());
    let idle = Rc::new(Trace::default());
    let window = open_test(
        &mut cx,
        &[
            (Spec::default(), Rc::clone(&moving)),
            (
                Spec {
                    target: 0.25,
                    initial: None,
                    color: 0x442266,
                    ..Default::default()
                },
                Rc::clone(&idle),
            ),
        ],
    )?;
    let idle_renders = idle.renders.get();
    let idle_samples = idle.len();
    tick(&mut cx, window, Duration::from_millis(50));
    assert!(moving.latest().pose.coordinate > 0.);
    assert_eq!(idle.renders.get(), idle_renders);
    assert_eq!(idle.len(), idle_samples);
    let before = moving.len();
    cx.update(|app| app.set_reduce_motion(true));
    assert!(cx.read(App::reduce_motion));
    assert!(moving.len() > before);
    assert!(idle.renders.get() > idle_renders);
    close(moving.latest().pose.coordinate, 1.);
    close(idle.latest().pose.coordinate, 0.25);
    park(&cx, window);
    let static_samples = (moving.len(), idle.len());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(!cx.simulate_window_scheduled_frame(window.into()));
    assert_eq!((moving.len(), idle.len()), static_samples);
    cx.update(|app| app.set_reduce_motion(true));
    assert_eq!((moving.len(), idle.len()), static_samples);
    cx.update(|app| app.set_reduce_motion(false));
    assert!(!cx.read(App::reduce_motion));
    close(moving.latest().pose.coordinate, 1.);
    park(&cx, window);
    update_spec(&mut cx, window, 0, |spec| spec.target = 0.2)?;
    let retargeted = moving.latest();
    close(retargeted.pose.coordinate, 1.);
    tick(&mut cx, window, Duration::from_millis(60));
    let state = assert_step(
        Spec::default().config,
        SpringState {
            position: 1.,
            velocity: 0.,
        },
        0.2,
        retargeted,
        moving.latest(),
    );
    assert!((0.2..1.).contains(&state.position));
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert_eq!(moving.drops.get(), 1);
    assert_eq!(idle.drops.get(), 1);
    cx.quit();
    Ok(())
}

#[derive(Default)]
struct Recorded {
    scenes: RefCell<Vec<Option<Scene>>>,
    atlases: RefCell<Vec<Weak<dyn PlatformAtlas>>>,
    renderer_drops: Cell<usize>,
    app_drops: Cell<usize>,
}

struct Recorder {
    slot: usize,
    atlas: Arc<dyn PlatformAtlas>,
    recorded: Rc<Recorded>,
}

impl PlatformHeadlessRenderer for Recorder {
    fn render_scene(&mut self, scene: &Scene, _: Size<DevicePixels>) -> Result<()> {
        self.recorded.scenes.borrow_mut()[self.slot] = Some(scene.clone());
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        _: &Scene,
        _: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        anyhow::bail!("the Spring consumer records CPU Scenes without producing pixels")
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        Arc::clone(&self.atlas)
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.recorded
            .renderer_drops
            .set(self.recorded.renderer_drops.get() + 1);
    }
}

struct AppOwner(Rc<Recorded>);

impl Global for AppOwner {}

impl Drop for AppOwner {
    fn drop(&mut self) {
        self.0.app_drops.set(self.0.app_drops.get() + 1);
    }
}

fn headless(recorded: &Rc<Recorded>) -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), {
        let recorded = Rc::clone(recorded);
        move || {
            let slot = recorded.scenes.borrow().len();
            recorded.scenes.borrow_mut().push(None);
            let atlas: Arc<dyn PlatformAtlas> = Arc::new(HeadlessAtlas::default());
            recorded.atlases.borrow_mut().push(Arc::downgrade(&atlas));
            Ok(Some(Box::new(Recorder {
                slot,
                atlas,
                recorded: Rc::clone(&recorded),
            })))
        }
    });
    cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.set_global(AppOwner(Rc::clone(recorded)));
    });
    cx
}

fn submit_scene(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<SpringRoot>,
    recorded: &Recorded,
    slot: usize,
    traces: &[&Trace],
) -> Result<()> {
    cx.update_window(window.into(), |_, window, app| window.draw(app).clear(app))?;
    assert!(cx.simulate_window_scheduled_frame(window.into())?);
    let scale = cx.update_window(window.into(), |_, window, _| {
        let facts = window.presentation_facts();
        let accepted = facts
            .frame_accepted_generation
            .expect("a real Scene must be accepted");
        assert_eq!(facts.present_submitted_generation, Some(accepted));
        assert_eq!(facts.non_empty_presented_generation, Some(accepted));
        assert_eq!(
            facts.latest_present_attempt.unwrap().outcome,
            PlatformWindowPresentOutcome::Submitted,
        );
        window.scale_factor()
    })?;
    let scenes = recorded.scenes.borrow();
    let scene = scenes[slot]
        .as_ref()
        .expect("the configured CPU renderer must receive the Scene");
    for trace in traces {
        let pose = trace.latest().pose;
        let quads: Vec<_> = scene
            .quads
            .iter()
            .filter(|quad| quad.background == pose.color.into())
            .collect();
        assert_eq!(
            quads.len(),
            1,
            "each typed Spring output must paint its own quad"
        );
        close(
            quads[0].bounds.size.width.0,
            (pose.width.as_f32() * scale).round(),
        );
        close(quads[0].bounds.size.height.0, 20. * scale);
    }
    assert_eq!(
        scene
            .quads
            .iter()
            .filter(|quad| quad.background == rgb(0x77ee55).into())
            .count(),
        traces.len(),
        "children appended after the Spring wrapper must actually paint",
    );
    assert!(scene.monochrome_sprites.is_empty());
    assert!(scene.subpixel_sprites.is_empty());
    assert!(scene.polychrome_sprites.is_empty());
    Ok(())
}

#[test]
fn public_headless_spring_paints_typed_parent_children_and_releases_reconstructed_windows()
-> Result<()> {
    let recorded = Rc::new(Recorded::default());
    let mut cx = headless(&recorded);
    let weak_app = Rc::downgrade(&cx.app);
    let weak_text_system = Arc::downgrade(cx.text_system());
    let moving = Rc::new(Trace::default());
    let idle = Rc::new(Trace::default());
    let window = cx.update(|app| {
        app.open_window(options(), |_, app| {
            build_root(
                app,
                &[
                    (Spec::default(), Rc::clone(&moving)),
                    (
                        Spec {
                            target: 0.5,
                            initial: None,
                            color: 0x662244,
                            ..Default::default()
                        },
                        Rc::clone(&idle),
                    ),
                ],
            )
        })
    })?;
    let weak_root = window.entity(&cx)?.downgrade();
    let weak_child = window
        .entity(&cx)?
        .read_with(&cx, |root, _| root.tiles[0].downgrade());
    cx.update_window(window.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    submit_scene(&mut cx, window, &recorded, 0, &[&moving, &idle])?;
    close(moving.latest().pose.coordinate, 0.);
    close(idle.latest().pose.coordinate, 0.5);
    let idle_renders = idle.renders.get();
    let initial_samples = moving.len();
    cx.advance_clock(Duration::from_millis(50));
    assert!(cx.simulate_window_scheduled_frame(window.into())?);
    assert!(moving.len() > initial_samples);
    assert!(moving.latest().pose.coordinate > 0.);
    assert_eq!(idle.renders.get(), idle_renders);
    submit_scene(&mut cx, window, &recorded, 0, &[&moving, &idle])?;
    assert_eq!(idle.renders.get(), idle_renders);
    let before = moving.latest().pose.coordinate;
    update_spec(&mut cx, window, 0, |spec| spec.target = 0.)?;
    cx.advance_clock(Duration::from_millis(5));
    assert!(cx.simulate_window_scheduled_frame(window.into())?);
    assert!(moving.latest().pose.coordinate > before);
    submit_scene(&mut cx, window, &recorded, 0, &[&moving, &idle])?;
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    assert!(weak_child.upgrade().is_none());
    assert_eq!(moving.drops.get(), 1);
    assert_eq!(idle.drops.get(), 1);
    assert_eq!(recorded.renderer_drops.get(), 1);
    assert!(recorded.atlases.borrow()[0].upgrade().is_none());
    let retired_samples = (moving.len(), idle.len());
    cx.advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!((moving.len(), idle.len()), retired_samples);
    let replacement_trace = Rc::new(Trace::default());
    let replacement = cx.update(|app| {
        app.open_window(options(), |_, app| {
            build_root(
                app,
                &[(
                    Spec {
                        target: 0.6,
                        initial: None,
                        ..Default::default()
                    },
                    Rc::clone(&replacement_trace),
                )],
            )
        })
    })?;
    let weak_replacement = replacement.entity(&cx)?.downgrade();
    cx.update_window(replacement.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    submit_scene(&mut cx, replacement, &recorded, 1, &[&replacement_trace])?;
    close(replacement_trace.latest().pose.coordinate, 0.6);
    cx.update_window(replacement.into(), |_, window, app| {
        window.remove_window(app)
    })?;
    cx.run_until_parked();
    assert!(weak_replacement.upgrade().is_none());
    assert_eq!(replacement_trace.drops.get(), 1);
    assert!(cx.update(|app| app.windows().is_empty()));
    assert_eq!(recorded.renderer_drops.get(), 2);
    assert!(
        recorded
            .atlases
            .borrow()
            .iter()
            .all(|atlas| atlas.upgrade().is_none())
    );
    assert_eq!(recorded.app_drops.get(), 0);
    drop(cx);
    assert!(weak_app.upgrade().is_none());
    assert!(weak_text_system.upgrade().is_none());
    assert_eq!(recorded.app_drops.get(), 1);
    Ok(())
}
