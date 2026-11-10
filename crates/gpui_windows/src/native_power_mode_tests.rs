//! Battery-saver messages over the actual hidden Windows platform HWND and public App observers.
//!
//! Synthetic `PBT_POWERSETTINGCHANGE` payloads exercise the fixed Zed Windows mapping: disabled
//! battery saver is HighPower, enabled battery saver is LowPower, and Balanced is unavailable.
//! Window consumers use the actual native frame callback and nonempty renderer submissions.
//! These tests do not change physical power settings or measure energy use or compositor scanout.

use super::*;
use crate::platform::{FAIL_NEXT_INITIAL_POWER_STATUS_QUERY, WindowsPlatformState};
use open_gpui::{
    App, FrameRequestSource, InteractiveElement as _, MouseButton, MouseDownEvent,
    NativeAppNotificationHandler, PlatformInput, PowerMode, RequestFrameOptions, Subscription,
    WindowHandle, WindowInitialPresentationStatus, div, fill, rgb,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::{
    Win32::{
        System::{
            Power::{GetSystemPowerStatus, POWERBROADCAST_SETTING, SYSTEM_POWER_STATUS},
            SystemServices::GUID_POWER_SAVING_STATUS,
        },
        UI::WindowsAndMessaging::{
            GA_ROOT, GetAncestor, GetParent, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND,
            PBT_POWERSETTINGCHANGE, WM_POWERBROADCAST,
        },
    },
    core::GUID,
};

#[repr(C)]
struct NativePowerSetting {
    setting: GUID,
    data_length: u32,
    data: [u8; 4],
}

impl NativePowerSetting {
    fn battery_saver(value: u32) -> Self {
        Self {
            setting: GUID_POWER_SAVING_STATUS,
            data_length: 4,
            data: value.to_ne_bytes(),
        }
    }
}

fn dispatch_power_setting(hwnd: HWND, setting: Option<&NativePowerSetting>) {
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(
        std::mem::offset_of!(NativePowerSetting, data),
        std::mem::offset_of!(POWERBROADCAST_SETTING, Data),
    );
    let lparam = setting.map_or(LPARAM(0), |setting| {
        LPARAM(std::ptr::from_ref(setting) as isize)
    });
    // SAFETY: The borrowed header and four payload bytes remain live until SendMessageW returns.
    // A null payload deliberately exercises the parser's documented no-op path.
    assert_eq!(
        unsafe {
            SendMessageW(
                hwnd,
                WM_POWERBROADCAST,
                Some(WPARAM(PBT_POWERSETTINGCHANGE as usize)),
                Some(lparam),
            )
        },
        LRESULT(1),
    );
}

fn dispatch_power_barrier(hwnd: HWND, event: u32) {
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_POWERBROADCAST, Some(WPARAM(event as usize)), None) },
        LRESULT(1),
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PowerModeEvent {
    Mode(usize, PowerMode),
    Sleep,
    Wake,
}

fn observe_power_mode(cx: &App, events: &Rc<RefCell<Vec<PowerModeEvent>>>) -> Vec<Subscription> {
    (0..2)
        .map(|subscriber| {
            let events = events.clone();
            cx.on_power_mode_change(move |cx| {
                events
                    .borrow_mut()
                    .push(PowerModeEvent::Mode(subscriber, cx.power_mode()));
            })
        })
        .collect()
}

fn power_mode_diagnostics(
    diagnostics: &open_gpui::NativeBoundaryDiagnosticsSnapshot,
) -> Vec<(u64, NativeCallbackKind, NativeBoundaryDisposition)> {
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|diagnostic| !matches!(
        diagnostic.disposition,
        NativeBoundaryDisposition::InvariantFailure(_)
    )));
    diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| diagnostic.target == NativeBoundaryTarget::Application)
        .filter_map(|diagnostic| {
            let NativeBoundaryKind::Callback(kind) = diagnostic.kind else {
                return None;
            };
            matches!(
                kind,
                NativeCallbackKind::PowerModeChanged
                    | NativeCallbackKind::SystemSleep
                    | NativeCallbackKind::SystemWake
            )
            .then_some((diagnostic.sequence, kind, diagnostic.disposition))
        })
        .collect()
}

struct InitialPowerStatusQueryFailureGuard(bool);

impl InitialPowerStatusQueryFailureGuard {
    fn new() -> Self {
        Self(FAIL_NEXT_INITIAL_POWER_STATUS_QUERY.with(|fail| fail.replace(true)))
    }
}

impl Drop for InitialPowerStatusQueryFailureGuard {
    fn drop(&mut self) {
        FAIL_NEXT_INITIAL_POWER_STATUS_QUERY.with(|fail| fail.set(self.0));
    }
}

#[test]
fn native_power_mode_recovers_after_initial_status_query_failure() {
    discard_stale_quit_messages();
    let platform = {
        let _query_failure = InitialPowerStatusQueryFailureGuard::new();
        let platform = WindowsPlatform::new(true).expect("failed-query native power-mode backend");
        assert!(!FAIL_NEXT_INITIAL_POWER_STATUS_QUERY.with(Cell::get));
        platform
    };
    let hwnd = platform.handle;
    assert!(!unsafe { IsWindowVisible(hwnd).as_bool() });
    assert_eq!(platform.power_mode(), PowerMode::Unknown);
    dispatch_power_setting(hwnd, None);
    assert_eq!(platform.power_mode(), PowerMode::Unknown);
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    assert_eq!(platform.power_mode(), PowerMode::LowPower);
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    assert_eq!(platform.power_mode(), PowerMode::HighPower);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "failed-query power-mode platform retirement",
        Duration::from_millis(5),
        || !unsafe { IsWindow(Some(hwnd)).as_bool() } && lifecycle.ole_uninitialize_count() == 1,
    );
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
}

#[test]
fn native_power_mode_initializes_and_delivers_busy_app_notifications_in_order() {
    discard_stale_quit_messages();
    assert_eq!(
        WindowsPlatformState::new().power_mode.get(),
        PowerMode::Unknown
    );
    let platform = Rc::new(WindowsPlatform::new(true).expect("native power-mode backend"));
    let hwnd = platform.handle;
    assert!(!unsafe { IsWindowVisible(hwnd).as_bool() });
    assert_eq!(unsafe { GetAncestor(hwnd, GA_ROOT) }, hwnd);
    assert!(unsafe { GetParent(hwnd) }.is_err());
    assert!(platform.power_saving_notification.borrow().is_none());
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_ok() {
        assert_eq!(
            platform.power_mode(),
            if status.SystemStatusFlag == 1 {
                PowerMode::LowPower
            } else {
                PowerMode::HighPower
            }
        );
    }

    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let notification = platform
        .power_saving_notification
        .borrow()
        .expect("App must register the actual platform HWND for battery-saver changes");
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "initial native battery-saver notification",
        Duration::from_millis(2),
        || platform.lifecycle_test_probe.power_saving_notifications() > 0,
    );
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "public initial HighPower query",
        || app.update_for_test(|cx| cx.power_mode()) == PowerMode::HighPower,
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut subscriptions = app.update_for_test(|cx| {
        let mut subscriptions = observe_power_mode(cx, &events);
        subscriptions.push(cx.on_system_sleep({
            let events = events.clone();
            move |_| events.borrow_mut().push(PowerModeEvent::Sleep)
        }));
        subscriptions.push(cx.on_system_wake({
            let events = events.clone();
            move |_| events.borrow_mut().push(PowerModeEvent::Wake)
        }));
        subscriptions
    });
    let cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let sequences = app.update_for_test(|cx| {
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
        assert_eq!(platform.power_mode(), PowerMode::LowPower);
        assert_eq!(cx.power_mode(), PowerMode::HighPower);
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
        dispatch_power_setting(hwnd, None);
        let mut invalid = NativePowerSetting::battery_saver(0);
        invalid.setting = GUID::zeroed();
        dispatch_power_setting(hwnd, Some(&invalid));
        invalid.setting = GUID_POWER_SAVING_STATUS;
        for length in [0, 1, 3, 5, u32::MAX] {
            invalid.data_length = length;
            dispatch_power_setting(hwnd, Some(&invalid));
        }
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(2)));
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(u32::MAX)));
        assert_eq!(platform.power_mode(), PowerMode::LowPower);
        dispatch_power_barrier(hwnd, PBT_APMSUSPEND);
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
        dispatch_power_barrier(hwnd, PBT_APMRESUMEAUTOMATIC);
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
        assert!(events.borrow().is_empty());
        let diagnostics = cx.native_boundary_diagnostics(cursor);
        assert!(power_mode_diagnostics(&diagnostics).is_empty());
        let pending = diagnostics
            .pending
            .iter()
            .map(|entry| {
                assert_eq!(entry.target, NativeBoundaryTarget::Application);
                (entry.sequence, entry.kind)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pending.iter().map(|entry| entry.1).collect::<Vec<_>>(),
            [
                NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged),
                NativeBoundaryKind::Callback(NativeCallbackKind::SystemSleep),
                NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged),
                NativeBoundaryKind::Callback(NativeCallbackKind::SystemWake),
                NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged),
            ]
        );
        assert!(pending.windows(2).all(|pair| pair[0].0 < pair[1].0));
        pending.into_iter().map(|entry| entry.0).collect::<Vec<_>>()
    });
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "ordered busy-App battery-saver observers",
        || events.borrow().len() == 8,
    );
    assert_eq!(
        events.borrow().as_slice(),
        &[
            PowerModeEvent::Mode(0, PowerMode::LowPower),
            PowerModeEvent::Mode(1, PowerMode::LowPower),
            PowerModeEvent::Sleep,
            PowerModeEvent::Mode(0, PowerMode::LowPower),
            PowerModeEvent::Mode(1, PowerMode::LowPower),
            PowerModeEvent::Wake,
            PowerModeEvent::Mode(0, PowerMode::LowPower),
            PowerModeEvent::Mode(1, PowerMode::LowPower),
        ]
    );
    let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(cursor));
    let delivered = power_mode_diagnostics(&diagnostics);
    assert_eq!(delivered.len(), sequences.len());
    for ((sequence, _, disposition), expected) in delivered.iter().zip(sequences) {
        assert_eq!(*sequence, expected);
        assert_eq!(
            *disposition,
            NativeBoundaryDisposition::Delivered { input_result: None }
        );
    }

    events.borrow_mut().clear();
    let cursor = diagnostics.cursor;
    let coalesced_sequences = app.update_for_test(|cx| {
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
        dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
        let diagnostics = cx.native_boundary_diagnostics(cursor);
        let pending = diagnostics
            .pending
            .iter()
            .filter(|entry| {
                entry.target == NativeBoundaryTarget::Application
                    && entry.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged)
            })
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 3);
        let sequences = pending
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>();
        assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(power_mode_diagnostics(&diagnostics).is_empty());
        assert!(events.borrow().is_empty());
        sequences
    });
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "coalesced public HighPower query",
        || events.borrow().len() == 2,
    );
    assert_eq!(
        events.borrow().as_slice(),
        &[
            PowerModeEvent::Mode(0, PowerMode::HighPower),
            PowerModeEvent::Mode(1, PowerMode::HighPower),
        ]
    );
    let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(cursor));
    let delivered = power_mode_diagnostics(&diagnostics);
    assert_eq!(delivered.len(), 3);
    for (index, successor) in coalesced_sequences.iter().copied().enumerate().skip(1) {
        assert_eq!(
            delivered[index - 1],
            (
                coalesced_sequences[index - 1],
                NativeCallbackKind::PowerModeChanged,
                NativeBoundaryDisposition::Coalesced {
                    into_sequence: successor
                },
            )
        );
    }
    assert_eq!(
        delivered[2],
        (
            coalesced_sequences[2],
            NativeCallbackKind::PowerModeChanged,
            NativeBoundaryDisposition::Delivered { input_result: None },
        )
    );
    drop(subscriptions.remove(1));
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "remaining power-mode subscription",
        || events.borrow().len() == 3,
    );
    assert_eq!(
        events.borrow()[2],
        PowerModeEvent::Mode(0, PowerMode::LowPower)
    );
    drop(app);
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    pump_messages_until_idle(
        &Rc::downgrade(&platform.inner),
        "late battery-saver notifications after App retirement",
    );
    assert_eq!(events.borrow().len(), 3);
    drop(subscriptions);
    let mut rebuilt =
        Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let rebuilt_events = Rc::new(RefCell::new(Vec::new()));
    let rebuilt_subscriptions = rebuilt.update_for_test(|cx| {
        assert_eq!(cx.power_mode(), PowerMode::LowPower);
        observe_power_mode(cx, &rebuilt_events)
    });
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "reconstructed App power-mode observers",
        || rebuilt_events.borrow().len() == 2,
    );
    assert_eq!(
        rebuilt_events.borrow().as_slice(),
        &[
            PowerModeEvent::Mode(0, PowerMode::HighPower),
            PowerModeEvent::Mode(1, PowerMode::HighPower),
        ]
    );
    assert_eq!(events.borrow().len(), 3);
    assert_eq!(
        *platform.power_saving_notification.borrow(),
        Some(notification)
    );
    let lifecycle = platform.lifecycle_test_probe.clone();
    drop(rebuilt_subscriptions);
    drop(rebuilt);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "native power-mode platform retirement",
        Duration::from_millis(5),
        || !unsafe { IsWindow(Some(hwnd)).as_bool() } && lifecycle.ole_uninitialize_count() == 1,
    );
    assert_eq!(lifecycle.power_saving_unregister_attempts(), 1);
    assert_eq!(lifecycle.suspend_resume_unregister_attempts(), 1);
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
}

thread_local! {
    static REENTRANT_POWER_PLATFORM: RefCell<Option<Rc<WindowsPlatform>>> = const { RefCell::new(None) };
}

struct ReentrantPowerPlatformGuard;

impl Drop for ReentrantPowerPlatformGuard {
    fn drop(&mut self) {
        REENTRANT_POWER_PLATFORM.with(|slot| slot.borrow_mut().take());
    }
}

#[test]
fn native_power_mode_reentrant_replacement_and_retry_release_exact_registration() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new(true).expect("reentrant power-mode backend"));
    let hwnd = platform.handle;
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    let before_registration = platform.lifecycle_test_probe.power_saving_notifications();
    platform.on_system_sleep(Arc::new(|| {}));
    platform.on_system_wake(Arc::new(|| {}));
    platform.on_power_mode_change(Arc::new(|| {}));
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "initial reentrant battery-saver notification",
        Duration::from_millis(2),
        || platform.lifecycle_test_probe.power_saving_notifications() > before_registration,
    );
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    let notification = *platform.power_saving_notification.borrow();
    assert!(notification.is_some());
    REENTRANT_POWER_PLATFORM.with(|slot| {
        assert!(slot.borrow_mut().replace(platform.clone()).is_none());
    });
    let reentry_guard = ReentrantPowerPlatformGuard;
    let original_capture = Arc::new(());
    let original_weak = Arc::downgrade(&original_capture);
    let replacement_capture = Arc::new(());
    let replacement_weak = Arc::downgrade(&replacement_capture);
    let original_calls = Arc::new(AtomicUsize::new(0));
    let replacement_calls = Arc::new(AtomicUsize::new(0));
    platform.on_power_mode_change(Arc::new({
        let original_calls = original_calls.clone();
        let replacement_calls = replacement_calls.clone();
        move || {
            let _keep_alive = &original_capture;
            original_calls.fetch_add(1, Ordering::Relaxed);
            let platform = REENTRANT_POWER_PLATFORM.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .expect("reentrant native platform")
                    .clone()
            });
            assert_eq!(platform.power_mode(), PowerMode::LowPower);
            let replacement: NativeAppNotificationHandler = Arc::new({
                let replacement_capture = replacement_capture.clone();
                let replacement_calls = replacement_calls.clone();
                move || {
                    let _keep_alive = &replacement_capture;
                    replacement_calls.fetch_add(1, Ordering::Relaxed);
                }
            });
            platform.on_power_mode_change(replacement);
            dispatch_power_setting(platform.handle, Some(&NativePowerSetting::battery_saver(0)));
            assert_eq!(platform.power_mode(), PowerMode::HighPower);
        }
    }));
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    assert_eq!(original_calls.load(Ordering::Relaxed), 1);
    assert_eq!(replacement_calls.load(Ordering::Relaxed), 1);
    assert!(original_weak.upgrade().is_none());
    assert!(replacement_weak.upgrade().is_some());
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    assert_eq!(original_calls.load(Ordering::Relaxed), 1);
    assert_eq!(replacement_calls.load(Ordering::Relaxed), 2);
    assert_eq!(*platform.power_saving_notification.borrow(), notification);
    drop(reentry_guard);

    let lifecycle = platform.lifecycle_test_probe.clone();
    let platform_inner = Rc::downgrade(&platform.inner);
    lifecycle.fail_next_power_saving_unregister();
    lifecycle.fail_next_platform_destroy();
    platform.begin_native_finalization();
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(platform.power_saving_notification.borrow().is_none());
    assert_eq!(
        platform
            .inner
            .native_retirement
            .borrow()
            .finalization
            .as_ref()
            .expect("failed unregistration must retain retirement resources")
            .resources
            .power_saving_notification,
        notification
    );
    let rejected_capture = Arc::new(());
    let rejected_weak = Arc::downgrade(&rejected_capture);
    let rejected_calls = Arc::new(AtomicUsize::new(0));
    platform.on_power_mode_change(Arc::new({
        let rejected_calls = rejected_calls.clone();
        move || {
            let _keep_alive = &rejected_capture;
            rejected_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    assert!(platform.power_saving_notification.borrow().is_none());
    assert!(rejected_weak.upgrade().is_none());
    assert!(replacement_weak.upgrade().is_some());
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    assert_eq!(platform.power_mode(), PowerMode::LowPower);
    assert_eq!(original_calls.load(Ordering::Relaxed), 1);
    assert_eq!(replacement_calls.load(Ordering::Relaxed), 2);
    assert_eq!(rejected_calls.load(Ordering::Relaxed), 0);
    drop(platform);
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(lifecycle.suspend_resume_unregister_attempts(), 1);
    assert_eq!(lifecycle.power_saving_unregister_attempts(), 1);
    assert_eq!(lifecycle.platform_destroy_attempts(), 0);
    assert_eq!(lifecycle.ole_uninitialize_count(), 0);
    assert!(replacement_weak.upgrade().is_some());
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    assert_eq!(original_calls.load(Ordering::Relaxed), 1);
    assert_eq!(replacement_calls.load(Ordering::Relaxed), 2);
    assert_eq!(rejected_calls.load(Ordering::Relaxed), 0);
    pump_messages_until_with_delay(
        &platform_inner,
        "retryable exact power-setting retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && lifecycle.ole_uninitialize_count() == 1
                && platform_inner.upgrade().is_none()
                && replacement_weak.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle.suspend_resume_unregister_attempts(), 1);
    assert_eq!(lifecycle.power_saving_unregister_attempts(), 2);
    assert_eq!(lifecycle.platform_destroy_attempts(), 2);
    assert_eq!(replacement_calls.load(Ordering::Relaxed), 2);
}

#[derive(Clone, Copy, Debug)]
struct NativePowerModePaint {
    source: u32,
    previous_revision: u64,
}

struct NativePowerModeRoot {
    source: u32,
    paints: Rc<RefCell<Vec<NativePowerModePaint>>>,
}

impl Render for NativePowerModeRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source;
        let paints = self.paints.clone();
        div()
            .size_full()
            .on_mouse_down(MouseButton::Left, |_, window, _| window.refresh())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window.paint_quad(fill(bounds, rgb(0x284060 + source)));
                        paints.borrow_mut().push(NativePowerModePaint {
                            source,
                            previous_revision: window.rendered_frame_revision(),
                        });
                    },
                )
                .size_full(),
            )
    }
}

struct NativePowerModeWindow {
    handle: WindowHandle<NativePowerModeRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    paints: Rc<RefCell<Vec<NativePowerModePaint>>>,
    signals: Rc<RefCell<Vec<RequestFrameOptions>>>,
    callback_runs: Rc<RefCell<Vec<u32>>>,
}

impl NativePowerModeWindow {
    fn open(
        app: &mut Application,
        platform: &WindowsPlatform,
        low_power_frame_interval: Option<Duration>,
        x_offset: f32,
    ) -> Self {
        let paints = Rc::new(RefCell::new(Vec::new()));
        let handle = app
            .update_for_test(|cx| {
                let mut bounds = Bounds::centered(None, size(px(240.), px(160.)), cx);
                bounds.origin.x += px(x_offset);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        focus_on_appearing: false,
                        show: true,
                        inactive_frame_interval: None,
                        low_power_frame_interval,
                        thermal_frame_interval: None,
                        ..WindowOptions::default()
                    },
                    |_, cx| {
                        cx.new(|_| NativePowerModeRoot {
                            source: 0,
                            paints: paints.clone(),
                        })
                    },
                )
            })
            .expect("native power-mode window consumer");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("power-mode consumer's exact HWND registration");
        let native = platform
            .window_from_hwnd(registration.as_raw())
            .expect("power-mode consumer's live native owner");
        assert_eq!(native.registration, registration);
        let signals = Rc::new(RefCell::new(Vec::new()));
        let callback_signals = signals.clone();
        let request_frame = native
            .state
            .callbacks
            .request_frame
            .take()
            .expect("power-mode consumer's production frame callback");
        native
            .state
            .callbacks
            .request_frame
            .set(Some(Rc::new(move |options| {
                callback_signals.borrow_mut().push(options);
                request_frame(options);
            })));
        Self {
            handle,
            registration,
            native,
            paints,
            signals,
            callback_runs: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn wait_for_submission(
        &self,
        platform: &WindowsPlatform,
        app: &mut Application,
        source: u32,
        after: u64,
    ) -> u64 {
        let mut submitted = None;
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            "power-mode consumer's exact source and nonempty Submitted frame",
            Duration::from_millis(5),
            || {
                submitted = app
                    .update_for_test(|cx| {
                        self.handle.update(cx, |_, window, _| {
                            let facts = window.presentation_facts();
                            let attempt = facts.latest_present_attempt?;
                            let paint = *self.paints.borrow().last()?;
                            (paint.source == source
                                && paint.previous_revision.checked_add(1)
                                    == Some(attempt.generation)
                                && attempt.generation > after
                                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                                && attempt.contained_valid_primitives
                                && facts.initial_presentation
                                    == WindowInitialPresentationStatus::Completed
                                && facts.native_visible
                                && facts.frame_accepted_generation == Some(attempt.generation)
                                && facts.present_submitted_generation == Some(attempt.generation)
                                && facts.non_empty_presented_generation == Some(attempt.generation)
                                && window.rendered_frame_revision() == attempt.generation
                                && window.submitted_frame_diagnostics_for_test().is_some_and(
                                    |diagnostics| diagnostics.generation() == attempt.generation,
                                ))
                            .then_some(attempt.generation)
                        })
                    })
                    .expect("power-mode consumer must survive native submission");
                submitted.is_some()
            },
        );
        submitted.expect("exact Submitted power-mode source generation")
    }

    fn queue_source(&self, app: &mut Application, source: u32) -> std::rc::Weak<()> {
        let capture = Rc::new(());
        let weak = Rc::downgrade(&capture);
        app.update_for_test(|cx| {
            self.handle.update(cx, |root, window, cx| {
                root.source = source;
                cx.notify();
                window.refresh();
                let callback_runs = self.callback_runs.clone();
                window.on_next_frame(move |window, _| {
                    let _keep_alive = &capture;
                    callback_runs.borrow_mut().push(source);
                    window.refresh();
                });
            })
        })
        .expect("power-mode consumer must accept public frame demand");
        weak
    }

    fn dispatch_paint(&self, app: &mut Application, platform: &WindowsPlatform) {
        let _ = self
            .native
            .dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(40.), px(40.)),
                click_count: 1,
                ..Default::default()
            }));
        let at = Instant::now();
        let signal_cursor = self.signals.borrow().len();
        let cursor = app.update_for_test(|cx| {
            let cursor = cx
                .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
                .cursor;
            self.native.state.frame_signal.take();
            platform.raw_window_handles.record_frame_signal(
                self.registration,
                Some(at),
                FrameRequestSource::NativeCallback,
            );
            unsafe { SendMessageW(self.registration.as_raw(), WM_PAINT, None, None) };
            cursor
        });
        assert!(
            self.signals.borrow()[signal_cursor..]
                .iter()
                .any(|options| {
                    options.signal_at == Some(at)
                        && options.signal_source == FrameRequestSource::NativeCallback
                        && !options.require_presentation
                        && !options.force_render
                })
        );
        pump_messages_until(
            &Rc::downgrade(&platform.inner),
            "power-mode consumer's actual native frame delivery",
            || {
                app.update_for_test(|cx| {
                    let diagnostics = cx.native_boundary_diagnostics(cursor);
                    assert!(diagnostics.terminal.iter().all(|entry| !matches!(
                        entry.disposition,
                        NativeBoundaryDisposition::InvariantFailure(_)
                    )));
                    diagnostics.terminal.iter().any(|entry| {
                        entry.target == NativeBoundaryTarget::Window(self.handle.window_id())
                            && entry.kind
                                == NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
                            && matches!(
                                entry.disposition,
                                NativeBoundaryDisposition::Delivered { input_result: None }
                            )
                    })
                })
            },
        );
    }

    fn assert_deferred(&self, app: &mut Application, generation: u64, source: u32) {
        app.update_for_test(|cx| {
            self.handle.update(cx, |root, window, _| {
                assert_eq!(root.source, source);
                assert_eq!(
                    window.rendered_frame_revision(),
                    generation,
                    "deferred source {source}; facts={:?}; paints={:?}",
                    window.presentation_facts(),
                    self.paints.borrow()
                );
                assert_eq!(
                    window.presentation_facts().non_empty_presented_generation,
                    Some(generation)
                );
                assert!(
                    self.paints
                        .borrow()
                        .iter()
                        .all(|paint| paint.source != source)
                );
                assert!(!self.callback_runs.borrow().contains(&source));
            })
        })
        .expect("deferred power-mode consumer must stay live");
    }

    fn retire(self, app: &mut Application, platform: &WindowsPlatform) {
        let hwnd = self.registration.as_raw();
        let native_lifetime = Rc::downgrade(&self.native);
        app.update_for_test(|cx| {
            self.handle
                .update(cx, |_, window, cx| window.remove_window(cx))
        })
        .expect("power-mode consumer must retire its exact WindowId");
        drop(self);
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            "power-mode consumer's exact HWND retirement",
            Duration::from_millis(5),
            || {
                !unsafe { IsWindow(Some(hwnd)).as_bool() }
                    && !platform
                        .raw_window_handles
                        .read()
                        .iter()
                        .any(|entry| entry.as_raw() == hwnd)
                    && native_lifetime.upgrade().is_none()
                    && platform.inner.native_retirement_is_settled()
            },
        );
    }
}

#[test]
fn native_low_power_configuration_defers_callbacks_and_recovers_submitted_source() {
    discard_stale_quit_messages();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native low-power frame backend"));
    let hwnd = platform.handle;
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "initial low-power frame registration",
        Duration::from_millis(2),
        || platform.lifecycle_test_probe.power_saving_notifications() > 0,
    );
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "public LowPower mode before window construction",
        || app.update_for_test(|cx| cx.power_mode()) == PowerMode::LowPower,
    );
    let limited =
        NativePowerModeWindow::open(&mut app, &platform, Some(Duration::from_secs(3600)), -130.);
    let unlimited = NativePowerModeWindow::open(&mut app, &platform, None, 130.);
    limited.wait_for_submission(&platform, &mut app, 0, 0);
    unlimited.wait_for_submission(&platform, &mut app, 0, 0);
    pump_messages_until_idle(
        &Rc::downgrade(&platform.inner),
        "initial low-power window submissions",
    );
    let initial_limited = limited.wait_for_submission(&platform, &mut app, 0, 0);
    let initial_unlimited = unlimited.wait_for_submission(&platform, &mut app, 0, 0);
    #[cfg(feature = "profiler")]
    let mut journal = app.update_for_test(|cx| cx.foreground_journal().collector());

    let limited_capture = limited.queue_source(&mut app, 1);
    let unlimited_capture = unlimited.queue_source(&mut app, 1);
    limited.dispatch_paint(&mut app, &platform);
    limited.assert_deferred(&mut app, initial_limited, 1);
    assert!(limited_capture.upgrade().is_some());
    #[cfg(feature = "profiler")]
    {
        use open_gpui::profiler::journal::{
            ForegroundJournalEntry, FrameSkipReason, IntervalBoundary,
        };
        let drained = journal.collect_unseen();
        assert_eq!(drained.lost, 0);
        assert!(drained.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::FrameSkipped(skipped))
                if skipped.window_id == limited.handle.window_id()
                    && skipped.reason == FrameSkipReason::LowPowerFrameRateLimit
        )));
    }
    unlimited.dispatch_paint(&mut app, &platform);
    unlimited.wait_for_submission(&platform, &mut app, 1, initial_unlimited);
    assert_eq!(unlimited.callback_runs.borrow().as_slice(), &[1]);
    assert!(unlimited_capture.upgrade().is_none());
    limited.assert_deferred(&mut app, initial_limited, 1);

    limited.native.request_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: false,
        signal_at: Some(Instant::now()),
        signal_source: FrameRequestSource::LocalSchedule,
    });
    let mandatory_generation = limited.wait_for_submission(&platform, &mut app, 1, initial_limited);
    assert!(limited_capture.upgrade().is_none());
    assert_eq!(limited.callback_runs.borrow().as_slice(), &[1]);
    assert_eq!(
        app.update_for_test(|cx| cx.power_mode()),
        PowerMode::LowPower
    );
    assert!(limited.signals.borrow().iter().any(|options| {
        options.require_presentation && options.signal_source == FrameRequestSource::LocalSchedule
    }));

    let recovered_capture = limited.queue_source(&mut app, 2);
    limited.dispatch_paint(&mut app, &platform);
    limited.assert_deferred(&mut app, mandatory_generation, 2);
    assert!(recovered_capture.upgrade().is_some());
    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(0)));
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "public HighPower recovery releases deferred demand",
        || app.update_for_test(|cx| cx.power_mode()) == PowerMode::HighPower,
    );
    limited.dispatch_paint(&mut app, &platform);
    let recovered_generation =
        limited.wait_for_submission(&platform, &mut app, 2, mandatory_generation);
    assert!(recovered_generation > mandatory_generation);
    assert_eq!(limited.callback_runs.borrow().as_slice(), &[1, 2]);
    assert!(recovered_capture.upgrade().is_none());

    dispatch_power_setting(hwnd, Some(&NativePowerSetting::battery_saver(1)));
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "public LowPower mode before pending-demand retirement",
        || app.update_for_test(|cx| cx.power_mode()) == PowerMode::LowPower,
    );
    let retired_capture = limited.queue_source(&mut app, 3);
    limited.dispatch_paint(&mut app, &platform);
    limited.assert_deferred(&mut app, recovered_generation, 3);
    assert!(retired_capture.upgrade().is_some());
    let callback_runs = limited.callback_runs.clone();
    limited.retire(&mut app, &platform);
    assert!(retired_capture.upgrade().is_none());
    assert_eq!(callback_runs.borrow().as_slice(), &[1, 2]);
    unlimited.retire(&mut app, &platform);
    let lifecycle = platform.lifecycle_test_probe.clone();
    drop(app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "low-power frame platform retirement",
        Duration::from_millis(5),
        || !unsafe { IsWindow(Some(hwnd)).as_bool() } && lifecycle.ole_uninitialize_count() == 1,
    );
    assert_eq!(lifecycle.power_saving_unregister_attempts(), 1);
    assert_eq!(callback_runs.borrow().as_slice(), &[1, 2]);
}
