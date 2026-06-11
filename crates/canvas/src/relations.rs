use crate::{BindingId, CanvasRecordId};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasRecordRelationKind {
    Parent,
    Group,
    Binding,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasRecordRelationKey<'a> {
    Parent {
        child: &'a CanvasRecordId,
        parent: &'a CanvasRecordId,
    },
    Group {
        group: &'a CanvasRecordId,
        member: &'a CanvasRecordId,
    },
    Binding {
        id: &'a BindingId,
    },
}

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordBindingRelation {
    pub id: BindingId,
    #[serde(default = "default_binding_kind")]
    pub kind: String,
    pub source: CanvasRecordId,
    pub target: CanvasRecordId,
}

impl CanvasRecordBindingRelation {
    pub fn new(
        id: impl Into<BindingId>,
        source: impl Into<CanvasRecordId>,
        target: impl Into<CanvasRecordId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: default_binding_kind(),
            source: source.into(),
            target: target.into(),
        }
    }

    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }
}

fn default_binding_kind() -> String {
    "binding".to_string()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasRecordRelation {
    Parent(CanvasRecordParentRelation),
    Group(CanvasRecordGroupRelation),
    Binding(CanvasRecordBindingRelation),
}

impl CanvasRecordRelation {
    pub fn kind(&self) -> CanvasRecordRelationKind {
        match self {
            Self::Parent(_) => CanvasRecordRelationKind::Parent,
            Self::Group(_) => CanvasRecordRelationKind::Group,
            Self::Binding(_) => CanvasRecordRelationKind::Binding,
        }
    }

    pub fn relation_key(&self) -> CanvasRecordRelationKey<'_> {
        match self {
            Self::Parent(relation) => CanvasRecordRelationKey::Parent {
                child: &relation.child,
                parent: &relation.parent,
            },
            Self::Group(relation) => CanvasRecordRelationKey::Group {
                group: &relation.group,
                member: &relation.member,
            },
            Self::Binding(relation) => CanvasRecordRelationKey::Binding { id: &relation.id },
        }
    }
}

impl From<CanvasRecordParentRelation> for CanvasRecordRelation {
    fn from(value: CanvasRecordParentRelation) -> Self {
        Self::Parent(value)
    }
}

impl From<CanvasRecordGroupRelation> for CanvasRecordRelation {
    fn from(value: CanvasRecordGroupRelation) -> Self {
        Self::Group(value)
    }
}

impl From<CanvasRecordBindingRelation> for CanvasRecordRelation {
    fn from(value: CanvasRecordBindingRelation) -> Self {
        Self::Binding(value)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CanvasRecordRelations {
    #[serde(default)]
    parents: Vec<CanvasRecordParentRelation>,
    #[serde(default)]
    groups: Vec<CanvasRecordGroupRelation>,
    #[serde(default)]
    bindings: Vec<CanvasRecordBindingRelation>,
}

impl PartialEq for CanvasRecordRelations {
    fn eq(&self, other: &Self) -> bool {
        self.parents.len() == other.parents.len()
            && self.groups.len() == other.groups.len()
            && self.bindings.len() == other.bindings.len()
            && self.parents.iter().all(|relation| {
                other.contains_relation(&CanvasRecordRelation::Parent(relation.clone()))
            })
            && self.groups.iter().all(|relation| {
                other.contains_relation(&CanvasRecordRelation::Group(relation.clone()))
            })
            && self.bindings.iter().all(|relation| {
                other.contains_relation(&CanvasRecordRelation::Binding(relation.clone()))
            })
    }
}

impl Eq for CanvasRecordRelations {}

impl CanvasRecordRelations {
    pub fn builder() -> CanvasRecordRelationsBuilder {
        CanvasRecordRelationsBuilder::new()
    }

    pub fn is_empty(&self) -> bool {
        self.parents.is_empty() && self.groups.is_empty() && self.bindings.is_empty()
    }

    pub fn parents(&self) -> impl Iterator<Item = &CanvasRecordParentRelation> {
        self.parents.iter()
    }

    pub fn groups(&self) -> impl Iterator<Item = &CanvasRecordGroupRelation> {
        self.groups.iter()
    }

    pub fn bindings(&self) -> impl Iterator<Item = &CanvasRecordBindingRelation> {
        self.bindings.iter()
    }

    pub fn binding(&self, id: &BindingId) -> Option<&CanvasRecordBindingRelation> {
        self.bindings.iter().find(|relation| &relation.id == id)
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

    pub fn bindings_for(
        &self,
        record: &CanvasRecordId,
    ) -> impl Iterator<Item = &CanvasRecordBindingRelation> {
        self.bindings
            .iter()
            .filter(move |relation| &relation.source == record || &relation.target == record)
    }

    pub(crate) fn collect_related_records(
        &self,
        seeds: impl IntoIterator<Item = CanvasRecordId>,
        mut can_include: impl FnMut(&CanvasRecordId) -> bool,
    ) -> IndexSet<CanvasRecordId> {
        let mut records = IndexSet::new();
        let mut pending = Vec::new();

        for record_id in seeds {
            Self::insert_related_record(record_id, &mut records, &mut pending, &mut can_include);
        }

        while let Some(record_id) = pending.pop() {
            for child in self.children_of(&record_id).cloned().collect::<Vec<_>>() {
                Self::insert_related_record(child, &mut records, &mut pending, &mut can_include);
            }
            for member in self.members_of(&record_id).cloned().collect::<Vec<_>>() {
                Self::insert_related_record(member, &mut records, &mut pending, &mut can_include);
            }
        }

        records
    }

    pub fn contains_relation(&self, relation: &CanvasRecordRelation) -> bool {
        match relation {
            CanvasRecordRelation::Parent(relation) => {
                self.parent_of(&relation.child) == Some(&relation.parent)
            }
            CanvasRecordRelation::Group(relation) => self.contains_group_relation(relation),
            CanvasRecordRelation::Binding(relation) => self.binding(&relation.id) == Some(relation),
        }
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

    pub(crate) fn set_binding(
        &mut self,
        binding: CanvasRecordBindingRelation,
    ) -> Option<CanvasRecordBindingRelation> {
        if let Some(relation) = self
            .bindings
            .iter_mut()
            .find(|relation| relation.id == binding.id)
        {
            if *relation == binding {
                return Some(binding);
            }

            let previous = std::mem::replace(relation, binding);
            return Some(previous);
        }

        self.bindings.push(binding);
        None
    }

    pub(crate) fn remove_binding(&mut self, id: &BindingId) -> Option<CanvasRecordBindingRelation> {
        let index = self
            .bindings
            .iter()
            .position(|relation| &relation.id == id)?;
        Some(self.bindings.remove(index))
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

        let binding_count = self.bindings.len();
        self.bindings.retain(|relation| {
            existing.contains(&relation.source) && existing.contains(&relation.target)
        });

        self.parents.len() != parent_count
            || self.groups.len() != group_count
            || self.bindings.len() != binding_count
    }

    fn contains_group_relation(&self, relation: &CanvasRecordGroupRelation) -> bool {
        self.groups
            .iter()
            .any(|existing| existing.group == relation.group && existing.member == relation.member)
    }

    fn insert_related_record<F>(
        record_id: CanvasRecordId,
        records: &mut IndexSet<CanvasRecordId>,
        pending: &mut Vec<CanvasRecordId>,
        can_include: &mut F,
    ) where
        F: FnMut(&CanvasRecordId) -> bool,
    {
        if !can_include(&record_id) {
            return;
        }
        if records.insert(record_id.clone()) {
            pending.push(record_id);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasRecordRelationsBuilder {
    relations: CanvasRecordRelations,
}

impl CanvasRecordRelationsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_parent(
        &mut self,
        child: impl Into<CanvasRecordId>,
        parent: impl Into<CanvasRecordId>,
    ) -> &mut Self {
        self.relations.set_parent(child.into(), parent.into());
        self
    }

    pub fn add_group_member(
        &mut self,
        group: impl Into<CanvasRecordId>,
        member: impl Into<CanvasRecordId>,
    ) -> &mut Self {
        self.relations.add_to_group(group.into(), member.into());
        self
    }

    pub fn add_relation(&mut self, relation: impl Into<CanvasRecordRelation>) -> &mut Self {
        match relation.into() {
            CanvasRecordRelation::Parent(relation) => {
                self.add_parent(relation.child, relation.parent);
            }
            CanvasRecordRelation::Group(relation) => {
                self.add_group_member(relation.group, relation.member);
            }
            CanvasRecordRelation::Binding(relation) => {
                self.add_binding(relation);
            }
        }
        self
    }

    pub fn add_binding(&mut self, binding: CanvasRecordBindingRelation) -> &mut Self {
        self.relations.set_binding(binding);
        self
    }

    pub fn build(self) -> CanvasRecordRelations {
        self.relations
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
        assert_eq!(relations.bindings().count(), 0);
    }

    #[test]
    fn collect_related_records_expands_descendants_only() {
        let mut relations = CanvasRecordRelations::default();
        let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        let leaf = CanvasRecordId::Node(NodeId::from("leaf"));
        let outside = CanvasRecordId::Node(NodeId::from("outside"));

        relations.set_parent(group.clone(), frame.clone());
        relations.add_to_group(group.clone(), leaf.clone());
        relations.add_to_group(frame.clone(), outside.clone());

        let records = relations.collect_related_records([frame.clone()], |_| true);

        assert_eq!(records.len(), 4);
        assert!(records.contains(&frame));
        assert!(records.contains(&group));
        assert!(records.contains(&leaf));
        assert!(records.contains(&outside));
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
    fn relation_records_report_kind_and_identity() {
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let parent = CanvasRecordId::Shape(ShapeId::from("frame"));
        let parent_relation = CanvasRecordRelation::from(CanvasRecordParentRelation::new(
            child.clone(),
            parent.clone(),
        ));
        let group_relation = CanvasRecordRelation::from(CanvasRecordGroupRelation::new(
            parent.clone(),
            child.clone(),
        ));
        let binding_relation = CanvasRecordRelation::from(CanvasRecordBindingRelation::new(
            "binding",
            child.clone(),
            parent.clone(),
        ));

        assert_eq!(parent_relation.kind(), CanvasRecordRelationKind::Parent);
        assert_eq!(
            parent_relation.relation_key(),
            CanvasRecordRelationKey::Parent {
                child: &child,
                parent: &parent,
            }
        );
        assert_eq!(group_relation.kind(), CanvasRecordRelationKind::Group);
        assert_eq!(
            group_relation.relation_key(),
            CanvasRecordRelationKey::Group {
                group: &parent,
                member: &child,
            }
        );
        assert_eq!(binding_relation.kind(), CanvasRecordRelationKind::Binding);
        assert_eq!(
            binding_relation.relation_key(),
            CanvasRecordRelationKey::Binding {
                id: &BindingId::from("binding"),
            }
        );
    }

    #[test]
    fn relations_builder_constructs_structural_relation_sets() {
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let parent = CanvasRecordId::Shape(ShapeId::from("frame"));
        let mut builder = CanvasRecordRelations::builder();
        builder
            .add_parent(child.clone(), parent.clone())
            .add_group_member(parent.clone(), child.clone());

        let relations = builder.build();

        assert_eq!(relations.parent_of(&child), Some(&parent));
        assert!(relations.contains_relation(&CanvasRecordRelation::Group(
            CanvasRecordGroupRelation::new(parent, child)
        )));
    }

    #[test]
    fn relations_builder_accepts_unified_relation_records() {
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let parent = CanvasRecordId::Shape(ShapeId::from("frame"));
        let mut builder = CanvasRecordRelationsBuilder::new();
        builder.add_relation(CanvasRecordParentRelation::new(
            child.clone(),
            parent.clone(),
        ));
        builder.add_relation(CanvasRecordGroupRelation::new(
            parent.clone(),
            child.clone(),
        ));
        builder.add_relation(CanvasRecordBindingRelation::new(
            "binding",
            child.clone(),
            parent.clone(),
        ));

        let relations = builder.build();

        assert!(relations.contains_relation(&CanvasRecordRelation::Parent(
            CanvasRecordParentRelation::new(child.clone(), parent.clone())
        )));
        assert!(relations.contains_relation(&CanvasRecordRelation::Group(
            CanvasRecordGroupRelation::new(parent.clone(), child.clone())
        )));
        assert!(relations.contains_relation(&CanvasRecordRelation::Binding(
            CanvasRecordBindingRelation::new("binding", child, parent)
        )));
    }

    #[test]
    fn binding_relations_are_keyed_by_binding_id() {
        let source = CanvasRecordId::Node(NodeId::from("source"));
        let target = CanvasRecordId::Shape(ShapeId::from("target"));
        let replacement_target = CanvasRecordId::Shape(ShapeId::from("replacement-target"));
        let mut relations = CanvasRecordRelations::default();

        assert_eq!(
            relations.set_binding(CanvasRecordBindingRelation::new(
                "binding",
                source.clone(),
                target.clone(),
            )),
            None
        );
        assert_eq!(
            relations.set_binding(CanvasRecordBindingRelation::new(
                "binding",
                source.clone(),
                replacement_target.clone(),
            )),
            Some(CanvasRecordBindingRelation::new(
                "binding",
                source.clone(),
                target
            ))
        );

        assert_eq!(
            relations.bindings_for(&source).cloned().collect::<Vec<_>>(),
            vec![CanvasRecordBindingRelation::new(
                "binding",
                source,
                replacement_target
            )]
        );
        assert_eq!(
            relations.remove_binding(&BindingId::from("binding")),
            Some(CanvasRecordBindingRelation::new(
                "binding",
                CanvasRecordId::Node(NodeId::from("source")),
                CanvasRecordId::Shape(ShapeId::from("replacement-target")),
            ))
        );
        assert!(relations.is_empty());
    }

    #[test]
    fn relation_equality_uses_semantic_ordering() {
        let group_a = CanvasRecordId::Shape(ShapeId::from("group-a"));
        let group_b = CanvasRecordId::Shape(ShapeId::from("group-b"));
        let member_a = CanvasRecordId::Node(NodeId::from("member-a"));
        let member_b = CanvasRecordId::Node(NodeId::from("member-b"));

        let mut left = CanvasRecordRelations::default();
        left.set_parent(member_a.clone(), group_a.clone());
        left.set_parent(member_b.clone(), group_b.clone());
        left.add_to_group(group_a.clone(), member_a.clone());
        left.add_to_group(group_b.clone(), member_b.clone());
        left.set_binding(CanvasRecordBindingRelation::new(
            "binding-a",
            member_a.clone(),
            group_a.clone(),
        ));
        left.set_binding(CanvasRecordBindingRelation::new(
            "binding-b",
            member_b.clone(),
            group_b.clone(),
        ));

        let mut right = CanvasRecordRelations::default();
        right.set_parent(member_b.clone(), group_b.clone());
        right.set_parent(member_a.clone(), group_a.clone());
        right.add_to_group(group_b.clone(), member_b.clone());
        right.add_to_group(group_a.clone(), member_a.clone());
        right.set_binding(CanvasRecordBindingRelation::new(
            "binding-b",
            member_b,
            group_b,
        ));
        right.set_binding(CanvasRecordBindingRelation::new(
            "binding-a",
            member_a,
            group_a,
        ));

        assert_eq!(left, right);
    }

    #[test]
    fn prune_missing_records_removes_dangling_relations() {
        let mut relations = CanvasRecordRelations::default();
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        let member = CanvasRecordId::Node(NodeId::from("member"));
        let missing = CanvasRecordId::Node(NodeId::from("missing"));
        relations.set_parent(member.clone(), group.clone());
        relations.add_to_group(group.clone(), member.clone());
        relations.add_to_group(group.clone(), missing.clone());
        relations.set_binding(CanvasRecordBindingRelation::new(
            "binding",
            member.clone(),
            missing,
        ));

        let existing = IndexSet::from_iter([group.clone(), member.clone()]);

        assert!(relations.prune_missing_records(&existing));
        assert_eq!(relations.parent_of(&member), Some(&group));
        assert_eq!(
            relations.members_of(&group).cloned().collect::<Vec<_>>(),
            vec![member]
        );
        assert!(relations.bindings().next().is_none());
    }
}
