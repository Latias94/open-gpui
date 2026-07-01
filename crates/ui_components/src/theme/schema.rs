use std::collections::BTreeSet;
use std::path::Path;

use open_gpui_ui_core::{TokenKey, semantic};
use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};

use crate::color::ColorState;

use super::registry::{ThemeDefinition, ThemeRegistry, ThemeRegistryEntry, ThemeValidationError};
use super::snapshot::{ThemeColor, ThemeMode};

/// Current JSON schema version supported by the theme file loader.
pub const THEME_JSON_SCHEMA_VERSION: u32 = 1;

/// Theme JSON file field used by structured loader errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeFileField {
    /// Top-level schema version.
    SchemaVersion,
    /// Stable theme id.
    Id,
    /// Display label.
    Label,
    /// Theme color mode.
    Mode,
    /// Revision used for cache invalidation.
    Revision,
    /// Color entry array.
    Colors,
    /// Per-color semantic token key.
    ColorToken,
    /// Per-color state key.
    ColorState,
    /// Per-color RGB value.
    ColorRgb,
}

/// Structured failure emitted while loading a JSON theme file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeLoadError {
    /// The file could not be read.
    ReadFile {
        /// Path passed to the loader.
        path: String,
    },
    /// The JSON was syntactically invalid or had an unsupported shape.
    InvalidJson {
        /// One-based line number reported by the JSON parser.
        line: usize,
        /// One-based column number reported by the JSON parser.
        column: usize,
    },
    /// A required top-level field is missing or blank.
    MissingField(ThemeFileField),
    /// A required color-entry field is missing or blank.
    MissingColorField {
        /// Zero-based color entry index.
        index: usize,
        /// Missing color-entry field.
        field: ThemeFileField,
    },
    /// The file asks for a schema version this loader does not support.
    UnsupportedSchemaVersion {
        /// Requested schema version.
        version: u32,
        /// Current supported schema version.
        supported: u32,
    },
    /// The file references a color mode outside the supported vocabulary.
    UnsupportedMode {
        /// Unsupported mode string.
        mode: String,
    },
    /// The file references a semantic token outside the current theme token vocabulary.
    UnsupportedToken {
        /// Unsupported token string.
        token: String,
    },
    /// The file references a component color state outside the current resolver vocabulary.
    UnsupportedColorState {
        /// Unsupported state string.
        state: String,
    },
    /// The file supplies the same token/state pair more than once.
    DuplicateColor {
        /// Duplicate semantic token.
        token: String,
        /// Duplicate color state.
        state: String,
    },
    /// The file supplies an RGB value that is not a six-digit hex color.
    InvalidRgb {
        /// Invalid RGB string.
        value: String,
    },
    /// The parsed definition failed registry validation.
    Registration(ThemeValidationError),
}

/// Returns the JSON schema for portable theme definition files.
pub fn theme_json_schema() -> Schema {
    schema_for!(ThemeJsonSchemaFile)
}

/// Parses and validates a JSON theme definition string.
pub fn theme_definition_from_json_str(source: &str) -> Result<ThemeDefinition, ThemeLoadError> {
    let document = serde_json::from_str::<ThemeFileDocument>(source).map_err(|error| {
        ThemeLoadError::InvalidJson {
            line: error.line(),
            column: error.column(),
        }
    })?;
    theme_definition_from_document(document)
}

/// Reads, parses, and validates a JSON theme definition file.
pub fn theme_definition_from_json_file(
    path: impl AsRef<Path>,
) -> Result<ThemeDefinition, ThemeLoadError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|_| ThemeLoadError::ReadFile {
        path: path.display().to_string(),
    })?;
    theme_definition_from_json_str(&source)
}

/// Parses a JSON theme definition string and registers it in the supplied registry.
pub fn register_theme_json_str<'a>(
    registry: &'a mut ThemeRegistry,
    source: &str,
) -> Result<&'a ThemeRegistryEntry, ThemeLoadError> {
    let definition = theme_definition_from_json_str(source)?;
    registry
        .register(definition)
        .map_err(ThemeLoadError::Registration)
}

/// Reads a JSON theme definition file and registers it in the supplied registry.
pub fn register_theme_json_file<'a>(
    registry: &'a mut ThemeRegistry,
    path: impl AsRef<Path>,
) -> Result<&'a ThemeRegistryEntry, ThemeLoadError> {
    let definition = theme_definition_from_json_file(path)?;
    registry
        .register(definition)
        .map_err(ThemeLoadError::Registration)
}

fn theme_definition_from_document(
    document: ThemeFileDocument,
) -> Result<ThemeDefinition, ThemeLoadError> {
    let schema_version = document
        .schema_version
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::SchemaVersion))?;
    if schema_version != THEME_JSON_SCHEMA_VERSION {
        return Err(ThemeLoadError::UnsupportedSchemaVersion {
            version: schema_version,
            supported: THEME_JSON_SCHEMA_VERSION,
        });
    }

    let id = required_theme_file_string(document.id, ThemeFileField::Id)?;
    let label = required_theme_file_string(document.label, ThemeFileField::Label)?;
    let mode = parse_theme_mode(required_theme_file_string(
        document.mode,
        ThemeFileField::Mode,
    )?)?;
    let revision = document
        .revision
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::Revision))?;
    let fallback_mode = document
        .fallback_mode
        .map(required_fallback_mode)
        .transpose()?;
    let colors = parse_theme_colors(document.colors)?;

    let mut definition = ThemeDefinition::new(id, label, mode, revision).colors(colors);
    if let Some(fallback_mode) = fallback_mode {
        definition = definition.fallback_mode(fallback_mode);
    }
    Ok(definition)
}

fn required_fallback_mode(mode: String) -> Result<ThemeMode, ThemeLoadError> {
    parse_theme_mode(required_theme_file_string(
        Some(mode),
        ThemeFileField::Mode,
    )?)
}

fn parse_theme_colors(
    colors: Option<Vec<ThemeFileColorDocument>>,
) -> Result<Vec<ThemeColor>, ThemeLoadError> {
    let colors = colors.ok_or(ThemeLoadError::MissingField(ThemeFileField::Colors))?;
    if colors.is_empty() {
        return Err(ThemeLoadError::MissingField(ThemeFileField::Colors));
    }

    let mut parsed = Vec::with_capacity(colors.len());
    let mut seen = BTreeSet::new();
    for (index, color) in colors.into_iter().enumerate() {
        let token_source = required_color_string(color.token, index, ThemeFileField::ColorToken)?;
        let state_source = required_color_string(color.state, index, ThemeFileField::ColorState)?;
        let rgb_source = required_color_string(color.rgb, index, ThemeFileField::ColorRgb)?;
        let token = parse_token(token_source.clone())?;
        let state = parse_color_state(state_source.clone())?;
        let rgb = parse_rgb(rgb_source)?;
        let key = (token.as_str().to_owned(), state.as_str().to_owned());
        if !seen.insert(key.clone()) {
            return Err(ThemeLoadError::DuplicateColor {
                token: key.0,
                state: key.1,
            });
        }
        parsed.push(ThemeColor::new(token, state, rgb));
    }

    Ok(parsed)
}

fn required_theme_file_string(
    value: Option<String>,
    field: ThemeFileField,
) -> Result<String, ThemeLoadError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or(ThemeLoadError::MissingField(field))
}

fn required_color_string(
    value: Option<String>,
    index: usize,
    field: ThemeFileField,
) -> Result<String, ThemeLoadError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or(ThemeLoadError::MissingColorField { index, field })
}

fn parse_theme_mode(mode: String) -> Result<ThemeMode, ThemeLoadError> {
    match mode.as_str() {
        "light" => Ok(ThemeMode::Light),
        "dark" => Ok(ThemeMode::Dark),
        "high-contrast" => Ok(ThemeMode::HighContrast),
        _ => Err(ThemeLoadError::UnsupportedMode { mode }),
    }
}

fn parse_token(token: String) -> Result<TokenKey, ThemeLoadError> {
    semantic::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == token)
        .ok_or(ThemeLoadError::UnsupportedToken { token })
}

fn parse_color_state(state: String) -> Result<ColorState, ThemeLoadError> {
    match state.as_str() {
        "default" => Ok(ColorState::Default),
        "hover" => Ok(ColorState::Hover),
        "selected" => Ok(ColorState::Selected),
        "disabled" => Ok(ColorState::Disabled),
        "read-only" => Ok(ColorState::ReadOnly),
        "invalid" => Ok(ColorState::Invalid),
        "required" => Ok(ColorState::Required),
        "placeholder" => Ok(ColorState::Placeholder),
        "message" => Ok(ColorState::Message),
        "focus-visible" => Ok(ColorState::FocusVisible),
        "overlay" => Ok(ColorState::Overlay),
        "modal-overlay" => Ok(ColorState::ModalOverlay),
        _ => Err(ThemeLoadError::UnsupportedColorState { state }),
    }
}

fn parse_rgb(value: String) -> Result<u32, ThemeLoadError> {
    let hex = value
        .strip_prefix('#')
        .or_else(|| value.strip_prefix("0x"))
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value.as_str());
    if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ThemeLoadError::InvalidRgb { value });
    }
    u32::from_str_radix(hex, 16).map_err(|_| ThemeLoadError::InvalidRgb { value })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileDocument {
    schema_version: Option<u32>,
    id: Option<String>,
    label: Option<String>,
    mode: Option<String>,
    revision: Option<u64>,
    fallback_mode: Option<String>,
    colors: Option<Vec<ThemeFileColorDocument>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileColorDocument {
    token: Option<String>,
    state: Option<String>,
    rgb: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonSchemaFile {
    schema_version: u32,
    id: String,
    label: String,
    mode: ThemeJsonMode,
    revision: u64,
    fallback_mode: Option<ThemeJsonMode>,
    colors: Vec<ThemeJsonColorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonColorEntry {
    token: ThemeJsonToken,
    state: ThemeJsonColorState,
    rgb: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ThemeJsonMode {
    Light,
    Dark,
    HighContrast,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
enum ThemeJsonToken {
    #[serde(rename = "semantic.surface")]
    Surface,
    #[serde(rename = "semantic.surface_muted")]
    SurfaceMuted,
    #[serde(rename = "semantic.border")]
    Border,
    #[serde(rename = "semantic.text")]
    Text,
    #[serde(rename = "semantic.text_muted")]
    TextMuted,
    #[serde(rename = "semantic.accent")]
    Accent,
    #[serde(rename = "semantic.accent_foreground")]
    AccentForeground,
    #[serde(rename = "semantic.focus_ring")]
    FocusRing,
    #[serde(rename = "semantic.destructive")]
    Destructive,
    #[serde(rename = "semantic.destructive_foreground")]
    DestructiveForeground,
    #[serde(rename = "semantic.overlay")]
    Overlay,
    #[serde(rename = "semantic.modal_overlay")]
    ModalOverlay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ThemeJsonColorState {
    Default,
    Hover,
    Selected,
    Disabled,
    ReadOnly,
    Invalid,
    Required,
    Placeholder,
    Message,
    FocusVisible,
    Overlay,
    ModalOverlay,
}
