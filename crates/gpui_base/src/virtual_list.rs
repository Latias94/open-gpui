use std::{hash::Hash, ops::Range, rc::Rc};

use open_gpui::{
    AccessibleAction, AnyElement, App, ClickEvent, Context, Div, EntityId, EventEmitter,
    FocusHandle, Focusable, Global, KeyBinding, ListSizingBehavior, Pixels, Role, ScrollStrategy,
    SharedString, Stateful, StyleRefinement, UniformListScrollHandle, Window, div, prelude::*, px,
    uniform_list,
};

mod model;

use model::TreeModel;
pub use model::{DropPlacement, Reorder, TreeError, TreeRow};

/// The view state supplied to a visible row's renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowState {
    pub depth: usize,
    pub selected: bool,
    pub focused: bool,
    pub expanded: bool,
    pub has_children: bool,
}

/// User intent from a [`VirtualList`]. Selection and expansion update immediately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtualListEvent<K> {
    SelectionChanged(Vec<K>),
    Expanded { key: K, expanded: bool },
    Activate(K),
    ReorderRequested(Reorder<K>),
}

type RowRenderer<K> = Rc<dyn Fn(&TreeRow<K>, RowState, &mut Window, &mut App) -> AnyElement>;

/// A fixed-height virtual list. Optional parent keys add expandable tree rows.
/// Place it inside a container with a bounded height.
pub struct VirtualList<K> {
    model: TreeModel<K>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    row_height: Pixels,
    indent: Pixels,
    multiple: bool,
    tree: bool,
    reorder: bool,
    renderer: RowRenderer<K>,
    style: StyleRefinement,
}

/// The tree form of [`VirtualList`]. Use [`VirtualList::tree`] to enable tree drop
/// zones and roles before the first child exists.
pub type VirtualTree<K> = VirtualList<K>;

impl<K: Clone + Eq + Hash + 'static> VirtualList<K> {
    pub fn new(row_height: Pixels, cx: &mut Context<Self>) -> Self {
        install_keys(cx);
        Self {
            model: TreeModel::new(),
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::default(),
            row_height: if row_height > px(0.) && row_height.to_f64().is_finite() {
                row_height
            } else {
                px(24.)
            },
            indent: px(12.),
            multiple: true,
            tree: false,
            reorder: false,
            renderer: Rc::new(|row, _, _, _| row.label().clone().into_any_element()),
            style: StyleRefinement::default(),
        }
    }

    pub fn tree(row_height: Pixels, cx: &mut Context<Self>) -> Self {
        Self {
            tree: true,
            ..Self::new(row_height, cx)
        }
    }

    pub fn set_rows(
        &mut self,
        rows: impl IntoIterator<Item = TreeRow<K>>,
        cx: &mut Context<Self>,
    ) -> Result<(), TreeError> {
        self.model.replace(rows)?;
        cx.notify();
        Ok(())
    }

    /// Changes expansion programmatically without emitting a user intent.
    pub fn set_expanded(&mut self, key: &K, expanded: bool, cx: &mut Context<Self>) -> bool {
        let changed = self.model.set_expanded(key, expanded);
        if changed {
            cx.notify();
        }
        changed
    }

    pub fn selection(&self) -> Vec<K> {
        self.model.selection()
    }

    pub fn set_selection(&mut self, keys: impl IntoIterator<Item = K>, cx: &mut Context<Self>) {
        let keys = keys
            .into_iter()
            .filter(|key| self.model.indices.contains_key(key));
        self.model.selected = keys
            .take(if self.multiple { usize::MAX } else { 1 })
            .collect();
        cx.notify();
    }

    pub fn set_multiselect(&mut self, multiple: bool, cx: &mut Context<Self>) {
        self.multiple = multiple;
        if !multiple && self.model.selected.len() > 1 {
            self.model.selected = self.model.selection().into_iter().take(1).collect();
        }
        cx.notify();
    }

    pub fn set_reorder_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.reorder = enabled;
        cx.notify();
    }

    /// Applies a validated reorder. Applications with their own model can update it instead.
    pub fn apply_reorder(&mut self, request: &Reorder<K>, cx: &mut Context<Self>) -> bool {
        let changed = self.model.apply_reorder(request);
        if changed {
            cx.notify();
        }
        changed
    }

    pub fn scroll_to(&self, key: &K, cx: &mut Context<Self>) -> bool {
        let Some(position) = self.model.position(key) else {
            return false;
        };
        self.scroll
            .scroll_to_item(position, ScrollStrategy::Nearest);
        cx.notify();
        true
    }

    pub fn set_row_renderer<E: IntoElement>(
        &mut self,
        renderer: impl Fn(&TreeRow<K>, RowState, &mut Window, &mut App) -> E + 'static,
        cx: &mut Context<Self>,
    ) {
        self.renderer = Rc::new(move |row, state, window, cx| {
            renderer(row, state, window, cx).into_any_element()
        });
        cx.notify();
    }

    pub fn set_indent(&mut self, indent: Pixels, cx: &mut Context<Self>) {
        if indent >= px(0.) && indent.to_f64().is_finite() {
            self.indent = indent;
            cx.notify();
        }
    }

    pub fn set_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    fn is_tree(&self) -> bool {
        self.tree || self.model.has_hierarchy()
    }

    fn select(&mut self, key: &K, extend: bool, toggle: bool, cx: &mut Context<Self>) {
        if self
            .model
            .select(key, self.multiple && extend, self.multiple && toggle)
        {
            cx.emit(VirtualListEvent::SelectionChanged(self.model.selection()));
        }
        cx.notify();
    }

    fn expand(&mut self, key: &K, expanded: bool, cx: &mut Context<Self>) {
        if self.set_expanded(key, expanded, cx) {
            cx.emit(VirtualListEvent::Expanded {
                key: key.clone(),
                expanded,
            });
        }
    }

    fn command(&mut self, command: &ListCommand, _: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .model
            .active
            .as_ref()
            .and_then(|key| self.model.position(key));
        let last = self.model.visible.len().saturating_sub(1);
        let next = match *command {
            ListCommand::Step { forward, extend } => {
                let index = current.map_or(0, |index| {
                    if forward {
                        (index + 1).min(last)
                    } else {
                        index.saturating_sub(1)
                    }
                });
                Some((index, extend))
            }
            ListCommand::Edge { end, extend } => Some((if end { last } else { 0 }, extend)),
            ListCommand::Activate
            | ListCommand::Toggle
            | ListCommand::Collapse
            | ListCommand::Expand => {
                let current = current.unwrap_or(0);
                let Some(key) = self.model.key_at(current).cloned() else {
                    return;
                };
                let index = self.model.indices[&key];
                let expanded = self.model.expanded.contains(&key);
                match *command {
                    ListCommand::Activate => cx.emit(VirtualListEvent::Activate(key)),
                    ListCommand::Toggle => self.select(&key, false, true, cx),
                    ListCommand::Collapse if expanded => self.expand(&key, false, cx),
                    ListCommand::Collapse => {
                        if let Some(parent) = self.model.nodes[index].row.parent().cloned() {
                            self.select(&parent, false, false, cx);
                            self.scroll_to(&parent, cx);
                        }
                    }
                    ListCommand::Expand
                        if expanded && !self.model.nodes[index].children.is_empty() =>
                    {
                        if let Some(child) = self.model.key_at(current + 1).cloned() {
                            self.select(&child, false, false, cx);
                            self.scroll_to(&child, cx);
                        }
                    }
                    ListCommand::Expand => self.expand(&key, true, cx),
                    _ => {}
                }
                None
            }
        };
        if let Some((position, extend)) = next
            && let Some(key) = self.model.key_at(position).cloned()
        {
            self.select(&key, extend, false, cx);
            self.scroll_to(&key, cx);
        }
    }

    fn rows_for(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let tree = self.is_tree();
        range.filter_map(|position| {
            let (index, depth) = *self.model.visible.get(position)?;
            let node = &self.model.nodes[index];
            let key = node.row.key();
            let state = RowState {
                depth,
                selected: self.model.selected.contains(key),
                focused: self.model.active.as_ref() == Some(key) && self.focus.is_focused(window),
                expanded: self.model.expanded.contains(key),
                has_children: !node.children.is_empty(),
            };
            let row = div().id(node.id)
                .role(if tree { Role::TreeItem } else { Role::ListBoxOption })
                .aria_label(node.row.label().clone())
                .aria_selected(state.selected)
                .flex().items_center().h(self.row_height).w_full();
            let mut row = if state.focused { row.aria_active_descendant() } else { row };
            if tree {
                row = row.aria_level(depth + 1).pl(self.indent * depth as f32);
            }
            if state.has_children {
                let target = key.clone();
                let toggle = cx.listener(move |list, _: &ClickEvent, window, cx| {
                    window.focus(&list.focus, cx);
                    list.expand(&target, !state.expanded, cx);
                });
                let target = key.clone();
                let view = cx.weak_entity();
                let action = if state.expanded { AccessibleAction::Collapse } else { AccessibleAction::Expand };
                row = row.aria_expanded(state.expanded)
                    .on_a11y_action(action, move |_, _, cx| {
                        let _ = view.update(cx, |list, cx| list.expand(&target, !state.expanded, cx));
                    })
                    .child(crate::IconButton::new("disclosure", if state.expanded { "Collapse" } else { "Expand" }, if state.expanded { "▾" } else { "▸" }).on_click(toggle));
            } else if tree {
                row = row.child(div().w(self.indent).flex_none());
            }

            let target = key.clone();
            let accessible_target = key.clone();
            let view = cx.weak_entity();
            row = row.on_click(cx.listener(move |list, event: &open_gpui::TargetedEvent<ClickEvent>, window, cx| {
                window.focus(&list.focus, cx);
                let click = event.window_event();
                let modifiers = click.modifiers();
                list.select(&target, modifiers.shift, modifiers.secondary(cx), cx);
                if matches!(click, ClickEvent::Mouse(event) if event.up.click_count == 2)
                    || matches!(click, ClickEvent::Touch(event) if event.tap_count == 2)
                {
                    cx.emit(VirtualListEvent::Activate(target.clone()));
                }
            })).on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                let _ = view.update(cx, |list, cx| {
                    window.focus(&list.focus, cx);
                    list.select(&accessible_target, false, false, cx);
                });
            });

            if self.reorder {
                let source = key.clone();
                let target = key.clone();
                let list_id = cx.entity_id();
                let view = cx.weak_entity();
                let payload = DragRows { list_id, source, label: node.row.label().clone() };
                row = row.on_drag(payload, |payload, _, _, cx| {
                    cx.new(|_| DragLabel(payload.label.clone()))
                }).on_drop::<DragRows<K>>(move |event, _, cx| {
                    let drag = event.value();
                    if drag.list_id != list_id { return; }
                    let pointer = event.pointer();
                    let Ok(position) = pointer.target_local_position() else { return };
                    let placement = drop_placement(position.y, pointer.target_local_bounds().size.height, tree);
                    let _ = view.update(cx, |list, cx| {
                        let moved = if list.model.selected.contains(&drag.source) { list.selection() } else { vec![drag.source.clone()] };
                        if let Some(plan) = list.model.plan_reorder(&moved, &target, placement) {
                            cx.emit(VirtualListEvent::ReorderRequested(plan));
                        }
                    });
                });
            }
            Some(row.child((self.renderer)(&node.row, state, window, cx)))
        }).collect()
    }
}

fn drop_placement(y: Pixels, height: Pixels, tree: bool) -> DropPlacement {
    if tree && y >= height / 3. && y < height * (2. / 3.) {
        DropPlacement::Into
    } else if y < height / 2. {
        DropPlacement::Before
    } else {
        DropPlacement::After
    }
}

struct DragRows<K> {
    list_id: EntityId,
    source: K,
    label: SharedString,
}

struct DragLabel(SharedString);

impl Render for DragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.0.clone())
    }
}

impl<K: 'static> Focusable for VirtualList<K> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl<K: 'static> EventEmitter<VirtualListEvent<K>> for VirtualList<K> {}

impl<K: Clone + Eq + Hash + 'static> Render for VirtualList<K> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().flex().flex_col().size_full().min_h_0();
        let style = root.style();
        open_gpui::Refineable::refine(style, &self.style);
        root.id("virtual-list")
            .role(if self.is_tree() {
                Role::Tree
            } else {
                Role::ListBox
            })
            .track_focus(&self.focus)
            .tab_stop(true)
            .key_context(LIST_CONTEXT)
            .on_action(cx.listener(Self::command))
            .child(
                uniform_list(
                    "rows",
                    self.model.visible.len(),
                    cx.processor(Self::rows_for),
                )
                .track_scroll(&self.scroll)
                .size_full()
                .min_h_0()
                .with_sizing_behavior(ListSizingBehavior::Auto),
            )
    }
}

const LIST_CONTEXT: &str = "OpenGpuiBaseVirtualList";

#[derive(Clone, Default, PartialEq, open_gpui::Action)]
#[action(namespace = open_gpui_base, no_json, no_register)]
enum ListCommand {
    Step {
        forward: bool,
        extend: bool,
    },
    Edge {
        end: bool,
        extend: bool,
    },
    Collapse,
    Expand,
    Toggle,
    #[default]
    Activate,
}

struct ListKeysInstalled;
impl Global for ListKeysInstalled {}

fn install_keys(cx: &mut App) {
    if cx.has_global::<ListKeysInstalled>() {
        return;
    }
    cx.set_global(ListKeysInstalled);
    use ListCommand::*;
    cx.bind_keys(
        [
            (
                "up",
                Step {
                    forward: false,
                    extend: false,
                },
            ),
            (
                "down",
                Step {
                    forward: true,
                    extend: false,
                },
            ),
            (
                "shift-up",
                Step {
                    forward: false,
                    extend: true,
                },
            ),
            (
                "shift-down",
                Step {
                    forward: true,
                    extend: true,
                },
            ),
            (
                "home",
                Edge {
                    end: false,
                    extend: false,
                },
            ),
            (
                "end",
                Edge {
                    end: true,
                    extend: false,
                },
            ),
            (
                "shift-home",
                Edge {
                    end: false,
                    extend: true,
                },
            ),
            (
                "shift-end",
                Edge {
                    end: true,
                    extend: true,
                },
            ),
            ("left", Collapse),
            ("right", Expand),
            ("enter", Activate),
            ("space", Toggle),
        ]
        .map(|(key, command)| KeyBinding::new(key, command, Some(LIST_CONTEXT))),
    );
}
