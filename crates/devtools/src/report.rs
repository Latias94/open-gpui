//! Machine-readable DevTools reports and diagnostics.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsCapture, DevtoolsDiffStatus, DevtoolsDomainId, DevtoolsEventIdentity,
    DevtoolsSessionExport, DevtoolsSessionFrame, DevtoolsTargetId, SnapshotDiagnostic,
    adapters::sanitize_sensitive_text,
};

/// Schema version used by serialized DevTools reports.
pub const DEVTOOLS_REPORT_SCHEMA_VERSION: &str = "open-gpui-devtools-report/v1";

/// Source artifact used to produce a DevTools report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DevtoolsReportSourceKind {
    /// The report was produced directly from one capture.
    Capture,
    /// The report was produced from one session frame.
    SessionFrame,
    /// The report was produced from the current frame in a session export.
    SessionExport,
}

impl DevtoolsReportSourceKind {
    /// Returns the stable label for this report source kind.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::SessionFrame => "session-frame",
            Self::SessionExport => "session-export",
        }
    }
}

/// Metadata describing the source artifact for a DevTools report.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsReportSource {
    /// Source artifact kind.
    pub kind: DevtoolsReportSourceKind,
    /// Sanitized session id, when the source is session-scoped.
    pub session_id: Option<String>,
    /// Current generation, when the source is session-scoped.
    pub generation: Option<u64>,
    /// Previous generation used for diffing, when available.
    pub previous_generation: Option<u64>,
    /// Retained frame count, when the source is a session export.
    pub retained_frames: Option<usize>,
}

impl DevtoolsReportSource {
    fn capture() -> Self {
        Self {
            kind: DevtoolsReportSourceKind::Capture,
            ..Self::default()
        }
    }

    fn session_frame(frame: &DevtoolsSessionFrame) -> Self {
        Self {
            kind: DevtoolsReportSourceKind::SessionFrame,
            session_id: Some(sanitize_sensitive_text(&frame.session_id)),
            generation: Some(frame.generation),
            previous_generation: frame.previous_generation,
            retained_frames: None,
        }
    }

    fn session_export(export: &DevtoolsSessionExport) -> Self {
        let current = export.frames.last();
        Self {
            kind: DevtoolsReportSourceKind::SessionExport,
            session_id: Some(sanitize_sensitive_text(&export.session_id)),
            generation: export.current_generation,
            previous_generation: current.and_then(|frame| frame.previous_generation),
            retained_frames: Some(export.retained_frames),
        }
    }
}

impl Default for DevtoolsReportSourceKind {
    fn default() -> Self {
        Self::Capture
    }
}

/// Severity for a DevTools report finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DevtoolsReportSeverity {
    /// Informational finding.
    Info,
    /// Warning finding that should be investigated.
    Warning,
    /// Error finding that usually indicates broken or unsafe diagnostics.
    Error,
}

impl DevtoolsReportSeverity {
    /// Returns the stable label for this severity.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Returns true when this severity is at least `threshold`.
    pub const fn is_at_least(self, threshold: Self) -> bool {
        severity_rank(self) >= severity_rank(threshold)
    }
}

const fn severity_rank(severity: DevtoolsReportSeverity) -> u8 {
    match severity {
        DevtoolsReportSeverity::Info => 1,
        DevtoolsReportSeverity::Warning => 2,
        DevtoolsReportSeverity::Error => 3,
    }
}

/// One machine-readable DevTools report finding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsReportFinding {
    /// Stable finding id.
    pub id: String,
    /// Finding severity.
    pub severity: DevtoolsReportSeverity,
    /// Broad finding category.
    pub category: String,
    /// Human-readable title.
    pub title: String,
    /// Human-readable message.
    pub message: String,
    /// Target associated with the finding, when known.
    pub target_id: Option<DevtoolsTargetId>,
    /// Domain associated with the finding, when known.
    pub domain_id: Option<DevtoolsDomainId>,
    /// Event associated with the finding, when known.
    pub event_identity: Option<DevtoolsEventIdentity>,
    /// Recommended next diagnostic action.
    pub recommendation: Option<String>,
}

impl DevtoolsReportFinding {
    fn new(
        id: impl Into<String>,
        severity: DevtoolsReportSeverity,
        category: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: sanitize_sensitive_text(&id.into()),
            severity,
            category: sanitize_sensitive_text(&category.into()),
            title: sanitize_sensitive_text(&title.into()),
            message: sanitize_sensitive_text(&message.into()),
            target_id: None,
            domain_id: None,
            event_identity: None,
            recommendation: None,
        }
    }

    fn target_id(mut self, target_id: DevtoolsTargetId) -> Self {
        self.target_id = Some(target_id);
        self
    }

    fn domain_id(mut self, domain_id: DevtoolsDomainId) -> Self {
        self.domain_id = Some(domain_id);
        self
    }

    fn event_identity(mut self, event_identity: DevtoolsEventIdentity) -> Self {
        self.event_identity = Some(event_identity);
        self
    }

    fn recommendation(mut self, recommendation: impl Into<String>) -> Self {
        self.recommendation = Some(sanitize_sensitive_text(&recommendation.into()));
        self
    }
}

/// Aggregate counts for a DevTools report.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsReportSummary {
    /// Target count in the current capture.
    pub target_count: usize,
    /// Domain count in the current capture.
    pub domain_count: usize,
    /// Event count in the current capture.
    pub event_count: usize,
    /// Legacy snapshot count in the current capture.
    pub snapshot_count: usize,
    /// Capture-level and domain-level diagnostic count.
    pub diagnostic_count: usize,
    /// Redacted value count across legacy snapshots.
    pub redacted_value_count: usize,
    /// Diff row count when a previous frame exists.
    pub diff_row_count: usize,
    /// Added diff row count.
    pub added_diff_rows: usize,
    /// Removed diff row count.
    pub removed_diff_rows: usize,
    /// Changed diff row count.
    pub changed_diff_rows: usize,
    /// Collision diff row count.
    pub collision_diff_rows: usize,
    /// Total finding count.
    pub finding_count: usize,
    /// Error finding count.
    pub error_count: usize,
    /// Warning finding count.
    pub warning_count: usize,
    /// Informational finding count.
    pub info_count: usize,
}

/// Machine-readable report over a DevTools capture or session frame.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsReport {
    /// Report schema version.
    pub schema_version: String,
    /// Source artifact metadata.
    pub source: DevtoolsReportSource,
    /// Aggregate counts.
    pub summary: DevtoolsReportSummary,
    /// Findings derived from diagnostics and structural validation.
    pub findings: Vec<DevtoolsReportFinding>,
}

impl DevtoolsReport {
    /// Builds a report from one sanitized capture.
    pub fn from_capture(capture: &DevtoolsCapture) -> Self {
        Self::from_capture_parts(DevtoolsReportSource::capture(), capture, None)
    }

    /// Builds a report from one sanitized session frame.
    pub fn from_session_frame(frame: &DevtoolsSessionFrame) -> Self {
        Self::from_capture_parts(
            DevtoolsReportSource::session_frame(frame),
            &frame.capture,
            frame.diff_from_previous.as_ref(),
        )
    }

    /// Builds a report from the current frame in a sanitized session export.
    pub fn from_session_export(export: &DevtoolsSessionExport) -> Self {
        match export.frames.last() {
            Some(frame) => {
                let mut report = Self::from_session_frame(frame);
                report.source = DevtoolsReportSource::session_export(export);
                report
            }
            None => {
                let mut report = Self::from_capture_parts(
                    DevtoolsReportSource::session_export(export),
                    &DevtoolsCapture::default(),
                    None,
                );
                report.findings.push(
                    DevtoolsReportFinding::new(
                        "devtools.session.no-current-frame",
                        DevtoolsReportSeverity::Warning,
                        "session",
                        "Session export has no current frame",
                        "The session export contains no retained frames to inspect.",
                    )
                    .recommendation(
                        "Collect at least one DevTools session frame before exporting.",
                    ),
                );
                report.finish_counts();
                report
            }
        }
    }

    fn from_capture_parts(
        source: DevtoolsReportSource,
        capture: &DevtoolsCapture,
        diff: Option<&crate::DevtoolsCaptureDiff>,
    ) -> Self {
        let capture = capture.clone().sanitized();
        let mut report = Self {
            schema_version: DEVTOOLS_REPORT_SCHEMA_VERSION.to_owned(),
            source,
            summary: summary_for_capture(&capture, diff),
            findings: findings_for_capture(&capture, diff),
        };
        report.finish_counts();
        report
    }

    /// Returns true when a finding at or above `threshold` exists.
    pub fn has_finding_at_or_above(&self, threshold: DevtoolsReportSeverity) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity.is_at_least(threshold))
    }

    /// Renders a compact markdown report for logs, issues, and local debugging.
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();
        output.push_str("# Open GPUI DevTools Report\n\n");
        output.push_str(&format!(
            "- Schema: `{}`\n- Source: `{}`\n",
            self.schema_version,
            self.source.kind.as_label()
        ));
        if let Some(session_id) = &self.source.session_id {
            output.push_str(&format!("- Session: `{session_id}`\n"));
        }
        if let Some(generation) = self.source.generation {
            output.push_str(&format!("- Generation: `{generation}`\n"));
        }
        if let Some(previous_generation) = self.source.previous_generation {
            output.push_str(&format!("- Previous generation: `{previous_generation}`\n"));
        }
        if let Some(retained_frames) = self.source.retained_frames {
            output.push_str(&format!("- Retained frames: `{retained_frames}`\n"));
        }

        output.push_str("\n## Summary\n\n");
        output.push_str("| Metric | Count |\n|---|---:|\n");
        for (metric, count) in [
            ("targets", self.summary.target_count),
            ("domains", self.summary.domain_count),
            ("events", self.summary.event_count),
            ("snapshots", self.summary.snapshot_count),
            ("diagnostics", self.summary.diagnostic_count),
            ("redacted values", self.summary.redacted_value_count),
            ("diff rows", self.summary.diff_row_count),
            ("findings", self.summary.finding_count),
            ("errors", self.summary.error_count),
            ("warnings", self.summary.warning_count),
            ("info", self.summary.info_count),
        ] {
            output.push_str(&format!("| {metric} | {count} |\n"));
        }

        if self.summary.diff_row_count > 0 {
            output.push_str("\n## Diff\n\n");
            output.push_str("| Status | Count |\n|---|---:|\n");
            output.push_str(&format!("| added | {} |\n", self.summary.added_diff_rows));
            output.push_str(&format!(
                "| removed | {} |\n",
                self.summary.removed_diff_rows
            ));
            output.push_str(&format!(
                "| changed | {} |\n",
                self.summary.changed_diff_rows
            ));
            output.push_str(&format!(
                "| collisions | {} |\n",
                self.summary.collision_diff_rows
            ));
        }

        output.push_str("\n## Findings\n\n");
        if self.findings.is_empty() {
            output.push_str("No findings.\n");
            return output;
        }

        output.push_str("| Severity | ID | Target | Domain | Message |\n|---|---|---|---|---|\n");
        for finding in &self.findings {
            let target = finding
                .target_id
                .as_ref()
                .map(|target| target.as_str())
                .unwrap_or("-");
            let domain = finding
                .domain_id
                .as_ref()
                .map(|domain| domain.as_str())
                .unwrap_or("-");
            output.push_str(&format!(
                "| {} | `{}` | `{}` | `{}` | {} |\n",
                finding.severity.as_label(),
                finding.id,
                target,
                domain,
                finding.message.replace('|', "\\|")
            ));
        }

        output
    }

    fn finish_counts(&mut self) {
        self.summary.finding_count = self.findings.len();
        self.summary.error_count = self
            .findings
            .iter()
            .filter(|finding| finding.severity == DevtoolsReportSeverity::Error)
            .count();
        self.summary.warning_count = self
            .findings
            .iter()
            .filter(|finding| finding.severity == DevtoolsReportSeverity::Warning)
            .count();
        self.summary.info_count = self
            .findings
            .iter()
            .filter(|finding| finding.severity == DevtoolsReportSeverity::Info)
            .count();
    }
}

fn summary_for_capture(
    capture: &DevtoolsCapture,
    diff: Option<&crate::DevtoolsCaptureDiff>,
) -> DevtoolsReportSummary {
    let domain_diagnostics = capture
        .domains
        .iter()
        .map(|domain| domain.diagnostics.len())
        .sum::<usize>();
    let redacted_value_count = capture
        .snapshots
        .iter()
        .map(|snapshot| snapshot.redaction.redacted_values)
        .sum();
    let mut summary = DevtoolsReportSummary {
        target_count: capture.targets.targets.len(),
        domain_count: capture.domains.len(),
        event_count: capture.events.len(),
        snapshot_count: capture.snapshots.len(),
        diagnostic_count: capture.diagnostics.len() + domain_diagnostics,
        redacted_value_count,
        ..DevtoolsReportSummary::default()
    };

    if let Some(diff) = diff {
        summary.diff_row_count = diff.rows.len();
        summary.added_diff_rows = diff.summary.added;
        summary.removed_diff_rows = diff.summary.removed;
        summary.changed_diff_rows = diff.summary.changed;
        summary.collision_diff_rows = diff.summary.collisions;
    }

    summary
}

fn findings_for_capture(
    capture: &DevtoolsCapture,
    diff: Option<&crate::DevtoolsCaptureDiff>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    if capture.targets.targets.is_empty()
        && capture.domains.is_empty()
        && capture.events.is_empty()
        && capture.snapshots.is_empty()
    {
        findings.push(
            DevtoolsReportFinding::new(
                "devtools.capture.empty",
                DevtoolsReportSeverity::Warning,
                "capture",
                "Capture has no inspectable data",
                "The capture contains no targets, domains, events, or snapshots.",
            )
            .recommendation("Register at least one capture provider or snapshot probe."),
        );
    }

    for diagnostic in &capture.diagnostics {
        findings.push(finding_for_diagnostic(
            diagnostic,
            DevtoolsReportSeverity::Warning,
            "capture-diagnostic",
        ));
    }

    for domain in &capture.domains {
        for diagnostic in &domain.diagnostics {
            findings.push(
                finding_for_diagnostic(
                    diagnostic,
                    DevtoolsReportSeverity::Warning,
                    "domain-diagnostic",
                )
                .target_id(domain.target_id.clone())
                .domain_id(domain.id.clone()),
            );
        }
    }

    findings.extend(structural_findings(capture));

    if let Some(diff) = diff {
        for row in diff
            .rows
            .iter()
            .filter(|row| row.status == DevtoolsDiffStatus::Collision)
        {
            findings.push(
                DevtoolsReportFinding::new(
                    format!("devtools.diff.collision.{}", row.identity),
                    DevtoolsReportSeverity::Error,
                    "diff",
                    "Diff identity collision",
                    format!(
                        "{} `{}` has a sanitized identity collision.",
                        row.kind.as_label(),
                        row.identity
                    ),
                )
                .recommendation("Make the underlying DevTools identity stable and non-sensitive."),
            );
        }
    }

    findings
}

fn finding_for_diagnostic(
    diagnostic: &SnapshotDiagnostic,
    default_severity: DevtoolsReportSeverity,
    category: &str,
) -> DevtoolsReportFinding {
    let severity = if diagnostic.code == SnapshotDiagnostic::COLLECTION_FAILED
        || diagnostic.code.contains("collision")
        || diagnostic.code.contains("missing")
    {
        DevtoolsReportSeverity::Error
    } else {
        default_severity
    };

    DevtoolsReportFinding::new(
        format!("{}.{}", category, diagnostic.code),
        severity,
        category,
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    )
    .recommendation("Inspect the producer that emitted this DevTools diagnostic.")
}

fn structural_findings(capture: &DevtoolsCapture) -> Vec<DevtoolsReportFinding> {
    let target_ids = capture
        .targets
        .targets
        .iter()
        .map(|target| target.id.clone())
        .collect::<BTreeSet<_>>();
    let domain_ids = capture
        .domains
        .iter()
        .map(|domain| domain.id.clone())
        .collect::<BTreeSet<_>>();
    let mut findings = Vec::new();

    for target in &capture.targets.targets {
        if let Some(parent_id) = &target.parent_id {
            if !target_ids.contains(parent_id) {
                findings.push(
                    DevtoolsReportFinding::new(
                        format!("devtools.target.missing-parent.{}", target.id.as_str()),
                        DevtoolsReportSeverity::Error,
                        "target",
                        "Target parent is missing",
                        format!(
                            "Target `{}` references missing parent `{}`.",
                            target.id.as_str(),
                            parent_id.as_str()
                        ),
                    )
                    .target_id(target.id.clone())
                    .recommendation("Emit parent targets before child targets in the capture."),
                );
            }
        }
    }

    for domain in &capture.domains {
        if !target_ids.contains(&domain.target_id) {
            findings.push(
                DevtoolsReportFinding::new(
                    format!("devtools.domain.missing-target.{}", domain.id.as_str()),
                    DevtoolsReportSeverity::Error,
                    "domain",
                    "Domain target is missing",
                    format!(
                        "Domain `{}` references missing target `{}`.",
                        domain.id.as_str(),
                        domain.target_id.as_str()
                    ),
                )
                .target_id(domain.target_id.clone())
                .domain_id(domain.id.clone())
                .recommendation("Attach each domain to an emitted target."),
            );
        }
    }

    for event in &capture.events {
        if let Some(target_id) = event.target_id_ref() {
            if !target_ids.contains(target_id) {
                findings.push(
                    DevtoolsReportFinding::new(
                        format!(
                            "devtools.event.missing-target.{}",
                            event.identity().as_key()
                        ),
                        DevtoolsReportSeverity::Warning,
                        "event",
                        "Event target is missing",
                        format!(
                            "Event `{}` references missing target `{}`.",
                            event.id(),
                            target_id.as_str()
                        ),
                    )
                    .target_id(target_id.clone())
                    .event_identity(event.identity())
                    .recommendation(
                        "Attach event records to emitted targets or omit the target id.",
                    ),
                );
            }
        }
        if let Some(domain_id) = event.domain_id_ref() {
            if !domain_ids.contains(domain_id) {
                findings.push(
                    DevtoolsReportFinding::new(
                        format!(
                            "devtools.event.missing-domain.{}",
                            event.identity().as_key()
                        ),
                        DevtoolsReportSeverity::Warning,
                        "event",
                        "Event domain is missing",
                        format!(
                            "Event `{}` references missing domain `{}`.",
                            event.id(),
                            domain_id.as_str()
                        ),
                    )
                    .domain_id(domain_id.clone())
                    .event_identity(event.identity())
                    .recommendation(
                        "Attach event records to emitted domains or omit the domain id.",
                    ),
                );
            }
        }
    }

    findings
}
