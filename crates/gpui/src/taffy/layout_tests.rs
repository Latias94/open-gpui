use super::*;
use crate::{AppContext as _, Empty, TestAppContext, px};
use std::{cell::RefCell, rc::Rc};

#[test]
fn measured_leaf_applies_padding_border_and_logical_content_constraints_at_each_scale() {
    let mut app = TestAppContext::single();
    let handle = app.open_window(size(px(320.), px(200.)), |_, _| Empty);
    for (scale, expected_content_width, expected_height) in [
        (1., 110., 30.),
        (1.5, 110.666_664, 29.333_334),
        (2., 110., 30.),
    ] {
        app.set_platform_window_physical_client_geometry(handle.into(), None, scale);
        let mut platform_window = app
            .update_window(handle.into(), |_, window, _| {
                window.platform_window.as_test().unwrap().clone()
            })
            .unwrap();
        platform_window.simulate_resize(size(px(320.), px(200.)));
        app.run_until_parked();
        app.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.scale_factor(), scale);
            let observations = Rc::new(RefCell::new(Vec::new()));
            let observed = observations.clone();
            let mut engine = TaffyLayoutEngine::new();
            let node = engine.request_measured_layout(
                Style {
                    size: size(px(120.).into(), Length::Auto),
                    padding: Edges::all(px(4.).into()),
                    border_widths: Edges::all(px(1.).into()),
                    ..Style::default()
                },
                px(16.),
                scale,
                move |known, available, _, _| {
                    observed.borrow_mut().push((known, available));
                    let width = known.width.unwrap_or(match available.width {
                        AvailableSpace::Definite(width) => width,
                        _ => px(80.),
                    });
                    size(width, px(20.))
                },
            );
            engine.compute_layout(
                node,
                size(AvailableSpace::Definite(px(160.)), AvailableSpace::MaxContent),
                window,
                cx,
            );
            let actual = engine.layout_bounds(node, scale);
            assert_eq!(actual.size.width, px(120.));
            assert!((actual.size.height - px(expected_height)).abs() < px(0.001));
            let observations = observations.borrow();
            assert!(!observations.is_empty());
            assert!(observations.iter().any(|(known, available)| {
                known.width.is_none()
                    && matches!(available.width, AvailableSpace::Definite(width)
                        if (width - px(expected_content_width)).abs() < px(0.001))
            }), "{scale}: {observations:?}");
            assert!(observations.iter().all(|(_, available)| {
                available.width == AvailableSpace::MaxContent
                    || matches!(available.width, AvailableSpace::Definite(width) if width <= px(160.))
            }));
        })
        .unwrap();
    }
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))
        .unwrap();
}

#[test]
fn remeasurement_changes_constraints_without_reusing_an_old_leaf_size() {
    let mut app = TestAppContext::single();
    let handle = app.open_window(size(px(320.), px(200.)), |_, _| Empty);
    app.update_window(handle.into(), |_, window, cx| {
        let scale = window.scale_factor();
        let observations = Rc::new(RefCell::new(Vec::new()));
        let observed = observations.clone();
        let mut engine = TaffyLayoutEngine::new();
        let node = engine.request_measured_layout(
            Style::default(),
            px(16.),
            scale,
            move |known, available, _, _| {
                observed.borrow_mut().push(available.width);
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::MinContent => px(25.),
                    AvailableSpace::MaxContent => px(100.),
                    AvailableSpace::Definite(width) => width.min(px(100.)),
                });
                size(width, px(20.))
            },
        );
        for (constraint, expected) in [
            (AvailableSpace::Definite(px(40.)), px(40.)),
            (AvailableSpace::MaxContent, px(100.)),
            (AvailableSpace::MinContent, px(25.)),
            (AvailableSpace::Definite(px(80.)), px(80.)),
            (AvailableSpace::MaxContent, px(100.)),
        ] {
            engine.compute_layout(
                node,
                size(constraint, AvailableSpace::MaxContent),
                window,
                cx,
            );
            assert_eq!(
                engine.layout_bounds(node, scale).size,
                size(expected, px(20.))
            );
        }
        let observations = observations.borrow();
        for expected in [AvailableSpace::MinContent, AvailableSpace::MaxContent] {
            assert!(observations.contains(&expected));
        }
        window.remove_window(cx);
    })
    .unwrap();
}
