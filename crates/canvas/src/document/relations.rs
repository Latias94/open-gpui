use super::*;
use indexmap::{IndexMap, IndexSet};

impl CanvasDocument {
    pub(crate) fn prune_missing_relations(&mut self) -> bool {
        let existing = self.record_id_set();
        self.relations.prune_missing_records(&existing)
    }

    pub fn validate_relations(&self) -> Result<(), DocumentError> {
        let mut parent_children = IndexSet::new();
        for relation in self.relations.parents() {
            if !parent_children.insert(relation.child.clone()) {
                return Err(DocumentError::DuplicateParentRelation(
                    relation.child.clone(),
                ));
            }
            if relation.child == relation.parent {
                return Err(DocumentError::SelfParentRelation(relation.child.clone()));
            }
            self.validate_record_id(&relation.child)?;
            self.validate_record_id(&relation.parent)?;
        }

        let mut group_relations = IndexSet::new();
        for relation in self.relations.groups() {
            if !group_relations.insert((relation.group.clone(), relation.member.clone())) {
                return Err(DocumentError::DuplicateGroupRelation {
                    group: relation.group.clone(),
                    member: relation.member.clone(),
                });
            }
            if relation.group == relation.member {
                return Err(DocumentError::SelfParentRelation(relation.group.clone()));
            }
            self.validate_record_id(&relation.group)?;
            self.validate_record_id(&relation.member)?;
        }

        let mut binding_ids = IndexSet::new();
        for relation in self.relations.bindings() {
            if !binding_ids.insert(relation.id.clone()) {
                return Err(DocumentError::DuplicateBindingRelation(relation.id.clone()));
            }
            if relation.source == relation.target {
                return Err(DocumentError::SelfBindingRelation(relation.source.clone()));
            }
            self.validate_record_id(&relation.source)?;
            self.validate_record_id(&relation.target)?;
        }

        self.validate_relation_graph_is_acyclic()?;

        Ok(())
    }

    fn validate_relation_graph_is_acyclic(&self) -> Result<(), DocumentError> {
        let mut graph = IndexMap::<CanvasRecordId, Vec<CanvasRecordId>>::new();
        for relation in self.relations.parents() {
            graph
                .entry(relation.parent.clone())
                .or_default()
                .push(relation.child.clone());
        }
        for relation in self.relations.groups() {
            graph
                .entry(relation.group.clone())
                .or_default()
                .push(relation.member.clone());
        }

        let mut visited = IndexSet::new();
        let mut visiting = IndexSet::new();
        for record_id in graph.keys() {
            self.validate_relation_subgraph_is_acyclic(
                record_id,
                &graph,
                &mut visited,
                &mut visiting,
            )?;
        }
        Ok(())
    }

    fn validate_relation_subgraph_is_acyclic(
        &self,
        record_id: &CanvasRecordId,
        graph: &IndexMap<CanvasRecordId, Vec<CanvasRecordId>>,
        visited: &mut IndexSet<CanvasRecordId>,
        visiting: &mut IndexSet<CanvasRecordId>,
    ) -> Result<(), DocumentError> {
        if visited.contains(record_id) {
            return Ok(());
        }
        if !visiting.insert(record_id.clone()) {
            return Err(DocumentError::CyclicRecordRelation(record_id.clone()));
        }

        if let Some(children) = graph.get(record_id) {
            for child in children {
                self.validate_relation_subgraph_is_acyclic(child, graph, visited, visiting)?;
            }
        }

        visiting.shift_remove(record_id);
        visited.insert(record_id.clone());
        Ok(())
    }
}
