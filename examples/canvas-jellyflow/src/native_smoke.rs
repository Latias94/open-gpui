use super::*;
use jellyflow_open_gpui::testing::{
    OpenGpuiNativeLifecycleEvidence, assert_native_lifecycle_evidence_gates,
};

pub(super) fn product_gallery_native_lifecycle_evidence(
    cx: &mut open_gpui::TestAppContext,
) -> OpenGpuiNativeLifecycleEvidence {
    cx.update(|cx| cx.set_quit_mode(QuitMode::LastWindowClosed));

    let (gallery, store, editor, projection) = product_gallery_state();
    let node_kit_registry = NodeKitRegistry::builtin();
    let semantic_registry = node_kit_registry.node_registry();
    let window = cx.open_window(size(px(CANVAS_WIDTH), px(CANVAS_HEIGHT)), move |_, cx| {
        JellyflowCanvasView {
            editor,
            store,
            focus_handle: cx.focus_handle(),
            projection,
            gallery,
            adapter: OpenGpuiAdapter::default(),
            semantic_registry,
            node_kit_registry,
            measured_regions: OpenGpuiBoundsCollector::new(),
            measurement_coverage: BTreeMap::new(),
            measurement_revision: 1,
            measurement_refresh_requested: false,
            measurement_frame_pending: false,
            measurement_frame_generation: 0,
            auto_fit_viewport: true,
            deferred_editor_refresh: false,
            last_canvas_view_size: None,
            last_canvas_bounds: None,
            last_canvas_scene: None,
        }
    });

    let Ok(view) = window.root(cx) else {
        let mut evidence = OpenGpuiNativeLifecycleEvidence::default();
        evidence.mark_close_automation_skipped(
            "product gallery window did not expose JellyflowCanvasView root",
        );
        return evidence;
    };

    let mut evidence = OpenGpuiNativeLifecycleEvidence::default();
    let (fixture_id, rendered_product_surface_count) = cx.read_entity(&view, |this, _| {
        let fixture_id = this.gallery.active_id().to_owned();
        let rendered_product_surface_count = if this.projection.graph_nodes > 0
            && this.projection.graph_edges > 0
            && !fixture_id.is_empty()
        {
            this.editor.document().nodes().count()
        } else {
            0
        };
        (fixture_id, rendered_product_surface_count)
    });
    evidence.mark_product_gallery_rendered(fixture_id, rendered_product_surface_count);

    let drag_sequence_checked = drag_product_gallery_node_without_resetting_siblings(cx, &view);
    evidence.mark_product_drag_checked(drag_sequence_checked);

    let window_handle = window.into();
    let windows_before_close = cx.windows().len();
    let close_requested = cx.simulate_window_close(window_handle);
    evidence.mark_last_window_close(
        windows_before_close,
        close_requested,
        cx.windows().len(),
        cx.did_quit(),
    );

    evidence
}

fn drag_product_gallery_node_without_resetting_siblings(
    cx: &mut open_gpui::TestAppContext,
    view: &open_gpui::Entity<JellyflowCanvasView>,
) -> bool {
    cx.update_entity(view, |this, cx| {
        this.last_canvas_bounds = Some(Bounds::new(
            point(px(24.0), px(46.0)),
            default_canvas_view_size(),
        ));

        let nodes = this
            .editor
            .document()
            .nodes()
            .map(|node| (node.id.clone(), node.position))
            .collect::<Vec<_>>();
        let Some((target_id, target_initial_position)) = nodes.first().cloned() else {
            return false;
        };
        let sibling_positions = nodes.iter().skip(1).cloned().collect::<Vec<_>>();
        let Some(target_node) = this.editor.document().node(&target_id).cloned() else {
            return false;
        };

        let node_view_bounds = this
            .editor
            .viewport()
            .document_bounds_to_view(target_node.bounds());
        let down = point(
            node_view_bounds.origin.x + node_view_bounds.size.width * 0.5,
            node_view_bounds.origin.y + px(24.0),
        );
        let first_move = down + point(px(42.0), px(18.0));
        let second_move = down + point(px(90.0), px(40.0));

        this.handle_canvas_event(
            Some(CanvasEvent::PointerDown {
                position: down,
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            }),
            cx,
        );
        if this.editor.is_tool_state_idle() {
            return false;
        }

        this.handle_canvas_event(
            Some(CanvasEvent::PointerMove {
                position: first_move,
                modifiers: CanvasKeyModifiers::default(),
            }),
            cx,
        );
        let Some(after_first_move) = this
            .editor
            .document()
            .node(&target_id)
            .map(|node| node.position)
        else {
            return false;
        };
        if after_first_move == target_initial_position {
            return false;
        }

        this.handle_canvas_event(
            Some(CanvasEvent::PointerMove {
                position: second_move,
                modifiers: CanvasKeyModifiers::default(),
            }),
            cx,
        );
        let Some(after_second_move) = this
            .editor
            .document()
            .node(&target_id)
            .map(|node| node.position)
        else {
            return false;
        };
        if after_second_move == after_first_move {
            return false;
        }

        this.handle_canvas_event(
            Some(CanvasEvent::PointerUp {
                position: second_move,
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            }),
            cx,
        );
        if !this.editor.is_tool_state_idle() || this.deferred_editor_refresh {
            return false;
        }

        sibling_positions.into_iter().all(|(id, position)| {
            this.editor
                .document()
                .node(&id)
                .is_some_and(|node| node.position == position)
        })
    })
}

#[open_gpui::test]
fn product_gallery_native_smoke_covers_launch_drag_and_close(cx: &mut open_gpui::TestAppContext) {
    let evidence = product_gallery_native_lifecycle_evidence(cx);
    assert_native_lifecycle_evidence_gates(&evidence);
}

#[test]
fn product_gallery_native_smoke_gate_rejects_blank_window_report() {
    let mut evidence = OpenGpuiNativeLifecycleEvidence::default();
    evidence.mark_product_drag_checked(true);
    evidence.mark_last_window_close(1, true, 0, true);

    let result = std::panic::catch_unwind(|| {
        assert_native_lifecycle_evidence_gates(&evidence);
    });
    assert!(
        result.is_err(),
        "blank windows must not satisfy native smoke gates"
    );
}
