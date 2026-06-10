use open_gpui_canvas::{
    EdgeId, HandleId, JsonCanvas, JsonCanvasEndpointShape, JsonCanvasSide, NodeId,
    document_from_json_canvas_str, document_to_json_canvas_string,
};
use serde_json::json;

const NOTES_SAMPLE: &str = include_str!("../../../examples/canvas-notes/assets/sample.canvas");

#[test]
fn imports_canvas_notes_fixture() {
    let document = document_from_json_canvas_str(NOTES_SAMPLE).unwrap();

    assert_eq!(document.node_count(), 5);
    assert_eq!(document.edge_count(), 4);

    let question = document.node(&NodeId::from("research-question")).unwrap();
    assert_eq!(question.kind, "text");
    assert_eq!(question.style.fill.as_deref(), Some("#fff7ed"));
    assert!(
        question
            .data
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap()
            .contains("dense note maps")
    );
    assert_eq!(
        question.data.get("tags"),
        Some(&json!(["canvas", "local-first"]))
    );
    assert!(
        question
            .handle(Some(&HandleId::from("json_canvas:right")))
            .is_some()
    );
    assert!(
        question
            .handle(Some(&HandleId::from("json_canvas:bottom")))
            .is_some()
    );

    let source = document.node(&NodeId::from("source-doc")).unwrap();
    assert_eq!(
        source.data.get("file"),
        Some(&json!(
            "docs/research/canvas-spatial-index-benchmark-results.md"
        ))
    );
    assert_eq!(source.data.get("subpath"), Some(&json!("#summary")));
    assert_eq!(source.data.get("role"), Some(&json!("source")));

    let link = document.node(&NodeId::from("xyflow-reference")).unwrap();
    assert_eq!(link.kind, "link");
    assert_eq!(link.data.get("url"), Some(&json!("https://xyflow.com/")));
    assert_eq!(link.data.get("confidence"), Some(&json!(0.82)));

    let edge = document
        .edge(&EdgeId::from("edge-question-source"))
        .unwrap();
    assert_eq!(edge.source.node_id, NodeId::from("research-question"));
    assert_eq!(
        edge.source.handle_id.as_ref().map(|id| id.as_str()),
        Some("json_canvas:right")
    );
    assert_eq!(edge.target.node_id, NodeId::from("source-doc"));
    assert_eq!(
        edge.target.handle_id.as_ref().map(|id| id.as_str()),
        Some("json_canvas:left")
    );
    assert_eq!(edge.data.get("fromEnd"), Some(&json!("none")));
    assert_eq!(edge.data.get("toEnd"), Some(&json!("arrow")));
    assert_eq!(edge.data.get("label"), Some(&json!("validated by")));
    assert_eq!(edge.data.get("weight"), Some(&json!(2)));
}

#[test]
fn canvas_notes_fixture_round_trips_json_canvas_fields() {
    let document = document_from_json_canvas_str(NOTES_SAMPLE).unwrap();
    let exported = JsonCanvas::from_document(&document).unwrap();

    let group = exported
        .nodes
        .iter()
        .find(|node| node.id == "cluster-margin-notes")
        .unwrap();
    assert_eq!(group.kind, "group");
    assert_eq!(group.label.as_deref(), Some("Margin note research map"));
    assert_eq!(group.extra.get("purpose"), Some(&json!("fixture-group")));

    let link = exported
        .nodes
        .iter()
        .find(|node| node.id == "xyflow-reference")
        .unwrap();
    assert_eq!(link.kind, "link");
    assert_eq!(link.url.as_deref(), Some("https://xyflow.com/"));
    assert_eq!(link.background_style.as_deref(), Some("cover"));
    assert_eq!(link.extra.get("confidence"), Some(&json!(0.82)));

    let edge = exported
        .edges
        .iter()
        .find(|edge| edge.id == "edge-question-source")
        .unwrap();
    assert_eq!(edge.from_side, Some(JsonCanvasSide::Right));
    assert_eq!(edge.from_end, Some(JsonCanvasEndpointShape::None));
    assert_eq!(edge.to_side, Some(JsonCanvasSide::Left));
    assert_eq!(edge.to_end, Some(JsonCanvasEndpointShape::Arrow));
    assert_eq!(edge.label.as_deref(), Some("validated by"));
    assert_eq!(edge.extra.get("weight"), Some(&json!(2)));

    let serialized = document_to_json_canvas_string(&document).unwrap();
    assert!(serialized.contains("\"fromSide\": \"right\""));
    assert!(serialized.contains("\"backgroundStyle\": \"cover\""));

    let reparsed = document_from_json_canvas_str(&serialized).unwrap();
    assert_eq!(reparsed.node_count(), document.node_count());
    assert_eq!(reparsed.edge_count(), document.edge_count());
}
