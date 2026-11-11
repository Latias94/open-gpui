#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui_log::log_impl;
use std::{io::ErrorKind, path::PathBuf};

mod verbose {
    pub fn emit() {
        super::log_impl::trace!("verbose module message");
    }
}

#[test]
fn host_installs_filters_and_flushes_one_logger() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let output_dir = directory.path().join("not-yet-created");
    let file: &'static PathBuf = Box::leak(Box::new(output_dir.join("consumer.log")));
    open_gpui_log::try_init(Some("info".into()))?;
    assert_eq!(
        open_gpui_log::init_output_file(file, None)
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
    std::fs::create_dir(output_dir)?;
    open_gpui_log::init_output_file(file, None)?;
    assert_eq!(
        open_gpui_log::init_output_file(file, None)
            .unwrap_err()
            .kind(),
        ErrorKind::AlreadyExists
    );
    assert!(open_gpui_log::try_init(None).is_err());

    let settings = [
        ("logging", "info"),
        ("logging::verbose", "trace"),
        ("fixture::muted", "off"),
        ("app_events", "off"),
        ("app_details", "debug"),
        ("logging.transaction", "debug"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect();
    open_gpui_log::filter::refresh_from_settings(&settings);

    verbose::emit();
    log_impl::error!(target: "fixture::muted", "muted module message");
    log_impl::error!(target: "app_events", "muted target message");
    log_impl::debug!(target: "app_details", "verbose target message");
    open_gpui_log::debug!(open_gpui_log::scoped!("transaction") => "scoped application message");
    log_impl::logger().flush();

    let output = std::fs::read_to_string(file)?;
    assert_eq!(
        [
            "verbose module message",
            "verbose target message",
            "scoped application message",
            "muted module message",
            "muted target message",
        ]
        .map(|text| output.contains(text)),
        [true, true, true, false, false],
        "{output}",
    );
    Ok(())
}
