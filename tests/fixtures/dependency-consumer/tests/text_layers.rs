use open_gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, StyledText, TextLayout, TextRun, Window, fill, rgb,
};

struct SearchText(StyledText);

impl Element for SearchText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.0.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.prepaint(id, inspector_id, bounds, state, window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.0.layout();
        layout
            .paint_background(window, cx)
            .expect("run backgrounds");
        window.paint_quad(fill(layout.bounds(), rgb(0x287d3e)));
        layout
            .paint_foreground(window, cx)
            .expect("glyphs and decorations");
    }
}

#[test]
fn downstream_custom_text_element_compiles_with_the_public_layer_signatures() {
    let _: fn(&TextLayout, &mut Window, &mut App) -> open_gpui::Result<()> =
        TextLayout::paint_background;
    let _: fn(&TextLayout, &mut Window, &mut App) -> open_gpui::Result<()> =
        TextLayout::paint_foreground;
    let source = "independently locked layered consumer";
    let consumer = SearchText(StyledText::new(source).with_runs(vec![TextRun {
        len: source.len(),
        background_color: Some(rgb(0x602040).into()),
        ..Default::default()
    }]));
    let _: SearchText = consumer.into_element();
}
