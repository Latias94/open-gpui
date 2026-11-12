use std::{cell::RefCell, fmt::Debug};

use open_gpui_collections::FxHashMap;

use crate::{FocusHandle, FocusId, geometry::SubtreeGeometryValidity};

/// Represents a collection of focus handles using the tab-index APIs.
#[derive(Debug, Default)]
pub(crate) struct TabStopMap {
    current_path: TabStopPath,
    pub(crate) insertion_history: Vec<TabStopOperation>,
    order: RefCell<TabStopOrder>,
}

#[derive(Debug, Default)]
struct TabStopOrder {
    by_id: FxHashMap<FocusId, usize>,
    nodes: Vec<TabStopNode>,
    sorted: bool,
}

#[derive(Debug, Clone)]
pub enum TabStopOperation {
    Insert {
        focus_handle: FocusHandle,
        validity: Option<SubtreeGeometryValidity>,
    },
    Group(TabIndex),
    GroupEnd,
}

impl TabStopOperation {
    fn focus_handle(&self) -> Option<&FocusHandle> {
        match self {
            TabStopOperation::Insert { focus_handle, .. } => Some(focus_handle),
            _ => None,
        }
    }
}

type TabIndex = isize;

#[derive(Debug, Default, PartialEq, Eq, Clone, Ord, PartialOrd)]
struct TabStopPath(smallvec::SmallVec<[TabIndex; 6]>);

#[derive(Clone, Debug, Default)]
struct TabStopNode {
    /// Path to access the node in the tree
    /// The final node in the list is a leaf node corresponding to an actual focus handle,
    /// all other nodes are group nodes
    path: TabStopPath,
    /// index into the backing array of nodes. Corresponds to insertion order
    node_insertion_index: usize,

    /// Whether this node is a tab stop
    tab_stop: bool,
    validity: Option<SubtreeGeometryValidity>,
}

impl PartialEq for TabStopNode {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.node_insertion_index == other.node_insertion_index
            && self.tab_stop == other.tab_stop
    }
}

impl Eq for TabStopNode {}

impl Ord for TabStopNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path
            .cmp(&other.path)
            .then(self.node_insertion_index.cmp(&other.node_insertion_index))
    }
}

impl PartialOrd for TabStopNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl TabStopMap {
    pub fn tab_stop_count(&self) -> usize {
        let order = self.order.borrow();
        order
            .by_id
            .values()
            .filter(|index| {
                order
                    .nodes
                    .get(**index)
                    .is_some_and(|node| node.tab_stop && node.is_active())
            })
            .count()
    }

    pub fn insert_scoped(
        &mut self,
        focus_handle: &FocusHandle,
        validity: Option<SubtreeGeometryValidity>,
    ) {
        self.insertion_history.push(TabStopOperation::Insert {
            focus_handle: focus_handle.clone(),
            validity: validity.clone(),
        });
        let mut path = self.current_path.clone();
        path.0.push(focus_handle.tab_index);
        let order = self.order.get_mut();
        order.by_id.insert(focus_handle.id, order.nodes.len());
        order.sorted = false;
        order.nodes.push(TabStopNode {
            node_insertion_index: self.insertion_history.len() - 1,
            tab_stop: focus_handle.tab_stop,
            path,
            validity,
        });
    }

    pub fn begin_group(&mut self, tab_index: isize) {
        self.insertion_history
            .push(TabStopOperation::Group(tab_index));
        self.current_path.0.push(tab_index);
    }

    pub fn end_group(&mut self) {
        self.insertion_history.push(TabStopOperation::GroupEnd);
        self.current_path.0.pop();
    }

    pub fn clear(&mut self) {
        self.current_path.0.clear();
        self.insertion_history.clear();
        let order = self.order.get_mut();
        order.by_id.clear();
        order.nodes.clear();
        order.sorted = false;
    }

    pub fn next(&self, focused_id: Option<&FocusId>) -> Option<FocusHandle> {
        let mut order = self.order.borrow_mut();
        order.ensure_sorted(&self.insertion_history);
        let start = focused_id
            .and_then(|id| order.by_id.get(id))
            .map_or(0, |position| position + 1);
        let node = order.nodes[start..]
            .iter()
            .chain(&order.nodes[..start])
            .find(|node| node.tab_stop && node.is_active())?;
        self.focus_handle_for_order(node)
    }

    pub fn next_where(
        &self,
        focused_id: Option<&FocusId>,
        predicate: impl Fn(&FocusHandle) -> bool,
    ) -> Option<FocusHandle> {
        let mut current = focused_id.copied();
        for _ in 0..self.insertion_history.len() {
            let candidate = self.next(current.as_ref())?;
            if predicate(&candidate) {
                return Some(candidate);
            }
            current = Some(candidate.id);
        }
        None
    }

    pub fn prev(&self, focused_id: Option<&FocusId>) -> Option<FocusHandle> {
        let mut order = self.order.borrow_mut();
        order.ensure_sorted(&self.insertion_history);
        let end = focused_id
            .and_then(|id| order.by_id.get(id).copied())
            .unwrap_or(order.nodes.len());
        let node = order.nodes[..end]
            .iter()
            .rev()
            .chain(order.nodes[end..].iter().rev())
            .find(|node| node.tab_stop && node.is_active())?;
        self.focus_handle_for_order(node)
    }

    pub fn prev_where(
        &self,
        focused_id: Option<&FocusId>,
        predicate: impl Fn(&FocusHandle) -> bool,
    ) -> Option<FocusHandle> {
        let mut current = focused_id.copied();
        for _ in 0..self.insertion_history.len() {
            let candidate = self.prev(current.as_ref())?;
            if predicate(&candidate) {
                return Some(candidate);
            }
            current = Some(candidate.id);
        }
        None
    }

    pub fn replay_scoped(
        &mut self,
        nodes: &[TabStopOperation],
        validity: Option<SubtreeGeometryValidity>,
    ) {
        for node in nodes {
            match node {
                TabStopOperation::Insert {
                    focus_handle,
                    validity: recorded_validity,
                } => {
                    let replayed_validity = SubtreeGeometryValidity::replayed_under(
                        recorded_validity.as_ref(),
                        validity.clone(),
                    );
                    self.insert_scoped(focus_handle, replayed_validity)
                }
                TabStopOperation::Group(tab_index) => self.begin_group(*tab_index),
                TabStopOperation::GroupEnd => self.end_group(),
            }
        }
    }

    pub fn paint_index(&self) -> usize {
        self.insertion_history.len()
    }

    fn focus_handle_for_order(&self, order: &TabStopNode) -> Option<FocusHandle> {
        let handle = self.insertion_history[order.node_insertion_index].focus_handle();
        debug_assert!(
            handle.is_some(),
            "The order node did not correspond to an element, this is a GPUI bug"
        );
        handle.cloned()
    }
}

impl TabStopNode {
    fn is_active(&self) -> bool {
        self.validity
            .as_ref()
            .is_none_or(SubtreeGeometryValidity::is_valid)
    }
}

impl TabStopOrder {
    fn ensure_sorted(&mut self, insertion_history: &[TabStopOperation]) {
        if self.sorted {
            return;
        }
        self.nodes.sort_unstable();
        self.by_id.clear();
        for (position, node) in self.nodes.iter().enumerate() {
            let Some(focus_handle) = insertion_history
                .get(node.node_insertion_index)
                .and_then(TabStopOperation::focus_handle)
            else {
                continue;
            };
            let latest = self.by_id.entry(focus_handle.id).or_insert(position);
            if self.nodes[*latest].node_insertion_index < node.node_insertion_index {
                *latest = position;
            }
        }
        self.sorted = true;
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools as _;

    use crate::{
        FocusHandle, FocusId, FocusMap, SubtreeTransformError, TabStopMap,
        geometry::SubtreeGeometryValidity,
    };
    use parking_lot::RwLock;
    use std::sync::Arc;

    #[test]
    fn test_tab_handles() {
        let focus_map = Arc::new(RwLock::new(FocusMap::default()));
        let mut tab_index_map = TabStopMap::default();

        let focus_handles = [
            FocusHandle::new(&focus_map).tab_stop(true).tab_index(0),
            FocusHandle::new(&focus_map).tab_stop(true).tab_index(1),
            FocusHandle::new(&focus_map).tab_stop(true).tab_index(1),
            FocusHandle::new(&focus_map),
            FocusHandle::new(&focus_map).tab_index(2),
            FocusHandle::new(&focus_map).tab_stop(true).tab_index(0),
            FocusHandle::new(&focus_map).tab_stop(true).tab_index(2),
        ];

        for handle in focus_handles.iter() {
            tab_index_map.insert_scoped(handle, None);
        }
        let expected = [
            focus_handles[0].clone(),
            focus_handles[5].clone(),
            focus_handles[1].clone(),
            focus_handles[2].clone(),
            focus_handles[6].clone(),
        ];

        let mut prev = None;
        let mut found = vec![];
        for _ in 0..expected.len() {
            let handle = tab_index_map.next(prev.as_ref()).unwrap();
            prev = Some(handle.id);
            found.push(handle.id);
        }

        assert_eq!(
            found,
            expected.iter().map(|handle| handle.id).collect::<Vec<_>>()
        );

        // Select first tab index if no handle is currently focused.
        assert_eq!(tab_index_map.next(None), Some(expected[0].clone()));
        // Select last tab index if no handle is currently focused.
        assert_eq!(tab_index_map.prev(None), expected.last().cloned(),);

        assert_eq!(
            tab_index_map.next(Some(&expected[0].id)),
            Some(expected[1].clone())
        );
        assert_eq!(
            tab_index_map.next(Some(&expected[1].id)),
            Some(expected[2].clone())
        );
        assert_eq!(
            tab_index_map.next(Some(&expected[2].id)),
            Some(expected[3].clone())
        );
        assert_eq!(
            tab_index_map.next(Some(&expected[3].id)),
            Some(expected[4].clone())
        );
        assert_eq!(
            tab_index_map.next(Some(&expected[4].id)),
            Some(expected[0].clone())
        );

        // prev
        assert_eq!(tab_index_map.prev(None), Some(expected[4].clone()));
        assert_eq!(
            tab_index_map.prev(Some(&expected[0].id)),
            Some(expected[4].clone())
        );
        assert_eq!(
            tab_index_map.prev(Some(&expected[1].id)),
            Some(expected[0].clone())
        );
        assert_eq!(
            tab_index_map.prev(Some(&expected[2].id)),
            Some(expected[1].clone())
        );
        assert_eq!(
            tab_index_map.prev(Some(&expected[3].id)),
            Some(expected[2].clone())
        );
        assert_eq!(
            tab_index_map.prev(Some(&expected[4].id)),
            Some(expected[3].clone())
        );
    }

    #[test]
    fn test_tab_non_stop_filtering() {
        let focus_map = Arc::new(RwLock::new(FocusMap::default()));
        let mut tab_index_map = TabStopMap::default();

        // Check that we can query next from a non-stop tab
        let tab_non_stop_1 = FocusHandle::new(&focus_map).tab_stop(false).tab_index(1);
        let tab_stop_2 = FocusHandle::new(&focus_map).tab_stop(true).tab_index(2);
        tab_index_map.insert_scoped(&tab_non_stop_1, None);
        tab_index_map.insert_scoped(&tab_stop_2, None);
        let result = tab_index_map.next(Some(&tab_non_stop_1.id)).unwrap();
        assert_eq!(result.id, tab_stop_2.id);

        // Check that we skip over non-stop tabs
        let tab_stop_0 = FocusHandle::new(&focus_map).tab_stop(true).tab_index(0);
        let tab_non_stop_0 = FocusHandle::new(&focus_map).tab_stop(false).tab_index(0);
        tab_index_map.insert_scoped(&tab_stop_0, None);
        tab_index_map.insert_scoped(&tab_non_stop_0, None);
        let result = tab_index_map.next(Some(&tab_stop_0.id)).unwrap();
        assert_eq!(result.id, tab_stop_2.id);
    }

    #[test]
    fn test_insert_after_sorting() {
        let focus_map = Arc::new(RwLock::new(FocusMap::default()));
        let mut tab_index_map = TabStopMap::default();
        let second = FocusHandle::new(&focus_map).tab_stop(true).tab_index(1);
        let first = FocusHandle::new(&focus_map).tab_stop(true).tab_index(0);

        tab_index_map.insert_scoped(&second, None);
        assert_eq!(tab_index_map.next(None), Some(second.clone()));

        tab_index_map.insert_scoped(&first, None);
        assert_eq!(tab_index_map.next(None), Some(first.clone()));
        assert_eq!(tab_index_map.next(Some(&first.id)), Some(second.clone()));
        assert_eq!(tab_index_map.prev(Some(&first.id)), Some(second));

        tab_index_map.clear();
        assert_eq!(tab_index_map.paint_index(), 0);
        assert_eq!(tab_index_map.tab_stop_count(), 0);
        assert_eq!(tab_index_map.next(None), None);
        assert_eq!(tab_index_map.prev(None), None);

        tab_index_map.insert_scoped(&first, None);
        assert_eq!(tab_index_map.next(Some(&first.id)), Some(first.clone()));
        assert_eq!(tab_index_map.prev(Some(&first.id)), Some(first));
    }

    #[test]
    fn test_duplicate_focus_ids_use_latest_insertion() {
        let focus_map = Arc::new(RwLock::new(FocusMap::default()));
        let mut tab_index_map = TabStopMap::default();
        let first = FocusHandle::new(&focus_map).tab_stop(true).tab_index(0);
        let middle = FocusHandle::new(&focus_map).tab_stop(true).tab_index(2);
        let repeated = FocusHandle::new(&focus_map).tab_stop(true).tab_index(3);
        let last = FocusHandle::new(&focus_map).tab_stop(true).tab_index(4);

        for handle in [&first, &middle, &repeated, &last] {
            tab_index_map.insert_scoped(handle, None);
        }
        tab_index_map.insert_scoped(&repeated.clone().tab_index(-1), None);
        assert_eq!(tab_index_map.tab_stop_count(), 4);
        assert_eq!(tab_index_map.next(Some(&repeated.id)), Some(first.clone()));
        assert_eq!(tab_index_map.prev(Some(&repeated.id)), Some(last.clone()));
        assert_eq!(tab_index_map.next(Some(&middle.id)), Some(repeated.clone()));
        assert_eq!(tab_index_map.tab_stop_count(), 4);

        tab_index_map.insert_scoped(&repeated.clone().tab_stop(false).tab_index(1), None);
        assert_eq!(tab_index_map.tab_stop_count(), 3);
        assert_eq!(tab_index_map.next(Some(&repeated.id)), Some(middle));
        assert_eq!(tab_index_map.prev(Some(&repeated.id)), Some(first));
        assert_eq!(tab_index_map.prev(Some(&last.id)), Some(repeated));
        assert_eq!(tab_index_map.tab_stop_count(), 3);
    }

    #[test]
    fn test_geometry_filtering_after_sorting() {
        let focus_map = Arc::new(RwLock::new(FocusMap::default()));
        let mut tab_index_map = TabStopMap::default();
        let boundary_validity = SubtreeGeometryValidity::new(None);
        let middle_validity = SubtreeGeometryValidity::new(None);
        let first = FocusHandle::new(&focus_map).tab_stop(true).tab_index(0);
        let middle = FocusHandle::new(&focus_map).tab_stop(true).tab_index(1);
        let last = FocusHandle::new(&focus_map).tab_stop(true).tab_index(2);

        tab_index_map.insert_scoped(&first, Some(boundary_validity.clone()));
        tab_index_map.insert_scoped(&middle, Some(middle_validity.clone()));
        tab_index_map.insert_scoped(
            &last,
            Some(SubtreeGeometryValidity::new(Some(
                boundary_validity.clone(),
            ))),
        );
        assert_eq!(tab_index_map.next(None), Some(first.clone()));
        assert_eq!(tab_index_map.prev(None), Some(last.clone()));

        boundary_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        assert_eq!(tab_index_map.tab_stop_count(), 1);
        assert_eq!(tab_index_map.next(Some(&first.id)), Some(middle.clone()));
        assert_eq!(tab_index_map.prev(Some(&last.id)), Some(middle.clone()));
        assert_eq!(
            tab_index_map.next_where(None, |candidate| {
                tab_index_map.prev(Some(&candidate.id)).as_ref() == Some(candidate)
            }),
            Some(middle.clone())
        );
        assert_eq!(
            tab_index_map.prev_where(None, |candidate| {
                tab_index_map.next(Some(&candidate.id)).as_ref() == Some(candidate)
            }),
            Some(middle)
        );
        assert_eq!(
            tab_index_map.next_where(None, |candidate| candidate.id == first.id),
            None
        );
        assert_eq!(
            tab_index_map.prev_where(None, |candidate| candidate.id == last.id),
            None
        );

        middle_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        assert_eq!(tab_index_map.tab_stop_count(), 0);
        assert_eq!(tab_index_map.next(None), None);
        assert_eq!(tab_index_map.prev(None), None);
    }

    #[test]
    fn test_nested_group_replay_preserves_order_and_geometry() {
        let focus_map = Arc::new(RwLock::new(FocusMap::default()));
        let mut recorded = TabStopMap::default();
        let recorded_validity = SubtreeGeometryValidity::new(None);
        let invalid_validity = SubtreeGeometryValidity::new(Some(recorded_validity.clone()));
        let first = FocusHandle::new(&focus_map).tab_stop(true).tab_index(2);
        let nested = FocusHandle::new(&focus_map).tab_stop(true).tab_index(1);
        let invalid = FocusHandle::new(&focus_map).tab_stop(true).tab_index(0);
        let last = FocusHandle::new(&focus_map).tab_stop(true).tab_index(4);
        let outside = FocusHandle::new(&focus_map).tab_stop(true).tab_index(0);
        let excluded = FocusHandle::new(&focus_map).tab_stop(true).tab_index(5);

        recorded.insert_scoped(&excluded, Some(recorded_validity.clone()));
        let start = recorded.paint_index();
        recorded.begin_group(2);
        recorded.insert_scoped(&first, Some(recorded_validity.clone()));
        recorded.begin_group(1);
        recorded.insert_scoped(&nested, Some(recorded_validity.clone()));
        recorded.insert_scoped(&invalid, Some(invalid_validity.clone()));
        recorded.end_group();
        recorded.end_group();
        recorded.insert_scoped(&last, Some(recorded_validity.clone()));
        let end = recorded.paint_index();
        assert_eq!(recorded.next(None), Some(invalid));
        invalid_validity.invalidate(SubtreeTransformError::UnrepresentableResult);

        let replay_validity = SubtreeGeometryValidity::new(None);
        let mut candidate = TabStopMap::default();
        candidate.replay_scoped(
            &recorded.insertion_history[start..end],
            Some(replay_validity.clone()),
        );
        assert_eq!(candidate.next(None), Some(nested.clone()));
        candidate.clear();
        assert_eq!(candidate.next(None), None);
        assert_eq!(recorded.next(None), Some(nested.clone()));

        candidate.insert_scoped(&outside, None);
        assert_eq!(candidate.next(None), Some(outside.clone()));
        candidate.begin_group(3);
        candidate.replay_scoped(
            &recorded.insertion_history[start..end],
            Some(replay_validity.clone()),
        );
        candidate.end_group();
        assert_eq!(candidate.paint_index(), end - start + 3);
        assert_eq!(recorded.paint_index(), end);
        assert_eq!(candidate.tab_stop_count(), 4);

        let expected = [outside.clone(), nested, first, last];
        for (position, handle) in expected.iter().enumerate() {
            assert_eq!(
                candidate.next(Some(&handle.id)),
                Some(expected[(position + 1) % expected.len()].clone())
            );
            assert_eq!(
                candidate.prev(Some(&handle.id)),
                Some(expected[(position + expected.len() - 1) % expected.len()].clone())
            );
        }

        recorded_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        assert_eq!(recorded.tab_stop_count(), 0);
        assert_eq!(candidate.tab_stop_count(), 4);
        assert_eq!(candidate.next(None), expected.first().cloned());
        assert_eq!(candidate.prev(None), expected.last().cloned());

        replay_validity.invalidate(SubtreeTransformError::UnrepresentableResult);
        assert_eq!(candidate.tab_stop_count(), 1);
        assert_eq!(candidate.next(Some(&outside.id)), Some(outside.clone()));
        assert_eq!(candidate.prev(Some(&outside.id)), Some(outside));
    }

    #[must_use]
    struct TabStopMapTest {
        tab_map: TabStopMap,
        focus_map: Arc<RwLock<FocusMap>>,
        expected: Vec<(usize, FocusId)>,
    }

    impl TabStopMapTest {
        #[must_use]
        fn new() -> Self {
            Self {
                tab_map: TabStopMap::default(),
                focus_map: Arc::new(RwLock::new(FocusMap::default())),
                expected: Vec::default(),
            }
        }

        #[must_use]
        fn tab_non_stop(mut self, index: isize) -> Self {
            let handle = FocusHandle::new(&self.focus_map)
                .tab_stop(false)
                .tab_index(index);
            self.tab_map.insert_scoped(&handle, None);
            self
        }

        #[must_use]
        fn tab_stop(mut self, index: isize, expected: usize) -> Self {
            let handle = FocusHandle::new(&self.focus_map)
                .tab_stop(true)
                .tab_index(index);
            self.tab_map.insert_scoped(&handle, None);
            self.expected.push((expected, handle.id));
            self.expected.sort_by_key(|(expected, _)| *expected);
            self
        }

        #[must_use]
        fn tab_group(mut self, tab_index: isize, children: impl FnOnce(Self) -> Self) -> Self {
            self.tab_map.begin_group(tab_index);
            self = children(self);
            self.tab_map.end_group();
            self
        }

        fn traverse_tab_map(
            &self,
            traverse: impl Fn(&TabStopMap, Option<&FocusId>) -> Option<FocusHandle>,
        ) -> Vec<FocusId> {
            let mut last_focus_id = None;
            let mut found = vec![];
            for _ in 0..self.expected.len() {
                let handle = traverse(&self.tab_map, last_focus_id.as_ref()).unwrap();
                last_focus_id = Some(handle.id);
                found.push(handle.id);
            }
            found
        }

        fn assert(self) {
            let mut expected = self.expected.iter().map(|(_, id)| *id).collect_vec();

            // Check next order
            let forward_found = self.traverse_tab_map(|tab_map, prev| tab_map.next(prev));
            assert_eq!(forward_found, expected);

            // Test overflow. Last to first
            assert_eq!(
                self.tab_map
                    .next(forward_found.last())
                    .map(|handle| handle.id),
                expected.first().cloned()
            );

            // Check previous order
            let reversed_found = self.traverse_tab_map(|tab_map, prev| tab_map.prev(prev));
            expected.reverse();
            assert_eq!(reversed_found, expected);

            // Test overflow. First to last
            assert_eq!(
                self.tab_map
                    .prev(reversed_found.last())
                    .map(|handle| handle.id),
                expected.first().cloned(),
            );
        }
    }

    #[test]
    fn test_with_disabled_tab_stop() {
        TabStopMapTest::new()
            .tab_stop(0, 0)
            .tab_non_stop(1)
            .tab_stop(2, 1)
            .tab_stop(3, 2)
            .assert();
    }

    #[test]
    fn test_with_multiple_disabled_tab_stops() {
        TabStopMapTest::new()
            .tab_non_stop(0)
            .tab_stop(1, 0)
            .tab_non_stop(3)
            .tab_stop(3, 1)
            .tab_non_stop(4)
            .assert();
    }

    #[test]
    fn test_tab_group_functionality() {
        TabStopMapTest::new()
            .tab_stop(0, 0)
            .tab_stop(0, 1)
            .tab_group(2, |t| t.tab_stop(0, 2).tab_stop(1, 3))
            .tab_stop(3, 4)
            .tab_stop(4, 5)
            .assert()
    }

    #[test]
    fn test_sibling_groups() {
        TabStopMapTest::new()
            .tab_stop(0, 0)
            .tab_stop(1, 1)
            .tab_group(2, |test| test.tab_stop(0, 2).tab_stop(1, 3))
            .tab_stop(3, 4)
            .tab_stop(4, 5)
            .tab_group(6, |test| test.tab_stop(0, 6).tab_stop(1, 7))
            .tab_stop(7, 8)
            .tab_stop(8, 9)
            .assert();
    }

    #[test]
    fn test_nested_group() {
        TabStopMapTest::new()
            .tab_stop(0, 0)
            .tab_stop(1, 1)
            .tab_group(2, |t| {
                t.tab_group(0, |t| t.tab_stop(0, 2).tab_stop(1, 3))
                    .tab_stop(1, 4)
            })
            .tab_stop(3, 5)
            .tab_stop(4, 6)
            .assert();
    }

    #[test]
    fn test_sibling_nested_groups() {
        TabStopMapTest::new()
            .tab_stop(0, 0)
            .tab_stop(1, 1)
            .tab_group(2, |builder| {
                builder
                    .tab_stop(0, 2)
                    .tab_stop(2, 5)
                    .tab_group(1, |builder| builder.tab_stop(0, 3).tab_stop(1, 4))
                    .tab_group(3, |builder| builder.tab_stop(0, 6).tab_stop(1, 7))
            })
            .tab_stop(3, 8)
            .tab_stop(4, 9)
            .assert();
    }

    #[test]
    fn test_sibling_nested_groups_out_of_order() {
        TabStopMapTest::new()
            .tab_stop(9, 9)
            .tab_stop(8, 8)
            .tab_group(7, |builder| {
                builder
                    .tab_stop(0, 2)
                    .tab_stop(2, 5)
                    .tab_group(3, |builder| builder.tab_stop(1, 7).tab_stop(0, 6))
                    .tab_group(1, |builder| builder.tab_stop(0, 3).tab_stop(1, 4))
            })
            .tab_stop(3, 0)
            .tab_stop(4, 1)
            .assert();
    }
}
