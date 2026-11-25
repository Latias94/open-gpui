use std::{env, fs, path::PathBuf};

const NATIVE_SCENARIO_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_ID";
const NATIVE_SCENARIO_BEHAVIOR_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_BEHAVIOR";
const NATIVE_SCENARIO_RECEIPT_PATH_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_RECEIPT_PATH";
const NATIVE_SCENARIO_RECEIPT_TOKEN_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_RECEIPT_TOKEN";

/// Proves that an exact native test executed the behavior selected by the manifest runner.
///
/// Ordinary package tests do not carry a native scenario environment and therefore remain
/// unaffected. The dedicated runner requires a generation-unique receipt so redirecting a
/// manifest row to another existing test cannot silently satisfy the owning-platform gate.
#[doc(hidden)]
pub fn native_test_confirm_scenario_behavior(expected_behavior: &str) {
    let Ok(scenario_id) = env::var(NATIVE_SCENARIO_ENV) else {
        return;
    };
    let behavior = env::var(NATIVE_SCENARIO_BEHAVIOR_ENV).unwrap_or_else(|error| {
        panic!("native scenario `{scenario_id}` omitted {NATIVE_SCENARIO_BEHAVIOR_ENV}: {error}")
    });
    assert_eq!(
        behavior, expected_behavior,
        "native scenario `{scenario_id}` reached a test for the wrong behavior"
    );
    let receipt_path = PathBuf::from(env::var(NATIVE_SCENARIO_RECEIPT_PATH_ENV).unwrap_or_else(
        |error| {
            panic!(
                "native scenario `{scenario_id}` omitted {NATIVE_SCENARIO_RECEIPT_PATH_ENV}: {error}"
            )
        },
    ));
    let token = env::var(NATIVE_SCENARIO_RECEIPT_TOKEN_ENV).unwrap_or_else(|error| {
        panic!(
            "native scenario `{scenario_id}` omitted {NATIVE_SCENARIO_RECEIPT_TOKEN_ENV}: {error}"
        )
    });
    fs::write(
        &receipt_path,
        format!("{token}\n{scenario_id}\n{behavior}\n"),
    )
    .unwrap_or_else(|error| {
        panic!(
            "native scenario `{scenario_id}` could not publish `{}`: {error}",
            receipt_path.display()
        )
    });
}
