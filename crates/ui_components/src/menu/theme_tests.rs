use super::*;

use open_gpui::{AppContext as _, Context, Empty, Render};

struct MenuThemeBindingProbe;

impl Render for MenuThemeBindingProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

fn resolve_menu_state(open: bool, open_path: &[String]) -> MenuState {
    MenuState::resolve_with_paths(
        Size::Medium,
        false,
        Some(open),
        false,
        Some("branch"),
        None,
        open_path,
        [MenuItemDescriptor::submenu(
            "branch",
            "Branch",
            [MenuItemDescriptor::action("child", "Child")],
        )],
        OverlayPlacementSide::Bottom,
        OverlayPlacementAlignment::Start,
        OutsidePressPolicy::DismissAndConsume,
        EscapeKeyPolicy::Dismiss,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        ThemeTokens::default(),
    )
}

fn root_registration(state: &MenuState) -> OverlayLayerRegistration {
    OverlayLayerRegistration::new(
        "menu:opening-theme-test",
        state.overlay().policy().clone(),
        OverlayOwnership::Controlled,
    )
}

fn branch_opening_mode(
    bindings: &MenuBranchBindings,
    branch_path: &[String],
) -> Option<crate::theme::ThemeMode> {
    bindings
        .get(&menu_path_key(branch_path))
        .and_then(OverlayLayerBinding::opening_theme)
        .map(|theme| theme.mode())
}

#[open_gpui::test]
fn submenu_opening_theme_follows_the_root_generation(cx: &mut open_gpui::TestAppContext) {
    let window = cx.add_window(|_, _| MenuThemeBindingProbe);

    window
        .update(cx, |_, window, cx| {
            let owner = cx.new(|_| MenuRuntime::new(true, Some("branch".to_owned())));
            let overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
            let root_state = resolve_menu_state(true, &[]);
            let branch_path = root_state.items()[0].path().to_vec();

            crate::theme::override_window_theme(window, cx, ThemeContext::dark());
            let root_binding = overlay_runtime
                .bind_component_layer(
                    &owner,
                    None,
                    root_registration(&root_state),
                    window,
                    cx,
                )
                .expect("dark menu root should bind");
            let hidden_branches = sync_menu_branch_bindings(
                "menu",
                "opening-theme-test",
                &root_state,
                &owner,
                &overlay_runtime,
                &root_binding,
                window,
                cx,
            );
            assert_eq!(
                root_binding.opening_theme().map(|theme| theme.mode()),
                Some(crate::theme::ThemeMode::Dark)
            );
            assert_eq!(branch_opening_mode(&hidden_branches, &branch_path), None);

            crate::theme::override_window_theme(window, cx, ThemeContext::high_contrast());
            let root_binding = overlay_runtime
                .bind_component_layer(
                    &owner,
                    Some(&root_binding),
                    root_registration(&root_state),
                    window,
                    cx,
                )
                .expect("open root should retain its generation");
            let branch_state = resolve_menu_state(true, &branch_path);
            let dark_branches = sync_menu_branch_bindings(
                "menu",
                "opening-theme-test",
                &branch_state,
                &owner,
                &overlay_runtime,
                &root_binding,
                window,
                cx,
            );
            assert_eq!(
                branch_opening_mode(&dark_branches, &branch_path),
                Some(crate::theme::ThemeMode::Dark),
                "a submenu opened later in the same root generation must inherit the root opening theme"
            );

            let closed_state = resolve_menu_state(false, &[]);
            let root_binding = overlay_runtime
                .bind_component_layer(
                    &owner,
                    Some(&root_binding),
                    root_registration(&closed_state),
                    window,
                    cx,
                )
                .expect("menu root should close");
            let closed_branches = sync_menu_branch_bindings(
                "menu",
                "opening-theme-test",
                &closed_state,
                &owner,
                &overlay_runtime,
                &root_binding,
                window,
                cx,
            );
            assert_eq!(root_binding.opening_theme(), None);
            assert_eq!(branch_opening_mode(&closed_branches, &branch_path), None);

            let root_binding = overlay_runtime
                .bind_component_layer(
                    &owner,
                    Some(&root_binding),
                    root_registration(&branch_state),
                    window,
                    cx,
                )
                .expect("menu root should reopen under high contrast");
            let high_contrast_branches = sync_menu_branch_bindings(
                "menu",
                "opening-theme-test",
                &branch_state,
                &owner,
                &overlay_runtime,
                &root_binding,
                window,
                cx,
            );
            assert_eq!(
                root_binding.opening_theme().map(|theme| theme.mode()),
                Some(crate::theme::ThemeMode::HighContrast)
            );
            assert_eq!(
                branch_opening_mode(&high_contrast_branches, &branch_path),
                Some(crate::theme::ThemeMode::HighContrast),
                "a new root generation must recapture the current outer theme"
            );
        })
        .expect("menu theme test window should remain open");
}
