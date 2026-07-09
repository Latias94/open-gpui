use std::{fs, io::Write, path::Path};

use clap::ValueEnum;
use open_gpui_devtools::{DevtoolsCaptureDiff, DevtoolsReport};
use serde_json::json;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum DevtoolsOutputFormat {
    Json,
    Markdown,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum DevtoolsStreamFormat {
    Jsonl,
    Markdown,
}

pub(super) fn render_report(
    report: &DevtoolsReport,
    format: DevtoolsOutputFormat,
) -> Result<String, ()> {
    match format {
        DevtoolsOutputFormat::Json => serde_json::to_string_pretty(report).map_err(|error| {
            eprintln!("failed to serialize devtools report: {error}");
        }),
        DevtoolsOutputFormat::Markdown => Ok(report.to_markdown()),
    }
}

pub(super) fn render_diff_markdown(diff: &DevtoolsCaptureDiff) -> String {
    let mut output = String::from("# Open GPUI DevTools Diff\n\n");
    output.push_str("| Status | Count |\n|---|---:|\n");
    output.push_str(&format!("| added | {} |\n", diff.summary.added));
    output.push_str(&format!("| removed | {} |\n", diff.summary.removed));
    output.push_str(&format!("| changed | {} |\n", diff.summary.changed));
    output.push_str(&format!("| unchanged | {} |\n", diff.summary.unchanged));
    output.push_str(&format!("| collisions | {} |\n", diff.summary.collisions));
    output.push_str("\n## Rows\n\n");
    output.push_str("| Kind | Status | Identity | Label |\n|---|---|---|---|\n");
    for row in &diff.rows {
        output.push_str(&format!(
            "| {} | {} | `{}` | {} |\n",
            row.kind.as_label(),
            row.status.as_label(),
            row.identity,
            row.label.replace('|', "\\|")
        ));
    }
    output
}

pub(super) fn write_stream_record(
    stdout: &mut impl Write,
    format: DevtoolsStreamFormat,
    sequence: usize,
    report: &DevtoolsReport,
) -> Result<(), ()> {
    match format {
        DevtoolsStreamFormat::Jsonl => {
            let record = json!({
                "schema_version": "open-gpui-devtools-stream/v1",
                "sequence": sequence,
                "kind": "report",
                "report": report,
            });
            writeln!(stdout, "{record}").map_err(|error| {
                eprintln!("failed to write devtools stream record: {error}");
            })?;
        }
        DevtoolsStreamFormat::Markdown => {
            writeln!(stdout, "<!-- devtools-stream sequence={sequence} -->").map_err(|error| {
                eprintln!("failed to write devtools stream marker: {error}");
            })?;
            writeln!(stdout, "{}", report.to_markdown()).map_err(|error| {
                eprintln!("failed to write devtools stream report: {error}");
            })?;
        }
    }
    stdout.flush().map_err(|error| {
        eprintln!("failed to flush devtools stream record: {error}");
    })
}

pub(super) fn write_output(path: Option<&Path>, output: Result<String, ()>) -> Result<(), ()> {
    let output = output?;
    match path.filter(|path| *path != Path::new("-")) {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    eprintln!(
                        "failed to create output directory `{}`: {error}",
                        parent.display()
                    );
                })?;
            }
            fs::write(path, output).map_err(|error| {
                eprintln!("failed to write `{}`: {error}", path.display());
            })
        }
        None => {
            println!("{output}");
            Ok(())
        }
    }
}
