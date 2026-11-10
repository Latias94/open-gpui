//! Fixed-target drag and exit integration regressions.
//! Zed target: 635026fabc38440fce5e1767d125026a17b12012.
//!
//! Sources: f52fd9ac44b, 693962917b5, 1a84d5d92bd, a8491e63b5 and c7bea34383.
//! Fork adaptations: transformed target geometry, exact pointer ownership and candidate listeners.

use super::*;
use crate::{
    AppContext as _, Context, CursorStyle, Empty, ExternalPaths, FileDragPaths, InputEvent,
    Keystroke, SubtreePresentation, SubtreePresentationExt as _, SubtreeTransform,
    SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, canvas,
    window::cached_paint_atlas_tests::RejectOnceAtlas,
};
use std::path::PathBuf;

struct ExternalDragProbe {
    resolved: Rc<Cell<usize>>,
    source: Rc<Cell<Option<PointerCaptureHandle>>>,
    convenience: bool,
}

impl Render for ExternalDragProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source.clone();
        let constructor = move |value: &usize,
                                geometry: &DragStartGeometry,
                                window: &mut Window,
                                cx: &mut App| {
            assert_eq!(*value, 42);
            assert_eq!(geometry.window_position(), point(px(20.), px(5.)));
            assert_eq!(
                geometry.target_local_position().unwrap(),
                point(px(10.), px(10.))
            );
            assert_eq!(geometry.layout_bounds().size, size(px(4.), px(50.)));
            assert_eq!(geometry.displayed_bounds().size, size(px(8.), px(25.)));
            let handle = geometry.pointer_capture_handle();
            assert!(window.owns_pointer_capture(handle, MouseButton::Left));
            source.set(Some(handle));
            cx.new(|_| Empty)
        };
        let resolved = self.resolved.clone();
        let resolver = move |value: &usize, _: &mut Window, _: &mut App| {
            assert_eq!(*value, 42);
            resolved.set(resolved.get() + 1);
            Some(ExternalDragPayload::Files(FileDragPaths::new([(
                PathBuf::from("drag-source"),
                true,
            )])))
        };
        let target = div()
            .id("external-drag-source")
            .w(px(4.))
            .h(px(50.))
            .cursor_col_resize();
        let target = if self.convenience {
            target.on_drag_with_external(42_usize, constructor, resolver)
        } else {
            target
                .on_drag(42_usize, constructor)
                .external_drag_payload(resolver)
        };
        div().size_full().cursor_text().child(
            target.with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2., 0.5),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            ),
        )
    }
}

#[crate::test]
fn external_drag_keeps_exact_transformed_start_and_cursor_before_paint(cx: &mut TestAppContext) {
    for convenience in [false, true] {
        let resolved = Rc::new(Cell::new(0));
        let source = Rc::new(Cell::new(None));
        let handle = cx.add_window({
            let resolved = resolved.clone();
            let source = source.clone();
            move |_, _| ExternalDragProbe {
                resolved,
                source,
                convenience,
            }
        });
        cx.set_platform_hovered_window(Some(handle.into()));
        cx.update_window(handle.into(), |_, window, _| {
            let _ = window.activate_window();
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(resolved.get(), 0);
        let observer = cx.clone();
        cx.update_window(handle.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_event(
                MouseDownEvent {
                    position: point(px(2.), px(5.)),
                    button: MouseButton::Left,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseMoveEvent {
                    position: point(px(20.), px(5.)),
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            assert_eq!(cx.active_drag_value::<usize>(), Some(&42));
            assert_eq!(cx.active_drag.as_ref().unwrap().source, source.get());
            assert_eq!(resolved.get(), 0);
            assert_eq!(observer.platform_cursor_style(), CursorStyle::ResizeColumn);
            let resolve = cx
                .active_drag
                .as_mut()
                .unwrap()
                .external_payload_source
                .take()
                .expect("the accepted source must retain its lazy payload");
            assert_eq!(
                resolve(window, cx),
                Some(ExternalDragPayload::Files(FileDragPaths::new([(
                    PathBuf::from("drag-source"),
                    true,
                )])))
            );
            assert_eq!(resolved.get(), 1);
            assert!(
                cx.active_drag
                    .as_ref()
                    .unwrap()
                    .external_payload_source
                    .is_none()
            );
            assert!(cx.stop_active_drag(window));
            assert!(!window.has_pointer_capture());
            window.draw(cx).clear(cx);
        })
        .unwrap();
        assert_eq!(observer.platform_cursor_style(), CursorStyle::IBeam);
    }
}

type ExitObservations = Rc<RefCell<Vec<(usize, &'static str, Point<Pixels>)>>>;

struct ExitProbe {
    version: Rc<Cell<usize>>,
    observations: ExitObservations,
    presentation: Rc<Cell<SubtreePresentation>>,
    paint_sprite: bool,
}

impl Render for ExitProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let version = self.version.get();
        let mouse_observations = self.observations.clone();
        let file_observations = self.observations.clone();
        let target = div()
            .size(px(20.))
            .on_mouse_exit(move |event, _, _| {
                mouse_observations.borrow_mut().push((
                    version,
                    "mouse",
                    event
                        .target_local_point(event.window_event().position)
                        .unwrap(),
                ));
            })
            .on_file_drop_exit(move |event, window, _| {
                assert!(matches!(event.window_event(), FileDropEvent::Exited));
                file_observations.borrow_mut().push((
                    version,
                    "file",
                    event.target_local_point(window.mouse_position()).unwrap(),
                ));
            })
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2., 0.5),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
            .with_subtree_presentation(self.presentation.get());
        div()
            .size_full()
            .child(div().size(px(20.)).overflow_hidden().child(target))
            .children((self.paint_sprite && version >= 2).then(|| {
                canvas(
                    |_, _, _| (),
                    |bounds, (), window, _| {
                        let image = Arc::new(crate::RenderImage::new(smallvec::smallvec![
                            image::Frame::new(image::RgbaImage::from_pixel(
                                1,
                                1,
                                image::Rgba([255, 255, 255, 255]),
                            )),
                        ]));
                        window
                            .paint_image(bounds, bounds, Default::default(), image, 0, false)
                            .unwrap();
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size(px(16.))
            }))
    }
}

fn dispatch_mouse_exit(window: &mut Window, cx: &mut App, position: Point<Pixels>) {
    window.simulate_mouse_move(position, cx);
    window.dispatch_event(
        MouseExitEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}

fn dispatch_file_exit(window: &mut Window, cx: &mut App, position: Point<Pixels>) {
    window.dispatch_event(
        FileDropEvent::Entered {
            position,
            paths: ExternalPaths::default(),
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(FileDropEvent::Exited.to_platform_input(), cx);
}

#[crate::test]
fn exit_only_listeners_use_clipped_geometry_and_window_local_file_hover(cx: &mut TestAppContext) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let first = cx.add_window({
        let observations = observations.clone();
        move |_, _| ExitProbe {
            version: Rc::new(Cell::new(1)),
            observations,
            presentation: Rc::new(Cell::new(SubtreePresentation::Visible)),
            paint_sprite: false,
        }
    });
    let second = cx.add_window({
        let observations = observations.clone();
        move |_, _| ExitProbe {
            version: Rc::new(Cell::new(2)),
            observations,
            presentation: Rc::new(Cell::new(SubtreePresentation::Visible)),
            paint_sprite: false,
        }
    });
    let inside = point(px(10.), px(5.));
    let clipped = point(px(30.), px(5.));
    cx.update_window(first.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        dispatch_mouse_exit(window, cx, clipped);
        dispatch_file_exit(window, cx, clipped);
        assert!(observations.borrow().is_empty());
        dispatch_mouse_exit(window, cx, inside);
        dispatch_file_exit(window, cx, inside);
    })
    .unwrap();
    assert_eq!(
        *observations.borrow(),
        [
            (1, "mouse", point(px(5.), px(10.))),
            (1, "file", point(px(5.), px(10.))),
        ]
    );
    cx.update_window(second.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        window.dispatch_event(
            KeyDownEvent {
                keystroke: Keystroke::parse_with_os("down", cx.platform_info().os).unwrap(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
        dispatch_file_exit(window, cx, inside);
        window.dispatch_event(FileDropEvent::Ended.to_platform_input(), cx);
        assert!(!cx.has_active_drag());
    })
    .unwrap();
    assert_eq!(
        *observations.borrow(),
        [
            (1, "mouse", point(px(5.), px(10.))),
            (1, "file", point(px(5.), px(10.))),
            (2, "file", point(px(5.), px(10.))),
        ]
    );
}

#[crate::test]
fn exit_listeners_belong_to_the_accepted_candidate_and_presentation(cx: &mut TestAppContext) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let version = Rc::new(Cell::new(1));
    let presentation = Rc::new(Cell::new(SubtreePresentation::Visible));
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window({
        let observations = observations.clone();
        let version = version.clone();
        let presentation = presentation.clone();
        let atlas = atlas.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            ExitProbe {
                version,
                observations,
                presentation,
                paint_sprite: true,
            }
        }
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let accepted = window.rendered_frame_revision();
        version.set(2);
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), accepted);
        let position = point(px(10.), px(5.));
        dispatch_mouse_exit(window, cx, position);
        dispatch_file_exit(window, cx, position);
        assert_eq!(
            *observations.borrow(),
            [
                (1, "mouse", point(px(5.), px(10.))),
                (1, "file", point(px(5.), px(10.))),
            ]
        );
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), accepted + 1);
        dispatch_mouse_exit(window, cx, position);
        dispatch_file_exit(window, cx, position);
        assert_eq!(
            *observations.borrow(),
            [
                (1, "mouse", point(px(5.), px(10.))),
                (1, "file", point(px(5.), px(10.))),
                (2, "mouse", point(px(5.), px(10.))),
                (2, "file", point(px(5.), px(10.))),
            ]
        );
        presentation.set(SubtreePresentation::Inert);
        window.refresh();
        window.draw(cx).clear(cx);
        dispatch_mouse_exit(window, cx, position);
        dispatch_file_exit(window, cx, position);
        assert_eq!(observations.borrow().len(), 4);
    })
    .unwrap();
}
