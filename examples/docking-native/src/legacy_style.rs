use open_gpui::{Rgba, rgb};
use open_gpui_docking::legacy::{DockVisualPalette, DockVisualStyle, DockVisualStyleResolver};
use open_gpui_ui_components::{
    ColorState,
    theme::{ThemeResolver, ThemeSnapshot},
};
use open_gpui_ui_core::{TokenKey, semantic};

pub(super) fn dock_visual_style_from_theme(theme: &ThemeSnapshot) -> DockVisualStyle {
    let mut palette = DockVisualPalette::built_in();
    palette.surface = theme_color(
        theme,
        semantic::SURFACE,
        ColorState::Default,
        palette.surface,
    );
    palette.surface_muted = theme_color(
        theme,
        semantic::SURFACE_MUTED,
        ColorState::Default,
        palette.surface_muted,
    );
    palette.surface_hovered = theme_color(
        theme,
        semantic::SURFACE_MUTED,
        ColorState::Hover,
        palette.surface_hovered,
    );
    palette.surface_disabled = theme_color(
        theme,
        semantic::SURFACE_MUTED,
        ColorState::Disabled,
        palette.surface_disabled,
    );
    palette.border = theme_color(theme, semantic::BORDER, ColorState::Default, palette.border);
    palette.text = theme_color(theme, semantic::TEXT, ColorState::Default, palette.text);
    palette.text_muted = theme_color(
        theme,
        semantic::TEXT_MUTED,
        ColorState::Default,
        palette.text_muted,
    );
    palette.text_disabled = theme_color(
        theme,
        semantic::TEXT,
        ColorState::Disabled,
        palette.text_disabled,
    );
    palette.accent = theme_color(theme, semantic::ACCENT, ColorState::Default, palette.accent);
    palette.accent_hovered = theme_color(
        theme,
        semantic::ACCENT,
        ColorState::Hover,
        palette.accent_hovered,
    );
    palette.accent_foreground = theme_color(
        theme,
        semantic::ACCENT_FOREGROUND,
        ColorState::Default,
        palette.accent_foreground,
    );
    palette.focus_ring = theme_color(
        theme,
        semantic::FOCUS_RING,
        ColorState::FocusVisible,
        palette.focus_ring,
    );
    palette.destructive = theme_color(
        theme,
        semantic::DESTRUCTIVE,
        ColorState::Default,
        palette.destructive,
    );
    palette.destructive_foreground = theme_color(
        theme,
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        palette.destructive_foreground,
    );
    let shadow = theme_color(
        theme,
        semantic::OVERLAY,
        ColorState::Overlay,
        palette.shadow,
    );
    palette.shadow = Rgba {
        a: palette.shadow.a,
        ..shadow
    };
    DockVisualStyle::from_palette(palette)
}

fn theme_color(theme: &ThemeSnapshot, token: TokenKey, state: ColorState, fallback: Rgba) -> Rgba {
    theme.color_rgb(token, state).map(rgb).unwrap_or(fallback)
}

pub(super) fn dock_visual_style_resolver() -> DockVisualStyleResolver {
    DockVisualStyleResolver::new(|window, cx| {
        dock_visual_style_from_theme(&ThemeResolver::current_snapshot(window, cx))
    })
}
