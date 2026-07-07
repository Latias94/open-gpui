use super::*;

#[test]
fn custom_tool_reducer_applies_effects_through_editor() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "anchor",
            point(px(100.0), px(50.0)),
            size(px(80.0), px(80.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
    editor.set_tool(CanvasTool::custom("stamp")).unwrap();
    let mut tool = StampTool::default();

    editor
        .handle_event_with_custom_tool(
            CanvasEvent::PointerDown {
                position: point(px(20.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            },
            &mut tool,
        )
        .unwrap();

    assert_eq!(tool.calls, 1);
    assert_eq!(tool.last_tool_id, Some(CanvasToolId::from("stamp")));
    assert_eq!(tool.last_hit, Some(HitTarget::Node(NodeId::from("anchor"))));

    let stamped = editor.document().node(&NodeId::from("stamp-1")).unwrap();
    assert_eq!(stamped.position, point(px(110.0), px(55.0)));
    assert_eq!(editor.history().undo_depth(), 1);
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("stamp-1")]
    );
    assert_eq!(editor.session.state, ToolState::Idle);

    assert!(editor.undo().unwrap());
    assert!(!editor.document().contains_node(&NodeId::from("stamp-1")));
}

#[test]
fn custom_tool_context_exposes_selection_record_scope() {
    #[derive(Clone)]
    struct ScopeProbeTool {
        observed: Arc<Mutex<Vec<CanvasRecordId>>>,
    }

    impl CanvasToolReducer for ScopeProbeTool {
        fn handle_event(
            &mut self,
            context: CanvasToolContext<'_>,
            _event: CanvasEvent,
        ) -> Result<Vec<CanvasToolIntent>, DocumentError> {
            let scope = context
                .selection_record_scope(CanvasRecordScopeOptions::structural_with_internal_edges())
                .records()
                .cloned()
                .collect::<Vec<_>>();
            *self.observed.lock().unwrap() = scope;
            Ok(Vec::new())
        }
    }

    let mut document = document_fixture()
        .shape(CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
        ))
        .node(CanvasNode::new(
            "child",
            point(px(20.0), px(20.0)),
            size(px(40.0), px(40.0)),
        ))
        .node(CanvasNode::new(
            "peer",
            point(px(80.0), px(20.0)),
            size(px(40.0), px(40.0)),
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
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("frame")),
                member: CanvasRecordId::Node(NodeId::from("peer")),
            },
        ]))
        .unwrap();

    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut editor = CanvasEditor::new(document);
    editor.set_tool(CanvasTool::custom("probe")).unwrap();
    editor
        .session
        .selection
        .shapes
        .insert(ShapeId::from("frame"));
    let mut tool = ScopeProbeTool {
        observed: Arc::clone(&observed),
    };

    editor
        .handle_event_with_custom_tool(
            CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            },
            &mut tool,
        )
        .unwrap();

    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            CanvasRecordId::Shape(ShapeId::from("frame")),
            CanvasRecordId::Node(NodeId::from("child")),
            CanvasRecordId::Node(NodeId::from("peer")),
            CanvasRecordId::Edge(EdgeId::from("child-peer")),
        ]
    );
}

#[test]
fn custom_tool_entry_uses_builtin_tools_without_calling_custom_reducer() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "n1",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    let mut tool = StampTool::default();

    editor
        .handle_event_with_custom_tool(
            CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            },
            &mut tool,
        )
        .unwrap();

    assert_eq!(tool.calls, 0);
    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("n1")]
    );
}

#[test]
fn tool_registry_dispatches_registered_custom_tool() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "anchor",
            point(px(100.0), px(50.0)),
            size(px(80.0), px(80.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    editor.session.viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
    editor.set_tool(CanvasTool::custom("stamp")).unwrap();
    let mut registry = CanvasToolRegistry::new();

    assert!(registry.is_empty());
    assert!(registry.insert("stamp", StampTool::default()).is_none());
    assert!(registry.contains(&CanvasToolId::from("stamp")));
    assert_eq!(
        registry.ids().cloned().collect::<Vec<_>>(),
        vec![CanvasToolId::from("stamp")]
    );

    editor
        .handle_event_with_tool_registry(
            CanvasEvent::PointerDown {
                position: point(px(20.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            },
            &mut registry,
        )
        .unwrap();

    let stamped = editor.document().node(&NodeId::from("stamp-1")).unwrap();
    assert_eq!(stamped.position, point(px(110.0), px(55.0)));
    assert_eq!(editor.history().undo_depth(), 1);
    assert!(registry.remove(&CanvasToolId::from("stamp")).is_some());
    assert!(!registry.contains(&CanvasToolId::from("stamp")));
}

#[test]
fn tool_registry_accepts_boxed_reducers() {
    let mut registry = CanvasToolRegistry::new();

    assert!(
        registry
            .insert_boxed("stamp", Box::new(StampTool::default()))
            .is_none()
    );

    assert_eq!(registry.len(), 1);
    assert!(registry.reducer_mut(&CanvasToolId::from("stamp")).is_some());
}

#[test]
fn tool_registry_reports_missing_custom_tool() {
    let mut editor = CanvasEditor::default();
    editor.set_tool(CanvasTool::custom("missing")).unwrap();
    let mut registry = CanvasToolRegistry::new();

    let err = editor
        .handle_event_with_tool_registry(CanvasEvent::Cancel, &mut registry)
        .unwrap_err();

    assert_eq!(
        err,
        CanvasToolRegistryError::MissingTool(CanvasToolId::from("missing"))
    );
}

#[test]
fn tool_registry_entry_uses_builtin_tools_without_registered_reducer() {
    let document = document_fixture()
        .node(CanvasNode::new(
            "n1",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        ))
        .build();
    let mut editor = CanvasEditor::new(document);
    let mut registry = CanvasToolRegistry::new();

    editor
        .handle_event_with_tool_registry(
            CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            },
            &mut registry,
        )
        .unwrap();

    assert_eq!(
        editor
            .session
            .selection
            .nodes
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::from("n1")]
    );
}

#[test]
fn set_tool_effect_switches_tool_and_resets_state() {
    let mut editor = CanvasEditor::default();
    editor.session.state = ToolState::Pointing {
        origin: point(px(10.0), px(20.0)),
        selection_mode: CanvasSelectionMode::Replace,
        base_selection: CanvasSelection::default(),
    };

    editor
        .apply_tool_effect(CanvasToolEffect::SetTool(CanvasTool::custom("stamp")))
        .unwrap();

    assert_eq!(editor.session.tool, CanvasTool::custom("stamp"));
    assert_eq!(editor.session.state, ToolState::Idle);
}
