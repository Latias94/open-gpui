use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, SubtreeTransform, SubtreeTransformError, Window,
    geometry::{ResolvedSubtreeTransform, SubtreeTransformValidity},
};

/// A layout-neutral transform applied consistently to one interactive element subtree.
pub struct SubtreeTransformElement {
    transform: SubtreeTransform,
    resolved: Option<(ResolvedSubtreeTransform, SubtreeTransformValidity)>,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

impl SubtreeTransformElement {
    #[track_caller]
    pub(crate) fn new(transform: SubtreeTransform, child: impl IntoElement) -> Self {
        Self {
            transform,
            resolved: None,
            child: Some(child.into_any_element()),
            source: core::panic::Location::caller(),
        }
    }

    fn report_failure(error: SubtreeTransformError, window: &mut Window) {
        window.invalidate_portal_anchor_capture();
        window.invalidate_reveal_target_capture();
        window.record_subtree_transform_diagnostic(error);
    }
}

impl Element for SubtreeTransformElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Some(self.source)
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self.child.take().expect("subtree transform child missing");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let resolved = match window.resolve_subtree_transform(self.transform, bounds) {
            Ok(resolved) => resolved,
            Err(error) => {
                Self::report_failure(error, window);
                self.resolved = None;
                return;
            }
        };

        let validity = window.new_subtree_transform_validity();
        let result = window.transact_subtree_transform(Some(validity.clone()), |window| {
            window.with_resolved_subtree_transform(resolved, Some(validity.clone()), |window| {
                child.prepaint(window, cx)
            });
        });
        match result {
            Ok(()) => self.resolved = Some((resolved, validity)),
            Err(error) => {
                window.record_subtree_transform_scope_diagnostic(&validity);
                debug_assert_eq!(validity.failure(), Some(error));
                self.resolved = None;
            }
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some((transform, validity)) = self.resolved.as_ref() else {
            return;
        };
        if !validity.is_valid() {
            return;
        }
        window.with_resolved_subtree_transform(*transform, Some(validity.clone()), |window| {
            child.paint(window, cx)
        });
        window.record_subtree_transform_scope_diagnostic(validity);
    }
}

impl IntoElement for SubtreeTransformElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Adds a checked, layout-neutral transform to any element subtree.
pub trait SubtreeTransformExt: IntoElement + Sized + 'static {
    /// Applies `transform` after layout to this element and all of its descendants.
    fn with_subtree_transform(self, transform: SubtreeTransform) -> SubtreeTransformElement {
        SubtreeTransformElement::new(transform, self)
    }
}

impl<T> SubtreeTransformExt for T where T: IntoElement + Sized + 'static {}
