use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub(super) fn get_powershell() -> Option<&'static Path> {
    static POWERSHELL: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
        find_powershell(
            |key| std::env::var_os(key),
            |program| which::which_global(program).ok(),
        )
        .inspect(|path| log::info!("Found PowerShell at {}", path.display()))
    });
    POWERSHELL.as_deref()
}

fn find_powershell(
    var_os: impl Fn(&str) -> Option<OsString>,
    which_global: impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    fn find_pwsh_in_programfiles(
        program_files: Option<OsString>,
        find_preview: bool,
    ) -> Option<PathBuf> {
        let install_base_dir = PathBuf::from(program_files?).join("PowerShell");
        install_base_dir
            .read_dir()
            .ok()?
            .filter_map(Result::ok)
            .filter(|entry| matches!(entry.file_type(), Ok(ft) if ft.is_dir()))
            .filter_map(|entry| {
                let dir_name = entry.file_name();
                let dir_name = dir_name.to_string_lossy();

                let version = if find_preview {
                    let dash_index = dir_name.find('-')?;
                    if &dir_name[dash_index + 1..] != "preview" {
                        return None;
                    };
                    dir_name[..dash_index].parse::<u32>().ok()?
                } else {
                    dir_name.parse::<u32>().ok()?
                };

                let exe_path = entry.path().join("pwsh.exe");
                if exe_path.is_file() {
                    Some((version, exe_path))
                } else {
                    None
                }
            })
            .max_by_key(|(version, _)| *version)
            .map(|(_, path)| path)
    }

    fn find_pwsh_in_msix(local_app_data: Option<OsString>, find_preview: bool) -> Option<PathBuf> {
        let msix_app_dir = PathBuf::from(local_app_data?).join("Microsoft\\WindowsApps");
        let package_family_name = if find_preview {
            "Microsoft.PowerShellPreview_8wekyb3d8bbwe"
        } else {
            "Microsoft.PowerShell_8wekyb3d8bbwe"
        };
        let pwsh_exe = msix_app_dir.join(package_family_name).join("pwsh.exe");
        pwsh_exe.exists().then_some(pwsh_exe)
    }

    fn find_pwsh_in_scoop(
        scoop: Option<OsString>,
        user_profile: Option<OsString>,
    ) -> Option<PathBuf> {
        // Scoop can be installed to a custom location; $SCOOP points at the
        // scoop root in that case and defaults to %USERPROFILE%\scoop otherwise.
        let scoop_dir = scoop
            .map(PathBuf::from)
            .or_else(|| user_profile.map(|home| PathBuf::from(home).join("scoop")))?;
        let pwsh_exe = scoop_dir.join("shims").join("pwsh.exe");
        pwsh_exe.is_file().then_some(pwsh_exe)
    }

    fn find_pwsh_in_dotnet_tools(user_profile: Option<OsString>) -> Option<PathBuf> {
        let pwsh_exe = PathBuf::from(user_profile?).join(".dotnet\\tools\\pwsh.exe");
        pwsh_exe.is_file().then_some(pwsh_exe)
    }

    fn find_windows_powershell(system_root: Option<OsString>) -> Option<PathBuf> {
        let powershell =
            PathBuf::from(system_root?).join("System32\\WindowsPowerShell\\v1.0\\powershell.exe");
        powershell.is_file().then_some(powershell)
    }

    #[cfg(target_pointer_width = "64")]
    let alternate_program_files = "ProgramFiles(x86)";
    #[cfg(target_pointer_width = "32")]
    let alternate_program_files = "ProgramW6432";

    find_pwsh_in_programfiles(var_os("ProgramFiles"), false)
        .or_else(|| find_pwsh_in_programfiles(var_os(alternate_program_files), false))
        .or_else(|| find_pwsh_in_msix(var_os("LOCALAPPDATA"), false))
        .or_else(|| find_pwsh_in_programfiles(var_os("ProgramFiles"), true))
        .or_else(|| find_pwsh_in_msix(var_os("LOCALAPPDATA"), true))
        .or_else(|| find_pwsh_in_programfiles(var_os(alternate_program_files), true))
        .or_else(|| find_pwsh_in_scoop(var_os("SCOOP"), var_os("USERPROFILE")))
        .or_else(|| find_pwsh_in_dotnet_tools(var_os("USERPROFILE")))
        .or_else(|| which_global("pwsh.exe"))
        .or_else(|| which_global("powershell.exe"))
        .or_else(|| find_windows_powershell(var_os("SystemRoot")))
}

#[cfg(test)]
#[path = "powershell_tests.rs"]
mod tests;
