use std::{any::TypeId, ops::Range, rc::Rc};

use crate::{
    AnyElement, AnyWeakView, App, Bounds, DispatchNodeId, EntityId, FocusId, GlobalElementId,
    LayoutId, PaintIndex, Pixels, PrepaintStateIndex, Size, SubtreePresentation, TextStyle, Window,
    collections::{FxHashMap, FxHashSet},
    geometry::{ClipStackSnapshot, ResolvedSubtreeTransform},
    window::RetainedViewContext,
};

use super::dependencies::{self, ElementStateKey, Reads, RevisionKey};
use super::layout_key::LayoutKey;

#[path = "splice.rs"]
mod splice;

#[cfg(not(target_family = "wasm"))]
use super::a11y::{self, A11yFragment};
#[cfg(not(target_family = "wasm"))]
use crate::Point;

/// Stores only outputs that reached the frame journal. A rejected candidate never
/// replaces the previous frame's lookup table.
#[derive(Default)]
pub(crate) struct FrameViews {
    views: Vec<ViewOutput>,
    indices: FxHashMap<GlobalElementId, Option<usize>>,
    prebuilt: FxHashMap<GlobalElementId, (AnyElement, Box<BuiltLayout>)>,
}

impl FrameViews {
    pub(crate) fn clear(&mut self) {
        self.views.clear();
        self.indices.clear();
        self.clear_prebuilt();
    }

    pub(crate) fn clear_prebuilt(&mut self) {
        self.prebuilt.clear();
    }

    pub(crate) fn checkpoint(&self) -> usize {
        self.views.len()
    }

    pub(crate) fn mark_hitboxes_captured_since(&mut self, checkpoint: usize) {
        for view in &mut self.views[checkpoint..] {
            view.hitboxes_captured = true;
        }
    }

    pub(crate) fn input_changed(&self, window: &Window) -> bool {
        self.views
            .iter()
            .any(|view| !view.data.reads.input_is_current(window))
    }

    pub(crate) fn global_changed(&self, key: TypeId, present: bool) -> bool {
        self.views
            .iter()
            .any(|view| view.data.reads.global_changed(key, present))
    }

    pub(crate) fn rollback(&mut self, len: usize) {
        if self.views.len() == len {
            return;
        }
        self.views.truncate(len);
        self.indices.clear();
        for (index, output) in self.views.iter().enumerate() {
            Self::index(&mut self.indices, &output.id, index);
        }
    }

    fn push(&mut self, output: ViewOutput) -> usize {
        let index = self.views.len();
        Self::index(&mut self.indices, &output.id, index);
        self.views.push(output);
        index
    }

    fn index(
        indices: &mut FxHashMap<GlobalElementId, Option<usize>>,
        id: &GlobalElementId,
        index: usize,
    ) {
        indices
            .entry(id.clone())
            .and_modify(|index| *index = None)
            .or_insert(Some(index));
    }
}

struct ViewOutput {
    id: GlobalElementId,
    data: Rc<ViewData>,
    prepaint: Range<PrepaintStateIndex>,
    paint: Option<Range<PaintIndex>>,
    descendants: Range<usize>,
    dispatch: Option<DispatchNodeId>,
    hitboxes_captured: bool,
    prepaint_context: Option<Rc<RetainedViewContext>>,
    paint_context: Option<Rc<RetainedViewContext>>,
    paint_inputs: Option<PaintInputs>,
    #[cfg(not(target_family = "wasm"))]
    prepaint_a11y: Option<Rc<A11yFragment>>,
    #[cfg(not(target_family = "wasm"))]
    paint_a11y: Option<Rc<A11yFragment>>,
}

impl ViewOutput {
    fn replayed(
        &self,
        prepaint: Range<PrepaintStateIndex>,
        descendants: Range<usize>,
        dispatch: Option<DispatchNodeId>,
    ) -> Self {
        Self {
            id: self.id.clone(),
            data: self.data.clone(),
            prepaint,
            paint: None,
            descendants,
            dispatch,
            hitboxes_captured: self.hitboxes_captured,
            prepaint_context: self.prepaint_context.clone(),
            paint_context: self.paint_context.clone(),
            paint_inputs: self.paint_inputs.clone(),
            #[cfg(not(target_family = "wasm"))]
            prepaint_a11y: self.prepaint_a11y.clone(),
            #[cfg(not(target_family = "wasm"))]
            paint_a11y: self.paint_a11y.clone(),
        }
    }
}

struct ViewData {
    entity: EntityId,
    layout: LayoutId,
    layout_inputs: LayoutInputs,
    inputs: PaintInputs,
    reads: Reads,
    accessed: FxHashSet<EntityId>,
    layout_states: Vec<ElementStateKey>,
    rebuild: Option<Rebuild>,
}

#[derive(Clone)]
struct Rebuild {
    view: AnyWeakView,
    key: LayoutKey,
    context: Rc<RetainedViewContext>,
}

#[derive(PartialEq)]
struct LayoutInputs {
    rem: Pixels,
    scale: f32,
    viewport_size: Size<Pixels>,
    font_generation: usize,
    text: TextStyle,
    keyboard: bool,
    touch: bool,
    focus: Option<FocusId>,
}

impl LayoutInputs {
    fn capture(window: &Window) -> Self {
        Self {
            rem: window.rem_size(),
            scale: window.scale_factor(),
            viewport_size: window.viewport_size,
            font_generation: window.text_system().font_generation(),
            text: window.text_style(),
            keyboard: window.last_input_was_keyboard(),
            touch: window.last_input_was_touch(),
            focus: window.focus,
        }
    }
}

#[derive(Clone, PartialEq)]
struct PaintInputs {
    bounds: Bounds<Pixels>,
    clip: ClipStackSnapshot,
    text: TextStyle,
    focus: Option<FocusId>,
    opacity: u32,
    presentation: SubtreePresentation,
    transform: ResolvedSubtreeTransform,
}

impl PaintInputs {
    fn capture(window: &Window, bounds: Bounds<Pixels>) -> Self {
        Self {
            bounds,
            clip: window.clip_stack(),
            text: window.text_style(),
            focus: window.focus,
            opacity: window.element_opacity().to_bits(),
            presentation: window.subtree_presentation(),
            transform: window.subtree_transform(),
        }
    }
}

pub(crate) struct BuiltLayout {
    id: LayoutId,
    inputs: LayoutInputs,
    reads: Reads,
    accessed: FxHashSet<EntityId>,
    layout_states: Vec<ElementStateKey>,
    rebuild: Option<Rebuild>,
}

pub(crate) enum Layout {
    Built(AnyElement, Box<BuiltLayout>),
    Previous(usize),
    Spliced(Box<splice::SpliceLayout>),
}

pub(crate) enum Prepaint {
    Built(AnyElement, usize),
    Replayed { previous: usize, current: usize },
    Spliced(Box<splice::SplicePaint>),
    Aborted,
}

pub(crate) fn can_record(window: &Window, cx: &App) -> bool {
    #[cfg(target_family = "wasm")]
    if window.a11y.is_active() {
        return false;
    }
    window.view_retention_enabled()
        && !window.is_inspector_picking(cx)
        && cx.active_drag.is_none()
        && window.image_cache_stack.is_empty()
        && window.subtree_presentation().paints()
}

pub(crate) fn layout(
    id: &GlobalElementId,
    entity: EntityId,
    view: AnyWeakView,
    window: &mut Window,
    cx: &mut App,
    render: &mut dyn FnMut(&mut Window, &mut App) -> AnyElement,
) -> (LayoutId, Layout) {
    let inputs = LayoutInputs::capture(window);
    let key = window.layout_keys.current();
    if let Some((element, layout)) = window.next_frame.retained_views.prebuilt.remove(id)
        && layout.inputs == inputs
        && layout.rebuild.as_ref().map(|rebuild| rebuild.key) == key
    {
        return (layout.id, Layout::Built(element, layout));
    }

    let previous = &window.rendered_frame.retained_views;
    if !window.refreshing
        && !window.refresh_retained_views
        && id.len() == 1
        && window
            .root
            .as_ref()
            .is_some_and(|root| root.entity_id() == entity)
        && let Some(&Some(index)) = previous.indices.get(id)
        && let Some(view) = previous.views.get(index)
        && view.data.layout_inputs == inputs
    {
        let layout = view.data.layout;
        if reusable(window, view, cx) && window.try_retain_layout(layout) {
            return (layout, Layout::Previous(index));
        }
        if let Some(splice) = splice::try_layout(window, cx, index) {
            return (layout, Layout::Spliced(Box::new(splice)));
        }
    }

    let rebuild = key.and_then(|key| {
        Some(Rebuild {
            view,
            key,
            context: Rc::new(window.retained_view_context()?),
        })
    });
    let (element, layout) = build_layout(entity, inputs, rebuild, window, cx, render);
    (layout.id, Layout::Built(element, layout))
}

fn build_layout(
    entity: EntityId,
    inputs: LayoutInputs,
    rebuild: Option<Rebuild>,
    window: &mut Window,
    cx: &mut App,
    render: impl FnOnce(&mut Window, &mut App) -> AnyElement,
) -> (AnyElement, Box<BuiltLayout>) {
    let (((element, id), reads, layout_states), accessed) = cx.detect_accessed_entities(|cx| {
        dependencies::capture_layout(|| {
            cx.retained_revisions.read(RevisionKey::Entity(entity));
            let mut element = render(window, cx);
            let id = element.request_layout(window, cx);
            (element, id)
        })
    });
    (
        element,
        Box::new(BuiltLayout {
            id,
            inputs,
            reads,
            accessed,
            layout_states,
            rebuild,
        }),
    )
}

fn reusable(window: &Window, view: &ViewOutput, cx: &App) -> bool {
    if !can_replay_window_outputs(window, view) {
        return false;
    }
    #[cfg(not(target_family = "wasm"))]
    if window.a11y.is_active() && (view.prepaint_a11y.is_none() || view.paint_a11y.is_none()) {
        return false;
    }
    std::iter::once(view)
        .chain(&window.rendered_frame.retained_views.views[view.descendants.clone()])
        .all(|view| {
            !window.dirty_views.contains(&view.data.entity) && owns_current_reads(window, cx, view)
        })
}

fn owns_current_reads(window: &Window, cx: &App, view: &ViewOutput) -> bool {
    !window.portal_anchor_dependency_invalidates_view(view.data.entity)
        && view.data.reads.is_current(window, cx)
}

fn can_replay_window_outputs(window: &Window, view: &ViewOutput) -> bool {
    !window.portal_anchor_capture_requires_fresh_prepaint()
        && !window.reveal_target_capture_requires_fresh_prepaint()
        && view
            .paint
            .as_ref()
            .is_some_and(|paint| window.can_replay_view_output(&view.prepaint, paint))
}

pub(crate) fn prepaint(
    id: &GlobalElementId,
    entity: EntityId,
    bounds: Bounds<Pixels>,
    layout: Layout,
    window: &mut Window,
    cx: &mut App,
) -> Prepaint {
    let inputs = PaintInputs::capture(window, bounds);
    match layout {
        Layout::Built(element, layout) => {
            record_prepaint(id, entity, inputs, element, *layout, window, cx)
        }
        Layout::Spliced(splice) => {
            if let Some(paint) = splice::prepaint(window, cx, *splice, &inputs) {
                Prepaint::Spliced(Box::new(paint))
            } else {
                window.reject_retained_view_replay();
                Prepaint::Aborted
            }
        }
        Layout::Previous(previous) => {
            let old = &window.rendered_frame.retained_views.views[previous];
            if old.data.inputs != inputs
                || !reusable(window, old, cx)
                || !can_replay_accessibility(window, old)
            {
                window.reject_retained_view_replay();
                return Prepaint::Aborted;
            }
            let current = replay_prepaint(window, cx, previous);
            Prepaint::Replayed { previous, current }
        }
    }
}

fn record_prepaint(
    id: &GlobalElementId,
    entity: EntityId,
    inputs: PaintInputs,
    mut element: AnyElement,
    mut layout: BuiltLayout,
    window: &mut Window,
    cx: &mut App,
) -> Prepaint {
    let start = window.prepaint_index();
    let dispatch = window.next_frame.dispatch_tree.active_node_id();
    let context = layout
        .rebuild
        .as_ref()
        .and_then(|_| window.retained_view_context());
    #[cfg(not(target_family = "wasm"))]
    let a11y_start = a11y::checkpoint(&window.a11y);
    let children = window.next_frame.retained_views.views.len();
    let (((), reads), accessed) = cx.detect_accessed_entities(|cx| {
        dependencies::capture(|| {
            element.prepaint(window, cx);
        })
    });
    layout.reads.extend(reads);
    layout.accessed.extend(accessed);
    let end = window.prepaint_index();
    let current = window.next_frame.retained_views.views.len();
    let index = window.next_frame.retained_views.push(ViewOutput {
        id: id.clone(),
        data: Rc::new(ViewData {
            entity,
            layout: layout.id,
            layout_inputs: layout.inputs,
            inputs,
            reads: layout.reads,
            accessed: layout.accessed,
            layout_states: layout.layout_states,
            rebuild: layout.rebuild,
        }),
        prepaint: start..end,
        paint: None,
        descendants: children..current,
        dispatch,
        hitboxes_captured: false,
        prepaint_context: context.map(Rc::new),
        paint_context: None,
        paint_inputs: None,
        #[cfg(not(target_family = "wasm"))]
        prepaint_a11y: a11y::capture(&window.a11y, &a11y_start),
        #[cfg(not(target_family = "wasm"))]
        paint_a11y: None,
    });
    Prepaint::Built(element, index)
}

fn can_replay_accessibility(_window: &Window, _view: &ViewOutput) -> bool {
    #[cfg(not(target_family = "wasm"))]
    {
        let offset = Point::default();
        a11y::can_replay(&_window.a11y, _view.prepaint_a11y.as_deref(), offset)
            && a11y::can_replay(&_window.a11y, _view.paint_a11y.as_deref(), offset)
    }
    #[cfg(target_family = "wasm")]
    true
}

fn replay_prepaint(window: &mut Window, cx: &mut App, index: usize) -> usize {
    let old = &window.rendered_frame.retained_views.views[index];
    let range = old.prepaint.clone();
    cx.entities.extend_accessed(&old.data.accessed);
    let retained_states = std::iter::once(old)
        .chain(&window.rendered_frame.retained_views.views[old.descendants.clone()])
        .map(|view| view.data.clone())
        .collect::<Vec<_>>();
    for view in retained_states {
        window.retain_view_layout_states(&view.layout_states);
    }
    let start = window.prepaint_index();
    #[cfg(not(target_family = "wasm"))]
    let old = &window.rendered_frame.retained_views.views[index];
    #[cfg(not(target_family = "wasm"))]
    if !a11y::replay(
        &mut window.a11y,
        old.prepaint_a11y.as_deref(),
        Point::default(),
    ) {
        window.reject_retained_view_replay();
    }
    let dispatch = window.reuse_prepaint(range.clone());
    let end = window.prepaint_index();
    let active = window.next_frame.dispatch_tree.active_node_id();

    let earlier = &window.rendered_frame.retained_views.views;
    let next = &mut window.next_frame.retained_views;
    let output = &earlier[index];
    let children = output.descendants.clone();
    let new_children = next.views.len();
    for old in &earlier[children.clone()] {
        cx.entities.extend_accessed(&old.data.accessed);
        next.push(
            old.replayed(
                old.prepaint.start.rebase(&range.start, &start)
                    ..old.prepaint.end.rebase(&range.start, &start),
                (new_children + old.descendants.start - children.start)
                    ..(new_children + old.descendants.end - children.start),
                old.dispatch
                    .filter(|&node| node.is_in(&PrepaintStateIndex::dispatch_range(&range)))
                    .map(|node| dispatch.refresh_node_id(node)),
            ),
        );
    }
    let current = next.views.len();
    next.push(output.replayed(start..end, new_children..current, active))
}

pub(crate) fn paint(state: &mut Prepaint, window: &mut Window, cx: &mut App) {
    let start = window.paint_index();
    match state {
        Prepaint::Built(element, index) => {
            let output = &window.next_frame.retained_views.views[*index];
            let context = output
                .data
                .rebuild
                .as_ref()
                .and_then(|_| window.retained_view_context());
            let inputs = PaintInputs::capture(window, output.data.inputs.bounds);
            #[cfg(not(target_family = "wasm"))]
            let a11y_start = a11y::checkpoint(&window.a11y);
            let (((), reads), accessed) = cx.detect_accessed_entities(|cx| {
                dependencies::capture(|| {
                    window.with_atlas_texture_lease_paint_scope(|window| element.paint(window, cx));
                })
            });
            let end = window.paint_index();
            let output = &mut window.next_frame.retained_views.views[*index];
            let data = Rc::get_mut(&mut output.data).expect("output is recorded in this frame");
            data.reads.extend(reads);
            data.accessed.extend(accessed);
            output.paint = Some(start..end);
            output.paint_context = context.map(Rc::new);
            output.paint_inputs = Some(inputs);
            #[cfg(not(target_family = "wasm"))]
            {
                output.paint_a11y = a11y::capture(&window.a11y, &a11y_start);
            }
        }
        Prepaint::Replayed { previous, current } => {
            let earlier = &window.rendered_frame.retained_views.views[*previous];
            let range = earlier.paint.clone().expect("retained view has painted");
            if !paint_inputs_match(earlier, window)
                || !std::iter::once(earlier)
                    .chain(&window.rendered_frame.retained_views.views[earlier.descendants.clone()])
                    .all(|view| view.data.reads.input_is_current(window))
            {
                window.reject_retained_view_replay();
                return;
            }
            #[cfg(not(target_family = "wasm"))]
            if !a11y::replay(
                &mut window.a11y,
                earlier.paint_a11y.as_deref(),
                Point::default(),
            ) {
                window.reject_retained_view_replay();
                return;
            }
            if !window.reuse_retained_view_paint(range.clone()) {
                window.reject_retained_view_replay();
                return;
            }
            let end = window.paint_index();
            let earlier = &window.rendered_frame.retained_views.views;
            let child_range = earlier[*previous].descendants.clone();
            let next = &mut window.next_frame.retained_views.views;
            let children = next[*current].descendants.clone();
            for (new, old) in next[children].iter_mut().zip(&earlier[child_range]) {
                new.paint = old.paint.as_ref().map(|old| {
                    old.start.rebase(&range.start, &start)..old.end.rebase(&range.start, &start)
                });
            }
            next[*current].paint = Some(start..end);
        }
        Prepaint::Spliced(paint) => splice::paint(window, cx, paint),
        Prepaint::Aborted => {}
    }
}

fn paint_inputs_match(view: &ViewOutput, window: &Window) -> bool {
    view.paint_inputs
        .as_ref()
        .is_some_and(|inputs| *inputs == PaintInputs::capture(window, view.data.inputs.bounds))
}
