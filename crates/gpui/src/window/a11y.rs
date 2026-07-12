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

use crate::{App, Bounds, FocusId, Pixels, Window};
use accesskit::{Action, NodeId, TreeUpdate};
use open_gpui_collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// The fixed AccessKit node ID used for the root of every window's a11y tree.
pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);

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
    node_bounds: FxHashMap<NodeId, Bounds<Pixels>>,
    action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
}

macro_rules! define_published_action_masks {
    ($($variant:ident),+ $(,)?) => {
        fn action_mask(action: Action) -> u32 {
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
    candidate_focus_ids: Vec<(NodeId, FocusId)>,
    candidate_node_bounds: Vec<(NodeId, Bounds<Pixels>)>,
    candidate_action_listeners: Vec<(NodeId, Action, A11yActionListener)>,
    published: Option<PublishedA11yDispatch>,
    next_published_revision: u64,
}

pub(crate) struct A11yPrepaintCheckpoint {
    nodes: A11yNodeBuilderCheckpoint,
    focus_ids_len: usize,
    node_bounds_len: usize,
    action_listeners_len: usize,
}

impl A11y {
    pub(crate) fn new(active_state: Arc<AtomicU64>, force_disabled: bool) -> Self {
        Self {
            force_disabled,
            active_state,
            active_this_frame: false,
            activation_generation_this_frame: 0,
            nodes: A11yNodeBuilder::new(),
            candidate_focus_ids: Vec::new(),
            candidate_node_bounds: Vec::new(),
            candidate_action_listeners: Vec::new(),
            published: None,
            next_published_revision: 0,
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
        self.candidate_node_bounds.clear();
        self.candidate_action_listeners.clear();
        self.nodes.begin_frame();
    }

    /// Finalize the tree and produce a [`TreeUpdate`] for the platform adapter.
    pub(crate) fn end_frame(&mut self) -> TreeUpdate {
        self.nodes.finalize()
    }

    /// Replace action routing with the exact tree and frame state delivered to the platform.
    pub(crate) fn publish(&mut self, update: &TreeUpdate, activation_generation: u64) {
        let mut published = self.published.take().unwrap_or_default();
        published.action_masks.clear();
        published.action_masks.reserve(update.nodes.len());
        for (id, node) in &update.nodes {
            let mask = node_action_mask(node);
            if mask != 0 {
                published.action_masks.insert(*id, mask);
            }
        }

        published.focus_ids.clear();
        published.focus_ids.reserve(self.candidate_focus_ids.len());
        for (id, focus_id) in self.candidate_focus_ids.drain(..) {
            if published.action_masks.contains_key(&id) {
                published.focus_ids.insert(id, focus_id);
            }
        }

        published.node_bounds.clear();
        published
            .node_bounds
            .reserve(self.candidate_node_bounds.len());
        for (id, bounds) in self.candidate_node_bounds.drain(..) {
            if published.action_masks.contains_key(&id) {
                published.node_bounds.insert(id, bounds);
            }
        }

        published.action_listeners.clear();
        published
            .action_listeners
            .reserve(self.candidate_action_listeners.len());
        for (id, action, listener) in self.candidate_action_listeners.drain(..) {
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
    }

    pub(crate) fn record_focus_id(&mut self, node_id: NodeId, focus_id: FocusId) {
        self.candidate_focus_ids.push((node_id, focus_id));
    }

    pub(crate) fn record_node_bounds(&mut self, node_id: NodeId, bounds: Bounds<Pixels>) {
        self.candidate_node_bounds.push((node_id, bounds));
    }

    pub(crate) fn record_action_listener(
        &mut self,
        node_id: NodeId,
        action: Action,
        listener: A11yActionListener,
    ) {
        self.candidate_action_listeners
            .push((node_id, action, listener));
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

    pub(crate) fn published_node_bounds(&self, node_id: NodeId) -> Option<Bounds<Pixels>> {
        self.published.as_ref()?.node_bounds.get(&node_id).copied()
    }

    pub(crate) fn published_focus_id(&self, node_id: NodeId) -> Option<FocusId> {
        self.published.as_ref()?.focus_ids.get(&node_id).copied()
    }

    pub(crate) fn prepaint_checkpoint(&self) -> A11yPrepaintCheckpoint {
        A11yPrepaintCheckpoint {
            nodes: self.nodes.checkpoint(),
            focus_ids_len: self.candidate_focus_ids.len(),
            node_bounds_len: self.candidate_node_bounds.len(),
            action_listeners_len: self.candidate_action_listeners.len(),
        }
    }

    pub(crate) fn current_tree_scope(&self) -> AccessibilityTreeScope {
        self.nodes.effective_scope()
    }

    pub(crate) fn current_tree_hidden(&self) -> bool {
        self.nodes.is_hidden_subtree()
    }

    #[cfg(test)]
    pub(crate) fn set_requested_active_for_test(&self, active: bool) {
        set_requested_active(&self.active_state, active);
    }

    pub(crate) fn rollback_prepaint(&mut self, checkpoint: A11yPrepaintCheckpoint) {
        self.candidate_action_listeners
            .truncate(checkpoint.action_listeners_len);
        self.candidate_node_bounds
            .truncate(checkpoint.node_bounds_len);
        self.candidate_focus_ids.truncate(checkpoint.focus_ids_len);
        self.nodes.rollback(checkpoint.nodes);
    }

    #[cfg(test)]
    pub(crate) fn has_candidate_focus_id(&self, node_id: NodeId) -> bool {
        self.candidate_focus_ids
            .iter()
            .any(|(candidate, _)| *candidate == node_id)
    }

    #[cfg(test)]
    pub(crate) fn has_candidate_node_bounds(&self, node_id: NodeId) -> bool {
        self.candidate_node_bounds
            .iter()
            .any(|(candidate, _)| *candidate == node_id)
    }

    #[cfg(test)]
    pub(crate) fn has_candidate_action_listener(&self, node_id: NodeId) -> bool {
        self.candidate_action_listeners
            .iter()
            .any(|(candidate, _, _)| *candidate == node_id)
    }
}

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

pub(crate) fn requested_generation(state: &AtomicU64) -> u64 {
    requested_state(state).1
}

pub(crate) struct A11yNodeBuilder {
    ids_stack: SmallVec<[NodeId; 16]>,
    nodes_stack: SmallVec<[accesskit::Node; 16]>,
    /// This is the exact type required by accesskit, so we can't just make it a
    /// `HashMap<NodeId, Node>` to remove the need for `seen_ids`
    all_nodes: Vec<(NodeId, accesskit::Node)>,
    seen_ids: FxHashSet<NodeId>,
    scope_stack: Rc<RefCell<SmallVec<[AccessibilityTreeScope; 8]>>>,
    hidden_depth: Rc<Cell<usize>>,
    memberships: Vec<(NodeId, AccessibilityTreeScope)>,
    modal_restricted: bool,
    focus: NodeId,
    #[cfg(debug_assertions)]
    has_set_focus: bool,
}

struct A11yNodeBuilderCheckpoint {
    stack_depth: usize,
    top_id: Option<NodeId>,
    top_children_len: usize,
    all_nodes_len: usize,
    scope_depth: usize,
    hidden_depth: usize,
    memberships_len: usize,
    modal_restricted: bool,
    focus: NodeId,
    #[cfg(debug_assertions)]
    has_set_focus: bool,
}

pub(crate) struct AccessibilityTreeScopeGuard {
    stack: Rc<RefCell<SmallVec<[AccessibilityTreeScope; 8]>>>,
    depth: usize,
}

pub(crate) struct AccessibilityHiddenSubtreeGuard {
    depth: Rc<Cell<usize>>,
    previous_depth: usize,
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

impl Drop for AccessibilityHiddenSubtreeGuard {
    fn drop(&mut self) {
        debug_assert_eq!(
            self.depth.get(),
            self.previous_depth + 1,
            "accessibility hidden subtrees must be dropped in nesting order"
        );
        self.depth.set(self.previous_depth);
    }
}

impl A11yNodeBuilder {
    fn new() -> Self {
        Self {
            ids_stack: SmallVec::new(),
            nodes_stack: SmallVec::new(),
            all_nodes: Vec::new(),
            seen_ids: FxHashSet::default(),
            scope_stack: Rc::new(RefCell::new(SmallVec::new())),
            hidden_depth: Rc::new(Cell::new(0)),
            memberships: Vec::new(),
            modal_restricted: false,
            focus: ROOT_NODE_ID,
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
            hidden_depth: self.hidden_depth.get(),
            memberships_len: self.memberships.len(),
            modal_restricted: self.modal_restricted,
            focus: self.focus,
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

        for (id, _) in &self.memberships[checkpoint.memberships_len..] {
            self.seen_ids.remove(id);
        }
        self.all_nodes.truncate(checkpoint.all_nodes_len);
        self.memberships.truncate(checkpoint.memberships_len);

        if stack_prefix_is_intact {
            self.ids_stack.truncate(checkpoint.stack_depth);
            self.nodes_stack.truncate(checkpoint.stack_depth);
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
        self.hidden_depth.set(checkpoint.hidden_depth);
        self.modal_restricted = checkpoint.modal_restricted;
        self.focus = checkpoint.focus;
        #[cfg(debug_assertions)]
        {
            self.has_set_focus = checkpoint.has_set_focus;
        }
    }

    /// Push a new node onto the stack. It becomes a child of the current
    /// top-of-stack node.
    ///
    /// Returns `true` if the node was successfully pushed.
    pub(crate) fn push(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        debug_assert!(!self.ids_stack.is_empty(), "push called before push_root");

        if self.is_hidden_subtree() {
            return false;
        }

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

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.ids_stack.push(id);
        self.nodes_stack.push(node);
        self.memberships.push((id, scope));
        true
    }

    /// Pop the current node off the stack and finalize it into the all_nodes
    /// list.
    pub(crate) fn pop(&mut self) {
        debug_assert!(self.ids_stack.len() > 1, "pop would remove the root node");

        if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
            self.all_nodes.push((id, node));
        }
    }

    /// Push the root node to start a new frame.
    fn begin_frame(&mut self) {
        self.all_nodes.clear();
        self.ids_stack.clear();
        self.nodes_stack.clear();
        self.seen_ids.clear();
        self.scope_stack.borrow_mut().clear();
        self.hidden_depth.set(0);
        self.memberships.clear();
        self.modal_restricted = false;
        #[cfg(debug_assertions)]
        {
            self.has_set_focus = false;
        }
        let root_node = accesskit::Node::new(accesskit::Role::Window);

        self.ids_stack.push(ROOT_NODE_ID);
        self.nodes_stack.push(root_node);
        self.focus = ROOT_NODE_ID;
    }

    pub(crate) fn enter_scope(
        &mut self,
        scope: AccessibilityTreeScope,
    ) -> AccessibilityTreeScopeGuard {
        if !self.is_hidden_subtree()
            && matches!(
                scope,
                AccessibilityTreeScope::ModalRoot | AccessibilityTreeScope::ModalDescendant
            )
        {
            self.modal_restricted = true;
        }

        let depth = self.scope_stack.borrow().len();
        self.scope_stack.borrow_mut().push(scope);
        AccessibilityTreeScopeGuard {
            stack: self.scope_stack.clone(),
            depth,
        }
    }

    pub(crate) fn enter_hidden_subtree(&mut self) -> AccessibilityHiddenSubtreeGuard {
        let previous_depth = self.hidden_depth.get();
        self.hidden_depth.set(previous_depth + 1);
        AccessibilityHiddenSubtreeGuard {
            depth: self.hidden_depth.clone(),
            previous_depth,
        }
    }

    fn is_hidden_subtree(&self) -> bool {
        self.hidden_depth.get() != 0
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

    /// Set the focused node for this frame.
    pub(crate) fn set_focus(&mut self, id: NodeId) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                !self.has_set_focus,
                "set_focus called more than once in a single frame"
            );
            self.has_set_focus = true;
        }
        self.focus = id;
    }

    fn finalize(&mut self) -> TreeUpdate {
        // Stack should contain only the root node
        debug_assert_eq!(self.ids_stack.len(), 1);
        debug_assert_eq!(self.ids_stack[0], ROOT_NODE_ID);
        debug_assert!(
            self.scope_stack.borrow().is_empty(),
            "accessibility tree scope stack must be empty at frame end"
        );
        debug_assert_eq!(
            self.hidden_depth.get(),
            0,
            "accessibility hidden subtree stack must be empty at frame end"
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
            if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
                self.all_nodes.push((id, node));
            }
        }

        let nodes = std::mem::take(&mut self.all_nodes);
        let modal_restricted = std::mem::take(&mut self.modal_restricted);
        let update = TreeUpdate {
            nodes,
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: self.focus,
        };

        let update = Self::filter_published_tree(update, &self.memberships, modal_restricted);
        self.memberships.clear();
        Self::repair_tree_update(update)
    }

    fn filter_published_tree(
        mut update: TreeUpdate,
        memberships: &[(NodeId, AccessibilityTreeScope)],
        modal_restricted: bool,
    ) -> TreeUpdate {
        if !modal_restricted && !update.nodes.iter().any(|(_, node)| node.is_hidden()) {
            return update;
        }

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

        let root_children: Vec<NodeId> = memberships
            .iter()
            .filter_map(|(id, _)| {
                if !retained.contains(id) {
                    return None;
                }
                let parent = parents.get(id).copied();
                (parent == Some(root)
                    || parent.is_none()
                    || parent.is_some_and(|parent| !retained.contains(&parent)))
                .then_some(*id)
            })
            .collect();

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
        }

        update
    }
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
    fn published_action_authority_changes_only_after_matching_activation_delivery() {
        let active_state = Arc::new(AtomicU64::new(0));
        let mut a11y = A11y::new(active_state.clone(), false);
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
        let mut a11y = A11y::new(active_state, false);
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
        let mut a11y = A11y::new(active_state, false);
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
                a11y.record_node_bounds(node_id, Bounds::default());
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
                published.node_bounds.capacity(),
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
                published.node_bounds.capacity(),
                published.action_listeners.capacity(),
            ),
            first_dispatch_capacities
        );
    }

    #[test]
    fn nested_prepaint_checkpoints_restore_only_their_candidate_suffix() {
        let active_state = Arc::new(AtomicU64::new(0));
        set_requested_active(&active_state, true);
        let mut a11y = A11y::new(active_state, false);
        let outer_id = NodeId(1);
        let inner_id = NodeId(2);
        let focus_id = FocusId::default();

        let record_node = |a11y: &mut A11y, node_id| {
            let mut node = accesskit::Node::new(accesskit::Role::Button);
            node.add_action(Action::Click);
            assert!(a11y.nodes.push(node_id, node));
            a11y.record_focus_id(node_id, focus_id);
            a11y.record_node_bounds(node_id, Bounds::default());
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
}
