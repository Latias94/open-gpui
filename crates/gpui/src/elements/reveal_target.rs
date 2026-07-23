use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, RevealTargetHandle, Window,
};

/// A layout-neutral element wrapper that binds its root geometry as a bring-into-view target.
pub struct RevealTargetElement {
    handle: RevealTargetHandle,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

impl RevealTargetElement {
    #[track_caller]
    fn new(handle: RevealTargetHandle, child: impl IntoElement) -> Self {
        Self {
            handle,
            child: Some(child.into_any_element()),
            source: core::panic::Location::caller(),
        }
    }
}

impl Element for RevealTargetElement {
    type RequestLayoutState = (LayoutId, AnyElement);
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
        let mut child = self.child.take().expect("reveal target child missing");
        let layout_id = child.request_layout(window, cx);
        (layout_id, (layout_id, child))
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (layout_id, child) = request_layout;
        window
            .with_reveal_target(&self.handle, *layout_id, bounds, |window| {
                child.prepaint(window, cx)
            })
            .unwrap_or_else(|error| panic!("failed to bind reveal target handle: {error}"));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        request_layout.1.paint(window, cx);
    }
}

impl IntoElement for RevealTargetElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Binds any element's post-layout root geometry to a stable reveal target.
pub trait RevealTargetExt: IntoElement + Sized + 'static {
    /// Tracks this element as the handle's only target in each rendered frame.
    fn track_reveal_target(self, handle: &RevealTargetHandle) -> RevealTargetElement {
        RevealTargetElement::new(*handle, self)
    }
}

impl<T> RevealTargetExt for T where T: IntoElement + Sized + 'static {}
