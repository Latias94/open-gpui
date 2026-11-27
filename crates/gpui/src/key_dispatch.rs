//! KeyDispatch is where GPUI deals with binding actions to key events.
//!
//! The key pieces to making a key binding work are to define an action,
//! implement a method that takes that action as a type parameter,
//! and then to register the action during render on a focused node
//! with a keymap context:
//!
//! ```ignore
//! actions!(editor,[Undo, Redo]);
//!
//! impl Editor {
//!   fn undo(&mut self, _: &Undo, _window: &mut Window, _cx: &mut Context<Self>) { ... }
//!   fn redo(&mut self, _: &Redo, _window: &mut Window, _cx: &mut Context<Self>) { ... }
//! }
//!
//! impl Render for Editor {
//!   fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!     div()
//!       .track_focus(&self.focus_handle(cx))
//!       .key_context("Editor")
//!       .on_action(cx.listener(Editor::undo))
//!       .on_action(cx.listener(Editor::redo))
//!     ...
//!    }
//! }
//!```
//!
//! The keybindings themselves are managed independently by calling cx.bind_keys().
//! Applications can also load equivalent bindings from their own default keymap files.
//!
//! ```ignore
//! cx.bind_keys([
//!   KeyBinding::new("cmd-z", Editor::undo, Some("Editor")),
//!   KeyBinding::new("cmd-shift-z", Editor::redo, Some("Editor")),
//! ])
//! ```
//!
//! With all of this in place, GPUI will ensure that if you have an Editor that contains
//! the focus, hitting cmd-z will Undo.
//!
//! In real apps, it is a little more complicated than this, because typically you have
//! several nested views that each register keyboard handlers. In this case action matching
//! bubbles up from the bottom. For example, an application might have a Workspace top-level view
//! containing panes, which contain editors. If there are conflicting keybindings defined, then the
//! editor's bindings take precedence over the pane's bindings, which take precedence over the
//! workspace.
//!
//! In GPUI, keybindings are not limited to just single keystrokes, you can define
//! sequences by separating the keys with a space:
//!
//!  KeyBinding::new("cmd-k left", pane::SplitLeft, Some("Pane"))

use crate::collections::FxHashMap;
use crate::{
    Action, ActionRegistry, App, DispatchPhase, EntityId, FocusId, KeyBinding, KeyContext, Keymap,
    Keystroke, ModifiersChangedEvent, Window, geometry::SubtreeGeometryValidity,
};
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    ops::Range,
    rc::Rc,
};

/// ID of a node within `DispatchTree`. Note that these are **not** stable between frames, and so a
/// `DispatchNodeId` should only be used with the `DispatchTree` that provided it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct DispatchNodeId(usize);

impl DispatchNodeId {
    pub(crate) fn is_in(self, range: &Range<usize>) -> bool {
        range.contains(&self.0)
    }
}

/// Maps ordered dispatch segments around the freshly built interiors of a retained subtree.
pub(crate) struct DispatchSplice {
    source_anchor: DispatchNodeId,
    target_anchor: DispatchNodeId,
    segments: SmallVec<[ReusedSubtree; 4]>,
}

impl DispatchSplice {
    pub(crate) fn new(source_anchor: DispatchNodeId, target_anchor: DispatchNodeId) -> Self {
        Self {
            source_anchor,
            target_anchor,
            segments: SmallVec::new(),
        }
    }

    pub(crate) fn target_for(&self, source: DispatchNodeId) -> Option<DispatchNodeId> {
        if source == self.source_anchor {
            return Some(self.target_anchor);
        }
        let index = self
            .segments
            .partition_point(|segment| segment.old_range.end <= source.0);
        let segment = self.segments.get(index)?;
        source
            .is_in(&segment.old_range)
            .then(|| segment.refresh_node_id(source))
    }
}

pub(crate) struct DispatchTree {
    node_stack: Vec<DispatchNodeId>,
    pub(crate) context_stack: Vec<KeyContext>,
    view_stack: Vec<EntityId>,
    nodes: Vec<DispatchNode>,
    focusable_node_ids: FxHashMap<FocusId, DispatchNodeId>,
    view_node_ids: FxHashMap<EntityId, DispatchNodeId>,
    keymap: Rc<RefCell<Keymap>>,
    action_registry: Rc<ActionRegistry>,
}

#[derive(Default)]
pub(crate) struct DispatchNode {
    pub key_listeners: Vec<KeyListener>,
    pub action_listeners: Vec<DispatchActionListener>,
    pub modifiers_changed_listeners: Vec<ModifiersChangedListener>,
    pub context: Option<KeyContext>,
    pub focus_id: Option<FocusId>,
    view_id: Option<EntityId>,
    parent: Option<DispatchNodeId>,
    validity: Option<SubtreeGeometryValidity>,
}

#[derive(Clone)]
pub(crate) struct ReusedSubtree {
    old_range: Range<usize>,
    new_range: Range<usize>,
    contains_focus: bool,
}

impl ReusedSubtree {
    pub fn refresh_node_id(&self, node_id: DispatchNodeId) -> DispatchNodeId {
        debug_assert!(
            node_id.is_in(&self.old_range),
            "node {} was not part of the reused subtree {:?}",
            node_id.0,
            self.old_range
        );
        DispatchNodeId((node_id.0 - self.old_range.start) + self.new_range.start)
    }

    pub fn contains_focus(&self) -> bool {
        self.contains_focus
    }
}

#[derive(Default, Debug)]
pub(crate) struct Replay {
    pub(crate) keystroke: Keystroke,
    pub(crate) bindings: SmallVec<[KeyBinding; 1]>,
}

#[derive(Default, Debug)]
pub(crate) struct DispatchResult {
    pub(crate) pending: SmallVec<[Keystroke; 1]>,
    pub(crate) pending_has_binding: bool,
    pub(crate) bindings: SmallVec<[KeyBinding; 1]>,
    pub(crate) to_replay: SmallVec<[Replay; 1]>,
    pub(crate) context_stack: Vec<KeyContext>,
}

type KeyListener = Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App)>;
type ModifiersChangedListener = Rc<dyn Fn(&ModifiersChangedEvent, &mut Window, &mut App)>;

#[derive(Clone)]
pub(crate) struct DispatchActionListener {
    pub(crate) action_type: TypeId,
    pub(crate) listener: Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App)>,
}

impl DispatchTree {
    pub fn new(keymap: Rc<RefCell<Keymap>>, action_registry: Rc<ActionRegistry>) -> Self {
        Self {
            node_stack: Vec::new(),
            context_stack: Vec::new(),
            view_stack: Vec::new(),
            nodes: Vec::new(),
            focusable_node_ids: FxHashMap::default(),
            view_node_ids: FxHashMap::default(),
            keymap,
            action_registry,
        }
    }

    pub fn clear(&mut self) {
        self.node_stack.clear();
        self.context_stack.clear();
        self.view_stack.clear();
        self.nodes.clear();
        self.focusable_node_ids.clear();
        self.view_node_ids.clear();
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn push_node_scoped(
        &mut self,
        validity: Option<SubtreeGeometryValidity>,
    ) -> DispatchNodeId {
        let parent = self.node_stack.last().copied();
        let node_id = DispatchNodeId(self.nodes.len());

        self.nodes.push(DispatchNode {
            parent,
            validity,
            ..Default::default()
        });
        self.node_stack.push(node_id);
        node_id
    }

    pub fn set_active_node(&mut self, node_id: DispatchNodeId) {
        let next_node_parent = self.nodes[node_id.0].parent;
        while self.node_stack.last().copied() != next_node_parent && !self.node_stack.is_empty() {
            self.pop_node();
        }

        if self.node_stack.last().copied() == next_node_parent {
            self.push_active_node(node_id);
        } else {
            debug_assert_eq!(self.node_stack.len(), 0);

            let mut current_node_id = Some(node_id);
            while let Some(node_id) = current_node_id {
                let node = &self.nodes[node_id.0];
                if let Some(context) = node.context.clone() {
                    self.context_stack.push(context);
                }
                if let Some(view_id) = node.view_id {
                    self.view_stack.push(view_id);
                }
                self.node_stack.push(node_id);
                current_node_id = node.parent;
            }

            self.context_stack.reverse();
            self.view_stack.reverse();
            self.node_stack.reverse();
        }
    }

    fn push_active_node(&mut self, node_id: DispatchNodeId) {
        self.node_stack.push(node_id);
        let node = &self.nodes[node_id.0];
        if let Some(view_id) = node.view_id {
            self.view_stack.push(view_id);
        }
        if let Some(context) = node.context.clone() {
            self.context_stack.push(context);
        }
    }

    pub fn set_key_context(&mut self, context: KeyContext) {
        self.active_node().context = Some(context.clone());
        self.context_stack.push(context);
    }

    pub fn set_focus_id(&mut self, focus_id: FocusId) {
        let node_id = *self.node_stack.last().unwrap();
        self.nodes[node_id.0].focus_id = Some(focus_id);
        self.focusable_node_ids.insert(focus_id, node_id);
    }

    pub fn set_view_id(&mut self, view_id: EntityId) {
        if self.view_stack.last().copied() != Some(view_id) {
            let node_id = *self.node_stack.last().unwrap();
            self.nodes[node_id.0].view_id = Some(view_id);
            self.view_node_ids.insert(view_id, node_id);
            self.view_stack.push(view_id);
        }
    }

    pub fn pop_node(&mut self) {
        let node = &self.nodes[self.active_node_id().unwrap().0];
        if node.context.is_some() {
            self.context_stack.pop();
        }
        if node.view_id.is_some() {
            self.view_stack.pop();
        }
        self.node_stack.pop();
    }

    fn replay_node(&mut self, source: &DispatchNode, validity: Option<SubtreeGeometryValidity>) {
        let node_id = self.append_replayed_node(
            source,
            self.active_node_id(),
            self.view_stack.last().copied(),
            validity,
        );
        self.push_active_node(node_id);
    }

    fn append_replayed_node(
        &mut self,
        source: &DispatchNode,
        parent: Option<DispatchNodeId>,
        enclosing_view: Option<EntityId>,
        validity: Option<SubtreeGeometryValidity>,
    ) -> DispatchNodeId {
        let node_id = DispatchNodeId(self.nodes.len());
        let view_id = source.view_id.filter(|&id| Some(id) != enclosing_view);
        if let Some(focus_id) = source.focus_id {
            self.focusable_node_ids.insert(focus_id, node_id);
        }
        if let Some(view_id) = view_id {
            self.view_node_ids.insert(view_id, node_id);
        }
        self.nodes.push(DispatchNode {
            key_listeners: source.key_listeners.clone(),
            action_listeners: source.action_listeners.clone(),
            modifiers_changed_listeners: source.modifiers_changed_listeners.clone(),
            context: source.context.clone(),
            focus_id: source.focus_id,
            view_id,
            parent,
            validity: SubtreeGeometryValidity::replayed_under(source.validity.as_ref(), validity),
        });
        node_id
    }

    pub fn reuse_subtree(
        &mut self,
        old_range: Range<usize>,
        source: &Self,
        focus: Option<FocusId>,
        validity: Option<SubtreeGeometryValidity>,
    ) -> ReusedSubtree {
        let new_range = self.nodes.len()..self.nodes.len() + old_range.len();

        let mut contains_focus = false;
        let mut source_stack = vec![];
        for (source_node_id, source_node) in source
            .nodes
            .iter()
            .enumerate()
            .skip(old_range.start)
            .take(old_range.len())
        {
            let source_node_id = DispatchNodeId(source_node_id);
            while let Some(source_ancestor) = source_stack.last() {
                if source_node.parent == Some(*source_ancestor) {
                    break;
                } else {
                    source_stack.pop();
                    self.pop_node();
                }
            }

            source_stack.push(source_node_id);
            if source_node.focus_id.is_some() && source_node.focus_id == focus {
                contains_focus = true;
            }
            self.replay_node(source_node, validity.clone());
        }

        while !source_stack.is_empty() {
            source_stack.pop();
            self.pop_node();
        }

        ReusedSubtree {
            old_range,
            new_range,
            contains_focus,
        }
    }

    /// Copy a segment in journal order. Each source parent must be the anchor or an earlier
    /// copied node, so a gap can omit a balanced interior while retaining its wrapper.
    pub(crate) fn reuse_segment(
        &mut self,
        old_range: Range<usize>,
        source: &Self,
        focus: Option<FocusId>,
        validity: Option<SubtreeGeometryValidity>,
        splice: &mut DispatchSplice,
    ) -> ReusedSubtree {
        assert!(
            !splice.source_anchor.is_in(&old_range)
                && splice
                    .segments
                    .last()
                    .is_none_or(|segment| segment.old_range.end <= old_range.start),
            "dispatch splice segments must be disjoint, ordered and below the source anchor"
        );
        let new_start = self.nodes.len();
        let mut contains_focus = false;
        let source_nodes = &source.nodes[old_range.clone()];
        self.nodes.reserve(source_nodes.len());
        for source_node in source_nodes {
            let parent = source_node
                .parent
                .and_then(|source_parent| {
                    if source_parent.is_in(&old_range) {
                        Some(DispatchNodeId(
                            new_start + (source_parent.0 - old_range.start),
                        ))
                    } else {
                        splice.target_for(source_parent)
                    }
                })
                .expect("dispatch splice must retain each source parent");
            let enclosing_view = source_node.view_id.and_then(|_| {
                std::iter::successors(Some(parent), |&id| self.node(id).parent)
                    .find_map(|id| self.node(id).view_id)
            });
            contains_focus |= focus.is_some() && source_node.focus_id == focus;
            self.append_replayed_node(source_node, Some(parent), enclosing_view, validity.clone());
        }
        if self.active_node_id() != Some(splice.target_anchor) {
            self.set_active_node(splice.target_anchor);
        }
        let reused = ReusedSubtree {
            old_range,
            new_range: new_start..self.nodes.len(),
            contains_focus,
        };
        if !reused.old_range.is_empty() {
            splice.segments.push(reused.clone());
        }
        reused
    }

    pub fn truncate(&mut self, index: usize) {
        for node in &self.nodes[index..] {
            if let Some(focus_id) = node.focus_id {
                self.focusable_node_ids.remove(&focus_id);
            }

            if let Some(view_id) = node.view_id {
                self.view_node_ids.remove(&view_id);
            }
        }
        self.nodes.truncate(index);
    }

    pub fn suppress_invalid_nodes(&mut self) {
        for (index, node) in self.nodes.iter_mut().enumerate() {
            if node
                .validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
            {
                continue;
            }
            let node_id = DispatchNodeId(index);
            if let Some(focus_id) = node.focus_id
                && self.focusable_node_ids.get(&focus_id) == Some(&node_id)
            {
                self.focusable_node_ids.remove(&focus_id);
            }
            if let Some(view_id) = node.view_id
                && self.view_node_ids.get(&view_id) == Some(&node_id)
            {
                self.view_node_ids.remove(&view_id);
            }
            node.key_listeners.clear();
            node.action_listeners.clear();
            node.modifiers_changed_listeners.clear();
        }
    }

    pub fn on_key_event(&mut self, listener: KeyListener) {
        self.active_node().key_listeners.push(listener);
    }

    pub fn on_modifiers_changed(&mut self, listener: ModifiersChangedListener) {
        self.active_node()
            .modifiers_changed_listeners
            .push(listener);
    }

    pub fn on_action(
        &mut self,
        action_type: TypeId,
        listener: Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App)>,
    ) {
        self.active_node()
            .action_listeners
            .push(DispatchActionListener {
                action_type,
                listener,
            });
    }

    pub fn focus_contains(&self, parent: FocusId, child: FocusId) -> bool {
        if parent == child {
            return true;
        }

        if let Some(parent_node_id) = self.focusable_node_ids.get(&parent) {
            let mut current_node_id = self.focusable_node_ids.get(&child).copied();
            while let Some(node_id) = current_node_id {
                if node_id == *parent_node_id {
                    return true;
                }
                current_node_id = self.nodes[node_id.0].parent;
            }
        }
        false
    }

    pub fn available_actions(&self, target: DispatchNodeId) -> Vec<Box<dyn Action>> {
        let mut actions = Vec::<Box<dyn Action>>::new();
        for node_id in self.dispatch_path(target) {
            let node = &self.nodes[node_id.0];
            for DispatchActionListener { action_type, .. } in &node.action_listeners {
                if let Err(ix) = actions.binary_search_by_key(action_type, |a| a.as_any().type_id())
                {
                    // Intentionally silence these errors without logging.
                    // If an action cannot be built by default, it's not available.
                    let action = self.action_registry.build_action_type(action_type).ok();
                    if let Some(action) = action {
                        actions.insert(ix, action);
                    }
                }
            }
        }
        actions
    }

    pub fn is_action_available(&self, action: &dyn Action, target: DispatchNodeId) -> bool {
        for node_id in self.dispatch_path(target) {
            let node = &self.nodes[node_id.0];
            if node
                .action_listeners
                .iter()
                .any(|listener| listener.action_type == action.as_any().type_id())
            {
                return true;
            }
        }
        false
    }

    /// Returns key bindings that invoke an action on the currently focused element. Bindings are
    /// returned in the order they were added. For display, the last binding should take precedence.
    ///
    /// Bindings are only included if they are the highest precedence match for their keystrokes, so
    /// shadowed bindings are not included.
    pub fn bindings_for_action(
        &self,
        action: &dyn Action,
        context_stack: &[KeyContext],
    ) -> Vec<KeyBinding> {
        // Ideally this would return a `DoubleEndedIterator` to avoid `highest_precedence_*`
        // methods, but this can't be done very cleanly since keymap must be borrowed.
        let keymap = self.keymap.borrow();
        keymap
            .bindings_for_action(action)
            .filter(|binding| {
                Self::binding_matches_predicate_and_not_shadowed(&keymap, binding, context_stack)
            })
            .cloned()
            .collect()
    }

    /// Returns the highest precedence binding for the given action and context stack. This is the
    /// same as the last result of `bindings_for_action`, but more efficient than getting all bindings.
    pub fn highest_precedence_binding_for_action(
        &self,
        action: &dyn Action,
        context_stack: &[KeyContext],
    ) -> Option<KeyBinding> {
        let keymap = self.keymap.borrow();
        keymap
            .bindings_for_action(action)
            .rev()
            .find(|binding| {
                Self::binding_matches_predicate_and_not_shadowed(&keymap, binding, context_stack)
            })
            .cloned()
    }

    fn binding_matches_predicate_and_not_shadowed(
        keymap: &Keymap,
        binding: &KeyBinding,
        context_stack: &[KeyContext],
    ) -> bool {
        let (bindings, _) = keymap.bindings_for_input(&binding.keystrokes, context_stack);
        if let Some(found) = bindings.iter().next() {
            found.action.partial_eq(binding.action.as_ref())
        } else {
            false
        }
    }

    fn bindings_for_input(
        &self,
        input: &[Keystroke],
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> (SmallVec<[KeyBinding; 1]>, bool, Vec<KeyContext>) {
        let context_stack: Vec<KeyContext> = dispatch_path
            .iter()
            .filter_map(|node_id| self.node(*node_id).context.clone())
            .collect();

        let (bindings, partial) = self
            .keymap
            .borrow()
            .bindings_for_input(input, &context_stack);
        (bindings, partial, context_stack)
    }

    /// Find the bindings that can follow the current input sequence.
    pub fn possible_next_bindings_for_input(
        &self,
        input: &[Keystroke],
        context_stack: &[KeyContext],
    ) -> Vec<KeyBinding> {
        self.keymap
            .borrow()
            .possible_next_bindings_for_input(input, context_stack)
    }

    /// dispatch_key processes the keystroke
    /// input should be set to the value of `pending` from the previous call to dispatch_key.
    /// This returns three instructions to the input handler:
    /// - bindings: any bindings to execute before processing this keystroke
    /// - pending: the new set of pending keystrokes to store
    /// - to_replay: any keystroke that had been pushed to pending, but are no-longer matched,
    ///   these should be replayed first.
    pub fn dispatch_key(
        &mut self,
        mut input: SmallVec<[Keystroke; 1]>,
        keystroke: Keystroke,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> DispatchResult {
        input.push(keystroke.clone());
        let (bindings, pending, context_stack) = self.bindings_for_input(&input, dispatch_path);

        if pending {
            return DispatchResult {
                pending: input,
                pending_has_binding: !bindings.is_empty(),
                context_stack,
                ..Default::default()
            };
        } else if !bindings.is_empty() {
            return DispatchResult {
                bindings,
                context_stack,
                ..Default::default()
            };
        } else if input.len() == 1 {
            return DispatchResult {
                context_stack,
                ..Default::default()
            };
        }
        input.pop();

        let (suffix, mut to_replay) = self.replay_prefix(input, dispatch_path);

        let mut result = self.dispatch_key(suffix, keystroke, dispatch_path);
        to_replay.extend(result.to_replay);
        result.to_replay = to_replay;
        result
    }

    /// If the user types a matching prefix of a binding and then waits for a timeout
    /// flush_dispatch() converts any previously pending input to replay events.
    pub fn flush_dispatch(
        &mut self,
        input: SmallVec<[Keystroke; 1]>,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> SmallVec<[Replay; 1]> {
        let (suffix, mut to_replay) = self.replay_prefix(input, dispatch_path);

        if !suffix.is_empty() {
            to_replay.extend(self.flush_dispatch(suffix, dispatch_path))
        }

        to_replay
    }

    /// Converts the longest prefix of input to a replay event and returns the rest.
    fn replay_prefix(
        &self,
        mut input: SmallVec<[Keystroke; 1]>,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> (SmallVec<[Keystroke; 1]>, SmallVec<[Replay; 1]>) {
        let mut to_replay: SmallVec<[Replay; 1]> = Default::default();
        for last in (0..input.len()).rev() {
            let (bindings, _, _) = self.bindings_for_input(&input[0..=last], dispatch_path);
            if !bindings.is_empty() {
                to_replay.push(Replay {
                    keystroke: input.drain(0..=last).next_back().unwrap(),
                    bindings,
                });
                break;
            }
        }
        if to_replay.is_empty() {
            to_replay.push(Replay {
                keystroke: input.remove(0),
                ..Default::default()
            });
        }
        (input, to_replay)
    }

    pub fn dispatch_path(&self, target: DispatchNodeId) -> SmallVec<[DispatchNodeId; 32]> {
        let mut dispatch_path: SmallVec<[DispatchNodeId; 32]> = SmallVec::new();
        let mut current_node_id = Some(target);
        while let Some(node_id) = current_node_id {
            dispatch_path.push(node_id);
            current_node_id = self.nodes.get(node_id.0).and_then(|node| node.parent);
        }
        dispatch_path.reverse(); // Reverse the path so it goes from the root to the focused node.
        dispatch_path
    }

    pub fn focus_path(&self, focus_id: FocusId) -> SmallVec<[FocusId; 8]> {
        let mut focus_path: SmallVec<[FocusId; 8]> = SmallVec::new();
        let mut current_node_id = self.focusable_node_ids.get(&focus_id).copied();
        while let Some(node_id) = current_node_id {
            let node = self.node(node_id);
            if let Some(focus_id) = node.focus_id {
                focus_path.push(focus_id);
            }
            current_node_id = node.parent;
        }
        focus_path.reverse(); // Reverse the path so it goes from the root to the focused node.
        focus_path
    }

    pub fn view_path_reversed(&self, view_id: EntityId) -> impl Iterator<Item = EntityId> {
        let mut current_node_id = self.view_node_ids.get(&view_id).copied();

        std::iter::successors(
            current_node_id.map(|node_id| self.node(node_id)),
            |node_id| Some(self.node(node_id.parent?)),
        )
        .filter_map(|node| node.view_id)
    }

    pub fn node(&self, node_id: DispatchNodeId) -> &DispatchNode {
        &self.nodes[node_id.0]
    }

    fn active_node(&mut self) -> &mut DispatchNode {
        let active_node_id = self.active_node_id().unwrap();
        &mut self.nodes[active_node_id.0]
    }

    pub fn focusable_node_id(&self, target: FocusId) -> Option<DispatchNodeId> {
        self.focusable_node_ids.get(&target).copied()
    }

    pub fn valid_focusable_node_id(&self, target: FocusId) -> Option<DispatchNodeId> {
        self.focusable_node_id(target).filter(|node_id| {
            self.nodes[node_id.0]
                .validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
        })
    }

    pub fn root_node_id(&self) -> DispatchNodeId {
        debug_assert!(!self.nodes.is_empty());
        DispatchNodeId(0)
    }

    pub fn active_node_id(&self) -> Option<DispatchNodeId> {
        self.node_stack.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::DispatchSplice;
    use crate::{
        AppContext, DispatchResult, Element, ElementId, GlobalElementId, InspectorElementId,
        InteractiveElement as _, Keystroke, LayoutId, ParentElement as _, Style, div,
    };
    use core::panic;
    use slotmap::SlotMap;
    use smallvec::SmallVec;
    use std::{any::TypeId, cell::RefCell, ops::Range, rc::Rc};

    use crate::{
        ActionRegistry, App, Bounds, Context, DispatchTree, FocusHandle, InputHandler, IntoElement,
        KeyBinding, KeyContext, Keymap, Pixels, Point, Render, Subscription, TestAppContext,
        UTF16Selection, Unbind, Window,
    };

    actions!(dispatch_test, [TestAction, SecondaryTestAction]);

    fn test_dispatch_tree(bindings: Vec<KeyBinding>) -> DispatchTree {
        let registry = ActionRegistry::default();

        DispatchTree::new(
            Rc::new(RefCell::new(Keymap::new(bindings))),
            Rc::new(registry),
        )
    }

    #[test]
    fn segmented_replay_reconnects_wrappers_around_a_fresh_interior() {
        let mut focus_ids = SlotMap::<crate::FocusId, ()>::with_key();
        let anchor_focus = focus_ids.insert(());
        let wrapper_focus = focus_ids.insert(());
        let old_focus = focus_ids.insert(());
        let fresh_focus = focus_ids.insert(());
        let tail_focus = focus_ids.insert(());
        let mut view_ids = SlotMap::<crate::EntityId, ()>::with_key();
        let anchor_view = view_ids.insert(());
        let wrapper_view = view_ids.insert(());

        let mut source = test_dispatch_tree(vec![]);
        let source_anchor = source.push_node_scoped(None);
        let prefix_start = source.len();
        let outer = source.push_node_scoped(None);
        source.set_key_context(KeyContext::parse("Outer").unwrap());
        source.on_action(TypeId::of::<TestAction>(), Rc::new(|_, _, _, _| {}));
        let wrapper = source.push_node_scoped(None);
        source.set_key_context(KeyContext::parse("Wrapper").unwrap());
        source.set_focus_id(wrapper_focus);
        source.set_view_id(wrapper_view);
        let prefix_end = source.len();
        let omitted = source.push_node_scoped(None);
        source.set_focus_id(old_focus);
        let omitted_child = source.push_node_scoped(None);
        source.pop_node();
        source.pop_node();
        let suffix_start = source.len();
        let tail = source.push_node_scoped(None);
        source.set_key_context(KeyContext::parse("Tail").unwrap());
        source.set_focus_id(tail_focus);
        source.on_key_event(Rc::new(|_, _, _, _| {}));
        source.on_modifiers_changed(Rc::new(|_, _, _| {}));
        source.pop_node();
        source.pop_node();
        source.pop_node();
        let adjacent = source.push_node_scoped(None);
        source.pop_node();

        let mut target = test_dispatch_tree(vec![]);
        let root = target.push_node_scoped(None);
        target.push_node_scoped(None);
        target.pop_node();
        let target_anchor = target.push_node_scoped(None);
        target.set_key_context(KeyContext::parse("Anchor").unwrap());
        target.set_focus_id(anchor_focus);
        target.set_view_id(anchor_view);
        let mut splice = DispatchSplice::new(source_anchor, target_anchor);
        let prefix = target.reuse_segment(
            prefix_start..prefix_end,
            &source,
            Some(tail_focus),
            None,
            &mut splice,
        );
        assert!(!prefix.contains_focus());
        assert_eq!(target.active_node_id(), Some(target_anchor));
        let target_wrapper = splice.target_for(wrapper).unwrap();
        assert_eq!(target_wrapper, prefix.refresh_node_id(wrapper));
        target.set_active_node(target_wrapper);
        let fresh = target.push_node_scoped(None);
        target.set_focus_id(fresh_focus);
        target.pop_node();

        let suffix = target.reuse_segment(
            suffix_start..source.len(),
            &source,
            Some(tail_focus),
            None,
            &mut splice,
        );
        let target_tail = splice.target_for(tail).unwrap();
        let target_outer = splice.target_for(outer).unwrap();
        let target_adjacent = splice.target_for(adjacent).unwrap();
        assert!(suffix.contains_focus());
        assert_eq!(target_tail, suffix.refresh_node_id(tail));
        assert_eq!(splice.target_for(source_anchor), Some(target_anchor));
        assert_eq!(splice.target_for(omitted), None);
        assert_eq!(splice.target_for(omitted_child), None);
        assert_eq!(target.focusable_node_id(old_focus), None);
        assert_eq!(target.focusable_node_id(fresh_focus), Some(fresh));
        assert_eq!(target.node(fresh).parent, Some(target_wrapper));
        assert_eq!(target.node(target_adjacent).parent, Some(target_anchor));
        assert_eq!(target.active_node_id(), Some(target_anchor));
        assert_eq!(target.context_stack, [KeyContext::parse("Anchor").unwrap()]);
        assert_eq!(target.view_stack, [anchor_view]);
        assert_eq!(target.view_node_ids[&wrapper_view], target_wrapper);
        assert_eq!(
            target.focus_path(tail_focus).as_slice(),
            &[anchor_focus, wrapper_focus, tail_focus]
        );
        assert_eq!(
            target.dispatch_path(target_tail).as_slice(),
            &[
                root,
                target_anchor,
                target_outer,
                target_wrapper,
                target_tail
            ]
        );
        assert!(target.is_action_available(&TestAction, target_tail));
        assert!(Rc::ptr_eq(
            &target.node(target_tail).key_listeners[0],
            &source.node(tail).key_listeners[0],
        ));
        assert!(Rc::ptr_eq(
            &target.node(target_tail).modifiers_changed_listeners[0],
            &source.node(tail).modifiers_changed_listeners[0],
        ));

        target.set_active_node(target_wrapper);
        let count = target.len();
        let empty =
            target.reuse_segment(source.len()..source.len(), &source, None, None, &mut splice);
        assert!(!empty.contains_focus());
        assert_eq!(target.len(), count);
        assert_eq!(target.active_node_id(), Some(target_anchor));
    }

    #[test]
    fn segmented_replay_rebinds_geometry_and_uses_destination_view_ancestry() {
        use crate::geometry::{SubtreeGeometryError, SubtreeGeometryValidity};

        let mut view_ids = SlotMap::<crate::EntityId, ()>::with_key();
        let view = view_ids.insert(());
        let mut focus_ids = SlotMap::<crate::FocusId, ()>::with_key();
        let rejected_focus = focus_ids.insert(());
        let accepted_focus = focus_ids.insert(());
        let mut source = test_dispatch_tree(vec![]);
        let source_anchor = source.push_node_scoped(None);
        let start = source.len();
        let source_validity = SubtreeGeometryValidity::new(None);
        let rejected = source.push_node_scoped(Some(source_validity.clone()));
        source.set_focus_id(rejected_focus);
        source.set_view_id(view);
        source_validity.invalidate(SubtreeGeometryError::DeviceConversion);
        source.pop_node();
        let accepted = source.push_node_scoped(None);
        source.set_focus_id(accepted_focus);
        source.set_view_id(view);
        source.pop_node();

        let mut target = test_dispatch_tree(vec![]);
        let target_anchor = target.push_node_scoped(None);
        target.set_view_id(view);
        let target_validity = SubtreeGeometryValidity::new(None);
        let mut splice = DispatchSplice::new(source_anchor, target_anchor);
        target.reuse_segment(
            start..source.len(),
            &source,
            None,
            Some(target_validity.clone()),
            &mut splice,
        );
        let target_rejected = splice.target_for(rejected).unwrap();
        let target_accepted = splice.target_for(accepted).unwrap();
        assert_eq!(target.node(target_rejected).view_id, None);
        assert_eq!(target.node(target_accepted).view_id, None);
        assert_eq!(target.view_node_ids[&view], target_anchor);
        assert_eq!(target.valid_focusable_node_id(rejected_focus), None);
        assert_eq!(
            target.valid_focusable_node_id(accepted_focus),
            Some(target_accepted)
        );
        target_validity.invalidate(SubtreeGeometryError::DeviceConversion);
        assert_eq!(target.valid_focusable_node_id(accepted_focus), None);
        target.suppress_invalid_nodes();
        assert_eq!(target.focusable_node_id(rejected_focus), None);
        assert_eq!(target.focusable_node_id(accepted_focus), None);
        assert_eq!(
            source.valid_focusable_node_id(accepted_focus),
            Some(accepted)
        );
    }

    #[test]
    fn test_keybinding_for_action_bounds() {
        let tree = test_dispatch_tree(vec![KeyBinding::new(
            "cmd-n",
            TestAction,
            Some("ProjectPanel"),
        )]);

        let contexts = vec![
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("ProjectPanel").unwrap(),
        ];

        let keybinding = tree.bindings_for_action(&TestAction, &contexts);

        assert!(keybinding[0].action.partial_eq(&TestAction))
    }

    #[test]
    fn test_bindings_for_action_hides_targeted_unbind_in_active_context() {
        let tree = test_dispatch_tree(vec![
            KeyBinding::new("tab", TestAction, Some("Editor")),
            KeyBinding::new(
                "tab",
                Unbind("dispatch_test::TestAction".into()),
                Some("Editor && edit_prediction"),
            ),
            KeyBinding::new(
                "tab",
                SecondaryTestAction,
                Some("Editor && showing_completions"),
            ),
        ]);

        let contexts = vec![
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("Editor showing_completions edit_prediction").unwrap(),
        ];

        let bindings = tree.bindings_for_action(&TestAction, &contexts);
        assert!(bindings.is_empty());

        let highest = tree.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(highest.is_none());

        let fallback_bindings = tree.bindings_for_action(&SecondaryTestAction, &contexts);
        assert_eq!(fallback_bindings.len(), 1);
        assert!(fallback_bindings[0].action.partial_eq(&SecondaryTestAction));
    }

    #[test]
    fn test_bindings_for_action_keeps_targeted_binding_outside_unbind_context() {
        let tree = test_dispatch_tree(vec![
            KeyBinding::new("tab", TestAction, Some("Editor")),
            KeyBinding::new(
                "tab",
                Unbind("dispatch_test::TestAction".into()),
                Some("Editor && edit_prediction"),
            ),
            KeyBinding::new(
                "tab",
                SecondaryTestAction,
                Some("Editor && showing_completions"),
            ),
        ]);

        let contexts = vec![
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("Editor").unwrap(),
        ];

        let bindings = tree.bindings_for_action(&TestAction, &contexts);
        assert_eq!(bindings.len(), 1);
        assert!(bindings[0].action.partial_eq(&TestAction));

        let highest = tree.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(highest.is_some_and(|binding| binding.action.partial_eq(&TestAction)));
    }

    /// Models the picker preview footer scenario: a picker action is bound in
    /// `Picker > Editor`, but a base keymap binds the same chord to an editor
    /// action in `Editor`. `Picker > Editor` and `Editor` resolve at the same
    /// context depth, so at equal depth precedence is decided purely by load
    /// order (later wins). Because base keymaps load after the default keymap,
    /// the picker binding is shadowed unless it is (re)bound by an overlay that
    /// loads after the base keymap - which is exactly what
    /// `keymaps/specific-overrides*.json` does.
    #[test]
    fn test_overlay_after_base_restores_shadowed_picker_binding() {
        // SecondaryTestAction stands in for the editor/base action (e.g.
        // editor::AddSelectionBelow), TestAction for the picker action.
        let contexts = vec![
            KeyContext::parse("Picker").unwrap(),
            KeyContext::parse("Editor").unwrap(),
        ];

        // Default keymap (picker binding) followed by a base keymap that binds
        // the same chord to an editor action: the base binding wins and the
        // picker action is shadowed, so its footer tooltip renders no shortcut.
        let shadowed = test_dispatch_tree(vec![
            KeyBinding::new("ctrl-alt-down", TestAction, Some("Picker > Editor")),
            KeyBinding::new("ctrl-alt-down", SecondaryTestAction, Some("Editor")),
        ]);
        let highest = shadowed.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(
            highest.is_none(),
            "picker binding should be shadowed by the base editor binding"
        );

        // Re-binding the picker action in an overlay loaded after the base keymap
        // restores it as the resolved binding.
        let fixed = test_dispatch_tree(vec![
            KeyBinding::new("ctrl-alt-down", TestAction, Some("Picker > Editor")),
            KeyBinding::new("ctrl-alt-down", SecondaryTestAction, Some("Editor")),
            // overlay loaded last:
            KeyBinding::new("ctrl-alt-down", TestAction, Some("Picker > Editor")),
        ]);
        let highest = fixed.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(
            highest.is_some_and(|binding| binding.action.partial_eq(&TestAction)),
            "overlay loaded after base should restore the picker binding"
        );

        // Conversely, putting the override in the default keymap (i.e. before the
        // base keymap) does NOT help: the later base binding still wins at equal
        // depth. This is why the overlay must be loaded after the base keymap.
        let override_before_base = test_dispatch_tree(vec![
            KeyBinding::new("ctrl-alt-down", TestAction, Some("Picker > Editor")),
            KeyBinding::new("ctrl-alt-down", TestAction, Some("Picker > Editor")),
            KeyBinding::new("ctrl-alt-down", SecondaryTestAction, Some("Editor")),
        ]);
        let highest =
            override_before_base.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(
            highest.is_none(),
            "an override loaded before the base binding cannot win the equal-depth tie"
        );
    }

    #[test]
    fn test_pending_has_binding_state() {
        let bindings = vec![
            KeyBinding::new("ctrl-b h", TestAction, None),
            KeyBinding::new("space", TestAction, Some("ContextA")),
            KeyBinding::new("space f g", TestAction, Some("ContextB")),
        ];
        let mut tree = test_dispatch_tree(bindings);

        type DispatchPath = SmallVec<[super::DispatchNodeId; 32]>;
        fn dispatch(
            tree: &mut DispatchTree,
            pending: SmallVec<[Keystroke; 1]>,
            key: &'static str,
            path: &DispatchPath,
        ) -> DispatchResult {
            tree.dispatch_key(pending, Keystroke::parse(key).unwrap(), path)
        }

        let dispatch_path: DispatchPath = SmallVec::new();
        let result = dispatch(&mut tree, SmallVec::new(), "ctrl-b", &dispatch_path);
        assert_eq!(result.pending.len(), 1);
        assert!(!result.pending_has_binding);

        let result = dispatch(&mut tree, result.pending, "h", &dispatch_path);
        assert_eq!(result.pending.len(), 0);
        assert_eq!(result.bindings.len(), 1);
        assert!(!result.pending_has_binding);

        let node_id = tree.push_node_scoped(None);
        tree.set_key_context(KeyContext::parse("ContextB").unwrap());
        tree.pop_node();

        let dispatch_path = tree.dispatch_path(node_id);
        let result = dispatch(&mut tree, SmallVec::new(), "space", &dispatch_path);

        assert_eq!(result.pending.len(), 1);
        assert!(!result.pending_has_binding);
    }

    #[crate::test]
    fn test_pending_input_observers_notified_on_focus_change(cx: &mut TestAppContext) {
        #[derive(Clone)]
        struct CustomElement {
            focus_handle: FocusHandle,
            sibling_focus_handle: FocusHandle,
            text: Rc<RefCell<String>>,
        }

        impl CustomElement {
            fn new(cx: &mut Context<Self>) -> Self {
                Self {
                    focus_handle: cx.focus_handle(),
                    sibling_focus_handle: cx.focus_handle(),
                    text: Rc::default(),
                }
            }
        }

        impl Element for CustomElement {
            type RequestLayoutState = ();

            type PrepaintState = ();

            fn id(&self) -> Option<ElementId> {
                Some("custom".into())
            }

            fn source_location(&self) -> Option<&'static panic::Location<'static>> {
                None
            }

            fn request_layout(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                window: &mut Window,
                cx: &mut App,
            ) -> (LayoutId, Self::RequestLayoutState) {
                (window.request_layout(Style::default(), [], cx), ())
            }

            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                window: &mut Window,
                cx: &mut App,
            ) -> Self::PrepaintState {
                window.set_focus_handle(&self.focus_handle, cx);
            }

            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                _: &mut Self::PrepaintState,
                window: &mut Window,
                cx: &mut App,
            ) {
                let mut key_context = KeyContext::default();
                key_context.add("Terminal");
                window.set_key_context(key_context);
                window.handle_input(&self.focus_handle, self.clone(), cx);
                window.on_action(std::any::TypeId::of::<TestAction>(), |_, _, _, _| {});
            }
        }

        impl InputHandler for CustomElement {
            fn selected_text_range(
                &mut self,
                _: bool,
                _: &mut Window,
                _: &mut App,
            ) -> Option<UTF16Selection> {
                None
            }

            fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
                None
            }

            fn text_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<String> {
                None
            }

            fn replace_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                text: &str,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(text)
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                new_text: &str,
                _: Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(new_text)
            }

            fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

            fn bounds_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<Bounds<Pixels>> {
                None
            }

            fn character_index_for_point(
                &mut self,
                _: Point<Pixels>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<usize> {
                None
            }
        }

        impl Render for CustomElement {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .child(self.clone())
                    .child(div().track_focus(&self.sibling_focus_handle))
            }
        }

        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("ctrl-b", TestAction, Some("Terminal"))]);
            cx.bind_keys([KeyBinding::new("ctrl-b h", TestAction, Some("Terminal"))]);
        });

        let (test, cx) = cx.add_window_view(|_, cx| CustomElement::new(cx));
        let (focus_handle, sibling_focus_handle) = test.update(cx, |test, _| {
            (test.focus_handle.clone(), test.sibling_focus_handle.clone())
        });

        let pending_input_changed_count = Rc::new(RefCell::new(0usize));
        let pending_input_changed_count_for_observer = pending_input_changed_count.clone();

        struct PendingInputObserver {
            _subscription: Subscription,
        }

        let _observer = cx.update(|window, cx| {
            cx.new(|cx| PendingInputObserver {
                _subscription: cx.observe_pending_input(window, move |_, _, _| {
                    *pending_input_changed_count_for_observer.borrow_mut() += 1;
                }),
            })
        });

        cx.update(|window, cx| {
            window.focus(&focus_handle, cx);
            let _ = window.activate_window();
        });

        cx.simulate_keystrokes("ctrl-b");

        let count_after_pending = Rc::new(RefCell::new(0usize));
        let count_after_pending_for_assertion = count_after_pending.clone();

        cx.update(|window, cx| {
            assert!(window.has_pending_keystrokes());
            *count_after_pending.borrow_mut() = *pending_input_changed_count.borrow();
            assert!(*count_after_pending.borrow() > 0);

            window.focus(&cx.focus_handle(), cx);
            assert!(
                window.has_pending_keystrokes(),
                "an unqualified focus claim must not clear the active chord"
            );

            window.focus(&sibling_focus_handle, cx);

            assert!(!window.has_pending_keystrokes());
        });

        // Focus-triggered pending-input notifications are deferred to the end of the current
        // effect cycle, so the observer callback should run after the focus update completes.
        cx.update(|_, _| {
            let count_after_focus_change = *pending_input_changed_count.borrow();
            assert!(count_after_focus_change > *count_after_pending_for_assertion.borrow());
        });
    }

    #[crate::test]
    fn test_input_handler_pending(cx: &mut TestAppContext) {
        #[derive(Clone)]
        struct CustomElement {
            focus_handle: FocusHandle,
            text: Rc<RefCell<String>>,
        }
        impl CustomElement {
            fn new(cx: &mut Context<Self>) -> Self {
                Self {
                    focus_handle: cx.focus_handle(),
                    text: Rc::default(),
                }
            }
        }
        impl Element for CustomElement {
            type RequestLayoutState = ();

            type PrepaintState = ();

            fn id(&self) -> Option<ElementId> {
                Some("custom".into())
            }
            fn source_location(&self) -> Option<&'static panic::Location<'static>> {
                None
            }
            fn request_layout(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                window: &mut Window,
                cx: &mut App,
            ) -> (LayoutId, Self::RequestLayoutState) {
                (window.request_layout(Style::default(), [], cx), ())
            }
            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                window: &mut Window,
                cx: &mut App,
            ) -> Self::PrepaintState {
                window.set_focus_handle(&self.focus_handle, cx);
            }
            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                _: &mut Self::PrepaintState,
                window: &mut Window,
                cx: &mut App,
            ) {
                let mut key_context = KeyContext::default();
                key_context.add("Terminal");
                window.set_key_context(key_context);
                window.handle_input(&self.focus_handle, self.clone(), cx);
                window.on_action(std::any::TypeId::of::<TestAction>(), |_, _, _, _| {});
            }
        }
        impl InputHandler for CustomElement {
            fn selected_text_range(
                &mut self,
                _: bool,
                _: &mut Window,
                _: &mut App,
            ) -> Option<UTF16Selection> {
                None
            }

            fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
                None
            }

            fn text_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<String> {
                None
            }

            fn replace_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                text: &str,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(text)
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                new_text: &str,
                _: Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(new_text)
            }

            fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

            fn bounds_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<Bounds<Pixels>> {
                None
            }

            fn character_index_for_point(
                &mut self,
                _: Point<Pixels>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<usize> {
                None
            }
        }
        impl Render for CustomElement {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.clone()
            }
        }

        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("ctrl-b", TestAction, Some("Terminal"))]);
            cx.bind_keys([KeyBinding::new("ctrl-b h", TestAction, Some("Terminal"))]);
        });
        let (test, cx) = cx.add_window_view(|_, cx| CustomElement::new(cx));
        let focus_handle = test.update(cx, |test, _| test.focus_handle.clone());
        cx.update(|window, cx| {
            window.focus(&focus_handle, cx);
            let _ = window.activate_window();
        });
        cx.simulate_keystrokes("ctrl-b [");
        test.update(cx, |test, _| assert_eq!(test.text.borrow().as_str(), "["))
    }
}
