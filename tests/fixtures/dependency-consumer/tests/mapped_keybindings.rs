//! Exercises command preflight with an ordinary App and the real Windows keyboard mapper.
//! Uses ordinary dependencies without GPUI test-support or a replacement mapper, and opens
//! no application Window.
//!
//! The ordinary Windows platform owns a blocking message loop and exits its process on quit,
//! including in headless mode. `Application::run_embedded` enters that same platform loop;
//! it does not provide a returning external-loop integration on this backend. This consumer
//! therefore uses `Application::run` and performs its assertions in the launch callback.
//!
//! A child process isolates native exit from the test harness. After the assertions and
//! shutdown request, the child flushes a completion marker before requesting native quit.
//! The parent requires that marker and successful exit within 30 seconds, so early process
//! exit cannot pass the regression. This qualifies native App/mapper preflight only, not
//! external-loop integration or native Window rendering.

#![cfg(target_os = "windows")]

use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use open_gpui::{Action, Application, KeyContext, Keymap, OperatingSystem, QuitMode};
use open_gpui_command::{
    CommandCenter, CommandContribution, CommandDescriptor, CommandKeyBinding,
    command_shortcut_label, parse_command_key_sequence,
};
use open_gpui_platform::current_platform;
use open_gpui_ui_components::CommandPaletteController;

#[derive(Clone, Debug, Default, PartialEq, Action)]
#[action(no_json)]
struct MappedCommand {
    marker: u32,
}

#[test]
fn downstream_native_mapper_preflights_shifted_single_keys_and_chords() {
    const CHILD: &str = "OPEN_GPUI_MAPPED_KEYBINDINGS_CHILD";
    const COMPLETED: &str = "native mapped keybinding assertions completed";
    if std::env::var_os(CHILD).is_none() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "downstream_native_mapper_preflights_shifted_single_keys_and_chords",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("RUST_BACKTRACE", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the ordinary native App consumer");
        let started = Instant::now();
        let timed_out = loop {
            if child.try_wait().unwrap().is_some() {
                break false;
            }
            if started.elapsed() >= Duration::from_secs(30) {
                child.kill().expect("stop a stalled native App consumer");
                break true;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !timed_out,
            "native App consumer timed out\n{stdout}\n{stderr}"
        );
        assert!(
            output.status.success(),
            "native App consumer failed\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.lines().any(|line| line == COMPLETED),
            "native App exited before completion\n{stdout}\n{stderr}"
        );
        return;
    }

    Application::with_platform(current_platform(true))
        .with_quit_mode(QuitMode::Explicit)
        .run(|cx| {
            assert!(!cx.is_test());
            let os = cx.platform_info().os;
            assert_eq!(os, OperatingSystem::Windows);
            let raw = parse_command_key_sequence("ctrl-shift-4", os).unwrap();
            assert_eq!(raw[0].key, "4");
            assert!(raw[0].modifiers.shift);

            let mut center = CommandCenter::new("native-mapped-bindings");
            center
                .register_source(
                    "mapped",
                    "core",
                    [
                        CommandContribution::new(CommandDescriptor::new(
                            "mapped.single",
                            "Single key",
                        )),
                        CommandContribution::new(CommandDescriptor::new("mapped.chord", "Chord")),
                    ],
                )
                .unwrap();
            center
                .register_action("mapped.single", MappedCommand { marker: 41 })
                .register_action("mapped.chord", MappedCommand { marker: 42 })
                .set_key_contexts([
                    KeyContext::new_with_defaults(os),
                    KeyContext::parse("MappedBindings mode=normal").unwrap(),
                ])
                .register_key_bindings(
                    "defaults",
                    [CommandKeyBinding::new("mapped.single", "ctrl-shift-4")
                        .context("MappedBindings && mode == normal")],
                );

            let mut single_keymap = Keymap::default();
            let single_report = center.install_key_bindings(&mut single_keymap, cx);
            assert!(single_report.is_clean());
            let single_binding = &single_report.key_bindings()[0];
            assert_ne!(
                single_binding.keystrokes()[0].inner(),
                &raw[0],
                "the installed binding must contain actual layout normalization",
            );
            assert!(
                single_binding
                    .action()
                    .partial_eq(&MappedCommand { marker: 41 })
            );
            assert_eq!(
                single_report.projection().projected_entries()[0].raw_context_ref(),
                Some("MappedBindings && mode == normal"),
            );
            let single = center
                .resolve_key_sequence_for_keymap("ctrl-shift-4", &single_keymap, cx)
                .unwrap();
            assert!(!single.is_pending());
            assert_eq!(
                single.primary_dispatchable_command().unwrap().command_id(),
                "mapped.single"
            );
            assert_eq!(
                single.primary_command().unwrap().shortcut(),
                command_shortcut_label(single_binding, os)
            );
            let normalized = [cx
                .keyboard_mapper()
                .map_key_equivalent(raw[0].clone(), false)
                .inner()
                .clone()];
            assert_eq!(
                center.resolve_key_input_for_keymap(&normalized, &single_keymap, os),
                single
            );
            assert!(
                center
                    .resolve_key_input_for_keymap(&raw, &single_keymap, os)
                    .matched_commands()
                    .is_empty(),
                "event-input resolution must not map binding syntax",
            );

            center.register_key_bindings(
                "defaults",
                [
                    CommandKeyBinding::new("mapped.chord", "ctrl-shift-4 ctrl-o")
                        .context("MappedBindings && mode == normal"),
                ],
            );
            let mut chord_keymap = Keymap::default();
            let chord_report = center.install_key_bindings(&mut chord_keymap, cx);
            assert!(chord_report.is_clean());
            let chord_binding = &chord_report.key_bindings()[0];
            assert!(
                chord_binding
                    .action()
                    .partial_eq(&MappedCommand { marker: 42 })
            );
            let controller = CommandPaletteController::new().with_query("mapped");
            let pending = controller
                .preflight_key_sequence_for_keymap(&center, "ctrl-shift-4", &chord_keymap, cx)
                .unwrap();
            assert_eq!(pending.query(), "mapped");
            assert!(pending.is_pending());
            assert!(pending.matched_commands().is_empty());
            assert_eq!(pending.pending_commands()[0].command_id(), "mapped.chord");
            assert_eq!(
                pending.pending_commands()[0].shortcut(),
                command_shortcut_label(chord_binding, os)
            );
            let full = controller
                .preflight_key_sequence_for_keymap(
                    &center,
                    "ctrl-shift-4 ctrl-o",
                    &chord_keymap,
                    cx,
                )
                .unwrap();
            assert!(!full.is_pending());
            assert_eq!(full.primary_dispatchable_command_id(), Some("mapped.chord"));
            let normalized = parse_command_key_sequence("ctrl-shift-4 ctrl-o", os)
                .unwrap()
                .into_iter()
                .map(|stroke| {
                    cx.keyboard_mapper()
                        .map_key_equivalent(stroke, false)
                        .inner()
                        .clone()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                &center.resolve_key_input_for_keymap(&normalized, &chord_keymap, os),
                full.resolution(),
                "backend-normalized event input and binding-syntax preflight must agree",
            );
            assert_eq!(
                center
                    .resolve_key_sequence_for_keymap("ctrl-shift-4 ctrl-o", &chord_keymap, cx)
                    .unwrap(),
                *full.resolution(),
            );
            assert_eq!(
                center
                    .resolve_key_sequence_for_keymap("ctrl-shift-4 ctrl-x-y", &chord_keymap, cx)
                    .unwrap_err()
                    .keystroke,
                "ctrl-x-y",
            );
            cx.shutdown();
            println!("\n{COMPLETED}");
            std::io::stdout().flush().unwrap();
            cx.quit();
        });
}
