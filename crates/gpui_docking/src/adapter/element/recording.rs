//! Records the core paint program in its original order, independently of GPUI layout.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{Result, anyhow};
use dockspace::{
    geometry::LogicalRect,
    model::{DockspaceAxis, ItemId},
    runtime::{
        ContainedResizeDirection, DockspaceDropEligibility, DockspacePresentationCommandKind,
        DockspacePreviewVisual, DockspaceReceiverDescriptor, SurfaceContainedResizeAdjustment,
        SurfacePaintPlan, SurfacePaintProgramOperation as Operation,
        SurfacePaintProgramStep as Step, SurfaceSplitterAdjustment, SurfaceTabListNavigation,
        SurfaceTabNavigation, TabStripControlKind,
    },
};

use crate::adapter::{DockPane, DockspaceColors};

use super::{
    Draw, Packet,
    chrome::{Control, ControlAction, ControlKind, GuideState, Icon, PresentationMenu},
};

pub(super) fn record(
    plan: SurfacePaintPlan<'_>,
    panels: &BTreeMap<ItemId, DockPane>,
    pointer: Option<dockspace::geometry::LogicalPoint>,
    colors: &DockspaceColors,
    open_menu: Option<dockspace::runtime::DockspaceVisualId>,
) -> Result<(Packet, dockspace::runtime::SurfaceProgramExecutionReceipt)> {
    let mut draws = Vec::new();
    let mut native_preview = None;
    let mut menu_backdrop = None;
    let label = |item: Option<ItemId>| {
        item.and_then(|item| panels.get(&item))
            .map_or_else(|| "Workspace".to_owned(), |pane| pane.title.to_string())
    };
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
                        receiver(&mut draws, plan.receiver_for_pane(pane));
                        if !paint.is_source_gap() {
                            draws.push(Draw::Fill(pane.content_bounds(), colors.pane));
                            if let Some(item) = pane.selected() {
                                draws.push(Draw::Pane(pane.content_bounds(), item));
                            }
                        }
                    }
                    Operation::TabBar(bar) => {
                        if !paint.is_source_gap() {
                            draws.push(Draw::Fill(bar.bounds(), colors.tab_bar));
                            draws.push(Draw::TabList(bar.bounds(), bar.visual_id()));
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
                                tab.item(),
                                if tab.selected() {
                                    colors.selected_tab
                                } else {
                                    colors.tab
                                },
                                tab.selected(),
                            ));
                            draws.push(Draw::TabText(
                                tab.text_layout_bounds(),
                                tab.text_bounds(),
                                tab.item(),
                                label(Some(tab.item())),
                                if tab.selected() {
                                    colors.text
                                } else {
                                    colors.muted_text
                                },
                            ));
                            let body = plan.receiver_for_tab_body(tab);
                            let mut control = Control::new(
                                tab.visual_id(),
                                tab.visible_bounds(),
                                ControlKind::Tab(tab.item()),
                                label(Some(tab.item())),
                                body,
                            );
                            control.selected = Some(tab.selected());
                            control.action(
                                ControlAction::Activate,
                                plan.prepare_tab_select(tab.item()),
                            );
                            control
                                .action(ControlAction::Close, plan.prepare_tab_close(tab.item()));
                            for (key, navigation) in [
                                (ControlAction::Previous, SurfaceTabNavigation::Previous),
                                (ControlAction::Next, SurfaceTabNavigation::Next),
                                (ControlAction::First, SurfaceTabNavigation::First),
                                (ControlAction::Last, SurfaceTabNavigation::Last),
                            ] {
                                control.action(
                                    key,
                                    plan.prepare_tab_navigation(tab.item(), navigation),
                                );
                            }
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
                                    format!("Close {}", label(Some(tab.item()))),
                                    Some(close),
                                );
                                control.action(
                                    ControlAction::Activate,
                                    plan.prepare_tab_close(tab.item()),
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
                            draws.push(Draw::Text(
                                contained.title_drag_bounds(),
                                label(root.title_item()),
                                colors.text,
                            ));
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
                                    "Close floating group".into(),
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
                            if control.kind() == TabStripControlKind::TabListMenu {
                                semantic.expanded = Some(control.menu_open());
                                semantic.opens_menu = Some(control.tab_bar_visual_id());
                            }
                            semantic.action(
                                ControlAction::Activate,
                                plan.prepare_tab_strip_control_activation(control),
                            );
                            draws.push(Draw::Control(Rc::new(semantic)));
                        }
                    }
                    Operation::PresentationMenuAnchor(anchor) => {
                        if !paint.is_source_gap() {
                            let mut choices = Vec::new();
                            if open_menu == Some(anchor.visual_id()) {
                                for (kind, label) in [
                                    (DockspacePresentationCommandKind::Float, "Float"),
                                    (
                                        DockspacePresentationCommandKind::MoveToNewWindow,
                                        "Move to New Window",
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
                                        let action = plan
                                            .prepare_presentation_command(command)
                                            .map(|action| Rc::new(RefCell::new(Some(action))));
                                        choices.push((label, action));
                                    }
                                }
                                choices.push((
                                    "Close Group",
                                    plan.prepare_root_close(anchor.root())
                                        .map(|action| Rc::new(RefCell::new(Some(action)))),
                                ));
                            }
                            draws.push(Draw::PresentationMenu(Rc::new(PresentationMenu {
                                id: anchor.visual_id(),
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
                Step::DragDecoration(decoration) => {
                    if !decoration.source_is_projected() {
                        if let Some(pointer) = pointer {
                            if let Ok(ghost) = LogicalRect::new(
                                pointer.x() + 12.0,
                                pointer.y() + 14.0,
                                160.0,
                                28.0,
                            ) {
                                draws.push(Draw::Fill(ghost, colors.drag_ghost));
                                draws.push(Draw::Outline(ghost, colors.accent));
                                draws.push(Draw::Text(
                                    ghost,
                                    label(decoration.ghost_item()),
                                    colors.text,
                                ));
                            }
                        }
                    }
                }
                Step::PopupBackdrop(backdrop) => {
                    menu_backdrop = Some(backdrop);
                    receiver(
                        &mut draws,
                        plan.receiver_for_tab_list_menu_backdrop(backdrop),
                    );
                }
                Step::PopupMenu { menu, .. } => {
                    draws.push(Draw::Fill(menu.bounds(), colors.selected_tab));
                    receiver(&mut draws, plan.receiver_for_tab_list_menu_frame(menu));
                    for row in menu.rows() {
                        receiver(&mut draws, plan.receiver_for_tab_list_menu_row(row));
                        draws.push(Draw::Text(
                            row.bounds(),
                            label(Some(row.item())),
                            colors.text,
                        ));
                        if let Some(bounds) = row.interaction_bounds() {
                            let mut control = Control::new(
                                row.visual_id(),
                                bounds,
                                ControlKind::MenuRow,
                                label(Some(row.item())),
                                plan.receiver_for_tab_list_menu_row(row),
                            );
                            control.selected = Some(row.selected());
                            control.focused = row.focused();
                            control.menu_owner = Some(menu.tab_bar_visual_id());
                            control.action(
                                ControlAction::Activate,
                                plan.prepare_tab_list_menu_row_activation(row),
                            );
                            for (key, navigation) in [
                                (ControlAction::Previous, SurfaceTabListNavigation::Previous),
                                (ControlAction::Next, SurfaceTabListNavigation::Next),
                                (ControlAction::First, SurfaceTabListNavigation::First),
                                (ControlAction::Last, SurfaceTabListNavigation::Last),
                            ] {
                                control.action(
                                    key,
                                    plan.prepare_tab_list_menu_navigation(menu, navigation),
                                );
                            }
                            if let Some(backdrop) = menu_backdrop {
                                control.action(
                                    ControlAction::Dismiss,
                                    plan.prepare_tab_list_menu_dismiss(backdrop),
                                );
                            }
                            draws.push(Draw::Control(Rc::new(control)));
                        }
                    }
                }
                Step::DropTarget(_)
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
            deferred: false,
            native_preview,
        },
        receipt,
    ))
}
