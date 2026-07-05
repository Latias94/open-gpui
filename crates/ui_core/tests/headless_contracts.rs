use std::collections::BTreeSet;

#[test]
fn ui_core_extraction_blockers_match_allowlist() {
    let expected: [(&str, &str); 0] = [];
    let mut expected = expected
        .into_iter()
        .map(|(file, token)| SourceBlocker::new(file.to_owned(), token.to_owned()))
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = source_blockers(&[
        "Bounds<Px>",
        "Edges<Px>",
        "Pixels as Px",
        "Point<Px>",
        "Size<Px>",
    ]);
    actual.sort();

    assert_eq!(
        actual, expected,
        "ui_core public contracts gained or removed headless extraction blockers; update this inventory as neutral facades land"
    );
}

#[test]
fn ui_core_strict_boundary_blockers_match_allowlist() {
    let expected: [BoundaryBlocker; 0] = [];
    let expected = expected.into_iter().collect::<BTreeSet<_>>();
    let actual = strict_boundary_blockers()
        .into_iter()
        .collect::<BTreeSet<_>>();

    assert_eq!(
        actual, expected,
        "ui_core strict headless boundary blockers changed; shrink this inventory only when the adapter boundary migration removes the corresponding GPUI dependency path"
    );
}

#[test]
fn ui_core_motion_value_stays_private_while_consumed_motion_contracts_stay_public() {
    let lib_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs");
    let lib_source = std::fs::read_to_string(lib_path)
        .unwrap_or_else(|error| panic!("failed to read {lib_path}: {error}"));

    assert!(
        lib_source.contains("\nmod motion_value;\n"),
        "motion_value should remain an internal implementation module"
    );
    assert!(
        !lib_source.contains("\npub mod motion_value;\n"),
        "MotionValue should not be reachable as open_gpui_ui_core::motion_value::MotionValue"
    );

    let _track: Option<open_gpui_ui_core::MotionScalarTrack> = None;
    let _controller: open_gpui_ui_core::MotionScalarController<&'static str> =
        open_gpui_ui_core::MotionScalarController::new();
    let demand = open_gpui_ui_core::MotionFrameDemand::Idle;
    assert!(!demand.needs_frame());

    let model = open_gpui_ui_core::MotionPreset::Immediate.resolve_model();
    let plan = open_gpui_ui_core::MotionExecutionPlan::resolve(
        open_gpui_ui_core::MotionPolicyInput::new(
            open_gpui_ui_core::MotionPolicyContext::CommittedLayout,
            model,
        )
        .with_reduced_motion_final_state(true),
    );
    assert_eq!(
        plan.state(),
        open_gpui_ui_core::MotionExecutionState::Immediate
    );
    let _execution: Option<open_gpui_ui_core::MotionScalarExecution> = None;
    let _execution_sample: Option<open_gpui_ui_core::MotionScalarExecutionSample> = None;
    let _clip: Option<open_gpui_ui_core::MotionProjectionClip> = None;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SourceBlocker {
    file: String,
    token: String,
}

impl SourceBlocker {
    fn new(file: String, token: String) -> Self {
        Self { file, token }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BoundaryBlocker {
    category: String,
    file: String,
    detail: String,
}

impl BoundaryBlocker {
    fn new(
        category: impl Into<String>,
        file: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            file: file.into(),
            detail: detail.into(),
        }
    }
}

fn strict_boundary_blockers() -> Vec<BoundaryBlocker> {
    let mut blockers = cargo_dependency_blockers()
        .into_iter()
        .collect::<BTreeSet<_>>();

    for (file, line) in source_lines_with("open_gpui") {
        blockers.insert(BoundaryBlocker::new("source reference", file, line));
    }

    for blocker in source_blockers(&["Pixels as Px"]) {
        blockers.insert(BoundaryBlocker::new(
            "adaptive pixels alias",
            blocker.file,
            blocker.token,
        ));
    }

    for (file, line) in source_lines_with("impl From<UiPx> for open_gpui::") {
        blockers.insert(BoundaryBlocker::new(
            "ui px gpui conversion impl",
            file,
            line,
        ));
    }

    blockers.into_iter().collect()
}

fn cargo_dependency_blockers() -> Vec<BoundaryBlocker> {
    let manifest_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest = std::fs::read_to_string(manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {manifest_path}: {error}"));

    uncommented_manifest_lines(&manifest)
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("open_gpui"))
        .map(|line| BoundaryBlocker::new("cargo dependency", "Cargo.toml", line))
        .collect()
}

fn source_lines_with(token: &str) -> Vec<(String, String)> {
    let mut source_files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("ui_core src directory should be readable")
        .map(|entry| {
            entry
                .expect("source directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    source_files.sort();

    let mut matches = BTreeSet::new();
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let source = uncommented_lines(&source);
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>")
            .to_owned();

        for line in source.lines() {
            let line = line.trim();
            if line.contains(token) {
                matches.insert((file_name.clone(), line.to_owned()));
            }
        }
    }

    matches.into_iter().collect()
}

fn source_blockers(tokens: &[&str]) -> Vec<SourceBlocker> {
    let mut source_files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("ui_core src directory should be readable")
        .map(|entry| {
            entry
                .expect("source directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    source_files.sort();

    let mut blockers = Vec::new();
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let source = uncommented_lines(&source);
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for token in tokens {
            if source.contains(token) {
                blockers.push(SourceBlocker::new(
                    file_name.to_owned(),
                    (*token).to_owned(),
                ));
            }
        }
    }

    blockers
}

fn uncommented_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn uncommented_manifest_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with('#')
        })
        .collect::<Vec<_>>()
        .join("\n")
}
