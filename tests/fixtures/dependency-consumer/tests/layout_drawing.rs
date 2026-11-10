use open_gpui::{
    Anchor, AnyElement, Bounds, Corners, DevicePixels, IntoElement as _, ListAlignment, ListState,
    ParentElement as _, Styled as _, SvgRenderer, SvgSize, anchored, canvas, container_query, div,
    list, point, px, size, svg,
};
use std::sync::Arc;

#[test]
fn standalone_layout_and_svg_elements_compose_through_public_apis() -> open_gpui::Result<()> {
    let bytes = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24pt" height="12pt"><rect width="100%" height="100%" fill="red"/></svg>"#;
    let renderer = SvgRenderer::new(Arc::new(()));
    let parsed = renderer.parse_svg(bytes)?;
    let image = renderer.render_parsed(
        &parsed,
        SvgSize::ExactSize(size(DevicePixels(32), DevicePixels(16))),
    )?;
    assert_eq!(image.size(0), size(DevicePixels(32), DevicePixels(16)));

    let _container: AnyElement = container_query(move |allocated, _, _| {
        let image = image.clone();
        div().w(allocated.width).h(allocated.height).child(canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window
                    .paint_image(bounds, bounds, Corners::default(), image, 0, false)
                    .expect("the ordinary low-level paint signature remains available");
            },
        ))
    })
    .w(px(120.))
    .h(px(40.))
    .into_any_element();
    let _attached: AnyElement = anchored()
        .anchor(Anchor::TopLeft)
        .attach(
            Bounds::new(point(px(10.), px(20.)), size(px(30.), px(40.))),
            Anchor::TopRight,
        )
        .offset(point(px(4.), px(0.)))
        .child(svg().data(bytes).size(px(24.)))
        .into_any_element();
    let state = ListState::new(10, ListAlignment::Top, px(20.)).with_uniform_item_height(px(24.));
    state.set_item_measurement_readiness(|_, _| true);
    state.reset_with_uniform_height(12, px(24.));
    state.pause_following_tail();
    state.scroll_to_item(4);
    let _list: AnyElement = list(state, |_, _, _| div().h(px(24.)).into_any_element())
        .size_full()
        .into_any_element();
    Ok(())
}
