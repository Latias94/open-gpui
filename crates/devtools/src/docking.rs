//! Pure DevTools projections of the shared dockspace session's owned inspection.

use open_gpui::WindowFrameSubmissionOutcome;
use open_gpui_docking::{DockInspectionError, DockNativeInspection, DockSurfaceInspection};
use serde::{Deserialize, Serialize};

use crate::{
    CaptureProvider, DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId,
    ProbeSnapshotError, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{opaque_stable_id, sanitize_sensitive_text, snapshot_node_with_payload},
};

/// Diagnostic code for the host's latest retained transport error.
pub const DOCKING_HOST_ERROR: &str = "docking.host.error";

/// Sanitized projection of one shared workspace, without runtime authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingSurfaceInspection {
    /// Stable workspace target identity.
    pub surface_target_id: DevtoolsTargetId,
    /// Stable docking domain identity.
    pub domain_id: DevtoolsDomainId,
    /// Model counts and pending-work facts.
    pub summary: DockingSurfaceSummary,
    /// Committed model surfaces, without application titles or external keys.
    pub surfaces: Vec<DockingModelSurfaceRow>,
    /// Existing physical mounts and their exact submission facts.
    pub mounts: Vec<DockingMountRow>,
    /// Managed owner facts, absent when native management is disabled.
    pub native: Option<DockingNativeRow>,
    /// Latest retained error with sensitive text sanitized.
    pub last_error: Option<String>,
    /// Text redactions performed before constructing this projection.
    pub redaction: SnapshotRedactionSummary,
}

/// Counts and scheduling facts read from the existing session and host.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingSurfaceSummary {
    /// Actual number of committed logical surfaces.
    pub model_surface_count: usize,
    /// Actual number of open panes in the committed topology.
    pub open_pane_count: usize,
    /// Retained application views, including registered closed panes.
    pub registered_pane_count: usize,
    /// Number of retained physical mounts, including detached mounts.
    pub physical_mount_count: usize,
    /// Whether an unaccepted candidate owns the semantic update lease.
    pub update_pending: bool,
    /// Whether the session owns a pointer gesture.
    pub active_gesture: bool,
    /// Whether the session retains unresolved presentation work.
    pub pending_presentation: bool,
    /// Whether host presentation observations await settlement.
    pub pending_presentation_reports: bool,
    /// Local pointer inputs outside physical candidates.
    pub queued_input_count: usize,
    /// Native effects retained by the unmanaged host.
    pub queued_native_effect_count: usize,
    /// Committed native window bindings retained by the session.
    pub native_window_binding_count: usize,
    /// Whether recorded native input awaits commit.
    pub pending_native_input: bool,
    /// Whether native creation retains preparation or first-live obligations.
    pub pending_native_create: bool,
    /// Whether this workspace has a managed native host.
    pub managed_native: bool,
}

/// A committed logical surface with an opaque diagnostic identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingModelSurfaceRow {
    /// Stable opaque surface target identity.
    pub target_id: DevtoolsTargetId,
    /// Number of open panes across main and contained roots.
    pub open_pane_count: usize,
    /// Number of contained floating presentations.
    pub contained_count: usize,
}

/// A physical mount without native handles, tickets or callback authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingMountRow {
    /// Stable opaque physical window identity.
    pub target_id: DevtoolsTargetId,
    /// Whether the docking element remains attached.
    pub attached: bool,
    /// Current physical candidate serial.
    pub serial: u64,
    /// Whether another mount's semantic update blocks this mount.
    pub waiting_for_update: bool,
    /// Whether this mount's candidate holds an unaccepted update.
    pub update_pending: bool,
    /// Whether the managed host retains a callback binding for this mount.
    pub native_binding_present: bool,
    /// Facts from the pending original ticket, absent when no ticket is retained.
    pub submission: Option<DockingSubmissionRow>,
    /// Physical regions and their retained input/presentation correlations.
    pub regions: Vec<DockingRegionRow>,
}

/// Exact GPUI ticket evidence, preserving acceptance and submission separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingSubmissionRow {
    /// First frame eligible to carry the candidate.
    pub minimum_frame_generation: u64,
    /// Exact accepted generation, when observed.
    pub accepted_frame_generation: Option<u64>,
    /// Exact submitted generation, when observed.
    pub submitted_frame_generation: Option<u64>,
    /// Structured terminal or pending outcome of the original ticket.
    pub outcome: DockingSubmissionOutcome,
}

/// Structured projection of GPUI's exact frame submission outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockingSubmissionOutcome {
    /// The candidate has not reached an accepted frame.
    Pending,
    /// GPUI accepted the frame and renderer submission is still outstanding.
    Accepted,
    /// The renderer submitted the exact accepted frame.
    Submitted,
    /// The renderer terminally rejected the frame.
    Rejected,
    /// The window became terminal before submission.
    WindowTerminal,
}

/// A retained region, which can outlive its committed model surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingRegionRow {
    /// Stable opaque surface identity, shared with the model row when present.
    pub surface_target_id: DevtoolsTargetId,
    /// Whether this region needs a fresh encoding.
    pub dirty: bool,
    /// Whether a packet is retained and deferred program execution.
    pub packet_deferred: Option<bool>,
    /// Serial of the retained input frame, when present.
    pub input_serial: Option<u64>,
    /// Whether an exact native placement is retained by the input frame.
    pub input_native_binding_present: bool,
    /// Whether the retained native input binding is still current.
    pub input_binding_current: Option<bool>,
    /// Whether the retained input placement matches this mount's callback window.
    ///
    /// Absent when either binding is unavailable; no identity is inferred.
    pub input_matches_mount_binding: Option<bool>,
    /// Last exact presentation serial reported for this region.
    pub last_presented_serial: Option<u64>,
    /// Whether that presentation matches the current serial and session artifact.
    pub current_presentation: bool,
}

/// Counters scoped to the managed host's explicitly named retained owners.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockingNativeRow {
    /// Whether native admission is frozen for shutdown.
    pub admission_frozen: bool,
    /// Whether the bridge is available rather than checked out for dispatch.
    pub bridge_available: bool,
    /// Full host quiescence predicate observed at capture time.
    pub quiescent: bool,
    /// Whether the transport FIFO is empty.
    pub transport_empty: bool,
    /// Application-facing reports retained by the managed host.
    pub retained_report_count: usize,
    /// Committed host reports waiting for the bridge.
    pub pending_host_report_count: usize,
    /// Retained physical callback routes.
    pub callback_window_count: usize,
    /// Root window identities retained by the managed host.
    pub root_window_count: usize,
    /// Placement bindings awaiting host quiescence.
    pub retiring_binding_count: usize,
    /// Root registrations awaiting recovery.
    pub pending_root_registration_count: usize,
    /// Root rollback tickets retained by the managed host.
    pub root_retirement_count: usize,
    /// Those tickets without native terminal observations.
    pub pending_root_terminal_count: usize,
    /// Those tickets without presentation quiescence.
    pub pending_root_quiescence_count: usize,
    /// Whether a native pump is already scheduled.
    pub pump_scheduled: bool,
    /// Whether the host retains a pointer continuation task.
    pub continuation_pending: bool,
}

/// Projects an owned inspection without reading or advancing the live host.
pub fn docking_surface_inspection(source: &DockSurfaceInspection) -> DockingSurfaceInspection {
    inspection_with_namespace(source, "docking.surface")
}

/// Converts an owned inspection into targets, a docking domain and a snapshot.
pub fn docking_surface_capture(source: &DockSurfaceInspection) -> DevtoolsCapture {
    capture_with_namespace(source, "docking.surface")
}

/// Builds a read-only snapshot tree from the owned inspection.
pub fn docking_surface_probe_snapshot(source: &DockSurfaceInspection) -> SnapshotProbeSnapshot {
    let inspection = docking_surface_inspection(source);
    SnapshotProbeSnapshot::new(inspection_tree(&inspection)).with_redaction(inspection.redaction)
}

/// Creates a capture provider over application-published owned inspections.
///
/// The callback must satisfy the registry's `Send + Sync` contract. An application
/// can publish a successful `DockSurface::inspection()` through its own synchronized snapshot
/// storage; the GPUI surface itself remains on its owning thread. Provider ids
/// derive opaque namespaces before sanitization, preserving distinct identities.
/// A busy source returns a collection error without substituting a previous snapshot.
pub fn docking_surface_capture_provider<F>(
    id: impl Into<String>,
    inspection: F,
) -> Result<
    CaptureProvider<impl Fn() -> Result<DevtoolsCapture, ProbeSnapshotError>>,
    ProbeSnapshotError,
>
where
    F: Fn() -> Result<DockSurfaceInspection, DockInspectionError> + Send + Sync + 'static,
{
    let id = id.into();
    let namespace = format!(
        "docking.provider.{}.surface",
        opaque_stable_id("docking-provider", &id)
    );
    CaptureProvider::new(id, move || {
        let source = inspection()
            .map_err(|error| ProbeSnapshotError::CollectionFailed(error.to_string()))?;
        Ok(capture_with_namespace(&source, &namespace))
    })
}

fn inspection_with_namespace(
    source: &DockSurfaceInspection,
    namespace: &str,
) -> DockingSurfaceInspection {
    let mut redaction = SnapshotRedactionSummary::default();
    let last_error = source.last_error.as_ref().map(|error| {
        let sanitized = sanitize_sensitive_text(error);
        if &sanitized != error {
            redaction.record_redacted("Sensitive docking host error text was sanitized");
        }
        sanitized
    });
    DockingSurfaceInspection {
        surface_target_id: DevtoolsTargetId::new(namespace),
        domain_id: DevtoolsDomainId::new(namespace),
        summary: DockingSurfaceSummary {
            model_surface_count: source.model_surface_count,
            open_pane_count: source.open_pane_count,
            registered_pane_count: source.registered_pane_count,
            physical_mount_count: source.mounts.len(),
            update_pending: source.update_pending,
            active_gesture: source.active_gesture,
            pending_presentation: source.pending_presentation,
            pending_presentation_reports: source.pending_presentation_reports,
            queued_input_count: source.queued_input_count,
            queued_native_effect_count: source.queued_native_effect_count,
            native_window_binding_count: source.native_window_binding_count,
            pending_native_input: source.pending_native_input,
            pending_native_create: source.pending_native_create,
            managed_native: source.native.is_some(),
        },
        surfaces: source
            .surfaces
            .iter()
            .map(|surface| DockingModelSurfaceRow {
                target_id: model_target_id(namespace, surface.surface.get()),
                open_pane_count: surface.open_pane_count,
                contained_count: surface.contained_count,
            })
            .collect(),
        mounts: source
            .mounts
            .iter()
            .map(|mount| DockingMountRow {
                target_id: DevtoolsTargetId::new(format!(
                    "{namespace}.mount.{}",
                    opaque_stable_id("docking-window", &mount.window_id.to_string())
                )),
                attached: mount.attached,
                serial: mount.serial,
                waiting_for_update: mount.waiting_for_update,
                update_pending: mount.update_pending,
                native_binding_present: mount.native_binding.is_some(),
                submission: mount.submission.map(|submission| DockingSubmissionRow {
                    minimum_frame_generation: submission.minimum_frame_generation(),
                    accepted_frame_generation: submission.accepted_frame_generation(),
                    submitted_frame_generation: submission.submitted_frame_generation(),
                    outcome: match submission.outcome() {
                        WindowFrameSubmissionOutcome::Pending => DockingSubmissionOutcome::Pending,
                        WindowFrameSubmissionOutcome::Accepted => {
                            DockingSubmissionOutcome::Accepted
                        }
                        WindowFrameSubmissionOutcome::Submitted => {
                            DockingSubmissionOutcome::Submitted
                        }
                        WindowFrameSubmissionOutcome::Rejected => {
                            DockingSubmissionOutcome::Rejected
                        }
                        WindowFrameSubmissionOutcome::WindowTerminal => {
                            DockingSubmissionOutcome::WindowTerminal
                        }
                    },
                }),
                regions: mount
                    .regions
                    .iter()
                    .map(|region| DockingRegionRow {
                        surface_target_id: model_target_id(namespace, region.surface.get()),
                        dirty: region.dirty,
                        packet_deferred: region.packet_deferred,
                        input_serial: region.input_serial,
                        input_native_binding_present: region.input_native_binding.is_some(),
                        input_binding_current: region.input_binding_current,
                        input_matches_mount_binding: region
                            .input_native_binding
                            .zip(mount.native_binding)
                            .map(|(input, mounted)| input.window() == mounted),
                        last_presented_serial: region.last_presented_serial,
                        current_presentation: region.current_presentation,
                    })
                    .collect(),
            })
            .collect(),
        native: source.native.as_ref().map(DockingNativeRow::from),
        last_error,
        redaction,
    }
}

impl From<&DockNativeInspection> for DockingNativeRow {
    fn from(source: &DockNativeInspection) -> Self {
        Self {
            admission_frozen: source.admission_frozen,
            bridge_available: source.bridge_available,
            quiescent: source.quiescent,
            transport_empty: source.transport_empty,
            retained_report_count: source.retained_report_count,
            pending_host_report_count: source.pending_host_report_count,
            callback_window_count: source.callback_window_count,
            root_window_count: source.root_window_count,
            retiring_binding_count: source.retiring_binding_count,
            pending_root_registration_count: source.pending_root_registration_count,
            root_retirement_count: source.root_retirement_count,
            pending_root_terminal_count: source.pending_root_terminal_count,
            pending_root_quiescence_count: source.pending_root_quiescence_count,
            pump_scheduled: source.pump_scheduled,
            continuation_pending: source.continuation_pending,
        }
    }
}

fn model_target_id(namespace: &str, surface: u64) -> DevtoolsTargetId {
    DevtoolsTargetId::new(format!(
        "{namespace}.model.{}",
        opaque_stable_id("docking-surface", &surface.to_string())
    ))
}

fn inspection_tree(inspection: &DockingSurfaceInspection) -> SnapshotTree {
    let namespace = inspection.surface_target_id.as_str();
    let mut root =
        snapshot_node_with_payload([namespace], "Dockspace session", &inspection.summary);
    for surface in &inspection.surfaces {
        root = root.with_child(snapshot_node_with_payload(
            [surface.target_id.as_str()],
            "Logical surface",
            surface,
        ));
    }
    for mount in &inspection.mounts {
        root = root.with_child(snapshot_node_with_payload(
            [mount.target_id.as_str()],
            "Physical mount",
            mount,
        ));
    }
    if let Some(native) = &inspection.native {
        root = root.with_child(snapshot_node_with_payload(
            [namespace, "native"],
            "Managed native owners",
            native,
        ));
    }
    if let Some(error) = &inspection.last_error {
        root = root.with_child(
            SnapshotNode::new(format!("{namespace}.last-error"), "Last host error")
                .with_payload(serde_json::json!({ "message": error })),
        );
    }
    SnapshotTree::new([root])
}

fn capture_with_namespace(source: &DockSurfaceInspection, namespace: &str) -> DevtoolsCapture {
    let inspection = inspection_with_namespace(source, namespace);
    let probe_id = ProbeId::new(namespace).expect("internal docking probe identity is non-empty");
    let snapshot = SnapshotEnvelope::new(
        probe_id.clone(),
        SnapshotKind::Docking,
        inspection_tree(&inspection),
    )
    .with_redaction(inspection.redaction.clone());
    let mut targets = vec![
        DevtoolsTargetSnapshot::new(
            inspection.surface_target_id.clone(),
            DevtoolsTargetKind::Runtime,
            "Dockspace session",
        )
        .with_metadata(&inspection.summary),
    ];
    targets.extend(inspection.surfaces.iter().map(|surface| {
        DevtoolsTargetSnapshot::new(
            surface.target_id.clone(),
            DevtoolsTargetKind::Dockspace,
            "Logical surface",
        )
        .parent_id(inspection.surface_target_id.clone())
        .with_metadata(surface)
    }));
    targets.extend(inspection.mounts.iter().map(|mount| {
        DevtoolsTargetSnapshot::new(
            mount.target_id.clone(),
            DevtoolsTargetKind::Window,
            "Physical mount",
        )
        .parent_id(inspection.surface_target_id.clone())
        .with_metadata(mount)
    }));
    let diagnostics: Vec<_> = inspection
        .last_error
        .as_ref()
        .map(|error| SnapshotDiagnostic::new(probe_id, DOCKING_HOST_ERROR, error.clone()))
        .into_iter()
        .collect();
    let mut domain = DevtoolsDomainSnapshot::new(
        inspection.domain_id,
        inspection.surface_target_id,
        DevtoolsDomainKind::Docking,
        "Dockspace session",
    )
    .with_summary(&inspection.summary)
    .with_snapshot(snapshot.clone());
    for diagnostic in &diagnostics {
        domain = domain.with_diagnostic(diagnostic.clone());
    }
    DevtoolsCapture::new(
        DevtoolsTargetTree::new(targets),
        [domain],
        [],
        [snapshot],
        diagnostics,
    )
}
