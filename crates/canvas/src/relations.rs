use crate::CanvasRecordId;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordParentRelation {
    pub child: CanvasRecordId,
    pub parent: CanvasRecordId,
}

impl CanvasRecordParentRelation {
    pub fn new(child: impl Into<CanvasRecordId>, parent: impl Into<CanvasRecordId>) -> Self {
        Self {
            child: child.into(),
            parent: parent.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordGroupRelation {
    pub group: CanvasRecordId,
    pub member: CanvasRecordId,
}

impl CanvasRecordGroupRelation {
    pub fn new(group: impl Into<CanvasRecordId>, member: impl Into<CanvasRecordId>) -> Self {
        Self {
            group: group.into(),
            member: member.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordRelations {
    #[serde(default)]
    parents: Vec<CanvasRecordParentRelation>,
    #[serde(default)]
    groups: Vec<CanvasRecordGroupRelation>,
}

impl CanvasRecordRelations {
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty() && self.groups.is_empty()
    }

    pub fn parents(&self) -> impl Iterator<Item = &CanvasRecordParentRelation> {
        self.parents.iter()
    }

    pub fn groups(&self) -> impl Iterator<Item = &CanvasRecordGroupRelation> {
        self.groups.iter()
    }

    pub fn parent_of(&self, child: &CanvasRecordId) -> Option<&CanvasRecordId> {
        self.parents
            .iter()
            .find(|relation| &relation.child == child)
            .map(|relation| &relation.parent)
    }

    pub fn children_of(&self, parent: &CanvasRecordId) -> impl Iterator<Item = &CanvasRecordId> {
        self.parents
            .iter()
            .filter(move |relation| &relation.parent == parent)
            .map(|relation| &relation.child)
    }

    pub fn members_of(&self, group: &CanvasRecordId) -> impl Iterator<Item = &CanvasRecordId> {
        self.groups
            .iter()
            .filter(move |relation| &relation.group == group)
            .map(|relation| &relation.member)
    }

    pub fn groups_for(&self, member: &CanvasRecordId) -> impl Iterator<Item = &CanvasRecordId> {
        self.groups
            .iter()
            .filter(move |relation| &relation.member == member)
            .map(|relation| &relation.group)
    }

    pub(crate) fn set_parent(
        &mut self,
        child: CanvasRecordId,
        parent: CanvasRecordId,
    ) -> Option<CanvasRecordId> {
        if let Some(relation) = self
            .parents
            .iter_mut()
            .find(|relation| relation.child == child)
        {
            if relation.parent == parent {
                return Some(parent);
            }

            let previous = std::mem::replace(&mut relation.parent, parent);
            return Some(previous);
        }

        self.parents
            .push(CanvasRecordParentRelation { child, parent });
        None
    }

    pub(crate) fn clear_parent(&mut self, child: &CanvasRecordId) -> Option<CanvasRecordId> {
        let index = self
            .parents
            .iter()
            .position(|relation| &relation.child == child)?;
        Some(self.parents.remove(index).parent)
    }

    pub(crate) fn add_to_group(&mut self, group: CanvasRecordId, member: CanvasRecordId) -> bool {
        if self
            .groups
            .iter()
            .any(|relation| relation.group == group && relation.member == member)
        {
            return false;
        }

        self.groups
            .push(CanvasRecordGroupRelation { group, member });
        true
    }

    pub(crate) fn remove_from_group(
        &mut self,
        group: &CanvasRecordId,
        member: &CanvasRecordId,
    ) -> bool {
        let Some(index) = self
            .groups
            .iter()
            .position(|relation| &relation.group == group && &relation.member == member)
        else {
            return false;
        };
        self.groups.remove(index);
        true
    }

    pub(crate) fn prune_missing_records(&mut self, existing: &IndexSet<CanvasRecordId>) -> bool {
        let parent_count = self.parents.len();
        self.parents.retain(|relation| {
            existing.contains(&relation.child) && existing.contains(&relation.parent)
        });

        let group_count = self.groups.len();
        self.groups.retain(|relation| {
            existing.contains(&relation.group) && existing.contains(&relation.member)
        });

        self.parents.len() != parent_count || self.groups.len() != group_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeId, ShapeId};

    #[test]
    fn relation_defaults_are_empty() {
        let relations = CanvasRecordRelations::default();

        assert!(relations.is_empty());
        assert_eq!(relations.parents().count(), 0);
        assert_eq!(relations.groups().count(), 0);
    }

    #[test]
    fn parent_and_child_lookup_use_record_ids() {
        let mut relations = CanvasRecordRelations::default();
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let parent = CanvasRecordId::Shape(ShapeId::from("frame"));

        assert_eq!(relations.set_parent(child.clone(), parent.clone()), None);

        assert_eq!(relations.parent_of(&child), Some(&parent));
        assert_eq!(
            relations.children_of(&parent).cloned().collect::<Vec<_>>(),
            vec![child]
        );
    }

    #[test]
    fn group_memberships_are_deduplicated() {
        let mut relations = CanvasRecordRelations::default();
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        let member = CanvasRecordId::Node(NodeId::from("member"));

        assert!(relations.add_to_group(group.clone(), member.clone()));
        assert!(!relations.add_to_group(group.clone(), member.clone()));

        assert_eq!(
            relations.members_of(&group).cloned().collect::<Vec<_>>(),
            vec![member.clone()]
        );
        assert_eq!(
            relations.groups_for(&member).cloned().collect::<Vec<_>>(),
            vec![group]
        );
    }

    #[test]
    fn prune_missing_records_removes_dangling_relations() {
        let mut relations = CanvasRecordRelations::default();
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        let member = CanvasRecordId::Node(NodeId::from("member"));
        let missing = CanvasRecordId::Node(NodeId::from("missing"));
        relations.set_parent(member.clone(), group.clone());
        relations.add_to_group(group.clone(), member.clone());
        relations.add_to_group(group.clone(), missing);

        let existing = IndexSet::from_iter([group.clone(), member.clone()]);

        assert!(relations.prune_missing_records(&existing));
        assert_eq!(relations.parent_of(&member), Some(&group));
        assert_eq!(
            relations.members_of(&group).cloned().collect::<Vec<_>>(),
            vec![member]
        );
    }
}
