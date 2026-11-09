#![cfg(feature = "test-support")]

//! Crate-external consumers of generic animation controls in fixed Zed source
//! 635026fabc38440fce5e1767d125026a17b12012. The configured renderer records CPU Scenes;
//! its Submitted facts do not establish GPU execution, native presentation or performance.
//! These workflows deliberately use no profiler API and cover no spring APIs.

use anyhow::Result;
use open_gpui::{
    Animation, AnimationExt as _, App, AppContext as _, Bounds, Context, DevicePixels, Entity,
    Global, HeadlessAppContext, HeadlessAtlas, InteractiveElement as _, IntoElement,
    NoopTextSystem, ParentElement as _, PlatformAtlas, PlatformHeadlessRenderer,
    PlatformWindowPresentOutcome, QuitMode, Render, Scene, Size, StyleRefinement, Styled as _,
    TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions, div, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Weak},
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
struct Sample {
    index: usize,
    value: f32,
}

#[derive(Default)]
struct Trace {
    renders: Cell<usize>,
    drops: Cell<usize>,
    samples: RefCell<Vec<Sample>>,
}

impl Trace {
    fn latest(&self) -> Sample {
        *self
            .samples
            .borrow()
            .last()
            .expect("the real animation must run")
    }

    fn len(&self) -> usize {
        self.samples.borrow().len()
    }
}

struct AnimatedTile {
    animations: Vec<Animation>,
    trace: Rc<Trace>,
    color: u32,
}

impl Render for AnimatedTile {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.trace.renders.set(self.trace.renders.get() + 1);
        let trace = Rc::clone(&self.trace);
        div()
            .id("animated-parent")
            .h(px(20.))
            .bg(rgb(self.color))
            .with_animations(
                "public-animation",
                self.animations.clone(),
                move |element, index, value| {
                    assert!(value.is_finite());
                    trace.samples.borrow_mut().push(Sample { index, value });
                    element.w(px(20. + value * 40.))
                },
            )
            .child(div().size(px(4.)).bg(rgb(0x66ff99)))
    }
}

impl Drop for AnimatedTile {
    fn drop(&mut self) {
        self.trace.drops.set(self.trace.drops.get() + 1);
    }
}

struct AnimationRoot {
    tiles: Vec<Entity<AnimatedTile>>,
}

impl Render for AnimationRoot {
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
                    .map(|tile| tile.cached(StyleRefinement::default().size(px(80.)))),
            )
    }
}

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(160.), px(440.)),
        ))),
        inactive_frame_interval: None,
        low_power_frame_interval: None,
        thermal_frame_interval: None,
        ..Default::default()
    }
}

fn tile(
    app: &mut App,
    trace: &Rc<Trace>,
    animations: Vec<Animation>,
    color: u32,
) -> Entity<AnimatedTile> {
    app.new(|_| AnimatedTile {
        animations,
        trace: Rc::clone(trace),
        color,
    })
}

fn assert_value(trace: &Trace, index: usize, expected: f32) {
    let actual = trace.latest();
    assert_eq!(actual.index, index, "{actual:?}");
    assert!(
        (actual.value - expected).abs() < 0.01,
        "expected {expected}, got {actual:?}",
    );
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
        anyhow::bail!("the animation consumer records CPU Scenes without producing pixels")
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

fn submit(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<AnimationRoot>,
    recorded: &Recorded,
    slot: usize,
    colors: &[u32],
) -> Result<()> {
    let generation = cx.update_window(window.into(), |_, window, app| {
        window.refresh();
        window.draw(app).clear(app);
        window
            .presentation_facts()
            .frame_accepted_generation
            .unwrap()
    })?;
    assert!(cx.simulate_window_scheduled_frame(window.into())?);
    cx.update_window(window.into(), |_, window, _| {
        let facts = window.presentation_facts();
        assert_eq!(facts.frame_accepted_generation, Some(generation));
        assert_eq!(facts.present_submitted_generation, Some(generation));
        assert_eq!(facts.non_empty_presented_generation, Some(generation));
        assert_eq!(
            facts.latest_present_attempt.unwrap().outcome,
            PlatformWindowPresentOutcome::Submitted
        );
    })?;
    let scenes = recorded.scenes.borrow();
    let scene = scenes[slot]
        .as_ref()
        .expect("the configured CPU renderer must receive the Scene");
    for color in colors {
        assert_eq!(
            scene
                .quads
                .iter()
                .filter(|quad| quad.background == rgb(*color).into())
                .count(),
            1,
            "each real animated view must paint once in the submitted Scene",
        );
    }
    assert_eq!(
        scene
            .quads
            .iter()
            .filter(|quad| quad.background == rgb(0x66ff99).into())
            .count(),
        colors.len(),
        "children added after the animation wrapper must actually paint",
    );
    assert!(scene.monochrome_sprites.is_empty());
    assert!(scene.polychrome_sprites.is_empty());
    Ok(())
}

#[test]
fn public_synced_views_share_the_app_clock_through_cache_window_reconstruction_and_release()
-> Result<()> {
    let recorded = Rc::new(Recorded::default());
    let mut cx = headless(&recorded);
    let weak_app = Rc::downgrade(&cx.app);
    let traces: [Rc<Trace>; 5] = std::array::from_fn(|_| Rc::new(Trace::default()));
    let animation = || {
        Animation::new(Duration::from_secs(1))
            .repeat_synced()
            .with_max_fps(1.)
    };
    let first = cx.update(|app| {
        app.open_window(options(), |_, app| {
            let first = tile(app, &traces[0], vec![animation()], 0x204080);
            app.new(|_| AnimationRoot { tiles: vec![first] })
        })
    })?;
    let weak_first = first.entity(&cx)?.downgrade();
    cx.update_window(first.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    submit(&mut cx, first, &recorded, 0, &[0x204080])?;
    assert_value(&traces[0], 0, 0.);

    cx.advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    let cached_renders = traces[0].renders.get();
    first.update(&mut cx, |root, _, app| {
        root.tiles
            .push(tile(app, &traces[1], vec![animation()], 0x408020));
        root.tiles.push(tile(
            app,
            &traces[2],
            vec![
                Animation::new(Duration::from_secs(1))
                    .repeat()
                    .with_max_fps(1.),
            ],
            0x802040,
        ));
        app.notify();
    })?;
    assert_eq!(
        traces[0].renders.get(),
        cached_renders,
        "an unchanged sibling must replay its cached view"
    );
    assert_value(&traces[1], 0, 0.25);
    assert_value(&traces[2], 0, 0.);
    let second = cx.update(|app| {
        app.open_window(options(), |_, app| {
            let second = tile(app, &traces[3], vec![animation()], 0x804020);
            app.new(|_| AnimationRoot {
                tiles: vec![second],
            })
        })
    })?;
    let weak_second = second.entity(&cx)?.downgrade();
    cx.update_window(second.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    submit(
        &mut cx,
        first,
        &recorded,
        0,
        &[0x204080, 0x408020, 0x802040],
    )?;
    submit(&mut cx, second, &recorded, 1, &[0x804020])?;
    for trace in [&traces[0], &traces[1], &traces[3]] {
        assert_value(trace, 0, 0.25);
    }
    assert_value(&traces[2], 0, 0.);

    cx.advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    submit(
        &mut cx,
        first,
        &recorded,
        0,
        &[0x204080, 0x408020, 0x802040],
    )?;
    submit(&mut cx, second, &recorded, 1, &[0x804020])?;
    for trace in [&traces[0], &traces[1], &traces[3]] {
        assert_value(trace, 0, 0.5);
    }
    assert_value(&traces[2], 0, 0.25);
    cx.update_window(first.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_first.upgrade().is_none());
    for trace in &traces[..3] {
        assert_eq!(trace.drops.get(), 1);
    }
    assert_eq!(recorded.renderer_drops.get(), 1);
    assert!(recorded.atlases.borrow()[0].upgrade().is_none());
    let replacement = cx.update(|app| {
        app.open_window(options(), |_, app| {
            let replacement = tile(app, &traces[4], vec![animation()], 0x208040);
            app.new(|_| AnimationRoot {
                tiles: vec![replacement],
            })
        })
    })?;
    let weak_replacement = replacement.entity(&cx)?.downgrade();
    cx.update_window(replacement.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    submit(&mut cx, replacement, &recorded, 2, &[0x208040])?;
    assert_value(&traces[4], 0, 0.5);
    for handle in [second, replacement] {
        cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
    }
    cx.run_until_parked();
    assert!(weak_second.upgrade().is_none());
    assert!(weak_replacement.upgrade().is_none());
    assert!(cx.update(|app| app.windows().is_empty()));
    assert_eq!(recorded.renderer_drops.get(), 3);
    assert!(
        recorded
            .atlases
            .borrow()
            .iter()
            .all(|atlas| atlas.upgrade().is_none())
    );
    let samples = traces.each_ref().map(|trace| trace.len());
    cx.advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(
        traces.each_ref().map(|trace| trace.len()),
        samples,
        "pending animation timers must not revive retired windows"
    );
    for trace in &traces {
        assert_eq!(trace.drops.get(), 1);
    }
    assert_eq!(recorded.app_drops.get(), 0);
    drop(cx);
    assert!(weak_app.upgrade().is_none());
    assert_eq!(recorded.app_drops.get(), 1);
    Ok(())
}

fn open_test(
    cx: &mut TestAppContext,
    tracks: &[(&Rc<Trace>, Vec<Animation>)],
) -> Result<WindowHandle<AnimationRoot>> {
    cx.defer_next_window_frame_requests();
    let handle = cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.open_window(options(), |_, app| {
            let tiles = tracks
                .iter()
                .enumerate()
                .map(|(index, (trace, animations))| {
                    tile(
                        app,
                        trace,
                        animations.clone(),
                        0x204080 + index as u32 * 0x1010,
                    )
                })
                .collect();
            app.new(|_| AnimationRoot { tiles })
        })
    })?;
    cx.update_window(handle.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    Ok(handle)
}

fn park(cx: &TestAppContext, handle: WindowHandle<AnimationRoot>) {
    for _ in 0..4 {
        if !cx.simulate_window_scheduled_frame(handle.into()) {
            return;
        }
    }
    panic!("a completed or timer-driven animation must park between deadlines");
}

#[test]
fn public_fps_cap_uses_one_pending_timer_while_uncapped_animation_uses_frame_callbacks()
-> Result<()> {
    let mut cx = TestAppContext::single();
    let capped = Rc::new(Trace::default());
    let handle = open_test(
        &mut cx,
        &[(
            &capped,
            vec![
                Animation::new(Duration::from_secs(1))
                    .repeat_synced()
                    .with_max_fps(10.),
            ],
        )],
    )?;
    park(&cx, handle);
    let initial = capped.len();
    cx.executor().advance_clock(Duration::from_millis(45));
    cx.run_until_parked();
    assert_eq!(
        capped.len(),
        initial,
        "the cap must not schedule ordinary per-frame animation work"
    );
    let child = handle
        .entity(&cx)?
        .read_with(&cx, |root, _| root.tiles[0].clone());
    for _ in 0..2 {
        child.update(&mut cx, |_, app| app.notify());
    }
    let externally_redrawn = capped.len();
    assert_eq!(externally_redrawn, initial + 2);
    park(&cx, handle);
    cx.executor().advance_clock(Duration::from_millis(65));
    cx.run_until_parked();
    assert_eq!(
        capped.len(),
        externally_redrawn + 1,
        "an external redraw must not postpone or duplicate the original timer"
    );
    assert!(
        (0.095..0.12).contains(&capped.latest().value),
        "the original deadline must render near 100 ms, got {:?}",
        capped.latest(),
    );
    park(&cx, handle);
    let after_timer = capped.len();
    cx.executor().advance_clock(Duration::from_millis(60));
    cx.run_until_parked();
    assert_eq!(
        capped.len(),
        after_timer,
        "overlapping redraws must not stack independent timers"
    );
    drop(child);
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert_eq!(capped.drops.get(), 1);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(capped.len(), after_timer);

    let uncapped = Rc::new(Trace::default());
    let handle = open_test(
        &mut cx,
        &[(
            &uncapped,
            vec![Animation::new(Duration::from_secs(1)).repeat()],
        )],
    )?;
    let mut renders = uncapped.len();
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_millis(17));
        assert!(cx.simulate_window_scheduled_frame(handle.into()));
        assert!(
            uncapped.len() > renders,
            "uncapped animation must advance through actual scheduled callbacks"
        );
        renders = uncapped.len();
    }
    assert!(uncapped.latest().value > 0.04);
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert_eq!(uncapped.drops.get(), 1);
    cx.quit();
    Ok(())
}

#[test]
fn public_reduced_motion_updates_cached_views_static_chain_endpoints_and_resumes_the_clock()
-> Result<()> {
    let mut cx = TestAppContext::single();
    let traces: [Rc<Trace>; 5] = std::array::from_fn(|_| Rc::new(Trace::default()));
    let one = || Animation::new(Duration::from_secs(1));
    let handle = open_test(
        &mut cx,
        &[
            (&traces[0], vec![one().repeat()]),
            (&traces[1], vec![one()]),
            (&traces[2], vec![one(), one()]),
            (&traces[3], vec![one(), one().repeat()]),
            (&traces[4], vec![one().with_easing(|value| value + 0.25)]),
        ],
    )?;
    assert!(!cx.read(App::reduce_motion));
    assert_value(&traces[4], 0, 0.25);
    cx.executor().advance_clock(Duration::from_millis(250));
    assert!(cx.simulate_window_scheduled_frame(handle.into()));
    assert_value(&traces[0], 0, 0.25);
    assert_value(&traces[1], 0, 0.25);
    assert_value(&traces[4], 0, 0.5);
    let before = traces.each_ref().map(|trace| trace.len());
    cx.update(|app| app.set_reduce_motion(true));
    assert!(cx.read(App::reduce_motion));
    for (trace, before) in traces.iter().zip(before) {
        assert!(
            trace.len() > before,
            "changing the App preference must update already mounted cached views"
        );
    }
    assert_value(&traces[0], 0, 0.);
    assert_value(&traces[1], 0, 1.);
    assert_value(&traces[2], 1, 1.);
    assert_value(&traces[3], 1, 0.);
    assert_value(&traces[4], 0, 1.25);
    park(&cx, handle);
    let static_samples = traces.each_ref().map(|trace| trace.len());
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    assert!(!cx.simulate_window_scheduled_frame(handle.into()));
    assert_eq!(traces.each_ref().map(|trace| trace.len()), static_samples);
    cx.update(|app| app.set_reduce_motion(true));
    assert_eq!(
        traces.each_ref().map(|trace| trace.len()),
        static_samples,
        "setting the same preference must not create animation work"
    );
    cx.update(|app| app.set_reduce_motion(false));
    assert!(!cx.read(App::reduce_motion));
    assert_value(&traces[0], 0, 0.25);
    assert_value(&traces[1], 0, 1.);
    assert_value(&traces[4], 0, 1.25);
    cx.executor().advance_clock(Duration::from_millis(100));
    assert!(cx.simulate_window_scheduled_frame(handle.into()));
    assert_value(&traces[0], 0, 0.35);
    assert_value(&traces[2], 1, 0.1);
    assert_value(&traces[3], 1, 0.1);
    let weak_root = handle.entity(&cx)?.downgrade();
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    for trace in &traces {
        assert_eq!(trace.drops.get(), 1);
    }
    cx.quit();
    Ok(())
}
