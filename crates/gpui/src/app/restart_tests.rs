//! Core restart arguments from Zed 715076597926d566d560b30b806749e1a2166080,
//! retained at fixed target 635026fabc38440fce5e1767d125026a17b12012.

use super::Application;
use crate::{App, Platform, TestAppContext, TestPlatform, VisualTestPlatform};
use futures::channel::oneshot;
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    path::PathBuf,
    rc::Rc,
};

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

fn application(cx: &TestAppContext) -> (Application, Rc<TestPlatform>) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    (Application::with_platform(platform.clone()), platform)
}

#[crate::test]
fn production_application_defaults_to_no_restart_path_or_arguments(cx: &mut TestAppContext) {
    let (mut application, platform) = application(cx);
    let (sender, mut request) = oneshot::channel();
    platform.expect_restart.replace(Some(sender));

    application.update_for_test(|app| {
        assert!(app.restart_path.is_none());
        assert!(app.restart_arguments.is_empty());
        app.restart();
    });

    assert_eq!(request.try_recv().unwrap(), Some((None, Vec::new())));
    application.update_for_test(App::shutdown);
}

#[crate::test]
fn builder_replaces_arguments_on_the_existing_application(cx: &mut TestAppContext) {
    let path = PathBuf::from("selected executable");
    cx.update(|app| app.set_restart_path(path.clone()));
    let arguments = vec![OsString::from("--replacement"), OsString::new()];
    let application = Application(cx.app.clone())
        .with_restart_arguments(vec![OsString::from("--discarded")])
        .with_restart_arguments(arguments.clone());
    assert!(Rc::ptr_eq(&application.0, &cx.app));
    let mut request = cx.expect_restart();

    cx.update(App::restart);

    assert_eq!(request.try_recv().unwrap(), Some((Some(path), arguments)),);
}

#[crate::test]
fn builder_accepts_an_empty_replacement(cx: &mut TestAppContext) {
    let _application = Application(cx.app.clone())
        .with_restart_arguments(vec![OsString::from("--discarded")])
        .with_restart_arguments(Vec::new());
    let mut request = cx.expect_restart();

    cx.update(App::restart);

    assert_eq!(request.try_recv().unwrap(), Some((None, Vec::new())));
}

#[crate::test]
fn restart_preserves_owned_native_path_and_ordered_arguments(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let native = OsString::from_vec(b"restart data/\xff".to_vec());
    #[cfg(windows)]
    let native = OsString::from_wide(&[0x72, 0x65, 0xd800, 0x20, 0xdcff, 0x5c]);
    #[cfg(not(any(unix, windows)))]
    let native = OsString::from("restart data/🦀");

    let arguments = vec![
        OsString::new(),
        OsString::from("--user-data-dir"),
        native.clone(),
        OsString::from("space and\ttab\nline"),
        OsString::from("quote\" apostrophe' $value `command` ; &"),
        OsString::from("trailing\\"),
        OsString::from("🦀"),
        OsString::new(),
    ];
    let expected = arguments.clone();
    let path = PathBuf::from(native);
    let _application = Application(cx.app.clone()).with_restart_arguments(arguments);
    let mut request = cx.expect_restart();

    cx.update(|app| {
        app.set_restart_path(path.clone());
        app.restart();
    });

    let (actual_path, actual_arguments) = request.try_recv().unwrap().unwrap();
    assert_eq!(actual_path.as_ref(), Some(&path));
    assert_eq!(actual_arguments, expected);
    #[cfg(unix)]
    {
        assert_eq!(actual_arguments[2].as_bytes(), b"restart data/\xff");
        assert_eq!(
            actual_path.unwrap().as_os_str().as_bytes(),
            b"restart data/\xff"
        );
    }
    #[cfg(windows)]
    {
        let expected = [0x72, 0x65, 0xd800, 0x20, 0xdcff, 0x5c];
        assert_eq!(
            actual_arguments[2].encode_wide().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            actual_path
                .unwrap()
                .as_os_str()
                .encode_wide()
                .collect::<Vec<_>>(),
            expected,
        );
    }
}

#[crate::test]
fn restart_observer_can_replace_the_path_before_platform_delivery(cx: &mut TestAppContext) {
    let arguments = vec![OsString::from("--retained")];
    let _application = Application(cx.app.clone()).with_restart_arguments(arguments.clone());
    let initial_path = PathBuf::from("initial executable");
    let observer_path = PathBuf::from("observer executable");
    let request = Rc::new(RefCell::new(cx.expect_restart()));
    let _subscription = cx.read(|app| {
        app.on_app_restart({
            let request = request.clone();
            let initial_path = initial_path.clone();
            let observer_path = observer_path.clone();
            let arguments = arguments.clone();
            move |app| {
                assert_eq!(request.borrow_mut().try_recv().unwrap(), None);
                assert_eq!(app.restart_path.as_ref(), Some(&initial_path));
                assert_eq!(app.restart_arguments, arguments);
                app.set_restart_path(observer_path.clone());
            }
        })
    });

    cx.update(|app| {
        app.set_restart_path(initial_path);
        app.restart();
    });

    assert_eq!(
        request.borrow_mut().try_recv().unwrap(),
        Some((Some(observer_path), arguments)),
    );
}

#[crate::test]
fn restart_consumes_path_and_arguments_once_and_keeps_observers(cx: &mut TestAppContext) {
    let path = PathBuf::from("replacement executable");
    let arguments = vec![OsString::from("--once")];
    let _application = Application(cx.app.clone()).with_restart_arguments(arguments.clone());
    let observed = Rc::new(Cell::new(0));
    let _subscription = cx.read(|app| {
        app.on_app_restart({
            let observed = observed.clone();
            move |_| observed.set(observed.get() + 1)
        })
    });
    let mut first = cx.expect_restart();
    cx.update(|app| {
        app.set_restart_path(path.clone());
        app.restart();
        assert!(app.restart_path.is_none());
        assert!(app.restart_arguments.is_empty());
    });
    assert_eq!(first.try_recv().unwrap(), Some((Some(path), arguments)));

    let mut second = cx.expect_restart();
    cx.update(App::restart);
    assert_eq!(second.try_recv().unwrap(), Some((None, Vec::new())));
    assert_eq!(observed.get(), 2);
}

#[crate::test]
fn restart_configuration_and_delivery_are_isolated_between_apps(cx: &mut TestAppContext) {
    let other = cx.new_app();
    let first_arguments = vec![OsString::from("--first")];
    let second_arguments = vec![OsString::from("--second"), OsString::new()];
    let _first = Application(cx.app.clone()).with_restart_arguments(first_arguments.clone());
    let _second = Application(other.app.clone()).with_restart_arguments(second_arguments.clone());
    let first_path = PathBuf::from("first executable");
    let second_path = PathBuf::from("second executable");
    cx.update(|app| app.set_restart_path(first_path.clone()));
    other.update(|app| app.set_restart_path(second_path.clone()));
    let mut first = cx.expect_restart();
    let mut second = other.expect_restart();

    cx.update(App::restart);

    assert_eq!(
        first.try_recv().unwrap(),
        Some((Some(first_path), first_arguments))
    );
    assert_eq!(second.try_recv().unwrap(), None);
    other.read(|app| {
        assert_eq!(app.restart_path.as_ref(), Some(&second_path));
        assert_eq!(app.restart_arguments, second_arguments);
    });
    other.update(App::restart);
    assert_eq!(
        second.try_recv().unwrap(),
        Some((Some(second_path), second_arguments)),
    );
    other.quit();
}

#[crate::test]
fn embedded_delayed_launch_retains_restart_configuration_and_host_ownership(
    cx: &mut TestAppContext,
) {
    let (application, platform) = application(cx);
    let lifetime = Rc::downgrade(&application.0);
    let arguments = vec![OsString::from("--embedded"), OsString::new()];
    let path = PathBuf::from("embedded executable");
    let application = application.with_restart_arguments(arguments.clone());
    let (sender, mut request) = oneshot::channel();
    platform.expect_restart.replace(Some(sender));
    platform.defer_next_launch();
    let launched = Rc::new(Cell::new(false));
    let handle = application.run_embedded({
        let launched = launched.clone();
        let path = path.clone();
        move |app| {
            app.set_restart_path(path);
            app.restart();
            launched.set(true);
        }
    });
    assert!(!launched.get());
    assert_eq!(request.try_recv().unwrap(), None);
    cx.run_until_parked();
    assert!(launched.get());
    assert_eq!(request.try_recv().unwrap(), Some((Some(path), arguments)));
    assert!(lifetime.upgrade().is_some());

    let (sender, mut second) = oneshot::channel();
    platform.expect_restart.replace(Some(sender));
    handle.update(App::restart);
    assert_eq!(second.try_recv().unwrap(), Some((None, Vec::new())));
    handle.update(App::shutdown);
    cx.run_until_parked();
    drop(handle);
    assert!(lifetime.upgrade().is_none());
}

#[crate::test]
fn visual_test_restart_does_not_forward_to_its_backend(cx: &mut TestAppContext) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    let visual = VisualTestPlatform::new(platform.clone(), 0);
    let (sender, mut request) = oneshot::channel();
    platform.expect_restart.replace(Some(sender));

    visual.restart(
        Some(PathBuf::from("unused executable")),
        vec![OsString::from("--unused")],
    );
    visual.dispatcher().run_until_parked();

    assert_eq!(request.try_recv().unwrap(), None);
}
