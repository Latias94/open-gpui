//! Adaptive layout vocabulary for the Open GPUI component ecosystem.

use crate::geometry::{UiPx, ui_px};
use crate::overlay::OverlayEdges as Edges;
use crate::sizing::Density;

/// Explicit query-source selector for responsive surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaptiveQuerySource {
    /// The local container determines the branch.
    Container,
    /// The current viewport or window determines the branch.
    Viewport,
}

/// Coarse device-shell classification derived from viewport width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceAdaptiveClass {
    /// Compact shell.
    #[default]
    Compact,
    /// Regular shell.
    Regular,
    /// Expanded shell.
    Expanded,
}

impl DeviceAdaptiveClass {
    /// Returns the preferred density for this device class.
    pub const fn density(self) -> Density {
        match self {
            Self::Compact => Density::Compact,
            Self::Regular => Density::Comfortable,
            Self::Expanded => Density::Spacious,
        }
    }
}

/// Coarse panel/container classification derived from container width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelAdaptiveClass {
    /// Compact panel.
    #[default]
    Compact,
    /// Medium panel.
    Medium,
    /// Wide panel.
    Wide,
}

/// Shared policy for device-shell classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceAdaptivePolicy {
    /// The minimum width at which the shell becomes regular.
    pub regular_min_width: UiPx,
    /// The minimum width at which the shell becomes expanded.
    pub expanded_min_width: UiPx,
    /// The fallback hover capability when the input modality is unknown.
    pub default_can_hover_when_unknown: bool,
    /// The fallback pointer type when the input modality is unknown.
    pub default_coarse_pointer_when_unknown: bool,
}

impl Default for DeviceAdaptivePolicy {
    fn default() -> Self {
        Self {
            regular_min_width: ui_px(768.0),
            expanded_min_width: ui_px(1280.0),
            default_can_hover_when_unknown: true,
            default_coarse_pointer_when_unknown: false,
        }
    }
}

impl DeviceAdaptivePolicy {
    /// Sets the regular-shell threshold.
    pub const fn regular_min_width(mut self, width: UiPx) -> Self {
        self.regular_min_width = width;
        self
    }

    /// Sets the expanded-shell threshold.
    pub const fn expanded_min_width(mut self, width: UiPx) -> Self {
        self.expanded_min_width = width;
        self
    }

    /// Sets the fallback hover capability.
    pub const fn default_can_hover_when_unknown(mut self, value: bool) -> Self {
        self.default_can_hover_when_unknown = value;
        self
    }

    /// Sets the fallback pointer type.
    pub const fn default_coarse_pointer_when_unknown(mut self, value: bool) -> Self {
        self.default_coarse_pointer_when_unknown = value;
        self
    }

    /// Classifies a width using this policy.
    pub fn classify(self, width: UiPx) -> DeviceAdaptiveClass {
        device_adaptive_class(width, self)
    }
}

/// Binary device-shell branch result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceShellMode {
    /// Desktop shell.
    Desktop,
    /// Mobile shell.
    Mobile,
}

impl DeviceShellMode {
    /// Returns true when the shell is desktop.
    pub const fn is_desktop(self) -> bool {
        matches!(self, Self::Desktop)
    }

    /// Returns true when the shell is mobile.
    pub const fn is_mobile(self) -> bool {
        matches!(self, Self::Mobile)
    }
}

/// Shared policy for binary desktop/mobile shell switching.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceShellSwitchPolicy {
    /// The minimum width at which the desktop shell should be used.
    pub desktop_min_width: UiPx,
}

impl Default for DeviceShellSwitchPolicy {
    fn default() -> Self {
        Self {
            desktop_min_width: ui_px(960.0),
        }
    }
}

impl DeviceShellSwitchPolicy {
    /// Sets the desktop-shell threshold.
    pub const fn desktop_min_width(mut self, width: UiPx) -> Self {
        self.desktop_min_width = width;
        self
    }

    /// Classifies a width using this policy.
    pub fn mode(self, width: UiPx) -> DeviceShellMode {
        device_shell_mode(width, self)
    }
}

/// Shared policy for panel/container classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelAdaptivePolicy {
    /// The minimum width at which the panel becomes medium.
    pub medium_min_width: UiPx,
    /// The minimum width at which the panel becomes wide.
    pub wide_min_width: UiPx,
}

impl Default for PanelAdaptivePolicy {
    fn default() -> Self {
        Self {
            medium_min_width: ui_px(360.0),
            wide_min_width: ui_px(640.0),
        }
    }
}

impl PanelAdaptivePolicy {
    /// Sets the medium-panel threshold.
    pub const fn medium_min_width(mut self, width: UiPx) -> Self {
        self.medium_min_width = width;
        self
    }

    /// Sets the wide-panel threshold.
    pub const fn wide_min_width(mut self, width: UiPx) -> Self {
        self.wide_min_width = width;
        self
    }

    /// Classifies a width using this policy.
    pub fn classify(self, width: UiPx) -> PanelAdaptiveClass {
        panel_adaptive_class(width, self)
    }
}

/// Snapshot of the common device-shell adaptive signals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceAdaptiveSnapshot {
    /// The coarse device class.
    pub class: DeviceAdaptiveClass,
    /// The preferred density derived from the class.
    pub density: Density,
    /// Whether the input modality can hover.
    pub can_hover: bool,
    /// Whether the primary pointer is coarse.
    pub coarse_pointer: bool,
    /// Safe area insets in device pixels.
    pub safe_area_insets: Edges,
    /// Occlusion insets in device pixels.
    pub occlusion_insets: Edges,
}

/// Resolves a coarse device-shell class from viewport width.
pub fn device_adaptive_class(width: UiPx, policy: DeviceAdaptivePolicy) -> DeviceAdaptiveClass {
    let regular = policy.regular_min_width.as_f32();
    let expanded = policy.expanded_min_width.as_f32();
    let width = width.as_f32();

    if regular <= expanded {
        if width >= expanded {
            DeviceAdaptiveClass::Expanded
        } else if width >= regular {
            DeviceAdaptiveClass::Regular
        } else {
            DeviceAdaptiveClass::Compact
        }
    } else if width >= regular {
        DeviceAdaptiveClass::Expanded
    } else if width >= expanded {
        DeviceAdaptiveClass::Regular
    } else {
        DeviceAdaptiveClass::Compact
    }
}

/// Resolves a binary desktop/mobile shell branch from viewport width.
pub fn device_shell_mode(width: UiPx, policy: DeviceShellSwitchPolicy) -> DeviceShellMode {
    if width.as_f32() >= policy.desktop_min_width.as_f32() {
        DeviceShellMode::Desktop
    } else {
        DeviceShellMode::Mobile
    }
}

/// Resolves a coarse panel/container class from container width.
pub fn panel_adaptive_class(width: UiPx, policy: PanelAdaptivePolicy) -> PanelAdaptiveClass {
    let medium = policy.medium_min_width.as_f32();
    let wide = policy.wide_min_width.as_f32();
    let width = width.as_f32();

    if medium <= wide {
        if width >= wide {
            PanelAdaptiveClass::Wide
        } else if width >= medium {
            PanelAdaptiveClass::Medium
        } else {
            PanelAdaptiveClass::Compact
        }
    } else if width >= medium {
        PanelAdaptiveClass::Wide
    } else if width >= wide {
        PanelAdaptiveClass::Medium
    } else {
        PanelAdaptiveClass::Compact
    }
}

/// Returns a bundle of common adaptive signals for a device shell.
pub fn device_adaptive_snapshot(
    width: UiPx,
    can_hover: bool,
    coarse_pointer: bool,
    safe_area_insets: Edges,
    occlusion_insets: Edges,
    policy: DeviceAdaptivePolicy,
) -> DeviceAdaptiveSnapshot {
    let class = device_adaptive_class(width, policy);
    DeviceAdaptiveSnapshot {
        class,
        density: class.density(),
        can_hover,
        coarse_pointer,
        safe_area_insets,
        occlusion_insets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_policy_normalizes_threshold_order_via_classification() {
        let policy = DeviceAdaptivePolicy::default()
            .regular_min_width(ui_px(1280.0))
            .expanded_min_width(ui_px(768.0));

        assert_eq!(
            device_adaptive_class(ui_px(500.0), policy),
            DeviceAdaptiveClass::Compact
        );
        assert_eq!(
            device_adaptive_class(ui_px(900.0), policy),
            DeviceAdaptiveClass::Regular
        );
        assert_eq!(
            device_adaptive_class(ui_px(1400.0), policy),
            DeviceAdaptiveClass::Expanded
        );
    }

    #[test]
    fn device_shell_mode_uses_threshold() {
        let policy = DeviceShellSwitchPolicy::default().desktop_min_width(ui_px(1000.0));
        assert_eq!(
            device_shell_mode(ui_px(999.0), policy),
            DeviceShellMode::Mobile
        );
        assert_eq!(
            device_shell_mode(ui_px(1000.0), policy),
            DeviceShellMode::Desktop
        );
    }

    #[test]
    fn panel_policy_normalizes_threshold_order_via_classification() {
        let policy = PanelAdaptivePolicy::default()
            .medium_min_width(ui_px(640.0))
            .wide_min_width(ui_px(360.0));

        assert_eq!(
            panel_adaptive_class(ui_px(200.0), policy),
            PanelAdaptiveClass::Compact
        );
        assert_eq!(
            panel_adaptive_class(ui_px(500.0), policy),
            PanelAdaptiveClass::Medium
        );
        assert_eq!(
            panel_adaptive_class(ui_px(700.0), policy),
            PanelAdaptiveClass::Wide
        );
    }

    #[test]
    fn snapshot_derives_density_from_class() {
        let zero = Edges::default();
        let snapshot = device_adaptive_snapshot(
            ui_px(1400.0),
            true,
            false,
            zero,
            zero,
            DeviceAdaptivePolicy::default(),
        );

        assert_eq!(snapshot.class, DeviceAdaptiveClass::Expanded);
        assert_eq!(snapshot.density, Density::Spacious);
        assert!(snapshot.can_hover);
        assert!(!snapshot.coarse_pointer);
    }
}
