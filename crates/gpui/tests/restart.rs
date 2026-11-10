//! Public consumers of Zed 715076597926d566d560b30b806749e1a2166080,
//! retained at fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! Test-platform delivery does not qualify native process relaunch.

use open_gpui::{App, Application, Platform};
use std::{ffi::OsString, path::PathBuf};

#[test]
fn restart_api_is_public_without_test_support() {
    let _: fn(Application, Vec<OsString>) -> Application = Application::with_restart_arguments;
    let _: fn(&mut App) = App::restart;
    let _: fn(&mut App, PathBuf) = App::set_restart_path;
    let _: fn(&dyn Platform, Option<PathBuf>, Vec<OsString>) = <dyn Platform>::restart;
}

#[cfg(feature = "test-support")]
mod test_contexts {
    use super::*;
    use open_gpui::TestAppContext;
    use std::{cell::RefCell, rc::Rc};

    #[open_gpui::test]
    fn public_default_restart_receives_no_executable_or_arguments(cx: &mut TestAppContext) {
        let mut request = cx.expect_restart();

        cx.update(App::restart);

        assert_eq!(request.try_recv().unwrap(), Some((None, Vec::new())));
    }

    #[open_gpui::test]
    fn public_restart_observers_select_the_executable_before_one_shot_delivery(
        cx: &mut TestAppContext,
    ) {
        let request = Rc::new(RefCell::new(cx.expect_restart()));
        let path = PathBuf::from("observer selected executable");
        let subscription = cx.read(|app| {
            app.on_app_restart({
                let request = request.clone();
                let path = path.clone();
                move |app| {
                    assert_eq!(request.borrow_mut().try_recv().unwrap(), None);
                    app.set_restart_path(path.clone());
                }
            })
        });

        cx.update(|app| {
            app.set_restart_path(PathBuf::from("superseded executable"));
            app.restart();
        });
        assert_eq!(
            request.borrow_mut().try_recv().unwrap(),
            Some((Some(path), Vec::new())),
        );

        drop(subscription);
        let mut second = cx.expect_restart();
        cx.update(App::restart);
        assert_eq!(second.try_recv().unwrap(), Some((None, Vec::new())));
    }

    #[open_gpui::test]
    fn public_restart_requests_are_isolated_between_applications(cx: &mut TestAppContext) {
        let other = cx.new_app();
        let first_path = PathBuf::from("first executable");
        let second_path = PathBuf::from("second executable");
        cx.update(|app| app.set_restart_path(first_path.clone()));
        other.update(|app| app.set_restart_path(second_path.clone()));
        let mut first = cx.expect_restart();
        let mut second = other.expect_restart();

        other.update(App::restart);

        assert_eq!(first.try_recv().unwrap(), None);
        assert_eq!(
            second.try_recv().unwrap(),
            Some((Some(second_path), Vec::new()))
        );
        cx.update(App::restart);
        assert_eq!(
            first.try_recv().unwrap(),
            Some((Some(first_path), Vec::new()))
        );
        other.quit();
    }
}
