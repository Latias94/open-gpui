use super::*;

impl CanvasToolReducerContext<'_> {
    pub(crate) fn snap_delta_for_translation(
        &self,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> crate::snap::CanvasSnapResult {
        let mut selection = CanvasSelection::default();
        for id in node_ids {
            selection.insert_node(id.clone());
        }
        for id in shape_ids {
            selection.insert_shape(id.clone());
        }
        snap_delta_for_selection(
            self.document,
            &selection,
            delta,
            DEFAULT_SNAP_THRESHOLD,
            Some(self.kind_registry),
        )
    }
    pub(crate) fn snap_delta_for_resize(
        &self,
        handle: CanvasResizeHandle,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> crate::snap::CanvasSnapResult {
        let mut selection = CanvasSelection::default();
        for id in node_ids {
            selection.insert_node(id.clone());
        }
        for id in shape_ids {
            selection.insert_shape(id.clone());
        }
        snap_delta_for_resize_selection(
            self.document,
            &selection,
            handle,
            delta,
            DEFAULT_SNAP_THRESHOLD,
            Some(self.kind_registry),
        )
    }
}
