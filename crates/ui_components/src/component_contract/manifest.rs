//! Portable component registry manifest derived from typed contract rows.

use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};

use super::{
    COMPONENT_API_INVENTORY, COMPONENT_CONTRACT_REGISTRY, ComponentApiInventoryEntry,
    ComponentContractEntry, PublicSurfaceOwnerClass, SurfaceDocsStatus, SurfaceGalleryStatus,
    component_public_methods, component_render_inputs,
};

/// Current component registry manifest schema version.
pub const COMPONENT_REGISTRY_MANIFEST_VERSION: u32 = 1;

/// Renderer-neutral metadata manifest for the component registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryManifest {
    /// Manifest schema version.
    pub schema_version: u32,
    /// Package and distribution authority for the official component surface.
    pub package: ComponentRegistryPackage,
    /// Sorted component, recipe, state-contract, helper, anatomy, and removal rows.
    pub entries: Vec<ComponentRegistryEntry>,
}

/// Cargo package metadata for the generated registry manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryPackage {
    /// Cargo package that ships the official implementation.
    pub cargo_package: String,
    /// Rust crate name used by applications.
    pub crate_name: String,
    /// Distribution authority for official component implementations.
    pub distribution_authority: ComponentRegistryDistributionAuthority,
}

/// Official distribution authority for component implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRegistryDistributionAuthority {
    /// Official components ship as Cargo crate APIs.
    CargoCrate,
}

/// One portable component-registry entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryEntry {
    /// Public component, recipe, state contract, helper, anatomy, or removal token.
    pub name: String,
    /// Product ownership classification.
    pub owner: ComponentRegistryOwnerClass,
    /// Registry-owned component family or ownership group.
    pub family: Option<String>,
    /// Documentation evidence expected for this row.
    pub docs: ComponentRegistryDocs,
    /// Gallery evidence expected for this row.
    pub gallery: ComponentRegistryGallery,
    /// Source ownership facts.
    pub source: ComponentRegistrySource,
    /// Public export intent for crate root and prelude surfaces.
    pub public_export: ComponentRegistryPublicExport,
    /// Public API summary for official component and recipe entries.
    pub api: Option<ComponentRegistryApiInventory>,
    /// Local verification owners that keep this row aligned.
    pub verification: Vec<ComponentRegistryVerificationOwner>,
}

/// Product ownership class for a registry row.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRegistryOwnerClass {
    /// Official rendered component.
    OfficialComponent,
    /// Official recipe/helper component that belongs to a larger family.
    OfficialComponentRecipe,
    /// Renderer-neutral state or behavior contract.
    RendererNeutralStateContract,
    /// Concrete GPUI adapter helper outside renderer-neutral state.
    GpuiAdapterHelper,
    /// Diagnostic-only verification or example surface.
    DiagnosticSurface,
    /// Removed compatibility surface that must not reappear.
    DeprecatedRemovalTarget,
    /// Public but non-promoted anatomy needed by component families.
    InternalImplementationDetail,
}

/// Registry-owned gallery classification for a manifest row.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRegistryGalleryStatus {
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

/// Documentation location expected for a manifest row.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRegistryDocsStatus {
    /// Documented through the component catalog.
    ComponentCatalog,
    /// Documented in the component contract guide.
    ComponentContract,
    /// Documented in either the component contract guide or verification guide.
    ComponentContractOrVerification,
    /// Documented in verification guidance.
    Verification,
}

/// Documentation evidence expected for a manifest row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryDocs {
    /// Documentation location expected for this row.
    pub status: ComponentRegistryDocsStatus,
    /// Stable token that should appear in the owning docs.
    pub token: Option<String>,
}

/// Gallery evidence expected for a manifest row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryGallery {
    /// Gallery classification for rendered dogfood or adjacent readouts.
    pub status: ComponentRegistryGalleryStatus,
    /// Human-readable evidence owner for this gallery status.
    pub evidence_owner: Option<String>,
}

/// Source ownership facts for a manifest row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistrySource {
    /// Primary registry-owned source home.
    pub home: String,
    /// Source files or module directories that own this surface.
    pub inputs: Vec<String>,
}

/// Public export intent for a manifest row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryPublicExport {
    /// Whether the row is intended to be exported from the crate root default surface.
    pub root: bool,
    /// Whether the row is intended to be exported from the prelude.
    pub prelude: bool,
}

/// Public API summary for an official component or recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryApiInventory {
    /// Builder methods that shape rendered output.
    pub render_inputs: Vec<String>,
    /// Inputs that may be controlled by callers.
    pub controlled_inputs: Vec<String>,
    /// Default-state seed methods and their runtime-owned values.
    pub default_seeds: Vec<ComponentRegistryDefaultSeed>,
    /// Legacy seed inputs that still require explicit migration decisions.
    pub legacy_seed_inputs: Vec<String>,
    /// Policy/configuration knobs that shape behavior without owning state.
    pub policy_hints: Vec<String>,
    /// Callback methods and payload types.
    pub callbacks: Vec<ComponentRegistryCallback>,
    /// Expected public builder and state method surface.
    pub public_methods: Vec<String>,
    /// Whether resolved state for this row remains renderer-neutral.
    pub renderer_neutral_state: bool,
    /// Explanation for display-only rows that do not expose interaction inputs.
    pub no_interaction_note: Option<String>,
}

/// Default-state seed method and runtime-owned value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryDefaultSeed {
    /// Public builder method that seeds default state.
    pub builder: String,
    /// Runtime-owned state value seeded by the builder.
    pub runtime_value: String,
}

/// Public callback method and payload type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryCallback {
    /// Public callback builder method.
    pub name: String,
    /// Payload type delivered to the callback.
    pub payload: String,
}

/// Local verification owner that keeps a manifest row aligned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentRegistryVerificationOwner {
    /// Verification owner class.
    pub kind: ComponentRegistryVerificationKind,
    /// File path or command that owns the evidence.
    pub target: String,
    /// Why this owner is relevant to the row.
    pub reason: String,
}

/// Verification owner class for registry manifest evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRegistryVerificationKind {
    /// Static source file evidence.
    SourcePath,
    /// Local verification command.
    Command,
    /// Documentation file evidence.
    Documentation,
    /// Gallery dogfood or conformance evidence.
    Gallery,
}

/// Returns the deterministic component registry manifest.
pub fn component_registry_manifest() -> ComponentRegistryManifest {
    let mut entries = COMPONENT_CONTRACT_REGISTRY
        .iter()
        .map(component_registry_entry)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.cmp(&right.name));

    ComponentRegistryManifest {
        schema_version: COMPONENT_REGISTRY_MANIFEST_VERSION,
        package: ComponentRegistryPackage {
            cargo_package: "open-gpui-ui-components".to_owned(),
            crate_name: "open_gpui_ui_components".to_owned(),
            distribution_authority: ComponentRegistryDistributionAuthority::CargoCrate,
        },
        entries,
    }
}

/// Returns the JSON schema for the component registry manifest.
pub fn component_registry_manifest_schema() -> Schema {
    schema_for!(ComponentRegistryManifest)
}

fn component_registry_entry(entry: &ComponentContractEntry) -> ComponentRegistryEntry {
    ComponentRegistryEntry {
        name: entry.name.to_owned(),
        owner: entry.owner.into(),
        family: entry.family.map(str::to_owned),
        docs: ComponentRegistryDocs {
            status: entry.docs_status.into(),
            token: entry.docs_token.map(str::to_owned),
        },
        gallery: ComponentRegistryGallery {
            status: entry.gallery_status.into(),
            evidence_owner: gallery_evidence_owner(entry).map(str::to_owned),
        },
        source: ComponentRegistrySource {
            home: entry.source_home.to_owned(),
            inputs: strings(entry.source_inputs),
        },
        public_export: ComponentRegistryPublicExport {
            root: entry.default_export,
            prelude: entry.default_export,
        },
        api: api_inventory_for(entry.name).map(component_registry_api_inventory),
        verification: verification_owners(entry),
    }
}

fn api_inventory_for(name: &str) -> Option<&'static ComponentApiInventoryEntry> {
    COMPONENT_API_INVENTORY
        .iter()
        .find(|entry| entry.component == name)
}

fn component_registry_api_inventory(
    entry: &ComponentApiInventoryEntry,
) -> ComponentRegistryApiInventory {
    ComponentRegistryApiInventory {
        render_inputs: strings(component_render_inputs(entry.component)),
        controlled_inputs: strings(entry.controlled_inputs),
        default_seeds: entry
            .default_seeds
            .iter()
            .map(|seed| ComponentRegistryDefaultSeed {
                builder: seed.builder.to_owned(),
                runtime_value: seed.runtime_value.to_owned(),
            })
            .collect(),
        legacy_seed_inputs: strings(entry.legacy_seed_inputs),
        policy_hints: strings(entry.policy_hints),
        callbacks: entry
            .callbacks
            .iter()
            .map(|callback| ComponentRegistryCallback {
                name: callback.name.to_owned(),
                payload: callback.payload.to_owned(),
            })
            .collect(),
        public_methods: strings(component_public_methods(entry.component)),
        renderer_neutral_state: entry.renderer_neutral_state,
        no_interaction_note: entry.no_interaction_note.map(str::to_owned),
    }
}

fn gallery_evidence_owner(entry: &ComponentContractEntry) -> Option<&'static str> {
    match entry.gallery_status {
        SurfaceGalleryStatus::OfficialComponent => {
            Some("examples/ui-foundation-gallery/src/pages/components/catalog.rs")
        }
        SurfaceGalleryStatus::OfficialOverlay => {
            Some("examples/ui-foundation-gallery/src/pages/overlay.rs")
        }
        SurfaceGalleryStatus::AdapterOnly
        | SurfaceGalleryStatus::InternalAnatomy
        | SurfaceGalleryStatus::StateContract => {
            Some("examples/ui-foundation-gallery/src/pages/components/conformance.rs")
        }
        SurfaceGalleryStatus::NotInGallery => None,
    }
}

fn verification_owners(entry: &ComponentContractEntry) -> Vec<ComponentRegistryVerificationOwner> {
    let mut owners = vec![
        ComponentRegistryVerificationOwner {
            kind: ComponentRegistryVerificationKind::SourcePath,
            target: "crates/ui_components/src/component_contract/rows.rs".to_owned(),
            reason: "typed registry authority".to_owned(),
        },
        ComponentRegistryVerificationOwner {
            kind: ComponentRegistryVerificationKind::Command,
            target: "cargo run -p xtask -- scan-ui-contract".to_owned(),
            reason: "component contract drift gate".to_owned(),
        },
    ];

    if entry.default_export {
        owners.push(ComponentRegistryVerificationOwner {
            kind: ComponentRegistryVerificationKind::SourcePath,
            target: "crates/ui_components/tests/public_surface/exports.rs".to_owned(),
            reason: "root and prelude export intent".to_owned(),
        });
    }

    if entry.docs_token.is_some() {
        owners.push(ComponentRegistryVerificationOwner {
            kind: ComponentRegistryVerificationKind::Documentation,
            target: docs_owner_path(entry.docs_status).to_owned(),
            reason: "documentation token coverage".to_owned(),
        });
    }

    if let Some(owner) = gallery_evidence_owner(entry) {
        owners.push(ComponentRegistryVerificationOwner {
            kind: ComponentRegistryVerificationKind::Gallery,
            target: owner.to_owned(),
            reason: "gallery evidence owner".to_owned(),
        });
    }

    if entry.family == Some("theme") {
        owners.push(ComponentRegistryVerificationOwner {
            kind: ComponentRegistryVerificationKind::Command,
            target: "cargo run -p xtask -- scan-theme-schema".to_owned(),
            reason: "theme schema drift gate".to_owned(),
        });
    }

    owners
}

fn docs_owner_path(status: SurfaceDocsStatus) -> &'static str {
    match status {
        SurfaceDocsStatus::ComponentCatalog => {
            "examples/ui-foundation-gallery/src/pages/components/catalog.rs"
        }
        SurfaceDocsStatus::ComponentContract => "docs/ui/component-contract.md",
        SurfaceDocsStatus::ComponentContractOrVerification => {
            "docs/ui/component-contract.md or docs/verification.md"
        }
        SurfaceDocsStatus::Verification => "docs/verification.md",
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

impl From<PublicSurfaceOwnerClass> for ComponentRegistryOwnerClass {
    fn from(value: PublicSurfaceOwnerClass) -> Self {
        match value {
            PublicSurfaceOwnerClass::OfficialComponent => Self::OfficialComponent,
            PublicSurfaceOwnerClass::OfficialComponentRecipe => Self::OfficialComponentRecipe,
            PublicSurfaceOwnerClass::RendererNeutralStateContract => {
                Self::RendererNeutralStateContract
            }
            PublicSurfaceOwnerClass::GpuiAdapterHelper => Self::GpuiAdapterHelper,
            PublicSurfaceOwnerClass::DiagnosticSurface => Self::DiagnosticSurface,
            PublicSurfaceOwnerClass::DeprecatedRemovalTarget => Self::DeprecatedRemovalTarget,
            PublicSurfaceOwnerClass::InternalImplementationDetail => {
                Self::InternalImplementationDetail
            }
        }
    }
}

impl From<SurfaceGalleryStatus> for ComponentRegistryGalleryStatus {
    fn from(value: SurfaceGalleryStatus) -> Self {
        match value {
            SurfaceGalleryStatus::OfficialComponent => Self::OfficialComponent,
            SurfaceGalleryStatus::OfficialOverlay => Self::OfficialOverlay,
            SurfaceGalleryStatus::AdapterOnly => Self::AdapterOnly,
            SurfaceGalleryStatus::InternalAnatomy => Self::InternalAnatomy,
            SurfaceGalleryStatus::StateContract => Self::StateContract,
            SurfaceGalleryStatus::NotInGallery => Self::NotInGallery,
        }
    }
}

impl From<SurfaceDocsStatus> for ComponentRegistryDocsStatus {
    fn from(value: SurfaceDocsStatus) -> Self {
        match value {
            SurfaceDocsStatus::ComponentCatalog => Self::ComponentCatalog,
            SurfaceDocsStatus::ComponentContract => Self::ComponentContract,
            SurfaceDocsStatus::ComponentContractOrVerification => {
                Self::ComponentContractOrVerification
            }
            SurfaceDocsStatus::Verification => Self::Verification,
        }
    }
}
