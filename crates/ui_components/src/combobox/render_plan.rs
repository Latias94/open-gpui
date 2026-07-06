use open_gpui::ElementId;
use open_gpui_ui_core::{Role, UiPx};

use crate::focus::FocusRing;

use super::{ComboboxColors, ComboboxMetrics, ComboboxState};

#[derive(Debug, Clone)]
pub(super) struct ComboboxRenderPlan {
    pub(super) root_id: ElementId,
    pub(super) debug_id: String,
    pub(super) input_id: ElementId,
    pub(super) input_row_id: ElementId,
    pub(super) toggle_id: ElementId,
    pub(super) content_id: ElementId,
    pub(super) listbox_id: ElementId,
    pub(super) metrics: ComboboxMetrics,
    pub(super) colors: ComboboxColors,
    pub(super) focus_ring: FocusRing,
    pub(super) open: bool,
    pub(super) disabled: bool,
    pub(super) input_role: Role,
    pub(super) label: String,
    pub(super) placeholder: String,
    pub(super) input_height: UiPx,
    pub(super) input_radius: UiPx,
}

impl ComboboxRenderPlan {
    pub(super) fn from_state(root_id: ElementId, state: &ComboboxState) -> Self {
        Self {
            debug_id: root_id.to_string(),
            input_id: (root_id.clone(), "input").into(),
            input_row_id: (root_id.clone(), "input-row").into(),
            toggle_id: (root_id.clone(), "toggle").into(),
            content_id: (root_id.clone(), "content").into(),
            listbox_id: (root_id.clone(), "listbox").into(),
            root_id,
            metrics: state.metrics(),
            colors: state.colors(),
            focus_ring: state.focus_ring(),
            open: state.open(),
            disabled: state.disabled(),
            input_role: state.input_role(),
            label: state.label().to_owned(),
            placeholder: state.placeholder().to_owned(),
            input_height: state.input().metrics().height(),
            input_radius: state.input().metrics().radius(),
        }
    }
}
