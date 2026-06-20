//! Adaptive foundation page metadata.

use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceAdaptivePolicy, DeviceShellMode, DeviceShellSwitchPolicy,
    PanelAdaptiveClass, PanelAdaptivePolicy, UiPx, ui_px,
};

/// Page title.
pub const TITLE: &str = "Adaptive";
/// Page summary.
pub const SUMMARY: &str = "Device and panel classes that choose shell layout and density.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "DeviceShellSwitchPolicy",
    "DeviceAdaptivePolicy",
    "DeviceAdaptiveClass",
    "PanelAdaptivePolicy",
    "AdaptiveQuerySource",
];

/// One device-width sample rendered by the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceAdaptiveSample {
    /// Sample viewport width.
    pub width: UiPx,
    /// Desktop/mobile branch.
    pub shell_mode: DeviceShellMode,
    /// Coarse device class.
    pub class: DeviceAdaptiveClass,
    /// Density derived from the device class.
    pub density: Density,
}

/// One panel-width sample rendered by the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelAdaptiveSample {
    /// Sample panel width.
    pub width: UiPx,
    /// Coarse panel class.
    pub class: PanelAdaptiveClass,
}

/// Returns representative device samples from the default policies.
pub fn device_samples() -> [DeviceAdaptiveSample; 3] {
    let widths = [ui_px(640.0), ui_px(1040.0), ui_px(1440.0)];
    let shell_policy = DeviceShellSwitchPolicy::default();
    let device_policy = DeviceAdaptivePolicy::default();

    widths.map(|width| {
        let class = device_policy.classify(width);
        DeviceAdaptiveSample {
            width,
            shell_mode: shell_policy.mode(width),
            class,
            density: class.density(),
        }
    })
}

/// Returns representative panel samples from the default policy.
pub fn panel_samples() -> [PanelAdaptiveSample; 3] {
    let widths = [ui_px(280.0), ui_px(480.0), ui_px(720.0)];
    let policy = PanelAdaptivePolicy::default();

    widths.map(|width| PanelAdaptiveSample {
        width,
        class: policy.classify(width),
    })
}
