use super::layout_key::LayoutKey;
use crate::{App, AvailableSpace, LayoutId, Pixels, Size, Window, collections::FxHashMap};
use std::any::{Any, TypeId};
use taffy::{TaffyTree, TraversePartialTree as _};

#[derive(Default)]
pub(crate) struct LayoutRetention {
    active: bool,
    nodes: FxHashMap<LayoutId, RetainedNode>,
    keyed: FxHashMap<LayoutKey, LayoutId>,
    subtree: Vec<LayoutId>,
}

struct RetainedNode {
    key: Option<LayoutKey>,
    claim: Option<Claim>,
    intact: bool,
    computed_root: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Claim {
    Request,
    Subtree(LayoutId),
}

impl LayoutRetention {
    pub(crate) fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn begin_frame(&mut self) {
        self.active = true;
    }

    pub(crate) fn finish_frame(&mut self, tree: &mut TaffyTree<NodeContext>) {
        self.active = false;
        self.nodes.retain(|id, node| {
            if node.claim.take().is_some() {
                if let Some(context) = tree.get_node_context_mut((*id).into()) {
                    context.release_callbacks();
                }
                true
            } else {
                if let Some(key) = node.key {
                    self.keyed.remove(&key);
                }
                tree.remove((*id).into())
                    .expect("unclaimed layout node belongs to this tree");
                false
            }
        });
    }

    pub(crate) fn clear(&mut self) {
        self.active = false;
        self.nodes.clear();
        self.keyed.clear();
        self.subtree.clear();
    }

    pub(crate) fn claim_request(&mut self, key: Option<LayoutKey>) -> Option<LayoutId> {
        if !self.active {
            return None;
        }
        let id = *self.keyed.get(&key?)?;
        let node = self.nodes.get_mut(&id)?;
        if node.claim.is_some() {
            return None;
        }
        node.claim = Some(Claim::Request);
        node.intact = true;
        node.computed_root = false;
        Some(id)
    }

    pub(crate) fn record_request(&mut self, key: Option<LayoutKey>, id: LayoutId) {
        if !self.active {
            return;
        }
        let key = key.filter(|key| *self.keyed.entry(*key).or_insert(id) == id);
        self.nodes.insert(
            id,
            RetainedNode {
                key,
                claim: Some(Claim::Request),
                intact: true,
                computed_root: false,
            },
        );
    }

    pub(crate) fn has_request(&self, id: LayoutId) -> bool {
        self.nodes.get(&id).is_some_and(|node| node.claim.is_some())
    }

    pub(crate) fn contains(&self, id: LayoutId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub(crate) fn invalidate_subtree(&mut self, id: LayoutId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.intact = false;
        }
    }

    pub(crate) fn record_computed_root(&mut self, id: LayoutId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.computed_root = true;
        }
    }

    pub(crate) fn retained_tree_root(
        &self,
        tree: &TaffyTree<NodeContext>,
        mut id: LayoutId,
    ) -> Option<LayoutId> {
        if !self.active {
            return None;
        }
        // Bound the walk by the number of tracked nodes in case of cyclic parent links.
        for _ in 0..self.nodes.len() {
            let node = self.claimable_node(tree, id)?;
            if let Some(parent) = tree.parent(id.into()) {
                id = parent.into();
            } else {
                return (node.computed_root && matches!(tree.dirty(id.into()), Ok(false)))
                    .then_some(id);
            }
        }
        None
    }

    fn claimable_node(&self, tree: &TaffyTree<NodeContext>, id: LayoutId) -> Option<&RetainedNode> {
        self.nodes.get(&id).filter(|node| {
            node.claim.is_none()
                && node.intact
                && !matches!(
                    tree.get_node_context(id.into()),
                    Some(NodeContext::Dynamic(_))
                )
        })
    }

    pub(crate) fn claim_subtree_except(
        &mut self,
        tree: &TaffyTree<NodeContext>,
        root: LayoutId,
        excluded: &[LayoutId],
    ) -> bool {
        if excluded.contains(&root) || self.retained_tree_root(tree, root) != Some(root) {
            return false;
        }

        self.subtree.clear();
        self.subtree.push(root);
        let mut index = 0;
        let mut skipped = 0;
        while let Some(&id) = self.subtree.get(index) {
            if excluded.contains(&id) {
                self.subtree.swap_remove(index);
                skipped += 1;
                continue;
            }
            if self.claimable_node(tree, id).is_none() {
                return false;
            }
            self.subtree
                .extend(tree.child_ids(id.into()).map(LayoutId::from));
            index += 1;
        }
        if skipped != excluded.len() {
            return false;
        }
        for id in self.subtree.drain(..) {
            self.nodes
                .get_mut(&id)
                .expect("validated subtree node")
                .claim = Some(Claim::Subtree(root));
        }
        true
    }

    pub(crate) fn release_subtree_claims(&mut self, root: LayoutId) {
        for node in self.nodes.values_mut() {
            if node.claim == Some(Claim::Subtree(root)) {
                node.claim = None;
            }
        }
    }
}

#[cfg(feature = "stacker")]
use stacksafe::StackSafe;
#[cfg(not(feature = "stacker"))]
type StackSafe<T> = T;

type MeasureFn =
    dyn FnMut(Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App) -> Size<Pixels>;
type CacheMeasureFn =
    dyn FnMut(MeasureConstraints, &mut Window, &mut App) -> (Size<Pixels>, Box<dyn Any>);
type PublishMeasureFn = dyn Fn(&dyn Any);
type CompareInputs = fn(&dyn Any, &dyn Any) -> bool;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct MeasureConstraints {
    pub(crate) known: Size<Option<Pixels>>,
    pub(crate) available: Size<AvailableSpace>,
    pub(crate) scale: f32,
}

pub(crate) enum NodeContext {
    Dynamic(Option<StackSafe<Box<MeasureFn>>>),
    Cached(Box<CachedMeasure>),
}

impl NodeContext {
    pub(crate) fn dynamic(
        measure: impl FnMut(
            Size<Option<Pixels>>,
            Size<AvailableSpace>,
            &mut Window,
            &mut App,
        ) -> Size<Pixels>
        + 'static,
    ) -> Self {
        let measure = Box::new(measure) as Box<MeasureFn>;
        #[cfg(feature = "stacker")]
        let measure = StackSafe::new(measure);
        Self::Dynamic(Some(measure))
    }

    pub(crate) fn measure(
        &mut self,
        constraints: MeasureConstraints,
        final_layout: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        match self {
            Self::Dynamic(measure) => {
                let measure = measure
                    .as_mut()
                    .expect("layout replay cannot remeasure a node");
                measure(constraints.known, constraints.available, window, cx)
            }
            Self::Cached(cache) => cache.measure(constraints, final_layout, window, cx),
        }
    }

    pub(crate) fn carry_from(&mut self, previous: &mut Self) -> bool {
        match (self, previous) {
            (Self::Cached(current), Self::Cached(previous)) => current.carry_from(previous),
            _ => false,
        }
    }

    fn release_callbacks(&mut self) {
        match self {
            Self::Dynamic(measure) => *measure = None,
            Self::Cached(cache) => cache.callbacks = None,
        }
    }
}

pub(crate) struct CachedMeasure {
    inputs: Box<dyn Any>,
    compare_inputs: CompareInputs,
    measure_type: TypeId,
    rem_size: Pixels,
    scale: f32,
    callbacks: Option<CacheCallbacks>,
    result: Option<MeasuredResult>,
}

struct CacheCallbacks {
    measure: StackSafe<Box<CacheMeasureFn>>,
    publish: Box<PublishMeasureFn>,
}

struct MeasuredResult {
    constraints: MeasureConstraints,
    size: Size<Pixels>,
    state: Box<dyn Any>,
}

impl CachedMeasure {
    pub(crate) fn new<Inputs, State, Measure>(
        inputs: Inputs,
        rem_size: Pixels,
        scale: f32,
        mut measure: Measure,
        publish: impl Fn(&State) + 'static,
    ) -> Self
    where
        Inputs: PartialEq + 'static,
        State: 'static,
        Measure: FnMut(
                Size<Option<Pixels>>,
                Size<AvailableSpace>,
                &mut Window,
                &mut App,
            ) -> (Size<Pixels>, State)
            + 'static,
    {
        let measure = Box::new(
            move |constraints: MeasureConstraints, window: &mut Window, cx: &mut App| {
                let (size, state) = measure(constraints.known, constraints.available, window, cx);
                (size, Box::new(state) as Box<dyn Any>)
            },
        ) as Box<CacheMeasureFn>;
        #[cfg(feature = "stacker")]
        let measure = StackSafe::new(measure);
        Self {
            inputs: Box::new(inputs),
            compare_inputs: |current, previous| {
                let (Some(current), Some(previous)) = (
                    current.downcast_ref::<Inputs>(),
                    previous.downcast_ref::<Inputs>(),
                ) else {
                    return false;
                };
                current == previous
            },
            measure_type: TypeId::of::<Measure>(),
            rem_size,
            scale,
            callbacks: Some(CacheCallbacks {
                measure,
                publish: Box::new(move |state| {
                    publish(
                        state
                            .downcast_ref::<State>()
                            .expect("matching measurement state type"),
                    );
                }),
            }),
            result: None,
        }
    }

    fn carry_from(&mut self, previous: &mut Self) -> bool {
        if self.measure_type != previous.measure_type
            || self.rem_size != previous.rem_size
            || self.scale != previous.scale
            || !(self.compare_inputs)(&*self.inputs, &*previous.inputs)
        {
            return false;
        }
        let Some(result) = previous.result.take() else {
            return false;
        };
        if let Some(callbacks) = &self.callbacks {
            (callbacks.publish)(&*result.state);
        }
        self.result = Some(result);
        true
    }

    fn measure(
        &mut self,
        constraints: MeasureConstraints,
        final_layout: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        if let Some(result) = &self.result
            && result.constraints == constraints
        {
            if final_layout && let Some(callbacks) = &self.callbacks {
                (callbacks.publish)(&*result.state);
            }
            return result.size;
        }

        let callbacks = self
            .callbacks
            .as_mut()
            .expect("layout replay cannot remeasure a node");
        let (size, state) = (callbacks.measure)(constraints, window, cx);
        if final_layout {
            (callbacks.publish)(&*state);
            self.result = Some(MeasuredResult {
                constraints,
                size,
                state,
            });
        }
        size
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutRetention, NodeContext};
    use taffy::{Style, TaffyTree, prelude::max_content};

    #[test]
    fn detached_root_lookup_validates_ancestry_without_claiming_it() {
        let mut tree = TaffyTree::new();
        let window_root = tree.new_leaf(Style::default()).unwrap();
        let pane_gap = tree.new_leaf(Style::default()).unwrap();
        let pane_branch = tree
            .new_with_children(Style::default(), &[pane_gap])
            .unwrap();
        let pane_root = tree
            .new_with_children(Style::default(), &[pane_branch])
            .unwrap();
        let dynamic = tree
            .new_leaf_with_context(Style::default(), NodeContext::Dynamic(None))
            .unwrap();
        let unrecorded_root = tree.new_leaf(Style::default()).unwrap();

        let mut retention = LayoutRetention::default();
        retention.begin_frame();
        for node in [
            window_root,
            pane_gap,
            pane_branch,
            pane_root,
            dynamic,
            unrecorded_root,
        ] {
            retention.record_request(None, node.into());
        }
        for root in [window_root, pane_root, dynamic] {
            tree.compute_layout(root, max_content()).unwrap();
            retention.record_computed_root(root.into());
        }
        tree.compute_layout(unrecorded_root, max_content()).unwrap();
        assert_eq!(retention.retained_tree_root(&tree, pane_gap.into()), None);
        retention.finish_frame(&mut tree);
        retention.begin_frame();

        assert_eq!(
            retention.retained_tree_root(&tree, window_root.into()),
            Some(window_root.into())
        );
        assert_eq!(
            retention.retained_tree_root(&tree, pane_gap.into()),
            Some(pane_root.into())
        );
        assert_eq!(retention.retained_tree_root(&tree, dynamic.into()), None);
        assert_eq!(
            retention.retained_tree_root(&tree, unrecorded_root.into()),
            None
        );
        assert!(!retention.claim_subtree_except(&tree, window_root.into(), &[pane_gap.into()]));
        assert!(retention.claim_subtree_except(&tree, window_root.into(), &[]));
        assert!(retention.claim_subtree_except(&tree, pane_root.into(), &[pane_gap.into()]));
        assert_eq!(retention.retained_tree_root(&tree, pane_gap.into()), None);

        retention.release_subtree_claims(pane_root.into());
        assert_eq!(
            retention.retained_tree_root(&tree, pane_gap.into()),
            Some(pane_root.into())
        );
        assert_eq!(
            retention.retained_tree_root(&tree, window_root.into()),
            None
        );
        tree.mark_dirty(pane_root).unwrap();
        assert_eq!(retention.retained_tree_root(&tree, pane_gap.into()), None);

        retention.finish_frame(&mut tree);
        retention.begin_frame();
        assert_eq!(retention.retained_tree_root(&tree, pane_gap.into()), None);
    }
}
