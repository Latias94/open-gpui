use anyhow::{Context as _, Result};
use std::{
    ffi::{OsStr, OsString},
    os::windows::ffi::{OsStrExt as _, OsStringExt as _},
    path::{Path, PathBuf},
    process::Command,
};

const RESTART_SCRIPT: &str = r#"
            $pidToWaitFor = $env:OPEN_GPUI_RESTART_PID
            $exePath = $env:OPEN_GPUI_RESTART_EXECUTABLE
            $argumentList = $env:OPEN_GPUI_RESTART_ARGUMENTS

            Remove-Item -LiteralPath Env:OPEN_GPUI_RESTART_PID -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath Env:OPEN_GPUI_RESTART_EXECUTABLE -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath Env:OPEN_GPUI_RESTART_ARGUMENTS -ErrorAction SilentlyContinue

            while ($true) {
                $process = Get-Process -Id $pidToWaitFor -ErrorAction SilentlyContinue
                if (-not $process) {
                    if ([string]::IsNullOrEmpty($argumentList)) {
                        Start-Process -FilePath $exePath -WindowStyle Hidden
                    } else {
                        Start-Process -FilePath $exePath -ArgumentList $argumentList -WindowStyle Hidden
                    }
                    break
                }
                Start-Sleep -Seconds 0.1
            }
            "#;

fn encode_restart_arguments(arguments: &[OsString]) -> OsString {
    // `Start-Process` accepts a single native command line, so quote each argument according to
    // the Windows argv parsing rules before passing the complete string through the environment.
    let mut encoded = Vec::new();

    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            encoded.push(b' ' as u16);
        }
        encoded.push(b'"' as u16);

        let mut backslash_count = 0;
        for code_unit in argument.encode_wide() {
            if code_unit == b'\\' as u16 {
                backslash_count += 1;
            } else {
                if code_unit == b'"' as u16 {
                    encoded.extend(std::iter::repeat_n(b'\\' as u16, backslash_count * 2 + 1));
                } else {
                    encoded.extend(std::iter::repeat_n(b'\\' as u16, backslash_count));
                }
                backslash_count = 0;
                encoded.push(code_unit);
            }
        }

        encoded.extend(std::iter::repeat_n(b'\\' as u16, backslash_count * 2));
        encoded.push(b'"' as u16);
    }

    OsString::from_wide(&encoded)
}

fn restart_command(
    powershell: &OsStr,
    pid: u32,
    app_path: &Path,
    arguments: &[OsString],
) -> Command {
    let mut command = ::open_gpui_util::command::new_std_command(powershell);
    command
        .arg("-command")
        .arg(RESTART_SCRIPT)
        .env("OPEN_GPUI_RESTART_PID", pid.to_string())
        .env("OPEN_GPUI_RESTART_EXECUTABLE", app_path)
        .env(
            "OPEN_GPUI_RESTART_ARGUMENTS",
            encode_restart_arguments(arguments),
        );
    command
}

pub(super) fn spawn_restart(
    powershell: Option<PathBuf>,
    pid: u32,
    app_path: &Path,
    arguments: &[OsString],
    quit: impl FnOnce(),
) -> Result<()> {
    let powershell = powershell.context("failed to restart: PowerShell is unavailable")?;
    let mut command = restart_command(powershell.as_os_str(), pid, app_path, arguments);
    #[allow(
        clippy::disallowed_methods,
        reason = "We are restarting ourselves, using std command thus is fine"
    )]
    command.spawn().context("failed to spawn restart script")?;
    quit();
    Ok(())
}

#[cfg(test)]
#[path = "restart_tests.rs"]
mod tests;
