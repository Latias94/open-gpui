use super::{RESTART_SCRIPT, encode_restart_arguments, restart_command, spawn_restart};
use std::{
    cell::Cell,
    ffi::{OsStr, OsString},
    io,
    os::windows::ffi::{OsStrExt as _, OsStringExt as _},
    path::Path,
    process::Command,
};
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        UI::Shell::CommandLineToArgvW,
    },
    core::PCWSTR,
};

fn environment<'a>(command: &'a Command, key: &str) -> Option<&'a OsStr> {
    command
        .get_envs()
        .find_map(|(name, value)| (name == key).then_some(value).flatten())
}

#[test]
fn encodes_no_arguments_and_empty_arguments_distinctly() {
    assert_eq!(encode_restart_arguments(&[]), OsStr::new(""));
    assert_eq!(
        encode_restart_arguments(&[OsString::new()]),
        OsStr::new(r#""""#),
    );
    assert_eq!(
        encode_restart_arguments(&[OsString::new(), OsString::new()]),
        OsStr::new(r#""" """#),
    );
}

#[test]
fn encodes_windows_native_argument_boundaries() {
    let cases = [
        ("simple", r#""simple""#),
        ("hello world", r#""hello world""#),
        ("hello\tworld", "\"hello\tworld\""),
        ("中文路径 🦀", r#""中文路径 🦀""#),
        (r#"say "hello""#, r#""say \"hello\"""#),
        (r#"before\"after"#, r#""before\\\"after""#),
        (r#"before\\"after"#, r#""before\\\\\"after""#),
        (r"C:\", r#""C:\\""#),
        (r"C:\Data Files\\", r#""C:\Data Files\\\\""#),
        (r"\\server\share\folder", r#""\\server\share\folder""#),
    ];

    for (argument, expected) in cases {
        assert_eq!(
            encode_restart_arguments(&[OsString::from(argument)]),
            OsStr::new(expected),
            "argument: {argument:?}",
        );
    }

    assert_eq!(
        encode_restart_arguments(&[
            OsString::from("--user-data-dir"),
            OsString::from(r"C:\Open GPUI Data"),
        ]),
        OsStr::new(r#""--user-data-dir" "C:\Open GPUI Data""#),
    );
}

#[test]
fn preserves_unpaired_utf16_surrogates() {
    let argument = OsString::from_wide(&[0xd800, b' ' as u16, 0xdc00, b'\\' as u16]);
    let encoded = encode_restart_arguments(&[argument]);
    assert_eq!(
        encoded.encode_wide().collect::<Vec<_>>(),
        [
            b'"' as u16,
            0xd800,
            b' ' as u16,
            0xdc00,
            b'\\' as u16,
            b'\\' as u16,
            b'"' as u16,
        ],
    );
}

#[test]
fn encoded_arguments_round_trip_through_windows_parser() {
    let cases = [
        Vec::new(),
        vec![OsString::new()],
        vec![
            OsString::new(),
            OsString::from("--user-data-dir"),
            OsString::from("中文路径 🦀"),
            OsString::from("one two\tthree"),
            OsString::from(r#"quotes " and \" and \\""#),
            OsString::from(r"C:\path with spaces\\"),
            OsString::from(r"\\server\share\"),
            OsString::from("$value; $(not-code) `ticks` 'quotes' & | <>"),
            OsString::from_wide(&[0xd800, b'"' as u16, 0xdc00, b'\\' as u16]),
            OsString::new(),
        ],
    ];

    for arguments in cases {
        let mut command_line = OsString::from("restart-test.exe ");
        command_line.push(encode_restart_arguments(&arguments));
        let command_line = command_line
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut count = 0;
        // SAFETY: The command line is a live, NUL-terminated UTF-16 buffer and count is writable.
        let parsed = unsafe { CommandLineToArgvW(PCWSTR(command_line.as_ptr()), &mut count) };
        assert!(!parsed.is_null());
        assert!(count > 0);
        // SAFETY: CommandLineToArgvW returned count valid pointers in its allocated argv buffer.
        let pointers = unsafe { std::slice::from_raw_parts(parsed, count as usize) };
        let decoded = pointers
            .iter()
            .skip(1)
            .map(|argument| {
                // SAFETY: Each argv pointer is NUL-terminated and the allocation is still live.
                OsString::from_wide(unsafe { argument.as_wide() })
            })
            .collect::<Vec<_>>();
        // SAFETY: This is the allocation returned by CommandLineToArgvW, released exactly once.
        let remaining = unsafe { LocalFree(Some(HLOCAL(parsed.cast()))) };
        assert!(remaining.0.is_null());
        assert_eq!(decoded, arguments);
    }
}

#[test]
fn passes_executable_and_arguments_only_in_child_environment() {
    let keys = [
        "OPEN_GPUI_RESTART_PID",
        "OPEN_GPUI_RESTART_EXECUTABLE",
        "OPEN_GPUI_RESTART_ARGUMENTS",
    ];
    let parent_environment = keys.map(std::env::var_os);
    let app_path = Path::new(r"C:\中文 O'Brien\$app`name; & folder\worker.exe");
    let arguments = [
        OsString::from("--data-dir"),
        OsString::from("$data; `literal`"),
    ];
    let command = restart_command(
        OsStr::new(r"C:\PowerShell 7\pwsh.exe"),
        417,
        app_path,
        &arguments,
    );

    assert_eq!(
        command.get_program(),
        OsStr::new(r"C:\PowerShell 7\pwsh.exe")
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [OsStr::new("-command"), OsStr::new(RESTART_SCRIPT)],
    );
    assert_eq!(command.get_envs().count(), 3);
    assert_eq!(environment(&command, keys[0]), Some(OsStr::new("417")));
    assert_eq!(environment(&command, keys[1]), Some(app_path.as_os_str()));
    assert_eq!(
        environment(&command, keys[2]),
        Some(encode_restart_arguments(&arguments).as_os_str()),
    );
    assert_eq!(keys.map(std::env::var_os), parent_environment);
}

#[test]
fn command_preserves_native_program_path_and_arguments() {
    let mut powershell = OsString::from(r"C:\PowerShell\");
    powershell.push(OsString::from_wide(&[0xd800]));
    powershell.push("\\pwsh.exe");
    let mut app_path = OsString::from(r"C:\App\");
    app_path.push(OsString::from_wide(&[0xdc00]));
    app_path.push("\\worker.exe");
    let arguments = [OsString::from_wide(&[0xd800, b' ' as u16, 0xdc00])];
    let command = restart_command(&powershell, 9, Path::new(&app_path), &arguments);

    assert_eq!(command.get_program(), powershell.as_os_str());
    assert_eq!(
        environment(&command, "OPEN_GPUI_RESTART_EXECUTABLE"),
        Some(app_path.as_os_str()),
    );
    assert_eq!(
        environment(&command, "OPEN_GPUI_RESTART_ARGUMENTS"),
        Some(encode_restart_arguments(&arguments).as_os_str()),
    );
}

#[test]
fn restart_commands_keep_payloads_independent() {
    let first = restart_command(
        OsStr::new("pwsh.exe"),
        1,
        Path::new(r"C:\first.exe"),
        &[OsString::from("first argument")],
    );
    let second = restart_command(
        OsStr::new("powershell.exe"),
        2,
        Path::new(r"C:\second.exe"),
        &[],
    );

    assert!(first.get_args().eq(second.get_args()));
    assert_eq!(
        environment(&first, "OPEN_GPUI_RESTART_PID"),
        Some(OsStr::new("1")),
    );
    assert_eq!(
        environment(&second, "OPEN_GPUI_RESTART_PID"),
        Some(OsStr::new("2")),
    );
    assert_eq!(
        environment(&first, "OPEN_GPUI_RESTART_ARGUMENTS"),
        Some(OsStr::new(r#""first argument""#)),
    );
    assert_eq!(
        environment(&second, "OPEN_GPUI_RESTART_ARGUMENTS"),
        Some(OsStr::new("")),
    );
}

#[test]
fn script_clears_restart_environment_before_waiting_and_launching() {
    let wait = RESTART_SCRIPT.find("while ($true)").unwrap();
    let launch = RESTART_SCRIPT.find("Start-Process").unwrap();
    for (variable, local) in [
        ("OPEN_GPUI_RESTART_PID", "$pidToWaitFor"),
        ("OPEN_GPUI_RESTART_EXECUTABLE", "$exePath"),
        ("OPEN_GPUI_RESTART_ARGUMENTS", "$argumentList"),
    ] {
        let read = RESTART_SCRIPT
            .find(&format!("{local} = $env:{variable}"))
            .unwrap();
        let clear = RESTART_SCRIPT
            .find(&format!(
                "Remove-Item -LiteralPath Env:{variable} -ErrorAction SilentlyContinue"
            ))
            .unwrap();
        assert!(read < clear && clear < wait && wait < launch);
    }
    assert!(RESTART_SCRIPT.contains("[string]::IsNullOrEmpty($argumentList)"));
    assert!(RESTART_SCRIPT.contains("Start-Process -FilePath $exePath -WindowStyle Hidden"));
    assert!(RESTART_SCRIPT.contains(
        "Start-Process -FilePath $exePath -ArgumentList $argumentList -WindowStyle Hidden",
    ));
    assert!(RESTART_SCRIPT.contains("Get-Process -Id $pidToWaitFor -ErrorAction SilentlyContinue"));
}

#[test]
fn missing_powershell_does_not_quit() {
    let quit = Cell::new(false);
    let error = spawn_restart(None, 1, Path::new(r"C:\worker.exe"), &[], || quit.set(true))
        .expect_err("restart must fail without PowerShell");

    assert!(error.to_string().contains("PowerShell is unavailable"));
    assert!(!quit.get());
}

#[test]
fn failed_helper_spawn_does_not_quit() {
    let powershell = std::env::temp_dir().join(format!(
        "open-gpui-missing-powershell-{}.exe",
        uuid::Uuid::new_v4(),
    ));
    let quit = Cell::new(false);
    let error = spawn_restart(
        Some(powershell),
        1,
        Path::new(r"C:\worker.exe"),
        &[],
        || quit.set(true),
    )
    .expect_err("restart must fail when the helper cannot spawn");

    assert_eq!(error.to_string(), "failed to spawn restart script");
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::NotFound,
    );
    assert!(!quit.get());
}

#[test]
fn invalid_native_payload_does_not_quit() {
    let quit = Cell::new(false);
    let error = spawn_restart(
        Some(std::env::current_exe().unwrap()),
        1,
        Path::new(r"C:\worker.exe"),
        &[OsString::from("before\0after")],
        || quit.set(true),
    )
    .expect_err("restart must reject an environment value containing NUL");

    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::InvalidInput,
    );
    assert!(!quit.get());
}
