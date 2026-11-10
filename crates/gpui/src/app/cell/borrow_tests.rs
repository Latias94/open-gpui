use super::*;
use crate::TestAppContext;

#[crate::test]
fn app_borrow_diagnostics(cx: &mut TestAppContext) {
    let app = cx.app.clone();
    let first_line = line!() + 1;
    let first = app.borrow();
    let second = app.borrow();
    drop(first);
    assert!(app.try_borrow_mut().is_err());
    let attempted_line = line!() + 1;
    let panic = catch_unwind(AssertUnwindSafe(|| drop(app.borrow_mut())));
    let message = *panic
        .expect_err("conflicting borrow")
        .downcast::<String>()
        .unwrap();
    assert!(message.contains(&format!(
        "mutable App borrow at {}:{attempted_line}:",
        file!()
    )));
    assert!(message.contains(&format!(
        "shared borrow group began at {}:{first_line}:",
        file!()
    )));
    drop(second);

    let origin_line = line!() + 2;
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _borrow = app.try_borrow_mut().expect("initial borrow");
        drop(app.borrow());
    }));
    let message = *panic
        .expect_err("conflicting borrow")
        .downcast::<String>()
        .unwrap();
    assert!(message.contains("attempted shared App borrow"));
    assert!(message.contains(&format!(
        "mutable borrow group began at {}:{origin_line}:",
        file!()
    )));

    let next_line = line!() + 1;
    let _borrow = app.borrow();
    assert_eq!(
        app.borrow_origin.get().expect("new origin").0.line(),
        next_line
    );
}

#[crate::test]
fn internal_lifecycle_borrow_does_not_report_a_retired_public_origin(cx: &mut TestAppContext) {
    let app = cx.app.clone();
    drop(app.borrow());
    let internal = app.app.borrow_mut();
    let attempted_line = line!() + 1;
    let error = app.try_borrow_mut().err().expect("conflicting borrow");
    let message = error.to_string();
    assert!(message.contains(&format!(
        "mutable App borrow at {}:{attempted_line}:",
        file!()
    )));
    assert!(message.contains("origin unavailable"));
    drop(internal);
    assert_eq!(app.app_borrow_depth.get(), 0);

    let internal = app.app.borrow();
    let origin_line = line!() + 1;
    let public = app.borrow();
    let error = app
        .try_borrow_mut()
        .err()
        .expect("conflicting shared group");
    assert!(error.to_string().contains(&format!(
        "shared borrow group began at {}:{origin_line}:",
        file!()
    )));
    drop(public);
    assert!(
        app.try_borrow_mut()
            .err()
            .unwrap()
            .to_string()
            .contains("origin unavailable")
    );
    drop(internal);
}
