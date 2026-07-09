//! DevTools adapters for `open-gpui-docking` public diagnostics.

use open_gpui_docking::advanced::{
    DockViewportInputStatus, DockViewportLifecycleRecord, DockViewportPayloadRecord,
    DockViewportPlatformCapabilityRecord, DockViewportPlatformSyncRecord,
    DockViewportReleaseUnavailableRecord, DockViewportRestoreReadinessRecord,
    DockViewportRouteRecord, DockViewportRouteSelectionRecord, DockViewportRouteStatus,
    DockViewportRouteTarget, DockViewportRuntimeStatus, DockViewportStaleStatusReason,
    DockViewportTearOffRecord, DockViewportVisualAffordanceRecord,
};

use crate::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsEventKind, DevtoolsEventRecord, DevtoolsTargetId, DevtoolsTargetKind,
    DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, SnapshotEnvelope, SnapshotKind,
    SnapshotNode, SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
};

const DOCKING_RUNTIME_PROBE_ID: &str = "docking.runtime";

/// Converts a docking viewport runtime status into a target/domain/event capture.
pub fn docking_runtime_capture(status: &DockViewportRuntimeStatus) -> DevtoolsCapture {
    let runtime_target_id = docking_runtime_target_id();
    let runtime_domain_id = docking_runtime_domain_id();
    let snapshot = docking_runtime_snapshot_envelope(status);

    let mut targets = vec![
        DevtoolsTargetSnapshot::new(
            runtime_target_id.clone(),
            DevtoolsTargetKind::Runtime,
            "Docking runtime",
        )
        .with_metadata(runtime_summary_payload(status)),
    ];
    targets.extend(
        status
            .viewport_lifecycle
            .iter()
            .enumerate()
            .map(|(index, lifecycle)| lifecycle_target(index, lifecycle, &runtime_target_id)),
    );
    targets.extend(
        status
            .visual_affordances
            .iter()
            .enumerate()
            .map(|(index, affordance)| {
                visual_affordance_target(index, affordance, &runtime_target_id)
            }),
    );

    let mut events = Vec::new();
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-route",
        "Last viewport route",
        status.last_route.as_ref().map(route_payload),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-drop-outcome",
        "Last drop outcome",
        status.last_drop_outcome.as_ref().map(|outcome| {
            serde_json::json!({
                "kind": format!("{:?}", outcome.kind),
                "has_action": outcome.action.is_some(),
                "has_error": outcome.error.is_some(),
                "action": outcome.action.map(|action| format!("{action:?}")),
                "error": outcome.error.as_ref().map(|error| format!("{error:?}")),
            })
        }),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-activation",
        "Last activation",
        status.last_activation.as_ref().map(|activation| {
            serde_json::json!({
                "space": activation.space.as_str(),
                "window_id": activation.window_id.as_u64(),
                "focus_request": format!("{:?}", activation.focus_request),
            })
        }),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-close",
        "Last close",
        status.last_close.as_ref().map(|close| {
            serde_json::json!({
                "space": close.space().map(|space| space.as_str()),
                "window_id": close.window_id().as_u64(),
                "status": format!("{:?}", close.status()),
            })
        }),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-should-close",
        "Last should-close",
        status.last_should_close.as_ref().map(|outcome| {
            serde_json::json!({
                "space": outcome.space.as_ref().map(|space| space.as_str()),
                "window_id": outcome.window_id.as_u64(),
                "status": format!("{:?}", outcome.status),
                "allows_close": outcome.allows_close(),
            })
        }),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-tear-off",
        "Last tear-off",
        status.last_tear_off.as_ref().map(tear_off_payload),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.last-platform-sync",
        "Last platform sync",
        status
            .last_platform_sync
            .as_ref()
            .map(platform_sync_payload),
    );
    events.extend(
        status
            .visual_affordances
            .iter()
            .enumerate()
            .map(|(index, affordance)| {
                DevtoolsEventRecord::new(
                    format!("docking.visual-affordance.{index}"),
                    format!("Visual affordance {index}"),
                    DevtoolsEventKind::Instant,
                )
                .target_id(visual_affordance_target_id(index, affordance))
                .domain_id(runtime_domain_id.clone())
                .with_payload(visual_affordance_payload(affordance))
            }),
    );

    let domain = DevtoolsDomainSnapshot::new(
        runtime_domain_id,
        runtime_target_id,
        DevtoolsDomainKind::Docking,
        "Docking runtime",
    )
    .with_summary(runtime_summary_payload(status))
    .with_snapshot(snapshot.clone());

    DevtoolsCapture::new(
        DevtoolsTargetTree::new(targets),
        [domain],
        events,
        [snapshot],
        Vec::new(),
    )
}

/// Converts a docking viewport runtime status into a DevTools tree.
pub fn docking_runtime_probe_snapshot(status: &DockViewportRuntimeStatus) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(docking_runtime_tree(status))
        .with_redaction(SnapshotRedactionSummary::default())
}

fn docking_runtime_snapshot_envelope(status: &DockViewportRuntimeStatus) -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        ProbeId::new(DOCKING_RUNTIME_PROBE_ID)
            .expect("internal docking runtime probe id is non-empty"),
        SnapshotKind::Docking,
        docking_runtime_tree(status),
    )
    .with_redaction(SnapshotRedactionSummary::default())
}

fn docking_runtime_tree(status: &DockViewportRuntimeStatus) -> SnapshotTree {
    let mut root = snapshot_node_with_payload(
        ["docking", "viewport-runtime"],
        "Viewport runtime",
        runtime_summary_payload(status),
    );

    if let Some(capabilities) = status.platform_capabilities {
        root = root.with_child(snapshot_node_with_payload(
            ["docking", "viewport-runtime", "platform"],
            "Platform capabilities",
            platform_capability_payload(capabilities),
        ));
    }

    if let Some(restore) = status.placement_restore {
        root = root.with_child(snapshot_node_with_payload(
            ["docking", "viewport-runtime", "placement-restore"],
            "Placement restore",
            placement_restore_payload(restore),
        ));
    }

    for (index, lifecycle) in status.viewport_lifecycle.iter().enumerate() {
        let index_label = index.to_string();
        root = root.with_child(snapshot_node_with_payload(
            [
                "docking",
                "viewport-runtime",
                "lifecycle",
                index_label.as_str(),
            ],
            format!("Viewport lifecycle {index}"),
            lifecycle_payload(lifecycle),
        ));
    }

    append_optional_node(&mut root, "last-route", &status.last_route, route_payload);
    append_optional_node(
        &mut root,
        "last-drop-outcome",
        &status.last_drop_outcome,
        |outcome| {
            serde_json::json!({
                "kind": format!("{:?}", outcome.kind),
                "has_action": outcome.action.is_some(),
                "has_error": outcome.error.is_some(),
                "action": outcome.action.map(|action| format!("{action:?}")),
                "error": outcome.error.as_ref().map(|error| format!("{error:?}")),
            })
        },
    );
    append_optional_node(
        &mut root,
        "last-activation",
        &status.last_activation,
        |activation| {
            serde_json::json!({
                "space": activation.space.as_str(),
                "window_id": activation.window_id.as_u64(),
                "focus_request": format!("{:?}", activation.focus_request),
            })
        },
    );
    append_optional_node(&mut root, "last-close", &status.last_close, |close| {
        serde_json::json!({
            "space": close.space().map(|space| space.as_str()),
            "window_id": close.window_id().as_u64(),
            "status": format!("{:?}", close.status()),
        })
    });
    append_optional_node(
        &mut root,
        "last-should-close",
        &status.last_should_close,
        |outcome| {
            serde_json::json!({
                "space": outcome.space.as_ref().map(|space| space.as_str()),
                "window_id": outcome.window_id.as_u64(),
                "status": format!("{:?}", outcome.status),
                "allows_close": outcome.allows_close(),
            })
        },
    );
    append_optional_node(
        &mut root,
        "last-tear-off",
        &status.last_tear_off,
        tear_off_payload,
    );
    append_optional_node(
        &mut root,
        "last-platform-sync",
        &status.last_platform_sync,
        platform_sync_payload,
    );

    for (index, affordance) in status.visual_affordances.iter().enumerate() {
        let index_label = index.to_string();
        root = root.with_child(snapshot_node_with_payload(
            [
                "docking",
                "viewport-runtime",
                "visual-affordance",
                index_label.as_str(),
            ],
            format!("Visual affordance {index}"),
            visual_affordance_payload(affordance),
        ));
    }

    SnapshotTree::new([root])
}

fn docking_runtime_target_id() -> DevtoolsTargetId {
    DevtoolsTargetId::from_parts(["docking", "runtime"])
}

fn docking_runtime_domain_id() -> DevtoolsDomainId {
    DevtoolsDomainId::from_parts(["docking", "runtime"])
}

fn lifecycle_target_id(index: usize, lifecycle: &DockViewportLifecycleRecord) -> DevtoolsTargetId {
    let index_label = index.to_string();
    let window_id = lifecycle.window_id.as_u64().to_string();
    DevtoolsTargetId::from_parts([
        "docking",
        "viewport",
        index_label.as_str(),
        lifecycle.space.as_str(),
        window_id.as_str(),
    ])
}

fn lifecycle_target(
    index: usize,
    lifecycle: &DockViewportLifecycleRecord,
    parent_id: &DevtoolsTargetId,
) -> DevtoolsTargetSnapshot {
    DevtoolsTargetSnapshot::new(
        lifecycle_target_id(index, lifecycle),
        DevtoolsTargetKind::Viewport,
        format!("Viewport {}", lifecycle.space.as_str()),
    )
    .parent_id(parent_id.clone())
    .with_metadata(lifecycle_payload(lifecycle))
}

fn visual_affordance_target_id(
    index: usize,
    affordance: &DockViewportVisualAffordanceRecord,
) -> DevtoolsTargetId {
    let index_label = index.to_string();
    let window_id = affordance.window_id.as_u64().to_string();
    DevtoolsTargetId::from_parts([
        "docking",
        "visual-affordance",
        index_label.as_str(),
        affordance.space.as_str(),
        window_id.as_str(),
    ])
}

fn visual_affordance_target(
    index: usize,
    affordance: &DockViewportVisualAffordanceRecord,
    parent_id: &DevtoolsTargetId,
) -> DevtoolsTargetSnapshot {
    DevtoolsTargetSnapshot::new(
        visual_affordance_target_id(index, affordance),
        DevtoolsTargetKind::Viewport,
        format!("Visual affordance {}", affordance.space.as_str()),
    )
    .parent_id(parent_id.clone())
    .with_metadata(visual_affordance_payload(affordance))
}

fn push_optional_event(
    events: &mut Vec<DevtoolsEventRecord>,
    target_id: &DevtoolsTargetId,
    domain_id: &DevtoolsDomainId,
    id: &'static str,
    label: &'static str,
    payload: Option<serde_json::Value>,
) {
    if let Some(payload) = payload {
        events.push(
            DevtoolsEventRecord::new(id, label, DevtoolsEventKind::Instant)
                .target_id(target_id.clone())
                .domain_id(domain_id.clone())
                .with_payload(payload),
        );
    }
}

fn append_optional_node<T>(
    root: &mut SnapshotNode,
    id: &'static str,
    value: &Option<T>,
    payload: impl Fn(&T) -> serde_json::Value,
) {
    if let Some(value) = value {
        root.children.push(snapshot_node_with_payload(
            ["docking", "viewport-runtime", id],
            id,
            payload(value),
        ));
    }
}

fn runtime_summary_payload(status: &DockViewportRuntimeStatus) -> serde_json::Value {
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
    })
}

fn platform_capability_payload(
    capabilities: DockViewportPlatformCapabilityRecord,
) -> serde_json::Value {
    serde_json::json!({
        "platform_viewport_windows": capabilities.platform_viewport_windows,
        "global_window_bounds": capabilities.global_window_bounds,
        "window_stack": capabilities.window_stack,
        "display_work_area": capabilities.display_work_area,
        "dpi_scale": capabilities.dpi_scale,
        "live_window_move": capabilities.live_window_move,
        "no_input_windows": capabilities.no_input_windows,
        "hovered_window_ignores_no_input": capabilities.hovered_window_ignores_no_input,
    })
}

fn placement_restore_payload(restore: DockViewportRestoreReadinessRecord) -> serde_json::Value {
    serde_json::json!({
        "matched": restore.matched,
        "missing": restore.missing,
    })
}

fn lifecycle_payload(lifecycle: &DockViewportLifecycleRecord) -> serde_json::Value {
    serde_json::json!({
        "space": lifecycle.space.as_str(),
        "window_id": lifecycle.window_id.as_u64(),
        "route_status": route_status_label(&lifecycle.route_status),
        "input_status": input_status_label(lifecycle.input_status),
        "platform_request_status": {
            "close_requested": lifecycle.platform_request_status.close_requested,
            "resize_requested": lifecycle.platform_request_status.resize_requested,
        },
        "coordinate_status": lifecycle.coordinate_status.as_ref().map(|status| {
            serde_json::json!({
                "display_id": status.display_id.map(|display_id| format!("{display_id:?}")),
                "coordinate_space": format!("{:?}", status.coordinate_space),
                "facts_generation": status.facts_generation,
            })
        }),
        "facts_generation": lifecycle.facts_generation,
    })
}

fn route_payload(route: &DockViewportRouteRecord) -> serde_json::Value {
    serde_json::json!({
        "source_space": route.source_space.as_str(),
        "source_node": route.source_node.as_u64(),
        "payload": payload_record_payload(&route.payload),
        "drag_session_id": route.drag_session_id,
        "selection_source": route.selection_source.map(route_selection_label),
        "unavailable_reason": route.unavailable_reason.map(release_unavailable_label),
        "target": route_target_payload(&route.target),
    })
}

fn payload_record_payload(payload: &DockViewportPayloadRecord) -> serde_json::Value {
    match payload {
        DockViewportPayloadRecord::Item { item } => {
            serde_json::json!({ "kind": "item", "item": item.as_str() })
        }
        DockViewportPayloadRecord::Tabs => serde_json::json!({ "kind": "tabs" }),
        DockViewportPayloadRecord::Floating { floating } => {
            serde_json::json!({ "kind": "floating", "node_id": floating.as_u64() })
        }
    }
}

fn route_target_payload(target: &DockViewportRouteTarget) -> serde_json::Value {
    match target {
        DockViewportRouteTarget::Local {
            space,
            window_id,
            host_position,
        } => serde_json::json!({
            "kind": "local",
            "space": space.as_str(),
            "window_id": window_id.as_u64(),
            "host_position": format!("{host_position:?}"),
        }),
        DockViewportRouteTarget::KnownViewport {
            space,
            window_id,
            host_position,
        } => serde_json::json!({
            "kind": "known-viewport",
            "space": space.as_str(),
            "window_id": window_id.as_u64(),
            "host_position": format!("{host_position:?}"),
        }),
        DockViewportRouteTarget::TearOff { release_position } => serde_json::json!({
            "kind": "tear-off",
            "release_position": format!("{release_position:?}"),
        }),
        DockViewportRouteTarget::Unavailable => serde_json::json!({ "kind": "unavailable" }),
        DockViewportRouteTarget::Rejected { reason } => serde_json::json!({
            "kind": "rejected",
            "reason": format!("{reason:?}"),
        }),
    }
}

fn tear_off_payload(record: &DockViewportTearOffRecord) -> serde_json::Value {
    serde_json::json!({
        "kind": format!("{:?}", record.kind),
        "placement_source": record.placement_source.map(|source| format!("{source:?}")),
        "source_space": record.source_space.as_str(),
        "target_space": record.target_space.as_str(),
        "payload": payload_record_payload(&record.payload),
    })
}

fn platform_sync_payload(record: &DockViewportPlatformSyncRecord) -> serde_json::Value {
    serde_json::json!({
        "window_id": record.window_id.as_u64(),
        "applied_count": record.applied.len(),
        "skipped_count": record.skipped_requests.len(),
        "unsupported_count": record.unsupported_requests.len(),
        "applied": record.applied.iter().map(|action| format!("{action:?}")).collect::<Vec<_>>(),
        "skipped_requests": record.skipped_requests.iter().map(|skipped| {
            serde_json::json!({
                "request": format!("{:?}", skipped.request),
                "reason": format!("{:?}", skipped.reason),
            })
        }).collect::<Vec<_>>(),
        "unsupported_requests": record.unsupported_requests.iter().map(|unsupported| {
            serde_json::json!({
                "request": format!("{:?}", unsupported.request),
                "reason": format!("{:?}", unsupported.reason),
            })
        }).collect::<Vec<_>>(),
    })
}

fn visual_affordance_payload(record: &DockViewportVisualAffordanceRecord) -> serde_json::Value {
    serde_json::json!({
        "space": record.space.as_str(),
        "window_id": record.window_id.as_u64(),
        "summary": format!("{:?}", record.summary),
    })
}

fn route_status_label(status: &DockViewportRouteStatus) -> &'static str {
    match status {
        DockViewportRouteStatus::RegisteredNotReady => "registered-not-ready",
        DockViewportRouteStatus::RouteReady => "route-ready",
        DockViewportRouteStatus::Stale { reason } => match reason {
            DockViewportStaleStatusReason::WindowFactsChanged => "stale-window-facts-changed",
        },
        DockViewportRouteStatus::Minimized => "minimized",
        DockViewportRouteStatus::MissingRouteFacts => "missing-route-facts",
    }
}

fn input_status_label(status: DockViewportInputStatus) -> &'static str {
    match status {
        DockViewportInputStatus::ReceivesInput => "receives-input",
        DockViewportInputStatus::Minimized => "minimized",
        DockViewportInputStatus::NoInputPassThrough => "no-input-pass-through",
    }
}

fn route_selection_label(selection: DockViewportRouteSelectionRecord) -> &'static str {
    match selection {
        DockViewportRouteSelectionRecord::TrustedHoveredWindow => "trusted-hovered-window",
        DockViewportRouteSelectionRecord::EventReceiverLocalScene => "event-receiver-local-scene",
        DockViewportRouteSelectionRecord::FrontToBackWindowStackFallback => {
            "front-to-back-window-stack-fallback"
        }
        DockViewportRouteSelectionRecord::FocusStampWindowStackFallback => {
            "focus-stamp-window-stack-fallback"
        }
        DockViewportRouteSelectionRecord::DragLastHoveredViewportFallback => {
            "drag-last-hovered-viewport-fallback"
        }
    }
}

fn release_unavailable_label(reason: DockViewportReleaseUnavailableRecord) -> &'static str {
    match reason {
        DockViewportReleaseUnavailableRecord::PlatformViewportWindowsUnsupported => {
            "platform-viewport-windows-unsupported"
        }
        DockViewportReleaseUnavailableRecord::BlockedByViewportWindow => {
            "blocked-by-viewport-window"
        }
        DockViewportReleaseUnavailableRecord::NoViewportRouteSelection => {
            "no-viewport-route-selection"
        }
        DockViewportReleaseUnavailableRecord::TrustedHoveredNone => "trusted-hovered-none",
        _ => "unknown",
    }
}
