use super::*;
use crate::{
    AppContext as _, Corners, Empty, InputEvent, IntoElement, SubtreeClip, SubtreeClipExt as _,
    SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, red,
};

struct StyleOnlyDragTarget {
    transformed: bool,
    style_calls: Rc<Cell<usize>>,
}

impl Render for StyleOnlyDragTarget {
    fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
        let style_calls = self.style_calls.clone();
        let target = div()
            .size(px(20.))
            .drag_over::<usize>(move |style, value, _, _| {
                assert_eq!(*value, 42);
                style_calls.set(style_calls.get() + 1);
                style.bg(red())
            });
        let target = if self.transformed {
            let radius = size(px(10.), px(10.));
            target
                .with_subtree_clip(
                    SubtreeClip::try_own_rounded_border_box(Corners {
                        top_left: radius,
                        top_right: radius,
                        bottom_right: radius,
                        bottom_left: radius,
                    })
                    .unwrap(),
                )
                .with_subtree_transform(
                    SubtreeTransform::try_new(
                        size(2., 0.5),
                        Point::default(),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                )
                .into_any_element()
        } else {
            target.into_any_element()
        };
        div().size_full().child(target)
    }
}

#[crate::test]
fn style_only_drag_over_inserts_hitbox_and_respects_exact_geometry(cx: &mut TestAppContext) {
    for transformed in [false, true] {
        let style_calls = Rc::new(Cell::new(0));
        let handle = cx.add_window({
            let style_calls = style_calls.clone();
            move |_, _| StyleOnlyDragTarget {
                transformed,
                style_calls,
            }
        });
        cx.test_window(handle.into())
            .defer_frame_requests_for_test();
        cx.set_platform_hovered_window(Some(handle.into()));
        cx.update_window(handle.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(window.rendered_frame.hitboxes.len(), 1);
            let drag_view = cx.new(|_| Empty).into();
            cx.start_active_drag(AnyDrag {
                window_id: handle.window_id(),
                source: None,
                view: drag_view,
                value: Arc::new(42_usize),
                window_preview_offset: Point::default(),
                cursor_style: None,
                button: MouseButton::Left,
                external_payload_source: None,
            });
            let inside = if transformed {
                point(px(20.), px(5.))
            } else {
                point(px(10.), px(10.))
            };
            window.dispatch_event(
                MouseMoveEvent {
                    position: inside,
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            let before_inside = style_calls.get();
            window.draw(cx).clear(cx);
            assert!(style_calls.get() > before_inside);
            assert_eq!(window.rendered_frame.scene.quads.len(), 1);
            assert_eq!(window.rendered_frame.scene.quads[0].background.solid, red());

            let outside = if transformed {
                point(px(1.), px(0.25))
            } else {
                point(px(25.), px(10.))
            };
            window.dispatch_event(
                MouseMoveEvent {
                    position: outside,
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            let before_outside = style_calls.get();
            window.draw(cx).clear(cx);
            assert_eq!(style_calls.get(), before_outside);
            assert!(window.rendered_frame.scene.quads.is_empty());
            assert!(cx.stop_active_drag(window));
        })
        .unwrap();
    }
}
