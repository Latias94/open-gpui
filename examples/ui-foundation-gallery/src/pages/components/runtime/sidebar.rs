use open_gpui::{App, BorrowAppContext, Global};
use open_gpui_ui_components::{ActivationSource, SidebarActivation};

/// One semantic activation captured from a rendered gallery Sidebar sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSampleActivation {
    sample_id: String,
    activation: SidebarActivation,
    source: ActivationSource,
}

impl SidebarSampleActivation {
    /// Returns the stable gallery sample id.
    pub fn sample_id(&self) -> &str {
        &self.sample_id
    }

    /// Returns the typed Sidebar activation payload.
    pub const fn activation(&self) -> &SidebarActivation {
        &self.activation
    }

    /// Returns the normalized activation source.
    pub const fn source(&self) -> ActivationSource {
        self.source
    }
}

/// Runtime semantic activation log used by Sidebar gallery smoke tests and readouts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SidebarSampleRuntimeLog {
    activations: Vec<SidebarSampleActivation>,
}

impl Global for SidebarSampleRuntimeLog {}

impl SidebarSampleRuntimeLog {
    /// Returns captured activations in event order.
    pub fn activations(&self) -> &[SidebarSampleActivation] {
        &self.activations
    }

    /// Returns the latest activation for one sample.
    pub fn last_for(&self, sample_id: &str) -> Option<&SidebarSampleActivation> {
        self.activations
            .iter()
            .rev()
            .find(|entry| entry.sample_id == sample_id)
    }

    /// Clears captured activations.
    pub fn clear(&mut self) {
        self.activations.clear();
    }
}

/// Records a gallery Sidebar activation without flattening its typed payload or source.
pub fn record_sidebar_activation(
    sample_id: impl Into<String>,
    activation: SidebarActivation,
    source: ActivationSource,
    cx: &mut App,
) {
    cx.update_default_global::<SidebarSampleRuntimeLog, _>(|log, _| {
        log.activations.push(SidebarSampleActivation {
            sample_id: sample_id.into(),
            activation,
            source,
        });
    });
}
