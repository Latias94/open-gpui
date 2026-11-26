use std::{cell::RefCell, rc::Rc};

use open_gpui::{
    Context, Entity, EntityInputHandler as _, EventEmitter, FocusHandle, Focusable as _, Modifiers,
    MouseButton, ScrollHandle, StyleRefinement, TestAppContext, VisualContext as _,
    VisualTestContext, Window, WindowHandle, div, point, prelude::*, px, size,
};
use open_gpui_base::{
    Menu, MenuEvent, MenuItem, Modal, NumericEvent, NumericRange, NumericScrubber, Select,
    TextInput, TextInputEvent,
};

fn draw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

fn activate<V: Render>(window: WindowHandle<V>, app: &TestAppContext) -> VisualTestContext {
    let mut visual = VisualTestContext::from_window(window.into(), app);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    draw(&mut visual);
    visual
}

fn events<T: EventEmitter<E>, E: Clone + 'static>(
    visual: &mut VisualTestContext,
    entity: &Entity<T>,
) -> (Rc<RefCell<Vec<E>>>, open_gpui::Subscription) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    let subscription = visual.update(|_, cx| {
        cx.subscribe(entity, move |_, event: &E, _| {
            observed.borrow_mut().push(event.clone());
        })
    });
    (events, subscription)
}

#[test]
fn text_input_click_keeps_the_preedit_range_until_native_completion() {
    let mut app = TestAppContext::single();
    let window = app.open_window(size(px(160.), px(80.)), |_, cx| TextInput::new("a😀b", cx));
    let mut visual = activate(window, &app);
    let input = window.root(&mut visual).expect("input root");
    let (events, _subscription) = events(&mut visual, &input);
    visual.update(|window, cx| input.focus_handle(cx).focus(window, cx));
    draw(&mut visual);

    visual.simulate_marked_text(Some(1..3), "候", None);
    draw(&mut visual);
    let start = point(px(1.), px(8.));
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.update_window_entity(&input, |input, window, cx| {
        assert_eq!(input.value(), "a😀b");
        assert_eq!(input.marked_text_range(window, cx), Some(1..2));
        assert_eq!(
            input.selected_text_range(false, window, cx).unwrap().range,
            2..2
        );
        assert!(window.captured_pointer().is_none());
    });
    visual.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    assert!(events.borrow().is_empty());

    visual.simulate_marked_text(Some(1..2), "𠮷", Some(2..2));
    visual.update_window_entity(&input, |input, window, cx| {
        assert_eq!(input.marked_text_range(window, cx), Some(1..3));
        input.replace_text_in_range(None, "完", window, cx);
        assert_eq!(input.value(), "a完b");
        assert!(input.marked_text_range(window, cx).is_none());
    });

    draw(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.update_window_entity(&input, |input, window, cx| {
        assert_eq!(
            input.selected_text_range(false, window, cx).unwrap().range,
            0..0
        );
        assert!(window.captured_pointer().is_some());
    });

    visual.simulate_mouse_move(point(px(180.), px(110.)), None, Modifiers::none());
    visual.update(|window, _| assert!(window.captured_pointer().is_none()));
    visual.simulate_keystrokes("enter");
    assert_eq!(
        *events.borrow(),
        [
            TextInputEvent::Change("a完b".into()),
            TextInputEvent::Submit("a完b".into()),
        ]
    );
}

#[test]
fn numeric_scrubber_blur_commits_only_when_the_draft_changes() {
    let mut app = TestAppContext::single();
    let window = app.open_window(size(px(160.), px(80.)), |_, cx| {
        NumericScrubber::new(2., NumericRange::new(0., 10.).expect("range"), cx)
    });
    let mut visual = activate(window, &app);
    let scrubber = window.root(&mut visual).expect("scrubber root");
    let (events, _subscription) = events(&mut visual, &scrubber);

    for draft in [None, Some("4")] {
        visual.update(|window, cx| scrubber.focus_handle(cx).focus(window, cx));
        visual.simulate_keystrokes("enter");
        draw(&mut visual);
        if let Some(draft) = draft {
            visual.simulate_input(draft);
        }
        visual.update(|window, cx| window.blur(cx));
        draw(&mut visual);
        if draft.is_none() {
            assert!(events.borrow().is_empty());
            draw(&mut visual);
        }
    }
    assert_eq!(
        *events.borrow(),
        [NumericEvent::Change(4.), NumericEvent::Commit(4.)]
    );
}

struct ModalBody {
    select: Entity<Select<u32>>,
    menu: Entity<Menu<u32>>,
}

impl Render for ModalBody {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(120.))
            .child(self.select.clone())
            .child(self.menu.clone())
    }
}

struct ModalWindow {
    outside: FocusHandle,
    modal: Entity<Modal>,
    body: Entity<ModalBody>,
}

impl Render for ModalWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(
                div()
                    .track_focus(&self.outside)
                    .tab_stop(true)
                    .child("Outside"),
            )
            .child(self.modal.clone())
    }
}

#[test]
fn popup_tab_wraps_both_directions_inside_the_modal() {
    let mut app = TestAppContext::single();
    let window = app.open_window(size(px(240.), px(240.)), |_, cx| {
        let select = cx.new(|cx| Select::new("Select", [MenuItem::new(1, "Choice")], cx));
        let menu = cx.new(|cx| Menu::menu("Menu", [MenuItem::new(1, "Command")], cx));
        let body = cx.new(|_| ModalBody { select, menu });
        ModalWindow {
            outside: cx.focus_handle(),
            modal: cx.new(|cx| Modal::new("Dialog", body.clone(), cx)),
            body,
        }
    });
    let mut visual = activate(window, &app);
    let (modal, select, menu) = window
        .update(&mut visual, |root, _, cx| {
            let body = root.body.read(cx);
            (root.modal.clone(), body.select.clone(), body.menu.clone())
        })
        .expect("modal window");
    visual.update_window_entity(&modal, |modal, window, cx| modal.set_open(true, window, cx));
    draw(&mut visual);

    visual.update(|window, cx| menu.focus_handle(cx).focus(window, cx));
    for (source, key, target) in [(&menu, "tab", &select), (&select, "shift-tab", &menu)] {
        visual.simulate_keystrokes("enter");
        draw(&mut visual);
        visual.update(|_, cx| assert!(source.read(cx).is_open()));
        visual.simulate_keystrokes(key);
        draw(&mut visual);
        visual.update(|window, cx| {
            assert!(!source.read(cx).is_open());
            assert!(target.focus_handle(cx).is_focused(window));
            assert!(modal.read(cx).is_open());
        });
    }
}

struct ScrollWindow {
    select: Entity<Select<u32>>,
    scroll: ScrollHandle,
}

impl Render for ScrollWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().justify_end().child(
            div()
                .id("scroll")
                .w(px(96.))
                .h(px(140.))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .child(div().h(px(70.)))
                .child(self.select.clone())
                .child(div().h(px(140.))),
        )
    }
}

fn scroll_window(app: &mut TestAppContext) -> WindowHandle<ScrollWindow> {
    app.open_window(size(px(240.), px(240.)), |_, cx| ScrollWindow {
        select: cx.new(|cx| {
            let mut select = Select::new("Select", [MenuItem::new(1, "Choice")], cx);
            select.set_trigger_style(StyleRefinement::default().h(px(20.)), cx);
            select
        }),
        scroll: ScrollHandle::new(),
    })
}

#[test]
fn popup_dismisses_on_secondary_mouse_down_and_focus_loss() {
    let mut app = TestAppContext::single();
    let window = scroll_window(&mut app);
    let mut visual = activate(window, &app);
    let select = window
        .update(&mut visual, |root, _, _| root.select.clone())
        .expect("scroll window");
    let (events, _subscription) = events(&mut visual, &select);
    let trigger = point(px(160.), px(78.));
    visual.simulate_click(trigger, Modifiers::none());
    draw(&mut visual);
    visual.update(|_, cx| assert!(select.read(cx).is_open()));
    let outside = point(px(10.), px(10.));
    visual.simulate_mouse_down(outside, MouseButton::Right, Modifiers::none());
    visual.simulate_mouse_up(outside, MouseButton::Right, Modifiers::none());
    visual.update(|_, cx| assert!(!select.read(cx).is_open()));

    visual.simulate_click(trigger, Modifiers::none());
    draw(&mut visual);
    visual.update(|_, cx| assert!(select.read(cx).is_open()));
    visual.update(|window, cx| window.blur(cx));
    draw(&mut visual);
    visual.update(|_, cx| assert!(!select.read(cx).is_open()));
    assert_eq!(
        *events.borrow(),
        [MenuEvent::Dismissed, MenuEvent::Dismissed]
    );
}

#[test]
fn open_popup_follows_the_trigger_after_resize_and_scroll() {
    let mut app = TestAppContext::single();
    let window = scroll_window(&mut app);
    let mut visual = activate(window, &app);
    let select = window
        .update(&mut visual, |root, _, _| root.select.clone())
        .expect("scroll window");
    visual.simulate_click(point(px(160.), px(78.)), Modifiers::none());
    visual.update(|_, cx| assert!(select.read(cx).is_open()));

    visual.simulate_resize(size(px(360.), px(240.)));
    window
        .update(&mut visual, |root, window, _| {
            root.scroll.set_offset(point(px(0.), px(-48.)));
            window.refresh();
        })
        .expect("scroll window");
    draw(&mut visual);
    visual.simulate_click(point(px(280.), px(52.)), Modifiers::none());
    visual.update(|_, cx| {
        assert_eq!(select.read(cx).selected(), Some(&1));
        assert!(!select.read(cx).is_open());
    });
}

#[test]
fn scrolling_the_trigger_out_of_view_dismisses_the_popup() {
    let mut app = TestAppContext::single();
    let window = scroll_window(&mut app);
    let mut visual = activate(window, &app);
    let select = window
        .update(&mut visual, |root, _, _| root.select.clone())
        .unwrap();
    visual.simulate_click(point(px(160.), px(78.)), Modifiers::none());
    draw(&mut visual);
    visual.update(|_, cx| assert!(select.read(cx).is_open()));

    window
        .update(&mut visual, |root, window, _| {
            root.scroll.set_offset(point(px(0.), px(-100.)));
            window.refresh();
        })
        .unwrap();
    draw(&mut visual);
    visual.run_until_parked();
    visual.update(|_, cx| assert!(!select.read(cx).is_open()));
}
