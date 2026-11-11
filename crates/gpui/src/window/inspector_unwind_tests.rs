use crate::{
    AnyWindowHandle, App, Context, DivInspectorState, Empty, InspectorElementId, IntoElement,
    Modifiers, MouseButton, MouseDownEvent, ParentElement as _, PlatformInput, Render, Styled as _,
    TestAppContext, Window, div, px, size,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    rc::Rc,
};

struct InspectorFactoryPanic;

#[crate::test]
fn inspector_factory_panic_preserves_both_windows(cx: &mut TestAppContext) {
    let last_render = Rc::new(RefCell::new(None));
    cx.update(|cx| {
        let fail_once = Cell::new(true);
        let last_render = last_render.clone();
        cx.register_inspector_element::<DivInspectorState, _, _>(move |window, _| {
            if fail_once.replace(false) {
                panic_any(InspectorFactoryPanic);
            }
            let owner = window.window_handle().window_id();
            let last_render = last_render.clone();
            move |id, _: &DivInspectorState, _: &mut Window, _: &mut App| {
                *last_render.borrow_mut() = Some((owner, id));
                Empty
            }
        });
        cx.set_inspector_renderer(Box::new(|inspector, window, cx| {
            div()
                .children(inspector.render_inspector_states(window, cx))
                .into_any_element()
        }));
    });

    let viewport = size(px(800.), px(160.));
    let first: AnyWindowHandle = cx.open_window(viewport, |_, _| InspectableDiv).into();
    let second: AnyWindowHandle = cx.open_window(viewport, |_, _| InspectableDiv).into();

    let selected = first
        .update(cx, |_, window, cx| {
            let selected = open_and_pick(window, cx);
            let generation = window.rendered_frame.generation;
            let payload = catch_unwind(AssertUnwindSafe(|| window.draw(cx).clear(cx)))
                .expect_err("the factory panic must escape draw");
            assert!(payload.is::<InspectorFactoryPanic>());
            assert_eq!(window.rendered_frame.generation, generation);
            assert_eq!(
                window.inspector_active_element_id_for_test(cx),
                Some(selected.clone())
            );
            selected
        })
        .expect("first window remains open");

    second
        .update(cx, |_, window, cx| {
            let selected = open_and_pick(window, cx);
            window.draw(cx).clear(cx);
            assert_eq!(*last_render.borrow(), Some((second.window_id(), selected)));
        })
        .expect("second window uses the original registration");

    first
        .update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(*last_render.borrow(), Some((first.window_id(), selected)));
        })
        .expect("first window retries with its original selection");
}

fn open_and_pick(window: &mut Window, cx: &mut App) -> InspectorElementId {
    window.toggle_inspector(cx);
    window.draw(cx).clear(cx);
    let position = window
        .rendered_frame
        .hitboxes
        .iter()
        .find(|hitbox| {
            window
                .rendered_frame
                .inspector_hitboxes
                .contains_key(&hitbox.id)
        })
        .expect("inspectable div has a hitbox")
        .displayed_bounds()
        .center();
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window
        .inspector_active_element_id_for_test(cx)
        .expect("click selects the div")
}

struct InspectableDiv;

impl Render for InspectableDiv {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(40.))
    }
}
