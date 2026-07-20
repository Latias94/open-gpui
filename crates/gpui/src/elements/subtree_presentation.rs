use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

/// Controls which framework channels an element subtree participates in while preserving layout.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum SubtreePresentation {
    /// Participates in layout, paint, interaction, focus, IME, and accessibility.
    #[default]
    Visible,
    /// Participates in layout and paint, but not interaction, focus, IME, or accessibility.
    Inert,
    /// Participates only in layout.
    Hidden,
}

impl SubtreePresentation {
    pub(crate) fn resolve_under(self, ancestor: Self) -> Self {
        self.max(ancestor)
    }

    /// Returns whether the subtree participates in painting.
    pub const fn paints(self) -> bool {
        !matches!(self, Self::Hidden)
    }

    /// Returns whether the subtree participates in interactive and semantic channels.
    pub const fn is_interactive(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// A layout-neutral presentation scope applied consistently to one element subtree.
pub struct SubtreePresentationElement {
    presentation: SubtreePresentation,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

impl SubtreePresentationElement {
    #[track_caller]
    pub(crate) fn new(presentation: SubtreePresentation, child: impl IntoElement) -> Self {
        Self {
            presentation,
            child: Some(child.into_any_element()),
            source: core::panic::Location::caller(),
        }
    }
}

impl Element for SubtreePresentationElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = bool;

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
        let mut child = self
            .child
            .take()
            .expect("subtree presentation child missing");
        let layout_id = window.with_subtree_presentation(self.presentation, |window| {
            child.request_layout(window, cx)
        });
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        window.with_subtree_presentation(self.presentation, |window| {
            if window.subtree_presentation().paints() {
                child.prepaint(window, cx);
                true
            } else {
                false
            }
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        child_prepainted: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*child_prepainted {
            return;
        }
        window.with_subtree_presentation(self.presentation, |window| {
            if window.subtree_presentation().paints() {
                child.paint(window, cx);
            }
        });
    }
}

impl IntoElement for SubtreePresentationElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Adds inherited layout-preserving presentation semantics to any element subtree.
pub trait SubtreePresentationExt: IntoElement + Sized + 'static {
    /// Applies the requested presentation state to this element and all descendants.
    ///
    /// Ancestors remain authoritative: a descendant cannot escape an inert or hidden ancestor.
    fn with_subtree_presentation(
        self,
        presentation: SubtreePresentation,
    ) -> SubtreePresentationElement {
        SubtreePresentationElement::new(presentation, self)
    }
}

impl<T> SubtreePresentationExt for T where T: IntoElement + Sized + 'static {}
