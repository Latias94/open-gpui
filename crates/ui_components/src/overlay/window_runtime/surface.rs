//! Overlay surface projection and frame-scoped inside geometry.

use super::{
    AccessibilityTreeScope, AnyElement, App, Bounds, Element, ElementId, Entity, FocusHandle,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, LiveInsideRegion, MouseButton,
    OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerId, OverlayLayerLease,
    OverlayLayerLeaseStatus, OverlayLayerRegistration, Pixels, Rc, RefCell, Window,
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
        self.set_inside_region_for_button(binding, region, bounds, None, window, cx)
    }

    fn set_inside_region_for_button(
        &self,
        binding: &OverlayLayerBinding,
        region: OverlayInsideRegionId,
        bounds: Bounds<Pixels>,
        button: Option<MouseButton>,
        window: &mut Window,
        cx: &App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.state.read(cx).validate_mutable_lease(&binding.lease)?;
        let weak_state = self.state.downgrade();
        let lease = binding.lease.clone();
        window.record_prepaint_commit(move |valid_through, cx| {
            let _ = weak_state.update(cx, |state, _| {
                let _ = state.refresh_inside_region(
                    &lease,
                    region.clone(),
                    bounds,
                    button,
                    valid_through,
                );
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
        registration: OverlayLayerRegistration,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        self.bind_component_layer_with_optional_trigger(
            owner,
            binding,
            registration,
            None,
            window,
            cx,
        )
    }

    pub(crate) fn bind_component_layer_with_trigger_focus<T: 'static>(
        &self,
        owner: &Entity<T>,
        binding: Option<&OverlayLayerBinding>,
        registration: OverlayLayerRegistration,
        trigger_focus: FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        self.bind_component_layer_with_optional_trigger(
            owner,
            binding,
            registration,
            Some(trigger_focus),
            window,
            cx,
        )
    }

    fn bind_component_layer_with_optional_trigger<T: 'static>(
        &self,
        owner: &Entity<T>,
        binding: Option<&OverlayLayerBinding>,
        mut registration: OverlayLayerRegistration,
        trigger_focus: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<OverlayLayerBinding, WindowOverlayRuntimeError> {
        let frame_revision = window.rendered_frame_revision();
        if registration.parent.is_none()
            && let Some(parent) = self.current_parent_layer()
        {
            registration.parent = Some(parent);
        }
        if let Some(binding) = binding {
            self.rebind_layer(binding, registration, window, cx)?;
            self.state.update(cx, |state, _| {
                state.record_component_bind(&binding.lease, frame_revision)
            })?;
            return Ok(binding.clone());
        }

        let layer_id = registration.id.clone();
        self.replace_stale_component_subtree(
            &layer_id,
            frame_revision,
            owner.entity_id(),
            window,
            cx,
        )?;

        let binding = if let Some(trigger_focus) = trigger_focus {
            self.register_layer_for_entity_with_trigger_focus(
                registration,
                trigger_focus,
                owner,
                window,
                cx,
            )?
        } else {
            self.register_layer_for_entity(registration, owner, window, cx)?
        };
        self.state.update(cx, |state, _| {
            state.record_component_bind(&binding.lease, frame_revision)
        })?;
        Ok(binding)
    }

    pub(crate) fn unregister_component_subtree(
        &self,
        binding: &OverlayLayerBinding,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        self.unregister_released_subtree_by_lease(binding.lease.clone(), window, cx)
    }

    pub(crate) fn component_binding_status(
        &self,
        binding: &OverlayLayerBinding,
        window: &Window,
        cx: &App,
    ) -> Result<OverlayLayerLeaseStatus, WindowOverlayRuntimeError> {
        self.ensure_binding(binding, window)?;
        Ok(self.state.read(cx).lease_status(&binding.lease))
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
            region: Some(region),
            region_button: None,
            projects_parent: true,
            child: Some(child.into_any_element()),
        }
    }

    /// Wraps trigger geometry as an inside region without making trigger children overlay-owned.
    pub(crate) fn inside_region(
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
            region: Some(region),
            region_button: None,
            projects_parent: false,
            child: Some(child.into_any_element()),
        }
    }

    /// Wraps source geometry that is inside only for one mouse button.
    pub(crate) fn inside_region_for_button(
        &self,
        binding: &OverlayLayerBinding,
        region: OverlayInsideRegionId,
        button: MouseButton,
        id: impl Into<ElementId>,
        child: impl IntoElement,
    ) -> OverlaySurface {
        OverlaySurface {
            id: id.into(),
            runtime: self.clone(),
            binding: binding.clone(),
            region: Some(region),
            region_button: Some(button),
            projects_parent: false,
            child: Some(child.into_any_element()),
        }
    }

    /// Wraps a trigger focus target without contributing pointer-inside geometry.
    pub(crate) fn focus_target(
        &self,
        binding: &OverlayLayerBinding,
        id: impl Into<ElementId>,
        child: impl IntoElement,
    ) -> OverlaySurface {
        OverlaySurface {
            id: id.into(),
            runtime: self.clone(),
            binding: binding.clone(),
            region: None,
            region_button: None,
            projects_parent: false,
            child: Some(child.into_any_element()),
        }
    }

    pub(crate) fn parent_scope(
        &self,
        binding: &OverlayLayerBinding,
        id: impl Into<ElementId>,
        child: impl IntoElement,
    ) -> OverlaySurface {
        OverlaySurface {
            id: id.into(),
            runtime: self.clone(),
            binding: binding.clone(),
            region: None,
            region_button: None,
            projects_parent: true,
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
    region: Option<OverlayInsideRegionId>,
    region_button: Option<MouseButton>,
    projects_parent: bool,
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
        let binding_is_valid = self
            .runtime
            .validate_surface_binding(&self.binding, window, cx)
            .is_ok();
        let layout_id = if binding_is_valid && self.projects_parent {
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
        let accessibility_tree_scope = if self.projects_parent {
            self.runtime
                .state
                .read(cx)
                .accessibility_tree_scope(self.binding.lease(), self.runtime.window_id)
        } else {
            AccessibilityTreeScope::Unrestricted
        };
        let binding_is_valid = self
            .runtime
            .validate_surface_binding(&self.binding, window, cx)
            .is_ok();
        if binding_is_valid {
            if let Some(region) = self.region.as_ref() {
                let _ = self.runtime.set_inside_region_for_button(
                    &self.binding,
                    region.clone(),
                    bounds,
                    self.region_button,
                    window,
                    cx,
                );
            }
            if self.projects_parent {
                self.runtime.with_parent_layer(&self.binding, || {
                    window.with_accessibility_tree_scope(accessibility_tree_scope, |window| {
                        child.prepaint(window, cx)
                    });
                });
            } else {
                child.prepaint(window, cx);
            }
            let _ =
                self.runtime
                    .retry_focus_claim_after_surface_prepaint(&self.binding, window, cx);
        } else if self.projects_parent {
            window.with_accessibility_tree_scope(AccessibilityTreeScope::Excluded, |window| {
                child.prepaint(window, cx)
            });
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
        let binding_is_valid = self
            .runtime
            .validate_surface_binding(&self.binding, window, cx)
            .is_ok();
        if binding_is_valid && self.projects_parent {
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
        button: Option<MouseButton>,
        valid_through: u64,
    ) -> Result<(), WindowOverlayRuntimeError> {
        let entry = self.entry_for_lease_mut(lease)?;
        entry
            .inside_regions
            .retain(|_, current| current.valid_through >= valid_through);
        entry.inside_regions.insert(
            region,
            LiveInsideRegion {
                bounds,
                button,
                valid_through,
            },
        );
        Ok(())
    }
}
