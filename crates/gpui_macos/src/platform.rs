use crate::{
    BoolExt, MacDispatcher, MacDisplayTopologyHandle, MacDisplayTopologyRefresh,
    MacDisplayTopologyRetry, MacKeyboardLayout, MacKeyboardMapper, MacWindow, MacWindowRegistry,
    activity::AppleActivity,
    events::key_to_native,
    keychain, ns_string,
    pasteboard::Pasteboard,
    process_info::{self, ProcessInfoObserver},
    renderer,
    window::{macos_supports_focus_on_appearing, macos_supports_toplevel_creation_state},
};
use anyhow::{Context as _, anyhow};
use block2::RcBlock;
use cocoa::{
    appkit::{
        NSApplication, NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        NSControl as _, NSEventModifierFlags, NSMenu, NSMenuItem, NSModalResponse, NSOpenPanel,
        NSSavePanel, NSVisualEffectState, NSVisualEffectView, NSWindow,
    },
    base::{BOOL, NO, YES, id, nil, selector},
    foundation::{
        NSArray, NSAutoreleasePool, NSBundle, NSInteger, NSProcessInfo, NSString, NSUInteger, NSURL,
    },
};
use core_foundation::{base::CFRelease, runloop::CFRunLoopRun, string::CFStringRef};
use ctor::ctor;
use dispatch2::{DispatchQueue, DispatchTime};
use futures::channel::oneshot;
use itertools::Itertools;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use objc2_foundation::NSActivityOptions;
use open_gpui::{
    Action, ActivityGuard, AnyWindowHandle, BackgroundExecutor, ClipboardItem, DisplayId,
    ForegroundExecutor, KeyContext, Keymap, Menu, MenuItem, MouseButton,
    NativeAppNotificationHandler, NavigationDirection, OsMenu, OwnedMenu, PathPromptOptions,
    Platform, PlatformDisplay, PlatformDisplaySnapshot, PlatformFocusedWindow,
    PlatformHoveredWindow, PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformViewportCapabilities, PlatformWindow, PlatformWindowActivationSupport,
    PlatformWindowCapabilities, PlatformWindowCreationCapabilities,
    PlatformWindowMutationCapabilities, PowerMode, Result, SystemMenuType, Task, ThermalState,
    WindowAppearance, WindowCoordinateSpace, WindowCreationSupport, WindowInitialPresentationOrder,
    WindowKind, WindowMutationSupport, WindowParams,
};
use open_gpui_util::{
    ResultExt,
    command::{new_command, new_std_command},
};
use parking_lot::Mutex;
use ptr::null_mut;
use semver::Version;
use std::{
    cell::Cell,
    ffi::{CStr, OsStr, c_void},
    os::{raw::c_char, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
    ptr,
    rc::Rc,
    slice, str,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[allow(non_upper_case_globals)]
const NSUTF8StringEncoding: NSUInteger = 4;

const MAC_PLATFORM_IVAR: &str = "platform";
static mut APP_CLASS: *const Class = ptr::null();
static mut APP_DELEGATE_CLASS: *const Class = ptr::null();

#[ctor(unsafe)]
unsafe fn build_classes() {
    unsafe {
        APP_CLASS = {
            let mut decl = ClassDecl::new("GPUIApplication", class!(NSApplication)).unwrap();
            decl.add_ivar::<*mut c_void>(MAC_PLATFORM_IVAR);
            decl.register()
        }
    };
    unsafe {
        APP_DELEGATE_CLASS = {
            let mut decl = ClassDecl::new("GPUIApplicationDelegate", class!(NSResponder)).unwrap();
            decl.add_ivar::<*mut c_void>(MAC_PLATFORM_IVAR);
            decl.add_method(
                sel!(applicationWillFinishLaunching:),
                will_finish_launching as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationDidFinishLaunching:),
                did_finish_launching as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                should_handle_reopen as extern "C" fn(&mut Object, Sel, id, bool),
            );
            decl.add_method(
                sel!(applicationWillTerminate:),
                will_terminate as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(handleGPUIMenuItem:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            // Add menu item handlers so that OS save panels have the correct key commands
            decl.add_method(
                sel!(cut:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(copy:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(paste:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(selectAll:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(undo:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(redo:),
                handle_menu_item as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(validateMenuItem:),
                validate_menu_item as extern "C" fn(&mut Object, Sel, id) -> bool,
            );
            decl.add_method(
                sel!(menuWillOpen:),
                menu_will_open as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationDockMenu:),
                handle_dock_menu as extern "C" fn(&mut Object, Sel, id) -> id,
            );
            decl.add_method(
                sel!(application:openURLs:),
                open_urls as extern "C" fn(&mut Object, Sel, id, id),
            );

            decl.add_method(
                sel!(onKeyboardLayoutChange:),
                on_keyboard_layout_change as extern "C" fn(&mut Object, Sel, id),
            );

            decl.add_method(
                sel!(onSystemSleep:),
                on_system_sleep as extern "C" fn(&mut Object, Sel, id),
            );

            decl.add_method(
                sel!(onSystemWake:),
                on_system_wake as extern "C" fn(&mut Object, Sel, id),
            );

            decl.add_method(
                sel!(onScreenParametersChange:),
                on_screen_parameters_change as extern "C" fn(&mut Object, Sel, id),
            );

            decl.register()
        }
    }
}

pub struct MacPlatform(Mutex<MacPlatformState>, MacNativeNotificationCallbacks);

#[derive(Default)]
struct MacNativeNotificationCallbacks {
    system_sleep: OnceLock<NativeAppNotificationHandler>,
    system_wake: OnceLock<NativeAppNotificationHandler>,
}

pub(crate) struct MacPlatformState {
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    renderer_context: renderer::Context,
    display_topology: MacDisplayTopologyHandle,
    window_registry: MacWindowRegistry,
    headless: bool,
    general_pasteboard: Pasteboard,
    find_pasteboard: Pasteboard,
    reopen: Option<Box<dyn FnMut()>>,
    on_keyboard_layout_change: Option<Box<dyn FnMut()>>,
    on_displays_changed: Option<Box<dyn FnMut()>>,
    system_power_observers_registered: bool,
    thermal_observer: Option<ProcessInfoObserver>,
    power_observer: Option<ProcessInfoObserver>,
    quit: Option<Box<dyn FnMut()>>,
    menu_command: Option<Box<dyn FnMut(&dyn Action)>>,
    validate_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
    will_open_menu: Option<Box<dyn FnMut()>>,
    menu_actions: Vec<Box<dyn Action>>,
    open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    finish_launching: Option<Box<dyn FnOnce()>>,
    dock_menu: Option<id>,
    menus: Option<Vec<OwnedMenu>>,
    keyboard_mapper: Rc<MacKeyboardMapper>,
    /// Mirrors `[NSCursor setHiddenUntilMouseMoves:]` state, which AppKit doesn't expose.
    cursor_visible: Arc<AtomicBool>,
    system_notifications: crate::system_notifications::SystemNotificationState,
}

impl MacPlatform {
    pub fn new(headless: bool) -> Self {
        let dispatcher = Arc::new(MacDispatcher::new());

        #[cfg(feature = "font-kit")]
        let text_system = Arc::new(crate::MacTextSystem::new());

        #[cfg(not(feature = "font-kit"))]
        let text_system = Arc::new(open_gpui::NoopTextSystem::new());

        let keyboard_layout = MacKeyboardLayout::new();
        let keyboard_mapper = Rc::new(MacKeyboardMapper::new(keyboard_layout.id()));
        let display_topology = MacDisplayTopologyHandle::from_native();
        if let Err(error) = display_topology.exact_snapshot() {
            log::error!("cannot collect the initial macOS display topology: {error}");
        }

        Self(
            Mutex::new(MacPlatformState {
                headless,
                text_system,
                background_executor: BackgroundExecutor::new(dispatcher.clone()),
                foreground_executor: ForegroundExecutor::new(dispatcher),
                renderer_context: renderer::Context::default(),
                display_topology,
                window_registry: MacWindowRegistry::default(),
                general_pasteboard: Pasteboard::general(),
                find_pasteboard: Pasteboard::find(),
                reopen: None,
                quit: None,
                menu_command: None,
                validate_menu_command: None,
                will_open_menu: None,
                menu_actions: Default::default(),
                open_urls: None,
                finish_launching: None,
                dock_menu: None,
                on_keyboard_layout_change: None,
                on_displays_changed: None,
                system_power_observers_registered: false,
                thermal_observer: None,
                power_observer: None,
                menus: None,
                keyboard_mapper,
                cursor_visible: Arc::new(AtomicBool::new(true)),
                system_notifications: crate::system_notifications::SystemNotificationState::new(),
            }),
            MacNativeNotificationCallbacks::default(),
        )
    }

    fn display_topology(&self) -> MacDisplayTopologyHandle {
        self.0.lock().display_topology.clone()
    }

    fn ensure_system_power_observers(&self) {
        if self.0.lock().system_power_observers_registered {
            return;
        }

        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            let delegate: id = msg_send![app, delegate];
            if delegate != nil {
                register_system_power_observers(delegate);
                self.0.lock().system_power_observers_registered = true;
            }
        }
    }

    fn request_display_topology_refresh(&self) {
        let display_topology = self.display_topology();
        let request = display_topology.request_refresh();
        if !request.should_schedule {
            return;
        }
        let context = Box::into_raw(Box::new(display_topology)) as *mut c_void;
        unsafe {
            DispatchQueue::main()
                .exec_async_f(context, complete_scheduled_display_topology_refresh);
        }
    }

    fn refresh_display_topology_now(&self) {
        let display_topology = self.display_topology();
        display_topology.request_refresh();
        let request_epoch = display_topology.begin_scheduled_refresh();
        if let Some(request_epoch) = request_epoch {
            self.complete_display_topology_refresh(request_epoch);
        }
    }

    fn complete_display_topology_refresh(&self, request_epoch: u64) {
        complete_mac_display_topology_refresh(self.display_topology(), request_epoch);
    }

    fn open_window_against_display_topology(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
        expected_generation: Option<u64>,
    ) -> Result<Box<dyn PlatformWindow>> {
        let (
            cursor_visible,
            foreground_executor,
            background_executor,
            renderer_context,
            display_topology,
            window_registry,
        ) = {
            let guard = self.0.lock();
            (
                guard.cursor_visible.clone(),
                guard.foreground_executor.clone(),
                guard.background_executor.clone(),
                guard.renderer_context.clone(),
                guard.display_topology.clone(),
                guard.window_registry.clone(),
            )
        };
        let display_snapshot = display_topology.exact_snapshot().map_err(|error| {
            anyhow!("cannot open a macOS window without an exact display topology: {error}")
        })?;
        if let Some(expected_generation) = expected_generation {
            anyhow::ensure!(
                display_snapshot.generation() == expected_generation,
                "display topology changed after GPUI resolved the macOS window placement"
            );
        }
        let target_display = display_snapshot
            .resolve_native_target(options.display_id)
            .map_err(|error| anyhow!("cannot resolve the macOS target display: {error}"))?;
        let transient_owner = options
            .transient_for
            .map(|owner| window_registry.resolve_transient_owner(owner))
            .transpose()?;

        Ok(Box::new(MacWindow::open(
            handle,
            options,
            transient_owner,
            window_registry,
            display_topology,
            display_snapshot,
            target_display,
            cursor_visible,
            foreground_executor,
            background_executor,
            renderer_context,
        )?))
    }

    unsafe fn create_menu_bar(
        &self,
        menus: &Vec<Menu>,
        delegate: id,
        actions: &mut Vec<Box<dyn Action>>,
        keymap: &Keymap,
    ) -> id {
        unsafe {
            let application_menu = NSMenu::new(nil).autorelease();
            application_menu.setDelegate_(delegate);

            for menu_config in menus {
                let menu = NSMenu::new(nil).autorelease();
                let menu_title = ns_string(&menu_config.name);
                menu.setTitle_(menu_title);
                menu.setDelegate_(delegate);

                for item_config in &menu_config.items {
                    menu.addItem_(Self::create_menu_item(
                        item_config,
                        delegate,
                        actions,
                        keymap,
                    ));
                }

                let menu_item = NSMenuItem::new(nil).autorelease();
                menu_item.setTitle_(menu_title);
                menu_item.setSubmenu_(menu);
                application_menu.addItem_(menu_item);

                if menu_config.name == "Window" {
                    let app: id = msg_send![APP_CLASS, sharedApplication];
                    app.setWindowsMenu_(menu);
                }
            }

            application_menu
        }
    }

    unsafe fn create_dock_menu(
        &self,
        menu_items: Vec<MenuItem>,
        delegate: id,
        actions: &mut Vec<Box<dyn Action>>,
        keymap: &Keymap,
    ) -> id {
        unsafe {
            let dock_menu = NSMenu::new(nil);
            dock_menu.setDelegate_(delegate);
            for item_config in menu_items {
                dock_menu.addItem_(Self::create_menu_item(
                    &item_config,
                    delegate,
                    actions,
                    keymap,
                ));
            }

            dock_menu
        }
    }

    unsafe fn create_menu_item(
        item: &MenuItem,
        delegate: id,
        actions: &mut Vec<Box<dyn Action>>,
        keymap: &Keymap,
    ) -> id {
        static DEFAULT_CONTEXT: OnceLock<Vec<KeyContext>> = OnceLock::new();

        unsafe {
            match item {
                MenuItem::Separator => NSMenuItem::separatorItem(nil),
                MenuItem::Action {
                    name,
                    action,
                    os_action,
                    checked,
                    disabled,
                } => {
                    // Note that this is intentionally using earlier bindings, whereas typically
                    // later ones take display precedence. See the discussion on
                    // https://github.com/zed-industries/zed/issues/23621
                    let keystrokes = keymap
                        .bindings_for_action(action.as_ref())
                        .find_or_first(|binding| {
                            binding.predicate().is_none_or(|predicate| {
                                predicate.eval(DEFAULT_CONTEXT.get_or_init(|| {
                                    let mut workspace_context = KeyContext::new_with_defaults(
                                        open_gpui::OperatingSystem::Mac,
                                    );
                                    workspace_context.add("Workspace");
                                    let mut pane_context = KeyContext::new_with_defaults(
                                        open_gpui::OperatingSystem::Mac,
                                    );
                                    pane_context.add("Pane");
                                    let mut editor_context = KeyContext::new_with_defaults(
                                        open_gpui::OperatingSystem::Mac,
                                    );
                                    editor_context.add("Editor");

                                    pane_context.extend(&editor_context);
                                    workspace_context.extend(&pane_context);
                                    vec![workspace_context]
                                }))
                            })
                        })
                        .map(|binding| binding.keystrokes());

                    let selector = match os_action {
                        Some(open_gpui::OsAction::Cut) => selector("cut:"),
                        Some(open_gpui::OsAction::Copy) => selector("copy:"),
                        Some(open_gpui::OsAction::Paste) => selector("paste:"),
                        Some(open_gpui::OsAction::SelectAll) => selector("selectAll:"),
                        // "undo:" and "redo:" are always disabled in our case, as
                        // we don't have a NSTextView/NSTextField to enable them on.
                        Some(open_gpui::OsAction::Undo) => selector("handleGPUIMenuItem:"),
                        Some(open_gpui::OsAction::Redo) => selector("handleGPUIMenuItem:"),
                        None => selector("handleGPUIMenuItem:"),
                    };

                    let item;
                    if let Some(keystrokes) = keystrokes {
                        if keystrokes.len() == 1 {
                            let keystroke = &keystrokes[0];
                            let mut mask = NSEventModifierFlags::empty();
                            for (modifier, flag) in &[
                                (
                                    keystroke.modifiers().platform,
                                    NSEventModifierFlags::NSCommandKeyMask,
                                ),
                                (
                                    keystroke.modifiers().control,
                                    NSEventModifierFlags::NSControlKeyMask,
                                ),
                                (
                                    keystroke.modifiers().alt,
                                    NSEventModifierFlags::NSAlternateKeyMask,
                                ),
                                (
                                    keystroke.modifiers().shift,
                                    NSEventModifierFlags::NSShiftKeyMask,
                                ),
                            ] {
                                if *modifier {
                                    mask |= *flag;
                                }
                            }

                            item = NSMenuItem::alloc(nil)
                                .initWithTitle_action_keyEquivalent_(
                                    ns_string(name),
                                    selector,
                                    ns_string(key_to_native(keystroke.key()).as_ref()),
                                )
                                .autorelease();
                            if Self::os_version() >= Version::new(12, 0, 0) {
                                let _: () = msg_send![item, setAllowsAutomaticKeyEquivalentLocalization: NO];
                            }
                            item.setKeyEquivalentModifierMask_(mask);
                        } else {
                            item = NSMenuItem::alloc(nil)
                                .initWithTitle_action_keyEquivalent_(
                                    ns_string(name),
                                    selector,
                                    ns_string(""),
                                )
                                .autorelease();
                        }
                    } else {
                        item = NSMenuItem::alloc(nil)
                            .initWithTitle_action_keyEquivalent_(
                                ns_string(name),
                                selector,
                                ns_string(""),
                            )
                            .autorelease();
                    }

                    if *checked {
                        item.setState_(NSVisualEffectState::Active);
                    }
                    item.setEnabled_(if *disabled { NO } else { YES });

                    let tag = actions.len() as NSInteger;
                    let _: () = msg_send![item, setTag: tag];
                    actions.push(action.boxed_clone());
                    item
                }
                MenuItem::Submenu(Menu {
                    name,
                    items,
                    disabled,
                }) => {
                    let item = NSMenuItem::new(nil).autorelease();
                    let submenu = NSMenu::new(nil).autorelease();
                    submenu.setDelegate_(delegate);
                    for item in items {
                        submenu.addItem_(Self::create_menu_item(item, delegate, actions, keymap));
                    }
                    item.setSubmenu_(submenu);
                    item.setEnabled_(if *disabled { NO } else { YES });
                    item.setTitle_(ns_string(name));
                    item
                }
                MenuItem::SystemMenu(OsMenu { name, menu_type }) => {
                    let item = NSMenuItem::new(nil).autorelease();
                    let submenu = NSMenu::new(nil).autorelease();
                    submenu.setDelegate_(delegate);
                    item.setSubmenu_(submenu);
                    item.setTitle_(ns_string(name));

                    match menu_type {
                        SystemMenuType::Services => {
                            let app: id = msg_send![APP_CLASS, sharedApplication];
                            app.setServicesMenu_(item);
                        }
                    }

                    item
                }
            }
        }
    }

    fn os_version() -> Version {
        let version = unsafe {
            let process_info = NSProcessInfo::processInfo(nil);
            process_info.operatingSystemVersion()
        };
        Version::new(
            version.majorVersion,
            version.minorVersion,
            version.patchVersion,
        )
    }
}

fn complete_mac_display_topology_refresh(
    display_topology: MacDisplayTopologyHandle,
    request_epoch: u64,
) {
    let candidate = MacDisplayTopologyHandle::refresh_candidate_from_native();
    let refresh = display_topology.finish_refresh(request_epoch, candidate);
    match refresh {
        MacDisplayTopologyRefresh::Unchanged { .. }
        | MacDisplayTopologyRefresh::Published { .. }
        | MacDisplayTopologyRefresh::Superseded { .. } => {}
        MacDisplayTopologyRefresh::RetainedAfterFailure {
            generation,
            failure,
            retry,
        } => {
            log::error!(
                "retaining macOS display topology generation {:?} after refresh failure: {}",
                generation,
                failure
            );
            if let Some(retry) = retry {
                schedule_mac_display_topology_retry(display_topology, retry);
            }
        }
    }
}

fn schedule_mac_display_topology_retry(
    display_topology: MacDisplayTopologyHandle,
    retry: MacDisplayTopologyRetry,
) {
    let Ok(delay_nanos) = i64::try_from(retry.delay().as_nanos()) else {
        display_topology.cancel_retry(retry.retry_epoch());
        log::error!(
            "cannot schedule macOS display-topology retry {}: delay is out of range",
            retry.attempt()
        );
        return;
    };
    let when = DispatchTime::NOW.time(delay_nanos);
    let retry_topology = display_topology.downgrade();
    if let Err(error) = DispatchQueue::main().after(when, move || {
        let Some(retry_topology) = retry_topology.upgrade() else {
            return;
        };
        let Some(request_epoch) = retry_topology.begin_retry(retry.retry_epoch()) else {
            return;
        };
        complete_mac_display_topology_refresh(retry_topology.clone(), request_epoch);
    }) {
        display_topology.cancel_retry(retry.retry_epoch());
        log::error!(
            "cannot schedule macOS display-topology retry {}: {:?}",
            retry.attempt(),
            error
        );
    }
}

fn macos_window_capabilities(kind: &WindowKind) -> PlatformWindowCapabilities {
    let supports_toplevel_state = macos_supports_toplevel_creation_state(kind);
    let supports_transient_owner = !matches!(kind, WindowKind::PopUp);
    PlatformWindowCapabilities {
        creation: PlatformWindowCreationCapabilities {
            focus_on_appearing: if macos_supports_focus_on_appearing(kind) {
                WindowCreationSupport::Supported
            } else {
                WindowCreationSupport::Unsupported
            },
            transient_for: if supports_transient_owner {
                WindowCreationSupport::Supported
            } else {
                WindowCreationSupport::Unsupported
            },
            provisional_presentation: WindowCreationSupport::Unsupported,
            initial_presentation_order: WindowInitialPresentationOrder::AfterVisibility,
        },
        mutations: PlatformWindowMutationCapabilities {
            position: WindowMutationSupport::CreationOnly,
            physical_placement: WindowMutationSupport::Unsupported,
            size: WindowMutationSupport::CreationOnly,
            windowed: WindowMutationSupport::CreationOnly,
            maximized: if supports_toplevel_state {
                WindowMutationSupport::CreationOnly
            } else {
                WindowMutationSupport::Unsupported
            },
            fullscreen: if supports_toplevel_state {
                WindowMutationSupport::CreationOnly
            } else {
                WindowMutationSupport::Unsupported
            },
            minimized: WindowMutationSupport::Unsupported,
            restore_bounds: if supports_toplevel_state {
                WindowMutationSupport::CreationOnly
            } else {
                WindowMutationSupport::Unsupported
            },
            pointer_input: WindowMutationSupport::CreationOnly,
            activation_policy: if supports_toplevel_state {
                WindowMutationSupport::CreationOnly
            } else {
                WindowMutationSupport::Unsupported
            },
            alpha: WindowMutationSupport::CreationOnly,
            topmost: WindowMutationSupport::Unsupported,
            taskbar_visibility: WindowMutationSupport::Unsupported,
            coordinate_space: WindowCoordinateSpace::GlobalScreen,
        },
        activation: PlatformWindowActivationSupport::Observed,
    }
}

fn restart_command(
    app_pid: &str,
    app_path: &Path,
    arguments: Vec<std::ffi::OsString>,
) -> std::process::Command {
    use std::os::unix::process::CommandExt as _;

    // Wait until this process has exited and then re-open this path.
    let script = r#"
            while kill -0 $0 2> /dev/null; do
                sleep 0.1
            done
            app_path="$1"
            shift
            if (($# > 0)); then
                open "$app_path" --args "$@"
            else
                open "$app_path"
            fi
        "#;

    #[allow(
        clippy::disallowed_methods,
        reason = "We are restarting ourselves, using std command thus is fine"
    )]
    let mut command = new_std_command("/bin/bash");
    command
        .arg("-c")
        .arg(script)
        .arg(app_pid)
        .arg(app_path)
        .args(arguments)
        .process_group(0);
    command
}

impl Platform for MacPlatform {
    fn platform_info(&self) -> open_gpui::PlatformInfo {
        open_gpui::PlatformInfo {
            os: open_gpui::OperatingSystem::Mac,
            target: open_gpui::PlatformTarget::Native,
        }
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.0.lock().background_executor.clone()
    }

    fn foreground_executor(&self) -> open_gpui::ForegroundExecutor {
        self.0.lock().foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.0.lock().text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
        let display_notification_lifetime = Arc::new(());
        let notification_lifetime = Arc::downgrade(&display_notification_lifetime);
        let platform_address = self as *const Self as usize;
        let _display_subscription =
            self.display_topology()
                .subscribe_displays_changed(Arc::new(move |_| {
                    let notification = Box::new(MacDisplaysChangedNotification {
                        platform: platform_address as *const MacPlatform,
                        lifetime: notification_lifetime.clone(),
                    });
                    unsafe {
                        DispatchQueue::main().exec_async_f(
                            Box::into_raw(notification) as *mut c_void,
                            report_mac_displays_changed,
                        );
                    }
                }));
        let mut state = self.0.lock();
        if state.headless {
            drop(state);
            on_finish_launching();
            unsafe { CFRunLoopRun() };
        } else {
            state.finish_launching = Some(on_finish_launching);
            drop(state);
        }

        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            let app_delegate: id = msg_send![APP_DELEGATE_CLASS, new];
            app.setDelegate_(app_delegate);

            let self_ptr = self as *const Self as *const c_void;
            (*app).set_ivar(MAC_PLATFORM_IVAR, self_ptr);
            (*app_delegate).set_ivar(MAC_PLATFORM_IVAR, self_ptr);

            let pool = NSAutoreleasePool::new(nil);
            app.run();
            pool.drain();

            (*app).set_ivar(MAC_PLATFORM_IVAR, null_mut::<c_void>());
            (*NSWindow::delegate(app)).set_ivar(MAC_PLATFORM_IVAR, null_mut::<c_void>());
        }
    }

    fn quit(&self) {
        // Quitting the app causes us to close windows, which invokes `Window::on_close` callbacks
        // synchronously before this method terminates. If we call `Platform::quit` while holding a
        // borrow of the app state (which most of the time we will do), we will end up
        // double-borrowing the app state in the `on_close` callbacks for our open windows. To solve
        // this, we make quitting the application asynchronous so that we aren't holding borrows to
        // the app state on the stack when we actually terminate the app.

        unsafe {
            DispatchQueue::main().exec_async_f(ptr::null_mut(), quit);
        }

        extern "C" fn quit(_: *mut c_void) {
            unsafe {
                let app = NSApplication::sharedApplication(nil);
                let _: () = msg_send![app, terminate: nil];
            }
        }
    }

    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<std::ffi::OsString>) {
        let app_pid = std::process::id().to_string();
        let app_path = binary_path
            .or_else(|| {
                self.app_path()
                    .ok()
                    // When the app is not bundled, `app_path` returns the
                    // directory containing the executable. Disregard this
                    // and get the path to the executable itself.
                    .and_then(|path| (path.extension()?.to_str()? == "app").then_some(path))
            })
            .unwrap_or_else(|| std::env::current_exe().unwrap());

        let restart_process = restart_command(&app_pid, &app_path, arguments).spawn();

        match restart_process {
            Ok(_) => self.quit(),
            Err(e) => log::error!("failed to spawn restart script: {:?}", e),
        }
    }

    fn activate(&self, ignoring_other_apps: bool) {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            app.activateIgnoringOtherApps_(ignoring_other_apps.to_objc());
        }
    }

    fn hide(&self) {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let _: () = msg_send![app, hide: nil];
        }
    }

    fn hide_other_apps(&self) {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let _: () = msg_send![app, hideOtherApplications: nil];
        }
    }

    fn unhide_other_apps(&self) {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let _: () = msg_send![app, unhideAllApplications: nil];
        }
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.display_snapshot().primary_display()
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.display_snapshot().displays()
    }

    fn display_snapshot(&self) -> PlatformDisplaySnapshot {
        self.display_topology().retained_platform_snapshot()
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().on_displays_changed = Some(callback);
    }

    #[cfg(feature = "screen-capture")]
    fn is_screen_capture_supported(&self) -> bool {
        let min_version = cocoa::foundation::NSOperatingSystemVersion::new(12, 3, 0);
        crate::is_macos_version_at_least(min_version)
    }

    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn open_gpui::ScreenCaptureSource>>>> {
        crate::screen_capture::get_sources()
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        MacWindow::active_window()
    }

    fn focused_window(&self) -> PlatformFocusedWindow {
        PlatformFocusedWindow::from_window(MacWindow::focused_window())
    }

    fn hovered_window(&self) -> PlatformHoveredWindow {
        PlatformHoveredWindow::from_window(MacWindow::hovered_window())
    }

    // Returns the windows ordered front-to-back, meaning that the active
    // window is the first one in the returned vec.
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        Some(MacWindow::ordered_windows())
    }

    fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        PlatformViewportCapabilities {
            platform_viewport_windows: true,
            global_window_bounds: true,
            window_stack: true,
            display_work_area: true,
            dpi_scale: true,
            hovered_window_ignores_no_input: true,
            ..Default::default()
        }
    }

    fn window_capabilities(
        &self,
        kind: &WindowKind,
        _display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        macos_window_capabilities(kind)
    }

    fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        let button_number = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Navigate(NavigationDirection::Back) => 3,
            MouseButton::Navigate(NavigationDirection::Forward) => 4,
        };
        let pressed_buttons: NSUInteger =
            unsafe { msg_send![class!(NSEvent), pressedMouseButtons] };
        Some((pressed_buttons & (1 << button_number)) != 0)
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        self.open_window_against_display_topology(handle, options, None)
    }

    fn open_window_with_display_snapshot(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
        display_snapshot: PlatformDisplaySnapshot,
    ) -> Result<Box<dyn PlatformWindow>> {
        let expected_generation = display_snapshot.generation().ok_or_else(|| {
            anyhow!("macOS window creation requires an atomic display publication")
        })?;
        self.open_window_against_display_topology(handle, options, Some(expected_generation))
    }

    fn window_appearance(&self) -> WindowAppearance {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let appearance: id = msg_send![app, effectiveAppearance];
            crate::window_appearance::window_appearance_from_native(appearance)
        }
    }

    fn open_url(&self, url: &str) {
        unsafe {
            let ns_url = NSURL::alloc(nil).initWithString_(ns_string(url));
            if ns_url.is_null() {
                log::error!("Failed to create NSURL from string: {}", url);
                return;
            }
            let url = ns_url.autorelease();
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            msg_send![workspace, openURL: url]
        }
    }

    fn register_url_scheme(&self, scheme: &str) -> Task<anyhow::Result<()>> {
        // API only available post Monterey
        // https://developer.apple.com/documentation/appkit/nsworkspace/3753004-setdefaultapplicationaturl
        let (done_tx, done_rx) = oneshot::channel();
        if Self::os_version() < Version::new(12, 0, 0) {
            return Task::ready(Err(anyhow!(
                "macOS 12.0 or later is required to register URL schemes"
            )));
        }

        let bundle_id = unsafe {
            let bundle: id = msg_send![class!(NSBundle), mainBundle];
            let bundle_id: id = msg_send![bundle, bundleIdentifier];
            if bundle_id == nil {
                return Task::ready(Err(anyhow!("Can only register URL scheme in bundled apps")));
            }
            bundle_id
        };

        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let scheme: id = ns_string(scheme);
            let app: id = msg_send![workspace, URLForApplicationWithBundleIdentifier: bundle_id];
            if app == nil {
                return Task::ready(Err(anyhow!(
                    "Cannot register URL scheme until app is installed"
                )));
            }
            let done_tx = Cell::new(Some(done_tx));
            let block: RcBlock<dyn Fn(*mut c_void)> = RcBlock::new(move |error| {
                let error = error as id;
                let result = if error == nil {
                    Ok(())
                } else {
                    let msg: id = msg_send![error, localizedDescription];
                    Err(anyhow!("Failed to register: {msg:?}"))
                };

                if let Some(done_tx) = done_tx.take() {
                    let _ = done_tx.send(result);
                }
            });
            let _: () = msg_send![workspace, setDefaultApplicationAtURL: app toOpenURLsWithScheme: scheme completionHandler: &*block];
        }

        self.background_executor()
            .spawn(async { done_rx.await.map_err(|e| anyhow!(e))? })
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.0.lock().open_urls = Some(callback);
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (done_tx, done_rx) = oneshot::channel();
        self.foreground_executor()
            .spawn(async move {
                unsafe {
                    let panel = NSOpenPanel::openPanel(nil);
                    panel.setCanChooseDirectories_(options.directories.to_objc());
                    panel.setCanChooseFiles_(options.files.to_objc());
                    panel.setAllowsMultipleSelection_(options.multiple.to_objc());

                    panel.setCanCreateDirectories(true.to_objc());
                    panel.setResolvesAliases_(false.to_objc());
                    let done_tx = Cell::new(Some(done_tx));
                    let block: RcBlock<dyn Fn(NSInteger)> = RcBlock::new(move |response| {
                        let result = if response == NSModalResponse::NSModalResponseOk as NSInteger
                        {
                            let mut result = Vec::new();
                            let urls = panel.URLs();
                            for i in 0..urls.count() {
                                let url = urls.objectAtIndex(i);
                                if url.isFileURL() == YES
                                    && let Ok(path) = ns_url_to_path(url)
                                {
                                    result.push(path)
                                }
                            }
                            Some(result)
                        } else {
                            None
                        };

                        if let Some(done_tx) = done_tx.take() {
                            let _ = done_tx.send(Ok(result));
                        }
                    });

                    if let Some(prompt) = options.prompt {
                        let _: () = msg_send![panel, setPrompt: ns_string(&prompt)];
                    }

                    let _: () = msg_send![panel, beginWithCompletionHandler: &*block];
                }
            })
            .detach();
        done_rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let directory = directory.to_owned();
        let suggested_name = suggested_name.map(|s| s.to_owned());
        let (done_tx, done_rx) = oneshot::channel();
        self.foreground_executor()
            .spawn(async move {
                unsafe {
                    let panel = NSSavePanel::savePanel(nil);
                    let path = ns_string(directory.to_string_lossy().as_ref());
                    let url = NSURL::fileURLWithPath_isDirectory_(nil, path, true.to_objc());
                    panel.setDirectoryURL(url);

                    if let Some(suggested_name) = suggested_name {
                        let name_string = ns_string(&suggested_name);
                        let _: () = msg_send![panel, setNameFieldStringValue: name_string];
                    }

                    let done_tx = Cell::new(Some(done_tx));
                    let block: RcBlock<dyn Fn(NSInteger)> = RcBlock::new(move |response| {
                        let mut result = None;
                        if response == NSModalResponse::NSModalResponseOk as NSInteger {
                            let url = panel.URL();
                            if url.isFileURL() == YES {
                                result = ns_url_to_path(panel.URL()).ok().map(|mut result| {
                                    let Some(filename) = result.file_name() else {
                                        return result;
                                    };
                                    let chunks = filename
                                        .as_bytes()
                                        .split(|&b| b == b'.')
                                        .collect::<Vec<_>>();

                                    // https://github.com/zed-industries/zed/issues/16969
                                    // Workaround a bug in macOS Sequoia that adds an extra file-extension
                                    // sometimes. e.g. `a.sql` becomes `a.sql.s` or `a.txtx` becomes `a.txtx.txt`
                                    //
                                    // This is conditional on OS version because I'd like to get rid of it, so that
                                    // you can manually create a file called `a.sql.s`. That said it seems better
                                    // to break that use-case than breaking `a.sql`.
                                    if chunks.len() == 3
                                        && chunks[1].starts_with(chunks[2])
                                        && Self::os_version() >= Version::new(15, 0, 0)
                                    {
                                        let new_filename = OsStr::from_bytes(
                                            &filename.as_bytes()
                                                [..chunks[0].len() + 1 + chunks[1].len()],
                                        )
                                        .to_owned();
                                        result.set_file_name(&new_filename);
                                    }
                                    result
                                })
                            }
                        }

                        if let Some(done_tx) = done_tx.take() {
                            let _ = done_tx.send(Ok(result));
                        }
                    });
                    let _: () = msg_send![panel, beginWithCompletionHandler: &*block];
                }
            })
            .detach();

        done_rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, path: &Path) {
        unsafe {
            let path = path.to_path_buf();
            self.0
                .lock()
                .background_executor
                .spawn(async move {
                    let full_path = ns_string(path.to_str().unwrap_or(""));
                    let root_full_path = ns_string("");
                    let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
                    let _: BOOL = msg_send![
                        workspace,
                        selectFile: full_path
                        inFileViewerRootedAtPath: root_full_path
                    ];
                })
                .detach();
        }
    }

    fn open_with_system(&self, path: &Path) {
        let path = path.to_owned();
        self.0
            .lock()
            .background_executor
            .spawn(async move {
                if let Some(mut child) = new_command("open")
                    .arg("--")
                    .arg(path)
                    .spawn()
                    .context("invoking open command")
                    .log_err()
                {
                    child.status().await.log_err();
                }
            })
            .detach();
    }

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().quit = Some(callback);
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().reopen = Some(callback);
    }

    fn on_system_sleep(&self, callback: NativeAppNotificationHandler) {
        assert!(
            self.1.system_sleep.set(callback).is_ok(),
            "system sleep callback must be installed exactly once"
        );
        self.ensure_system_power_observers();
    }

    fn on_system_wake(&self, callback: NativeAppNotificationHandler) {
        assert!(
            self.1.system_wake.set(callback).is_ok(),
            "system wake callback must be installed exactly once"
        );
        self.ensure_system_power_observers();
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().on_keyboard_layout_change = Some(callback);
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.0.lock().menu_command = Some(callback);
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().will_open_menu = Some(callback);
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.0.lock().validate_menu_command = Some(callback);
    }

    fn on_thermal_state_change(&self, callback: NativeAppNotificationHandler) {
        let observer = ProcessInfoObserver::thermal(callback);
        let previous = self.0.lock().thermal_observer.replace(observer);
        drop(previous);
    }

    fn thermal_state(&self) -> ThermalState {
        process_info::thermal_state()
    }

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Ok(AppleActivity::begin(
            reason,
            NSActivityOptions::UserInitiated,
        )))
    }

    fn power_mode(&self) -> PowerMode {
        process_info::power_mode()
    }

    fn on_power_mode_change(&self, callback: NativeAppNotificationHandler) {
        let observer = ProcessInfoObserver::power(callback);
        let previous = self.0.lock().power_observer.replace(observer);
        drop(previous);
    }

    fn show_system_notification(&self, notification: open_gpui::SystemNotification) {
        let mut state = self.0.lock();
        let executor = state.foreground_executor.clone();
        state.system_notifications.show(&executor, notification);
    }

    fn dismiss_system_notification(&self, tag: &str) {
        let mut state = self.0.lock();
        let executor = state.foreground_executor.clone();
        state.system_notifications.dismiss(&executor, tag);
    }

    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(open_gpui::SystemNotificationResponse)>,
    ) {
        let mut state = self.0.lock();
        let executor = state.foreground_executor.clone();
        state.system_notifications.on_response(&executor, callback);
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(MacKeyboardLayout::new())
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.0.lock().keyboard_mapper.clone()
    }

    fn app_path(&self) -> Result<PathBuf> {
        unsafe {
            let bundle: id = NSBundle::mainBundle();
            anyhow::ensure!(!bundle.is_null(), "app is not running inside a bundle");
            Ok(path_from_objc(msg_send![bundle, bundlePath]))
        }
    }

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            let mut state = self.0.lock();
            let actions = &mut state.menu_actions;
            let menu = self.create_menu_bar(&menus, NSWindow::delegate(app), actions, keymap);
            drop(state);
            app.setMainMenu_(menu);
        }
        self.0.lock().menus = Some(menus.into_iter().map(|menu| menu.owned()).collect());
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.0.lock().menus.clone()
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        unsafe {
            let app: id = msg_send![APP_CLASS, sharedApplication];
            let mut state = self.0.lock();
            let actions = &mut state.menu_actions;
            let new = self.create_dock_menu(menu, NSWindow::delegate(app), actions, keymap);
            if let Some(old) = state.dock_menu.replace(new) {
                CFRelease(old as _)
            }
        }
    }

    fn add_recent_document(&self, path: &Path) {
        if let Some(path_str) = path.to_str() {
            unsafe {
                let document_controller: id =
                    msg_send![class!(NSDocumentController), sharedDocumentController];
                let url: id = NSURL::fileURLWithPath_(nil, ns_string(path_str));
                let _: () = msg_send![document_controller, noteNewRecentDocumentURL:url];
            }
        }
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        unsafe {
            let bundle: id = NSBundle::mainBundle();
            anyhow::ensure!(!bundle.is_null(), "app is not running inside a bundle");
            let name = ns_string(name);
            let url: id = msg_send![bundle, URLForAuxiliaryExecutable: name];
            anyhow::ensure!(!url.is_null(), "resource not found");
            ns_url_to_path(url)
        }
    }

    fn hide_cursor_until_mouse_moves(&self) {
        let cursor_visible = self.0.lock().cursor_visible.clone();
        if !cursor_visible.swap(false, Ordering::Relaxed) {
            return;
        }
        unsafe {
            let _: () = msg_send![class!(NSCursor), setHiddenUntilMouseMoves: YES];
        }
    }

    fn is_cursor_visible(&self) -> bool {
        self.0.lock().cursor_visible.load(Ordering::Relaxed)
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        #[allow(non_upper_case_globals)]
        const NSScrollerStyleOverlay: NSInteger = 1;

        unsafe {
            let style: NSInteger = msg_send![class!(NSScroller), preferredScrollerStyle];
            style == NSScrollerStyleOverlay
        }
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        let state = self.0.lock();
        state.general_pasteboard.read()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        let state = self.0.lock();
        state.general_pasteboard.write(item);
    }

    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        let state = self.0.lock();
        state.find_pasteboard.read()
    }

    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        let state = self.0.lock();
        state.find_pasteboard.write(item);
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        let url = url.to_string();
        let username = username.to_string();
        let password = password.to_vec();
        self.background_executor()
            .spawn(async move { Ok(keychain::write_credentials(&url, &username, &password)?) })
    }

    fn write_credentials_if_not_exists(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<Result<bool>> {
        let url = url.to_string();
        let username = username.to_string();
        let password = password.to_vec();
        self.background_executor().spawn(async move {
            Ok(keychain::write_credentials_if_not_exists(
                &url, &username, &password,
            )?)
        })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let url = url.to_string();
        self.background_executor()
            .spawn(async move { Ok(keychain::read_credentials(&url)?) })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let url = url.to_string();
        self.background_executor()
            .spawn(async move { Ok(keychain::delete_credentials(&url)?) })
    }
}

unsafe fn path_from_objc(path: id) -> PathBuf {
    let len = msg_send![path, lengthOfBytesUsingEncoding: NSUTF8StringEncoding];
    let bytes = unsafe { path.UTF8String() as *const u8 };
    let path = str::from_utf8(unsafe { slice::from_raw_parts(bytes, len) }).unwrap();
    PathBuf::from(path)
}

unsafe fn get_mac_platform(object: &mut Object) -> &MacPlatform {
    unsafe {
        let platform_ptr: *mut c_void = *object.get_ivar(MAC_PLATFORM_IVAR);
        assert!(!platform_ptr.is_null());
        &*(platform_ptr as *const MacPlatform)
    }
}

unsafe fn get_mac_notification_callbacks(object: &mut Object) -> &MacNativeNotificationCallbacks {
    unsafe {
        let platform_ptr: *mut c_void = *object.get_ivar(MAC_PLATFORM_IVAR);
        assert!(!platform_ptr.is_null());
        // MAC_PLATFORM_IVAR points at the stable Rc allocation for the duration of app.run().
        // Project only the Send + Sync notification field. The first tuple field contains
        // main-thread-only Cocoa and Rc state and must not be borrowed on a posting thread.
        &*std::ptr::addr_of!((*(platform_ptr as *const MacPlatform)).1)
    }
}

extern "C" fn will_finish_launching(_this: &mut Object, _: Sel, _: id) {
    unsafe {
        let user_defaults: id = msg_send![class!(NSUserDefaults), standardUserDefaults];

        // The autofill heuristic controller causes slowdown and high CPU usage.
        // We don't know exactly why. This disables the full heuristic controller.
        //
        // Adapted from: https://github.com/ghostty-org/ghostty/pull/8625
        let name = ns_string("NSAutoFillHeuristicControllerEnabled");
        let existing_value: id = msg_send![user_defaults, objectForKey: name];
        if existing_value == nil {
            let false_value: id = msg_send![class!(NSNumber), numberWithBool:false];
            let _: () = msg_send![user_defaults, setObject: false_value forKey: name];
        }
    }
}

extern "C" fn did_finish_launching(this: &mut Object, _: Sel, _: id) {
    unsafe {
        let app: id = msg_send![APP_CLASS, sharedApplication];
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        let notification_center: *mut Object =
            msg_send![class!(NSNotificationCenter), defaultCenter];
        let name = ns_string("NSTextInputContextKeyboardSelectionDidChangeNotification");
        let _: () = msg_send![notification_center, addObserver: this as id
            selector: sel!(onKeyboardLayoutChange:)
            name: name
            object: nil
        ];

        let screen_parameters_name =
            ns_string("NSApplicationDidChangeScreenParametersNotification");
        let _: () = msg_send![notification_center, addObserver: this as id
            selector: sel!(onScreenParametersChange:)
            name: screen_parameters_name
            object: app
        ];

        let observer = this as *mut Object as id;
        let platform = get_mac_platform(this);
        platform.refresh_display_topology_now();
        let observes_system_power =
            platform.1.system_sleep.get().is_some() || platform.1.system_wake.get().is_some();
        let callback = {
            let mut state = platform.0.lock();
            if observes_system_power && !state.system_power_observers_registered {
                register_system_power_observers(observer);
                state.system_power_observers_registered = true;
            }
            state.finish_launching.take()
        };
        if let Some(callback) = callback {
            callback();
        }
    }
}

/// The observer must be the app delegate implementing `onSystemSleep:` and `onSystemWake:`.
unsafe fn register_system_power_observers(observer: id) {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let workspace_center: *mut Object = msg_send![workspace, notificationCenter];
        let sleep_name = ns_string("NSWorkspaceWillSleepNotification");
        let _: () = msg_send![workspace_center, addObserver: observer
            selector: sel!(onSystemSleep:)
            name: sleep_name
            object: nil
        ];
        let wake_name = ns_string("NSWorkspaceDidWakeNotification");
        let _: () = msg_send![workspace_center, addObserver: observer
            selector: sel!(onSystemWake:)
            name: wake_name
            object: nil
        ];
    }
}

extern "C" fn should_handle_reopen(this: &mut Object, _: Sel, _: id, has_open_windows: bool) {
    if !has_open_windows {
        let platform = unsafe { get_mac_platform(this) };
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.reopen.take() {
            drop(lock);
            callback();
            platform.0.lock().reopen.get_or_insert(callback);
        }
    }
}

extern "C" fn will_terminate(this: &mut Object, _: Sel, _: id) {
    let platform = unsafe { get_mac_platform(this) };
    let mut lock = platform.0.lock();
    if let Some(mut callback) = lock.quit.take() {
        drop(lock);
        callback();
        platform.0.lock().quit.get_or_insert(callback);
    }
}

extern "C" fn on_keyboard_layout_change(this: &mut Object, _: Sel, _: id) {
    let platform = unsafe { get_mac_platform(this) };
    let mut lock = platform.0.lock();
    let keyboard_layout = MacKeyboardLayout::new();
    lock.keyboard_mapper = Rc::new(MacKeyboardMapper::new(keyboard_layout.id()));
    if let Some(mut callback) = lock.on_keyboard_layout_change.take() {
        drop(lock);
        callback();
        platform
            .0
            .lock()
            .on_keyboard_layout_change
            .get_or_insert(callback);
    }
}

extern "C" fn on_system_sleep(this: &mut Object, _: Sel, _: id) {
    let callback = unsafe {
        get_mac_notification_callbacks(this)
            .system_sleep
            .get()
            .cloned()
    };
    if let Some(callback) = callback {
        callback();
    }
}

extern "C" fn on_system_wake(this: &mut Object, _: Sel, _: id) {
    let callback = unsafe {
        get_mac_notification_callbacks(this)
            .system_wake
            .get()
            .cloned()
    };
    if let Some(callback) = callback {
        callback();
    }
}

extern "C" fn on_screen_parameters_change(this: &mut Object, _: Sel, _: id) {
    let platform = unsafe { get_mac_platform(this) };
    platform.request_display_topology_refresh();
}

extern "C" fn complete_scheduled_display_topology_refresh(context: *mut c_void) {
    let display_topology = unsafe { Box::from_raw(context as *mut MacDisplayTopologyHandle) };
    let request_epoch = display_topology.begin_scheduled_refresh();
    if let Some(request_epoch) = request_epoch {
        complete_mac_display_topology_refresh((*display_topology).clone(), request_epoch);
    }
}

struct MacDisplaysChangedNotification {
    platform: *const MacPlatform,
    lifetime: std::sync::Weak<()>,
}

extern "C" fn report_mac_displays_changed(context: *mut c_void) {
    let notification = unsafe { Box::from_raw(context as *mut MacDisplaysChangedNotification) };
    let Some(_lifetime) = notification.lifetime.upgrade() else {
        return;
    };
    let platform = unsafe { &*notification.platform };
    let callback = platform.0.lock().on_displays_changed.take();
    if let Some(mut callback) = callback {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
        platform
            .0
            .lock()
            .on_displays_changed
            .get_or_insert(callback);
        if let Err(panic) = result {
            std::mem::forget(panic);
            log::error!("isolated a panic from a macOS display-change callback");
        }
    }
}

extern "C" fn open_urls(this: &mut Object, _: Sel, _: id, urls: id) {
    let urls = unsafe {
        (0..urls.count())
            .filter_map(|i| {
                let url = urls.objectAtIndex(i);
                match CStr::from_ptr(url.absoluteString().UTF8String() as *mut c_char).to_str() {
                    Ok(string) => Some(string.to_string()),
                    Err(err) => {
                        log::error!("error converting path to string: {}", err);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
    };
    let platform = unsafe { get_mac_platform(this) };
    let mut lock = platform.0.lock();
    if let Some(mut callback) = lock.open_urls.take() {
        drop(lock);
        callback(urls);
        platform.0.lock().open_urls.get_or_insert(callback);
    }
}

extern "C" fn handle_menu_item(this: &mut Object, _: Sel, item: id) {
    unsafe {
        let platform = get_mac_platform(this);
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.menu_command.take() {
            let tag: NSInteger = msg_send![item, tag];
            let index = tag as usize;
            if let Some(action) = lock.menu_actions.get(index) {
                let action = action.boxed_clone();
                drop(lock);
                callback(&*action);
            }
            platform.0.lock().menu_command.get_or_insert(callback);
        }
    }
}

extern "C" fn validate_menu_item(this: &mut Object, _: Sel, item: id) -> bool {
    unsafe {
        let mut result = false;
        let platform = get_mac_platform(this);
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.validate_menu_command.take() {
            let tag: NSInteger = msg_send![item, tag];
            let index = tag as usize;
            if let Some(action) = lock.menu_actions.get(index) {
                let action = action.boxed_clone();
                drop(lock);
                result = callback(action.as_ref());
            }
            platform
                .0
                .lock()
                .validate_menu_command
                .get_or_insert(callback);
        }
        result
    }
}

extern "C" fn menu_will_open(this: &mut Object, _: Sel, _: id) {
    unsafe {
        let platform = get_mac_platform(this);
        let mut lock = platform.0.lock();
        if let Some(mut callback) = lock.will_open_menu.take() {
            drop(lock);
            callback();
            platform.0.lock().will_open_menu.get_or_insert(callback);
        }
    }
}

extern "C" fn handle_dock_menu(this: &mut Object, _: Sel, _: id) -> id {
    unsafe {
        let platform = get_mac_platform(this);
        let state = platform.0.lock();
        if let Some(id) = state.dock_menu {
            id
        } else {
            nil
        }
    }
}

unsafe fn ns_url_to_path(url: id) -> Result<PathBuf> {
    let path: *mut c_char = msg_send![url, fileSystemRepresentation];
    anyhow::ensure!(!path.is_null(), "url is not a file path: {}", unsafe {
        CStr::from_ptr(url.absoluteString().UTF8String()).to_string_lossy()
    });
    Ok(PathBuf::from(OsStr::from_bytes(unsafe {
        CStr::from_ptr(path).to_bytes()
    })))
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub(super) fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut Object;
    pub(super) fn TISCopyCurrentKeyboardInputSource() -> *mut Object;
    pub(super) fn TISGetInputSourceProperty(
        inputSource: *mut Object,
        propertyKey: *const c_void,
    ) -> *mut Object;

    pub(super) fn UCKeyTranslate(
        keyLayoutPtr: *const ::std::os::raw::c_void,
        virtualKeyCode: u16,
        keyAction: u16,
        modifierKeyState: u32,
        keyboardType: u32,
        keyTranslateOptions: u32,
        deadKeyState: *mut u32,
        maxStringLength: usize,
        actualStringLength: *mut usize,
        unicodeString: *mut u16,
    ) -> u32;
    pub(super) fn LMGetKbdType() -> u16;
    pub(super) static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
    pub(super) static kTISPropertyInputSourceID: CFStringRef;
    pub(super) static kTISPropertyLocalizedName: CFStringRef;
    pub(super) static kTISPropertyInputSourceIsASCIICapable: CFStringRef;
    pub(super) static kTISPropertyInputSourceType: CFStringRef;
    pub(super) static kTISTypeKeyboardInputMode: CFStringRef;
}

#[cfg(test)]
#[path = "restart_arguments_tests.rs"]
mod restart_arguments_tests;

#[cfg(test)]
mod window_capability_tests {
    use open_gpui::{
        PlatformWindowActivationSupport, PlatformWindowCapabilities,
        PlatformWindowCreationCapabilities, PlatformWindowMutationCapabilities,
        WindowCoordinateSpace, WindowCreationSupport, WindowInitialPresentationOrder,
        WindowMutationSupport,
    };

    fn expected_macos_capabilities(
        supports_toplevel_state: bool,
        supports_transient_owner: bool,
    ) -> PlatformWindowCapabilities {
        let toplevel_state = if supports_toplevel_state {
            WindowMutationSupport::CreationOnly
        } else {
            WindowMutationSupport::Unsupported
        };
        PlatformWindowCapabilities {
            creation: PlatformWindowCreationCapabilities {
                focus_on_appearing: if supports_toplevel_state {
                    WindowCreationSupport::Supported
                } else {
                    WindowCreationSupport::Unsupported
                },
                transient_for: if supports_transient_owner {
                    WindowCreationSupport::Supported
                } else {
                    WindowCreationSupport::Unsupported
                },
                provisional_presentation: WindowCreationSupport::Unsupported,
                initial_presentation_order: WindowInitialPresentationOrder::AfterVisibility,
            },
            mutations: PlatformWindowMutationCapabilities {
                position: WindowMutationSupport::CreationOnly,
                physical_placement: WindowMutationSupport::Unsupported,
                size: WindowMutationSupport::CreationOnly,
                windowed: WindowMutationSupport::CreationOnly,
                maximized: toplevel_state,
                fullscreen: toplevel_state,
                minimized: WindowMutationSupport::Unsupported,
                restore_bounds: toplevel_state,
                pointer_input: WindowMutationSupport::CreationOnly,
                activation_policy: toplevel_state,
                alpha: WindowMutationSupport::CreationOnly,
                topmost: WindowMutationSupport::Unsupported,
                taskbar_visibility: WindowMutationSupport::Unsupported,
                coordinate_space: WindowCoordinateSpace::GlobalScreen,
            },
            activation: PlatformWindowActivationSupport::Observed,
        }
    }

    #[test]
    fn capabilities_match_exact_kind_specific_creation_paths() {
        assert_eq!(
            super::macos_window_capabilities(&open_gpui::WindowKind::Normal),
            expected_macos_capabilities(true, true)
        );
        assert_eq!(
            super::macos_window_capabilities(&open_gpui::WindowKind::Floating),
            expected_macos_capabilities(true, true)
        );
        assert_eq!(
            super::macos_window_capabilities(&open_gpui::WindowKind::PopUp),
            expected_macos_capabilities(false, false)
        );
        assert_eq!(
            super::macos_window_capabilities(&open_gpui::WindowKind::Dialog),
            expected_macos_capabilities(false, true)
        );
    }
}
