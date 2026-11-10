//! Visual test platform combining a supplied rendering backend with controllable TestDispatcher.
//!
//! This platform is used for visual tests that need:
//! - Framework frame capture through the supplied renderer
//! - Deterministic task scheduling via TestDispatcher
//! - Controllable time via `advance_clock`

use crate::{ActivityGuard, ScreenCaptureSource};
use crate::{
    AnyWindowHandle, BackgroundExecutor, ClipboardItem, DevicePixels, ForegroundExecutor, Keymap,
    Menu, MenuItem, MouseButton, NativeAppNotificationHandler, OwnedMenu, PathPromptOptions,
    Platform, PlatformDisplay, PlatformDisplaySnapshot, PlatformHoveredWindow,
    PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformViewportCapabilities, PlatformWindow, PlatformWindowCapabilities,
    PlatformWindowHitStack, Point, Task, TestDispatcher, WindowAppearance, WindowParams,
};
use anyhow::Result;
use futures::channel::oneshot;
use parking_lot::Mutex;

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

/// A platform combining a supplied native rendering backend with controllable TestDispatcher.
///
/// This allows visual tests to:
/// - Render UI through the supplied backend for framework frame capture
/// - Control task scheduling deterministically via TestDispatcher
/// - Advance simulated time for testing time-based behaviors (tooltips, animations, etc.)
///
/// Its run method invokes the launch callback and returns. A test host must drive its
/// dispatcher and the supplied backend's native event pump through window retirement. It
/// does not invoke the backend's blocking run method or replace a production platform loop.
pub struct VisualTestPlatform {
    dispatcher: TestDispatcher,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    platform: Rc<dyn Platform>,
    clipboard: Mutex<Option<ClipboardItem>>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    primary_clipboard: Mutex<Option<ClipboardItem>>,
    #[cfg(target_os = "macos")]
    find_pasteboard: Mutex<Option<ClipboardItem>>,
}

impl VisualTestPlatform {
    /// Creates a new VisualTestPlatform with the given random seed.
    ///
    /// The seed is used for deterministic random number generation in the TestDispatcher.
    pub fn new(platform: Rc<dyn Platform>, seed: u64) -> Self {
        let dispatcher = TestDispatcher::new(seed);
        let arc_dispatcher = Arc::new(dispatcher.clone());

        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);

        Self {
            dispatcher,
            background_executor,
            foreground_executor,
            platform,
            clipboard: Mutex::new(None),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            primary_clipboard: Mutex::new(None),
            #[cfg(target_os = "macos")]
            find_pasteboard: Mutex::new(None),
        }
    }

    /// Returns a reference to the TestDispatcher for controlling task scheduling and time.
    pub fn dispatcher(&self) -> &TestDispatcher {
        &self.dispatcher
    }
}

impl Platform for VisualTestPlatform {
    fn platform_info(&self) -> crate::PlatformInfo {
        self.platform.platform_info()
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.platform.text_system()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
    }

    fn quit(&self) {}

    fn restart(&self, _binary_path: Option<PathBuf>, _arguments: Vec<OsString>) {}

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.platform.displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.platform.primary_display()
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.platform.on_displays_changed(callback);
    }

    fn display_snapshot(&self) -> PlatformDisplaySnapshot {
        self.platform.display_snapshot()
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.platform.active_window()
    }

    fn focused_window(&self) -> crate::platform::PlatformFocusedWindow {
        self.platform.focused_window()
    }

    fn hovered_window(&self) -> PlatformHoveredWindow {
        self.platform.hovered_window()
    }

    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.platform.window_stack()
    }

    fn window_hit_stack_at(&self, point: Point<DevicePixels>) -> PlatformWindowHitStack {
        self.platform.window_hit_stack_at(point)
    }

    fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        self.platform.viewport_capabilities()
    }

    fn window_capabilities(
        &self,
        kind: &crate::WindowKind,
        display_id: Option<crate::DisplayId>,
    ) -> PlatformWindowCapabilities {
        self.platform.window_capabilities(kind, display_id)
    }

    fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        self.platform.mouse_button_is_pressed(button)
    }

    fn is_screen_capture_supported(&self) -> bool {
        self.platform.is_screen_capture_supported()
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.platform.screen_capture_sources()
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        self.platform.open_window(handle, options)
    }

    fn open_window_with_display_snapshot(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
        display_snapshot: PlatformDisplaySnapshot,
    ) -> Result<Box<dyn PlatformWindow>> {
        self.platform
            .open_window_with_display_snapshot(handle, options, display_snapshot)
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.platform.window_appearance()
    }

    fn open_url(&self, url: &str) {
        self.platform.open_url(url)
    }

    fn on_open_urls(&self, _callback: Box<dyn FnMut(Vec<String>)>) {}

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, path: &Path) {
        self.platform.reveal_path(path)
    }

    fn open_with_system(&self, path: &Path) {
        self.platform.open_with_system(path)
    }

    fn on_quit(&self, _callback: Box<dyn FnMut() -> bool>) {}

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {}

    fn on_system_wake(&self, _callback: NativeAppNotificationHandler) {}

    fn on_system_sleep(&self, _callback: NativeAppNotificationHandler) {}

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {}

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        None
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {}

    fn on_app_menu_action(&self, _callback: Box<dyn FnMut(&dyn crate::Action)>) {}

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {}

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn crate::Action) -> bool>) {}

    fn app_path(&self) -> Result<PathBuf> {
        self.platform.app_path()
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.platform.path_for_auxiliary_executable(name)
    }

    fn hide_cursor_until_mouse_moves(&self) {
        self.platform.hide_cursor_until_mouse_moves();
    }

    fn is_cursor_visible(&self) -> bool {
        self.platform.is_cursor_visible()
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        self.platform.should_auto_hide_scrollbars()
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.clipboard.lock().clone()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        *self.clipboard.lock() = Some(item);
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.primary_clipboard.lock().clone()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        *self.primary_clipboard.lock() = Some(item);
    }

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.find_pasteboard.lock().clone()
    }

    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        *self.find_pasteboard.lock() = Some(item);
    }

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.platform.keyboard_layout()
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.platform.keyboard_mapper()
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {}

    fn thermal_state(&self) -> super::ThermalState {
        super::ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, _callback: NativeAppNotificationHandler) {}

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Err(anyhow::anyhow!(
            "Idle sleep prevention for {reason:?} is not supported in visual tests"
        )))
    }
}

#[cfg(test)]
mod platform_info_tests {
    use super::*;
    use crate::{OperatingSystem, PlatformInfo, PlatformTarget, TestPlatform};

    #[test]
    fn wrapper_delegates_live_backend_identity() {
        let dispatcher = Arc::new(TestDispatcher::new(0));
        let backend = TestPlatform::new(
            BackgroundExecutor::new(dispatcher.clone()),
            ForegroundExecutor::new(dispatcher),
        );
        let visual = VisualTestPlatform::new(backend.clone(), 1);

        for os in [
            OperatingSystem::Mac,
            OperatingSystem::Windows,
            OperatingSystem::Linux,
            OperatingSystem::FreeBsd,
            OperatingSystem::Ios,
            OperatingSystem::Android,
            OperatingSystem::Unknown,
        ] {
            for target in [PlatformTarget::Native, PlatformTarget::Web] {
                let identity = PlatformInfo { os, target };
                backend.platform_info.set(identity);
                assert_eq!(visual.platform_info(), identity);
            }
        }
    }
}
