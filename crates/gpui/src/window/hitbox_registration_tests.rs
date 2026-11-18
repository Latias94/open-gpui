use super::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    AnyElement, AnyView, AppContext as _, AtlasTextureKind, AvailableSpace, Context, Corners,
    Hitbox, HitboxBehavior, HitboxId, HitboxTag, InteractiveElement as _, IntoElement,
    MonochromeSprite, ParentElement as _, PrimitiveTransform, Render, StyleRefinement, Styled as _,
    SubtreeClip, SubtreeClipExt as _, SubtreePresentation, SubtreePresentationExt as _,
    SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, Window,
    canvas, deferred, div, point, px, red, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

struct RegistrationRoot {
    content: Box<dyn Fn() -> AnyElement>,
    captured: Rc<RefCell<Vec<Hitbox>>>,
}

impl Render for RegistrationRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut content = (self.content)();
        let captured = self.captured.clone();
        canvas(
            move |bounds, window, cx| {
                let ((), hitboxes) = window.capture_prepaint_hitboxes(|window| {
                    content.prepaint_as_root(
                        bounds.origin,
                        size(
                            AvailableSpace::Definite(bounds.size.width),
                            AvailableSpace::Definite(bounds.size.height),
                        ),
                        window,
                        cx,
                    );
                });
                captured.replace(hitboxes);
                content
            },
            |_, mut content, window, cx| content.paint(window, cx),
        )
        .size(px(100.))
    }
}

#[test]
fn hitbox_tag_labels_only_the_existing_target_and_does_not_label_descendants() {
    let mut test_app = TestAppContext::single();
    let tag = HitboxTag::new();
    let captured = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = test_app.add_window_view({
        let captured = captured.clone();
        move |_, _| RegistrationRoot {
            content: Box::new(move || {
                div()
                    .size_full()
                    .relative()
                    .hitbox_tag(tag)
                    .on_mouse_move(|_, _, _| {})
                    .child(div().size_full().on_mouse_move(|_, _, _| {}))
                    .child(div().absolute().size(px(10.)).hitbox_tag(tag))
                    .into_any_element()
            }),
            captured,
        }
    });

    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let captured = captured.borrow();
        assert_eq!(captured.len(), 2, "a tag must not create an inert target");
        let tagged = captured
            .iter()
            .find(|hitbox| hitbox.tag() == Some(tag))
            .expect("the interactive parent must carry its explicit tag");
        let child = captured
            .iter()
            .find(|hitbox| hitbox.tag().is_none())
            .expect("the interactive child must remain untagged");
        let position = point(px(5.), px(5.));
        assert_eq!(
            tagged.id.window_point_target_rank(position, window),
            Some(1)
        );
        assert_eq!(child.id.window_point_target_rank(position, window), Some(0));
    });
}

struct CachedRegistrationChild {
    tag: HitboxTag,
    renders: Rc<Cell<usize>>,
    rolled_back: Rc<Cell<Option<HitboxId>>>,
    deferred: Rc<Cell<Option<HitboxId>>>,
}

impl Render for CachedRegistrationChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let tag = self.tag;
        let rolled_back = self.rolled_back.clone();
        let deferred_hitbox = self.deferred.clone();
        div()
            .size_full()
            .relative()
            .child(
                canvas(
                    move |bounds, window, _| {
                        window.insert_hitbox_with_tag(bounds, HitboxBehavior::Normal, tag);
                        let result = window.transact(|window| {
                            let hitbox =
                                window.insert_hitbox_with_tag(bounds, HitboxBehavior::Normal, tag);
                            rolled_back.set(Some(hitbox.id));
                            Err::<(), ()>(())
                        });
                        assert!(result.is_err());
                    },
                    |_, (), _, _| {},
                )
                .size_full(),
            )
            .child(deferred(
                canvas(
                    move |bounds, window, _| {
                        let hitbox =
                            window.insert_hitbox_with_tag(bounds, HitboxBehavior::Normal, tag);
                        deferred_hitbox.set(Some(hitbox.id));
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .size(px(20.)),
            ))
    }
}

#[test]
fn capture_replays_cached_targets_without_collecting_deferred_or_rolled_back_targets() {
    let mut test_app = TestAppContext::single();
    let tag = HitboxTag::new();
    let renders = Rc::new(Cell::new(0));
    let rolled_back = Rc::new(Cell::new(None));
    let deferred_hitbox = Rc::new(Cell::new(None));
    let child = test_app.new(|_| CachedRegistrationChild {
        tag,
        renders: renders.clone(),
        rolled_back: rolled_back.clone(),
        deferred: deferred_hitbox.clone(),
    });
    let captured = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = test_app.add_window_view({
        let captured = captured.clone();
        move |_, _| RegistrationRoot {
            content: Box::new(move || {
                AnyView::from(child.clone())
                    .cached(StyleRefinement::default().size_full())
                    .into_any_element()
            }),
            captured,
        }
    });
    let initial_renders = renders.get();
    assert!(initial_renders > 0);
    let first = {
        let captured = captured.borrow();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].tag(), Some(tag));
        captured[0].id
    };
    captured.borrow_mut().clear();

    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(
            renders.get(),
            initial_renders,
            "the view must reuse its cache"
        );
        let captured = captured.borrow();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].id, first);
        assert_eq!(captured[0].tag(), Some(tag));
        assert_eq!(
            first.window_point_target_rank(point(px(50.), px(50.)), window),
            Some(0),
            "the replayed captured target must remain committed outside the overlay"
        );
        let deferred_id = deferred_hitbox.get().expect("deferred prepaint must run");
        assert!(
            window
                .rendered_frame
                .hitboxes
                .iter()
                .any(|hitbox| { hitbox.id == deferred_id && hitbox.tag() == Some(tag) })
        );
        assert_eq!(
            deferred_id.window_point_target_rank(point(px(5.), px(5.)), window),
            Some(0),
            "the excluded deferred overlay must still block the captured target"
        );
        assert_ne!(first, deferred_id);
        let rolled_back_id = rolled_back.get().expect("the retryable attempt must run");
        assert_eq!(
            rolled_back_id.window_point_target_rank(point(px(5.), px(5.)), window),
            None
        );
        assert!(
            window
                .rendered_frame
                .hitboxes
                .iter()
                .all(|hitbox| hitbox.id != rolled_back_id)
        );
    });
}

#[test]
fn captured_tags_respect_transformed_exact_clips_and_inert_presentation() {
    for presentation in [SubtreePresentation::Visible, SubtreePresentation::Inert] {
        let mut test_app = TestAppContext::single();
        let tag = HitboxTag::new();
        let captured = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = test_app.add_window_view({
            let captured = captured.clone();
            move |_, _| RegistrationRoot {
                content: Box::new(move || {
                    let radius = size(px(50.), px(50.));
                    let clip = SubtreeClip::try_own_rounded_border_box(Corners {
                        top_left: radius,
                        top_right: radius,
                        bottom_right: radius,
                        bottom_left: radius,
                    })
                    .expect("valid rounded content clip");
                    div()
                        .size_full()
                        .on_mouse_move(|_, _, _| {})
                        .hitbox_tag(tag)
                        .with_subtree_clip(clip)
                        .with_subtree_transform(
                            SubtreeTransform::try_new(
                                size(2., 2.),
                                point(px(30.), px(40.)),
                                SubtreeTransformOrigin::TOP_LEFT,
                            )
                            .expect("valid content transform"),
                        )
                        .with_subtree_presentation(presentation)
                        .into_any_element()
                }),
                captured,
            }
        });

        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            let captured = captured.borrow();
            if presentation == SubtreePresentation::Inert {
                assert!(
                    captured.is_empty(),
                    "inert content must not register a target"
                );
                assert!(
                    window
                        .rendered_frame
                        .hitboxes
                        .iter()
                        .all(|hitbox| hitbox.tag() != Some(tag))
                );
                return;
            }
            assert_eq!(captured.len(), 1);
            let hitbox = &captured[0];
            assert_eq!(hitbox.tag(), Some(tag));
            let corner = point(px(31.), px(41.));
            assert!(hitbox.displayed_bounds().contains(&corner));
            assert_eq!(hitbox.id.window_point_target_rank(corner, window), None);
            assert_eq!(
                hitbox
                    .id
                    .window_point_target_rank(point(px(130.), px(140.)), window),
                Some(0)
            );
        });
    }
}

#[test]
fn rejected_candidate_tags_cannot_replace_committed_hit_authority() {
    let mut test_app = TestAppContext::single();
    let atlas = Arc::new(RejectOnceAtlas::new());
    let accepted_tag = HitboxTag::new();
    let candidate_tag = HitboxTag::new();
    let current_tag = Rc::new(Cell::new(accepted_tag));
    let captured = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = test_app.add_window_view({
        let atlas = atlas.clone();
        let current_tag = current_tag.clone();
        let captured = captured.clone();
        move |window, _| {
            window.sprite_atlas = atlas;
            RegistrationRoot {
                content: Box::new(move || {
                    let tag = current_tag.get();
                    canvas(
                        move |bounds, window, _| {
                            window.insert_hitbox_with_tag(bounds, HitboxBehavior::Normal, tag);
                        },
                        |bounds, (), window, _| {
                            let bounds = window.device_local_bounds(bounds);
                            let _ = window.insert_scene_primitive(MonochromeSprite {
                                order: 0,
                                animation: Default::default(),
                                transformation: Default::default(),
                                bounds,
                                clip: Default::default(),
                                color: red(),
                                tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                                transform: PrimitiveTransform::IDENTITY,
                            });
                        },
                    )
                    .size_full()
                    .into_any_element()
                }),
                captured,
            }
        }
    });

    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let accepted = captured.borrow()[0].clone();
        assert_eq!(accepted.tag(), Some(accepted_tag));
        let generation = window.rendered_frame_revision();
        let position = point(px(5.), px(5.));
        assert_eq!(
            accepted.id.window_point_target_rank(position, window),
            Some(0)
        );
        assert_eq!(
            cx.window_point_target_rank(window.window_handle(), accepted.id, position),
            Some(0),
            "the baseline must publish its snapshot after the window registration commits"
        );
        current_tag.set(candidate_tag);
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);

        let candidate = captured.borrow()[0].clone();
        assert_eq!(candidate.tag(), Some(candidate_tag));
        assert_ne!(candidate.id, accepted.id);
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .expect("the candidate must fail a real atlas lease")
                .generation,
            generation + 1
        );
        assert_eq!(
            candidate.id.window_point_target_rank(position, window),
            None
        );
        assert_eq!(
            accepted.id.window_point_target_rank(position, window),
            Some(0)
        );
        assert_eq!(
            cx.window_point_target_rank(window.window_handle(), candidate.id, position),
            None
        );
        assert_eq!(
            cx.window_point_target_rank(window.window_handle(), accepted.id, position),
            Some(0)
        );

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        let recovered = window
            .rendered_frame
            .hitboxes
            .iter()
            .find(|hitbox| hitbox.tag() == Some(candidate_tag))
            .expect("the accepted retry must register the exact candidate tag");
        assert_eq!(recovered.tag(), Some(candidate_tag));
        assert_eq!(
            recovered.id.window_point_target_rank(position, window),
            Some(0)
        );
        assert_eq!(
            cx.window_point_target_rank(window.window_handle(), recovered.id, position),
            Some(0)
        );
        assert_eq!(accepted.id.window_point_target_rank(position, window), None);
        assert_eq!(
            candidate.id.window_point_target_rank(position, window),
            None
        );
        assert_eq!(
            cx.window_point_target_rank(window.window_handle(), accepted.id, position),
            None
        );
        assert_eq!(
            cx.window_point_target_rank(window.window_handle(), candidate.id, position),
            None
        );
    });
}
