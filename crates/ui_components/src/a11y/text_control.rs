use std::{collections::HashSet, ops::Range, rc::Rc};

use open_gpui::{App, Context, Entity, EntityInputHandler, Window, accesskit};
use open_gpui_ui_core::{
    AccessibleAction, AccessibleTextPosition, AccessibleTextSelection, Role, SemanticDescriptor,
};

use crate::{form_control::FormControlState, text_editing};

const TEXT_FOCUS_ACTIONS: &[AccessibleAction] = &[AccessibleAction::Focus];
const TEXT_VALUE_ACTIONS: &[AccessibleAction] =
    &[AccessibleAction::Focus, AccessibleAction::SetValue];
const TEXT_SELECTION_ACTIONS: &[AccessibleAction] = &[
    AccessibleAction::Focus,
    AccessibleAction::ReplaceSelectedText,
    AccessibleAction::SetTextSelection,
    AccessibleAction::SetValue,
];

/// Owned, ephemeral accessibility projection for a resolved text control.
///
/// Components derive this value on demand from their resolved state. It owns policy-projected text
/// so password masking cannot be bypassed by a caller-supplied descriptor value.
#[derive(Debug, Clone, PartialEq)]
pub struct TextControlSemanticProjection<NodeId = std::convert::Infallible> {
    role: Role,
    value: String,
    placeholder: Option<String>,
    control: FormControlState,
    exposes_text_runs: bool,
    text_selection: Option<AccessibleTextSelection<NodeId>>,
}

impl<NodeId> TextControlSemanticProjection<NodeId> {
    pub(crate) fn new(
        role: Role,
        value: String,
        placeholder: Option<&str>,
        control: FormControlState,
        exposes_text_runs: bool,
    ) -> Self {
        Self {
            role,
            value,
            placeholder: placeholder
                .filter(|placeholder| !placeholder.is_empty())
                .map(str::to_owned),
            control,
            exposes_text_runs,
            text_selection: None,
        }
    }

    /// Returns the policy-projected accessible value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns whether this control can expose fine-grained TextRun semantics.
    pub const fn exposes_text_runs(&self) -> bool {
        self.exposes_text_runs
    }

    /// Borrows this projection as the renderer-neutral semantic descriptor.
    pub fn descriptor(&self) -> SemanticDescriptor<'_, NodeId> {
        let mut descriptor = text_control_semantic_descriptor(
            self.role,
            self.value.as_str(),
            self.control,
            self.exposes_text_runs,
            self.text_selection.as_ref(),
        );
        if let Some(placeholder) = self.placeholder.as_deref() {
            descriptor = descriptor.with_placeholder(placeholder);
        }
        descriptor
    }

    pub(crate) fn with_text_selection(
        mut self,
        text_selection: Option<AccessibleTextSelection<NodeId>>,
    ) -> Self {
        if self.exposes_text_runs && self.control.controller_driven() {
            self.text_selection = text_selection;
        }
        self
    }
}

fn text_control_semantic_descriptor<'a, NodeId>(
    role: Role,
    semantic_value: &'a str,
    control: FormControlState,
    exposes_text_runs: bool,
    text_selection: Option<&'a AccessibleTextSelection<NodeId>>,
) -> SemanticDescriptor<'a, NodeId> {
    let actions = if !control.controller_driven() {
        TEXT_FOCUS_ACTIONS
    } else if exposes_text_runs {
        TEXT_SELECTION_ACTIONS
    } else {
        TEXT_VALUE_ACTIONS
    };
    let mut descriptor = SemanticDescriptor::new(role)
        .with_value(semantic_value)
        .with_required(control.required())
        .with_invalid(control.invalid())
        .with_busy(control.busy())
        .with_read_only(control.read_only())
        .with_disabled(control.disabled())
        .with_actions(actions);
    if exposes_text_runs {
        if let Some(text_selection) = text_selection {
            descriptor = descriptor.with_text_selection(text_selection);
        }
    }
    descriptor
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessibleTextReplacementTarget {
    SelectedText,
    EntireValue,
}

pub(crate) trait AccessibleTextInputHandler: EntityInputHandler {
    fn value(&self) -> &str;
    fn selected_range_bytes(&self) -> Range<usize>;
    fn selection_reversed(&self) -> bool;
    fn accepts_accessible_selection(&self) -> bool;
    fn set_accessible_selection(&mut self, anchor: usize, focus: usize, cx: &mut Context<Self>);

    fn value_utf16_len(&self) -> usize {
        self.value().encode_utf16().count()
    }

    fn selected_range_utf16(&self) -> Range<usize> {
        text_editing::range_to_utf16(self.value(), &self.selected_range_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccessibleTextRunRange {
    node_id: accesskit::NodeId,
    byte_range: Range<usize>,
    character_lengths: Rc<[u8]>,
}

impl AccessibleTextRunRange {
    pub(crate) fn from_text(
        node_id: accesskit::NodeId,
        byte_range: Range<usize>,
        value: &str,
    ) -> Option<Self> {
        let run_value = value.get(byte_range.clone())?;
        let character_lengths = text_editing::accessible_character_lengths(run_value)?.into();
        Self::from_character_lengths(node_id, byte_range, character_lengths)
    }

    pub(crate) fn from_character_lengths(
        node_id: accesskit::NodeId,
        byte_range: Range<usize>,
        character_lengths: Rc<[u8]>,
    ) -> Option<Self> {
        let byte_len = byte_range.end.checked_sub(byte_range.start)?;
        if character_lengths.iter().any(|length| *length == 0) {
            return None;
        }
        let represented_len = character_lengths
            .iter()
            .try_fold(0_usize, |total, length| {
                total.checked_add(usize::from(*length))
            })?;
        (represented_len == byte_len).then_some(Self {
            node_id,
            byte_range,
            character_lengths,
        })
    }

    pub(crate) fn character_lengths(&self) -> &[u8] {
        &self.character_lengths
    }

    pub(crate) fn character_index_from_offset(&self, offset: usize) -> Option<usize> {
        let local_offset = offset.checked_sub(self.byte_range.start)?;
        let mut represented_len = 0_usize;
        for (index, length) in self.character_lengths.iter().enumerate() {
            if represented_len == local_offset {
                return Some(index);
            }
            represented_len = represented_len.checked_add(usize::from(*length))?;
        }
        (represented_len == local_offset).then_some(self.character_lengths.len())
    }

    pub(crate) fn offset_from_character_index(&self, index: usize) -> Option<usize> {
        if index > self.character_lengths.len() {
            return None;
        }
        let local_offset = self.character_lengths[..index]
            .iter()
            .try_fold(0_usize, |total, length| {
                total.checked_add(usize::from(*length))
            })?;
        self.byte_range.start.checked_add(local_offset)
    }
}

pub(crate) fn dispatch_accessible_text_selection<T: AccessibleTextInputHandler>(
    controller: &Entity<T>,
    published_value: &str,
    data: Option<&accesskit::ActionData>,
    text_run_id: accesskit::NodeId,
    cx: &mut App,
) {
    let Some(text_run) =
        AccessibleTextRunRange::from_text(text_run_id, 0..published_value.len(), published_value)
    else {
        return;
    };
    let text_runs = [text_run];
    dispatch_accessible_text_selection_in_runs(controller, published_value, data, &text_runs, cx);
}

pub(crate) fn project_accessible_text_selection_in_runs<T: AccessibleTextInputHandler>(
    controller: &T,
    text_runs: &[AccessibleTextRunRange],
) -> Option<AccessibleTextSelection<accesskit::NodeId>> {
    let value = controller.value();
    if !text_runs_cover_value(value, text_runs) {
        return None;
    }

    let range = controller.selected_range_bytes();
    let (anchor, focus) = if controller.selection_reversed() {
        (range.end, range.start)
    } else {
        (range.start, range.end)
    };
    Some(AccessibleTextSelection::new(
        accessible_text_position_from_offset(value, anchor, text_runs)?,
        accessible_text_position_from_offset(value, focus, text_runs)?,
    ))
}

pub(crate) fn dispatch_accessible_text_selection_in_runs<T: AccessibleTextInputHandler>(
    controller: &Entity<T>,
    published_value: &str,
    data: Option<&accesskit::ActionData>,
    text_runs: &[AccessibleTextRunRange],
    cx: &mut App,
) {
    let Some(accesskit::ActionData::SetTextSelection(selection)) = data else {
        return;
    };
    controller.update(cx, |controller, cx| {
        if !controller.accepts_accessible_selection() || controller.value() != published_value {
            return;
        }
        let value = controller.value();
        if !text_runs_cover_value(value, text_runs) {
            return;
        }
        let Some(anchor) = accessible_text_offset_from_position(value, selection.anchor, text_runs)
        else {
            return;
        };
        let Some(focus) = accessible_text_offset_from_position(value, selection.focus, text_runs)
        else {
            return;
        };
        controller.set_accessible_selection(anchor, focus, cx);
    });
}

pub(super) fn text_runs_cover_value(value: &str, text_runs: &[AccessibleTextRunRange]) -> bool {
    if text_runs.is_empty() {
        return false;
    }

    let mut expected_start = 0;
    let mut node_ids = HashSet::with_capacity(text_runs.len());
    for text_run in text_runs {
        if text_run.byte_range.start != expected_start
            || text_run.byte_range.end < text_run.byte_range.start
            || !node_ids.insert(text_run.node_id)
        {
            return false;
        }
        let Some(_) = value.get(text_run.byte_range.clone()) else {
            return false;
        };
        expected_start = text_run.byte_range.end;
    }
    expected_start == value.len()
}

fn accessible_text_position_from_offset(
    value: &str,
    offset: usize,
    text_runs: &[AccessibleTextRunRange],
) -> Option<AccessibleTextPosition<accesskit::NodeId>> {
    if offset > value.len() {
        return None;
    }

    text_runs
        .iter()
        .enumerate()
        .find_map(|(index, text_run)| {
            let is_last = index + 1 == text_runs.len();
            (offset >= text_run.byte_range.start
                && (offset < text_run.byte_range.end
                    || (is_last && offset == text_run.byte_range.end)))
                .then_some(text_run)
        })
        .and_then(|text_run| {
            Some(AccessibleTextPosition::new(
                text_run.node_id,
                text_run.character_index_from_offset(offset)?,
            ))
        })
}

fn accessible_text_offset_from_position(
    value: &str,
    position: accesskit::TextPosition,
    text_runs: &[AccessibleTextRunRange],
) -> Option<usize> {
    let text_run = text_runs
        .iter()
        .find(|text_run| text_run.node_id == position.node)?;
    let _ = value.get(text_run.byte_range.clone())?;
    text_run.offset_from_character_index(position.character_index)
}

pub(crate) fn dispatch_accessible_text_replacement<T: AccessibleTextInputHandler>(
    controller: &Entity<T>,
    data: Option<&accesskit::ActionData>,
    target: AccessibleTextReplacementTarget,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(accesskit::ActionData::Value(value)) = data else {
        return;
    };

    controller.update(cx, |controller, cx| {
        let range = match target {
            AccessibleTextReplacementTarget::SelectedText => {
                Some(controller.selected_range_utf16())
            }
            AccessibleTextReplacementTarget::EntireValue => Some(0..controller.value_utf16_len()),
        };
        EntityInputHandler::replace_text_in_range(controller, range, value, window, cx);
    });
}
