//! User-observable gallery story contracts.

use crate::pages::GalleryPage;
use open_gpui_ui_components::component_contract::ComponentContractMetadata;

/// User-facing operations a story probe can exercise through the rendered gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoryProbeOperation {
    /// Open a disclosure, popup, overlay, or controlled sample.
    Open,
    /// Dismiss an opened disclosure, popup, overlay, or controlled sample.
    Dismiss,
    /// Select a stable option, row, item, or value.
    Select,
    /// Edit user-owned text or value content.
    Edit,
    /// Scroll an inner viewport or story surface.
    Scroll,
    /// Move focus to a story target and assert focus restoration.
    Focus,
    /// Activate a user action target.
    Activate,
    /// Read public state, callback, or sample payload.
    ReadPublicPayload,
}

impl StoryProbeOperation {
    /// Returns the stable operation label used by contract tests and docs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Dismiss => "dismiss",
            Self::Select => "select",
            Self::Edit => "edit",
            Self::Scroll => "scroll",
            Self::Focus => "focus",
            Self::Activate => "activate",
            Self::ReadPublicPayload => "read-public-payload",
        }
    }
}

/// Complete operation vocabulary that story contracts should be able to express.
pub const STORY_PROBE_OPERATIONS: &[StoryProbeOperation] = &[
    StoryProbeOperation::Open,
    StoryProbeOperation::Dismiss,
    StoryProbeOperation::Select,
    StoryProbeOperation::Edit,
    StoryProbeOperation::Scroll,
    StoryProbeOperation::Focus,
    StoryProbeOperation::Activate,
    StoryProbeOperation::ReadPublicPayload,
];

/// One user-observable probe contract for a gallery story.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryProbeContract {
    operation: StoryProbeOperation,
    target: &'static str,
    observes: &'static str,
}

impl StoryProbeContract {
    /// Creates a story probe contract.
    pub const fn new(
        operation: StoryProbeOperation,
        target: &'static str,
        observes: &'static str,
    ) -> Self {
        Self {
            operation,
            target,
            observes,
        }
    }

    /// Returns the operation exercised by this probe.
    pub const fn operation(self) -> StoryProbeOperation {
        self.operation
    }

    /// Returns the semantic target name for this probe.
    pub const fn target(self) -> &'static str {
        self.target
    }

    /// Returns the public observation this probe should assert.
    pub const fn observes(self) -> &'static str {
        self.observes
    }
}

/// Gallery story ownership class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoryContractKind {
    /// Concrete official component sample.
    Component,
    /// Concrete official overlay sample.
    Overlay,
    /// Concrete Focus/A11y interaction scenario.
    FocusAccessibility,
    /// Renderer-neutral state contract readout.
    StateContract,
}

impl StoryContractKind {
    /// Returns the stable kind label used by tests and docs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Component => "component",
            Self::Overlay => "overlay",
            Self::FocusAccessibility => "focus-accessibility",
            Self::StateContract => "state-contract",
        }
    }
}

/// Stable selectors that tests may use to drive a story through user-observable targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorySelectorContract {
    catalog: Option<String>,
    sample: Option<&'static str>,
    state_readout: Option<&'static str>,
    trigger: Option<&'static str>,
    surface: Option<&'static str>,
    control: Option<&'static str>,
}

impl StorySelectorContract {
    /// Creates selectors for a component or state-contract story.
    pub fn component(
        catalog: impl Into<String>,
        sample: Option<&'static str>,
        state_readout: Option<&'static str>,
    ) -> Self {
        Self {
            catalog: Some(catalog.into()),
            sample,
            state_readout,
            trigger: sample,
            surface: sample,
            control: None,
        }
    }

    /// Creates selectors for an overlay story.
    pub fn overlay(
        catalog: &'static str,
        sample: &'static str,
        trigger: Option<&'static str>,
        surface: Option<&'static str>,
        control: Option<&'static str>,
    ) -> Self {
        Self {
            catalog: Some(catalog.to_owned()),
            sample: Some(sample),
            state_readout: None,
            trigger,
            surface,
            control,
        }
    }

    /// Creates selectors for a Focus/A11y scenario.
    pub fn focus_accessibility(sample: &'static str, control: Option<&'static str>) -> Self {
        Self {
            catalog: None,
            sample: Some(sample),
            state_readout: None,
            trigger: control,
            surface: Some(sample),
            control,
        }
    }

    /// Returns the catalog card selector, if this story owns one.
    pub fn catalog_selector(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    /// Returns the rendered sample selector, if this story owns one.
    pub const fn sample_selector(&self) -> Option<&'static str> {
        self.sample
    }

    /// Returns the state readout selector, if this story owns one.
    pub const fn state_readout_selector(&self) -> Option<&'static str> {
        self.state_readout
    }

    /// Returns the primary trigger selector, if this story exposes one.
    pub const fn trigger_selector(&self) -> Option<&'static str> {
        self.trigger
    }

    /// Returns the primary opened surface selector, if this story exposes one.
    pub const fn surface_selector(&self) -> Option<&'static str> {
        self.surface
    }

    /// Returns the auxiliary control selector, if this story exposes one.
    pub const fn control_selector(&self) -> Option<&'static str> {
        self.control
    }

    /// Returns the best public selector for proving the story is rendered.
    pub fn primary_selector(&self) -> Option<&str> {
        if let Some(selector) = self.sample {
            Some(selector)
        } else if let Some(selector) = self.state_readout {
            Some(selector)
        } else if let Some(selector) = self.trigger {
            Some(selector)
        } else if let Some(selector) = self.surface {
            Some(selector)
        } else if let Some(selector) = self.control {
            Some(selector)
        } else {
            self.catalog.as_deref()
        }
    }
}

/// Public, user-observable story contract for a gallery sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryContract {
    page: GalleryPage,
    kind: StoryContractKind,
    owner: StoryOwner,
    state: Option<&'static str>,
    section_id: Option<&'static str>,
    selectors: StorySelectorContract,
    probes: &'static [StoryProbeContract],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoryOwner {
    Component(ComponentContractMetadata),
    Local {
        name: &'static str,
        family: &'static str,
    },
}

impl StoryContract {
    /// Creates an official component story from canonical product metadata.
    pub fn component(
        contract: ComponentContractMetadata,
        state: Option<&'static str>,
        section_id: Option<&'static str>,
        catalog_selector: impl Into<String>,
        sample_selector: Option<&'static str>,
        state_readout_selector: Option<&'static str>,
        probes: &'static [StoryProbeContract],
    ) -> Self {
        Self {
            page: GalleryPage::Components,
            kind: StoryContractKind::Component,
            owner: StoryOwner::Component(contract),
            state,
            section_id,
            selectors: StorySelectorContract::component(
                catalog_selector,
                sample_selector,
                state_readout_selector,
            ),
            probes,
        }
    }

    /// Creates a Gallery-local renderer-neutral state-contract story.
    pub fn state_contract(
        owner_name: &'static str,
        family: &'static str,
        state: Option<&'static str>,
        section_id: Option<&'static str>,
        catalog_selector: impl Into<String>,
        state_readout_selector: &'static str,
        probes: &'static [StoryProbeContract],
    ) -> Self {
        Self {
            page: GalleryPage::Components,
            kind: StoryContractKind::StateContract,
            owner: StoryOwner::Local {
                name: owner_name,
                family,
            },
            state,
            section_id,
            selectors: StorySelectorContract::component(
                catalog_selector,
                None,
                Some(state_readout_selector),
            ),
            probes,
        }
    }

    /// Creates an overlay story.
    pub fn overlay(
        contract: ComponentContractMetadata,
        state: &'static str,
        catalog_selector: &'static str,
        sample_selector: &'static str,
        trigger_selector: Option<&'static str>,
        surface_selector: Option<&'static str>,
        control_selector: Option<&'static str>,
        probes: &'static [StoryProbeContract],
    ) -> Self {
        Self {
            page: GalleryPage::Overlay,
            kind: StoryContractKind::Overlay,
            owner: StoryOwner::Component(contract),
            state: Some(state),
            section_id: None,
            selectors: StorySelectorContract::overlay(
                catalog_selector,
                sample_selector,
                trigger_selector,
                surface_selector,
                control_selector,
            ),
            probes,
        }
    }

    /// Creates a Focus/A11y scenario story.
    pub fn focus_accessibility(
        owner_name: &'static str,
        family: &'static str,
        state: &'static str,
        sample_selector: &'static str,
        control_selector: Option<&'static str>,
        probes: &'static [StoryProbeContract],
    ) -> Self {
        Self {
            page: GalleryPage::FocusAccessibility,
            kind: StoryContractKind::FocusAccessibility,
            owner: StoryOwner::Local {
                name: owner_name,
                family,
            },
            state: Some(state),
            section_id: None,
            selectors: StorySelectorContract::focus_accessibility(
                sample_selector,
                control_selector,
            ),
            probes,
        }
    }

    /// Returns the page that owns this story.
    pub const fn page(&self) -> GalleryPage {
        self.page
    }

    /// Returns the story kind.
    pub const fn kind(&self) -> StoryContractKind {
        self.kind
    }

    /// Returns the public component or overlay name.
    pub const fn owner_name(&self) -> &'static str {
        match self.owner {
            StoryOwner::Component(metadata) => metadata.id().as_str(),
            StoryOwner::Local { name, .. } => name,
        }
    }

    /// Returns the story family label.
    pub const fn family(&self) -> &'static str {
        match self.owner {
            StoryOwner::Component(metadata) => metadata.family().as_str(),
            StoryOwner::Local { family, .. } => family,
        }
    }

    /// Returns canonical component metadata for official component and overlay stories.
    pub const fn component_contract(&self) -> Option<ComponentContractMetadata> {
        match self.owner {
            StoryOwner::Component(metadata) => Some(metadata),
            StoryOwner::Local { .. } => None,
        }
    }

    /// Returns the public state or contract type, if any.
    pub const fn state(&self) -> Option<&'static str> {
        self.state
    }

    /// Returns the Components page section id for component stories.
    pub const fn section_id(&self) -> Option<&'static str> {
        self.section_id
    }

    /// Returns the stable selector contract for this story.
    pub const fn selectors(&self) -> &StorySelectorContract {
        &self.selectors
    }

    /// Returns the supported user-observable probes.
    pub const fn probes(&self) -> &'static [StoryProbeContract] {
        self.probes
    }

    /// Returns true when the story declares the given probe operation.
    pub fn has_operation(&self, operation: StoryProbeOperation) -> bool {
        self.probes
            .iter()
            .any(|probe| probe.operation() == operation)
    }
}
