#![cfg(target_os = "windows")]

#[cfg(feature = "legacy-docking")]
#[path = "../src/legacy_style.rs"]
mod legacy_style;

mod native_interactive_tests;
