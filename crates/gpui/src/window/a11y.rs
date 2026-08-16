//! Accessibility support, provided by [AccessKit][accesskit].
//!
//! There are user-facing guide-level docs [here](crate::_accessibility).
//!
//! ## Architecture
//!
//! ```text
//!                              ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           ┌─▶│ AccessKit Adapter (MacOS)      │◀─▶│ MacOS System APIs   │
//!                           │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//! ┌──────┐   ┌───────────┐  │  ┌────────────────────────────────┐   ┌─────────────────────┐
//! │ GPUI │◀─▶│ AccessKit │◀─┼─▶│ AccessKit Adapter (Windows)    │◀─▶│ Windows System APIs │
//! └──────┘   └───────────┘  │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//!                           │  ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           └─▶│ AccessKit Adapter (Linux)      │◀─▶│ dbus                │
//!                              └────────────────────────────────┘   └─────────────────────┘
//! ```
//!
//! In order for GPUI apps to be usable for people using assistive technology,
//! we must do a few things:
//! - Inform the system when the UI changes meaningfully. This includes:
//!   - Reporting new/removed/changed UI elements
//!   - *Not* reporting irrelevant UI changes, e.g. an invisible `div()` being
//!     added.
//!   - Reporting the appearance and capabilities of each UI element. For example:
//!     - What does this piece of text say?
//!     - How far along is this progress bar?
//!     - Can this node be focused?
//!     - Can this node have a value directly assigned? (e.g. a slider)
//! - Allowing the system to interact with the UI by dispatching actions to
//!   nodes. Note that AccessKit has its own [`Action`] type, which is not the
//!   [`crate::Action`] trait.
//! - Activate and deactivate accessibility features when requested by the
//!   system.
//!
//! Activating and deactivating at the right time is trivial, so I won't go into
//! detail here. The other two are almost orthogonal in implementation.
//!
//! The state for both lives in the [`A11y`] struct in this module.
//!
//! ### Reporting UI changes
//!
//! Every frame, we build a [`TreeUpdate`] and send it to the platform-specific
//! adapter. A [`TreeUpdate`] is a representation of a subset of the UI tree.
//! When the adapter receives the update, it diffs it against the previous
//! update, and calls platform-specific APIs to inform screen readers about the
//! changes. Nodes may have been created, destroyed, or updated.
//!
//! Each node has an ID, and this ID *should* be stable across frames. If a
//! node's ID changes, then, from AccessKit's point of view, it is a different
//! node.
//!
//! We derive the node ID from the [`GlobalElementId`] in
//! [`GlobalElementId::accesskit_node_id`]. Nodes without [`GlobalElementId`]s
//! cannot produce an AccessKit [`NodeId`], and so are not included in the
//! accessibility tree. We try to warn when using accessibility APIs on
//! [`div()`] without setting an ID.
//!
//! This all happens in [`Drawable::prepaint`]. The [`A11y`] struct maintains a
//! stack of nodes during prepainting, which we can use to calculate the
//! [`NodeId`]s, and record parent-child relationships. Once all [`Element`]s in
//! a frame have been prepainted, we send the resulting [`TreeUpdate`] object to
//! the adapter and the screen reader can announce the changes.
//!
//! ### Responding to actions
//!  
//! On adapter creation, we provide a callback to the adapter, which can be used
//! to dispatch actions. Candidate listeners are collected per frame, then only
//! the listeners attached to the delivered tree become part of the published
//! action snapshot.
//!
//! This is populated in:
//! - [`Window::on_a11y_action`], which is called by:
//! - [`Interactivity::paint`], which is called by:
//! - [`InteractiveElement::on_a11y_action`], which is a public-facing API
//!
//! Candidate listeners are cleared at the start of a frame and re-populated
//! during painting. Published listeners remain available until a matching
//! activation generation delivers a replacement tree.
//!
//! [`Element`]: crate::Element
//! [`GlobalElementId`]: crate::GlobalElementId
//! [`div()`]: crate::div
//! [`Interactivity::paint`]: crate::Interactivity::paint
//! [`InteractiveElement::on_a11y_action`]: crate::InteractiveElement::on_a11y_action
//! [`NodeId`]: accesskit::NodeId
//! [`Drawable::prepaint`]: crate::Drawable::prepaint

use crate::{
    App, Bounds, FocusId, Pixels, Point, SharedString, Window, WindowId,
    geometry::SubtreeGeometryValidity,
};
use accesskit::{Action, NodeId, TreeUpdate};
use open_gpui_collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// The fixed AccessKit node ID used for the root of every window's a11y tree.
pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);

const ANNOUNCEMENT_QUEUE_CAPACITY: usize = 32;
const ANNOUNCEMENT_DIAGNOSTIC_CAPACITY: usize = 128;

/// The priority hint for a window-scoped accessibility announcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAnnouncementPoliteness {
    /// Announce at the next suitable opportunity.
    Polite,
    /// Announce with high priority; focus remains unchanged.
    Assertive,
}

impl AccessibilityAnnouncementPoliteness {
    const fn accesskit_live(self) -> accesskit::Live {
        match self {
            Self::Polite => accesskit::Live::Polite,
            Self::Assertive => accesskit::Live::Assertive,
        }
    }

    const fn accesskit_role(self) -> accesskit::Role {
        match self {
            Self::Polite => accesskit::Role::Status,
            Self::Assertive => accesskit::Role::Alert,
        }
    }
}

/// A transient, window-scoped accessibility announcement request.
#[derive(Clone)]
pub struct AccessibilityAnnouncement {
    message: SharedString,
    politeness: AccessibilityAnnouncementPoliteness,
}

impl AccessibilityAnnouncement {
    /// Creates a polite announcement request.
    pub fn polite(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            politeness: AccessibilityAnnouncementPoliteness::Polite,
        }
    }

    /// Creates an assertive announcement request.
    pub fn assertive(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            politeness: AccessibilityAnnouncementPoliteness::Assertive,
        }
    }

    /// Returns the request's priority hint.
    pub const fn politeness(&self) -> AccessibilityAnnouncementPoliteness {
        self.politeness
    }
}

impl fmt::Debug for AccessibilityAnnouncement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccessibilityAnnouncement")
            .field("message", &"<redacted>")
            .field("politeness", &self.politeness)
            .finish()
    }
}

/// A per-window identity allocated for every announcement request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessibilityAnnouncementRequestId(u64);

impl AccessibilityAnnouncementRequestId {
    /// Returns the numeric request identity.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// A per-window sequence allocated only for accepted announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessibilityAnnouncementSequence(u64);

impl AccessibilityAnnouncementSequence {
    /// Returns the numeric accepted-announcement sequence.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Why a transient announcement request was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAnnouncementDropReason {
    /// The platform accessibility adapter is not active for this window.
    AccessibilityInactive,
    /// The fixed per-window queue already contains 32 pending or retained requests.
    QueueFull,
    /// The live window permanently revoked user interaction while retaining its final visuals.
    InteractionQuiesced,
    /// The window has started closing or has already closed.
    WindowClosed,
}

/// Why an accepted announcement was cleared before completing its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAnnouncementClearReason {
    /// Accessibility was deactivated for the window.
    AccessibilityDeactivated,
    /// A replacement accessibility activation generation superseded the request.
    ActivationReplaced,
    /// The live window permanently revoked user interaction while retaining its final visuals.
    InteractionQuiesced,
    /// The window started closing.
    WindowClosed,
}

/// The metadata-only lifecycle recorded for an announcement request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAnnouncementLifecycle {
    /// The request entered the bounded queue.
    Accepted,
    /// The synthetic node entered a matching committed accessibility tree.
    Committed,
    /// A later matching tree committed removal of the synthetic node.
    Removed,
    /// The request was rejected before receiving an announcement sequence.
    Dropped(AccessibilityAnnouncementDropReason),
    /// An accepted request was cleared by a lifecycle boundary.
    Cleared(AccessibilityAnnouncementClearReason),
}

/// A metadata-only accessibility announcement diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccessibilityAnnouncementDiagnostic {
    window_id: WindowId,
    request_id: AccessibilityAnnouncementRequestId,
    sequence: Option<AccessibilityAnnouncementSequence>,
    politeness: AccessibilityAnnouncementPoliteness,
    lifecycle: AccessibilityAnnouncementLifecycle,
}

impl AccessibilityAnnouncementDiagnostic {
    /// Returns the window that owned the request.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the identity allocated for the request.
    pub const fn request_id(self) -> AccessibilityAnnouncementRequestId {
        self.request_id
    }

    /// Returns the accepted sequence, or `None` when the request was dropped.
    pub const fn sequence(self) -> Option<AccessibilityAnnouncementSequence> {
        self.sequence
    }

    /// Returns the request's priority hint.
    pub const fn politeness(self) -> AccessibilityAnnouncementPoliteness {
        self.politeness
    }

    /// Returns the recorded lifecycle transition.
    pub const fn lifecycle(self) -> AccessibilityAnnouncementLifecycle {
        self.lifecycle
    }
}

/// The synchronous result of submitting a window-scoped announcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAnnouncementOutcome {
    /// The request entered the bounded queue.
    Accepted {
        /// Identity allocated for this call.
        request_id: AccessibilityAnnouncementRequestId,
        /// Monotonic per-window accepted-announcement sequence.
        sequence: AccessibilityAnnouncementSequence,
    },
    /// The request was rejected and will never replay.
    Dropped {
        /// Identity allocated for this call.
        request_id: AccessibilityAnnouncementRequestId,
        /// Typed rejection reason.
        reason: AccessibilityAnnouncementDropReason,
    },
}

impl AccessibilityAnnouncementOutcome {
    /// Returns the identity allocated for this call.
    pub const fn request_id(self) -> AccessibilityAnnouncementRequestId {
        match self {
            Self::Accepted { request_id, .. } | Self::Dropped { request_id, .. } => request_id,
        }
    }

    /// Returns the accepted sequence, or `None` for a dropped request.
    pub const fn sequence(self) -> Option<AccessibilityAnnouncementSequence> {
        match self {
            Self::Accepted { sequence, .. } => Some(sequence),
            Self::Dropped { .. } => None,
        }
    }

    /// Returns the rejection reason, or `None` for an accepted request.
    pub const fn drop_reason(self) -> Option<AccessibilityAnnouncementDropReason> {
        match self {
            Self::Accepted { .. } => None,
            Self::Dropped { reason, .. } => Some(reason),
        }
    }

    /// Returns whether the request entered the queue.
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted { .. })
    }
}

#[derive(Clone, Copy)]
struct AnnouncementMetadata {
    request_id: AccessibilityAnnouncementRequestId,
    sequence: AccessibilityAnnouncementSequence,
    activation_generation: u64,
    node_id: NodeId,
    node_probe: u64,
    politeness: AccessibilityAnnouncementPoliteness,
}

struct PendingAnnouncement {
    metadata: AnnouncementMetadata,
    message: SharedString,
}

struct RetainedAnnouncement {
    metadata: AnnouncementMetadata,
}

enum QueuedAnnouncement {
    Pending(PendingAnnouncement),
    Retained(RetainedAnnouncement),
}

impl QueuedAnnouncement {
    const fn metadata(&self) -> AnnouncementMetadata {
        match self {
            Self::Pending(pending) => pending.metadata,
            Self::Retained(retained) => retained.metadata,
        }
    }
}

/// Frame-local accessibility membership projected by a higher-level surface runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityTreeScope {
    /// Preserve the nearest enclosing accessibility membership.
    Unrestricted,
    /// Include nodes as part of the active modal's root surface.
    ModalRoot,
    /// Include nodes as part of a registered descendant of the active modal.
    ModalDescendant,
    /// Exclude nodes from the delivered accessibility tree.
    Excluded,
}

/// A listener for an accessibility action on a specific node.
pub(crate) type A11yActionListener =
    Box<dyn FnMut(Option<&accesskit::ActionData>, &mut Window, &mut App) + 'static>;

#[derive(Default)]
struct PublishedA11yDispatch {
    revision: u64,
    activation_generation: u64,
    action_masks: FxHashMap<NodeId, u32>,
    focus_ids: FxHashMap<NodeId, FocusId>,
    node_geometry: FxHashMap<NodeId, (Bounds<Pixels>, Option<Point<Pixels>>)>,
    action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
}

macro_rules! define_published_action_masks {
    ($($variant:ident),+ $(,)?) => {
        pub(crate) const ACCESSKIT_ACTIONS: &[Action] = &[$(Action::$variant),+];

        pub(crate) fn action_mask(action: Action) -> u32 {
            match action {
                $(Action::$variant => 1 << (Action::$variant as u8),)+
            }
        }

        fn node_action_mask(node: &accesskit::Node) -> u32 {
            let mut mask = 0;
            $(
                if node.supports_action(Action::$variant) {
                    mask |= action_mask(Action::$variant);
                }
            )+
            mask
        }
    };
}

define_published_action_masks!(
    Click,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    HideTooltip,
    ShowTooltip,
    ReplaceSelectedText,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint,
    SetScrollOffset,
    SetTextSelection,
    SetSequentialFocusNavigationStartingPoint,
    SetValue,
    ShowContextMenu,
);

/// Per-window accessibility state.
///
/// Manages the AccessKit tree that is built each frame and the mappings
/// needed to dispatch incoming action requests back to the right elements.
pub(crate) struct A11y {
    window_id: WindowId,
    /// Whether accessibility has been [forcibly disabled] for this window.
    ///
    /// [forcibly disabled]: crate::Application::new_inaccessible
    force_disabled: bool,
    /// Active bit and generation requested by the platform accessibility adapter.
    ///
    /// Updated by AccessKit using callbacks provided to the adapter. Can change
    /// halfway through a frame.
    active_state: Arc<AtomicU64>,
    /// Whether a11y features are active for *this specific frame*.
    ///
    /// At the start of each frame, we load [`Self::active_state`] (using
    /// [`Self::sync_active_flag`]) and use this to determine whether we
    /// should construct a [`TreeUpdate`] for this frame. It's important that
    /// this value is stable within a frame, because the builder API exposed by
    /// this type maintains a stack of nodes and each must be pushed and popped
    /// exactly once.
    ///
    /// At the end of the frame, we re-call [`Self::sync_active_flag`] to
    /// determine whether we should actually send the finished [`TreeUpdate`].
    active_this_frame: bool,
    activation_generation_this_frame: u64,
    pub(crate) nodes: A11yNodeBuilder,
    candidate_focus_ids: Vec<(NodeId, FocusId, Option<SubtreeGeometryValidity>)>,
    candidate_node_geometry: Vec<(
        NodeId,
        Bounds<Pixels>,
        Option<Point<Pixels>>,
        Option<SubtreeGeometryValidity>,
    )>,
    candidate_action_listeners: Vec<(
        NodeId,
        Action,
        A11yActionListener,
        Option<SubtreeGeometryValidity>,
    )>,
    published: Option<PublishedA11yDispatch>,
    next_published_revision: u64,
    announcement_generation: Option<u64>,
    next_announcement_request_id: u64,
    next_announcement_sequence: u64,
    announcements: VecDeque<QueuedAnnouncement>,
    announcement_diagnostics: Vec<AccessibilityAnnouncementDiagnostic>,
    staged_announcement_nodes: Vec<NodeId>,
    announcement_followup_refresh_required: bool,
}

pub(crate) struct A11yPrepaintCheckpoint {
    nodes: A11yNodeBuilderCheckpoint,
    focus_ids_len: usize,
    node_bounds_len: usize,
    action_listeners_len: usize,
}

impl A11y {
    pub(crate) fn new(
        active_state: Arc<AtomicU64>,
        force_disabled: bool,
        window_id: WindowId,
    ) -> Self {
        Self {
            window_id,
            force_disabled,
            active_state,
            active_this_frame: false,
            activation_generation_this_frame: 0,
            nodes: A11yNodeBuilder::new(),
            candidate_focus_ids: Vec::new(),
            candidate_node_geometry: Vec::new(),
            candidate_action_listeners: Vec::new(),
            published: None,
            next_published_revision: 0,
            announcement_generation: None,
            next_announcement_request_id: 0,
            next_announcement_sequence: 0,
            announcements: VecDeque::with_capacity(ANNOUNCEMENT_QUEUE_CAPACITY),
            announcement_diagnostics: Vec::with_capacity(ANNOUNCEMENT_DIAGNOSTIC_CAPACITY),
            staged_announcement_nodes: Vec::with_capacity(ANNOUNCEMENT_QUEUE_CAPACITY),
            announcement_followup_refresh_required: false,
        }
    }

    /// Ensures that [`Self::is_active`] returns up to date information.
    ///
    /// See the docs for [`Self::active_state`] and [`Self::active_this_frame`]
    /// for more commentary.
    pub(crate) fn sync_active_flag(&mut self) {
        let (active, generation) = requested_state(&self.active_state);
        self.active_this_frame = !self.force_disabled && active;
        self.activation_generation_this_frame = generation;
        self.reconcile_announcement_generation(self.active_this_frame, generation);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active_this_frame
    }

    pub(crate) fn activation_generation(&self) -> u64 {
        self.activation_generation_this_frame
    }

    /// Clear per-frame state and push the root node to start a new frame.
    pub(crate) fn begin_frame(&mut self) {
        self.candidate_focus_ids.clear();
        self.candidate_node_geometry.clear();
        self.candidate_action_listeners.clear();
        self.staged_announcement_nodes.clear();
        self.nodes.begin_frame();
    }

    /// Finalize the tree and produce a [`TreeUpdate`] for the platform adapter.
    pub(crate) fn end_frame(&mut self) -> TreeUpdate {
        let mut update = self.nodes.finalize();
        self.stage_pending_announcements(&mut update);
        update
    }

    pub(crate) fn discard_candidate_frame(&mut self) {
        self.candidate_focus_ids.clear();
        self.candidate_node_geometry.clear();
        self.candidate_action_listeners.clear();
        self.staged_announcement_nodes.clear();
    }

    #[cfg(test)]
    pub(crate) fn published_revision_for_test(&self) -> Option<u64> {
        self.published.as_ref().map(|published| published.revision)
    }

    /// Replace action routing with the exact tree and frame state delivered to the platform.
    pub(crate) fn publish(&mut self, update: &TreeUpdate, activation_generation: u64) {
        let mut published = self.published.take().unwrap_or_default();
        published.action_masks.clear();
        for (id, node) in &update.nodes {
            let mask = node_action_mask(node);
            if mask != 0 {
                published.action_masks.insert(*id, mask);
            }
        }

        published.focus_ids.clear();
        published.focus_ids.reserve(self.candidate_focus_ids.len());
        for (id, focus_id, validity) in self.candidate_focus_ids.drain(..) {
            if validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
            {
                continue;
            }
            if published.action_masks.contains_key(&id) {
                published.focus_ids.insert(id, focus_id);
            }
        }

        published.node_geometry.clear();
        published
            .node_geometry
            .reserve(self.candidate_node_geometry.len());
        for (id, bounds, witness, validity) in self.candidate_node_geometry.drain(..) {
            if validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
            {
                continue;
            }
            if published.action_masks.contains_key(&id) {
                published.node_geometry.insert(id, (bounds, witness));
            }
        }

        published.action_listeners.clear();
        published
            .action_listeners
            .reserve(self.candidate_action_listeners.len());
        for (id, action, listener, validity) in self.candidate_action_listeners.drain(..) {
            if validity
                .as_ref()
                .is_some_and(|validity| !validity.is_valid())
            {
                continue;
            }
            if published
                .action_masks
                .get(&id)
                .is_some_and(|mask| mask & action_mask(action) != 0)
            {
                published
                    .action_listeners
                    .entry(id)
                    .or_default()
                    .push((action, listener));
            }
        }

        self.next_published_revision = self.next_published_revision.wrapping_add(1);
        published.revision = self.next_published_revision;
        published.activation_generation = activation_generation;
        self.published = Some(published);
        self.commit_announcements(activation_generation);
    }

    pub(crate) fn enqueue_announcement(
        &mut self,
        announcement: AccessibilityAnnouncement,
    ) -> AccessibilityAnnouncementOutcome {
        let request_id = self.next_request_id();
        let (requested_active, generation) = requested_state(&self.active_state);
        let active = !self.force_disabled && requested_active;
        self.reconcile_announcement_generation(active, generation);

        if !active {
            return self.drop_announcement(
                request_id,
                announcement.politeness,
                AccessibilityAnnouncementDropReason::AccessibilityInactive,
            );
        }
        if self.announcements.len() >= ANNOUNCEMENT_QUEUE_CAPACITY {
            return self.drop_announcement(
                request_id,
                announcement.politeness,
                AccessibilityAnnouncementDropReason::QueueFull,
            );
        }

        self.next_announcement_sequence = self.next_announcement_sequence.wrapping_add(1);
        let sequence = AccessibilityAnnouncementSequence(self.next_announcement_sequence);
        let metadata = AnnouncementMetadata {
            request_id,
            sequence,
            activation_generation: generation,
            node_id: announcement_node_id(self.window_id, sequence, 0),
            node_probe: 0,
            politeness: announcement.politeness,
        };
        self.announcements
            .push_back(QueuedAnnouncement::Pending(PendingAnnouncement {
                metadata,
                message: announcement.message,
            }));
        self.record_announcement_diagnostic(metadata, AccessibilityAnnouncementLifecycle::Accepted);
        self.announcement_followup_refresh_required = true;
        AccessibilityAnnouncementOutcome::Accepted {
            request_id,
            sequence,
        }
    }

    pub(crate) fn reject_announcement_for_closed_window(
        &mut self,
        announcement: AccessibilityAnnouncement,
    ) -> AccessibilityAnnouncementOutcome {
        let request_id = self.next_request_id();
        self.drop_announcement(
            request_id,
            announcement.politeness,
            AccessibilityAnnouncementDropReason::WindowClosed,
        )
    }

    pub(crate) fn reject_announcement_for_interaction_quiescence(
        &mut self,
        announcement: AccessibilityAnnouncement,
    ) -> AccessibilityAnnouncementOutcome {
        let request_id = self.next_request_id();
        self.drop_announcement(
            request_id,
            announcement.politeness,
            AccessibilityAnnouncementDropReason::InteractionQuiesced,
        )
    }

    pub(crate) fn clear_announcements_for_window_close(&mut self) {
        self.clear_announcements(AccessibilityAnnouncementClearReason::WindowClosed);
        self.announcement_generation = None;
    }

    pub(crate) fn clear_announcements_for_interaction_quiescence(&mut self) {
        self.clear_announcements(AccessibilityAnnouncementClearReason::InteractionQuiesced);
        self.announcement_generation = None;
    }

    pub(crate) fn announcement_diagnostics(&self) -> &[AccessibilityAnnouncementDiagnostic] {
        &self.announcement_diagnostics
    }

    pub(crate) fn take_announcement_followup_refresh_required(&mut self) -> bool {
        std::mem::take(&mut self.announcement_followup_refresh_required)
    }

    fn stage_pending_announcements(&mut self, update: &mut TreeUpdate) {
        if !self.active_this_frame
            || self.announcement_generation != Some(self.activation_generation_this_frame)
        {
            return;
        }

        let mut used_ids = update
            .nodes
            .iter()
            .map(|(node_id, _)| *node_id)
            .collect::<FxHashSet<_>>();
        let mut staged = Vec::with_capacity(self.announcements.len());
        for queued in &mut self.announcements {
            let QueuedAnnouncement::Pending(pending) = queued else {
                continue;
            };
            if pending.metadata.activation_generation != self.activation_generation_this_frame {
                continue;
            }

            while pending.metadata.node_id == ROOT_NODE_ID
                || used_ids.contains(&pending.metadata.node_id)
            {
                pending.metadata.node_probe = pending.metadata.node_probe.wrapping_add(1);
                pending.metadata.node_id = announcement_node_id(
                    self.window_id,
                    pending.metadata.sequence,
                    pending.metadata.node_probe,
                );
            }

            let node_id = pending.metadata.node_id;
            used_ids.insert(node_id);
            let text = pending.message.to_string();
            let mut node = accesskit::Node::new(pending.metadata.politeness.accesskit_role());
            node.set_label(text.clone());
            node.set_value(text);
            node.set_live(pending.metadata.politeness.accesskit_live());
            node.set_live_atomic();
            staged.push((node_id, node));
        }

        if staged.is_empty() {
            return;
        }
        let root = update
            .tree
            .as_ref()
            .map(|tree| tree.root)
            .unwrap_or(ROOT_NODE_ID);
        let Some((_, root_node)) = update
            .nodes
            .iter_mut()
            .find(|(node_id, _)| *node_id == root)
        else {
            return;
        };
        for (node_id, _) in &staged {
            root_node.push_child(*node_id);
            self.staged_announcement_nodes.push(*node_id);
        }
        update.nodes.extend(staged);
    }

    fn commit_announcements(&mut self, activation_generation: u64) {
        let staged = self
            .staged_announcement_nodes
            .drain(..)
            .collect::<FxHashSet<_>>();
        let mut remaining = VecDeque::with_capacity(ANNOUNCEMENT_QUEUE_CAPACITY);
        while let Some(queued) = self.announcements.pop_front() {
            match queued {
                QueuedAnnouncement::Pending(pending)
                    if pending.metadata.activation_generation == activation_generation
                        && staged.contains(&pending.metadata.node_id) =>
                {
                    self.record_announcement_diagnostic(
                        pending.metadata,
                        AccessibilityAnnouncementLifecycle::Committed,
                    );
                    remaining.push_back(QueuedAnnouncement::Retained(RetainedAnnouncement {
                        metadata: pending.metadata,
                    }));
                }
                QueuedAnnouncement::Pending(pending)
                    if pending.metadata.activation_generation == activation_generation =>
                {
                    remaining.push_back(QueuedAnnouncement::Pending(pending));
                }
                QueuedAnnouncement::Retained(retained)
                    if retained.metadata.activation_generation == activation_generation =>
                {
                    self.record_announcement_diagnostic(
                        retained.metadata,
                        AccessibilityAnnouncementLifecycle::Removed,
                    );
                }
                stale => {
                    self.record_announcement_diagnostic(
                        stale.metadata(),
                        AccessibilityAnnouncementLifecycle::Cleared(
                            AccessibilityAnnouncementClearReason::ActivationReplaced,
                        ),
                    );
                }
            }
        }
        self.announcements = remaining;
        self.announcement_followup_refresh_required = !self.announcements.is_empty();
    }

    fn reconcile_announcement_generation(&mut self, active: bool, generation: u64) {
        if !active {
            if self.announcement_generation.is_some() || !self.announcements.is_empty() {
                self.clear_announcements(
                    AccessibilityAnnouncementClearReason::AccessibilityDeactivated,
                );
            }
            self.announcement_generation = None;
            return;
        }

        if self
            .announcement_generation
            .is_some_and(|current| current != generation)
        {
            self.clear_announcements(AccessibilityAnnouncementClearReason::ActivationReplaced);
        }
        self.announcement_generation = Some(generation);
    }

    fn clear_announcements(&mut self, reason: AccessibilityAnnouncementClearReason) {
        let metadata = self
            .announcements
            .drain(..)
            .map(|queued| queued.metadata())
            .collect::<Vec<_>>();
        for metadata in metadata {
            self.record_announcement_diagnostic(
                metadata,
                AccessibilityAnnouncementLifecycle::Cleared(reason),
            );
        }
        self.staged_announcement_nodes.clear();
        self.announcement_followup_refresh_required = false;
    }

    fn drop_announcement(
        &mut self,
        request_id: AccessibilityAnnouncementRequestId,
        politeness: AccessibilityAnnouncementPoliteness,
        reason: AccessibilityAnnouncementDropReason,
    ) -> AccessibilityAnnouncementOutcome {
        self.push_announcement_diagnostic(AccessibilityAnnouncementDiagnostic {
            window_id: self.window_id,
            request_id,
            sequence: None,
            politeness,
            lifecycle: AccessibilityAnnouncementLifecycle::Dropped(reason),
        });
        AccessibilityAnnouncementOutcome::Dropped { request_id, reason }
    }

    fn next_request_id(&mut self) -> AccessibilityAnnouncementRequestId {
        self.next_announcement_request_id = self.next_announcement_request_id.wrapping_add(1);
        AccessibilityAnnouncementRequestId(self.next_announcement_request_id)
    }

    fn record_announcement_diagnostic(
        &mut self,
        metadata: AnnouncementMetadata,
        lifecycle: AccessibilityAnnouncementLifecycle,
    ) {
        self.push_announcement_diagnostic(AccessibilityAnnouncementDiagnostic {
            window_id: self.window_id,
            request_id: metadata.request_id,
            sequence: Some(metadata.sequence),
            politeness: metadata.politeness,
            lifecycle,
        });
    }

    fn push_announcement_diagnostic(&mut self, diagnostic: AccessibilityAnnouncementDiagnostic) {
        if self.announcement_diagnostics.len() == ANNOUNCEMENT_DIAGNOSTIC_CAPACITY {
            self.announcement_diagnostics.remove(0);
        }
        self.announcement_diagnostics.push(diagnostic);
    }

    pub(crate) fn record_focus_id(&mut self, node_id: NodeId, focus_id: FocusId) {
        self.candidate_focus_ids
            .push((node_id, focus_id, self.nodes.current_geometry_validity()));
    }

    pub(crate) fn resolve_focus(&mut self, focus: Option<FocusId>) {
        if !self.is_active() {
            return;
        }
        let Some(focus) = focus else {
            return;
        };

        let resolved = {
            let mut candidates =
                self.candidate_focus_ids
                    .iter()
                    .filter(|(node_id, focus_id, validity)| {
                        *focus_id == focus
                            && validity
                                .as_ref()
                                .is_none_or(SubtreeGeometryValidity::is_valid)
                            && self.nodes.has_node(*node_id)
                    });
            let first = candidates.next().cloned();
            if candidates.next().is_some() {
                None
            } else {
                first
            }
        };

        if let Some((node_id, _, validity)) = resolved {
            self.nodes.set_focus_with_validity(node_id, validity);
        }
    }

    pub(crate) fn record_node_bounds(
        &mut self,
        node_id: NodeId,
        bounds: Bounds<Pixels>,
        witness: Option<Point<Pixels>>,
    ) {
        self.candidate_node_geometry.push((
            node_id,
            bounds,
            witness,
            self.nodes.current_geometry_validity(),
        ));
    }

    pub(crate) fn record_action_listener(
        &mut self,
        node_id: NodeId,
        action: Action,
        listener: A11yActionListener,
    ) {
        self.candidate_action_listeners.push((
            node_id,
            action,
            listener,
            self.nodes.current_geometry_validity(),
        ));
    }

    pub(crate) fn accepts_action(
        &self,
        request_activation_generation: u64,
        tree_id: accesskit::TreeId,
        node_id: NodeId,
        action: Action,
    ) -> bool {
        let (active, activation_generation) = requested_state(&self.active_state);
        active
            && request_activation_generation == activation_generation
            && tree_id == accesskit::TreeId::ROOT
            && self.published.as_ref().is_some_and(|published| {
                published.activation_generation == activation_generation
                    && published
                        .action_masks
                        .get(&node_id)
                        .is_some_and(|mask| mask & action_mask(action) != 0)
            })
    }

    pub(crate) fn take_published_action_listeners(
        &mut self,
        node_id: NodeId,
    ) -> Option<(u64, Vec<(Action, A11yActionListener)>)> {
        let published = self.published.as_mut()?;
        let listeners = published.action_listeners.remove(&node_id)?;
        Some((published.revision, listeners))
    }

    pub(crate) fn restore_published_action_listeners(
        &mut self,
        revision: u64,
        node_id: NodeId,
        mut listeners: Vec<(Action, A11yActionListener)>,
    ) {
        let Some(published) = self.published.as_mut() else {
            return;
        };
        if published.revision != revision {
            return;
        }
        let Some(mask) = published.action_masks.get(&node_id) else {
            return;
        };
        listeners.retain(|(action, _)| mask & action_mask(*action) != 0);
        if !listeners.is_empty() {
            published.action_listeners.insert(node_id, listeners);
        }
    }

    pub(crate) fn published_node_witness(&self, node_id: NodeId) -> Option<Point<Pixels>> {
        self.published
            .as_ref()?
            .node_geometry
            .get(&node_id)
            .and_then(|(_, witness)| *witness)
    }

    pub(crate) fn published_focus_id(&self, node_id: NodeId) -> Option<FocusId> {
        self.published.as_ref()?.focus_ids.get(&node_id).copied()
    }

    pub(crate) fn prepaint_checkpoint(&self) -> A11yPrepaintCheckpoint {
        A11yPrepaintCheckpoint {
            nodes: self.nodes.checkpoint(),
            focus_ids_len: self.candidate_focus_ids.len(),
            node_bounds_len: self.candidate_node_geometry.len(),
            action_listeners_len: self.candidate_action_listeners.len(),
        }
    }

    pub(crate) fn current_tree_scope(&self) -> AccessibilityTreeScope {
        self.nodes.effective_scope()
    }

    #[cfg(test)]
    pub(crate) fn set_requested_active_for_test(&self, active: bool) {
        set_requested_active(&self.active_state, active);
    }

    pub(crate) fn rollback_prepaint(&mut self, checkpoint: A11yPrepaintCheckpoint) {
        self.candidate_action_listeners
            .truncate(checkpoint.action_listeners_len);
        self.candidate_node_geometry
            .truncate(checkpoint.node_bounds_len);
        self.candidate_focus_ids.truncate(checkpoint.focus_ids_len);
        self.nodes.rollback(checkpoint.nodes);
    }

    #[cfg(test)]
    pub(crate) fn has_candidate_focus_id(&self, node_id: NodeId) -> bool {
        self.candidate_focus_ids
            .iter()
            .any(|(candidate, _, _)| *candidate == node_id)
    }

    #[cfg(test)]
    pub(crate) fn has_candidate_node_bounds(&self, node_id: NodeId) -> bool {
        self.candidate_node_geometry
            .iter()
            .any(|(candidate, _, _, _)| *candidate == node_id)
    }

    #[cfg(test)]
    pub(crate) fn has_candidate_action_listener(&self, node_id: NodeId) -> bool {
        self.candidate_action_listeners
            .iter()
            .any(|(candidate, _, _, _)| *candidate == node_id)
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn set_requested_active(state: &AtomicU64, active: bool) {
    let _ = state.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
        let generation = (current >> 1).wrapping_add(1);
        Some((generation << 1) | u64::from(active))
    });
}

fn requested_state(state: &AtomicU64) -> (bool, u64) {
    let state = state.load(Ordering::SeqCst);
    (state & 1 == 1, state >> 1)
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn requested_generation(state: &AtomicU64) -> u64 {
    requested_state(state).1
}

fn announcement_node_id(
    window_id: WindowId,
    sequence: AccessibilityAnnouncementSequence,
    probe: u64,
) -> NodeId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "open-gpui-accessibility-announcement".hash(&mut hasher);
    window_id.hash(&mut hasher);
    sequence.hash(&mut hasher);
    probe.hash(&mut hasher);
    NodeId(hasher.finish())
}

pub(crate) struct A11yNodeBuilder {
    ids_stack: SmallVec<[NodeId; 16]>,
    nodes_stack: SmallVec<[accesskit::Node; 16]>,
    node_validity_stack: SmallVec<[Option<SubtreeGeometryValidity>; 16]>,
    /// This is the exact type required by accesskit, so we can't just make it a
    /// `HashMap<NodeId, Node>` to remove the need for `seen_ids`
    all_nodes: Vec<(NodeId, accesskit::Node, Option<SubtreeGeometryValidity>)>,
    seen_ids: FxHashSet<NodeId>,
    scope_stack: Rc<RefCell<SmallVec<[AccessibilityTreeScope; 8]>>>,
    geometry_validity_stack: Rc<RefCell<SmallVec<[Option<SubtreeGeometryValidity>; 8]>>>,
    clip_owner_scope_depths: Rc<RefCell<SmallVec<[usize; 8]>>>,
    deferred_parent_scopes: Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
    window_portal_scopes: Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
    parent_child_mutations: Vec<AccessibilityParentChildMutation>,
    deferred_child_orders: FxHashMap<NodeId, AccessibilityDeferredParent>,
    next_deferred_order: u64,
    memberships: Vec<(
        NodeId,
        AccessibilityTreeScope,
        Option<SubtreeGeometryValidity>,
    )>,
    modal_restrictions: Vec<Option<SubtreeGeometryValidity>>,
    focus: NodeId,
    focus_validity: Option<SubtreeGeometryValidity>,
    #[cfg(debug_assertions)]
    has_set_focus: bool,
}

struct A11yNodeBuilderCheckpoint {
    stack_depth: usize,
    top_id: Option<NodeId>,
    top_children_len: usize,
    all_nodes_len: usize,
    scope_depth: usize,
    geometry_validity_depth: usize,
    clip_owner_scope_depth: usize,
    deferred_parent_scope_depth: usize,
    window_portal_scope_depth: usize,
    parent_child_mutations_len: usize,
    memberships_len: usize,
    modal_restrictions_len: usize,
    focus: NodeId,
    focus_validity: Option<SubtreeGeometryValidity>,
    #[cfg(debug_assertions)]
    has_set_focus: bool,
}

pub(crate) struct AccessibilityTreeScopeGuard {
    stack: Rc<RefCell<SmallVec<[AccessibilityTreeScope; 8]>>>,
    depth: usize,
}

pub(crate) struct AccessibilityGeometryValidityGuard {
    stack: Rc<RefCell<SmallVec<[Option<SubtreeGeometryValidity>; 8]>>>,
    depth: usize,
}

pub(crate) struct AccessibilityClipOwnerScopeGuard {
    stack: Rc<RefCell<SmallVec<[usize; 8]>>>,
    depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccessibilityDeferredParent {
    parent_id: NodeId,
    normal_child_index: usize,
    order_path: SmallVec<[u64; 4]>,
    root_order: AccessibilityRootOrder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AccessibilityRootOrder {
    normal_child_index: usize,
    order_path: SmallVec<[u64; 4]>,
}

#[derive(Clone)]
struct AccessibilityDeferredParentScope {
    depth: usize,
    parent: AccessibilityDeferredParent,
    next_child_order: u64,
}

pub(crate) struct AccessibilityDeferredParentScopeGuard {
    stack: Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
    depth: usize,
}

pub(crate) struct AccessibilityWindowPortalScopeGuard {
    stack: Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
    depth: usize,
}

#[derive(Clone, Copy)]
struct AccessibilityParentChildMutation {
    parent_id: NodeId,
    child_id: NodeId,
    child_index: usize,
}

impl Drop for AccessibilityTreeScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        debug_assert_eq!(
            stack.len(),
            self.depth + 1,
            "accessibility tree scopes must be dropped in nesting order"
        );
        stack.truncate(self.depth);
    }
}

impl Drop for AccessibilityGeometryValidityGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        debug_assert_eq!(stack.len(), self.depth + 1);
        stack.truncate(self.depth);
    }
}

impl Drop for AccessibilityClipOwnerScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        debug_assert_eq!(
            stack.len(),
            self.depth + 1,
            "accessibility clip-owner scopes must be dropped in nesting order"
        );
        stack.truncate(self.depth);
    }
}

impl Drop for AccessibilityDeferredParentScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        debug_assert_eq!(
            stack.len(),
            self.depth + 1,
            "accessibility deferred-parent scopes must be dropped in nesting order"
        );
        stack.truncate(self.depth);
    }
}

impl Drop for AccessibilityWindowPortalScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        debug_assert_eq!(
            stack.len(),
            self.depth + 1,
            "accessibility window-portal scopes must be dropped in nesting order"
        );
        stack.truncate(self.depth);
    }
}

impl A11yNodeBuilder {
    fn new() -> Self {
        Self {
            ids_stack: SmallVec::new(),
            nodes_stack: SmallVec::new(),
            node_validity_stack: SmallVec::new(),
            all_nodes: Vec::new(),
            seen_ids: FxHashSet::default(),
            scope_stack: Rc::new(RefCell::new(SmallVec::new())),
            geometry_validity_stack: Rc::new(RefCell::new(SmallVec::new())),
            clip_owner_scope_depths: Rc::new(RefCell::new(SmallVec::new())),
            deferred_parent_scopes: Rc::new(RefCell::new(SmallVec::new())),
            window_portal_scopes: Rc::new(RefCell::new(SmallVec::new())),
            parent_child_mutations: Vec::new(),
            deferred_child_orders: FxHashMap::default(),
            next_deferred_order: 0,
            memberships: Vec::new(),
            modal_restrictions: Vec::new(),
            focus: ROOT_NODE_ID,
            focus_validity: None,
            #[cfg(debug_assertions)]
            has_set_focus: false,
        }
    }

    fn checkpoint(&self) -> A11yNodeBuilderCheckpoint {
        A11yNodeBuilderCheckpoint {
            stack_depth: self.ids_stack.len(),
            top_id: self.ids_stack.last().copied(),
            top_children_len: self
                .nodes_stack
                .last()
                .map_or(0, |node| node.children().len()),
            all_nodes_len: self.all_nodes.len(),
            scope_depth: self.scope_stack.borrow().len(),
            geometry_validity_depth: self.geometry_validity_stack.borrow().len(),
            clip_owner_scope_depth: self.clip_owner_scope_depths.borrow().len(),
            deferred_parent_scope_depth: self.deferred_parent_scopes.borrow().len(),
            window_portal_scope_depth: self.window_portal_scopes.borrow().len(),
            parent_child_mutations_len: self.parent_child_mutations.len(),
            memberships_len: self.memberships.len(),
            modal_restrictions_len: self.modal_restrictions.len(),
            focus: self.focus,
            focus_validity: self.focus_validity.clone(),
            #[cfg(debug_assertions)]
            has_set_focus: self.has_set_focus,
        }
    }

    fn rollback(&mut self, checkpoint: A11yNodeBuilderCheckpoint) {
        let stack_prefix_is_intact = self.ids_stack.len() >= checkpoint.stack_depth
            && self.nodes_stack.len() >= checkpoint.stack_depth
            && checkpoint
                .stack_depth
                .checked_sub(1)
                .is_none_or(|top_index| {
                    self.ids_stack.get(top_index).copied() == checkpoint.top_id
                });
        debug_assert!(
            stack_prefix_is_intact,
            "an accessibility transaction consumed a node that predates its checkpoint"
        );

        for (id, _, _) in &self.memberships[checkpoint.memberships_len..] {
            self.seen_ids.remove(id);
            self.deferred_child_orders.remove(id);
        }
        self.rollback_parent_child_mutations(checkpoint.parent_child_mutations_len);
        self.all_nodes.truncate(checkpoint.all_nodes_len);
        self.memberships.truncate(checkpoint.memberships_len);
        self.modal_restrictions
            .truncate(checkpoint.modal_restrictions_len);

        if stack_prefix_is_intact {
            self.ids_stack.truncate(checkpoint.stack_depth);
            self.nodes_stack.truncate(checkpoint.stack_depth);
            self.node_validity_stack.truncate(checkpoint.stack_depth);
            if let Some(top) = self.nodes_stack.last_mut() {
                debug_assert!(top.children().len() >= checkpoint.top_children_len);
                if top.children().len() > checkpoint.top_children_len {
                    if checkpoint.top_children_len == 0 {
                        top.clear_children();
                    } else {
                        top.set_children(top.children()[..checkpoint.top_children_len].to_vec());
                    }
                }
            }
        }

        let mut scope_stack = self.scope_stack.borrow_mut();
        debug_assert!(
            scope_stack.len() >= checkpoint.scope_depth,
            "an accessibility transaction consumed a scope that predates its checkpoint"
        );
        scope_stack.truncate(checkpoint.scope_depth);
        self.geometry_validity_stack
            .borrow_mut()
            .truncate(checkpoint.geometry_validity_depth);
        self.clip_owner_scope_depths
            .borrow_mut()
            .truncate(checkpoint.clip_owner_scope_depth);
        self.deferred_parent_scopes
            .borrow_mut()
            .truncate(checkpoint.deferred_parent_scope_depth);
        self.window_portal_scopes
            .borrow_mut()
            .truncate(checkpoint.window_portal_scope_depth);
        self.focus = checkpoint.focus;
        self.focus_validity = checkpoint.focus_validity;
        #[cfg(debug_assertions)]
        {
            self.has_set_focus = checkpoint.has_set_focus;
        }
    }

    /// Push a new node onto the stack. It becomes a child of the current
    /// top-of-stack node.
    ///
    /// Returns `true` if the node was successfully pushed.
    pub(crate) fn push(&mut self, id: NodeId, mut node: accesskit::Node) -> bool {
        debug_assert!(!self.ids_stack.is_empty(), "push called before push_root");

        let scope = self.effective_scope();
        if scope == AccessibilityTreeScope::Excluded {
            return false;
        }

        if !self.seen_ids.insert(id) {
            debug_assert!(
                false,
                "Duplicate a11y node id: {id:?}. In a release build, this node would be silently discarded from the a11y tree."
            );
            // We need to return `false` here because inserting a duplicate
            // node will cause a panic in accesskit
            return false;
        }

        if self.current_depth_has_clip_owner_scope() {
            node.set_clips_children();
        }

        let Some((parent_id, deferred_parent)) = self.take_accessibility_parent_for_child() else {
            self.seen_ids.remove(&id);
            log::error!("a11y: node {id:?} has no current parent");
            return false;
        };
        if !self.attach_child(parent_id, id, deferred_parent.as_ref()) {
            self.seen_ids.remove(&id);
            log::error!("a11y: node {id:?} could not resolve its recorded parent {parent_id:?}");
            return false;
        }
        if let Some(deferred_parent) = deferred_parent {
            self.deferred_child_orders.insert(id, deferred_parent);
        }
        self.ids_stack.push(id);
        self.nodes_stack.push(node);
        let validity = self.current_geometry_validity();
        self.node_validity_stack.push(validity.clone());
        self.memberships.push((id, scope, validity));
        true
    }

    /// Pop the current node off the stack and finalize it into the all_nodes
    /// list.
    pub(crate) fn pop(&mut self) {
        debug_assert!(self.ids_stack.len() > 1, "pop would remove the root node");

        if let (Some(id), Some(node), Some(validity)) = (
            self.ids_stack.pop(),
            self.nodes_stack.pop(),
            self.node_validity_stack.pop(),
        ) {
            self.all_nodes.push((id, node, validity));
        }
    }

    /// Push the root node to start a new frame.
    fn begin_frame(&mut self) {
        self.all_nodes.clear();
        self.ids_stack.clear();
        self.nodes_stack.clear();
        self.node_validity_stack.clear();
        self.seen_ids.clear();
        self.scope_stack.borrow_mut().clear();
        self.geometry_validity_stack.borrow_mut().clear();
        self.clip_owner_scope_depths.borrow_mut().clear();
        self.deferred_parent_scopes.borrow_mut().clear();
        self.window_portal_scopes.borrow_mut().clear();
        self.parent_child_mutations.clear();
        self.deferred_child_orders.clear();
        self.next_deferred_order = 0;
        self.memberships.clear();
        self.modal_restrictions.clear();
        #[cfg(debug_assertions)]
        {
            self.has_set_focus = false;
        }
        let root_node = accesskit::Node::new(accesskit::Role::Window);

        self.ids_stack.push(ROOT_NODE_ID);
        self.nodes_stack.push(root_node);
        self.node_validity_stack.push(None);
        self.focus = ROOT_NODE_ID;
        self.focus_validity = None;
    }

    pub(crate) fn enter_scope(
        &mut self,
        scope: AccessibilityTreeScope,
    ) -> AccessibilityTreeScopeGuard {
        if matches!(
            scope,
            AccessibilityTreeScope::ModalRoot | AccessibilityTreeScope::ModalDescendant
        ) {
            self.modal_restrictions
                .push(self.current_geometry_validity());
        }

        let depth = self.scope_stack.borrow().len();
        self.scope_stack.borrow_mut().push(scope);
        AccessibilityTreeScopeGuard {
            stack: self.scope_stack.clone(),
            depth,
        }
    }

    pub(crate) fn enter_geometry_validity(
        &mut self,
        validity: Option<SubtreeGeometryValidity>,
    ) -> AccessibilityGeometryValidityGuard {
        let depth = self.geometry_validity_stack.borrow().len();
        self.geometry_validity_stack.borrow_mut().push(validity);
        AccessibilityGeometryValidityGuard {
            stack: self.geometry_validity_stack.clone(),
            depth,
        }
    }

    pub(crate) fn enter_clip_owner_scope(&mut self) -> AccessibilityClipOwnerScopeGuard {
        let depth = self.clip_owner_scope_depths.borrow().len();
        self.clip_owner_scope_depths
            .borrow_mut()
            .push(self.ids_stack.len());
        AccessibilityClipOwnerScopeGuard {
            stack: self.clip_owner_scope_depths.clone(),
            depth,
        }
    }

    pub(crate) fn enter_deferred_parent_scope(
        &mut self,
        parent: AccessibilityDeferredParent,
    ) -> AccessibilityDeferredParentScopeGuard {
        let depth = self.deferred_parent_scopes.borrow().len();
        self.deferred_parent_scopes
            .borrow_mut()
            .push(AccessibilityDeferredParentScope {
                depth: self.ids_stack.len(),
                parent,
                next_child_order: 0,
            });
        AccessibilityDeferredParentScopeGuard {
            stack: self.deferred_parent_scopes.clone(),
            depth,
        }
    }

    pub(crate) fn enter_window_portal_scope(
        &mut self,
        parent: AccessibilityDeferredParent,
    ) -> AccessibilityWindowPortalScopeGuard {
        let depth = self.window_portal_scopes.borrow().len();
        self.window_portal_scopes
            .borrow_mut()
            .push(AccessibilityDeferredParentScope {
                depth: self.ids_stack.len(),
                parent,
                next_child_order: 0,
            });
        AccessibilityWindowPortalScopeGuard {
            stack: self.window_portal_scopes.clone(),
            depth,
        }
    }

    pub(crate) fn reserve_deferred_parent(&mut self) -> Option<AccessibilityDeferredParent> {
        if let Some(parent) = self.take_current_scoped_parent_order() {
            return Some(parent);
        }

        let parent_id = self.ids_stack.last().copied()?;
        let normal_child_index = self.normal_child_count(parent_id)?;
        let root_order = self.take_current_root_order();
        let deferred_order = self.take_deferred_order();
        let mut order_path = SmallVec::new();
        order_path.push(deferred_order);
        let root_order = if let Some(root_order) = root_order {
            root_order
        } else {
            self.root_order_for_current_source(deferred_order)?
        };
        Some(AccessibilityDeferredParent {
            parent_id,
            normal_child_index,
            order_path,
            root_order,
        })
    }

    pub(crate) fn reserve_window_portal_parent(&mut self) -> Option<AccessibilityDeferredParent> {
        let root_order = if let Some(root_order) = self.take_current_root_order() {
            root_order
        } else {
            let deferred_order = self.take_deferred_order();
            self.root_order_for_current_source(deferred_order)?
        };
        Some(AccessibilityDeferredParent {
            parent_id: ROOT_NODE_ID,
            normal_child_index: root_order.normal_child_index,
            order_path: root_order.order_path.clone(),
            root_order,
        })
    }

    pub(crate) fn current_depth_has_clip_owner_scope(&self) -> bool {
        !self.current_depth_is_window_portal_scope()
            && self
                .clip_owner_scope_depths
                .borrow()
                .iter()
                .any(|depth| *depth == self.ids_stack.len())
    }

    pub(crate) fn mark_current_node_clips_children(&mut self, id: NodeId) -> bool {
        if self.ids_stack.last().copied() != Some(id) {
            return false;
        }
        let Some(node) = self.nodes_stack.last_mut() else {
            return false;
        };
        node.set_clips_children();
        true
    }

    pub(crate) fn is_current_node(&self, id: NodeId) -> bool {
        self.ids_stack.last().copied() == Some(id)
    }

    pub(crate) fn current_geometry_validity(&self) -> Option<SubtreeGeometryValidity> {
        self.geometry_validity_stack
            .borrow()
            .last()
            .cloned()
            .flatten()
    }

    fn effective_scope(&self) -> AccessibilityTreeScope {
        self.scope_stack
            .borrow()
            .iter()
            .rev()
            .find(|scope| **scope != AccessibilityTreeScope::Unrestricted)
            .copied()
            .unwrap_or(AccessibilityTreeScope::Unrestricted)
    }

    /// Returns whether a node with the given ID has been pushed in this frame.
    pub(crate) fn has_node(&self, id: NodeId) -> bool {
        id == ROOT_NODE_ID || self.seen_ids.contains(&id)
    }

    fn current_depth_is_window_portal_scope(&self) -> bool {
        self.window_portal_scopes
            .borrow()
            .iter()
            .rev()
            .any(|scope| scope.depth == self.ids_stack.len())
    }

    fn take_accessibility_parent_for_child(
        &mut self,
    ) -> Option<(NodeId, Option<AccessibilityDeferredParent>)> {
        if let Some(parent) =
            Self::take_scoped_parent_order(&self.window_portal_scopes, self.ids_stack.len())
        {
            return Some((ROOT_NODE_ID, Some(parent)));
        }
        if let Some(parent) =
            Self::take_scoped_parent_order(&self.deferred_parent_scopes, self.ids_stack.len())
        {
            return Some((parent.parent_id, Some(parent)));
        }
        self.ids_stack.last().copied().map(|parent| (parent, None))
    }

    fn take_current_scoped_parent_order(&self) -> Option<AccessibilityDeferredParent> {
        Self::take_scoped_parent_order(&self.window_portal_scopes, self.ids_stack.len()).or_else(
            || Self::take_scoped_parent_order(&self.deferred_parent_scopes, self.ids_stack.len()),
        )
    }

    fn take_scoped_parent_order(
        scopes: &Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
        current_depth: usize,
    ) -> Option<AccessibilityDeferredParent> {
        let mut scopes = scopes.borrow_mut();
        let scope = scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.depth == current_depth)?;
        let mut parent = scope.parent.clone();
        let child_order = scope.next_child_order;
        parent.order_path.push(child_order);
        parent.root_order.order_path.push(child_order);
        scope.next_child_order = scope
            .next_child_order
            .checked_add(1)
            .expect("accessibility deferred child order overflowed");
        Some(parent)
    }

    fn take_current_root_order(&self) -> Option<AccessibilityRootOrder> {
        let current_depth = self.ids_stack.len();
        let window_portal_depth =
            Self::scoped_root_order_depth(&self.window_portal_scopes, current_depth);
        let deferred_parent_depth =
            Self::scoped_root_order_depth(&self.deferred_parent_scopes, current_depth);
        match (window_portal_depth, deferred_parent_depth) {
            (Some(_), None) => {
                Self::take_scoped_root_order(&self.window_portal_scopes, current_depth)
            }
            (Some(window_portal_depth), Some(deferred_parent_depth))
                if window_portal_depth >= deferred_parent_depth =>
            {
                Self::take_scoped_root_order(&self.window_portal_scopes, current_depth)
            }
            (None, Some(_)) | (Some(_), Some(_)) => {
                Self::take_scoped_root_order(&self.deferred_parent_scopes, current_depth)
            }
            (None, None) => None,
        }
    }

    fn scoped_root_order_depth(
        scopes: &Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
        current_depth: usize,
    ) -> Option<usize> {
        scopes
            .borrow()
            .iter()
            .rev()
            .find(|scope| scope.depth <= current_depth)
            .map(|scope| scope.depth)
    }

    fn take_scoped_root_order(
        scopes: &Rc<RefCell<SmallVec<[AccessibilityDeferredParentScope; 8]>>>,
        current_depth: usize,
    ) -> Option<AccessibilityRootOrder> {
        let mut scopes = scopes.borrow_mut();
        let scope = scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.depth <= current_depth)?;
        let mut root_order = scope.parent.root_order.clone();
        root_order.order_path.push(scope.next_child_order);
        scope.next_child_order = scope
            .next_child_order
            .checked_add(1)
            .expect("accessibility deferred child order overflowed");
        Some(root_order)
    }

    fn root_order_for_current_source(&self, deferred_order: u64) -> Option<AccessibilityRootOrder> {
        let mut order_path = SmallVec::new();
        order_path.push(deferred_order);
        Some(AccessibilityRootOrder {
            normal_child_index: self.normal_child_count(ROOT_NODE_ID)?,
            order_path,
        })
    }

    fn take_deferred_order(&mut self) -> u64 {
        let order = self.next_deferred_order;
        self.next_deferred_order = self
            .next_deferred_order
            .checked_add(1)
            .expect("accessibility deferred order overflowed");
        order
    }

    fn normal_child_count(&self, parent_id: NodeId) -> Option<usize> {
        let parent = if let Some(index) = self.ids_stack.iter().position(|id| *id == parent_id) {
            self.nodes_stack.get(index)?
        } else {
            self.all_nodes
                .iter()
                .find(|(id, _, _)| *id == parent_id)
                .map(|(_, node, _)| node)?
        };

        Some(
            parent
                .children()
                .iter()
                .filter(|child_id| {
                    self.deferred_child_orders
                        .get(child_id)
                        .is_none_or(|deferred| deferred.parent_id != parent_id)
                })
                .count(),
        )
    }

    fn attach_child(
        &mut self,
        parent_id: NodeId,
        child_id: NodeId,
        deferred_parent: Option<&AccessibilityDeferredParent>,
    ) -> bool {
        if let Some(parent_index) = self.ids_stack.iter().position(|id| *id == parent_id) {
            let insertion_index = self.child_insertion_index(
                &self.nodes_stack[parent_index],
                parent_id,
                deferred_parent,
            );
            let previous_children_len = self.nodes_stack[parent_index].children().len();
            if parent_index + 1 != self.nodes_stack.len()
                || insertion_index != previous_children_len
            {
                self.parent_child_mutations
                    .push(AccessibilityParentChildMutation {
                        parent_id,
                        child_id,
                        child_index: insertion_index,
                    });
            }
            Self::insert_child(
                &mut self.nodes_stack[parent_index],
                child_id,
                insertion_index,
            );
            return true;
        }

        if let Some(parent_index) = self
            .all_nodes
            .iter()
            .position(|(id, _, _)| *id == parent_id)
        {
            let insertion_index = self.child_insertion_index(
                &self.all_nodes[parent_index].1,
                parent_id,
                deferred_parent,
            );
            self.parent_child_mutations
                .push(AccessibilityParentChildMutation {
                    parent_id,
                    child_id,
                    child_index: insertion_index,
                });
            Self::insert_child(
                &mut self.all_nodes[parent_index].1,
                child_id,
                insertion_index,
            );
            return true;
        }

        false
    }

    fn child_insertion_index(
        &self,
        parent: &accesskit::Node,
        parent_id: NodeId,
        deferred_parent: Option<&AccessibilityDeferredParent>,
    ) -> usize {
        let Some(deferred_parent) = deferred_parent else {
            return parent.children().len();
        };

        let mut normal_child_index = 0;
        for (index, child_id) in parent.children().iter().enumerate() {
            let existing_deferred = self
                .deferred_child_orders
                .get(child_id)
                .filter(|existing| existing.parent_id == parent_id);
            if let Some(existing_deferred) = existing_deferred {
                if (
                    existing_deferred.normal_child_index,
                    &existing_deferred.order_path,
                ) > (
                    deferred_parent.normal_child_index,
                    &deferred_parent.order_path,
                ) {
                    return index;
                }
            } else {
                if normal_child_index >= deferred_parent.normal_child_index {
                    return index;
                }
                normal_child_index += 1;
            }
        }
        parent.children().len()
    }

    fn insert_child(parent: &mut accesskit::Node, child_id: NodeId, insertion_index: usize) {
        let mut children = parent.children().to_vec();
        children.insert(insertion_index.min(children.len()), child_id);
        parent.set_children(children);
    }

    fn rollback_parent_child_mutations(&mut self, checkpoint_len: usize) {
        while self.parent_child_mutations.len() > checkpoint_len {
            let mutation = self
                .parent_child_mutations
                .pop()
                .expect("mutation length was checked");
            let parent = self
                .ids_stack
                .iter()
                .position(|id| *id == mutation.parent_id)
                .and_then(|index| self.nodes_stack.get_mut(index))
                .or_else(|| {
                    self.all_nodes
                        .iter_mut()
                        .find(|(id, _, _)| *id == mutation.parent_id)
                        .map(|(_, node, _)| node)
                });
            let Some(parent) = parent else {
                debug_assert!(
                    false,
                    "accessibility rollback lost parent {:?}",
                    mutation.parent_id
                );
                continue;
            };
            let mut children = parent.children().to_vec();
            if children.get(mutation.child_index) == Some(&mutation.child_id) {
                children.remove(mutation.child_index);
            } else if let Some(index) = children.iter().position(|id| *id == mutation.child_id) {
                debug_assert_eq!(
                    index, mutation.child_index,
                    "accessibility child moved before rollback"
                );
                children.remove(index);
            } else {
                debug_assert!(
                    false,
                    "accessibility rollback lost child {:?} from parent {:?}",
                    mutation.child_id, mutation.parent_id
                );
                continue;
            }
            if children.is_empty() {
                parent.clear_children();
            } else {
                parent.set_children(children);
            }
        }
    }

    /// Set the focused node for this frame.
    #[cfg(test)]
    pub(crate) fn set_focus(&mut self, id: NodeId) {
        self.set_focus_with_validity(id, self.current_geometry_validity());
    }

    fn set_focus_with_validity(&mut self, id: NodeId, validity: Option<SubtreeGeometryValidity>) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                !self.has_set_focus,
                "set_focus called more than once in a single frame"
            );
            self.has_set_focus = true;
        }
        self.focus = id;
        self.focus_validity = validity;
    }

    fn finalize(&mut self) -> TreeUpdate {
        // Stack should contain only the root node
        debug_assert_eq!(self.ids_stack.len(), 1);
        debug_assert_eq!(self.ids_stack[0], ROOT_NODE_ID);
        debug_assert!(
            self.scope_stack.borrow().is_empty(),
            "accessibility tree scope stack must be empty at frame end"
        );
        debug_assert!(
            self.clip_owner_scope_depths.borrow().is_empty(),
            "accessibility clip-owner scope stack must be empty at frame end"
        );
        debug_assert!(
            self.deferred_parent_scopes.borrow().is_empty(),
            "accessibility deferred-parent scope stack must be empty at frame end"
        );
        debug_assert!(
            self.window_portal_scopes.borrow().is_empty(),
            "accessibility window-portal scope stack must be empty at frame end"
        );
        if self.ids_stack.len() != 1 {
            log::error!(
                "a11y: Stack imbalance at end of frame: expected 1 (root), got {}. \
                 Some elements may have pushed without popping.",
                self.ids_stack.len()
            );
        }

        // Pop remaining nodes (should just be the root).
        while !self.ids_stack.is_empty() {
            if let (Some(id), Some(node), Some(validity)) = (
                self.ids_stack.pop(),
                self.nodes_stack.pop(),
                self.node_validity_stack.pop(),
            ) {
                self.all_nodes.push((id, node, validity));
            }
        }

        let nodes = std::mem::take(&mut self.all_nodes)
            .into_iter()
            .filter_map(|(id, node, validity)| {
                validity
                    .as_ref()
                    .is_none_or(SubtreeGeometryValidity::is_valid)
                    .then_some((id, node))
            })
            .collect();
        let update = TreeUpdate {
            nodes,
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: if self
                .focus_validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
            {
                self.focus
            } else {
                ROOT_NODE_ID
            },
        };

        let memberships = self
            .memberships
            .iter()
            .filter_map(|(id, scope, validity)| {
                validity
                    .as_ref()
                    .is_none_or(SubtreeGeometryValidity::is_valid)
                    .then_some((*id, *scope))
            })
            .collect::<Vec<_>>();
        let modal_restricted = self.modal_restrictions.iter().any(|validity| {
            validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
        });
        let update = Self::filter_published_tree(update, &memberships, modal_restricted);
        self.memberships.clear();
        self.modal_restrictions.clear();
        self.parent_child_mutations.clear();
        self.deferred_child_orders.clear();
        self.next_deferred_order = 0;
        Self::repair_tree_update(update)
    }

    fn filter_published_tree(
        mut update: TreeUpdate,
        memberships: &[(NodeId, AccessibilityTreeScope)],
        modal_restricted: bool,
    ) -> TreeUpdate {
        let root = update
            .tree
            .as_ref()
            .map(|tree| tree.root)
            .unwrap_or(ROOT_NODE_ID);
        let children: FxHashMap<NodeId, Vec<NodeId>> = update
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.children().to_vec()))
            .collect();
        let parents: FxHashMap<NodeId, NodeId> = children
            .iter()
            .flat_map(|(parent, children)| children.iter().map(move |child| (*child, *parent)))
            .collect();
        let mut source_order = FxHashMap::default();
        let mut pending = children
            .get(&root)
            .into_iter()
            .flat_map(|children| children.iter().rev().copied())
            .collect::<Vec<_>>();
        while let Some(id) = pending.pop() {
            if source_order.contains_key(&id) {
                continue;
            }
            source_order.insert(id, source_order.len());
            if let Some(children) = children.get(&id) {
                pending.extend(children.iter().rev().copied());
            }
        }
        let membership_by_id: FxHashMap<NodeId, AccessibilityTreeScope> =
            memberships.iter().copied().collect();
        let mut hidden = FxHashSet::default();
        for hidden_root in update
            .nodes
            .iter()
            .filter_map(|(id, node)| node.is_hidden().then_some(*id))
        {
            let mut stack = vec![hidden_root];
            while let Some(id) = stack.pop() {
                if !hidden.insert(id) {
                    continue;
                }
                if let Some(children) = children.get(&id) {
                    stack.extend(children.iter().copied());
                }
            }
        }

        if !modal_restricted && hidden.is_empty() {
            return update;
        }

        let retained: FxHashSet<NodeId> = update
            .nodes
            .iter()
            .filter_map(|(id, _)| {
                if *id == root {
                    return Some(*id);
                }
                if hidden.contains(id) {
                    return None;
                }

                let membership = membership_by_id
                    .get(id)
                    .copied()
                    .unwrap_or(AccessibilityTreeScope::Unrestricted);
                (!modal_restricted
                    || matches!(
                        membership,
                        AccessibilityTreeScope::ModalRoot | AccessibilityTreeScope::ModalDescendant
                    ))
                .then_some(*id)
            })
            .collect();

        let mut root_children = memberships
            .iter()
            .enumerate()
            .filter_map(|(fallback_order, (id, _))| {
                if !retained.contains(id) {
                    return None;
                }
                let parent = parents.get(id).copied();
                (parent == Some(root)
                    || parent.is_none()
                    || parent.is_some_and(|parent| !retained.contains(&parent)))
                .then_some((
                    source_order.get(id).copied().unwrap_or(usize::MAX),
                    fallback_order,
                    *id,
                ))
            })
            .collect::<Vec<_>>();
        root_children
            .sort_by_key(|(source_order, fallback_order, _)| (*source_order, *fallback_order));
        let root_children = root_children
            .into_iter()
            .map(|(_, _, id)| id)
            .collect::<Vec<_>>();

        update.nodes.retain(|(id, _)| retained.contains(id));
        for (id, node) in &mut update.nodes {
            if *id == root {
                node.set_children(root_children.clone());
            } else {
                node.set_children(
                    node.children()
                        .iter()
                        .copied()
                        .filter(|child| retained.contains(child))
                        .collect::<Vec<_>>(),
                );
            }
        }

        if !retained.contains(&update.focus) {
            update.focus = memberships
                .iter()
                .find_map(|(id, membership)| {
                    (*membership == AccessibilityTreeScope::ModalRoot && retained.contains(id))
                        .then_some(*id)
                })
                .or_else(|| {
                    memberships.iter().find_map(|(id, membership)| {
                        (*membership == AccessibilityTreeScope::ModalDescendant
                            && retained.contains(id))
                        .then_some(*id)
                    })
                })
                .unwrap_or(root);
        }
        update
    }

    /// Accesskit panics on invalid [`TreeUpdate`]s. This function defensively
    /// checks invariants that accesskit panics on, and tries to fix them.
    fn repair_tree_update(mut update: TreeUpdate) -> TreeUpdate {
        let node_ids: FxHashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
        let mut text_runs = FxHashMap::default();
        let mut has_text_selection = false;
        for (id, node) in &mut update.nodes {
            has_text_selection |= node.text_selection().is_some();
            if node.role() != accesskit::Role::TextRun {
                continue;
            }
            if valid_text_run(node) {
                text_runs.insert(*id, node.character_lengths().len());
            } else {
                log::error!(
                    "a11y: TextRun {:?} has invalid value/character lengths. Stripping text-run \
                     indexing metadata.",
                    id
                );
                node.clear_character_lengths();
                node.clear_character_positions();
                node.clear_character_widths();
                node.clear_word_starts();
            }
        }
        let parents = has_text_selection.then(|| {
            update
                .nodes
                .iter()
                .flat_map(|(parent, node)| node.children().iter().map(|child| (*child, *parent)))
                .collect::<FxHashMap<NodeId, NodeId>>()
        });

        // Focus must point to a node in the tree.
        if !node_ids.contains(&update.focus) {
            log::error!(
                "a11y: Focused node {:?} is not in the tree ({} nodes). \
                 Falling back to root. This is a bug in the a11y tree builder.",
                update.focus,
                update.nodes.len()
            );
            update.focus = ROOT_NODE_ID;
        }

        macro_rules! repair_node_id_slice {
            ($node:ident, $id:ident, $getter:ident, $setter:ident) => {
                if let Some(valid) =
                    filter_node_id_slice($id, stringify!($getter), $node.$getter(), &node_ids)
                {
                    $node.$setter(valid);
                }
            };
        }

        macro_rules! repair_node_id {
            ($node:ident, $id:ident, $getter:ident, $clearer:ident) => {
                if let Some(reference) = $node.$getter()
                    && !node_ids.contains(&reference)
                {
                    log_invalid_node_id_reference($id, stringify!($getter), reference);
                    $node.$clearer();
                }
            };
        }

        for (id, node) in &mut update.nodes {
            repair_node_id_slice!(node, id, children, set_children);
            repair_node_id_slice!(node, id, controls, set_controls);
            repair_node_id_slice!(node, id, details, set_details);
            repair_node_id_slice!(node, id, described_by, set_described_by);
            repair_node_id_slice!(node, id, flow_to, set_flow_to);
            repair_node_id_slice!(node, id, labelled_by, set_labelled_by);
            repair_node_id_slice!(node, id, owns, set_owns);
            repair_node_id_slice!(node, id, radio_group, set_radio_group);

            repair_node_id!(node, id, active_descendant, clear_active_descendant);
            repair_node_id!(node, id, error_message, clear_error_message);
            repair_node_id!(node, id, in_page_link_target, clear_in_page_link_target);
            repair_node_id!(node, id, member_of, clear_member_of);
            repair_node_id!(node, id, next_on_line, clear_next_on_line);
            repair_node_id!(node, id, previous_on_line, clear_previous_on_line);
            repair_node_id!(node, id, popup_for, clear_popup_for);

            if let Some(selection) = node.text_selection().copied() {
                let valid = parents.as_ref().is_some_and(|parents| {
                    valid_text_position(selection.anchor, *id, &text_runs, parents)
                        && valid_text_position(selection.focus, *id, &text_runs, parents)
                });
                if !valid {
                    log::error!(
                        "a11y: Node {:?} has a text selection outside a valid text run. \
                         Stripping invalid selection.",
                        id
                    );
                    node.clear_text_selection();
                }
            }
        }

        update
    }
}

fn valid_text_run(node: &accesskit::Node) -> bool {
    let Some(value) = node.value() else {
        return false;
    };
    let lengths = node.character_lengths();
    let mut offset = 0usize;
    for length in lengths {
        let Some(next) = offset.checked_add(*length as usize) else {
            return false;
        };
        if next <= offset || next > value.len() || !value.is_char_boundary(next) {
            return false;
        }
        offset = next;
    }
    offset == value.len()
}

fn valid_text_position(
    position: accesskit::TextPosition,
    selection_owner: NodeId,
    text_runs: &FxHashMap<NodeId, usize>,
    parents: &FxHashMap<NodeId, NodeId>,
) -> bool {
    text_runs
        .get(&position.node)
        .is_some_and(|character_count| position.character_index <= *character_count)
        && node_is_descendant_of(position.node, selection_owner, parents)
}

fn node_is_descendant_of(
    mut node: NodeId,
    ancestor: NodeId,
    parents: &FxHashMap<NodeId, NodeId>,
) -> bool {
    for _ in 0..=parents.len() {
        let Some(parent) = parents.get(&node).copied() else {
            return false;
        };
        if parent == ancestor {
            return true;
        }
        node = parent;
    }
    false
}

fn log_invalid_node_id_reference(node_id: &NodeId, property: &'static str, reference: NodeId) {
    log::error!(
        "a11y: Node {:?} references {} node {:?} not present in the tree. \
         Stripping invalid reference.",
        node_id,
        property,
        reference
    );
}

fn filter_node_id_slice(
    node_id: &NodeId,
    property: &'static str,
    references: &[NodeId],
    node_ids: &FxHashSet<NodeId>,
) -> Option<Vec<NodeId>> {
    if references
        .iter()
        .all(|reference| node_ids.contains(reference))
    {
        return None;
    }

    let invalid_count = references
        .iter()
        .filter(|reference| !node_ids.contains(reference))
        .count();
    log::error!(
        "a11y: Node {:?} references {} {} nodes not present in the tree. \
         Stripping invalid references.",
        node_id,
        invalid_count,
        property
    );
    Some(
        references
            .iter()
            .copied()
            .filter(|reference| node_ids.contains(reference))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_action_mask_mapping_covers_the_action_universe() {
        for (bit_index, action) in ACCESSKIT_ACTIONS.iter().copied().enumerate() {
            assert!(bit_index < u32::BITS as usize);
            assert_eq!(
                action_mask(action),
                1_u32 << bit_index,
                "{action:?} must have exactly one stable action-mask bit"
            );
        }
    }

    #[test]
    fn transient_announcement_queue_is_bounded_and_commits_then_removes() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(7));
        a11y.sync_active_flag();

        for index in 0..ANNOUNCEMENT_QUEUE_CAPACITY {
            let outcome = a11y.enqueue_announcement(AccessibilityAnnouncement::polite(format!(
                "Announcement {index}"
            )));
            assert_eq!(
                outcome.sequence().map(|sequence| sequence.as_u64()),
                Some(index as u64 + 1)
            );
        }
        let overflow = a11y.enqueue_announcement(AccessibilityAnnouncement::assertive(
            "Rejected queue payload",
        ));
        assert_eq!(
            overflow.drop_reason(),
            Some(AccessibilityAnnouncementDropReason::QueueFull)
        );
        assert_eq!(overflow.sequence(), None);
        assert_eq!(a11y.announcements.len(), ANNOUNCEMENT_QUEUE_CAPACITY);

        a11y.begin_frame();
        let committed = a11y.end_frame();
        let generation = a11y.activation_generation();
        let live_nodes = committed
            .nodes
            .iter()
            .filter(|(_, node)| node.live().is_some())
            .collect::<Vec<_>>();
        assert_eq!(live_nodes.len(), ANNOUNCEMENT_QUEUE_CAPACITY);
        assert_eq!(live_nodes[0].1.label(), Some("Announcement 0"));
        assert_eq!(live_nodes[0].1.value(), Some("Announcement 0"));
        assert_eq!(live_nodes[0].1.live(), Some(accesskit::Live::Polite));
        assert!(live_nodes[0].1.is_live_atomic());
        let root = committed
            .nodes
            .iter()
            .find(|(node_id, _)| *node_id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .expect("the final accessibility tree must contain its root node");
        let ordered_live_labels = root
            .children()
            .iter()
            .filter_map(|child_id| {
                committed
                    .nodes
                    .iter()
                    .find(|(node_id, _)| node_id == child_id)
                    .map(|(_, node)| node)
            })
            .filter(|node| node.live().is_some())
            .map(|node| {
                node.label()
                    .expect("announcement nodes must expose a label")
            })
            .collect::<Vec<_>>();
        let expected_labels = (0..ANNOUNCEMENT_QUEUE_CAPACITY)
            .map(|index| format!("Announcement {index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            ordered_live_labels,
            expected_labels
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        assert!(live_nodes.iter().all(|(_, node)| {
            ACCESSKIT_ACTIONS
                .iter()
                .all(|action| !node.supports_action(*action))
        }));
        a11y.publish(&committed, generation);
        assert!(
            a11y.announcements
                .iter()
                .all(|entry| matches!(entry, QueuedAnnouncement::Retained(_)))
        );

        let still_full = a11y.enqueue_announcement(AccessibilityAnnouncement::polite(
            "Retained entries still own capacity",
        ));
        assert_eq!(
            still_full.drop_reason(),
            Some(AccessibilityAnnouncementDropReason::QueueFull)
        );

        a11y.begin_frame();
        let removed = a11y.end_frame();
        assert!(removed.nodes.iter().all(|(_, node)| node.live().is_none()));
        a11y.publish(&removed, generation);
        assert!(a11y.announcements.is_empty());
        assert!(!a11y.take_announcement_followup_refresh_required());
    }

    #[test]
    fn repeated_announcement_text_gets_distinct_sequence_and_node_identity() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(8));
        a11y.sync_active_flag();

        let first = a11y.enqueue_announcement(AccessibilityAnnouncement::polite("Repeated"));
        let second = a11y.enqueue_announcement(AccessibilityAnnouncement::polite("Repeated"));
        assert_ne!(first.sequence(), second.sequence());

        a11y.begin_frame();
        let update = a11y.end_frame();
        let nodes = update
            .nodes
            .iter()
            .filter(|(_, node)| node.label() == Some("Repeated"))
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 2);
        assert_ne!(nodes[0], nodes[1]);
    }

    #[test]
    fn deactivation_clears_pending_and_retained_announcements_without_replay() {
        const RETAINED: &str = "Retained announcement must not replay";
        const PENDING: &str = "Pending announcement must not replay";

        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state.clone(), false, WindowId::from(10));
        a11y.sync_active_flag();

        let retained = a11y.enqueue_announcement(AccessibilityAnnouncement::polite(RETAINED));
        a11y.begin_frame();
        let committed = a11y.end_frame();
        let generation = a11y.activation_generation();
        a11y.publish(&committed, generation);
        assert!(matches!(
            a11y.announcements.front(),
            Some(QueuedAnnouncement::Retained(_))
        ));

        let pending = a11y.enqueue_announcement(AccessibilityAnnouncement::assertive(PENDING));
        assert!(matches!(
            a11y.announcements.back(),
            Some(QueuedAnnouncement::Pending(_))
        ));

        set_requested_active(&active_state, false);
        a11y.sync_active_flag();
        assert!(a11y.announcements.is_empty());
        assert!(!a11y.take_announcement_followup_refresh_required());

        for outcome in [retained, pending] {
            assert!(a11y.announcement_diagnostics().iter().any(|diagnostic| {
                diagnostic.request_id() == outcome.request_id()
                    && diagnostic.sequence() == outcome.sequence()
                    && diagnostic.lifecycle()
                        == AccessibilityAnnouncementLifecycle::Cleared(
                            AccessibilityAnnouncementClearReason::AccessibilityDeactivated,
                        )
            }));
        }
        assert!(!format!("{:?}", a11y.announcement_diagnostics()).contains(RETAINED));
        assert!(!format!("{:?}", a11y.announcement_diagnostics()).contains(PENDING));

        set_requested_active(&active_state, true);
        a11y.sync_active_flag();
        a11y.begin_frame();
        let reactivated = a11y.end_frame();
        assert!(
            reactivated.nodes.iter().all(|(_, node)| {
                node.label() != Some(RETAINED) && node.label() != Some(PENDING)
            })
        );
    }

    #[test]
    fn inactive_and_replacement_generation_requests_never_replay() {
        const PRIVACY_CANARY: &str = "u14-announcement-private-canary";

        let active_state = Arc::new(AtomicU64::new(0));
        let mut a11y = A11y::new(active_state.clone(), false, WindowId::from(9));
        let inactive = a11y.enqueue_announcement(AccessibilityAnnouncement::polite(PRIVACY_CANARY));
        assert_eq!(
            inactive.drop_reason(),
            Some(AccessibilityAnnouncementDropReason::AccessibilityInactive)
        );

        set_requested_active(&active_state, true);
        let accepted =
            a11y.enqueue_announcement(AccessibilityAnnouncement::assertive(PRIVACY_CANARY));
        assert!(accepted.is_accepted());
        set_requested_active(&active_state, true);
        a11y.sync_active_flag();
        assert!(a11y.announcements.is_empty());

        a11y.begin_frame();
        let update = a11y.end_frame();
        assert!(
            update
                .nodes
                .iter()
                .all(|(_, node)| node.label() != Some(PRIVACY_CANARY))
        );
        assert!(!format!("{:?}", a11y.announcement_diagnostics()).contains(PRIVACY_CANARY));
        assert!(a11y.announcement_diagnostics().iter().any(|diagnostic| {
            diagnostic.lifecycle()
                == AccessibilityAnnouncementLifecycle::Cleared(
                    AccessibilityAnnouncementClearReason::ActivationReplaced,
                )
        }));
    }

    #[test]
    fn published_action_authority_changes_only_after_matching_activation_delivery() {
        let active_state = Arc::new(AtomicU64::new(0));
        let mut a11y = A11y::new(active_state.clone(), false, WindowId::from(1));
        let node_id = NodeId(1);

        let prepare_update = |a11y: &mut A11y| {
            a11y.sync_active_flag();
            let generation = a11y.activation_generation();
            a11y.begin_frame();
            let mut node = accesskit::Node::new(accesskit::Role::Button);
            node.add_action(Action::Click);
            assert!(a11y.nodes.push(node_id, node));
            a11y.nodes.pop();
            (a11y.end_frame(), generation)
        };

        set_requested_active(&active_state, true);
        let (initial, initial_generation) = prepare_update(&mut a11y);
        assert!(!a11y.accepts_action(
            initial_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
        a11y.publish(&initial, initial_generation);
        assert!(a11y.accepts_action(
            initial_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));

        set_requested_active(&active_state, false);
        assert!(!a11y.accepts_action(
            initial_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
        set_requested_active(&active_state, true);
        assert!(!a11y.accepts_action(
            initial_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));

        let (reactivated, reactivated_generation) = prepare_update(&mut a11y);
        assert!(!a11y.accepts_action(
            initial_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
        a11y.publish(&reactivated, reactivated_generation);
        assert!(!a11y.accepts_action(
            initial_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
        assert!(a11y.accepts_action(
            reactivated_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));

        set_requested_active(&active_state, true);
        assert!(!a11y.accepts_action(
            reactivated_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
        let (repeated, repeated_generation) = prepare_update(&mut a11y);
        assert!(!a11y.accepts_action(
            reactivated_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
        a11y.publish(&repeated, repeated_generation);
        assert!(a11y.accepts_action(
            repeated_generation,
            accesskit::TreeId::ROOT,
            node_id,
            Action::Click
        ));
    }

    #[test]
    fn stale_listener_restore_cannot_overwrite_a_new_publication() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(1));
        let node_id = NodeId(1);

        let begin_candidate = |a11y: &mut A11y| {
            a11y.sync_active_flag();
            a11y.begin_frame();
            let mut node = accesskit::Node::new(accesskit::Role::Button);
            node.add_action(Action::Click);
            assert!(a11y.nodes.push(node_id, node));
            a11y.nodes.pop();
        };
        let listener = || -> A11yActionListener { Box::new(|_, _, _| {}) };

        begin_candidate(&mut a11y);
        a11y.record_action_listener(node_id, Action::Click, listener());
        a11y.record_action_listener(node_id, Action::Click, listener());
        let first = a11y.end_frame();
        let generation = a11y.activation_generation();
        a11y.publish(&first, generation);
        let (first_revision, first_listeners) = a11y
            .take_published_action_listeners(node_id)
            .expect("first publication should expose its listeners");
        assert_eq!(first_listeners.len(), 2);

        begin_candidate(&mut a11y);
        a11y.record_action_listener(node_id, Action::Click, listener());
        let second = a11y.end_frame();
        a11y.publish(&second, generation);
        a11y.restore_published_action_listeners(first_revision, node_id, first_listeners);

        let (_, current_listeners) = a11y
            .take_published_action_listeners(node_id)
            .expect("new publication should retain its own listeners");
        assert_eq!(current_listeners.len(), 1);
    }

    #[test]
    fn stable_publication_reuses_dispatch_and_membership_capacity() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(1));
        let focus_id = FocusId::default();

        let publish_frame = |a11y: &mut A11y| {
            a11y.sync_active_flag();
            a11y.begin_frame();
            for raw_id in 1..=64 {
                let node_id = NodeId(raw_id);
                let mut node = accesskit::Node::new(accesskit::Role::Button);
                node.add_action(Action::Click);
                assert!(a11y.nodes.push(node_id, node));
                a11y.record_focus_id(node_id, focus_id);
                a11y.record_node_bounds(node_id, Bounds::default(), Some(Point::default()));
                a11y.record_action_listener(node_id, Action::Click, Box::new(|_, _, _| {}));
                a11y.nodes.pop();
            }
            let update = a11y.end_frame();
            let generation = a11y.activation_generation();
            a11y.publish(&update, generation);
        };

        publish_frame(&mut a11y);
        let first_membership_capacity = a11y.nodes.memberships.capacity();
        let first_dispatch_capacities = {
            let published = a11y.published.as_ref().unwrap();
            (
                published.action_masks.capacity(),
                published.focus_ids.capacity(),
                published.node_geometry.capacity(),
                published.action_listeners.capacity(),
            )
        };

        publish_frame(&mut a11y);
        let published = a11y.published.as_ref().unwrap();
        assert_eq!(a11y.nodes.memberships.capacity(), first_membership_capacity);
        assert_eq!(
            (
                published.action_masks.capacity(),
                published.focus_ids.capacity(),
                published.node_geometry.capacity(),
                published.action_listeners.capacity(),
            ),
            first_dispatch_capacities
        );
    }

    #[test]
    fn final_focus_resolution_uses_the_unique_committed_candidate() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(1));
        let previous_node = NodeId(1);
        let claimed_node = NodeId(2);
        let mut focus_ids = slotmap::SlotMap::<FocusId, ()>::with_key();
        let previous_focus = focus_ids.insert(());
        let claimed_focus = focus_ids.insert(());

        a11y.sync_active_flag();
        a11y.begin_frame();
        for (node_id, focus_id) in [
            (previous_node, previous_focus),
            (claimed_node, claimed_focus),
        ] {
            let node = accesskit::Node::new(accesskit::Role::Button);
            assert!(a11y.nodes.push(node_id, node));
            a11y.record_focus_id(node_id, focus_id);
            a11y.nodes.pop();
        }

        a11y.resolve_focus(Some(claimed_focus));
        assert_eq!(a11y.end_frame().focus, claimed_node);
    }

    #[test]
    fn final_focus_resolution_ignores_rolled_back_candidates() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(1));
        let rolled_back_node = NodeId(1);
        let focus = FocusId::default();

        a11y.sync_active_flag();
        a11y.begin_frame();
        let checkpoint = a11y.prepaint_checkpoint();
        let node = accesskit::Node::new(accesskit::Role::Button);
        assert!(a11y.nodes.push(rolled_back_node, node));
        a11y.record_focus_id(rolled_back_node, focus);
        a11y.nodes.pop();
        a11y.rollback_prepaint(checkpoint);

        a11y.resolve_focus(Some(focus));
        assert_eq!(a11y.end_frame().focus, ROOT_NODE_ID);
    }

    #[test]
    fn nested_prepaint_checkpoints_restore_only_their_candidate_suffix() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false, WindowId::from(1));
        let outer_id = NodeId(1);
        let inner_id = NodeId(2);
        let focus_id = FocusId::default();

        let record_node = |a11y: &mut A11y, node_id| {
            let mut node = accesskit::Node::new(accesskit::Role::Button);
            node.add_action(Action::Click);
            assert!(a11y.nodes.push(node_id, node));
            a11y.record_focus_id(node_id, focus_id);
            a11y.record_node_bounds(node_id, Bounds::default(), Some(Point::default()));
            a11y.record_action_listener(node_id, Action::Click, Box::new(|_, _, _| {}));
            a11y.nodes.pop();
        };

        a11y.sync_active_flag();
        a11y.begin_frame();
        let _outer_checkpoint = a11y.prepaint_checkpoint();
        record_node(&mut a11y, outer_id);
        let inner_checkpoint = a11y.prepaint_checkpoint();
        record_node(&mut a11y, inner_id);
        a11y.nodes.set_focus(inner_id);
        a11y.rollback_prepaint(inner_checkpoint);
        a11y.nodes.set_focus(outer_id);

        assert!(a11y.nodes.has_node(outer_id));
        assert!(!a11y.nodes.has_node(inner_id));
        assert!(a11y.has_candidate_focus_id(outer_id));
        assert!(!a11y.has_candidate_focus_id(inner_id));
        assert!(a11y.has_candidate_node_bounds(outer_id));
        assert!(!a11y.has_candidate_node_bounds(inner_id));
        assert!(a11y.has_candidate_action_listener(outer_id));
        assert!(!a11y.has_candidate_action_listener(inner_id));

        let update = a11y.end_frame();
        let generation = a11y.activation_generation();
        a11y.publish(&update, generation);
        assert!(a11y.take_published_action_listeners(outer_id).is_some());
        assert!(a11y.take_published_action_listeners(inner_id).is_none());

        a11y.begin_frame();
        let outer_checkpoint = a11y.prepaint_checkpoint();
        record_node(&mut a11y, outer_id);
        let _inner_checkpoint = a11y.prepaint_checkpoint();
        record_node(&mut a11y, inner_id);
        a11y.rollback_prepaint(outer_checkpoint);

        assert!(!a11y.nodes.has_node(outer_id));
        assert!(!a11y.nodes.has_node(inner_id));
        assert!(!a11y.has_candidate_focus_id(outer_id));
        assert!(!a11y.has_candidate_focus_id(inner_id));
        assert!(!a11y.has_candidate_node_bounds(outer_id));
        assert!(!a11y.has_candidate_node_bounds(inner_id));
        assert!(!a11y.has_candidate_action_listener(outer_id));
        assert!(!a11y.has_candidate_action_listener(inner_id));

        record_node(&mut a11y, outer_id);
        record_node(&mut a11y, inner_id);
        let update = a11y.end_frame();
        assert!(update.nodes.iter().any(|(id, _)| *id == outer_id));
        assert!(update.nodes.iter().any(|(id, _)| *id == inner_id));

        a11y.begin_frame();
        let focus_checkpoint = a11y.prepaint_checkpoint();
        a11y.nodes.set_focus(inner_id);
        a11y.rollback_prepaint(focus_checkpoint);
        let update = a11y.end_frame();
        assert_eq!(update.focus, ROOT_NODE_ID);
    }

    #[test]
    fn clip_owner_scope_restores_before_a_later_sibling() {
        let mut nodes = A11yNodeBuilder::new();
        let clipped_id = NodeId(1);
        let sibling_id = NodeId(2);
        nodes.begin_frame();

        {
            let _scope = nodes.enter_clip_owner_scope();
            assert!(nodes.push(clipped_id, accesskit::Node::new(accesskit::Role::Group)));
            nodes.pop();
        }
        assert!(nodes.push(sibling_id, accesskit::Node::new(accesskit::Role::Group)));
        nodes.pop();

        let update = nodes.finalize();
        let clipped = update
            .nodes
            .iter()
            .find(|(id, _)| *id == clipped_id)
            .map(|(_, node)| node)
            .unwrap();
        let sibling = update
            .nodes
            .iter()
            .find(|(id, _)| *id == sibling_id)
            .map(|(_, node)| node)
            .unwrap();
        assert!(clipped.clips_children());
        assert!(!sibling.clips_children());
    }

    #[test]
    fn rollback_removes_deferred_children_from_a_finalized_parent() {
        let mut nodes = A11yNodeBuilder::new();
        let parent_id = NodeId(1);
        let rolled_back_id = NodeId(2);
        let sibling_id = NodeId(3);
        let first_child_id = NodeId(4);
        let last_child_id = NodeId(5);
        nodes.begin_frame();

        assert!(nodes.push(parent_id, accesskit::Node::new(accesskit::Role::Group)));
        assert!(nodes.push(
            first_child_id,
            accesskit::Node::new(accesskit::Role::Button)
        ));
        nodes.pop();
        let deferred_parent = nodes.reserve_deferred_parent().unwrap();
        assert!(nodes.push(last_child_id, accesskit::Node::new(accesskit::Role::Button)));
        nodes.pop();
        nodes.pop();
        let checkpoint = nodes.checkpoint();
        {
            let _parent_scope = nodes.enter_deferred_parent_scope(deferred_parent);
            let _clip_scope = nodes.enter_clip_owner_scope();
            assert!(nodes.push(
                rolled_back_id,
                accesskit::Node::new(accesskit::Role::Button)
            ));
            nodes.pop();
        }
        nodes.rollback(checkpoint);

        assert!(nodes.push(sibling_id, accesskit::Node::new(accesskit::Role::Button)));
        nodes.pop();

        let update = nodes.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        let parent = update
            .nodes
            .iter()
            .find(|(id, _)| *id == parent_id)
            .map(|(_, node)| node)
            .unwrap();
        let sibling = update
            .nodes
            .iter()
            .find(|(id, _)| *id == sibling_id)
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(root.children(), &[parent_id, sibling_id]);
        assert_eq!(parent.children(), &[first_child_id, last_child_id]);
        assert!(update.nodes.iter().all(|(id, _)| *id != rolled_back_id));
        assert!(!sibling.clips_children());
    }

    #[test]
    fn deferred_sibling_order_survives_out_of_order_scope_replay() {
        let mut nodes = A11yNodeBuilder::new();
        let before_id = NodeId(1);
        let first_deferred_id = NodeId(2);
        let second_deferred_id = NodeId(3);
        let after_id = NodeId(4);
        let hidden_id = NodeId(5);
        nodes.begin_frame();

        assert!(nodes.push(before_id, accesskit::Node::new(accesskit::Role::Button)));
        nodes.pop();
        let first_parent = nodes.reserve_deferred_parent().unwrap();
        let second_parent = nodes.reserve_deferred_parent().unwrap();
        assert!(nodes.push(after_id, accesskit::Node::new(accesskit::Role::Button)));
        nodes.pop();
        let mut hidden = accesskit::Node::new(accesskit::Role::Button);
        hidden.set_hidden();
        assert!(nodes.push(hidden_id, hidden));
        nodes.pop();

        {
            let _scope = nodes.enter_deferred_parent_scope(second_parent);
            assert!(nodes.push(
                second_deferred_id,
                accesskit::Node::new(accesskit::Role::Button)
            ));
            nodes.pop();
        }
        {
            let _scope = nodes.enter_deferred_parent_scope(first_parent);
            assert!(nodes.push(
                first_deferred_id,
                accesskit::Node::new(accesskit::Role::Button)
            ));
            nodes.pop();
        }

        let update = nodes.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        assert_eq!(
            root.children(),
            &[before_id, first_deferred_id, second_deferred_id, after_id]
        );
    }

    #[test]
    fn nested_window_portal_reuses_captured_root_order_after_filtering() {
        let mut nodes = A11yNodeBuilder::new();
        let owner_id = NodeId(1);
        let portal_id = NodeId(2);
        let after_id = NodeId(3);
        let hidden_id = NodeId(4);
        nodes.begin_frame();

        let deferred_parent = {
            let _modal_scope = nodes.enter_scope(AccessibilityTreeScope::ModalRoot);
            assert!(nodes.push(owner_id, accesskit::Node::new(accesskit::Role::Group)));
            let deferred_parent = nodes.reserve_deferred_parent().unwrap();
            nodes.pop();
            assert!(nodes.push(after_id, accesskit::Node::new(accesskit::Role::Button)));
            nodes.pop();
            let mut hidden = accesskit::Node::new(accesskit::Role::Button);
            hidden.set_hidden();
            assert!(nodes.push(hidden_id, hidden));
            nodes.pop();
            deferred_parent
        };

        {
            let _modal_scope = nodes.enter_scope(AccessibilityTreeScope::ModalRoot);
            let _deferred_scope = nodes.enter_deferred_parent_scope(deferred_parent);
            let portal_parent = nodes.reserve_window_portal_parent().unwrap();
            let _portal_scope = nodes.enter_window_portal_scope(portal_parent);
            assert!(nodes.push(portal_id, accesskit::Node::new(accesskit::Role::Button)));
            nodes.pop();
        }

        let update = nodes.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        assert_eq!(root.children(), &[owner_id, portal_id, after_id]);
    }

    #[test]
    fn nested_deferred_portal_reuses_the_outer_root_order() {
        let mut nodes = A11yNodeBuilder::new();
        let owner_id = NodeId(1);
        let intermediate_id = NodeId(2);
        let portal_id = NodeId(3);
        let after_id = NodeId(4);
        nodes.begin_frame();

        assert!(nodes.push(owner_id, accesskit::Node::new(accesskit::Role::Group)));
        let outer_parent = nodes.reserve_deferred_parent().unwrap();
        nodes.pop();
        assert!(nodes.push(after_id, accesskit::Node::new(accesskit::Role::Button)));
        nodes.pop();

        let inner_parent = {
            let _outer_scope = nodes.enter_deferred_parent_scope(outer_parent);
            assert!(nodes.push(
                intermediate_id,
                accesskit::Node::new(accesskit::Role::Group)
            ));
            let inner_parent = nodes.reserve_deferred_parent().unwrap();
            nodes.pop();
            inner_parent
        };

        {
            let _inner_scope = nodes.enter_deferred_parent_scope(inner_parent);
            let portal_parent = nodes.reserve_window_portal_parent().unwrap();
            let _portal_scope = nodes.enter_window_portal_scope(portal_parent);
            assert!(nodes.push(portal_id, accesskit::Node::new(accesskit::Role::Button)));
            nodes.pop();
        }

        let update = nodes.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        let owner = update
            .nodes
            .iter()
            .find(|(id, _)| *id == owner_id)
            .map(|(_, node)| node)
            .unwrap();
        assert_eq!(root.children(), &[owner_id, portal_id, after_id]);
        assert_eq!(owner.children(), &[intermediate_id]);
    }

    #[test]
    fn unavailable_deferred_parent_fails_closed_without_poisoning_later_nodes() {
        let mut nodes = A11yNodeBuilder::new();
        let child_id = NodeId(1);
        nodes.begin_frame();

        {
            let _scope = nodes.enter_deferred_parent_scope(AccessibilityDeferredParent {
                parent_id: NodeId(99),
                normal_child_index: 0,
                order_path: SmallVec::from_slice(&[0]),
                root_order: AccessibilityRootOrder {
                    normal_child_index: 0,
                    order_path: SmallVec::from_slice(&[0]),
                },
            });
            assert!(!nodes.push(child_id, accesskit::Node::new(accesskit::Role::Button)));
        }
        assert!(!nodes.has_node(child_id));

        assert!(nodes.push(child_id, accesskit::Node::new(accesskit::Role::Button)));
        nodes.pop();

        let update = nodes.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        assert_eq!(root.children(), &[child_id]);
        assert_eq!(
            update
                .nodes
                .iter()
                .filter(|(id, _)| *id == child_id)
                .count(),
            1
        );
    }

    #[test]
    fn rollback_removes_window_portal_child_from_a_non_top_root() {
        let mut nodes = A11yNodeBuilder::new();
        let owner_id = NodeId(1);
        let portal_child_id = NodeId(2);
        nodes.begin_frame();

        assert!(nodes.push(owner_id, accesskit::Node::new(accesskit::Role::Group)));
        let portal_parent = nodes.reserve_window_portal_parent().unwrap();
        let checkpoint = nodes.checkpoint();
        {
            let _clip_scope = nodes.enter_clip_owner_scope();
            let _portal_scope = nodes.enter_window_portal_scope(portal_parent);
            assert!(nodes.push(
                portal_child_id,
                accesskit::Node::new(accesskit::Role::Button)
            ));
            nodes.pop();
        }
        nodes.rollback(checkpoint);
        nodes.pop();

        let update = nodes.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        let owner = update
            .nodes
            .iter()
            .find(|(id, _)| *id == owner_id)
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(root.children(), &[owner_id]);
        assert!(owner.children().is_empty());
        assert!(update.nodes.iter().all(|(id, _)| *id != portal_child_id));
    }

    #[test]
    fn repair_tree_update_strips_invalid_node_references() {
        let valid_label = NodeId(2);
        let missing = NodeId(99);
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        let mut button = accesskit::Node::new(accesskit::Role::Button);
        let label = accesskit::Node::new(accesskit::Role::Label);

        root.set_children([NodeId(1), missing]);
        button.set_controls([valid_label, missing]);
        button.set_labelled_by([valid_label, missing]);
        button.set_active_descendant(missing);

        let update = accesskit::TreeUpdate {
            nodes: vec![
                (ROOT_NODE_ID, root),
                (NodeId(1), button),
                (valid_label, label),
            ],
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: missing,
        };

        let repaired = A11yNodeBuilder::repair_tree_update(update);
        let root = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        let button = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(1))
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(repaired.focus, ROOT_NODE_ID);
        assert_eq!(root.children(), &[NodeId(1)]);
        assert_eq!(button.controls(), &[valid_label]);
        assert_eq!(button.labelled_by(), &[valid_label]);
        assert_eq!(button.active_descendant(), None);
    }

    #[test]
    fn repair_tree_update_preserves_valid_node_references() {
        let button_id = NodeId(1);
        let label_id = NodeId(2);
        let controlled_id = NodeId(3);
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        let mut button = accesskit::Node::new(accesskit::Role::Button);
        let label = accesskit::Node::new(accesskit::Role::Label);
        let controlled = accesskit::Node::new(accesskit::Role::List);

        root.set_children([button_id, label_id, controlled_id]);
        button.set_controls([controlled_id]);
        button.set_labelled_by([label_id]);
        button.set_active_descendant(controlled_id);

        let update = accesskit::TreeUpdate {
            nodes: vec![
                (ROOT_NODE_ID, root),
                (button_id, button),
                (label_id, label),
                (controlled_id, controlled),
            ],
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: button_id,
        };

        let repaired = A11yNodeBuilder::repair_tree_update(update);
        let root = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        let button = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == button_id)
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(repaired.focus, button_id);
        assert_eq!(root.children(), &[button_id, label_id, controlled_id]);
        assert_eq!(button.controls(), &[controlled_id]);
        assert_eq!(button.labelled_by(), &[label_id]);
        assert_eq!(button.active_descendant(), Some(controlled_id));
    }

    #[test]
    fn repair_tree_update_clears_invalid_single_node_references() {
        let input_id = NodeId(1);
        let missing_error = NodeId(42);
        let missing_popup = NodeId(43);
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        let mut input = accesskit::Node::new(accesskit::Role::TextInput);

        root.set_children([input_id]);
        input.set_error_message(missing_error);
        input.set_popup_for(missing_popup);

        let update = accesskit::TreeUpdate {
            nodes: vec![(ROOT_NODE_ID, root), (input_id, input)],
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: input_id,
        };

        let repaired = A11yNodeBuilder::repair_tree_update(update);
        let input = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == input_id)
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(repaired.focus, input_id);
        assert_eq!(input.error_message(), None);
        assert_eq!(input.popup_for(), None);
    }

    #[test]
    fn repair_tree_update_clears_invalid_text_selections() {
        let missing = NodeId(99);
        let label_id = NodeId(1);
        let text_run_id = NodeId(2);
        let input_ids = [NodeId(3), NodeId(4), NodeId(5)];
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        let label = accesskit::Node::new(accesskit::Role::Label);
        let mut text_run = accesskit::Node::new(accesskit::Role::TextRun);
        text_run.set_value("ok");
        text_run.set_character_lengths([1, 1]);
        let selections = [
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: missing,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 1,
                },
            },
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: label_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: label_id,
                    character_index: 0,
                },
            },
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 3,
                },
            },
        ];
        let mut inputs = input_ids.map(|id| (id, accesskit::Node::new(accesskit::Role::TextInput)));
        for ((_, input), selection) in inputs.iter_mut().zip(selections) {
            input.set_text_selection(selection);
        }
        root.set_children(
            [label_id, text_run_id]
                .into_iter()
                .chain(input_ids)
                .collect::<Vec<_>>(),
        );

        let update = accesskit::TreeUpdate {
            nodes: vec![
                (ROOT_NODE_ID, root),
                (label_id, label),
                (text_run_id, text_run),
            ]
            .into_iter()
            .chain(inputs)
            .collect(),
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: input_ids[0],
        };

        let repaired = A11yNodeBuilder::repair_tree_update(update);
        for input_id in input_ids {
            let input = repaired
                .nodes
                .iter()
                .find(|(id, _)| *id == input_id)
                .map(|(_, node)| node)
                .unwrap();
            assert_eq!(input.text_selection(), None);
        }
    }

    #[test]
    fn repair_tree_update_preserves_valid_text_selection() {
        let input_id = NodeId(1);
        let text_run_id = NodeId(2);
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        let mut input = accesskit::Node::new(accesskit::Role::TextInput);
        let mut text_run = accesskit::Node::new(accesskit::Role::TextRun);
        let selection = accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: text_run_id,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: text_run_id,
                character_index: 2,
            },
        };
        root.set_children([input_id]);
        input.set_children([text_run_id]);
        input.set_text_selection(selection);
        text_run.set_value("ok");
        text_run.set_character_lengths([1, 1]);

        let repaired = A11yNodeBuilder::repair_tree_update(accesskit::TreeUpdate {
            nodes: vec![
                (ROOT_NODE_ID, root),
                (input_id, input),
                (text_run_id, text_run),
            ],
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: input_id,
        });
        let input = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == input_id)
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(input.text_selection(), Some(&selection));
    }

    #[test]
    fn repair_tree_update_rejects_a_foreign_text_run_selection() {
        let first_input_id = NodeId(1);
        let second_input_id = NodeId(2);
        let second_text_run_id = NodeId(3);
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        let mut first_input = accesskit::Node::new(accesskit::Role::TextInput);
        let mut second_input = accesskit::Node::new(accesskit::Role::TextInput);
        let mut second_text_run = accesskit::Node::new(accesskit::Role::TextRun);
        root.set_children([first_input_id, second_input_id]);
        second_input.set_children([second_text_run_id]);
        second_text_run.set_value("ok");
        second_text_run.set_character_lengths([1, 1]);
        first_input.set_text_selection(accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: second_text_run_id,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: second_text_run_id,
                character_index: 2,
            },
        });

        let repaired = A11yNodeBuilder::repair_tree_update(accesskit::TreeUpdate {
            nodes: vec![
                (ROOT_NODE_ID, root),
                (first_input_id, first_input),
                (second_input_id, second_input),
                (second_text_run_id, second_text_run),
            ],
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: first_input_id,
        });
        let first_input = repaired
            .nodes
            .iter()
            .find(|(id, _)| *id == first_input_id)
            .map(|(_, node)| node)
            .unwrap();

        assert_eq!(first_input.text_selection(), None);
    }

    #[test]
    fn repair_tree_update_strips_malformed_text_run_indexing_and_selection() {
        let cases = [
            (None, vec![1]),
            (Some("ok"), vec![1]),
            (Some("é"), vec![1, 1]),
        ];

        for (case_index, (value, lengths)) in cases.into_iter().enumerate() {
            let input_id = NodeId(10 + case_index as u64 * 2);
            let text_run_id = NodeId(input_id.0 + 1);
            let mut root = accesskit::Node::new(accesskit::Role::Window);
            let mut input = accesskit::Node::new(accesskit::Role::TextInput);
            let mut text_run = accesskit::Node::new(accesskit::Role::TextRun);
            root.set_children([input_id]);
            input.set_children([text_run_id]);
            input.set_text_selection(accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: text_run_id,
                    character_index: lengths.len(),
                },
            });
            if let Some(value) = value {
                text_run.set_value(value);
            }
            text_run.set_character_lengths(lengths);
            text_run.set_character_positions([0.0]);
            text_run.set_character_widths([1.0]);
            text_run.set_word_starts([0]);

            let repaired = A11yNodeBuilder::repair_tree_update(accesskit::TreeUpdate {
                nodes: vec![
                    (ROOT_NODE_ID, root),
                    (input_id, input),
                    (text_run_id, text_run),
                ],
                tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
                tree_id: accesskit::TreeId::ROOT,
                focus: input_id,
            });
            let input = repaired
                .nodes
                .iter()
                .find(|(id, _)| *id == input_id)
                .map(|(_, node)| node)
                .unwrap();
            let text_run = repaired
                .nodes
                .iter()
                .find(|(id, _)| *id == text_run_id)
                .map(|(_, node)| node)
                .unwrap();

            assert_eq!(input.text_selection(), None);
            assert!(text_run.character_lengths().is_empty());
            assert!(text_run.character_positions().is_none());
            assert!(text_run.character_widths().is_none());
            assert!(text_run.word_starts().is_empty());
        }
    }
}
