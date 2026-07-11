#[cfg(any(test, feature = "test-support"))]
use crate::Bounds;
use crate::{
    AnyElement, AnyTooltip, App, AtlasAccessDiagnostic, CursorStyle, DispatchNodeId, DispatchTree,
    ElementId, EntityId, GlobalElementId, Hitbox, HitboxBehavior, HitboxId, LineLayoutIndex,
    Pixels, PlatformInputHandler, Point, PointerCaptureId, Scene, TabStopMap, TextStyleRefinement,
    Window, WindowControlArea,
};
use itertools::FoldWhile::{Continue, Done};
use itertools::Itertools;
use open_gpui_collections::FxHashMap;
use smallvec::SmallVec;
use std::{any::TypeId, ops::Range, rc::Rc};

use super::{
    AnyMouseListener, AnyPointerCancelListener, ContentMask, CursorStyleRequest, ElementStateBox,
    FocusId, HitTest, ImagePaintDiagnostic, TooltipId,
};

#[derive(Clone)]
pub(crate) struct TooltipRequest {
    pub(super) id: TooltipId,
    pub(super) tooltip: AnyTooltip,
}

pub(crate) struct DeferredDraw {
    pub(super) current_view: EntityId,
    pub(super) priority: usize,
    pub(super) parent_node: DispatchNodeId,
    pub(super) element_id_stack: SmallVec<[ElementId; 32]>,
    pub(super) text_style_stack: Vec<TextStyleRefinement>,
    pub(super) content_mask: Option<ContentMask<Pixels>>,
    pub(super) rem_size: Pixels,
    pub(super) element: Option<AnyElement>,
    pub(super) absolute_offset: Point<Pixels>,
    pub(super) prepaint_range: Range<PrepaintStateIndex>,
    pub(super) paint_range: Range<PaintIndex>,
}

pub(crate) struct Frame {
    pub(crate) generation: u64,
    pub(crate) focus: Option<FocusId>,
    pub(crate) window_active: bool,
    pub(crate) element_states: FxHashMap<(GlobalElementId, TypeId), ElementStateBox>,
    pub(super) accessed_element_states: Vec<(GlobalElementId, TypeId)>,
    pub(crate) mouse_listeners: Vec<Option<AnyMouseListener>>,
    pub(crate) pointer_cancel_listeners: Vec<Option<AnyPointerCancelListener>>,
    pub(crate) dispatch_tree: DispatchTree,
    pub(crate) scene: Scene,
    pub(crate) atlas_access_diagnostics: Vec<AtlasAccessDiagnostic>,
    pub(crate) image_paint_diagnostics: Vec<ImagePaintDiagnostic>,
    pub(crate) hitboxes: Vec<Hitbox>,
    pub(crate) pointer_capture_bindings: Vec<(PointerCaptureId, HitboxId)>,
    pub(crate) prepaint_commits: Vec<Rc<dyn Fn(u64, &mut App)>>,
    pub(crate) window_control_hitboxes: Vec<(WindowControlArea, Hitbox)>,
    pub(crate) deferred_draws: Vec<DeferredDraw>,
    pub(crate) input_handlers: Vec<Option<PlatformInputHandler>>,
    pub(crate) tooltip_requests: Vec<Option<TooltipRequest>>,
    pub(crate) cursor_styles: Vec<CursorStyleRequest>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_bounds: FxHashMap<String, Bounds<Pixels>>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_focus_handles: FxHashMap<String, FocusId>,
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
    pub(super) prepaint_commits_index: usize,
    pub(super) tooltips_index: usize,
    pub(super) deferred_draws_index: usize,
    pub(super) dispatch_tree_index: usize,
    pub(super) accessed_element_states_index: usize,
    pub(super) line_layout_index: LineLayoutIndex,
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
    pub(super) accessed_element_states_index: usize,
    pub(super) tab_handle_index: usize,
    pub(super) line_layout_index: LineLayoutIndex,
}

impl Frame {
    pub(crate) fn new(dispatch_tree: DispatchTree) -> Self {
        Frame {
            generation: 0,
            focus: None,
            window_active: false,
            element_states: FxHashMap::default(),
            accessed_element_states: Vec::new(),
            mouse_listeners: Vec::new(),
            pointer_cancel_listeners: Vec::new(),
            dispatch_tree,
            scene: Scene::default(),
            atlas_access_diagnostics: Vec::new(),
            image_paint_diagnostics: Vec::new(),
            hitboxes: Vec::new(),
            pointer_capture_bindings: Vec::new(),
            prepaint_commits: Vec::new(),
            window_control_hitboxes: Vec::new(),
            deferred_draws: Vec::new(),
            input_handlers: Vec::new(),
            tooltip_requests: Vec::new(),
            cursor_styles: Vec::new(),

            #[cfg(any(test, feature = "test-support"))]
            debug_bounds: FxHashMap::default(),
            #[cfg(any(test, feature = "test-support"))]
            debug_focus_handles: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            next_inspector_instance_ids: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector_hitboxes: FxHashMap::default(),
            tab_stops: TabStopMap::default(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.element_states.clear();
        self.accessed_element_states.clear();
        self.mouse_listeners.clear();
        self.pointer_cancel_listeners.clear();
        self.dispatch_tree.clear();
        self.scene.clear();
        self.generation = 0;
        self.atlas_access_diagnostics.clear();
        self.image_paint_diagnostics.clear();
        self.input_handlers.clear();
        self.tooltip_requests.clear();
        self.cursor_styles.clear();
        self.hitboxes.clear();
        self.pointer_capture_bindings.clear();
        self.prepaint_commits.clear();
        self.window_control_hitboxes.clear();
        self.deferred_draws.clear();
        self.tab_stops.clear();
        self.focus = None;

        #[cfg(any(test, feature = "test-support"))]
        {
            self.debug_bounds.clear();
            self.debug_focus_handles.clear();
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
            let bounds = hitbox.bounds.intersect(&hitbox.content_mask.bounds);
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
            if let Some((element_state_key, element_state)) =
                prev_frame.element_states.remove_entry(element_state_key)
            {
                self.element_states.insert(element_state_key, element_state);
            }
        }

        self.scene.finish();
    }
}
