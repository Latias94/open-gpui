use open_gpui::{
    App, Bounds, Context, Hsla, Window, WindowBounds, WindowOptions, div, point, prelude::*, px,
    rgb, size,
};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEdgeRoute, CanvasEndpoint, CanvasHandle, CanvasNode,
    CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme, CanvasShape, CanvasStyle,
    CanvasViewport, HandleRole, canvas_view,
};
use open_gpui_platform::application;

struct SmokeView {
    model: CanvasPaintModel,
}

impl Render for SmokeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = CanvasPaintTheme {
            background: Some(Hsla::from(rgb(0xf8fafc))),
            ..CanvasPaintTheme::default()
        };

        div().size_full().bg(rgb(0xf8fafc)).child(
            canvas_view(
                self.model.clone(),
                CanvasPaintOptions {
                    include_handles: true,
                    ..CanvasPaintOptions::default()
                },
                theme,
            )
            .size_full(),
        )
    }
}

fn demo_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();

    let mut frame = CanvasShape::new(
        "frame",
        Bounds::new(point(px(36.0), px(36.0)), size(px(660.0), px(400.0))),
    );
    frame.z_index = -10;
    frame.style = style(Some("#eef6ff"), Some("#d0d7de"), px(1.0));
    document.insert_shape(frame).unwrap();

    let mut source = node("source", px(72.0), px(116.0), px(180.0), px(104.0));
    source.style = style(Some("#ffffff"), Some("#0969da"), px(2.0));
    source
        .handles
        .push(source_handle("out", px(180.0), px(52.0)));
    document.insert_node(source).unwrap();

    let mut branch = node("branch", px(376.0), px(84.0), px(184.0), px(96.0));
    branch.style = style(Some("#fff7ed"), Some("#ea580c"), px(2.0));
    branch.handles.push(target_handle("in", px(0.0), px(48.0)));
    branch
        .handles
        .push(source_handle("out", px(184.0), px(48.0)));
    document.insert_node(branch).unwrap();

    let mut note = node("note", px(376.0), px(264.0), px(184.0), px(96.0));
    note.style = style(Some("#f0fdf4"), Some("#16a34a"), px(2.0));
    note.handles.push(target_handle("in", px(0.0), px(48.0)));
    document.insert_node(note).unwrap();

    let mut first_edge = CanvasEdge::new(
        "source-branch",
        CanvasEndpoint::new("source", Some("out")),
        CanvasEndpoint::new("branch", Some("in")),
    );
    first_edge.z_index = 2;
    first_edge.style = style(None, Some("#0969da"), px(2.0));
    first_edge.route = CanvasEdgeRoute::polyline([point(px(316.0), px(132.0))]);
    document.insert_edge(first_edge).unwrap();

    let mut second_edge = CanvasEdge::new(
        "branch-note",
        CanvasEndpoint::new("branch", Some("out")),
        CanvasEndpoint::new("note", Some("in")),
    );
    second_edge.z_index = 2;
    second_edge.style = style(None, Some("#16a34a"), px(2.0));
    second_edge.route =
        CanvasEdgeRoute::polyline([point(px(624.0), px(132.0)), point(px(624.0), px(312.0))]);
    document.insert_edge(second_edge).unwrap();

    document
}

fn node(
    id: &'static str,
    x: open_gpui::Pixels,
    y: open_gpui::Pixels,
    width: open_gpui::Pixels,
    height: open_gpui::Pixels,
) -> CanvasNode {
    let mut node = CanvasNode::new(id, point(x, y), size(width, height));
    node.z_index = 1;
    node
}

fn source_handle(id: &'static str, x: open_gpui::Pixels, y: open_gpui::Pixels) -> CanvasHandle {
    let mut handle = CanvasHandle::new(id, point(x, y));
    handle.role = HandleRole::Source;
    handle
}

fn target_handle(id: &'static str, x: open_gpui::Pixels, y: open_gpui::Pixels) -> CanvasHandle {
    let mut handle = CanvasHandle::new(id, point(x, y));
    handle.role = HandleRole::Target;
    handle
}

fn style(fill: Option<&str>, stroke: Option<&str>, stroke_width: open_gpui::Pixels) -> CanvasStyle {
    CanvasStyle {
        fill: fill.map(str::to_string),
        stroke: stroke.map(str::to_string),
        stroke_width,
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| SmokeView {
                    model: CanvasPaintModel::new(demo_document(), CanvasViewport::default()),
                })
            },
        )
        .expect("failed to open native smoke window");

        cx.activate(true);
    });
}
