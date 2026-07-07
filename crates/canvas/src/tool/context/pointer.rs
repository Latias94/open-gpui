use super::*;

impl CanvasToolReducerContext<'_> {
    pub(crate) fn transform_handle_at(
        &self,
        point: Point<Pixels>,
    ) -> Option<CanvasTransformHandle> {
        canvas_transform_handles(
            self.document,
            self.selection,
            self.viewport,
            Some(self.kind_registry),
        )
        .into_iter()
        .rev()
        .find(|handle| handle.document_bounds.contains(&point))
    }
    pub(crate) fn pointer_owner_at(&self, point: Point<Pixels>) -> CanvasPointerOwner {
        if let Some(target) = self.selected_reconnect_target_at(point) {
            return CanvasPointerOwner::Reconnect(target);
        }

        if let CanvasConnectionHit::Valid(source) =
            self.connection_hit_at(point, CanvasConnectionEndpointRole::Source)
            && source.handle_id.is_some()
        {
            return CanvasPointerOwner::ConnectionSource(source);
        }

        if let Some(handle) = self.transform_handle_at(point) {
            return CanvasPointerOwner::Transform(handle);
        }

        let Some(target) = self
            .runtime()
            .precise_hit_test_with_kind_registry(
                self.document(),
                self.kind_registry(),
                point,
                HitOptions::default(),
            )
            .map(|record| record.target.clone())
            .next()
        else {
            return CanvasPointerOwner::Pane;
        };

        match target {
            target @ (HitTarget::Node(_) | HitTarget::Shape(_)) => {
                CanvasPointerOwner::NodeDrag(target)
            }
            target => CanvasPointerOwner::Record(target),
        }
    }
}
