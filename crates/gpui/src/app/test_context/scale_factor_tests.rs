use crate::{
    AnyWindowHandle, AppContext as _, Bounds, DevicePixels, Empty, PlatformWindow, TestAppContext,
    point, px, size,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[crate::test]
fn scale_factor_change_preserves_bounds_and_survives_resize(cx: &mut TestAppContext) {
    let handle: AnyWindowHandle = cx.add_window(|_, _| Empty).into();
    let native = cx.test_window(handle);
    let mut expected = native.bounds();
    assert_eq!(native.scale_factor(), 2.0);

    let physical_origin = point(DevicePixels(-400), DevicePixels(200));
    cx.set_platform_window_physical_client_geometry(
        handle,
        Some(Bounds::new(
            physical_origin,
            expected.size.to_device_pixels(2.0),
        )),
        2.0,
    );

    for (scale, resized_size) in [
        (1.0, size(px(800.), px(600.))),
        (1.25, size(px(640.), px(480.))),
        (2.0, size(px(1024.), px(768.))),
    ] {
        cx.simulate_window_scale_factor_change(handle, scale);
        cx.update_window(handle, |_, window, _| {
            assert_eq!(window.scale_factor(), scale);
            assert_eq!(window.bounds(), expected);
            assert_eq!(window.viewport_size(), expected.size);
        })
        .unwrap();
        let physical = native.platform_facts().physical_geometry.unwrap();
        assert_eq!(physical.scale_factor(), scale);
        assert_eq!(physical.client_bounds().origin, physical_origin);
        assert_eq!(
            physical.client_bounds().size,
            expected.size.to_device_pixels(scale)
        );
        assert_eq!(
            physical.display_observation().unwrap().scale_factor(),
            scale
        );

        cx.simulate_window_resize(handle, resized_size);
        expected.size = resized_size;
        cx.update_window(handle, |_, window, _| {
            assert_eq!(window.scale_factor(), scale);
            assert_eq!(window.bounds(), expected);
            assert_eq!(window.viewport_size(), resized_size);
        })
        .unwrap();
        assert_eq!(
            native
                .platform_facts()
                .physical_geometry
                .unwrap()
                .client_bounds(),
            Bounds::new(physical_origin, resized_size.to_device_pixels(scale)),
        );
    }
}

#[crate::test]
fn scale_factor_change_waits_for_native_ingress_after_app_borrow(cx: &mut TestAppContext) {
    let handle: AnyWindowHandle = cx.add_window(|_, _| Empty).into();
    let mut native = cx.test_window(handle);
    let original_size = native.content_size();
    cx.update(|app| {
        native.simulate_scale_factor_change(1.25);
        assert_eq!(native.scale_factor(), 1.25);
        app.update_window(handle, |_, window, _| {
            assert_eq!(window.scale_factor(), 2.0);
        })
        .unwrap();
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, _| {
        assert_eq!(window.scale_factor(), 1.25);
        assert_eq!(window.viewport_size(), original_size);
    })
    .unwrap();
}

#[crate::test]
fn window_scale_factor_setter_updates_platform_geometry_and_survives_resize(
    cx: &mut TestAppContext,
) {
    let visual = cx.add_empty_window();
    let handle = visual.window;
    let native = visual.test_window(handle);
    let original_size = native.content_size();
    let origin = point(DevicePixels(120), DevicePixels(-240));
    visual.set_platform_window_physical_client_geometry(
        handle,
        Some(Bounds::new(origin, original_size.to_device_pixels(2.0))),
        2.0,
    );
    visual.update(|window, _| {
        window.set_scale_factor(1.25);
        assert_eq!(window.scale_factor(), 1.25);
        assert_eq!(window.viewport_size(), original_size);
        let physical = window.platform_facts().physical_geometry.unwrap();
        assert_eq!(physical.scale_factor(), 1.25);
        assert_eq!(
            physical.client_bounds(),
            Bounds::new(origin, original_size.to_device_pixels(1.25)),
        );
    });
    assert_eq!(native.scale_factor(), 1.25);
    visual.simulate_scale_factor_change(1.0);
    visual.simulate_resize(size(px(801.), px(601.)));
    visual.update(|window, _| {
        assert_eq!(window.scale_factor(), 1.0);
        assert_eq!(window.viewport_size(), size(px(801.), px(601.)));
    });
    assert_eq!(
        native
            .platform_facts()
            .physical_geometry
            .unwrap()
            .client_bounds(),
        Bounds::new(origin, size(DevicePixels(801), DevicePixels(601))),
    );
}

#[crate::test]
fn invalid_scale_factor_does_not_mutate_window_geometry(cx: &mut TestAppContext) {
    let handle: AnyWindowHandle = cx.add_window(|_, _| Empty).into();
    let mut native = cx.test_window(handle);
    let bounds = native.bounds();
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                native.simulate_scale_factor_change(scale);
            }))
            .is_err()
        );
        assert_eq!(native.scale_factor(), 2.0);
        assert_eq!(native.bounds(), bounds);
    }
    cx.run_until_parked();
    cx.update_window(handle, |_, window, _| {
        assert_eq!(window.scale_factor(), 2.0)
    })
    .unwrap();
}
