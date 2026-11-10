//! Independent public consumers of credential tasks and Zed's macOS-only
//! 79fe6a645f API at fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! Runtime cases use bench-support without test-support or native secret storage.

#[cfg(feature = "bench-support")]
mod runtime {
    use open_gpui::{
        App, Application, ApplicationHandle, Global, NoopTextSystem, QuitMode, Result, Task,
        bench_platform,
    };
    use std::{
        cell::Cell,
        future::Future,
        pin::pin,
        rc::Rc,
        sync::Arc,
        task::{Context as TaskContext, Poll, Waker},
    };

    fn application() -> ApplicationHandle {
        Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem)))
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|_| {})
    }

    fn ready<T: 'static>(task: Task<T>) -> T {
        let mut task = pin!(task);
        match task
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("the credential stub must return a ready task"),
        }
    }

    #[test]
    fn downstream_app_and_platform_credentials_execute_ready_headless_stubs() {
        let platform = bench_platform(None, Arc::new(NoopTextSystem));
        let app = Application::with_platform(Rc::clone(&platform))
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|_| {});
        let (write, read, delete) = app.update(|app| {
            let app: &App = app;
            let url = String::from("open-gpui-test://consumer-app-stubs");
            let username = String::from("fixture-account");
            let password = vec![0, 0xff, 0x65];
            let write: Task<Result<()>> = app.write_credentials(&url, &username, &password);
            let read: Task<Result<Option<(String, Vec<u8>)>>> = app.read_credentials(&url);
            let delete: Task<Result<()>> = app.delete_credentials(&url);
            (write, read, delete)
        });
        assert!(write.is_ready(), "the App stub write must be ready");
        assert!(read.is_ready(), "the App stub read must be ready");
        assert!(delete.is_ready(), "the App stub delete must be ready");
        assert!(ready(write).is_ok(), "the App stub write must succeed");
        assert!(
            matches!(ready(read), Ok(None)),
            "the App stub must remain empty after a successful stub write"
        );
        assert!(ready(delete).is_ok(), "the App stub delete must succeed");

        let (write, read, delete) = {
            let url = String::from("open-gpui-test://consumer-platform-stubs");
            let username = String::from("fixture-account");
            let password = vec![0xff, 0, 0x66];
            (
                platform.write_credentials(&url, &username, &password),
                platform.read_credentials(&url),
                platform.delete_credentials(&url),
            )
        };
        assert!(write.is_ready(), "the Platform stub write must be ready");
        assert!(read.is_ready(), "the Platform stub read must be ready");
        assert!(delete.is_ready(), "the Platform stub delete must be ready");
        assert!(ready(write).is_ok(), "the Platform stub write must succeed");
        assert!(
            matches!(ready(read), Ok(None)),
            "the Platform stub must remain empty after a successful stub write"
        );
        assert!(
            ready(delete).is_ok(),
            "the Platform stub delete must succeed"
        );
        app.update(App::shutdown);
    }

    struct AppOwner(Rc<Cell<usize>>);

    impl Global for AppOwner {}

    impl Drop for AppOwner {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn downstream_credential_tasks_and_async_context_do_not_retain_the_app() {
        let app = application();
        let app_drops = Rc::new(Cell::new(0));
        app.update(|app| app.set_global(AppOwner(Rc::clone(&app_drops))));
        let async_app = app.to_async();
        let (write, read, delete) = async_app.update(|app| {
            let url = String::from("open-gpui-test://consumer-credential-lifetime");
            let username = String::from("fixture-account");
            let password = vec![0, 0xff, 0x67];
            (
                app.write_credentials(&url, &username, &password),
                app.read_credentials(&url),
                app.delete_credentials(&url),
            )
        });
        #[cfg(target_os = "macos")]
        let add_only = async_app.update(|app| {
            let url = String::from("open-gpui-test://consumer-add-only-lifetime");
            let username = String::from("fixture-account");
            let password = vec![0xff, 0, 0x68];
            app.write_credentials_if_not_exists(&url, &username, &password)
        });

        assert!(app_drops.get() == 0, "the live app must own its global");
        app.update(App::shutdown);
        drop(app);
        assert!(
            app_drops.get() == 1,
            "credential tasks and AsyncApp must not keep the app alive"
        );
        assert!(ready(write).is_ok(), "the owned stub write must complete");
        assert!(
            matches!(ready(read), Ok(None)),
            "the owned stub read must complete after app release"
        );
        assert!(ready(delete).is_ok(), "the owned stub delete must complete");
        #[cfg(target_os = "macos")]
        assert_unsupported(add_only);
        drop(async_app);
        assert!(
            app_drops.get() == 1,
            "the app global must drop exactly once"
        );
    }

    #[cfg(target_os = "macos")]
    fn assert_unsupported(task: Task<Result<bool>>) {
        assert!(task.is_ready(), "the unsupported default must be ready");
        match ready(task) {
            Err(error) => assert!(
                error.to_string() == "this platform does not support atomic credential writes",
                "the default must preserve the official unsupported error"
            ),
            Ok(_) => panic!("an unsupported platform must not report an insertion or duplicate"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn downstream_add_only_default_is_unsupported_through_platform_and_shared_app() {
        let platform = bench_platform(None, Arc::new(NoopTextSystem));
        let app = Application::with_platform(Rc::clone(&platform))
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|_| {});
        let direct: Task<Result<bool>> = {
            let url = String::from("open-gpui-test://consumer-platform-add-only-default");
            let username = String::from("fixture-account");
            let password = vec![0, 0xff, 0x69];
            assert!(
                ready(platform.write_credentials(&url, &username, &password)).is_ok(),
                "the ordinary platform stub write must succeed"
            );
            platform.write_credentials_if_not_exists(&url, &username, &password)
        };
        let delegated: Task<Result<bool>> = app.update(|app| {
            let app: &App = app;
            let url = String::from("open-gpui-test://consumer-app-add-only-default");
            let username = String::from("fixture-account");
            let password = vec![0xff, 0, 0x6a];
            assert!(
                ready(app.write_credentials(&url, &username, &password)).is_ok(),
                "the ordinary App stub write must succeed"
            );
            app.write_credentials_if_not_exists(&url, &username, &password)
        });
        app.update(App::shutdown);
        drop(app);
        drop(platform);

        assert_unsupported(direct);
        assert_unsupported(delegated);
    }
}

#[cfg(target_os = "macos")]
mod keychain_errors {
    use open_gpui::Result;
    use open_gpui_apple::keychain::{KeychainError, KeychainOperation};

    #[test]
    fn downstream_constructed_keychain_security_errors_keep_operation_and_status() {
        for (operation, status) in [
            (KeychainOperation::Read, -25293),
            (KeychainOperation::Update, -25293),
            (KeychainOperation::Create, -25300),
            (KeychainOperation::Delete, -25300),
            (KeychainOperation::Delete, -25293),
        ] {
            let typed = KeychainError::Security { operation, status };
            let public_error: &dyn std::error::Error = &typed;
            assert!(
                public_error.to_string()
                    == format!("keychain {operation:?} failed with OSStatus {status}"),
                "native status diagnostics must contain only the operation and status"
            );
            assert!(
                public_error.source().is_none(),
                "the typed error has no source"
            );
            let result: Result<()> = Err(typed.into());
            let error = result
                .expect_err("the constructed status control must fail")
                .context("credential operation failed");
            assert!(
                matches!(
                    error.downcast_ref::<KeychainError>(),
                    Some(KeychainError::Security {
                        operation: actual_operation,
                        status: actual_status,
                    }) if *actual_operation == operation && *actual_status == status
                ),
                "application context must preserve the exact typed operation and status"
            );
            assert!(
                matches!(
                    error.downcast::<KeychainError>(),
                    Ok(KeychainError::Security {
                        operation: actual_operation,
                        status: actual_status,
                    }) if actual_operation == operation && actual_status == status
                ),
                "the application must recover the owned native status error"
            );
        }
    }

    #[test]
    fn downstream_constructed_keychain_cancellation_is_an_owned_typed_error() {
        for operation in [
            KeychainOperation::Read,
            KeychainOperation::Update,
            KeychainOperation::Create,
            KeychainOperation::Delete,
        ] {
            let typed = KeychainError::UserCanceled { operation };
            assert!(
                typed.to_string() == "Keychain access was cancelled.",
                "cancellation diagnostics must preserve the official secret-free message"
            );
            let result: Result<()> = Err(typed.into());
            let error = result
                .expect_err("the constructed cancellation control must fail")
                .context("credential operation cancelled");
            assert!(
                matches!(
                    error.downcast_ref::<KeychainError>(),
                    Some(KeychainError::UserCanceled { operation: actual }) if *actual == operation
                ),
                "application context must preserve the cancelled operation"
            );
            assert!(
                matches!(
                    error.downcast::<KeychainError>(),
                    Ok(KeychainError::UserCanceled { operation: actual }) if actual == operation
                ),
                "cancellation must remain an owned error rather than absence or a duplicate"
            );
        }
    }

    #[test]
    fn downstream_constructed_keychain_invalid_response_survives_anyhow_context() {
        let result: Result<()> =
            Err(KeychainError::InvalidResponse("account was not a string").into());
        let error = result
            .expect_err("the constructed response control must fail")
            .context("credential response rejected");
        assert!(
            matches!(
                error.downcast_ref::<KeychainError>(),
                Some(KeychainError::InvalidResponse("account was not a string"))
            ),
            "application context must preserve the exact invalid-response reason"
        );
        match error.downcast::<KeychainError>() {
            Ok(error) => assert!(
                error.to_string() == "invalid keychain response: account was not a string",
                "response diagnostics must preserve the official secret-free message"
            ),
            Err(_) => panic!("the application must recover the owned invalid-response error"),
        }
    }
}
