use super::*;
use crate::{
    PlatformWindowPresentOutcome, RetainAllImageCache, VisualTestContext, WindowBounds,
    WindowHandle, WindowOptions, black, fill,
};
use futures::channel::oneshot;
use open_gpui_http_client::{FakeHttpClient, Response};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const ANIMATED_WEBP: &[u8] =
    include_bytes!("../../../../../../tests/fixtures/images/red-blue-100ms.webp");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StateSnapshot {
    frame_index: usize,
    last_frame_time: Option<Instant>,
    started_loading: Option<Instant>,
}

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    loading_paints: Cell<usize>,
    state: Cell<Option<StateSnapshot>>,
    loaded_image: RefCell<Option<Arc<RenderImage>>>,
}

struct ImageLeaf {
    source: ImageSource,
    stable_id: bool,
    cache: Option<Entity<RetainAllImageCache>>,
    observed: Rc<Observations>,
}

impl Render for ImageLeaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let loading = self.observed.clone();
        let mut image = img(self.source.clone())
            .size(px(32.))
            .with_loading(move || {
                let loading = loading.clone();
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        loading.loading_paints.set(loading.loading_paints.get() + 1);
                        window.paint_quad(fill(bounds, black()));
                    },
                )
                .size(px(32.))
                .into_any_element()
            });
        if let Some(cache) = &self.cache {
            image = image.image_cache(cache);
        }
        let image = if self.stable_id {
            image.id(TEST_IMG_ID).into_any_element()
        } else {
            image.into_any_element()
        };
        let stable_id = self.stable_id;
        let observed = self.observed.clone();
        let source = self.source.clone();
        let cache = self.cache.clone();
        div().size_full().child(image).child(
            canvas(
                |_, _, _| (),
                move |_, (), window, app| {
                    if stable_id {
                        window.with_global_id(TEST_IMG_ID.into(), |id, window| {
                            window.with_element_state::<ImgState, _>(id, |state, _| {
                                let state = state.expect("the sibling image initialized its state");
                                observed.state.set(Some(StateSnapshot {
                                    frame_index: state.frame_index,
                                    last_frame_time: state.last_frame_time,
                                    started_loading: state
                                        .started_loading
                                        .as_ref()
                                        .map(|(started, _)| *started),
                                }));
                                ((), state)
                            });
                        });
                    }
                    if observed
                        .state
                        .get()
                        .is_some_and(|state| state.started_loading.is_none())
                        && let (Some(cache), ImageSource::Resource(source)) = (&cache, &source)
                        && let Some(Ok(image)) =
                            cache.update(app, |cache, app| cache.load(source, window, app))
                    {
                        *observed.loaded_image.borrow_mut() = Some(image);
                    }
                },
            )
            .size(px(1.)),
        )
    }
}

struct ImageRoot {
    leaf: Entity<ImageLeaf>,
}

impl Render for ImageRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(black()).child(
            self.leaf
                .clone()
                .cached(StyleRefinement::default().size(px(40.))),
        )
    }
}

struct Fixture {
    handle: WindowHandle<ImageRoot>,
    visual: VisualTestContext,
    observed: Rc<Observations>,
}

impl Fixture {
    fn new(
        cx: &mut TestAppContext,
        source: ImageSource,
        stable_id: bool,
        cache: Option<Entity<RetainAllImageCache>>,
    ) -> Self {
        let observed = Rc::new(Observations::default());
        cx.defer_next_window_frame_requests();
        let handle = cx.update(|app| {
            app.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(96.), px(96.)),
                    ))),
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..Default::default()
                },
                |_, app| {
                    app.new(|cx| ImageRoot {
                        leaf: cx.new(|_| ImageLeaf {
                            source,
                            stable_id,
                            cache,
                            observed: observed.clone(),
                        }),
                    })
                },
            )
            .unwrap()
        });
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        let _activation = visual.update(|window, _| window.activate_window());
        visual.run_until_parked();
        visual.update(|window, app| {
            assert!(window.is_window_active());
            window.draw(app).clear(app);
        });
        Self {
            handle,
            visual,
            observed,
        }
    }

    fn state(&self) -> StateSnapshot {
        self.observed
            .state
            .get()
            .expect("the image has a stable ID")
    }

    fn assert_frame(&mut self, image: &RenderImage, frame_index: usize) {
        self.visual.update(|window, _| {
            assert!(window.is_window_active());
            let paints = window.rendered_frame_image_paint_diagnostics();
            assert_eq!(paints.len(), 1);
            assert_eq!(paints[0].image.image_id, image.id);
            assert_eq!(paints[0].image.frame_index, frame_index);
        });
        if let Some(state) = self.observed.state.get() {
            assert_eq!(state.frame_index, frame_index);
        }
    }

    fn advance_frame(&mut self, duration: Duration) {
        self.visual.executor().advance_clock(duration);
        assert!(self.visual.simulate_scheduled_frame());
    }

    fn park(&mut self) {
        for _ in 0..3 {
            if !self.visual.frame_scheduled() {
                break;
            }
            assert!(self.visual.simulate_scheduled_frame());
        }
        assert!(!self.visual.frame_scheduled());
        assert!(!self.visual.simulate_scheduled_frame());
        self.visual.update(|window, _| {
            assert!(window.next_frame_callbacks.borrow().is_empty());
            let facts = window.presentation_facts();
            let present = facts.latest_present_attempt.expect("a frame was presented");
            assert_eq!(present.outcome, PlatformWindowPresentOutcome::Submitted);
            assert_eq!(
                facts.present_submitted_generation,
                facts.frame_accepted_generation
            );
        });
    }
}

fn animated_image() -> Arc<RenderImage> {
    let image = Arc::new(RenderImage::new(decode_webp_image(ANIMATED_WEBP).unwrap()));
    assert_eq!(image.frame_count(), 2);
    for frame in 0..2 {
        assert_eq!(
            Duration::from(image.delay(frame)),
            Duration::from_millis(100)
        );
    }
    image
}

#[open_gpui::test]
fn reduced_motion_preserves_the_current_frame_and_restarts_the_executor_clock(
    cx: &mut TestAppContext,
) {
    let image = animated_image();
    assert!(!cx.read(App::reduce_motion));
    let mut fixture = Fixture::new(cx, ImageSource::Render(image.clone()), true, None);
    fixture.assert_frame(&image, 0);
    let initial_time = fixture.state().last_frame_time.unwrap();

    fixture.advance_frame(Duration::from_millis(99));
    fixture.assert_frame(&image, 0);
    assert_eq!(fixture.state().last_frame_time, Some(initial_time));
    fixture.advance_frame(Duration::from_millis(1));
    fixture.assert_frame(&image, 1);
    assert_eq!(
        fixture.state().last_frame_time,
        Some(initial_time + Duration::from_millis(100))
    );

    fixture.visual.update(|_, app| app.set_reduce_motion(true));
    fixture.assert_frame(&image, 1);
    assert_eq!(fixture.state().last_frame_time, None);
    fixture.park();
    let frozen_revision = fixture
        .visual
        .update(|window, _| window.rendered_frame_revision());
    let frozen_renders = fixture.observed.renders.get();
    fixture
        .visual
        .executor()
        .advance_clock(Duration::from_secs(10));
    fixture.visual.run_until_parked();
    assert_eq!(fixture.observed.renders.get(), frozen_renders);
    assert!(!fixture.visual.frame_scheduled());
    assert_eq!(
        fixture
            .visual
            .update(|window, _| window.rendered_frame_revision()),
        frozen_revision
    );
    fixture.assert_frame(&image, 1);

    fixture
        .handle
        .update(&mut fixture.visual, |_, _, cx| cx.notify())
        .unwrap();
    fixture.assert_frame(&image, 1);
    assert_eq!(fixture.observed.renders.get(), frozen_renders);
    assert_eq!(fixture.state().last_frame_time, None);
    fixture.park();

    let resumed_at = fixture.visual.executor().now();
    fixture.visual.update(|_, app| app.set_reduce_motion(false));
    fixture.assert_frame(&image, 1);
    assert!(fixture.observed.renders.get() > frozen_renders);
    assert_eq!(fixture.state().last_frame_time, Some(resumed_at));
    fixture.advance_frame(Duration::from_millis(99));
    fixture.assert_frame(&image, 1);
    assert_eq!(fixture.state().last_frame_time, Some(resumed_at));
    fixture.advance_frame(Duration::from_millis(1));
    fixture.assert_frame(&image, 0);
    assert_eq!(
        fixture.state().last_frame_time,
        Some(resumed_at + Duration::from_millis(100))
    );
}

#[open_gpui::test]
fn static_empty_and_idless_images_do_not_schedule_animation_for_either_motion_policy(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(crate::QuitMode::Explicit));
    for (frame_count, stable_id) in [(0, true), (1, true), (2, false)] {
        let image = if frame_count == 2 {
            animated_image()
        } else {
            test_image(frame_count)
        };
        let mut fixture = Fixture::new(cx, ImageSource::Render(image.clone()), stable_id, None);
        for reduce_motion in [false, true, false] {
            fixture
                .visual
                .update(|_, app| app.set_reduce_motion(reduce_motion));
            fixture.park();
            if frame_count == 0 {
                fixture.visual.update(|window, _| {
                    assert!(window.rendered_frame_image_paint_diagnostics().is_empty());
                });
            } else {
                fixture.assert_frame(&image, 0);
            }
            if stable_id {
                assert_eq!(fixture.state().frame_index, 0);
                assert_eq!(fixture.state().last_frame_time, None);
            } else {
                assert!(fixture.observed.state.get().is_none());
            }
            let renders = fixture.observed.renders.get();
            fixture
                .visual
                .executor()
                .advance_clock(Duration::from_secs(1));
            fixture.visual.run_until_parked();
            assert_eq!(fixture.observed.renders.get(), renders);
            assert!(!fixture.visual.frame_scheduled());
        }
        fixture
            .visual
            .update(|window, app| window.remove_window(app));
        fixture.visual.run_until_parked();
    }
}

#[open_gpui::test]
fn reduced_motion_preserves_loading_deadline_cache_notifications_and_image_release(
    cx: &mut TestAppContext,
) {
    let (requests, pending) = async_channel::unbounded();
    cx.update(|app| {
        app.set_reduce_motion(true);
        app.set_http_client(FakeHttpClient::create(move |_| {
            let (complete, receive) = oneshot::channel();
            requests.try_send(complete).unwrap();
            async move {
                receive.await?;
                Ok(Response::builder()
                    .status(200)
                    .body(ANIMATED_WEBP.into())
                    .unwrap())
            }
        }));
    });
    let cache = cx.update(RetainAllImageCache::new);
    let source = Resource::Uri("https://test.example/reduced-motion.webp".into());
    let mut fixture = Fixture::new(
        cx,
        ImageSource::Resource(source.clone()),
        true,
        Some(cache.clone()),
    );
    fixture.visual.run_until_parked();
    let complete = pending.try_recv().unwrap();
    assert!(pending.try_recv().is_err());
    let started_loading = fixture.state().started_loading.unwrap();
    assert_eq!(started_loading, fixture.visual.executor().now());
    fixture.park();
    assert_eq!(fixture.observed.loading_paints.get(), 0);
    fixture
        .visual
        .executor()
        .advance_clock(LOADING_DELAY - Duration::from_millis(1));
    fixture.visual.run_until_parked();
    assert_eq!(fixture.observed.loading_paints.get(), 0);
    assert!(!fixture.visual.frame_scheduled());
    assert_eq!(fixture.state().started_loading, Some(started_loading));

    fixture
        .visual
        .executor()
        .advance_clock(Duration::from_millis(1));
    fixture.visual.run_until_parked();
    assert_eq!(fixture.observed.loading_paints.get(), 1);
    assert_eq!(fixture.state().started_loading, Some(started_loading));
    fixture.visual.update(|window, _| {
        assert!(window.next_frame_callbacks.borrow().is_empty());
        assert!(window.rendered_frame_image_paint_diagnostics().is_empty());
    });
    fixture.park();
    let renders_before_completion = fixture.observed.renders.get();
    assert!(fixture.observed.loaded_image.borrow().is_none());
    complete.send(()).unwrap();
    fixture.visual.run_until_parked();
    assert!(fixture.observed.renders.get() > renders_before_completion);
    assert_eq!(fixture.state().started_loading, None);
    fixture.visual.update(|window, _| {
        assert_eq!(window.rendered_frame_image_paint_diagnostics().len(), 1);
    });
    let image = fixture
        .observed
        .loaded_image
        .borrow()
        .clone()
        .expect("the rendered view must observe its loaded cache entry");
    assert_eq!(cache.read_with(cx, |cache, _| cache.len()), 1);
    assert!(pending.try_recv().is_err());
    assert_eq!(image.frame_count(), 2);
    assert_eq!(Duration::from(image.delay(0)), Duration::from_millis(100));
    assert_eq!(fixture.state().started_loading, None);
    assert_eq!(fixture.state().last_frame_time, None);
    fixture.assert_frame(&image, 0);
    fixture.park();

    fixture.visual.update(|_, app| app.set_reduce_motion(false));
    fixture.advance_frame(Duration::from_millis(99));
    fixture.assert_frame(&image, 0);
    fixture.advance_frame(Duration::from_millis(1));
    fixture.assert_frame(&image, 1);
    fixture.visual.update(|_, app| app.set_reduce_motion(true));
    fixture.assert_frame(&image, 1);
    fixture.park();
    let weak_image = Arc::downgrade(&image);
    let weak_cache = cache.downgrade();
    fixture
        .visual
        .update(|window, app| window.remove_window(app));
    drop(fixture);
    drop(image);
    drop(cache);
    cx.update(|_| {});
    cx.run_until_parked();
    assert!(weak_cache.upgrade().is_none());
    assert!(weak_image.upgrade().is_none());
}
