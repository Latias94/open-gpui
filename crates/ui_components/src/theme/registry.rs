use open_gpui_ui_core::{ThemeDesignScales, TokenKey};

use crate::color::ColorState;

use super::runtime::ThemeContext;
use super::snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};

/// Validation failure for a user-supplied complete Theme v1 definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeValidationError {
    /// The definition did not provide a non-empty stable id.
    MissingId,
    /// The definition did not provide a non-empty display label.
    MissingLabel,
    /// The definition did not provide a color mode.
    MissingMode,
    /// The definition did not provide source revision metadata.
    MissingSourceRevision,
    /// The definition did not provide the complete design-scale payload.
    MissingDesignScales,
    /// A required color token/state pair was omitted.
    MissingColor {
        /// Missing semantic token.
        token: TokenKey,
        /// Missing component color state.
        state: ColorState,
    },
    /// A color token/state pair occurred more than once.
    DuplicateColor {
        /// Duplicate semantic token.
        token: TokenKey,
        /// Duplicate component color state.
        state: ColorState,
    },
    /// A color token/state pair is outside the complete Theme v1 table.
    UnsupportedColor {
        /// Unsupported semantic token.
        token: TokenKey,
        /// Unsupported component color state.
        state: ColorState,
    },
    /// A color value is outside the supported 24-bit RGB range.
    InvalidColorRgb {
        /// Semantic token carrying the invalid value.
        token: TokenKey,
        /// Component color state carrying the invalid value.
        state: ColorState,
        /// Invalid RGB value.
        rgb: u32,
    },
    /// An elevation layer opacity is outside zero through one hundred percent.
    InvalidElevationOpacity {
        /// Zero-based layer index.
        layer: usize,
        /// Invalid percentage.
        opacity_percent: u8,
    },
}

/// User-loadable complete Theme v1 definition before registry validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDefinition {
    id: Option<String>,
    label: Option<String>,
    mode: Option<ThemeMode>,
    source_revision: Option<u64>,
    design_scales: Option<ThemeDesignScales>,
    colors: Vec<ThemeColor>,
}

impl ThemeDefinition {
    /// Creates an empty definition that can be populated by loaders or builders.
    pub const fn draft() -> Self {
        Self {
            id: None,
            label: None,
            mode: None,
            source_revision: None,
            design_scales: None,
            colors: Vec::new(),
        }
    }

    /// Creates a definition with required identity and source metadata.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        mode: ThemeMode,
        source_revision: u64,
    ) -> Self {
        Self::draft()
            .id(id)
            .label(label)
            .mode(mode)
            .source_revision(source_revision)
    }

    /// Creates a complete definition from an existing immutable snapshot.
    pub fn from_snapshot(
        id: impl Into<String>,
        label: impl Into<String>,
        snapshot: &ThemeSnapshot,
    ) -> Self {
        Self::new(id, label, snapshot.mode(), snapshot.source_revision())
            .design_scales(snapshot.design_scales())
            .colors(snapshot.colors().iter().copied())
    }

    /// Applies the stable theme id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Applies the display label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Applies the color mode.
    pub const fn mode(mut self, mode: ThemeMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Applies source-file revision metadata.
    pub const fn source_revision(mut self, source_revision: u64) -> Self {
        self.source_revision = Some(source_revision);
        self
    }

    /// Applies the complete non-color design scales.
    pub const fn design_scales(mut self, design_scales: ThemeDesignScales) -> Self {
        self.design_scales = Some(design_scales);
        self
    }

    /// Adds one color entry.
    ///
    /// Duplicate entries are retained so complete validation can reject them atomically.
    pub fn color(mut self, color: ThemeColor) -> Self {
        self.colors.push(color);
        self
    }

    /// Adds color entries.
    ///
    /// Duplicate entries are retained so complete validation can reject them atomically.
    pub fn colors(mut self, colors: impl IntoIterator<Item = ThemeColor>) -> Self {
        self.colors.extend(colors);
        self
    }

    /// Returns explicitly supplied color entries.
    pub fn color_entries(&self) -> &[ThemeColor] {
        &self.colors
    }

    /// Returns the supplied design scales, if present.
    pub const fn supplied_design_scales(&self) -> Option<ThemeDesignScales> {
        self.design_scales
    }

    pub(super) fn validate_complete(self) -> Result<Self, ThemeValidationError> {
        let validated = self.validate()?;
        Ok(Self {
            id: Some(validated.id),
            label: Some(validated.label),
            mode: Some(validated.mode),
            source_revision: Some(validated.source_revision),
            design_scales: Some(validated.design_scales),
            colors: validated.colors,
        })
    }

    fn validate(self) -> Result<ValidatedThemeDefinition, ThemeValidationError> {
        let id = required_theme_string(self.id, ThemeValidationError::MissingId)?;
        let label = required_theme_string(self.label, ThemeValidationError::MissingLabel)?;
        let mode = self.mode.ok_or(ThemeValidationError::MissingMode)?;
        let source_revision = self
            .source_revision
            .ok_or(ThemeValidationError::MissingSourceRevision)?;
        let design_scales = self
            .design_scales
            .ok_or(ThemeValidationError::MissingDesignScales)?;
        validate_design_scales(design_scales)?;
        Ok(ValidatedThemeDefinition {
            id,
            label,
            mode,
            source_revision,
            design_scales,
            colors: validate_complete_colors(self.colors)?,
        })
    }
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self::draft()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ValidatedThemeDefinition {
    id: String,
    label: String,
    mode: ThemeMode,
    source_revision: u64,
    design_scales: ThemeDesignScales,
    colors: Vec<ThemeColor>,
}

/// One registered complete Theme v1 entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRegistryEntry {
    id: String,
    label: String,
    context: ThemeContext,
}

impl ThemeRegistryEntry {
    /// Returns the stable registry id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the display label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the color mode.
    pub const fn mode(&self) -> ThemeMode {
        self.context.mode()
    }

    /// Returns source-file revision metadata.
    pub const fn source_revision(&self) -> u64 {
        self.context.source_revision()
    }

    /// Returns the runtime-owned effective revision for this registered content.
    pub const fn effective_revision(&self) -> u64 {
        self.context.effective_revision()
    }

    /// Returns the immutable complete Theme v1 snapshot.
    pub fn snapshot(&self) -> &ThemeSnapshot {
        self.context.snapshot()
    }

    pub(super) fn context(&self) -> &ThemeContext {
        &self.context
    }
}

/// App-level registry for built-in and user-loaded complete Theme v1 snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRegistry {
    entries: Vec<ThemeRegistryEntry>,
}

impl ThemeRegistry {
    /// Creates an empty registry.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Creates a registry preloaded with complete built-in snapshots.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.insert_builtin("light", "Light", ThemeContext::light());
        registry.insert_builtin("dark", "Dark", ThemeContext::dark());
        registry.insert_builtin(
            "high-contrast",
            "High contrast",
            ThemeContext::high_contrast(),
        );
        registry
    }

    /// Registers a complete definition, replacing an existing entry with the same id.
    ///
    /// Validation and canonicalization complete before the registry is mutated. Metadata-only or
    /// byte-for-byte effective reloads preserve the previous effective revision.
    pub fn register(
        &mut self,
        definition: ThemeDefinition,
    ) -> Result<&ThemeRegistryEntry, ThemeValidationError> {
        let definition = definition.validate()?;
        let snapshot = ThemeSnapshot::new(
            definition.mode,
            definition.source_revision,
            definition.colors,
            definition.design_scales,
        );

        let index = if let Some(index) = self
            .entries
            .iter()
            .position(|item| item.id == definition.id)
        {
            let previous = &self.entries[index];
            let context = if previous.snapshot().has_same_effective_content(&snapshot) {
                previous
                    .context
                    .with_snapshot_preserving_effective_revision(snapshot)
            } else {
                ThemeContext::new(snapshot)
            };
            self.entries[index] = ThemeRegistryEntry {
                id: definition.id,
                label: definition.label,
                context,
            };
            index
        } else {
            let index = self.entries.len();
            self.entries.push(ThemeRegistryEntry {
                id: definition.id,
                label: definition.label,
                context: ThemeContext::new(snapshot),
            });
            index
        };

        Ok(&self.entries[index])
    }

    /// Returns all registered entries in registration order.
    pub fn entries(&self) -> &[ThemeRegistryEntry] {
        &self.entries
    }

    /// Looks up a registered entry by id.
    pub fn entry(&self, id: impl AsRef<str>) -> Option<&ThemeRegistryEntry> {
        let id = id.as_ref();
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Looks up a registered immutable snapshot by id.
    pub fn snapshot(&self, id: impl AsRef<str>) -> Option<&ThemeSnapshot> {
        self.entry(id).map(ThemeRegistryEntry::snapshot)
    }

    pub(super) fn context(&self, id: impl AsRef<str>) -> Option<&ThemeContext> {
        self.entry(id).map(ThemeRegistryEntry::context)
    }

    fn insert_builtin(&mut self, id: &str, label: &str, context: ThemeContext) {
        self.entries.push(ThemeRegistryEntry {
            id: id.to_owned(),
            label: label.to_owned(),
            context,
        });
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

fn required_theme_string(
    value: Option<String>,
    error: ThemeValidationError,
) -> Result<String, ThemeValidationError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or(error)
}

fn validate_complete_colors(
    colors: Vec<ThemeColor>,
) -> Result<Vec<ThemeColor>, ThemeValidationError> {
    for color in &colors {
        if color.rgb() > 0x00ff_ffff {
            return Err(ThemeValidationError::InvalidColorRgb {
                token: color.token(),
                state: color.state(),
                rgb: color.rgb(),
            });
        }
    }
    for (index, color) in colors.iter().copied().enumerate() {
        if colors[..index]
            .iter()
            .any(|previous| previous.token() == color.token() && previous.state() == color.state())
        {
            return Err(ThemeValidationError::DuplicateColor {
                token: color.token(),
                state: color.state(),
            });
        }
    }

    let required = ThemeSnapshot::light();
    for color in &colors {
        if !required.colors().iter().any(|candidate| {
            candidate.token() == color.token() && candidate.state() == color.state()
        }) {
            return Err(ThemeValidationError::UnsupportedColor {
                token: color.token(),
                state: color.state(),
            });
        }
    }

    let mut canonical = Vec::with_capacity(required.colors().len());
    for required_color in required.colors().iter().copied() {
        let Some(color) = colors.iter().copied().find(|candidate| {
            candidate.token() == required_color.token()
                && candidate.state() == required_color.state()
        }) else {
            return Err(ThemeValidationError::MissingColor {
                token: required_color.token(),
                state: required_color.state(),
            });
        };
        canonical.push(color);
    }
    Ok(canonical)
}

fn validate_design_scales(scales: ThemeDesignScales) -> Result<(), ThemeValidationError> {
    for (layer, elevation) in scales.elevation().overlay().into_iter().enumerate() {
        let (_, _, _, _, opacity_percent) = elevation.raw_values();
        if opacity_percent > 100 {
            return Err(ThemeValidationError::InvalidElevationOpacity {
                layer,
                opacity_percent,
            });
        }
    }
    Ok(())
}
