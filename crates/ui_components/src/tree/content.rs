use std::rc::Rc;

use open_gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, Styled, Window, div,
};

use super::{TreeItemState, TreeRowAppearance};

type TreeRowContentRenderer = Rc<dyn Fn(&TreeItemState, &mut Window, &mut App) -> AnyElement>;
type TreeRowAppearanceResolver =
    Rc<dyn Fn(&TreeItemState, TreeRowAppearance, &mut Window, &mut App) -> TreeRowAppearance>;

#[derive(Clone, Default)]
pub(super) struct TreeRowPresentation {
    pub(super) content: Option<TreeRowContentRenderer>,
    pub(super) appearance: Option<TreeRowAppearanceResolver>,
}

impl TreeRowPresentation {
    pub(super) fn render_content(
        &self,
        item: &TreeItemState,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        if let Some(render) = self.content.as_ref() {
            render(item, window, cx)
        } else {
            div().child(item.label().to_owned()).into_any_element()
        }
    }

    pub(super) fn resolve_appearance(
        &self,
        item: &TreeItemState,
        window: &mut Window,
        cx: &mut App,
    ) -> TreeRowAppearance {
        let appearance = TreeRowAppearance::from_item(item);
        if let Some(resolve) = self.appearance.as_ref() {
            resolve(item, appearance, window, cx)
        } else {
            appearance
        }
    }
}

/// Isolates an interactive child from its tree row's selection, keys, and drag gesture.
///
/// Wrap row buttons, menus, or editors in this container inside [`super::Tree::row_content`].
/// The child retains its own focus, semantic activation, and accessibility node.
#[derive(IntoElement)]
pub struct TreeRowAction {
    id: ElementId,
    content: AnyElement,
}

impl TreeRowAction {
    /// Creates an independent row action without adding a second activation owner.
    pub fn new(id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for TreeRowAction {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id(self.id)
            .flex_none()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.content)
    }
}
