use std::{cell::RefCell, rc::Rc};

use super::anchored;
use crate::{
    Anchor, Bounds, Context, Hitbox, HitboxBehavior, IntoElement, ParentElement, PortalAnchorExt,
    PortalAnchorHandle, PortalAnchorSnapshot, Render, Styled, SubtreeTransform,
    SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, Window, canvas, div, point,
    portal_anchor_follower, px, size,
};

struct AttachedPortalView {
    handle: PortalAnchorHandle,
    observed: Rc<RefCell<Option<PortalAnchorSnapshot>>>,
    hitbox: Rc<RefCell<Option<Hitbox>>>,
}

impl Render for AttachedPortalView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let observed = self.observed.clone();
        let hitbox = self.hitbox.clone();
        let follower = portal_anchor_follower(&self.handle, move |snapshot, _, _| {
            *observed.borrow_mut() = snapshot;
            snapshot.map(|snapshot| {
                anchored()
                    .attach(snapshot.geometry().displayed_bounds(), Anchor::TopRight)
                    .anchor(Anchor::TopLeft)
                    .offset(point(px(4.), px(6.)))
                    .child(
                        canvas(
                            move |bounds, window, _| {
                                *hitbox.borrow_mut() =
                                    Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                            },
                            |_, _, _, _| {},
                        )
                        .w(px(200.))
                        .h(px(100.)),
                    )
                    .into_any_element()
            })
        });

        div().size_full().child(
            div()
                .absolute()
                .left(px(480.))
                .top(px(0.))
                .w(px(40.))
                .h(px(40.))
                .overflow_hidden()
                .child(follower)
                .child(
                    div()
                        .absolute()
                        .left(px(20.))
                        .top(px(30.))
                        .w(px(100.))
                        .h(px(50.))
                        .track_portal_anchor(&self.handle),
                )
                .with_subtree_transform(
                    SubtreeTransform::try_new(
                        size(2., 4.),
                        point(px(100.), px(50.)),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                ),
        )
    }
}

#[crate::test]
fn attached_anchor_uses_the_existing_window_portal_geometry_owner(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(None));
    let hitbox = Rc::new(RefCell::new(None));
    let window = cx.open_window(size(px(800.), px(600.)), |window, _| AttachedPortalView {
        handle: window.new_portal_anchor(),
        observed: observed.clone(),
        hitbox: hitbox.clone(),
    });
    cx.run_until_parked();

    let target = observed
        .borrow()
        .expect("the follower should resolve the later transformed target");
    let hitbox = hitbox
        .borrow()
        .clone()
        .expect("the attached portal should prepaint");
    assert_eq!(
        target.geometry().displayed_bounds(),
        Bounds::new(point(px(620.), px(170.)), size(px(200.), px(200.)))
    );
    assert_eq!(
        hitbox.layout_bounds(),
        Bounds::new(point(px(416.), px(176.)), size(px(200.), px(100.)))
    );
    assert_eq!(hitbox.displayed_bounds(), hitbox.layout_bounds());
    assert_eq!(
        hitbox.displayed_clip_bounds(),
        Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)))
    );
    assert!(target.effective_clip_bounds().left() > hitbox.displayed_bounds().left());
    assert!(hitbox.contains_window_point(point(px(500.), px(200.))));
    window
        .update(cx, |view, window, _| {
            assert_eq!(
                window
                    .resolve_portal_anchor(&view.handle, |snapshot, _| snapshot)
                    .unwrap(),
                Some(target)
            );
        })
        .unwrap();
}
