use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use open_gpui::{App, Entity, Global, Rgba, Subscription, Window};

use crate::color::ColorIntent;

use super::registry::{ThemeDefinition, ThemeRegistry, ThemeValidationError};
use super::snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};

/// Stable id for the built-in light theme.
pub const LIGHT_THEME_ID: &str = "light";

/// Stable id for the built-in dark theme.
pub const DARK_THEME_ID: &str = "dark";

/// Stable id for the built-in high-contrast theme.
pub const HIGH_CONTRAST_THEME_ID: &str = "high-contrast";

/// The built-in fallback used when an application has no installed theme registry.
pub const DEFAULT_THEME_ID: &str = LIGHT_THEME_ID;

/// Failure returned when selecting a registered theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeSelectionError {
    /// No registered theme matched the requested id.
    UnknownThemeId(String),
}

impl ThemeSelectionError {
    /// Returns the missing theme id.
    pub fn theme_id(&self) -> &str {
        match self {
            Self::UnknownThemeId(theme_id) => theme_id,
        }
    }
}

/// Owned render-time view over an immutable theme snapshot.
///
/// The context owns an atomically shared color table so render and deferred paths can retain the
/// exact effective snapshot without borrowing the application registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeContext {
    mode: ThemeMode,
    revision: u64,
    colors: Arc<[ThemeColor]>,
}

static LIGHT_THEME_CONTEXT: LazyLock<ThemeContext> =
    LazyLock::new(|| ThemeContext::new(ThemeSnapshot::light()));
static DARK_THEME_CONTEXT: LazyLock<ThemeContext> =
    LazyLock::new(|| ThemeContext::new(ThemeSnapshot::dark()));
static HIGH_CONTRAST_THEME_CONTEXT: LazyLock<ThemeContext> =
    LazyLock::new(|| ThemeContext::new(ThemeSnapshot::high_contrast()));

impl ThemeContext {
    /// Creates an owned context from an immutable theme snapshot.
    pub fn new(snapshot: ThemeSnapshot<'_>) -> Self {
        Self {
            mode: snapshot.mode(),
            revision: snapshot.revision(),
            colors: snapshot.colors().to_vec().into(),
        }
    }

    /// Creates a context over the built-in light theme.
    pub fn light() -> Self {
        LazyLock::force(&LIGHT_THEME_CONTEXT).clone()
    }

    /// Creates a context over the built-in dark theme.
    pub fn dark() -> Self {
        LazyLock::force(&DARK_THEME_CONTEXT).clone()
    }

    /// Creates a context over the built-in high-contrast theme.
    pub fn high_contrast() -> Self {
        LazyLock::force(&HIGH_CONTRAST_THEME_CONTEXT).clone()
    }

    /// Returns an immutable snapshot over this owned context.
    pub fn snapshot(&self) -> ThemeSnapshot<'_> {
        ThemeSnapshot::new(self.mode, self.revision, &self.colors)
    }

    /// Returns the active theme mode.
    pub const fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// Returns the active theme revision.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Resolves a component color intent with this context.
    pub fn resolve(&self, intent: ColorIntent) -> Rgba {
        self.snapshot().resolve(intent)
    }
}

impl From<ThemeSnapshot<'_>> for ThemeContext {
    fn from(snapshot: ThemeSnapshot<'_>) -> Self {
        Self::new(snapshot)
    }
}

impl Default for ThemeContext {
    fn default() -> Self {
        Self::light()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AppThemeState {
    registry: ThemeRegistry,
    selected_id: String,
    selected_context: ThemeContext,
}

impl Global for AppThemeState {}

impl AppThemeState {
    fn new(
        registry: ThemeRegistry,
        selected_id: impl Into<String>,
    ) -> Result<Self, ThemeSelectionError> {
        let selected_id = selected_id.into();
        let selected_context = registry
            .snapshot(&selected_id)
            .map(ThemeContext::new)
            .ok_or_else(|| ThemeSelectionError::UnknownThemeId(selected_id.clone()))?;
        Ok(Self {
            registry,
            selected_id,
            selected_context,
        })
    }

    fn context_for(&self, theme_id: &str) -> Result<ThemeContext, ThemeSelectionError> {
        self.registry
            .snapshot(theme_id)
            .map(ThemeContext::new)
            .ok_or_else(|| ThemeSelectionError::UnknownThemeId(theme_id.to_owned()))
    }
}

impl Default for AppThemeState {
    fn default() -> Self {
        Self::new(ThemeRegistry::with_builtins(), DEFAULT_THEME_ID)
            .expect("the built-in default theme must be registered")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowThemeAuthority {
    InheritApp,
    Selected(String),
    Override,
}

struct WindowThemeState {
    authority: WindowThemeAuthority,
    base_context: ThemeContext,
    scope_stack: Rc<RefCell<Vec<ThemeContext>>>,
    _app_theme_subscription: Subscription,
}

impl WindowThemeState {
    fn new(window: &mut Window, cx: &mut open_gpui::Context<Self>) -> Self {
        let weak_state = cx.weak_entity();
        let app_theme_subscription =
            window.observe_global::<AppThemeState>(cx, move |window, cx| {
                let Some(state) = weak_state.upgrade() else {
                    return;
                };
                let authority = state.read(cx).authority.clone();
                if authority == WindowThemeAuthority::Override {
                    return;
                }
                let previous = state.read(cx).base_context.clone();
                let next = resolve_base_context(&authority, &previous, cx);
                let changed = replace_window_theme_base(&state, authority, next, cx);
                if changed {
                    window.refresh();
                }
            });
        Self {
            authority: WindowThemeAuthority::InheritApp,
            base_context: app_theme_context(cx),
            scope_stack: Rc::default(),
            _app_theme_subscription: app_theme_subscription,
        }
    }
}

fn window_theme_state(window: &mut Window, cx: &mut App) -> Entity<WindowThemeState> {
    window.use_window_state(cx, WindowThemeState::new)
}

fn replace_window_theme_base(
    state: &Entity<WindowThemeState>,
    authority: WindowThemeAuthority,
    context: ThemeContext,
    cx: &mut App,
) -> bool {
    let changed = {
        let state = state.read(cx);
        state.authority != authority || state.base_context != context
    };
    if changed {
        state.update(cx, |state, _| {
            state.authority = authority;
            state.base_context = context;
        });
    }
    changed
}

fn resolve_base_context(
    authority: &WindowThemeAuthority,
    previous: &ThemeContext,
    cx: &App,
) -> ThemeContext {
    match authority {
        WindowThemeAuthority::InheritApp => app_theme_context(cx),
        WindowThemeAuthority::Selected(theme_id) => {
            registered_theme_context(cx, theme_id).unwrap_or_else(|_| previous.clone())
        }
        WindowThemeAuthority::Override => {
            unreachable!("an explicit override keeps its owned context")
        }
    }
}

fn builtin_theme_context(theme_id: &str) -> Option<ThemeContext> {
    match theme_id {
        LIGHT_THEME_ID => Some(ThemeContext::light()),
        DARK_THEME_ID => Some(ThemeContext::dark()),
        HIGH_CONTRAST_THEME_ID => Some(ThemeContext::high_contrast()),
        _ => None,
    }
}

/// Returns the built-in theme id for a mode.
pub const fn theme_id_for_mode(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => LIGHT_THEME_ID,
        ThemeMode::Dark => DARK_THEME_ID,
        ThemeMode::HighContrast => HIGH_CONTRAST_THEME_ID,
    }
}

/// Installs a complete application registry and app selection atomically.
pub fn install_theme_registry(
    cx: &mut App,
    registry: ThemeRegistry,
    app_theme_id: impl Into<String>,
) -> Result<(), ThemeSelectionError> {
    let next = AppThemeState::new(registry, app_theme_id)?;
    if cx.try_global::<AppThemeState>() != Some(&next) {
        cx.set_global(next);
    }
    Ok(())
}

/// Registers or replaces an application theme atomically.
pub fn register_theme(
    cx: &mut App,
    definition: ThemeDefinition,
) -> Result<ThemeContext, ThemeValidationError> {
    let mut next = cx
        .try_global::<AppThemeState>()
        .cloned()
        .unwrap_or_default();
    let context = {
        let entry = next.registry.register(definition)?;
        ThemeContext::new(entry.snapshot())
    };
    next.selected_context = next
        .registry
        .snapshot(&next.selected_id)
        .map(ThemeContext::new)
        .expect("the selected application theme must remain registered");
    if cx.try_global::<AppThemeState>() != Some(&next) {
        cx.set_global(next);
    }
    Ok(context)
}

/// Returns the installed application registry, if one exists.
pub fn theme_registry(cx: &App) -> Option<&ThemeRegistry> {
    cx.try_global::<AppThemeState>()
        .map(|state| &state.registry)
}

/// Resolves a registered theme without changing any selection authority.
pub fn registered_theme_context(
    cx: &App,
    theme_id: impl AsRef<str>,
) -> Result<ThemeContext, ThemeSelectionError> {
    let theme_id = theme_id.as_ref();
    match cx.try_global::<AppThemeState>() {
        Some(state) => state.context_for(theme_id),
        None => builtin_theme_context(theme_id)
            .ok_or_else(|| ThemeSelectionError::UnknownThemeId(theme_id.to_owned())),
    }
}

/// Returns the selected app theme, or the built-in fallback when no registry is installed.
pub fn app_theme_context(cx: &App) -> ThemeContext {
    cx.try_global::<AppThemeState>()
        .map(|state| state.selected_context.clone())
        .unwrap_or_default()
}

/// Returns the selected app theme id.
pub fn app_theme_id(cx: &App) -> &str {
    cx.try_global::<AppThemeState>()
        .map(|state| state.selected_id.as_str())
        .unwrap_or(DEFAULT_THEME_ID)
}

/// Selects the application fallback theme.
pub fn set_app_theme(cx: &mut App, theme_id: impl Into<String>) -> Result<(), ThemeSelectionError> {
    let theme_id = theme_id.into();
    if let Some(state) = cx.try_global::<AppThemeState>() {
        if state.selected_id == theme_id {
            return Ok(());
        }
        let selected_context = state.context_for(&theme_id)?;
        let state = cx.global_mut::<AppThemeState>();
        state.selected_id = theme_id;
        state.selected_context = selected_context;
    } else {
        cx.set_global(AppThemeState::new(
            ThemeRegistry::with_builtins(),
            theme_id,
        )?);
    }
    Ok(())
}

/// Selects the built-in application theme for a mode.
pub fn set_app_theme_mode(cx: &mut App, mode: ThemeMode) -> Result<(), ThemeSelectionError> {
    set_app_theme(cx, theme_id_for_mode(mode))
}

/// Selects a registered theme for one window.
pub fn set_window_theme(
    window: &mut Window,
    cx: &mut App,
    theme_id: impl Into<String>,
) -> Result<(), ThemeSelectionError> {
    let theme_id = theme_id.into();
    let context = registered_theme_context(cx, &theme_id)?;
    let state = window_theme_state(window, cx);
    let changed = replace_window_theme_base(
        &state,
        WindowThemeAuthority::Selected(theme_id),
        context,
        cx,
    );
    if changed {
        window.refresh();
    }
    Ok(())
}

/// Applies an explicit immutable theme override to one window.
pub fn override_window_theme(window: &mut Window, cx: &mut App, context: ThemeContext) {
    let state = window_theme_state(window, cx);
    let changed = replace_window_theme_base(&state, WindowThemeAuthority::Override, context, cx);
    if changed {
        window.refresh();
    }
}

/// Clears a window selection or override so the window inherits the application theme.
pub fn clear_window_theme(window: &mut Window, cx: &mut App) {
    let context = app_theme_context(cx);
    let state = window_theme_state(window, cx);
    let changed = replace_window_theme_base(&state, WindowThemeAuthority::InheritApp, context, cx);
    if changed {
        window.refresh();
    }
}

pub(crate) fn current_theme_context(window: &mut Window, cx: &mut App) -> ThemeContext {
    let state = window_theme_state(window, cx);
    let state = state.read(cx);
    if let Some(context) = state.scope_stack.borrow().last().cloned() {
        return context;
    }
    match state.authority {
        WindowThemeAuthority::InheritApp => app_theme_context(cx),
        WindowThemeAuthority::Selected(_) | WindowThemeAuthority::Override => {
            state.base_context.clone()
        }
    }
}

pub(crate) fn theme_scope_stack(
    window: &mut Window,
    cx: &mut App,
) -> Rc<RefCell<Vec<ThemeContext>>> {
    window_theme_state(window, cx).read(cx).scope_stack.clone()
}
