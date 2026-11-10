use super::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    AnyElement, App, AppContext as _, AtlasTextureKind, AtlasTextureLeaseError, ClickEvent,
    Context, Entity, FocusHandle, InputEvent, InteractiveElement as _, IntoElement, KeyDownEvent,
    KeyUpEvent, KeyboardButton, Keystroke, MonochromeSprite, ParentElement as _,
    PrimitiveTransform, Render, StatefulInteractiveElement as _, StyleRefinement, Styled as _,
    SubtreePresentation, SubtreePresentationExt as _, TargetedEvent, TestAppContext, Window,
    WindowHandle, canvas, div, px, red,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

type Clicks = Rc<RefCell<Vec<(&'static str, KeyboardButton)>>>;

struct KeyboardTarget {
    id: &'static str,
    focus: FocusHandle,
    clicks: Clicks,
    renders: usize,
}

impl Render for KeyboardTarget {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let id = self.id;
        let clicks = self.clicks.clone();
        div()
            .id(id)
            .size(px(40.))
            .track_focus(&self.focus)
            .on_click(move |event: &TargetedEvent<ClickEvent>, _, _| {
                let ClickEvent::Keyboard(click) = event.window_event() else {
                    panic!("expected a keyboard click");
                };
                clicks.borrow_mut().push((id, click.button));
            })
    }
}

struct KeyboardAdmissionRoot {
    targets: [Entity<KeyboardTarget>; 2],
    presentations: [SubtreePresentation; 2],
    show_a: bool,
    cached: bool,
}

impl KeyboardAdmissionRoot {
    fn target(&self, index: usize) -> AnyElement {
        let target = self.targets[index].clone();
        let element = if self.cached {
            target
                .cached(StyleRefinement::default().size(px(40.)))
                .into_any_element()
        } else {
            target.into_any_element()
        };
        element
            .with_subtree_presentation(self.presentations[index])
            .into_any_element()
    }
}

impl Render for KeyboardAdmissionRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().size_full();
        if self.show_a {
            root = root.child(self.target(0));
        }
        root.child(self.target(1)).child(
            canvas(
                |_, _, _| {},
                |bounds, (), window, _| {
                    let _ = window.insert_scene_primitive(MonochromeSprite {
                        order: 0,
                        pad: 0,
                        bounds: window.device_local_bounds(bounds),
                        clip: Default::default(),
                        color: red(),
                        tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                        transform: PrimitiveTransform::IDENTITY,
                    });
                },
            )
            .size(px(10.)),
        )
    }
}

struct Fixture {
    handle: WindowHandle<KeyboardAdmissionRoot>,
    atlas: Arc<RejectOnceAtlas>,
    focus: FocusHandle,
    clicks: Clicks,
}

impl Fixture {
    fn open(cx: &mut TestAppContext, cached: bool) -> Self {
        let focus = cx.read(|app| app.focus_handle());
        let clicks = Clicks::default();
        let targets = ["a", "b"].map(|id| {
            cx.new(|_| KeyboardTarget {
                id,
                focus: focus.clone(),
                clicks: clicks.clone(),
                renders: 0,
            })
        });
        let atlas = Arc::new(RejectOnceAtlas::new());
        cx.defer_next_window_frame_requests();
        let handle = cx.add_window(|window, _| {
            window.sprite_atlas = atlas.clone();
            KeyboardAdmissionRoot {
                targets,
                presentations: [SubtreePresentation::Visible, SubtreePresentation::Inert],
                show_a: true,
                cached,
            }
        });
        let fixture = Self {
            handle,
            atlas,
            focus,
            clicks,
        };
        fixture.update(cx, |root, window, app| {
            let _ = window.activate_window();
            draw(&root, window, app);
            window.focus(&fixture.focus, app);
            draw(&root, window, app);
            let renders = root.read(app).targets[0].read(app).renders;
            draw(&root, window, app);
            if cached {
                assert_eq!(root.read(app).targets[0].read(app).renders, renders);
            }
            assert_eq!(window.committed_focus(app).as_ref(), Some(&fixture.focus));
            assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
            assert!(window.last_atlas_frame_rejection.is_none());
        });
        fixture
    }

    fn update(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(Entity<KeyboardAdmissionRoot>, &mut Window, &mut App),
    ) {
        cx.update_window(self.handle.into(), |root, window, app| {
            f(
                root.downcast::<KeyboardAdmissionRoot>().unwrap(),
                window,
                app,
            );
        })
        .unwrap();
    }
}

fn draw(root: &Entity<KeyboardAdmissionRoot>, window: &mut Window, app: &mut App) {
    window.mark_view_dirty(root.entity_id());
    window.draw(app).clear(app);
}

fn key_down(window: &mut Window, app: &mut App, key: &str) {
    window.dispatch_event(
        KeyDownEvent {
            keystroke: Keystroke::parse_with_os(key, app.platform_info().os).unwrap(),
            is_held: false,
            prefer_character_input: false,
        }
        .to_platform_input(),
        app,
    );
}

fn key_up(window: &mut Window, app: &mut App, key: &str) {
    window.dispatch_event(
        KeyUpEvent {
            keystroke: Keystroke::parse_with_os(key, app.platform_info().os).unwrap(),
        }
        .to_platform_input(),
        app,
    );
}

#[crate::test]
fn refused_inert_candidate_preserves_keyboard_activation(cx: &mut TestAppContext) {
    for cached in [false, true] {
        for (key, button) in [
            ("enter", KeyboardButton::Enter),
            ("space", KeyboardButton::Space),
        ] {
            let fixture = Fixture::open(cx, cached);
            fixture.update(cx, |root, window, app| {
                key_down(window, app, key);
                let revision = window.rendered_frame_revision();
                let generation = window.focus_generation;
                root.update(app, |root, _| {
                    root.presentations[0] = SubtreePresentation::Inert;
                });
                fixture.atlas.fail_next_lease();
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision);
                let rejection = window.last_atlas_frame_rejection.as_ref().unwrap();
                assert_eq!(rejection.generation, revision + 1);
                assert!(matches!(
                    &rejection.error,
                    AtlasTextureLeaseError::TextureUnavailable { .. }
                ));
                assert_eq!(window.focus_generation, generation);
                assert_eq!(window.committed_focus(app).as_ref(), Some(&fixture.focus));

                root.update(app, |root, _| {
                    root.presentations[0] = SubtreePresentation::Visible;
                });
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision + 1);
                assert_eq!(window.focus_generation, generation);
                key_up(window, app, key);
                assert_eq!(fixture.clicks.borrow().as_slice(), &[("a", button)]);
            });
        }
    }
}

#[crate::test]
fn accepted_same_handle_target_switch_cancels_keyboard_activation(cx: &mut TestAppContext) {
    for cached in [false, true] {
        for key in ["enter", "space"] {
            let fixture = Fixture::open(cx, cached);
            fixture.update(cx, |root, window, app| {
                key_down(window, app, key);
                let generation = window.focus_generation;
                let revision = window.rendered_frame_revision();
                root.update(app, |root, _| {
                    root.presentations = [SubtreePresentation::Inert, SubtreePresentation::Visible];
                });
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision + 1);
                assert_eq!(window.focus_generation, generation);
                assert_eq!(window.committed_focus(app).as_ref(), Some(&fixture.focus));

                root.update(app, |root, _| {
                    root.presentations = [SubtreePresentation::Visible, SubtreePresentation::Inert];
                });
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision + 2);
                assert_eq!(window.focus_generation, generation);
                key_up(window, app, key);
                assert!(fixture.clicks.borrow().is_empty());
            });
        }
    }
}

#[crate::test]
fn accepted_target_removal_and_rebuild_cancels_keyboard_activation(cx: &mut TestAppContext) {
    for cached in [false, true] {
        for key in ["enter", "space"] {
            let fixture = Fixture::open(cx, cached);
            fixture.update(cx, |root, window, app| {
                key_down(window, app, key);
                let generation = window.focus_generation;
                let revision = window.rendered_frame_revision();
                root.update(app, |root, _| {
                    root.show_a = false;
                    root.presentations[1] = SubtreePresentation::Visible;
                });
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision + 1);
                assert_eq!(window.focus_generation, generation);
                assert_eq!(window.committed_focus(app).as_ref(), Some(&fixture.focus));

                root.update(app, |root, _| {
                    root.show_a = true;
                    root.presentations[1] = SubtreePresentation::Inert;
                });
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision + 2);
                assert_eq!(window.focus_generation, generation);
                key_up(window, app, key);
                assert!(fixture.clicks.borrow().is_empty());
            });
        }
    }
}

#[crate::test]
fn accepted_inert_target_and_refocus_cancels_keyboard_activation(cx: &mut TestAppContext) {
    for cached in [false, true] {
        for key in ["enter", "space"] {
            let fixture = Fixture::open(cx, cached);
            fixture.update(cx, |root, window, app| {
                key_down(window, app, key);
                let revision = window.rendered_frame_revision();
                root.update(app, |root, _| {
                    root.presentations[0] = SubtreePresentation::Inert;
                });
                draw(&root, window, app);
                assert_eq!(window.rendered_frame_revision(), revision + 1);
                assert!(window.committed_focus(app).is_none());

                root.update(app, |root, _| {
                    root.presentations[0] = SubtreePresentation::Visible;
                });
                draw(&root, window, app);
                window.focus(&fixture.focus, app);
                draw(&root, window, app);
                assert_eq!(window.committed_focus(app).as_ref(), Some(&fixture.focus));
                key_up(window, app, key);
                assert!(fixture.clicks.borrow().is_empty());
            });
        }
    }
}
