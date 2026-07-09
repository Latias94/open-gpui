use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsReport, DevtoolsSessionExport, DevtoolsSessionImportLimits,
};

#[derive(Args, Clone, Copy, Debug)]
pub(super) struct WaitArgs {
    /// Milliseconds to wait for an artifact to appear or finish writing. Zero is fail-fast.
    #[arg(long, default_value_t = 0)]
    pub(super) timeout_ms: u64,
    /// Poll interval used when --timeout-ms is non-zero.
    #[arg(long, default_value_t = 100)]
    pub(super) poll_ms: u64,
}

pub(super) enum LoadedArtifact {
    Capture(DevtoolsCapture),
    SessionExport(DevtoolsSessionExport),
    Report(DevtoolsReport),
}

pub(super) fn load_report(path: &Path, wait: WaitArgs) -> Result<DevtoolsReport, ()> {
    match load_artifact(path, wait)? {
        LoadedArtifact::Capture(capture) => Ok(DevtoolsReport::from_capture(&capture)),
        LoadedArtifact::SessionExport(export) => Ok(DevtoolsReport::from_session_export(&export)),
        LoadedArtifact::Report(report) => Ok(report),
    }
}

pub(super) fn load_capture(path: &Path, wait: WaitArgs) -> Result<DevtoolsCapture, ()> {
    match load_artifact(path, wait)? {
        LoadedArtifact::Capture(capture) => Ok(capture),
        LoadedArtifact::SessionExport(export) => export
            .frames
            .last()
            .map(|frame| frame.capture.clone())
            .ok_or_else(|| {
                eprintln!(
                    "devtools session export `{}` has no current frame",
                    path.display()
                );
            }),
        LoadedArtifact::Report(_) => {
            eprintln!(
                "devtools report `{}` does not contain raw capture data for diffing",
                path.display()
            );
            Err(())
        }
    }
}

pub(super) fn load_artifact(path: &Path, wait: WaitArgs) -> Result<LoadedArtifact, ()> {
    let timeout = Duration::from_millis(wait.timeout_ms);
    let poll = Duration::from_millis(wait.poll_ms.max(1));
    let started = Instant::now();

    loop {
        match load_artifact_once(path) {
            Ok(artifact) => return Ok(artifact),
            Err(error) => {
                if timeout.is_zero() || started.elapsed() >= timeout {
                    eprintln!("{error}");
                    return Err(());
                }
                thread::sleep(poll);
            }
        }
    }
}

fn load_artifact_once(path: &Path) -> Result<LoadedArtifact, String> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read devtools artifact `{}`: {error}",
            path.display()
        )
    })?;
    if let Ok(report) = serde_json::from_str::<DevtoolsReport>(&source) {
        return Ok(LoadedArtifact::Report(report));
    }

    if let Ok(export) =
        DevtoolsSessionExport::from_json_str(&source, DevtoolsSessionImportLimits::default())
    {
        return Ok(LoadedArtifact::SessionExport(export));
    }

    if let Ok(capture) = serde_json::from_str::<DevtoolsCapture>(&source) {
        return Ok(LoadedArtifact::Capture(capture.sanitized()));
    }

    Err(format!(
        "unsupported devtools artifact `{}`: expected report, session export, or capture JSON",
        path.display()
    ))
}
