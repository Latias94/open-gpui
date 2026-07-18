use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

use open_gpui::{App, Entity, Global, Rgba, Subscription, Window};
use open_gpui_motion::MotionPreference;
use open_gpui_ui_core::{Density, ThemeDesignScales};

use crate::color::ColorIntent;

use super::registry::{ThemeDefinition, ThemeRegistry, ThemeValidationError};
use super::snapshot::{ThemeMode, ThemeSnapshot};

/// Stable id for the built-in light theme.
pub const LIGHT_THEME_ID: &str = "light";

/// Stable id for the built-in dark theme.
pub const DARK_THEME_ID: &str = "dark";

/// Stable id for the built-in high-contrast theme.
pub const HIGH_CONTRAST_THEME_ID: &str = "high-contrast";

/// The built-in fallback used when an application has no installed theme registry.
pub const DEFAULT_THEME_ID: &str = LIGHT_THEME_ID;

static NEXT_EFFECTIVE_REVISION: AtomicU64 = AtomicU64::new(1);

fn allocate_effective_revision() -> u64 {
    let revision = NEXT_EFFECTIVE_REVISION.fetch_add(1, Ordering::Relaxed);
    assert_ne!(
        revision,
        u64::MAX,
        "theme effective revision space exhausted"
    );
    revision
}

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

/// Owned render-time view over one complete immutable Theme v1 snapshot.
///
/// Source revision remains metadata on the snapshot. Effective revision is allocated only by this
/// runtime authority and is preserved by clones and detached opening-generation capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeContext {
    effective_revision: u64,
    snapshot: ThemeSnapshot,
}

static LIGHT_THEME_CONTEXT: LazyLock<ThemeContext> =
    LazyLock::new(|| ThemeContext::new(ThemeSnapshot::light()));
static DARK_THEME_CONTEXT: LazyLock<ThemeContext> =
    LazyLock::new(|| ThemeContext::new(ThemeSnapshot::dark()));
static HIGH_CONTRAST_THEME_CONTEXT: LazyLock<ThemeContext> =
    LazyLock::new(|| ThemeContext::new(ThemeSnapshot::high_contrast()));

impl ThemeContext {
    /// Creates a runtime context from a complete immutable snapshot.
    ///
    /// The caller supplies source metadata and effective content, never the runtime revision.
    pub fn new(snapshot: ThemeSnapshot) -> Self {
        Self {
            effective_revision: allocate_effective_revision(),
            snapshot,
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

    /// Returns the complete immutable source snapshot.
    pub const fn snapshot(&self) -> &ThemeSnapshot {
        &self.snapshot
    }

    /// Returns the active theme mode.
    pub const fn mode(&self) -> ThemeMode {
        self.snapshot.mode()
    }

    /// Returns source-file revision metadata.
    pub const fn source_revision(&self) -> u64 {
        self.snapshot.source_revision()
    }

    /// Returns the runtime-owned effective revision.
    pub const fn effective_revision(&self) -> u64 {
        self.effective_revision
    }

    /// Returns the complete non-color design scales.
    pub const fn design_scales(&self) -> ThemeDesignScales {
        self.snapshot.design_scales()
    }

    /// Returns the theme density default.
    pub const fn density(&self) -> Density {
        self.snapshot.density()
    }

    /// Returns the theme motion policy.
    pub const fn motion_preference(&self) -> MotionPreference {
        self.snapshot.motion_preference()
    }

    /// Resolves a component color intent with this context.
    pub fn resolve(&self, intent: ColorIntent) -> Rgba {
        self.snapshot.resolve(intent)
    }

    pub(super) fn has_same_effective_content(&self, other: &Self) -> bool {
        self.snapshot.has_same_effective_content(&other.snapshot)
    }

    pub(super) fn has_same_effective_identity(&self, other: &Self) -> bool {
        self.effective_revision == other.effective_revision
            && self.has_same_effective_content(other)
    }

    pub(super) fn rebound(&self) -> Self {
        Self::new(self.snapshot.clone())
    }

    pub(super) fn with_snapshot_preserving_effective_revision(
        &self,
        snapshot: ThemeSnapshot,
    ) -> Self {
        debug_assert!(self.snapshot.has_same_effective_content(&snapshot));
        Self {
            effective_revision: self.effective_revision,
            snapshot,
        }
    }
}

impl From<ThemeSnapshot> for ThemeContext {
    fn from(snapshot: ThemeSnapshot) -> Self {
        Self::new(snapshot)
    }
}

impl Default for ThemeContext {
    fn default() -> Self {
        Self::light()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            .context(&selected_id)
            .map(ThemeContext::rebound)
            .ok_or_else(|| ThemeSelectionError::UnknownThemeId(selected_id.clone()))?;
        Ok(Self {
            registry,
            selected_id,
            selected_context,
        })
    }

    fn context_for(&self, theme_id: &str) -> Result<ThemeContext, ThemeSelectionError> {
        self.registry
            .context(theme_id)
            .cloned()
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
    inherited_app_revision: u64,
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
                let (_, changed) = synchronize_window_theme_base(&state, cx);
                if changed {
                    window.refresh();
                }
            });
        let base_context = app_theme_context(cx);
        Self {
            authority: WindowThemeAuthority::InheritApp,
            inherited_app_revision: base_context.effective_revision(),
            base_context,
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
    inherited_app_revision: Option<u64>,
    cx: &mut App,
) -> bool {
    let (should_update, should_refresh) = {
        let state = state.read(cx);
        (
            state.authority != authority || state.base_context != context,
            state.authority != authority
                || !state.base_context.has_same_effective_identity(&context),
        )
    };
    if should_update {
        state.update(cx, |state, _| {
            state.authority = authority;
            state.base_context = context;
            if let Some(inherited_app_revision) = inherited_app_revision {
                state.inherited_app_revision = inherited_app_revision;
            }
        });
    }
    should_refresh
}

fn resolve_base_context(
    authority: &WindowThemeAuthority,
    previous: &ThemeContext,
    inherited_app_revision: u64,
    cx: &App,
) -> (ThemeContext, Option<u64>) {
    match authority {
        WindowThemeAuthority::InheritApp => {
            let (context, app_revision) =
                resolve_inherited_context(previous, inherited_app_revision, cx);
            (context, Some(app_revision))
        }
        WindowThemeAuthority::Selected(theme_id) => registered_theme_context(cx, theme_id)
            .map(|registered| {
                if previous.has_same_effective_content(&registered) {
                    previous
                        .with_snapshot_preserving_effective_revision(registered.snapshot().clone())
                } else {
                    registered.rebound()
                }
            })
            .map(|context| (context, None))
            .unwrap_or_else(|_| (previous.clone(), None)),
        WindowThemeAuthority::Override => {
            unreachable!("an explicit override keeps its owned context")
        }
    }
}

fn resolve_inherited_context(
    previous: &ThemeContext,
    inherited_app_revision: u64,
    cx: &App,
) -> (ThemeContext, u64) {
    let app_context = app_theme_context(cx);
    let app_revision = app_context.effective_revision();
    let context = if app_revision == inherited_app_revision {
        if previous.has_same_effective_content(&app_context) {
            previous.with_snapshot_preserving_effective_revision(app_context.snapshot().clone())
        } else {
            app_context.rebound()
        }
    } else {
        app_context
    };
    (context, app_revision)
}

fn synchronize_window_theme_base(
    state: &Entity<WindowThemeState>,
    cx: &mut App,
) -> (ThemeContext, bool) {
    let (authority, previous, inherited_app_revision) = {
        let state = state.read(cx);
        (
            state.authority.clone(),
            state.base_context.clone(),
            state.inherited_app_revision,
        )
    };
    if authority == WindowThemeAuthority::Override {
        return (previous, false);
    }
    let (context, next_inherited_app_revision) =
        resolve_base_context(&authority, &previous, inherited_app_revision, cx);
    let changed = replace_window_theme_base(
        state,
        authority,
        context.clone(),
        next_inherited_app_revision,
        cx,
    );
    (context, changed)
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
    let app_theme_id = app_theme_id.into();
    let registered = registry
        .context(&app_theme_id)
        .cloned()
        .ok_or_else(|| ThemeSelectionError::UnknownThemeId(app_theme_id.clone()))?;
    let selected_context = match cx.try_global::<AppThemeState>() {
        Some(previous)
            if previous.selected_id == app_theme_id
                && previous
                    .selected_context
                    .has_same_effective_content(&registered) =>
        {
            previous
                .selected_context
                .with_snapshot_preserving_effective_revision(registered.snapshot().clone())
        }
        _ => registered.rebound(),
    };
    let next = AppThemeState {
        registry,
        selected_id: app_theme_id,
        selected_context,
    };
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
    let (registered_id, entry_context) = {
        let entry = next.registry.register(definition)?;
        (entry.id().to_owned(), entry.context().clone())
    };
    if next.selected_id == registered_id {
        next.selected_context = if next
            .selected_context
            .has_same_effective_content(&entry_context)
        {
            next.selected_context
                .with_snapshot_preserving_effective_revision(entry_context.snapshot().clone())
        } else {
            entry_context.clone()
        };
    }
    if cx.try_global::<AppThemeState>() != Some(&next) {
        cx.set_global(next);
    }
    Ok(entry_context)
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
        let selected_context = state.context_for(&theme_id)?.rebound();
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
    let registered = registered_theme_context(cx, &theme_id)?;
    let state = window_theme_state(window, cx);
    let context = {
        let state = state.read(cx);
        match &state.authority {
            WindowThemeAuthority::Selected(selected_id)
                if selected_id == &theme_id
                    && state.base_context.has_same_effective_content(&registered) =>
            {
                state
                    .base_context
                    .with_snapshot_preserving_effective_revision(registered.snapshot().clone())
            }
            _ => registered.rebound(),
        }
    };
    let changed = replace_window_theme_base(
        &state,
        WindowThemeAuthority::Selected(theme_id),
        context,
        None,
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
    let context = {
        let state = state.read(cx);
        if state.authority == WindowThemeAuthority::Override
            && state.base_context.has_same_effective_content(&context)
        {
            state
                .base_context
                .with_snapshot_preserving_effective_revision(context.snapshot().clone())
        } else {
            context.rebound()
        }
    };
    let changed =
        replace_window_theme_base(&state, WindowThemeAuthority::Override, context, None, cx);
    if changed {
        window.refresh();
    }
}

/// Clears a window selection or override so the window inherits the application theme.
pub fn clear_window_theme(window: &mut Window, cx: &mut App) {
    let state = window_theme_state(window, cx);
    let (authority, previous, previous_app_revision) = {
        let state = state.read(cx);
        (
            state.authority.clone(),
            state.base_context.clone(),
            state.inherited_app_revision,
        )
    };
    let (context, inherited_app_revision) = if authority == WindowThemeAuthority::InheritApp {
        resolve_inherited_context(&previous, previous_app_revision, cx)
    } else {
        let app_context = app_theme_context(cx);
        let app_revision = app_context.effective_revision();
        (app_context.rebound(), app_revision)
    };
    let changed = replace_window_theme_base(
        &state,
        WindowThemeAuthority::InheritApp,
        context,
        Some(inherited_app_revision),
        cx,
    );
    if changed {
        window.refresh();
    }
}

pub(crate) fn current_theme_context(window: &mut Window, cx: &mut App) -> ThemeContext {
    let state = window_theme_state(window, cx);
    if let Some(context) = state.read(cx).scope_stack.borrow().last().cloned() {
        return context;
    }
    if state.read(cx).authority == WindowThemeAuthority::Override {
        return state.read(cx).base_context.clone();
    }
    let (context, changed) = synchronize_window_theme_base(&state, cx);
    if changed {
        window.refresh();
    }
    context
}

pub(crate) fn theme_scope_stack(
    window: &mut Window,
    cx: &mut App,
) -> Rc<RefCell<Vec<ThemeContext>>> {
    window_theme_state(window, cx).read(cx).scope_stack.clone()
}
