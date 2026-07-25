use crate::{
    Bounds, ElementGeometry, LayoutId, Pixels, SubtreePresentation, Window, WindowId,
    geometry::{ResolvedSubtreeTransform, SubtreeGeometryValidity},
};
use smallvec::SmallVec;
use std::{cell::RefCell, rc::Rc};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) struct PortalAnchorId(u64);

impl PortalAnchorId {
    fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("portal anchor handle space exhausted"),
        )
    }
}

/// A stable, window-owned capability for following one element target across frames.
///
/// Bind the handle exactly once in every frame where its target is present. Multiple followers may
/// resolve the same handle, but the handle cannot be transported to another window.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PortalAnchorHandle {
    window_id: WindowId,
    id: PortalAnchorId,
}

impl PortalAnchorHandle {
    /// Returns the window that created this handle.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }
}

/// Immutable geometry published by one portal-anchor target.
///
/// The snapshot deliberately exposes neither a raw transform nor mutable target state. Followers
/// receive the target's effective presentation and clip facts and apply their own eligibility
/// policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortalAnchorSnapshot {
    window_id: WindowId,
    frame_generation: u64,
    geometry: ElementGeometry,
    presentation: SubtreePresentation,
    effective_clip_bounds: Bounds<Pixels>,
}

impl PortalAnchorSnapshot {
    /// Returns the window that owns this snapshot.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the candidate or committed frame generation that produced this snapshot.
    pub const fn frame_generation(self) -> u64 {
        self.frame_generation
    }

    /// Returns the target's opaque post-layout geometry.
    pub const fn geometry(self) -> ElementGeometry {
        self.geometry
    }

    /// Returns the target's effective layout-preserving presentation state.
    pub const fn presentation(self) -> SubtreePresentation {
        self.presentation
    }

    /// Returns the effective window-space clip AABB inherited by the target.
    pub const fn effective_clip_bounds(self) -> Bounds<Pixels> {
        self.effective_clip_bounds
    }
}

/// An error produced while binding or resolving a portal anchor.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PortalAnchorError {
    /// The handle was used with a window other than the one that created it.
    #[error(
        "portal anchor handle belongs to window {handle_window:?}, not window {target_window:?}"
    )]
    WrongWindow {
        /// The window that created the handle.
        handle_window: WindowId,
        /// The window on which the operation was attempted.
        target_window: WindowId,
    },
    /// The stable handle already claimed a target in the candidate frame.
    #[error("portal anchor handle {handle:?} is already bound in the current frame")]
    HandleAlreadyBound {
        /// The duplicate handle.
        handle: PortalAnchorHandle,
    },
}

#[derive(Clone)]
pub(super) struct PortalAnchorBinding {
    handle: PortalAnchorHandle,
    snapshot: Option<PortalAnchorSnapshot>,
}

pub(super) struct PortalAnchorCapture {
    handle: PortalAnchorHandle,
    root_layout_ids: SmallVec<[LayoutId; 2]>,
    layout_bounds: Bounds<Pixels>,
    transform: ResolvedSubtreeTransform,
    validity: Option<SubtreeGeometryValidity>,
    presentation: SubtreePresentation,
    effective_clip_bounds: Bounds<Pixels>,
    failed: bool,
}

impl PortalAnchorCapture {
    fn new(
        handle: PortalAnchorHandle,
        layout_id: LayoutId,
        layout_bounds: Bounds<Pixels>,
        transform: ResolvedSubtreeTransform,
        validity: Option<SubtreeGeometryValidity>,
        presentation: SubtreePresentation,
        effective_clip_bounds: Bounds<Pixels>,
    ) -> Self {
        let mut root_layout_ids = SmallVec::new();
        root_layout_ids.push(layout_id);
        Self {
            handle,
            root_layout_ids,
            layout_bounds,
            transform,
            validity,
            presentation,
            effective_clip_bounds,
            failed: false,
        }
    }

    fn contains_root_layout(&self, layout_id: LayoutId) -> bool {
        self.root_layout_ids.contains(&layout_id)
    }

    fn add_root_layout_alias(&mut self, layout_id: LayoutId) {
        if !self.contains_root_layout(layout_id) {
            self.root_layout_ids.push(layout_id);
        }
    }
}

struct PortalAnchorCaptureGuard {
    stack: Rc<RefCell<Vec<PortalAnchorCapture>>>,
    entered_depth: usize,
    armed: bool,
}

impl Drop for PortalAnchorCaptureGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert_eq!(stack.len(), self.entered_depth + 1);
        }
        stack.truncate(self.entered_depth);
    }
}

impl PortalAnchorBinding {
    pub(super) fn id(&self) -> PortalAnchorId {
        self.handle.id
    }

    pub(super) fn snapshot(&self) -> Option<PortalAnchorSnapshot> {
        self.snapshot
    }

    /// Replays already-resolved window-space facts under an unchanged view cache key.
    ///
    /// Bounds, presentation, transform, and clip-stack changes rebuild the cached view. A valid
    /// journal replay therefore changes only the owning frame generation.
    pub(super) fn replayed(&self, frame_generation: u64) -> Self {
        let snapshot = self.snapshot.map(|mut snapshot| {
            snapshot.frame_generation = frame_generation;
            snapshot
        });
        Self {
            handle: self.handle,
            snapshot,
        }
    }
}

impl Window {
    /// Creates a stable portal-anchor handle owned by this window.
    pub fn new_portal_anchor(&mut self) -> PortalAnchorHandle {
        let id = self.next_portal_anchor_id;
        self.next_portal_anchor_id = id.next();
        PortalAnchorHandle {
            window_id: self.handle.window_id(),
            id,
        }
    }

    /// Binds a portal-anchor handle to `layout_bounds` in the frame being built.
    ///
    /// Custom elements call this once during prepaint after their target geometry is final. Most
    /// callers should use [`crate::PortalAnchorExt::track_portal_anchor`]. Hidden and numerically
    /// invalid targets claim the handle for duplicate detection but resolve as explicitly unlinked.
    pub fn bind_portal_anchor(
        &mut self,
        handle: &PortalAnchorHandle,
        layout_bounds: Bounds<Pixels>,
    ) -> Result<(), PortalAnchorError> {
        self.invalidator.debug_assert_prepaint();
        self.ensure_portal_anchor_window(handle)?;
        if self.next_frame.has_portal_anchor_binding(handle.id) {
            return Err(PortalAnchorError::HandleAlreadyBound { handle: *handle });
        }

        let presentation = self.subtree_presentation();
        let snapshot = if presentation.paints() {
            self.try_element_geometry(layout_bounds)
                .ok()
                .map(|geometry| PortalAnchorSnapshot {
                    window_id: self.handle.window_id(),
                    frame_generation: self.next_frame.generation,
                    geometry,
                    presentation,
                    effective_clip_bounds: self.clip_bounds(),
                })
        } else {
            None
        };
        self.next_frame
            .record_portal_anchor_binding(super::FrameOutput::new(
                PortalAnchorBinding {
                    handle: *handle,
                    snapshot,
                },
                self.subtree_geometry_validity(),
            ));
        Ok(())
    }

    pub(crate) fn with_portal_anchor_target<R>(
        &mut self,
        handle: &PortalAnchorHandle,
        layout_id: LayoutId,
        layout_bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PortalAnchorError> {
        self.invalidator.debug_assert_prepaint();
        self.ensure_portal_anchor_window(handle)?;
        if self.next_frame.has_portal_anchor_binding(handle.id) {
            return Err(PortalAnchorError::HandleAlreadyBound { handle: *handle });
        }

        let stack = self.portal_anchor_capture_stack.clone();
        let entered_depth = stack.borrow().len();
        stack.borrow_mut().push(PortalAnchorCapture::new(
            *handle,
            layout_id,
            layout_bounds,
            self.subtree_transform(),
            self.subtree_geometry_validity(),
            self.subtree_presentation(),
            self.clip_bounds(),
        ));
        let _guard = PortalAnchorCaptureGuard {
            stack: stack.clone(),
            entered_depth,
            armed: true,
        };

        let result = f(self);
        let capture = stack
            .borrow_mut()
            .pop()
            .expect("portal anchor capture stack entry missing");
        // The capture is removed explicitly so it can be finalized after the child scope exits.
        // Disarm the panic-cleanup guard once ownership has moved to `capture`.
        let mut guard = _guard;
        guard.armed = false;
        drop(guard);
        self.bind_portal_anchor_capture(capture)?;
        Ok(result)
    }

    fn bind_portal_anchor_capture(
        &mut self,
        capture: PortalAnchorCapture,
    ) -> Result<(), PortalAnchorError> {
        if self.next_frame.has_portal_anchor_binding(capture.handle.id) {
            return Err(PortalAnchorError::HandleAlreadyBound {
                handle: capture.handle,
            });
        }
        let snapshot = if !capture.failed
            && capture.presentation.paints()
            && capture
                .validity
                .as_ref()
                .is_none_or(SubtreeGeometryValidity::is_valid)
        {
            match capture.transform.try_project_bounds(capture.layout_bounds) {
                Ok(displayed_bounds) => Some(PortalAnchorSnapshot {
                    window_id: self.handle.window_id(),
                    frame_generation: self.next_frame.generation,
                    geometry: ElementGeometry::from_resolved(
                        capture.layout_bounds,
                        displayed_bounds,
                        capture.transform,
                    ),
                    presentation: capture.presentation,
                    effective_clip_bounds: capture.effective_clip_bounds,
                }),
                Err(error) => {
                    if let Some(validity) = capture.validity.as_ref() {
                        validity.invalidate(error);
                    }
                    self.record_subtree_transform_diagnostic(error);
                    None
                }
            }
        } else {
            None
        };
        self.next_frame
            .record_portal_anchor_binding(super::FrameOutput::new(
                PortalAnchorBinding {
                    handle: capture.handle,
                    snapshot,
                },
                capture.validity,
            ));
        Ok(())
    }

    pub(crate) fn update_portal_anchor_presentation(&mut self, presentation: SubtreePresentation) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .portal_anchor_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.presentation = presentation;
        }
    }

    pub(crate) fn update_portal_anchor_transform(
        &mut self,
        transform: ResolvedSubtreeTransform,
        validity: Option<SubtreeGeometryValidity>,
    ) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .portal_anchor_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.transform = transform;
            capture.validity = validity.clone();
        }
    }

    pub(crate) fn invalidate_portal_anchor_capture(&self) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .portal_anchor_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.failed = true;
        }
    }

    pub(crate) fn register_portal_anchor_root_layout_alias(&mut self, alias: LayoutId) {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return;
        };
        for capture in self
            .portal_anchor_capture_stack
            .borrow_mut()
            .iter_mut()
            .filter(|capture| capture.contains_root_layout(layout_id))
        {
            capture.add_root_layout_alias(alias);
        }
    }

    pub(crate) fn portal_anchor_capture_requires_fresh_prepaint(&self) -> bool {
        let Some(layout_id) = self.current_prepaint_layout_id() else {
            return false;
        };
        self.portal_anchor_capture_stack
            .borrow()
            .iter()
            .any(|capture| capture.contains_root_layout(layout_id))
    }

    /// Resolves the handle from the frame currently being built or the last committed frame.
    ///
    /// During prepaint and paint, only the candidate frame is consulted, so a follower that runs
    /// before its target observes `None` instead of stale committed geometry. Outside the draw
    /// transaction, only the last committed frame is visible. Frame output recorded inside
    /// `resolve` inherits the candidate target's transform validity, so a target invalidated later
    /// in paint also suppresses its same-frame followers.
    pub fn resolve_portal_anchor<R>(
        &mut self,
        handle: &PortalAnchorHandle,
        resolve: impl FnOnce(Option<PortalAnchorSnapshot>, &mut Window) -> R,
    ) -> Result<R, PortalAnchorError> {
        self.ensure_portal_anchor_window(handle)?;
        let building_frame = self.invalidator.is_building_frame();
        if building_frame {
            self.record_portal_anchor_dependency(handle)?;
        }
        let (snapshot, dependency, duplicate) = {
            let frame = if building_frame {
                &self.next_frame
            } else {
                &self.rendered_frame
            };
            let duplicate = frame.portal_anchor_binding_is_duplicate(handle.id);
            let resolved = frame
                .portal_anchor_binding(handle.id)
                .filter(|binding| binding.is_valid())
                .map(|binding| {
                    (
                        binding.value.snapshot(),
                        building_frame.then(|| binding.validity.clone()).flatten(),
                    )
                })
                .unwrap_or((None, None));
            (resolved.0, resolved.1, duplicate)
        };

        if duplicate {
            return Err(PortalAnchorError::HandleAlreadyBound { handle: *handle });
        }

        if building_frame && snapshot.is_some() {
            let transform = self.subtree_transform();
            let validity =
                SubtreeGeometryValidity::joined(self.subtree_geometry_validity(), dependency);
            Ok(
                self.with_resolved_subtree_transform(transform, validity, |window| {
                    resolve(snapshot, window)
                }),
            )
        } else {
            Ok(resolve(snapshot, self))
        }
    }

    pub(crate) fn record_portal_anchor_dependency(
        &mut self,
        handle: &PortalAnchorHandle,
    ) -> Result<(), PortalAnchorError> {
        self.ensure_portal_anchor_window(handle)?;
        self.invalidator.debug_assert_paint_or_prepaint();
        let current_view = self.current_view();
        let dependent_views = self
            .next_frame
            .dispatch_tree
            .view_path_reversed(current_view)
            .collect::<Vec<_>>();
        self.next_frame
            .portal_anchor_dependent_views
            .insert(current_view);
        self.next_frame
            .portal_anchor_dependent_views
            .extend(dependent_views);
        Ok(())
    }

    pub(crate) fn portal_anchor_dependency_invalidates_view(
        &self,
        view_id: crate::EntityId,
    ) -> bool {
        self.rendered_frame
            .portal_anchor_dependent_views
            .contains(&view_id)
    }

    fn ensure_portal_anchor_window(
        &self,
        handle: &PortalAnchorHandle,
    ) -> Result<(), PortalAnchorError> {
        let target_window = self.handle.window_id();
        if handle.window_id != target_window {
            return Err(PortalAnchorError::WrongWindow {
                handle_window: handle.window_id,
                target_window,
            });
        }
        Ok(())
    }
}
