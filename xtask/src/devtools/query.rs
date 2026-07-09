//! DevTools query and assert command implementation.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsCaptureDiff, DevtoolsDiffStatus, DevtoolsReport,
    DevtoolsReportSeverity,
};
use serde::Serialize;
use serde_json::{Value, json};

use super::{
    artifact::{LoadedArtifact, WaitArgs, load_artifact},
    render::write_output,
};

const QUERY_SCHEMA_VERSION: &str = "open-gpui-devtools-query/v1";
const ASSERT_SCHEMA_VERSION: &str = "open-gpui-devtools-assert/v1";

#[derive(Args, Debug)]
pub(super) struct QueryArgs {
    /// Input DevTools capture, session export, artifact record, or report JSON. Use '-' for stdin.
    #[arg(short, long, value_name = "PATH")]
    input: PathBuf,
    /// Output path. Defaults to stdout. Use '-' for stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = QueryOutputFormat::Json)]
    format: QueryOutputFormat,
    #[command(flatten)]
    selectors: QuerySelectorArgs,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Args, Debug)]
pub(super) struct AssertArgs {
    /// Input DevTools capture, session export, artifact record, or report JSON. Use '-' for stdin.
    #[arg(short, long, value_name = "PATH")]
    input: PathBuf,
    /// Output path. Defaults to stdout. Use '-' for stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = QueryOutputFormat::Json)]
    format: QueryOutputFormat,
    /// Fail when a report finding reaches this severity threshold.
    #[arg(long, value_enum)]
    fail_on_finding: Option<SeverityArg>,
    /// Require the artifact generation to be at least this value.
    #[arg(long)]
    min_generation: Option<u64>,
    /// Require at least one added, removed, changed, or collision diff row.
    #[arg(long)]
    require_diff_change: bool,
    /// Require no added, removed, changed, or collision diff rows.
    #[arg(long)]
    require_no_diff_change: bool,
    #[command(flatten)]
    selectors: QuerySelectorArgs,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Args, Clone, Debug, Default)]
pub(super) struct QuerySelectorArgs {
    /// Restrict rows to one row kind.
    #[arg(long, value_enum)]
    pub(super) row_kind: Option<QueryRowKind>,
    /// Match target rows by target id.
    #[arg(long)]
    pub(super) target_id: Option<String>,
    /// Match target rows by target kind, such as viewport, window, dockspace, or panel.
    #[arg(long)]
    pub(super) target_kind: Option<String>,
    /// Match domain rows by domain id.
    #[arg(long)]
    pub(super) domain_id: Option<String>,
    /// Match domain rows by domain kind, such as docking, layout, motion, command, or data.
    #[arg(long)]
    pub(super) domain_kind: Option<String>,
    /// Match event rows by event id.
    #[arg(long)]
    pub(super) event_id: Option<String>,
    /// Match event rows by stable event identity key.
    #[arg(long)]
    pub(super) event_identity: Option<String>,
    /// Match snapshot rows by snapshot kind.
    #[arg(long)]
    pub(super) snapshot_kind: Option<String>,
    /// Match snapshot rows by probe id.
    #[arg(long)]
    pub(super) probe_id: Option<String>,
    /// Match finding rows by finding id.
    #[arg(long)]
    pub(super) finding_id: Option<String>,
    /// Match finding rows by category.
    #[arg(long)]
    pub(super) finding_category: Option<String>,
    /// Match finding rows by exact severity.
    #[arg(long, value_enum)]
    pub(super) finding_severity: Option<SeverityArg>,
    /// Match finding rows at or above this severity.
    #[arg(long, value_enum)]
    pub(super) finding_at_or_above: Option<SeverityArg>,
    /// Match diff rows by diff item kind.
    #[arg(long)]
    pub(super) diff_kind: Option<String>,
    /// Match diff rows by status, such as added, removed, changed, unchanged, or collision.
    #[arg(long)]
    pub(super) diff_status: Option<String>,
    /// Match the artifact generation row.
    #[arg(long)]
    pub(super) generation: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(super) enum QueryOutputFormat {
    Json,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(super) enum QueryRowKind {
    Target,
    Domain,
    Event,
    Snapshot,
    Finding,
    Diff,
    Generation,
}

impl QueryRowKind {
    const fn as_label(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Domain => "domain",
            Self::Event => "event",
            Self::Snapshot => "snapshot",
            Self::Finding => "finding",
            Self::Diff => "diff",
            Self::Generation => "generation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SeverityArg {
    Info,
    Warning,
    Error,
}

impl SeverityArg {
    pub(super) const fn as_label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    const fn severity(self) -> DevtoolsReportSeverity {
        match self {
            Self::Info => DevtoolsReportSeverity::Info,
            Self::Warning => DevtoolsReportSeverity::Warning,
            Self::Error => DevtoolsReportSeverity::Error,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct QueryResult {
    schema_version: &'static str,
    source: QuerySource,
    selector_summary: Value,
    row_count: usize,
    rows: Vec<QueryRow>,
}

#[derive(Clone, Debug, Serialize)]
struct QuerySource {
    artifact_kind: &'static str,
    report_source_kind: Option<&'static str>,
    session_id: Option<String>,
    generation: Option<u64>,
    retained_frames: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct QueryRow {
    kind: QueryRowKind,
    id: String,
    label: String,
    generation: Option<u64>,
    target_id: Option<String>,
    domain_id: Option<String>,
    event_identity: Option<String>,
    severity: Option<String>,
    status: Option<String>,
    details: Value,
}

#[derive(Serialize)]
struct AssertResult {
    schema_version: &'static str,
    ok: bool,
    source: QuerySource,
    selector_summary: Value,
    row_count: usize,
    rows: Vec<QueryRow>,
    failures: Vec<AssertFailure>,
}

#[derive(Serialize)]
struct AssertFailure {
    code: &'static str,
    message: String,
    details: Value,
}

struct QueryFacts<'a> {
    source: QuerySource,
    capture: Option<&'a DevtoolsCapture>,
    diff: Option<&'a DevtoolsCaptureDiff>,
    report: DevtoolsReport,
}

pub(super) fn query_command(args: QueryArgs) -> Result<(), ()> {
    let artifact = load_artifact(&args.input, args.wait)?;
    let result = query_artifact(&artifact, &args.selectors);
    write_output(
        args.output.as_deref(),
        render_query_result(&result, args.format),
    )
}

pub(super) fn assert_command(args: AssertArgs) -> Result<(), ()> {
    let artifact = load_artifact(&args.input, args.wait)?;
    let query_result = query_artifact(&artifact, &args.selectors);
    let facts = QueryFacts::new(&artifact);
    let mut failures = Vec::new();

    if args.selectors.has_presence_selector() && query_result.rows.is_empty() {
        failures.push(AssertFailure {
            code: "devtools.assert.no-query-match",
            message: "No DevTools rows matched the requested selector.".to_owned(),
            details: args.selectors.summary(),
        });
    }

    if let Some(threshold) = args.fail_on_finding {
        let matched = facts
            .report
            .findings
            .iter()
            .filter(|finding| finding.severity.is_at_least(threshold.severity()))
            .map(|finding| {
                json!({
                    "id": finding.id,
                    "severity": finding.severity.as_label(),
                    "category": finding.category,
                    "message": finding.message,
                })
            })
            .collect::<Vec<_>>();
        if !matched.is_empty() {
            failures.push(AssertFailure {
                code: "devtools.assert.finding-threshold",
                message: format!(
                    "DevTools findings reached severity threshold `{}`.",
                    threshold.as_label()
                ),
                details: json!({
                    "threshold": threshold.as_label(),
                    "findings": matched,
                }),
            });
        }
    }

    if let Some(min_generation) = args.min_generation {
        match facts.source.generation {
            Some(generation) if generation >= min_generation => {}
            generation => failures.push(AssertFailure {
                code: "devtools.assert.generation-too-low",
                message: format!(
                    "DevTools artifact generation is below required minimum `{min_generation}`."
                ),
                details: json!({
                    "required_min_generation": min_generation,
                    "actual_generation": generation,
                }),
            }),
        }
    }

    let diff_has_changes = facts.diff.is_some_and(diff_has_change);
    if args.require_diff_change && !diff_has_changes {
        failures.push(AssertFailure {
            code: "devtools.assert.diff-unchanged",
            message: "DevTools diff contains no added, removed, changed, or collision rows."
                .to_owned(),
            details: json!({ "required": "changed" }),
        });
    }
    if args.require_no_diff_change && diff_has_changes {
        failures.push(AssertFailure {
            code: "devtools.assert.diff-changed",
            message: "DevTools diff contains changed rows.".to_owned(),
            details: json!({ "required": "unchanged" }),
        });
    }
    if args.require_diff_change && args.require_no_diff_change {
        failures.push(AssertFailure {
            code: "devtools.assert.conflicting-diff-requirements",
            message: "Cannot require both changed and unchanged diff state.".to_owned(),
            details: json!({}),
        });
    }

    if !args.has_any_assertion() {
        failures.push(AssertFailure {
            code: "devtools.assert.no-condition",
            message: "Provide a selector, --fail-on-finding, --min-generation, or diff assertion."
                .to_owned(),
            details: json!({}),
        });
    }

    let result = AssertResult {
        schema_version: ASSERT_SCHEMA_VERSION,
        ok: failures.is_empty(),
        source: query_result.source,
        selector_summary: query_result.selector_summary,
        row_count: query_result.row_count,
        rows: query_result.rows,
        failures,
    };
    let ok = result.ok;
    write_output(
        args.output.as_deref(),
        render_assert_result(&result, args.format),
    )?;

    if ok { Ok(()) } else { Err(()) }
}

pub(super) fn query_artifact(
    artifact: &LoadedArtifact,
    selectors: &QuerySelectorArgs,
) -> QueryResult {
    let facts = QueryFacts::new(artifact);
    let mut rows = Vec::new();

    if selectors.wants_kind(QueryRowKind::Generation) {
        rows.push(generation_row(&facts.source));
    }

    if let Some(capture) = facts.capture {
        if selectors.wants_kind(QueryRowKind::Target) {
            rows.extend(capture.targets.targets.iter().map(|target| QueryRow {
                kind: QueryRowKind::Target,
                id: target.id.as_str().to_owned(),
                label: target.label.clone(),
                generation: facts.source.generation,
                target_id: Some(target.id.as_str().to_owned()),
                domain_id: None,
                event_identity: None,
                severity: None,
                status: None,
                details: json!({
                    "target_id": target.id.as_str(),
                    "target_kind": target.kind.as_label(),
                    "parent_id": target.parent_id.as_ref().map(|id| id.as_str()),
                    "metadata": target.metadata,
                }),
            }));
        }

        if selectors.wants_kind(QueryRowKind::Domain) {
            rows.extend(capture.domains.iter().map(|domain| QueryRow {
                kind: QueryRowKind::Domain,
                id: domain.id.as_str().to_owned(),
                label: domain.label.clone(),
                generation: facts.source.generation,
                target_id: Some(domain.target_id.as_str().to_owned()),
                domain_id: Some(domain.id.as_str().to_owned()),
                event_identity: None,
                severity: None,
                status: None,
                details: json!({
                    "domain_id": domain.id.as_str(),
                    "domain_kind": domain.kind.as_label(),
                    "target_id": domain.target_id.as_str(),
                    "diagnostic_count": domain.diagnostics.len(),
                    "snapshot_kind": domain.snapshot.as_ref().map(|snapshot| snapshot.kind.as_label()),
                    "summary": domain.summary,
                }),
            }));
        }

        if selectors.wants_kind(QueryRowKind::Event) {
            rows.extend(capture.events.iter().map(|event| {
                let identity = event.identity();
                QueryRow {
                    kind: QueryRowKind::Event,
                    id: event.id().to_owned(),
                    label: event.label().to_owned(),
                    generation: facts.source.generation,
                    target_id: event.target_id_ref().map(|id| id.as_str().to_owned()),
                    domain_id: event.domain_id_ref().map(|id| id.as_str().to_owned()),
                    event_identity: Some(identity.as_key()),
                    severity: None,
                    status: None,
                    details: json!({
                        "event_id": event.id(),
                        "event_kind": event.kind().as_label(),
                        "event_identity": identity.as_key(),
                        "scope_id": identity.scope_id,
                        "sequence": identity.sequence,
                        "target_id": event.target_id_ref().map(|id| id.as_str()),
                        "domain_id": event.domain_id_ref().map(|id| id.as_str()),
                        "timestamp_ms": event.timestamp_ms_value(),
                        "duration_ms": event.duration_ms_value(),
                        "payload": event.payload(),
                    }),
                }
            }));
        }

        if selectors.wants_kind(QueryRowKind::Snapshot) {
            rows.extend(capture.snapshots.iter().map(|snapshot| {
                let snapshot_kind = snapshot.kind.as_label();
                QueryRow {
                    kind: QueryRowKind::Snapshot,
                    id: format!("{}:{}", snapshot.probe_id.as_str(), snapshot_kind),
                    label: snapshot_kind.to_string(),
                    generation: facts.source.generation,
                    target_id: None,
                    domain_id: None,
                    event_identity: None,
                    severity: None,
                    status: None,
                    details: json!({
                        "probe_id": snapshot.probe_id.as_str(),
                        "snapshot_kind": snapshot_kind,
                        "root_count": snapshot.tree.nodes.len(),
                        "redacted_values": snapshot.redaction.redacted_values,
                        "redaction_notes": snapshot.redaction.notes,
                    }),
                }
            }));
        }
    }

    if selectors.wants_kind(QueryRowKind::Finding) {
        rows.extend(facts.report.findings.iter().map(|finding| QueryRow {
            kind: QueryRowKind::Finding,
            id: finding.id.clone(),
            label: finding.title.clone(),
            generation: facts.source.generation,
            target_id: finding
                .target_id
                .as_ref()
                .map(|target_id| target_id.as_str().to_owned()),
            domain_id: finding
                .domain_id
                .as_ref()
                .map(|domain_id| domain_id.as_str().to_owned()),
            event_identity: finding
                .event_identity
                .as_ref()
                .map(|identity| identity.as_key()),
            severity: Some(finding.severity.as_label().to_owned()),
            status: None,
            details: json!({
                "finding_id": finding.id,
                "severity": finding.severity.as_label(),
                "category": finding.category,
                "message": finding.message,
                "target_id": finding.target_id.as_ref().map(|id| id.as_str()),
                "domain_id": finding.domain_id.as_ref().map(|id| id.as_str()),
                "event_identity": finding.event_identity.as_ref().map(|identity| identity.as_key()),
                "recommendation": finding.recommendation,
            }),
        }));
    }

    if let Some(diff) = facts.diff {
        if selectors.wants_kind(QueryRowKind::Diff) {
            rows.extend(diff.rows.iter().map(|row| QueryRow {
                kind: QueryRowKind::Diff,
                id: row.identity.clone(),
                label: row.label.clone(),
                generation: facts.source.generation,
                target_id: None,
                domain_id: None,
                event_identity: None,
                severity: row.diagnostic.as_ref().map(|_| "error".to_owned()),
                status: Some(row.status.as_label().to_owned()),
                details: json!({
                    "diff_kind": row.kind.as_label(),
                    "diff_status": row.status.as_label(),
                    "identity": row.identity,
                    "diagnostic": row.diagnostic,
                    "previous": row.previous,
                    "current": row.current,
                }),
            }));
        }
    }

    rows.retain(|row| selectors.matches_row(row));

    QueryResult {
        schema_version: QUERY_SCHEMA_VERSION,
        source: facts.source,
        selector_summary: selectors.summary(),
        row_count: rows.len(),
        rows,
    }
}

pub(super) fn render_query_result(
    result: &QueryResult,
    format: QueryOutputFormat,
) -> Result<String, ()> {
    match format {
        QueryOutputFormat::Json => serde_json::to_string_pretty(result).map_err(|error| {
            eprintln!("failed to serialize devtools query result: {error}");
        }),
        QueryOutputFormat::Markdown => Ok(render_query_markdown(result)),
    }
}

fn render_assert_result(result: &AssertResult, format: QueryOutputFormat) -> Result<String, ()> {
    match format {
        QueryOutputFormat::Json => serde_json::to_string_pretty(result).map_err(|error| {
            eprintln!("failed to serialize devtools assert result: {error}");
        }),
        QueryOutputFormat::Markdown => Ok(render_assert_markdown(result)),
    }
}

fn render_query_markdown(result: &QueryResult) -> String {
    let mut output = String::from("# Open GPUI DevTools Query\n\n");
    output.push_str(&format!("- Rows: `{}`\n", result.row_count));
    if let Some(generation) = result.source.generation {
        output.push_str(&format!("- Generation: `{generation}`\n"));
    }
    output.push_str("\n| Kind | ID | Label | Generation | Status | Severity |\n");
    output.push_str("|---|---|---|---:|---|---|\n");
    for row in &result.rows {
        output.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            row.kind.as_label(),
            row.id,
            row.label.replace('|', "\\|"),
            row.generation
                .map(|generation| generation.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            row.status.as_deref().unwrap_or("-"),
            row.severity.as_deref().unwrap_or("-"),
        ));
    }
    output
}

fn render_assert_markdown(result: &AssertResult) -> String {
    let mut output = String::from("# Open GPUI DevTools Assert\n\n");
    output.push_str(&format!(
        "- Status: `{}`\n",
        if result.ok { "ok" } else { "failed" }
    ));
    output.push_str(&format!("- Rows: `{}`\n", result.row_count));
    if result.failures.is_empty() {
        output.push_str("\nNo failures.\n");
    } else {
        output.push_str("\n| Code | Message |\n|---|---|\n");
        for failure in &result.failures {
            output.push_str(&format!(
                "| `{}` | {} |\n",
                failure.code,
                failure.message.replace('|', "\\|")
            ));
        }
    }
    output
}

impl QueryFacts<'_> {
    fn new(artifact: &LoadedArtifact) -> QueryFacts<'_> {
        match artifact {
            LoadedArtifact::Capture(capture) => QueryFacts {
                source: QuerySource {
                    artifact_kind: "capture",
                    report_source_kind: Some("capture"),
                    session_id: None,
                    generation: None,
                    retained_frames: None,
                },
                capture: Some(capture),
                diff: None,
                report: DevtoolsReport::from_capture(capture),
            },
            LoadedArtifact::SessionExport(export) => {
                let current = export.frames.last();
                QueryFacts {
                    source: QuerySource {
                        artifact_kind: "session-export",
                        report_source_kind: Some("session-export"),
                        session_id: Some(export.session_id.clone()),
                        generation: export.current_generation,
                        retained_frames: Some(export.retained_frames),
                    },
                    capture: current.map(|frame| &frame.capture),
                    diff: current.and_then(|frame| frame.diff_from_previous.as_ref()),
                    report: DevtoolsReport::from_session_export(export),
                }
            }
            LoadedArtifact::Report(report) => QueryFacts {
                source: QuerySource {
                    artifact_kind: "report",
                    report_source_kind: Some(report.source.kind.as_label()),
                    session_id: report.source.session_id.clone(),
                    generation: report.source.generation,
                    retained_frames: report.source.retained_frames,
                },
                capture: None,
                diff: None,
                report: report.clone(),
            },
        }
    }
}

impl AssertArgs {
    fn has_any_assertion(&self) -> bool {
        self.selectors.has_presence_selector()
            || self.fail_on_finding.is_some()
            || self.min_generation.is_some()
            || self.require_diff_change
            || self.require_no_diff_change
    }
}

impl QuerySelectorArgs {
    pub(super) fn has_presence_selector(&self) -> bool {
        self.row_kind.is_some()
            || self.target_id.is_some()
            || self.target_kind.is_some()
            || self.domain_id.is_some()
            || self.domain_kind.is_some()
            || self.event_id.is_some()
            || self.event_identity.is_some()
            || self.snapshot_kind.is_some()
            || self.probe_id.is_some()
            || self.finding_id.is_some()
            || self.finding_category.is_some()
            || self.finding_severity.is_some()
            || self.finding_at_or_above.is_some()
            || self.diff_kind.is_some()
            || self.diff_status.is_some()
            || self.generation.is_some()
    }

    pub(super) fn summary(&self) -> Value {
        json!({
            "row_kind": self.row_kind.map(QueryRowKind::as_label),
            "target_id": self.target_id,
            "target_kind": self.target_kind,
            "domain_id": self.domain_id,
            "domain_kind": self.domain_kind,
            "event_id": self.event_id,
            "event_identity": self.event_identity,
            "snapshot_kind": self.snapshot_kind,
            "probe_id": self.probe_id,
            "finding_id": self.finding_id,
            "finding_category": self.finding_category,
            "finding_severity": self.finding_severity.map(SeverityArg::as_label),
            "finding_at_or_above": self.finding_at_or_above.map(SeverityArg::as_label),
            "diff_kind": self.diff_kind,
            "diff_status": self.diff_status,
            "generation": self.generation,
        })
    }

    fn wants_kind(&self, kind: QueryRowKind) -> bool {
        if let Some(row_kind) = self.row_kind {
            return row_kind == kind;
        }

        let target = self.target_id.is_some() || self.target_kind.is_some();
        let domain = self.domain_id.is_some() || self.domain_kind.is_some();
        let event = self.event_id.is_some() || self.event_identity.is_some();
        let snapshot = self.snapshot_kind.is_some() || self.probe_id.is_some();
        let finding = self.finding_id.is_some()
            || self.finding_category.is_some()
            || self.finding_severity.is_some()
            || self.finding_at_or_above.is_some();
        let diff = self.diff_kind.is_some() || self.diff_status.is_some();
        let generation = self.generation.is_some();
        let any_specific = target || domain || event || snapshot || finding || diff || generation;

        if !any_specific {
            return true;
        }

        match kind {
            QueryRowKind::Target => target,
            QueryRowKind::Domain => domain,
            QueryRowKind::Event => event,
            QueryRowKind::Snapshot => snapshot,
            QueryRowKind::Finding => finding,
            QueryRowKind::Diff => diff,
            QueryRowKind::Generation => generation,
        }
    }

    fn matches_row(&self, row: &QueryRow) -> bool {
        match row.kind {
            QueryRowKind::Target => {
                matches_optional(&self.target_id, row.target_id.as_deref())
                    && matches_detail(&self.target_kind, &row.details, "target_kind")
            }
            QueryRowKind::Domain => {
                matches_optional(&self.domain_id, row.domain_id.as_deref())
                    && matches_detail(&self.domain_kind, &row.details, "domain_kind")
            }
            QueryRowKind::Event => {
                matches_detail(&self.event_id, &row.details, "event_id")
                    && matches_optional(&self.event_identity, row.event_identity.as_deref())
            }
            QueryRowKind::Snapshot => {
                matches_detail(&self.snapshot_kind, &row.details, "snapshot_kind")
                    && matches_detail(&self.probe_id, &row.details, "probe_id")
            }
            QueryRowKind::Finding => {
                matches_detail(&self.finding_id, &row.details, "finding_id")
                    && matches_detail(&self.finding_category, &row.details, "category")
                    && self.matches_finding_severity(row)
            }
            QueryRowKind::Diff => {
                matches_detail(&self.diff_kind, &row.details, "diff_kind")
                    && matches_detail(&self.diff_status, &row.details, "diff_status")
            }
            QueryRowKind::Generation => self
                .generation
                .is_none_or(|expected| row.generation == Some(expected)),
        }
    }

    fn matches_finding_severity(&self, row: &QueryRow) -> bool {
        let Some(severity) = row.severity.as_deref() else {
            return self.finding_severity.is_none() && self.finding_at_or_above.is_none();
        };
        if let Some(expected) = self.finding_severity {
            if !severity.eq_ignore_ascii_case(expected.as_label()) {
                return false;
            }
        }
        if let Some(threshold) = self.finding_at_or_above {
            let Some(actual) = severity_from_label(severity) else {
                return false;
            };
            if !actual.is_at_least(threshold.severity()) {
                return false;
            }
        }
        true
    }
}

fn generation_row(source: &QuerySource) -> QueryRow {
    let label = source
        .generation
        .map(|generation| format!("generation {generation}"))
        .unwrap_or_else(|| "generation unavailable".to_owned());
    QueryRow {
        kind: QueryRowKind::Generation,
        id: source
            .generation
            .map(|generation| generation.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        label,
        generation: source.generation,
        target_id: None,
        domain_id: None,
        event_identity: None,
        severity: None,
        status: None,
        details: json!({
            "artifact_kind": source.artifact_kind,
            "report_source_kind": source.report_source_kind,
            "session_id": source.session_id,
            "generation": source.generation,
            "retained_frames": source.retained_frames,
        }),
    }
}

fn diff_has_change(diff: &DevtoolsCaptureDiff) -> bool {
    diff.rows.iter().any(|row| {
        matches!(
            row.status,
            DevtoolsDiffStatus::Added
                | DevtoolsDiffStatus::Removed
                | DevtoolsDiffStatus::Changed
                | DevtoolsDiffStatus::Collision
        )
    })
}

fn matches_optional(expected: &Option<String>, actual: Option<&str>) -> bool {
    expected
        .as_ref()
        .is_none_or(|expected| actual.is_some_and(|actual| actual.eq_ignore_ascii_case(expected)))
}

fn matches_detail(expected: &Option<String>, details: &Value, key: &str) -> bool {
    expected.as_ref().is_none_or(|expected| {
        details
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
    })
}

fn severity_from_label(label: &str) -> Option<DevtoolsReportSeverity> {
    if label.eq_ignore_ascii_case("info") {
        Some(DevtoolsReportSeverity::Info)
    } else if label.eq_ignore_ascii_case("warning") {
        Some(DevtoolsReportSeverity::Warning)
    } else if label.eq_ignore_ascii_case("error") {
        Some(DevtoolsReportSeverity::Error)
    } else {
        None
    }
}
