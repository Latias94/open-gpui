use super::*;
use crate::{
    App, InputEvent as _, MouseButton, MouseDownEvent, MouseUpEvent, PlatformWindowPresentOutcome,
    RenderImage, StyleRefinement, SubtreePresentation, WindowHandle, img, point, rgb,
    window::cached_paint_atlas_tests::RejectOnceAtlas,
};
use std::{cell::Cell, sync::Arc};

#[derive(Default)]
struct PointerCounts {
    downs: Cell<usize>,
    ups: Cell<usize>,
    clicks: Cell<usize>,
}

struct VisibilityButton(Rc<PointerCounts>);

impl Render for VisibilityButton {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let downs = self.0.clone();
        let ups = self.0.clone();
        let clicks = self.0.clone();
        div()
            .id("refused-visibility-button")
            .w(px(120.0))
            .h(px(48.0))
            .flex_shrink_0()
            .bg(rgb(0x2468ac))
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                downs.downs.set(downs.downs.get() + 1);
            })
            .on_mouse_up(MouseButton::Left, move |_, _, _| {
                ups.ups.set(ups.ups.get() + 1);
            })
            .on_click(move |_, _, _| {
                clicks.clicks.set(clicks.clicks.get() + 1);
            })
    }
}

struct RefusedVisibilityRoot {
    tree: Entity<RandomizedElementTree>,
    button: Entity<VisibilityButton>,
    cached_button: bool,
    image: Arc<RenderImage>,
}

impl Render for RefusedVisibilityRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let presentation = if self.tree.read(cx).snapshot().root.visible {
            SubtreePresentation::Visible
        } else {
            SubtreePresentation::Hidden
        };
        let button = if self.cached_button {
            self.button
                .clone()
                .cached(StyleRefinement::default().w(px(120.0)).h(px(48.0)))
                .into_any_element()
        } else {
            self.button.clone().into_any_element()
        };
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .top(px(16.0))
                    .w(px(280.0))
                    .h(px(240.0))
                    .flex()
                    .flex_col()
                    .child(button)
                    .child(self.tree.clone())
                    .with_subtree_presentation(presentation),
            )
            .child(
                img(self.image.clone())
                    .absolute()
                    .left(px(360.0))
                    .top(px(16.0))
                    .w(px(8.0))
                    .h(px(8.0)),
            )
    }
}

struct Fixture {
    handle: WindowHandle<RefusedVisibilityRoot>,
    tree: Entity<RandomizedElementTree>,
    atlas: Arc<RejectOnceAtlas>,
    counts: Rc<PointerCounts>,
}

impl Fixture {
    fn open(cx: &mut TestAppContext, cached_button: bool) -> Self {
        let tree = cx.new(|cx| {
            let tree = RandomizedElementTree::new_with_config(
                RandomizedElementTreeConfig::new(0, 4)
                    .with_topology(RandomizedElementTreeTopology::Wide)
                    .with_entity_density(1.0)
                    .with_cached_views(cached_button),
                cx,
            );
            {
                let mut snapshot = tree.snapshot.borrow_mut();
                snapshot.root.width = 240.0;
                snapshot.root.height = 160.0;
                snapshot.root.visible = true;
            }
            tree
        });
        let counts = Rc::new(PointerCounts::default());
        let button = cx.new(|_| VisibilityButton(counts.clone()));
        let atlas = Arc::new(RejectOnceAtlas::new());
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            image::ImageBuffer::from_pixel(1, 1, image::Rgba([30, 60, 90, 255])),
        )]));
        cx.defer_next_window_frame_requests();
        let handle = cx.open_window(WINDOW_SIZE, {
            let tree = tree.clone();
            let atlas = atlas.clone();
            move |window, _| {
                window.set_sprite_atlas_for_test(atlas);
                RefusedVisibilityRoot {
                    tree,
                    button,
                    cached_button,
                    image,
                }
            }
        });
        let fixture = Self {
            handle,
            tree,
            atlas,
            counts,
        };
        fixture.update(cx, |window, app| {
            let _ = window.activate_window();
            window.draw(app).clear(app);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            assert_eq!(window.rendered_frame.scene.polychrome_sprites.len(), 1);
        });
        fixture
    }

    fn update(&self, cx: &mut TestAppContext, update: impl FnOnce(&mut Window, &mut App)) {
        cx.update_window(self.handle.into(), |_, window, app| update(window, app))
            .expect("the visibility fixture must retain its native window");
    }

    fn toggle_root_visibility(&self, app: &mut App) {
        self.tree.update(app, |tree, cx| {
            assert_eq!(
                tree.apply_mutation(RandomizedElementTreeMutationKind::RootVisibility, cx),
                RandomizedElementTreeMutation::RootVisibility
            );
        });
    }

    fn refuse_hidden_candidate(&self, window: &mut Window, app: &mut App) -> u64 {
        let revision = window.rendered_frame_revision();
        let accepted = SceneSnapshot::from_window(window);
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            Some(revision)
        );
        self.toggle_root_visibility(app);
        assert!(!self.tree.read(app).snapshot().root.visible);
        self.atlas.fail_next_lease();
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), revision);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision)
        );
        assert_eq!(SceneSnapshot::from_window(window), accepted);
        assert!(window.refresh_pending_for_test());
        revision
    }
}

fn mouse_down(window: &mut Window, app: &mut App) {
    window.dispatch_event(
        MouseDownEvent {
            position: point(px(40.0), px(32.0)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Default::default(),
            first_mouse: false,
        }
        .to_platform_input(),
        app,
    );
}

fn mouse_up(window: &mut Window, app: &mut App) {
    window.dispatch_event(
        MouseUpEvent {
            position: point(px(40.0), px(32.0)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Default::default(),
        }
        .to_platform_input(),
        app,
    );
}

#[crate::test]
fn refused_root_visibility_preserves_the_presented_mouse_click(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(crate::QuitMode::Explicit));
    for cached_button in [false, true] {
        let fixture = Fixture::open(cx, cached_button);
        fixture.update(cx, |window, app| {
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            mouse_down(window, app);
            assert_eq!(fixture.counts.downs.get(), 1);
            assert_eq!(fixture.counts.clicks.get(), 0);
            let revision = fixture.refuse_hidden_candidate(window, app);

            mouse_up(window, app);
            assert_eq!(window.rendered_frame_revision(), revision);
            assert_eq!(fixture.counts.ups.get(), 1);
            assert_eq!(
                fixture.counts.clicks.get(),
                1,
                "a refused Hidden candidate must preserve the presented mouse press; \
                 cached_button={cached_button}"
            );
            mouse_up(window, app);
            assert_eq!(
                fixture.counts.clicks.get(),
                1,
                "one press may activate only once"
            );
        });
        close_window(fixture.handle.into(), cx);
    }
}

#[crate::test]
fn accepted_hidden_retry_clears_the_presented_mouse_press_before_restore(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(crate::QuitMode::Explicit));
    for cached_button in [false, true] {
        let fixture = Fixture::open(cx, cached_button);
        fixture.update(cx, |window, app| {
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            mouse_down(window, app);
            assert_eq!(fixture.counts.downs.get(), 1);
            let revision = fixture.refuse_hidden_candidate(window, app);

            window.draw(app).clear(app);
            assert_eq!(window.rendered_frame_revision(), revision + 1);
            assert!(!window.refresh_pending_for_test());
            assert_eq!(window.rendered_frame.scene.polychrome_sprites.len(), 1);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            mouse_up(window, app);
            assert_eq!(
                fixture.counts.ups.get(),
                0,
                "accepted Hidden suppresses input"
            );
            assert_eq!(fixture.counts.clicks.get(), 0);

            fixture.toggle_root_visibility(app);
            assert!(fixture.tree.read(app).snapshot().root.visible);
            window.refresh();
            window.draw(app).clear(app);
            assert_eq!(window.rendered_frame_revision(), revision + 2);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            mouse_up(window, app);
            assert_eq!(
                fixture.counts.ups.get(),
                1,
                "restored listeners receive MouseUp"
            );
            assert_eq!(
                fixture.counts.clicks.get(),
                0,
                "an accepted Hidden retry must retire the old press before restoring Visible; \
                 cached_button={cached_button}"
            );

            mouse_down(window, app);
            mouse_up(window, app);
            assert_eq!(fixture.counts.downs.get(), 2);
            assert_eq!(
                fixture.counts.clicks.get(),
                1,
                "a new visible press still activates"
            );
            mouse_up(window, app);
            assert_eq!(
                fixture.counts.clicks.get(),
                1,
                "stale releases never activate twice"
            );
        });
        close_window(fixture.handle.into(), cx);
    }
}
