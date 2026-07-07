use super::*;

impl CanvasDocument {
    pub fn from_snapshot(snapshot: CanvasSnapshot) -> Result<Self, DocumentError> {
        Self::from_snapshot_with_kind_registry(snapshot, &CanvasKindRegistry::open())
    }

    pub fn from_snapshot_with_kind_registry(
        snapshot: CanvasSnapshot,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<Self, DocumentError> {
        let snapshot = migrate_canvas_snapshot(snapshot)?;

        let mut builder = Self::builder()
            .with_format_version(snapshot.format_version)
            .with_metadata(snapshot.metadata)
            .with_relations(snapshot.relations);

        for node in snapshot.nodes {
            let node = kind_registry.normalize_node(node)?;
            builder.add_node(node)?;
        }

        for shape in snapshot.shapes {
            let shape = kind_registry.normalize_shape(shape)?;
            builder.add_shape(shape)?;
        }

        for edge in snapshot.edges {
            let edge = kind_registry.normalize_edge(edge)?;
            builder.add_edge(edge)?;
        }

        builder.build_with_kind_registry(kind_registry)
    }

    pub fn to_snapshot(&self) -> CanvasSnapshot {
        CanvasSnapshot {
            format_version: self.format_version,
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.values().cloned().collect(),
            shapes: self.shapes.values().cloned().collect(),
            metadata: self.metadata.clone(),
            relations: self.relations.clone(),
        }
    }
}

impl TryFrom<CanvasSnapshot> for CanvasDocument {
    type Error = DocumentError;

    fn try_from(value: CanvasSnapshot) -> Result<Self, Self::Error> {
        Self::from_snapshot(value)
    }
}

impl From<&CanvasDocument> for CanvasSnapshot {
    fn from(value: &CanvasDocument) -> Self {
        value.to_snapshot()
    }
}
