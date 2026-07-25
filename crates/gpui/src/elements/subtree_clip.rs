use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, PreparedSubtreeClip, SubtreeClip, Window,
};

/// A layout-neutral clip applied consistently to one complete element subtree.
pub struct SubtreeClipElement {
    clip: SubtreeClip,
    prepared: Option<PreparedSubtreeClip>,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

impl SubtreeClipElement {
    /// Wraps `child` in a checked child-local subtree clip.
    #[track_caller]
    pub fn new(clip: SubtreeClip, child: impl IntoElement) -> Self {
        Self {
            clip,
            prepared: None,
            child: Some(child.into_any_element()),
            source: core::panic::Location::caller(),
        }
    }
}

impl Element for SubtreeClipElement {
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
        let mut child = self.child.take().expect("subtree clip child missing");
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
        let prepared = window.prepare_subtree_clip(&self.clip, bounds);
        let entered = window.with_prepared_subtree_clip(&prepared, |window| {
            child.prepaint(window, cx);
        });
        self.prepared = (entered.is_some() && prepared.is_valid()).then_some(prepared);
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
        let Some(prepared) = self.prepared.as_ref() else {
            return;
        };
        let _ = window.with_prepared_subtree_clip(prepared, |window| child.paint(window, cx));
    }
}

impl IntoElement for SubtreeClipElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Adds checked child-local clipping to any complete element subtree.
pub trait SubtreeClipExt: IntoElement + Sized + 'static {
    /// Applies `clip` after layout to this element and all descendants.
    fn with_subtree_clip(self, clip: SubtreeClip) -> SubtreeClipElement {
        SubtreeClipElement::new(clip, self)
    }

    /// Clips this element subtree to its own post-layout border box.
    fn clip_to_border_box(self) -> SubtreeClipElement {
        self.with_subtree_clip(SubtreeClip::own_border_box())
    }
}

impl<T> SubtreeClipExt for T where T: IntoElement + Sized + 'static {}
