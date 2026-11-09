#![cfg(feature = "test-support")]

//! Downstream property tests of automatic TestPlatform drawing and public lifecycle effects.
//! These observations do not qualify native presentation or GPU pixels.

use open_gpui::{
    App, AppContext as _, Context, IntoElement, ParentElement as _, QuitMode, Render, Styled as _,
    TestAppContext, Window, WindowHandle, canvas, div, fill, px, rgb, size,
};
use std::{cell::Cell, rc::Rc};

struct PropertyControl {
    value: (u64, u8),
    painted: Rc<Cell<Option<(u64, u8)>>>,
}

impl Render for PropertyControl {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let value = self.value;
        let painted = self.painted.clone();
        div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let color = ((value.0 as u32) & 0x00ff_ff00) | u32::from(value.1);
                    window.paint_quad(fill(bounds, rgb(color)));
                    painted.set(Some(value));
                },
            )
            .size(px(24.)),
        )
    }
}

fn open_control(
    cx: &mut TestAppContext,
    value: (u64, u8),
    painted: Rc<Cell<Option<(u64, u8)>>>,
    released: Rc<Cell<Option<(u64, u8)>>>,
) -> WindowHandle<PropertyControl> {
    cx.open_window(size(px(96.), px(64.)), move |_, cx| {
        cx.on_release(move |control: &mut PropertyControl, app| {
            assert!(app.is_test());
            released.set(Some(control.value));
        })
        .detach();
        PropertyControl { value, painted }
    })
}

#[open_gpui::property_test(config = open_gpui::proptest::test_runner::Config::with_cases(4))]
fn generated_view_updates_draw_and_release(cx: &mut TestAppContext, seed: u64, value: u8) {
    assert!(cx.read(App::is_test));
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let painted = Rc::new(Cell::new(None));
    let released = Rc::new(Cell::new(None));
    let window = open_control(cx, (seed, value), painted.clone(), released.clone());
    let root = window.root(cx).expect("the Test window must own its root");
    let weak = root.downgrade();
    cx.update_window(window.into(), |_, window, _| window.refresh())
        .expect("the Test window must refresh");
    assert_eq!(painted.get(), Some((seed, value)));

    let observed = Rc::new(Cell::new(None));
    let _observer = cx.update(|app| {
        let observed = observed.clone();
        app.observe(&root, move |root, app| {
            observed.set(Some(root.read(app).value));
        })
    });
    let expected = (seed, value.wrapping_add(1));
    painted.set(None);
    root.update(cx, |control, cx| {
        control.value = expected;
        cx.notify();
    });
    assert_eq!(observed.get(), Some(expected));
    assert_eq!(painted.get(), Some(expected));

    drop(root);
    assert!(weak.upgrade().is_some());
    assert_eq!(released.get(), None);
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))
        .expect("the Test window must close");
    cx.run_until_parked();
    cx.update(|_| ());
    weak.assert_released();
    assert_eq!(released.get(), Some(expected));
}

#[open_gpui::property_test(config = open_gpui::proptest::test_runner::Config::with_cases(4))]
fn generated_deferred_view_update_survives_until_shutdown(
    cx: &mut TestAppContext,
    seed: u64,
    value: u8,
) {
    let painted = Rc::new(Cell::new(None));
    let released = Rc::new(Cell::new(None));
    let window = open_control(cx, (seed, value), painted.clone(), released.clone());
    let root = window.root(cx).expect("the Test window must own its root");
    let weak = root.downgrade();
    cx.update_window(window.into(), |_, window, _| window.refresh())
        .expect("the Test window must refresh");
    assert_eq!(painted.get(), Some((seed, value)));

    let expected = (seed.rotate_left(1), value.wrapping_add(1));
    painted.set(None);
    cx.update(|app| {
        let root = root.clone();
        app.defer(move |app| {
            root.update(app, |control, cx| {
                control.value = expected;
                cx.notify();
            });
        });
    });
    assert_eq!(painted.get(), Some(expected));

    let quit_completed = Rc::new(Cell::new(false));
    cx.update(|app| {
        let quit_completed = quit_completed.clone();
        app.on_app_quit(move |_| {
            let quit_completed = quit_completed.clone();
            async move { quit_completed.set(true) }
        })
        .detach();
    });
    drop(root);
    assert!(weak.upgrade().is_some());
    assert_eq!(released.get(), None);
    assert!(!quit_completed.get());
    cx.quit();
    cx.run_until_parked();
    weak.assert_released();
    assert!(quit_completed.get());
    assert_eq!(released.get(), Some(expected));
}
