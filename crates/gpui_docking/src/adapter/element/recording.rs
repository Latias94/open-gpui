//! Records the core paint program in its original order, independently of GPUI layout.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use crate::adapter::{DockPane, DockspaceColors};
use anyhow::{Result, anyhow};
use dockspace::{
    model::{DockspaceActionRejection, DockspaceAxis, ItemId, TabLabel},
    runtime::{
        ContainedResizeDirection, DockspaceDropEligibility, DockspacePresentationCommandKind,
        DockspacePresentationCommandUnavailable, DockspacePreviewVisual,
        DockspaceReceiverDescriptor, DockspaceTabsGroupCommandKind,
        SurfaceContainedResizeAdjustment, SurfacePaintPlan,
        SurfacePaintProgramOperation as Operation, SurfacePaintProgramStep as Step,
        SurfaceSplitterAdjustment, TabStripControlKind,
    },
};

use super::{
    ControlFocusId, Draw, Packet, PacketState,
    chrome::{Control, ControlAction, ControlKind, GuideState, Icon, PresentationMenu},
    presentation_menu::{OpenMenu, Scope},
    tab_label::CustomTabLabel,
};

pub(super) fn record(
    plan: SurfacePaintPlan<'_>,
    panels: &BTreeMap<ItemId, DockPane>,
    colors: &DockspaceColors,
    open_menu: Option<OpenMenu>,
) -> Result<(Packet, dockspace::runtime::SurfaceProgramExecutionReceipt)> {
    let mut draws = Vec::new();
    let mut pane_bars = BTreeMap::new();
    let mut pane_receivers = BTreeMap::new();
    let mut native_preview = None;
    let mut tab_list_dismiss = None;
    let mut tab_strips = BTreeMap::new();
    let mut menu_openers = BTreeMap::new();
    let receiver = |draws: &mut Vec<Draw>, value: Option<DockspaceReceiverDescriptor>| {
        if let Some(value) = value {
            draws.push(Draw::Receiver(value));
        }
    };
    let receipt = plan
        .execute_surface_program(|step| {
            match step {
                Step::RootBegin { root, paint } => {
                    if let Some(contained) = root.contained() {
                        receiver(&mut draws, plan.receiver_for_contained_frame(contained));
                        if !paint.is_source_gap() {
                            draws.push(Draw::ContainedFrame(contained.outer_bounds()));
                        }
                    }
                }
                Step::Operation {
                    operation,
                    paint,
                    root,
                } => match operation {
                    Operation::Pane(pane) => {
                        let pane_receiver = plan.receiver_for_pane(pane);
                        receiver(&mut draws, pane_receiver);
                        if !paint.is_source_gap() {
                            if let Some(receiver) = pane_receiver {
                                pane_receivers.insert(pane.item(), receiver);
                            }
                            let geometry = pane.content_geometry();
                            pane_bars.insert(pane.item(), pane.tab_bar_visual_id());
                            draws.push(Draw::Fill(geometry.clip(), colors.pane));
                            draws.push(Draw::Pane(pane.item(), geometry));
                        }
                    }
                    Operation::Tabs(tabs) => {
                        receiver(&mut draws, plan.receiver_for_tabs(tabs));
                        if !paint.is_source_gap() {
                            draws.push(Draw::Fill(tabs.bounds(), colors.pane));
                        }
                    }
                    Operation::TabBar(bar) => {
                        if !paint.is_source_gap() {
                            if let Some(strip) = plan.retain_tab_strip(bar) {
                                tab_strips.insert(bar.visual_id(), Rc::new(strip));
                            }
                            if let Some(receiver) = plan.receiver_for_tab_strip_scroll(bar) {
                                draws.push(Draw::Scroll(super::wheel::ScrollTarget::Strip(
                                    receiver,
                                )));
                            }
                            draws.push(Draw::Fill(bar.bounds(), colors.tab_bar));
                            draws.push(Draw::TabList(bar.bounds(), bar.visual_id()));
                            if let Some(bounds) = bar.group_grip_bounds() {
                                draws.push(Draw::Icon(bounds, Icon::Grip, true));
                            }
                            if let Some(bounds) = bar.group_menu_bounds() {
                                let mut choices = Vec::new();
                                if open_menu.is_some_and(|open| open.matches(bar.visual_id(), plan))
                                {
                                    for (kind, label) in [
                                        (DockspaceTabsGroupCommandKind::Float, "Float Tab Group"),
                                        (
                                            DockspaceTabsGroupCommandKind::MoveToNewWindow,
                                            "Move Tab Group to New Window",
                                        ),
                                    ] {
                                        if let Some(command) = plan.tabs_group_command(bar, kind)? {
                                            if native_unavailable(&command) {
                                                continue;
                                            }
                                            choices.push((
                                                label,
                                                plan.prepare_presentation_command(command).map(
                                                    |action| {
                                                        Rc::new(RefCell::new(Some(action.into())))
                                                    },
                                                ),
                                            ));
                                        }
                                    }
                                    choices.push((
                                        "Close Tab Group",
                                        plan.prepare_tabs_close(bar).map(|action| {
                                            Rc::new(RefCell::new(Some(action.into())))
                                        }),
                                    ));
                                }
                                draws.push(Draw::PresentationMenu(Rc::new(PresentationMenu {
                                    id: bar.visual_id(),
                                    workspace: plan.workspace_stamp(),
                                    open: open_menu
                                        .filter(|open| open.matches(bar.visual_id(), plan)),
                                    scope: Scope::Tabs,
                                    bounds,
                                    enabled: plan.tabs_group_menu_operable(bar),
                                    choices,
                                })));
                            }
                        }
                        for region in bar.group_drag_regions() {
                            receiver(&mut draws, plan.receiver_for_tab_group_drag_region(region));
                        }
                    }
                    Operation::Tab(tab) => {
                        receiver(&mut draws, plan.receiver_for_tab_body(tab));
                        if !paint.is_source_gap() {
                            draws.push(Draw::Tab(
                                tab.visible_bounds(),
                                tab.key(),
                                if tab.selected() {
                                    colors.selected_tab
                                } else {
                                    colors.tab
                                },
                                tab.selected(),
                            ));
                            let title = tab_label(tab.label(), panels);
                            draws.push(Draw::TabLabel {
                                layout: tab.text_layout_bounds(),
                                clip: tab.text_bounds(),
                                key: tab.key(),
                                title: title.clone(),
                                content: tab
                                    .item()
                                    .and_then(|item| panels.get(&item))
                                    .and_then(CustomTabLabel::new),
                                selected: tab.selected(),
                                color: if tab.selected() {
                                    colors.text
                                } else {
                                    colors.muted_text
                                },
                            });
                            let body = plan.receiver_for_tab_body(tab);
                            let mut control = Control::new(
                                tab.visual_id(),
                                tab.visible_bounds(),
                                ControlKind::Tab(tab.key()),
                                title,
                                body,
                            );
                            control.identity = ControlFocusId::TabChrome(tab.input_id());
                            control.selected = Some(tab.selected());
                            control.tab_strip = tab_strips.get(&tab.tab_bar_visual_id()).cloned();
                            control.action(
                                ControlAction::Activate,
                                plan.prepare_tab_select(tab.key()),
                            );
                            control.action(ControlAction::Close, plan.prepare_tab_close(tab.key()));
                            draws.push(Draw::Control(Rc::new(control)));
                            let close = plan.receiver_for_tab_close(tab);
                            receiver(&mut draws, close);
                            if let Some(rect) = tab.close_visual_bounds() {
                                draws.push(Draw::Icon(rect, Icon::Close, close.is_some()));
                            }
                            if let Some(close) = close {
                                let mut control = Control::new(
                                    close.visual_id(),
                                    close.bounds(),
                                    ControlKind::Button,
                                    format!("Close {}", tab_label(tab.label(), panels)),
                                    Some(close),
                                );
                                control.identity = ControlFocusId::TabClose(tab.input_id());
                                control.action(
                                    ControlAction::Activate,
                                    plan.prepare_tab_close(tab.key()),
                                );
                                draws.push(Draw::Control(Rc::new(control)));
                            }
                        }
                    }
                    Operation::Splitter(splitter) => {
                        receiver(&mut draws, plan.receiver_for_splitter(splitter));
                        if !paint.is_source_gap() {
                            draws.push(Draw::Fill(splitter.draw_bounds(), colors.splitter));
                            let mut control = Control::new(
                                splitter.visual_id(),
                                splitter.hit_bounds(),
                                ControlKind::Splitter(splitter.axis()),
                                "Resize panes".into(),
                                plan.receiver_for_splitter(splitter),
                            );
                            control.value = Some((
                                match splitter.axis() {
                                    DockspaceAxis::Horizontal => splitter.before_bounds().width(),
                                    DockspaceAxis::Vertical => splitter.before_bounds().height(),
                                },
                                splitter.minimum_extents().0,
                                splitter.maximum_extents().0,
                            ));
                            for (key, adjustment) in [
                                (
                                    ControlAction::Activate,
                                    SurfaceSplitterAdjustment::Preferred,
                                ),
                                (ControlAction::Equalize, SurfaceSplitterAdjustment::Equalize),
                                (
                                    ControlAction::Decrement,
                                    SurfaceSplitterAdjustment::Decrement,
                                ),
                                (
                                    ControlAction::Increment,
                                    SurfaceSplitterAdjustment::Increment,
                                ),
                                (ControlAction::Minimum, SurfaceSplitterAdjustment::Minimum),
                                (ControlAction::Maximum, SurfaceSplitterAdjustment::Maximum),
                            ] {
                                control.action(
                                    key,
                                    plan.prepare_splitter_adjustment(
                                        splitter.visual_id(),
                                        adjustment,
                                    ),
                                );
                            }
                            draws.push(Draw::Control(Rc::new(control)));
                        }
                    }
                    Operation::SplitterJunction(junction) => {
                        let descriptor = plan.receiver_for_splitter_junction(junction);
                        receiver(&mut draws, descriptor);
                        if !paint.is_source_gap() {
                            let mut control = Control::new(
                                junction.visual_id(),
                                junction.hit_bounds(),
                                ControlKind::Junction,
                                "Resize pane junction".into(),
                                descriptor,
                            );
                            for (key, axis, adjustment) in [
                                (
                                    ControlAction::Decrement,
                                    DockspaceAxis::Horizontal,
                                    SurfaceSplitterAdjustment::Decrement,
                                ),
                                (
                                    ControlAction::Increment,
                                    DockspaceAxis::Horizontal,
                                    SurfaceSplitterAdjustment::Increment,
                                ),
                                (
                                    ControlAction::Up,
                                    DockspaceAxis::Vertical,
                                    SurfaceSplitterAdjustment::Decrement,
                                ),
                                (
                                    ControlAction::Down,
                                    DockspaceAxis::Vertical,
                                    SurfaceSplitterAdjustment::Increment,
                                ),
                            ] {
                                control.action(
                                    key,
                                    plan.prepare_splitter_junction_adjustment(
                                        junction.visual_id(),
                                        axis,
                                        adjustment,
                                    ),
                                );
                            }
                            draws.push(Draw::Control(Rc::new(control)));
                        }
                    }
                    Operation::ContainedControls(contained) => {
                        receiver(&mut draws, plan.receiver_for_contained_title(contained));
                        for resize in contained.resize() {
                            let descriptor = plan.receiver_for_contained_resize(contained, resize);
                            receiver(&mut draws, descriptor);
                            if !paint.is_source_gap()
                                && let Some(descriptor) = descriptor
                            {
                                let axis = match resize.direction() {
                                    ContainedResizeDirection::North
                                    | ContainedResizeDirection::South => {
                                        Some(DockspaceAxis::Vertical)
                                    }
                                    ContainedResizeDirection::East
                                    | ContainedResizeDirection::West => {
                                        Some(DockspaceAxis::Horizontal)
                                    }
                                    _ => None,
                                };
                                if let Some(axis) = axis {
                                    let mut control = Control::new(
                                        descriptor.visual_id(),
                                        descriptor.bounds(),
                                        ControlKind::Splitter(axis),
                                        "Resize floating pane".into(),
                                        Some(descriptor),
                                    );
                                    control.action(
                                        ControlAction::Decrement,
                                        plan.prepare_contained_resize_adjustment(
                                            contained.floating(),
                                            resize.direction(),
                                            SurfaceContainedResizeAdjustment::Decrement,
                                        ),
                                    );
                                    control.action(
                                        ControlAction::Increment,
                                        plan.prepare_contained_resize_adjustment(
                                            contained.floating(),
                                            resize.direction(),
                                            SurfaceContainedResizeAdjustment::Increment,
                                        ),
                                    );
                                    draws.push(Draw::Control(Rc::new(control)));
                                }
                            }
                        }
                        if !paint.is_source_gap() {
                            draws.push(Draw::ContainedTitle(
                                contained.title_bounds(),
                                colors.contained_title,
                            ));
                            draws.push(Draw::Text {
                                layout: contained.title_drag_bounds(),
                                clip: contained.title_drag_bounds(),
                                title: tab_label(root.label(), panels),
                                color: colors.text,
                            });
                            let close = plan.receiver_for_contained_close(contained);
                            receiver(&mut draws, close);
                            if let Some(rect) = contained.close_bounds() {
                                draws.push(Draw::Icon(rect, Icon::Close, close.is_some()));
                            }
                            if let Some(close) = close {
                                let mut control = Control::new(
                                    close.visual_id(),
                                    close.bounds(),
                                    ControlKind::Button,
                                    "Close floating layout".into(),
                                    Some(close),
                                );
                                control.action(
                                    ControlAction::Activate,
                                    plan.prepare_contained_close(contained.floating()),
                                );
                                draws.push(Draw::Control(Rc::new(control)));
                            }
                        }
                    }
                    Operation::TabStripControl(control) => {
                        receiver(&mut draws, plan.receiver_for_tab_strip_control(control));
                        if !paint.is_source_gap() {
                            draws.push(Draw::Fill(control.bounds(), colors.tab));
                            let (icon, title) = match control.kind() {
                                TabStripControlKind::ScrollBackward => {
                                    (Icon::Back, "Scroll tabs backward")
                                }
                                TabStripControlKind::ScrollForward => {
                                    (Icon::Forward, "Scroll tabs forward")
                                }
                                TabStripControlKind::TabListMenu => (Icon::List, "Show all tabs"),
                                TabStripControlKind::RevealTabBar => {
                                    (Icon::Reveal, "Reveal tab bar")
                                }
                            };
                            draws.push(Draw::Icon(control.bounds(), icon, control.enabled()));
                            let mut semantic = Control::new(
                                control.visual_id(),
                                control.hit_bounds(),
                                ControlKind::Button,
                                title.into(),
                                plan.receiver_for_tab_strip_control(control),
                            );
                            semantic.identity = ControlFocusId::TabChrome(control.input_id());
                            if control.kind() == TabStripControlKind::TabListMenu {
                                semantic.expanded = Some(control.menu_open());
                                semantic.opens_menu = Some(control.tab_bar_visual_id());
                            }
                            semantic.action(
                                ControlAction::Activate,
                                plan.prepare_tab_strip_control_activation(control),
                            );
                            if let Some(bar) = semantic.opens_menu
                                && let Some(action) = semantic.slot(ControlAction::Activate)
                                && let Some(action) = action.borrow_mut().as_mut()
                            {
                                action.tab_list_opener = Some((bar, control.input_id()));
                            }
                            let semantic = Rc::new(semantic);
                            if let Some(bar) = semantic.opens_menu {
                                menu_openers.insert(bar, semantic.clone());
                            }
                            draws.push(Draw::Control(semantic));
                        }
                    }
                    Operation::PresentationMenuAnchor(anchor) => {
                        if !paint.is_source_gap() {
                            if let Some(bounds) = anchor.reserved_bounds() {
                                draws.push(Draw::Fill(bounds, colors.tab_bar));
                            }
                            let mut choices = Vec::new();
                            if open_menu.is_some_and(|open| open.matches(anchor.visual_id(), plan))
                            {
                                for (kind, label) in [
                                    (DockspacePresentationCommandKind::Float, "Float Layout"),
                                    (
                                        DockspacePresentationCommandKind::MoveToNewWindow,
                                        "Move Layout to New Window",
                                    ),
                                    (DockspacePresentationCommandKind::DockBack, "Dock Back"),
                                    (DockspacePresentationCommandKind::DockLeft, "Dock Left"),
                                    (DockspacePresentationCommandKind::DockRight, "Dock Right"),
                                    (DockspacePresentationCommandKind::DockTop, "Dock Top"),
                                    (DockspacePresentationCommandKind::DockBottom, "Dock Bottom"),
                                    (DockspacePresentationCommandKind::DockCenter, "Dock Center"),
                                ] {
                                    if let Some(command) =
                                        plan.presentation_command(anchor.root(), kind)?
                                    {
                                        if native_unavailable(&command) {
                                            continue;
                                        }
                                        let action =
                                            plan.prepare_presentation_command(command).map(
                                                |action| Rc::new(RefCell::new(Some(action.into()))),
                                            );
                                        choices.push((label, action));
                                    }
                                }
                                choices.push((
                                    "Close Layout",
                                    plan.prepare_root_close(anchor.root())
                                        .map(|action| Rc::new(RefCell::new(Some(action.into())))),
                                ));
                            }
                            draws.push(Draw::PresentationMenu(Rc::new(PresentationMenu {
                                id: anchor.visual_id(),
                                workspace: plan.workspace_stamp(),
                                open: open_menu
                                    .filter(|open| open.matches(anchor.visual_id(), plan)),
                                scope: Scope::Root,
                                bounds: anchor.bounds(),
                                enabled: anchor.operable(),
                                choices,
                            })));
                        }
                    }
                },
                Step::DropGuide {
                    affordance: Some(cluster),
                    ..
                } => {
                    for target in cluster.targets() {
                        receiver(&mut draws, plan.receiver_for_drop_target(target));
                        draws.push(Draw::Guide(
                            target.draw_bounds(),
                            GuideState {
                                direction: target.direction(),
                                active: target.is_active(),
                                eligible: target.eligibility()
                                    == DockspaceDropEligibility::Eligible,
                            },
                        ));
                    }
                }
                Step::DragPreview(preview) => {
                    if let Some(target) = preview.target() {
                        draws.push(Draw::Outline(target.bounds(), colors.accent));
                    }
                    if !preview.is_layout_projected() {
                        match preview.visual() {
                            DockspacePreviewVisual::Dock { rect, .. }
                            | DockspacePreviewVisual::Contained { rect, .. } => {
                                draws.push(Draw::Preview(rect))
                            }
                            DockspacePreviewVisual::Native {
                                host_surface,
                                target_surface,
                                placement,
                            } => {
                                native_preview =
                                    Some(crate::adapter::host_native::provisional::Preview {
                                        host_surface,
                                        target_surface,
                                        placement,
                                    });
                                draws.push(Draw::NativePreview(
                                    crate::adapter::host_native::provisional::Preview {
                                        host_surface,
                                        target_surface,
                                        placement,
                                    },
                                ));
                            }
                        }
                    }
                }
                Step::ContainedTransformPreview(preview) => {
                    if !preview.is_layout_projected() {
                        draws.push(Draw::Preview(preview.rect()));
                    }
                }
                Step::DragDecoration(decoration) if !decoration.source_is_projected() => {
                    draws.push(Draw::DragGhost(tab_label(decoration.label(), panels)));
                }
                Step::PopupBackdrop(backdrop) => {
                    tab_list_dismiss = plan
                        .prepare_tab_list_menu_dismiss(backdrop)
                        .map(|action| Rc::new(RefCell::new(Some(action.into()))));
                    receiver(
                        &mut draws,
                        plan.receiver_for_tab_list_menu_backdrop(backdrop),
                    );
                }
                Step::PopupMenu { menu, .. } => {
                    let bar = menu.tab_bar_visual_id();
                    let menu_keys = plan.retain_tab_list_menu(menu).map(|retained| {
                        Rc::new(super::keyboard::MenuKeys::new(
                            retained,
                            bar,
                            menu_openers.get(&bar).cloned(),
                        ))
                    });
                    draws.push(Draw::Fill(menu.bounds(), colors.selected_tab));
                    if let Some(keys) = &menu_keys
                        && let Some(row) = menu.rows().find(|row| row.focused())
                        && row.interaction_bounds().is_none()
                    {
                        // Give the popup a keyboard target behind its pointer receivers
                        // while the focused row is outside the viewport.
                        let mut control = Control::new(
                            row.visual_id(),
                            menu.viewport(),
                            ControlKind::MenuRow,
                            String::new(),
                            None,
                        );
                        control.menu_row = Some(super::keyboard::MenuRow {
                            id: row.input_id(),
                            focused: true,
                            keys: keys.clone(),
                        });
                        draws.push(Draw::TabListMenuFocus {
                            menu: menu.input_id(),
                            row: Rc::new(control),
                        });
                    }
                    if let Some(receiver) = plan.receiver_for_tab_list_menu_scroll(menu) {
                        draws.push(Draw::Scroll(super::wheel::ScrollTarget::Menu(receiver)));
                    }
                    receiver(&mut draws, plan.receiver_for_tab_list_menu_frame(menu));
                    for row in menu.rows() {
                        let Some(clip) = row.interaction_bounds() else {
                            continue;
                        };
                        let row_receiver = plan.receiver_for_tab_list_menu_row(row);
                        receiver(&mut draws, row_receiver);
                        let title = tab_label(row.label(), panels);
                        draws.push(Draw::Text {
                            layout: row.bounds(),
                            clip,
                            title: title.clone(),
                            color: colors.text,
                        });
                        let mut control = Control::new(
                            row.visual_id(),
                            clip,
                            ControlKind::MenuRow,
                            title,
                            row_receiver,
                        );
                        control.selected = Some(row.selected());
                        control.menu_row =
                            menu_keys.as_ref().map(|keys| super::keyboard::MenuRow {
                                id: row.input_id(),
                                focused: row.focused(),
                                keys: keys.clone(),
                            });
                        draws.push(Draw::Control(Rc::new(control)));
                    }
                }
                Step::DropTarget(_)
                | Step::DragDecoration(_)
                | Step::DropGuide {
                    affordance: None, ..
                }
                | Step::InteractionFeedback
                | Step::RootEnd(_)
                | Step::SurfaceEnd { .. } => {}
            }
            Ok::<(), dockspace::runtime::DockspaceRuntimeError>(())
        })
        .map_err(|error| anyhow!("core surface program rejected: {error:?}"))?;
    Ok((
        Packet {
            draws,
            pane_bars,
            pane_receivers,
            state: PacketState::Ready,
            native_preview,
            tab_list_dismiss,
        },
        receipt,
    ))
}

fn native_unavailable(command: &dockspace::runtime::PresentationCommandPaintRecord) -> bool {
    command.unavailable_reason()
        == Some(DockspacePresentationCommandUnavailable::Action(
            DockspaceActionRejection::NativeUnavailable,
        ))
}

/// Resolve the core's frozen scope label without substituting a descendant pane.
pub(super) fn tab_label(label: TabLabel<'_>, panels: &BTreeMap<ItemId, DockPane>) -> String {
    match label {
        TabLabel::Item(item) => panels.get(&item).map_or_else(
            || "Pane unavailable".to_owned(),
            |pane| pane.title.to_string(),
        ),
        TabLabel::Layout { title } => title.to_owned(),
    }
}
