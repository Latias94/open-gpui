use super::*;

impl CanvasDocument {
    pub fn diff_against(&self, previous: &CanvasDocument) -> CanvasDocumentDiff {
        let mut diff = CanvasDocumentDiff::default();

        for id in previous.nodes.keys() {
            if !self.nodes.contains_key(id) {
                diff.record_remove(id.clone());
            }
        }

        for (id, node) in &self.nodes {
            match previous.nodes.get(id) {
                None => diff.record_insert(id.clone()),
                Some(previous_node) if previous_node != node => diff.record_update(id.clone()),
                Some(_) => {}
            }
        }

        for id in previous.edges.keys() {
            if !self.edges.contains_key(id) {
                diff.record_remove(id.clone());
            }
        }

        for (id, edge) in &self.edges {
            match previous.edges.get(id) {
                None => diff.record_insert(id.clone()),
                Some(previous_edge) if previous_edge != edge => diff.record_update(id.clone()),
                Some(_) => {}
            }
        }

        for id in previous.shapes.keys() {
            if !self.shapes.contains_key(id) {
                diff.record_remove(id.clone());
            }
        }

        for (id, shape) in &self.shapes {
            match previous.shapes.get(id) {
                None => diff.record_insert(id.clone()),
                Some(previous_shape) if previous_shape != shape => diff.record_update(id.clone()),
                Some(_) => {}
            }
        }

        diff.metadata_changed = self.metadata != previous.metadata;
        diff.relations_changed = self.relations != previous.relations;
        diff
    }
}
