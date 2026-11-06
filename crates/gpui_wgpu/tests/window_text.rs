//! Public ownership consumers for Zed 635026fabc38440fce5e1767d125026a17b12012,
//! including 772a8a9100 and 2a74c8dff6.
//!
//! Rc/cache ownership is the upstream contract. Frame acceptance observes this fork's
//! journal; headless execution does not qualify native renderer submission.
#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui::{
    AppContext as _, Context, FontId, GlyphId, HeadlessAppContext, IntoElement, LineLayout,
    ParentElement as _, Pixels, PlatformTextSystem as _, Point, QuitMode, Render, ShapedLine,
    SharedString, Styled as _, TextAlign, TextRenderingMode, TextRun, TextSystem, Window,
    WindowHandle, WindowTextSystem, WrappedLineLayout, canvas, div, font, point, px, rgb, size,
};
use open_gpui_wgpu::CosmicTextSystem;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

const IBM_PLEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const LILEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/Lilex-Regular.ttf");
const SOURCE: &str = "mi café e\u{301} alpha beta gamma delta epsilon omega";

#[derive(Clone)]
struct ObservedLayouts {
    plain: ShapedLine,
    wrapped: Arc<WrappedLineLayout>,
    wrapped_text: SharedString,
    width: Pixels,
    version: usize,
    previous_revision: u64,
    requested_font: FontId,
}

struct WindowTextConsumer {
    layouts: Rc<RefCell<Vec<ObservedLayouts>>>,
    width: Pixels,
    version: usize,
    retained: Option<ShapedLine>,
}

impl Render for WindowTextConsumer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let layouts = self.layouts.clone();
        let version = self.version;
        let retained = self.retained.clone();
        div().size_full().child(
            canvas(
                move |bounds, window, cx| {
                    let window_text: &Rc<WindowTextSystem> = window.text_system();
                    let global: &Arc<TextSystem> = window_text.as_ref();
                    assert!(Arc::ptr_eq(global, cx.text_system()));
                    let runs = [
                        TextRun {
                            len: 1,
                            font: font("Lilex"),
                            color: rgb(0x203040).into(),
                            ..Default::default()
                        },
                        TextRun {
                            len: SOURCE.len() - 1,
                            font: font("IBM Plex Sans"),
                            color: rgb(0x203040).into(),
                            ..Default::default()
                        },
                    ];
                    let plain = window_text.shape_line(SOURCE.into(), px(20.), &runs, None);
                    let repeated = window_text.shape_line(SOURCE.into(), px(20.), &runs, None);
                    assert_eq!(plain.text.as_ref(), SOURCE);
                    assert_eq!(repeated.text.as_ref(), SOURCE);
                    assert!(Arc::ptr_eq(&*plain, &*repeated));
                    assert!(Arc::ptr_eq(
                        &*plain,
                        &window_text.layout_line(SOURCE, px(20.), &runs, None),
                    ));
                    let wrapped = window_text
                        .shape_text(SOURCE.into(), px(20.), &runs, Some(bounds.size.width), None)
                        .expect("the full source must shape through the window cache");
                    let repeated = window_text
                        .shape_text(SOURCE.into(), px(20.), &runs, Some(bounds.size.width), None)
                        .expect("unchanged full-source wrapping must be reusable");
                    assert_eq!(wrapped.len(), 1);
                    assert_eq!(repeated.len(), 1);
                    assert_eq!(wrapped[0].text.as_ref(), SOURCE);
                    assert_eq!(repeated[0].text.as_ref(), SOURCE);
                    assert!(Arc::ptr_eq(&*wrapped[0], &*repeated[0]));
                    assert!(Arc::ptr_eq(&*plain, &wrapped[0].unwrapped_layout));
                    let requested_font = window_text.resolve_font(&font("Lilex"));
                    (plain, wrapped, requested_font)
                },
                move |bounds, (plain, wrapped, requested_font), window, cx| {
                    plain
                        .paint(bounds.origin, px(28.), TextAlign::Left, None, window, cx)
                        .expect("the window must paint its own plain glyphs");
                    wrapped[0]
                        .paint(
                            bounds.origin + point(px(0.), px(40.)),
                            px(28.),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .expect("the window must paint its own wrapped glyphs");
                    if let Some(retained) = &retained {
                        retained
                            .paint(
                                bounds.origin + point(px(0.), px(500.)),
                                px(28.),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            )
                            .expect("old glyphs must remain paintable in their original namespace");
                    }
                    layouts.borrow_mut().push(ObservedLayouts {
                        plain,
                        wrapped: Arc::clone(&wrapped[0]),
                        wrapped_text: wrapped[0].text.clone(),
                        width: bounds.size.width,
                        version,
                        previous_revision: window.rendered_frame_revision(),
                        requested_font,
                    });
                },
            )
            .w(self.width)
            .h(px(560.)),
        )
    }
}

fn open_consumer(
    app: &mut HeadlessAppContext,
    layouts: &Rc<RefCell<Vec<ObservedLayouts>>>,
    width: Pixels,
) -> Result<WindowHandle<WindowTextConsumer>> {
    let layouts = layouts.clone();
    app.open_window(size(px(800.), px(640.)), move |_, cx| {
        cx.new(|_| WindowTextConsumer {
            layouts,
            width,
            version: 0,
            retained: None,
        })
    })
}

fn draw_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<WindowTextConsumer>,
    layouts: &Rc<RefCell<Vec<ObservedLayouts>>>,
) -> Result<ObservedLayouts> {
    let before = layouts.borrow().len();
    app.update_window(handle.into(), |_, window, cx| {
        let previous = window.rendered_frame_revision();
        window.refresh();
        window.draw(cx).clear(cx);
        assert!(window.rendered_frame_revision() > previous);
    })?;
    observe_frame(app, handle, layouts, before)
}

fn observe_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<WindowTextConsumer>,
    layouts: &Rc<RefCell<Vec<ObservedLayouts>>>,
    before: usize,
) -> Result<ObservedLayouts> {
    let revision = app.update_window(handle.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision),
        );
        revision
    })?;
    let layouts = layouts.borrow();
    assert!(layouts.len() > before);
    let observed = layouts
        .last()
        .expect("an admitted frame must consume text")
        .clone();
    assert_eq!(observed.previous_revision.checked_add(1), Some(revision));
    assert_eq!(observed.plain.text.as_ref(), SOURCE);
    assert_eq!(observed.wrapped_text.as_ref(), SOURCE);
    assert_eq!(observed.wrapped.wrap_width, Some(observed.width));
    assert!(!observed.wrapped.wrap_boundaries.is_empty());
    assert!(Arc::ptr_eq(
        &*observed.plain,
        &observed.wrapped.unwrapped_layout,
    ));
    Ok(observed)
}

fn assert_reused(before: &ObservedLayouts, after: &ObservedLayouts) {
    assert!(Arc::ptr_eq(&*before.plain, &*after.plain));
    assert!(Arc::ptr_eq(&before.wrapped, &after.wrapped));
    assert_eq!(before.plain.text, after.plain.text);
    assert_eq!(before.wrapped_text, after.wrapped_text);
    assert_eq!(before.width, after.width);
}

fn assert_separate(left: &ObservedLayouts, right: &ObservedLayouts) {
    assert!(!Arc::ptr_eq(&*left.plain, &*right.plain));
    assert!(!Arc::ptr_eq(&left.wrapped, &right.wrapped));
    assert!(!Arc::ptr_eq(
        &left.wrapped.unwrapped_layout,
        &right.wrapped.unwrapped_layout,
    ));
}

fn glyph_metadata(line: &LineLayout) -> Vec<(FontId, GlyphId, Point<Pixels>, usize, bool)> {
    assert_eq!(line.len, SOURCE.len());
    assert!(line.width > px(0.) && line.width.as_f32().is_finite());
    let glyphs = line
        .runs
        .iter()
        .flat_map(|run| {
            run.glyphs.iter().map(move |glyph| {
                assert_ne!(glyph.id, GlyphId(0));
                assert!(glyph.index < SOURCE.len() && SOURCE.is_char_boundary(glyph.index));
                assert!(glyph.position.x.as_f32().is_finite());
                assert!(glyph.position.y.as_f32().is_finite());
                (
                    run.font_id,
                    glyph.id,
                    glyph.position,
                    glyph.index,
                    glyph.is_emoji,
                )
            })
        })
        .collect::<Vec<_>>();
    assert!(!glyphs.is_empty());
    glyphs
}

#[test]
fn public_windows_own_local_caches_and_release_the_exact_shared_text_system() -> Result<()> {
    let backend = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    let backend_lifetime = Arc::downgrade(&backend);
    let mut app = HeadlessAppContext::new(backend.clone());
    app.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    });
    let global = app.text_system().clone();
    let global_lifetime = Arc::downgrade(&global);
    global.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    assert!(backend.font_id(&font("Lilex")).is_err());
    let background_owner = global.clone();
    let prewarm = background_owner.clone();
    let original_font = std::thread::spawn(move || {
        prewarm.prewarm_fonts(&[font("Lilex"), font("IBM Plex Sans")]);
        prewarm.resolve_font(&font("Lilex"))
    })
    .join()
    .expect("the global Arc<TextSystem> must support background prewarming");
    let first_layouts = Rc::new(RefCell::new(Vec::new()));
    let second_layouts = Rc::new(RefCell::new(Vec::new()));
    let first = open_consumer(&mut app, &first_layouts, px(240.))?;
    let second = open_consumer(&mut app, &second_layouts, px(240.))?;
    assert_ne!(first.window_id(), second.window_id());
    let first_text = app.update_window(first.into(), |_, window, cx| {
        let owner: &Rc<WindowTextSystem> = window.text_system();
        let shared: &Arc<TextSystem> = owner.as_ref();
        assert!(Arc::ptr_eq(shared, &global));
        assert!(Arc::ptr_eq(shared, cx.text_system()));
        owner.clone()
    })?;
    let second_text = app.update_window(second.into(), |_, window, cx| {
        let owner: &Rc<WindowTextSystem> = window.text_system();
        let shared: &Arc<TextSystem> = owner.as_ref();
        assert!(Arc::ptr_eq(shared, &global));
        assert!(Arc::ptr_eq(shared, cx.text_system()));
        owner.clone()
    })?;
    assert!(!Rc::ptr_eq(&first_text, &second_text));
    let first_lifetime = Rc::downgrade(&first_text);
    let second_lifetime = Rc::downgrade(&second_text);
    let original = draw_frame(&mut app, first, &first_layouts)?;
    let other = draw_frame(&mut app, second, &second_layouts)?;
    assert_eq!(original.requested_font, original_font);
    assert_eq!(other.requested_font, original_font);
    assert_separate(&original, &other);
    let original_metadata = glyph_metadata(&original.plain);
    let original_wrapped_metadata = glyph_metadata(&original.wrapped.unwrapped_layout);
    let original_m = original_metadata.iter().find(|glyph| glyph.3 == 0).unwrap();
    assert_eq!(original_m.0, original_font);
    let original_advance = backend.advance(original_m.0, original_m.1)?;
    let original_bounds = backend.typographic_bounds(original_m.0, original_m.1)?;
    for _ in 0..3 {
        assert_reused(&original, &draw_frame(&mut app, first, &first_layouts)?);
        assert_reused(&other, &draw_frame(&mut app, second, &second_layouts)?);
    }

    let root = first.root(&mut app)?;
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let before_narrow = first_layouts.borrow().len();
    first.update(&mut app, |view, _, cx| {
        view.width = px(120.);
        view.version = 1;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 1);
    let narrower = observe_frame(&mut app, first, &first_layouts, before_narrow)?;
    assert_eq!(narrower.version, 1);
    assert_eq!(narrower.width, px(120.));
    assert!(Arc::ptr_eq(&*original.plain, &*narrower.plain));
    assert!(!Arc::ptr_eq(&original.wrapped, &narrower.wrapped));
    assert!(narrower.wrapped.wrap_boundaries.len() > original.wrapped.wrap_boundaries.len());
    assert_reused(&narrower, &draw_frame(&mut app, first, &first_layouts)?);
    assert_reused(&other, &draw_frame(&mut app, second, &second_layouts)?);

    let installer = background_owner.clone();
    let installed_font = std::thread::spawn(move || {
        installer.add_fonts(vec![Cow::Borrowed(LILEX)]).unwrap();
        installer.prewarm_fonts(&[font("Lilex"), font("IBM Plex Sans")]);
        installer.resolve_font(&font("Lilex"))
    })
    .join()
    .expect("font installation must use the same shared background TextSystem");
    assert_ne!(installed_font, original_font);
    assert_eq!(backend.font_id(&font("Lilex"))?, installed_font);
    let before_first_install = first_layouts.borrow().len();
    let before_second_install = second_layouts.borrow().len();
    first.update(&mut app, |view, _, cx| {
        view.version = 2;
        view.retained = Some(original.plain.clone());
        cx.notify();
    })?;
    second.update(&mut app, |view, _, cx| {
        view.version = 1;
        view.retained = Some(other.plain.clone());
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 2);
    let installed = observe_frame(&mut app, first, &first_layouts, before_first_install)?;
    let other_installed = observe_frame(&mut app, second, &second_layouts, before_second_install)?;
    assert_eq!(installed.version, 2);
    assert_eq!(other_installed.version, 1);
    for (before, after) in [(&narrower, &installed), (&other, &other_installed)] {
        assert_eq!(after.requested_font, installed_font);
        assert!(!Arc::ptr_eq(&*before.plain, &*after.plain));
        assert!(!Arc::ptr_eq(&before.wrapped, &after.wrapped));
        let metadata = glyph_metadata(&after.plain);
        assert_eq!(
            metadata.iter().find(|glyph| glyph.3 == 0).unwrap().0,
            installed_font,
        );
    }
    assert_separate(&installed, &other_installed);
    assert_reused(&installed, &draw_frame(&mut app, first, &first_layouts)?);
    assert_reused(
        &other_installed,
        &draw_frame(&mut app, second, &second_layouts)?,
    );
    assert_eq!(glyph_metadata(&original.plain), original_metadata);
    assert_eq!(
        backend.advance(original_m.0, original_m.1)?,
        original_advance
    );
    assert_eq!(
        backend.typographic_bounds(original_m.0, original_m.1)?,
        original_bounds,
    );

    drop(subscription);
    drop(root);
    app.update_window(first.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert_eq!(app.update(|cx| cx.windows().len()), 1);
    assert!(first_lifetime.upgrade().is_some());
    assert_reused(
        &other_installed,
        &draw_frame(&mut app, second, &second_layouts)?,
    );
    let rebuilt_layouts = Rc::new(RefCell::new(Vec::new()));
    let rebuilt = open_consumer(&mut app, &rebuilt_layouts, px(120.))?;
    assert_ne!(rebuilt.window_id(), first.window_id());
    let rebuilt_text = app.update_window(rebuilt.into(), |_, window, cx| {
        let owner: &Rc<WindowTextSystem> = window.text_system();
        let shared: &Arc<TextSystem> = owner.as_ref();
        assert!(Arc::ptr_eq(shared, &global));
        assert!(Arc::ptr_eq(shared, cx.text_system()));
        owner.clone()
    })?;
    let rebuilt_lifetime = Rc::downgrade(&rebuilt_text);
    assert!(!Rc::ptr_eq(&first_text, &rebuilt_text));
    assert!(!Rc::ptr_eq(&second_text, &rebuilt_text));
    let reconstructed = draw_frame(&mut app, rebuilt, &rebuilt_layouts)?;
    assert_eq!(reconstructed.requested_font, installed_font);
    assert_eq!(
        glyph_metadata(&reconstructed.plain),
        glyph_metadata(&installed.plain),
    );
    assert_eq!(
        reconstructed.wrapped.wrap_boundaries,
        installed.wrapped.wrap_boundaries,
    );
    assert_separate(&installed, &reconstructed);
    assert_separate(&other_installed, &reconstructed);
    assert_reused(
        &reconstructed,
        &draw_frame(&mut app, rebuilt, &rebuilt_layouts)?,
    );
    app.update_window(second.into(), |_, window, cx| window.remove_window(cx))?;
    app.update_window(rebuilt.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    drop(second_text);
    drop(rebuilt_text);
    assert!(second_lifetime.upgrade().is_none());
    assert!(rebuilt_lifetime.upgrade().is_none());
    assert!(first_lifetime.upgrade().is_some());
    assert!(Arc::ptr_eq(app.text_system(), &global));

    drop(global);
    drop(app);
    drop(backend);
    assert!(global_lifetime.upgrade().is_some());
    assert!(backend_lifetime.upgrade().is_some());
    let shared: &Arc<TextSystem> = first_text.as_ref();
    assert!(Arc::ptr_eq(shared, &background_owner));
    drop(background_owner);
    assert!(first_lifetime.upgrade().is_some());
    assert!(global_lifetime.upgrade().is_some());
    assert!(backend_lifetime.upgrade().is_some());
    drop(first_text);
    assert!(first_lifetime.upgrade().is_none());
    assert!(global_lifetime.upgrade().is_none());
    assert!(backend_lifetime.upgrade().is_none());
    assert_eq!(glyph_metadata(&original.plain), original_metadata);
    assert_eq!(
        glyph_metadata(&original.wrapped.unwrapped_layout),
        original_wrapped_metadata,
    );
    assert_eq!(
        glyph_metadata(&reconstructed.plain),
        glyph_metadata(&installed.plain),
    );
    Ok(())
}
