use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use open_gpui_devtools::{
    DEVTOOLS_SESSION_SCHEMA_VERSION, DevtoolsCapture, DevtoolsDiffStatus, DevtoolsDomainId,
    DevtoolsDomainKind, DevtoolsDomainSnapshot, DevtoolsEventKind, DevtoolsEventRecord,
    DevtoolsRegistry, DevtoolsSession, DevtoolsSessionError, DevtoolsSessionExport,
    DevtoolsSessionImportError, DevtoolsSessionImportLimits, DevtoolsTargetId, DevtoolsTargetKind,
    DevtoolsTargetSnapshot, DevtoolsTargetTree, DevtoolsWorkbench, DevtoolsWorkbenchDiffState,
    DevtoolsWorkbenchRefreshStatus, ProbeSnapshotError, SnapshotEnvelope,
};

#[test]
fn session_refreshes_generations_and_diffs_previous_frame() {
    let counter = Arc::new(AtomicUsize::new(0));
    let provider_counter = Arc::clone(&counter);
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("provider.runtime", move || {
            let value = provider_counter.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(runtime_capture(value))
        })
        .unwrap();
    let mut session = DevtoolsSession::new("gallery.session", registry);

    let first = session.refresh().unwrap();
    assert_eq!(first.generation, 1);
    assert!(first.diff_from_previous.is_none());

    let second = session.refresh().unwrap();
    let diff = second.diff_from_previous.expect("second frame diff");

    assert_eq!(second.generation, 2);
    assert_eq!(second.previous_generation, Some(1));
    assert!(diff.summary.changed > 0);
    assert!(
        diff.rows
            .iter()
            .any(|row| row.status == DevtoolsDiffStatus::Changed)
    );
}

#[test]
fn session_keeps_bounded_history_without_resetting_generation() {
    let mut session =
        DevtoolsSession::new("bounded.session", registry_for_value(1)).with_history_limit(2);

    session.refresh().unwrap();
    session.refresh().unwrap();
    session.refresh().unwrap();

    let generations = session
        .frames()
        .map(|frame| frame.generation)
        .collect::<Vec<_>>();
    assert_eq!(generations, [2, 3]);
    assert_eq!(session.previous_frame().unwrap().generation, 2);
    assert_eq!(session.current_frame().unwrap().generation, 3);
    assert_eq!(session.next_generation(), 4);
}

#[test]
fn session_close_makes_refresh_fail_deterministically() {
    let mut session = DevtoolsSession::new("closing.session", DevtoolsRegistry::default());
    session.close();

    let error = session.refresh().unwrap_err();

    assert!(session.is_closed());
    assert_eq!(
        error,
        DevtoolsSessionError::Closed {
            session_id: "closing.session".to_owned()
        }
    );
}

#[test]
fn session_refresh_preserves_provider_failure_diagnostic() {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("provider.failing", || {
            Err(ProbeSnapshotError::CollectionFailed(
                "provider token=secret unavailable".to_owned(),
            ))
        })
        .unwrap();
    let mut session = DevtoolsSession::new("failure.session", registry);

    let frame = session.refresh().unwrap();

    assert_eq!(frame.capture.diagnostics.len(), 1);
    assert_eq!(
        frame.capture.diagnostics[0].probe_id.as_str(),
        "provider.failing"
    );
    assert!(!frame.capture.diagnostics[0].message.contains("secret"));
}

#[test]
fn workbench_tracks_refresh_status_history_and_inspector_state() {
    let counter = Arc::new(AtomicUsize::new(0));
    let provider_counter = Arc::clone(&counter);
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("provider.runtime", move || {
            let value = provider_counter.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(runtime_capture(value))
        })
        .unwrap();
    let mut workbench = DevtoolsWorkbench::new("app.devtools", registry).with_history_limit(2);

    assert_eq!(
        workbench.refresh_status(),
        DevtoolsWorkbenchRefreshStatus::Idle
    );
    assert_eq!(workbench.current_generation(), None);
    assert_eq!(
        workbench.diff_state(),
        DevtoolsWorkbenchDiffState::NoPreviousFrame
    );

    let first = workbench.refresh().unwrap();
    assert_eq!(first.generation, 1);
    assert_eq!(
        workbench.refresh_status(),
        DevtoolsWorkbenchRefreshStatus::NoPreviousFrame
    );
    assert_eq!(workbench.diff_state_label(), "no-previous-frame");
    assert_eq!(
        workbench
            .inspector_state()
            .session_frame()
            .map(|summary| summary.generation),
        Some(1)
    );

    let second = workbench.refresh().unwrap();
    assert_eq!(second.generation, 2);
    assert_eq!(
        workbench.refresh_status(),
        DevtoolsWorkbenchRefreshStatus::Changed
    );
    assert_eq!(workbench.diff_state(), DevtoolsWorkbenchDiffState::Changed);
    assert!(workbench.diff_row_count() > 0);

    workbench.refresh().unwrap();
    assert_eq!(workbench.retained_frames(), 2);
    assert_eq!(workbench.history_limit(), 2);
    assert_eq!(workbench.current_generation(), Some(3));
    assert_eq!(workbench.previous_generation(), Some(2));

    workbench.mark_idle();
    assert_eq!(
        workbench.refresh_status(),
        DevtoolsWorkbenchRefreshStatus::Idle
    );
}

#[test]
fn workbench_reports_no_change_and_capture_error() {
    let mut workbench =
        DevtoolsWorkbench::new("static.devtools", registry_for_value(1)).with_history_limit(4);

    workbench.refresh().unwrap();
    workbench.refresh().unwrap();

    assert_eq!(
        workbench.refresh_status(),
        DevtoolsWorkbenchRefreshStatus::NoChange
    );
    assert_eq!(workbench.diff_state(), DevtoolsWorkbenchDiffState::NoChange);
    assert_eq!(workbench.last_error(), None);

    workbench.session_mut().close();
    let error = workbench.refresh().unwrap_err();

    assert!(matches!(error, DevtoolsSessionError::Closed { .. }));
    assert_eq!(
        workbench.refresh_status(),
        DevtoolsWorkbenchRefreshStatus::CaptureError
    );
    assert!(workbench.last_error().is_some());
}

#[test]
fn session_export_import_resanitizes_untrusted_json() {
    let mut session = DevtoolsSession::new("import.session", registry_for_value(1));
    session.refresh().unwrap();
    let mut value = serde_json::to_value(session.export()).unwrap();
    value["frames"][0]["capture"]["targets"]["targets"][0]["label"] =
        serde_json::Value::String("alice@example.com password=hunter2".to_owned());
    let json = serde_json::to_string(&value).unwrap();

    let imported =
        DevtoolsSessionExport::from_json_str(&json, DevtoolsSessionImportLimits::default())
            .unwrap();
    let label = &imported.frames[0].capture.targets.targets[0].label;

    assert_eq!(imported.schema_version, DEVTOOLS_SESSION_SCHEMA_VERSION);
    assert!(label.contains("[redacted-email]"));
    assert!(!label.contains("alice@example.com"));
    assert!(!label.contains("hunter2"));
}

#[test]
fn session_import_rejects_bad_schema_and_oversized_event_batches() {
    let mut session = DevtoolsSession::new("limits.session", registry_for_value(1));
    session.refresh().unwrap();
    let mut export = session.export();
    export.schema_version = "future-schema".to_owned();
    let json = serde_json::to_string(&export).unwrap();

    assert!(matches!(
        DevtoolsSessionExport::from_json_str(&json, DevtoolsSessionImportLimits::default()),
        Err(DevtoolsSessionImportError::UnsupportedSchema { .. })
    ));

    let mut export = session.export();
    export.frames[0]
        .capture
        .events
        .push(DevtoolsEventRecord::new(
            "runtime.extra",
            "Runtime extra",
            DevtoolsEventKind::Instant,
        ));
    let json = serde_json::to_string(&export).unwrap();
    let limits = DevtoolsSessionImportLimits {
        max_events_per_frame: 0,
        ..DevtoolsSessionImportLimits::default()
    };

    assert!(matches!(
        DevtoolsSessionExport::from_json_str(&json, limits),
        Err(DevtoolsSessionImportError::TooManyEvents { .. })
    ));
}

fn registry_for_value(value: usize) -> DevtoolsRegistry {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("provider.runtime", move || Ok(runtime_capture(value)))
        .unwrap();
    registry
}

fn runtime_capture(value: usize) -> DevtoolsCapture {
    let target_id = DevtoolsTargetId::from_parts(["runtime", "session"]);
    let domain_id = DevtoolsDomainId::from_parts(["runtime", "session", "state"]);
    let target = DevtoolsTargetSnapshot::new(
        target_id.clone(),
        DevtoolsTargetKind::Runtime,
        format!("Runtime {value}"),
    );
    let domain = DevtoolsDomainSnapshot::new(
        domain_id.clone(),
        target_id.clone(),
        DevtoolsDomainKind::Diagnostic,
        "Runtime state",
    )
    .with_summary(serde_json::json!({ "value": value }));
    let event = DevtoolsEventRecord::new(
        "runtime.changed",
        "Runtime changed",
        DevtoolsEventKind::Instant,
    )
    .scope_id("session")
    .target_id(target_id)
    .domain_id(domain_id)
    .with_payload(serde_json::json!({ "value": value }));

    DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [domain],
        [event],
        Vec::<SnapshotEnvelope>::new(),
        Vec::new(),
    )
}
