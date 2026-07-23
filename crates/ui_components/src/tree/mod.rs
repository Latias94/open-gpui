//! Tree component and renderer-neutral state for hierarchical tree surfaces.

mod descriptor;
mod model;
mod movement;
mod render_plan;
mod runtime;
mod style;

use crate::a11y::UiA11yElementExt;
use crate::collection_typeahead::CollectionTypeaheadInput;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::scroll_surface::{
    ScrollSurfaceRuntime, materialize_fixed_row, scroll_surface_handle, vertical_scroll_offset,
    vertical_viewport_extent,
};
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, BringIntoViewOptions, CursorStyle, Empty, Entity, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce, RevealTargetExt,
    ScrollChainFence, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px, rgb, rgba,
};
use open_gpui_ui_core::{AccessibleAction, Role, SemanticDescriptor, Sizable, Size, UiPx, ui_px};
use std::rc::Rc;

pub(crate) use descriptor::apply_tree_expanded_overrides;
pub use descriptor::{TreeChildrenLoadState, TreeItemDescriptor};
pub use model::{
    TreeFocusTarget, TreeItemState, TreeKeyboardAction, TreeSelection, TreeState, TreeToggle,
    tree_navigation_target,
};
pub(crate) use model::{nonnegative_px, tree_children_load_hint};
pub use movement::{TreeDropPosition, TreeMove, TreeMoveTarget, apply_tree_move};
pub(crate) use render_plan::TreeRenderPlan;
pub use render_plan::{TreeBehaviorSnapshot, TreeRowBehaviorSnapshot};
use runtime::{TreeFocusOperation, TreeFocusOperationStage, TreeRuntime};
pub use style::TreeMetrics;

type TreeSelectHandler = Rc<dyn Fn(TreeSelection, &mut Window, &mut App)>;
type TreeToggleHandler = Rc<dyn Fn(TreeToggle, &mut Window, &mut App)>;
type TreeMoveHandler = Rc<dyn Fn(TreeMove, &mut Window, &mut App)>;

const DEFAULT_TREE_VIEWPORT_ITEM_COUNT: usize = 12;
const DEFAULT_TREE_OVERSCAN_COUNT: usize = 4;

#[derive(Clone)]
struct TreeDragPayload {
    tree_id: String,
    source_value: String,
}

/// A concrete GPUI tree renderer backed by [`TreeState`].
#[derive(IntoElement)]
pub struct Tree {
    id: String,
    label: SharedString,
    items: Vec<TreeItemDescriptor>,
    size: Size,
    selected_value: Option<String>,
    focused_value: Option<String>,
    virtualized: bool,
    viewport_item_count: usize,
    overscan_count: usize,
    draggable: bool,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
    on_move: Option<TreeMoveHandler>,
}

impl Tree {
    /// Creates a new tree renderer.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = TreeItemDescriptor>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items: items.into_iter().collect(),
            size: Size::Medium,
            selected_value: None,
            focused_value: None,
            virtualized: false,
            viewport_item_count: DEFAULT_TREE_VIEWPORT_ITEM_COUNT,
            overscan_count: DEFAULT_TREE_OVERSCAN_COUNT,
            draggable: false,
            on_select: None,
            on_toggle: None,
            on_move: None,
        }
    }

    /// Adds one root item descriptor.
    pub fn item(mut self, item: TreeItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Applies the default selected item value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected_value = Some(value.into().to_string());
        self
    }

    /// Applies the default focused item value for adapter-owned runtime state.
    pub fn default_focused(mut self, value: impl Into<SharedString>) -> Self {
        self.focused_value = Some(value.into().to_string());
        self
    }

    /// Enables or disables fixed-row virtualized rendering.
    pub fn virtualized(mut self, virtualized: bool) -> Self {
        self.virtualized = virtualized;
        self
    }

    /// Applies the fallback viewport item count used before the viewport is measured.
    pub fn viewport_item_count(mut self, viewport_item_count: usize) -> Self {
        self.viewport_item_count = viewport_item_count.max(1);
        self
    }

    /// Applies the virtualized overscan item budget.
    pub fn overscan_count(mut self, overscan_count: usize) -> Self {
        self.overscan_count = overscan_count;
        self
    }

    /// Enables or disables pointer drag move affordances.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// Registers a tree selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(TreeSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Registers a tree expansion toggle handler.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(TreeToggle, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Registers a controlled tree move handler.
    pub fn on_move(mut self, handler: impl Fn(TreeMove, &mut Window, &mut App) + 'static) -> Self {
        self.on_move = Some(Rc::new(handler));
        self
    }

    /// Returns root item descriptors.
    pub fn items(&self) -> &[TreeItemDescriptor] {
        &self.items
    }

    /// Returns resolved tree state from the builder seed.
    pub fn state(&self) -> TreeState {
        self.resolve_state(
            self.items.clone(),
            self.selected_value.as_deref(),
            self.focused_value.as_deref(),
        )
    }

    /// Returns a fixed-row virtualized behavior snapshot from the builder seed.
    pub fn behavior_snapshot(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> TreeBehaviorSnapshot {
        let plan = self.render_plan(scroll_offset, viewport_extent);
        TreeBehaviorSnapshot::from_render_plan(&plan)
    }

    fn render_plan(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> TreeRenderPlan {
        TreeRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.state(),
            scroll_offset,
            viewport_extent,
            self.viewport_item_count,
            self.overscan_count,
        )
    }

    fn resolve_state(
        &self,
        items: Vec<TreeItemDescriptor>,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
    ) -> TreeState {
        TreeState::resolve(
            self.size,
            self.label.to_string(),
            selected_value,
            focused_value,
            items,
        )
    }
}

impl Sizable for Tree {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Tree {
            id,
            label,
            items,
            size,
            selected_value,
            focused_value,
            virtualized,
            viewport_item_count,
            overscan_count,
            draggable,
            on_select,
            on_toggle,
            on_move,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = id.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| {
                TreeRuntime::new(
                    ScrollSurfaceRuntime::new(None),
                    selected_value.clone(),
                    focused_value.clone(),
                )
            });
            runtime.update(cx, |runtime, _| runtime.set_virtualized(virtualized));
            let scroll_chain_anchor = if virtualized {
                Some(runtime.update(cx, |runtime, _| runtime.scroll_chain_anchor(window)))
            } else {
                None
            };
            let runtime_snapshot = runtime.read(cx).clone();
            let resolved_items =
                apply_tree_expanded_overrides(&items, &runtime_snapshot.expanded_values);
            let state = TreeState::resolve(
                size,
                label.to_string(),
                runtime_snapshot
                    .selected_value
                    .as_deref()
                    .or(selected_value.as_deref()),
                runtime_snapshot
                    .focused_value
                    .as_deref()
                    .or(focused_value.as_deref()),
                resolved_items,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, cx));
            let focus_claim_revision = window.focus_claim_revision();
            runtime.update(cx, |runtime, _| {
                runtime.retain_current_focus_claim(focus_claim_revision);
            });

            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let root_focus_handle = state
                .focused_index()
                .and_then(|index| focus_handles.get(index).cloned().flatten());
            let scroll_handle = scroll_surface_handle(&runtime.read(cx).scroll_surface, None);
            let pending_focus = runtime.read(cx).pending_focus(focus_claim_revision);
            let metrics = state.metrics();
            let static_pending_focus = if virtualized {
                None
            } else {
                pending_focus.as_ref().and_then(|operation| {
                    (operation.stage == TreeFocusOperationStage::Materializing)
                        .then(|| {
                            state
                                .item_by_value(&operation.value)
                                .filter(|item| item.focusable())
                                .and_then(|item| {
                                    focus_handles.get(item.index()).cloned().flatten().map(
                                        |handle| {
                                            (
                                                operation.sequence,
                                                handle,
                                                operation.materialization_fence(),
                                            )
                                        },
                                    )
                                })
                        })
                        .flatten()
                })
            };
            let content = if virtualized {
                let viewport_extent = vertical_viewport_extent(&scroll_handle);
                let scroll_offset = vertical_scroll_offset(&scroll_handle);
                let plan = TreeRenderPlan::resolve(
                    debug_id.clone(),
                    label.to_string(),
                    state.clone(),
                    scroll_offset,
                    viewport_extent,
                    viewport_item_count,
                    overscan_count,
                );

                render_virtual_tree_body(
                    debug_id.clone(),
                    plan,
                    focus_handles.clone(),
                    runtime.clone(),
                    scroll_handle.clone(),
                    scroll_chain_anchor.expect("virtual Tree should create a scroll-chain anchor"),
                    pending_focus,
                    draggable,
                    on_select.clone(),
                    on_toggle.clone(),
                    on_move.clone(),
                )
            } else {
                render_full_tree_body(
                    debug_id.clone(),
                    state.items().to_vec(),
                    focus_handles.clone(),
                    metrics,
                    runtime.clone(),
                    state.clone(),
                    draggable,
                    on_select.clone(),
                    on_toggle.clone(),
                    on_move.clone(),
                )
            };
            if let Some((sequence, focus_handle, scroll_fence)) = static_pending_focus {
                submit_tree_focus_claim(
                    runtime.clone(),
                    sequence,
                    focus_handle,
                    scroll_fence,
                    window,
                    cx,
                );
            }
            let root_label = label.to_string();
            let root_semantics = SemanticDescriptor::new(state.role()).with_label(&root_label);

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("tree:{debug_id}:root")
                })
                .size_full()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(6.0))
                .border_1()
                .border_color(rgb(0xd6d8ce))
                .bg(rgb(0xffffff))
                .text_size(gpui_px_from_ui(metrics.text_size()))
                .text_color(rgb(0x2f3845))
                .ui_semantics(&root_semantics)
                .on_click(move |_, window, cx| {
                    if let Some(focus_handle) = root_focus_handle.as_ref() {
                        focus_handle.focus(window, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                })
                .child(
                    div().flex_1().min_h(px(0.0)).child(
                        ScrollArea::new(format!("tree:{id}:scroll"), content)
                            .vertical()
                            .with_size(size)
                            .scroll_handle(&scroll_handle),
                    ),
                )
        })
    }
}

fn render_full_tree_body(
    tree_id: String,
    rows: Vec<TreeItemState>,
    focus_handles: Vec<Option<FocusHandle>>,
    metrics: TreeMetrics,
    runtime: Entity<TreeRuntime>,
    state: TreeState,
    draggable: bool,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
    on_move: Option<TreeMoveHandler>,
) -> AnyElement {
    div()
        .debug_selector({
            let tree_id = tree_id.clone();
            move || format!("tree:{tree_id}:content")
        })
        .flex()
        .flex_col()
        .gap_1()
        .p(gpui_px_from_ui(ui_px(6.0)))
        .children(rows.into_iter().enumerate().map(move |(index, item)| {
            render_tree_item(
                tree_id.clone(),
                item,
                focus_handles.get(index).cloned().flatten(),
                metrics,
                runtime.clone(),
                state.clone(),
                draggable,
                on_select.clone(),
                on_toggle.clone(),
                on_move.clone(),
                None,
            )
        }))
        .into_any_element()
}

fn render_virtual_tree_body(
    tree_id: String,
    plan: TreeRenderPlan,
    focus_handles: Vec<Option<FocusHandle>>,
    runtime: Entity<TreeRuntime>,
    scroll_handle: ScrollHandle,
    scroll_chain_anchor: open_gpui::RevealTargetHandle,
    pending_focus: Option<TreeFocusOperation>,
    draggable: bool,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
    on_move: Option<TreeMoveHandler>,
) -> AnyElement {
    let metrics = plan.metrics();
    let state = plan.state().clone();
    let rows = plan.rows().to_vec();
    let total_size = plan.virtualizer().total_size();
    let pending_focus_target = pending_focus.and_then(|operation| {
        state.item_by_value(&operation.value).and_then(|item| {
            item.focusable().then(|| {
                (
                    operation.sequence,
                    operation.stage,
                    item.index(),
                    item.render_identity().to_owned(),
                )
            })
        })
    });
    let target_is_rendered =
        pending_focus_target
            .as_ref()
            .is_some_and(|(_, _, target_index, target_identity)| {
                rows.iter().any(|row| {
                    row.index() == *target_index && row.item().render_identity() == target_identity
                })
            });

    let mut body = div();
    if let Some((sequence, stage, target_index, _)) = pending_focus_target {
        match stage {
            TreeFocusOperationStage::Materializing => {
                let runtime_for_prepaint = runtime.clone();
                let runtime_for_commit = runtime.clone();
                let state_for_materialization = state.clone();
                let scroll_handle = scroll_handle.clone();
                body = body.on_children_prepainted(move |_, window, cx| {
                    let prepared = runtime_for_prepaint.update(cx, |runtime, cx| {
                        runtime.prepare_virtual_materialization(
                            sequence,
                            window.focus_claim_revision(),
                            window,
                            cx,
                        )
                    });
                    if !prepared {
                        return;
                    }

                    let runtime_for_commit = runtime_for_commit.clone();
                    let state_for_materialization = state_for_materialization.clone();
                    let scroll_handle = scroll_handle.clone();
                    let scroll_chain_anchor = scroll_chain_anchor;
                    window.record_prepaint_focus_stable_commit(move |_, window, cx| {
                        let accepted = runtime_for_commit.update(cx, |runtime, cx| {
                            runtime.commit_virtual_materialization(
                                sequence,
                                window.focus_claim_revision(),
                                window,
                                cx,
                            )
                        });
                        if !accepted {
                            return;
                        }

                        materialize_tree_item(
                            &scroll_handle,
                            &state_for_materialization,
                            target_index,
                        );
                        let refreshed_fence = window
                            .capture_committed_scroll_chain_fence(
                                &scroll_chain_anchor,
                                BringIntoViewOptions::nearest(),
                            )
                            .ok()
                            .flatten();
                        let refreshed = refreshed_fence.is_some_and(|fence| {
                            runtime_for_commit.update(cx, |runtime, _| {
                                runtime.refresh_virtual_materialization_fence(sequence, fence)
                            })
                        });
                        if !refreshed {
                            runtime_for_commit.update(cx, |runtime, cx| {
                                runtime.abandon_virtual_focus(sequence, cx);
                            });
                        }
                        window.refresh();
                    });
                });
            }
            TreeFocusOperationStage::AwaitingMount if target_is_rendered => {
                let runtime_for_prepaint = runtime.clone();
                let runtime_for_commit = runtime.clone();
                body = body.on_children_prepainted(move |_, window, cx| {
                    let prepared = runtime_for_prepaint.update(cx, |runtime, cx| {
                        runtime.prepare_virtual_focus_submission(
                            sequence,
                            window.focus_claim_revision(),
                            window,
                            cx,
                        )
                    });
                    if !prepared {
                        return;
                    }

                    let runtime_for_commit = runtime_for_commit.clone();
                    window.record_prepaint_window_commit(move |_, window, cx| {
                        let focus_handle = runtime_for_commit.update(cx, |runtime, cx| {
                            runtime.take_virtual_focus_submission(
                                sequence,
                                window.focus_claim_revision(),
                                window,
                                cx,
                            )
                        });
                        if let Some((focus_handle, scroll_fence)) = focus_handle {
                            submit_tree_focus_claim(
                                runtime_for_commit.clone(),
                                sequence,
                                focus_handle,
                                Some(scroll_fence),
                                window,
                                cx,
                            );
                        }
                    });
                });
            }
            TreeFocusOperationStage::AwaitingMount | TreeFocusOperationStage::InFlight => {}
        }
    }

    body.debug_selector({
        let tree_id = tree_id.clone();
        move || format!("tree:{tree_id}:content")
    })
    .relative()
    .w_full()
    .h(gpui_px_from_ui(total_size))
    .children(rows.into_iter().map(move |row| {
        let index = row.index();
        render_tree_item(
            tree_id.clone(),
            row.item().clone(),
            focus_handles.get(index).cloned().flatten(),
            metrics,
            runtime.clone(),
            state.clone(),
            draggable,
            on_select.clone(),
            on_toggle.clone(),
            on_move.clone(),
            Some((row.virtual_start(), row.virtual_size())),
        )
    }))
    .track_reveal_target(&scroll_chain_anchor)
    .into_any_element()
}

fn render_tree_item(
    tree_id: String,
    item: TreeItemState,
    focus_handle: Option<FocusHandle>,
    metrics: TreeMetrics,
    runtime: Entity<TreeRuntime>,
    state: TreeState,
    draggable: bool,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
    on_move: Option<TreeMoveHandler>,
    virtual_geometry: Option<(UiPx, UiPx)>,
) -> impl IntoElement {
    let item_value = item.value().to_owned();
    let item_identity = item.render_identity().to_owned();
    let item_label = item.label().to_owned();
    let disabled = item.disabled();
    let selected = item.selected();
    let focused = item.focused();
    let has_children = item.has_children();
    let children_load_state = item.children_load_state().clone();
    let expanded = item.expanded();
    let selection = TreeSelection::from_item(&item);
    let toggle = TreeToggle::from_item(&item);
    let row_background = if selected {
        rgb(0xe8f3ef)
    } else if focused {
        rgb(0xeef2f7)
    } else {
        rgb(0xffffff)
    };
    let text_color = if disabled {
        rgb(0x7a8492)
    } else {
        rgb(0x2f3845)
    };
    let indent = metrics.indent_width() * item.depth() as f32;
    let item_position = item.position_in_set();
    let item_size_of_set = item.size_of_set();
    let virtual_start = virtual_geometry.map(|(start, _)| start);
    let virtual_size = virtual_geometry.map(|(_, size)| size);
    let move_enabled = draggable && on_move.is_some() && !disabled;
    let mut semantics = SemanticDescriptor::new(item.role())
        .with_label(&item_label)
        .with_selected(selected)
        .with_disabled(disabled)
        .with_busy(children_load_state.is_loading())
        .with_level(item.depth() + 1)
        .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);
    if has_children {
        semantics = semantics.with_expanded(expanded);
    }
    if let Some(position) = item_position {
        semantics = semantics
            .with_position_in_set(position)
            .with_size_of_set(item_size_of_set);
    }

    div()
        .id(format!("tree:{tree_id}:item:{item_identity}"))
        .debug_selector({
            let tree_id = tree_id.clone();
            let item_identity = item_identity.clone();
            move || format!("tree:{tree_id}:item:{item_identity}")
        })
        .when_some(virtual_start, |this, start| {
            this.absolute()
                .top(gpui_px_from_ui(start))
                .left(px(0.0))
                .right(px(0.0))
        })
        .when_some(virtual_size, |this, size| this.h(gpui_px_from_ui(size)))
        .when(virtual_size.is_none(), |this| {
            this.min_h(gpui_px_from_ui(metrics.row_height()))
        })
        .w_full()
        .px(gpui_px_from_ui(metrics.row_padding_x()))
        .py(gpui_px_from_ui(metrics.row_padding_y()))
        .flex()
        .items_center()
        .gap_2()
        .rounded_sm()
        .bg(row_background)
        .text_color(text_color)
        .overflow_hidden()
        .relative()
        .ui_semantics(&semantics)
        .focusable()
        .tab_stop(focused)
        .when_some(focus_handle.clone(), |this, focus_handle| {
            this.track_focus(&focus_handle)
        })
        .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
        .when(!disabled, |this| {
            this.cursor_pointer().hover(|style| style.bg(rgb(0xf1f5ee)))
        })
        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
        .when(!disabled, |this| {
            let runtime = runtime.clone();
            let item_value = item_value.clone();
            this.on_ui_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                if let Some(focus_handle) =
                    runtime.update(cx, |runtime, cx| runtime.set_focused(&item_value, cx))
                {
                    focus_handle.focus(window, cx);
                }
            })
        })
        .when(move_enabled, |this| {
            let tree_id = tree_id.clone();
            let item_value = item_value.clone();
            this.cursor(CursorStyle::OpenHand).on_drag(
                TreeDragPayload {
                    tree_id,
                    source_value: item_value,
                },
                |_, _, _, cx| cx.new(|_| Empty),
            )
        })
        .when(!disabled, |this| {
            let runtime = runtime.clone();
            let on_select = on_select.clone();
            let selection = selection.clone();
            let focus_handle = focus_handle.clone();
            let item_value = item_value.clone();
            this.on_click(move |_event, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(&item_value, cx);
                    runtime.set_selected(&item_value, cx);
                });
                if let Some(focus_handle) = focus_handle.as_ref() {
                    focus_handle.focus(window, cx);
                }
                if let Some(selection) = selection.clone() {
                    if let Some(on_select) = on_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                }
            })
        })
        .on_key_down({
            let runtime = runtime.clone();
            let on_select = on_select.clone();
            let on_toggle = on_toggle.clone();
            let state = state.clone();
            move |event: &KeyDownEvent, window, cx| {
                handle_tree_key_down(
                    &state,
                    runtime.clone(),
                    on_select.clone(),
                    on_toggle.clone(),
                    event,
                    window,
                    cx,
                );
            }
        })
        .child(div().w(gpui_px_from_ui(indent)).flex_none())
        .child(tree_disclosure(
            tree_id.clone(),
            item_identity.clone(),
            item_label.clone(),
            has_children,
            children_load_state.clone(),
            expanded,
            disabled,
            toggle,
            runtime,
            focus_handle,
            on_toggle,
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(item_label.clone()),
        )
        .when_some(tree_children_load_hint(&children_load_state), {
            let tree_id = tree_id.clone();
            let item_identity = item_identity.clone();
            let children_load_state = children_load_state.clone();
            move |this, hint| {
                let role = if children_load_state.is_failed() {
                    Role::Alert
                } else {
                    Role::Status
                };
                let semantics = SemanticDescriptor::new(role).with_live_text(&hint);
                this.child(
                    div()
                        .id(format!("tree:{tree_id}:load-state:{item_identity}"))
                        .debug_selector({
                            let tree_id = tree_id.clone();
                            let item_identity = item_identity.clone();
                            move || format!("tree:{tree_id}:load-state:{item_identity}")
                        })
                        .flex_none()
                        .text_xs()
                        .text_color(rgb(0x5a6472))
                        .ui_semantics(&semantics)
                        .child(hint.clone()),
                )
            }
        })
        .when(move_enabled, |this| {
            this.child(tree_drop_zone(
                tree_id.clone(),
                item_identity.clone(),
                item_value.clone(),
                state.clone(),
                metrics,
                TreeDropPosition::Before,
                on_move.clone(),
            ))
            .child(tree_drop_zone(
                tree_id.clone(),
                item_identity.clone(),
                item_value.clone(),
                state.clone(),
                metrics,
                TreeDropPosition::Inside,
                on_move.clone(),
            ))
            .child(tree_drop_zone(
                tree_id.clone(),
                item_identity.clone(),
                item_value.clone(),
                state.clone(),
                metrics,
                TreeDropPosition::After,
                on_move.clone(),
            ))
        })
}

fn tree_drop_zone(
    tree_id: String,
    target_identity: String,
    target_value: String,
    state: TreeState,
    metrics: TreeMetrics,
    position: TreeDropPosition,
    on_move: Option<TreeMoveHandler>,
) -> AnyElement {
    let Some(on_move) = on_move else {
        return div().into_any_element();
    };
    let position_key = position.as_str().to_owned();
    let zone_extent = gpui_px_from_ui(metrics.row_height() * (1.0 / 3.0));
    let state_for_can_drop = state.clone();
    let target_for_can_drop = target_value.clone();
    let tree_for_can_drop = tree_id.clone();
    let state_for_drag_over = state.clone();
    let target_for_drag_over = target_value.clone();
    let tree_for_drag_over = tree_id.clone();
    let state_for_drop = state;
    let target_for_drop = target_value.clone();
    let tree_for_drop = tree_id.clone();

    div()
        .debug_selector({
            let tree_id = tree_id.clone();
            let target_identity = target_identity.clone();
            move || format!("tree:{tree_id}:drop:{position_key}:{target_identity}")
        })
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .h(zone_extent)
        .when(position == TreeDropPosition::Before, |this| {
            this.top(px(0.0))
        })
        .when(position == TreeDropPosition::Inside, |this| {
            this.top(zone_extent)
        })
        .when(position == TreeDropPosition::After, |this| {
            this.bottom(px(0.0))
        })
        .can_drop(move |dragged, _, _| {
            dragged
                .downcast_ref::<TreeDragPayload>()
                .filter(|drag| drag.tree_id == tree_for_can_drop)
                .and_then(|drag| {
                    state_for_can_drop.move_for_drop(
                        &drag.source_value,
                        &target_for_can_drop,
                        position,
                    )
                })
                .is_some()
        })
        .drag_over::<TreeDragPayload>(move |style, drag, _, _| {
            if drag.tree_id != tree_for_drag_over
                || state_for_drag_over
                    .move_for_drop(&drag.source_value, &target_for_drag_over, position)
                    .is_none()
            {
                return style;
            }

            match position {
                TreeDropPosition::Before | TreeDropPosition::After => style.bg(rgba(0x1f7a662e)),
                TreeDropPosition::Inside => style
                    .border_1()
                    .border_color(rgb(0x1f7a66))
                    .bg(rgb(0xe8f3ef)),
            }
        })
        .on_drop(move |event, window, cx| {
            let drag: &TreeDragPayload = event.value();
            if drag.tree_id != tree_for_drop {
                return;
            }
            if let Some(tree_move) =
                state_for_drop.move_for_drop(&drag.source_value, &target_for_drop, position)
            {
                on_move(tree_move, window, cx);
            }
        })
        .into_any_element()
}

fn tree_disclosure(
    tree_id: String,
    item_identity: String,
    item_label: String,
    has_children: bool,
    children_load_state: TreeChildrenLoadState,
    expanded: bool,
    disabled: bool,
    toggle: Option<TreeToggle>,
    runtime: Entity<TreeRuntime>,
    focus_handle: Option<FocusHandle>,
    on_toggle: Option<TreeToggleHandler>,
) -> impl IntoElement {
    let children_loading = children_load_state.is_loading();
    let glyph = if !has_children {
        ""
    } else if children_loading {
        "..."
    } else if expanded {
        "v"
    } else {
        ">"
    };
    let aria_label = if children_loading {
        format!("Loading {item_label}")
    } else if expanded {
        format!("Collapse {item_label}")
    } else {
        format!("Expand {item_label}")
    };
    let semantics = SemanticDescriptor::new(Role::Button)
        .with_label(&aria_label)
        .with_expanded(expanded)
        .with_disabled(disabled || !has_children || children_loading)
        .with_actions(&[AccessibleAction::Click]);
    div()
        .id(format!("tree:{tree_id}:toggle:{item_identity}"))
        .debug_selector({
            let tree_id = tree_id.clone();
            let item_identity = item_identity.clone();
            move || format!("tree:{tree_id}:toggle:{item_identity}")
        })
        .w(px(18.0))
        .h(px(18.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_xs()
        .ui_semantics(&semantics)
        .when(has_children && !disabled && !children_loading, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(rgb(0xe8ede6)))
                .on_click(move |_event, window, cx| {
                    cx.stop_propagation();
                    window.prevent_default();
                    let Some(toggle) = toggle.clone() else {
                        return;
                    };
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_focused(toggle.value(), cx);
                        runtime.set_expanded(toggle.value(), toggle.expanded(), cx);
                    });
                    if let Some(focus_handle) = focus_handle.as_ref() {
                        focus_handle.focus(window, cx);
                    }
                    if let Some(on_toggle) = on_toggle.as_ref() {
                        on_toggle(toggle, window, cx);
                    }
                })
        })
        .child(glyph)
}

fn handle_tree_key_down(
    state: &TreeState,
    runtime: Entity<TreeRuntime>,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if window.default_prevented() {
        return;
    }

    let key = event.keystroke.key.as_str();
    let current_value = runtime.read(cx).focused_value.clone();
    let command_input = !event.keystroke.modifiers.modified() && !event.prefer_character_input;

    if command_input {
        if let Some(action) =
            state.keyboard_action_for_key_from_value(key, current_value.as_deref())
        {
            cx.stop_propagation();
            window.prevent_default();

            match action {
                TreeKeyboardAction::Focus(target) => {
                    focus_tree_target(&runtime, &target, window, cx);
                }
                TreeKeyboardAction::Toggle(toggle) => {
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_focused(toggle.value(), cx);
                        runtime.set_expanded(toggle.value(), toggle.expanded(), cx);
                    });
                    if let Some(on_toggle) = on_toggle.as_ref() {
                        on_toggle(toggle, window, cx);
                    }
                }
                TreeKeyboardAction::Select(selection) => {
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_selected(selection.value(), cx);
                    });
                    queue_tree_focus(&runtime, selection.value(), window, cx);
                    if let Some(on_select) = on_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                }
            }
            return;
        }
    }

    let now = cx.background_executor().now();
    let update = runtime.update(cx, |runtime, _| {
        let update = runtime
            .typeahead
            .push(CollectionTypeaheadInput::from_key_down(event), now)?;
        Some(update)
    });
    let Some(update) = update else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();

    if let Some(target) = state.typeahead_target_from_value(
        update.match_query(),
        current_value.as_deref(),
        update.searches_after_current(),
    ) {
        let target = TreeFocusTarget::new(target.index(), target.value());
        focus_tree_target(&runtime, &target, window, cx);
    }
}

fn focus_tree_target(
    runtime: &Entity<TreeRuntime>,
    target: &TreeFocusTarget,
    window: &mut Window,
    cx: &mut App,
) {
    queue_tree_focus(runtime, target.value(), window, cx);
}

fn queue_tree_focus(runtime: &Entity<TreeRuntime>, value: &str, window: &mut Window, cx: &mut App) {
    let focus_revision = window.focus_claim_revision();
    let materialization_fence = runtime.update(cx, |runtime, _| {
        if !runtime.is_virtualized() {
            return None;
        }
        let anchor = runtime.scroll_chain_anchor(window);
        window
            .capture_committed_scroll_chain_fence(&anchor, BringIntoViewOptions::nearest())
            .ok()
            .flatten()
    });
    let pending_focus = runtime.update(cx, |runtime, cx| {
        runtime.queue_focus(value, focus_revision, materialization_fence, cx)
    });
    if let Some((sequence, focus_handle)) = pending_focus {
        submit_tree_focus_claim(runtime.clone(), sequence, focus_handle, None, window, cx);
    }
}

fn submit_tree_focus_claim(
    runtime: Entity<TreeRuntime>,
    sequence: u64,
    focus_handle: FocusHandle,
    scroll_fence: Option<ScrollChainFence>,
    window: &mut Window,
    cx: &mut App,
) {
    let runtime_for_completion = runtime.clone();
    let completion =
        move |outcome: open_gpui::FocusClaimOutcome, window: &mut Window, cx: &mut App| {
            let retry = runtime_for_completion.update(cx, |runtime, cx| {
                runtime.finish_focus(sequence, outcome, window.focus_claim_revision(), cx)
            });
            if let Some((retry_sequence, retry_handle, retry_fence)) = retry {
                submit_tree_focus_claim(
                    runtime_for_completion.clone(),
                    retry_sequence,
                    retry_handle,
                    retry_fence,
                    window,
                    cx,
                );
            }
        };
    let subscription = if let Some(scroll_fence) = scroll_fence {
        window.focus_with_completion_and_scroll_fence(&focus_handle, scroll_fence, cx, completion)
    } else {
        window.focus_with_completion(&focus_handle, cx, completion)
    };
    subscription.detach();
    let claim_revision = window.focus_claim_revision();
    runtime.update(cx, |runtime, _| {
        runtime.bind_focus_claim(sequence, claim_revision);
    });
}

fn materialize_tree_item(scroll_handle: &ScrollHandle, state: &TreeState, index: usize) {
    let row_height = nonnegative_px(state.metrics().row_height());
    materialize_fixed_row(
        scroll_handle,
        index,
        state.items().len(),
        row_height,
        Some(row_height * DEFAULT_TREE_VIEWPORT_ITEM_COUNT as f32),
    );
}
