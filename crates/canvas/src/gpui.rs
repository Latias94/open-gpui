mod frame;
mod input;
mod model;
mod painter;
mod style;
mod view;

pub use frame::*;
pub use input::*;
pub use model::*;
pub use painter::paint_canvas_frame;
pub use view::*;

#[cfg(test)]
use open_gpui::TextAlign;

#[cfg(test)]
mod tests {
    use super::*;
    use super::{
        frame::label_line_clamp,
        style::{
            CanvasResolvedEdgePaintStyle, CanvasResolvedPaintStyle, edge_paint_style,
            node_paint_style, parse_color, shape_paint_style, style_color,
        },
    };
    use crate::{
        CanvasDocument, CanvasEdge, CanvasEdgeKind, CanvasEdgeRenderPolicy, CanvasEdgeRouter,
        CanvasEditor, CanvasEndpoint, CanvasEvent, CanvasHandle, CanvasKey, CanvasKeyModifiers,
        CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNode, CanvasNodeGeometryPolicy,
        CanvasNodeKind, CanvasNodeRenderPolicy, CanvasRecordId, CanvasRoutePath,
        CanvasRouteRequest, CanvasSelection, CanvasSelectionMode, CanvasShape, CanvasShapeKind,
        CanvasShapeRenderPolicy, CanvasSnapAxis, CanvasSnapGuide, CanvasStyle, CanvasTransaction,
        CanvasTransformTarget, CanvasViewport, DocumentCommand, EdgeId, HandleRole, HitTarget,
        PointerButton,
        session::ToolState,
        test_support::{connected_pair_fixture, document_fixture},
        tool::CanvasToolEffect,
    };
    use open_gpui::{
        Bounds, Hsla, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent,
        MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, point, px, rgb, size,
    };

    #[test]
    fn collect_visible_records_culls_and_transforms_bounds() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "inside",
                point(px(60.0), px(10.0)),
                size(px(20.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(200.0), px(10.0)),
                size(px(20.0), px(10.0)),
            ))
            .build();
        let model = CanvasPaintModel::new(
            document,
            CanvasViewport::new(point(px(50.0), px(0.0)), 2.0).unwrap(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(100.0), px(100.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.records.len(), 1);
        assert_eq!(
            frame.records[0].target,
            HitTarget::Node(crate::NodeId::from("inside"))
        );
        assert_eq!(
            frame.records[0].view_bounds,
            Bounds::new(point(px(20.0), px(20.0)), size(px(40.0), px(20.0)))
        );
    }

    #[test]
    fn collect_visible_records_keeps_large_canvas_frame_bounded() {
        let document = large_grid_document(128, 96);
        let total_records = document.node_count();
        let model = CanvasPaintModel::new(
            document,
            CanvasViewport::new(point(px(2_400.0), px(1_800.0)), 1.0).unwrap(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(total_records, 12_288);
        assert!(!frame.records.is_empty());
        assert!(frame.records.len() < 80);
        assert!(frame.records.iter().all(|record| {
            frame
                .visible_document_bounds
                .intersects(&record.document_bounds)
        }));
    }

    #[test]
    fn collect_visible_records_keeps_locked_records_visible() {
        let mut node = CanvasNode::new("locked", point(px(0.0), px(0.0)), size(px(20.0), px(20.0)));
        node.locked = true;
        let document = document_fixture().node(node).build();
        let model = CanvasPaintModel::new(document, CanvasViewport::default());

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.records.len(), 1);
        assert_eq!(
            frame.records[0].target,
            HitTarget::Node(crate::NodeId::from("locked"))
        );
        assert!(frame.records[0].locked);
    }

    #[test]
    fn handles_are_only_collected_when_requested() {
        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(40.0), px(40.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(40.0), px(20.0))));
        let document = document_fixture().node(node).build();
        let model = CanvasPaintModel::new(document, CanvasViewport::default());
        let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));

        let frame = collect_visible_records(&model, canvas_bounds, CanvasPaintOptions::default());
        assert_eq!(frame.records.len(), 1);

        let frame = collect_visible_records(
            &model,
            canvas_bounds,
            CanvasPaintOptions {
                include_handles: true,
                ..CanvasPaintOptions::default()
            },
        );

        assert!(frame.records.iter().any(|record| {
            matches!(
                &record.target,
                HitTarget::Handle { node_id, handle_id }
                    if node_id.as_str() == "node" && handle_id.as_str() == "out"
            )
        }));
    }

    #[test]
    fn paint_model_culls_edges_with_custom_router_geometry() {
        let document = connected_edge_document();
        let model = CanvasPaintModel::new_with_router(
            document,
            CanvasViewport::default(),
            &VerticalDetourRouter,
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(76.0)), size(px(12.0), px(12.0))),
            CanvasPaintOptions::default(),
        );

        assert!(frame.records.iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.document_bounds.origin == point(px(-1.0), px(-1.0))
                && record.document_bounds.size == size(px(32.0), px(87.0))
        }));
        assert_eq!(
            model
                .runtime
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(25.0), px(5.0)),
            ]
        );
    }

    #[test]
    fn edge_paint_geometry_comes_from_runtime_geometry_as_view_path() {
        let document = connected_edge_document();
        let model = CanvasPaintModel::new_with_router(
            document,
            CanvasViewport::new(point(px(0.0), px(0.0)), 2.0).unwrap(),
            &VerticalDetourRouter,
        );
        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(120.0), px(200.0))),
            CanvasPaintOptions::default(),
        );
        let edge_record = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Edge(EdgeId::from("a-b")))
            .unwrap();

        assert_eq!(
            edge_record
                .edge_geometry
                .as_ref()
                .unwrap()
                .view_path
                .document_points(),
            vec![
                point(px(10.0), px(10.0)),
                point(px(10.0), px(160.0)),
                point(px(50.0), px(10.0)),
            ]
        );
    }

    #[test]
    fn paint_model_uses_kind_registry_bounds_in_frame_records() {
        let mut node = CanvasNode::new("wide", point(px(10.0), px(10.0)), size(px(20.0), px(20.0)));
        node.kind = "wide".to_string();
        let document = document_fixture().node(node).build();
        let model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::default(),
            geometry_registry(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        let record = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Node(crate::NodeId::from("wide")))
            .unwrap();
        assert_eq!(
            record.document_bounds,
            Bounds::new(point(px(5.0), px(5.0)), size(px(30.0), px(30.0)))
        );
        assert_eq!(record.view_bounds, record.document_bounds);
    }

    #[test]
    fn paint_frame_carries_kind_label_metadata_for_nodes_and_shapes() {
        let mut node = CanvasNode::new(
            "painted",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        node.kind = "painted-node".to_string();
        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(150.0), px(20.0)), size(px(90.0), px(70.0))),
        );
        shape.kind = "painted-shape".to_string();
        let document = document_fixture().node(node).shape(shape).build();
        let model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::new(point(px(10.0), px(10.0)), 2.0).unwrap(),
            paint_registry(),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(1_000.0), px(1_000.0))),
            CanvasPaintOptions::default(),
        );

        let node_label = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Node(crate::NodeId::from("painted")))
            .and_then(|record| record.label.as_ref())
            .unwrap();
        assert_eq!(node_label.text, "Node label");
        assert_eq!(
            node_label.document_bounds,
            Bounds::new(point(px(18.0), px(28.0)), size(px(84.0), px(64.0)))
        );
        assert_eq!(
            node_label.view_bounds,
            Bounds::new(point(px(16.0), px(36.0)), size(px(168.0), px(128.0)))
        );
        assert_eq!(node_label.color, parse_color("#24292f"));

        let shape_label = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Shape(crate::ShapeId::from("shape")))
            .and_then(|record| record.label.as_ref())
            .unwrap();
        assert_eq!(shape_label.text, "Shape label");
        assert_eq!(
            shape_label.document_bounds,
            Bounds::new(point(px(154.0), px(24.0)), size(px(82.0), px(62.0)))
        );
        assert_eq!(shape_label.color, parse_color("#0969da"));
    }

    #[test]
    fn prepared_frame_preserves_snapshot_when_no_labels_are_visible() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "plain",
                point(px(10.0), px(20.0)),
                size(px(100.0), px(80.0)),
            ))
            .build();
        let model = CanvasPaintModel::new(document, CanvasViewport::default());
        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            CanvasPaintOptions::default(),
        );

        let prepared = CanvasPreparedPaintFrame {
            label_indices: frame.records.iter().map(|_| None).collect(),
            labels: Vec::new(),
            frame,
        };

        assert_eq!(prepared.record_count(), 1);
        assert_eq!(prepared.prepared_label_count(), 0);
        assert!(!prepared.has_prepared_label(0));
        assert_eq!(
            prepared.frame().records[0].target,
            HitTarget::Node("plain".into())
        );
    }

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
    fn selected_records_are_marked_in_paint_frame() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "plain",
                point(px(70.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("selected"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert!(frame.records.iter().any(|record| {
            record.target == HitTarget::Node(crate::NodeId::from("selected")) && record.selected
        }));
        assert!(frame.records.iter().any(|record| {
            record.target == HitTarget::Node(crate::NodeId::from("plain")) && !record.selected
        }));
    }

    #[test]
    fn structurally_selected_records_are_marked_in_paint_frame() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(120.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(20.0), px(20.0)),
                size(px(40.0), px(30.0)),
            ))
            .node(CanvasNode::new(
                "peer",
                point(px(80.0), px(20.0)),
                size(px(40.0), px(30.0)),
            ))
            .edge(CanvasEdge::new(
                "child-peer",
                CanvasEndpoint::new("child", None::<&str>),
                CanvasEndpoint::new("peer", None::<&str>),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(crate::NodeId::from("child")),
                    parent: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
                    member: CanvasRecordId::Node(crate::NodeId::from("peer")),
                },
            ]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Shape(
                crate::ShapeId::from("frame"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(160.0))),
            CanvasPaintOptions::default(),
        );

        let frame_record = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Shape(crate::ShapeId::from("frame")))
            .unwrap();
        assert!(frame_record.selected);
        assert!(!frame_record.structurally_selected);
        for target in [
            HitTarget::Node(crate::NodeId::from("child")),
            HitTarget::Node(crate::NodeId::from("peer")),
            HitTarget::Edge(crate::EdgeId::from("child-peer")),
        ] {
            let record = frame
                .records
                .iter()
                .find(|record| record.target == target)
                .unwrap();
            assert!(!record.selected);
            assert!(record.structurally_selected);
        }

        let overlay = frame.widget_overlay_frame(CanvasWidgetOverlayOptions::selected_records());
        assert_eq!(overlay.len(), 1);
        assert_eq!(
            overlay.placements[0].target,
            HitTarget::Shape(crate::ShapeId::from("frame"))
        );
    }

    #[test]
    fn structural_selection_bounds_do_not_replace_transform_handles() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(160.0), px(20.0)),
                size(px(40.0), px(30.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(crate::NodeId::from("child")),
                parent: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
            }]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Shape(
                crate::ShapeId::from("frame"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(140.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.structural_selection_bounds,
            Some(Bounds::new(
                point(px(160.0), px(20.0)),
                size(px(40.0), px(30.0))
            ))
        );
        assert_eq!(frame.interaction.transform_handles.len(), 4);
        assert!(frame.interaction.transform_handles.iter().any(|handle| {
            handle.target == CanvasTransformTarget::Shape(crate::ShapeId::from("frame"))
                && handle.handle == crate::CanvasResizeHandle::BottomRight
                && handle.view_bounds.contains(&point(px(100.0), px(80.0)))
        }));
        assert!(
            !frame
                .interaction
                .transform_handles
                .iter()
                .any(|handle| handle.view_bounds.contains(&point(px(200.0), px(80.0))))
        );
    }

    #[test]
    fn child_only_selection_has_no_structural_selection_bounds() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(120.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(20.0), px(20.0)),
                size(px(40.0), px(30.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(crate::NodeId::from("child")),
                parent: CanvasRecordId::Shape(crate::ShapeId::from("frame")),
            }]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Shape(
                crate::ShapeId::from("frame"),
            )))
            .unwrap();
        editor
            .apply_tool_effect(CanvasToolEffect::ToggleSelection(HitTarget::Shape(
                crate::ShapeId::from("frame"),
            )))
            .unwrap();
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("child"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(160.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.interaction.structural_selection_bounds, None);
    }

    #[test]
    fn widget_overlay_frame_uses_selected_visible_record_placements() {
        let mut selected = CanvasNode::new(
            "selected",
            point(px(10.0), px(10.0)),
            size(px(40.0), px(20.0)),
        );
        selected.z_index = 5;
        let mut locked = CanvasNode::new(
            "locked",
            point(px(70.0), px(10.0)),
            size(px(40.0), px(20.0)),
        );
        locked.locked = true;
        let mut hidden = CanvasNode::new(
            "hidden",
            point(px(130.0), px(10.0)),
            size(px(40.0), px(20.0)),
        );
        hidden.hidden = true;
        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(10.0), px(60.0)), size(px(60.0), px(30.0))),
        );
        shape.z_index = 4;

        let document = document_fixture()
            .node(selected)
            .node(locked)
            .node(hidden)
            .shape(shape)
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effects([
                CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("selected"))),
                CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("locked"))),
                CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("hidden"))),
                CanvasToolEffect::AddSelection(HitTarget::Shape(crate::ShapeId::from("shape"))),
            ])
            .unwrap();
        let model = CanvasPaintModel::from(&editor);
        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(120.0))),
            CanvasPaintOptions {
                include_hidden: true,
                ..CanvasPaintOptions::default()
            },
        );

        let overlay = frame.widget_overlay_frame(
            CanvasWidgetOverlayOptions::selected_records()
                .with_hit_priority(CanvasWidgetOverlayHitPriority::CanvasFirst),
        );

        assert_eq!(overlay.len(), 2);
        assert_eq!(
            overlay.placements[0],
            CanvasWidgetOverlayPlacement {
                target: HitTarget::Shape(crate::ShapeId::from("shape")),
                document_bounds: Bounds::new(point(px(10.0), px(60.0)), size(px(60.0), px(30.0))),
                view_bounds: Bounds::new(point(px(10.0), px(60.0)), size(px(60.0), px(30.0))),
                z_index: 4,
                hit_priority: CanvasWidgetOverlayHitPriority::CanvasFirst,
            }
        );
        assert_eq!(
            overlay.placements[1],
            CanvasWidgetOverlayPlacement {
                target: HitTarget::Node(crate::NodeId::from("selected")),
                document_bounds: Bounds::new(point(px(10.0), px(10.0)), size(px(40.0), px(20.0))),
                view_bounds: Bounds::new(point(px(10.0), px(10.0)), size(px(40.0), px(20.0))),
                z_index: 5,
                hit_priority: CanvasWidgetOverlayHitPriority::CanvasFirst,
            }
        );

        let including_locked = frame.widget_overlay_frame(
            CanvasWidgetOverlayOptions::selected_nodes()
                .with_locked(true)
                .with_hit_priority(CanvasWidgetOverlayHitPriority::WidgetFirst),
        );
        assert_eq!(including_locked.len(), 2);
        assert!(including_locked.placements.iter().any(|placement| {
            placement.target == HitTarget::Node(crate::NodeId::from("locked"))
                && placement.hit_priority == CanvasWidgetOverlayHitPriority::WidgetFirst
        }));
    }

    #[test]
    fn widget_overlay_bounds_come_from_paint_frame_geometry() {
        let mut node = CanvasNode::new("wide", point(px(10.0), px(10.0)), size(px(20.0), px(20.0)));
        node.kind = "wide".to_string();
        let document = document_fixture().node(node).build();
        let mut editor =
            CanvasEditor::try_new_with_kind_registry(document, geometry_registry()).unwrap();
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("wide"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);
        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );
        let paint_record = frame
            .records
            .iter()
            .find(|record| record.target == HitTarget::Node(crate::NodeId::from("wide")))
            .unwrap();

        let overlay =
            collect_widget_overlay_frame(&frame, CanvasWidgetOverlayOptions::selected_nodes());

        assert_eq!(overlay.len(), 1);
        assert_eq!(
            overlay.placements[0].document_bounds,
            paint_record.document_bounds
        );
        assert_eq!(overlay.placements[0].view_bounds, paint_record.view_bounds);
        assert_eq!(
            overlay.placements[0].document_bounds,
            Bounds::new(point(px(5.0), px(5.0)), size(px(30.0), px(30.0)))
        );
    }

    #[test]
    fn paint_model_from_editor_keeps_an_immutable_editor_snapshot() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "stable",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        let snapshot = CanvasPaintModel::from(&editor);

        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "after-snapshot",
                point(px(70.0), px(10.0)),
                size(px(40.0), px(20.0)),
            )))
            .unwrap();

        let canvas_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(160.0), px(80.0)));
        let old_frame =
            collect_visible_records(&snapshot, canvas_bounds, CanvasPaintOptions::default());
        let new_frame = collect_visible_records(
            &CanvasPaintModel::from(&editor),
            canvas_bounds,
            CanvasPaintOptions::default(),
        );

        assert_eq!(snapshot.document().node_count(), 1);
        assert!(
            !snapshot
                .document()
                .contains_node(&crate::NodeId::from("after-snapshot"))
        );
        assert!(old_frame.records.iter().all(|record| {
            record.target != HitTarget::Node(crate::NodeId::from("after-snapshot"))
        }));
        assert!(new_frame.records.iter().any(|record| {
            record.target == HitTarget::Node(crate::NodeId::from("after-snapshot"))
        }));
    }

    #[test]
    fn paint_model_from_editor_keeps_an_immutable_session_snapshot() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        let viewport = CanvasViewport::new(point(px(5.0), px(-3.0)), 1.5).unwrap();
        editor.set_viewport(viewport);
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("selected"),
            )))
            .unwrap();
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Selecting {
                origin: point(px(0.0), px(0.0)),
                current: point(px(20.0), px(20.0)),
                selection_mode: CanvasSelectionMode::Replace,
                base_selection: CanvasSelection::default(),
            }))
            .unwrap();

        let snapshot = CanvasPaintModel::from(&editor);

        editor.set_viewport(CanvasViewport::default());
        editor
            .apply_tool_effect(CanvasToolEffect::ClearSelection)
            .unwrap();
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Idle))
            .unwrap();

        assert_eq!(snapshot.viewport(), viewport);
        assert!(
            snapshot
                .interaction()
                .selection()
                .contains_node(&crate::NodeId::from("selected"))
        );
        assert!(matches!(
            snapshot.interaction().state(),
            CanvasPaintInteractionState::Selecting { .. }
        ));

        let new_snapshot = CanvasPaintModel::from(&editor);
        assert_eq!(new_snapshot.viewport(), CanvasViewport::default());
        assert!(new_snapshot.interaction().selection().is_empty());
        assert!(matches!(
            new_snapshot.interaction().state(),
            CanvasPaintInteractionState::Idle
        ));
    }

    #[test]
    fn selected_records_add_transform_handles_to_paint_frame() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Node(
                crate::NodeId::from("selected"),
            )))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(frame.interaction.transform_handles.len(), 4);
        assert!(frame.interaction.transform_handles.iter().any(|handle| {
            handle.target == CanvasTransformTarget::Node(crate::NodeId::from("selected"))
                && handle.handle == crate::CanvasResizeHandle::BottomRight
                && handle.view_bounds.contains(&point(px(50.0), px(30.0)))
        }));
    }

    #[test]
    fn translating_state_adds_snap_guides_to_paint_frame() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .build();
        let mut selection = CanvasSelection::default();
        selection.insert_node(crate::NodeId::from("selected"));
        let model = CanvasPaintModel::new(
            document,
            CanvasViewport::new(point(px(10.0), px(20.0)), 2.0).unwrap(),
        )
        .with_interaction(
            CanvasPaintInteraction::new(selection).with_internal_tool_state(
                ToolState::Translating {
                    origin: point(px(10.0), px(10.0)),
                    last: point(px(20.0), px(20.0)),
                    constraint_axis: None,
                    node_ids: vec![crate::NodeId::from("selected")],
                    shape_ids: Vec::new(),
                    snap_guides: vec![CanvasSnapGuide {
                        axis: CanvasSnapAxis::Horizontal,
                        document_start: point(px(40.0), px(10.0)),
                        document_end: point(px(40.0), px(90.0)),
                    }],
                },
            ),
        );

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.snap_guides,
            vec![CanvasPaintSnapGuide {
                axis: CanvasSnapAxis::Horizontal,
                view_start: point(px(60.0), px(-20.0)),
                view_end: point(px(60.0), px(140.0)),
            }]
        );
    }

    #[test]
    fn interaction_feedback_can_be_disabled() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "selected",
                point(px(10.0), px(10.0)),
                size(px(40.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effects([
                CanvasToolEffect::AddSelection(HitTarget::Node(crate::NodeId::from("selected"))),
                CanvasToolEffect::SetState(ToolState::Selecting {
                    origin: point(px(10.0), px(10.0)),
                    current: point(px(40.0), px(50.0)),
                    selection_mode: CanvasSelectionMode::Replace,
                    base_selection: CanvasSelection::default(),
                }),
            ])
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(100.0))),
            CanvasPaintOptions {
                include_interaction_feedback: false,
                ..CanvasPaintOptions::default()
            },
        );

        assert!(frame.records.iter().all(|record| !record.selected));
        assert_eq!(frame.interaction, CanvasPaintInteractionFrame::default());
    }

    #[test]
    fn selecting_state_adds_selection_bounds_feedback() {
        let model = CanvasPaintModel::new(
            document_fixture().build(),
            CanvasViewport::new(point(px(10.0), px(20.0)), 2.0).unwrap(),
        )
        .with_interaction(CanvasPaintInteraction::default().with_internal_tool_state(
            ToolState::Selecting {
                origin: point(px(40.0), px(80.0)),
                current: point(px(20.0), px(50.0)),
                selection_mode: CanvasSelectionMode::Replace,
                base_selection: CanvasSelection::default(),
            },
        ));

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.selection_bounds,
            Some(Bounds::new(
                point(px(20.0), px(60.0)),
                size(px(40.0), px(60.0))
            ))
        );
    }

    #[test]
    fn connecting_state_adds_connection_preview_feedback() {
        let mut node = CanvasNode::new(
            "source",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        node.handles
            .push(CanvasHandle::new("out", point(px(100.0), px(40.0))));
        let document = document_fixture().node(node).build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(180.0), px(120.0)),
            }))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(110.0), px(60.0)),
                target_view_position: point(px(180.0), px(120.0)),
            })
        );
    }

    #[test]
    fn connecting_preview_uses_kind_registry_endpoint_positions() {
        let mut source =
            CanvasNode::new("source", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        source.kind = "wide".to_string();
        let mut source_handle = CanvasHandle::new("out", point(px(10.0), px(5.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target =
            CanvasNode::new("target", point(px(60.0), px(0.0)), size(px(10.0), px(10.0)));
        target.kind = "wide".to_string();
        let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(5.0)));
        target_handle.role = HandleRole::Target;
        target.handles.push(target_handle);

        let document = document_fixture().node(source).node(target).build();
        let model = CanvasPaintModel::new_with_kind_registry(
            document,
            CanvasViewport::default(),
            geometry_registry(),
        )
        .with_interaction(CanvasPaintInteraction::default().with_internal_tool_state(
            ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(40.0), px(5.0)),
            },
        ));

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(30.0), px(5.0)),
                target_view_position: point(px(40.0), px(5.0)),
            })
        );
    }

    #[test]
    fn connecting_preview_snaps_to_valid_target_handle() {
        let mut source = CanvasNode::new(
            "source",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(40.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target = CanvasNode::new(
            "target",
            point(px(200.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(40.0)));
        target_handle.role = HandleRole::Target;
        target.handles.push(target_handle);

        let document = document_fixture().node(source).node(target).build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(204.0), px(64.0)),
            }))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(140.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(110.0), px(60.0)),
                target_view_position: point(px(200.0), px(60.0)),
            })
        );
    }

    #[test]
    fn connecting_preview_does_not_snap_to_invalid_target_handle() {
        let mut source = CanvasNode::new(
            "source",
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(40.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target = CanvasNode::new(
            "target",
            point(px(200.0), px(20.0)),
            size(px(100.0), px(80.0)),
        );
        let mut invalid_target_handle = CanvasHandle::new("out", point(px(0.0), px(40.0)));
        invalid_target_handle.role = HandleRole::Source;
        target.handles.push(invalid_target_handle);

        let document = document_fixture().node(source).node(target).build();
        let mut editor = CanvasEditor::new(document);
        editor
            .apply_tool_effect(CanvasToolEffect::SetState(ToolState::Connecting {
                source: CanvasEndpoint::new("source", Some("out")),
                current: point(px(204.0), px(64.0)),
            }))
            .unwrap();
        let model = CanvasPaintModel::from(&editor);

        let frame = collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(140.0))),
            CanvasPaintOptions::default(),
        );

        assert_eq!(
            frame.interaction.connection_preview,
            Some(CanvasPaintConnectionPreview {
                source_view_position: point(px(110.0), px(60.0)),
                target_view_position: point(px(204.0), px(64.0)),
            })
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

    #[test]
    fn input_mapper_localizes_pointer_events() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ));

        assert_eq!(
            mapper.mouse_down(&MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(120.0), px(80.0)),
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..MouseDownEvent::default()
            }),
            Some(CanvasEvent::PointerDown {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
        );
        assert_eq!(
            mapper.mouse_up(&MouseUpEvent {
                button: MouseButton::Right,
                position: point(px(140.0), px(90.0)),
                ..MouseUpEvent::default()
            }),
            Some(CanvasEvent::PointerUp {
                position: point(px(40.0), px(40.0)),
                button: PointerButton::Secondary,
                modifiers: CanvasKeyModifiers::default(),
            })
        );
        assert_eq!(
            mapper.mouse_move(&MouseMoveEvent {
                position: point(px(150.0), px(95.0)),
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..MouseMoveEvent::default()
            }),
            Some(CanvasEvent::PointerMove {
                position: point(px(50.0), px(45.0)),
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
        );
    }

    #[test]
    fn input_mapper_filters_outside_or_unsupported_pointer_events() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ));

        assert_eq!(
            mapper.mouse_down(&MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(20.0), px(80.0)),
                ..MouseDownEvent::default()
            }),
            None
        );
        assert_eq!(
            mapper.mouse_down(&MouseDownEvent {
                button: MouseButton::Navigate(open_gpui::NavigationDirection::Back),
                position: point(px(120.0), px(80.0)),
                ..MouseDownEvent::default()
            }),
            None
        );
    }

    #[test]
    fn editor_input_mapper_keeps_drag_events_after_pointer_leaves_bounds() {
        let mapper = CanvasEditorInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ))
        .with_pointer_interacting(true);

        assert_eq!(
            mapper.mouse_move(&MouseMoveEvent {
                position: point(px(20.0), px(80.0)),
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..MouseMoveEvent::default()
            }),
            Some(CanvasEvent::PointerMove {
                position: point(px(-80.0), px(30.0)),
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
        );
        assert_eq!(
            mapper.mouse_up(&MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(20.0), px(80.0)),
                ..MouseUpEvent::default()
            }),
            Some(CanvasEvent::PointerUp {
                position: point(px(-80.0), px(30.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
        );
    }

    #[test]
    fn editor_input_mapper_filters_outside_events_when_not_dragging() {
        let mapper = CanvasEditorInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ));

        assert_eq!(
            mapper.mouse_move(&MouseMoveEvent {
                position: point(px(20.0), px(80.0)),
                ..MouseMoveEvent::default()
            }),
            None
        );
        assert_eq!(
            mapper.mouse_up(&MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(20.0), px(80.0)),
                ..MouseUpEvent::default()
            }),
            None
        );
    }

    #[test]
    fn input_mapper_converts_scroll_delta_to_canvas_wheel() {
        let mapper = CanvasInputMapper::new(Bounds::new(
            point(px(100.0), px(50.0)),
            size(px(200.0), px(120.0)),
        ))
        .with_line_height(px(20.0));

        assert_eq!(
            mapper.scroll_wheel(&ScrollWheelEvent {
                position: point(px(120.0), px(80.0)),
                delta: ScrollDelta::Lines(point(1.0, -2.0)),
                ..ScrollWheelEvent::default()
            }),
            Some(CanvasEvent::Wheel {
                delta: point(px(20.0), px(-40.0)),
            })
        );
        assert_eq!(
            mapper.scroll_wheel(&ScrollWheelEvent {
                position: point(px(20.0), px(80.0)),
                delta: ScrollDelta::Pixels(point(px(1.0), px(2.0))),
                ..ScrollWheelEvent::default()
            }),
            None
        );
    }

    #[test]
    fn input_mapper_converts_key_down_events() {
        assert_eq!(
            CanvasInputMapper::key_down_event(&KeyDownEvent {
                keystroke: Keystroke::parse("backspace").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }),
            CanvasEvent::KeyDown {
                key: CanvasKey::Backspace,
                modifiers: CanvasKeyModifiers::default(),
                repeat: false,
            }
        );
        assert_eq!(
            CanvasInputMapper::key_down_event(&KeyDownEvent {
                keystroke: Keystroke::parse("ctrl-a").unwrap(),
                is_held: true,
                prefer_character_input: false,
            }),
            CanvasEvent::KeyDown {
                key: CanvasKey::Character("a".to_string()),
                modifiers: CanvasKeyModifiers {
                    control: true,
                    ..CanvasKeyModifiers::default()
                },
                repeat: true,
            }
        );
        assert_eq!(
            CanvasInputMapper::key_down_event(&KeyDownEvent {
                keystroke: Keystroke::parse("escape").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }),
            CanvasEvent::Cancel
        );
    }

    fn large_grid_document(columns: usize, rows: usize) -> CanvasDocument {
        let mut fixture = document_fixture();

        for row in 0..rows {
            for column in 0..columns {
                fixture.add_node(CanvasNode::new(
                    format!("node-{row}-{column}"),
                    point(px(column as f32 * 160.0), px(row as f32 * 120.0)),
                    size(px(96.0), px(56.0)),
                ));
            }
        }

        fixture.build()
    }

    fn connected_edge_document() -> CanvasDocument {
        connected_pair_fixture().build()
    }

    fn geometry_registry() -> CanvasKindRegistry {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "wide",
            CanvasNodeKind::new().with_geometry_policy(WideNodeKind),
        );
        registry
    }

    fn paint_registry() -> CanvasKindRegistry {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "painted-node",
            CanvasNodeKind::new().with_render_policy(PaintedNodeKind),
        );
        registry.register_edge_kind(
            "painted-edge",
            CanvasEdgeKind::new().with_render_policy(PaintedEdgeKind),
        );
        registry.register_shape_kind(
            "painted-shape",
            CanvasShapeKind::new().with_render_policy(PaintedShapeKind),
        );
        registry
    }

    struct WideNodeKind;

    impl CanvasNodeGeometryPolicy for WideNodeKind {
        fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<open_gpui::Pixels>> {
            Some(node.bounds().dilate(px(5.0)))
        }

        fn handle_position(
            &self,
            node: &CanvasNode,
            handle_id: &crate::HandleId,
        ) -> Option<open_gpui::Point<open_gpui::Pixels>> {
            match handle_id.as_str() {
                "out" => Some(point(
                    node.position.x + node.size.width + px(20.0),
                    node.position.y + px(5.0),
                )),
                "in" => Some(point(node.position.x - px(20.0), node.position.y + px(5.0))),
                _ => None,
            }
        }
    }

    struct PaintedNodeKind;

    impl CanvasNodeRenderPolicy for PaintedNodeKind {
        fn node_paint(&self, _node: &CanvasNode) -> Option<CanvasKindPaint> {
            Some(CanvasKindPaint {
                fill: Some("#fff8c5".to_string()),
                stroke: Some("#bf8700".to_string()),
                stroke_width: Some(px(2.0)),
                corner_radius: Some(px(10.0)),
            })
        }

        fn node_label(&self, _node: &CanvasNode) -> Option<CanvasKindLabel> {
            Some(
                CanvasKindLabel::new("Node label")
                    .with_inset(px(8.0))
                    .with_color("#24292f"),
            )
        }
    }

    struct PaintedEdgeKind;

    impl CanvasEdgeRenderPolicy for PaintedEdgeKind {
        fn edge_paint(&self, _edge: &CanvasEdge) -> Option<CanvasKindPaint> {
            Some(CanvasKindPaint {
                fill: None,
                stroke: Some("#d1242f".to_string()),
                stroke_width: Some(px(5.0)),
                corner_radius: None,
            })
        }
    }

    struct PaintedShapeKind;

    impl CanvasShapeRenderPolicy for PaintedShapeKind {
        fn shape_paint(&self, _shape: &CanvasShape) -> Option<CanvasKindPaint> {
            Some(CanvasKindPaint {
                fill: Some("#ddf4ff".to_string()),
                stroke: Some("#0969da".to_string()),
                stroke_width: Some(px(3.0)),
                corner_radius: Some(px(4.0)),
            })
        }

        fn shape_label(&self, _shape: &CanvasShape) -> Option<CanvasKindLabel> {
            Some(
                CanvasKindLabel::new("Shape label")
                    .with_inset(px(4.0))
                    .with_color("#0969da"),
            )
        }
    }

    struct VerticalDetourRouter;

    impl CanvasEdgeRouter for VerticalDetourRouter {
        fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
            CanvasRoutePath::polyline([
                request.source,
                point(request.source.x, px(80.0)),
                request.target,
            ])
        }
    }
}
