use super::*;
use crate::mutation::{CanvasCommittedMutation, CanvasMutationJournal, CanvasPreparedMutation};

impl CanvasDocument {
    pub(crate) fn apply(&mut self, command: DocumentCommand) -> Result<(), DocumentError> {
        match command {
            DocumentCommand::InsertNode(node) => self.insert_node_rule(node),
            DocumentCommand::UpdateNode(node) => self.update_node_rule(node),
            DocumentCommand::RemoveNode(id) => self.remove_node_rule(&id).map(drop),
            DocumentCommand::InsertEdge(edge) => self.insert_edge_rule(edge),
            DocumentCommand::UpdateEdge(edge) => self.update_edge_rule(edge),
            DocumentCommand::RemoveEdge(id) => self.remove_edge_rule(&id).map(drop),
            DocumentCommand::InsertShape(shape) => self.insert_shape_rule(shape),
            DocumentCommand::UpdateShape(shape) => self.update_shape_rule(shape),
            DocumentCommand::RemoveShape(id) => self.remove_shape_rule(&id).map(drop),
            DocumentCommand::SetRecordParent { child, parent } => {
                self.set_record_parent_rule(child, parent)
            }
            DocumentCommand::ClearRecordParent { child } => {
                self.relations.clear_parent(&child);
                Ok(())
            }
            DocumentCommand::AddRecordToGroup { group, member } => {
                self.add_record_to_group_rule(group, member)
            }
            DocumentCommand::RemoveRecordFromGroup { group, member } => {
                self.relations.remove_from_group(&group, &member);
                Ok(())
            }
            DocumentCommand::SetRecordBinding(binding) => self.set_record_binding_rule(binding),
            DocumentCommand::RemoveRecordBinding { id } => {
                self.relations.remove_binding(&id);
                Ok(())
            }
        }
    }

    pub fn apply_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<(), DocumentError> {
        self.apply_transaction_with_diff(transaction).map(drop)
    }

    pub fn apply_transaction_with_diff(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        self.commit_transaction(transaction)
            .map(CanvasCommittedMutation::into_diff)
    }

    pub fn commit_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasCommittedMutation, DocumentError> {
        CanvasMutationJournal::commit(self, transaction)
    }

    pub fn commit_transaction_with_kind_registry(
        &mut self,
        transaction: CanvasTransaction,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<CanvasCommittedMutation, DocumentError> {
        CanvasMutationJournal::commit_with_kind_registry(self, transaction, kind_registry)
    }

    pub(crate) fn prepare_transaction_with_kind_registry(
        &self,
        transaction: CanvasTransaction,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<CanvasPreparedMutation, DocumentError> {
        CanvasMutationJournal::prepare_with_kind_registry(self, transaction, kind_registry)
    }

    pub fn invert_transaction(
        &self,
        transaction: &CanvasTransaction,
    ) -> Result<CanvasTransaction, DocumentError> {
        let mut draft = self.clone();
        let mut inverse_segments = Vec::new();

        for command in &transaction.commands {
            inverse_segments.push(draft.inverse_for(command)?);
            draft.apply(command.clone())?;
        }

        Ok(CanvasTransaction {
            commands: inverse_segments.into_iter().rev().flatten().collect(),
            metadata: CanvasValue::new(),
        })
    }

    pub(super) fn insert_node_rule(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DocumentError::DuplicateNode(node.id));
        }
        Self::validate_node(&node)?;

        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    fn update_node_rule(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        if !self.nodes.contains_key(&node.id) {
            return Err(DocumentError::MissingNode(node.id));
        }
        Self::validate_node(&node)?;

        let mut draft = self.clone();
        draft.nodes.insert(node.id.clone(), node);
        draft.validate_integrity()?;
        *self = draft;
        Ok(())
    }

    fn remove_node_rule(&mut self, id: &NodeId) -> Result<CanvasNode, DocumentError> {
        let Some(node) = self.nodes.shift_remove(id) else {
            return Err(DocumentError::MissingNode(id.clone()));
        };

        self.edges
            .retain(|_, edge| edge.source.node_id != *id && edge.target.node_id != *id);
        Ok(node)
    }

    pub(super) fn insert_edge_rule(&mut self, edge: CanvasEdge) -> Result<(), DocumentError> {
        if self.edges.contains_key(&edge.id) {
            return Err(DocumentError::DuplicateEdge(edge.id));
        }
        self.validate_edge(&edge)?;

        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    fn update_edge_rule(&mut self, edge: CanvasEdge) -> Result<(), DocumentError> {
        if !self.edges.contains_key(&edge.id) {
            return Err(DocumentError::MissingEdge(edge.id));
        }
        self.validate_edge(&edge)?;

        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    fn remove_edge_rule(&mut self, id: &EdgeId) -> Result<CanvasEdge, DocumentError> {
        self.edges
            .shift_remove(id)
            .ok_or_else(|| DocumentError::MissingEdge(id.clone()))
    }

    pub(super) fn insert_shape_rule(&mut self, shape: CanvasShape) -> Result<(), DocumentError> {
        if self.shapes.contains_key(&shape.id) {
            return Err(DocumentError::DuplicateShape(shape.id));
        }

        self.shapes.insert(shape.id.clone(), shape);
        Ok(())
    }

    fn update_shape_rule(&mut self, shape: CanvasShape) -> Result<(), DocumentError> {
        if !self.shapes.contains_key(&shape.id) {
            return Err(DocumentError::MissingShape(shape.id));
        }

        self.shapes.insert(shape.id.clone(), shape);
        Ok(())
    }

    fn remove_shape_rule(&mut self, id: &ShapeId) -> Result<CanvasShape, DocumentError> {
        self.shapes
            .shift_remove(id)
            .ok_or_else(|| DocumentError::MissingShape(id.clone()))
    }

    fn set_record_parent_rule(
        &mut self,
        child: CanvasRecordId,
        parent: CanvasRecordId,
    ) -> Result<(), DocumentError> {
        if child == parent {
            return Err(DocumentError::SelfParentRelation(child));
        }
        self.validate_record_id(&child)?;
        self.validate_record_id(&parent)?;
        self.relations.set_parent(child, parent);
        Ok(())
    }

    fn add_record_to_group_rule(
        &mut self,
        group: CanvasRecordId,
        member: CanvasRecordId,
    ) -> Result<(), DocumentError> {
        if group == member {
            return Err(DocumentError::SelfParentRelation(group));
        }
        self.validate_record_id(&group)?;
        self.validate_record_id(&member)?;
        self.relations.add_to_group(group, member);
        Ok(())
    }

    fn set_record_binding_rule(
        &mut self,
        binding: CanvasRecordBindingRelation,
    ) -> Result<(), DocumentError> {
        if binding.source == binding.target {
            return Err(DocumentError::SelfBindingRelation(binding.source));
        }
        self.validate_record_id(&binding.source)?;
        self.validate_record_id(&binding.target)?;
        self.relations.set_binding(binding);
        Ok(())
    }

    // Fixture helpers are test-only; production mutations go through builder or journaled paths.
    #[cfg(test)]
    pub(crate) fn insert_node(&mut self, node: CanvasNode) -> Result<(), DocumentError> {
        self.insert_node_rule(node)
    }

    #[cfg(test)]
    pub(crate) fn remove_node(&mut self, id: &NodeId) -> Result<CanvasNode, DocumentError> {
        self.remove_node_rule(id)
    }

    #[cfg(test)]
    pub(crate) fn insert_edge(&mut self, edge: CanvasEdge) -> Result<(), DocumentError> {
        self.insert_edge_rule(edge)
    }

    #[cfg(test)]
    pub(crate) fn insert_shape(&mut self, shape: CanvasShape) -> Result<(), DocumentError> {
        self.insert_shape_rule(shape)
    }

    fn inverse_for(
        &self,
        command: &DocumentCommand,
    ) -> Result<Vec<DocumentCommand>, DocumentError> {
        match command {
            DocumentCommand::InsertNode(node) => {
                if self.nodes.contains_key(&node.id) {
                    return Err(DocumentError::DuplicateNode(node.id.clone()));
                }
                Self::validate_node(node)?;
                Ok(vec![DocumentCommand::RemoveNode(node.id.clone())])
            }
            DocumentCommand::UpdateNode(node) => Ok(vec![DocumentCommand::UpdateNode(
                self.nodes
                    .get(&node.id)
                    .ok_or_else(|| DocumentError::MissingNode(node.id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::RemoveNode(id) => {
                let node = self
                    .nodes
                    .get(id)
                    .ok_or_else(|| DocumentError::MissingNode(id.clone()))?
                    .clone();
                let mut inverse = vec![DocumentCommand::InsertNode(node)];
                inverse.extend(
                    self.edges
                        .values()
                        .filter(|edge| edge.source.node_id == *id || edge.target.node_id == *id)
                        .cloned()
                        .map(DocumentCommand::InsertEdge),
                );
                Ok(inverse)
            }
            DocumentCommand::InsertEdge(edge) => {
                if self.edges.contains_key(&edge.id) {
                    return Err(DocumentError::DuplicateEdge(edge.id.clone()));
                }
                self.validate_edge(edge)?;
                Ok(vec![DocumentCommand::RemoveEdge(edge.id.clone())])
            }
            DocumentCommand::UpdateEdge(edge) => Ok(vec![DocumentCommand::UpdateEdge(
                self.edges
                    .get(&edge.id)
                    .ok_or_else(|| DocumentError::MissingEdge(edge.id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::RemoveEdge(id) => Ok(vec![DocumentCommand::InsertEdge(
                self.edges
                    .get(id)
                    .ok_or_else(|| DocumentError::MissingEdge(id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::InsertShape(shape) => {
                if self.shapes.contains_key(&shape.id) {
                    return Err(DocumentError::DuplicateShape(shape.id.clone()));
                }
                Ok(vec![DocumentCommand::RemoveShape(shape.id.clone())])
            }
            DocumentCommand::UpdateShape(shape) => Ok(vec![DocumentCommand::UpdateShape(
                self.shapes
                    .get(&shape.id)
                    .ok_or_else(|| DocumentError::MissingShape(shape.id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::RemoveShape(id) => Ok(vec![DocumentCommand::InsertShape(
                self.shapes
                    .get(id)
                    .ok_or_else(|| DocumentError::MissingShape(id.clone()))?
                    .clone(),
            )]),
            DocumentCommand::SetRecordParent { child, parent } => {
                if child == parent {
                    return Err(DocumentError::SelfParentRelation(child.clone()));
                }
                self.validate_record_id(child)?;
                self.validate_record_id(parent)?;
                Ok(match self.relations.parent_of(child).cloned() {
                    Some(previous) => vec![DocumentCommand::SetRecordParent {
                        child: child.clone(),
                        parent: previous,
                    }],
                    None => vec![DocumentCommand::ClearRecordParent {
                        child: child.clone(),
                    }],
                })
            }
            DocumentCommand::ClearRecordParent { child } => {
                self.validate_record_id(child)?;
                Ok(match self.relations.parent_of(child).cloned() {
                    Some(parent) => vec![DocumentCommand::SetRecordParent {
                        child: child.clone(),
                        parent,
                    }],
                    None => Vec::new(),
                })
            }
            DocumentCommand::AddRecordToGroup { group, member } => {
                if group == member {
                    return Err(DocumentError::SelfParentRelation(group.clone()));
                }
                self.validate_record_id(group)?;
                self.validate_record_id(member)?;
                let already_member = self.relations.groups_for(member).any(|id| id == group);
                Ok(if already_member {
                    Vec::new()
                } else {
                    vec![DocumentCommand::RemoveRecordFromGroup {
                        group: group.clone(),
                        member: member.clone(),
                    }]
                })
            }
            DocumentCommand::RemoveRecordFromGroup { group, member } => {
                self.validate_record_id(group)?;
                self.validate_record_id(member)?;
                let already_member = self.relations.groups_for(member).any(|id| id == group);
                Ok(if already_member {
                    vec![DocumentCommand::AddRecordToGroup {
                        group: group.clone(),
                        member: member.clone(),
                    }]
                } else {
                    Vec::new()
                })
            }
            DocumentCommand::SetRecordBinding(binding) => {
                if binding.source == binding.target {
                    return Err(DocumentError::SelfBindingRelation(binding.source.clone()));
                }
                self.validate_record_id(&binding.source)?;
                self.validate_record_id(&binding.target)?;
                Ok(match self.relations.binding(&binding.id).cloned() {
                    Some(previous) => vec![DocumentCommand::SetRecordBinding(previous)],
                    None => vec![DocumentCommand::RemoveRecordBinding {
                        id: binding.id.clone(),
                    }],
                })
            }
            DocumentCommand::RemoveRecordBinding { id } => Ok(match self.relations.binding(id) {
                Some(binding) => vec![DocumentCommand::SetRecordBinding(binding.clone())],
                None => Vec::new(),
            }),
        }
    }
}
