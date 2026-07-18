//! Narrow product and public-surface metadata used by federated conformance.

/// Stable identifier for one official component contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentContractId(&'static str);

impl ComponentContractId {
    /// Creates a stable component contract identifier.
    const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the identifier text.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Monotonic revision for one component product contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentContractRevision(u16);

impl ComponentContractRevision {
    /// Creates a component contract revision.
    const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the numeric revision.
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Stable product family for an official component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentFamily(&'static str);

impl ComponentFamily {
    /// Creates a component family identifier.
    const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the family identifier text.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Shared product metadata projected into Gallery and DevTools adapters.
///
/// Canonical metadata is obtained through [`super::component_contract_metadata`] or a canonical
/// [`ComponentContractEntry`]. Downstream crates cannot construct unregistered ids or revisions.
///
/// ```compile_fail
/// use open_gpui_ui_components::component_contract::ComponentContractMetadata;
///
/// let _forged = ComponentContractMetadata::new("Forged", 0, "");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentContractMetadata {
    id: ComponentContractId,
    revision: ComponentContractRevision,
    family: ComponentFamily,
}

impl ComponentContractMetadata {
    /// Creates component product metadata.
    const fn new(id: &'static str, revision: u16, family: &'static str) -> Self {
        Self {
            id: ComponentContractId::new(id),
            revision: ComponentContractRevision::new(revision),
            family: ComponentFamily::new(family),
        }
    }

    /// Returns the stable component contract identifier.
    pub const fn id(self) -> ComponentContractId {
        self.id
    }

    /// Returns the component contract revision.
    pub const fn revision(self) -> ComponentContractRevision {
        self.revision
    }

    /// Returns the canonical component family.
    pub const fn family(self) -> ComponentFamily {
        self.family
    }
}

/// Canonical product row for one official component.
///
/// Package names, test targets, Rust symbols, source paths, Gallery selectors, and public export
/// coordinates deliberately live with their natural owners instead of this row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentContractEntry {
    metadata: ComponentContractMetadata,
    required_scenarios: &'static [&'static str],
}

impl ComponentContractEntry {
    /// Creates an official component product row.
    pub(crate) const fn new(id: &'static str, revision: u16, family: &'static str) -> Self {
        Self {
            metadata: ComponentContractMetadata::new(id, revision, family),
            required_scenarios: &[],
        }
    }

    /// Adds component-specific executable scenario requirements.
    pub(crate) const fn with_required_scenarios(
        mut self,
        required_scenarios: &'static [&'static str],
    ) -> Self {
        self.required_scenarios = required_scenarios;
        self
    }

    /// Returns the shared product metadata.
    pub const fn metadata(self) -> ComponentContractMetadata {
        self.metadata
    }

    /// Returns the stable component contract identifier.
    pub const fn id(self) -> ComponentContractId {
        self.metadata.id()
    }

    /// Returns the component contract revision.
    pub const fn revision(self) -> ComponentContractRevision {
        self.metadata.revision()
    }

    /// Returns the canonical component family.
    pub const fn family(self) -> ComponentFamily {
        self.metadata.family()
    }

    /// Returns component-specific required scenario identifiers.
    pub const fn required_scenarios(self) -> &'static [&'static str] {
        self.required_scenarios
    }
}

/// Public export tier owned by the module that declares the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PublicApiTier {
    /// Common application interface exported by the crate root, common module, and prelude.
    Common,
    /// Extended crate-root interface intentionally excluded from the common prelude.
    Extended,
    /// Diagnostic interface available only through its explicit owner module.
    Diagnostic,
}

/// Typed public export fact generated by the owning `pub use` declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicApiExport {
    name: &'static str,
    owner: &'static str,
    tier: PublicApiTier,
}

impl PublicApiExport {
    /// Creates one public export fact.
    pub(crate) const fn new(name: &'static str, owner: &'static str, tier: PublicApiTier) -> Self {
        Self { name, owner, tier }
    }

    /// Returns the exported Rust identifier.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the module path that owns the export declaration.
    pub const fn owner(self) -> &'static str {
        self.owner
    }

    /// Returns the public export tier.
    pub const fn tier(self) -> PublicApiTier {
        self.tier
    }
}
