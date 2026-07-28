//! DevTools adapters for `open-gpui-docking` public diagnostics.

use open_gpui::{
    PlatformWindowCapabilities, PlatformWindowCreationCapabilities,
    PlatformWindowMutationCapabilities, WindowCoordinateSpace, WindowCreationSupport,
    WindowInitialPresentationOrder, WindowMutationRequest, WindowMutationSupport,
    WindowPlacementState, WindowPlatformFacts,
};
use open_gpui_docking::advanced::{
    DockViewportInputStatus, DockViewportLifecycleRecord, DockViewportPayloadRecord,
    DockViewportPlatformCapabilityRecord, DockViewportPlatformSyncDispatch,
    DockViewportPlatformSyncObservedRecord, DockViewportPlatformSyncRecord,
    DockViewportPlatformSyncRequest, DockViewportReleaseUnavailableRecord,
    DockViewportRestoreReadinessRecord, DockViewportRouteRecord, DockViewportRouteSelectionRecord,
    DockViewportRouteStatus, DockViewportRouteTarget, DockViewportRuntimeStatus,
    DockViewportStaleStatusReason, DockViewportTearOffRecord, DockViewportVisualAffordanceRecord,
    DockViewportWindowProfileRecord,
};
use serde::{Deserialize, Serialize};

use crate::{
    CaptureProvider, DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsEventKind, DevtoolsEventRecord, DevtoolsTargetId, DevtoolsTargetKind,
    DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, ProbeSnapshotError, SnapshotDiagnostic,
    SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbeSnapshot, SnapshotRedactionSummary,
    SnapshotTree,
    adapters::{sanitize_sensitive_text, snapshot_node_with_payload},
};

const DOCKING_RUNTIME_PROBE_ID: &str = "docking.runtime";

/// Diagnostic code emitted when public platform facts say viewport windows are unsupported.
pub const DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED: &str =
    "docking.platform_viewport_windows.unsupported";
/// Diagnostic code emitted when public lifecycle facts say route facts are missing.
pub const DOCKING_VIEWPORT_ROUTE_FACTS_MISSING: &str = "docking.viewport.route_facts.missing";
/// Diagnostic code emitted when public lifecycle facts say route facts are stale.
pub const DOCKING_VIEWPORT_ROUTE_FACTS_STALE: &str = "docking.viewport.route_facts.stale";

/// Structured DevTools projection of one docking viewport runtime status.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingRuntimeInspection {
    /// Stable runtime target id used by the DevTools target tree.
    pub runtime_target_id: DevtoolsTargetId,
    /// Stable docking domain id used by the DevTools domain output.
    pub domain_id: DevtoolsDomainId,
    /// Compact status summary suitable for headers and history rows.
    pub summary: DockingRuntimeSummary,
    /// Platform capability facts, present only when the application supplied them.
    pub platform_capabilities: Option<DockingPlatformCapabilityRow>,
    /// Creation and mutation capabilities captured for each viewport's actual platform window kind.
    pub window_profiles: Vec<DockingViewportWindowProfileRow>,
    /// Saved-placement restore facts, present only after a restore check ran.
    pub placement_restore: Option<DockingPlacementRestoreRow>,
    /// Per-viewport lifecycle rows in deterministic runtime order.
    pub viewport_lifecycle: Vec<DockingViewportLifecycleRow>,
    /// Optional route/drop/tear-off/close/sync facts that were actually observed.
    pub runtime_events: Vec<DockingRuntimeEventRow>,
    /// Visual affordance rows published by rendered viewport hosts.
    pub visual_affordances: Vec<DockingVisualAffordanceRow>,
    /// Diagnostics derived only from explicit public fact records.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

/// Header-level counts and capability hints for a docking runtime inspection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingRuntimeSummary {
    /// Whether platform capability facts were present in the status.
    pub platform_capabilities_present: bool,
    /// Whether platform viewport windows are supported, when that fact is present.
    pub platform_viewport_windows: Option<bool>,
    /// Whether saved-placement restore facts were present in the status.
    pub placement_restore_present: bool,
    /// Number of registered viewport lifecycle rows.
    pub viewport_lifecycle_count: usize,
    /// Number of viewport windows with an actual-kind creation and mutation profile.
    pub window_profile_count: usize,
    /// Number of lifecycle rows that are route-ready.
    pub route_ready_count: usize,
    /// Number of lifecycle rows with stale route facts.
    pub stale_viewport_count: usize,
    /// Number of lifecycle rows missing required route facts.
    pub missing_route_facts_count: usize,
    /// Number of optional runtime event rows that are present.
    pub runtime_event_count: usize,
    /// Number of visual affordance rows published by hosts.
    pub visual_affordance_count: usize,
    /// Number of diagnostics produced from explicit public facts.
    pub diagnostic_count: usize,
}

/// Platform capability facts relevant to multi-viewport debugging.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingPlatformCapabilityRow {
    /// Whether independent application viewport windows can be opened.
    pub platform_viewport_windows: bool,
    /// Whether bounds are reported in a shared desktop coordinate space.
    pub global_window_bounds: bool,
    /// Whether the platform can report windows in front-to-back order.
    pub window_stack: bool,
    /// Whether display visible bounds exclude reserved work areas.
    pub display_work_area: bool,
    /// Whether per-window DPI scale facts are reliable.
    pub dpi_scale: bool,
    /// Whether hovered-window queries ignore no-input application windows.
    pub hovered_window_ignores_no_input: bool,
}

/// Complete creation and mutation capabilities for one platform window kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingWindowCapabilitiesRow {
    /// Creation-only capabilities.
    pub creation: DockingWindowCreationCapabilityRow,
    /// Live and creation-only mutation capabilities.
    pub mutations: DockingWindowMutationCapabilityRow,
}

/// Creation-only platform window capabilities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingWindowCreationCapabilityRow {
    /// Support for a non-activating first appearance.
    pub focus_on_appearing: DockingWindowCreationSupport,
    /// Support for a typed top-level transient owner relationship.
    pub transient_for: DockingWindowCreationSupport,
    /// Ordering required between first frame submission and native visibility.
    pub initial_presentation_order: DockingWindowInitialPresentationOrder,
}

/// Property-specific platform window support relevant to viewport mutation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingWindowMutationCapabilityRow {
    /// Support for changing desktop position.
    pub position: DockingWindowMutationSupport,
    /// Support for changing content size.
    pub size: DockingWindowMutationSupport,
    /// Support for restoring windowed state.
    pub windowed: DockingWindowMutationSupport,
    /// Support for maximized state.
    pub maximized: DockingWindowMutationSupport,
    /// Support for fullscreen state.
    pub fullscreen: DockingWindowMutationSupport,
    /// Support for minimized state.
    pub minimized: DockingWindowMutationSupport,
    /// Support for windowed restore bounds.
    pub restore_bounds: DockingWindowMutationSupport,
    /// Support for native pointer-input acceptance.
    pub pointer_input: DockingWindowMutationSupport,
    /// Support for coherently changing lifetime activation and click-focus policy.
    pub activation_policy: DockingWindowMutationSupport,
    /// Support for alpha or transparent backgrounds.
    pub alpha: DockingWindowMutationSupport,
    /// Support for topmost windows.
    pub topmost: DockingWindowMutationSupport,
    /// Support for taskbar visibility.
    pub taskbar_visibility: DockingWindowMutationSupport,
    /// Coordinate space backing committed window geometry.
    pub coordinate_space: DockingWindowCoordinateSpace,
}

/// Complete platform profile captured for one docking viewport window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingViewportWindowProfileRow {
    /// Logical dock space rendered by the viewport.
    pub space: String,
    /// GPUI window id bound to the logical space.
    pub window_id: u64,
    /// Stable label for the actual platform window kind used at creation.
    pub window_kind: String,
    /// Creation and mutation support for that kind.
    pub capabilities: DockingWindowCapabilitiesRow,
}

/// Support level for one creation-only platform window property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockingWindowCreationSupport {
    /// The backend cannot represent the property.
    Unsupported,
    /// The backend can apply and report the property during creation.
    Supported,
}

/// Required ordering between first frame submission and native visibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockingWindowInitialPresentationOrder {
    /// The first frame can be submitted before the native window becomes visible.
    BeforeVisibility,
    /// The native window must be visible before the first frame can be submitted.
    AfterVisibility,
    /// The first frame submission itself establishes native visibility or mapping.
    PresentationEstablishesVisibility,
}

/// Support level for one platform window property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockingWindowMutationSupport {
    /// The backend cannot apply this property.
    Unsupported,
    /// The property can be selected only while opening a window.
    CreationOnly,
    /// The property can be requested and observed on an open window.
    Live,
}

/// Coordinate space backing platform window facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockingWindowCoordinateSpace {
    /// Geometry belongs to one backend-local window frame.
    WindowLocal,
    /// Geometry belongs to one shared desktop frame.
    GlobalScreen,
}

/// Saved-placement restore readiness facts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingPlacementRestoreRow {
    /// Number of saved placements that matched registered runtime windows.
    pub matched: usize,
    /// Number of saved placements without a registered runtime window.
    pub missing: usize,
    /// Whether any saved placement is currently missing.
    pub has_missing: bool,
}

/// One registered platform viewport lifecycle row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingViewportLifecycleRow {
    /// Stable target id matching the target tree entry for this viewport.
    pub target_id: DevtoolsTargetId,
    /// Deterministic index from the runtime status vector.
    pub index: usize,
    /// Logical dock space rendered by the viewport.
    pub space: String,
    /// GPUI window id currently bound to the dock space.
    pub window_id: u64,
    /// Stable route status label.
    pub route_status: String,
    /// Stable input status label.
    pub input_status: String,
    /// Whether the platform requested that this viewport should close.
    pub close_requested: bool,
    /// Whether the platform requested or reported an authoritative resize.
    pub resize_requested: bool,
    /// Latest lifecycle facts generation.
    pub facts_generation: u64,
    /// Display id for coordinate facts, when the backend supplied one.
    pub display_id: Option<String>,
    /// Coordinate space backing the latest viewport bounds.
    pub coordinate_space: Option<String>,
    /// Route-facts generation attached to the coordinate facts.
    pub coordinate_facts_generation: Option<u64>,
}

/// One present optional runtime event row for route/drop/close/tear-off diagnostics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingRuntimeEventRow {
    /// Stable event id used by the DevTools event stream.
    pub event_id: String,
    /// Human-readable event label.
    pub label: String,
    /// Target that owns the event.
    pub target_id: DevtoolsTargetId,
    /// Domain that owns the event.
    pub domain_id: DevtoolsDomainId,
    /// Sanitized structured payload for the event.
    pub payload: serde_json::Value,
}

/// One visual affordance diagnostic row published by a viewport host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingVisualAffordanceRow {
    /// Stable target id matching the visual-affordance target tree entry.
    pub target_id: DevtoolsTargetId,
    /// Deterministic index from the runtime status vector.
    pub index: usize,
    /// Logical dock space rendered by the host.
    pub space: String,
    /// GPUI window id that produced the diagnostic.
    pub window_id: u64,
    /// Dock space reported by the visual summary, when present.
    pub summary_space: Option<String>,
    /// Render-frame generation attached to the scene, when present.
    pub frame_generation: Option<u64>,
    /// Total number of visual affordance layers.
    pub layer_count: usize,
    /// Number of active visual affordance layers.
    pub active_count: usize,
    /// Stable id of the first active layer, when present.
    pub active_layer_id: Option<String>,
    /// Kind of the first active layer, when present.
    pub active_layer_kind: Option<String>,
    /// Scope of the first active layer, when present.
    pub active_layer_scope: Option<String>,
    /// State of the first active layer, when present.
    pub active_layer_state: Option<String>,
    /// Target node id of the first active layer, when present.
    pub active_target_node: Option<u64>,
    /// Drop zone of the first active layer, when present.
    pub active_zone: Option<String>,
    /// Drag payload index of the first active layer, when present.
    pub active_payload_index: Option<usize>,
    /// Whether the active layer carried a label that was intentionally not exported.
    pub active_has_label: bool,
    /// Current visual affordance motion executor state, when present.
    pub motion_state: Option<String>,
    /// Stable churn signature for visual-affordance retarget debugging.
    pub churn_signature: String,
}

/// Converts a docking viewport runtime status into a target/domain/event capture.
pub fn docking_runtime_capture(status: &DockViewportRuntimeStatus) -> DevtoolsCapture {
    let runtime_target_id = docking_runtime_target_id();
    let runtime_domain_id = docking_runtime_domain_id();
    let snapshot = docking_runtime_snapshot_envelope(status);
    let diagnostics = docking_runtime_diagnostics(status);

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
        "docking.last-platform-dispatch",
        "Last platform dispatch",
        status
            .last_platform_dispatch
            .as_ref()
            .map(platform_sync_payload),
    );
    push_optional_event(
        &mut events,
        &runtime_target_id,
        &runtime_domain_id,
        "docking.platform-observations",
        "Recent platform observations",
        (!status.recent_platform_observations.is_empty())
            .then(|| platform_observations_payload(&status.recent_platform_observations)),
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

    let mut domain = DevtoolsDomainSnapshot::new(
        runtime_domain_id,
        runtime_target_id,
        DevtoolsDomainKind::Docking,
        "Docking runtime",
    )
    .with_summary(runtime_summary_payload(status))
    .with_snapshot(snapshot.clone());
    for diagnostic in diagnostics.iter().cloned() {
        domain = domain.with_diagnostic(diagnostic);
    }

    DevtoolsCapture::new(
        DevtoolsTargetTree::new(targets),
        [domain],
        events,
        [snapshot],
        diagnostics,
    )
}

/// Creates a capture provider for docking viewport runtime status snapshots.
pub fn docking_runtime_capture_provider<F>(
    id: impl Into<String>,
    status: F,
) -> Result<
    CaptureProvider<impl Fn() -> Result<DevtoolsCapture, ProbeSnapshotError>>,
    ProbeSnapshotError,
>
where
    F: Fn() -> DockViewportRuntimeStatus + Send + Sync + 'static,
{
    CaptureProvider::new(id, move || Ok(docking_runtime_capture(&status())))
}

/// Converts a docking viewport runtime status into a DevTools tree.
pub fn docking_runtime_probe_snapshot(status: &DockViewportRuntimeStatus) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(docking_runtime_tree(status))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts docking runtime status into structured rows for inspector/workbench UIs.
pub fn docking_runtime_inspection(status: &DockViewportRuntimeStatus) -> DockingRuntimeInspection {
    let runtime_target_id = docking_runtime_target_id();
    let domain_id = docking_runtime_domain_id();
    let runtime_events = docking_runtime_event_rows(status, &runtime_target_id, &domain_id);
    let diagnostics = docking_runtime_diagnostics(status);
    let summary =
        DockingRuntimeSummary::from_status(status, runtime_events.len(), diagnostics.len());

    DockingRuntimeInspection {
        runtime_target_id,
        domain_id,
        summary,
        platform_capabilities: status
            .platform_capabilities
            .map(DockingPlatformCapabilityRow::from),
        window_profiles: status
            .window_profiles
            .iter()
            .map(DockingViewportWindowProfileRow::from)
            .collect(),
        placement_restore: status
            .placement_restore
            .map(DockingPlacementRestoreRow::from),
        viewport_lifecycle: status
            .viewport_lifecycle
            .iter()
            .enumerate()
            .map(|(index, lifecycle)| DockingViewportLifecycleRow::from_lifecycle(index, lifecycle))
            .collect(),
        runtime_events,
        visual_affordances: status
            .visual_affordances
            .iter()
            .enumerate()
            .map(|(index, affordance)| DockingVisualAffordanceRow::from_record(index, affordance))
            .collect(),
        diagnostics,
    }
}

impl DockingRuntimeSummary {
    fn from_status(
        status: &DockViewportRuntimeStatus,
        runtime_event_count: usize,
        diagnostic_count: usize,
    ) -> Self {
        Self {
            platform_capabilities_present: status.platform_capabilities.is_some(),
            platform_viewport_windows: status
                .platform_capabilities
                .map(|capabilities| capabilities.platform_viewport_windows),
            placement_restore_present: status.placement_restore.is_some(),
            viewport_lifecycle_count: status.viewport_lifecycle.len(),
            window_profile_count: status.window_profiles.len(),
            route_ready_count: status
                .viewport_lifecycle
                .iter()
                .filter(|lifecycle| {
                    matches!(lifecycle.route_status, DockViewportRouteStatus::RouteReady)
                })
                .count(),
            stale_viewport_count: status
                .viewport_lifecycle
                .iter()
                .filter(|lifecycle| {
                    matches!(
                        lifecycle.route_status,
                        DockViewportRouteStatus::Stale { .. }
                    )
                })
                .count(),
            missing_route_facts_count: status
                .viewport_lifecycle
                .iter()
                .filter(|lifecycle| {
                    matches!(
                        lifecycle.route_status,
                        DockViewportRouteStatus::MissingRouteFacts
                    )
                })
                .count(),
            runtime_event_count,
            visual_affordance_count: status.visual_affordances.len(),
            diagnostic_count,
        }
    }
}

impl From<DockViewportPlatformCapabilityRecord> for DockingPlatformCapabilityRow {
    fn from(capabilities: DockViewportPlatformCapabilityRecord) -> Self {
        Self {
            platform_viewport_windows: capabilities.platform_viewport_windows,
            global_window_bounds: capabilities.global_window_bounds,
            window_stack: capabilities.window_stack,
            display_work_area: capabilities.display_work_area,
            dpi_scale: capabilities.dpi_scale,
            hovered_window_ignores_no_input: capabilities.hovered_window_ignores_no_input,
        }
    }
}

impl From<PlatformWindowCapabilities> for DockingWindowCapabilitiesRow {
    fn from(capabilities: PlatformWindowCapabilities) -> Self {
        Self {
            creation: capabilities.creation.into(),
            mutations: capabilities.mutations.into(),
        }
    }
}

impl From<PlatformWindowCreationCapabilities> for DockingWindowCreationCapabilityRow {
    fn from(capabilities: PlatformWindowCreationCapabilities) -> Self {
        Self {
            focus_on_appearing: capabilities.focus_on_appearing.into(),
            transient_for: capabilities.transient_for.into(),
            initial_presentation_order: capabilities.initial_presentation_order.into(),
        }
    }
}

impl From<PlatformWindowMutationCapabilities> for DockingWindowMutationCapabilityRow {
    fn from(capabilities: PlatformWindowMutationCapabilities) -> Self {
        Self {
            position: capabilities.position.into(),
            size: capabilities.size.into(),
            windowed: capabilities.windowed.into(),
            maximized: capabilities.maximized.into(),
            fullscreen: capabilities.fullscreen.into(),
            minimized: capabilities.minimized.into(),
            restore_bounds: capabilities.restore_bounds.into(),
            pointer_input: capabilities.pointer_input.into(),
            activation_policy: capabilities.activation_policy.into(),
            alpha: capabilities.alpha.into(),
            topmost: capabilities.topmost.into(),
            taskbar_visibility: capabilities.taskbar_visibility.into(),
            coordinate_space: capabilities.coordinate_space.into(),
        }
    }
}

impl From<&DockViewportWindowProfileRecord> for DockingViewportWindowProfileRow {
    fn from(record: &DockViewportWindowProfileRecord) -> Self {
        Self {
            space: sanitize_sensitive_text(record.space.as_str()),
            window_id: record.window_id.as_u64(),
            window_kind: record.window_kind.as_str().to_string(),
            capabilities: DockingWindowCapabilitiesRow::from(record.capabilities),
        }
    }
}

impl From<WindowCreationSupport> for DockingWindowCreationSupport {
    fn from(support: WindowCreationSupport) -> Self {
        match support {
            WindowCreationSupport::Unsupported => Self::Unsupported,
            WindowCreationSupport::Supported => Self::Supported,
        }
    }
}

impl From<WindowInitialPresentationOrder> for DockingWindowInitialPresentationOrder {
    fn from(order: WindowInitialPresentationOrder) -> Self {
        match order {
            WindowInitialPresentationOrder::BeforeVisibility => Self::BeforeVisibility,
            WindowInitialPresentationOrder::AfterVisibility => Self::AfterVisibility,
            WindowInitialPresentationOrder::PresentationEstablishesVisibility => {
                Self::PresentationEstablishesVisibility
            }
        }
    }
}

impl From<WindowMutationSupport> for DockingWindowMutationSupport {
    fn from(support: WindowMutationSupport) -> Self {
        match support {
            WindowMutationSupport::Unsupported => Self::Unsupported,
            WindowMutationSupport::CreationOnly => Self::CreationOnly,
            WindowMutationSupport::Live => Self::Live,
        }
    }
}

impl From<WindowCoordinateSpace> for DockingWindowCoordinateSpace {
    fn from(space: WindowCoordinateSpace) -> Self {
        match space {
            WindowCoordinateSpace::WindowLocal => Self::WindowLocal,
            WindowCoordinateSpace::GlobalScreen => Self::GlobalScreen,
        }
    }
}

impl From<DockViewportRestoreReadinessRecord> for DockingPlacementRestoreRow {
    fn from(restore: DockViewportRestoreReadinessRecord) -> Self {
        Self {
            matched: restore.matched,
            missing: restore.missing,
            has_missing: restore.missing > 0,
        }
    }
}

impl DockingViewportLifecycleRow {
    fn from_lifecycle(index: usize, lifecycle: &DockViewportLifecycleRecord) -> Self {
        Self {
            target_id: lifecycle_target_id(index, lifecycle),
            index,
            space: sanitize_sensitive_text(lifecycle.space.as_str()),
            window_id: lifecycle.window_id.as_u64(),
            route_status: route_status_label(&lifecycle.route_status).to_owned(),
            input_status: input_status_label(lifecycle.input_status).to_owned(),
            close_requested: lifecycle.platform_request_status.close_requested,
            resize_requested: lifecycle.platform_request_status.resize_requested,
            facts_generation: lifecycle.facts_generation,
            display_id: lifecycle
                .coordinate_status
                .as_ref()
                .and_then(|status| status.display_id)
                .map(|display_id| sanitize_sensitive_text(&format!("{display_id:?}"))),
            coordinate_space: lifecycle
                .coordinate_status
                .as_ref()
                .map(|status| sanitize_sensitive_text(&format!("{:?}", status.coordinate_space))),
            coordinate_facts_generation: lifecycle
                .coordinate_status
                .as_ref()
                .map(|status| status.facts_generation),
        }
    }
}

impl DockingVisualAffordanceRow {
    fn from_record(index: usize, record: &DockViewportVisualAffordanceRecord) -> Self {
        let active = record.summary.active.as_ref();

        Self {
            target_id: visual_affordance_target_id(index, record),
            index,
            space: sanitize_sensitive_text(record.space.as_str()),
            window_id: record.window_id.as_u64(),
            summary_space: record.summary.space.as_deref().map(sanitize_sensitive_text),
            frame_generation: record.summary.frame_generation,
            layer_count: record.summary.layer_count,
            active_count: record.summary.active_count,
            active_layer_id: active.map(|layer| sanitize_sensitive_text(&layer.id)),
            active_layer_kind: active.map(|layer| sanitize_sensitive_text(&layer.kind)),
            active_layer_scope: active.map(|layer| sanitize_sensitive_text(&layer.scope)),
            active_layer_state: active.map(|layer| sanitize_sensitive_text(&layer.state)),
            active_target_node: active.and_then(|layer| layer.target_node),
            active_zone: active
                .and_then(|layer| layer.zone.as_ref())
                .map(|zone| sanitize_sensitive_text(&format!("{zone:?}"))),
            active_payload_index: active.and_then(|layer| layer.payload_index),
            active_has_label: active.is_some_and(|layer| layer.label.is_some()),
            motion_state: record
                .summary
                .motion_state
                .as_deref()
                .map(sanitize_sensitive_text),
            churn_signature: sanitize_sensitive_text(&record.summary.churn_signature),
        }
    }
}

fn docking_runtime_diagnostics(status: &DockViewportRuntimeStatus) -> Vec<SnapshotDiagnostic> {
    let mut diagnostics = Vec::new();

    if status
        .platform_capabilities
        .is_some_and(|capabilities| !capabilities.platform_viewport_windows)
    {
        diagnostics.push(SnapshotDiagnostic::new(
            docking_runtime_probe_id(),
            DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED,
            "platform viewport windows are unsupported by current platform capabilities",
        ));
    }

    for lifecycle in &status.viewport_lifecycle {
        match lifecycle.route_status {
            DockViewportRouteStatus::MissingRouteFacts => {
                diagnostics.push(SnapshotDiagnostic::new(
                    docking_runtime_probe_id(),
                    DOCKING_VIEWPORT_ROUTE_FACTS_MISSING,
                    format!(
                        "viewport `{}` is registered but has no route facts",
                        lifecycle.space.as_str()
                    ),
                ));
            }
            DockViewportRouteStatus::Stale { reason } => {
                diagnostics.push(SnapshotDiagnostic::new(
                    docking_runtime_probe_id(),
                    DOCKING_VIEWPORT_ROUTE_FACTS_STALE,
                    format!(
                        "viewport `{}` has stale route facts: {}",
                        lifecycle.space.as_str(),
                        stale_reason_label(reason)
                    ),
                ));
            }
            DockViewportRouteStatus::RegisteredNotReady
            | DockViewportRouteStatus::RouteReady
            | DockViewportRouteStatus::Minimized => {}
        }
    }

    diagnostics
}

fn docking_runtime_event_rows(
    status: &DockViewportRuntimeStatus,
    target_id: &DevtoolsTargetId,
    domain_id: &DevtoolsDomainId,
) -> Vec<DockingRuntimeEventRow> {
    let mut rows = Vec::new();
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
        "docking.last-route",
        "Last viewport route",
        status.last_route.as_ref().map(route_payload),
    );
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
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
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
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
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
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
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
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
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
        "docking.last-tear-off",
        "Last tear-off",
        status.last_tear_off.as_ref().map(tear_off_payload),
    );
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
        "docking.last-platform-dispatch",
        "Last platform dispatch",
        status
            .last_platform_dispatch
            .as_ref()
            .map(platform_sync_payload),
    );
    push_optional_runtime_event_row(
        &mut rows,
        target_id,
        domain_id,
        "docking.platform-observations",
        "Recent platform observations",
        (!status.recent_platform_observations.is_empty())
            .then(|| platform_observations_payload(&status.recent_platform_observations)),
    );
    rows
}

fn push_optional_runtime_event_row(
    rows: &mut Vec<DockingRuntimeEventRow>,
    target_id: &DevtoolsTargetId,
    domain_id: &DevtoolsDomainId,
    event_id: &'static str,
    label: &'static str,
    payload: Option<serde_json::Value>,
) {
    if let Some(payload) = payload {
        rows.push(DockingRuntimeEventRow {
            event_id: sanitize_sensitive_text(event_id),
            label: sanitize_sensitive_text(label),
            target_id: target_id.clone(),
            domain_id: domain_id.clone(),
            payload,
        });
    }
}

fn docking_runtime_snapshot_envelope(status: &DockViewportRuntimeStatus) -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        docking_runtime_probe_id(),
        SnapshotKind::Docking,
        docking_runtime_tree(status),
    )
    .with_redaction(SnapshotRedactionSummary::default())
}

fn docking_runtime_probe_id() -> ProbeId {
    ProbeId::new(DOCKING_RUNTIME_PROBE_ID).expect("internal docking runtime probe id is non-empty")
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
    for (index, profile) in status.window_profiles.iter().enumerate() {
        let index_label = index.to_string();
        root = root.with_child(snapshot_node_with_payload(
            [
                "docking",
                "viewport-runtime",
                "window-profiles",
                index_label.as_str(),
            ],
            format!("Window profile {index}"),
            window_profile_payload(profile),
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
        "last-platform-dispatch",
        &status.last_platform_dispatch,
        platform_sync_payload,
    );
    if !status.recent_platform_observations.is_empty() {
        root.children.push(snapshot_node_with_payload(
            ["docking", "viewport-runtime", "platform-observations"],
            "Recent platform observations",
            platform_observations_payload(&status.recent_platform_observations),
        ));
    }

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
        "window_profile_count": status.window_profiles.len(),
        "has_placement_restore": status.placement_restore.is_some(),
        "viewport_lifecycle_count": status.viewport_lifecycle.len(),
        "has_last_route": status.last_route.is_some(),
        "has_last_drop_outcome": status.last_drop_outcome.is_some(),
        "has_last_activation": status.last_activation.is_some(),
        "has_last_close": status.last_close.is_some(),
        "has_last_should_close": status.last_should_close.is_some(),
        "has_last_tear_off": status.last_tear_off.is_some(),
        "has_last_platform_dispatch": status.last_platform_dispatch.is_some(),
        "recent_platform_observation_count": status.recent_platform_observations.len(),
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
        "hovered_window_ignores_no_input": capabilities.hovered_window_ignores_no_input,
    })
}

fn window_profile_payload(profile: &DockViewportWindowProfileRecord) -> serde_json::Value {
    serde_json::json!(DockingViewportWindowProfileRow::from(profile))
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
        "dispatch_count": record.dispatches.len(),
        "observation_count": record.observations.len(),
        "dispatches": record
            .dispatches
            .iter()
            .map(platform_sync_dispatch_payload)
            .collect::<Vec<_>>(),
        "observations": record.observations.iter().map(|observation| {
            serde_json::json!({
                "domain": format!("{:?}", observation.domain),
                "generation": observation.generation,
                "request": window_mutation_request_payload(&observation.request),
                "outcome": format!("{:?}", observation.outcome),
                "facts": window_platform_facts_payload(&observation.facts),
            })
        }).collect::<Vec<_>>(),
    })
}

fn platform_sync_dispatch_payload(
    dispatch: &DockViewportPlatformSyncDispatch,
) -> serde_json::Value {
    match dispatch {
        DockViewportPlatformSyncDispatch::Immediate { action } => serde_json::json!({
            "kind": "immediate",
            "action": format!("{action:?}"),
        }),
        DockViewportPlatformSyncDispatch::Queued {
            request,
            domain,
            generation,
        } => serde_json::json!({
            "kind": "queued",
            "request": platform_sync_request_payload(request),
            "domain": format!("{domain:?}"),
            "generation": generation,
        }),
        DockViewportPlatformSyncDispatch::Unchanged { request } => serde_json::json!({
            "kind": "unchanged",
            "request": platform_sync_request_payload(request),
        }),
        DockViewportPlatformSyncDispatch::Unsupported(unsupported) => serde_json::json!({
            "kind": "unsupported",
            "request": platform_sync_request_payload(&unsupported.request),
            "reason": format!("{:?}", unsupported.reason),
        }),
        DockViewportPlatformSyncDispatch::Rejected(rejected) => serde_json::json!({
            "kind": "rejected",
            "request": platform_sync_request_payload(&rejected.request),
            "reason": format!("{:?}", rejected.reason),
        }),
        DockViewportPlatformSyncDispatch::WindowClosed { request } => serde_json::json!({
            "kind": "window-closed",
            "request": platform_sync_request_payload(request),
        }),
    }
}

fn platform_sync_request_payload(request: &DockViewportPlatformSyncRequest) -> serde_json::Value {
    match request {
        DockViewportPlatformSyncRequest::WindowUnavailable => {
            serde_json::json!({ "kind": "window-unavailable" })
        }
        DockViewportPlatformSyncRequest::Show { requested } => {
            serde_json::json!({ "kind": "show", "requested": requested })
        }
        DockViewportPlatformSyncRequest::WindowKind => {
            serde_json::json!({ "kind": "window-kind" })
        }
        DockViewportPlatformSyncRequest::Movable { requested } => {
            serde_json::json!({ "kind": "movable", "requested": requested })
        }
        DockViewportPlatformSyncRequest::Resizable { requested } => {
            serde_json::json!({ "kind": "resizable", "requested": requested })
        }
        DockViewportPlatformSyncRequest::Minimizable { requested } => {
            serde_json::json!({ "kind": "minimizable", "requested": requested })
        }
        DockViewportPlatformSyncRequest::PointerInput { requested } => {
            serde_json::json!({ "kind": "pointer-input", "requested": requested })
        }
        DockViewportPlatformSyncRequest::ActivationPolicy { requested } => serde_json::json!({
            "kind": "activation-policy",
            "accepts_activation": requested.accepts_activation,
            "focus_on_click": requested.focus_on_click,
        }),
        DockViewportPlatformSyncRequest::BackgroundAppearance { requested } => {
            serde_json::json!({
                "kind": "background-appearance",
                "requested": format!("{requested:?}"),
            })
        }
        DockViewportPlatformSyncRequest::Display { requested } => {
            serde_json::json!({ "kind": "display", "requested": u64::from(*requested) })
        }
        DockViewportPlatformSyncRequest::WindowMinSize { requested } => {
            serde_json::json!({ "kind": "window-min-size", "requested": requested })
        }
        DockViewportPlatformSyncRequest::Placement { requested } => {
            let state = match requested {
                open_gpui::WindowBounds::Windowed(_) => "windowed",
                open_gpui::WindowBounds::Maximized(_) => "maximized",
                open_gpui::WindowBounds::Fullscreen(_) => "fullscreen",
            };
            serde_json::json!({
                "kind": "placement",
                "state": state,
                "bounds": requested.get_bounds(),
            })
        }
        DockViewportPlatformSyncRequest::Icon => serde_json::json!({ "kind": "icon" }),
        DockViewportPlatformSyncRequest::TabbingIdentifier { requested } => serde_json::json!({
            "kind": "tabbing-identifier",
            "requested": requested,
        }),
        DockViewportPlatformSyncRequest::TitlebarPresence { requested } => serde_json::json!({
            "kind": "titlebar-presence",
            "requested": requested,
        }),
        DockViewportPlatformSyncRequest::TitlebarTransparency { requested } => serde_json::json!({
            "kind": "titlebar-transparency",
            "requested": requested,
        }),
        DockViewportPlatformSyncRequest::TrafficLightPosition { requested } => serde_json::json!({
            "kind": "traffic-light-position",
            "requested": requested,
        }),
        request => serde_json::json!({
            "kind": "unknown",
            "debug": format!("{request:?}"),
        }),
    }
}

fn platform_observations_payload(
    observations: &[DockViewportPlatformSyncObservedRecord],
) -> serde_json::Value {
    serde_json::json!(
        observations
            .iter()
            .map(|record| {
                serde_json::json!({
                    "window_id": record.window_id.as_u64(),
                    "domain": format!("{:?}", record.observation.domain),
                    "generation": record.observation.generation,
                    "request": window_mutation_request_payload(&record.observation.request),
                    "outcome": format!("{:?}", record.observation.outcome),
                    "facts": window_platform_facts_payload(&record.observation.facts),
                })
            })
            .collect::<Vec<_>>()
    )
}

fn window_mutation_request_payload(request: &WindowMutationRequest) -> serde_json::Value {
    match request {
        WindowMutationRequest::Placement(request) => serde_json::json!({
            "kind": "placement",
            "position": request.position,
            "size": request.size,
            "state": request.state.map(window_placement_state_label),
            "restore_bounds": request.restore_bounds,
        }),
        WindowMutationRequest::PointerInput(accepts_pointer_input) => serde_json::json!({
            "kind": "pointer-input",
            "accepts_pointer_input": accepts_pointer_input,
        }),
        WindowMutationRequest::ActivationPolicy(policy) => serde_json::json!({
            "kind": "activation-policy",
            "accepts_activation": policy.accepts_activation,
            "focus_on_click": policy.focus_on_click,
        }),
        WindowMutationRequest::Alpha(background) => serde_json::json!({
            "kind": "alpha",
            "background": format!("{background:?}"),
        }),
        WindowMutationRequest::Topmost(topmost) => serde_json::json!({
            "kind": "topmost",
            "topmost": topmost,
        }),
        WindowMutationRequest::TaskbarVisibility(visible) => serde_json::json!({
            "kind": "taskbar-visibility",
            "visible": visible,
        }),
    }
}

fn window_platform_facts_payload(facts: &WindowPlatformFacts) -> serde_json::Value {
    serde_json::json!({
        "bounds": facts.bounds,
        "coordinate_space": match facts.coordinate_space {
            WindowCoordinateSpace::WindowLocal => "window-local",
            WindowCoordinateSpace::GlobalScreen => "global-screen",
        },
        "window_state": if facts.is_minimized {
            "minimized"
        } else if facts.is_fullscreen {
            "fullscreen"
        } else if facts.is_maximized {
            "maximized"
        } else {
            "windowed"
        },
        "restore_bounds": facts.window_bounds.get_bounds(),
        "inner_window_bounds": facts.inner_window_bounds.get_bounds(),
        "content_size": facts.content_size,
        "scale_factor": facts.scale_factor,
        "display_id": facts.display_id.map(u64::from),
        "accepts_pointer_input": facts.accepts_pointer_input,
        "accepts_activation": facts.accepts_activation,
        "focus_on_click": facts.focus_on_click,
        "background_appearance": format!("{:?}", facts.background_appearance),
        "topmost": facts.topmost,
        "taskbar_visible": facts.taskbar_visible,
        "is_active": facts.is_active,
    })
}

fn window_placement_state_label(state: WindowPlacementState) -> &'static str {
    match state {
        WindowPlacementState::Windowed => "windowed",
        WindowPlacementState::Maximized => "maximized",
        WindowPlacementState::Fullscreen => "fullscreen",
        WindowPlacementState::Minimized => "minimized",
    }
}

fn visual_affordance_payload(record: &DockViewportVisualAffordanceRecord) -> serde_json::Value {
    let active = record.summary.active.as_ref();
    serde_json::json!({
        "space": record.space.as_str(),
        "window_id": record.window_id.as_u64(),
        "summary_space": record.summary.space.as_deref(),
        "frame_generation": record.summary.frame_generation,
        "layer_count": record.summary.layer_count,
        "active_count": record.summary.active_count,
        "active": active.map(|layer| serde_json::json!({
            "id": layer.id.as_str(),
            "kind": layer.kind.as_str(),
            "scope": layer.scope.as_str(),
            "state": layer.state.as_str(),
            "target_node": layer.target_node,
            "zone": layer.zone.as_ref().map(|zone| format!("{zone:?}")),
            "payload_index": layer.payload_index,
            "has_label": layer.label.is_some(),
        })),
        "motion_state": record.summary.motion_state.as_deref(),
        "churn_signature": record.summary.churn_signature.as_str(),
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

fn stale_reason_label(reason: DockViewportStaleStatusReason) -> &'static str {
    match reason {
        DockViewportStaleStatusReason::WindowFactsChanged => "window-facts-changed",
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
