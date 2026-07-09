use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use open_gpui_devtools::{
    DEVTOOLS_ARTIFACT_RECORD_SCHEMA_VERSION, DevtoolsArtifact, DevtoolsArtifactFileMode,
    DevtoolsArtifactFileSink, DevtoolsArtifactJsonlSink, DevtoolsArtifactMetadata,
    DevtoolsArtifactRecord, DevtoolsArtifactSink, DevtoolsCapture, DevtoolsReport, DevtoolsSession,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree,
};

fn capture_with_secret_metadata() -> DevtoolsCapture {
    DevtoolsCapture::new(
        DevtoolsTargetTree::new([DevtoolsTargetSnapshot::new(
            DevtoolsTargetId::new("target token=secret"),
            DevtoolsTargetKind::App,
            "owner@example.com",
        )]),
        [],
        [],
        [],
        [],
    )
}

#[test]
fn artifact_record_wraps_capture_with_sanitized_metadata() {
    let record = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("gallery token=secret")
            .scenario_id("scenario alice@example.com")
            .sequence(7)
            .flush_reason("manual export"),
        DevtoolsArtifact::capture(&capture_with_secret_metadata()),
    );

    let json = serde_json::to_string(&record).unwrap();

    assert_eq!(
        record.schema_version,
        DEVTOOLS_ARTIFACT_RECORD_SCHEMA_VERSION
    );
    assert_eq!(record.metadata.producer_id, "gallery token=[redacted]");
    assert_eq!(
        record.metadata.scenario_id.as_deref(),
        Some("scenario [redacted-email]")
    );
    assert_eq!(record.metadata.sequence, 7);
    assert!(matches!(record.artifact, DevtoolsArtifact::Capture(_)));
    assert!(!json.contains("secret"), "{json}");
    assert!(!json.contains("owner@example.com"), "{json}");
}

#[test]
fn artifact_record_derives_session_metadata_and_redaction_counts() {
    let mut session = DevtoolsSession::new("session token=secret", {
        let mut registry = open_gpui_devtools::DevtoolsRegistry::default();
        registry
            .register_capture_provider_fn("provider", || Ok(capture_with_secret_metadata()))
            .unwrap();
        registry
    });
    session.refresh().unwrap();
    session.refresh().unwrap();
    let export = session.export();

    let record = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("producer").sequence(2),
        DevtoolsArtifact::session_export(&export),
    );

    assert_eq!(
        record.metadata.session_id.as_deref(),
        Some("session token=[redacted]")
    );
    assert_eq!(record.metadata.generation, Some(2));
    assert_eq!(record.metadata.sequence, 2);
}

#[test]
fn jsonl_sink_writes_one_flushable_record_per_line() {
    let report = DevtoolsReport::from_capture(&capture_with_secret_metadata());
    let record = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("producer").sequence(1),
        DevtoolsArtifact::report(&report),
    );
    let mut output = Cursor::new(Vec::new());
    {
        let mut sink = DevtoolsArtifactJsonlSink::new(&mut output);
        sink.write_record(&record).unwrap();
        sink.write_record(&record.with_sequence(2)).unwrap();
    }

    let output = String::from_utf8(output.into_inner()).unwrap();
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"sequence\":1"));
    assert!(lines[1].contains("\"sequence\":2"));
}

#[test]
fn file_sink_writes_atomic_latest_and_jsonl_append() {
    let temp = unique_temp_dir("open-gpui-devtools-artifact");
    fs::create_dir_all(&temp).unwrap();
    let latest = temp.join("latest.json");
    let jsonl = temp.join("events.jsonl");
    let report = DevtoolsReport::from_capture(&capture_with_secret_metadata());
    let record = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("producer").sequence(1),
        DevtoolsArtifact::report(&report),
    );

    DevtoolsArtifactFileSink::new(&latest, DevtoolsArtifactFileMode::ReplaceAtomic)
        .write_record(&record)
        .unwrap();
    DevtoolsArtifactFileSink::new(&jsonl, DevtoolsArtifactFileMode::AppendJsonl)
        .write_record(&record)
        .unwrap();
    DevtoolsArtifactFileSink::new(&jsonl, DevtoolsArtifactFileMode::AppendJsonl)
        .write_record(&record.with_sequence(2))
        .unwrap();

    let latest_json = fs::read_to_string(latest).unwrap();
    let parsed: DevtoolsArtifactRecord = serde_json::from_str(&latest_json).unwrap();
    assert_eq!(parsed.metadata.sequence, 1);

    let lines = fs::read_to_string(jsonl).unwrap().lines().count();
    assert_eq!(lines, 2);
    fs::remove_dir_all(temp).unwrap();
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}
