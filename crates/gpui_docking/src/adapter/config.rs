//! Application-owned chrome and input preferences.

use open_gpui::{BoxShadow, Font, Hsla, Keystroke, Modifiers, Pixels, font, px};

use crate::{DockVisualPalette, DockVisualStyle};

/// Chrome appearance and gesture bindings for one retained dockspace.
///
/// Geometry, drag thresholds, and docking policies remain configured on the
/// renderer-neutral session.
#[derive(Clone, Debug)]
pub struct GpuiDockspaceConfig {
    /// Font used for both intrinsic measurement and chrome painting.
    pub font: Font,
    /// Text size used for both intrinsic measurement and chrome painting.
    pub text_size: Pixels,
    /// Chrome colors and elevation layers; pane content keeps its own styling.
    pub colors: DockspaceColors,
    /// Keyboard bindings observed during pointer gestures.
    pub input: DockspaceInputBindings,
}

impl Default for GpuiDockspaceConfig {
    fn default() -> Self {
        Self {
            font: font("Arial"),
            text_size: px(13.0),
            colors: DockspaceColors::default(),
            input: DockspaceInputBindings::default(),
        }
    }
}

/// Colors and elevation layers of the chrome painted by the adapter.
///
/// Constructors map the adapter's visual inputs from the shared [`DockVisualStyle`]
/// into an owned snapshot. Floating shadows retain every supplied layer.
#[derive(Clone, Debug)]
pub struct DockspaceColors {
    /// Unoccupied surface background.
    pub background: Hsla,
    /// Pane content backdrop.
    pub pane: Hsla,
    /// Tab strip background.
    pub tab_bar: Hsla,
    /// Unselected tab background.
    pub tab: Hsla,
    /// Selected tab background.
    pub selected_tab: Hsla,
    /// Inactive tab under the actual GPUI pointer target.
    pub hovered_tab: Hsla,
    /// Primary chrome text.
    pub text: Hsla,
    /// Inactive chrome text.
    pub muted_text: Hsla,
    /// Chrome outline.
    pub border: Hsla,
    /// Visible splitter fill.
    pub splitter: Hsla,
    /// Contained floating title background.
    pub contained_title: Hsla,
    /// Selection and active gesture accent.
    pub accent: Hsla,
    /// Available drop guide background.
    pub guide: Hsla,
    /// Active drop guide background.
    pub active_guide: Hsla,
    /// Unavailable drop guide background.
    pub disabled_guide: Hsla,
    /// Projected drop destination fill.
    pub preview: Hsla,
    /// Dragged tab decoration background.
    pub drag_ghost: Hsla,
    /// Complete contained floating elevation layers, in paint order.
    pub shadow: Vec<BoxShadow>,
}

impl Default for DockspaceColors {
    fn default() -> Self {
        Self::from_visual_style(&DockVisualStyle::built_in())
    }
}

impl DockspaceColors {
    /// Derives adapter chrome from a complete semantic palette.
    ///
    /// Uses [`DockVisualStyle::from_palette`] to share the existing color and
    /// elevation derivation rules.
    pub fn from_palette(palette: DockVisualPalette) -> Self {
        Self::from_visual_style(&DockVisualStyle::from_palette(palette))
    }

    /// Maps the visual inputs used by the adapter from a complete Dock style.
    ///
    /// Guide backgrounds use the center-guide palettes, and unavailable guides
    /// use the rejected-target body background. The selection and gesture accent
    /// uses the accepted-target border. Floating elevation layers are cloned in
    /// full, preserving their colors, geometry, and order.
    pub fn from_visual_style(style: &DockVisualStyle) -> Self {
        Self {
            background: style.host.background.into(),
            pane: style.tabs.frame_background.into(),
            tab_bar: style.tabs.strip_background.into(),
            tab: style.tabs.idle.background.into(),
            selected_tab: style.tabs.selected.background.into(),
            hovered_tab: style.tabs.hovered.background.into(),
            text: style.host.foreground.into(),
            muted_text: style.tabs.idle.text.into(),
            border: style.tabs.frame_border.into(),
            splitter: style.splitters.idle.into(),
            contained_title: style.floating.title_background.into(),
            accent: style.previews.accepted_target.border.into(),
            guide: style.previews.center_idle.background.into(),
            active_guide: style.previews.center_active.background.into(),
            disabled_guide: style.previews.rejected_target.body_background.into(),
            preview: style.previews.accepted_target.body_background.into(),
            drag_ghost: style.drag.background.into(),
            shadow: style.floating.shadow.clone(),
        }
    }
}

/// Bindings applied to event-time modifiers, before input is queued.
#[derive(Clone, Debug)]
pub struct DockspaceInputBindings {
    /// Holding these modifiers suspends docking. Extra modifiers are allowed.
    /// `None` disables this binding.
    pub suspend_docking: Option<Modifiers>,
    /// Cancels a captured gesture when the logical key matches and all required
    /// modifiers are held. Extra modifiers are allowed, so cancellation remains
    /// available while docking is suspended. `None` disables this binding.
    pub cancel: Option<Keystroke>,
}

impl DockspaceInputBindings {
    pub(crate) fn docking_enabled(&self, modifiers: Modifiers) -> bool {
        !self
            .suspend_docking
            .is_some_and(|binding| binding.is_subset_of(&modifiers))
    }

    pub(crate) fn cancel_matches(&self, keystroke: &Keystroke) -> bool {
        self.cancel.as_ref().is_some_and(|binding| {
            binding.key == keystroke.key && binding.modifiers.is_subset_of(&keystroke.modifiers)
        })
    }
}

impl Default for DockspaceInputBindings {
    fn default() -> Self {
        Self {
            suspend_docking: Some(Modifiers {
                shift: true,
                ..Modifiers::default()
            }),
            cancel: Some(Keystroke {
                modifiers: Modifiers::default(),
                key: "escape".into(),
                key_char: None,
            }),
        }
    }
}
