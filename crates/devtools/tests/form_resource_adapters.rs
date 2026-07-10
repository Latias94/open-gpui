#![cfg(all(feature = "form", feature = "resource"))]

use open_gpui_devtools::{
    DevtoolsDomainKind, DevtoolsProbe, DevtoolsTargetKind, ProbeId, SnapshotKind, form, resource,
};
use open_gpui_form::{
    FieldId, FieldMetaSnapshot, FieldPath, FieldSnapshot, FormSnapshot, FormStatus, RedactedValue,
};
use open_gpui_resource::{
    MutationSnapshot, MutationStatus, PaginatedResourceSnapshot, PaginatedResourceSnapshotView,
    QueryKey, RedactedResourceValue, ResourcePage, ResourceRedactionPolicy, ResourceSnapshot,
    ResourceStatus,
};

#[test]
fn form_resource_adapters_count_redacted_form_fields() {
    let snapshot = FormSnapshot {
        status: FormStatus::SubmitFailed,
        fields: vec![
            FieldSnapshot {
                id: FieldId::new("account.email").unwrap(),
                path: FieldPath::new("account.email").unwrap(),
                value: RedactedValue::Redacted,
                meta: FieldMetaSnapshot {
                    dirty: true,
                    touched: true,
                    errors: vec!["violet meadow 744".to_owned()],
                    ..FieldMetaSnapshot::default()
                },
            },
            FieldSnapshot {
                id: FieldId::new("azure ridge 581").unwrap(),
                path: FieldPath::new("azure ridge 581").unwrap(),
                value: RedactedValue::Json(serde_json::json!("silver harbor 319")),
                meta: FieldMetaSnapshot::default(),
            },
        ],
        errors: vec!["amber canyon 882".to_owned()],
        submit_count: 2,
    };

    let probe_snapshot = form::form_probe_snapshot(&snapshot);
    let envelope = form::form_snapshot_envelope(ProbeId::new("form").unwrap(), &snapshot);
    let capture = form::form_capture(ProbeId::new("form.capture").unwrap(), &snapshot);
    let serialized = serde_json::to_string(&envelope).unwrap();
    let serialized_capture = serde_json::to_string(&capture).unwrap();

    assert_eq!(probe_snapshot.redaction().redacted_values, 4);
    assert_eq!(envelope.kind, SnapshotKind::Form);
    assert_eq!(capture.targets.targets[0].kind, DevtoolsTargetKind::Runtime);
    assert_eq!(capture.domains[0].kind, DevtoolsDomainKind::Data);
    assert_eq!(capture.snapshots[0].kind, SnapshotKind::Form);
    assert!(serialized.contains("SubmitFailed"));
    assert!(serialized.contains("submit_count"));
    for canary in [
        "violet meadow 744",
        "silver harbor 319",
        "amber canyon 882",
        "azure ridge 581",
        "account.email",
    ] {
        assert!(!serialized.contains(canary));
        assert!(!serialized_capture.contains(canary));
    }
    assert!(serialized.contains("error_count"));
    assert!(serialized.contains("redacted"));
}

#[test]
fn form_resource_adapters_convert_resource_mutation_and_pages() {
    let resource = ResourceSnapshot {
        key: QueryKey::new(["projects", "https://example.test/projects?token=raw-secret"]).unwrap(),
        status: ResourceStatus::Refetching,
        data: Some(RedactedResourceValue::Redacted),
        error: Some("fetch failed for alice@example.com".to_owned()),
        observer_count: 2,
        fetch_attempts: 3,
    };
    let mutation = MutationSnapshot {
        id: "invite:alice@example.com:token=raw-secret".to_owned(),
        status: MutationStatus::Error,
        data: Some(RedactedResourceValue::Summary("object:2 keys".to_owned())),
        error: Some("mutation token=raw-secret failed".to_owned()),
    };
    let mut paginated =
        PaginatedResourceSnapshot::new(QueryKey::new(["projects", "pages"]).unwrap());
    paginated.push_page(ResourcePage::new(
        Some("cursor-token=raw-secret".to_owned()),
        [serde_json::json!({"name": "secret project"})],
    ));
    let page_view = paginated.snapshot(ResourceRedactionPolicy::RedactAll);

    let envelope = resource::resource_snapshot_envelope(
        ProbeId::new("resource").unwrap(),
        [&resource],
        [&mutation],
        [&page_view],
    );
    let capture = resource::resource_capture(
        ProbeId::new("resource.capture").unwrap(),
        [&resource],
        [&mutation],
        [&page_view],
    );
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Resource);
    assert_eq!(envelope.redaction.redacted_values, 2);
    assert_eq!(capture.targets.targets[0].kind, DevtoolsTargetKind::Runtime);
    assert_eq!(capture.domains[0].kind, DevtoolsDomainKind::Data);
    assert_eq!(capture.snapshots[0].kind, SnapshotKind::Resource);
    assert!(serialized.contains("Refetching"));
    assert!(serialized.contains("observer_count"));
    assert!(serialized.contains("fetch_attempts"));
    assert!(serialized.contains("mutation"));
    assert!(serialized.contains("page_index"));
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("raw-secret"));
    assert!(!serialized.contains("secret project"));
}

#[test]
fn form_resource_adapters_build_closure_backed_probes() {
    let form_snapshot = FormSnapshot {
        status: FormStatus::Submitted,
        submit_count: 1,
        ..FormSnapshot::default()
    };
    let resource_snapshot = ResourceSnapshot {
        key: QueryKey::new(["projects"]).unwrap(),
        status: ResourceStatus::Success,
        data: Some(RedactedResourceValue::Json(serde_json::json!([
            { "id": 1, "name": "Project" }
        ]))),
        error: None,
        observer_count: 1,
        fetch_attempts: 1,
    };

    let form_probe =
        form::form_snapshot_probe("form", move || form_snapshot.clone()).expect("form probe");
    let resource_probe = resource::resource_snapshot_probe(
        "resource",
        move || vec![resource_snapshot.clone()],
        Vec::<MutationSnapshot>::new,
        Vec::<PaginatedResourceSnapshotView>::new,
    )
    .expect("resource probe");

    assert_eq!(form_probe.snapshot().unwrap().kind, SnapshotKind::Form);
    assert_eq!(
        resource_probe.snapshot().unwrap().kind,
        SnapshotKind::Resource
    );
}
