use std::{cell::Cell, rc::Rc};

use open_gpui::{
    AccessibleAction, Anchor, App, Bounds, ClickEvent, Context, EventEmitter, FocusHandle,
    Focusable, FontWeight, MouseButton, Pixels, Refineable, Role, ScrollHandle, SharedString,
    StyleRefinement, TargetedEvent, Window, anchored, canvas, div, prelude::*, window_portal,
};

use super::{PopupCommand, SELECT_CONTEXT, install_keys};

/// A labeled choice or menu command.
#[derive(Clone, Debug)]
pub struct MenuItem<K> {
    key: K,
    label: SharedString,
    disabled: bool,
}

impl<K> MenuItem<K> {
    pub fn new(key: K, label: impl Into<SharedString>) -> Self {
        Self {
            key,
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuEvent<K> {
    Selected(K),
    Dismissed,
}

/// A single-choice select with a window-bounded popup and keyboard navigation.
pub struct Select<K> {
    items: Vec<MenuItem<K>>,
    label: SharedString,
    selected: Option<K>,
    active: Option<usize>,
    focus: FocusHandle,
    popup_focus: FocusHandle,
    trigger_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    anchor: Option<Bounds<Pixels>>,
    scroll: ScrollHandle,
    is_menu: bool,
    trigger_style: StyleRefinement,
    popup_style: StyleRefinement,
}

/// The command-menu form of [`Select`]. Construct it with [`Select::menu`].
pub type Menu<K> = Select<K>;

impl<K: Clone + Eq + 'static> Select<K> {
    pub fn new(
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = MenuItem<K>>,
        cx: &mut Context<Self>,
    ) -> Self {
        install_keys(cx);
        Self {
            label: label.into(),
            items: items.into_iter().collect(),
            selected: None,
            active: None,
            focus: cx.focus_handle(),
            popup_focus: cx.focus_handle(),
            trigger_bounds: Rc::new(Cell::new(None)),
            anchor: None,
            scroll: ScrollHandle::new(),
            is_menu: false,
            trigger_style: StyleRefinement::default(),
            popup_style: StyleRefinement::default(),
        }
    }

    pub fn menu(
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = MenuItem<K>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            is_menu: true,
            ..Self::new(label, items, cx)
        }
    }

    pub fn selected(&self) -> Option<&K> {
        self.selected.as_ref()
    }

    pub fn set_selected(&mut self, selected: Option<K>, cx: &mut Context<Self>) -> bool {
        if self.is_menu
            || selected
                .as_ref()
                .is_some_and(|key| !self.items.iter().any(|item| &item.key == key))
        {
            return false;
        }
        self.selected = selected;
        cx.notify();
        true
    }

    pub fn set_items(
        &mut self,
        items: impl IntoIterator<Item = MenuItem<K>>,
        cx: &mut Context<Self>,
    ) {
        let active = self
            .active
            .and_then(|index| self.items.get(index))
            .map(|item| item.key.clone());
        self.items = items.into_iter().collect();
        self.selected = self
            .selected
            .take()
            .filter(|key| self.items.iter().any(|item| &item.key == key));
        self.active = active.and_then(|key| {
            self.items
                .iter()
                .position(|item| item.key == key && !item.disabled)
        });
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.anchor.is_some()
    }

    pub fn set_trigger_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.trigger_style = style;
        cx.notify();
    }

    pub fn set_popup_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.popup_style = style;
        cx.notify();
    }

    fn open(
        &mut self,
        anchor: Bounds<Pixels>,
        last: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.items.is_empty() {
            return;
        }
        self.anchor = Some(anchor);
        self.active = self
            .selected
            .as_ref()
            .and_then(|key| {
                self.items
                    .iter()
                    .position(|item| &item.key == key && !item.disabled)
            })
            .or_else(|| enabled_edge(&self.items, last));
        self.scroll_active(cx);
        window.focus(&self.popup_focus, cx);
    }

    fn close(&mut self, restore_focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.anchor = None;
        self.active = None;
        cx.notify();
        if restore_focus {
            window.focus(&self.focus, cx);
        }
    }

    fn dismiss(&mut self, restore_focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_open() {
            self.close(restore_focus, window, cx);
            cx.emit(MenuEvent::Dismissed);
        }
    }

    fn choose(&mut self, key: &K, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open()
            || !self
                .items
                .iter()
                .any(|item| &item.key == key && !item.disabled)
        {
            return;
        }
        if !self.is_menu {
            self.selected = Some(key.clone());
        }
        self.close(true, window, cx);
        cx.emit(MenuEvent::Selected(key.clone()));
    }

    fn scroll_active(&self, cx: &mut Context<Self>) {
        if let Some(index) = self.active {
            self.scroll.scroll_to_item(index);
        }
        cx.notify();
    }

    fn toggle(
        &mut self,
        anchor: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_open() {
            self.dismiss(true, window, cx);
        } else if let Some(anchor) = anchor {
            self.open(anchor, false, window, cx);
        }
    }

    fn command(&mut self, command: &PopupCommand, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() {
            match command {
                PopupCommand::Enter | PopupCommand::Step(_) => {
                    if let Some(anchor) = self.trigger_bounds.get() {
                        self.open(
                            anchor,
                            matches!(command, PopupCommand::Step(false)),
                            window,
                            cx,
                        );
                    }
                }
                _ => cx.propagate(),
            }
            return;
        }
        match *command {
            PopupCommand::Step(forward) => {
                self.active = next_enabled(&self.items, self.active, forward);
                self.scroll_active(cx);
            }
            PopupCommand::Edge(last) => {
                self.active = enabled_edge(&self.items, last);
                self.scroll_active(cx);
            }
            PopupCommand::Enter => {
                if let Some(item) = self.active.and_then(|index| self.items.get(index)) {
                    self.choose(&item.key.clone(), window, cx);
                }
            }
            PopupCommand::Escape => self.dismiss(true, window, cx),
            PopupCommand::Tab { backward } => {
                self.dismiss(true, window, cx);
                if backward {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            }
        }
    }
}

impl<K: 'static> EventEmitter<MenuEvent<K>> for Select<K> {}

impl<K: 'static> Focusable for Select<K> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl<K: Clone + Eq + 'static> Render for Select<K> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self
            .selected
            .as_ref()
            .and_then(|key| self.items.iter().find(|item| &item.key == key))
            .map_or_else(|| self.label.clone(), |item| item.label.clone());
        let mut trigger = div().relative();
        trigger.style().refine(&self.trigger_style);
        let bounds = self.trigger_bounds.clone();
        let view = cx.weak_entity();
        let trigger = trigger
            .id("trigger")
            .role(if self.is_menu {
                Role::Button
            } else {
                Role::ComboBox
            })
            .aria_label(self.label.clone())
            .aria_expanded(self.is_open())
            .aria_value(label.clone())
            .aria_disabled(self.items.is_empty())
            .track_focus(&self.focus)
            .tab_stop(!self.items.is_empty())
            .key_context(SELECT_CONTEXT)
            .cursor_pointer()
            .on_action(cx.listener(Self::command))
            .on_click(
                cx.listener(|select, event: &TargetedEvent<ClickEvent>, window, cx| {
                    select.toggle(Some(event.hitbox().displayed_bounds()), window, cx);
                }),
            )
            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                let _ = view.update(cx, |select, cx| {
                    select.toggle(select.trigger_bounds.get(), window, cx)
                });
            })
            .child(label)
            .child(
                canvas(
                    move |rect, window, _| {
                        bounds.set(
                            window
                                .try_element_geometry(rect)
                                .ok()
                                .map(|geometry| geometry.displayed_bounds()),
                        );
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            );

        let mut root = div().relative().child(trigger);
        if let Some(anchor) = self.anchor {
            let mut popup = div()
                .flex()
                .flex_col()
                .min_w(anchor.size.width)
                .max_h(window.viewport_size().height / 2.)
                .overflow_y_scroll();
            popup.style().refine(&self.popup_style);
            let popup =
                popup
                    .id("choices")
                    .role(if self.is_menu {
                        Role::Menu
                    } else {
                        Role::ListBox
                    })
                    .aria_label(self.label.clone())
                    .occlude()
                    .track_focus(&self.popup_focus)
                    .track_scroll(&self.scroll)
                    .key_context(SELECT_CONTEXT)
                    .on_action(cx.listener(Self::command))
                    .on_mouse_down_out(cx.listener(
                        |select, event: &TargetedEvent<open_gpui::MouseDownEvent>, window, cx| {
                            if event.window_event().button == MouseButton::Left
                                && select.trigger_bounds.get().is_none_or(|bounds| {
                                    !bounds.contains(&event.window_event().position)
                                })
                            {
                                select.dismiss(false, window, cx);
                            }
                        },
                    ))
                    .children(self.items.iter().enumerate().map(|(index, item)| {
                        let active = self.active == Some(index);
                        let mut row = div()
                            .id(index)
                            .role(if self.is_menu {
                                Role::MenuItem
                            } else {
                                Role::ListBoxOption
                            })
                            .aria_label(item.label.clone())
                            .aria_disabled(item.disabled)
                            .aria_selected(self.selected.as_ref() == Some(&item.key))
                            .child(item.label.clone());
                        if active {
                            row = row
                                .aria_active_descendant()
                                .font_weight(FontWeight::SEMIBOLD);
                        }
                        if item.disabled {
                            return row;
                        }
                        let key = item.key.clone();
                        let accessible_key = key.clone();
                        let view = cx.weak_entity();
                        row.cursor_pointer()
                            .on_click(cx.listener(move |select, _, window, cx| {
                                select.choose(&key, window, cx)
                            }))
                            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                                let _ = view.update(cx, |select, cx| {
                                    select.choose(&accessible_key, window, cx)
                                });
                            })
                    }));
            root = root.child(
                window_portal(anchored().attach(anchor, Anchor::BottomLeft).child(popup))
                    .with_priority(2),
            );
        }
        root
    }
}

fn enabled_edge<K>(items: &[MenuItem<K>], last: bool) -> Option<usize> {
    if last {
        items.iter().rposition(|item| !item.disabled)
    } else {
        items.iter().position(|item| !item.disabled)
    }
}

fn next_enabled<K>(items: &[MenuItem<K>], active: Option<usize>, forward: bool) -> Option<usize> {
    let Some(active) = active else {
        return enabled_edge(items, !forward);
    };
    let len = items.len();
    (1..=len).find_map(|step| {
        let index = if forward {
            (active + step) % len
        } else {
            (active + len - step) % len
        };
        (!items[index].disabled).then_some(index)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_navigation_skips_disabled_items_and_wraps() {
        let items = [
            MenuItem::new(1, "one"),
            MenuItem::new(2, "two").disabled(true),
            MenuItem::new(3, "three"),
        ];
        assert_eq!(next_enabled(&items, Some(0), true), Some(2));
        assert_eq!(next_enabled(&items, Some(2), true), Some(0));
        assert_eq!(next_enabled(&items, Some(0), false), Some(2));
        assert_eq!(
            next_enabled(&[MenuItem::new(1, "disabled").disabled(true)], None, true),
            None
        );
    }
}
