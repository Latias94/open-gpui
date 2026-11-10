use super::*;
use crate::{
    Context, InputEvent, Modifiers, OperatingSystem, PlatformInfo, PlatformTarget, TestAppContext,
};

struct RuntimeDebugSource {
    ordinary_presses: Rc<Cell<usize>>,
}

#[cfg(feature = "bench-support")]
struct SiblingDebugSource {
    editor_focus: crate::FocusHandle,
}

#[cfg(feature = "bench-support")]
impl Render for SiblingDebugSource {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .id("sibling-source-label")
                    .size(px(300.))
                    .occlude()
                    .debug(),
            )
            .child(
                div()
                    .id("sibling-editor")
                    .size(px(100.))
                    .track_focus(&self.editor_focus),
            )
    }
}

#[cfg(feature = "bench-support")]
#[test]
fn debug_source_underline_refreshes_when_keyboard_focus_is_in_a_sibling() {
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Linux,
    ] {
        let mut test = TestAppContext::single();
        test.set_platform_info(PlatformInfo {
            os,
            target: PlatformTarget::Web,
        });
        let (root, visual) = test.add_window_view(|_, cx| SiblingDebugSource {
            editor_focus: cx.focus_handle(),
        });
        root.update_in(visual, |root, window, cx| {
            root.editor_focus.focus(window, cx)
        });
        visual.simulate_mouse_move(point(px(2.), px(2.)), None, Modifiers::none());
        visual.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let underline_count = |window: &Window| {
            window
                .rendered_scene()
                .quads
                .iter()
                .filter(|quad| quad.bounds.size.height == px(1.).scale(window.scale_factor()))
                .count()
        };
        assert_eq!(visual.update(|window, _| underline_count(window)), 0);
        let secondary = if os == OperatingSystem::Mac {
            Modifiers::command()
        } else {
            Modifiers::control()
        };
        visual.simulate_modifiers_change(secondary);
        assert_eq!(
            visual.update(|window, _| underline_count(window)),
            1,
            "the hovered source label must redraw independently of sibling focus: {os:?}",
        );
        visual.simulate_mouse_move(point(px(200.), px(200.)), None, secondary);
        assert_eq!(
            visual.update(|window, _| underline_count(window)),
            0,
            "leaving the label inside its hitbox must remove its underline: {os:?}",
        );
        visual.simulate_mouse_move(point(px(2.), px(2.)), None, secondary);
        assert_eq!(
            visual.update(|window, _| underline_count(window)),
            1,
            "returning to the label must restore its underline: {os:?}",
        );
        visual.simulate_modifiers_change(Modifiers::none());
        assert_eq!(
            visual.update(|window, _| underline_count(window)),
            0,
            "releasing secondary must remove the source underline: {os:?}",
        );
        visual.update(|window, cx| window.remove_window(cx));
        drop(root);
        test.quit();
    }
}

#[test]
fn debug_source_secondary_press_and_release_follow_input_without_forced_redraw() {
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Linux,
    ] {
        let mut test = TestAppContext::single();
        test.set_platform_info(PlatformInfo {
            os,
            target: PlatformTarget::Web,
        });
        let presses = Rc::new(Cell::new(0));
        let (root, visual) = test.add_window_view(|_, _| RuntimeDebugSource {
            ordinary_presses: presses.clone(),
        });
        let position = point(px(2.), px(2.));
        visual.simulate_mouse_move(position, None, Modifiers::none());
        visual.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let secondary = if os == OperatingSystem::Mac {
            Modifiers::command()
        } else {
            Modifiers::control()
        };
        visual.simulate_modifiers_change(secondary);
        let captured = visual.update(|window, cx| {
            window.dispatch_event(
                MouseDownEvent {
                    position,
                    modifiers: secondary,
                    button: MouseButton::Left,
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            )
        });
        assert!(
            !captured.propagate,
            "pressing runtime secondary must capture the source label: {os:?}"
        );
        assert_eq!(presses.get(), 0);
        visual.simulate_mouse_up(position, MouseButton::Left, secondary);
        visual.simulate_modifiers_change(Modifiers::none());
        let ordinary = visual.update(|window, cx| {
            window.dispatch_event(
                MouseDownEvent {
                    position,
                    modifiers: Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            )
        });
        assert!(
            ordinary.propagate,
            "released secondary must restore ordinary input: {os:?}"
        );
        assert_eq!(presses.get(), 1);
        visual.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        visual.update(|window, cx| window.remove_window(cx));
        drop(root);
        test.quit();
    }
}

impl Render for RuntimeDebugSource {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let presses = self.ordinary_presses.clone();
        div()
            .id("runtime-debug-source")
            .size(px(300.))
            .debug()
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                presses.set(presses.get() + 1);
            })
    }
}

#[test]
fn debug_source_click_uses_runtime_host_secondary_and_displayed_text_bounds() {
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Linux,
    ] {
        let mut test = TestAppContext::single();
        test.set_platform_info(PlatformInfo {
            os,
            target: PlatformTarget::Web,
        });
        let presses = Rc::new(Cell::new(0));
        let (root, visual) = test.add_window_view(|_, _| RuntimeDebugSource {
            ordinary_presses: presses.clone(),
        });
        let weak_root = root.downgrade();
        let secondary = if os == OperatingSystem::Mac {
            Modifiers::command()
        } else {
            Modifiers::control()
        };
        let other = if os == OperatingSystem::Mac {
            Modifiers::control()
        } else {
            Modifiers::command()
        };

        for (modifiers, position, captured) in [
            (other, point(px(2.), px(2.)), false),
            (secondary, point(px(2.), px(2.)), true),
            (secondary, point(px(200.), px(200.)), false),
            (Modifiers::none(), point(px(2.), px(2.)), false),
        ] {
            visual.simulate_mouse_move(position, None, modifiers);
            visual.simulate_modifiers_change(modifiers);
            visual.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            });
            let before = presses.get();
            let result = visual.update(|window, cx| {
                window.dispatch_event(
                    MouseDownEvent {
                        position,
                        modifiers,
                        button: MouseButton::Left,
                        click_count: 1,
                        first_mouse: false,
                    }
                    .to_platform_input(),
                    cx,
                )
            });
            assert_eq!(
                result.propagate, !captured,
                "source debug capture must use the reported host and displayed label: {os:?}",
            );
            assert_eq!(
                presses.get(),
                before + usize::from(!captured),
                "only source-debug capture may suppress the ordinary press: {os:?}",
            );
            visual.simulate_mouse_up(position, MouseButton::Left, modifiers);
        }

        visual.update(|window, cx| window.remove_window(cx));
        drop(root);
        visual.run_until_parked();
        assert!(
            weak_root.upgrade().is_none(),
            "retirement must release the debug source"
        );
        test.quit();
    }
}
