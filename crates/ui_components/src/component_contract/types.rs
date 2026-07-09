//! Component contract row and classification types.

use super::component_render_inputs;
use crate::a11y::{A11yLabelSource, A11yStateEvidence, A11yValueKind};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role};

/// Builder/runtime-value pair for a defaulted state seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultSeedApi {
    /// Public builder method that seeds default state.
    pub builder: &'static str,
    /// Runtime-owned state value seeded by the builder.
    pub runtime_value: &'static str,
}

/// Public callback method and payload type recorded by the component contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackApi {
    /// Public callback builder method.
    pub name: &'static str,
    /// Payload type delivered to the callback.
    pub payload: &'static str,
}

/// API inventory row for one official component or component recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentApiInventoryEntry {
    /// Public component type name.
    pub component: &'static str,
    /// Inputs that may be controlled by callers.
    pub controlled_inputs: &'static [&'static str],
    /// Default-state seed methods and their runtime-owned values.
    pub default_seeds: &'static [DefaultSeedApi],
    /// Policy/configuration knobs that shape behavior without owning state.
    pub policy_hints: &'static [&'static str],
    /// Callback methods and payload types.
    pub callbacks: &'static [CallbackApi],
    /// Whether resolved state for this row remains renderer-neutral.
    pub renderer_neutral_state: bool,
    /// Explanation for display-only rows that do not expose interaction inputs.
    pub no_interaction_note: Option<&'static str>,
}

impl ComponentApiInventoryEntry {
    /// Returns whether the inventory row has at least one ownership bucket.
    pub fn has_classification(&self) -> bool {
        !component_render_inputs(self.component).is_empty()
            || !self.controlled_inputs.is_empty()
            || !self.default_seeds.is_empty()
            || !self.policy_hints.is_empty()
            || !self.callbacks.is_empty()
            || self.no_interaction_note.is_some()
    }
}

/// Ownership class for public surfaces adjacent to official components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PublicSurfaceOwnerClass {
    /// An official rendered component.
    OfficialComponent,
    /// An official recipe/helper component that belongs to a larger family.
    OfficialComponentRecipe,
    /// Renderer-neutral state or behavior contract.
    RendererNeutralStateContract,
    /// Concrete GPUI adapter helper intentionally outside renderer-neutral state.
    GpuiAdapterHelper,
    /// Diagnostic surface for verification and examples.
    DiagnosticSurface,
    /// Removed compatibility surface that must not reappear.
    DeprecatedRemovalTarget,
    /// Public but non-promoted anatomy needed by component families.
    InternalImplementationDetail,
}

/// Public surface row adjacent to component inventory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicSurfaceOwnerEntry {
    /// Public surface token.
    pub name: &'static str,
    /// Product ownership classification.
    pub owner: PublicSurfaceOwnerClass,
    /// Source home for the surface.
    pub home: &'static str,
}

/// Contract-owned gallery classification for a component or adjacent surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SurfaceGalleryStatus {
    /// Official component sample in the Components gallery.
    OfficialComponent,
    /// Official overlay sample in the Overlay gallery.
    OfficialOverlay,
    /// Adapter-only row shown for concrete runtime integration.
    AdapterOnly,
    /// Internal anatomy row shown as implementation evidence.
    InternalAnatomy,
    /// Renderer-neutral state-contract row.
    StateContract,
    /// No gallery row is expected.
    NotInGallery,
}

/// Documentation location expected for a contract row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SurfaceDocsStatus {
    /// Documented through the component catalog.
    ComponentCatalog,
    /// Documented in the component contract guide.
    ComponentContract,
    /// Documented either in the component contract guide or verification guide.
    ComponentContractOrVerification,
    /// Documented in verification guidance.
    Verification,
}

/// Canonical product metadata for a public component-library surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentContractEntry {
    /// Public component, recipe, state contract, helper, or anatomy token.
    pub name: &'static str,
    /// Product ownership classification.
    pub owner: PublicSurfaceOwnerClass,
    /// Contract-owned component family or ownership group.
    pub family: Option<&'static str>,
    /// Gallery classification for rendered dogfood or adjacent readouts.
    pub gallery_status: SurfaceGalleryStatus,
    /// Documentation location expected for this row.
    pub docs_status: SurfaceDocsStatus,
    /// Token that should appear in the owning docs when docs coverage is expected.
    pub docs_token: Option<&'static str>,
    /// Whether root and prelude should expose this row through the default surface.
    pub default_export: bool,
    /// Source files or module directories that own this surface.
    pub source_inputs: &'static [&'static str],
    /// Primary source home used by public-surface manifests.
    pub source_home: &'static str,
}

/// Renderer-neutral accessibility evidence for one representative component or component part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentA11yEvidence {
    /// Component or component part covered by the evidence.
    pub component: &'static str,
    /// Renderer-neutral role expected for the component or part.
    pub role: Role,
    /// Source that provides the accessible name.
    pub label_source: A11yLabelSource,
    /// Optional value metadata kind exposed by the component.
    pub value_kind: Option<A11yValueKind>,
    /// Optional orientation metadata exposed by the component.
    pub orientation: Option<Orientation>,
    /// Supported accessibility actions covered by the representative contract.
    pub actions: &'static [AccessibleAction],
    /// Semantic state and focus behavior covered by this evidence row.
    pub state_coverage: &'static [A11yStateEvidence],
}

/// One component conformance gate shown by product documentation and gallery dogfood.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentConformanceGate {
    /// Stable gate id.
    pub id: &'static str,
    /// Visible gate title.
    pub title: &'static str,
    /// Behavior or contract that this gate protects.
    pub summary: &'static str,
    /// Durable test or document evidence for this gate.
    pub evidence: &'static [&'static str],
}
