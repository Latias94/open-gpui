//! Overlay surface projection and frame-scoped inside geometry.

use super::{
    AnyElement, App, Bounds, Element, ElementId, Entity, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, LiveInsideRegion, OverlayInsideRegionId, OverlayLayerBinding,
    OverlayLayerId, OverlayLayerLease, OverlayLayerRegistration, Pixels, Rc, RefCell, Window,
    WindowOverlayRuntime, WindowOverlayRuntimeError, WindowOverlayRuntimeState,
};

impl WindowOverlayRuntime {
    /// Registers current-frame bounds as an inside region for the layer.
    ///
    /// Call this during prepaint on every frame where the region is rendered. A region that is
    /// not refreshed becomes stale after that frame and cannot claim an outside press.
    pub fn set_inside_region(
        &self,
        binding: &OverlayLayerBinding,
        region: OverlayInsideRegionId,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state.read(cx).validate_mutable_lease(&binding.lease)?;
        let weak_state = self.state.downgrade();
        let lease = binding.lease.clone();
        window.record_prepaint_commit(move |valid_through, cx| {
            let _ = weak_state.update(cx, |state, _| {
                let _ = state.refresh_inside_region(&lease, region.clone(), bounds, valid_through);
            });
        });
        Ok(())
    }

    pub(crate) fn current_parent_layer(&self) -> Option<OverlayLayerId> {
        self.ambient_parent_layers.borrow().last().cloned()
    }

    pub(crate) fn bind_component_layer<T: 'static>(
        &self,
        owner: &Entity<T>,
        binding: Option<&OverlayLayerBinding>,
        mut registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        if registration.parent.is_none()
            && let Some(parent) = self.current_parent_layer()
        {
            registration.parent = Some(parent);
        }
        if let Some(binding) = binding {
            self.rebind_layer(binding, registration, window, cx)?;
            return Ok(binding.clone());
        }

        self.register_layer_for_entity(registration, owner, window, cx)
    }

    /// Wraps a rendered overlay surface with live bounds and nested-parent projection.
    pub fn surface(
        &self,
        binding: &OverlayLayerBinding,
        region: OverlayInsideRegionId,
        id: impl Into<ElementId>,
        child: impl IntoElement,
    ) -> OverlaySurface {
        OverlaySurface {
            id: id.into(),
            runtime: self.clone(),
            binding: binding.clone(),
            region,
            child: Some(child.into_any_element()),
        }
    }

    pub(super) fn with_parent_layer<R>(
        &self,
        binding: &OverlayLayerBinding,
        f: impl FnOnce() -> R,
    ) -> R {
        self.ambient_parent_layers
            .borrow_mut()
            .push(binding.lease.layer_id.clone());
        let _guard = AmbientParentGuard {
            stack: self.ambient_parent_layers.clone(),
            expected: binding.lease.layer_id.clone(),
        };
        f()
    }

    pub(super) fn validate_surface_binding(
        &self,
        binding: &OverlayLayerBinding,
        window: &Window,
        cx: &App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state.read(cx).validate_mutable_lease(&binding.lease)
    }
}

struct AmbientParentGuard {
    stack: Rc<RefCell<Vec<OverlayLayerId>>>,
    expected: OverlayLayerId,
}

impl Drop for AmbientParentGuard {
    fn drop(&mut self) {
        let actual = self.stack.borrow_mut().pop();
        debug_assert_eq!(actual.as_ref(), Some(&self.expected));
    }
}

/// Element wrapper that projects one runtime-owned overlay surface into GPUI.
pub struct OverlaySurface {
    id: ElementId,
    runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
    region: OverlayInsideRegionId,
    child: Option<AnyElement>,
}

impl IntoElement for OverlaySurface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for OverlaySurface {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self.child.take().expect("overlay surface child missing");
        let layout_id = if self
            .runtime
            .validate_surface_binding(&self.binding, window, cx)
            .is_ok()
        {
            self.runtime
                .with_parent_layer(&self.binding, || child.request_layout(window, cx))
        } else {
            child.request_layout(window, cx)
        };
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self
            .runtime
            .set_inside_region(&self.binding, self.region.clone(), bounds, window, cx)
            .is_ok()
        {
            self.runtime
                .with_parent_layer(&self.binding, || child.prepaint(window, cx));
        } else {
            child.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self
            .runtime
            .validate_surface_binding(&self.binding, window, cx)
            .is_ok()
        {
            self.runtime
                .with_parent_layer(&self.binding, || child.paint(window, cx));
        } else {
            child.paint(window, cx);
        }
    }
}

impl WindowOverlayRuntimeState {
    pub(super) fn refresh_inside_region(
        &mut self,
        lease: &OverlayLayerLease,
        region: OverlayInsideRegionId,
        bounds: Bounds<Pixels>,
        valid_through: u64,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let entry = self.entry_for_lease_mut(lease)?;
        entry.inside_regions.insert(
            region,
            LiveInsideRegion {
                bounds,
                valid_through,
            },
        );
        Ok(())
    }
}
