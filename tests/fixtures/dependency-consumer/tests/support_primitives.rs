//! Ordinary support APIs, with process implementation identity checked by the root's Git patch.

use open_gpui::SharedString;

#[test]
fn shared_string_handles_characters_and_serde_without_an_application() {
    let value = SharedString::from('🦀');
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: SharedString = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.as_str(), "🦀");
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn ordinary_dedicated_execution_and_process_interfaces_are_public() {
    use open_gpui::{BackgroundExecutor, DedicatedExecutor, Task};
    let _construct = |background: &BackgroundExecutor| {
        let dedicated = DedicatedExecutor::new(&background.scheduler_executor());
        let task: Task<usize> = dedicated.spawn(async { 42 });
        (dedicated, task)
    };
    let _: fn(
        &mut open_gpui_util::command::Command,
    ) -> std::io::Result<open_gpui_util::command::Child> = open_gpui_util::command::Command::spawn;
    let _: fn(smol::process::Child) -> async_process::Child = std::convert::identity;
    #[cfg(target_os = "macos")]
    let _pipe = |child: &mut open_gpui_util::command::Child| {
        let _: Option<&mut smol::Async<std::fs::File>> = child.stdout.as_mut();
    };
}
