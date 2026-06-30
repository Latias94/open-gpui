use super::palette::builtin_theme_snapshot;
use super::snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};

/// Validation failure for a user-supplied theme definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeValidationError {
    /// The definition did not provide a non-empty stable id.
    MissingId,
    /// The definition did not provide a non-empty display label.
    MissingLabel,
    /// The definition did not provide a color mode.
    MissingMode,
    /// The definition did not provide a revision.
    MissingRevision,
}

/// User-loadable theme definition before registry validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDefinition {
    id: Option<String>,
    label: Option<String>,
    mode: Option<ThemeMode>,
    revision: Option<u64>,
    fallback_mode: Option<ThemeMode>,
    colors: Vec<ThemeColor>,
}

impl ThemeDefinition {
    /// Creates an empty definition that can be populated by loaders or builders.
    pub const fn draft() -> Self {
        Self {
            id: None,
            label: None,
            mode: None,
            revision: None,
            fallback_mode: None,
            colors: Vec::new(),
        }
    }

    /// Creates a complete definition with required identity fields.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        mode: ThemeMode,
        revision: u64,
    ) -> Self {
        Self::draft()
            .id(id)
            .label(label)
            .mode(mode)
            .revision(revision)
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

    /// Applies the revision used for cache invalidation.
    pub const fn revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    /// Applies the built-in snapshot used to fill omitted color tokens.
    pub const fn fallback_mode(mut self, fallback_mode: ThemeMode) -> Self {
        self.fallback_mode = Some(fallback_mode);
        self
    }

    /// Adds or replaces one color entry.
    pub fn color(mut self, color: ThemeColor) -> Self {
        upsert_theme_color(&mut self.colors, color);
        self
    }

    /// Adds or replaces color entries.
    pub fn colors(mut self, colors: impl IntoIterator<Item = ThemeColor>) -> Self {
        for color in colors {
            upsert_theme_color(&mut self.colors, color);
        }
        self
    }

    /// Returns explicitly supplied color entries.
    pub fn color_entries(&self) -> &[ThemeColor] {
        &self.colors
    }

    fn validate(self) -> Result<ValidatedThemeDefinition, ThemeValidationError> {
        Ok(ValidatedThemeDefinition {
            id: required_theme_string(self.id, ThemeValidationError::MissingId)?,
            label: required_theme_string(self.label, ThemeValidationError::MissingLabel)?,
            mode: self.mode.ok_or(ThemeValidationError::MissingMode)?,
            revision: self.revision.ok_or(ThemeValidationError::MissingRevision)?,
            fallback_mode: self.fallback_mode,
            colors: self.colors,
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
    revision: u64,
    fallback_mode: Option<ThemeMode>,
    colors: Vec<ThemeColor>,
}

/// Diagnostics emitted while registering a theme definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThemeRegistrationDiagnostics {
    fallback_mode: ThemeMode,
    fallback_color_count: usize,
}

impl ThemeRegistrationDiagnostics {
    /// Returns the built-in mode used to fill missing color entries.
    pub const fn fallback_mode(self) -> ThemeMode {
        self.fallback_mode
    }

    /// Returns how many registered color entries came from the fallback snapshot.
    pub const fn fallback_color_count(self) -> usize {
        self.fallback_color_count
    }
}

/// One registered theme entry with owned color storage.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeRegistryEntry {
    id: String,
    label: String,
    mode: ThemeMode,
    revision: u64,
    colors: Vec<ThemeColor>,
    diagnostics: ThemeRegistrationDiagnostics,
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
        self.mode
    }

    /// Returns the revision used for cache invalidation.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns registration diagnostics.
    pub const fn diagnostics(&self) -> ThemeRegistrationDiagnostics {
        self.diagnostics
    }

    /// Returns an immutable snapshot over this entry's owned color table.
    pub fn snapshot(&self) -> ThemeSnapshot<'_> {
        ThemeSnapshot::new(self.mode, self.revision, &self.colors)
    }
}

/// App-level registry for built-in and user-loaded theme snapshots.
#[derive(Debug, Clone, PartialEq)]
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

    /// Creates a registry preloaded with the built-in snapshots.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.insert_builtin("light", "Light", ThemeSnapshot::light());
        registry.insert_builtin("dark", "Dark", ThemeSnapshot::dark());
        registry.insert_builtin(
            "high-contrast",
            "High contrast",
            ThemeSnapshot::high_contrast(),
        );
        registry
    }

    /// Registers a user definition, replacing an existing entry with the same id.
    pub fn register(
        &mut self,
        definition: ThemeDefinition,
    ) -> Result<&ThemeRegistryEntry, ThemeValidationError> {
        let definition = definition.validate()?;
        let fallback_mode = definition.fallback_mode.unwrap_or(definition.mode);
        let fallback_snapshot = builtin_theme_snapshot(fallback_mode);
        let fallback_color_count = fallback_snapshot
            .colors()
            .iter()
            .filter(|fallback| !theme_colors_contain(&definition.colors, **fallback))
            .count();
        let mut colors = fallback_snapshot.colors().to_vec();
        for color in definition.colors {
            upsert_theme_color(&mut colors, color);
        }
        let entry = ThemeRegistryEntry {
            id: definition.id,
            label: definition.label,
            mode: definition.mode,
            revision: definition.revision,
            colors,
            diagnostics: ThemeRegistrationDiagnostics {
                fallback_mode,
                fallback_color_count,
            },
        };

        let index = if let Some(index) = self.entries.iter().position(|item| item.id == entry.id) {
            self.entries[index] = entry;
            index
        } else {
            let index = self.entries.len();
            self.entries.push(entry);
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
    pub fn snapshot(&self, id: impl AsRef<str>) -> Option<ThemeSnapshot<'_>> {
        self.entry(id).map(ThemeRegistryEntry::snapshot)
    }

    fn insert_builtin(&mut self, id: &str, label: &str, snapshot: ThemeSnapshot<'static>) {
        self.entries.push(ThemeRegistryEntry {
            id: id.to_owned(),
            label: label.to_owned(),
            mode: snapshot.mode(),
            revision: snapshot.revision(),
            colors: snapshot.colors().to_vec(),
            diagnostics: ThemeRegistrationDiagnostics {
                fallback_mode: snapshot.mode(),
                fallback_color_count: 0,
            },
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

fn upsert_theme_color(colors: &mut Vec<ThemeColor>, color: ThemeColor) {
    if let Some(existing) = colors
        .iter_mut()
        .find(|entry| entry.token() == color.token() && entry.state() == color.state())
    {
        *existing = color;
    } else {
        colors.push(color);
    }
}

fn theme_colors_contain(colors: &[ThemeColor], color: ThemeColor) -> bool {
    colors
        .iter()
        .any(|entry| entry.token() == color.token() && entry.state() == color.state())
}
