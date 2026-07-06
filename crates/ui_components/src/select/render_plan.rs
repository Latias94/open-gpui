use open_gpui::ElementId;
use open_gpui_ui_core::Role;

use crate::focus::FocusRing;

use super::{SelectColors, SelectMetrics, SelectState};

#[derive(Debug, Clone)]
pub(super) struct SelectRenderPlan {
    pub(super) root_id: ElementId,
    pub(super) debug_id: String,
    pub(super) trigger_id: ElementId,
    pub(super) content_id: ElementId,
    pub(super) listbox_id: ElementId,
    pub(super) metrics: SelectMetrics,
    pub(super) colors: SelectColors,
    pub(super) focus_ring: FocusRing,
    pub(super) disabled: bool,
    pub(super) open: bool,
    pub(super) selected: bool,
    pub(super) trigger_role: Role,
    pub(super) trigger_selected: bool,
    pub(super) trigger_label: String,
}

impl SelectRenderPlan {
    pub(super) fn from_state(root_id: ElementId, state: &SelectState) -> Self {
        Self {
            debug_id: root_id.to_string(),
            trigger_id: (root_id.clone(), "trigger").into(),
            content_id: (root_id.clone(), "content").into(),
            listbox_id: (root_id.clone(), "listbox").into(),
            root_id,
            metrics: state.metrics(),
            colors: state.colors(),
            focus_ring: state.focus_ring(),
            disabled: state.disabled(),
            open: state.open(),
            selected: state.selected_value().is_some(),
            trigger_role: state.trigger_role(),
            trigger_selected: state.trigger_selected(),
            trigger_label: state.trigger_label().to_owned(),
        }
    }
}
