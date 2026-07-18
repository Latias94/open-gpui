use std::collections::BTreeSet;
use std::path::Path;

use open_gpui_motion::MotionPreference;
use open_gpui_ui_core::{
    Density, SizeScale, ThemeDesignScales, ThemeElevationLayer, ThemeElevationScale,
    ThemeRadiusScale, ThemeSpacingScale, ThemeTypographyScale, TokenKey, semantic,
};
use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};

use crate::color::ColorState;

use super::palette::COMPLETE_THEME_COLOR_COUNT;
use super::registry::{ThemeDefinition, ThemeRegistry, ThemeRegistryEntry, ThemeValidationError};
use super::snapshot::{ThemeColor, ThemeMode};

/// Current JSON schema version supported by the complete theme file loader.
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
    /// Source-file revision metadata.
    Revision,
    /// Complete color entry array.
    Colors,
    /// Complete non-color design object.
    Design,
    /// Typography scale object.
    Typography,
    /// Control text size scale.
    ControlText,
    /// Control line-height scale.
    ControlLineHeight,
    /// Spacing scale object.
    Spacing,
    /// Inline control spacing scale.
    ControlInline,
    /// Block control spacing scale.
    ControlBlock,
    /// Radius scale object.
    Radius,
    /// Control radius scale.
    ControlRadius,
    /// Elevation scale object.
    Elevation,
    /// Two-layer elevated overlay recipe.
    OverlayElevation,
    /// Horizontal elevation offset.
    ElevationOffsetX,
    /// Vertical elevation offset.
    ElevationOffsetY,
    /// Elevation blur radius.
    ElevationBlurRadius,
    /// Elevation spread radius.
    ElevationSpreadRadius,
    /// Elevation opacity percentage.
    ElevationOpacityPercent,
    /// Theme density default.
    Density,
    /// Theme motion policy.
    MotionPolicy,
    /// Extra-small size value.
    XSmall,
    /// Small size value.
    Small,
    /// Medium size value.
    Medium,
    /// Large size value.
    Large,
    /// Per-color semantic token key.
    ColorToken,
    /// Per-color state key.
    ColorState,
    /// Per-color RGB value.
    ColorRgb,
}

/// Structured failure emitted while loading a complete Theme v1 JSON file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeLoadError {
    /// The file could not be read.
    ReadFile {
        /// Path passed to the loader.
        path: String,
    },
    /// The JSON was syntactically invalid or had an unsupported shape or type.
    InvalidJson {
        /// One-based line number reported by the JSON parser.
        line: usize,
        /// One-based column number reported by the JSON parser.
        column: usize,
    },
    /// A required field is missing or blank.
    MissingField(ThemeFileField),
    /// A required color-entry field is missing or blank.
    MissingColorField {
        /// Zero-based color entry index.
        index: usize,
        /// Missing color-entry field.
        field: ThemeFileField,
    },
    /// A required field is missing from an elevation layer.
    MissingElevationField {
        /// Zero-based elevation layer index.
        index: usize,
        /// Missing elevation-layer field.
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
    /// The file references a density outside the supported vocabulary.
    UnsupportedDensity {
        /// Unsupported density string.
        density: String,
    },
    /// The file references a motion policy outside the supported vocabulary.
    UnsupportedMotionPolicy {
        /// Unsupported motion policy string.
        policy: String,
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
    /// The elevated overlay recipe does not contain exactly two layers.
    InvalidElevationLayerCount {
        /// Actual layer count.
        count: usize,
    },
    /// An elevation opacity is outside zero through one hundred percent.
    InvalidElevationOpacity {
        /// Zero-based elevation layer index.
        index: usize,
        /// Invalid percentage.
        value: u8,
    },
    /// The parsed definition failed complete registry validation.
    Registration(ThemeValidationError),
}

/// Returns the JSON schema for complete portable Theme v1 definition files.
pub fn theme_json_schema() -> Schema {
    schema_for!(ThemeJsonSchemaFile)
}

/// Serializes one registered complete Theme v1 entry.
pub fn theme_json_string(entry: &ThemeRegistryEntry) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&ThemeJsonSchemaFile::from(entry))
}

/// Parses and validates the shape of a complete Theme v1 JSON definition string.
pub fn theme_definition_from_json_str(source: &str) -> Result<ThemeDefinition, ThemeLoadError> {
    let document = serde_json::from_str::<ThemeFileDocument>(source).map_err(|error| {
        ThemeLoadError::InvalidJson {
            line: error.line(),
            column: error.column(),
        }
    })?;
    theme_definition_from_document(document)
}

/// Reads and parses a complete Theme v1 JSON definition file.
pub fn theme_definition_from_json_file(
    path: impl AsRef<Path>,
) -> Result<ThemeDefinition, ThemeLoadError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|_| ThemeLoadError::ReadFile {
        path: path.display().to_string(),
    })?;
    theme_definition_from_json_str(&source)
}

/// Parses and atomically registers a complete Theme v1 JSON definition string.
pub fn register_theme_json_str<'a>(
    registry: &'a mut ThemeRegistry,
    source: &str,
) -> Result<&'a ThemeRegistryEntry, ThemeLoadError> {
    let definition = theme_definition_from_json_str(source)?;
    registry
        .register(definition)
        .map_err(ThemeLoadError::Registration)
}

/// Reads, parses, and atomically registers a complete Theme v1 JSON definition file.
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
    let mode = parse_theme_mode(required_theme_file_literal(
        document.mode,
        ThemeFileField::Mode,
    )?)?;
    let source_revision = document
        .revision
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::Revision))?;
    let colors = parse_theme_colors(document.colors)?;
    let design_scales = parse_design_scales(
        document
            .design
            .ok_or(ThemeLoadError::MissingField(ThemeFileField::Design))?,
    )?;

    ThemeDefinition::new(id, label, mode, source_revision)
        .design_scales(design_scales)
        .colors(colors)
        .validate_complete()
        .map_err(ThemeLoadError::Registration)
}

fn parse_design_scales(
    design: ThemeFileDesignDocument,
) -> Result<ThemeDesignScales, ThemeLoadError> {
    let typography = design
        .typography
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::Typography))?;
    let spacing = design
        .spacing
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::Spacing))?;
    let radius = design
        .radius
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::Radius))?;
    let elevation = design
        .elevation
        .ok_or(ThemeLoadError::MissingField(ThemeFileField::Elevation))?;
    let density = parse_density(required_theme_file_literal(
        design.density,
        ThemeFileField::Density,
    )?)?;
    let motion = parse_motion_policy(required_theme_file_literal(
        design.motion_policy,
        ThemeFileField::MotionPolicy,
    )?)?;

    Ok(ThemeDesignScales::new(
        ThemeTypographyScale::new(
            parse_size_scale(
                typography
                    .control_text
                    .ok_or(ThemeLoadError::MissingField(ThemeFileField::ControlText))?,
            )?,
            parse_size_scale(typography.control_line_height.ok_or(
                ThemeLoadError::MissingField(ThemeFileField::ControlLineHeight),
            )?)?,
        ),
        ThemeSpacingScale::new(
            parse_size_scale(
                spacing
                    .control_inline
                    .ok_or(ThemeLoadError::MissingField(ThemeFileField::ControlInline))?,
            )?,
            parse_size_scale(
                spacing
                    .control_block
                    .ok_or(ThemeLoadError::MissingField(ThemeFileField::ControlBlock))?,
            )?,
        ),
        ThemeRadiusScale::new(parse_size_scale(
            radius
                .control
                .ok_or(ThemeLoadError::MissingField(ThemeFileField::ControlRadius))?,
        )?),
        parse_elevation_scale(elevation)?,
        density,
        motion,
    ))
}

fn parse_size_scale(scale: ThemeFileSizeScale) -> Result<SizeScale, ThemeLoadError> {
    Ok(SizeScale::new(
        scale
            .xsmall
            .ok_or(ThemeLoadError::MissingField(ThemeFileField::XSmall))?,
        scale
            .small
            .ok_or(ThemeLoadError::MissingField(ThemeFileField::Small))?,
        scale
            .medium
            .ok_or(ThemeLoadError::MissingField(ThemeFileField::Medium))?,
        scale
            .large
            .ok_or(ThemeLoadError::MissingField(ThemeFileField::Large))?,
    ))
}

fn parse_elevation_scale(
    elevation: ThemeFileElevationDocument,
) -> Result<ThemeElevationScale, ThemeLoadError> {
    let overlay = elevation.overlay.ok_or(ThemeLoadError::MissingField(
        ThemeFileField::OverlayElevation,
    ))?;
    if overlay.len() != 2 {
        return Err(ThemeLoadError::InvalidElevationLayerCount {
            count: overlay.len(),
        });
    }
    let mut layers = Vec::with_capacity(2);
    for (index, layer) in overlay.into_iter().enumerate() {
        let offset_x =
            required_elevation_value(layer.offset_x, index, ThemeFileField::ElevationOffsetX)?;
        let offset_y =
            required_elevation_value(layer.offset_y, index, ThemeFileField::ElevationOffsetY)?;
        let blur_radius = required_elevation_value(
            layer.blur_radius,
            index,
            ThemeFileField::ElevationBlurRadius,
        )?;
        let spread_radius = required_elevation_value(
            layer.spread_radius,
            index,
            ThemeFileField::ElevationSpreadRadius,
        )?;
        let opacity_percent = required_elevation_value(
            layer.opacity_percent,
            index,
            ThemeFileField::ElevationOpacityPercent,
        )?;
        if opacity_percent > 100 {
            return Err(ThemeLoadError::InvalidElevationOpacity {
                index,
                value: opacity_percent,
            });
        }
        layers.push(ThemeElevationLayer::new(
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            opacity_percent,
        ));
    }
    Ok(ThemeElevationScale::new(
        layers
            .try_into()
            .expect("the elevated overlay layer count was validated"),
    ))
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

fn required_theme_file_literal(
    value: Option<String>,
    field: ThemeFileField,
) -> Result<String, ThemeLoadError> {
    let value = value.ok_or(ThemeLoadError::MissingField(field))?;
    if value.trim().is_empty() {
        Err(ThemeLoadError::MissingField(field))
    } else {
        Ok(value)
    }
}

fn required_color_string(
    value: Option<String>,
    index: usize,
    field: ThemeFileField,
) -> Result<String, ThemeLoadError> {
    let value = value.ok_or(ThemeLoadError::MissingColorField { index, field })?;
    if value.trim().is_empty() {
        Err(ThemeLoadError::MissingColorField { index, field })
    } else {
        Ok(value)
    }
}

fn required_elevation_value<T>(
    value: Option<T>,
    index: usize,
    field: ThemeFileField,
) -> Result<T, ThemeLoadError> {
    value.ok_or(ThemeLoadError::MissingElevationField { index, field })
}

fn parse_theme_mode(mode: String) -> Result<ThemeMode, ThemeLoadError> {
    match mode.as_str() {
        "light" => Ok(ThemeMode::Light),
        "dark" => Ok(ThemeMode::Dark),
        "high-contrast" => Ok(ThemeMode::HighContrast),
        _ => Err(ThemeLoadError::UnsupportedMode { mode }),
    }
}

fn parse_density(density: String) -> Result<Density, ThemeLoadError> {
    match density.as_str() {
        "compact" => Ok(Density::Compact),
        "comfortable" => Ok(Density::Comfortable),
        "spacious" => Ok(Density::Spacious),
        _ => Err(ThemeLoadError::UnsupportedDensity { density }),
    }
}

fn parse_motion_policy(policy: String) -> Result<MotionPreference, ThemeLoadError> {
    match policy.as_str() {
        "animated" => Ok(MotionPreference::Animated),
        "reduced" => Ok(MotionPreference::Reduced),
        _ => Err(ThemeLoadError::UnsupportedMotionPolicy { policy }),
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
    let hex = value.strip_prefix('#').unwrap_or(value.as_str());
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
    colors: Option<Vec<ThemeFileColorDocument>>,
    design: Option<ThemeFileDesignDocument>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileColorDocument {
    token: Option<String>,
    state: Option<String>,
    rgb: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileDesignDocument {
    typography: Option<ThemeFileTypographyDocument>,
    spacing: Option<ThemeFileSpacingDocument>,
    radius: Option<ThemeFileRadiusDocument>,
    elevation: Option<ThemeFileElevationDocument>,
    density: Option<String>,
    motion_policy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileTypographyDocument {
    control_text: Option<ThemeFileSizeScale>,
    control_line_height: Option<ThemeFileSizeScale>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileSpacingDocument {
    control_inline: Option<ThemeFileSizeScale>,
    control_block: Option<ThemeFileSizeScale>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileRadiusDocument {
    control: Option<ThemeFileSizeScale>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileSizeScale {
    xsmall: Option<u16>,
    small: Option<u16>,
    medium: Option<u16>,
    large: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileElevationDocument {
    overlay: Option<Vec<ThemeFileElevationLayer>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFileElevationLayer {
    offset_x: Option<i16>,
    offset_y: Option<i16>,
    blur_radius: Option<u16>,
    spread_radius: Option<i16>,
    opacity_percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonSchemaFile {
    #[schemars(range(min = THEME_JSON_SCHEMA_VERSION, max = THEME_JSON_SCHEMA_VERSION))]
    schema_version: u32,
    #[schemars(length(min = 1), regex(pattern = r"\S"))]
    id: String,
    #[schemars(length(min = 1), regex(pattern = r"\S"))]
    label: String,
    mode: ThemeJsonMode,
    revision: u64,
    #[schemars(length(equal = COMPLETE_THEME_COLOR_COUNT))]
    colors: Vec<ThemeJsonColorEntry>,
    design: ThemeJsonDesignScales,
}

impl From<&ThemeRegistryEntry> for ThemeJsonSchemaFile {
    fn from(entry: &ThemeRegistryEntry) -> Self {
        let snapshot = entry.snapshot();
        Self {
            schema_version: THEME_JSON_SCHEMA_VERSION,
            id: entry.id().to_owned(),
            label: entry.label().to_owned(),
            mode: ThemeJsonMode::from(snapshot.mode()),
            revision: snapshot.source_revision(),
            colors: snapshot
                .colors()
                .iter()
                .copied()
                .map(ThemeJsonColorEntry::from)
                .collect(),
            design: ThemeJsonDesignScales::from(snapshot.design_scales()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonColorEntry {
    token: ThemeJsonToken,
    state: ThemeJsonColorState,
    #[schemars(regex(pattern = r"^#?[0-9A-Fa-f]{6}$"))]
    rgb: String,
}

impl From<ThemeColor> for ThemeJsonColorEntry {
    fn from(color: ThemeColor) -> Self {
        Self {
            token: ThemeJsonToken::from(color.token()),
            state: ThemeJsonColorState::from(color.state()),
            rgb: format!("#{:06x}", color.rgb()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonDesignScales {
    typography: ThemeJsonTypographyScale,
    spacing: ThemeJsonSpacingScale,
    radius: ThemeJsonRadiusScale,
    elevation: ThemeJsonElevationScale,
    density: ThemeJsonDensity,
    motion_policy: ThemeJsonMotionPolicy,
}

impl From<ThemeDesignScales> for ThemeJsonDesignScales {
    fn from(scales: ThemeDesignScales) -> Self {
        Self {
            typography: ThemeJsonTypographyScale {
                control_text: ThemeJsonSizeScale::from(scales.typography().control_text()),
                control_line_height: ThemeJsonSizeScale::from(
                    scales.typography().control_line_height(),
                ),
            },
            spacing: ThemeJsonSpacingScale {
                control_inline: ThemeJsonSizeScale::from(scales.spacing().control_inline()),
                control_block: ThemeJsonSizeScale::from(scales.spacing().control_block()),
            },
            radius: ThemeJsonRadiusScale {
                control: ThemeJsonSizeScale::from(scales.radius().control()),
            },
            elevation: ThemeJsonElevationScale {
                overlay: scales
                    .elevation()
                    .overlay()
                    .map(ThemeJsonElevationLayer::from),
            },
            density: ThemeJsonDensity::from(scales.density()),
            motion_policy: ThemeJsonMotionPolicy::from(scales.motion()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonTypographyScale {
    control_text: ThemeJsonSizeScale,
    control_line_height: ThemeJsonSizeScale,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonSpacingScale {
    control_inline: ThemeJsonSizeScale,
    control_block: ThemeJsonSizeScale,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonRadiusScale {
    control: ThemeJsonSizeScale,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonSizeScale {
    xsmall: u16,
    small: u16,
    medium: u16,
    large: u16,
}

impl From<SizeScale> for ThemeJsonSizeScale {
    fn from(scale: SizeScale) -> Self {
        let [xsmall, small, medium, large] = scale.raw_values();
        Self {
            xsmall,
            small,
            medium,
            large,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonElevationScale {
    overlay: [ThemeJsonElevationLayer; 2],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeJsonElevationLayer {
    offset_x: i16,
    offset_y: i16,
    blur_radius: u16,
    spread_radius: i16,
    #[schemars(range(min = 0, max = 100))]
    opacity_percent: u8,
}

impl From<ThemeElevationLayer> for ThemeJsonElevationLayer {
    fn from(layer: ThemeElevationLayer) -> Self {
        let (offset_x, offset_y, blur_radius, spread_radius, opacity_percent) = layer.raw_values();
        Self {
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            opacity_percent,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ThemeJsonMode {
    Light,
    Dark,
    HighContrast,
}

impl From<ThemeMode> for ThemeJsonMode {
    fn from(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
            ThemeMode::HighContrast => Self::HighContrast,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ThemeJsonDensity {
    Compact,
    Comfortable,
    Spacious,
}

impl From<Density> for ThemeJsonDensity {
    fn from(density: Density) -> Self {
        match density {
            Density::Compact => Self::Compact,
            Density::Comfortable => Self::Comfortable,
            Density::Spacious => Self::Spacious,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ThemeJsonMotionPolicy {
    Animated,
    Reduced,
}

impl From<MotionPreference> for ThemeJsonMotionPolicy {
    fn from(preference: MotionPreference) -> Self {
        match preference {
            MotionPreference::Animated => Self::Animated,
            MotionPreference::Reduced => Self::Reduced,
        }
    }
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

impl From<TokenKey> for ThemeJsonToken {
    fn from(token: TokenKey) -> Self {
        match token.as_str() {
            "semantic.surface" => Self::Surface,
            "semantic.surface_muted" => Self::SurfaceMuted,
            "semantic.border" => Self::Border,
            "semantic.text" => Self::Text,
            "semantic.text_muted" => Self::TextMuted,
            "semantic.accent" => Self::Accent,
            "semantic.accent_foreground" => Self::AccentForeground,
            "semantic.focus_ring" => Self::FocusRing,
            "semantic.destructive" => Self::Destructive,
            "semantic.destructive_foreground" => Self::DestructiveForeground,
            "semantic.overlay" => Self::Overlay,
            "semantic.modal_overlay" => Self::ModalOverlay,
            _ => unreachable!("registered complete theme contains an unsupported token"),
        }
    }
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

impl From<ColorState> for ThemeJsonColorState {
    fn from(state: ColorState) -> Self {
        match state {
            ColorState::Default => Self::Default,
            ColorState::Hover => Self::Hover,
            ColorState::Selected => Self::Selected,
            ColorState::Disabled => Self::Disabled,
            ColorState::ReadOnly => Self::ReadOnly,
            ColorState::Invalid => Self::Invalid,
            ColorState::Required => Self::Required,
            ColorState::Placeholder => Self::Placeholder,
            ColorState::Message => Self::Message,
            ColorState::FocusVisible => Self::FocusVisible,
            ColorState::Overlay => Self::Overlay,
            ColorState::ModalOverlay => Self::ModalOverlay,
        }
    }
}
