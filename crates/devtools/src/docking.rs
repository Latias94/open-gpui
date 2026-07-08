//! DevTools adapters for `open-gpui-docking` public diagnostics.

use open_gpui_docking::advanced::DockViewportRuntimeStatus;

use crate::{
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
};

/// Converts a docking viewport runtime status into a DevTools tree.
pub fn docking_runtime_probe_snapshot(status: &DockViewportRuntimeStatus) -> SnapshotProbeSnapshot {
    let mut root = snapshot_node_with_payload(
        ["docking", "viewport-runtime"],
        "Viewport runtime",
        serde_json::json!({
            "has_platform_capabilities": status.platform_capabilities.is_some(),
            "has_placement_restore": status.placement_restore.is_some(),
            "viewport_lifecycle_count": status.viewport_lifecycle.len(),
            "has_last_route": status.last_route.is_some(),
            "has_last_drop_outcome": status.last_drop_outcome.is_some(),
            "has_last_activation": status.last_activation.is_some(),
            "has_last_close": status.last_close.is_some(),
            "has_last_should_close": status.last_should_close.is_some(),
            "has_last_tear_off": status.last_tear_off.is_some(),
            "has_last_platform_sync": status.last_platform_sync.is_some(),
            "visual_affordance_count": status.visual_affordances.len(),
        }),
    );

    if let Some(capabilities) = status.platform_capabilities {
        root = root.with_child(snapshot_node_with_payload(
            ["docking", "viewport-runtime", "platform"],
            "Platform capabilities",
            serde_json::json!({
                "platform_viewport_windows": capabilities.platform_viewport_windows,
                "global_window_bounds": capabilities.global_window_bounds,
                "window_stack": capabilities.window_stack,
                "display_work_area": capabilities.display_work_area,
                "dpi_scale": capabilities.dpi_scale,
                "live_window_move": capabilities.live_window_move,
                "no_input_windows": capabilities.no_input_windows,
                "hovered_window_ignores_no_input": capabilities.hovered_window_ignores_no_input,
            }),
        ));
    }

    if let Some(restore) = status.placement_restore {
        root = root.with_child(snapshot_node_with_payload(
            ["docking", "viewport-runtime", "placement-restore"],
            "Placement restore",
            serde_json::json!({
                "matched": restore.matched,
                "missing": restore.missing,
            }),
        ));
    }

    for (index, lifecycle) in status.viewport_lifecycle.iter().enumerate() {
        root = root.with_child(snapshot_node_with_payload(
            [
                "docking",
                "viewport-runtime",
                "lifecycle",
                &index.to_string(),
            ],
            format!("Viewport lifecycle {index}"),
            serde_json::json!({
                "space": format!("{:?}", lifecycle.space),
                "window_id": format!("{:?}", lifecycle.window_id),
                "route_status": format!("{:?}", lifecycle.route_status),
                "input_status": format!("{:?}", lifecycle.input_status),
                "platform_request_status": format!("{:?}", lifecycle.platform_request_status),
                "coordinate_status": lifecycle
                    .coordinate_status
                    .as_ref()
                    .map(|status| format!("{status:?}")),
                "facts_generation": lifecycle.facts_generation,
            }),
        ));
    }

    append_optional_debug_node(&mut root, "last-route", &status.last_route);
    append_optional_debug_node(&mut root, "last-drop-outcome", &status.last_drop_outcome);
    append_optional_debug_node(&mut root, "last-activation", &status.last_activation);
    append_optional_debug_node(&mut root, "last-close", &status.last_close);
    append_optional_debug_node(&mut root, "last-should-close", &status.last_should_close);
    append_optional_debug_node(&mut root, "last-tear-off", &status.last_tear_off);
    append_optional_debug_node(&mut root, "last-platform-sync", &status.last_platform_sync);

    for (index, affordance) in status.visual_affordances.iter().enumerate() {
        root = root.with_child(snapshot_node_with_payload(
            [
                "docking",
                "viewport-runtime",
                "visual-affordance",
                &index.to_string(),
            ],
            format!("Visual affordance {index}"),
            serde_json::json!({
                "space": format!("{:?}", affordance.space),
                "window_id": format!("{:?}", affordance.window_id),
                "summary": format!("{:?}", affordance.summary),
            }),
        ));
    }

    SnapshotProbeSnapshot::new(SnapshotTree::new([root]))
        .with_redaction(SnapshotRedactionSummary::default())
}

fn append_optional_debug_node<T: std::fmt::Debug>(
    root: &mut crate::SnapshotNode,
    id: &'static str,
    value: &Option<T>,
) {
    if let Some(value) = value {
        root.children.push(snapshot_node_with_payload(
            ["docking", "viewport-runtime", id],
            id,
            serde_json::json!({
                "debug": format!("{value:?}"),
            }),
        ));
    }
}
