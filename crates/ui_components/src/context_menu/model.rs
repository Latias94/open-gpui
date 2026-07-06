use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    Role, Size, ThemeTokens, UiPoint, ui_px, ui_size,
};

use crate::menu::{MenuColors, MenuItemDescriptor, MenuMetrics, MenuOpenMode, MenuState};
use crate::overlay::OverlayResolvedState;
/// Resolved context-menu state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuState {
    size: Size,
    open: bool,
    default_open: bool,
    open_mode: MenuOpenMode,
    anchor_point: UiPoint,
    menu: MenuState,
    placement_input: OverlayPlacementInput,
}

impl ContextMenuState {
    /// Resolves public state for a point-anchored context menu.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        open: Option<bool>,
        default_open: bool,
        anchor_point: UiPoint,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = MenuItemDescriptor>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<MenuItemDescriptor> = items.into_iter().collect();
        let menu = MenuState::resolve(
            size,
            false,
            open,
            default_open,
            focused_value,
            descriptors,
            OverlayPlacementSide::Bottom,
            OverlayPlacementAlignment::Start,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        );

        Self::from_menu(size, default_open, anchor_point, menu)
    }

    /// Resolves context-menu state with adapter-owned submenu and focus paths applied.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_with_paths(
        size: Size,
        open: Option<bool>,
        default_open: bool,
        anchor_point: UiPoint,
        focused_value: Option<&str>,
        focused_path: Option<&[String]>,
        open_path: &[String],
        items: impl IntoIterator<Item = MenuItemDescriptor>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<MenuItemDescriptor> = items.into_iter().collect();
        let menu = MenuState::resolve_with_paths(
            size,
            false,
            open,
            default_open,
            focused_value,
            focused_path,
            open_path,
            descriptors,
            OverlayPlacementSide::Bottom,
            OverlayPlacementAlignment::Start,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        );

        Self::from_menu(size, default_open, anchor_point, menu)
    }

    fn from_menu(size: Size, default_open: bool, anchor_point: UiPoint, menu: MenuState) -> Self {
        let placement_input = context_menu_placement_input(anchor_point, &menu);

        Self {
            size,
            open: menu.open(),
            default_open,
            open_mode: menu.open_mode(),
            anchor_point,
            menu,
            placement_input,
        }
    }

    /// Returns context-menu size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether context-menu content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> MenuOpenMode {
        self.open_mode
    }

    /// Returns the point anchor.
    pub const fn anchor_point(&self) -> UiPoint {
        self.anchor_point
    }

    /// Returns the shared menu state.
    pub const fn menu(&self) -> &MenuState {
        &self.menu
    }

    /// Returns renderer-neutral placement input for the context-menu surface.
    pub const fn placement_input(&self) -> OverlayPlacementInput {
        self.placement_input
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        self.menu.overlay()
    }

    /// Returns resolved menu metrics.
    pub const fn metrics(&self) -> MenuMetrics {
        self.menu.metrics()
    }

    /// Returns resolved menu colors.
    pub const fn colors(&self) -> MenuColors {
        self.menu.colors()
    }

    /// Returns content accessibility role.
    pub const fn content_role(&self) -> Role {
        Role::Menu
    }
}

fn context_menu_placement_input(anchor_point: UiPoint, menu: &MenuState) -> OverlayPlacementInput {
    let metrics = menu.metrics();
    let visible_count = menu.visible_items().len();
    let row_gap = ui_px(4.0);
    let gap_height = row_gap.as_f32() * visible_count.saturating_sub(1) as f32;
    let content_height = ui_px(metrics.max_height().as_f32().min(
        metrics.surface_padding().as_f32() * 2.0
            + metrics.item_height().as_f32() * visible_count as f32
            + gap_height,
    ));

    OverlayPlacementInput::new(
        OverlayAnchorInput::from_point(anchor_point),
        ui_size(metrics.min_width(), content_height),
    )
    .with_side(OverlayPlacementSide::Bottom)
    .with_alignment(OverlayPlacementAlignment::Start)
    .with_offset(ui_px(0.0))
}
