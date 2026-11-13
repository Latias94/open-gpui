//! Durable documents and one-way migration from the retired docking wire formats.
//!
//! The embedded document is produced and restored exclusively by dockspace's public
//! `serde` API. Its append-only external-key map includes closed historical panes.
//! A pane key identifies a View occurrence, not the document shared by several Views.
//! Factories, `AnyView`, native handles, focus, and frame/submission proofs belong to
//! the host and never enter this envelope. Recognizers must be read-only; factories
//! run only after the complete restore has succeeded.
//!
//! Legacy migration requires an explicit [`LegacyTopologyMigration`] selection.
//! Binary v3 branches become weighted splits with shares `[ratio, 1 - ratio]`.
//! Dockspace may flatten adjacent splits on the same axis into canonical N-ary
//! splits. This preserves relative shares and child order, not binary structural
//! identity, pixel rounding, minimum-size behavior, or absolute `size_ref` behavior.
//! Legacy structural/group identities and exact size references survive in the
//! migration receipt, without becoming a second live graph. Noncentral v3 branches
//! use the ratio of both children's axis size references when their sum is positive,
//! matching the expressible part of the old ratio rule. Absolute sizes and central
//! sibling-size preservation cannot be represented by constant relative weights.
//! Floating identities have a recorded deterministic crosswalk; bounds and z-order
//! are retained. The receipt explicitly lists contracts requiring host decisions.
//! V2 keeps its ordered N-ary children, collapses unary splits, and resolves a
//! split-valued central marker to its last descendant leaf. Along that central
//! path, the central child receives the remainder after its siblings' fractions.
//! Unrepresentable zero-share children are rejected rather than dropped or widened.
//!
//! Legacy logical window coordinates have no persisted DPI. They stay logical
//! host hints and are never submitted as dockspace desktop-physical observations.
//! Central keep-alive/passthrough declarations are returned as host policy hints;
//! the host must select/apply its corresponding facade policy. Only the central
//! leaf itself is represented in the core. An entirely empty legacy space becomes
//! an empty central root so its external space key remains addressable.

use std::collections::{BTreeMap, BTreeSet};

use dockspace::geometry::LogicalRect;
use dockspace::model::{
    DockspaceAxis, DockspaceContainedLayout, DockspaceLayout, DockspaceLayoutError, DockspaceNode,
    DockspaceNodeView, DockspaceRootLayout, DockspaceSurfaceLayout, FloatingPresentationId, ItemId,
    RootId, SurfaceId,
};
use dockspace::policy::DockPolicy;
use dockspace::runtime::{
    DockspaceDocumentBootstrap, DockspaceDocumentId, DockspacePersistenceError, DockspaceSession,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Adapter envelope format identifier, independent of dockspace's document version.
pub const DOCK_WORKSPACE_DOCUMENT_FORMAT: &str = "open-gpui-dockspace";
/// The supported adapter envelope version.
pub const DOCK_WORKSPACE_DOCUMENT_VERSION: u32 = 1;

/// Explicit acceptance of the selected binary-to-N-ary persistence contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyTopologyMigration {
    /// Retain relative weights/order and adopt dockspace's canonical layout semantics.
    RelativeWeights,
}

/// A rectangle in legacy logical pixels, without current desktop/DPI authority.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLogicalPlacementRect {
    /// Logical horizontal origin; negative desktop coordinates are permitted.
    pub x: f32,
    /// Logical vertical origin.
    pub y: f32,
    /// Non-negative logical width.
    pub width: f32,
    /// Non-negative logical height.
    pub height: f32,
}

impl DockLogicalPlacementRect {
    fn validate(self) -> Result<(), DockPersistenceError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < 0.0
            || self.height < 0.0
        {
            return Err(invalid(
                "placement rectangles must be finite with non-negative sizes",
            ));
        }
        Ok(())
    }

    fn into_contained_rect(self) -> Result<LogicalRect, DockPersistenceError> {
        self.validate()?;
        if self.width == 0.0 || self.height == 0.0 {
            return Err(invalid(
                "dockspace contained floating bounds must have positive area",
            ));
        }
        LogicalRect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.width),
            f64::from(self.height),
        )
        .map_err(|error| invalid(error.to_string()))
    }
}

/// A durable window presentation preference, not a live native state observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockWindowPlacementState {
    /// Restore as an ordinary window.
    Windowed,
    /// Request maximization with the accompanying restore bounds.
    Maximized,
    /// Request fullscreen with the accompanying restore bounds.
    Fullscreen,
}

/// Saved logical restore geometry and a requested presentation state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLogicalWindowPlacement {
    /// Requested presentation state.
    pub state: DockWindowPlacementState,
    /// Logical restore bounds; the host resolves the current display and scale.
    pub bounds: DockLogicalPlacementRect,
}

/// Host-owned logical placement hints migrated without inventing native facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLogicalViewportPlacement {
    /// Historical display identifier, used only as a restore preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_id: Option<u64>,
    /// Historical logical window geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_bounds: Option<DockLogicalWindowPlacement>,
    /// Historical logical host rectangle within its window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_bounds: Option<DockLogicalPlacementRect>,
}

impl DockLogicalViewportPlacement {
    fn validate(&self) -> Result<(), DockPersistenceError> {
        if let Some(window) = self.window_bounds {
            window.bounds.validate()?;
        }
        if let Some(bounds) = self.host_bounds {
            bounds.validate()?;
        }
        Ok(())
    }
}

/// Legacy facade declarations that require a host policy decision on adoption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockCentralPolicyHint {
    /// Whether the source facade retained an empty central region.
    pub keep_alive_when_empty: bool,
    /// Whether the source facade let input pass through an empty center.
    pub passthrough_when_empty: bool,
}

/// A stable external space key bound to the document's logical core surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockSurfaceKeyBinding {
    /// Exact application-owned space key.
    pub space_key: String,
    /// Logical surface identity from the complete core document.
    pub surface: SurfaceId,
    /// Optional logical placement hints, independent of native bindings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_placement: Option<DockLogicalViewportPlacement>,
    /// Optional source facade policy; this is not a second content graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub central_policy: Option<DockCentralPolicyHint>,
}

/// An exact legacy size reference retained as migration provenance.
///
/// The reference is not an absolute-size constraint on the new core. Where both
/// children of a noncentral v3 branch have references with a positive axis sum,
/// that pair determines the branch's initial relative weights.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLegacySizeReference {
    /// Historical logical width.
    pub width: f32,
    /// Historical logical height.
    pub height: f32,
}

impl DockLegacySizeReference {
    fn validate(self) -> Result<(), DockPersistenceError> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < 0.0
            || self.height < 0.0
        {
            return Err(invalid(
                "legacy size references must be finite and non-negative",
            ));
        }
        Ok(())
    }

    fn axis_size(self, axis: LegacyAxis) -> f32 {
        match axis {
            LegacyAxis::Horizontal => self.width,
            LegacyAxis::Vertical => self.height,
        }
    }
}

/// Original node identity and optional v3 metadata, without source graph edges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLegacyNodeIdentity {
    /// Original snapshot-local structural id; never a live dockspace node key.
    pub source_node: u32,
    /// Original v3 durable group id, which dockspace has no stable group-id slot for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// Exact original v3 size reference, preserved even when only partly expressible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_reference: Option<DockLegacySizeReference>,
}

/// Historical floating identity crosswalk, fixed at migration time.
///
/// These initial identities are provenance, not current ownership after later
/// closes, moves or live restores. Current content ownership is queried in the core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLegacyFloatingIdentity {
    /// Exact external source space key.
    pub space_key: String,
    /// Original structural root id.
    pub source_root: u32,
    /// Original entry ordinal, including for v2 which has no separate floating id.
    pub source_ordinal: usize,
    /// Exact v3 floating id, absent for v2.
    pub source_id: Option<String>,
    /// Exact v3 z-order; v2 instead uses the entry ordinal.
    pub source_z_order: Option<i32>,
    /// Stable floating identity initially assigned in the candidate document.
    pub initial_floating: FloatingPresentationId,
    /// Stable content-root identity initially assigned in the candidate document.
    pub initial_root: RootId,
}

/// A deliberately recorded behavior/identity difference, never a runtime fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockLegacyContractDifference {
    /// The sole runtime canonicalizes to weighted N-ary topology and fresh private node ids.
    CanonicalNaryTopology,
    /// Relative geometry replaces old rounding, absolute-size and central sibling-size rules.
    RelativeGeometry,
    /// Original stable group ids survive as provenance, without a core group-id equivalent.
    GroupIdentityIsProvenance,
    /// Original floating ids and numeric z-order survive in the historical crosswalk.
    FloatingIdentityCrosswalk,
    /// Keep-alive/passthrough hints require a host facade-policy decision.
    CentralPolicyRequiresHost,
    /// Logical placement hints require fresh host display/DPI/native observations.
    LogicalPlacementRequiresHost,
    /// An empty legacy space is represented by an empty core central root.
    EmptySpaceUsesCentralRoot,
}

/// Durable receipt of the explicitly selected legacy conversion semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLegacyMigration {
    /// Source layout schema, either 2 or 3.
    pub layout_version: u32,
    /// Source snapshot revision, absent for a standalone layout.
    pub snapshot_revision: Option<u64>,
    /// Accepted topology and geometry contract.
    pub topology: LegacyTopologyMigration,
    /// Original structural ids, group ids and exact size references, without topology.
    pub source_nodes: Vec<DockLegacyNodeIdentity>,
    /// Historical source-to-candidate floating identity assignments.
    pub floating_identities: Vec<DockLegacyFloatingIdentity>,
    /// Explicit non-equivalences and host policy work required by this selection.
    pub differences: Vec<DockLegacyContractDifference>,
}

/// Host-owned durable aliases and hints paired with one complete core document.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockHostDocument {
    /// Exactly one external binding for each logical core surface.
    pub surfaces: Vec<DockSurfaceKeyBinding>,
    /// Optional one-way migration receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<DockLegacyMigration>,
}

impl DockHostDocument {
    fn validate(&self) -> Result<(), DockPersistenceError> {
        let mut keys = BTreeSet::new();
        let mut surfaces = BTreeSet::new();
        for binding in &self.surfaces {
            if binding.space_key.is_empty() || !keys.insert(binding.space_key.as_str()) {
                return Err(invalid("surface keys must be nonempty and unique"));
            }
            if !surfaces.insert(binding.surface) {
                return Err(invalid(
                    "a logical surface has more than one external binding",
                ));
            }
            if let Some(placement) = &binding.logical_placement {
                placement.validate()?;
            }
        }
        if let Some(migration) = &self.migration
            && !matches!(migration.layout_version, 2 | 3)
        {
            return Err(DockPersistenceError::UnsupportedLegacyVersion {
                found: migration.layout_version,
            });
        }
        if let Some(migration) = &self.migration {
            let mut nodes = BTreeSet::new();
            let mut groups = BTreeSet::new();
            for node in &migration.source_nodes {
                if !nodes.insert(node.source_node) {
                    return Err(invalid("duplicate structural id in migration provenance"));
                }
                if let Some(group) = &node.group_id
                    && (group.is_empty() || !groups.insert(group.as_str()))
                {
                    return Err(invalid(
                        "migration group identities must be nonempty and unique",
                    ));
                }
                if let Some(size) = node.size_reference {
                    size.validate()?;
                }
            }
            let mut floating_ids = BTreeSet::new();
            let mut initial_ids = BTreeSet::new();
            let mut ordinals = BTreeSet::new();
            for floating in &migration.floating_identities {
                if floating.space_key.is_empty()
                    || !nodes.contains(&floating.source_root)
                    || !initial_ids.insert(floating.initial_floating)
                    || !ordinals.insert((floating.space_key.as_str(), floating.source_ordinal))
                {
                    return Err(invalid("invalid floating identity crosswalk"));
                }
                if let Some(id) = &floating.source_id
                    && (id.is_empty() || !floating_ids.insert(id.as_str()))
                {
                    return Err(invalid(
                        "migration floating identities must be nonempty and unique",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_session(&self, session: &DockspaceSession) -> Result<(), DockPersistenceError> {
        self.validate()?;
        let actual = session
            .view()
            .surfaces()
            .map(|surface| surface.id())
            .collect::<BTreeSet<_>>();
        let declared = self
            .surfaces
            .iter()
            .map(|binding| binding.surface)
            .collect::<BTreeSet<_>>();
        if actual != declared {
            return Err(invalid(
                "host surface bindings must exactly match the core document surfaces",
            ));
        }
        Ok(())
    }
}

/// Complete adapter envelope containing the opaque public dockspace JSON document.
///
/// The inner JSON is a string intentionally: its original tokens reach the core
/// decoder, so duplicate fields cannot be erased by a `serde_json::Value` pass.
/// Dockspace owns integrity validation of its graph, key map and allocator history.
/// The outer host hints are structurally validated, not authenticated; applications
/// needing tamper protection should authenticate the entire encoded envelope.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DockWorkspaceDocument {
    format: String,
    version: u32,
    document_json: String,
    host: DockHostDocument,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentWire {
    format: String,
    version: u32,
    document_json: String,
    host: DockHostDocument,
}

#[derive(Deserialize)]
struct DocumentVersion {
    format: String,
    version: u32,
}

impl super::DockSurface {
    /// Captures the live workspace and complete pane-key history with validated host hints.
    ///
    /// Uses the original session without replacing its document or semantic owner.
    /// An unaccepted candidate or an outstanding host borrow returns
    /// [`DockPersistenceError::Busy`] without advancing the save generation.
    /// Accepted outputs waiting for renderer submission do not hold the update
    /// lease; the capture records committed semantics, not presentation proof.
    pub fn capture_document(
        &self,
        host: DockHostDocument,
    ) -> Result<DockWorkspaceDocument, DockPersistenceError> {
        let mut state = self
            .host
            .try_borrow_mut()
            .map_err(|_| DockPersistenceError::Busy)?;
        if state.has_unaccepted_update() {
            return Err(DockPersistenceError::Busy);
        }
        DockWorkspaceDocument::capture(&mut state.session, host)
    }
}

impl DockWorkspaceDocument {
    /// Captures the core's graph/key-history document together with validated host hints.
    ///
    /// Validation of host hints happens before advancing the core save generation.
    /// Core serialization retains every historical external pane key, even when closed.
    /// Use [`super::DockSurface::capture_document`] after giving the session to the adapter.
    pub fn capture(
        session: &mut DockspaceSession,
        host: DockHostDocument,
    ) -> Result<Self, DockPersistenceError> {
        host.validate_session(session)?;
        let document_json = String::from_utf8(session.save_document_json()?)
            .map_err(|_| invalid("dockspace emitted a document that is not UTF-8"))?;
        Ok(Self {
            format: DOCK_WORKSPACE_DOCUMENT_FORMAT.to_owned(),
            version: DOCK_WORKSPACE_DOCUMENT_VERSION,
            document_json,
            host,
        })
    }

    /// Decodes the versioned envelope without authorizing or publishing a workspace.
    ///
    /// Call [`Self::restore`] to validate the embedded core document and every key.
    /// Unknown fields and duplicate fields in supported envelope DTOs are errors.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, DockPersistenceError> {
        let version: DocumentVersion = serde_json::from_slice(bytes)?;
        if version.format != DOCK_WORKSPACE_DOCUMENT_FORMAT {
            return Err(invalid("unrecognized adapter document format"));
        }
        if version.version != DOCK_WORKSPACE_DOCUMENT_VERSION {
            return Err(DockPersistenceError::UnsupportedEnvelopeVersion {
                found: version.version,
            });
        }
        let wire: DocumentWire = serde_json::from_slice(bytes)?;
        wire.host.validate()?;
        Ok(Self {
            format: wire.format,
            version: wire.version,
            document_json: wire.document_json,
            host: wire.host,
        })
    }

    /// Encodes this complete durable envelope.
    pub fn to_json(&self) -> Result<Vec<u8>, DockPersistenceError> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Returns the unchanged public core document for its prepared/live restore API.
    ///
    /// Live replacement must publish the host hints only after the corresponding
    /// dockspace frame commits; decoding this envelope is not a commit receipt.
    pub fn document_json(&self) -> &str {
        &self.document_json
    }

    /// Returns the host metadata without granting native or presentation authority.
    pub fn host(&self) -> &DockHostDocument {
        &self.host
    }

    /// Validates into a fresh candidate session without touching a running workspace.
    ///
    /// The pure recognizer must accept every exact persisted key, including closed
    /// history. A rejection, malformed mapping, or invalid host binding discards the
    /// whole candidate. No factory is called and no live host state is changed.
    pub fn restore(
        &self,
        policy: DockPolicy,
        recognize: impl Fn(DockspaceDocumentId, &str) -> bool,
    ) -> Result<RestoredDockWorkspace, DockPersistenceError> {
        self.host.validate()?;
        let session =
            DockspaceSession::from_document_json(self.document_json.as_bytes(), policy, recognize)?;
        self.host.validate_session(&session)?;
        Ok(RestoredDockWorkspace {
            session,
            host: self.host.clone(),
        })
    }

    /// Decodes and validates an envelope into a fresh, unpublished session.
    pub fn restore_json(
        bytes: &[u8],
        policy: DockPolicy,
        recognize: impl Fn(DockspaceDocumentId, &str) -> bool,
    ) -> Result<RestoredDockWorkspace, DockPersistenceError> {
        Self::decode_json(bytes)?.restore(policy, recognize)
    }
}

/// A fully validated candidate and its host hints, ready for host admission.
#[derive(Debug)]
pub struct RestoredDockWorkspace {
    /// The sole semantic runtime; numeric pane identities remain core-owned.
    pub session: DockspaceSession,
    /// Durable aliases and policy/placement hints for the host to adopt atomically.
    pub host: DockHostDocument,
}

/// A validated open pane occurrence passed to a host-owned lazy factory registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockPaneBinding<'session> {
    /// Exact external View key, distinct for different Views of the same document.
    pub pane_key: &'session str,
    /// Item identity resolved from the complete core document.
    pub item: ItemId,
}

/// Enumerates open occurrences once, in surface/root/child/tab order.
///
/// All locations are validated before returning any bindings. A host can then
/// prepare one View per key in a temporary registry and publish it atomically.
/// Closed keys stay in the core history and are resolved with
/// `session.item_id_for_external_key`; they do not require eager View creation.
pub fn open_pane_bindings(
    session: &DockspaceSession,
) -> Result<Vec<DockPaneBinding<'_>>, DockPersistenceError> {
    let view = session.view();
    let mut bindings = Vec::new();
    let mut seen = BTreeSet::new();
    for surface in view.surfaces() {
        let roots = surface
            .main_root()
            .into_iter()
            .chain(surface.contained().filter_map(|floating| floating.root()));
        for root in roots {
            let mut pending = root
                .content()
                .into_iter()
                .collect::<Vec<DockspaceNodeView<'_>>>();
            while let Some(node) = pending.pop() {
                if let Some(tabs) = node.tabs() {
                    for &item in tabs.items() {
                        if !seen.insert(item) {
                            return Err(invalid(
                                "a pane occurs in more than one core content location",
                            ));
                        }
                        let pane_key = session
                            .external_key_for_item(item)
                            .ok_or_else(|| invalid("an open core item has no external pane key"))?;
                        bindings.push(DockPaneBinding { pane_key, item });
                    }
                } else if let Some(split) = node.split() {
                    let children = split.children().collect::<Vec<_>>();
                    pending.extend(children.into_iter().rev());
                }
            }
        }
    }
    Ok(bindings)
}

/// An atomic adapter-document or legacy-conversion failure.
#[derive(Debug, Error)]
pub enum DockPersistenceError {
    /// An unaccepted update or an outstanding host borrow prevents a live capture.
    #[error("dockspace host is borrowed or an unaccepted update is in progress")]
    Busy,
    /// A JSON token, field, duplicate field, or shape is invalid.
    #[error("docking document JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// The adapter envelope requires a newer or different schema.
    #[error("unsupported adapter docking document version {found}")]
    UnsupportedEnvelopeVersion {
        /// Rejected version.
        found: u32,
    },
    /// The retired docking layout version is not supported.
    #[error("unsupported legacy docking layout version {found}")]
    UnsupportedLegacyVersion {
        /// Rejected version.
        found: u32,
    },
    /// The caller supplied a repeated external key or the graph repeats a pane.
    #[error("duplicate external pane key {key:?}")]
    DuplicatePaneKey {
        /// Exact repeated key.
        key: String,
    },
    /// A legacy open pane is absent from the supplied host registry.
    #[error("unrecognized legacy pane key {key:?}")]
    UnknownPaneKey {
        /// Exact rejected key.
        key: String,
    },
    /// Positive-only dockspace weights cannot represent this legacy layout.
    #[error("legacy split {node} has an unrepresentable zero-share child")]
    ZeroShare {
        /// Source split node identity.
        node: u32,
    },
    /// A structural, identity, bounds, or cross-component invariant failed.
    #[error("invalid docking document: {reason}")]
    Invalid {
        /// Concrete failure description.
        reason: String,
    },
    /// The public core layout compiler rejected a declaration.
    #[error(transparent)]
    Layout(#[from] DockspaceLayoutError),
    /// The public core document API rejected a document or identity association.
    #[error(transparent)]
    Core(#[from] DockspacePersistenceError),
}

fn invalid(reason: impl Into<String>) -> DockPersistenceError {
    DockPersistenceError::Invalid {
        reason: reason.into(),
    }
}

#[derive(Clone, Copy, Deserialize)]
enum LegacyAxis {
    Horizontal,
    Vertical,
}

impl From<LegacyAxis> for DockspaceAxis {
    fn from(axis: LegacyAxis) -> Self {
        match axis {
            LegacyAxis::Horizontal => Self::Horizontal,
            LegacyAxis::Vertical => Self::Vertical,
        }
    }
}

#[derive(Deserialize)]
struct LegacyVersion {
    layout_version: u32,
}

#[derive(Deserialize)]
struct LegacySnapshotVersion {
    layout: LegacyVersion,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySnapshot<Layout> {
    #[serde(default)]
    revision: u64,
    layout: Layout,
    viewport_placement: LegacyViewportPlacements,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyViewportPlacements {
    placement_version: u32,
    viewports: Vec<LegacyViewportPlacement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyViewportPlacement {
    space: String,
    #[serde(default)]
    display_id: Option<u64>,
    #[serde(default)]
    window_bounds: Option<DockLogicalWindowPlacement>,
    #[serde(default)]
    host_bounds: Option<DockLogicalPlacementRect>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCentral {
    #[serde(default)]
    node: Option<u32>,
    #[serde(default = "default_true")]
    keep_alive_when_empty: bool,
    #[serde(default)]
    passthrough_when_empty: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLayoutV2 {
    layout_version: u32,
    spaces: Vec<LegacySpaceV2>,
    nodes: Vec<LegacyNodeV2>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySpaceV2 {
    id: String,
    #[serde(default)]
    root: Option<u32>,
    #[serde(default)]
    floatings: Vec<LegacyFloatingV2>,
    #[serde(default)]
    central: Option<LegacyCentral>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyFloatingV2 {
    root: u32,
    bounds: DockLogicalPlacementRect,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum LegacyNodeV2 {
    #[serde(rename = "tabs")]
    Tabs {
        id: u32,
        items: Vec<String>,
        #[serde(default)]
        selected: Option<String>,
    },
    #[serde(rename = "split")]
    Split {
        id: u32,
        axis: LegacyAxis,
        children: Vec<u32>,
        fractions: Vec<f32>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLayoutV3 {
    layout_version: u32,
    spaces: Vec<LegacySpaceV3>,
    nodes: Vec<LegacyNodeV3>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySpaceV3 {
    id: String,
    #[serde(default)]
    root: Option<u32>,
    #[serde(default)]
    floating_roots: Vec<LegacyFloatingV3>,
    #[serde(default)]
    central: Option<LegacyCentral>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyFloatingV3 {
    id: String,
    root: u32,
    bounds: DockLogicalPlacementRect,
    z_order: i32,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum LegacyNodeV3 {
    Branch {
        id: u32,
        axis: LegacyAxis,
        first: u32,
        second: u32,
        ratio: f32,
        #[serde(default)]
        size_ref: Option<DockLegacySizeReference>,
    },
    Group {
        id: u32,
        group_id: String,
        items: Vec<String>,
        selected: String,
        #[serde(default)]
        size_ref: Option<DockLegacySizeReference>,
    },
}

struct LegacyLayoutData {
    version: u32,
    spaces: Vec<LegacySpaceData>,
    nodes: Vec<(u32, LegacyNodeData)>,
    source_nodes: Vec<DockLegacyNodeIdentity>,
}

struct LegacySpaceData {
    key: String,
    root: Option<u32>,
    floatings: Vec<LegacyFloatingData>,
    central: Option<LegacyCentral>,
}

struct LegacyFloatingData {
    root: u32,
    bounds: DockLogicalPlacementRect,
    source_ordinal: usize,
    source_id: Option<String>,
    source_z_order: Option<i32>,
}

enum LegacyNodeData {
    Tabs {
        items: Vec<String>,
        selected: Option<String>,
    },
    Split {
        axis: LegacyAxis,
        children: Vec<u32>,
        weights: Vec<f32>,
    },
}

impl From<LegacyLayoutV2> for LegacyLayoutData {
    fn from(layout: LegacyLayoutV2) -> Self {
        let source_nodes = layout
            .nodes
            .iter()
            .map(|node| DockLegacyNodeIdentity {
                source_node: match node {
                    LegacyNodeV2::Tabs { id, .. } | LegacyNodeV2::Split { id, .. } => *id,
                },
                group_id: None,
                size_reference: None,
            })
            .collect();
        Self {
            version: layout.layout_version,
            spaces: layout
                .spaces
                .into_iter()
                .map(|space| LegacySpaceData {
                    key: space.id,
                    root: space.root,
                    floatings: space
                        .floatings
                        .into_iter()
                        .enumerate()
                        .map(|(ordinal, floating)| LegacyFloatingData {
                            root: floating.root,
                            bounds: floating.bounds,
                            source_ordinal: ordinal,
                            source_id: None,
                            source_z_order: None,
                        })
                        .collect(),
                    central: space.central,
                })
                .collect(),
            nodes: layout
                .nodes
                .into_iter()
                .map(|node| match node {
                    LegacyNodeV2::Tabs {
                        id,
                        items,
                        selected,
                    } => (id, LegacyNodeData::Tabs { items, selected }),
                    LegacyNodeV2::Split {
                        id,
                        axis,
                        children,
                        fractions,
                    } => (
                        id,
                        LegacyNodeData::Split {
                            axis,
                            children,
                            weights: fractions,
                        },
                    ),
                })
                .collect(),
            source_nodes,
        }
    }
}

impl TryFrom<LegacyLayoutV3> for LegacyLayoutData {
    type Error = DockPersistenceError;

    fn try_from(layout: LegacyLayoutV3) -> Result<Self, Self::Error> {
        let mut groups = BTreeSet::new();
        let mut source_nodes = Vec::new();
        let mut nodes = Vec::new();
        for node in layout.nodes {
            let (id, group_id, size_ref, data) = match node {
                LegacyNodeV3::Group {
                    id,
                    group_id,
                    items,
                    selected,
                    size_ref,
                } => {
                    if group_id.is_empty() || !groups.insert(group_id.clone()) {
                        return Err(invalid("legacy group ids must be nonempty and unique"));
                    }
                    (
                        id,
                        Some(group_id),
                        size_ref,
                        LegacyNodeData::Tabs {
                            items,
                            selected: Some(selected),
                        },
                    )
                }
                LegacyNodeV3::Branch {
                    id,
                    axis,
                    first,
                    second,
                    ratio,
                    size_ref,
                } => {
                    if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
                        return Err(invalid(format!("legacy branch {id} has an invalid ratio")));
                    }
                    (
                        id,
                        None,
                        size_ref,
                        LegacyNodeData::Split {
                            axis,
                            children: vec![first, second],
                            weights: vec![ratio, 1.0 - ratio],
                        },
                    )
                }
            };
            if let Some(size_ref) = size_ref {
                size_ref.validate()?;
            }
            source_nodes.push(DockLegacyNodeIdentity {
                source_node: id,
                group_id,
                size_reference: size_ref,
            });
            nodes.push((id, data));
        }
        let mut floating_ids = BTreeSet::new();
        let mut spaces = Vec::new();
        for space in layout.spaces {
            let mut z_orders = BTreeSet::new();
            for floating in &space.floating_roots {
                if floating.id.is_empty() || !floating_ids.insert(floating.id.clone()) {
                    return Err(invalid(
                        "legacy floating ids must be nonempty and globally unique",
                    ));
                }
                if !z_orders.insert(floating.z_order) {
                    return Err(invalid(
                        "legacy floating z-order must be unique within a space",
                    ));
                }
            }
            let mut floatings = space
                .floating_roots
                .into_iter()
                .enumerate()
                .collect::<Vec<_>>();
            floatings.sort_by_key(|(_, floating)| floating.z_order);
            spaces.push(LegacySpaceData {
                key: space.id,
                root: space.root,
                floatings: floatings
                    .into_iter()
                    .map(|(ordinal, floating)| LegacyFloatingData {
                        root: floating.root,
                        bounds: floating.bounds,
                        source_ordinal: ordinal,
                        source_id: Some(floating.id),
                        source_z_order: Some(floating.z_order),
                    })
                    .collect(),
                central: space.central,
            });
        }
        Ok(Self {
            version: layout.layout_version,
            spaces,
            nodes,
            source_nodes,
        })
    }
}

/// Migrates a serialized legacy `DockLayout` without linking the retired runtime.
///
/// `known_pane_keys` is the complete host registry, including closed historical keys
/// absent from the source layout. Duplicate/empty registry keys and unknown open
/// keys fail atomically. Identities are allocated in lexical key order and then
/// owned solely by the candidate document; factories remain owned by the caller.
pub fn migrate_legacy_layout_json<I, Key>(
    bytes: &[u8],
    document_id: DockspaceDocumentId,
    known_pane_keys: I,
    topology: LegacyTopologyMigration,
    policy: DockPolicy,
) -> Result<RestoredDockWorkspace, DockPersistenceError>
where
    I: IntoIterator<Item = Key>,
    Key: Into<String>,
{
    let version: LegacyVersion = serde_json::from_slice(bytes)?;
    let layout = match version.layout_version {
        2 => LegacyLayoutData::from(serde_json::from_slice::<LegacyLayoutV2>(bytes)?),
        3 => LegacyLayoutData::try_from(serde_json::from_slice::<LegacyLayoutV3>(bytes)?)?,
        found => return Err(DockPersistenceError::UnsupportedLegacyVersion { found }),
    };
    migrate_legacy(
        layout,
        None,
        None,
        document_id,
        known_pane_keys,
        topology,
        policy,
    )
}

/// Migrates the actual `DockSurfaceSnapshot` envelope, including revision and placement v1.
///
/// A missing legacy revision means zero, matching the original schema. Unknown
/// placement spaces, duplicate entries, malformed geometry and all layout errors
/// reject the entire candidate before any host factory or publication is possible.
pub fn migrate_legacy_surface_snapshot_json<I, Key>(
    bytes: &[u8],
    document_id: DockspaceDocumentId,
    known_pane_keys: I,
    topology: LegacyTopologyMigration,
    policy: DockPolicy,
) -> Result<RestoredDockWorkspace, DockPersistenceError>
where
    I: IntoIterator<Item = Key>,
    Key: Into<String>,
{
    let version: LegacySnapshotVersion = serde_json::from_slice(bytes)?;
    let (layout, revision, placements) = match version.layout.layout_version {
        2 => {
            let snapshot: LegacySnapshot<LegacyLayoutV2> = serde_json::from_slice(bytes)?;
            (
                LegacyLayoutData::from(snapshot.layout),
                snapshot.revision,
                snapshot.viewport_placement,
            )
        }
        3 => {
            let snapshot: LegacySnapshot<LegacyLayoutV3> = serde_json::from_slice(bytes)?;
            (
                LegacyLayoutData::try_from(snapshot.layout)?,
                snapshot.revision,
                snapshot.viewport_placement,
            )
        }
        found => return Err(DockPersistenceError::UnsupportedLegacyVersion { found }),
    };
    migrate_legacy(
        layout,
        Some(revision),
        Some(placements),
        document_id,
        known_pane_keys,
        topology,
        policy,
    )
}

fn migrate_legacy<I, Key>(
    mut layout: LegacyLayoutData,
    revision: Option<u64>,
    placements: Option<LegacyViewportPlacements>,
    document_id: DockspaceDocumentId,
    known_pane_keys: I,
    topology: LegacyTopologyMigration,
    policy: DockPolicy,
) -> Result<RestoredDockWorkspace, DockPersistenceError>
where
    I: IntoIterator<Item = Key>,
    Key: Into<String>,
{
    let mut keys = BTreeSet::new();
    for key in known_pane_keys {
        let key = key.into();
        if key.is_empty() {
            return Err(invalid("external pane keys must be nonempty"));
        }
        if !keys.insert(key.clone()) {
            return Err(DockPersistenceError::DuplicatePaneKey { key });
        }
    }
    layout
        .spaces
        .sort_by(|left, right| left.key.cmp(&right.key));
    layout.source_nodes.sort_by_key(|node| node.source_node);
    let validation = validate_legacy(&layout, &keys)?;
    let mut placements = validate_legacy_placements(placements, &layout.spaces)?;
    let mut bootstrap = DockspaceDocumentBootstrap::new(document_id);
    bootstrap.ensure_items(keys)?;
    let mut roots = 0_u64;
    let mut floating_ids = 0_u64;
    let mut surfaces = Vec::new();
    let mut bindings = Vec::new();
    let mut floating_identities = Vec::new();
    let mut differences = vec![
        DockLegacyContractDifference::CanonicalNaryTopology,
        DockLegacyContractDifference::RelativeGeometry,
    ];
    if layout
        .source_nodes
        .iter()
        .any(|node| node.group_id.is_some())
    {
        differences.push(DockLegacyContractDifference::GroupIdentityIsProvenance);
    }
    if layout
        .spaces
        .iter()
        .any(|space| !space.floatings.is_empty())
    {
        differences.push(DockLegacyContractDifference::FloatingIdentityCrosswalk);
    }
    if layout.spaces.iter().any(|space| space.central.is_some()) {
        differences.push(DockLegacyContractDifference::CentralPolicyRequiresHost);
    }
    if !placements.is_empty() {
        differences.push(DockLegacyContractDifference::LogicalPlacementRequiresHost);
    }
    if layout.spaces.iter().any(|space| {
        space.root.is_none()
            && (space.floatings.is_empty()
                || space
                    .central
                    .as_ref()
                    .is_some_and(|central| central.keep_alive_when_empty))
    }) {
        differences.push(DockLegacyContractDifference::EmptySpaceUsesCentralRoot);
    }
    for (ordinal, space) in layout.spaces.iter().enumerate() {
        let surface_id = SurfaceId::new(
            u64::try_from(ordinal)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| invalid("surface identity space exhausted"))?,
        );
        let central_leaf = validation.central_leaves.get(&space.key).copied();
        let main_content = if let Some(root) = space.root {
            Some(compile_legacy_node(
                root,
                central_leaf,
                &validation,
                layout.version,
                &bootstrap,
            )?)
        } else if space.floatings.is_empty()
            || space
                .central
                .as_ref()
                .is_some_and(|central| central.keep_alive_when_empty)
        {
            Some(DockspaceNode::central_tabs([]))
        } else {
            None
        };
        let mut surface = match main_content {
            Some(content) => DockspaceSurfaceLayout::new(
                surface_id,
                DockspaceRootLayout::new(RootId::new(next_identity(&mut roots)?), content),
            ),
            None => DockspaceSurfaceLayout::rootless(surface_id),
        };
        for floating in &space.floatings {
            let content =
                compile_legacy_node(floating.root, None, &validation, layout.version, &bootstrap)?;
            let initial_floating = FloatingPresentationId::new(next_identity(&mut floating_ids)?);
            let initial_root = RootId::new(next_identity(&mut roots)?);
            surface = surface.with_contained(DockspaceContainedLayout::new(
                initial_floating,
                DockspaceRootLayout::new(initial_root, content),
                floating.bounds.into_contained_rect()?,
            ));
            floating_identities.push(DockLegacyFloatingIdentity {
                space_key: space.key.clone(),
                source_root: floating.root,
                source_ordinal: floating.source_ordinal,
                source_id: floating.source_id.clone(),
                source_z_order: floating.source_z_order,
                initial_floating,
                initial_root,
            });
        }
        surfaces.push(surface);
        bindings.push(DockSurfaceKeyBinding {
            space_key: space.key.clone(),
            surface: surface_id,
            logical_placement: placements.remove(&space.key),
            central_policy: space.central.as_ref().map(|central| DockCentralPolicyHint {
                keep_alive_when_empty: central.keep_alive_when_empty,
                passthrough_when_empty: central.passthrough_when_empty,
            }),
        });
    }
    let session = DockspaceSession::from_persistent_layout(
        DockspaceLayout::new(surfaces)?,
        policy,
        bootstrap,
    )?;
    let host = DockHostDocument {
        surfaces: bindings,
        migration: Some(DockLegacyMigration {
            layout_version: layout.version,
            snapshot_revision: revision,
            topology,
            source_nodes: layout.source_nodes,
            floating_identities,
            differences,
        }),
    };
    host.validate_session(&session)?;
    Ok(RestoredDockWorkspace { session, host })
}

fn next_identity(value: &mut u64) -> Result<u64, DockPersistenceError> {
    *value = value
        .checked_add(1)
        .ok_or_else(|| invalid("root or floating identity space exhausted"))?;
    Ok(*value)
}

struct LegacyValidation<'layout> {
    nodes: BTreeMap<u32, &'layout LegacyNodeData>,
    central_leaves: BTreeMap<String, u32>,
    central_path: BTreeSet<u32>,
    size_references: BTreeMap<u32, DockLegacySizeReference>,
}

fn validate_legacy<'layout>(
    layout: &'layout LegacyLayoutData,
    keys: &BTreeSet<String>,
) -> Result<LegacyValidation<'layout>, DockPersistenceError> {
    let mut nodes = BTreeMap::new();
    let mut items_seen = BTreeSet::new();
    for (id, node) in &layout.nodes {
        if nodes.insert(*id, node).is_some() {
            return Err(invalid(format!("duplicate legacy node {id}")));
        }
        match node {
            LegacyNodeData::Tabs { items, selected } => {
                if items.is_empty()
                    || !selected
                        .as_ref()
                        .is_some_and(|selected| items.contains(selected))
                {
                    return Err(invalid(format!(
                        "legacy tabs {id} must be nonempty with a selected member"
                    )));
                }
                for item in items {
                    if !keys.contains(item) {
                        return Err(DockPersistenceError::UnknownPaneKey { key: item.clone() });
                    }
                    if !items_seen.insert(item.as_str()) {
                        return Err(DockPersistenceError::DuplicatePaneKey { key: item.clone() });
                    }
                }
            }
            LegacyNodeData::Split {
                children, weights, ..
            } => {
                if children.is_empty()
                    || children.len() != weights.len()
                    || weights
                        .iter()
                        .any(|weight| !weight.is_finite() || *weight < 0.0)
                {
                    return Err(invalid(format!(
                        "legacy split {id} has invalid children or weights"
                    )));
                }
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut space_keys = BTreeSet::new();
    let mut central_leaves = BTreeMap::new();
    let mut central_path = BTreeSet::new();
    for space in &layout.spaces {
        if space.key.is_empty() || !space_keys.insert(space.key.as_str()) {
            return Err(invalid("legacy space keys must be nonempty and unique"));
        }
        let mut main_nodes = BTreeSet::new();
        let mut parents = BTreeMap::new();
        if let Some(root) = space.root {
            validate_legacy_tree(root, &nodes, &mut seen, &mut main_nodes, &mut parents)?;
        }
        for floating in &space.floatings {
            floating.bounds.into_contained_rect()?;
            validate_legacy_tree(
                floating.root,
                &nodes,
                &mut seen,
                &mut BTreeSet::new(),
                &mut BTreeMap::new(),
            )?;
        }
        if let Some(central) = space.central.as_ref().and_then(|central| central.node) {
            if !main_nodes.contains(&central) {
                return Err(invalid(format!(
                    "central node {central} lies outside space {:?}'s main root",
                    space.key
                )));
            }
            let mut leaf = central;
            while let Some(LegacyNodeData::Split { children, .. }) = nodes.get(&leaf).copied() {
                if layout.version == 3 {
                    return Err(invalid(
                        "legacy v3 central markers must identify group leaves",
                    ));
                }
                leaf = *children
                    .last()
                    .ok_or_else(|| invalid("empty central split"))?;
            }
            central_leaves.insert(space.key.clone(), leaf);
            let mut ancestor = Some(leaf);
            while let Some(node) = ancestor {
                central_path.insert(node);
                ancestor = parents.get(&node).copied();
            }
        }
    }
    if seen.len() != nodes.len() {
        return Err(invalid(
            "legacy layout contains unreachable nodes or an unreachable cycle",
        ));
    }
    let size_references = layout
        .source_nodes
        .iter()
        .filter_map(|node| node.size_reference.map(|size| (node.source_node, size)))
        .collect();
    Ok(LegacyValidation {
        nodes,
        central_leaves,
        central_path,
        size_references,
    })
}

fn validate_legacy_tree(
    root: u32,
    nodes: &BTreeMap<u32, &LegacyNodeData>,
    seen: &mut BTreeSet<u32>,
    subtree: &mut BTreeSet<u32>,
    parents: &mut BTreeMap<u32, u32>,
) -> Result<(), DockPersistenceError> {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            return Err(invalid(format!(
                "legacy node {id} is shared or participates in a cycle"
            )));
        }
        subtree.insert(id);
        let node = nodes
            .get(&id)
            .ok_or_else(|| invalid(format!("legacy node {id} is missing")))?;
        if let LegacyNodeData::Split { children, .. } = node {
            for &child in children.iter().rev() {
                parents.insert(child, id);
                pending.push(child);
            }
        }
    }
    Ok(())
}

fn validate_legacy_placements(
    placements: Option<LegacyViewportPlacements>,
    spaces: &[LegacySpaceData],
) -> Result<BTreeMap<String, DockLogicalViewportPlacement>, DockPersistenceError> {
    let mut result = BTreeMap::new();
    let Some(placements) = placements else {
        return Ok(result);
    };
    if placements.placement_version != 1 {
        return Err(invalid(format!(
            "unsupported legacy placement version {}",
            placements.placement_version
        )));
    }
    let keys = spaces
        .iter()
        .map(|space| space.key.as_str())
        .collect::<BTreeSet<_>>();
    for placement in placements.viewports {
        if !keys.contains(placement.space.as_str()) {
            return Err(invalid(format!(
                "placement references unknown space {:?}",
                placement.space
            )));
        }
        let hint = DockLogicalViewportPlacement {
            display_id: placement.display_id,
            window_bounds: placement.window_bounds,
            host_bounds: placement.host_bounds,
        };
        hint.validate()?;
        if result.insert(placement.space, hint).is_some() {
            return Err(invalid("duplicate legacy viewport placement space"));
        }
    }
    Ok(result)
}

fn compile_legacy_node(
    root: u32,
    central_leaf: Option<u32>,
    validation: &LegacyValidation<'_>,
    version: u32,
    bootstrap: &DockspaceDocumentBootstrap,
) -> Result<DockspaceNode, DockPersistenceError> {
    enum Step {
        Enter(u32),
        Finish {
            id: u32,
            axis: LegacyAxis,
            weights: Vec<f32>,
            base: usize,
        },
    }
    let mut pending = vec![Step::Enter(root)];
    let mut completed = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(id) => {
                let node = validation
                    .nodes
                    .get(&id)
                    .ok_or_else(|| invalid(format!("missing legacy node {id}")))?;
                match node {
                    LegacyNodeData::Tabs { items, selected } => {
                        let items = items
                            .iter()
                            .map(|key| {
                                bootstrap.item_id(key).ok_or_else(|| {
                                    DockPersistenceError::UnknownPaneKey { key: key.clone() }
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let selected = selected
                            .as_ref()
                            .map(|key| {
                                bootstrap.item_id(key).ok_or_else(|| {
                                    DockPersistenceError::UnknownPaneKey { key: key.clone() }
                                })
                            })
                            .transpose()?;
                        completed.push(if central_leaf == Some(id) {
                            DockspaceNode::central_tabs_with_selection(items, selected)?
                        } else {
                            DockspaceNode::tabs_with_selection(items, selected)?
                        });
                    }
                    LegacyNodeData::Split {
                        axis,
                        children,
                        weights,
                    } => {
                        let mut weights = weights.clone();
                        if version == 3
                            && !validation.central_path.contains(&id)
                            && let (Some(first), Some(second)) = (
                                validation.size_references.get(&children[0]),
                                validation.size_references.get(&children[1]),
                            )
                        {
                            let first = f64::from(first.axis_size(*axis));
                            let second = f64::from(second.axis_size(*axis));
                            let total = first + second;
                            if total > 0.0 {
                                weights = vec![(first / total) as f32, (second / total) as f32];
                            }
                        }
                        if version == 2
                            && children.len() > 1
                            && let Some(central) = children
                                .iter()
                                .position(|child| validation.central_path.contains(child))
                        {
                            let other_sum = weights
                                .iter()
                                .enumerate()
                                .filter(|(index, _)| *index != central)
                                .map(|(_, weight)| f64::from(*weight))
                                .sum::<f64>();
                            if other_sum >= 1.0 {
                                return Err(DockPersistenceError::ZeroShare { node: id });
                            }
                            weights[central] = (1.0 - other_sum) as f32;
                        }
                        if children.len() > 1 && weights.iter().any(|weight| *weight <= 0.0) {
                            return Err(DockPersistenceError::ZeroShare { node: id });
                        }
                        if children.len() == 1 && weights[0] == 0.0 {
                            return Err(DockPersistenceError::ZeroShare { node: id });
                        }
                        pending.push(Step::Finish {
                            id,
                            axis: *axis,
                            weights,
                            base: completed.len(),
                        });
                        pending.extend(children.iter().rev().copied().map(Step::Enter));
                    }
                }
            }
            Step::Finish {
                id,
                axis,
                weights,
                base,
            } => {
                let children = completed.split_off(base);
                let node = if children.len() == 1 {
                    children
                        .into_iter()
                        .next()
                        .ok_or_else(|| invalid(format!("legacy split {id} has no child")))?
                } else {
                    DockspaceNode::split(axis.into(), children.into_iter().zip(weights))?
                };
                completed.push(node);
            }
        }
    }
    completed
        .pop()
        .ok_or_else(|| invalid("legacy root is missing"))
}

#[cfg(test)]
#[path = "persistence_tests.rs"]
mod tests;
