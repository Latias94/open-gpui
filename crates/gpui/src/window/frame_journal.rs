#[cfg(any(test, feature = "test-support"))]
use crate::Bounds;
use crate::{
    AccessibilityTreeScope, AnyElement, AnyTooltip, App, AtlasAccessDiagnostic, CursorStyle,
    DispatchNodeId, DispatchTree, ElementId, EntityId, GlobalElementId, Hitbox, HitboxBehavior,
    HitboxId, LineLayoutIndex, Pixels, PlatformInputHandler, Point, PointerCaptureId, Scene,
    SubtreeTransformDiagnostic, TabStopMap, TextStyleRefinement, Window, WindowControlArea,
    geometry::{ResolvedSubtreeTransform, SubtreeTransformValidity},
};
use itertools::FoldWhile::{Continue, Done};
use itertools::Itertools;
use open_gpui_collections::FxHashMap;
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    ops::Range,
    rc::Rc,
};

use super::{
    AnyMouseListener, AnyPointerCancelListener, ContentMask, CursorStyleRequest, ElementStateBox,
    FocusId, HitTest, ImagePaintDiagnostic, PrepaintPublicationId, TooltipId,
};

#[derive(Clone)]
pub(crate) struct FrameOutput<T> {
    pub(super) value: T,
    pub(super) validity: Option<SubtreeTransformValidity>,
}

#[derive(Clone)]
pub(crate) struct PrepaintCommit {
    pub(super) publication: Option<PrepaintPublicationId>,
    pub(super) commit: Rc<dyn Fn(u64, &mut Window, &mut App)>,
    pub(super) discard: Option<Rc<dyn Fn(u64, &mut Window, &mut App)>>,
}

impl<T> FrameOutput<T> {
    pub(super) fn new(value: T, validity: Option<SubtreeTransformValidity>) -> Self {
        Self { value, validity }
    }

    pub(super) fn is_valid(&self) -> bool {
        self.validity
            .as_ref()
            .is_none_or(SubtreeTransformValidity::is_valid)
    }
}

#[derive(Clone)]
pub(crate) struct TooltipRequest {
    pub(super) id: TooltipId,
    pub(super) tooltip: AnyTooltip,
    pub(super) validity: Option<SubtreeTransformValidity>,
}

pub(crate) struct DeferredDraw {
    pub(super) current_view: EntityId,
    pub(super) priority: usize,
    pub(super) parent_node: DispatchNodeId,
    pub(super) element_id_stack: SmallVec<[ElementId; 32]>,
    pub(super) text_style_stack: Vec<TextStyleRefinement>,
    pub(super) accessibility_tree_scope: AccessibilityTreeScope,
    pub(super) accessibility_hidden: bool,
    pub(super) content_mask: Option<ContentMask<Pixels>>,
    pub(super) rem_size: Pixels,
    pub(super) element: Option<AnyElement>,
    pub(super) absolute_offset: Point<Pixels>,
    pub(super) subtree_transform: ResolvedSubtreeTransform,
    pub(super) subtree_transform_validity: Option<SubtreeTransformValidity>,
    pub(super) prepaint_range: Range<PrepaintStateIndex>,
    pub(super) paint_range: Range<PaintIndex>,
}

pub(crate) struct Frame {
    pub(crate) generation: u64,
    pub(crate) focus: Option<FocusId>,
    pub(crate) window_active: bool,
    pub(crate) element_states: FxHashMap<(GlobalElementId, TypeId), ElementStateBox>,
    pub(crate) element_state_validities:
        FxHashMap<(GlobalElementId, TypeId), Option<SubtreeTransformValidity>>,
    pub(super) accessed_element_states: Vec<(GlobalElementId, TypeId)>,
    pub(crate) mouse_listeners: Vec<FrameOutput<Option<AnyMouseListener>>>,
    pub(crate) pointer_cancel_listeners: Vec<FrameOutput<Option<AnyPointerCancelListener>>>,
    pub(crate) dispatch_tree: DispatchTree,
    pub(crate) scene: Scene,
    pub(crate) atlas_access_diagnostic_entries: Vec<FrameOutput<AtlasAccessDiagnostic>>,
    pub(crate) image_paint_diagnostic_entries: Vec<FrameOutput<ImagePaintDiagnostic>>,
    pub(crate) atlas_access_diagnostics: Vec<AtlasAccessDiagnostic>,
    pub(crate) image_paint_diagnostics: Vec<ImagePaintDiagnostic>,
    pub(crate) subtree_transform_diagnostics: Vec<SubtreeTransformDiagnostic>,
    pub(crate) hitboxes: Vec<Hitbox>,
    pub(crate) pointer_capture_bindings: Vec<(PointerCaptureId, HitboxId)>,
    pub(crate) retained_resources: Vec<Rc<dyn Any>>,
    pub(crate) prepaint_commits: Vec<FrameOutput<PrepaintCommit>>,
    pub(crate) window_control_hitboxes: Vec<(WindowControlArea, Hitbox)>,
    pub(crate) deferred_draws: Vec<DeferredDraw>,
    pub(crate) input_handlers: Vec<FrameOutput<Option<PlatformInputHandler>>>,
    pub(crate) tooltip_requests: Vec<Option<TooltipRequest>>,
    pub(crate) cursor_styles: Vec<CursorStyleRequest>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_bounds: FxHashMap<String, Bounds<Pixels>>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_bounds_entries:
        Vec<(String, Bounds<Pixels>, Option<SubtreeTransformValidity>)>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_focus_handles: FxHashMap<String, FocusId>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_focus_entries: Vec<(String, FocusId, Option<SubtreeTransformValidity>)>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) next_inspector_instance_ids: FxHashMap<Rc<crate::InspectorElementPath>, usize>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_hitboxes: FxHashMap<HitboxId, crate::InspectorElementId>,
    pub(crate) tab_stops: TabStopMap,
}

#[derive(Clone, Default)]
pub(crate) struct PrepaintStateIndex {
    pub(super) hitboxes_index: usize,
    pub(super) pointer_capture_bindings_index: usize,
    pub(super) retained_resources_index: usize,
    pub(super) prepaint_commits_index: usize,
    pub(super) tooltips_index: usize,
    pub(super) deferred_draws_index: usize,
    pub(super) dispatch_tree_index: usize,
    pub(super) accessed_element_states_index: usize,
    pub(super) line_layout_index: LineLayoutIndex,
    pub(super) subtree_transform_diagnostics_index: usize,
}

#[derive(Clone, Default)]
pub(crate) struct PaintIndex {
    pub(super) scene_index: usize,
    pub(super) atlas_access_diagnostics_index: usize,
    pub(super) image_paint_diagnostics_index: usize,
    pub(super) mouse_listeners_index: usize,
    pub(super) pointer_cancel_listeners_index: usize,
    pub(super) input_handlers_index: usize,
    pub(super) cursor_styles_index: usize,
    pub(super) window_control_hitboxes_index: usize,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) debug_bounds_entries_index: usize,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) debug_focus_entries_index: usize,
    pub(super) accessed_element_states_index: usize,
    pub(super) tab_handle_index: usize,
    pub(super) line_layout_index: LineLayoutIndex,
    pub(super) subtree_transform_diagnostics_index: usize,
}

impl Frame {
    pub(crate) fn new(dispatch_tree: DispatchTree) -> Self {
        Frame {
            generation: 0,
            focus: None,
            window_active: false,
            element_states: FxHashMap::default(),
            element_state_validities: FxHashMap::default(),
            accessed_element_states: Vec::new(),
            mouse_listeners: Vec::new(),
            pointer_cancel_listeners: Vec::new(),
            dispatch_tree,
            scene: Scene::default(),
            atlas_access_diagnostic_entries: Vec::new(),
            image_paint_diagnostic_entries: Vec::new(),
            atlas_access_diagnostics: Vec::new(),
            image_paint_diagnostics: Vec::new(),
            subtree_transform_diagnostics: Vec::new(),
            hitboxes: Vec::new(),
            pointer_capture_bindings: Vec::new(),
            retained_resources: Vec::new(),
            prepaint_commits: Vec::new(),
            window_control_hitboxes: Vec::new(),
            deferred_draws: Vec::new(),
            input_handlers: Vec::new(),
            tooltip_requests: Vec::new(),
            cursor_styles: Vec::new(),

            #[cfg(any(test, feature = "test-support"))]
            debug_bounds: FxHashMap::default(),
            #[cfg(any(test, feature = "test-support"))]
            debug_bounds_entries: Vec::new(),
            #[cfg(any(test, feature = "test-support"))]
            debug_focus_handles: FxHashMap::default(),
            #[cfg(any(test, feature = "test-support"))]
            debug_focus_entries: Vec::new(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            next_inspector_instance_ids: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector_hitboxes: FxHashMap::default(),
            tab_stops: TabStopMap::default(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.element_states.clear();
        self.element_state_validities.clear();
        self.accessed_element_states.clear();
        self.mouse_listeners.clear();
        self.pointer_cancel_listeners.clear();
        self.dispatch_tree.clear();
        self.scene.clear();
        self.generation = 0;
        self.atlas_access_diagnostic_entries.clear();
        self.image_paint_diagnostic_entries.clear();
        self.atlas_access_diagnostics.clear();
        self.image_paint_diagnostics.clear();
        self.subtree_transform_diagnostics.clear();
        self.input_handlers.clear();
        self.tooltip_requests.clear();
        self.cursor_styles.clear();
        self.hitboxes.clear();
        self.pointer_capture_bindings.clear();
        self.retained_resources.clear();
        self.prepaint_commits.clear();
        self.window_control_hitboxes.clear();
        self.deferred_draws.clear();
        self.tab_stops.clear();
        self.focus = None;

        #[cfg(any(test, feature = "test-support"))]
        {
            self.debug_bounds.clear();
            self.debug_bounds_entries.clear();
            self.debug_focus_handles.clear();
            self.debug_focus_entries.clear();
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            self.next_inspector_instance_ids.clear();
            self.inspector_hitboxes.clear();
        }
    }

    pub(crate) fn cursor_style(&self, window: &Window) -> Option<CursorStyle> {
        if !window.mouse_in_window {
            return None;
        }

        self.cursor_styles
            .iter()
            .rev()
            .filter(|request| {
                request
                    .validity
                    .as_ref()
                    .is_none_or(SubtreeTransformValidity::is_valid)
            })
            .fold_while(None, |style, request| match request.hitbox_id {
                None => Done(Some(request.style)),
                Some(hitbox_id) => Continue(style.or_else(|| {
                    hitbox_id
                        .is_hovered_ignoring_last_input(window)
                        .then_some(request.style)
                })),
            })
            .into_inner()
    }

    pub(crate) fn hit_test(&self, position: Point<Pixels>) -> HitTest {
        let mut set_hover_hitbox_count = false;
        let mut hit_test = HitTest::default();
        for hitbox in self.hitboxes.iter().rev() {
            if !hitbox.is_active() {
                continue;
            }
            let bounds = hitbox
                .displayed_bounds()
                .intersect(&hitbox.content_mask.bounds);
            if bounds.contains(&position) {
                hit_test.ids.push(hitbox.id);
                if !set_hover_hitbox_count
                    && hitbox.behavior == HitboxBehavior::BlockMouseExceptScroll
                {
                    hit_test.hover_hitbox_count = hit_test.ids.len();
                    set_hover_hitbox_count = true;
                }
                if hitbox.behavior == HitboxBehavior::BlockMouse {
                    break;
                }
            }
        }
        if !set_hover_hitbox_count {
            hit_test.hover_hitbox_count = hit_test.ids.len();
        }
        hit_test
    }

    pub(crate) fn focus_path(&self) -> SmallVec<[FocusId; 8]> {
        self.focus
            .map(|focus_id| self.dispatch_tree.focus_path(focus_id))
            .unwrap_or_default()
    }

    pub(crate) fn finish(&mut self, prev_frame: &mut Self) {
        for element_state_key in &self.accessed_element_states {
            if self
                .element_state_validities
                .get(element_state_key)
                .and_then(Option::as_ref)
                .is_some_and(|validity| !validity.is_valid())
            {
                self.element_states.remove(element_state_key);
                continue;
            }
            if let Some((element_state_key, element_state)) =
                prev_frame.element_states.remove_entry(element_state_key)
            {
                self.element_states.insert(element_state_key, element_state);
            }
        }

        self.dispatch_tree.suppress_invalid_nodes();
        if self
            .focus
            .is_some_and(|focus| self.dispatch_tree.focusable_node_id(focus).is_none())
        {
            self.focus = None;
        }
        self.atlas_access_diagnostics.clear();
        self.atlas_access_diagnostics.extend(
            self.atlas_access_diagnostic_entries
                .iter()
                .filter(|entry| entry.is_valid())
                .map(|entry| entry.value),
        );
        self.image_paint_diagnostics.clear();
        self.image_paint_diagnostics.extend(
            self.image_paint_diagnostic_entries
                .iter()
                .filter(|entry| entry.is_valid())
                .map(|entry| entry.value),
        );
        #[cfg(any(test, feature = "test-support"))]
        {
            self.debug_bounds.clear();
            for (selector, bounds, validity) in &self.debug_bounds_entries {
                if validity
                    .as_ref()
                    .is_none_or(SubtreeTransformValidity::is_valid)
                {
                    self.debug_bounds.insert(selector.clone(), *bounds);
                }
            }
            self.debug_focus_handles.clear();
            for (selector, focus_id, validity) in &self.debug_focus_entries {
                if validity
                    .as_ref()
                    .is_none_or(SubtreeTransformValidity::is_valid)
                {
                    self.debug_focus_handles.insert(selector.clone(), *focus_id);
                }
            }
        }
        self.scene.finish();
    }
}
