use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
};

use open_gpui::SharedString;

/// A row's stable key, parent, and default label. Input order defines sibling order.
#[derive(Clone, Debug)]
pub struct TreeRow<K> {
    key: K,
    parent: Option<K>,
    label: SharedString,
}

impl<K> TreeRow<K> {
    pub fn new(key: K, parent: Option<K>, label: impl Into<SharedString>) -> Self {
        Self {
            key,
            parent,
            label: label.into(),
        }
    }

    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn parent(&self) -> Option<&K> {
        self.parent.as_ref()
    }
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// Invalid row data. A rejected update leaves the old list intact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeError {
    DuplicateKey,
    MissingParent,
    Cycle,
    RowIdExhausted,
}

impl fmt::Display for TreeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(match self {
            Self::DuplicateKey => "row keys must be unique",
            Self::MissingParent => "each parent must identify a supplied row",
            Self::Cycle => "parent relationships must be acyclic",
            Self::RowIdExhausted => "the list exhausted its stable element IDs",
        })
    }
}

impl std::error::Error for TreeError {}

/// Drop placement relative to a target row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropPlacement {
    Before,
    After,
    Into,
}

/// An ordered move of sibling subtrees relative to an existing row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reorder<K> {
    pub moved: Vec<K>,
    pub target: K,
    pub placement: DropPlacement,
}

pub(super) struct TreeModel<K> {
    pub(super) nodes: Vec<Node<K>>,
    pub(super) indices: HashMap<K, usize>,
    pub(super) visible: Vec<(usize, usize)>,
    visible_positions: Vec<Option<usize>>,
    roots: Vec<usize>,
    ordered: Vec<(usize, usize)>,
    pub(super) expanded: HashSet<K>,
    pub(super) selected: HashSet<K>,
    pub(super) active: Option<K>,
    anchor: Option<K>,
    next_id: usize,
}

pub(super) struct Node<K> {
    pub(super) row: TreeRow<K>,
    pub(super) children: Vec<usize>,
    pub(super) id: usize,
}

impl<K: Clone + Eq + Hash> TreeModel<K> {
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            indices: HashMap::new(),
            visible: Vec::new(),
            visible_positions: Vec::new(),
            roots: Vec::new(),
            ordered: Vec::new(),
            expanded: HashSet::new(),
            selected: HashSet::new(),
            active: None,
            anchor: None,
            next_id: 0,
        }
    }

    pub(super) fn replace(
        &mut self,
        rows: impl IntoIterator<Item = TreeRow<K>>,
    ) -> Result<(), TreeError> {
        let mut next = Self::new();
        next.next_id = self.next_id;
        for row in rows {
            if next
                .indices
                .insert(row.key.clone(), next.nodes.len())
                .is_some()
            {
                return Err(TreeError::DuplicateKey);
            }
            let id = match self.indices.get(&row.key) {
                Some(index) => self.nodes[*index].id,
                None => {
                    let id = next.next_id;
                    next.next_id = id.checked_add(1).ok_or(TreeError::RowIdExhausted)?;
                    id
                }
            };
            next.nodes.push(Node {
                row,
                children: Vec::new(),
                id,
            });
        }
        for index in 0..next.nodes.len() {
            if let Some(parent) = next.nodes[index].row.parent() {
                let parent = *next.indices.get(parent).ok_or(TreeError::MissingParent)?;
                next.nodes[parent].children.push(index);
            } else {
                next.roots.push(index);
            }
        }
        next.ordered = next.flatten(false);
        if next.ordered.len() != next.nodes.len() {
            return Err(TreeError::Cycle);
        }
        self.expanded.retain(|key| next.indices.contains_key(key));
        self.selected.retain(|key| next.indices.contains_key(key));
        next.expanded = std::mem::take(&mut self.expanded);
        next.selected = std::mem::take(&mut self.selected);
        next.active = self.active.take();
        next.anchor = self.anchor.take();
        next.refresh_visible();
        *self = next;
        Ok(())
    }

    pub(super) fn position(&self, key: &K) -> Option<usize> {
        self.visible_positions[*self.indices.get(key)?]
    }

    pub(super) fn key_at(&self, position: usize) -> Option<&K> {
        let (index, _) = self.visible.get(position)?;
        Some(self.nodes[*index].row.key())
    }

    pub(super) fn has_hierarchy(&self) -> bool {
        self.roots.len() != self.nodes.len()
    }

    pub(super) fn set_expanded(&mut self, key: &K, expanded: bool) -> bool {
        if self
            .indices
            .get(key)
            .is_none_or(|index| self.nodes[*index].children.is_empty())
        {
            return false;
        }
        let changed = if expanded {
            self.expanded.insert(key.clone())
        } else {
            self.expanded.remove(key)
        };
        if changed {
            self.refresh_visible();
        }
        changed
    }

    pub(super) fn select(&mut self, key: &K, extend: bool, toggle: bool) -> bool {
        let Some(position) = self.position(key) else {
            return false;
        };
        self.active = Some(key.clone());
        if extend {
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|anchor| self.position(anchor))
                .unwrap_or(position);
            self.anchor = self.key_at(anchor).cloned();
            let mut selected = if toggle {
                self.selected.clone()
            } else {
                HashSet::new()
            };
            selected.extend(
                self.visible[anchor.min(position)..=anchor.max(position)]
                    .iter()
                    .map(|(index, _)| self.nodes[*index].row.key.clone()),
            );
            let changed = self.selected != selected;
            self.selected = selected;
            return changed;
        }
        self.anchor = Some(key.clone());
        if toggle {
            if !self.selected.remove(key) {
                self.selected.insert(key.clone());
            }
            return true;
        }
        if self.selected.len() == 1 && self.selected.contains(key) {
            return false;
        }
        self.selected.clear();
        self.selected.insert(key.clone());
        true
    }

    pub(super) fn selection(&self) -> Vec<K> {
        self.ordered
            .iter()
            .filter_map(|(index, _)| {
                let key = &self.nodes[*index].row.key;
                self.selected.contains(key).then(|| key.clone())
            })
            .collect()
    }

    pub(super) fn plan_reorder(
        &self,
        moved: &[K],
        target: &K,
        placement: DropPlacement,
    ) -> Option<Reorder<K>> {
        self.indices.get(target)?;
        let moved: HashSet<_> = moved.iter().collect();
        if moved.contains(target) || self.ancestors(target).any(|key| moved.contains(key)) {
            return None;
        }
        let moved: Vec<_> = self
            .ordered
            .iter()
            .filter_map(|(index, _)| {
                let key = self.nodes[*index].row.key();
                (moved.contains(key)
                    && !self.ancestors(key).any(|ancestor| moved.contains(ancestor)))
                .then(|| key.clone())
            })
            .collect();
        (!moved.is_empty()).then(|| Reorder {
            moved,
            target: target.clone(),
            placement,
        })
    }

    pub(super) fn apply_reorder(&mut self, request: &Reorder<K>) -> bool {
        let Some(plan) = self.plan_reorder(&request.moved, &request.target, request.placement)
        else {
            return false;
        };
        if plan != *request {
            return false;
        }
        let target = self.indices[&plan.target];
        let new_parent = match plan.placement {
            DropPlacement::Into => Some(plan.target),
            DropPlacement::Before | DropPlacement::After => self.nodes[target].row.parent.clone(),
        };
        let moved: Vec<_> = plan.moved.iter().map(|key| self.indices[key]).collect();
        for &index in &moved {
            let previous = self.nodes[index].row.parent.clone();
            self.children_mut(previous.as_ref())
                .retain(|child| *child != index);
            self.nodes[index].row.parent.clone_from(&new_parent);
        }
        let siblings = self.children_mut(new_parent.as_ref());
        let insertion = match plan.placement {
            DropPlacement::Into => siblings.len(),
            DropPlacement::Before | DropPlacement::After => {
                siblings
                    .iter()
                    .take_while(|index| **index != target)
                    .count()
                    + usize::from(plan.placement == DropPlacement::After)
            }
        };
        siblings.splice(insertion..insertion, moved);
        if let Some(parent) = new_parent.filter(|_| plan.placement == DropPlacement::Into) {
            self.expanded.insert(parent);
        }
        self.ordered = self.flatten(false);
        self.refresh_visible();
        true
    }

    fn children_mut(&mut self, parent: Option<&K>) -> &mut Vec<usize> {
        match parent {
            Some(key) => &mut self.nodes[self.indices[key]].children,
            None => &mut self.roots,
        }
    }

    fn ancestors<'a>(&'a self, key: &'a K) -> impl Iterator<Item = &'a K> + 'a {
        let parent = |key: &K| self.nodes[*self.indices.get(key)?].row.parent();
        std::iter::successors(parent(key), move |key| parent(key))
    }

    fn refresh_visible(&mut self) {
        self.visible = self.flatten(true);
        self.visible_positions.clear();
        self.visible_positions.resize(self.nodes.len(), None);
        for (position, (index, _)) in self.visible.iter().enumerate() {
            self.visible_positions[*index] = Some(position);
        }
        self.active = self.active.as_ref().and_then(|key| {
            std::iter::once(key)
                .chain(self.ancestors(key))
                .find(|key| self.position(key).is_some())
                .cloned()
        });
        self.anchor = self
            .anchor
            .take()
            .filter(|key| self.position(key).is_some());
    }

    fn flatten(&self, visible_only: bool) -> Vec<(usize, usize)> {
        let mut output = Vec::new();
        let mut pending: Vec<_> = self.roots.iter().rev().map(|index| (*index, 0)).collect();
        while let Some((index, depth)) = pending.pop() {
            output.push((index, depth));
            let node = &self.nodes[index];
            if !visible_only || self.expanded.contains(node.row.key()) {
                pending.extend(node.children.iter().rev().map(|index| (*index, depth + 1)));
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TreeModel<u8> {
        let mut tree = TreeModel::new();
        tree.replace([
            TreeRow::new(2, Some(1), "child"),
            TreeRow::new(1, None, "parent"),
            TreeRow::new(3, None, "target"),
            TreeRow::new(4, None, "last"),
        ])
        .expect("valid tree");
        tree
    }

    #[test]
    fn shift_range_uses_expanded_visible_order_and_preserves_hidden_selection() {
        let mut tree = sample();
        assert!(tree.set_expanded(&1, true));
        tree.select(&1, false, false);
        tree.select(&3, true, false);
        assert_eq!(tree.selection(), [1, 2, 3]);
        tree.select(&2, false, true);
        tree.select(&2, false, true);
        assert!(tree.set_expanded(&1, false));
        assert_eq!(tree.selection(), [1, 2, 3]);
        assert_eq!(tree.active, Some(1));
        assert_eq!(tree.position(&2), None);
    }

    #[test]
    fn reorder_moves_selected_subtrees_once_and_rejects_ancestor_drop() {
        let mut tree = sample();
        assert!(tree.plan_reorder(&[1], &2, DropPlacement::After).is_none());
        let original_id = tree.nodes[tree.indices[&2]].id;
        let plan = tree
            .plan_reorder(&[4, 2, 1], &3, DropPlacement::Into)
            .expect("valid move");
        assert_eq!(plan.moved, [1, 4]);
        assert!(tree.apply_reorder(&plan));
        tree.set_expanded(&1, true);
        assert_eq!(
            (0..4)
                .filter_map(|position| tree.key_at(position).copied())
                .collect::<Vec<_>>(),
            [3, 1, 2, 4]
        );
        assert_eq!(tree.nodes[tree.indices[&2]].id, original_id);
    }

    #[test]
    fn rejects_cycles_and_keeps_existing_keys_stable_on_replacement() {
        let mut tree = sample();
        let original_id = tree.nodes[tree.indices[&1]].id;
        assert_eq!(
            tree.replace([TreeRow::new(1, Some(2), "a"), TreeRow::new(2, Some(1), "b")]),
            Err(TreeError::Cycle)
        );
        assert_eq!(tree.key_at(0), Some(&1));
        tree.replace([TreeRow::new(5, None, "new"), TreeRow::new(1, None, "a")])
            .expect("valid tree");
        assert_eq!(tree.nodes[tree.indices[&1]].id, original_id);
    }
}
