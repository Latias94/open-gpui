//! Copies the accepted journal around dirty, independently rebuildable views.

use crate::{
    AnyView, IntoElement, LayoutId, View, geometry::SubtreeGeometryValidity,
    key_dispatch::DispatchSplice,
};

use super::{
    AnyElement, App, BuiltLayout, DispatchNodeId, LayoutInputs, PaintIndex, PaintInputs, Prepaint,
    PrepaintStateIndex, Range, ViewOutput, Window, build_layout, can_replay_window_outputs,
    owns_current_reads, paint_inputs_match, record_prepaint,
};

pub(crate) struct SpliceLayout {
    root: usize,
    gaps: Vec<LaidOutGap>,
    segments: Vec<Segment>,
    copies: Vec<CopyRecord>,
}

struct LaidOutGap {
    previous: usize,
    element: AnyElement,
    layout: Box<BuiltLayout>,
    validity: SubtreeGeometryValidity,
}

pub(crate) struct SplicePaint {
    root: usize,
    current: usize,
    gaps: Vec<PaintGap>,
    segments: Vec<Segment>,
    copies: Vec<CopyRecord>,
}

struct PaintGap {
    previous: usize,
    dispatch: DispatchNodeId,
    prepaint: Prepaint,
    validity: SubtreeGeometryValidity,
}

struct Segment {
    prepaint: Range<PrepaintStateIndex>,
    paint: Range<PaintIndex>,
}

struct CopyRecord {
    previous: usize,
    current: usize,
    first_segment: usize,
    last_segment: usize,
}

pub(super) fn try_layout(
    window: &mut Window,
    cx: &mut App,
    previous: usize,
) -> Option<SpliceLayout> {
    let source = &window.rendered_frame.retained_views;
    let root = &source.views[previous];
    if window.a11y.is_active()
        || !can_replay_window_outputs(window, root)
        || !owns_current_reads(window, cx, root)
        || root.dispatch.is_none()
        || window.dirty_views.iter().any(|entity| {
            root.data.entity != *entity
                && source.views[root.descendants.clone()]
                    .iter()
                    .all(|view| view.data.entity != *entity)
        })
    {
        return None;
    }

    // Records are postorder. Skip a dirty view's descendants once it is selected as one gap.
    let mut gaps = Vec::new();
    let mut cursor = root.descendants.end;
    while cursor > root.descendants.start {
        cursor -= 1;
        let view = &source.views[cursor];
        if !owns_current_reads(window, cx, view) {
            let rebuild = view.data.rebuild.as_ref()?;
            // A caller outside the gap may have used these hitboxes to build its own output.
            if view.hitboxes_captured
                || view.prepaint_context.is_none()
                || view.paint_context.is_none()
                || source.indices.get(&view.id) != Some(&Some(cursor))
            {
                return None;
            }
            gaps.push((cursor, rebuild.view.upgrade()?));
            cursor = view.descendants.start;
        }
    }
    if gaps.is_empty() {
        return None;
    }
    gaps.reverse();
    let (segments, copies) = plan_segments(root, &source.views, &gaps)?;
    let layout_root = root.data.layout;
    let mut layout_trees = vec![(layout_root, Vec::new())];
    for (index, _) in &gaps {
        let layout = source.views[*index].data.layout;
        let tree = window.retained_layout_root(layout)?;
        // The gap must have an enclosing layout that still owns the computed geometry.
        if tree == layout {
            return None;
        }
        if let Some((_, excluded)) = layout_trees.iter_mut().find(|(root, _)| *root == tree) {
            excluded.push(layout);
        } else {
            layout_trees.push((tree, vec![layout]));
        }
    }
    let revision = window.retained_layout_revision();
    if !claim_trees(window, &layout_trees) {
        return None;
    }

    let mut rebuilt = Vec::with_capacity(gaps.len());
    for (index, view) in gaps {
        let old = &window.rendered_frame.retained_views.views[index];
        let id = old.id.clone();
        let entity = old.data.entity;
        let old_layout = old.data.layout;
        let rebuild = old.data.rebuild.clone().expect("qualified rebuild handle");
        let context = rebuild.context.clone();
        let key = rebuild.key;
        let validity = SubtreeGeometryValidity::new(window.subtree_geometry_validity());
        let (element, layout) =
            window.with_retained_view_context(&id, &context, &validity, |window| {
                window.layout_keys.push_existing(key);
                let inputs = LayoutInputs::capture(window);
                let built = window.with_rendered_view(entity, |window| {
                    build_layout(entity, inputs, Some(rebuild), window, cx, |window, cx| {
                        View::render(view, window, cx).into_any_element()
                    })
                });
                window.layout_keys.pop();
                built
            });
        let unchanged = validity.is_valid()
            && layout.id == old_layout
            && window.retained_layout_revision() == revision;
        rebuilt.push(LaidOutGap {
            previous: index,
            element,
            layout,
            validity,
        });
        if !unchanged {
            for (root, _) in layout_trees {
                window.release_spliced_layout(root);
            }
            for gap in rebuilt {
                if !gap.validity.is_valid() {
                    continue;
                }
                let id = window.rendered_frame.retained_views.views[gap.previous]
                    .id
                    .clone();
                window
                    .next_frame
                    .retained_views
                    .prebuilt
                    .insert(id, (gap.element, gap.layout));
            }
            return None;
        }
    }
    Some(SpliceLayout {
        root: previous,
        gaps: rebuilt,
        segments,
        copies,
    })
}

fn claim_trees(window: &mut Window, trees: &[(LayoutId, Vec<LayoutId>)]) -> bool {
    for (index, (root, excluded)) in trees.iter().enumerate() {
        if !window.claim_spliced_layout(*root, excluded) {
            for (claimed, _) in &trees[..index] {
                window.release_spliced_layout(*claimed);
            }
            return false;
        }
    }
    true
}

fn plan_segments(
    root: &ViewOutput,
    records: &[ViewOutput],
    gaps: &[(usize, AnyView)],
) -> Option<(Vec<Segment>, Vec<CopyRecord>)> {
    let root_paint = root.paint.as_ref()?;
    let mut prepaint = root.prepaint.start.clone();
    let mut paint = root_paint.start.clone();
    let mut segments = Vec::with_capacity(gaps.len() + 1);
    for &(index, _) in gaps {
        let gap = &records[index];
        let gap_paint = gap.paint.as_ref()?;
        let range = prepaint.clone()..gap.prepaint.start.clone();
        if !prepaint.precedes(&gap.prepaint.start)
            || !paint.precedes(&gap_paint.start)
            || !gap
                .dispatch?
                .is_in(&PrepaintStateIndex::dispatch_range(&range))
        {
            return None;
        }
        segments.push(Segment {
            prepaint: range,
            paint: paint..gap_paint.start.clone(),
        });
        prepaint = gap.prepaint.end.clone();
        paint = gap_paint.end.clone();
    }
    if !prepaint.precedes(&root.prepaint.end) || !paint.precedes(&root_paint.end) {
        return None;
    }
    segments.push(Segment {
        prepaint: prepaint..root.prepaint.end.clone(),
        paint: paint..root_paint.end.clone(),
    });

    let mut copies = Vec::new();
    let mut start = root.descendants.start;
    for (last_segment, end) in gaps
        .iter()
        .map(|(index, _)| records[*index].descendants.start)
        .chain(std::iter::once(root.descendants.end))
        .enumerate()
    {
        for previous in start..end {
            let view = &records[previous];
            let first_segment = gaps.partition_point(|(index, _)| *index < view.descendants.start);
            let first = &segments[first_segment];
            let last = &segments[last_segment];
            let view_paint = view.paint.as_ref()?;
            // Custom elements can paint their children in a different order from prepaint.
            // A split only retains records whose cursors actually belong to these segments.
            if !contains(
                &first.prepaint,
                &view.prepaint.start,
                PrepaintStateIndex::precedes,
            ) || !contains(
                &last.prepaint,
                &view.prepaint.end,
                PrepaintStateIndex::precedes,
            ) || !contains(&first.paint, &view_paint.start, PaintIndex::precedes)
                || !contains(&last.paint, &view_paint.end, PaintIndex::precedes)
            {
                return None;
            }
            copies.push(CopyRecord {
                previous,
                current: 0,
                first_segment,
                last_segment,
            });
        }
        if let Some((gap, _)) = gaps.get(last_segment) {
            start = gap + 1;
        }
    }
    Some((segments, copies))
}

fn contains<T>(range: &Range<T>, cursor: &T, precedes: fn(&T, &T) -> bool) -> bool {
    precedes(&range.start, cursor) && precedes(cursor, &range.end)
}

pub(super) fn prepaint(
    window: &mut Window,
    cx: &mut App,
    splice: SpliceLayout,
    inputs: &PaintInputs,
) -> Option<SplicePaint> {
    let SpliceLayout {
        root,
        gaps,
        segments,
        mut copies,
    } = splice;
    let source = &window.rendered_frame.retained_views;
    let old = &source.views[root];
    let target_dispatch = window.next_frame.dispatch_tree.active_node_id()?;
    let source_dispatch = old.dispatch?;
    if &old.data.inputs != inputs
        || window.a11y.is_active()
        || !can_replay_window_outputs(window, old)
        || !owns_current_reads(window, cx, old)
        || copies
            .iter()
            .any(|copy| !owns_current_reads(window, cx, &source.views[copy.previous]))
    {
        window.reject_retained_view_replay();
        return None;
    }
    cx.entities.extend_accessed(&old.data.accessed);
    let source_base = old.descendants.start;
    let descendant_count = old.descendants.len();
    let retained_states = std::iter::once(root)
        .chain(copies.iter().map(|copy| copy.previous))
        .map(|index| source.views[index].data.clone())
        .collect::<Vec<_>>();
    for view in retained_states {
        window.retain_view_layout_states(&view.layout_states);
    }
    let target_base = window.next_frame.retained_views.views.len();
    let mut positions = vec![target_base; descendant_count];
    let start = window.prepaint_index();
    let mut dispatch = DispatchSplice::new(source_dispatch, target_dispatch);
    let mut segment_starts = Vec::with_capacity(segments.len());
    let mut paint_gaps = Vec::with_capacity(gaps.len());
    let mut gaps = gaps.into_iter();
    let mut pending = copies.iter_mut().peekable();

    for (index, part) in segments.iter().enumerate() {
        segment_starts.push(window.prepaint_index());
        window.reuse_prepaint_segment(part.prepaint.clone(), &mut dispatch);
        while let Some(copy) = pending.next_if(|copy| copy.last_segment == index) {
            let old = &window.rendered_frame.retained_views.views[copy.previous];
            cx.entities.extend_accessed(&old.data.accessed);
            let next = &mut window.next_frame.retained_views;
            let current = next.views.len();
            positions[copy.previous - source_base] = current;
            let first = copy.first_segment;
            let range = old
                .prepaint
                .start
                .rebase(&segments[first].prepaint.start, &segment_starts[first])
                ..old
                    .prepaint
                    .end
                    .rebase(&part.prepaint.start, &segment_starts[index]);
            let descendants = positions[old.descendants.start - source_base]..current;
            let node = old.dispatch.and_then(|node| dispatch.target_for(node));
            copy.current = next.push(old.replayed(range, descendants, node));
        }
        if let Some(gap) = gaps.next() {
            let old = &window.rendered_frame.retained_views.views[gap.previous];
            positions[old.descendants.start - source_base] =
                window.next_frame.retained_views.views.len();
            let id = old.id.clone();
            let entity = old.data.entity;
            let node = dispatch.target_for(old.dispatch.expect("qualified gap wrapper"));
            let context = old
                .prepaint_context
                .clone()
                .expect("qualified prepaint context");
            let key = gap
                .layout
                .rebuild
                .as_ref()
                .expect("qualified rebuild handle")
                .key;
            let Some(node) = node else {
                window.reject_retained_view_replay();
                return None;
            };
            window.next_frame.dispatch_tree.set_active_node(node);
            let prepaint =
                window.with_retained_view_context(&id, &context, &gap.validity, |window| {
                    window.layout_keys.push_prepaint(key);
                    let layout_id = gap.layout.id;
                    let bounds = window.layout_bounds(layout_id);
                    let inputs = PaintInputs::capture(window, bounds);
                    let output = window.with_prepaint_layout_id(layout_id, |window| {
                        window.with_rendered_view(entity, |window| {
                            record_prepaint(
                                &id,
                                entity,
                                inputs,
                                gap.element,
                                *gap.layout,
                                window,
                                cx,
                            )
                        })
                    });
                    window.layout_keys.pop();
                    output
                });
            window
                .next_frame
                .dispatch_tree
                .set_active_node(target_dispatch);
            if !gap.validity.is_valid() {
                return None;
            }
            paint_gaps.push(PaintGap {
                previous: gap.previous,
                dispatch: node,
                prepaint,
                validity: gap.validity,
            });
        }
    }
    let end = window.prepaint_index();
    let old = &window.rendered_frame.retained_views.views[root];
    let next = &mut window.next_frame.retained_views;
    let current = next.views.len();
    next.push(old.replayed(start..end, target_base..current, Some(target_dispatch)));
    Some(SplicePaint {
        root,
        current,
        gaps: paint_gaps,
        segments,
        copies,
    })
}

pub(super) fn paint(window: &mut Window, cx: &mut App, splice: &mut SplicePaint) {
    let source = &window.rendered_frame.retained_views.views;
    if !paint_inputs_match(&source[splice.root], window)
        || !owns_current_reads(window, cx, &source[splice.root])
        || splice
            .copies
            .iter()
            .any(|copy| !owns_current_reads(window, cx, &source[copy.previous]))
    {
        window.reject_retained_view_replay();
        return;
    }

    let start = window.paint_index();
    let mut segment_starts = Vec::with_capacity(splice.segments.len());
    for (index, part) in splice.segments.iter().enumerate() {
        segment_starts.push(window.paint_index());
        if !window.reuse_retained_view_paint(part.paint.clone()) {
            window.reject_retained_view_replay();
            return;
        }
        if let Some(gap) = splice.gaps.get_mut(index) {
            let old = &window.rendered_frame.retained_views.views[gap.previous];
            let id = old.id.clone();
            let entity = old.data.entity;
            let context = old.paint_context.clone().expect("qualified paint context");
            window
                .next_frame
                .dispatch_tree
                .set_active_node(gap.dispatch);
            window.with_retained_view_context(&id, &context, &gap.validity, |window| {
                window.with_rendered_view(entity, |window| {
                    super::paint(&mut gap.prepaint, window, cx)
                });
            });
            if !gap.validity.is_valid() {
                window.reject_retained_view_replay();
                return;
            }
        }
    }
    let end = window.paint_index();
    let source = &window.rendered_frame.retained_views.views;
    let next = &mut window.next_frame.retained_views.views;
    for copy in &splice.copies {
        let old = source[copy.previous]
            .paint
            .as_ref()
            .expect("qualified paint range");
        let first = copy.first_segment;
        let last = copy.last_segment;
        next[copy.current].paint = Some(
            old.start
                .rebase(&splice.segments[first].paint.start, &segment_starts[first])
                ..old
                    .end
                    .rebase(&splice.segments[last].paint.start, &segment_starts[last]),
        );
    }
    next[splice.current].paint = Some(start..end);
}
