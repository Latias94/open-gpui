use super::*;

#[test]
fn connect_tool_ignores_node_body_endpoints_by_default() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .node(CanvasNode::new(
            "b",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(editor.document().edge_count(), 0);
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(editor.take_connection_release(), None);
}

#[test]
fn connect_tool_creates_edge_between_policy_node_endpoints() {
    let mut a = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    a.kind = "whole-node".to_owned();
    let mut b = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    b.kind = "whole-node".to_owned();

    let document = document_fixture().node(a).node(b).build();
    let mut registry = CanvasKindRegistry::default();
    registry.register_node_kind("whole-node", whole_node_endpoint_kind());

    let mut editor = CanvasEditor::new(document);
    editor.set_kind_registry(registry).unwrap();
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(editor.document().edge_count(), 1);
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::Connected(CanvasConnectedRelease {
            source: CanvasEndpoint::new("a", None::<&str>),
            target: CanvasEndpoint::new("b", None::<&str>),
            edge_id: EdgeId::from("a->b:0"),
            position: point(px(210.0), px(10.0)),
        }))
    );
}

#[test]
fn connect_tool_reports_dropped_release_for_empty_canvas() {
    use crate::{CanvasHandle, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);
    let target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(320.0), px(180.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(editor.document().edge_count(), 0);
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::Dropped(
            CanvasDroppedConnectionRelease {
                source: CanvasEndpoint::new("a", Some("out")),
                position: point(px(320.0), px(180.0)),
            }
        ))
    );
    assert_eq!(editor.take_connection_release(), None);
}

#[test]
fn connect_tool_ignores_locked_endpoints() {
    let mut locked = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    locked.locked = true;
    let document = document_fixture()
        .node(CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .node(locked)
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(10.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(editor.document().edge_count() == 0);
    assert_eq!(editor.history().undo_depth(), 0);
}

#[test]
fn connect_tool_uses_handles_when_available() {
    use crate::{CanvasHandle, HandleId, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(200.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let edge = editor.document().edges().next().unwrap();
    assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
    assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
}

#[test]
fn pointer_owner_prioritizes_source_handle_before_node_drag() {
    use crate::{CanvasHandle, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let document = document_fixture().node(source).build();
    let editor = CanvasEditor::new(document);

    assert_eq!(
        editor
            .reducer_context()
            .pointer_owner_at(point(px(100.0), px(50.0))),
        context::CanvasPointerOwner::ConnectionSource(CanvasEndpoint::new("a", Some("out"))),
    );
}

#[test]
fn pointer_owner_classifies_node_body_and_empty_pane() {
    let node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let document = document_fixture().node(node).build();
    let editor = CanvasEditor::new(document);

    assert_eq!(
        editor
            .reducer_context()
            .pointer_owner_at(point(px(50.0), px(50.0))),
        context::CanvasPointerOwner::NodeDrag(HitTarget::Node(NodeId::from("a"))),
    );
    assert_eq!(
        editor
            .reducer_context()
            .pointer_owner_at(point(px(150.0), px(150.0))),
        context::CanvasPointerOwner::Pane,
    );
}

#[test]
fn select_tool_starts_connection_from_source_handle_before_node_drag() {
    use crate::{CanvasHandle, HandleId, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    assert_eq!(
        editor.connection_drag_state(),
        Some(CanvasConnectionDragState {
            source: CanvasEndpoint::new("a", Some("out")),
            current: point(px(100.0), px(50.0)),
        })
    );

    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(200.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let edge = editor.document().edges().next().unwrap();
    assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
    assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::Connected(CanvasConnectedRelease {
            source: CanvasEndpoint::new("a", Some("out")),
            target: CanvasEndpoint::new("b", Some("in")),
            edge_id: EdgeId::from("a->b:0"),
            position: point(px(200.0), px(50.0)),
        }))
    );
}

#[test]
fn select_tool_reconnects_selected_edge_target_handle() {
    use crate::{CanvasHandle, HandleId, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut first_target =
        CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut first_target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    first_target_handle.role = HandleRole::Target;
    first_target.handles.push(first_target_handle);

    let mut second_target =
        CanvasNode::new("c", point(px(400.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut second_target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    second_target_handle.role = HandleRole::Target;
    second_target.handles.push(second_target_handle);

    let document = document_fixture()
        .node(source)
        .node(first_target)
        .node(second_target)
        .edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", Some("out")),
            CanvasEndpoint::new("b", Some("in")),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Edge(
            EdgeId::from("edge"),
        )))
        .unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(200.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(400.0), px(50.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(400.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let edge = editor.document().edge(&EdgeId::from("edge")).unwrap();
    assert_eq!(edge.source.node_id, NodeId::from("a"));
    assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
    assert_eq!(edge.target.node_id, NodeId::from("c"));
    assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
    assert_eq!(editor.document().edge_count(), 1);
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::Reconnected(
            CanvasReconnectedRelease {
                edge_id: EdgeId::from("edge"),
                endpoint: CanvasConnectionEndpointRole::Target,
                fixed: CanvasEndpoint::new("a", Some("out")),
                replacement: CanvasEndpoint::new("c", Some("in")),
                position: point(px(400.0), px(50.0)),
            }
        ))
    );
}

#[test]
fn select_tool_reconnects_selected_edge_source_handle() {
    use crate::{CanvasHandle, HandleId, HandleRole};

    let mut first_source =
        CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut first_source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    first_source_handle.role = HandleRole::Source;
    first_source.handles.push(first_source_handle);

    let mut second_source =
        CanvasNode::new("c", point(px(-200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut second_source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    second_source_handle.role = HandleRole::Source;
    second_source.handles.push(second_source_handle);

    let mut target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture()
        .node(first_source)
        .node(second_source)
        .node(target)
        .edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", Some("out")),
            CanvasEndpoint::new("b", Some("in")),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Edge(
            EdgeId::from("edge"),
        )))
        .unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerMove {
            position: point(px(-100.0), px(50.0)),
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(-100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let edge = editor.document().edge(&EdgeId::from("edge")).unwrap();
    assert_eq!(edge.source.node_id, NodeId::from("c"));
    assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
    assert_eq!(edge.target.node_id, NodeId::from("b"));
    assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
    assert_eq!(editor.document().edge_count(), 1);
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::Reconnected(
            CanvasReconnectedRelease {
                edge_id: EdgeId::from("edge"),
                endpoint: CanvasConnectionEndpointRole::Source,
                fixed: CanvasEndpoint::new("b", Some("in")),
                replacement: CanvasEndpoint::new("c", Some("out")),
                position: point(px(-100.0), px(50.0)),
            }
        ))
    );
}

#[test]
fn select_tool_reconnect_pointer_down_clears_stale_release() {
    use crate::{CanvasHandle, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
    target_handle.role = HandleRole::Target;
    target.handles.push(target_handle);

    let document = document_fixture()
        .node(source)
        .node(target)
        .edge(CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", Some("out")),
            CanvasEndpoint::new("b", Some("in")),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor
        .apply_tool_effect(CanvasToolEffect::AddSelection(HitTarget::Edge(
            EdgeId::from("edge"),
        )))
        .unwrap();
    editor
        .apply_tool_effect(CanvasToolEffect::SetConnectionRelease(Some(
            CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                reason: CanvasConnectionRejectReason::InvalidTarget,
                source: None,
                edge_id: Some(EdgeId::from("edge")),
                endpoint: Some(CanvasConnectionEndpointRole::Target),
                position: point(px(999.0), px(999.0)),
            }),
        )))
        .unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(200.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert_eq!(editor.take_connection_release(), None);
}

#[test]
fn connect_tool_exposes_read_only_drag_state() {
    use crate::{CanvasHandle, HandleId, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);
    let target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    let drag = editor
        .connection_drag_state()
        .expect("connect drag should expose source state");
    assert_eq!(drag.source.node_id, NodeId::from("a"));
    assert_eq!(drag.source.handle_id, Some(HandleId::from("out")));
    assert_eq!(drag.current, point(px(100.0), px(50.0)));
}

#[test]
fn connect_tool_does_not_start_from_target_only_handle() {
    use crate::{CanvasHandle, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut target_only = CanvasHandle::new("in", point(px(100.0), px(50.0)));
    target_only.role = HandleRole::Target;
    source.handles.push(target_only);
    let target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));

    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(210.0), px(10.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(matches!(editor.session.state, ToolState::Idle));
    assert!(editor.document().edge_count() == 0);
    assert_eq!(editor.history().undo_depth(), 0);
    assert_eq!(
        editor.take_connection_release(),
        Some(CanvasConnectionRelease::Rejected(
            CanvasRejectedConnectionRelease {
                reason: CanvasConnectionRejectReason::InvalidSource,
                source: None,
                edge_id: None,
                endpoint: Some(CanvasConnectionEndpointRole::Source),
                position: point(px(100.0), px(50.0)),
            }
        ))
    );
}

#[test]
fn connect_tool_does_not_end_on_source_only_handle() {
    use crate::{CanvasHandle, HandleRole};

    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
    source_handle.role = HandleRole::Source;
    source.handles.push(source_handle);

    let mut target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
    let mut invalid_target_handle = CanvasHandle::new("out", point(px(0.0), px(50.0)));
    invalid_target_handle.role = HandleRole::Source;
    target.handles.push(invalid_target_handle);

    let document = document_fixture().node(source).node(target).build();
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::Connect).unwrap();

    editor
        .handle_event(CanvasEvent::PointerDown {
            position: point(px(100.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();
    editor
        .handle_event(CanvasEvent::PointerUp {
            position: point(px(200.0), px(50.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
        .unwrap();

    assert!(matches!(editor.session.state, ToolState::Idle));
    assert!(editor.document().edge_count() == 0);
    assert_eq!(editor.history().undo_depth(), 0);
}
