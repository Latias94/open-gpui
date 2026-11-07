//! Type-only composite accessibility, keyboard click and focus navigation consumers.
//!
//! These deliberately unexecuted closures use ordinary dependency features and public reexports.
//! Actual input, accepted accessibility publication and teardown belong to the framework's
//! crate-external `composite_focus` runtime target.

use open_gpui::{
    App, ClickEvent, Div, FocusHandle, InteractiveElement as _, KeyDownEvent, KeyUpEvent,
    KeyboardButton, Role, Stateful, StatefulInteractiveElement as _, TargetedEvent, Window,
    accesskit::{Node, NodeId, TreeUpdate},
    div,
};

#[test]
fn downstream_composite_active_descendant_and_synthetic_children_are_type_only() {
    let _: fn(Stateful<Div>) -> Stateful<Div> =
        <Stateful<Div> as open_gpui::StatefulInteractiveElement>::aria_active_descendant;
    let _composite_type_only = |container: &FocusHandle| -> Stateful<Div> {
        use open_gpui::ParentElement as _;

        div()
            .id("consumer-menu")
            .role(Role::Menu)
            .track_focus(container)
            .child(
                div()
                    .id("consumer-selected-item")
                    .role(Role::MenuItem)
                    .aria_active_descendant()
                    .a11y_synthetic_children(|builder| {
                        let id: NodeId = builder.synthetic_node_id("detail");
                        let mut detail = Node::new(Role::Label);
                        detail.set_label("Selected item detail");
                        let _: bool = builder.push_child(id, detail);
                    }),
            )
    };
    let _: fn(&Window) -> Option<String> = Window::debug_a11y_tree_json;
    let _publication_type_only =
        |window: &Window, update: &TreeUpdate| -> (NodeId, Option<String>) {
            (update.focus, window.debug_a11y_tree_json())
        };
    let _display_override_type_only = |display: &FocusHandle| -> Stateful<Div> {
        div()
            .id("consumer-display-only-menu")
            .role(Role::Menu)
            .track_accessibility_focus(display)
    };
}

#[test]
fn downstream_keyboard_click_and_container_navigation_signatures_are_type_only() {
    let _keyboard_type_only = |focus: &FocusHandle| -> Stateful<Div> {
        div()
            .id("consumer-trigger")
            .role(Role::Button)
            .track_focus(focus)
            .on_key_down(|_: &KeyDownEvent, _: &mut Window, _: &mut App| {})
            .on_key_up(|_: &KeyUpEvent, _: &mut Window, _: &mut App| {})
            .on_click(
                |event: &TargetedEvent<ClickEvent>, _: &mut Window, _: &mut App| {
                    let _: bool = event.window_event().is_keyboard();
                },
            )
    };
    let _tab_group_type_only = |container: &FocusHandle| -> Stateful<Div> {
        div()
            .id("consumer-group")
            .track_focus(container)
            .tab_group()
            .tab_index(1)
            .tab_stop(false)
    };
    let _: fn(&FocusHandle, &mut Window, &mut App) = FocusHandle::focus;
    let _: fn(&FocusHandle, &Window) -> bool = FocusHandle::is_focused;
    let _: fn(&mut Window, &mut App) = Window::focus_next;
    let _focus_type_only = |focus: &FocusHandle, window: &mut Window, app: &mut App| {
        focus.focus(window, app);
        let _: bool = focus.is_focused(window);
        window.focus_next(app);
    };
    let _: [KeyboardButton; 2] = [KeyboardButton::Enter, KeyboardButton::Space];
}
