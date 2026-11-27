//! Replay balanced fragments into the current accessibility candidate.
//!
//! Finished nodes already carry their exact children, including synthetic hierarchies. The
//! candidate keeps those nodes in finish order, plus the roots appended to the open parent and
//! the metadata collected by normal prepaint and paint. Publication and rollback are shared with
//! the ordinary draw path.

use super::{A11y, A11yActionListener, A11yNodeBuilder, AccessibilityTreeScope};
use crate::{Bounds, FocusId, Pixels, Point};
use accesskit::{Action, Node, NodeId};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Arc, atomic::AtomicU64},
};

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(super) struct ActionListener(Rc<RefCell<A11yActionListener>>);

impl ActionListener {
    pub(super) fn new(listener: A11yActionListener) -> Self {
        Self(Rc::new(RefCell::new(listener)))
    }

    pub(super) fn into_dispatch(self) -> A11yActionListener {
        match Rc::try_unwrap(self.0) {
            Ok(listener) => listener.into_inner(),
            Err(shared) => Box::new(move |data, window, cx| {
                if let Ok(mut listener) = shared.try_borrow_mut() {
                    listener(data, window, cx);
                }
            }),
        }
    }
}

struct CandidateIdentity {
    activation: Arc<AtomicU64>,
    generation: u64,
}

#[derive(Default)]
pub(super) struct Candidate {
    identity: Option<Rc<CandidateIdentity>>,
    captured: RefCell<Vec<Weak<A11yFragment>>>,
}

impl Candidate {
    pub(super) fn begin(&mut self, activation: &Arc<AtomicU64>, generation: u64) {
        self.captured.get_mut().clear();
        self.identity = Some(Rc::new(CandidateIdentity {
            activation: activation.clone(),
            generation,
        }));
    }

    pub(super) fn len(&self) -> usize {
        self.captured.borrow().len()
    }

    pub(super) fn rollback(&mut self, len: usize) {
        self.captured.get_mut().truncate(len);
    }

    pub(super) fn discard(&mut self) {
        self.identity = None;
        self.captured.get_mut().clear();
    }

    pub(super) fn accept(&mut self, activation_generation: u64) {
        if self
            .identity
            .as_ref()
            .is_some_and(|identity| identity.generation == activation_generation)
        {
            for fragment in self.captured.get_mut().iter().filter_map(Weak::upgrade) {
                fragment.accepted.set(true);
            }
        }
        self.discard();
    }
}

#[derive(Default)]
pub(super) struct NodeCapture {
    barrier: u64,
    edits: Vec<NodeId>,
}

impl NodeCapture {
    pub(super) fn clear(&mut self) {
        self.barrier = 0;
        self.edits.clear();
    }

    pub(super) fn block(&mut self) {
        self.barrier = self.barrier.wrapping_add(1);
    }

    pub(super) fn record_node_edit(&mut self, node: NodeId) {
        self.edits.push(node);
    }
}

impl A11yNodeBuilder {
    fn ordinary_replay_parent(&self) -> Option<(NodeId, &Node)> {
        if self.effective_scope() != AccessibilityTreeScope::Unrestricted
            || self
                .geometry_validity_stack
                .borrow()
                .iter()
                .any(Option::is_some)
            || !self.clip_owner_scope_depths.borrow().is_empty()
            || !self.deferred_parent_scopes.borrow().is_empty()
            || !self.window_portal_scopes.borrow().is_empty()
        {
            return None;
        }
        self.ids_stack.last().copied().zip(self.nodes_stack.last())
    }
}

impl A11y {
    fn active_replay_candidate(&self) -> Option<&Rc<CandidateIdentity>> {
        self.retained.identity.as_ref().filter(|candidate| {
            self.is_active() && candidate.generation == self.activation_generation()
        })
    }
}

#[derive(Clone)]
struct CaptureStart {
    candidate: Rc<CandidateIdentity>,
    parent: NodeId,
    stack_depth: usize,
    children: usize,
    barrier: u64,
    edits: usize,
    nodes: usize,
    focus_ids: usize,
    missing_focus: usize,
    node_geometry: usize,
    active_descendants: usize,
    action_listeners: usize,
}

/// Start of a balanced native accessibility prepaint or paint range.
#[derive(Clone, Default)]
pub(crate) struct A11yCheckpoint(Option<CaptureStart>);

/// A balanced range of the native accessibility candidate and its action registrations.
pub(crate) struct A11yFragment {
    candidate: Rc<CandidateIdentity>,
    accepted: Cell<bool>,
    parent: Option<NodeId>,
    roots: Vec<NodeId>,
    nodes: Vec<(NodeId, Node)>,
    focus_ids: Vec<(NodeId, FocusId)>,
    missing_focus: Vec<(FocusId, &'static str)>,
    node_geometry: Vec<(NodeId, Bounds<Pixels>, Option<Point<Pixels>>)>,
    active_descendants: Vec<NodeId>,
    action_listeners: Vec<(NodeId, Action, ActionListener)>,
    #[cfg(debug_assertions)]
    node_info: Vec<(NodeId, super::debug::NodeDebugInfo)>,
}

/// Disqualify enclosing ranges when prepaint changes window-owned accessibility state.
pub(crate) fn block_capture(a11y: &mut A11y) {
    if a11y.is_active() {
        a11y.nodes.retained.block();
    }
}

/// Mark the start of one prepaint or paint phase while building a candidate.
pub(crate) fn checkpoint(a11y: &A11y) -> A11yCheckpoint {
    let start = (|| {
        let candidate = a11y.active_replay_candidate()?;
        let nodes = &a11y.nodes;
        let (parent, node) = nodes.ordinary_replay_parent()?;
        Some(CaptureStart {
            candidate: candidate.clone(),
            parent,
            stack_depth: nodes.ids_stack.len(),
            children: node.children().len(),
            barrier: nodes.retained.barrier,
            edits: nodes.retained.edits.len(),
            nodes: nodes.all_nodes.len(),
            focus_ids: a11y.candidate_focus_ids.len(),
            missing_focus: a11y.candidate_missing_focus.len(),
            node_geometry: a11y.candidate_node_geometry.len(),
            active_descendants: nodes.active_descendant_candidates.len(),
            action_listeners: a11y.candidate_action_listeners.len(),
        })
    })();
    A11yCheckpoint(start)
}

/// Capture a phase's candidate suffix. The fragment becomes eligible upon native publication.
pub(crate) fn capture(a11y: &A11y, start: &A11yCheckpoint) -> Option<Rc<A11yFragment>> {
    let start = start.0.as_ref()?;
    let candidate = a11y.active_replay_candidate()?;
    let nodes = &a11y.nodes;
    let (parent, parent_node) = nodes.ordinary_replay_parent()?;
    if !Rc::ptr_eq(candidate, &start.candidate)
        || nodes.retained.barrier != start.barrier
        || nodes.ids_stack.len() != start.stack_depth
        || parent != start.parent
        || nodes.retained.edits.get(start.edits..)?.contains(&parent)
    {
        return None;
    }

    let finished = nodes.all_nodes.get(start.nodes..)?;
    let roots = parent_node.children().get(start.children..)?;
    let fragment = Rc::new(A11yFragment {
        candidate: candidate.clone(),
        accepted: Cell::new(false),
        parent: (!finished.is_empty()).then_some(parent),
        roots: roots.to_vec(),
        nodes: finished
            .iter()
            .map(|(id, node, _)| (*id, node.clone()))
            .collect(),
        focus_ids: a11y
            .candidate_focus_ids
            .get(start.focus_ids..)?
            .iter()
            .map(|&(id, focus, _)| (id, focus))
            .collect(),
        missing_focus: a11y
            .candidate_missing_focus
            .get(start.missing_focus..)?
            .iter()
            .map(|&(focus, reason, _)| (focus, reason))
            .collect(),
        node_geometry: a11y
            .candidate_node_geometry
            .get(start.node_geometry..)?
            .iter()
            .map(|&(id, bounds, witness, _)| (id, bounds, witness))
            .collect(),
        active_descendants: nodes
            .active_descendant_candidates
            .get(start.active_descendants..)?
            .iter()
            .map(|&(id, _)| id)
            .collect(),
        action_listeners: a11y
            .candidate_action_listeners
            .get(start.action_listeners..)?
            .iter()
            .map(|(id, action, listener, _)| (*id, *action, listener.clone()))
            .collect(),
        #[cfg(debug_assertions)]
        node_info: finished
            .iter()
            .filter_map(|(id, _, _)| Some((*id, nodes.node_info.get(id)?.clone())))
            .collect(),
    });
    a11y.retained
        .captured
        .borrow_mut()
        .push(Rc::downgrade(&fragment));
    Some(fragment)
}

/// Qualify replay before mutating the candidate. The caller must match the original layout,
/// clipping and scale. Bounds and native action witnesses are only replayed in place.
pub(crate) fn can_replay(
    a11y: &A11y,
    fragment: Option<&A11yFragment>,
    offset: Point<Pixels>,
) -> bool {
    if !a11y.is_active() {
        return true;
    }
    let (Some(fragment), Some(candidate), Some((parent, _))) = (
        fragment,
        a11y.active_replay_candidate(),
        a11y.nodes.ordinary_replay_parent(),
    ) else {
        return false;
    };
    offset == Point::default()
        && fragment.accepted.get()
        && Arc::ptr_eq(&candidate.activation, &fragment.candidate.activation)
        && candidate.generation == fragment.candidate.generation
        && fragment.parent.is_none_or(|source| source == parent)
        && fragment
            .nodes
            .iter()
            .all(|(id, _)| !a11y.nodes.has_node(*id))
}

/// Append the qualified suffix to the candidate. A refusal leaves it untouched.
pub(crate) fn replay(
    a11y: &mut A11y,
    fragment: Option<&A11yFragment>,
    offset: Point<Pixels>,
) -> bool {
    if !can_replay(a11y, fragment, offset) {
        return false;
    }
    let Some(fragment) = fragment.filter(|_| a11y.is_active()) else {
        return true;
    };

    let nodes = &mut a11y.nodes;
    if !fragment.roots.is_empty()
        && let Some(parent) = nodes.nodes_stack.last_mut()
    {
        let mut children = parent.children().to_vec();
        children.extend_from_slice(&fragment.roots);
        parent.set_children(children);
    }
    for (id, node) in &fragment.nodes {
        nodes.seen_ids.insert(*id);
        nodes
            .memberships
            .push((*id, AccessibilityTreeScope::Unrestricted, None));
        nodes.all_nodes.push((*id, node.clone(), None));
    }
    nodes
        .active_descendant_candidates
        .extend(fragment.active_descendants.iter().map(|&id| (id, None)));
    #[cfg(debug_assertions)]
    nodes.node_info.extend(fragment.node_info.iter().cloned());
    a11y.candidate_focus_ids.extend(
        fragment
            .focus_ids
            .iter()
            .map(|&(id, focus)| (id, focus, None)),
    );
    a11y.candidate_missing_focus.extend(
        fragment
            .missing_focus
            .iter()
            .map(|&(focus, reason)| (focus, reason, None)),
    );
    a11y.candidate_node_geometry.extend(
        fragment
            .node_geometry
            .iter()
            .map(|&(id, bounds, witness)| (id, bounds, witness, None)),
    );
    a11y.candidate_action_listeners.extend(
        fragment
            .action_listeners
            .iter()
            .map(|(id, action, listener)| (*id, *action, listener.clone(), None)),
    );
    true
}
