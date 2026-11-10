use super::*;
use open_gpui::point;

type CreationObserver = Box<dyn Fn(&WaylandWindow)>;

thread_local! {
    static CREATION_OBSERVER: RefCell<Option<CreationObserver>> = const { RefCell::new(None) };
}

pub(super) fn observe_creation(window: &WaylandWindow) {
    CREATION_OBSERVER.with_borrow(|observer| {
        if let Some(observer) = observer {
            observer(window);
        }
    });
}

#[path = "window_effect_native_tests.rs"]
mod native;

fn rectangles(area: Bounds<i32>, radii: Corners<Pixels>) -> Vec<(i32, i32, i32, i32)> {
    let mut rectangles = Vec::new();
    add_rounded_rect(area, radii, |x, y, width, height| {
        rectangles.push((x, y, width, height));
    });
    rectangles
}

#[test]
fn square_corners() {
    let area = Bounds::new(point(10, 20), size(8, 8));
    assert_eq!(rectangles(area, Corners::default()), vec![(10, 20, 8, 8)]);
}

#[test]
fn independent_corners() {
    let area = Bounds::new(point(10, 20), size(8, 8));
    let radii = Corners {
        top_left: px(4.0),
        top_right: px(0.0),
        bottom_left: px(0.0),
        bottom_right: px(4.0),
    };
    assert_eq!(
        rectangles(area, radii),
        vec![
            (12, 20, 6, 1),
            (11, 21, 7, 1),
            (10, 22, 8, 1),
            (10, 23, 8, 1),
            (10, 27, 6, 1),
            (10, 26, 7, 1),
            (10, 25, 8, 1),
            (10, 24, 8, 1),
        ]
    );
}

#[test]
fn clamps_radii() {
    let area = Bounds::new(point(10, 20), size(10, 6));
    assert_eq!(
        rectangles(area, Corners::all(px(100.0))),
        rectangles(area, Corners::all(px(3.0)))
    );
    assert_eq!(
        rectangles(area, Corners::all(px(-1.0))),
        rectangles(area, Corners::default())
    );
}

#[test]
fn empty_area() {
    for dimensions in [size(0, 10), size(10, 0), size(-1, 10), size(10, -1)] {
        let area = Bounds::new(point(10, 20), dimensions);
        assert!(rectangles(area, Corners::all(px(4.0))).is_empty());
    }
}

#[test]
fn either_official_protocol_preserves_blur_without_enabling_other_backdrops() {
    for (ext, kde) in [(false, false), (false, true), (true, false), (true, true)] {
        for decorations in [WindowDecorations::Client, WindowDecorations::Server] {
            for requested in [
                WindowBackgroundAppearance::Opaque,
                WindowBackgroundAppearance::Transparent,
                WindowBackgroundAppearance::Blurred,
                WindowBackgroundAppearance::MicaBackdrop,
                WindowBackgroundAppearance::MicaAltBackdrop,
            ] {
                let projection =
                    WaylandWindowBackgroundProjection::new(requested, decorations, ext, kde);
                let expected = match requested {
                    WindowBackgroundAppearance::Opaque => WindowBackgroundAppearance::Opaque,
                    WindowBackgroundAppearance::Blurred if ext || kde => {
                        WindowBackgroundAppearance::Blurred
                    }
                    _ => WindowBackgroundAppearance::Transparent,
                };
                assert_eq!(projection.observed_appearance, expected);
                assert_eq!(
                    projection.blur_enabled,
                    expected == WindowBackgroundAppearance::Blurred
                );
                assert_eq!(
                    projection.compositor_opaque_region,
                    expected == WindowBackgroundAppearance::Opaque
                        && decorations == WindowDecorations::Server
                );
                assert_eq!(
                    projection.renderer_transparent,
                    !projection.compositor_opaque_region
                );
            }
        }
    }
}
