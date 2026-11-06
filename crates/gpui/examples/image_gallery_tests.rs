use super::*;
use futures::channel::oneshot;
use open_gpui::{
    AtlasRemoveOutcome, Empty, ImageSource, Resource, TestAppContext,
    http_client::{FakeHttpClient, Response},
};

const IMAGE_BYTES: &[u8] = include_bytes!("../../../tests/fixtures/images/red-blue-100ms.webp");

fn pending_images(cx: &TestAppContext) -> async_channel::Receiver<oneshot::Sender<()>> {
    let (requests, pending) = async_channel::unbounded();
    cx.update(|cx| {
        cx.set_http_client(FakeHttpClient::create(move |_| {
            let (complete, receive) = oneshot::channel();
            requests.try_send(complete).unwrap();
            async move {
                receive.await?;
                Ok(Response::builder()
                    .status(200)
                    .body(IMAGE_BYTES.into())
                    .unwrap())
            }
        }));
    });
    pending
}

struct CachedImageView {
    cache: Option<Entity<SimpleLruCache>>,
    sources: Vec<Resource>,
}

impl Render for CachedImageView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if let Some(cache) = &self.cache {
            div()
                .children(self.sources.iter().cloned().map(|source| {
                    img(ImageSource::Resource(source))
                        .image_cache(cache)
                        .size_8()
                }))
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }
}

#[open_gpui::test]
fn lru_pending_eviction_cancels_loading_and_isolates_replacement(cx: &mut TestAppContext) {
    for capacity in [1, 2] {
        let requests = pending_images(cx);
        let source = Resource::Uri("https://test.example/original.webp".into());
        let evictor = Resource::Uri("https://test.example/evictor.webp".into());
        let cache = cx.update(|cx| cx.new(|cx| SimpleLruCache::new(capacity, cx)));
        let window = cx.add_window(|_, _| CachedImageView {
            cache: Some(cache.clone()),
            sources: vec![source.clone()],
        });
        cx.run_until_parked();
        let original = requests.try_recv().unwrap();
        assert!(!original.is_canceled());
        assert!(requests.try_recv().is_err());

        let retained = if capacity == 2 {
            window
                .update(cx, |view, _, cx| {
                    view.sources
                        .push(Resource::Uri("https://test.example/retained.webp".into()));
                    cx.notify();
                })
                .unwrap();
            cx.run_until_parked();
            Some(requests.try_recv().unwrap())
        } else {
            None
        };
        assert!(requests.try_recv().is_err());
        window
            .update(cx, |view, _, cx| {
                view.sources.retain(|resource| resource != &source);
                view.sources.push(evictor.clone());
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        let evicting_load = requests.try_recv().unwrap();
        assert!(original.is_canceled(), "capacity {capacity}");
        assert!(!evicting_load.is_canceled());
        if let Some(retained) = &retained {
            assert!(!retained.is_canceled());
        }
        cache.read_with(cx, |cache, _| {
            assert_eq!(cache.cache.len(), capacity);
            assert!(!cache.cache.contains_key(&hash(&source)));
        });

        window
            .update(cx, |view, _, cx| {
                view.sources.pop().unwrap();
                view.sources.push(source.clone());
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        let replacement = requests.try_recv().unwrap();
        assert!(evicting_load.is_canceled());
        assert!(!replacement.is_canceled());
        assert!(original.send(()).is_err());
        cx.run_until_parked();
        cache.read_with(cx, |cache, _| {
            assert_eq!(cache.cache.len(), capacity);
            assert!(!cache.cache.contains_key(&hash(&evictor)));
            assert!(cache.cache[&hash(&source)].get().is_none());
        });

        replacement.send(()).unwrap();
        cx.run_until_parked();
        cache.read_with(cx, |cache, _| {
            let entry = &cache.cache[&hash(&source)];
            let image = entry.get().expect("replacement image published").unwrap();
            assert_eq!(image.frame_count(), 2);
            assert!(Arc::ptr_eq(&image, &entry.get().unwrap().unwrap()));
        });
        assert!(requests.try_recv().is_err());

        window
            .update(cx, |view, _, cx| {
                view.cache = None;
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        let weak_cache = cache.downgrade();
        drop(cache);
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(weak_cache.upgrade().is_none());
        if let Some(retained) = retained {
            assert!(retained.is_canceled());
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Removal {
    Evict,
    Release,
}

#[open_gpui::test]
fn lru_disposes_animated_frames_in_all_windows_on_eviction_and_release(cx: &mut TestAppContext) {
    for removal in [Removal::Evict, Removal::Release] {
        let requests = pending_images(cx);
        let source = Resource::Uri("https://test.example/animated.webp".into());
        let cache = cx.update(|cx| cx.new(|cx| SimpleLruCache::new(1, cx)));
        let windows = std::array::from_fn::<_, 2, _>(|_| {
            cx.add_window(|_, _| CachedImageView {
                cache: Some(cache.clone()),
                sources: vec![source.clone()],
            })
        });
        cx.run_until_parked();
        requests.try_recv().unwrap().send(()).unwrap();
        assert!(requests.try_recv().is_err());
        cx.run_until_parked();
        let image = cache.read_with(cx, |cache, _| {
            cache.cache[&hash(&source)].get().unwrap().unwrap()
        });
        assert_eq!(image.frame_count(), 2);

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
        let weak_cache = cache.downgrade();
        let mut replacement = None;
        let cache = match removal {
            Removal::Evict => {
                windows[0]
                    .update(cx, |view, _, cx| {
                        view.cache = Some(cache.clone());
                        view.sources = vec![Resource::Uri(
                            "https://test.example/replacement.webp".into(),
                        )];
                        cx.notify();
                    })
                    .unwrap();
                cx.run_until_parked();
                replacement = Some(requests.try_recv().unwrap());
                cache.read_with(cx, |cache, _| {
                    assert_eq!(cache.cache.len(), 1);
                    assert!(!cache.cache.contains_key(&hash(&source)));
                });
                Some(cache)
            }
            Removal::Release => {
                drop(cache);
                cx.update(|_| {});
                None
            }
        };
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

        for window in windows {
            window
                .update(cx, |view, _, cx| {
                    view.cache = None;
                    cx.notify();
                })
                .unwrap();
        }
        cx.run_until_parked();
        drop(cache);
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(weak_cache.upgrade().is_none());
        if let Some(replacement) = replacement {
            assert!(replacement.is_canceled());
        }
        assert!(requests.try_recv().is_err());
    }
}
