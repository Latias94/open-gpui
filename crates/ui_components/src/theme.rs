//! Theme resolution helpers for component color intents.

use open_gpui::{Rgba, rgb};
use open_gpui_ui_core::{ThemeTokens, TokenKey, semantic};

use crate::alert_dialog::{AlertDialogColors, AlertDialogIntent};
use crate::avatar::{AvatarColors, AvatarGroupCountColors};
use crate::badge::{BadgeColors, BadgeVariant};
use crate::button::{ButtonColors, ButtonVariant};
use crate::checkbox::CheckboxColors;
use crate::color::{ColorIntent, ColorState};
use crate::dialog::DialogColors;
use crate::feedback::{FeedbackColors, FeedbackIntent};
use crate::field::FieldColors;
use crate::hover_card::HoverCardColors;
use crate::kbd::KbdColors;
use crate::label::LabelColors;
use crate::popover::PopoverColors;
use crate::progress::ProgressColors;
use crate::radio::RadioGroupColors;
use crate::separator::SeparatorColors;
use crate::sheet::SheetColors;
use crate::skeleton::SkeletonColors;
use crate::switch::SwitchColors;
use crate::text_input::TextInputColors;
use crate::tooltip::TooltipColors;

const DEFAULT_SURFACE: u32 = 0xffffff;
const DEFAULT_GHOST_SURFACE: u32 = 0xf6f7f2;
const DEFAULT_BORDER: u32 = 0xcfd5cc;
const DEFAULT_TEXT: u32 = 0x18202a;
const DEFAULT_ACCENT: u32 = 0x1f7a66;
const DEFAULT_ACCENT_HOVER: u32 = 0x176656;
const DEFAULT_ACCENT_FOREGROUND: u32 = 0xffffff;
const DEFAULT_SWITCH_THUMB_SURFACE: u32 = 0xffffff;
const DEFAULT_FOCUS_RING: u32 = 0x2f80ed;
const DEFAULT_DESTRUCTIVE: u32 = 0xb42318;
const DEFAULT_DESTRUCTIVE_HOVER: u32 = 0x971b12;
const DEFAULT_DESTRUCTIVE_FOREGROUND: u32 = 0xffffff;
const DEFAULT_READ_ONLY_SURFACE: u32 = 0xf8f9f5;
const DEFAULT_PLACEHOLDER: u32 = 0x6d7785;
const DEFAULT_MESSAGE: u32 = 0x5a6472;
const DEFAULT_TEXT_MUTED: u32 = 0x5a6472;

const LIGHT_THEME_REVISION: u64 = 1;
const DARK_THEME_REVISION: u64 = 2;
const HIGH_CONTRAST_THEME_REVISION: u64 = 3;

/// A stable color mode supplied by a component theme snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Light color mode.
    #[default]
    Light,
    /// Dark color mode.
    Dark,
    /// High contrast color mode.
    HighContrast,
}

impl ThemeMode {
    /// Returns the stable mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high-contrast",
        }
    }
}

/// One runtime theme table color entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColor {
    token: TokenKey,
    state: ColorState,
    rgb: u32,
}

impl ThemeColor {
    /// Creates a theme color entry for a semantic token and state.
    pub const fn new(token: TokenKey, state: ColorState, rgb: u32) -> Self {
        Self { token, state, rgb }
    }

    /// Returns the semantic token key.
    pub const fn token(self) -> TokenKey {
        self.token
    }

    /// Returns the component color state.
    pub const fn state(self) -> ColorState {
        self.state
    }

    /// Returns the RGB color without alpha.
    pub const fn rgb(self) -> u32 {
        self.rgb
    }
}

/// Immutable runtime view of a component theme table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSnapshot<'a> {
    mode: ThemeMode,
    revision: u64,
    colors: &'a [ThemeColor],
}

impl<'a> ThemeSnapshot<'a> {
    /// Creates a theme snapshot from a mode, revision, and color table.
    pub const fn new(mode: ThemeMode, revision: u64, colors: &'a [ThemeColor]) -> Self {
        Self {
            mode,
            revision,
            colors,
        }
    }

    /// Returns the color mode for this snapshot.
    pub const fn mode(self) -> ThemeMode {
        self.mode
    }

    /// Returns the revision used by callers to invalidate cached resolutions.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Returns the raw color table.
    pub const fn colors(self) -> &'a [ThemeColor] {
        self.colors
    }

    /// Looks up an RGB color for a token and state.
    pub fn color_rgb(self, token: TokenKey, state: ColorState) -> Option<u32> {
        self.colors
            .iter()
            .find(|entry| entry.token == token && entry.state == state)
            .map(|entry| entry.rgb)
            .or_else(|| {
                self.colors
                    .iter()
                    .find(|entry| entry.token == token && entry.state == ColorState::Default)
                    .map(|entry| entry.rgb)
            })
    }

    /// Resolves a color intent to an RGB value, falling back to the intent RGB when needed.
    pub fn resolve_rgb(self, intent: ColorIntent) -> u32 {
        self.color_rgb(intent.token(), intent.state())
            .unwrap_or_else(|| intent.fallback_rgb())
    }

    /// Resolves a color intent to a concrete GPUI color.
    pub fn resolve(self, intent: ColorIntent) -> Rgba {
        rgb(self.resolve_rgb(intent))
    }
}

impl ThemeSnapshot<'static> {
    /// Returns the default light theme snapshot.
    pub const fn light() -> Self {
        Self::new(ThemeMode::Light, LIGHT_THEME_REVISION, LIGHT_THEME_COLORS)
    }

    /// Returns the default dark theme snapshot.
    pub const fn dark() -> Self {
        Self::new(ThemeMode::Dark, DARK_THEME_REVISION, DARK_THEME_COLORS)
    }

    /// Returns the default high contrast theme snapshot.
    pub const fn high_contrast() -> Self {
        Self::new(
            ThemeMode::HighContrast,
            HIGH_CONTRAST_THEME_REVISION,
            HIGH_CONTRAST_THEME_COLORS,
        )
    }
}

impl Default for ThemeSnapshot<'static> {
    fn default() -> Self {
        Self::light()
    }
}

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

/// Theme resolution namespace for component color intents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeResolver;

impl ThemeResolver {
    /// Resolves a color intent with the default light theme snapshot.
    pub fn resolve(intent: ColorIntent) -> Rgba {
        Self::resolve_with(intent, ThemeSnapshot::light())
    }

    /// Resolves a color intent with an explicit theme snapshot.
    pub fn resolve_with(intent: ColorIntent, theme: ThemeSnapshot<'_>) -> Rgba {
        theme.resolve(intent)
    }

    /// Resolves a color intent by using only the fallback RGB.
    pub fn resolve_fallback(intent: ColorIntent) -> Rgba {
        rgb(intent.fallback_rgb())
    }

    pub(crate) const fn button_colors(
        tokens: ThemeTokens,
        variant: ButtonVariant,
        selected: bool,
    ) -> ButtonColors {
        if selected {
            return Self::accent_button_colors(tokens, ColorState::Selected);
        }

        match variant {
            ButtonVariant::Default => Self::accent_button_colors(tokens, ColorState::Default),
            ButtonVariant::Secondary => ButtonColors {
                background: ColorIntent::new(tokens.surface_muted, 0xe8ede6),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, 0xd6d8ce),
                hover_background: ColorIntent::with_state(
                    tokens.surface_muted,
                    ColorState::Hover,
                    0xdfe6dc,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
            ButtonVariant::Outline => ButtonColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
                hover_background: ColorIntent::with_state(
                    tokens.surface_muted,
                    ColorState::Hover,
                    0xf1f5ee,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
            ButtonVariant::Ghost => ButtonColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_GHOST_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.surface, DEFAULT_GHOST_SURFACE),
                hover_background: ColorIntent::with_state(
                    tokens.surface_muted,
                    ColorState::Hover,
                    0xe8ede6,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
            ButtonVariant::Destructive => ButtonColors {
                background: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                foreground: ColorIntent::new(
                    tokens.destructive_foreground,
                    DEFAULT_DESTRUCTIVE_FOREGROUND,
                ),
                border: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                hover_background: ColorIntent::with_state(
                    tokens.destructive,
                    ColorState::Hover,
                    DEFAULT_DESTRUCTIVE_HOVER,
                ),
                focus_ring: ColorIntent::with_state(
                    tokens.focus_ring,
                    ColorState::FocusVisible,
                    DEFAULT_FOCUS_RING,
                ),
            },
        }
    }

    pub(crate) const fn badge_colors(tokens: ThemeTokens, variant: BadgeVariant) -> BadgeColors {
        match variant {
            BadgeVariant::Default => BadgeColors {
                background: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
                foreground: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
                border: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            },
            BadgeVariant::Secondary => BadgeColors {
                background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            },
            BadgeVariant::Destructive => BadgeColors {
                background: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
                foreground: ColorIntent::new(
                    tokens.destructive_foreground,
                    DEFAULT_DESTRUCTIVE_FOREGROUND,
                ),
                border: ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE),
            },
            BadgeVariant::Outline => BadgeColors {
                background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
                foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
                border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            },
        }
    }

    pub(crate) const fn switch_colors(tokens: ThemeTokens, checked: bool) -> SwitchColors {
        let track_token = if checked {
            tokens.accent
        } else {
            tokens.surface_muted
        };
        let track_fallback = if checked { DEFAULT_ACCENT } else { 0xdfe6dc };
        let border_token = if checked {
            tokens.accent
        } else {
            tokens.border
        };
        let border_fallback = if checked {
            DEFAULT_ACCENT
        } else {
            DEFAULT_BORDER
        };

        SwitchColors {
            track: ColorIntent::with_state(
                track_token,
                if checked {
                    ColorState::Selected
                } else {
                    ColorState::Default
                },
                track_fallback,
            ),
            thumb: ColorIntent::new(tokens.surface, DEFAULT_SWITCH_THUMB_SURFACE),
            border: ColorIntent::with_state(
                border_token,
                if checked {
                    ColorState::Selected
                } else {
                    ColorState::Default
                },
                border_fallback,
            ),
            label: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn text_input_colors(
        tokens: ThemeTokens,
        disabled: bool,
        read_only: bool,
        invalid: bool,
    ) -> TextInputColors {
        let border = if invalid {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, DEFAULT_DESTRUCTIVE)
        } else {
            ColorIntent::new(tokens.border, DEFAULT_BORDER)
        };
        let foreground = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };
        let background = if disabled {
            ColorIntent::with_state(tokens.surface_muted, ColorState::Disabled, 0xf1f5ee)
        } else if read_only {
            ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::ReadOnly,
                DEFAULT_READ_ONLY_SURFACE,
            )
        } else {
            ColorIntent::new(tokens.surface, DEFAULT_SURFACE)
        };

        TextInputColors {
            background,
            foreground,
            placeholder: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Placeholder,
                DEFAULT_PLACEHOLDER,
            ),
            border,
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn textarea_colors(
        tokens: ThemeTokens,
        disabled: bool,
        read_only: bool,
        invalid: bool,
    ) -> crate::textarea::TextareaColors {
        let colors = Self::text_input_colors(tokens, disabled, read_only, invalid);
        crate::textarea::TextareaColors {
            background: colors.background,
            foreground: colors.foreground,
            placeholder: colors.placeholder,
            border: colors.border,
            focus_ring: colors.focus_ring,
        }
    }

    pub(crate) const fn field_colors(
        tokens: ThemeTokens,
        disabled: bool,
        invalid: bool,
    ) -> FieldColors {
        let message = if invalid {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, DEFAULT_DESTRUCTIVE)
        } else {
            ColorIntent::with_state(tokens.text_muted, ColorState::Message, DEFAULT_MESSAGE)
        };
        let label = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };

        FieldColors {
            label,
            message,
            required_marker: ColorIntent::with_state(
                tokens.destructive,
                ColorState::Required,
                DEFAULT_DESTRUCTIVE,
            ),
        }
    }

    pub(crate) const fn checkbox_colors(
        tokens: ThemeTokens,
        checked: bool,
        indeterminate: bool,
        disabled: bool,
        invalid: bool,
    ) -> CheckboxColors {
        let selected = checked || indeterminate;
        let background = if disabled {
            ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Disabled,
                DEFAULT_READ_ONLY_SURFACE,
            )
        } else if selected {
            ColorIntent::with_state(tokens.accent, ColorState::Selected, DEFAULT_ACCENT)
        } else {
            ColorIntent::new(tokens.surface, DEFAULT_SURFACE)
        };
        let hover_background = if disabled {
            background
        } else if selected {
            ColorIntent::with_state(tokens.accent, ColorState::Hover, DEFAULT_ACCENT_HOVER)
        } else {
            ColorIntent::with_state(tokens.surface_muted, ColorState::Hover, 0xdfe6dc)
        };
        let border = if invalid {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, DEFAULT_DESTRUCTIVE)
        } else if selected {
            ColorIntent::with_state(tokens.accent, ColorState::Selected, DEFAULT_ACCENT)
        } else {
            ColorIntent::new(tokens.border, DEFAULT_BORDER)
        };
        let indicator = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND)
        };
        let label = if disabled {
            ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
        } else {
            ColorIntent::new(tokens.text, DEFAULT_TEXT)
        };

        CheckboxColors {
            background,
            hover_background,
            border,
            indicator,
            label,
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn radio_group_colors(tokens: ThemeTokens) -> RadioGroupColors {
        RadioGroupColors {
            control_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            control_background_selected: ColorIntent::with_state(
                tokens.accent,
                ColorState::Selected,
                DEFAULT_ACCENT,
            ),
            control_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            control_border_selected: ColorIntent::with_state(
                tokens.accent,
                ColorState::Selected,
                DEFAULT_ACCENT,
            ),
            indicator: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
            label: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            label_muted: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                DEFAULT_TEXT_MUTED,
            ),
            hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn label_colors(tokens: ThemeTokens, disabled: bool) -> LabelColors {
        LabelColors {
            text: if disabled {
                ColorIntent::with_state(tokens.text_muted, ColorState::Disabled, 0x7a8491)
            } else {
                ColorIntent::new(tokens.text, DEFAULT_TEXT)
            },
            required_marker: ColorIntent::with_state(
                tokens.destructive,
                ColorState::Required,
                DEFAULT_DESTRUCTIVE,
            ),
        }
    }

    pub(crate) const fn avatar_colors(tokens: ThemeTokens) -> AvatarColors {
        AvatarColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn avatar_group_count_colors(tokens: ThemeTokens) -> AvatarGroupCountColors {
        AvatarGroupCountColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn separator_colors(tokens: ThemeTokens) -> SeparatorColors {
        SeparatorColors {
            line: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn kbd_colors(tokens: ThemeTokens) -> KbdColors {
        KbdColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn progress_colors(tokens: ThemeTokens) -> ProgressColors {
        ProgressColors {
            track: ColorIntent::new(tokens.surface_muted, 0xdfe6dc),
            indicator: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
        }
    }

    pub(crate) const fn skeleton_colors(tokens: ThemeTokens) -> SkeletonColors {
        SkeletonColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
        }
    }

    pub(crate) const fn feedback_colors(
        tokens: ThemeTokens,
        intent: FeedbackIntent,
    ) -> FeedbackColors {
        let marker = match intent {
            FeedbackIntent::Neutral => ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            FeedbackIntent::Info | FeedbackIntent::Success => {
                ColorIntent::new(tokens.accent, DEFAULT_ACCENT)
            }
            FeedbackIntent::Warning => {
                ColorIntent::with_state(tokens.text_muted, ColorState::Message, 0xbf8700)
            }
            FeedbackIntent::Danger => ColorIntent::with_state(
                tokens.destructive,
                ColorState::Invalid,
                DEFAULT_DESTRUCTIVE,
            ),
        };

        FeedbackColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            marker,
        }
    }

    pub(crate) const fn tooltip_colors(tokens: ThemeTokens) -> TooltipColors {
        TooltipColors {
            background: ColorIntent::with_state(tokens.overlay, ColorState::Overlay, 0x263240),
            foreground: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn popover_colors(tokens: ThemeTokens, open: bool) -> PopoverColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        PopoverColors {
            background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                DEFAULT_GHOST_SURFACE,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn hover_card_colors(tokens: ThemeTokens, open: bool) -> HoverCardColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        HoverCardColors {
            background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                DEFAULT_GHOST_SURFACE,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn dialog_colors(tokens: ThemeTokens, open: bool) -> DialogColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        DialogColors {
            barrier: ColorIntent::with_state(
                tokens.modal_overlay,
                ColorState::ModalOverlay,
                0x000000,
            ),
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                DEFAULT_GHOST_SURFACE,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn alert_dialog_colors(
        tokens: ThemeTokens,
        intent: AlertDialogIntent,
        open: bool,
    ) -> AlertDialogColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };
        let action_background = match intent {
            AlertDialogIntent::Default => ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            AlertDialogIntent::Destructive => {
                ColorIntent::new(tokens.destructive, DEFAULT_DESTRUCTIVE)
            }
        };
        let action_hover_background = match intent {
            AlertDialogIntent::Default => {
                ColorIntent::with_state(tokens.accent, ColorState::Hover, DEFAULT_ACCENT_HOVER)
            }
            AlertDialogIntent::Destructive => ColorIntent::with_state(
                tokens.destructive,
                ColorState::Hover,
                DEFAULT_DESTRUCTIVE_HOVER,
            ),
        };
        let action_foreground = match intent {
            AlertDialogIntent::Default => {
                ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND)
            }
            AlertDialogIntent::Destructive => ColorIntent::new(
                tokens.destructive_foreground,
                DEFAULT_DESTRUCTIVE_FOREGROUND,
            ),
        };

        AlertDialogColors {
            barrier: ColorIntent::with_state(
                tokens.modal_overlay,
                ColorState::ModalOverlay,
                0x000000,
            ),
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                DEFAULT_GHOST_SURFACE,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            action_background,
            action_hover_background,
            action_foreground,
            action_border: action_background,
            cancel_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            cancel_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            cancel_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            cancel_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    pub(crate) const fn sheet_colors(tokens: ThemeTokens, open: bool) -> SheetColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        SheetColors {
            barrier: ColorIntent::with_state(
                tokens.modal_overlay,
                ColorState::ModalOverlay,
                0x000000,
            ),
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                DEFAULT_GHOST_SURFACE,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            trigger_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            close_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            close_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            close_foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            close_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    const fn accent_button_colors(tokens: ThemeTokens, state: ColorState) -> ButtonColors {
        ButtonColors {
            background: ColorIntent::with_state(tokens.accent, state, DEFAULT_ACCENT),
            foreground: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
            border: ColorIntent::with_state(tokens.accent, state, DEFAULT_ACCENT),
            hover_background: ColorIntent::with_state(
                tokens.accent,
                ColorState::Hover,
                DEFAULT_ACCENT_HOVER,
            ),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
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

const fn builtin_theme_snapshot(mode: ThemeMode) -> ThemeSnapshot<'static> {
    match mode {
        ThemeMode::Light => ThemeSnapshot::light(),
        ThemeMode::Dark => ThemeSnapshot::dark(),
        ThemeMode::HighContrast => ThemeSnapshot::high_contrast(),
    }
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

const LIGHT_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, DEFAULT_SURFACE),
    ThemeColor::new(
        semantic::SURFACE_MUTED,
        ColorState::Default,
        DEFAULT_GHOST_SURFACE,
    ),
    ThemeColor::new(
        semantic::SURFACE_MUTED,
        ColorState::Selected,
        DEFAULT_GHOST_SURFACE,
    ),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xdfe6dc),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::FocusVisible, 0xe8ede6),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0xf1f5ee),
    ThemeColor::new(
        semantic::SURFACE_MUTED,
        ColorState::ReadOnly,
        DEFAULT_READ_ONLY_SURFACE,
    ),
    ThemeColor::new(semantic::BORDER, ColorState::Default, DEFAULT_BORDER),
    ThemeColor::new(semantic::TEXT, ColorState::Default, DEFAULT_TEXT),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Default, DEFAULT_MESSAGE),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Disabled, 0x7a8491),
    ThemeColor::new(
        semantic::TEXT_MUTED,
        ColorState::Placeholder,
        DEFAULT_PLACEHOLDER,
    ),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Message, DEFAULT_MESSAGE),
    ThemeColor::new(semantic::ACCENT, ColorState::Default, DEFAULT_ACCENT),
    ThemeColor::new(semantic::ACCENT, ColorState::Selected, DEFAULT_ACCENT),
    ThemeColor::new(semantic::ACCENT, ColorState::Hover, DEFAULT_ACCENT_HOVER),
    ThemeColor::new(
        semantic::ACCENT_FOREGROUND,
        ColorState::Default,
        DEFAULT_ACCENT_FOREGROUND,
    ),
    ThemeColor::new(
        semantic::FOCUS_RING,
        ColorState::Default,
        DEFAULT_FOCUS_RING,
    ),
    ThemeColor::new(
        semantic::FOCUS_RING,
        ColorState::FocusVisible,
        DEFAULT_FOCUS_RING,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Default,
        DEFAULT_DESTRUCTIVE,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Invalid,
        DEFAULT_DESTRUCTIVE,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Required,
        DEFAULT_DESTRUCTIVE,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE,
        ColorState::Hover,
        DEFAULT_DESTRUCTIVE_HOVER,
    ),
    ThemeColor::new(
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        DEFAULT_DESTRUCTIVE_FOREGROUND,
    ),
    ThemeColor::new(semantic::OVERLAY, ColorState::Default, 0x263240),
    ThemeColor::new(semantic::OVERLAY, ColorState::Overlay, 0x263240),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::Default, 0x111827),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::ModalOverlay, 0x111827),
];

const DARK_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0x121417),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Default, 0x20262d),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Selected, 0x20262d),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0x2a333d),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::FocusVisible, 0x27313b),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0x1a1f25),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::ReadOnly, 0x171b20),
    ThemeColor::new(semantic::BORDER, ColorState::Default, 0x3b4450),
    ThemeColor::new(semantic::TEXT, ColorState::Default, 0xf4f7fb),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Default, 0xb7c0cc),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Disabled, 0x7e8996),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Placeholder, 0x9aa5b2),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Message, 0xb7c0cc),
    ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x5ee0bc),
    ThemeColor::new(semantic::ACCENT, ColorState::Selected, 0x5ee0bc),
    ThemeColor::new(semantic::ACCENT, ColorState::Hover, 0x39caa7),
    ThemeColor::new(semantic::ACCENT_FOREGROUND, ColorState::Default, 0x05231c),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::Default, 0x7ab8ff),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::FocusVisible, 0x7ab8ff),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Default, 0xff6b5f),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Invalid, 0xff6b5f),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Required, 0xff6b5f),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Hover, 0xff4d3f),
    ThemeColor::new(
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        0x1c0705,
    ),
    ThemeColor::new(semantic::OVERLAY, ColorState::Default, 0x0b0f14),
    ThemeColor::new(semantic::OVERLAY, ColorState::Overlay, 0x0b0f14),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::ModalOverlay, 0x000000),
];

const HIGH_CONTRAST_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Default, 0xf0f0f0),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Selected, 0xf0f0f0),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xd9e8ff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::FocusVisible, 0xe0f0ff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0xe6e6e6),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::ReadOnly, 0xf7f7f7),
    ThemeColor::new(semantic::BORDER, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::TEXT, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Default, 0x222222),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Disabled, 0x555555),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Placeholder, 0x333333),
    ThemeColor::new(semantic::TEXT_MUTED, ColorState::Message, 0x222222),
    ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x005fcc),
    ThemeColor::new(semantic::ACCENT, ColorState::Selected, 0x005fcc),
    ThemeColor::new(semantic::ACCENT, ColorState::Hover, 0x004799),
    ThemeColor::new(semantic::ACCENT_FOREGROUND, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::Default, 0xffbf00),
    ThemeColor::new(semantic::FOCUS_RING, ColorState::FocusVisible, 0xffbf00),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Default, 0xd00000),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Invalid, 0xd00000),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Required, 0xd00000),
    ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Hover, 0x9f0000),
    ThemeColor::new(
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        0xffffff,
    ),
    ThemeColor::new(semantic::OVERLAY, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::OVERLAY, ColorState::Overlay, 0x000000),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::Default, 0x000000),
    ThemeColor::new(semantic::MODAL_OVERLAY, ColorState::ModalOverlay, 0x000000),
];
