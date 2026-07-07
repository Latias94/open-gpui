use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasDocumentBuilder {
    document: CanvasDocument,
}

impl CanvasDocumentBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_format_version(mut self, format_version: u32) -> Self {
        self.document.format_version = format_version;
        self
    }

    pub fn with_metadata(mut self, metadata: CanvasValue) -> Self {
        self.document.metadata = metadata;
        self
    }

    pub fn with_relations(mut self, relations: CanvasRecordRelations) -> Self {
        self.document.relations = relations;
        self
    }

    pub fn add_node(&mut self, node: CanvasNode) -> Result<&mut Self, DocumentError> {
        self.document.insert_node_rule(node)?;
        Ok(self)
    }

    pub fn add_edge(&mut self, edge: CanvasEdge) -> Result<&mut Self, DocumentError> {
        self.document.insert_edge_rule(edge)?;
        Ok(self)
    }

    pub fn add_shape(&mut self, shape: CanvasShape) -> Result<&mut Self, DocumentError> {
        self.document.insert_shape_rule(shape)?;
        Ok(self)
    }

    pub fn build(mut self) -> Result<CanvasDocument, DocumentError> {
        self.document.prune_missing_relations();
        self.document.validate_relations()?;
        Ok(self.document)
    }

    pub fn build_with_kind_registry(
        mut self,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<CanvasDocument, DocumentError> {
        self.document.prune_missing_relations();
        self.document.validate_relations()?;
        kind_registry.validate_document(&self.document)?;
        Ok(self.document)
    }
}
