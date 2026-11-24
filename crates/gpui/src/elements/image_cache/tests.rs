use super::*;
use crate::{AtlasRemoveOutcome, Context, Empty, ImageSource, Render, TestAppContext, img};
use futures::channel::oneshot;

const IMAGE_BYTES: &[u8] =
    include_bytes!("../../../../../tests/fixtures/images/red-blue-100ms.webp");

fn pending_image(cx: &App) -> (ImageCacheItem, oneshot::Sender<()>) {
    let (complete, receive) = oneshot::channel();
    let load = async move {
        receive.await.map_err(anyhow::Error::from)?;
        let frames = crate::decode_webp_image(IMAGE_BYTES)?;
        Ok(Arc::new(RenderImage::new(frames)))
    };
    (ImageCacheItem(CachedLoad::new(load, cx)), complete)
}

struct CachedImageView {
    cache: Option<Entity<RetainAllImageCache>>,
    source: Resource,
}

impl Render for CachedImageView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if let Some(cache) = &self.cache {
            img(ImageSource::Resource(self.source.clone()))
                .image_cache(cache)
                .size_8()
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Removal {
    Remove,
    Clear,
    Release,
}

#[open_gpui::test]
fn image_cache_item_owns_real_loader_result_and_pending_cancellation(cx: &mut TestAppContext) {
    let source = Resource::Path(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/images/red-blue-100ms.webp")
            .into(),
    );
    let item = cx.update(|cx| ImageCacheItem::new(&source, cx));
    assert!(item.get().is_none());

    cx.run_until_parked();

    let image = item.get().expect("published image").unwrap();
    assert_eq!(image.frame_count(), 2);
    let pixel = &image.as_bytes(0).unwrap()[..4];
    assert!(pixel[2] > 240 && pixel[0] < 10 && pixel[1] < 10);
    assert_eq!(pixel[3], 255);
    assert!(Arc::ptr_eq(&image, &item.get().unwrap().unwrap()));

    let (pending_item, complete) = cx.update(|cx| pending_image(cx));
    cx.run_until_parked();
    assert!(!complete.is_canceled());
    drop(pending_item);
    cx.run_until_parked();
    assert!(complete.is_canceled());
}

#[open_gpui::test]
fn image_cache_item_retains_real_loader_error(cx: &mut TestAppContext) {
    let source = Resource::Embedded("missing.webp".into());
    let item = cx.update(|cx| ImageCacheItem::new(&source, cx));
    cx.run_until_parked();

    for _ in 0..2 {
        assert!(matches!(item.get(), Some(Err(ImageCacheError::Asset(_)))));
    }
}

#[open_gpui::test]
fn retain_all_cancels_pending_images_on_remove_clear_and_release(cx: &mut TestAppContext) {
    for removal in [Removal::Remove, Removal::Clear, Removal::Release] {
        let source = Resource::Embedded("image.webp".into());
        let cache = cx.update(RetainAllImageCache::new);
        let complete = cache.update(cx, |cache, cx| {
            let (pending, complete) = pending_image(cx);
            cache.0.insert(hash(&source), pending);
            complete
        });
        let window = cx.add_window(|_, _| CachedImageView {
            cache: Some(cache.clone()),
            source: source.clone(),
        });
        cx.run_until_parked();
        assert!(!complete.is_canceled());
        assert_eq!(cache.read_with(cx, |cache, _| cache.len()), 1);

        window
            .update(cx, |view, _, cx| {
                view.cache = None;
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        match removal {
            Removal::Remove => {
                window
                    .update(cx, |_, window, cx| {
                        cache.update(cx, |cache, cx| cache.remove(&source, window, cx));
                    })
                    .unwrap();
            }
            Removal::Clear => {
                window
                    .update(cx, |_, window, cx| {
                        cache.update(cx, |cache, cx| cache.clear(window, cx));
                    })
                    .unwrap();
            }
            Removal::Release => {}
        }
        if !matches!(removal, Removal::Release) {
            assert!(cache.read_with(cx, |cache, _| cache.is_empty()));
            cx.run_until_parked();
            assert!(complete.is_canceled(), "{removal:?}");
        }
        let weak_cache = cache.downgrade();
        drop(cache);
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(weak_cache.upgrade().is_none());
        assert!(complete.is_canceled(), "{removal:?}");
    }
}

#[open_gpui::test]
fn retain_all_disposes_loaded_images_in_all_windows_on_remove_clear_and_release(
    cx: &mut TestAppContext,
) {
    for removal in [Removal::Remove, Removal::Clear, Removal::Release] {
        let source = Resource::Embedded("image.webp".into());
        let cache = cx.update(RetainAllImageCache::new);
        let complete = cache.update(cx, |cache, cx| {
            let (pending, complete) = pending_image(cx);
            cache.0.insert(hash(&source), pending);
            complete
        });
        let windows = std::array::from_fn::<_, 2, _>(|_| {
            cx.add_window(|_, _| CachedImageView {
                cache: Some(cache.clone()),
                source: source.clone(),
            })
        });
        cx.run_until_parked();
        complete.send(()).unwrap();
        cx.run_until_parked();
        let image = cache.read_with(cx, |cache, _| {
            cache.0[&hash(&source)].get().unwrap().unwrap()
        });

        for window in windows {
            window
                .update(cx, |view, window, cx| {
                    assert!(
                        window
                            .rendered_frame_image_paint_diagnostics()
                            .iter()
                            .any(|paint| paint.image.image_id == image.id)
                    );
                    view.cache = None;
                    cx.notify();
                })
                .unwrap();
        }
        cx.run_until_parked();
        let before = windows.map(|window| {
            window
                .update(cx, |_, window, _| window.atlas_remove_diagnostics().len())
                .unwrap()
        });

        match removal {
            Removal::Remove => {
                windows[0]
                    .update(cx, |_, window, cx| {
                        cache.update(cx, |cache, cx| cache.remove(&source, window, cx));
                    })
                    .unwrap();
            }
            Removal::Clear => {
                windows[0]
                    .update(cx, |_, window, cx| {
                        cache.update(cx, |cache, cx| cache.clear(window, cx));
                    })
                    .unwrap();
            }
            Removal::Release => {}
        }
        if matches!(removal, Removal::Release) {
            drop(cache);
            cx.update(|_| {});
        } else {
            assert!(cache.read_with(cx, |cache, _| cache.is_empty()));
        }
        cx.run_until_parked();

        for (window, start) in windows.into_iter().zip(before) {
            window
                .update(cx, |_, window, _| {
                    let removals = &window.atlas_remove_diagnostics()[start..];
                    for frame_index in 0..image.frame_count() {
                        assert!(
                            removals.iter().any(|remove| {
                                remove.image.is_some_and(|params| {
                                    params.image_id == image.id && params.frame_index == frame_index
                                })
                            }),
                            "{removal:?}: frame {frame_index} must be removed from every window"
                        );
                    }
                    assert!(
                        removals.iter().any(|remove| {
                            remove
                                .image
                                .is_some_and(|params| params.image_id == image.id)
                                && remove.outcome != AtlasRemoveOutcome::RemoveNoop
                        }),
                        "{removal:?}: the painted image must occupy an atlas entry"
                    );
                })
                .unwrap();
        }
    }
}
