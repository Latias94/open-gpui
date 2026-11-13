use std::rc::Rc;

use open_gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, Styled, Window, div,
};

use super::ListboxOptionState;

pub(crate) type ListboxRowContentRenderer =
    Rc<dyn Fn(&ListboxOptionState, &mut Window, &mut App) -> AnyElement>;

const ROW_ACTION_KEY_CONTEXT: &str = "ListboxRowAction";

pub(super) fn row_action_focused(window: &Window) -> bool {
    window
        .context_stack()
        .iter()
        .any(|context| context.contains(ROW_ACTION_KEY_CONTEXT))
}

/// Isolates an interactive child from its listbox row's pointer and keyboard activation.
///
/// Wrap buttons, menus, and editors returned by [`super::Listbox::row_content`] or
/// [`crate::combobox::Combobox::option_content`] in this container. Child handlers
/// run first; unconsumed mouse-down, mouse-up, and non-character keys stop here.
/// Its key context excludes listbox activation, navigation, and typeahead while
/// printable keys continue to the native text input owner.
/// The wrapper leaves default behavior, child focus, and accessibility intact.
///
/// Child controls retain their own disabled policy. Pass the option's disabled
/// state to a child that should be unavailable with its row. Passive thumbnails,
/// labels, and metadata do not need this wrapper and remain part of row activation.
#[derive(IntoElement)]
pub struct ListboxRowAction {
    id: ElementId,
    content: AnyElement,
}

impl ListboxRowAction {
    /// Wraps child interaction without adding another semantic activation owner.
    pub fn new(id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for ListboxRowAction {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id(self.id)
            .flex_none()
            .key_context(ROW_ACTION_KEY_CONTEXT)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(|event, _, cx| {
                if event.keystroke.key_char.is_none() && !event.prefer_character_input {
                    cx.stop_propagation();
                }
            })
            .on_key_up(|_, _, cx| cx.stop_propagation())
            .child(self.content)
    }
}
