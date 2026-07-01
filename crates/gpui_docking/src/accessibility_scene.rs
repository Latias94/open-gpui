use crate::{
    DockItemId, DockNodeId, DropZone, SplitAxis,
    overlay_scene::{DockOverlayLayerKind, DockOverlayScene},
    presentation_scene::{DockPresentationPaneKind, DockPresentationScene},
};
use open_gpui::{
    AccessibleAction as GpuiAccessibleAction, Bounds, Orientation as GpuiOrientation, Pixels,
    Role as GpuiRole, StatefulInteractiveElement,
};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role};
use std::fmt;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibilityScene {
    pub(crate) descriptors: Vec<DockAccessibilityDescriptor>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibilityDescriptor {
    pub(crate) role: DockAccessibilityRole,
    pub(crate) node: Option<DockNodeId>,
    pub(crate) item: Option<DockItemId>,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) label: Option<String>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) active: bool,
    pub(crate) orientation: Option<Orientation>,
    pub(crate) selected: Option<bool>,
    pub(crate) disabled: Option<bool>,
    pub(crate) actions: Vec<AccessibleAction>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAccessibleElement {
    pub(crate) id: DockAccessibleElementId,
    pub(crate) role: Role,
    pub(crate) gpui_role: GpuiRole,
    pub(crate) node: Option<DockNodeId>,
    pub(crate) item: Option<DockItemId>,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) label: String,
    pub(crate) hint: Option<String>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) active: bool,
    pub(crate) orientation: Option<Orientation>,
    pub(crate) selected: Option<bool>,
    pub(crate) disabled: bool,
    pub(crate) numeric_value: Option<f64>,
    pub(crate) actions: Vec<AccessibleAction>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DockAccessibleElementId(String);

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockAccessibilityLayer {
    Final,
    Overlay,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockAccessibilityRole {
    Pane,
    TabList,
    Tab,
    TabPanel,
    Splitter,
    FloatingWindow,
    FocusRegion,
    DropTarget,
    DragSource,
    DropDestination,
    RejectedDropTarget,
}

impl DockAccessibilityScene {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_presentation(scene: &DockPresentationScene) -> Self {
        let mut descriptors = Vec::new();

        for pane in &scene.panes {
            let label = match pane.kind {
                DockPresentationPaneKind::Tabs => title_for_tabs(scene, pane.node),
                DockPresentationPaneKind::EmptyCentral => Some("Empty dock area".to_string()),
            };
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Pane,
                node: pane.node,
                item: None,
                bounds: pane.bounds,
                label,
                zone: None,
                active: true,
                orientation: None,
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
            });
        }

        for tab_bar in &scene.tab_bars {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::TabList,
                node: Some(tab_bar.tabs),
                item: None,
                bounds: tab_bar.bounds,
                label: Some(tab_list_label(scene, tab_bar.tabs)),
                zone: None,
                active: true,
                orientation: Some(Orientation::Horizontal),
                selected: None,
                disabled: Some(false),
                actions: Vec::new(),
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::DropDestination,
                node: Some(tab_bar.tabs),
                item: None,
                bounds: tab_bar.bounds,
                label: Some(format!("Drop into {}", tab_list_label(scene, tab_bar.tabs))),
                zone: Some(DropZone::Center),
                active: true,
                orientation: Some(Orientation::Horizontal),
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::CustomAction],
            });
        }

        for tab in &scene.tab_labels {
            let selected = scene
                .focus_regions
                .iter()
                .any(|focus| focus.tabs == tab.tabs && focus.item == tab.item);
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Tab,
                node: Some(tab.tabs),
                item: Some(tab.item.clone()),
                bounds: tab.bounds,
                label: Some(tab.title.clone()),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(selected),
                disabled: Some(false),
                actions: vec![AccessibleAction::Click, AccessibleAction::Focus],
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::DragSource,
                node: Some(tab.tabs),
                item: Some(tab.item.clone()),
                bounds: tab.bounds,
                label: Some(format!("Drag {}", tab.title)),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(selected),
                disabled: Some(false),
                actions: vec![AccessibleAction::CustomAction],
            });
        }

        for focus in &scene.focus_regions {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::TabPanel,
                node: Some(focus.tabs),
                item: Some(focus.item.clone()),
                bounds: focus.bounds,
                label: Some(format!("{} panel", title_for_item(scene, &focus.item))),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(true),
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
            });
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::FocusRegion,
                node: Some(focus.tabs),
                item: Some(focus.item.clone()),
                bounds: focus.bounds,
                label: Some(format!("Focused {}", title_for_item(scene, &focus.item))),
                zone: None,
                active: true,
                orientation: None,
                selected: Some(true),
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
            });
        }

        for splitter in &scene.splitters {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::Splitter,
                node: Some(splitter.split),
                item: None,
                bounds: splitter.bounds,
                label: Some(splitter_label(splitter.axis, splitter.index)),
                zone: None,
                active: true,
                orientation: Some(orientation_for_axis(splitter.axis)),
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::Increment, AccessibleAction::Decrement],
            });
        }

        for floating in &scene.floating_containers {
            descriptors.push(DockAccessibilityDescriptor {
                role: DockAccessibilityRole::FloatingWindow,
                node: Some(floating.node),
                item: None,
                bounds: floating.bounds,
                label: Some("Floating window".to_string()),
                zone: None,
                active: true,
                orientation: None,
                selected: None,
                disabled: Some(false),
                actions: vec![AccessibleAction::Focus],
            });
        }

        Self { descriptors }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_overlay(mut self, overlay: &DockOverlayScene) -> Self {
        for layer in &overlay.layers {
            match layer.kind {
                DockOverlayLayerKind::GuideBox | DockOverlayLayerKind::TabInsertion => {
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DropTarget,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some(drop_target_label(layer.zone, layer.target_node)),
                        zone: layer.zone,
                        active: layer.active,
                        orientation: None,
                        selected: None,
                        disabled: Some(!layer.active),
                        actions: vec![AccessibleAction::CustomAction],
                    });
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DropDestination,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some(drop_destination_label(layer.zone, layer.target_node)),
                        zone: layer.zone,
                        active: layer.active,
                        orientation: None,
                        selected: None,
                        disabled: Some(!layer.active),
                        actions: vec![AccessibleAction::CustomAction],
                    });
                }
                DockOverlayLayerKind::PayloadTab | DockOverlayLayerKind::PayloadGhost => {
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::DragSource,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some(drag_payload_label(layer.payload_title.as_deref())),
                        zone: layer.zone,
                        active: layer.active,
                        orientation: None,
                        selected: None,
                        disabled: Some(!layer.active),
                        actions: vec![AccessibleAction::CustomAction],
                    });
                }
                DockOverlayLayerKind::RejectedState => {
                    self.descriptors.push(DockAccessibilityDescriptor {
                        role: DockAccessibilityRole::RejectedDropTarget,
                        node: layer.target_node,
                        item: None,
                        bounds: layer.bounds,
                        label: Some("Dock target unavailable".to_string()),
                        zone: layer.zone,
                        active: true,
                        orientation: None,
                        selected: None,
                        disabled: Some(true),
                        actions: Vec::new(),
                    });
                }
                DockOverlayLayerKind::RouteMarker
                | DockOverlayLayerKind::TargetBody
                | DockOverlayLayerKind::FocusRing => {}
            }
        }
        self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn gpui_elements(
        &self,
        layer: DockAccessibilityLayer,
    ) -> Vec<DockAccessibleElement> {
        self.descriptors
            .iter()
            .enumerate()
            .filter_map(|(order, descriptor)| descriptor.gpui_element(layer, order))
            .collect()
    }

    pub(crate) fn tab_list_element_for_render(
        tabs: DockNodeId,
        tab_count: usize,
    ) -> DockAccessibleElement {
        DockAccessibilityDescriptor {
            role: DockAccessibilityRole::TabList,
            node: Some(tabs),
            item: None,
            bounds: Bounds::default(),
            label: Some(if tab_count == 1 {
                "Dock tabs, 1 item".to_string()
            } else {
                format!("Dock tabs, {tab_count} items")
            }),
            zone: None,
            active: true,
            orientation: Some(Orientation::Horizontal),
            selected: None,
            disabled: Some(false),
            actions: Vec::new(),
        }
        .gpui_element(DockAccessibilityLayer::Final, 0)
        .expect("tab list role should map to GPUI")
    }

    pub(crate) fn tab_element_for_render(
        tabs: DockNodeId,
        item: DockItemId,
        title: String,
        selected: bool,
        index: usize,
    ) -> DockAccessibleElement {
        DockAccessibilityDescriptor {
            role: DockAccessibilityRole::Tab,
            node: Some(tabs),
            item: Some(item),
            bounds: Bounds::default(),
            label: Some(title),
            zone: None,
            active: true,
            orientation: None,
            selected: Some(selected),
            disabled: Some(false),
            actions: vec![AccessibleAction::Click, AccessibleAction::Focus],
        }
        .gpui_element(DockAccessibilityLayer::Final, index)
        .expect("tab role should map to GPUI")
    }

    pub(crate) fn tab_panel_element_for_render(
        item: DockItemId,
        title: String,
    ) -> DockAccessibleElement {
        DockAccessibilityDescriptor {
            role: DockAccessibilityRole::TabPanel,
            node: None,
            item: Some(item),
            bounds: Bounds::default(),
            label: Some(format!("{title} panel")),
            zone: None,
            active: true,
            orientation: None,
            selected: Some(true),
            disabled: Some(false),
            actions: vec![AccessibleAction::Focus],
        }
        .gpui_element(DockAccessibilityLayer::Final, 0)
        .expect("tab panel role should map to GPUI")
    }

    pub(crate) fn splitter_element_for_render(
        split: DockNodeId,
        axis: SplitAxis,
        index: usize,
        value: f32,
    ) -> DockAccessibleElement {
        let mut element = DockAccessibilityDescriptor {
            role: DockAccessibilityRole::Splitter,
            node: Some(split),
            item: None,
            bounds: Bounds::default(),
            label: Some(splitter_label(axis, index)),
            zone: None,
            active: true,
            orientation: Some(orientation_for_axis(axis)),
            selected: None,
            disabled: Some(false),
            actions: vec![AccessibleAction::Increment, AccessibleAction::Decrement],
        }
        .gpui_element(DockAccessibilityLayer::Final, index)
        .expect("splitter role should map to GPUI");
        element.numeric_value = Some(value.clamp(0.0, 1.0) as f64);
        element
    }

    pub(crate) fn overlay_elements_for_render(
        overlay: &DockOverlayScene,
    ) -> Vec<DockAccessibleElement> {
        DockAccessibilityScene {
            descriptors: Vec::new(),
        }
        .with_overlay(overlay)
        .gpui_elements(DockAccessibilityLayer::Overlay)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn orientation_for_axis(axis: SplitAxis) -> Orientation {
    match axis {
        SplitAxis::Horizontal => Orientation::Horizontal,
        SplitAxis::Vertical => Orientation::Vertical,
    }
}

impl DockAccessibilityDescriptor {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn gpui_element(
        &self,
        layer: DockAccessibilityLayer,
        order: usize,
    ) -> Option<DockAccessibleElement> {
        let role = ui_role_for_dock_role(self.role)?;
        let disabled = self.disabled.unwrap_or(false);
        let mut actions = self
            .actions
            .iter()
            .copied()
            .filter(|action| !disabled && gpui_supports_action(self.role, *action))
            .collect::<Vec<_>>();
        actions.sort();
        actions.dedup();

        let label = self
            .label
            .clone()
            .unwrap_or_else(|| fallback_label(self.role));
        Some(DockAccessibleElement {
            id: self.element_id(layer, order),
            role,
            gpui_role: gpui_role_for_ui_role(role),
            node: self.node,
            item: self.item.clone(),
            bounds: self.bounds,
            label,
            hint: hint_for_role(self.role, self.zone),
            zone: self.zone,
            active: self.active,
            orientation: self.orientation,
            selected: self.selected,
            disabled,
            numeric_value: self.numeric_value(),
            actions,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn element_id(
        &self,
        layer: DockAccessibilityLayer,
        order: usize,
    ) -> DockAccessibleElementId {
        let layer = match layer {
            DockAccessibilityLayer::Final => "final",
            DockAccessibilityLayer::Overlay => "overlay",
        };
        let role = self.role.id_segment();
        let node = self
            .node
            .map(|node| node.as_u64().to_string())
            .unwrap_or_else(|| "none".to_string());
        let item = self
            .item
            .as_ref()
            .map(|item| item.as_str().to_string())
            .unwrap_or_else(|| "none".to_string());
        let zone = self
            .zone
            .map(|zone| zone.id_segment().to_string())
            .unwrap_or_else(|| "none".to_string());
        DockAccessibleElementId(format!(
            "dock-a11y:{layer}:{role}:node-{node}:item-{item}:zone-{zone}:order-{order}"
        ))
    }

    fn numeric_value(&self) -> Option<f64> {
        (self.role == DockAccessibilityRole::Splitter).then_some(0.0)
    }
}

impl DockAccessibleElement {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn id_str(&self) -> &str {
        self.id.as_str()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn has_action(&self, action: AccessibleAction) -> bool {
        self.actions.contains(&action)
    }

    pub(crate) fn apply_to<E: StatefulInteractiveElement>(&self, mut element: E) -> E {
        element = element.role(self.gpui_role).aria_label(self.label.clone());
        if let Some(selected) = self.selected {
            element = element.aria_selected(selected);
        }
        element = element.aria_disabled(self.disabled);
        if let Some(orientation) = self.orientation {
            element = element.aria_orientation(gpui_orientation_from_ui(orientation));
        }
        if let Some(value) = self.numeric_value {
            element = element.aria_numeric_value(value);
        }
        element
    }
}

impl DockAccessibleElementId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DockAccessibleElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl DockAccessibilityRole {
    fn id_segment(self) -> &'static str {
        match self {
            DockAccessibilityRole::Pane => "pane",
            DockAccessibilityRole::TabList => "tab-list",
            DockAccessibilityRole::Tab => "tab",
            DockAccessibilityRole::TabPanel => "tab-panel",
            DockAccessibilityRole::Splitter => "splitter",
            DockAccessibilityRole::FloatingWindow => "floating-window",
            DockAccessibilityRole::FocusRegion => "focus-region",
            DockAccessibilityRole::DropTarget => "drop-target",
            DockAccessibilityRole::DragSource => "drag-source",
            DockAccessibilityRole::DropDestination => "drop-destination",
            DockAccessibilityRole::RejectedDropTarget => "rejected-drop-target",
        }
    }
}

impl DropZone {
    fn id_segment(self) -> &'static str {
        match self {
            DropZone::Center => "center",
            DropZone::Left => "left",
            DropZone::Right => "right",
            DropZone::Top => "top",
            DropZone::Bottom => "bottom",
        }
    }
}

pub(crate) fn ui_role_for_dock_role(role: DockAccessibilityRole) -> Option<Role> {
    match role {
        DockAccessibilityRole::Pane => Some(Role::Group),
        DockAccessibilityRole::TabList => Some(Role::TabList),
        DockAccessibilityRole::Tab => Some(Role::Tab),
        DockAccessibilityRole::TabPanel => Some(Role::TabPanel),
        DockAccessibilityRole::Splitter => Some(Role::Splitter),
        DockAccessibilityRole::FloatingWindow => Some(Role::Window),
        DockAccessibilityRole::FocusRegion => Some(Role::Group),
        DockAccessibilityRole::DropTarget => Some(Role::Group),
        DockAccessibilityRole::DragSource => Some(Role::Group),
        DockAccessibilityRole::DropDestination => Some(Role::Group),
        DockAccessibilityRole::RejectedDropTarget => Some(Role::Group),
    }
}

fn gpui_role_for_ui_role(role: Role) -> GpuiRole {
    match role {
        Role::Label => GpuiRole::Label,
        Role::Image => GpuiRole::Image,
        Role::Button => GpuiRole::Button,
        Role::Link => GpuiRole::Link,
        Role::CheckBox => GpuiRole::CheckBox,
        Role::Switch => GpuiRole::Switch,
        Role::RadioButton => GpuiRole::RadioButton,
        Role::RadioGroup => GpuiRole::RadioGroup,
        Role::Toolbar => GpuiRole::Toolbar,
        Role::Navigation => GpuiRole::Navigation,
        Role::Section => GpuiRole::Section,
        Role::Group => GpuiRole::Group,
        Role::Tree => GpuiRole::Tree,
        Role::TreeItem => GpuiRole::TreeItem,
        Role::Table => GpuiRole::Table,
        Role::Row => GpuiRole::Row,
        Role::ColumnHeader => GpuiRole::ColumnHeader,
        Role::Cell => GpuiRole::Cell,
        Role::ListBox => GpuiRole::ListBox,
        Role::ListBoxOption => GpuiRole::ListBoxOption,
        Role::Menu => GpuiRole::Menu,
        Role::MenuItem => GpuiRole::MenuItem,
        Role::TextInput => GpuiRole::TextInput,
        Role::EditableComboBox => GpuiRole::EditableComboBox,
        Role::Dialog => GpuiRole::Dialog,
        Role::AlertDialog => GpuiRole::AlertDialog,
        Role::Window => GpuiRole::Window,
        Role::ProgressIndicator => GpuiRole::ProgressIndicator,
        Role::Separator => GpuiRole::Group,
        Role::SpinButton => GpuiRole::SpinButton,
        Role::Slider => GpuiRole::Slider,
        Role::Splitter => GpuiRole::Splitter,
        Role::TabList => GpuiRole::TabList,
        Role::Tab => GpuiRole::Tab,
        Role::TabPanel => GpuiRole::TabPanel,
    }
}

pub(crate) fn gpui_accessible_action_from_ui(action: AccessibleAction) -> GpuiAccessibleAction {
    match action {
        AccessibleAction::Click => GpuiAccessibleAction::Click,
        AccessibleAction::Focus => GpuiAccessibleAction::Focus,
        AccessibleAction::Blur => GpuiAccessibleAction::Blur,
        AccessibleAction::Collapse => GpuiAccessibleAction::Collapse,
        AccessibleAction::Expand => GpuiAccessibleAction::Expand,
        AccessibleAction::CustomAction => GpuiAccessibleAction::CustomAction,
        AccessibleAction::Decrement => GpuiAccessibleAction::Decrement,
        AccessibleAction::Increment => GpuiAccessibleAction::Increment,
        AccessibleAction::HideTooltip => GpuiAccessibleAction::HideTooltip,
        AccessibleAction::ShowTooltip => GpuiAccessibleAction::ShowTooltip,
        AccessibleAction::ReplaceSelectedText => GpuiAccessibleAction::ReplaceSelectedText,
        AccessibleAction::ScrollDown => GpuiAccessibleAction::ScrollDown,
        AccessibleAction::ScrollLeft => GpuiAccessibleAction::ScrollLeft,
        AccessibleAction::ScrollRight => GpuiAccessibleAction::ScrollRight,
        AccessibleAction::ScrollUp => GpuiAccessibleAction::ScrollUp,
        AccessibleAction::ScrollIntoView => GpuiAccessibleAction::ScrollIntoView,
        AccessibleAction::ScrollToPoint => GpuiAccessibleAction::ScrollToPoint,
        AccessibleAction::SetScrollOffset => GpuiAccessibleAction::SetScrollOffset,
        AccessibleAction::SetTextSelection => GpuiAccessibleAction::SetTextSelection,
        AccessibleAction::SetSequentialFocusNavigationStartingPoint => {
            GpuiAccessibleAction::SetSequentialFocusNavigationStartingPoint
        }
        AccessibleAction::SetValue => GpuiAccessibleAction::SetValue,
        AccessibleAction::ShowContextMenu => GpuiAccessibleAction::ShowContextMenu,
    }
}

fn gpui_orientation_from_ui(orientation: Orientation) -> GpuiOrientation {
    match orientation {
        Orientation::Horizontal => GpuiOrientation::Horizontal,
        Orientation::Vertical => GpuiOrientation::Vertical,
    }
}

fn gpui_supports_action(role: DockAccessibilityRole, action: AccessibleAction) -> bool {
    matches!(
        (role, action),
        (DockAccessibilityRole::Pane, AccessibleAction::Focus)
            | (DockAccessibilityRole::Tab, AccessibleAction::Click)
            | (DockAccessibilityRole::Tab, AccessibleAction::Focus)
            | (DockAccessibilityRole::TabPanel, AccessibleAction::Focus)
            | (DockAccessibilityRole::Splitter, AccessibleAction::Increment)
            | (DockAccessibilityRole::Splitter, AccessibleAction::Decrement)
            | (
                DockAccessibilityRole::FloatingWindow,
                AccessibleAction::Focus
            )
            | (DockAccessibilityRole::FocusRegion, AccessibleAction::Focus)
    )
}

fn hint_for_role(role: DockAccessibilityRole, zone: Option<DropZone>) -> Option<String> {
    match role {
        DockAccessibilityRole::Tab => Some("Activate to select this tab".to_string()),
        DockAccessibilityRole::Splitter => Some("Resize adjacent dock panes".to_string()),
        DockAccessibilityRole::DropDestination | DockAccessibilityRole::DropTarget => {
            Some(format!("Drop target{}", zone_suffix(zone)))
        }
        DockAccessibilityRole::RejectedDropTarget => {
            Some("This dock target cannot accept the current payload".to_string())
        }
        DockAccessibilityRole::DragSource => Some("Drag this dock item".to_string()),
        DockAccessibilityRole::Pane
        | DockAccessibilityRole::TabList
        | DockAccessibilityRole::TabPanel
        | DockAccessibilityRole::FloatingWindow
        | DockAccessibilityRole::FocusRegion => None,
    }
}

fn zone_suffix(zone: Option<DropZone>) -> String {
    zone.map(|zone| format!(" for {}", zone_label(zone)))
        .unwrap_or_default()
}

fn fallback_label(role: DockAccessibilityRole) -> String {
    match role {
        DockAccessibilityRole::Pane => "Dock pane",
        DockAccessibilityRole::TabList => "Dock tabs",
        DockAccessibilityRole::Tab => "Dock tab",
        DockAccessibilityRole::TabPanel => "Dock panel",
        DockAccessibilityRole::Splitter => "Dock splitter",
        DockAccessibilityRole::FloatingWindow => "Floating window",
        DockAccessibilityRole::FocusRegion => "Focused dock pane",
        DockAccessibilityRole::DropTarget => "Dock drop target",
        DockAccessibilityRole::DragSource => "Dock drag source",
        DockAccessibilityRole::DropDestination => "Dock drop destination",
        DockAccessibilityRole::RejectedDropTarget => "Dock target unavailable",
    }
    .to_string()
}

fn title_for_item(scene: &DockPresentationScene, item: &DockItemId) -> String {
    scene
        .tab_labels
        .iter()
        .find(|tab| tab.item == *item)
        .map(|tab| tab.title.clone())
        .unwrap_or_else(|| item.to_string())
}

fn title_for_tabs(scene: &DockPresentationScene, tabs: Option<DockNodeId>) -> Option<String> {
    let tabs = tabs?;
    let titles = scene
        .tab_labels
        .iter()
        .filter(|tab| tab.tabs == tabs)
        .map(|tab| tab.title.as_str())
        .collect::<Vec<_>>();
    Some(match titles.as_slice() {
        [] => "Dock pane".to_string(),
        [title] => format!("{title} pane"),
        titles => format!("{} tabs pane", titles.len()),
    })
}

fn tab_list_label(scene: &DockPresentationScene, tabs: DockNodeId) -> String {
    let titles = scene
        .tab_labels
        .iter()
        .filter(|tab| tab.tabs == tabs)
        .map(|tab| tab.title.as_str())
        .collect::<Vec<_>>();
    match titles.as_slice() {
        [] => "Dock tabs".to_string(),
        [title] => format!("{title} tabs"),
        titles => format!("Dock tabs, {} items", titles.len()),
    }
}

fn splitter_label(axis: SplitAxis, index: usize) -> String {
    let axis = match axis {
        SplitAxis::Horizontal => "horizontal",
        SplitAxis::Vertical => "vertical",
    };
    format!("{axis} dock splitter {}", index + 1)
}

fn drop_target_label(zone: Option<DropZone>, node: Option<DockNodeId>) -> String {
    format!("Dock target{}{}", zone_suffix(zone), node_suffix(node))
}

fn drop_destination_label(zone: Option<DropZone>, node: Option<DockNodeId>) -> String {
    format!("Drop into dock{}{}", zone_suffix(zone), node_suffix(node))
}

fn drag_payload_label(title: Option<&str>) -> String {
    match title.filter(|title| !title.is_empty()) {
        Some(title) => format!("Dragging {title}"),
        None => "Dock drag payload".to_string(),
    }
}

fn node_suffix(node: Option<DockNodeId>) -> String {
    node.map(|node| format!(" node {}", node.as_u64()))
        .unwrap_or_default()
}

fn zone_label(zone: DropZone) -> &'static str {
    match zone {
        DropZone::Center => "tabs",
        DropZone::Left => "left side",
        DropZone::Right => "right side",
        DropZone::Top => "top side",
        DropZone::Bottom => "bottom side",
    }
}
