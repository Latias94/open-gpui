//! Public image consumers through real headless windows and their paint diagnostics.
//!
//! Simulated scheduler time drives `Img` frame selection. Cache assertions cover decoded
//! asset identity and atlas lookups during fresh root draws. Headless frame acceptance
//! does not qualify cached-view replay, native submission or compositor pixels.

#![cfg(not(target_family = "wasm"))]

use open_gpui::{
    AppContext as _, AtlasAccessOutcome, AtlasRemoveOutcome, Context, HeadlessAppContext, Image,
    ImageFormat, ImagePaintDiagnostic, ImageSource, ImgResourceLoader, InteractiveElement as _,
    IntoElement, NoopTextSystem, ParentElement as _, QuitMode, Render, RenderImage, Resource,
    Result, Styled as _, StyledImage as _, Window, WindowActivationStatus,
    WindowActivationTerminal, WindowHandle, canvas, div, img, px, size,
};
use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

const ANIMATED_WEBP: &[u8] = include_bytes!("../../../tests/fixtures/images/red-blue-100ms.webp");
const EXIF_JPEG: &[u8] =
    include_bytes!("../../../tests/fixtures/images/exif-orientation-rotate-180.jpg");

struct ImageConsumer {
    image: Arc<Image>,
    path: PathBuf,
}

impl Render for ImageConsumer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(img(self.image.clone()).id("memory-image").size(px(32.)))
            .child(img(self.path.clone()).id("path-image").size(px(32.)))
    }
}

fn painted_images(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<ImageConsumer>,
) -> Result<(u64, Vec<ImagePaintDiagnostic>)> {
    app.update_window(handle.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision),
        );
        let paints = window.rendered_frame_image_paint_diagnostics().to_vec();
        assert!(
            paints
                .iter()
                .all(|paint| paint.frame_generation == revision)
        );
        let accesses = paints
            .iter()
            .map(|paint| paint.atlas_access)
            .collect::<Vec<_>>();
        assert_eq!(
            window.rendered_frame_atlas_access_diagnostics(),
            accesses.as_slice(),
        );
        (revision, paints)
    })
}

fn redraw(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<ImageConsumer>,
) -> Result<(u64, Vec<ImagePaintDiagnostic>)> {
    let previous = app.update_window(handle.into(), |_, window, cx| {
        let previous = window.rendered_frame_revision();
        window.refresh();
        window.draw(cx).clear(cx);
        previous
    })?;
    let frame = painted_images(app, handle)?;
    assert!(frame.0 > previous);
    Ok(frame)
}

fn cached_images(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<ImageConsumer>,
    image: &Arc<Image>,
    resource: &Resource,
) -> Result<[Arc<RenderImage>; 2]> {
    app.update_window(handle.into(), |_, window, cx| {
        [
            image
                .clone()
                .get_render_image(window, cx)
                .expect("the encoded image must have completed its public asset load"),
            window
                .get_asset::<ImgResourceLoader>(resource, cx)
                .expect("the path resource must have completed its public asset load")
                .expect("the fixture resource must decode successfully"),
        ]
    })
}

fn assert_decoded_fixture(format: ImageFormat, decoded: &RenderImage) {
    let (frame_count, dimensions) = match format {
        ImageFormat::Webp => (2, size(1.into(), 1.into())),
        ImageFormat::Jpeg => (1, size(16.into(), 32.into())),
        _ => unreachable!("only the two tracked image fixtures are consumed"),
    };
    assert_eq!(decoded.frame_count(), frame_count);
    for frame in 0..frame_count {
        assert_eq!(decoded.size(frame), dimensions);
        let pixels = decoded
            .as_bytes(frame)
            .expect("decoded frames must have pixels");
        if format == ImageFormat::Webp {
            assert_eq!(
                Duration::from(decoded.delay(frame)),
                Duration::from_millis(100),
            );
            assert_eq!(pixels.len(), 4);
            assert_eq!(pixels[3], 255);
            let dominant_channel = if frame == 0 { 2 } else { 0 };
            let other_channel = if frame == 0 { 0 } else { 2 };
            assert!(pixels[dominant_channel] > 240);
            assert!(pixels[other_channel] < 10 && pixels[1] < 10);
        } else {
            assert_eq!(pixels.len(), 16 * 32 * 4);
            assert_eq!(&pixels[..4], &[255, 255, 255, 255]);
            assert_eq!(&pixels[pixels.len() - 4..], &[0, 0, 0, 255]);
        }
    }
}

fn assert_img_paints(
    paints: &[ImagePaintDiagnostic],
    images: &[Arc<RenderImage>; 2],
    frame: usize,
) {
    assert_eq!(paints.len(), 2);
    for (paint, image) in paints.iter().zip(images) {
        assert_eq!(paint.image.image_id, image.id);
        assert_eq!(paint.image.frame_index, frame);
        assert_eq!(paint.tile.bounds.size, image.size(frame));
        assert_eq!(paint.atlas_access.image, Some(paint.image));
        assert_eq!(paint.atlas_access.tile, Some(paint.tile));
        assert!(matches!(
            paint.atlas_access.outcome,
            AtlasAccessOutcome::Inserted | AtlasAccessOutcome::Hit,
        ));
    }
}

fn exercise_img_timeline(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<ImageConsumer>,
    images: &[Arc<RenderImage>; 2],
    notifications: &Cell<usize>,
    animated: bool,
) -> Result<()> {
    let activation = app.update_window(handle.into(), |_, window, _| window.activate_window())?;
    app.run_until_parked();
    assert_eq!(activation.snapshot().target(), handle.window_id());
    assert_eq!(
        activation.snapshot().status(),
        WindowActivationStatus::Terminal(WindowActivationTerminal::Activated),
    );
    assert!(app.update_window(handle.into(), |_, window, _| window.is_window_active())?);
    let (mut revision, first) = redraw(app, handle)?;
    assert_img_paints(&first, images, 0);
    let mut seen = [Some(first), None];
    let mut notified = notifications.get();

    for (elapsed_ms, animated_frame) in [(99, 0), (1, 1), (99, 1), (1, 0), (100, 1), (100, 0)] {
        let started = app.background_executor().now();
        let elapsed = Duration::from_millis(elapsed_ms);
        app.advance_clock(elapsed);
        app.run_until_parked();
        assert_eq!(app.background_executor().now() - started, elapsed);
        let (next_revision, paints) = if animated {
            let callbacks = app.update_window(handle.into(), |_, window, cx| {
                window.drain_next_frame_callbacks_for_test(cx)
            })?;
            assert!(
                callbacks > 0,
                "the real img must request its next animation frame",
            );
            app.run_until_parked();
            assert!(notifications.get() > notified);
            painted_images(app, handle)?
        } else {
            let callbacks = app.update_window(handle.into(), |_, window, cx| {
                window.drain_next_frame_callbacks_for_test(cx)
            })?;
            assert_eq!(
                callbacks, 0,
                "a static image must not schedule animation callbacks"
            );
            app.run_until_parked();
            assert_eq!(notifications.get(), notified);
            assert_eq!(painted_images(app, handle)?.0, revision);
            redraw(app, handle)?
        };
        assert!(next_revision > revision);
        notified = notifications.get();
        let frame = if animated { animated_frame } else { 0 };
        assert_img_paints(&paints, images, frame);
        if let Some(previous) = &seen[frame] {
            for (paint, previous) in paints.iter().zip(previous) {
                assert_eq!(paint.atlas_access.outcome, AtlasAccessOutcome::Hit);
                assert_eq!(paint.tile, previous.tile);
            }
        } else {
            let initial = seen[0]
                .as_ref()
                .expect("frame zero must already have painted");
            for (paint, first) in paints.iter().zip(initial) {
                assert_eq!(paint.atlas_access.outcome, AtlasAccessOutcome::Inserted);
                assert_ne!(paint.tile, first.tile);
            }
            seen[frame] = Some(paints.clone());
        }
        let (repeated_revision, repeated) = redraw(app, handle)?;
        assert_img_paints(&repeated, images, frame);
        for (paint, previous) in repeated.iter().zip(&paints) {
            assert_eq!(paint.atlas_access.outcome, AtlasAccessOutcome::Hit);
            assert_eq!(paint.tile, previous.tile);
        }
        revision = repeated_revision;
    }
    Ok(())
}

fn exercise_img_loaders(format: ImageFormat, bytes: &[u8], filename: &str) -> Result<()> {
    let mut app = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
    app.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    let image = Arc::new(Image::from_bytes(format, bytes.to_vec()));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/images")
        .join(filename);
    let resource = Resource::from(path.clone());
    let mut previous_ids = None;
    let mut previous_window = None;

    for _ in 0..2 {
        let handle = app.open_window(size(px(96.), px(64.)), {
            let image = image.clone();
            let path = path.clone();
            move |_, cx| cx.new(|_| ImageConsumer { image, path })
        })?;
        if let Some(previous_window) = previous_window {
            assert_ne!(handle.window_id(), previous_window);
        }
        let root = handle.root(&mut app)?;
        let root_lifetime = root.downgrade();
        let notifications = Rc::new(Cell::new(0));
        let subscription = app.update(|cx| {
            let notifications = notifications.clone();
            cx.observe(&root, move |_, _| {
                notifications.set(notifications.get() + 1)
            })
        });
        let (pending_revision, pending) = redraw(&mut app, handle)?;
        assert!(pending.is_empty());
        assert_eq!(notifications.get(), 0);
        app.update_window(handle.into(), |_, window, cx| {
            assert!(!window.is_window_active());
            assert!(image.clone().get_render_image(window, cx).is_none());
            assert!(
                window
                    .get_asset::<ImgResourceLoader>(&resource, cx)
                    .is_none()
            );
        })?;

        let loading_started = app.background_executor().now();
        app.run_until_parked();
        app.update_window(handle.into(), |_, window, cx| {
            window.drain_next_frame_callbacks_for_test(cx);
        })?;
        app.run_until_parked();
        assert_eq!(app.background_executor().now(), loading_started);
        assert!(
            notifications.get() > 0,
            "asset completion must notify the real root",
        );
        let (loaded_revision, first_paints) = painted_images(&mut app, handle)?;
        assert!(loaded_revision > pending_revision);
        let decoded = cached_images(&mut app, handle, &image, &resource)?;
        assert_decoded_fixture(format, &decoded[0]);
        assert_decoded_fixture(format, &decoded[1]);
        assert_ne!(decoded[0].id, decoded[1].id);
        for frame in 0..decoded[0].frame_count() {
            assert_eq!(decoded[0].as_bytes(frame), decoded[1].as_bytes(frame));
            assert_eq!(decoded[0].delay(frame), decoded[1].delay(frame));
        }
        assert_img_paints(&first_paints, &decoded, 0);
        let ids = [decoded[0].id, decoded[1].id];
        if let Some(previous_ids) = previous_ids {
            for (current, previous) in ids.into_iter().zip(previous_ids) {
                assert_ne!(current, previous, "eviction must retire the decoded asset");
            }
        }
        let after_loading = notifications.get();
        for _ in 0..3 {
            let (_, paints) = redraw(&mut app, handle)?;
            assert_img_paints(&paints, &decoded, 0);
            let repeated = cached_images(&mut app, handle, &image, &resource)?;
            for ((paint, first), (cached, original)) in paints
                .iter()
                .zip(&first_paints)
                .zip(repeated.iter().zip(&decoded))
            {
                assert_eq!(paint.atlas_access.outcome, AtlasAccessOutcome::Hit);
                assert_eq!(paint.tile, first.tile);
                assert!(Arc::ptr_eq(cached, original));
            }
        }
        assert_eq!(notifications.get(), after_loading);

        let removals = app.update_window(handle.into(), |_, window, _| -> Result<_> {
            let before = window.atlas_remove_diagnostics().len();
            for decoded in &decoded {
                window.drop_image(decoded.clone())?;
            }
            Ok(window.atlas_remove_diagnostics()[before..].to_vec())
        })??;
        assert_eq!(
            removals.len(),
            decoded
                .iter()
                .map(|image| image.frame_count())
                .sum::<usize>(),
        );
        let mut removals = removals.iter();
        for decoded in &decoded {
            for frame in 0..decoded.frame_count() {
                let removal = removals
                    .next()
                    .expect("every decoded frame must be removed");
                let params = removal
                    .image
                    .expect("image removal must identify its frame");
                assert_eq!(params.image_id, decoded.id);
                assert_eq!(params.frame_index, frame);
                assert_eq!(
                    removal.outcome,
                    if frame == 0 {
                        AtlasRemoveOutcome::TextureRetained
                    } else {
                        AtlasRemoveOutcome::RemoveNoop
                    },
                );
            }
        }
        let (_, reinserted) = redraw(&mut app, handle)?;
        assert_img_paints(&reinserted, &decoded, 0);
        for (paint, first) in reinserted.iter().zip(&first_paints) {
            assert_eq!(paint.atlas_access.outcome, AtlasAccessOutcome::Inserted);
            assert_ne!(paint.tile, first.tile);
        }
        exercise_img_timeline(
            &mut app,
            handle,
            &decoded,
            &notifications,
            format == ImageFormat::Webp,
        )?;

        drop(subscription);
        drop(root);
        app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
        app.run_until_parked();
        assert!(app.update(|cx| cx.windows().is_empty()));
        assert!(root_lifetime.upgrade().is_none());
        assert!(app.update_window(handle.into(), |_, _, _| ()).is_err());
        app.update(|cx| {
            image.clone().remove_asset(cx);
            ImageSource::from(path.clone()).remove_asset(cx);
        });
        let decoded_lifetimes = decoded.each_ref().map(Arc::downgrade);
        drop(decoded);
        app.run_until_parked();
        assert!(
            decoded_lifetimes
                .iter()
                .all(|image| image.upgrade().is_none())
        );
        previous_ids = Some(ids);
        previous_window = Some(handle.window_id());
    }
    Ok(())
}

#[test]
fn public_animated_img_loaders_notify_advance_loop_reuse_and_reconstruct() -> Result<()> {
    exercise_img_loaders(ImageFormat::Webp, ANIMATED_WEBP, "red-blue-100ms.webp")
}

#[test]
fn public_static_img_loaders_notify_paint_reuse_and_reconstruct() -> Result<()> {
    exercise_img_loaders(
        ImageFormat::Jpeg,
        EXIF_JPEG,
        "exif-orientation-rotate-180.jpg",
    )
}

struct IsolatedImageConsumer {
    source: ImageSource,
}

impl Render for IsolatedImageConsumer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        img(self.source.clone()).id("isolated-image").size(px(32.))
    }
}

#[test]
fn public_image_loaders_each_notify_without_another_image_or_loading_deadline() -> Result<()> {
    for source in [
        ImageSource::Image(Arc::new(Image::from_bytes(
            ImageFormat::Webp,
            ANIMATED_WEBP.to_vec(),
        ))),
        ImageSource::Resource(Resource::from(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/images/red-blue-100ms.webp"),
        )),
    ] {
        let mut app = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
        app.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
        let handle = app.open_window(size(px(96.), px(64.)), {
            let source = source.clone();
            move |_, cx| cx.new(|_| IsolatedImageConsumer { source })
        })?;
        let root = handle.root(&mut app)?;
        let notifications = Rc::new(Cell::new(0));
        let subscription = app.update(|cx| {
            let notifications = notifications.clone();
            cx.observe(&root, move |_, _| {
                notifications.set(notifications.get() + 1)
            })
        });
        let pending = app.update_window(handle.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            assert!(window.rendered_frame_image_paint_diagnostics().is_empty());
            assert!(!window.is_window_active());
            window.rendered_frame_revision()
        })?;
        let started = app.background_executor().now();
        app.run_until_parked();
        app.update_window(handle.into(), |_, window, cx| {
            window.drain_next_frame_callbacks_for_test(cx);
        })?;
        app.run_until_parked();
        assert_eq!(app.background_executor().now(), started);
        assert!(
            notifications.get() > 0,
            "each isolated loader must notify its own root"
        );
        app.update_window(handle.into(), |_, window, cx| {
            let decoded = match &source {
                ImageSource::Image(image) => image.clone().get_render_image(window, cx).unwrap(),
                ImageSource::Resource(resource) => window
                    .get_asset::<ImgResourceLoader>(resource, cx)
                    .unwrap()
                    .unwrap(),
                _ => unreachable!(),
            };
            let revision = window.rendered_frame_revision();
            assert!(revision > pending);
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                Some(revision)
            );
            let paints = window.rendered_frame_image_paint_diagnostics();
            assert_eq!(paints.len(), 1);
            assert_eq!(paints[0].frame_generation, revision);
            assert_eq!(paints[0].image.image_id, decoded.id);
            assert_eq!(paints[0].image.frame_index, 0);
            assert_decoded_fixture(ImageFormat::Webp, &decoded);
        })?;
        drop(subscription);
        drop(root);
        app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
        app.run_until_parked();
        assert!(app.update(|cx| cx.windows().is_empty()));
        app.update(|cx| source.remove_asset(cx));
    }
    Ok(())
}

struct PendingImageConsumer {
    loading_paints: Rc<Cell<usize>>,
}

impl Render for PendingImageConsumer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paints = self.loading_paints.clone();
        img(ImageSource::Custom(Arc::new(|_, _| None)))
            .id("pending-image")
            .with_loading(move || {
                let paints = paints.clone();
                canvas(
                    |_, _, _| {},
                    move |_, (), _, _| paints.set(paints.get() + 1),
                )
                .size(px(16.))
                .into_any_element()
            })
    }
}

#[test]
fn public_loading_placeholder_paints_at_the_exact_timer_deadline() -> Result<()> {
    let mut app = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
    app.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    let loading_paints = Rc::new(Cell::new(0));
    let handle = app.open_window(size(px(96.), px(64.)), {
        let loading_paints = loading_paints.clone();
        move |_, cx| cx.new(|_| PendingImageConsumer { loading_paints })
    })?;
    let root = handle.root(&mut app)?;
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    app.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    app.run_until_parked();
    assert_eq!(loading_paints.get(), 0);
    assert_eq!(notifications.get(), 0);
    app.advance_clock(Duration::from_millis(199));
    app.run_until_parked();
    assert_eq!(loading_paints.get(), 0);
    assert_eq!(notifications.get(), 0);
    app.advance_clock(Duration::from_millis(1));
    app.run_until_parked();
    assert_eq!(notifications.get(), 1);
    assert!(
        loading_paints.get() > 0,
        "the timer notification must paint its loading state"
    );
    drop(subscription);
    drop(root);
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    Ok(())
}
