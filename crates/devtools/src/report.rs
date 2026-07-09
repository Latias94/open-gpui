//! Machine-readable DevTools reports and diagnostics.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    DevtoolsCapture, DevtoolsDiffStatus, DevtoolsDomainId, DevtoolsDomainKind,
    DevtoolsDomainSnapshot, DevtoolsEventIdentity, DevtoolsSessionExport, DevtoolsSessionFrame,
    DevtoolsTargetId, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
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
    findings.extend(domain_rule_findings(capture));

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

#[derive(Clone, Copy)]
struct ReportRuleScope<'a> {
    identity: &'a str,
    target_id: Option<&'a DevtoolsTargetId>,
    domain_id: Option<&'a DevtoolsDomainId>,
}

impl<'a> ReportRuleScope<'a> {
    fn domain(domain: &'a DevtoolsDomainSnapshot) -> Self {
        Self {
            identity: domain.id.as_str(),
            target_id: Some(&domain.target_id),
            domain_id: Some(&domain.id),
        }
    }

    fn snapshot(snapshot: &'a SnapshotEnvelope) -> Self {
        Self {
            identity: snapshot.probe_id.as_str(),
            target_id: None,
            domain_id: None,
        }
    }
}

fn scoped_finding(
    mut finding: DevtoolsReportFinding,
    scope: ReportRuleScope<'_>,
) -> DevtoolsReportFinding {
    if let Some(target_id) = scope.target_id {
        finding = finding.target_id(target_id.clone());
    }
    if let Some(domain_id) = scope.domain_id {
        finding = finding.domain_id(domain_id.clone());
    }
    finding
}

fn domain_rule_findings(capture: &DevtoolsCapture) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    let mut covered_snapshots = BTreeSet::new();

    for domain in &capture.domains {
        let scope = ReportRuleScope::domain(domain);
        findings.extend(domain_summary_rule_findings(domain, scope));

        if let Some(snapshot) = &domain.snapshot {
            covered_snapshots.insert(snapshot_identity(snapshot));
            findings.extend(snapshot_rule_findings(snapshot, scope, false));
        }
    }

    for snapshot in &capture.snapshots {
        if covered_snapshots.insert(snapshot_identity(snapshot)) {
            let scope = ReportRuleScope::snapshot(snapshot);
            findings.extend(snapshot_rule_findings(snapshot, scope, true));
        }
    }

    findings
}

fn snapshot_identity(snapshot: &SnapshotEnvelope) -> (String, String) {
    (
        snapshot.probe_id.as_str().to_owned(),
        snapshot.kind.as_label().into_owned(),
    )
}

fn domain_summary_rule_findings(
    domain: &DevtoolsDomainSnapshot,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    let Some(summary) = domain.summary.as_ref() else {
        return findings;
    };

    if domain.kind == DevtoolsDomainKind::Command {
        findings.extend(command_summary_rule_findings(summary, scope));
    }

    if domain.kind == DevtoolsDomainKind::Data {
        if looks_like_form_summary(summary) {
            findings.extend(form_summary_rule_findings(summary, scope));
        }
    }

    findings
}

fn snapshot_rule_findings(
    snapshot: &SnapshotEnvelope,
    scope: ReportRuleScope<'_>,
    include_root_summary_rules: bool,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = match &snapshot.kind {
        SnapshotKind::Layout => layout_rule_findings(snapshot, scope),
        SnapshotKind::Timeline => timeline_rule_findings(snapshot, scope),
        SnapshotKind::Motion => motion_rule_findings(snapshot, scope),
        SnapshotKind::Resource => resource_rule_findings(snapshot, scope),
        _ => Vec::new(),
    };

    if include_root_summary_rules {
        for node in &snapshot.tree.nodes {
            if let Some(payload) = node.payload.as_ref() {
                match &snapshot.kind {
                    SnapshotKind::Command => {
                        findings.extend(command_summary_rule_findings(payload, scope));
                    }
                    SnapshotKind::Form if looks_like_form_summary(payload) => {
                        findings.extend(form_summary_rule_findings(payload, scope));
                    }
                    _ => {}
                }
            }
        }
    }

    findings
}

fn layout_rule_findings(
    snapshot: &SnapshotEnvelope,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    for node in snapshot_nodes(snapshot) {
        let Some(payload) = node.payload.as_ref() else {
            continue;
        };

        if payload_path(payload, &["bounds"]).is_some() {
            let width = json_number_at(payload, &["bounds", "size", "width"]);
            let height = json_number_at(payload, &["bounds", "size", "height"]);
            if !is_positive_finite(width) || !is_positive_finite(height) {
                findings.push(scoped_finding(
                    DevtoolsReportFinding::new(
                        format!(
                            "devtools.layout.invalid-bounds.{}.{}",
                            scope.identity, node.id
                        ),
                        DevtoolsReportSeverity::Warning,
                        "layout",
                        "Layout node has invalid bounds",
                        format!(
                            "Layout node `{}` reports non-positive or invalid bounds.",
                            node.id
                        ),
                    )
                    .recommendation(
                        "Check the layout producer before relying on hit-testing or geometry diffs.",
                    ),
                    scope,
                ));
            }
        }

        let scroll_x = json_number_at(payload, &["scroll_offset", "x"]);
        let scroll_y = json_number_at(payload, &["scroll_offset", "y"]);
        let max_x = json_number_at(payload, &["max_scroll_offset", "x"]);
        let max_y = json_number_at(payload, &["max_scroll_offset", "y"]);
        if scroll_axis_out_of_range(scroll_x, max_x) || scroll_axis_out_of_range(scroll_y, max_y) {
            findings.push(scoped_finding(
                DevtoolsReportFinding::new(
                    format!(
                        "devtools.layout.scroll-offset-out-of-range.{}.{}",
                        scope.identity, node.id
                    ),
                    DevtoolsReportSeverity::Warning,
                    "layout",
                    "Layout scroll offset is out of range",
                    format!(
                        "Layout node `{}` reports a scroll offset outside its max scroll offset.",
                        node.id
                    ),
                )
                .recommendation(
                    "Clamp the exported scroll offset or inspect the viewport snapshot generation.",
                ),
                scope,
            ));
        }
    }
    findings
}

fn timeline_rule_findings(
    snapshot: &SnapshotEnvelope,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    for root in &snapshot.tree.nodes {
        timeline_order_rule_findings(&root.children, scope, &mut findings);
    }
    findings
}

fn timeline_order_rule_findings(
    nodes: &[SnapshotNode],
    scope: ReportRuleScope<'_>,
    findings: &mut Vec<DevtoolsReportFinding>,
) {
    let mut previous_order: Option<i64> = None;
    let mut previous_node_id: Option<&str> = None;

    for node in nodes {
        if let Some(order) = node
            .payload
            .as_ref()
            .and_then(|payload| json_i64_at(payload, &["order"]))
        {
            if let Some(previous_order) = previous_order {
                if order < previous_order {
                    findings.push(scoped_finding(
                        DevtoolsReportFinding::new(
                            format!(
                                "devtools.timeline.order-regression.{}.{}",
                                scope.identity, node.id
                            ),
                            DevtoolsReportSeverity::Warning,
                            "timeline",
                            "Timeline event order regressed",
                            format!(
                                "Timeline event `{}` has order {order}, which is lower than the previous sibling order {previous_order}.",
                                node.id
                            ),
                        )
                        .recommendation(format!(
                            "Emit timeline events in monotonic order{}.",
                            previous_node_id
                                .map(|id| format!(" after `{id}`"))
                                .unwrap_or_default()
                        )),
                        scope,
                    ));
                }
            }
            previous_order = Some(order);
            previous_node_id = Some(node.id.as_str());
        }

        timeline_order_rule_findings(&node.children, scope, findings);
    }
}

fn motion_rule_findings(
    snapshot: &SnapshotEnvelope,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    for node in snapshot_nodes(snapshot) {
        let Some(payload) = node.payload.as_ref() else {
            continue;
        };
        if json_str_at(payload, &["last_reset_reason"]) == Some("prune-terminal")
            && node_has_descendant_needing_frame(node)
        {
            findings.push(scoped_finding(
                DevtoolsReportFinding::new(
                    format!(
                        "devtools.motion.terminal-frame-demand.{}.{}",
                        scope.identity, node.id
                    ),
                    DevtoolsReportSeverity::Warning,
                    "motion",
                    "Terminal motion requested another frame",
                    format!(
                        "Motion node `{}` was pruned as terminal but still has a frame demand.",
                        node.id
                    ),
                )
                .recommendation(
                    "Inspect motion retargeting and terminal pruning before scheduling another frame.",
                ),
                scope,
            ));
        }
    }
    findings
}

fn command_summary_rule_findings(
    summary: &Value,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    let conflict_count = json_usize_at(summary, &["conflict_count"]).unwrap_or_default();
    let has_conflicts = json_bool_at(summary, &["has_conflicts"]).unwrap_or(false);
    if has_conflicts || conflict_count > 0 {
        findings.push(scoped_finding(
            DevtoolsReportFinding::new(
                format!("devtools.command.keybinding-conflicts.{}", scope.identity),
                DevtoolsReportSeverity::Warning,
                "command",
                "Command keybindings have conflicts",
                format!(
                    "Command keybinding projection reports {conflict_count} same-context conflict(s)."
                ),
            )
            .recommendation(
                "Resolve duplicate shortcuts or narrow their contexts before shipping the keymap.",
            ),
            scope,
        ));
    }

    let diagnostic_count = json_usize_at(summary, &["diagnostic_count"]).unwrap_or_default();
    if diagnostic_count > 0 {
        findings.push(scoped_finding(
            DevtoolsReportFinding::new(
                format!("devtools.command.keybinding-diagnostics.{}", scope.identity),
                DevtoolsReportSeverity::Warning,
                "command",
                "Command keybindings have diagnostics",
                format!(
                    "Command keybinding projection reports {diagnostic_count} invalid or unresolved binding diagnostic(s)."
                ),
            )
            .recommendation(
                "Inspect missing actions, invalid keystrokes, and invalid keybinding contexts.",
            ),
            scope,
        ));
    }

    if json_bool_at(summary, &["has_pending_commands"]).unwrap_or(false) {
        let pending_count = json_usize_at(summary, &["pending_count"]).unwrap_or_default();
        findings.push(scoped_finding(
            DevtoolsReportFinding::new(
                format!("devtools.command.pending-keymap.{}", scope.identity),
                DevtoolsReportSeverity::Info,
                "command",
                "Command keymap has pending matches",
                format!("Command keymap resolution reports {pending_count} pending command(s)."),
            )
            .recommendation(
                "Keep collecting keystrokes or disambiguate shortcuts that share the same prefix.",
            ),
            scope,
        ));
    }

    findings
}

fn form_summary_rule_findings(
    summary: &Value,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    let error_count = json_usize_at(summary, &["error_count"]).unwrap_or_else(|| {
        payload_path(summary, &["errors"])
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    });
    if error_count > 0 {
        findings.push(scoped_finding(
            DevtoolsReportFinding::new(
                format!("devtools.form.validation-errors.{}", scope.identity),
                DevtoolsReportSeverity::Warning,
                "form",
                "Form has validation errors",
                format!("Form snapshot reports {error_count} validation error(s)."),
            )
            .recommendation("Inspect form-level errors and field meta errors before submitting."),
            scope,
        ));
    }

    if json_str_at(summary, &["status"]) == Some("SubmitFailed") {
        findings.push(scoped_finding(
            DevtoolsReportFinding::new(
                format!("devtools.form.submit-failed.{}", scope.identity),
                DevtoolsReportSeverity::Warning,
                "form",
                "Form submit failed",
                "The latest form submit attempt failed.",
            )
            .recommendation("Inspect the sanitized form error summary and submission lifecycle."),
            scope,
        ));
    }

    findings
}

fn resource_rule_findings(
    snapshot: &SnapshotEnvelope,
    scope: ReportRuleScope<'_>,
) -> Vec<DevtoolsReportFinding> {
    let mut findings = Vec::new();
    for node in snapshot_nodes(snapshot) {
        let Some(payload) = node.payload.as_ref() else {
            continue;
        };
        let error = json_str_at(payload, &["error"]);
        if let Some(error) = error {
            findings.push(scoped_finding(
                DevtoolsReportFinding::new(
                    format!("devtools.resource.error.{}.{}", scope.identity, node.id),
                    DevtoolsReportSeverity::Warning,
                    "resource",
                    "Resource has an error",
                    format!("Resource node `{}` reports error: {error}", node.id),
                )
                .recommendation("Inspect the resource error summary and retry policy."),
                scope,
            ));
        }

        let attempts = json_usize_at(payload, &["fetch_attempts"]).unwrap_or_default();
        if attempts > 1 && error.is_some() {
            findings.push(scoped_finding(
                DevtoolsReportFinding::new(
                    format!("devtools.resource.retrying.{}.{}", scope.identity, node.id),
                    DevtoolsReportSeverity::Info,
                    "resource",
                    "Resource retried after an error",
                    format!(
                        "Resource node `{}` reports {attempts} fetch attempt(s) with an error.",
                        node.id
                    ),
                )
                .recommendation("Check whether the retry policy is expected for this scenario."),
                scope,
            ));
        }
    }
    findings
}

fn snapshot_nodes(snapshot: &SnapshotEnvelope) -> Vec<&SnapshotNode> {
    let mut nodes = Vec::new();
    for node in &snapshot.tree.nodes {
        collect_snapshot_nodes(node, &mut nodes);
    }
    nodes
}

fn collect_snapshot_nodes<'a>(node: &'a SnapshotNode, nodes: &mut Vec<&'a SnapshotNode>) {
    nodes.push(node);
    for child in &node.children {
        collect_snapshot_nodes(child, nodes);
    }
}

fn node_has_descendant_needing_frame(node: &SnapshotNode) -> bool {
    node.children.iter().any(|child| {
        child
            .payload
            .as_ref()
            .and_then(|payload| json_bool_at(payload, &["needs_frame"]))
            .unwrap_or(false)
            || node_has_descendant_needing_frame(child)
    })
}

fn looks_like_form_summary(value: &Value) -> bool {
    payload_path(value, &["field_count"]).is_some()
        || payload_path(value, &["submit_count"]).is_some()
        || payload_path(value, &["errors"]).is_some()
}

fn is_positive_finite(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && value > 0.0)
}

fn scroll_axis_out_of_range(offset: Option<f64>, max: Option<f64>) -> bool {
    match (offset, max) {
        (Some(offset), Some(max)) => {
            offset.is_finite() && max.is_finite() && (offset < 0.0 || offset > max)
        }
        _ => false,
    }
}

fn payload_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn json_number_at(value: &Value, path: &[&str]) -> Option<f64> {
    payload_path(value, path).and_then(Value::as_f64)
}

fn json_i64_at(value: &Value, path: &[&str]) -> Option<i64> {
    let value = payload_path(value, path)?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn json_usize_at(value: &Value, path: &[&str]) -> Option<usize> {
    let value = payload_path(value, path)?;
    value.as_u64().and_then(|value| usize::try_from(value).ok())
}

fn json_bool_at(value: &Value, path: &[&str]) -> Option<bool> {
    payload_path(value, path).and_then(Value::as_bool)
}

fn json_str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    payload_path(value, path).and_then(Value::as_str)
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
