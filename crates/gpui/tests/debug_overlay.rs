#![cfg(all(feature = "test-support", feature = "profiler"))]

//! Crate-external consumers of the official Zed frame overlay introduced by 07a0bd1220,
//! from fixed source 635026fabc38440fce5e1767d125026a17b12012.
//! The recording renderer observes complete CPU Scenes. Its configured-renderer presentation
//! facts do not establish GPU execution, native submission, compositor output or performance.

use anyhow::Result;
use open_gpui::{
    AppContext as _, Context, DebugFrameOverlayMode, DevicePixels, Global, HeadlessAppContext,
    IntoElement, NoopTextSystem, Pixels, PlatformAtlas, PlatformHeadlessRenderer,
    PlatformWindowPresentOutcome, Quad, QuitMode, Render, Scene, Size, Styled as _, TestAppContext,
    Window, WindowBounds, WindowHandle, WindowOptions, div, point, px, rgb, rgba, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Weak},
    time::Duration,
};

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    viewport: Cell<Option<Size<Pixels>>>,
    view_drops: Cell<usize>,
    app_drops: Cell<usize>,
    factories: Cell<usize>,
    renderer_drops: Cell<usize>,
    renderer_calls: Cell<usize>,
    frame: RefCell<Option<RecordedScene>>,
    atlases: RefCell<Vec<Weak<dyn PlatformAtlas>>>,
}

#[derive(Clone)]
struct RecordedScene {
    scene: Scene,
    size: Size<DevicePixels>,
}

struct RecordingRenderer {
    atlas: Arc<dyn PlatformAtlas>,
    observed: Rc<Observations>,
}

impl PlatformHeadlessRenderer for RecordingRenderer {
    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        self.observed
            .renderer_calls
            .set(self.observed.renderer_calls.get() + 1);
        self.observed.frame.replace(Some(RecordedScene {
            scene: scene.clone(),
            size,
        }));
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        _: &Scene,
        _: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        anyhow::bail!("this CPU Scene recorder does not render pixels")
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        Arc::clone(&self.atlas)
    }
}

impl Drop for RecordingRenderer {
    fn drop(&mut self) {
        self.observed
            .renderer_drops
            .set(self.observed.renderer_drops.get() + 1);
    }
}

struct AppOwner(Rc<Observations>);

impl Global for AppOwner {}

impl Drop for AppOwner {
    fn drop(&mut self) {
        self.0.app_drops.set(self.0.app_drops.get() + 1);
    }
}

struct OverlayConsumer(Rc<Observations>);

impl Render for OverlayConsumer {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let renders = self.0.renders.get() + 1;
        assert!(renders < 128, "the overlay must not invalidate itself");
        self.0.renders.set(renders);
        self.0.viewport.set(Some(window.viewport_size()));
        div().size_full().bg(rgb(0x123456))
    }
}

impl Drop for OverlayConsumer {
    fn drop(&mut self) {
        self.0.view_drops.set(self.0.view_drops.get() + 1);
    }
}

fn context(observed: &Rc<Observations>) -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), {
        let observed = Rc::clone(observed);
        move || {
            let atlas: Arc<dyn PlatformAtlas> = Arc::new(open_gpui::HeadlessAtlas::default());
            observed.factories.set(observed.factories.get() + 1);
            observed.atlases.borrow_mut().push(Arc::downgrade(&atlas));
            Ok(Some(Box::new(RecordingRenderer {
                atlas,
                observed: Rc::clone(&observed),
            })))
        }
    });
    cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.set_global(AppOwner(Rc::clone(observed)));
    });
    cx
}

fn park(cx: &HeadlessAppContext, handle: WindowHandle<OverlayConsumer>) -> Result<()> {
    for _ in 0..4 {
        if !cx.simulate_window_scheduled_frame(handle.into())? {
            return Ok(());
        }
    }
    panic!("the headless frame source must park after bounded compositor callbacks");
}

fn draw(
    cx: &mut HeadlessAppContext,
    handle: WindowHandle<OverlayConsumer>,
    observed: &Observations,
    control: impl FnOnce(&mut Window),
) -> Result<(RecordedScene, u64)> {
    let renderer_calls = observed.renderer_calls.get();
    let (draws_before, generation, physical_size) =
        cx.update_window(handle.into(), |_, window, app| {
            let draws_before = window
                .frame_duration_snapshot()
                .draw_duration_histogram
                .len();
            let generation_before = window.presentation_facts().frame_accepted_generation;
            control(window);
            window.draw(app).clear(app);
            let generation = window
                .presentation_facts()
                .frame_accepted_generation
                .expect("the public Window must accept the draw");
            assert!(Some(generation) > generation_before);
            assert_eq!(
                window
                    .frame_duration_snapshot()
                    .draw_duration_histogram
                    .len(),
                draws_before + 1,
            );
            (
                draws_before,
                generation,
                window
                    .viewport_size()
                    .to_device_pixels(window.scale_factor()),
            )
        })?;
    assert!(cx.simulate_window_scheduled_frame(handle.into())?);
    assert_eq!(observed.renderer_calls.get(), renderer_calls + 1);
    let recorded = observed
        .frame
        .borrow()
        .clone()
        .expect("the configured CPU renderer must receive the complete Scene");
    assert_eq!(recorded.size, physical_size);
    let facts = cx.update_window(handle.into(), |_, window, _| window.presentation_facts())?;
    assert_eq!(facts.frame_accepted_generation, Some(generation));
    assert_eq!(facts.present_submitted_generation, Some(generation));
    assert_eq!(
        facts.latest_present_attempt.unwrap().outcome,
        PlatformWindowPresentOutcome::Submitted,
    );
    let renders = observed.renders.get();
    park(cx, handle)?;
    cx.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(!cx.simulate_window_scheduled_frame(handle.into())?);
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(observed.renderer_calls.get(), renderer_calls + 1);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window
                .frame_duration_snapshot()
                .draw_duration_histogram
                .len(),
            draws_before + 1,
            "idle callbacks must not add accepted-draw samples",
        );
    })?;
    assert!(recorded.scene.shadows.is_empty());
    assert!(recorded.scene.paths.is_empty());
    assert!(recorded.scene.underlines.is_empty());
    assert!(recorded.scene.monochrome_sprites.is_empty());
    assert!(recorded.scene.subpixel_sprites.is_empty());
    assert!(recorded.scene.polychrome_sprites.is_empty());
    assert!(recorded.scene.surfaces.is_empty());
    let root = recorded
        .scene
        .quads
        .iter()
        .find(|quad| quad.background == rgb(0x123456).into())
        .expect("the real root view must paint alongside the overlay");
    assert_eq!(root.bounds.size.width.0, recorded.size.width.0 as f32);
    assert_eq!(root.bounds.size.height.0, recorded.size.height.0 as f32);
    Ok((recorded, draws_before))
}

fn glyph(rows: [u8; 7]) -> char {
    match rows {
        [0, 0, 0, 0, 0, 0, 0] => ' ',
        [14, 17, 19, 21, 25, 17, 14] => '0',
        [4, 12, 4, 4, 4, 4, 14] => '1',
        [14, 17, 1, 2, 4, 8, 31] => '2',
        [31, 2, 4, 2, 1, 17, 14] => '3',
        [2, 6, 10, 18, 31, 2, 2] => '4',
        [31, 16, 30, 1, 1, 17, 14] => '5',
        [6, 8, 16, 30, 17, 17, 14] => '6',
        [31, 1, 2, 4, 8, 8, 8] => '7',
        [14, 17, 17, 14, 17, 17, 14] => '8',
        [14, 17, 17, 15, 1, 2, 12] => '9',
        [0, 0, 0, 0, 0, 12, 12] => '.',
        [0, 0, 0, 31, 0, 0, 0] => '-',
        [25, 25, 2, 4, 8, 19, 19] => '%',
        [14, 17, 17, 31, 17, 17, 17] => 'A',
        [14, 17, 16, 16, 16, 17, 14] => 'C',
        [31, 16, 16, 30, 16, 16, 31] => 'E',
        [31, 16, 16, 30, 16, 16, 16] => 'F',
        [17, 27, 21, 21, 17, 17, 17] => 'M',
        [30, 17, 17, 30, 20, 18, 17] => 'R',
        [15, 16, 16, 14, 1, 1, 30] => 'S',
        [17, 17, 17, 17, 17, 17, 14] => 'U',
        [17, 17, 10, 4, 10, 17, 17] => 'X',
        _ => panic!("unrecognized bitmap glyph in the actual Scene: {rows:?}"),
    }
}

fn overlay_lines(recorded: &RecordedScene, line_count: usize) -> Vec<String> {
    let panels = recorded
        .scene
        .quads
        .iter()
        .filter(|quad| quad.background == rgba(0x000000aa).into())
        .collect::<Vec<_>>();
    assert_eq!(
        panels.len(),
        1,
        "the Scene must contain one translucent panel"
    );
    let panel = panels[0];
    let glyphs: Vec<&Quad> = recorded
        .scene
        .quads
        .iter()
        .filter(|quad| quad.background == rgba(0x33ff33ff).into())
        .collect();
    let cell = glyphs
        .first()
        .expect("bitmap glyphs must be quads")
        .bounds
        .size
        .height
        .0;
    assert!(cell >= 1.0);
    assert_eq!(panel.bounds.origin.y.0, 4.0 * cell);
    assert_eq!(
        panel.bounds.origin.x.0 + panel.bounds.size.width.0 + 4.0 * cell,
        recorded.size.width.0 as f32,
    );
    assert_eq!(
        panel.bounds.size.height.0,
        (line_count as f32 * 9.0 + 4.0) * cell,
    );
    for quad in std::iter::once(panel).chain(glyphs.iter().copied()) {
        assert_eq!(quad.clip.conservative_bounds.origin.x.0, 0.0);
        assert_eq!(quad.clip.conservative_bounds.origin.y.0, 0.0);
        assert_eq!(
            quad.clip.conservative_bounds.size.width.0,
            recorded.size.width.0 as f32,
        );
        assert_eq!(
            quad.clip.conservative_bounds.size.height.0,
            recorded.size.height.0 as f32,
        );
        assert_eq!(quad.clip.clip_count, 1);
        assert_eq!(
            recorded.scene.clip_shapes()[quad.clip.first_clip as usize].bounds,
            quad.clip.conservative_bounds,
        );
    }
    let columns = (panel.bounds.size.width.0 / cell - 4.0) / 6.0;
    assert_eq!(columns, columns.round());
    (0..line_count)
        .map(|line| {
            let text: String = (0..columns as usize)
                .map(|column| {
                    let left = panel.bounds.origin.x.0 + (2.0 + column as f32 * 6.0) * cell;
                    let top = panel.bounds.origin.y.0 + (2.0 + line as f32 * 9.0) * cell;
                    let mut rows = [0_u8; 7];
                    for quad in &glyphs {
                        let local_column = (quad.bounds.origin.x.0 - left) / cell;
                        let local_row = (quad.bounds.origin.y.0 - top) / cell;
                        if !(0.0..5.0).contains(&local_column) || !(0.0..7.0).contains(&local_row) {
                            continue;
                        }
                        assert_eq!(local_column, local_column.round());
                        assert_eq!(local_row, local_row.round());
                        assert_eq!(quad.bounds.size.height.0, cell);
                        let width = quad.bounds.size.width.0 / cell;
                        assert_eq!(width, width.round());
                        assert!(width >= 1.0 && local_column + width <= 5.0);
                        for offset in 0..width as usize {
                            rows[local_row as usize] |= 1 << (4 - local_column as usize - offset);
                        }
                    }
                    glyph(rows)
                })
                .collect();
            text.trim_end().to_owned()
        })
        .collect()
}

fn assert_duration(text: &str) {
    let milliseconds: f64 = text
        .strip_suffix(" MS")
        .expect("the bitmap readout must include milliseconds")
        .trim()
        .parse()
        .expect("accepted draw history must produce a numeric bitmap readout");
    assert!(milliseconds.is_finite() && milliseconds >= 0.0);
}

fn assert_full(recorded: &RecordedScene, draws_before: u64, reset: bool) {
    let lines = overlay_lines(recorded, 5);
    for (line, prefix) in lines.iter().zip(["CUR ", "1%  ", "10% ", "MAX "]) {
        let duration = line
            .strip_prefix(prefix)
            .expect("the full bitmap label must match");
        if reset {
            assert_eq!(duration, "   -- MS");
        } else {
            assert_duration(duration);
        }
    }
    assert_eq!(lines[4], format!("FRAMES {draws_before:>5}"));
}

#[test]
fn public_headless_overlay_paints_modes_reset_history_and_reconstructed_scenes() -> Result<()> {
    let observed = Rc::new(Observations::default());
    let mut cx = context(&observed);
    let weak_app = Rc::downgrade(&cx.app);
    for (index, viewport) in [size(px(640.0), px(240.0)), size(px(420.0), px(160.0))]
        .into_iter()
        .enumerate()
    {
        let handle = cx.open_window(viewport, |window, app| {
            assert_eq!(
                window.debug_frame_overlay_mode(),
                DebugFrameOverlayMode::Hidden
            );
            app.new(|_| OverlayConsumer(Rc::clone(&observed)))
        })?;
        let weak_view = handle.entity(&cx)?.downgrade();
        cx.update_window(handle.into(), |_, window, _| {
            let _ = window.activate_window();
        })?;
        cx.run_until_parked();
        park(&cx, handle)?;
        let (hidden, _) = draw(&mut cx, handle, &observed, Window::refresh)?;
        assert_eq!(observed.viewport.get(), Some(viewport));
        assert_eq!(hidden.scene.quads.len(), 1);
        let (minimal, _) = draw(
            &mut cx,
            handle,
            &observed,
            Window::cycle_debug_frame_overlay_mode,
        )?;
        let lines = overlay_lines(&minimal, 1);
        assert_duration(&lines[0]);
        let (full, draws_before) = draw(
            &mut cx,
            handle,
            &observed,
            Window::cycle_debug_frame_overlay_mode,
        )?;
        assert!(full.scene.quads.len() > minimal.scene.quads.len());
        assert_full(&full, draws_before, false);
        let (reset, reset_count) = draw(
            &mut cx,
            handle,
            &observed,
            Window::reset_debug_frame_overlay_stats,
        )?;
        assert_eq!(reset_count, draws_before + 1);
        assert_full(&reset, reset_count, true);
        let (repopulated, count) = draw(&mut cx, handle, &observed, Window::refresh)?;
        assert_eq!(count, reset_count + 1);
        assert_full(&repopulated, count, false);
        let (hidden_again, _) = draw(
            &mut cx,
            handle,
            &observed,
            Window::cycle_debug_frame_overlay_mode,
        )?;
        assert_eq!(hidden_again.scene.quads.len(), 1);
        let (restored, count) = draw(&mut cx, handle, &observed, |window| {
            window.set_debug_frame_overlay_mode(DebugFrameOverlayMode::Full);
        })?;
        assert_full(&restored, count, false);
        assert_eq!(count, reset_count + 3);
        cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
        cx.run_until_parked();
        assert!(cx.update(|app| app.windows().is_empty()));
        assert!(weak_view.upgrade().is_none());
        assert!(cx.update_window(handle.into(), |_, _, _| ()).is_err());
        assert_eq!(observed.view_drops.get(), index + 1);
        assert_eq!(observed.factories.get(), index + 1);
        assert_eq!(observed.renderer_drops.get(), index + 1);
        assert!(observed.atlases.borrow()[index].upgrade().is_none());
        assert_eq!(observed.app_drops.get(), 0);
    }
    drop(cx);
    assert!(weak_app.upgrade().is_none());
    assert_eq!(observed.app_drops.get(), 1);
    assert_eq!(observed.view_drops.get(), 2);
    assert_eq!(observed.renderer_drops.get(), 2);
    Ok(())
}

#[test]
fn public_controls_redraw_across_platform_resize_and_root_replacement() -> Result<()> {
    let mut cx = TestAppContext::single();
    let observed = Rc::new(Observations::default());
    let weak_app = Rc::downgrade(&cx.app);
    cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.set_global(AppOwner(Rc::clone(&observed)));
    });
    let handle = cx.update(|app| {
        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(open_gpui::Bounds::new(
                    point(px(0.0), px(0.0)),
                    size(px(640.0), px(240.0)),
                ))),
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            },
            |_, app| app.new(|_| OverlayConsumer(Rc::clone(&observed))),
        )
    })?;
    let weak_original = handle.entity(&cx)?.downgrade();
    for mode in [DebugFrameOverlayMode::Minimal, DebugFrameOverlayMode::Full] {
        let renders = observed.renders.get();
        cx.update_window(handle.into(), |_, window, _| {
            window.set_debug_frame_overlay_mode(mode);
            assert_eq!(window.debug_frame_overlay_mode(), mode);
        })?;
        assert!(observed.renders.get() > renders);
    }
    let renders = observed.renders.get();
    cx.update_window(handle.into(), |_, window, _| {
        window.reset_debug_frame_overlay_stats();
    })?;
    assert!(observed.renders.get() > renders);
    let viewport = size(px(420.0), px(160.0));
    let renders = observed.renders.get();
    cx.simulate_window_resize(handle.into(), viewport);
    assert!(observed.renders.get() > renders);
    assert_eq!(observed.viewport.get(), Some(viewport));
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.viewport_size(), viewport);
        assert_eq!(
            window.debug_frame_overlay_mode(),
            DebugFrameOverlayMode::Full
        );
    })?;
    let weak_replacement = cx.update_window(handle.into(), |_, window, app| {
        window
            .replace_root(app, |_, _| OverlayConsumer(Rc::clone(&observed)))
            .downgrade()
    })?;
    assert!(weak_original.upgrade().is_none());
    assert_eq!(observed.view_drops.get(), 1);
    assert_eq!(observed.viewport.get(), Some(viewport));
    let renders = observed.renders.get();
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.debug_frame_overlay_mode(),
            DebugFrameOverlayMode::Full
        );
        window.cycle_debug_frame_overlay_mode();
        assert_eq!(
            window.debug_frame_overlay_mode(),
            DebugFrameOverlayMode::Hidden
        );
    })?;
    assert!(observed.renders.get() > renders);
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
    cx.run_until_parked();
    assert!(weak_replacement.upgrade().is_none());
    assert!(cx.windows().is_empty());
    assert_eq!(observed.view_drops.get(), 2);
    cx.quit();
    drop(cx);
    assert!(weak_app.upgrade().is_none());
    assert_eq!(observed.app_drops.get(), 1);
    Ok(())
}
