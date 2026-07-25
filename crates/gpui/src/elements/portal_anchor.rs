use crate::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, PortalAnchorHandle,
    PortalAnchorSnapshot, Style, SubtreeGeometryValidity, Window,
};

type PortalAnchorFollowerBuilder =
    Box<dyn FnOnce(Option<PortalAnchorSnapshot>, &mut Window, &mut App) -> Option<AnyElement>>;

/// A layout-neutral element wrapper that binds its bounds to a portal-anchor handle.
pub struct PortalAnchorElement {
    handle: PortalAnchorHandle,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

impl PortalAnchorElement {
    #[track_caller]
    fn new(handle: PortalAnchorHandle, child: impl IntoElement) -> Self {
        Self {
            handle,
            child: Some(child.into_any_element()),
            source: core::panic::Location::caller(),
        }
    }
}

impl Element for PortalAnchorElement {
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
        let mut child = self.child.take().expect("portal anchor child missing");
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
            .with_portal_anchor_target(&self.handle, *layout_id, bounds, |window| {
                child.prepaint(window, cx)
            })
            .unwrap_or_else(|error| panic!("failed to bind portal anchor handle: {error}"));
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
        let (_, child) = request_layout;
        child.paint(window, cx);
    }
}

impl IntoElement for PortalAnchorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Binds any element's post-layout bounds to a typed portal-anchor handle.
pub trait PortalAnchorExt: IntoElement + Sized + 'static {
    /// Tracks this element as the handle's only target in each rendered frame.
    fn track_portal_anchor(self, handle: &PortalAnchorHandle) -> PortalAnchorElement {
        PortalAnchorElement::new(*handle, self)
    }
}

impl<T> PortalAnchorExt for T where T: IntoElement + Sized + 'static {}

/// Builds a deferred, window-space follower for a portal-anchor target.
///
/// Resolution runs after ordinary prepaint, so targets do not need to precede the follower in the
/// element tree. The callback receives the current candidate only; `None` never falls back to the
/// previous committed frame. An element returned by the callback is laid out immediately and then
/// rendered through an explicit window-space portal while retaining theme and presentation
/// inheritance.
#[track_caller]
pub fn portal_anchor_follower(
    handle: &PortalAnchorHandle,
    build: impl FnOnce(Option<PortalAnchorSnapshot>, &mut Window, &mut App) -> Option<AnyElement>
    + 'static,
) -> PortalAnchorFollower {
    PortalAnchorFollower {
        handle: *handle,
        task: Some(
            PortalAnchorFollowerTask {
                handle: *handle,
                build: Some(Box::new(build)),
            }
            .into_any_element(),
        ),
        priority: 0,
        source: core::panic::Location::caller(),
    }
}

/// A deferred element that resolves one portal anchor and emits an optional window-space child.
pub struct PortalAnchorFollower {
    handle: PortalAnchorHandle,
    task: Option<AnyElement>,
    priority: usize,
    source: &'static core::panic::Location<'static>,
}

impl PortalAnchorFollower {
    /// Sets the deferred paint priority of the follower surface.
    pub fn priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl Element for PortalAnchorFollower {
    type RequestLayoutState = Option<AnyElement>;
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
        let mut task = self
            .task
            .take()
            .expect("portal anchor follower task missing");
        let layout_id = task.request_layout(window, cx);
        (layout_id, Some(task))
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        task: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        window
            .record_portal_anchor_dependency(&self.handle)
            .unwrap_or_else(|error| panic!("failed to record portal anchor dependency: {error}"));
        let task = task.take().expect("portal anchor follower task missing");
        window.defer_draw(task, window.element_offset(), self.priority);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _task: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for PortalAnchorFollower {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PortalAnchorFollowerTask {
    handle: PortalAnchorHandle,
    build: Option<PortalAnchorFollowerBuilder>,
}

struct PreparedPortalAnchorFollower {
    child: AnyElement,
    validity: Option<SubtreeGeometryValidity>,
}

impl Element for PortalAnchorFollowerTask {
    type RequestLayoutState = ();
    type PrepaintState = Option<PreparedPortalAnchorFollower>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let build = self
            .build
            .take()
            .expect("portal anchor follower builder missing");
        window
            .resolve_portal_anchor(&self.handle, move |snapshot, window| {
                let Some(mut child) = build(snapshot, window, cx) else {
                    return None;
                };
                child.layout_as_root(AvailableSpace::min_size(), window, cx);
                let validity = window.subtree_geometry_validity();
                if validity
                    .as_ref()
                    .is_none_or(SubtreeGeometryValidity::is_valid)
                {
                    let prepaint_validity = validity.clone();
                    let result = window.transact_subtree_geometry(validity.clone(), |window| {
                        window.with_window_space_portal_prepaint(
                            Point::default(),
                            prepaint_validity,
                            |window| child.prepaint(window, cx),
                        )
                    });
                    if result.is_err()
                        && let Some(validity) = validity.as_ref()
                    {
                        window.record_subtree_geometry_scope_diagnostic(validity);
                    }
                }
                Some(PreparedPortalAnchorFollower { child, validity })
            })
            .unwrap_or_else(|error| panic!("failed to resolve portal anchor follower: {error}"))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepared: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(prepared) = prepared else {
            return;
        };
        if prepared
            .validity
            .as_ref()
            .is_some_and(|validity| !validity.is_valid())
        {
            return;
        }
        window.with_window_space_portal_paint(prepared.validity.clone(), |window| {
            prepared.child.paint(window, cx)
        });
        if let Some(validity) = prepared.validity.as_ref() {
            window.record_subtree_geometry_scope_diagnostic(validity);
        }
    }
}

impl IntoElement for PortalAnchorFollowerTask {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
