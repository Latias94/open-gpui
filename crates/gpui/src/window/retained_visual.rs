use crate::{
    AnyElement, App, AtlasAccessDiagnostic, AtlasTextureLease, AtlasTextureLeaseError, Bounds,
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Scene,
    SharedString, WindowId,
};
use open_gpui_collections::FxHashMap;
use std::{any::Any, fmt, ops::Range, rc::Rc};

use super::{
    Frame, FrameOutput, ImagePaintDiagnostic, SubtreeGeometryValidity, SubtreeTransformDiagnostic,
    VisualPaintIndex, VisualPrepaintIndex, Window,
};

/// Stable identity for one window-local visual source.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SourceId(SharedString);

impl SourceId {
    /// Creates a stable visual source identity.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// A window-owned lease for replaying one committed visual without its interactive channels.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ticket {
    source_window: WindowId,
    source_frame_generation: u64,
    lease_generation: u64,
    bounds: Bounds<Pixels>,
}

/// Prepared exact release of one retained-visual lease.
#[doc(hidden)]
#[derive(Debug)]
pub struct PreparedRelease {
    ticket: Ticket,
}

impl PreparedRelease {
    /// Returns the exact retained-visual lease named by this preparation.
    pub const fn ticket(&self) -> Ticket {
        self.ticket
    }

    /// Returns the exact retained-visual lease named by this preparation.
    pub const fn ticket_identity(&self) -> TicketIdentity {
        self.ticket.identity()
    }
}

/// Durable evidence that one exact retained-visual lease was released.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseReceipt {
    ticket: TicketIdentity,
}

impl ReleaseReceipt {
    /// Returns the exact retained-visual lease settled by this receipt.
    pub const fn ticket_identity(self) -> TicketIdentity {
        self.ticket
    }
}

/// Stable identity of one exact retained-visual lease.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TicketIdentity {
    source_window: WindowId,
    source_frame_generation: u64,
    lease_generation: u64,
}

impl TicketIdentity {
    /// Returns the window that owns the retained visual.
    pub const fn source_window(self) -> WindowId {
        self.source_window
    }

    /// Returns the committed source frame captured by the lease.
    pub const fn source_frame_generation(self) -> u64 {
        self.source_frame_generation
    }

    /// Returns the exact window-local lease generation.
    pub const fn lease_generation(self) -> u64 {
        self.lease_generation
    }
}

impl Ticket {
    /// Returns the exact identity that a source-proxy barrier must bind before replay.
    pub const fn identity(&self) -> TicketIdentity {
        TicketIdentity {
            source_window: self.source_window,
            source_frame_generation: self.source_frame_generation,
            lease_generation: self.lease_generation,
        }
    }

    /// Returns the only window in which this visual may be replayed.
    pub fn source_window(&self) -> WindowId {
        self.source_window
    }

    /// Returns the committed frame from which the visual was captured.
    pub fn source_frame_generation(&self) -> u64 {
        self.source_frame_generation
    }

    /// Returns the exact window-owned lease generation.
    pub fn lease_generation(&self) -> u64 {
        self.lease_generation
    }

    /// Returns the source subtree's committed logical bounds.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}

/// Exact evidence that one retained visual replayed into one candidate frame.
///
/// The receipt is created only after scene and resource replay succeeds. Consumers must still
/// wait for that candidate frame to commit before treating it as presentation authority.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayReceipt {
    source_window: WindowId,
    source_frame_generation: u64,
    lease_generation: u64,
    replay_frame_generation: u64,
    replay_attempt_id: u64,
}

impl ReplayReceipt {
    /// Returns the retained-visual ticket identity proven by this replay.
    pub const fn ticket(self) -> TicketIdentity {
        TicketIdentity {
            source_window: self.source_window,
            source_frame_generation: self.source_frame_generation,
            lease_generation: self.lease_generation,
        }
    }

    /// Returns the window that owns both the retained visual and the replay frame.
    pub const fn source_window(self) -> WindowId {
        self.source_window
    }

    /// Returns the committed frame from which the retained visual was captured.
    pub const fn source_frame_generation(self) -> u64 {
        self.source_frame_generation
    }

    /// Returns the exact retained-visual lease generation.
    pub const fn lease_generation(self) -> u64 {
        self.lease_generation
    }

    /// Returns the candidate frame into which the visual was replayed.
    pub const fn replay_frame_generation(self) -> u64 {
        self.replay_frame_generation
    }

    pub(crate) fn matches_candidate(self, window: &Window) -> bool {
        self.source_window == window.handle.window_id()
            && self.replay_frame_generation == window.preparing_frame_generation()
            && self.replay_attempt_id == window.preparing_frame_attempt_id()
    }
}

/// Describes why a window-local retained visual cannot be acquired or replayed.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum Invalidation {
    /// The source window has entered presentation shutdown.
    WindowClosed,
    /// The requested source did not publish a record in the committed frame.
    SourceUnavailable(SourceId),
    /// More than one subtree published the same source identity in one frame.
    DuplicateSource(SourceId),
    /// The captured subtree emitted deferred output whose paint lifetime is not source-local.
    DeferredOutputUnsupported(SourceId),
    /// The source produced no paint primitives.
    EmptyVisual(SourceId),
    /// The source scene range was stale or contained invalid geometry.
    SubtreeGeometryUnavailable(SourceId),
    /// A renderer-local atlas texture could not be retained.
    ResourceUnavailable(AtlasTextureLeaseError),
    /// The source already has an active exclusive visual lease.
    AlreadyLeased(SourceId),
    /// The lease generation space was exhausted.
    LeaseGenerationExhausted,
    /// The ticket was presented to a different window.
    WrongWindow {
        /// The source window bound into the ticket.
        expected: WindowId,
        /// The window asked to replay or release the ticket.
        actual: WindowId,
    },
    /// The source window's device scale changed after capture.
    ScaleChanged {
        /// The scale used to produce the retained primitives.
        expected: f32,
        /// The scale installed in the source window now.
        actual: f32,
    },
    /// The renderer atlas was reset after capture.
    RendererEpochChanged {
        /// The renderer epoch held by the retained visual.
        expected: u64,
        /// The renderer epoch installed now.
        actual: u64,
    },
    /// The ticket no longer names a live exact lease.
    StaleGeneration,
    /// The same visual was replayed more than once into one frame.
    DuplicateReplay,
    /// Paint-only replay was requested outside the paint phase.
    OutsidePaint,
}

impl fmt::Display for Invalidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowClosed => f.write_str("the source window is closed"),
            Self::SourceUnavailable(source) => {
                write!(f, "retained visual source `{source}` is unavailable")
            }
            Self::DuplicateSource(source) => {
                write!(
                    f,
                    "retained visual source `{source}` was published more than once"
                )
            }
            Self::DeferredOutputUnsupported(source) => write!(
                f,
                "retained visual source `{source}` emitted unsupported deferred output"
            ),
            Self::EmptyVisual(source) => {
                write!(
                    f,
                    "retained visual source `{source}` produced no primitives"
                )
            }
            Self::SubtreeGeometryUnavailable(source) => write!(
                f,
                "retained visual source `{source}` has unavailable geometry"
            ),
            Self::ResourceUnavailable(error) => error.fmt(f),
            Self::AlreadyLeased(source) => {
                write!(f, "retained visual source `{source}` is already leased")
            }
            Self::LeaseGenerationExhausted => {
                f.write_str("retained visual lease generation exhausted")
            }
            Self::WrongWindow { expected, actual } => write!(
                f,
                "retained visual belongs to window {expected:?}, not {actual:?}"
            ),
            Self::ScaleChanged { expected, actual } => write!(
                f,
                "retained visual scale changed from {expected} to {actual}"
            ),
            Self::RendererEpochChanged { expected, actual } => write!(
                f,
                "retained visual renderer epoch changed from {expected} to {actual}"
            ),
            Self::StaleGeneration => f.write_str("retained visual lease generation is stale"),
            Self::DuplicateReplay => {
                f.write_str("retained visual was replayed more than once in one frame")
            }
            Self::OutsidePaint => {
                f.write_str("retained visual replay is only valid during window paint")
            }
        }
    }
}

impl std::error::Error for Invalidation {}

pub(crate) type CommittedSourceRecord = Result<Rc<CommittedVisual>, Invalidation>;

pub(crate) struct CommittedVisual {
    source_window: WindowId,
    source_frame_generation: u64,
    bounds: Bounds<Pixels>,
    scale_factor: f32,
    scene: Rc<Scene>,
    retained_resources: Rc<[Rc<dyn Any>]>,
    atlas_access_diagnostics: Rc<[AtlasAccessDiagnostic]>,
    image_paint_diagnostics: Rc<[ImagePaintDiagnostic]>,
    subtree_transform_diagnostics: Rc<[SubtreeTransformDiagnostic]>,
    atlas_leases: Rc<[Rc<AtlasTextureLease>]>,
}

impl CommittedVisual {
    fn for_frame_generation(&self, source_frame_generation: u64) -> Rc<Self> {
        Rc::new(Self {
            source_window: self.source_window,
            source_frame_generation,
            bounds: self.bounds,
            scale_factor: self.scale_factor,
            scene: self.scene.clone(),
            retained_resources: self.retained_resources.clone(),
            atlas_access_diagnostics: self.atlas_access_diagnostics.clone(),
            image_paint_diagnostics: self.image_paint_diagnostics.clone(),
            subtree_transform_diagnostics: self.subtree_transform_diagnostics.clone(),
            atlas_leases: self.atlas_leases.clone(),
        })
    }
}

#[derive(Clone)]
pub(crate) struct PendingSourcePublication {
    source_id: SourceId,
    record: CommittedSourceRecord,
    operation_validities: Box<[Option<SubtreeGeometryValidity>]>,
}

impl PendingSourcePublication {
    fn failed(source_id: SourceId, invalidation: Invalidation) -> Self {
        Self {
            source_id,
            record: Err(invalidation),
            operation_validities: Box::default(),
        }
    }

    pub(crate) fn replayed_under(&self, parent: Option<SubtreeGeometryValidity>) -> Self {
        Self {
            source_id: self.source_id.clone(),
            record: self.record.clone(),
            operation_validities: self
                .operation_validities
                .iter()
                .map(|recorded| {
                    SubtreeGeometryValidity::replayed_under(recorded.as_ref(), parent.clone())
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    fn invalidate(&mut self, invalidation: Invalidation) {
        self.record = Err(invalidation);
        self.operation_validities = Box::default();
    }

    fn settle(&mut self, frame_generation: u64) -> CommittedSourceRecord {
        if self.record.is_ok()
            && self.operation_validities.iter().any(|validity| {
                validity
                    .as_ref()
                    .is_some_and(|validity| !validity.is_valid())
            })
        {
            self.invalidate(Invalidation::SubtreeGeometryUnavailable(
                self.source_id.clone(),
            ));
        }
        let visual = self.record.as_ref().map_err(Clone::clone)?;
        Ok(visual.for_frame_generation(frame_generation))
    }
}

impl Frame {
    pub(crate) fn settle_retained_visual_sources(&mut self) {
        let mut publication_counts = FxHashMap::default();
        for publication in &self.retained_visual_publications {
            *publication_counts
                .entry(publication.source_id.clone())
                .or_insert(0_usize) += 1;
        }

        let mut sources = FxHashMap::default();
        for publication in &mut self.retained_visual_publications {
            let source_id = publication.source_id.clone();
            if publication_counts
                .get(&source_id)
                .copied()
                .unwrap_or_default()
                > 1
            {
                publication.invalidate(Invalidation::DuplicateSource(source_id.clone()));
            }
            let record = publication.settle(self.generation);
            if sources.insert(source_id.clone(), record).is_some() {
                sources.insert(
                    source_id.clone(),
                    Err(Invalidation::DuplicateSource(source_id)),
                );
            }
        }
        self.retained_visual_sources = sources;
    }
}

struct ActiveVisual {
    source_id: SourceId,
    visual: Rc<CommittedVisual>,
}

#[derive(Default)]
pub(super) struct Registry {
    next_generation: u64,
    active_by_generation: FxHashMap<u64, ActiveVisual>,
    active_by_source: FxHashMap<SourceId, u64>,
}

impl Registry {
    fn active(&self, ticket: &Ticket) -> Result<&ActiveVisual, Invalidation> {
        let active = self
            .active_by_generation
            .get(&ticket.lease_generation)
            .ok_or(Invalidation::StaleGeneration)?;
        if active.visual.source_frame_generation != ticket.source_frame_generation
            || active.visual.source_window != ticket.source_window
        {
            return Err(Invalidation::StaleGeneration);
        }
        Ok(active)
    }

    fn activate(
        &mut self,
        source_id: &SourceId,
        visual: Rc<CommittedVisual>,
    ) -> Result<Ticket, Invalidation> {
        if self.active_by_source.contains_key(source_id) {
            return Err(Invalidation::AlreadyLeased(source_id.clone()));
        }
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(Invalidation::LeaseGenerationExhausted)?;
        self.next_generation = generation;
        self.active_by_source.insert(source_id.clone(), generation);
        self.active_by_generation.insert(
            generation,
            ActiveVisual {
                source_id: source_id.clone(),
                visual: visual.clone(),
            },
        );
        Ok(Ticket {
            source_window: visual.source_window,
            source_frame_generation: visual.source_frame_generation,
            lease_generation: generation,
            bounds: visual.bounds,
        })
    }

    fn active_mut(&mut self, ticket: &Ticket) -> Result<&mut ActiveVisual, Invalidation> {
        let active = self
            .active_by_generation
            .get_mut(&ticket.lease_generation)
            .ok_or(Invalidation::StaleGeneration)?;
        if active.visual.source_frame_generation != ticket.source_frame_generation
            || active.visual.source_window != ticket.source_window
        {
            return Err(Invalidation::StaleGeneration);
        }
        Ok(active)
    }

    fn prepare_release(&self, ticket: &Ticket) -> Result<PreparedRelease, Invalidation> {
        self.active(ticket)?;
        Ok(PreparedRelease { ticket: *ticket })
    }

    fn can_commit_prepared_release(&self, prepared: &PreparedRelease) -> bool {
        self.active(&prepared.ticket).is_ok()
    }

    fn commit_prepared_release(&mut self, prepared: PreparedRelease) -> ReleaseReceipt {
        let ticket = prepared.ticket.identity();
        assert!(
            self.can_commit_prepared_release(&prepared),
            "prepared retained-visual release must remain exact until commit"
        );
        self.release(&prepared.ticket)
            .expect("validated retained-visual release must commit");
        ReleaseReceipt { ticket }
    }

    fn observe_release(&self, ticket: TicketIdentity) -> Option<ReleaseReceipt> {
        if ticket.lease_generation == 0 || ticket.lease_generation > self.next_generation {
            return None;
        }
        if self
            .active_by_generation
            .contains_key(&ticket.lease_generation)
        {
            return None;
        }
        Some(ReleaseReceipt { ticket })
    }

    fn release(&mut self, ticket: &Ticket) -> Result<(), Invalidation> {
        self.active_mut(ticket)?;
        let active = self
            .active_by_generation
            .remove(&ticket.lease_generation)
            .ok_or(Invalidation::StaleGeneration)?;
        if self.active_by_source.get(&active.source_id) == Some(&ticket.lease_generation) {
            self.active_by_source.remove(&active.source_id);
        }
        Ok(())
    }

    pub(super) fn clear(&mut self) {
        self.active_by_generation.clear();
        self.active_by_source.clear();
    }
}

#[doc(hidden)]
pub struct SourcePrepaintState {
    journal_range: Range<VisualPrepaintIndex>,
}

/// A layout-neutral element that publishes paint-only committed visual records.
#[doc(hidden)]
pub struct SourceElement {
    source_id: SourceId,
    element_id: ElementId,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

/// Marks one subtree as a source for window-local retained visual leases.
#[doc(hidden)]
#[track_caller]
pub fn source(source_id: SourceId, child: impl IntoElement) -> SourceElement {
    let element_id = ElementId::from(format!("retained-visual-source:{source_id}"));
    SourceElement {
        source_id,
        element_id,
        child: Some(child.into_any_element()),
        source: core::panic::Location::caller(),
    }
}

impl Element for SourceElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = SourcePrepaintState;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Some(self.source)
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self
            .child
            .take()
            .expect("retained visual source child missing");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let start = window.retained_visual_prepaint_index();
        child.prepaint(window, cx);
        let end = window.retained_visual_prepaint_index();
        SourcePrepaintState {
            journal_range: start..end,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (start, end) = window.with_atlas_texture_lease_paint_scope(|window| {
            let start = window.retained_visual_paint_index();
            child.paint(window, cx);
            let end = window.retained_visual_paint_index();
            (start, end)
        });
        window.record_retained_visual_source(
            self.source_id.clone(),
            bounds,
            prepaint.journal_range.clone(),
            start..end,
        );
    }
}

impl IntoElement for SourceElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Acquires an exclusive lease from the source window's last committed frame.
#[doc(hidden)]
pub fn lease_committed(window: &mut Window, source_id: &SourceId) -> Result<Ticket, Invalidation> {
    window.lease_committed_retained_visual(source_id)
}

/// Replays only the retained scene and resources into the source window's current frame.
#[doc(hidden)]
pub fn replay(window: &mut Window, ticket: &Ticket) -> Result<ReplayReceipt, Invalidation> {
    window.replay_retained_visual(ticket)
}

/// Releases one exact window-owned retained visual lease.
#[doc(hidden)]
pub fn release(window: &mut Window, ticket: &Ticket) -> Result<(), Invalidation> {
    window.release_retained_visual(ticket)
}

/// Prepares an exact retained-visual release without mutating the active lease.
#[doc(hidden)]
pub fn prepare_release(
    window: &mut Window,
    ticket: &Ticket,
) -> Result<PreparedRelease, Invalidation> {
    window.prepare_retained_visual_release(ticket)
}

/// Returns whether an exact prepared retained-visual release can still commit.
#[doc(hidden)]
pub fn can_commit_prepared_release(window: &mut Window, prepared: &PreparedRelease) -> bool {
    window.can_commit_prepared_retained_visual_release(prepared)
}

/// Commits one previously validated retained-visual release.
#[doc(hidden)]
pub fn commit_prepared_release(window: &mut Window, prepared: PreparedRelease) -> ReleaseReceipt {
    window.commit_prepared_retained_visual_release(prepared)
}

/// Observes durable release of one exact retained-visual lease.
#[doc(hidden)]
pub fn observe_release(
    window: &mut Window,
    ticket: TicketIdentity,
) -> Result<Option<ReleaseReceipt>, Invalidation> {
    window.observe_retained_visual_release(ticket)
}

impl Window {
    fn validate_retained_visual_release_window(&self, ticket: &Ticket) -> Result<(), Invalidation> {
        let actual_window = self.handle.window_id();
        if ticket.source_window != actual_window {
            return Err(Invalidation::WrongWindow {
                expected: ticket.source_window,
                actual: actual_window,
            });
        }
        Ok(())
    }

    fn prepare_retained_visual_release(
        &mut self,
        ticket: &Ticket,
    ) -> Result<PreparedRelease, Invalidation> {
        self.validate_retained_visual_release_window(ticket)?;
        self.retained_visual_registry.prepare_release(ticket)
    }

    fn can_commit_prepared_retained_visual_release(&self, prepared: &PreparedRelease) -> bool {
        self.validate_retained_visual_release_window(&prepared.ticket)
            .is_ok()
            && self
                .retained_visual_registry
                .can_commit_prepared_release(prepared)
    }

    fn commit_prepared_retained_visual_release(
        &mut self,
        prepared: PreparedRelease,
    ) -> ReleaseReceipt {
        assert!(
            self.can_commit_prepared_retained_visual_release(&prepared),
            "prepared retained-visual release must remain exact in its source window"
        );
        self.retained_visual_registry
            .commit_prepared_release(prepared)
    }

    fn observe_retained_visual_release(
        &self,
        ticket: TicketIdentity,
    ) -> Result<Option<ReleaseReceipt>, Invalidation> {
        let actual_window = self.handle.window_id();
        if ticket.source_window != actual_window {
            return Err(Invalidation::WrongWindow {
                expected: ticket.source_window,
                actual: actual_window,
            });
        }
        Ok(self.retained_visual_registry.observe_release(ticket))
    }

    fn retained_visual_prepaint_index(&self) -> VisualPrepaintIndex {
        VisualPrepaintIndex {
            retained_resources_index: self.next_frame.retained_resources.len(),
            deferred_draws_index: self.next_frame.deferred_draws.len(),
        }
    }

    fn retained_visual_paint_index(&self) -> VisualPaintIndex {
        VisualPaintIndex {
            scene_index: self.next_frame.scene.journal_len(),
            atlas_texture_lease_entries_index: self.next_frame.atlas_texture_lease_entries.len(),
            atlas_access_diagnostics_index: self.next_frame.atlas_access_diagnostic_entries.len(),
            image_paint_diagnostics_index: self.next_frame.image_paint_diagnostic_entries.len(),
            subtree_transform_diagnostics_index: self
                .next_frame
                .subtree_transform_diagnostics
                .len(),
        }
    }

    fn record_retained_visual_source(
        &mut self,
        source_id: SourceId,
        bounds: Bounds<Pixels>,
        prepaint_range: Range<VisualPrepaintIndex>,
        paint_range: Range<VisualPaintIndex>,
    ) {
        let publication =
            self.capture_retained_visual(source_id, bounds, prepaint_range, paint_range);
        self.next_frame
            .retained_visual_publications
            .push(publication);
    }

    fn capture_retained_visual(
        &self,
        source_id: SourceId,
        bounds: Bounds<Pixels>,
        prepaint_range: Range<VisualPrepaintIndex>,
        paint_range: Range<VisualPaintIndex>,
    ) -> PendingSourcePublication {
        if prepaint_range.start.deferred_draws_index != prepaint_range.end.deferred_draws_index {
            return PendingSourcePublication::failed(
                source_id.clone(),
                Invalidation::DeferredOutputUnsupported(source_id),
            );
        }

        let (scene, operation_validities) =
            match self.next_frame.scene.retained_visual_fragment(
                paint_range.start.scene_index..paint_range.end.scene_index,
            ) {
                Ok(fragment) => fragment,
                Err(_) => {
                    return PendingSourcePublication::failed(
                        source_id.clone(),
                        Invalidation::SubtreeGeometryUnavailable(source_id),
                    );
                }
            };
        if !scene.has_primitives() {
            return PendingSourcePublication::failed(
                source_id.clone(),
                Invalidation::EmptyVisual(source_id),
            );
        }

        let atlas_texture_lease_entries = match self.next_frame.atlas_texture_lease_entries.get(
            paint_range.start.atlas_texture_lease_entries_index
                ..paint_range.end.atlas_texture_lease_entries_index,
        ) {
            Some(entries) => entries,
            None => {
                return PendingSourcePublication::failed(
                    source_id.clone(),
                    Invalidation::SubtreeGeometryUnavailable(source_id),
                );
            }
        };
        let mut atlas_leases = Vec::new();
        for entry in atlas_texture_lease_entries {
            let lease = match entry {
                Ok(lease) => lease,
                Err(error) => {
                    return PendingSourcePublication::failed(
                        source_id,
                        Invalidation::ResourceUnavailable(*error),
                    );
                }
            };
            if !atlas_leases
                .iter()
                .any(|existing| Rc::ptr_eq(existing, lease))
            {
                atlas_leases.push(lease.clone());
            }
        }
        for texture in scene.atlas_texture_instances() {
            if !atlas_leases
                .iter()
                .any(|lease| lease.texture_instances().contains(&texture))
            {
                return PendingSourcePublication::failed(
                    source_id,
                    Invalidation::ResourceUnavailable(AtlasTextureLeaseError::TextureUnavailable {
                        texture,
                        epoch: self.sprite_atlas.atlas_texture_lease_epoch(),
                    }),
                );
            }
        }
        let retained_resources: Rc<[Rc<dyn Any>]> = match self.next_frame.retained_resources.get(
            prepaint_range.start.retained_resources_index
                ..prepaint_range.end.retained_resources_index,
        ) {
            Some(resources) => Rc::from(resources.to_vec().into_boxed_slice()),
            None => {
                return PendingSourcePublication::failed(
                    source_id.clone(),
                    Invalidation::SubtreeGeometryUnavailable(source_id),
                );
            }
        };
        let atlas_access_diagnostics: Rc<[AtlasAccessDiagnostic]> = Rc::from(
            self.next_frame.atlas_access_diagnostic_entries[paint_range
                .start
                .atlas_access_diagnostics_index
                ..paint_range.end.atlas_access_diagnostics_index]
                .iter()
                .filter(|entry| entry.is_valid())
                .map(|entry| entry.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let image_paint_diagnostics: Rc<[ImagePaintDiagnostic]> = Rc::from(
            self.next_frame.image_paint_diagnostic_entries[paint_range
                .start
                .image_paint_diagnostics_index
                ..paint_range.end.image_paint_diagnostics_index]
                .iter()
                .filter(|entry| entry.is_valid())
                .map(|entry| entry.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let subtree_transform_diagnostics: Rc<[SubtreeTransformDiagnostic]> = Rc::from(
            self.next_frame.subtree_transform_diagnostics[paint_range
                .start
                .subtree_transform_diagnostics_index
                ..paint_range.end.subtree_transform_diagnostics_index]
                .to_vec()
                .into_boxed_slice(),
        );

        PendingSourcePublication {
            source_id,
            record: Ok(Rc::new(CommittedVisual {
                source_window: self.handle.window_id(),
                source_frame_generation: self.next_frame.generation,
                bounds,
                scale_factor: self.scale_factor,
                scene: Rc::new(scene),
                retained_resources,
                atlas_access_diagnostics,
                image_paint_diagnostics,
                subtree_transform_diagnostics,
                atlas_leases: Rc::from(atlas_leases.into_boxed_slice()),
            })),
            operation_validities,
        }
    }

    fn lease_committed_retained_visual(
        &mut self,
        source_id: &SourceId,
    ) -> Result<Ticket, Invalidation> {
        if self.removed || self.native_closed.get() {
            return Err(Invalidation::WindowClosed);
        }
        let visual = self
            .rendered_frame
            .retained_visual_sources
            .get(source_id)
            .cloned()
            .ok_or_else(|| Invalidation::SourceUnavailable(source_id.clone()))??;
        self.validate_retained_visual(&visual)?;
        self.retained_visual_registry.activate(source_id, visual)
    }

    fn validate_retained_visual(&self, visual: &CommittedVisual) -> Result<(), Invalidation> {
        if self.scale_factor.to_bits() != visual.scale_factor.to_bits() {
            return Err(Invalidation::ScaleChanged {
                expected: visual.scale_factor,
                actual: self.scale_factor,
            });
        }
        for lease in visual.atlas_leases.iter() {
            lease
                .validate()
                .map_err(|error| Invalidation::RendererEpochChanged {
                    expected: error.expected_epoch.get(),
                    actual: error.actual_epoch.get(),
                })?;
        }
        Ok(())
    }

    fn replay_retained_visual(&mut self, ticket: &Ticket) -> Result<ReplayReceipt, Invalidation> {
        if self.removed || self.native_closed.get() {
            return Err(Invalidation::WindowClosed);
        }
        if !self.invalidator.is_paint() {
            return Err(Invalidation::OutsidePaint);
        }
        let actual_window = self.handle.window_id();
        if ticket.source_window != actual_window {
            return Err(Invalidation::WrongWindow {
                expected: ticket.source_window,
                actual: actual_window,
            });
        }
        let frame_generation = self.next_frame.generation;
        let (source_id, visual) = {
            let active = self.retained_visual_registry.active_mut(ticket)?;
            (active.source_id.clone(), active.visual.clone())
        };
        self.validate_retained_visual(&visual)?;
        let ticket_identity = ticket.identity();
        let candidate_frame = self
            .candidate_frame_transaction
            .as_ref()
            .expect("retained visual replay must run inside one candidate frame transaction");
        let replay_attempt_id = candidate_frame.attempt_id.0;
        if candidate_frame.retained_visual_was_replayed(ticket_identity) {
            return Err(Invalidation::DuplicateReplay);
        }
        let validity = self.subtree_geometry_validity();

        if let Err(error) = self.next_frame.scene.replay(
            0..visual.scene.journal_len(),
            visual.scene.as_ref(),
            validity.clone(),
        ) {
            self.record_subtree_geometry_failure(error);
            return Err(Invalidation::SubtreeGeometryUnavailable(source_id));
        }

        self.candidate_frame_transaction
            .as_mut()
            .expect("retained visual replay must keep its candidate frame transaction")
            .record_retained_visual_replay(ticket_identity);
        self.next_frame
            .retained_visual_replays
            .push(ticket_identity);
        for lease in visual.atlas_leases.iter() {
            for texture in lease.texture_instances() {
                self.next_frame
                    .atlas_texture_leases_by_instance
                    .entry(*texture)
                    .or_insert_with(|| lease.clone());
            }
            self.record_atlas_texture_lease_entry(Ok(lease.clone()));
        }
        self.next_frame
            .retained_resources
            .extend(visual.retained_resources.iter().cloned());
        self.next_frame.atlas_access_diagnostic_entries.extend(
            visual
                .atlas_access_diagnostics
                .iter()
                .copied()
                .map(|diagnostic| FrameOutput::new(diagnostic, validity.clone())),
        );
        self.next_frame.image_paint_diagnostic_entries.extend(
            visual
                .image_paint_diagnostics
                .iter()
                .copied()
                .map(|mut diagnostic| {
                    diagnostic.frame_generation = frame_generation;
                    FrameOutput::new(diagnostic, validity.clone())
                }),
        );
        self.next_frame.subtree_transform_diagnostics.extend(
            visual
                .subtree_transform_diagnostics
                .iter()
                .copied()
                .map(|mut diagnostic| {
                    diagnostic.frame_generation = frame_generation;
                    diagnostic
                }),
        );
        Ok(ReplayReceipt {
            source_window: ticket.source_window,
            source_frame_generation: ticket.source_frame_generation,
            lease_generation: ticket.lease_generation,
            replay_frame_generation: frame_generation,
            replay_attempt_id,
        })
    }

    fn release_retained_visual(&mut self, ticket: &Ticket) -> Result<(), Invalidation> {
        self.validate_retained_visual_release_window(ticket)?;
        self.retained_visual_registry.release(ticket)
    }

    pub(super) fn retire_retained_visuals(&mut self) {
        self.retained_visual_registry.clear();
        self.rendered_frame.retained_visual_publications.clear();
        self.next_frame.retained_visual_publications.clear();
        self.rendered_frame.retained_visual_sources.clear();
        self.next_frame.retained_visual_sources.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyView, AppContext as _, AtlasKey, AtlasRemoveOutcome, Context, Corners,
        InteractiveElement as _, Modifiers, ParentElement as _, Render, RenderImage,
        RenderImageParams, StatefulInteractiveElement as _, StyleRefinement, Styled as _,
        SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, canvas,
        deferred, div, fill, point, px, red, size,
    };
    use image::{Rgba, RgbaImage};
    use std::{cell::Cell, cell::RefCell, sync::Arc};

    struct ResourceLifetime;

    #[derive(Clone, Copy)]
    enum ProbeMode {
        Source,
        Replay(Ticket),
        Empty,
    }

    struct RetainedVisualProbe {
        mode: ProbeMode,
        source_id: SourceId,
        clicks: Rc<Cell<usize>>,
        resource: Option<Rc<ResourceLifetime>>,
        replay_error: Rc<RefCell<Option<Invalidation>>>,
        replay_receipt: Rc<RefCell<Option<ReplayReceipt>>>,
    }

    impl Render for RetainedVisualProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            match self.mode {
                ProbeMode::Source => {
                    let clicks = self.clicks.clone();
                    source(
                        self.source_id.clone(),
                        div()
                            .size_full()
                            .bg(red())
                            .id("retained-visual-click-source")
                            .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                            .retain_for_frame(
                                self.resource
                                    .as_ref()
                                    .expect("source mode should own the test resource")
                                    .clone(),
                            ),
                    )
                    .into_any_element()
                }
                ProbeMode::Replay(ticket) => {
                    let replay_error = self.replay_error.clone();
                    let replay_receipt = self.replay_receipt.clone();
                    canvas(
                        |_, _, _| (),
                        move |_, _, window, _| match replay(window, &ticket) {
                            Ok(receipt) => *replay_receipt.borrow_mut() = Some(receipt),
                            Err(error) => *replay_error.borrow_mut() = Some(error),
                        },
                    )
                    .size_full()
                    .into_any_element()
                }
                ProbeMode::Empty => div().size_full().into_any_element(),
            }
        }
    }

    #[test]
    fn retained_visual_replays_only_paint_and_keeps_resources_until_release() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("retained-visual-probe");
        let clicks = Rc::new(Cell::new(0));
        let resource = Rc::new(ResourceLifetime);
        let resource_weak = Rc::downgrade(&resource);
        let replay_error = Rc::new(RefCell::new(None));
        let replay_receipt = Rc::new(RefCell::new(None));
        let (root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            let clicks = clicks.clone();
            let replay_error = replay_error.clone();
            let replay_receipt = replay_receipt.clone();
            move |_, _| RetainedVisualProbe {
                mode: ProbeMode::Source,
                source_id,
                clicks,
                resource: Some(resource),
                replay_error,
                replay_receipt,
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert!(
            cx.update(|window, _| window.rendered_frame.scene.has_primitives()),
            "the source frame should contain paint primitives"
        );
        cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
        assert_eq!(clicks.get(), 1);

        let ticket = cx.update(|window, _| {
            lease_committed(window, &source_id).expect("source should publish a committed visual")
        });
        root.update(cx, |root, cx| {
            root.mode = ProbeMode::Replay(ticket);
            root.resource = None;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());

        assert_eq!(*replay_error.borrow(), None);
        let replay_receipt = replay_receipt
            .borrow()
            .expect("successful replay must issue an exact candidate-frame receipt");
        assert_eq!(replay_receipt.source_window(), ticket.source_window());
        assert_eq!(
            replay_receipt.source_frame_generation(),
            ticket.source_frame_generation()
        );
        assert_eq!(replay_receipt.lease_generation(), ticket.lease_generation());
        assert_eq!(
            replay_receipt.replay_frame_generation(),
            cx.update(|window, _| window.rendered_frame.generation)
        );
        assert!(
            cx.update(|window, _| window.rendered_frame.scene.has_primitives()),
            "the paint-only replay should preserve the source pixels"
        );
        assert!(
            resource_weak.upgrade().is_some(),
            "the lease must keep frame resources alive"
        );
        cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
        assert_eq!(
            clicks.get(),
            1,
            "paint replay must not revive the source hitbox or click listener"
        );

        cx.update(|window, _| release(window, &ticket).expect("exact live ticket should release"));
        root.update(cx, |root, cx| {
            root.mode = ProbeMode::Empty;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(
            resource_weak.upgrade().is_none(),
            "the retained resource should release after both lease and replay frame retire"
        );
    }

    #[test]
    fn prepared_release_is_single_use_and_keeps_the_lease_active_until_commit() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("prepared-release-source");
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            move |_, _| RetainedVisualProbe {
                mode: ProbeMode::Source,
                source_id,
                clicks: Rc::new(Cell::new(0)),
                resource: Some(Rc::new(ResourceLifetime)),
                replay_error: Rc::new(RefCell::new(None)),
                replay_receipt: Rc::new(RefCell::new(None)),
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());

        let first_ticket = cx.update(|window, _| {
            lease_committed(window, &source_id).expect("source should publish a committed visual")
        });
        let first_generation = first_ticket.lease_generation();
        let prepared = cx.update(|window, _| {
            prepare_release(window, &first_ticket).expect("the exact active lease should prepare")
        });
        let prepared_identity = prepared.ticket_identity();

        assert_eq!(
            cx.update(|window, _| lease_committed(window, &source_id)),
            Err(Invalidation::AlreadyLeased(source_id.clone())),
            "preparing a release must not consume the active lease"
        );
        assert!(
            cx.update(|window, _| can_commit_prepared_release(window, &prepared)),
            "the prepared release must remain exact before commit"
        );
        assert_eq!(
            cx.update(|window, _| observe_release(window, prepared_identity)),
            Ok(None),
            "an active prepared lease must not be reported as released"
        );

        let receipt = cx.update(|window, _| commit_prepared_release(window, prepared));
        assert_eq!(receipt.ticket_identity(), prepared_identity);
        assert_eq!(
            cx.update(|window, _| observe_release(window, prepared_identity)),
            Ok(Some(receipt)),
            "the exact release must be replay-observable after the linear token is consumed"
        );
        assert_eq!(
            cx.update(|window, _| release(window, &first_ticket)),
            Err(Invalidation::StaleGeneration),
            "committing the prepared release must make the old ticket stale"
        );

        let second_ticket = cx.update(|window, _| {
            lease_committed(window, &source_id)
                .expect("the source should be leasable again after commit")
        });
        assert!(
            second_ticket.lease_generation() > first_generation,
            "a new lease must receive a later generation"
        );
        assert_eq!(
            cx.update(|window, _| observe_release(window, prepared_identity)),
            Ok(Some(receipt)),
            "a later lease generation must not invalidate exact release evidence"
        );
        cx.update(|window, _| release(window, &second_ticket).unwrap());
    }

    #[test]
    fn retained_visual_rejects_scale_changes_without_consuming_the_ticket() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("scale-bound-source");
        let replay_error = Rc::new(RefCell::new(None));
        let replay_receipt = Rc::new(RefCell::new(None));
        let (root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            let replay_error = replay_error.clone();
            let replay_receipt = replay_receipt.clone();
            move |_, _| RetainedVisualProbe {
                mode: ProbeMode::Source,
                source_id,
                clicks: Rc::new(Cell::new(0)),
                resource: Some(Rc::new(ResourceLifetime)),
                replay_error,
                replay_receipt,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        let ticket = cx.update(|window, _| lease_committed(window, &source_id).unwrap());

        root.update(cx, |root, cx| {
            root.mode = ProbeMode::Replay(ticket);
            cx.notify();
        });
        cx.update(|window, cx| {
            window.scale_factor *= 2.0;
            window.draw(cx).clear();
        });
        assert!(matches!(
            *replay_error.borrow(),
            Some(Invalidation::ScaleChanged { .. })
        ));
        cx.update(|window, _| {
            release(window, &ticket).expect("failed replay must leave the exact lease releasable")
        });
    }

    #[test]
    fn retained_visual_rejects_replay_outside_paint_without_consuming_the_ticket() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("paint-phase-source");
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            move |_, _| RetainedVisualProbe {
                mode: ProbeMode::Source,
                source_id,
                clicks: Rc::new(Cell::new(0)),
                resource: Some(Rc::new(ResourceLifetime)),
                replay_error: Rc::new(RefCell::new(None)),
                replay_receipt: Rc::new(RefCell::new(None)),
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        let ticket = cx.update(|window, _| lease_committed(window, &source_id).unwrap());

        assert_eq!(
            cx.update(|window, _| replay(window, &ticket)),
            Err(Invalidation::OutsidePaint)
        );
        cx.update(|window, _| release(window, &ticket).unwrap());
    }

    struct CachedSourceChild {
        source_id: SourceId,
        renders: Rc<Cell<usize>>,
        deferred: bool,
    }

    impl Render for CachedSourceChild {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let source = source(self.source_id.clone(), div().size_full().bg(red()));
            if self.deferred {
                deferred(source).into_any_element()
            } else {
                source.into_any_element()
            }
        }
    }

    struct CachedSourceRoot {
        child: crate::Entity<CachedSourceChild>,
    }

    impl Render for CachedSourceRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full())
        }
    }

    fn assert_cached_source_republishes_in_current_frame(deferred: bool) {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new(if deferred {
            "cached-deferred-source"
        } else {
            "cached-direct-source"
        });
        let renders = Rc::new(Cell::new(0));
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            let renders = renders.clone();
            move |_, cx| CachedSourceRoot {
                child: cx.new(|_| CachedSourceChild {
                    source_id,
                    renders,
                    deferred,
                }),
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        let first_generation = cx.update(|window, _| window.rendered_frame.generation);
        let first_render_count = renders.get();
        assert!(first_render_count > 0);

        cx.update(|window, cx| window.draw(cx).clear());
        let second_generation = cx.update(|window, _| window.rendered_frame.generation);
        assert!(second_generation > first_generation);
        assert_eq!(
            renders.get(),
            first_render_count,
            "the second frame must replay the cached paint journal"
        );

        let ticket = cx.update(|window, _| {
            lease_committed(window, &source_id)
                .expect("a cached source must republish into the current committed frame")
        });
        assert_eq!(ticket.source_frame_generation(), second_generation);
        cx.update(|window, _| release(window, &ticket).unwrap());
    }

    #[test]
    fn cached_view_republishes_retained_visual_in_the_current_frame() {
        assert_cached_source_republishes_in_current_frame(false);
    }

    #[test]
    fn cached_deferred_view_republishes_retained_visual_in_the_current_frame() {
        assert_cached_source_republishes_in_current_frame(true);
    }

    struct LateGeometryFailureSource {
        source_id: SourceId,
    }

    impl Render for LateGeometryFailureSource {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(source(
                    self.source_id.clone(),
                    div().w(px(10.0)).h(px(10.0)).bg(red()),
                ))
                .child(canvas(
                    |_, _, _| {},
                    |_, _, window, _| {
                        window.paint_quad(fill(
                            Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                            red(),
                        ));
                    },
                ))
                .with_subtree_transform(
                    SubtreeTransform::try_new(
                        size(2.0, 2.0),
                        point(px(0.0), px(0.0)),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                )
        }
    }

    #[test]
    fn late_geometry_failure_prevents_retained_visual_commit() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("late-geometry-failure-source");
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            move |_, _| LateGeometryFailureSource { source_id }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(
            cx.update(|window, _| lease_committed(window, &source_id)),
            Err(Invalidation::SubtreeGeometryUnavailable(source_id))
        );
    }

    struct AtlasImageProbe {
        mode: ProbeMode,
        source_id: SourceId,
        image: Option<Arc<RenderImage>>,
        replay_error: Rc<RefCell<Option<Invalidation>>>,
    }

    impl Render for AtlasImageProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            match self.mode {
                ProbeMode::Source => {
                    let image = self
                        .image
                        .as_ref()
                        .expect("source mode should own the image")
                        .clone();
                    source(
                        self.source_id.clone(),
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                window
                                    .paint_image(
                                        bounds,
                                        Corners::default(),
                                        image.clone(),
                                        0,
                                        false,
                                    )
                                    .expect("test image should paint");
                            },
                        )
                        .size_full(),
                    )
                    .into_any_element()
                }
                ProbeMode::Replay(ticket) => {
                    let replay_error = self.replay_error.clone();
                    canvas(
                        |_, _, _| (),
                        move |_, _, window, _| {
                            *replay_error.borrow_mut() = replay(window, &ticket).err();
                        },
                    )
                    .size_full()
                    .into_any_element()
                }
                ProbeMode::Empty => div().size_full().into_any_element(),
            }
        }
    }

    struct AtlasRemovalDuringSourceFrame {
        source_id: SourceId,
        image: Arc<RenderImage>,
        image_params: RenderImageParams,
        remove_outcome: Rc<Cell<Option<AtlasRemoveOutcome>>>,
    }

    struct AtlasRemovalInsideSourcePaint {
        source_id: SourceId,
        image: Arc<RenderImage>,
        image_params: RenderImageParams,
        remove_outcome: Rc<Cell<Option<AtlasRemoveOutcome>>>,
    }

    impl Render for AtlasRemovalInsideSourcePaint {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let image = self.image.clone();
            let image_params = self.image_params;
            let remove_outcome = self.remove_outcome.clone();
            source(
                self.source_id.clone(),
                div()
                    .size_full()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                window
                                    .paint_image(
                                        bounds,
                                        Corners::default(),
                                        image.clone(),
                                        0,
                                        false,
                                    )
                                    .expect("test image should paint");
                            },
                        )
                        .size_full(),
                    )
                    .child(canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            let removal = window
                                .sprite_atlas
                                .remove_with_diagnostics(&AtlasKey::Image(image_params));
                            remove_outcome.set(Some(removal.outcome));
                        },
                    )),
            )
        }
    }

    impl Render for AtlasRemovalDuringSourceFrame {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let image = self.image.clone();
            let image_params = self.image_params;
            let remove_outcome = self.remove_outcome.clone();
            div()
                .size_full()
                .child(source(
                    self.source_id.clone(),
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            window
                                .paint_image(bounds, Corners::default(), image.clone(), 0, false)
                                .expect("test image should paint");
                        },
                    )
                    .size_full(),
                ))
                .child(canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let removal = window
                            .sprite_atlas
                            .remove_with_diagnostics(&AtlasKey::Image(image_params));
                        remove_outcome.set(Some(removal.outcome));
                    },
                ))
        }
    }

    #[test]
    fn source_pins_atlas_textures_before_later_siblings_can_remove_the_key() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("same-frame-atlas-removal-source");
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            RgbaImage::from_pixel(1, 1, Rgba([0xff, 0, 0, 0xff])),
        )]));
        let image_params = RenderImageParams {
            image_id: image.id,
            frame_index: 0,
        };
        let remove_outcome = Rc::new(Cell::new(None));
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            let remove_outcome = remove_outcome.clone();
            move |_, _| AtlasRemovalDuringSourceFrame {
                source_id,
                image,
                image_params,
                remove_outcome,
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(
            remove_outcome.get(),
            Some(AtlasRemoveOutcome::TextureRetained)
        );
        let frame_generation = cx.update(|window, _| window.rendered_frame.generation);
        let ticket = cx.update(|window, _| {
            lease_committed(window, &source_id)
                .expect("the immediate source pin must survive later same-frame key removal")
        });
        assert_eq!(ticket.source_frame_generation(), frame_generation);
        cx.update(|window, _| release(window, &ticket).unwrap());
    }

    #[test]
    fn source_pins_atlas_textures_before_later_paint_inside_the_source_can_remove_the_key() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("same-source-atlas-removal");
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            RgbaImage::from_pixel(1, 1, Rgba([0xff, 0, 0, 0xff])),
        )]));
        let image_params = RenderImageParams {
            image_id: image.id,
            frame_index: 0,
        };
        let remove_outcome = Rc::new(Cell::new(None));
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            let remove_outcome = remove_outcome.clone();
            move |_, _| AtlasRemovalInsideSourcePaint {
                source_id,
                image,
                image_params,
                remove_outcome,
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(
            remove_outcome.get(),
            Some(AtlasRemoveOutcome::TextureRetained),
            "the primitive must acquire its frame pin before later paint in the same source runs"
        );
        let ticket = cx.update(|window, _| {
            lease_committed(window, &source_id)
                .expect("same-source key removal must not invalidate the captured visual")
        });
        cx.update(|window, _| release(window, &ticket).unwrap());
    }

    #[test]
    fn atlas_key_removal_is_deferred_through_the_last_replay_frame() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("atlas-image-source");
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            RgbaImage::from_pixel(1, 1, Rgba([0xff, 0, 0, 0xff])),
        )]));
        let image_params = RenderImageParams {
            image_id: image.id,
            frame_index: 0,
        };
        let replay_error = Rc::new(RefCell::new(None));
        let (root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            let replay_error = replay_error.clone();
            move |_, _| AtlasImageProbe {
                mode: ProbeMode::Source,
                source_id,
                image: Some(image),
                replay_error,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());

        let ticket = cx.update(|window, _| lease_committed(window, &source_id).unwrap());
        let texture = cx.update(|window, _| {
            let visual = window.rendered_frame.retained_visual_sources[&source_id]
                .as_ref()
                .expect("source visual should be committed");
            visual.atlas_leases[0].texture_instances()[0]
        });
        let remove = cx.update(|window, _| {
            window
                .sprite_atlas
                .remove_with_diagnostics(&AtlasKey::Image(image_params))
        });
        assert_eq!(remove.outcome, AtlasRemoveOutcome::TextureRetained);

        root.update(cx, |root, cx| {
            root.mode = ProbeMode::Replay(ticket);
            root.image = None;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(*replay_error.borrow(), None);

        cx.update(|window, _| release(window, &ticket).unwrap());
        let replay_frame_pin = cx.update(|window, _| {
            window
                .sprite_atlas
                .clone()
                .retain_texture_instances(&[texture])
                .expect("the committed replay frame must keep its atlas texture resident")
        });
        drop(replay_frame_pin);

        root.update(cx, |root, cx| {
            root.mode = ProbeMode::Empty;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(matches!(
            cx.update(|window, _| {
                window
                    .sprite_atlas
                    .clone()
                    .retain_texture_instances(&[texture])
            }),
            Err(AtlasTextureLeaseError::TextureUnavailable { .. })
        ));
    }

    struct DuplicateSourceProbe {
        source_id: SourceId,
    }

    impl Render for DuplicateSourceProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(source(
                    self.source_id.clone(),
                    div().w(px(10.0)).h(px(10.0)).bg(red()),
                ))
                .child(source(
                    self.source_id.clone(),
                    div().w(px(10.0)).h(px(10.0)).bg(red()),
                ))
        }
    }

    #[test]
    fn duplicate_source_identity_fails_closed() {
        let mut test_app = TestAppContext::single();
        let source_id = SourceId::new("duplicate-source");
        let (_root, cx) = test_app.add_window_view({
            let source_id = source_id.clone();
            move |_, _| DuplicateSourceProbe { source_id }
        });
        cx.update(|window, cx| window.draw(cx).clear());

        assert_eq!(
            cx.update(|window, _| lease_committed(window, &source_id)),
            Err(Invalidation::DuplicateSource(source_id))
        );
    }
}
