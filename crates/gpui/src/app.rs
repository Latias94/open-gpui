use open_gpui_scheduler::Instant;
use std::{
    any::{TypeId, type_name},
    cell::{Cell, RefCell},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, atomic::Ordering::SeqCst},
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use futures::{
    Future, FutureExt,
    channel::oneshot,
    future::{LocalBoxFuture, Shared},
};
use itertools::Itertools;
use parking_lot::RwLock;
use slotmap::SlotMap;

pub use async_context::*;
#[cfg(any(test, feature = "test-support"))]
pub use bench_context::{BenchAppContext, BenchWindowContext};
pub use context::*;
pub use entity_map::*;
#[cfg(any(test, feature = "test-support"))]
pub use headless_app_context::*;
use open_gpui_collections::{
    FxHashMap, FxHashSet, HashMap, TypeIdHashMap, TypeIdHashSet, VecDeque,
};
use open_gpui_core_util::debug_panic;
use open_gpui_http_client::{HttpClient, Url};
use smallvec::SmallVec;
#[cfg(any(test, feature = "test-support"))]
pub use test_app::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_context::*;
#[cfg(any(test, feature = "test-support"))]
pub use visual_test_context::*;

#[cfg(any(feature = "inspector", debug_assertions))]
use crate::InspectorElementRegistry;
use crate::{
    Action, ActionBuildError, ActionRegistry, Any, AnyView, AnyWindowHandle, AppContext, Arena,
    ArenaBox, Asset, AssetSource, BackgroundExecutor, Bounds, ClipboardItem, CursorStyle,
    DispatchPhase, DisplayId, EventEmitter, FocusHandle, FocusMap, ForegroundExecutor, Global,
    KeyBinding, KeyContext, Keymap, Keystroke, LayoutId, Menu, MenuItem, MouseButton, OwnedMenu,
    PathPromptOptions, Pixels, Platform, PlatformDisplay, PlatformDisplaySnapshot,
    PlatformFocusedWindow, PlatformHoveredWindow, PlatformKeyboardLayout, PlatformKeyboardMapper,
    PlatformViewportCapabilities, PlatformWindowCapabilities, PlatformWindowProfile, Point,
    PointerCaptureHandle, Priority, PromptBuilder, PromptButton, PromptHandle, PromptLevel, Render,
    RenderImage, RenderablePromptHandle, Reservation, ScreenCaptureSource, SharedString,
    SubscriberSet, Subscription, SvgRenderer, Task, TextRenderingMode, TextSystem, ThermalState,
    Window, WindowAppearance, WindowButtonLayout, WindowHandle, WindowId, WindowInvalidator,
    WindowKind, WindowTransientOwner,
    colors::{Colors, GlobalColors},
    hash, init_app_menus,
};

mod action_dispatch;
mod async_context;
#[cfg(any(test, feature = "test-support"))]
mod bench_context;
mod cell;
mod context;
mod entity_map;
#[cfg(any(test, feature = "test-support"))]
mod headless_app_context;
mod native_callback_diagnostics;
mod native_captured_drag;
mod native_event_ingress;
mod native_platform_commands;
mod native_query_snapshot;
#[cfg(any(test, feature = "test-support"))]
mod test_app;
#[cfg(any(test, feature = "test-support"))]
mod test_context;
#[cfg(any(test, feature = "test-support"))]
mod visual_test_context;
mod window_registry;
pub(crate) use cell::NativeCallbackLease;
pub use cell::{AppCell, AppRef, AppRefMut};
#[doc(hidden)]
pub use native_callback_diagnostics::{
    NativeBoundaryDiagnostic, NativeBoundaryDiagnosticCursor, NativeBoundaryDiagnosticsSnapshot,
    NativeBoundaryDisposition, NativeBoundaryGeneration, NativeBoundaryKind, NativeBoundaryTarget,
    NativeCallbackKind, NativeInputBoundary, NativeInputDeliveryResult,
    NativeInputHandlerOperation, NativeInvariantFailure, NativePlatformCommandKind,
};
pub(crate) use native_captured_drag::NativeCapturedDragStartToken;
use native_captured_drag::{ActiveNativeCapturedDragAuthority, WindowUpdateProvenance};
pub use native_captured_drag::{
    NativeCapturedDragEvent, NativeCapturedDragGeneration, NativeCapturedDragPhase,
    NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal, NativeIngressSequence,
    PreparedNativeCapturedDragConsumer,
};
pub(crate) use native_platform_commands::{
    NativePointerCaptureReleaseToken, PlatformWindowCommandSink,
};

/// The duration for which futures returned from [Context::on_app_quit] can run before the application fully quits.
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(200);

fn retain_first_app_shutdown_panic(
    first: &mut Option<Box<dyn std::any::Any + Send>>,
    candidate: Option<Box<dyn std::any::Any + Send>>,
) {
    if first.is_none() {
        *first = candidate;
    }
}

/// Stage at which a synchronous GPUI window-open transaction failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowOpenFailureStage {
    /// Application shutdown prevented this window-open transaction from starting or committing.
    AppShutdown,
    /// The platform window could not be created or mapped.
    NativeCreateOrMap,
    /// The native window synchronously closed while it was being created or mapped.
    ClosedDuringNativeCreateOrMap,
    /// The root builder synchronously closed the reserved window.
    ClosedDuringBuild,
    /// The initial draw synchronously closed the reserved window.
    ClosedDuringInitialDraw,
    /// The hidden initial-presentation attempt synchronously closed the reserved window.
    ClosedDuringInitialPresentation,
    /// A backend that presents before visibility rejected its hidden first frame.
    BeforeVisibilityPresentation,
    /// The fully built window could not commit to the application registry.
    CommitRejected,
}

/// Failure to register an exact native-window retirement dependency.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowRetirementDependencyError {
    /// The anchor is neither a current native window nor an owned retirement.
    UnknownAnchor {
        /// The retirement anchor that has no native lifetime authority.
        anchor: WindowId,
    },
    /// A dependency is neither current, retiring, nor already terminal.
    UnknownDependency {
        /// The retirement anchor receiving the dependency.
        anchor: WindowId,
        /// The dependency that has no native lifetime authority.
        dependency: WindowId,
    },
    /// The anchor already entered native retirement, so its dependency set is immutable.
    AnchorAlreadyRetiring {
        /// The retirement anchor whose native owner is already queued or retained.
        anchor: WindowId,
    },
    /// Adding the dependency would create a cycle and permanently retain native owners.
    Cycle {
        /// The retirement anchor receiving the dependency.
        anchor: WindowId,
        /// The dependency that can already reach the anchor.
        dependency: WindowId,
    },
}

/// Typed failure from one synchronous GPUI window-open transaction.
#[derive(Debug, thiserror::Error)]
#[error("window open failed during {stage:?}: {source}")]
pub struct WindowOpenError {
    stage: WindowOpenFailureStage,
    #[source]
    source: anyhow::Error,
}

impl WindowOpenError {
    fn new(stage: WindowOpenFailureStage, source: anyhow::Error) -> Self {
        Self { stage, source }
    }

    fn from_reservation(error: window_registry::WindowReservationError) -> Self {
        let stage = match error {
            window_registry::WindowReservationError::AppShutdown => {
                WindowOpenFailureStage::AppShutdown
            }
            window_registry::WindowReservationError::WindowClosed
            | window_registry::WindowReservationError::NotCurrent
            | window_registry::WindowReservationError::ProvisionalSession(_) => {
                WindowOpenFailureStage::CommitRejected
            }
        };
        Self::new(stage, error.into())
    }

    /// Returns the exact synchronous stage that failed.
    pub const fn stage(&self) -> WindowOpenFailureStage {
        self.stage
    }
}

#[cfg(target_family = "wasm")]
thread_local! {
    static RUNNING_WEB_APPLICATIONS: RefCell<Vec<Rc<AppCell>>> = RefCell::new(Vec::new());
}

/// A reference to a GPUI application, typically constructed in the `main` function of your app.
/// You won't interact with this type much outside of initial configuration and startup.
pub struct Application(Rc<AppCell>);

/// Represents an application before it is fully launched. Once your app is
/// configured, you'll start the app with `App::run`.
impl Application {
    /// Builds an app with a caller-provided platform implementation.
    pub fn with_platform(platform: Rc<dyn Platform>) -> Self {
        Self(App::new_app(
            platform,
            Arc::new(()),
            Arc::new(NullHttpClient),
        ))
    }

    /// Runs one complete application update without entering the platform event loop.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn update_for_test<R>(&mut self, update: impl FnOnce(&mut App) -> R) -> R {
        self.0.borrow_mut().update(update)
    }

    /// Builds an app with accessibility (AccessKit) integration forcibly
    /// disabled.
    ///
    /// In this mode, accessibility APIs (e.g.
    /// [`div().role()`][crate::StatefulInteractiveElement::role]) silently
    /// no-op.
    ///
    /// See the [accessibility guide](crate::_accessibility) for an overview of
    /// the features this disables.
    pub fn new_inaccessible(platform: Rc<dyn Platform>) -> Self {
        let this = Self::with_platform(platform);
        this.0.borrow_mut().accessibility_force_disabled = true;
        this
    }

    /// Assigns the source of assets for the application.
    pub fn with_assets(self, asset_source: impl AssetSource) -> Self {
        let mut context_lock = self.0.borrow_mut();
        let asset_source = Arc::new(asset_source);
        context_lock.asset_source = asset_source.clone();
        context_lock.svg_renderer = SvgRenderer::new(asset_source);
        drop(context_lock);
        self
    }

    /// Sets the HTTP client for the application.
    pub fn with_http_client(self, http_client: Arc<dyn HttpClient>) -> Self {
        let mut context_lock = self.0.borrow_mut();
        context_lock.http_client = http_client;
        drop(context_lock);
        self
    }

    /// Configures when the application should automatically quit.
    /// By default, [`QuitMode::Default`] is used.
    pub fn with_quit_mode(self, mode: QuitMode) -> Self {
        self.0.borrow_mut().quit_mode = mode;
        self
    }

    /// Start the application. The provided callback will be called once the
    /// app is fully launched.
    pub fn run<F>(self, on_finish_launching: F)
    where
        F: 'static + FnOnce(&mut App),
    {
        self.run_platform(on_finish_launching);
        #[cfg(target_family = "wasm")]
        RUNNING_WEB_APPLICATIONS.with(|applications| {
            applications.borrow_mut().push(self.0);
        });
    }

    /// Runs a returning test platform while retaining the application for post-run inspection.
    ///
    /// Native process-convergence tests use this to prove that normal last-window policy returns
    /// from the owning platform loop before the worker publishes its live pre-exit census.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn run_returning_for_test<F>(&mut self, on_finish_launching: F)
    where
        F: 'static + FnOnce(&mut App),
    {
        self.run_platform(on_finish_launching);
    }

    fn run_platform<F>(&self, on_finish_launching: F)
    where
        F: 'static + FnOnce(&mut App),
    {
        let this = self.0.clone();
        let platform = self.0.borrow().platform.clone();
        platform.run(Box::new(move || {
            let cx = &mut *this.borrow_mut();
            on_finish_launching(cx);
        }));
    }

    /// Register a handler to be invoked when the platform instructs the application
    /// to open one or more URLs.
    ///
    /// Replaces the previously registered handler, if any.
    pub fn on_open_urls<F>(&self, callback: F) -> &Self
    where
        F: 'static + FnMut(Vec<String>, &mut App),
    {
        self.0.set_open_urls_handler(Box::new(callback));
        self
    }

    /// Invokes a handler when an already-running application is launched.
    /// On macOS, this can occur when the application icon is double-clicked or the app is launched via the dock.
    ///
    /// Replaces the previously registered handler, if any.
    pub fn on_reopen<F>(&self, callback: F) -> &Self
    where
        F: 'static + FnMut(&mut App),
    {
        self.0.set_reopen_handler(Box::new(callback));
        self
    }

    /// Invokes a handler when the system wakes from sleep.
    ///
    /// Replaces the previously registered handler, if any.
    pub fn on_system_wake<F>(&self, callback: F) -> &Self
    where
        F: 'static + FnMut(&mut App),
    {
        self.0.set_system_wake_handler(Box::new(callback));
        self
    }

    /// Returns a handle to the [`BackgroundExecutor`] associated with this app, which can be used to spawn futures in the background.
    pub fn background_executor(&self) -> BackgroundExecutor {
        self.0.borrow().background_executor.clone()
    }

    /// Returns a handle to the [`ForegroundExecutor`] associated with this app, which can be used to spawn futures in the foreground.
    pub fn foreground_executor(&self) -> ForegroundExecutor {
        self.0.borrow().foreground_executor.clone()
    }

    /// Returns a reference to the [`TextSystem`] associated with this app.
    pub fn text_system(&self) -> Arc<TextSystem> {
        self.0.borrow().text_system.clone()
    }

    /// Returns the file URL of the executable with the specified name in the application bundle
    pub fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.0.borrow().path_for_auxiliary_executable(name)
    }
}

type Handler = Box<dyn FnMut(&mut App) -> bool + 'static>;
type Listener = Box<dyn FnMut(&dyn Any, &mut App) -> bool + 'static>;
pub(crate) type KeystrokeObserver =
    Box<dyn FnMut(&KeystrokeEvent, &mut Window, &mut App) -> bool + 'static>;
type QuitHandler = Box<dyn FnOnce(&mut App) -> LocalBoxFuture<'static, ()> + 'static>;
type WindowClosedHandler = Box<dyn FnMut(&mut App, WindowId)>;
type WindowNativeTerminalHandler = Box<dyn FnMut(&mut App, WindowId)>;
type ReleaseListener = Box<dyn FnOnce(&mut dyn Any, &mut App) + 'static>;
type NewEntityListener = Box<dyn FnMut(AnyEntity, &mut Option<&mut Window>, &mut App) + 'static>;

/// Defines when the application should automatically quit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuitMode {
    /// Use [`QuitMode::Explicit`] on macOS and [`QuitMode::LastWindowClosed`] on other platforms.
    #[default]
    Default,
    /// Quit automatically when the last window is closed.
    LastWindowClosed,
    /// Quit only when requested via [`App::quit`].
    Explicit,
}

/// Controls when GPUI hides the mouse cursor in response to keyboard input.
///
/// Restoration on mouse motion is handled by the platform layer; this enum
/// only describes the policy for *triggering* a hide.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CursorHideMode {
    /// Never hide the cursor automatically.
    Never,
    /// Hide on character-producing key presses (typing).
    OnTyping,
    /// Hide on character-producing key presses, *and* when a key binding
    /// resolves to an action that consumes the keystroke.
    #[default]
    OnTypingAndAction,
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq)]
pub struct SystemWindowTab {
    pub id: WindowId,
    pub title: SharedString,
    pub handle: AnyWindowHandle,
    pub last_active_at: Instant,
}

impl SystemWindowTab {
    /// Create a new instance of the window tab.
    pub fn new(title: SharedString, handle: AnyWindowHandle) -> Self {
        Self {
            id: handle.id,
            title,
            handle,
            last_active_at: Instant::now(),
        }
    }
}

/// A controller for managing window tabs.
#[derive(Default)]
pub struct SystemWindowTabController {
    visible: Option<bool>,
    tab_groups: FxHashMap<usize, Vec<SystemWindowTab>>,
}

impl Global for SystemWindowTabController {}

impl SystemWindowTabController {
    /// Create a new instance of the window tab controller.
    pub fn new() -> Self {
        Self {
            visible: None,
            tab_groups: FxHashMap::default(),
        }
    }

    /// Initialize the global window tab controller.
    pub fn init(cx: &mut App) {
        cx.set_global(SystemWindowTabController::new());
    }

    /// Get all tab groups.
    pub fn tab_groups(&self) -> &FxHashMap<usize, Vec<SystemWindowTab>> {
        &self.tab_groups
    }

    /// Get the next tab group window handle.
    pub fn get_next_tab_group_window(cx: &mut App, id: WindowId) -> Option<&AnyWindowHandle> {
        let controller = cx.global::<SystemWindowTabController>();
        let current_group = controller
            .tab_groups
            .iter()
            .find_map(|(group, tabs)| tabs.iter().find(|tab| tab.id == id).map(|_| group));

        let current_group = current_group?;
        // TODO: `.keys()` returns arbitrary order, what does "next" mean?
        let mut group_ids: Vec<_> = controller.tab_groups.keys().collect();
        let idx = group_ids.iter().position(|g| *g == current_group)?;
        let next_idx = (idx + 1) % group_ids.len();

        controller
            .tab_groups
            .get(group_ids[next_idx])
            .and_then(|tabs| {
                tabs.iter()
                    .max_by_key(|tab| tab.last_active_at)
                    .or_else(|| tabs.first())
                    .map(|tab| &tab.handle)
            })
    }

    /// Get the previous tab group window handle.
    pub fn get_prev_tab_group_window(cx: &mut App, id: WindowId) -> Option<&AnyWindowHandle> {
        let controller = cx.global::<SystemWindowTabController>();
        let current_group = controller
            .tab_groups
            .iter()
            .find_map(|(group, tabs)| tabs.iter().find(|tab| tab.id == id).map(|_| group));

        let current_group = current_group?;
        // TODO: `.keys()` returns arbitrary order, what does "previous" mean?
        let mut group_ids: Vec<_> = controller.tab_groups.keys().collect();
        let idx = group_ids.iter().position(|g| *g == current_group)?;
        let prev_idx = if idx == 0 {
            group_ids.len() - 1
        } else {
            idx - 1
        };

        controller
            .tab_groups
            .get(group_ids[prev_idx])
            .and_then(|tabs| {
                tabs.iter()
                    .max_by_key(|tab| tab.last_active_at)
                    .or_else(|| tabs.first())
                    .map(|tab| &tab.handle)
            })
    }

    /// Get all tabs in the same window.
    pub fn tabs(&self, id: WindowId) -> Option<&Vec<SystemWindowTab>> {
        self.tab_groups
            .values()
            .find(|tabs| tabs.iter().any(|tab| tab.id == id))
    }

    /// Initialize the visibility of the system window tab controller.
    pub fn init_visible(cx: &mut App, visible: bool) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        if controller.visible.is_none() {
            controller.visible = Some(visible);
        }
    }

    /// Get the visibility of the system window tab controller.
    pub fn is_visible(&self) -> bool {
        self.visible.unwrap_or(false)
    }

    /// Set the visibility of the system window tab controller.
    pub fn set_visible(cx: &mut App, visible: bool) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        controller.visible = Some(visible);
    }

    /// Update the last active of a window.
    pub fn update_last_active(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for windows in controller.tab_groups.values_mut() {
            for tab in windows.iter_mut() {
                if tab.id == id {
                    tab.last_active_at = Instant::now();
                }
            }
        }
    }

    /// Update the position of a tab within its group.
    pub fn update_tab_position(cx: &mut App, id: WindowId, ix: usize) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for (_, windows) in controller.tab_groups.iter_mut() {
            if let Some(current_pos) = windows.iter().position(|tab| tab.id == id) {
                if ix < windows.len() && current_pos != ix {
                    let window_tab = windows.remove(current_pos);
                    windows.insert(ix, window_tab);
                }
                break;
            }
        }
    }

    /// Update the title of a tab.
    pub fn update_tab_title(cx: &mut App, id: WindowId, title: SharedString) {
        let controller = cx.global::<SystemWindowTabController>();
        let tab = controller
            .tab_groups
            .values()
            .flat_map(|windows| windows.iter())
            .find(|tab| tab.id == id);

        if tab.map_or(true, |t| t.title == title) {
            return;
        }

        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for windows in controller.tab_groups.values_mut() {
            for tab in windows.iter_mut() {
                if tab.id == id {
                    tab.title = title;
                    return;
                }
            }
        }
    }

    /// Insert a tab into a tab group.
    pub fn add_tab(cx: &mut App, id: WindowId, tabs: Vec<SystemWindowTab>) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tab) = tabs.iter().find(|tab| tab.id == id).cloned() else {
            return;
        };

        let mut expected_tab_ids: Vec<_> = tabs
            .iter()
            .filter(|tab| tab.id != id)
            .map(|tab| tab.id)
            .sorted()
            .collect();

        let mut tab_group_id = None;
        for (group_id, group_tabs) in &controller.tab_groups {
            let tab_ids: Vec<_> = group_tabs.iter().map(|tab| tab.id).sorted().collect();
            if tab_ids == expected_tab_ids {
                tab_group_id = Some(*group_id);
                break;
            }
        }

        if let Some(tab_group_id) = tab_group_id {
            if let Some(tabs) = controller.tab_groups.get_mut(&tab_group_id) {
                tabs.push(tab);
            }
        } else {
            let new_group_id = controller.tab_groups.len();
            controller.tab_groups.insert(new_group_id, tabs);
        }
    }

    /// Remove a tab from a tab group.
    pub fn remove_tab(cx: &mut App, id: WindowId) -> Option<SystemWindowTab> {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let mut removed_tab = None;

        controller.tab_groups.retain(|_, tabs| {
            if let Some(pos) = tabs.iter().position(|tab| tab.id == id) {
                removed_tab = Some(tabs.remove(pos));
            }
            !tabs.is_empty()
        });

        removed_tab
    }

    /// Move a tab to a new tab group.
    pub fn move_tab_to_new_window(cx: &mut App, id: WindowId) {
        let mut removed_tab = Self::remove_tab(cx, id);
        let mut controller = cx.global_mut::<SystemWindowTabController>();

        if let Some(tab) = removed_tab {
            let new_group_id = controller.tab_groups.keys().max().map_or(0, |k| k + 1);
            controller.tab_groups.insert(new_group_id, vec![tab]);
        }
    }

    /// Merge all tab groups into a single group.
    pub fn merge_all_windows(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(initial_tabs) = controller.tabs(id) else {
            return;
        };

        let initial_tabs_len = initial_tabs.len();
        let mut all_tabs = initial_tabs.clone();

        for (_, mut tabs) in controller.tab_groups.drain() {
            tabs.retain(|tab| !all_tabs[..initial_tabs_len].contains(tab));
            all_tabs.extend(tabs);
        }

        controller.tab_groups.insert(0, all_tabs);
    }

    /// Selects the next tab in the tab group in the trailing direction.
    pub fn select_next_tab(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tabs) = controller.tabs(id) else {
            return;
        };

        let current_index = tabs.iter().position(|tab| tab.id == id).unwrap();
        let next_index = (current_index + 1) % tabs.len();

        let _ = &tabs[next_index].handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }

    /// Selects the previous tab in the tab group in the leading direction.
    pub fn select_previous_tab(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tabs) = controller.tabs(id) else {
            return;
        };

        let current_index = tabs.iter().position(|tab| tab.id == id).unwrap();
        let previous_index = if current_index == 0 {
            tabs.len() - 1
        } else {
            current_index - 1
        };

        let _ = &tabs[previous_index].handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }
}

pub(crate) enum GpuiMode {
    #[cfg(any(test, feature = "test-support"))]
    Test {
        skip_drawing: bool,
    },
    Production,
}

impl GpuiMode {
    #[cfg(any(test, feature = "test-support"))]
    pub fn test() -> Self {
        GpuiMode::Test {
            skip_drawing: false,
        }
    }

    #[inline]
    pub(crate) fn skip_drawing(&self) -> bool {
        match self {
            #[cfg(any(test, feature = "test-support"))]
            GpuiMode::Test { skip_drawing } => *skip_drawing,
            GpuiMode::Production => false,
        }
    }
}

/// Contains the state of the full application, and passed as a reference to a variety of callbacks.
/// Other [Context] derefs to this type.
/// You need a reference to an `App` to access the state of a [Entity].
pub struct App {
    pub(crate) this: Weak<AppCell>,
    pub(crate) platform: Rc<dyn Platform>,
    text_system: Arc<TextSystem>,

    pub(crate) actions: Rc<ActionRegistry>,
    pub(crate) active_drag: Option<AnyDrag>,
    active_native_captured_drag: Option<ActiveNativeCapturedDragAuthority>,
    next_native_captured_drag_generation: u64,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
    pub(crate) entities: EntityMap,
    pub(crate) new_entity_observers: SubscriberSet<TypeId, NewEntityListener>,
    pub(crate) windows: SlotMap<WindowId, Option<Box<Window>>>,
    pub(crate) window_handles: FxHashMap<WindowId, AnyWindowHandle>,
    pub(crate) window_profiles: FxHashMap<WindowId, PlatformWindowProfile>,
    pub(crate) focus_handles: Arc<FocusMap>,
    pub(crate) keymap: Rc<RefCell<Keymap>>,
    pub(crate) keyboard_layout: Box<dyn PlatformKeyboardLayout>,
    pub(crate) keyboard_mapper: Rc<dyn PlatformKeyboardMapper>,
    pub(crate) global_action_listeners:
        TypeIdHashMap<Vec<Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Self)>>>,
    pending_effects: VecDeque<Effect>,

    pub(crate) observers: SubscriberSet<EntityId, Handler>,
    pub(crate) event_listeners: SubscriberSet<EntityId, (TypeId, Listener)>,
    pub(crate) keystroke_observers: SubscriberSet<(), KeystrokeObserver>,
    pub(crate) keystroke_interceptors: SubscriberSet<(), KeystrokeObserver>,
    pub(crate) keyboard_layout_observers: SubscriberSet<(), Handler>,
    pub(crate) thermal_state_observers: SubscriberSet<(), Handler>,
    pub(crate) release_listeners: SubscriberSet<EntityId, ReleaseListener>,
    pub(crate) global_observers: SubscriberSet<TypeId, Handler>,
    pub(crate) quit_observers: SubscriberSet<(), QuitHandler>,
    pub(crate) restart_observers: SubscriberSet<(), Handler>,
    pub(crate) window_closed_observers: SubscriberSet<(), WindowClosedHandler>,
    pub(crate) pending_window_closed_notifications: VecDeque<WindowId>,
    pub(crate) notifying_window_closed: bool,
    window_native_terminal_observers: SubscriberSet<(), WindowNativeTerminalHandler>,
    pending_window_native_terminal_notifications: VecDeque<WindowId>,
    notifying_window_native_terminal: bool,

    /// Per-App element arena. This isolates element allocations between different
    /// App instances (important for tests where multiple Apps run concurrently).
    pub(crate) element_arena: RefCell<Arena>,
    /// Per-App event arena.
    pub(crate) event_arena: Arena,

    // Drop globals last. We need to ensure all tasks owned by entities and
    // callbacks are marked cancelled at this point as this will also shutdown
    // the tokio runtime. As any task attempting to spawn a blocking tokio task,
    // might panic.
    pub(crate) globals_by_type: TypeIdHashMap<Box<dyn Any>>,

    // assets
    pub(crate) loading_assets: FxHashMap<(TypeId, u64), Box<dyn Any>>,
    asset_source: Arc<dyn AssetSource>,
    pub(crate) svg_renderer: SvgRenderer,
    http_client: Arc<dyn HttpClient>,

    // below is plain data, the drop order is insignificant here
    pub(crate) pending_notifications: FxHashSet<EntityId>,
    pub(crate) pending_global_notifications: TypeIdHashSet,
    pub(crate) restart_path: Option<PathBuf>,
    pub(crate) layout_id_buffer: Vec<LayoutId>, // We recycle this memory across layout requests.
    pub(crate) propagate_event: bool,
    pub(crate) prompt_builder: Option<PromptBuilder>,
    pub(crate) window_invalidators_by_entity:
        FxHashMap<EntityId, FxHashMap<WindowId, WindowInvalidator>>,
    pub(crate) tracked_entities: FxHashMap<WindowId, FxHashSet<EntityId>>,
    pub(crate) current_window_by_entity: FxHashMap<EntityId, WindowId>,
    pub(crate) view_presentation_windows: crate::view_presentation_window::Registry,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_renderer: Option<crate::InspectorRenderer>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_element_registry: InspectorElementRegistry,
    #[cfg(any(test, feature = "test-support", debug_assertions))]
    pub(crate) name: Option<&'static str>,
    pub(crate) text_rendering_mode: Rc<Cell<TextRenderingMode>>,

    pub(crate) window_update_stack: Vec<WindowId>,
    window_update_provenance: WindowUpdateProvenance,
    pub(crate) mode: GpuiMode,
    pub(crate) cursor_hide_mode: CursorHideMode,
    /// Whether the app was created by [`Application::new_inaccessible`]. No
    /// accesskit APIs will be called when this flag is set.
    pub(crate) accessibility_force_disabled: bool,
    flushing_effects: bool,
    pending_updates: usize,
    current_update_generation: u64,
    quit_mode: QuitMode,
    quitting: bool,
    window_open_barrier_depth: usize,
    window_open_epoch: u64,

    // We need to ensure the leak detector drops last, after all tasks, callbacks and things have been dropped.
    // Otherwise it may report false positives.
    #[cfg(any(test, feature = "leak-detection"))]
    _ref_counts: Arc<RwLock<EntityRefCounts>>,
}

struct AppUpdateTransaction<'a> {
    app: &'a mut App,
    pending_updates_before: usize,
    flushing_effects_before: bool,
    finished: bool,
}

impl<'a> AppUpdateTransaction<'a> {
    fn begin(app: &'a mut App) -> Self {
        let pending_updates_before = app.pending_updates;
        let flushing_effects_before = app.flushing_effects;
        app.start_update();
        Self {
            app,
            pending_updates_before,
            flushing_effects_before,
            finished: false,
        }
    }

    fn app_mut(&mut self) -> &mut App {
        &mut *self.app
    }

    fn finish(mut self) {
        self.app.finish_update();
        self.finished = true;
    }
}

impl Drop for AppUpdateTransaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.app.pending_updates = self.pending_updates_before;
            self.app.flushing_effects = self.flushing_effects_before;
        }
    }
}

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new_app(
        platform: Rc<dyn Platform>,
        asset_source: Arc<dyn AssetSource>,
        http_client: Arc<dyn HttpClient>,
    ) -> Rc<AppCell> {
        let background_executor = platform.background_executor();
        let foreground_executor = platform.foreground_executor();
        assert!(
            background_executor.is_main_thread(),
            "must construct App on main thread"
        );

        let text_system = Arc::new(TextSystem::new(platform.text_system()));
        let entities = EntityMap::new();
        let keyboard_layout = platform.keyboard_layout();
        let keyboard_mapper = platform.keyboard_mapper();

        #[cfg(any(test, feature = "leak-detection"))]
        let _ref_counts = entities.ref_counts_drop_handle();

        let app = Rc::new_cyclic(|this| {
            AppCell::new(App {
                this: this.clone(),
                platform: platform.clone(),
                text_system,
                text_rendering_mode: Rc::new(Cell::new(TextRenderingMode::default())),
                mode: GpuiMode::Production,
                actions: Rc::new(ActionRegistry::default()),
                flushing_effects: false,
                pending_updates: 0,
                current_update_generation: 0,
                active_drag: None,
                active_native_captured_drag: None,
                // Zero is reserved so every exported generation can cross into non-zero identity
                // domains without a lossy compatibility mapping.
                next_native_captured_drag_generation: 1,
                background_executor,
                foreground_executor,
                svg_renderer: SvgRenderer::new(asset_source.clone()),
                loading_assets: Default::default(),
                asset_source,
                http_client,
                globals_by_type: Default::default(),
                entities,
                new_entity_observers: SubscriberSet::new(),
                windows: SlotMap::with_key(),
                window_update_stack: Vec::new(),
                window_update_provenance: WindowUpdateProvenance::Ordinary,
                window_handles: FxHashMap::default(),
                window_profiles: FxHashMap::default(),
                focus_handles: Arc::new(RwLock::new(SlotMap::with_key())),
                keymap: Rc::new(RefCell::new(Keymap::default())),
                keyboard_layout,
                keyboard_mapper,
                global_action_listeners: Default::default(),
                pending_effects: VecDeque::new(),
                pending_notifications: FxHashSet::default(),
                pending_global_notifications: Default::default(),
                observers: SubscriberSet::new(),
                tracked_entities: FxHashMap::default(),
                window_invalidators_by_entity: FxHashMap::default(),
                current_window_by_entity: FxHashMap::default(),
                view_presentation_windows: Default::default(),
                event_listeners: SubscriberSet::new(),
                release_listeners: SubscriberSet::new(),
                keystroke_observers: SubscriberSet::new(),
                keystroke_interceptors: SubscriberSet::new(),
                keyboard_layout_observers: SubscriberSet::new(),
                thermal_state_observers: SubscriberSet::new(),
                global_observers: SubscriberSet::new(),
                quit_observers: SubscriberSet::new(),
                restart_observers: SubscriberSet::new(),
                restart_path: None,
                window_closed_observers: SubscriberSet::new(),
                pending_window_closed_notifications: VecDeque::new(),
                notifying_window_closed: false,
                window_native_terminal_observers: SubscriberSet::new(),
                pending_window_native_terminal_notifications: VecDeque::new(),
                notifying_window_native_terminal: false,
                layout_id_buffer: Default::default(),
                propagate_event: true,
                prompt_builder: Some(PromptBuilder::Default),
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector_renderer: None,
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector_element_registry: InspectorElementRegistry::default(),
                quit_mode: QuitMode::default(),
                quitting: false,
                window_open_barrier_depth: 0,
                window_open_epoch: 0,
                cursor_hide_mode: CursorHideMode::default(),
                accessibility_force_disabled: false,

                #[cfg(any(test, feature = "test-support", debug_assertions))]
                name: None,
                element_arena: RefCell::new(Arena::new(1024 * 1024)),
                event_arena: Arena::new(1024 * 1024),

                #[cfg(any(test, feature = "leak-detection"))]
                _ref_counts,
            })
        });

        init_app_menus(platform.as_ref(), &app);
        SystemWindowTabController::init(&mut app.borrow_mut());

        platform.on_open_urls(Box::new({
            let app = Rc::downgrade(&app);
            move |urls| {
                if let Some(app) = app.upgrade() {
                    app.enqueue_native_app_event(native_event_ingress::NativeAppEvent::OpenUrls(
                        urls,
                    ));
                }
            }
        }));

        platform.on_reopen(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    app.enqueue_native_app_event(native_event_ingress::NativeAppEvent::Reopen);
                }
            }
        }));

        platform.on_system_wake(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    app.enqueue_native_app_event(native_event_ingress::NativeAppEvent::SystemWake);
                }
            }
        }));

        platform.on_keyboard_layout_change(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    app.enqueue_native_app_event(
                        native_event_ingress::NativeAppEvent::KeyboardLayoutChanged,
                    );
                }
            }
        }));

        platform.on_thermal_state_change(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    app.enqueue_native_app_event(
                        native_event_ingress::NativeAppEvent::ThermalStateChanged,
                    );
                }
            }
        }));

        platform.on_quit(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    app.enqueue_native_app_event(native_event_ingress::NativeAppEvent::Quit);
                }
            }
        }));

        app
    }

    #[doc(hidden)]
    pub fn ref_counts_drop_handle(&self) -> impl Sized + use<> {
        self.entities.ref_counts_drop_handle()
    }

    /// Captures a snapshot of all entities that currently have alive handles.
    ///
    /// The returned [`LeakDetectorSnapshot`] can later be passed to
    /// [`assert_no_new_leaks`](Self::assert_no_new_leaks) to verify that no
    /// entities created after the snapshot are still alive.
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn leak_detector_snapshot(&self) -> LeakDetectorSnapshot {
        self.entities.leak_detector_snapshot()
    }

    /// Asserts that no entities created after `snapshot` still have alive handles.
    ///
    /// Entities that were already tracked at the time of the snapshot are ignored,
    /// even if they still have handles. Only *new* entities (those whose
    /// `EntityId` was not present in the snapshot) are considered leaks.
    ///
    /// # Panics
    ///
    /// Panics if any new entity handles exist. The panic message lists every
    /// leaked entity with its type name, and includes allocation-site backtraces
    /// when `LEAK_BACKTRACE` is set.
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn assert_no_new_leaks(&self, snapshot: &LeakDetectorSnapshot) {
        self.entities.assert_no_new_leaks(snapshot)
    }

    /// Quit the application gracefully. Handlers registered with [`Context::on_app_quit`]
    /// will be given `SHUTDOWN_TIMEOUT` to complete before exiting.
    pub fn shutdown(&mut self) {
        self.begin_shutdown_with_window_open_barrier(false);
    }

    /// Starts the same terminal shutdown used by a native platform quit callback.
    ///
    /// Native process-level integration workers use this test-only entry point to prove that the
    /// application shutdown fence and native ingress have both reached terminal before asking the
    /// platform message loop to return.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn shutdown_for_native_exit_test(&mut self) {
        self.begin_shutdown_with_window_open_barrier(true);
    }

    /// Starts terminal App shutdown and returns the platform loop only after every native
    /// shutdown authority has settled.
    ///
    /// Native process integration workers use this instead of pairing
    /// [`Self::shutdown_for_native_exit_test`] with an early [`Self::quit`] call. The platform
    /// quit request is consumed by the same terminal fence that clears the window registry and
    /// drains presentation and native-retirement authorities.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn shutdown_and_quit_for_native_exit_test(&mut self) {
        self.quit_after_terminal_shutdown();
    }

    /// Returns whether terminal native shutdown owns no remaining application-bound authority.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn native_exit_authority_is_settled_for_test(&self) -> bool {
        self.this
            .upgrade()
            .is_none_or(|app| app.native_exit_authority_is_settled_for_test())
    }

    pub(super) fn shutdown_from_native_quit(&mut self) {
        self.begin_shutdown_with_window_open_barrier(true);
    }

    pub(super) fn quit_after_terminal_shutdown(&mut self) {
        let Some(app_cell) = self.this.upgrade() else {
            return;
        };
        app_cell.request_terminal_platform_quit();
        self.begin_shutdown_with_window_open_barrier(true);
    }

    fn begin_shutdown_with_window_open_barrier(&mut self, terminate_ingress: bool) {
        let Some(app_cell) = self.this.upgrade() else {
            return;
        };
        let (shutdown_generation, newly_started) =
            app_cell.begin_shutdown_fence(terminate_ingress, self.quitting);
        if !newly_started {
            return;
        }
        self.window_open_epoch = self
            .window_open_epoch
            .checked_add(1)
            .expect("window-open epoch overflowed");
        self.window_open_barrier_depth = self
            .window_open_barrier_depth
            .checked_add(1)
            .expect("window-open shutdown barrier overflowed");
        let mut futures = Vec::new();
        let mut first_panic = None;

        for observer in self.quit_observers.remove(&()) {
            let future = match catch_unwind(AssertUnwindSafe(|| observer(self))) {
                Ok(future) => AssertUnwindSafe(future).catch_unwind().boxed_local(),
                Err(payload) => futures::future::ready(Err(payload)).boxed_local(),
            };
            futures.push(future);
        }

        let futures = futures::future::join_all(futures);
        match self
            .foreground_executor
            .block_with_timeout(SHUTDOWN_TIMEOUT, futures)
        {
            Ok(results) => {
                for result in results {
                    retain_first_app_shutdown_panic(&mut first_panic, result.err());
                }
            }
            Err(_) => log::error!("timed out waiting on app_will_quit"),
        }

        // Quit observers own domain-specific shutdown effects. They must get the first chance to
        // attach a generation-fenced capture-release continuation before the generic fallback
        // revokes an otherwise still-active captured drag.
        let drag_cancellation = catch_unwind(AssertUnwindSafe(|| {
            self.cancel_active_native_captured_drag(crate::PointerCancelReason::WindowClosed);
        }));
        retain_first_app_shutdown_panic(&mut first_panic, drag_cancellation.err());
        app_cell.finish_shutdown_preparation(shutdown_generation, first_panic);
    }

    pub(super) fn prepare_shutdown_pointer_sessions(&mut self) {
        let window_ids = self.windows.keys().collect::<Vec<_>>();
        let mut first_panic = None;
        for window_id in window_ids {
            let cleanup = catch_unwind(AssertUnwindSafe(|| {
                let _ = self.update_window_id(window_id, |_, window, cx| {
                    window.cancel_pointer_session(crate::PointerCancelReason::WindowClosed, cx);
                    window.flush_pending_pointer_cancellations(cx);
                });
            }));
            retain_first_app_shutdown_panic(&mut first_panic, cleanup.err());
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }

    /// Get the id of the current keyboard layout
    pub fn keyboard_layout(&self) -> &dyn PlatformKeyboardLayout {
        self.keyboard_layout.as_ref()
    }

    /// Get the current keyboard mapper.
    pub fn keyboard_mapper(&self) -> &Rc<dyn PlatformKeyboardMapper> {
        &self.keyboard_mapper
    }

    /// Invokes a handler when the current keyboard layout changes
    pub fn on_keyboard_layout_change<F>(&self, mut callback: F) -> Subscription
    where
        F: 'static + FnMut(&mut App),
    {
        let (subscription, activate) = self.keyboard_layout_observers.insert(
            (),
            Box::new(move |cx| {
                callback(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Gracefully quit the application via the platform's standard routine.
    pub fn quit(&self) {
        self.platform.quit();
    }

    /// Returns the current policy for hiding the cursor in response to
    /// keyboard input.
    pub fn cursor_hide_mode(&self) -> CursorHideMode {
        self.cursor_hide_mode
    }

    /// Sets the policy controlling when GPUI hides the cursor in response
    /// to keyboard input.
    pub fn set_cursor_hide_mode(&mut self, mode: CursorHideMode) {
        self.cursor_hide_mode = mode;
    }

    /// Returns whether the cursor is currently visible according to the
    /// platform. This will report `false` after a keyboard input has hidden
    /// the cursor and the user has not yet moved the mouse to restore it.
    ///
    /// See [`App::set_cursor_hide_mode`].
    pub fn is_cursor_visible(&self) -> bool {
        self.platform.is_cursor_visible()
    }

    /// Schedules all windows in the application to be redrawn. This can be called
    /// multiple times in an update cycle and still result in a single redraw.
    pub fn refresh_windows(&mut self) {
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    pub(crate) fn update<R>(&mut self, update: impl FnOnce(&mut Self) -> R) -> R {
        let mut transaction = AppUpdateTransaction::begin(self);
        let result = update(transaction.app_mut());
        transaction.finish();
        result
    }

    pub(crate) fn start_update(&mut self) {
        if self.pending_updates == 0 {
            self.current_update_generation = self
                .current_update_generation
                .checked_add(1)
                .expect("application update generation space exhausted");
        }
        self.pending_updates += 1;
    }

    /// Returns the generation of the current outer application update.
    ///
    /// Framework integrations use this to distinguish work already queued in the current update
    /// from observations produced by a later update. The value is diagnostic ordering authority,
    /// not a wall-clock timestamp.
    #[doc(hidden)]
    pub fn current_update_generation(&self) -> u64 {
        self.current_update_generation
    }

    pub(crate) fn finish_update(&mut self) {
        let shutdown_fence_owns_effect_flush = self
            .this
            .upgrade()
            .is_some_and(|app| app.shutdown_fence_owns_effect_flush());
        if !shutdown_fence_owns_effect_flush && !self.flushing_effects && self.pending_updates == 1
        {
            self.flushing_effects = true;
            self.flush_effects();
            self.flushing_effects = false;
        }
        self.pending_updates -= 1;
    }

    fn abandon_pending_effects_after_shutdown_failure(&mut self) {
        self.pending_effects.clear();
        self.pending_notifications.clear();
        self.pending_global_notifications.clear();
        self.event_arena.clear();
    }

    /// Arrange a callback to be invoked when the given entity calls `notify` on its respective context.
    pub fn observe<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(Entity<W>, &mut App) + 'static,
    ) -> Subscription
    where
        W: 'static,
    {
        self.observe_internal(entity, move |e, cx| {
            on_notify(e, cx);
            true
        })
    }

    pub(crate) fn detect_accessed_entities<R>(
        &mut self,
        callback: impl FnOnce(&mut App) -> R,
    ) -> (R, FxHashSet<EntityId>) {
        let accessed_entities_start = self.entities.accessed_entities.get_mut().clone();
        let result = callback(self);
        let entities_accessed_in_callback = self
            .entities
            .accessed_entities
            .get_mut()
            .difference(&accessed_entities_start)
            .copied()
            .collect::<FxHashSet<EntityId>>();
        (result, entities_accessed_in_callback)
    }

    pub(crate) fn record_entities_accessed(
        &mut self,
        window_handle: AnyWindowHandle,
        invalidator: WindowInvalidator,
        entities: &FxHashSet<EntityId>,
    ) {
        let mut tracked_entities =
            std::mem::take(self.tracked_entities.entry(window_handle.id).or_default());
        for entity in tracked_entities.iter() {
            self.window_invalidators_by_entity
                .entry(*entity)
                .and_modify(|windows| {
                    windows.remove(&window_handle.id);
                });
        }
        for entity in entities.iter() {
            self.window_invalidators_by_entity
                .entry(*entity)
                .or_default()
                .insert(window_handle.id, invalidator.clone());
            if !self.view_presentation_windows.governs(*entity) {
                self.current_window_by_entity
                    .insert(*entity, window_handle.id);
            }
        }
        tracked_entities.clear();
        tracked_entities.extend(entities.iter().copied());
        self.tracked_entities
            .insert(window_handle.id, tracked_entities);
    }

    pub(crate) fn new_observer(&mut self, key: EntityId, value: Handler) -> Subscription {
        let (subscription, activate) = self.observers.insert(key, value);
        self.defer(move |_| activate());
        subscription
    }

    pub(crate) fn observe_internal<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(Entity<W>, &mut App) -> bool + 'static,
    ) -> Subscription
    where
        W: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        self.new_observer(
            entity_id,
            Box::new(move |cx| {
                if let Some(entity) = handle.upgrade() {
                    on_notify(entity, cx)
                } else {
                    false
                }
            }),
        )
    }

    /// Arrange for the given callback to be invoked whenever the given entity emits an event of a given type.
    /// The callback is provided a handle to the emitting entity and a reference to the emitted event.
    pub fn subscribe<T, Event>(
        &mut self,
        entity: &Entity<T>,
        mut on_event: impl FnMut(Entity<T>, &Event, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Event>,
        Event: 'static,
    {
        self.subscribe_internal(entity, move |entity, event, cx| {
            on_event(entity, event, cx);
            true
        })
    }

    pub(crate) fn new_subscription(
        &mut self,
        key: EntityId,
        value: (TypeId, Listener),
    ) -> Subscription {
        let (subscription, activate) = self.event_listeners.insert(key, value);
        self.defer(move |_| activate());
        subscription
    }
    pub(crate) fn subscribe_internal<T, Evt>(
        &mut self,
        entity: &Entity<T>,
        mut on_event: impl FnMut(Entity<T>, &Evt, &mut App) -> bool + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Evt>,
        Evt: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        self.new_subscription(
            entity_id,
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    let event: &Evt = event.downcast_ref().expect("invalid event type");
                    if let Some(entity) = handle.upgrade() {
                        on_event(entity, event, cx)
                    } else {
                        false
                    }
                }),
            ),
        )
    }

    /// Returns handles to all open windows in the application.
    /// Each handle could be downcast to a handle typed for the root view of that window.
    /// To find all windows of a given type, you could filter on
    pub fn windows(&self) -> Vec<AnyWindowHandle> {
        window_registry::handles(self)
    }

    pub(crate) fn set_native_window_control_area(
        &self,
        window_id: WindowId,
        area: Option<crate::WindowControlArea>,
    ) {
        if let Some(app) = self.this.upgrade() {
            app.set_native_window_control_area(window_id, area);
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn native_boundary_diagnostics(
        &self,
        cursor: NativeBoundaryDiagnosticCursor,
    ) -> NativeBoundaryDiagnosticsSnapshot {
        self.this
            .upgrade()
            .map(|app| app.native_boundary_diagnostics(cursor))
            .unwrap_or_default()
    }

    /// Returns the window handles ordered by their appearance on screen, front to back.
    ///
    /// The first window in the returned list is the active/topmost window of the application.
    ///
    /// This method returns None if the platform doesn't implement the method yet.
    pub fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.platform.window_stack()
    }

    /// Returns platform capabilities relevant to multi-viewport docking.
    pub fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        self.platform.viewport_capabilities()
    }

    /// Returns creation and mutation capabilities for ordinary platform windows.
    pub fn window_capabilities(&self) -> PlatformWindowCapabilities {
        self.window_capabilities_for(&WindowKind::Normal, None)
    }

    /// Returns creation and mutation capabilities for a platform window kind on the target display.
    ///
    /// `None` asks the backend about its primary or default display. An unavailable display id is
    /// resolved to that same default so capability projection matches window creation.
    pub fn window_capabilities_for(
        &self,
        kind: &WindowKind,
        display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        self.window_capabilities_for_resolved_display(kind, self.resolve_display_id(display_id))
    }

    pub(crate) fn window_capabilities_for_resolved_display(
        &self,
        kind: &WindowKind,
        display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        self.platform.window_capabilities(kind, display_id)
    }

    /// Returns the capability profile captured for an opened window's actual kind.
    ///
    /// The profile remains readable while GPUI is updating the window and temporarily owns its
    /// mutable state outside the window registry. Closed or uncommitted handles return `None`.
    pub fn window_profile(&self, window: AnyWindowHandle) -> Option<&PlatformWindowProfile> {
        self.window_profiles.get(&window.window_id())
    }

    /// Creates an application-bound transient-owner token for a live committed window.
    pub fn transient_window_owner(
        &self,
        window: AnyWindowHandle,
    ) -> anyhow::Result<WindowTransientOwner> {
        anyhow::ensure!(
            self.window_handles.get(&window.window_id()) == Some(&window),
            "transient owner must reference a live committed window"
        );
        anyhow::ensure!(
            self.this.upgrade().is_some(),
            "transient owner requires a live application"
        );
        Ok(WindowTransientOwner::new(self.this.clone(), window))
    }

    /// Returns the backend hovered-window signal for the current pointer snapshot.
    pub fn hovered_window(&self) -> PlatformHoveredWindow {
        self.platform.hovered_window()
    }

    /// Returns the backend focused-window signal for the current platform snapshot.
    pub fn focused_window(&self) -> PlatformFocusedWindow {
        self.platform.focused_window()
    }

    /// Returns a handle to the window that is currently focused at the platform level, if one exists.
    pub fn active_window(&self) -> Option<AnyWindowHandle> {
        self.platform.active_window()
    }

    /// Returns whether a mouse button is currently pressed when the platform can report it.
    pub fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        self.platform.mouse_button_is_pressed(button)
    }

    /// Opens a new window with the given option and the root view returned by the given function.
    /// The function is invoked with a `Window`, which can be used to interact with window-specific
    /// functionality.
    pub fn open_window<V: 'static + Render>(
        &mut self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> anyhow::Result<WindowHandle<V>> {
        self.open_window_detailed(options, build_root_view)
            .map_err(Into::into)
    }

    /// Opens a window and preserves the synchronous creation stage on failure.
    ///
    /// This is intended for lifecycle authorities that must distinguish a platform creation
    /// failure, a synchronous close, and a before-visibility presentation rejection.
    pub fn open_window_detailed<V: 'static + Render>(
        &mut self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> std::result::Result<WindowHandle<V>, WindowOpenError> {
        self.update(|cx| {
            let mut reservation =
                window_registry::reserve(cx).map_err(WindowOpenError::from_reservation)?;
            let id = reservation.id();
            let handle = WindowHandle::new(id);
            match Window::new(handle.into(), options, reservation.app_mut()) {
                Ok(window) => {
                    let mut rollback = window_registry::WindowCreationRollback::new(
                        id,
                        window,
                        reservation.app_mut().this.clone(),
                    );
                    reservation
                        .validate()
                        .map_err(WindowOpenError::from_reservation)?;
                    if !rollback.window_mut().creation_can_commit() {
                        return Err(WindowOpenError::new(
                            WindowOpenFailureStage::ClosedDuringNativeCreateOrMap,
                            anyhow!(
                                "native window closed while creating or mapping its reservation"
                            ),
                        ));
                    }
                    let root_view = reservation
                        .with_update_scope(|cx| build_root_view(rollback.window_mut(), cx));
                    reservation
                        .validate()
                        .map_err(WindowOpenError::from_reservation)?;
                    if !rollback.window_mut().creation_can_commit() {
                        return Err(WindowOpenError::new(
                            WindowOpenFailureStage::ClosedDuringBuild,
                            anyhow!("window closed while building its initial root view"),
                        ));
                    }
                    rollback.window_mut().root.replace(root_view.into());
                    rollback
                        .window_mut()
                        .defer(reservation.app_mut(), |window: &mut Window, cx| {
                            window.appearance_changed(cx)
                        });

                    // allow a window to draw at least once before returning
                    // this didn't cause any issues on non windows platforms as it seems we always won the race to on_request_frame
                    // on windows we quite frequently lose the race and return a window that has never rendered, which leads to a crash
                    // where DispatchTree::root_node_id asserts on empty nodes
                    let clear = rollback.window_mut().draw(reservation.app_mut());
                    if let Err(error) = reservation.validate() {
                        clear.clear();
                        return Err(WindowOpenError::from_reservation(error));
                    }
                    if !rollback.window_mut().creation_can_commit() {
                        clear.clear();
                        return Err(WindowOpenError::new(
                            WindowOpenFailureStage::ClosedDuringInitialDraw,
                            anyhow!("window closed during its initial draw"),
                        ));
                    }
                    // A resource-invalid first candidate keeps the native window hidden and
                    // retains its completion command. The committed window will actively retry a
                    // fresh frame instead of turning the recoverable draw into an open failure.
                    let initial_presentation = rollback.window_mut().prepare_initial_presentation();
                    clear.clear();
                    reservation
                        .validate()
                        .map_err(WindowOpenError::from_reservation)?;
                    if !rollback.window_mut().creation_can_commit() {
                        return Err(WindowOpenError::new(
                            WindowOpenFailureStage::ClosedDuringInitialPresentation,
                            anyhow!("window closed during its initial presentation"),
                        ));
                    }
                    initial_presentation.map_err(|error| {
                        WindowOpenError::new(
                            WindowOpenFailureStage::BeforeVisibilityPresentation,
                            error,
                        )
                    })?;

                    reservation
                        .commit(rollback)
                        .map_err(WindowOpenError::from_reservation)?;
                    Ok(handle)
                }
                Err(error) => match reservation.validate() {
                    Err(reservation_error) => {
                        Err(WindowOpenError::from_reservation(reservation_error))
                    }
                    Ok(()) => Err(WindowOpenError::new(
                        WindowOpenFailureStage::NativeCreateOrMap,
                        error,
                    )),
                },
            }
        })
    }

    /// Instructs the platform to activate the application by bringing it to the foreground.
    pub fn activate(&self, ignoring_other_apps: bool) {
        self.platform.activate(ignoring_other_apps);
    }

    /// Hide the application at the platform level.
    pub fn hide(&self) {
        self.platform.hide();
    }

    /// Hide other applications at the platform level.
    pub fn hide_other_apps(&self) {
        self.platform.hide_other_apps();
    }

    /// Unhide other applications at the platform level.
    pub fn unhide_other_apps(&self) {
        self.platform.unhide_other_apps();
    }

    /// Returns the list of currently active displays.
    pub fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.platform.displays()
    }

    /// Returns the primary display that will be used for new windows.
    pub fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.platform.primary_display()
    }

    pub(crate) fn display_snapshot(&self) -> PlatformDisplaySnapshot {
        self.platform.display_snapshot()
    }

    /// Returns whether `screen_capture_sources` may work.
    pub fn is_screen_capture_supported(&self) -> bool {
        self.platform.is_screen_capture_supported()
    }

    /// Returns a list of available screen capture sources.
    pub fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.platform.screen_capture_sources()
    }

    /// Returns the display with the given ID, if one exists.
    pub fn find_display(&self, id: DisplayId) -> Option<Rc<dyn PlatformDisplay>> {
        self.displays()
            .iter()
            .find(|display| display.id() == id)
            .cloned()
    }

    pub(crate) fn resolve_display_id(&self, display_id: Option<DisplayId>) -> Option<DisplayId> {
        display_id.filter(|display_id| self.find_display(*display_id).is_some())
    }

    /// Returns the current thermal state of the system.
    pub fn thermal_state(&self) -> ThermalState {
        self.platform.thermal_state()
    }

    /// Invokes a handler when the thermal state changes
    pub fn on_thermal_state_change<F>(&self, mut callback: F) -> Subscription
    where
        F: 'static + FnMut(&mut App),
    {
        let (subscription, activate) = self.thermal_state_observers.insert(
            (),
            Box::new(move |cx| {
                callback(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Returns the appearance of the application's windows.
    pub fn window_appearance(&self) -> WindowAppearance {
        self.platform.window_appearance()
    }

    /// Returns the window button layout configuration when supported.
    pub fn button_layout(&self) -> Option<WindowButtonLayout> {
        self.platform.button_layout()
    }

    /// Reads data from the platform clipboard.
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_clipboard()
    }

    /// Sets the text rendering mode for the application.
    pub fn set_text_rendering_mode(&mut self, mode: TextRenderingMode) {
        self.text_rendering_mode.set(mode);
    }

    /// Returns the current text rendering mode for the application.
    pub fn text_rendering_mode(&self) -> TextRenderingMode {
        self.text_rendering_mode.get()
    }

    /// Writes data to the platform clipboard.
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.platform.write_to_clipboard(item)
    }

    /// Reads data from the primary selection buffer.
    /// Only available on Linux.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.platform.read_from_primary()
    }

    /// Writes data to the primary selection buffer.
    /// Only available on Linux.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn write_to_primary(&self, item: ClipboardItem) {
        self.platform.write_to_primary(item)
    }

    /// Reads data from macOS's "Find" pasteboard.
    ///
    /// Used to share the current search string between apps.
    ///
    /// https://developer.apple.com/documentation/appkit/nspasteboard/name-swift.struct/find
    #[cfg(target_os = "macos")]
    pub fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_find_pasteboard()
    }

    /// Writes data to macOS's "Find" pasteboard.
    ///
    /// Used to share the current search string between apps.
    ///
    /// https://developer.apple.com/documentation/appkit/nspasteboard/name-swift.struct/find
    #[cfg(target_os = "macos")]
    pub fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.platform.write_to_find_pasteboard(item)
    }

    /// Writes credentials to the platform keychain.
    pub fn write_credentials(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<Result<()>> {
        self.platform.write_credentials(url, username, password)
    }

    /// Reads credentials from the platform keychain.
    pub fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        self.platform.read_credentials(url)
    }

    /// Deletes credentials from the platform keychain.
    pub fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        self.platform.delete_credentials(url)
    }

    /// Directs the platform's default browser to open the given URL.
    pub fn open_url(&self, url: &str) {
        self.platform.open_url(url);
    }

    /// Registers the given URL scheme (e.g. `open-gpui` for `open-gpui://` URLs) to be
    /// opened by the current app.
    ///
    /// On some platforms (e.g. macOS) you may be able to register URL schemes
    /// as part of app distribution, but this method exists to let you register
    /// schemes at runtime.
    pub fn register_url_scheme(&self, scheme: &str) -> Task<Result<()>> {
        self.platform.register_url_scheme(scheme)
    }

    /// Returns the full pathname of the current app bundle.
    ///
    /// Returns an error if the app is not being run from a bundle.
    pub fn app_path(&self) -> Result<PathBuf> {
        self.platform.app_path()
    }

    /// On Linux, returns the name of the compositor in use.
    ///
    /// Returns an empty string on other platforms.
    pub fn compositor_name(&self) -> &'static str {
        self.platform.compositor_name()
    }

    /// Returns the file URL of the executable with the specified name in the application bundle
    pub fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.platform.path_for_auxiliary_executable(name)
    }

    /// Displays a platform modal for selecting paths.
    ///
    /// When one or more paths are selected, they'll be relayed asynchronously via the returned oneshot channel.
    /// If cancelled, a `None` will be relayed instead.
    /// May return an error on Linux if the file picker couldn't be opened.
    pub fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        self.platform.prompt_for_paths(options)
    }

    /// Displays a platform modal for selecting a new path where a file can be saved.
    ///
    /// The provided directory will be used to set the initial location.
    /// When a path is selected, it is relayed asynchronously via the returned oneshot channel.
    /// If cancelled, a `None` will be relayed instead.
    /// May return an error on Linux if the file picker couldn't be opened.
    pub fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        self.platform.prompt_for_new_path(directory, suggested_name)
    }

    /// Reveals the specified path at the platform level, such as in Finder on macOS.
    pub fn reveal_path(&self, path: &Path) {
        self.platform.reveal_path(path)
    }

    /// Opens the specified path with the system's default application.
    pub fn open_with_system(&self, path: &Path) {
        self.platform.open_with_system(path)
    }

    /// Returns whether the user has configured scrollbars to auto-hide at the platform level.
    pub fn should_auto_hide_scrollbars(&self) -> bool {
        self.platform.should_auto_hide_scrollbars()
    }

    /// Restarts the application.
    pub fn restart(&mut self) {
        self.restart_observers
            .clone()
            .retain(&(), |observer| observer(self));
        self.platform.restart(self.restart_path.take())
    }

    /// Sets the path to use when restarting the application.
    pub fn set_restart_path(&mut self, path: PathBuf) {
        self.restart_path = Some(path);
    }

    /// Returns the HTTP client for the application.
    pub fn http_client(&self) -> Arc<dyn HttpClient> {
        self.http_client.clone()
    }

    /// Sets the HTTP client for the application.
    pub fn set_http_client(&mut self, new_client: Arc<dyn HttpClient>) {
        self.http_client = new_client;
    }

    /// Configures when the application should automatically quit.
    /// By default, [`QuitMode::Default`] is used.
    pub fn set_quit_mode(&mut self, mode: QuitMode) {
        self.quit_mode = mode;
    }

    /// Returns the SVG renderer used by the application.
    pub fn svg_renderer(&self) -> SvgRenderer {
        self.svg_renderer.clone()
    }

    pub(crate) fn push_effect(&mut self, effect: Effect) {
        match &effect {
            Effect::Notify { emitter } => {
                if !self.pending_notifications.insert(*emitter) {
                    return;
                }
            }
            Effect::NotifyGlobalObservers { global_type } => {
                if !self.pending_global_notifications.insert(*global_type) {
                    return;
                }
            }
            _ => {}
        };

        self.pending_effects.push_back(effect);
    }

    /// Called at the end of [`App::update`] to complete any side effects
    /// such as notifying observers, emitting events, etc. Effects can themselves
    /// cause effects, so we continue looping until all effects are processed.
    fn flush_effects(&mut self) {
        loop {
            self.release_dropped_entities();
            self.release_dropped_focus_handles();
            if let Some(effect) = self.pending_effects.pop_front() {
                match effect {
                    Effect::Notify { emitter } => {
                        self.apply_notify_effect(emitter);
                    }

                    Effect::Emit {
                        emitter,
                        event_type,
                        event,
                    } => self.apply_emit_effect(emitter, event_type, &*event),

                    Effect::RefreshWindows => {
                        self.apply_refresh_effect();
                    }

                    Effect::NotifyGlobalObservers { global_type } => {
                        self.apply_notify_global_observers_effect(global_type);
                    }

                    Effect::Defer { callback } => {
                        self.apply_defer_effect(callback);
                    }
                    Effect::EntityCreated {
                        entity,
                        tid,
                        window,
                    } => {
                        self.apply_entity_created_effect(entity, tid, window);
                    }
                }
            } else {
                #[cfg(any(test, feature = "test-support"))]
                for window in self
                    .windows
                    .values()
                    .filter_map(|window| {
                        let window = window.as_deref()?;
                        (window.invalidator.is_dirty() && !window.invalidator.is_focus_only_dirty())
                            .then_some(window.handle)
                    })
                    .collect::<Vec<_>>()
                {
                    self.update_window(window, |_, window, cx| window.draw(cx).clear())
                        .unwrap();
                }

                if self.pending_effects.is_empty() {
                    self.event_arena.clear();
                    break;
                }
            }
        }
    }

    /// Repeatedly called during `flush_effects` to release any entities whose
    /// reference count has become zero. We invoke any release observers before dropping
    /// each entity.
    fn release_dropped_entities(&mut self) {
        let mut first_panic = None;
        loop {
            let dropped = self.entities.take_dropped();
            if dropped.is_empty() {
                break;
            }

            for (entity_id, mut entity) in dropped {
                self.observers.remove(&entity_id);
                self.event_listeners.remove(&entity_id);
                self.window_invalidators_by_entity.remove(&entity_id);
                for released_entity_id in self.view_presentation_windows.entity_released(entity_id)
                {
                    self.current_window_by_entity.remove(&released_entity_id);
                }
                for release_callback in self.release_listeners.remove(&entity_id) {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        release_callback(entity.as_mut(), self);
                    }));
                    if first_panic.is_none() {
                        first_panic = result.err();
                    }
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    drop(entity);
                }));
                if first_panic.is_none() {
                    first_panic = result.err();
                }
            }
        }

        if let Some(payload) = first_panic {
            std::panic::resume_unwind(payload);
        }
    }

    /// Repeatedly called during `flush_effects` to handle a focused handle being dropped.
    fn release_dropped_focus_handles(&mut self) {
        self.focus_handles
            .clone()
            .write()
            .retain(|handle_id, focus| {
                if focus.ref_count.load(SeqCst) == 0 {
                    for window_handle in self.windows() {
                        window_handle
                            .update(self, |_, window, cx| {
                                window.clear_dropped_focus(handle_id, cx);
                            })
                            .unwrap();
                    }
                    false
                } else {
                    true
                }
            });
    }

    fn apply_notify_effect(&mut self, emitter: EntityId) {
        self.pending_notifications.remove(&emitter);

        self.observers
            .clone()
            .retain(&emitter, |handler| handler(self));
    }

    fn apply_emit_effect(&mut self, emitter: EntityId, event_type: TypeId, event: &dyn Any) {
        self.event_listeners
            .clone()
            .retain(&emitter, |(stored_type, handler)| {
                if *stored_type == event_type {
                    handler(event, self)
                } else {
                    true
                }
            });
    }

    fn apply_refresh_effect(&mut self) {
        for window in self.windows.values_mut() {
            if let Some(window) = window.as_deref_mut() {
                window.refreshing = true;
                window.invalidator.set_dirty(true);
            }
        }
    }

    fn apply_notify_global_observers_effect(&mut self, type_id: TypeId) {
        self.pending_global_notifications.remove(&type_id);
        self.global_observers
            .clone()
            .retain(&type_id, |observer| observer(self));
    }

    fn apply_defer_effect(&mut self, callback: Box<dyn FnOnce(&mut Self) + 'static>) {
        callback(self);
    }

    fn apply_entity_created_effect(
        &mut self,
        entity: AnyEntity,
        tid: TypeId,
        window: Option<WindowId>,
    ) {
        // Seed the entity's current window from its creation context so
        // `with_window` resolves correctly before the entity has ever been
        // rendered.
        if let Some(id) = window
            && !self.view_presentation_windows.governs(entity.entity_id())
        {
            self.current_window_by_entity.insert(entity.entity_id(), id);
        }

        self.new_entity_observers.clone().retain(&tid, |observer| {
            if let Some(id) = window {
                self.update_window_id(id, {
                    let entity = entity.clone();
                    |_, window, cx| (observer)(entity, &mut Some(window), cx)
                })
                .expect("All windows should be off the stack when flushing effects");
            } else {
                (observer)(entity.clone(), &mut None, self)
            }
            true
        });
    }

    /// Run `f` against the entity's *current* window — the most recently
    /// rendered window that referenced the entity, or its creation window if
    /// it has yet to be rendered. Returns `None` if the entity has no
    /// current window, or if that window has been closed, or if it is
    /// already on the update stack.
    pub fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let window_id = self
            .view_presentation_windows
            .resolved_window(entity_id)
            .or_else(|| self.current_window_by_entity.get(&entity_id).copied())?;
        self.update_window_id(window_id, |_, window, cx| f(window, cx))
            .ok()
    }

    fn ensure_window(&mut self, entity_id: EntityId, window: WindowId) {
        if self.view_presentation_windows.governs(entity_id) {
            return;
        }
        self.current_window_by_entity
            .entry(entity_id)
            .or_insert(window);
    }

    pub(crate) fn update_window_id<T, F>(&mut self, id: WindowId, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update_window_id_with_provenance(id, WindowUpdateProvenance::Ordinary, update)
    }

    pub(super) fn update_window_id_from_native<T, F>(
        &mut self,
        id: WindowId,
        sequence: NativeIngressSequence,
        update: F,
    ) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update_window_id_with_provenance(
            id,
            WindowUpdateProvenance::Native {
                source_window: id,
                sequence,
                captured_drag_fact_claimed: false,
            },
            update,
        )
    }

    pub(super) fn update_window_id_from_native_with_preclaimed_captured_drag<T, F>(
        &mut self,
        id: WindowId,
        sequence: NativeIngressSequence,
        update: F,
    ) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update_window_id_with_provenance(
            id,
            WindowUpdateProvenance::Native {
                source_window: id,
                sequence,
                captured_drag_fact_claimed: true,
            },
            update,
        )
    }

    fn update_window_id_with_provenance<T, F>(
        &mut self,
        id: WindowId,
        provenance: WindowUpdateProvenance,
        update: F,
    ) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update(|cx| {
            let mut transaction =
                window_registry::WindowUpdateTransaction::begin(cx, id, provenance)?;
            let result = transaction.update(update);
            transaction.finish()?;

            Some(result)
        })
        .context("window not found")
    }

    /// Creates an `AsyncApp`, which can be cloned and has a static lifetime
    /// so it can be held across `await` points.
    pub fn to_async(&self) -> AsyncApp {
        AsyncApp {
            app: self.this.clone(),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
        }
    }

    /// Obtains a reference to the executor, which can be used to spawn futures.
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// Obtains a reference to the executor, which can be used to spawn futures.
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        if self.quitting {
            panic!("Can't spawn on main thread after on_app_quit")
        };
        &self.foreground_executor
    }

    /// Spawns the future returned by the given function on the main thread. The closure will be invoked
    /// with [AsyncApp], which allows the application state to be accessed across await points.
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        if self.quitting {
            debug_panic!("Can't spawn on main thread after on_app_quit")
        };

        let mut cx = self.to_async();

        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// Spawns the future returned by the given function on the main thread with
    /// the given priority. The closure will be invoked with [AsyncApp], which
    /// allows the application state to be accessed across await points.
    pub fn spawn_with_priority<AsyncFn, R>(&self, priority: Priority, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        if self.quitting {
            debug_panic!("Can't spawn on main thread after on_app_quit")
        };

        let mut cx = self.to_async();

        self.foreground_executor
            .spawn_with_priority(priority, async move { f(&mut cx).await }.boxed_local())
    }

    /// Schedules the given function to be run at the end of the current effect cycle, allowing entities
    /// that are currently on the stack to be returned to the app.
    pub fn defer(&mut self, f: impl FnOnce(&mut App) + 'static) {
        self.push_effect(Effect::Defer {
            callback: Box::new(f),
        });
    }

    /// Schedules lifecycle-critical work before an active shutdown clears the window registry.
    ///
    /// Outside shutdown this behaves like [`Self::defer`], while retaining the callback in
    /// `AppCell` until that deferred slot runs. If shutdown starts first, the callback moves into
    /// the exact shutdown generation instead of remaining vulnerable to ordinary effect
    /// abandonment.
    #[doc(hidden)]
    pub fn defer_shutdown_critical_before_window_registry_clear(
        &mut self,
        callback: impl FnOnce(&mut App) + 'static,
    ) {
        let callback = Box::new(callback);
        let Some(cell) = self.this.upgrade() else {
            self.defer(move |cx| callback(cx));
            return;
        };
        match cell.enqueue_shutdown_critical(
            cell::NativeShutdownCriticalPhase::BeforeWindowRegistryClear,
            callback,
        ) {
            Ok(()) => {}
            Err(cell::NativeShutdownCriticalEnqueueError::Inactive(callback)) => {
                let ticket = cell.protect_pre_shutdown_critical(callback);
                let weak_cell = self.this.clone();
                self.defer(move |cx| {
                    let Some(cell) = weak_cell.upgrade() else {
                        return;
                    };
                    if let Some(callback) = cell.take_pre_shutdown_critical(ticket) {
                        callback(cx);
                    }
                });
            }
            Err(cell::NativeShutdownCriticalEnqueueError::PhasePassed(_)) => {
                panic!("pre-registry-clear shutdown work was scheduled after registry clear")
            }
        }
    }

    /// Defers lifecycle-critical work before registry clear, or runs it in the current App turn
    /// when that shutdown phase has already passed.
    ///
    /// Use this only for idempotent state-machine pumps that may be awakened by late native
    /// terminal events or timers. Ordinary shutdown participants should use
    /// [`Self::defer_shutdown_critical_before_window_registry_clear`] so phase misuse remains a
    /// hard failure.
    #[doc(hidden)]
    pub fn defer_shutdown_critical_before_window_registry_clear_or_run_now(
        &mut self,
        callback: impl FnOnce(&mut App) + 'static,
    ) {
        let callback = Box::new(callback);
        let Some(cell) = self.this.upgrade() else {
            self.defer(move |cx| callback(cx));
            return;
        };
        match cell.enqueue_shutdown_critical(
            cell::NativeShutdownCriticalPhase::BeforeWindowRegistryClear,
            callback,
        ) {
            Ok(()) => {}
            Err(cell::NativeShutdownCriticalEnqueueError::Inactive(callback)) => {
                let ticket = cell.protect_pre_shutdown_critical(callback);
                let weak_cell = self.this.clone();
                self.defer(move |cx| {
                    let Some(cell) = weak_cell.upgrade() else {
                        return;
                    };
                    if let Some(callback) = cell.take_pre_shutdown_critical(ticket) {
                        callback(cx);
                    }
                });
            }
            Err(cell::NativeShutdownCriticalEnqueueError::PhasePassed(callback)) => callback(self),
        }
    }

    /// Runs delayed lifecycle work after the App borrow is available, or transfers it into the
    /// active shutdown generation before the window registry is cleared.
    ///
    /// Native modal loops may poll foreground timers while their initiating update still owns the
    /// App `RefCell`. This primitive keeps the callback in `AppCell` until either the delay expires
    /// and a distinct borrow release occurs, or shutdown claims it as exact pre-clear work.
    #[doc(hidden)]
    pub fn defer_after_or_shutdown_critical_before_window_registry_clear(
        &mut self,
        delay: Duration,
        callback: impl FnOnce(&mut App) + 'static,
    ) {
        let callback = Box::new(callback);
        let Some(cell) = self.this.upgrade() else {
            self.defer(move |cx| callback(cx));
            return;
        };
        if cell.shutdown_fence_owns_effect_flush() {
            match cell.enqueue_shutdown_critical(
                cell::NativeShutdownCriticalPhase::BeforeWindowRegistryClear,
                callback,
            ) {
                Ok(()) => {}
                Err(cell::NativeShutdownCriticalEnqueueError::PhasePassed(callback)) => {
                    match cell.enqueue_shutdown_critical(
                        cell::NativeShutdownCriticalPhase::AfterWindowRegistryClear,
                        callback,
                    ) {
                        Ok(()) => {}
                        Err(
                            cell::NativeShutdownCriticalEnqueueError::Inactive(callback)
                            | cell::NativeShutdownCriticalEnqueueError::PhasePassed(callback),
                        ) => self.defer(move |cx| callback(cx)),
                    }
                }
                Err(cell::NativeShutdownCriticalEnqueueError::Inactive(callback)) => {
                    self.defer(move |cx| callback(cx));
                }
            }
            return;
        }

        let ticket = cell.protect_pre_shutdown_critical(callback);
        let weak_cell = Rc::downgrade(&cell);
        let background_executor = self.background_executor.clone();
        self.foreground_executor
            .spawn(async move {
                background_executor.timer(delay).await;
                loop {
                    let Some(cell) = weak_cell.upgrade() else {
                        return;
                    };
                    let borrow_released = match cell.try_borrow_mut() {
                        Ok(mut app) => {
                            let Some(callback) = cell.take_pre_shutdown_critical(ticket) else {
                                return;
                            };
                            app.update(|cx| callback(cx));
                            return;
                        }
                        Err(_) => cell.wait_for_app_borrow_release(),
                    };
                    drop(cell);
                    if borrow_released.await.is_err() {
                        return;
                    }
                }
            })
            .detach();
    }

    /// Schedules lifecycle-critical work after the active shutdown clears the window registry.
    ///
    /// This is an exact-generation shutdown participant rather than an ordinary deferred effect.
    /// It must only be called while shutdown is active.
    #[doc(hidden)]
    pub fn defer_shutdown_critical_after_window_registry_clear(
        &mut self,
        callback: impl FnOnce(&mut App) + 'static,
    ) {
        let callback = Box::new(callback);
        let Some(cell) = self.this.upgrade() else {
            panic!("post-registry-clear shutdown work requires a live AppCell")
        };
        match cell.enqueue_shutdown_critical(
            cell::NativeShutdownCriticalPhase::AfterWindowRegistryClear,
            callback,
        ) {
            Ok(()) => {}
            Err(cell::NativeShutdownCriticalEnqueueError::Inactive(_)) => {
                panic!("post-registry-clear shutdown work requires an active shutdown generation")
            }
            Err(cell::NativeShutdownCriticalEnqueueError::PhasePassed(_)) => {
                unreachable!("post-registry-clear work remains admissible until shutdown completes")
            }
        }
    }

    /// Requires every dependent native window to reach terminal before retiring `anchor`.
    ///
    /// Dependencies are exact full [`WindowId`] values and must be registered before the anchor
    /// is logically removed. Native ownership remains in `AppCell`; callers only declare the
    /// ordering relationship.
    #[doc(hidden)]
    pub fn register_native_window_retirement_dependencies(
        &self,
        anchor: WindowId,
        dependencies: impl IntoIterator<Item = WindowId>,
    ) -> std::result::Result<(), NativeWindowRetirementDependencyError> {
        self.this
            .upgrade()
            .expect("native retirement dependencies require a live AppCell")
            .register_native_window_retirement_dependencies(anchor, dependencies)
    }

    /// Cancels the native-retirement ordering group owned by `anchor`.
    ///
    /// This is reserved for an anchor whose direct logical or native disappearance has already
    /// invalidated a previously declared dependent-first close protocol. If the anchor retirement
    /// is already waiting on that group, its retained platform owner becomes dispatchable.
    #[doc(hidden)]
    pub fn cancel_native_window_retirement_dependencies(&self, anchor: WindowId) -> bool {
        self.this
            .upgrade()
            .expect("native retirement dependencies require a live AppCell")
            .cancel_native_window_retirement_dependencies(anchor)
    }

    /// Accessor for the application's asset source, which is provided when constructing the `App`.
    pub fn asset_source(&self) -> &Arc<dyn AssetSource> {
        &self.asset_source
    }

    /// Accessor for the text system.
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// Check whether a global of the given type has been assigned.
    pub fn has_global<G: Global>(&self) -> bool {
        self.globals_by_type.contains_key(&TypeId::of::<G>())
    }

    /// Access the global of the given type. Panics if a global for that type has not been assigned.
    #[track_caller]
    pub fn global<G: Global>(&self) -> &G {
        self.globals_by_type
            .get(&TypeId::of::<G>())
            .map(|any_state| any_state.downcast_ref::<G>().unwrap())
            .unwrap_or_else(|| panic!("no state of type {} exists", type_name::<G>()))
    }

    /// Access the global of the given type if a value has been assigned.
    pub fn try_global<G: Global>(&self) -> Option<&G> {
        self.globals_by_type
            .get(&TypeId::of::<G>())
            .map(|any_state| any_state.downcast_ref::<G>().unwrap())
    }

    /// Access the global of the given type mutably. Panics if a global for that type has not been assigned.
    #[track_caller]
    pub fn global_mut<G: Global>(&mut self) -> &mut G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type
            .get_mut(&global_type)
            .and_then(|any_state| any_state.downcast_mut::<G>())
            .unwrap_or_else(|| panic!("no state of type {} exists", type_name::<G>()))
    }

    /// Access the global of the given type mutably. A default value is assigned if a global of this type has not
    /// yet been assigned.
    pub fn default_global<G: Global + Default>(&mut self) -> &mut G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type
            .entry(global_type)
            .or_insert_with(|| Box::<G>::default())
            .downcast_mut::<G>()
            .unwrap()
    }

    /// Sets the value of the global of the given type.
    pub fn set_global<G: Global>(&mut self, global: G) {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type.insert(global_type, Box::new(global));
    }

    /// Clear all stored globals. Does not notify global observers.
    #[cfg(any(test, feature = "test-support"))]
    pub fn clear_globals(&mut self) {
        self.globals_by_type.drain();
    }

    /// Remove the global of the given type from the app context. Does not notify global observers.
    pub fn remove_global<G: Global>(&mut self) -> G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        *self
            .globals_by_type
            .remove(&global_type)
            .unwrap_or_else(|| panic!("no global added for {}", type_name::<G>()))
            .downcast()
            .unwrap()
    }

    /// Register a callback to be invoked when a global of the given type is updated.
    pub fn observe_global<G: Global>(
        &mut self,
        mut f: impl FnMut(&mut Self) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                f(cx);
                true
            }),
        );
        self.defer(move |_| activate());
        subscription
    }

    /// Move the global of the given type to the stack.
    #[track_caller]
    pub(crate) fn lease_global<G: Global>(&mut self) -> GlobalLease<G> {
        GlobalLease::new(
            self.globals_by_type
                .remove(&TypeId::of::<G>())
                .with_context(|| format!("no global registered of type {}", type_name::<G>()))
                .unwrap(),
        )
    }

    /// Restore the global of the given type after it is moved to the stack.
    pub(crate) fn end_global_lease<G: Global>(&mut self, lease: GlobalLease<G>) {
        let global_type = TypeId::of::<G>();

        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type.insert(global_type, lease.global);
    }

    pub(crate) fn new_entity_observer(
        &self,
        key: TypeId,
        value: NewEntityListener,
    ) -> Subscription {
        let (subscription, activate) = self.new_entity_observers.insert(key, value);
        activate();
        subscription
    }

    /// Arrange for the given function to be invoked whenever a view of the specified type is created.
    /// The function will be passed a mutable reference to the view along with an appropriate context.
    pub fn observe_new<T: 'static>(
        &self,
        on_new: impl 'static + Fn(&mut T, Option<&mut Window>, &mut Context<T>),
    ) -> Subscription {
        self.new_entity_observer(
            TypeId::of::<T>(),
            Box::new(
                move |any_entity: AnyEntity, window: &mut Option<&mut Window>, cx: &mut App| {
                    any_entity
                        .downcast::<T>()
                        .unwrap()
                        .update(cx, |entity_state, cx| {
                            on_new(entity_state, window.as_deref_mut(), cx)
                        })
                },
            ),
        )
    }

    /// Observe the release of a entity. The callback is invoked after the entity
    /// has no more strong references but before it has been dropped.
    pub fn observe_release<T>(
        &self,
        handle: &Entity<T>,
        on_release: impl FnOnce(&mut T, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let (subscription, activate) = self.release_listeners.insert(
            handle.entity_id(),
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                on_release(entity, cx)
            }),
        );
        activate();
        subscription
    }

    /// Observe the release of a entity. The callback is invoked after the entity
    /// has no more strong references but before it has been dropped.
    pub fn observe_release_in<T>(
        &self,
        handle: &Entity<T>,
        window: &Window,
        on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let window_handle = window.handle;
        self.observe_release(handle, move |entity, cx| {
            let _ = window_handle.update(cx, |_, window, cx| on_release(entity, window, cx));
        })
    }

    /// Register a callback to be invoked when a keystroke is received by the application
    /// in any window. Note that this fires after all other action and event mechanisms have resolved
    /// and that this API will not be invoked if the event's propagation is stopped.
    pub fn observe_keystrokes(
        &mut self,
        mut f: impl FnMut(&KeystrokeEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_observers: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_observers.insert((), handler);
            activate();
            subscription
        }

        inner(
            &self.keystroke_observers,
            Box::new(move |event, window, cx| {
                f(event, window, cx);
                true
            }),
        )
    }

    /// Register a callback to be invoked when a keystroke is received by the application
    /// in any window. Note that this fires _before_ all other action and event mechanisms have resolved
    /// unlike [`App::observe_keystrokes`] which fires after. This means that `cx.stop_propagation` calls
    /// within interceptors will prevent action dispatch
    pub fn intercept_keystrokes(
        &mut self,
        mut f: impl FnMut(&KeystrokeEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_interceptors: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_interceptors.insert((), handler);
            activate();
            subscription
        }

        inner(
            &self.keystroke_interceptors,
            Box::new(move |event, window, cx| {
                f(event, window, cx);
                true
            }),
        )
    }

    /// Register key bindings.
    pub fn bind_keys(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        self.keymap.borrow_mut().add_bindings(bindings);
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    /// Clear all key bindings in the app.
    pub fn clear_key_bindings(&mut self) {
        self.keymap.borrow_mut().clear();
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    /// Get all key bindings in the app.
    pub fn key_bindings(&self) -> Rc<RefCell<Keymap>> {
        self.keymap.clone()
    }

    /// Register a global handler for actions invoked via the keyboard. These handlers are run at
    /// the end of the bubble phase for actions, and so will only be invoked if there are no other
    /// handlers or if they called `cx.propagate()`.
    pub fn on_action<A: Action>(
        &mut self,
        listener: impl Fn(&A, &mut Self) + 'static,
    ) -> &mut Self {
        self.global_action_listeners
            .entry(TypeId::of::<A>())
            .or_default()
            .push(Rc::new(move |action, phase, cx| {
                if phase == DispatchPhase::Bubble {
                    let action = action.downcast_ref().unwrap();
                    listener(action, cx)
                }
            }));
        self
    }

    /// Event handlers propagate events by default. Call this method to stop dispatching to
    /// event handlers with a lower z-index (mouse) or higher in the tree (keyboard). This is
    /// the opposite of [`Self::propagate`]. It's also possible to cancel a call to [`Self::propagate`] by
    /// calling this method before effects are flushed.
    pub fn stop_propagation(&mut self) {
        self.propagate_event = false;
    }

    /// Action handlers stop propagation by default during the bubble phase of action dispatch
    /// dispatching to action handlers higher in the element tree. This is the opposite of
    /// [`Self::stop_propagation`]. It's also possible to cancel a call to [`Self::stop_propagation`] by calling
    /// this method before effects are flushed.
    pub fn propagate(&mut self) {
        self.propagate_event = true;
    }

    /// Build an action from some arbitrary data, typically a keymap entry.
    pub fn build_action(
        &self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> std::result::Result<Box<dyn Action>, ActionBuildError> {
        self.actions.build_action(name, data)
    }

    /// Get all action names that have been registered. Note that registration only allows for
    /// actions to be built dynamically, and is unrelated to binding actions in the element tree.
    pub fn all_action_names(&self) -> &[&'static str] {
        self.actions.all_action_names()
    }

    /// Returns key bindings that invoke the given action on the currently focused element, without
    /// checking context. Bindings are returned in the order they were added. For display, the last
    /// binding should take precedence.
    pub fn all_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        RefCell::borrow(&self.keymap).all_bindings_for_input(input)
    }

    /// Get all non-internal actions that have been registered, along with their schemas.
    pub fn action_schemas(
        &self,
        generator: &mut schemars::SchemaGenerator,
    ) -> Vec<(&'static str, Option<schemars::Schema>)> {
        self.actions.action_schemas(generator)
    }

    /// Get the schema for a specific action by name.
    /// Returns `None` if the action is not found.
    /// Returns `Some(None)` if the action exists but has no schema.
    /// Returns `Some(Some(schema))` if the action exists and has a schema.
    pub fn action_schema_by_name(
        &self,
        name: &str,
        generator: &mut schemars::SchemaGenerator,
    ) -> Option<Option<schemars::Schema>> {
        self.actions.action_schema_by_name(name, generator)
    }

    /// Get a map from a deprecated action name to the canonical name.
    pub fn deprecated_actions_to_preferred_actions(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.deprecated_aliases()
    }

    /// Get a map from an action name to the deprecation messages.
    pub fn action_deprecation_messages(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.deprecation_messages()
    }

    /// Get a map from an action name to the documentation.
    pub fn action_documentation(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.documentation()
    }

    /// Register a callback to be invoked when the application is about to quit.
    /// It is not possible to cancel the quit event at this point.
    pub fn on_app_quit<Fut>(
        &self,
        mut on_quit: impl FnMut(&mut App) -> Fut + 'static,
    ) -> Subscription
    where
        Fut: 'static + Future<Output = ()>,
    {
        let (subscription, activate) = self.quit_observers.insert(
            (),
            Box::new(move |cx| {
                let future = on_quit(cx);
                future.boxed_local()
            }),
        );
        activate();
        subscription
    }

    /// Register a callback to be invoked when the application is about to restart.
    ///
    /// These callbacks are called before any `on_app_quit` callbacks.
    pub fn on_app_restart(&self, mut on_restart: impl 'static + FnMut(&mut App)) -> Subscription {
        let (subscription, activate) = self.restart_observers.insert(
            (),
            Box::new(move |cx| {
                on_restart(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Register a callback to be invoked when a window is closed
    /// The window is no longer accessible at the point this callback is invoked.
    pub fn on_window_closed(
        &self,
        mut on_closed: impl FnMut(&mut App, WindowId) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.window_closed_observers.insert((), Box::new(on_closed));
        activate();
        subscription
    }

    /// Registers a callback for the terminal release of a native window authority.
    ///
    /// This is later than [`Self::on_window_closed`]: the backend's single-shot
    /// [`crate::PlatformWindow::on_close`] callback has delivered its terminal native `Closed`
    /// event and the logical window is already absent.
    pub fn on_window_native_terminal(
        &self,
        on_terminal: impl FnMut(&mut App, WindowId) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self
            .window_native_terminal_observers
            .insert((), Box::new(on_terminal));
        activate();
        subscription
    }

    pub(crate) fn notify_window_native_terminal(&mut self, window_id: WindowId) {
        self.pending_window_native_terminal_notifications
            .push_back(window_id);
        self.flush_window_native_terminal_notifications();
    }

    fn flush_window_native_terminal_notifications(&mut self) {
        if self.notifying_window_native_terminal {
            return;
        }

        self.notifying_window_native_terminal = true;
        let mut first_panic = None;
        while let Some(window_id) = self
            .pending_window_native_terminal_notifications
            .pop_front()
        {
            self.window_native_terminal_observers
                .clone()
                .retain(&(), |callback| {
                    let candidate =
                        catch_unwind(AssertUnwindSafe(|| callback(self, window_id))).err();
                    if first_panic.is_none() {
                        first_panic = candidate;
                    }
                    true
                });
        }
        self.notifying_window_native_terminal = false;

        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }

    pub(crate) fn clear_pending_keystrokes(&mut self) {
        for window in self.windows() {
            window
                .update(self, |_, window, cx| {
                    if window.pending_input_keystrokes().is_some() {
                        window.clear_pending_keystrokes();
                        window.pending_input_changed(cx);
                    }
                })
                .ok();
        }
    }

    /// Checks if the given action is bound in the current context, as defined by the app's current focus,
    /// the bindings in the element tree, and any global action listeners.
    pub fn is_action_available(&mut self, action: &dyn Action) -> bool {
        action_dispatch::is_action_available(self, action)
    }

    /// Sets the menu bar for this application. This will replace any existing menu bar.
    pub fn set_menus(&self, menus: impl IntoIterator<Item = Menu>) {
        let menus: Vec<Menu> = menus.into_iter().collect();
        self.platform.set_menus(menus, &self.keymap.borrow());
    }

    /// Gets the menu bar for this application.
    pub fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.platform.get_menus()
    }

    /// Sets the right click menu for the app icon in the dock
    pub fn set_dock_menu(&self, menus: Vec<MenuItem>) {
        self.platform.set_dock_menu(menus, &self.keymap.borrow())
    }

    /// Performs the action associated with the given dock menu item, only used on Windows for now.
    pub fn perform_dock_menu_action(&self, action: usize) {
        self.platform.perform_dock_menu_action(action);
    }

    /// Adds given path to the bottom of the list of recent paths for the application.
    /// The list is usually shown on the application icon's context menu in the dock,
    /// and allows to open the recent files via that context menu.
    /// If the path is already in the list, it will be moved to the bottom of the list.
    pub fn add_recent_document(&self, path: &Path) {
        self.platform.add_recent_document(path);
    }

    /// Updates the jump list with the updated list of recent paths for the application, only used on Windows for now.
    /// Note that this also sets the dock menu on Windows.
    pub fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.platform.update_jump_list(menus, entries)
    }

    /// Dispatch an action to the currently focused window or global action handler
    /// See [`crate::Action`] for more information on how actions work
    pub fn dispatch_action(&mut self, action: &dyn Action) {
        action_dispatch::dispatch_action(self, action);
    }

    /// Is there currently something being dragged?
    pub fn has_active_drag(&self) -> bool {
        self.active_drag.is_some()
    }

    pub(crate) fn clear_active_drag_for_window(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Option<NativeCapturedDragGeneration>) {
        let Some(active_drag) = self
            .active_drag
            .as_ref()
            .filter(|drag| drag.window_id == window_id)
        else {
            return (false, None);
        };
        let captured_generation = self
            .active_native_captured_drag
            .as_ref()
            .and_then(|authority| authority.generation_for(active_drag));
        self.active_drag = None;
        self.retire_native_captured_drag_authority();
        (true, captured_generation)
    }

    /// Returns the typed value for the current drag operation, when it matches `T`.
    pub fn active_drag_value<T: 'static>(&self) -> Option<&T> {
        self.active_drag
            .as_ref()
            .and_then(|drag| drag.value.downcast_ref::<T>())
    }

    /// Gets the cursor style of the currently active drag operation.
    pub fn active_drag_cursor_style(&self) -> Option<CursorStyle> {
        self.active_drag.as_ref().and_then(|drag| drag.cursor_style)
    }

    /// Stops active drag and clears any related effects.
    pub fn stop_active_drag(&mut self, window: &mut Window) -> bool {
        let Some(active_drag) = self.active_drag.take() else {
            return false;
        };
        self.retire_native_captured_drag_authority();
        if let Some(source) = active_drag.source {
            if source.window_id() == window.window_handle().window_id() {
                let _ = window.finish_drag_source(&source, active_drag.button);
            } else {
                let source_window_id = source.window_id();
                let button = active_drag.button;
                let release = self.update_window_id(source_window_id, |_, source_window, _| {
                    let _ = source_window.finish_drag_source(&source, button);
                    source_window.refresh();
                });
                if release.is_err() && self.windows.contains_key(source_window_id) {
                    self.defer(move |cx| {
                        cx.update_window_id(source_window_id, |_, source_window, _| {
                            let _ = source_window.finish_drag_source(&source, button);
                            source_window.refresh();
                        })
                        .ok();
                    });
                }
            }
        }
        window.refresh();
        true
    }

    /// Sets the cursor style for the currently active drag operation.
    pub fn set_active_drag_cursor_style(
        &mut self,
        cursor_style: CursorStyle,
        window: &mut Window,
    ) -> bool {
        if let Some(ref mut drag) = self.active_drag {
            drag.cursor_style = Some(cursor_style);
            window.refresh();
            true
        } else {
            false
        }
    }

    /// Set the prompt renderer for GPUI. This will replace the default or platform specific
    /// prompts with this custom implementation.
    pub fn set_prompt_builder(
        &mut self,
        renderer: impl Fn(
            PromptLevel,
            &str,
            Option<&str>,
            &[PromptButton],
            PromptHandle,
            &mut Window,
            &mut App,
        ) -> RenderablePromptHandle
        + 'static,
    ) {
        self.prompt_builder = Some(PromptBuilder::Custom(Box::new(renderer)));
    }

    /// Reset the prompt builder to the default implementation.
    pub fn reset_prompt_builder(&mut self) {
        self.prompt_builder = Some(PromptBuilder::Default);
    }

    /// Remove an asset from GPUI's cache
    pub fn remove_asset<A: Asset>(&mut self, source: &A::Source) {
        let asset_id = (TypeId::of::<A>(), hash(source));
        self.loading_assets.remove(&asset_id);
    }

    /// Asynchronously load an asset, if the asset hasn't finished loading this will return None.
    ///
    /// Note that the multiple calls to this method will only result in one `Asset::load` call at a
    /// time, and the results of this call will be cached
    pub fn fetch_asset<A: Asset>(&mut self, source: &A::Source) -> (Shared<Task<A::Output>>, bool) {
        let asset_id = (TypeId::of::<A>(), hash(source));
        let mut is_first = false;
        let task = self
            .loading_assets
            .remove(&asset_id)
            .map(|boxed_task| *boxed_task.downcast::<Shared<Task<A::Output>>>().unwrap())
            .unwrap_or_else(|| {
                is_first = true;
                let future = A::load(source.clone(), self);

                self.background_executor().spawn(future).shared()
            });

        self.loading_assets.insert(asset_id, Box::new(task.clone()));

        (task, is_first)
    }

    /// Obtain a new [`FocusHandle`], which allows you to track and manipulate the keyboard focus
    /// for elements rendered within this window.
    #[track_caller]
    pub fn focus_handle(&self) -> FocusHandle {
        FocusHandle::new(&self.focus_handles)
    }

    /// Tell GPUI that an entity has changed and observers of it should be notified.
    pub fn notify(&mut self, entity_id: EntityId) {
        let window_invalidators = mem::take(
            self.window_invalidators_by_entity
                .entry(entity_id)
                .or_default(),
        );

        // `window_invalidators_by_entity` is monotonic, so an entry alone
        // doesn't mean the window is currently rendering the entity. Filter
        // through `tracked_entities` to keep invalidation tight to windows
        // that actually display this entity right now.
        let live_invalidators: SmallVec<[WindowInvalidator; 2]> = window_invalidators
            .iter()
            .filter(|(window_id, _)| {
                self.tracked_entities
                    .get(window_id)
                    .is_some_and(|set| set.contains(&entity_id))
            })
            .map(|(_, invalidator)| invalidator.clone())
            .collect();

        if live_invalidators.is_empty() {
            if self.pending_notifications.insert(entity_id) {
                self.pending_effects
                    .push_back(Effect::Notify { emitter: entity_id });
            }
        } else {
            for invalidator in &live_invalidators {
                invalidator.invalidate_view(entity_id, self);
            }
        }

        self.window_invalidators_by_entity
            .insert(entity_id, window_invalidators);
    }

    /// Returns the name for this [`App`].
    #[cfg(any(test, feature = "test-support", debug_assertions))]
    pub fn get_name(&self) -> Option<&'static str> {
        self.name
    }

    /// Returns `true` if the platform file picker supports selecting a mix of files and directories.
    pub fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.platform.can_select_mixed_files_and_dirs()
    }

    /// Removes an image from the sprite atlas on all windows.
    ///
    /// If the current window is being updated, it will be removed from `App.windows`, you can use `current_window` to specify the current window.
    /// This is a no-op if the image is not in the sprite atlas.
    pub fn drop_image(&mut self, image: Arc<RenderImage>, current_window: Option<&mut Window>) {
        // remove the texture from all other windows
        for window in self.windows.values_mut().flatten() {
            _ = window.drop_image(image.clone());
        }

        // remove the texture from the current window
        if let Some(window) = current_window {
            _ = window.drop_image(image);
        }
    }

    /// Sets the renderer for the inspector.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn set_inspector_renderer(&mut self, f: crate::InspectorRenderer) {
        self.inspector_renderer = Some(f);
    }

    /// Registers a renderer specific to an inspector state.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn register_inspector_element<T: 'static, R: crate::IntoElement>(
        &mut self,
        f: impl 'static + Fn(crate::InspectorElementId, &T, &mut Window, &mut App) -> R,
    ) {
        self.inspector_element_registry.register(f);
    }

    /// Initializes gpui's default colors for the application.
    ///
    /// These colors can be accessed through `cx.default_colors()`.
    pub fn init_colors(&mut self) {
        self.set_global(GlobalColors(Arc::new(Colors::default())));
    }
}

impl AppContext for App {
    /// Builds an entity that is owned by the application.
    ///
    /// The given function will be invoked with a [`Context`] and must return an object representing the entity. An
    /// [`Entity`] handle will be returned, which can be used to access the entity in a context.
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        self.update(|cx| {
            let slot = cx.entities.reserve();
            let handle = slot.clone();
            let entity = build_entity(&mut Context::new_context(cx, slot.downgrade()));

            cx.push_effect(Effect::EntityCreated {
                entity: handle.into_any(),
                tid: TypeId::of::<T>(),
                window: cx.window_update_stack.last().cloned(),
            });

            cx.entities.insert(slot, entity)
        })
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        Reservation(self.entities.reserve())
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.update(|cx| {
            let slot = reservation.0;
            let entity = build_entity(&mut Context::new_context(cx, slot.downgrade()));
            cx.entities.insert(slot, entity)
        })
    }

    /// Updates the entity referenced by the given handle. The function is passed a mutable reference to the
    /// entity along with a `Context` for the entity.
    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        self.update(|cx| {
            let mut entity = cx.entities.lease(handle);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                update(
                    &mut entity,
                    &mut Context::new_context(cx, handle.downgrade()),
                )
            }));
            cx.entities.end_lease(entity);
            match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }

    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        GpuiBorrow::new(handle.clone(), self)
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let entity = self.entities.read(handle);
        read(entity, self)
    }

    fn update_window<T, F>(&mut self, handle: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update_window_id(handle.id, update)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        App::with_window(self, entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let window = self
            .windows
            .get(window.id)
            .context("window not found")?
            .as_deref()
            .expect("attempted to read a window that is already on the stack");

        let root_view = window.root.clone().unwrap();
        let view = root_view
            .downcast::<T>()
            .map_err(|_| anyhow!("root view's type has changed"))?;

        Ok(read(view, self))
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let mut g = self.global::<G>();
        callback(g, self)
    }
}

/// These effects are processed at the end of each application update cycle.
pub(crate) enum Effect {
    Notify {
        emitter: EntityId,
    },
    Emit {
        emitter: EntityId,
        event_type: TypeId,
        event: ArenaBox<dyn Any>,
    },
    RefreshWindows,
    NotifyGlobalObservers {
        global_type: TypeId,
    },
    Defer {
        callback: Box<dyn FnOnce(&mut App) + 'static>,
    },
    EntityCreated {
        entity: AnyEntity,
        tid: TypeId,
        window: Option<WindowId>,
    },
}

impl std::fmt::Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Notify { emitter } => write!(f, "Notify({})", emitter),
            Effect::Emit { emitter, .. } => write!(f, "Emit({:?})", emitter),
            Effect::RefreshWindows => write!(f, "RefreshWindows"),
            Effect::NotifyGlobalObservers { global_type } => {
                write!(f, "NotifyGlobalObservers({:?})", global_type)
            }
            Effect::Defer { .. } => write!(f, "Defer(..)"),
            Effect::EntityCreated { entity, .. } => write!(f, "EntityCreated({:?})", entity),
        }
    }
}

/// Wraps a global variable value during `update_global` while the value has been moved to the stack.
pub(crate) struct GlobalLease<G: Global> {
    global: Box<dyn Any>,
    global_type: PhantomData<G>,
}

impl<G: Global> GlobalLease<G> {
    fn new(global: Box<dyn Any>) -> Self {
        GlobalLease {
            global,
            global_type: PhantomData,
        }
    }
}

impl<G: Global> Deref for GlobalLease<G> {
    type Target = G;

    fn deref(&self) -> &Self::Target {
        self.global.downcast_ref().unwrap()
    }
}

impl<G: Global> DerefMut for GlobalLease<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.global.downcast_mut().unwrap()
    }
}

/// Contains state associated with an active drag operation, started by dragging an element
/// within the window or by dragging into the app from the underlying platform.
pub struct AnyDrag {
    /// The window where this drag gesture started.
    pub window_id: WindowId,

    /// Stable interactive owner for framework drags, or `None` for window-owned external drags.
    pub source: Option<PointerCaptureHandle>,

    /// The view used to render this drag
    pub view: AnyView,

    /// The value of the dragged item, to be dropped
    pub value: Arc<dyn Any>,

    /// Window-space offset used to keep the drag preview under the pointer.
    pub window_preview_offset: Point<Pixels>,

    /// The cursor style to use while dragging
    pub cursor_style: Option<CursorStyle>,

    /// The mouse button whose release terminates this drag gesture.
    pub button: MouseButton,
}

/// Contains state associated with a tooltip. You'll only need this struct if you're implementing
/// tooltip behavior on a custom element. Otherwise, use [Div::tooltip](crate::Interactivity::tooltip).
#[derive(Clone)]
pub struct AnyTooltip {
    /// The view used to display the tooltip
    pub view: AnyView,

    /// The absolute position of the mouse when the tooltip was deployed.
    pub mouse_position: Point<Pixels>,

    /// Given the bounds of the tooltip, checks whether the tooltip should still be visible and
    /// updates its state accordingly. This is needed atop the hovered element's mouse move handler
    /// to handle the case where the element is not painted (e.g. via use of `visible_on_hover`).
    pub check_visible_and_update: Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) -> bool>,
}

/// A keystroke event, and potentially the associated action
#[derive(Debug)]
pub struct KeystrokeEvent {
    /// The keystroke that occurred
    pub keystroke: Keystroke,

    /// The action that was resolved for the keystroke, if any
    pub action: Option<Box<dyn Action>>,

    /// The context stack at the time
    pub context_stack: Vec<KeyContext>,
}

struct NullHttpClient;

impl HttpClient for NullHttpClient {
    fn send(
        &self,
        _req: open_gpui_http_client::Request<open_gpui_http_client::AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        anyhow::Result<open_gpui_http_client::Response<open_gpui_http_client::AsyncBody>>,
    > {
        async move {
            anyhow::bail!("No HttpClient available");
        }
        .boxed()
    }

    fn user_agent(&self) -> Option<&open_gpui_http_client::http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}

/// A mutable reference to an entity owned by GPUI
pub struct GpuiBorrow<'a, T> {
    inner: Option<Lease<T>>,
    transaction: Option<AppUpdateTransaction<'a>>,
}

impl<'a, T: 'static> GpuiBorrow<'a, T> {
    fn new(inner: Entity<T>, app: &'a mut App) -> Self {
        let mut transaction = AppUpdateTransaction::begin(app);
        let lease = transaction.app_mut().entities.lease(&inner);
        Self {
            inner: Some(lease),
            transaction: Some(transaction),
        }
    }
}

impl<'a, T: 'static> std::borrow::Borrow<T> for GpuiBorrow<'a, T> {
    fn borrow(&self) -> &T {
        self.inner.as_ref().unwrap().borrow()
    }
}

impl<'a, T: 'static> std::borrow::BorrowMut<T> for GpuiBorrow<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap().borrow_mut()
    }
}

impl<'a, T: 'static> std::ops::Deref for GpuiBorrow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl<'a, T: 'static> std::ops::DerefMut for GpuiBorrow<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }
}

impl<'a, T> Drop for GpuiBorrow<'a, T> {
    fn drop(&mut self) {
        let lease = self.inner.take().unwrap();
        let mut transaction = self
            .transaction
            .take()
            .expect("GPUI borrow must own its update transaction");
        let app = transaction.app_mut();
        app.notify(lease.id);
        app.entities.end_lease(lease);

        if !std::thread::panicking() {
            transaction.finish();
        }
    }
}

#[cfg(test)]
mod test {
    use super::{native_event_ingress::NativeWindowEvent, window_registry};
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
        sync::Arc,
    };

    use crate::{
        AnyDrag, AnyWindowHandle, AppContext, Context, Empty, Entity, IntoElement, MouseButton,
        PointerCancelReason, QuitMode, Render, TestAppContext, Window, WindowOpenFailureStage,
        WindowOptions, point,
    };
    use crate::{px, size};

    actions!(
        app_focus_tests,
        [DispatchProbe, MenuProbe, UnknownMenuProbe]
    );

    #[test]
    fn test_gpui_borrow() {
        let cx = TestAppContext::single();
        let observation_count = Rc::new(RefCell::new(0));

        let state = cx.update(|cx| {
            let state = cx.new(|_| false);
            cx.observe(&state, {
                let observation_count = observation_count.clone();
                move |_, _| {
                    let mut count = observation_count.borrow_mut();
                    *count += 1;
                }
            })
            .detach();

            state
        });

        cx.update(|cx| {
            // Calling this like this so that we don't clobber the borrow_mut above
            *std::borrow::BorrowMut::borrow_mut(&mut state.as_mut(cx)) = true;
        });

        cx.update(|cx| {
            state.write(cx, false);
        });

        assert_eq!(*observation_count.borrow(), 2);
    }

    #[test]
    fn gpui_borrow_restores_app_transaction_when_observer_panics() {
        let cx = TestAppContext::single();
        let panic_on_observe = Rc::new(Cell::new(true));
        let successful_observations = Rc::new(Cell::new(0));
        let state = cx.update(|cx| {
            let state = cx.new(|_| false);
            cx.observe(&state, {
                let panic_on_observe = panic_on_observe.clone();
                let successful_observations = successful_observations.clone();
                move |_, _| {
                    assert!(
                        !panic_on_observe.get(),
                        "injected GPUI borrow observer panic"
                    );
                    successful_observations.set(successful_observations.get() + 1);
                }
            })
            .detach();
            state
        });

        let mut app = cx.app.borrow_mut();
        let result = catch_unwind(AssertUnwindSafe(|| {
            *std::borrow::BorrowMut::borrow_mut(&mut state.as_mut(&mut *app)) = true;
        }));

        assert!(result.is_err());
        assert_eq!(app.pending_updates, 0);
        assert!(!app.flushing_effects);
        panic_on_observe.set(false);
        *std::borrow::BorrowMut::borrow_mut(&mut state.as_mut(&mut *app)) = false;
        assert_eq!(
            successful_observations.get(),
            1,
            "the observer set must survive a callback panic"
        );
        assert_eq!(app.pending_updates, 0);
        assert!(!app.flushing_effects);
    }

    #[test]
    fn gpui_borrow_does_not_flush_observers_during_unwind() {
        let cx = TestAppContext::single();
        let observation_count = Rc::new(Cell::new(0));
        let state = cx.update(|cx| {
            let state = cx.new(|_| false);
            cx.observe(&state, {
                let observation_count = observation_count.clone();
                move |_, _| observation_count.set(observation_count.get() + 1)
            })
            .detach();
            state
        });

        let mut app = cx.app.borrow_mut();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut state = state.as_mut(&mut *app);
            *state = true;
            panic!("injected panic while holding a GPUI borrow");
        }));

        assert!(result.is_err());
        assert_eq!(observation_count.get(), 0);
        assert_eq!(app.pending_updates, 0);
        assert!(!app.flushing_effects);

        app.update(|_| {});
        assert_eq!(observation_count.get(), 1);
        assert_eq!(app.pending_updates, 0);
        assert!(!app.flushing_effects);
    }

    #[test]
    fn release_callback_panic_does_not_skip_remaining_entity_cleanup() {
        let cx = TestAppContext::single();
        let later_callback_count = Rc::new(Cell::new(0));
        let dependent_release_count = Rc::new(Cell::new(0));
        let first = cx.update(|app| {
            let first = app.new(|_| ());
            let dependent = app.new(|_| ());
            app.observe_release(&dependent, {
                let dependent_release_count = dependent_release_count.clone();
                move |_, _| dependent_release_count.set(dependent_release_count.get() + 1)
            })
            .detach();
            app.observe_release(&first, move |_, _| {
                drop(dependent);
                panic!("injected release callback panic");
            })
            .detach();
            app.observe_release(&first, {
                let later_callback_count = later_callback_count.clone();
                move |_, _| later_callback_count.set(later_callback_count.get() + 1)
            })
            .detach();
            first
        });

        drop(first);
        let result = catch_unwind(AssertUnwindSafe(|| cx.update(|_| {})));

        assert!(result.is_err());
        assert_eq!(
            later_callback_count.get(),
            1,
            "all callbacks for the panicking entity must reach a terminal outcome"
        );
        assert_eq!(
            dependent_release_count.get(),
            1,
            "entities dropped by a panicking callback must still be released"
        );
        cx.update(|_| {});
        assert_app_transaction_idle(&cx);
    }

    #[crate::test]
    fn entity_update_returns_lease_before_resuming_callback_panic(cx: &mut TestAppContext) {
        let state = cx.update(|app| app.new(|_| 0usize));
        let result = catch_unwind(AssertUnwindSafe(|| {
            state.update(cx, |state, _| {
                *state = 1;
                panic!("injected entity update panic");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            cx.read(|app| *state.read(app)),
            1,
            "the entity remains available with its last in-memory value"
        );
        state.update(cx, |state, _| *state = 2);
        assert_eq!(cx.read(|app| *state.read(app)), 2);
        assert_app_transaction_idle(cx);
    }

    fn assert_app_transaction_idle(cx: &TestAppContext) {
        cx.read(|app| {
            assert_eq!(app.pending_updates, 0);
            assert!(!app.flushing_effects);
            assert!(app.window_update_stack.is_empty());
        });
    }

    #[crate::test]
    fn app_update_restores_depth_after_callback_panics(cx: &mut TestAppContext) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                assert_eq!(app.pending_updates, 1);
                panic!("injected app update panic");
            })
        }));

        assert!(result.is_err());
        assert_app_transaction_idle(cx);
        cx.update(|app| assert_eq!(app.pending_updates, 1));
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn nested_updates_share_one_outer_update_generation(cx: &mut TestAppContext) {
        let first_generation = cx.update(|app| {
            let first_generation = app.current_update_generation();
            app.update(|app| {
                assert_eq!(app.current_update_generation(), first_generation);
            });
            first_generation
        });
        let second_generation = cx.update(|app| app.current_update_generation());

        assert_eq!(second_generation, first_generation + 1);
    }

    #[crate::test]
    fn open_window_rolls_back_exact_reservation_when_root_builder_panics(cx: &mut TestAppContext) {
        let reserved_id = Rc::new(Cell::new(None));
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                let _: anyhow::Result<crate::WindowHandle<Empty>> =
                    app.open_window(WindowOptions::default(), {
                        let reserved_id = reserved_id.clone();
                        move |window, _| -> Entity<Empty> {
                            reserved_id.set(Some(window.window_handle().window_id()));
                            panic!("injected root builder panic");
                        }
                    });
            })
        }));

        assert!(result.is_err());
        let reserved_id = reserved_id
            .get()
            .expect("the builder must observe its reserved window id");
        assert_app_transaction_idle(cx);
        cx.read(|app| {
            assert!(!app.windows.contains_key(reserved_id));
            assert!(!app.window_handles.contains_key(&reserved_id));
            assert!(!app.window_profiles.contains_key(&reserved_id));
        });
        assert!(
            cx.app.dispatch_window_should_close(reserved_id),
            "the rolled-back id must also be absent from native query snapshots"
        );

        let replacement = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
    }

    #[crate::test]
    fn reserved_window_rollback_cleans_entity_window_links(cx: &mut TestAppContext) {
        let anchor: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let marker = cx.update(|app| app.new(|_| ()));
        let marker_id = marker.entity_id();
        let reserved_id = cx.update(|app| {
            let invalidator = app
                .windows
                .get(anchor.window_id())
                .and_then(Option::as_deref)
                .expect("the anchor window must be committed")
                .invalidator
                .clone();
            let mut reservation =
                window_registry::reserve(app).expect("window reservation should be available");
            let reserved_id = reservation.id();
            let app = reservation.app_mut();
            app.tracked_entities
                .entry(reserved_id)
                .or_default()
                .insert(marker_id);
            app.current_window_by_entity.insert(marker_id, reserved_id);
            app.window_invalidators_by_entity
                .entry(marker_id)
                .or_default()
                .insert(reserved_id, invalidator);
            drop(reservation);
            reserved_id
        });

        cx.read(|app| {
            assert!(!app.windows.contains_key(reserved_id));
            assert!(!app.tracked_entities.contains_key(&reserved_id));
            assert_ne!(
                app.current_window_by_entity.get(&marker_id),
                Some(&reserved_id)
            );
            assert!(
                app.window_invalidators_by_entity
                    .get(&marker_id)
                    .is_none_or(|windows| !windows.contains_key(&reserved_id))
            );
        });
    }

    #[crate::test]
    fn builder_closed_reserved_window_is_not_committed(cx: &mut TestAppContext) {
        let reserved_id = Rc::new(Cell::new(None));
        let closed_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.set_quit_mode(QuitMode::LastWindowClosed);
            app.on_window_closed({
                let closed_count = closed_count.clone();
                move |_, _| closed_count.set(closed_count.get() + 1)
            })
            .detach();
        });

        let result: anyhow::Result<crate::WindowHandle<Empty>> = cx.update(|app| {
            app.open_window(WindowOptions::default(), {
                let reserved_id = reserved_id.clone();
                move |window, app| {
                    reserved_id.set(Some(window.window_handle().window_id()));
                    window.remove_window(app);
                    app.new(|_| Empty)
                }
            })
        });

        assert!(result.is_err());
        let reserved_id = reserved_id
            .get()
            .expect("the builder must observe its reserved window id");
        cx.read(|app| {
            assert!(!app.windows.contains_key(reserved_id));
            assert!(!app.window_handles.contains_key(&reserved_id));
            assert!(!app.window_profiles.contains_key(&reserved_id));
        });
        assert!(cx.app.dispatch_window_should_close(reserved_id));
        assert!(
            crate::WindowHandle::<Empty>::new(reserved_id)
                .update(cx, |_, _, _| ())
                .is_err()
        );
        assert_eq!(closed_count.get(), 0);
        assert!(!cx.did_quit());
        assert_app_transaction_idle(cx);
    }

    struct PanicOnInitialDraw;

    impl Render for PanicOnInitialDraw {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            if std::hint::black_box(true) {
                panic!("injected initial draw panic");
            }
            Empty
        }
    }

    struct CloseOnInitialDraw;

    impl Render for CloseOnInitialDraw {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            window.remove_window(cx);
            Empty
        }
    }

    struct NativeCloseOnInitialDraw;

    impl Render for NativeCloseOnInitialDraw {
        fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            assert!(
                window
                    .platform_window
                    .as_test()
                    .expect("the test must use TestPlatform")
                    .simulate_close(),
                "the native close callback must be installed before the initial draw"
            );
            Empty
        }
    }

    #[crate::test]
    fn detailed_window_open_reports_native_map_failure(cx: &mut TestAppContext) {
        cx.fail_next_window_map("injected map failure");
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| app.new(|_| Empty))
            })
            .expect_err("the injected native map failure must reject window creation");

        assert_eq!(error.stage(), WindowOpenFailureStage::NativeCreateOrMap);
        let platform_window = cx
            .last_created_test_window()
            .expect("the failed map must retain a test platform window for retirement");
        cx.run_until_parked();
        assert_eq!(
            platform_window.presentation_shutdown_counts(),
            (1, 1, 1),
            "a post-native map failure must use the exact retirement authority rather than direct backend Drop"
        );
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn detailed_window_open_reports_native_close_during_map(cx: &mut TestAppContext) {
        let root_builder_called = Rc::new(Cell::new(false));
        cx.close_next_window_during_map();
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| {
                    root_builder_called.set(true);
                    app.new(|_| Empty)
                })
            })
            .expect_err("a native close during map must reject window creation");

        assert_eq!(
            error.stage(),
            WindowOpenFailureStage::ClosedDuringNativeCreateOrMap
        );
        assert!(
            !root_builder_called.get(),
            "the root builder must not run after the native window has closed"
        );
        cx.run_until_parked();
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn detailed_window_open_reports_builder_close(cx: &mut TestAppContext) {
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |window, app| {
                    window.remove_window(app);
                    app.new(|_| Empty)
                })
            })
            .expect_err("a builder-closed reservation must not commit");

        assert_eq!(error.stage(), WindowOpenFailureStage::ClosedDuringBuild);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn detailed_window_open_reports_native_builder_close(cx: &mut TestAppContext) {
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |window, app| {
                    assert!(
                        window
                            .platform_window
                            .as_test()
                            .expect("the test must use TestPlatform")
                            .simulate_close(),
                        "the native close callback must be installed before the root builder"
                    );
                    app.new(|_| Empty)
                })
            })
            .expect_err("a native close during the root builder must reject the reservation");

        assert_eq!(error.stage(), WindowOpenFailureStage::ClosedDuringBuild);
        cx.run_until_parked();
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn detailed_window_open_reports_initial_draw_close(cx: &mut TestAppContext) {
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| {
                    app.new(|_| CloseOnInitialDraw)
                })
            })
            .expect_err("an initial-draw close must not commit");

        assert_eq!(
            error.stage(),
            WindowOpenFailureStage::ClosedDuringInitialDraw
        );
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn detailed_window_open_reports_native_initial_draw_close(cx: &mut TestAppContext) {
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| {
                    app.new(|_| NativeCloseOnInitialDraw)
                })
            })
            .expect_err("a native close during the initial draw must reject the reservation");

        assert_eq!(
            error.stage(),
            WindowOpenFailureStage::ClosedDuringInitialDraw
        );
        cx.run_until_parked();
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn detailed_window_open_reports_native_initial_presentation_close(cx: &mut TestAppContext) {
        cx.set_platform_window_creation_capabilities(crate::PlatformWindowCreationCapabilities {
            focus_on_appearing: crate::WindowCreationSupport::Supported,
            transient_for: crate::WindowCreationSupport::Supported,
            provisional_presentation: crate::WindowCreationSupport::Supported,
            initial_presentation_order: crate::WindowInitialPresentationOrder::BeforeVisibility,
        });
        cx.close_next_window_during_initial_presentation();

        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| app.new(|_| Empty))
            })
            .expect_err("a native close during hidden initial presentation must reject opening");

        assert_eq!(
            error.stage(),
            WindowOpenFailureStage::ClosedDuringInitialPresentation
        );
        cx.run_until_parked();
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn initial_draw_closed_reserved_window_is_not_committed(cx: &mut TestAppContext) {
        let reserved_id = Rc::new(Cell::new(None));
        let closed_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.set_quit_mode(QuitMode::LastWindowClosed);
            app.on_window_closed({
                let closed_count = closed_count.clone();
                move |_, _| closed_count.set(closed_count.get() + 1)
            })
            .detach();
        });

        let result: anyhow::Result<crate::WindowHandle<CloseOnInitialDraw>> = cx.update(|app| {
            app.open_window(WindowOptions::default(), {
                let reserved_id = reserved_id.clone();
                move |window, app| {
                    reserved_id.set(Some(window.window_handle().window_id()));
                    app.new(|_| CloseOnInitialDraw)
                }
            })
        });

        assert!(result.is_err());
        let reserved_id = reserved_id
            .get()
            .expect("the builder must observe its reserved window id");
        cx.read(|app| {
            assert!(!app.windows.contains_key(reserved_id));
            assert!(!app.window_handles.contains_key(&reserved_id));
            assert!(!app.window_profiles.contains_key(&reserved_id));
        });
        assert!(cx.app.dispatch_window_should_close(reserved_id));
        assert!(
            crate::WindowHandle::<CloseOnInitialDraw>::new(reserved_id)
                .update(cx, |_, _, _| ())
                .is_err()
        );
        assert_eq!(closed_count.get(), 0);
        assert!(!cx.did_quit());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn open_window_rolls_back_exact_reservation_when_initial_draw_panics(cx: &mut TestAppContext) {
        let reserved_id = Rc::new(Cell::new(None));
        let observed_creations = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.observe_new::<PanicOnInitialDraw>({
                let observed_creations = observed_creations.clone();
                move |_, _, _| observed_creations.set(observed_creations.get() + 1)
            })
            .detach();
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                let _: anyhow::Result<crate::WindowHandle<PanicOnInitialDraw>> =
                    app.open_window(WindowOptions::default(), {
                        let reserved_id = reserved_id.clone();
                        move |window, app| -> Entity<PanicOnInitialDraw> {
                            reserved_id.set(Some(window.window_handle().window_id()));
                            app.new(|_| PanicOnInitialDraw)
                        }
                    });
            })
        }));

        assert!(result.is_err());
        let reserved_id = reserved_id
            .get()
            .expect("the builder must observe its reserved window id");
        assert_app_transaction_idle(cx);
        cx.read(|app| {
            assert!(!app.windows.contains_key(reserved_id));
            assert!(!app.window_handles.contains_key(&reserved_id));
            assert!(!app.window_profiles.contains_key(&reserved_id));
        });
        assert!(
            cx.app.dispatch_window_should_close(reserved_id),
            "the rolled-back id must also be absent from native query snapshots"
        );
        cx.update(|_| {});
        assert_eq!(
            observed_creations.get(),
            0,
            "a root entity from a rolled-back window must not emit a creation effect"
        );
    }

    #[crate::test]
    fn window_update_panics_restore_exact_parent_stack_and_registry(cx: &mut TestAppContext) {
        let first: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let second: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();

        let result = catch_unwind(AssertUnwindSafe(|| {
            first
                .update(cx, |_, _, app| -> () {
                    assert_eq!(app.window_update_stack, [first.window_id()]);
                    let nested = catch_unwind(AssertUnwindSafe(|| {
                        app.update_window_id(second.window_id(), |_, _, _| -> () {
                            panic!("injected nested window update panic")
                        })
                    }));
                    assert!(nested.is_err());
                    assert_eq!(
                        app.window_update_stack,
                        [first.window_id()],
                        "the nested transaction must restore the exact parent stack"
                    );
                    panic!("injected outer window update panic");
                })
                .expect("the first window must exist before the injected panic");
        }));

        assert!(result.is_err());
        assert_app_transaction_idle(cx);
        cx.read(|app| {
            assert!(
                app.windows
                    .get(first.window_id())
                    .is_some_and(Option::is_some)
            );
            assert!(
                app.windows
                    .get(second.window_id())
                    .is_some_and(Option::is_some)
            );
        });
        first
            .update(cx, |_, _, _| ())
            .expect("the first window must remain updateable");
        second
            .update(cx, |_, _, _| ())
            .expect("the second window must remain updateable");
    }

    #[crate::test]
    fn window_removed_before_update_panic_is_not_restored(cx: &mut TestAppContext) {
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let closed_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.on_window_closed({
                let closed_count = closed_count.clone();
                move |_, closed_window| {
                    assert_eq!(closed_window, window.window_id());
                    closed_count.set(closed_count.get() + 1);
                }
            })
            .detach();
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            window
                .update(cx, |_, window, app| -> () {
                    window.remove_window(app);
                    panic!("injected panic after removing a window");
                })
                .expect("the window must exist before removal");
        }));

        assert!(result.is_err());
        assert_eq!(closed_count.get(), 1);
        assert_app_transaction_idle(cx);
        cx.read(|app| {
            assert!(!app.windows.contains_key(window.window_id()));
            assert!(!app.window_handles.contains_key(&window.window_id()));
            assert!(!app.window_profiles.contains_key(&window.window_id()));
        });
        assert!(
            window.update(cx, |_, _, _| ()).is_err(),
            "a removed window must not be resurrected after the callback panic"
        );
    }

    #[crate::test]
    fn window_close_observer_panic_does_not_skip_fanout_or_last_window_quit(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::LastWindowClosed));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let later_observer_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.on_window_closed({
                let expected = window.window_id();
                move |_, closed_window| {
                    assert_eq!(closed_window, expected);
                    panic!("injected first close observer panic");
                }
            })
            .detach();
            app.on_window_closed({
                let later_observer_count = later_observer_count.clone();
                move |_, closed_window| {
                    assert_eq!(closed_window, window.window_id());
                    later_observer_count.set(later_observer_count.get() + 1);
                }
            })
            .detach();
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            window
                .update(cx, |_, window, app| window.remove_window(app))
                .expect("the window must exist before removal");
        }));

        assert!(result.is_err());
        assert_eq!(
            later_observer_count.get(),
            1,
            "a panicking observer must not skip later close observers"
        );
        assert!(cx.windows().is_empty());
        cx.run_until_parked();
        assert!(
            cx.did_quit(),
            "LastWindowClosed must request platform quit after terminal shutdown"
        );
        assert!(cx.update(|app| app.native_exit_authority_is_settled_for_test()));
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| app.new(|_| Empty))
            })
            .expect_err("terminal LastWindowClosed shutdown must reject a replacement window");
        assert_eq!(error.stage(), WindowOpenFailureStage::AppShutdown);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn nested_window_close_observations_are_delivered_in_fifo_order(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::LastWindowClosed));
        let first: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let second: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let first_id = first.window_id();
        let second_id = second.window_id();
        let observations = Rc::new(RefCell::new(Vec::new()));

        cx.update(|app| {
            app.on_window_closed({
                let observations = observations.clone();
                move |app, closed_window| {
                    observations.borrow_mut().push((1, closed_window));
                    if closed_window == first_id {
                        second
                            .update(app, |_, window, app| window.remove_window(app))
                            .expect("the nested window must remain open until the first callback");
                    }
                }
            })
            .detach();
            app.on_window_closed({
                let observations = observations.clone();
                move |_, closed_window| observations.borrow_mut().push((2, closed_window))
            })
            .detach();
        });

        first
            .update(cx, |_, window, app| window.remove_window(app))
            .expect("the first window must exist before removal");

        assert_eq!(
            observations.borrow().as_slice(),
            &[(1, first_id), (2, first_id), (1, second_id), (2, second_id)]
        );
        assert!(cx.did_quit());
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn logical_window_close_precedes_native_terminal(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let window_id = window.window_id();
        let observations = Rc::new(RefCell::new(Vec::new()));

        cx.update(|app| {
            app.on_window_closed({
                let observations = observations.clone();
                move |_, closed_window| observations.borrow_mut().push(("logical", closed_window))
            })
            .detach();
            app.on_window_native_terminal({
                let observations = observations.clone();
                move |_, terminal_window| {
                    observations
                        .borrow_mut()
                        .push(("terminal", terminal_window))
                }
            })
            .detach();
        });

        cx.update(|app| {
            window
                .update(app, |_, window, app| window.remove_window(app))
                .expect("the window must exist before logical removal");
            assert_eq!(
                observations.borrow().as_slice(),
                &[("logical", window_id)],
                "dropping GPUI's platform-window owner is not a native terminal event"
            );
        });
        cx.run_until_parked();
        assert_eq!(
            observations.borrow().as_slice(),
            &[("logical", window_id), ("terminal", window_id)],
            "the delayed native Closed event must publish the terminal notification"
        );
    }

    #[crate::test]
    fn native_terminal_can_be_held_after_logical_window_removal(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let window_id = window.window_id();
        let terminal_windows = Rc::new(RefCell::new(Vec::new()));
        cx.update(|app| {
            app.on_window_native_terminal({
                let terminal_windows = terminal_windows.clone();
                move |_, terminal_window| terminal_windows.borrow_mut().push(terminal_window)
            })
            .detach();
        });
        let terminal_hold = cx.hold_window_native_terminal(window);

        window
            .update(cx, |_, window, app| window.remove_window(app))
            .expect("the window must exist before logical removal");
        cx.run_until_parked();

        assert!(cx.windows().is_empty());
        assert!(terminal_windows.borrow().is_empty());
        assert!(terminal_hold.release());
        cx.run_until_parked();
        assert_eq!(terminal_windows.borrow().as_slice(), &[window_id]);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn dropping_native_terminal_hold_before_close_restores_normal_delivery(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let terminal_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.on_window_native_terminal({
                let terminal_count = terminal_count.clone();
                move |_, _| terminal_count.set(terminal_count.get() + 1)
            })
            .detach();
        });

        drop(cx.hold_window_native_terminal(window));
        window
            .update(cx, |_, window, app| window.remove_window(app))
            .expect("the window must exist before logical removal");
        cx.run_until_parked();

        assert_eq!(terminal_count.get(), 1);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn stale_native_closed_event_still_notifies(cx: &mut TestAppContext) {
        let stale_window = cx.update(|app| {
            let reservation =
                window_registry::reserve(app).expect("window reservation should be available");
            let window_id = reservation.id();
            drop(reservation);
            window_id
        });
        let observations = Rc::new(RefCell::new(Vec::new()));
        cx.update(|app| {
            app.on_window_native_terminal({
                let observations = observations.clone();
                move |_, window_id| observations.borrow_mut().push(window_id)
            })
            .detach();
        });

        cx.app
            .enqueue_native_window_event(stale_window, NativeWindowEvent::Closed);
        cx.run_until_parked();

        assert_eq!(observations.borrow().as_slice(), &[stale_window]);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn terminating_shutdown_drains_closed_event_queued_behind_completion(cx: &mut TestAppContext) {
        let stale_window = cx.update(|app| {
            let reservation =
                window_registry::reserve(app).expect("window reservation should be available");
            let window_id = reservation.id();
            drop(reservation);
            window_id
        });
        let observations = Rc::new(RefCell::new(Vec::new()));
        cx.update(|app| {
            app.on_window_native_terminal({
                let observations = observations.clone();
                move |_, window_id| observations.borrow_mut().push(window_id)
            })
            .detach();
        });

        let mut app = cx.app.borrow_mut();
        app.shutdown_from_native_quit();
        cx.app
            .enqueue_native_window_event(stale_window, NativeWindowEvent::Closed);
        drop(app);
        cx.run_until_parked();

        assert_eq!(
            observations.borrow().as_slice(),
            &[stale_window],
            "terminal shutdown must deliver exact native lifecycle work queued behind its completion"
        );
        assert!(cx.app.native_exit_authority_is_settled_for_test());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn native_terminal_survives_logical_close_observer_panic(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let window_id = window.window_id();
        let terminal_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            app.on_window_closed(|_, _| panic!("injected logical close observer panic"))
                .detach();
            app.on_window_native_terminal({
                let terminal_count = terminal_count.clone();
                move |_, terminal_window| {
                    assert_eq!(terminal_window, window_id);
                    terminal_count.set(terminal_count.get() + 1);
                }
            })
            .detach();
        });
        let platform_window = cx.test_window(window);

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert!(platform_window.simulate_close());
            cx.run_until_parked();
        }));

        assert!(result.is_err());
        assert!(cx.windows().is_empty());
        assert_eq!(terminal_count.get(), 1);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn nested_native_terminal_observations_are_delivered_in_fifo_order(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let first: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let second: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let first_id = first.window_id();
        let second_id = second.window_id();
        let observations = Rc::new(RefCell::new(Vec::new()));

        cx.update(|app| {
            app.on_window_native_terminal({
                let observations = observations.clone();
                move |app, terminal_window| {
                    observations.borrow_mut().push((1, terminal_window));
                    if terminal_window == first_id {
                        second
                            .update(app, |_, window, app| window.remove_window(app))
                            .expect("the nested window must remain open until terminal fanout");
                    }
                }
            })
            .detach();
            app.on_window_native_terminal({
                let observations = observations.clone();
                move |_, terminal_window| observations.borrow_mut().push((2, terminal_window))
            })
            .detach();
        });

        cx.update(|app| {
            first
                .update(app, |_, window, app| window.remove_window(app))
                .expect("the first window must exist before removal");
            assert!(observations.borrow().is_empty());
        });
        cx.run_until_parked();

        assert_eq!(
            observations.borrow().as_slice(),
            &[(1, first_id), (2, first_id), (1, second_id), (2, second_id)]
        );
        assert_eq!(observations.borrow().len(), 4);
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn shutdown_barrier_rejects_immediate_and_deferred_window_opens(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let failures = Rc::new(RefCell::new(Vec::new()));
        cx.update(|app| {
            app.on_app_quit({
                let failures = failures.clone();
                move |app| {
                    let immediate = app
                        .open_window_detailed(WindowOptions::default(), |_, app| app.new(|_| Empty))
                        .expect_err("shutdown must reject a reentrant window open")
                        .stage();
                    failures.borrow_mut().push(immediate);

                    app.defer({
                        let failures = failures.clone();
                        move |app| {
                            let deferred = app
                                .open_window_detailed(WindowOptions::default(), |_, app| {
                                    app.new(|_| Empty)
                                })
                                .expect_err("shutdown must reject a deferred window open")
                                .stage();
                            failures.borrow_mut().push(deferred);
                        }
                    });
                    async {}
                }
            })
            .detach();
        });

        cx.quit();

        assert_eq!(
            failures.borrow().as_slice(),
            &[
                WindowOpenFailureStage::AppShutdown,
                WindowOpenFailureStage::AppShutdown
            ]
        );
        assert!(cx.windows().is_empty());
        let replacement = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn shutdown_observer_can_claim_the_active_captured_drag_before_the_generic_fallback(
        cx: &mut TestAppContext,
    ) {
        let source: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let generation = cx
            .update_window(source, |_, _, app| {
                let reservation = app.reserve_native_captured_drag_start();
                let generation = reservation.token().generation();
                let drag_view = app.new(|_| Empty).into();
                assert!(app.start_reserved_active_drag(
                    reservation,
                    AnyDrag {
                        window_id: source.window_id(),
                        source: None,
                        value: Arc::new("shutdown-observer-g1"),
                        view: drag_view,
                        window_preview_offset: point(px(0.0), px(0.0)),
                        cursor_style: None,
                        button: MouseButton::Left,
                    },
                ));
                generation
            })
            .expect("the source must start its captured drag");
        let observer_saw_active = Rc::new(Cell::new(false));
        let terminals = Rc::new(RefCell::new(Vec::new()));
        cx.update({
            let observer_saw_active = observer_saw_active.clone();
            let terminals = terminals.clone();
            move |app| {
                app.on_app_quit(move |app| {
                    let terminals = terminals.clone();
                    observer_saw_active.set(app.active_drag.is_some());
                    assert!(
                        app.cancel_native_captured_drag_with_release_barrier(
                            source.window_id(),
                            generation,
                            PointerCancelReason::WindowClosed,
                            move |barrier, terminal, _| {
                                terminals.borrow_mut().push((barrier, terminal));
                            },
                        )
                        .is_some()
                    );
                    std::future::ready(())
                })
                .detach();
            }
        });

        cx.quit();
        cx.run_until_parked();

        assert!(observer_saw_active.get());
        assert_eq!(terminals.borrow().len(), 1);
        assert!(cx.windows().is_empty());
        let replacement = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn panicking_quit_observer_does_not_skip_sibling_or_deferred_cleanup(cx: &mut TestAppContext) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let lifecycle = Rc::new(RefCell::new(Vec::new()));
        cx.update(|app| {
            app.on_app_quit({
                let lifecycle = lifecycle.clone();
                move |_| -> std::future::Ready<()> {
                    lifecycle.borrow_mut().push("panicking-observer");
                    panic!("injected quit observer panic");
                }
            })
            .detach();
            app.on_app_quit({
                let lifecycle = lifecycle.clone();
                move |app| {
                    lifecycle.borrow_mut().push("sibling-observer");
                    app.defer({
                        let lifecycle = lifecycle.clone();
                        move |_| lifecycle.borrow_mut().push("deferred-cleanup")
                    });
                    std::future::ready(())
                }
            })
            .detach();
        });

        let result = catch_unwind(AssertUnwindSafe(|| cx.quit()));

        assert!(result.is_err());
        assert_eq!(
            lifecycle.borrow().as_slice(),
            &["panicking-observer", "sibling-observer", "deferred-cleanup"]
        );
        assert!(cx.windows().is_empty());
        let replacement = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn panicking_quit_future_does_not_skip_siblings_and_preserves_observer_order(
        cx: &mut TestAppContext,
    ) {
        let _: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let lifecycle = Rc::new(RefCell::new(Vec::new()));
        cx.update(|app| {
            app.on_app_quit({
                let lifecycle = lifecycle.clone();
                move |_| {
                    lifecycle.borrow_mut().push("first-created");
                    let lifecycle = lifecycle.clone();
                    async move {
                        lifecycle.borrow_mut().push("first-polled");
                        panic!("injected quit future panic");
                    }
                }
            })
            .detach();
            app.on_app_quit({
                let lifecycle = lifecycle.clone();
                move |_| {
                    lifecycle.borrow_mut().push("second-created");
                    let lifecycle = lifecycle.clone();
                    async move {
                        lifecycle.borrow_mut().push("second-polled");
                    }
                }
            })
            .detach();
        });

        let result = catch_unwind(AssertUnwindSafe(|| cx.quit()));

        assert!(result.is_err());
        assert_eq!(
            lifecycle.borrow().as_slice(),
            &[
                "first-created",
                "second-created",
                "first-polled",
                "second-polled",
            ]
        );
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn shutdown_inside_root_builder_returns_typed_error_without_panicking(cx: &mut TestAppContext) {
        let error = cx
            .update(|app| {
                app.open_window_detailed(WindowOptions::default(), |_, app| {
                    let root = app.new(|_| Empty);
                    app.shutdown();
                    root
                })
            })
            .expect_err("shutdown must invalidate the in-flight window reservation");

        assert_eq!(error.stage(), WindowOpenFailureStage::AppShutdown);
        cx.run_until_parked();
        assert!(cx.windows().is_empty());
        let replacement = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        assert!(replacement.update(cx, |_, _, _| ()).is_ok());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn window_update_preserves_callback_panic_after_close_observer_panic(cx: &mut TestAppContext) {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        cx.update(|app| {
            app.on_window_closed(|_, _| panic!("injected close observer panic"))
                .detach();
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            window
                .update(cx, |_, window, app| -> () {
                    window.remove_window(app);
                    panic!("injected original window update panic");
                })
                .expect("the window must exist before removal");
        }));

        let payload = result.expect_err("the window callback must panic");
        let message = payload
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
        assert_eq!(message, Some("injected original window update panic"));
        assert!(cx.windows().is_empty());
        assert_app_transaction_idle(cx);
    }

    #[crate::test]
    fn application_on_system_wake_runs_callback(cx: &mut TestAppContext) {
        let wake_count = Rc::new(Cell::new(0));
        super::Application(cx.app.clone()).on_system_wake({
            let wake_count = wake_count.clone();
            move |_| wake_count.set(wake_count.get() + 1)
        });

        cx.simulate_system_wake();
        cx.simulate_system_wake();

        assert_eq!(wake_count.get(), 2);
    }

    #[crate::test]
    fn native_application_callbacks_wait_for_app_borrow_and_preserve_fifo(cx: &mut TestAppContext) {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let application = super::Application(cx.app.clone());
        application.on_open_urls({
            let observed = observed.clone();
            move |urls, _| {
                observed
                    .borrow_mut()
                    .push(format!("open:{}", urls.join(",")))
            }
        });
        application.on_reopen({
            let observed = observed.clone();
            move |_| observed.borrow_mut().push("reopen".to_string())
        });
        application.on_system_wake({
            let observed = observed.clone();
            move |_| observed.borrow_mut().push("wake".to_string())
        });

        let simulator = cx.clone();
        cx.update(|_| {
            simulator.simulate_open_urls(["one", "two"]);
            simulator.simulate_reopen();
            simulator.simulate_will_open_app_menu();
            simulator.simulate_system_wake();
            assert!(
                observed.borrow().is_empty(),
                "native application callbacks must not re-enter a borrowed App"
            );
        });

        assert_eq!(
            *observed.borrow(),
            ["open:one,two", "reopen", "wake"],
            "native application callbacks must retain callback-entry order"
        );
    }

    #[crate::test]
    fn replacing_native_application_handler_during_delivery_retires_old_generation(
        cx: &mut TestAppContext,
    ) {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let app = Rc::downgrade(&cx.app);
        super::Application(cx.app.clone()).on_system_wake({
            let observed = observed.clone();
            move |_| {
                observed.borrow_mut().push("old");
                let app = app
                    .upgrade()
                    .expect("test application must outlive its native handler");
                super::Application(app).on_system_wake({
                    let observed = observed.clone();
                    move |_| observed.borrow_mut().push("new")
                });
            }
        });

        cx.simulate_system_wake();
        cx.simulate_system_wake();

        assert_eq!(*observed.borrow(), ["old", "new"]);
    }

    #[crate::test]
    fn app_menu_action_is_deferred_and_validation_uses_committed_snapshot_when_busy(
        cx: &mut TestAppContext,
    ) {
        let dispatch_count = Rc::new(Cell::new(0));
        cx.update(|app| {
            let dispatch_count = dispatch_count.clone();
            app.on_action(move |_: &MenuProbe, _| {
                dispatch_count.set(dispatch_count.get() + 1);
            });
        });

        assert!(
            cx.simulate_validate_app_menu_command(&MenuProbe),
            "idle validation must commit the exact action availability"
        );

        let simulator = cx.clone();
        cx.update(|_| {
            simulator.simulate_app_menu_action(&MenuProbe);
            assert_eq!(
                dispatch_count.get(),
                0,
                "menu action must wait until the outer App borrow is released"
            );
        });
        assert_eq!(
            dispatch_count.get(),
            1,
            "menu action must dispatch after the outer App borrow is released"
        );

        cx.update(|app| {
            app.global_action_listeners.clear();
            assert!(
                simulator.simulate_validate_app_menu_command(&MenuProbe),
                "busy validation must use the last committed result"
            );
            assert!(
                !simulator.simulate_validate_app_menu_command(&UnknownMenuProbe),
                "an unknown busy validation must conservatively return false"
            );
        });

        assert!(
            !cx.simulate_validate_app_menu_command(&MenuProbe),
            "the next idle validation must refresh the committed result"
        );
    }

    #[crate::test]
    fn quit_barrier_discards_later_native_application_callbacks(cx: &mut TestAppContext) {
        let wake_count = Rc::new(Cell::new(0));
        super::Application(cx.app.clone()).on_system_wake({
            let wake_count = wake_count.clone();
            move |_| wake_count.set(wake_count.get() + 1)
        });

        let app = cx.app.clone();
        let simulator = cx.clone();
        cx.update(|_| {
            app.enqueue_quit_for_test();
            simulator.simulate_system_wake();
        });

        assert_eq!(wake_count.get(), 0);
    }

    struct WindowTrackingView {
        marker: Entity<usize>,
    }

    impl Render for WindowTrackingView {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let _ = self.marker.read(cx);
            Empty
        }
    }

    #[crate::test]
    fn closing_window_cleans_window_registry_and_entity_links(cx: &mut TestAppContext) {
        let marker = cx.update(|cx| cx.new(|_| 0usize));
        let marker_id = marker.entity_id();
        let window: AnyWindowHandle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| WindowTrackingView {
                marker: marker.clone(),
            })
            .into();

        cx.update(|app| {
            assert!(app.window_handles.contains_key(&window.id));
            assert!(app.windows.contains_key(window.id));
            assert!(
                app.tracked_entities
                    .get(&window.id)
                    .is_some_and(|tracked| tracked.contains(&marker_id))
            );
            assert_eq!(
                app.current_window_by_entity.get(&marker_id),
                Some(&window.id)
            );
            assert!(
                app.window_invalidators_by_entity
                    .get(&marker_id)
                    .is_some_and(|windows| windows.contains_key(&window.id))
            );
        });

        assert!(cx.simulate_window_close(window));

        cx.update(|app| {
            assert!(!app.window_handles.contains_key(&window.id));
            assert!(!app.windows.contains_key(window.id));
            assert!(!app.tracked_entities.contains_key(&window.id));
            assert!(!app.current_window_by_entity.contains_key(&marker_id));
            assert!(
                app.window_invalidators_by_entity
                    .get(&marker_id)
                    .is_none_or(|windows| !windows.contains_key(&window.id))
            );
        });
    }

    #[crate::test]
    fn app_dispatch_action_uses_focused_window_authority(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        window
            .update(cx, |_, window, _| window.activate_window())
            .expect("test window should be activatable");
        cx.run_until_parked();
        assert_eq!(
            cx.update(|app| app.active_window()),
            Some(window.into()),
            "setup should leave an active application window"
        );

        let global_dispatch_count = Rc::new(RefCell::new(0));
        cx.update(|app| {
            let global_dispatch_count = global_dispatch_count.clone();
            app.on_action(move |_: &DispatchProbe, _| {
                *global_dispatch_count.borrow_mut() += 1;
            });
        });

        cx.set_platform_focused_window_available(false);
        cx.update(|app| app.dispatch_action(&DispatchProbe));

        assert_eq!(
            *global_dispatch_count.borrow(),
            1,
            "when backend focus authority is unavailable, app dispatch must not target active_window"
        );
    }
}
