use super::*;

#[test]
fn paint_theme_defaults_include_bounded_label_text() {
    let theme = CanvasPaintTheme::default();

    assert_eq!(theme.label_color, parse_color("#24292f").unwrap());
    assert_eq!(theme.label_font_size, px(14.0));
    assert_eq!(theme.label_line_height, px(18.0));
    assert_eq!(theme.label_line_clamp, Some(3));
    assert_eq!(theme.label_text_align, TextAlign::Center);
}

#[test]
fn label_line_clamp_uses_theme_and_available_height() {
    let mut theme = CanvasPaintTheme {
        label_line_height: px(10.0),
        label_line_clamp: Some(5),
        ..CanvasPaintTheme::default()
    };
    let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(26.0)));

    assert_eq!(label_line_clamp(theme, bounds), Some(2));

    theme.label_line_clamp = None;
    assert_eq!(label_line_clamp(theme, bounds), Some(2));

    theme.label_line_clamp = Some(0);
    assert_eq!(label_line_clamp(theme, bounds), Some(1));
}

#[test]
fn paint_style_uses_record_style_then_kind_fallback_then_theme() {
    let mut node = CanvasNode::new(
        "painted",
        point(px(0.0), px(0.0)),
        size(px(100.0), px(80.0)),
    );
    node.kind = "painted-node".to_string();
    let mut shape = CanvasShape::new(
        "shape",
        Bounds::new(point(px(120.0), px(0.0)), size(px(100.0), px(80.0))),
    );
    shape.kind = "painted-shape".to_string();
    let mut edge = CanvasEdge::new(
        "edge",
        CanvasEndpoint::new("source", None::<&str>),
        CanvasEndpoint::new("target", None::<&str>),
    );
    edge.kind = "painted-edge".to_string();
    let document = document_fixture()
        .node(node)
        .shape(shape)
        .node(CanvasNode::new(
            "source",
            point(px(0.0), px(160.0)),
            size(px(100.0), px(80.0)),
        ))
        .node(CanvasNode::new(
            "target",
            point(px(180.0), px(160.0)),
            size(px(100.0), px(80.0)),
        ))
        .edge(edge)
        .build();
    let model = CanvasPaintModel::new_with_kind_registry(
        document,
        CanvasViewport::default(),
        paint_registry(),
    );
    let theme = CanvasPaintTheme::default();

    let node = model
        .document()
        .node(&crate::NodeId::from("painted"))
        .unwrap();
    let node_style = node_paint_style(&model, node, theme);
    assert_eq!(node_style.fill, parse_color("#fff8c5").unwrap());
    assert_eq!(node_style.stroke, parse_color("#bf8700").unwrap());
    assert_eq!(node_style.stroke_width, px(2.0));
    assert_eq!(node_style.corner_radius, px(10.0));

    let shape = model
        .document()
        .shape(&crate::ShapeId::from("shape"))
        .unwrap();
    let shape_style = shape_paint_style(&model, shape, theme);
    assert_eq!(shape_style.fill, parse_color("#ddf4ff").unwrap());
    assert_eq!(shape_style.stroke, parse_color("#0969da").unwrap());
    assert_eq!(shape_style.stroke_width, px(3.0));
    assert_eq!(shape_style.corner_radius, px(4.0));

    let edge = model.document().edge(&crate::EdgeId::from("edge")).unwrap();
    let edge_style = edge_paint_style(&model, edge, theme);
    assert_eq!(edge_style.stroke, parse_color("#d1242f").unwrap());
    assert_eq!(edge_style.stroke_width, px(5.0));

    let mut explicit = node.clone();
    explicit.style = CanvasStyle {
        fill: Some("#6f42c1".to_string()),
        stroke: Some("#1a7f37".to_string()),
        stroke_width: px(7.0),
    };
    let explicit_style = node_paint_style(&model, &explicit, theme);
    assert_eq!(explicit_style.fill, parse_color("#6f42c1").unwrap());
    assert_eq!(explicit_style.stroke, parse_color("#1a7f37").unwrap());
    assert_eq!(explicit_style.stroke_width, px(7.0));
    assert_eq!(explicit_style.corner_radius, px(10.0));

    let mut explicit_edge = edge.clone();
    explicit_edge.style.stroke = Some("#6f42c1".to_string());
    explicit_edge.style.stroke_width = px(9.0);
    let explicit_edge_style = edge_paint_style(&model, &explicit_edge, theme);
    assert_eq!(explicit_edge_style.stroke, parse_color("#6f42c1").unwrap());
    assert_eq!(explicit_edge_style.stroke_width, px(9.0));

    let unknown = CanvasNode::new(
        "unknown",
        point(px(240.0), px(0.0)),
        size(px(100.0), px(80.0)),
    );
    assert_eq!(
        node_paint_style(&model, &unknown, theme),
        CanvasResolvedPaintStyle {
            fill: theme.node_fill,
            stroke: theme.node_stroke,
            stroke_width: theme.node_stroke_width,
            corner_radius: theme.node_corner_radius,
        }
    );

    let unknown_edge = CanvasEdge::new(
        "unknown-edge",
        CanvasEndpoint::new("source", None::<&str>),
        CanvasEndpoint::new("target", None::<&str>),
    );
    assert_eq!(
        edge_paint_style(&model, &unknown_edge, theme),
        CanvasResolvedEdgePaintStyle {
            stroke: theme.edge_stroke,
            stroke_width: theme.edge_stroke_width,
        }
    );
}

#[test]
fn parses_style_hex_colors() {
    assert_eq!(
        style_color(&Some("#0969da".to_string())),
        Some(Hsla::from(rgb(0x0969da)))
    );
    assert_eq!(style_color(&Some("not-a-color".to_string())), None);
}
