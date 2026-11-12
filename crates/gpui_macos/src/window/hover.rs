use super::{get_window_state, is_gpui_window};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSApplication, NSEvent, NSWindow};
use objc2_foundation::NSInteger;
use open_gpui::PlatformHoveredWindow;

pub(super) fn hovered_window() -> PlatformHoveredWindow {
    let Some(marker) = crate::main_thread_marker() else {
        return PlatformHoveredWindow::Unavailable;
    };
    // AppKit's desktop-wide mouse-down hit test accounts for other applications and
    // ignoresMouseEvents. An application-only z-order cannot prove this result.
    let window_number = NSWindow::windowNumberAtPoint_belowWindowWithWindowNumber(
        NSEvent::mouseLocation(),
        0,
        marker,
    );
    hovered_window_from_number(window_number, |window_number| {
        let app = NSApplication::sharedApplication(marker);
        let Some(window) = app.windowWithWindowNumber(window_number) else {
            return PlatformHoveredWindow::NoWindow;
        };
        let native_window = Retained::as_ptr(&window).cast_mut().cast::<AnyObject>();
        // SAFETY: AppKit returned a retained window from this application's main-thread registry.
        if !unsafe { is_gpui_window(native_window) } {
            return PlatformHoveredWindow::NoWindow;
        }
        if !window.isVisible()
            || window.isMiniaturized()
            || !window.isOnActiveSpace()
            || window.ignoresMouseEvents()
        {
            return PlatformHoveredWindow::Unavailable;
        }
        // SAFETY: The exact GPUI NSWindow above owns its initialized state for this retained lifetime.
        let state = unsafe { get_window_state(&*native_window) };
        let state = state.lock();
        if state.is_closed() || state.interaction_is_quiesced() || !state.accepts_pointer_input {
            return PlatformHoveredWindow::Unavailable;
        }
        PlatformHoveredWindow::Window(state.handle)
    })
}

fn hovered_window_from_number(
    window_number: NSInteger,
    application_window: impl FnOnce(NSInteger) -> PlatformHoveredWindow,
) -> PlatformHoveredWindow {
    match window_number {
        0 => PlatformHoveredWindow::NoWindow,
        number if number < 0 => PlatformHoveredWindow::Unavailable,
        number => application_window(number),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_no_window_and_unknown_never_fall_back_to_the_application_stack() {
        for (number, expected) in [
            (0, PlatformHoveredWindow::NoWindow),
            (-1, PlatformHoveredWindow::Unavailable),
        ] {
            assert_eq!(
                hovered_window_from_number(number, |_| panic!("no native application hit exists")),
                expected,
            );
        }
    }

    #[open_gpui::test]
    fn a_foreign_native_hit_never_selects_the_gpui_window_behind_it(
        cx: &mut open_gpui::TestAppContext,
    ) {
        let gpui_window = cx.add_window(|_, _| open_gpui::Empty).into();
        let gpui_number = 17;
        let foreign_number = 23;
        let signal = hovered_window_from_number(foreign_number, |number| {
            if number == gpui_number {
                PlatformHoveredWindow::Window(gpui_window)
            } else {
                PlatformHoveredWindow::NoWindow
            }
        });
        assert_eq!(signal, PlatformHoveredWindow::NoWindow);
        assert_eq!(
            hovered_window_from_number(gpui_number, |number| {
                assert_eq!(number, gpui_number);
                PlatformHoveredWindow::Window(gpui_window)
            }),
            PlatformHoveredWindow::Window(gpui_window),
        );
    }

    #[test]
    fn native_hover_is_unavailable_off_the_appkit_main_thread() {
        std::thread::spawn(|| assert_eq!(hovered_window(), PlatformHoveredWindow::Unavailable))
            .join()
            .unwrap();
    }
}
