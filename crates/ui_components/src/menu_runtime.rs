//! Internal runtime helpers for menu-like overlays.

use crate::menu::MenuSubmenuNavigation;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use open_gpui::{App, Entity, FocusHandle, Pixels, Point, ScrollHandle, Task, Window};
use open_gpui_ui_core::Rect;

pub(crate) const MENU_SUBMENU_OPEN_DELAY: Duration = Duration::from_millis(120);
pub(crate) const MENU_SUBMENU_CLOSE_DELAY: Duration = Duration::from_millis(180);

#[derive(Debug, Clone)]
pub(crate) struct MenuRuntime {
    pub(crate) open: bool,
    pub(crate) did_initial_focus: bool,
    pub(crate) trigger_focus: FocusHandle,
    pub(crate) focused_value: Option<String>,
    pub(crate) focused_path: Option<Vec<String>>,
    pub(crate) open_path: Vec<String>,
    pub(crate) submenu_hovered_path: Option<Vec<String>>,
    pub(crate) submenu_hovering_surface: bool,
    pub(crate) submenu_hover_epoch: u64,
    pub(crate) submenu_hover_task: Option<Rc<Task<()>>>,
    pub(crate) scroll_handle: ScrollHandle,
    pub(crate) submenu_scroll_handles: HashMap<String, ScrollHandle>,
    pub(crate) submenu_trigger_bounds: HashMap<String, Rect>,
    pub(crate) content_focus: FocusHandle,
}

impl MenuRuntime {
    pub(crate) fn new(
        open: bool,
        trigger_focus: FocusHandle,
        content_focus: FocusHandle,
        focused_value: Option<String>,
    ) -> Self {
        Self {
            open,
            did_initial_focus: false,
            trigger_focus,
            content_focus,
            focused_value,
            focused_path: None,
            open_path: Vec::new(),
            submenu_hovered_path: None,
            submenu_hovering_surface: false,
            submenu_hover_epoch: 0,
            submenu_hover_task: None,
            scroll_handle: ScrollHandle::new(),
            submenu_scroll_handles: HashMap::new(),
            submenu_trigger_bounds: HashMap::new(),
        }
    }

    pub(crate) fn resolved_focused_value<'a>(
        &'a self,
        configured: Option<&'a str>,
    ) -> Option<&'a str> {
        self.focused_value.as_deref().or(configured)
    }

    pub(crate) fn sync_controlled_open(&mut self, open: bool) {
        self.open = open;
        if !open {
            self.reset_closed_state();
        }
    }

    pub(crate) fn reset_closed_state(&mut self) {
        self.did_initial_focus = false;
        self.focused_value = None;
        self.focused_path = None;
        self.open_path.clear();
        self.reset_submenu_state();
    }

    pub(crate) fn focus_item(&mut self, focused_path: Vec<String>, focused_value: String) {
        self.focused_path = Some(focused_path);
        self.focused_value = Some(focused_value);
    }

    pub(crate) fn apply_submenu_target(&mut self, target: &MenuSubmenuNavigation) {
        self.open_path = target.open_path().to_vec();
        self.focused_path = Some(target.focused_path().to_vec());
        self.focused_value = Some(target.focused_value().to_owned());
    }

    pub(crate) fn submenu_scroll_handle(&mut self, branch_key: &str) -> ScrollHandle {
        self.submenu_scroll_handles
            .entry(branch_key.to_owned())
            .or_insert_with(ScrollHandle::new)
            .clone()
    }

    pub(crate) fn reset_submenu_state(&mut self) {
        self.submenu_hovered_path = None;
        self.submenu_hovering_surface = false;
        self.submenu_hover_epoch = self.submenu_hover_epoch.wrapping_add(1);
        self.submenu_hover_task = None;
        self.submenu_scroll_handles.clear();
        self.submenu_trigger_bounds.clear();
    }

    fn bump_submenu_hover_epoch(&mut self) {
        self.submenu_hover_epoch = self.submenu_hover_epoch.wrapping_add(1);
        self.submenu_hover_task = None;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContextMenuRuntime {
    pub(crate) open: bool,
    pub(crate) anchor_point: Point<Pixels>,
    pub(crate) did_initial_focus: bool,
    pub(crate) seeded_focused_value: bool,
    pub(crate) focused_value: Option<String>,
    pub(crate) focused_path: Option<Vec<String>>,
    pub(crate) open_path: Vec<String>,
    pub(crate) scroll_handle: ScrollHandle,
    pub(crate) content_focus: FocusHandle,
    pub(crate) trigger_focus: FocusHandle,
}

impl ContextMenuRuntime {
    pub(crate) fn new(
        open: bool,
        anchor_point: Point<Pixels>,
        content_focus: FocusHandle,
        trigger_focus: FocusHandle,
        focused_value: Option<String>,
    ) -> Self {
        Self {
            open,
            anchor_point,
            did_initial_focus: false,
            seeded_focused_value: false,
            focused_value,
            focused_path: None,
            open_path: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            content_focus,
            trigger_focus,
        }
    }

    pub(crate) fn resolved_focused_value<'a>(
        &'a self,
        configured: Option<&'a str>,
    ) -> Option<&'a str> {
        if self.seeded_focused_value {
            self.focused_value.as_deref()
        } else {
            configured.or(self.focused_value.as_deref())
        }
    }

    pub(crate) fn sync_controlled_open(&mut self, open: bool) {
        self.open = open;
        if !open {
            self.reset_closed_state();
        }
    }

    pub(crate) fn reset_closed_state(&mut self) {
        self.did_initial_focus = false;
        self.seeded_focused_value = false;
        self.focused_value = None;
        self.focused_path = None;
        self.open_path.clear();
    }

    pub(crate) fn open_at(&mut self, anchor_point: Point<Pixels>) {
        self.open = true;
        self.anchor_point = anchor_point;
        self.focused_path = None;
        self.open_path.clear();
    }

    pub(crate) fn focus_item(&mut self, focused_path: Vec<String>, focused_value: String) {
        self.seeded_focused_value = true;
        self.focused_value = Some(focused_value);
        self.focused_path = Some(focused_path);
    }

    pub(crate) fn apply_submenu_target(&mut self, target: &MenuSubmenuNavigation) {
        self.seeded_focused_value = true;
        self.open_path = target.open_path().to_vec();
        self.focused_path = Some(target.focused_path().to_vec());
        self.focused_value = Some(target.focused_value().to_owned());
    }
}

pub(crate) fn update_menu_hover_target(
    runtime: Entity<MenuRuntime>,
    focused_path: Vec<String>,
    focused_value: String,
    submenu_navigation: Option<MenuSubmenuNavigation>,
    hovered: bool,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        if hovered {
            runtime.focus_item(focused_path.clone(), focused_value.clone());
            runtime.submenu_hovered_path = Some(focused_path.clone());
        } else if runtime.submenu_hovered_path.as_deref() == Some(focused_path.as_slice()) {
            runtime.submenu_hovered_path = None;
        }
    });

    if hovered {
        if let Some(submenu_navigation) = submenu_navigation {
            let open_path = submenu_navigation.open_path().to_vec();
            let should_open = runtime.read(cx).open_path != open_path;
            if should_open {
                schedule_menu_submenu_open(runtime, submenu_navigation, window, cx);
            }
        } else {
            let should_close = {
                let runtime_state = runtime.read(cx);
                !runtime_state.open_path.is_empty()
                    && runtime_state
                        .submenu_hovered_path
                        .as_deref()
                        .is_none_or(|path| !path.starts_with(runtime_state.open_path.as_slice()))
            };
            if should_close {
                schedule_menu_submenu_close(runtime, window, cx);
            }
        }
    } else {
        let should_close = {
            let runtime_state = runtime.read(cx);
            !runtime_state.open_path.is_empty()
                && runtime_state
                    .submenu_hovered_path
                    .as_deref()
                    .is_none_or(|path| path.starts_with(runtime_state.open_path.as_slice()))
        };
        if should_close {
            schedule_menu_submenu_close(runtime, window, cx);
        }
    }
}

pub(crate) fn handle_menu_submenu_surface_hover(
    runtime: Entity<MenuRuntime>,
    hovered: bool,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.submenu_hovering_surface = hovered;
    });

    if !hovered {
        let should_close = {
            let runtime_state = runtime.read(cx);
            !runtime_state.open_path.is_empty()
                && runtime_state
                    .submenu_hovered_path
                    .as_deref()
                    .is_none_or(|path| path.starts_with(runtime_state.open_path.as_slice()))
        };
        if should_close {
            schedule_menu_submenu_close(runtime, window, cx);
        }
    }
}

fn schedule_menu_submenu_open(
    runtime: Entity<MenuRuntime>,
    submenu_navigation: MenuSubmenuNavigation,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.bump_submenu_hover_epoch();
    });
    let epoch = runtime.read(cx).submenu_hover_epoch;
    let open_path = submenu_navigation.open_path().to_vec();
    let focused_path = submenu_navigation.focused_path().to_vec();
    let focused_value = submenu_navigation.focused_value().to_owned();

    if MENU_SUBMENU_OPEN_DELAY.is_zero() {
        runtime.update(cx, |runtime, _| {
            runtime.open_path = open_path;
            runtime.focused_path = Some(focused_path);
            runtime.focused_value = Some(focused_value);
        });
        return;
    }

    let task = window.spawn(cx, {
        let runtime = runtime.clone();
        let open_path = open_path.clone();
        let focused_path = focused_path.clone();
        let focused_value = focused_value.clone();
        async move |cx| {
            cx.background_executor()
                .timer(MENU_SUBMENU_OPEN_DELAY)
                .await;
            cx.update(|_, cx| {
                let should_open = {
                    let runtime_state = runtime.read(cx);
                    runtime_state.submenu_hover_epoch == epoch
                        && runtime_state
                            .submenu_hovered_path
                            .as_deref()
                            .is_some_and(|path| path.starts_with(open_path.as_slice()))
                };

                if should_open {
                    runtime.update(cx, |runtime, _| {
                        runtime.open_path = open_path.clone();
                        runtime.focused_path = Some(focused_path.clone());
                        runtime.focused_value = Some(focused_value.clone());
                        runtime.submenu_hover_task = None;
                        runtime.submenu_hover_epoch = runtime.submenu_hover_epoch.wrapping_add(1);
                    });
                }
            })
            .ok();
        }
    });
    runtime.update(cx, |runtime, _| {
        runtime.submenu_hover_task = Some(Rc::new(task));
    });
}

fn schedule_menu_submenu_close(runtime: Entity<MenuRuntime>, window: &mut Window, cx: &mut App) {
    runtime.update(cx, |runtime, _| {
        runtime.bump_submenu_hover_epoch();
    });
    let epoch = runtime.read(cx).submenu_hover_epoch;

    if MENU_SUBMENU_CLOSE_DELAY.is_zero() {
        runtime.update(cx, |runtime, _| {
            runtime.open_path.clear();
        });
        return;
    }

    let task = window.spawn(cx, {
        let runtime = runtime.clone();
        async move |cx| {
            cx.background_executor()
                .timer(MENU_SUBMENU_CLOSE_DELAY)
                .await;
            cx.update(|_, cx| {
                let should_close = {
                    let runtime_state = runtime.read(cx);
                    runtime_state.submenu_hover_epoch == epoch
                        && !runtime_state.submenu_hovering_surface
                        && !runtime_state.open_path.is_empty()
                        && runtime_state
                            .submenu_hovered_path
                            .as_deref()
                            .is_none_or(|path| {
                                !path.starts_with(runtime_state.open_path.as_slice())
                            })
                };

                if should_close {
                    runtime.update(cx, |runtime, _| {
                        runtime.open_path.clear();
                        runtime.submenu_hover_task = None;
                        runtime.submenu_hover_epoch = runtime.submenu_hover_epoch.wrapping_add(1);
                    });
                }
            })
            .ok();
        }
    });
    runtime.update(cx, |runtime, _| {
        runtime.submenu_hover_task = Some(Rc::new(task));
    });
}
