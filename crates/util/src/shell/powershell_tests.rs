use super::find_powershell;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

fn install(root: &Path, relative_path: &str) -> PathBuf {
    let executable = root.join(relative_path);
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(&executable, []).unwrap();
    executable
}

fn discover(environment: &[(&str, &Path)]) -> Option<PathBuf> {
    find_powershell(
        |key| {
            environment
                .iter()
                .find_map(|(name, path)| (*name == key).then(|| path.as_os_str().to_owned()))
        },
        |_| None,
    )
}

#[test]
fn discovers_highest_stable_program_files_version_before_preview() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), r"PowerShell\6\pwsh.exe");
    let stable = install(root.path(), r"PowerShell\7\pwsh.exe");
    install(root.path(), r"PowerShell\99-preview\pwsh.exe");
    fs::create_dir_all(root.path().join(r"PowerShell\100\pwsh.exe")).unwrap();
    install(root.path(), r"PowerShell\not-a-version\pwsh.exe");

    assert_eq!(discover(&[("ProgramFiles", root.path())]), Some(stable));
}

#[test]
fn discovers_highest_preview_when_stable_is_unavailable() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), r"PowerShell\7-preview\pwsh.exe");
    let preview = install(root.path(), r"PowerShell\8-preview\pwsh.exe");
    install(root.path(), r"PowerShell\99-other\pwsh.exe");

    assert_eq!(discover(&[("ProgramFiles", root.path())]), Some(preview));
}

#[test]
fn prefers_alternate_stable_installation_over_native_preview() {
    let root = tempfile::tempdir().unwrap();
    let native = root.path().join("native");
    let alternate = root.path().join("alternate");
    install(&native, r"PowerShell\99-preview\pwsh.exe");
    let stable = install(&alternate, r"PowerShell\7\pwsh.exe");
    let alternate_variable = if cfg!(target_pointer_width = "64") {
        "ProgramFiles(x86)"
    } else {
        "ProgramW6432"
    };

    assert_eq!(
        discover(&[("ProgramFiles", &native), (alternate_variable, &alternate)]),
        Some(stable),
    );
}

#[test]
fn discovers_official_msix_package_families_only() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        r"Microsoft\WindowsApps\Microsoft.PowerShell_untrusted\pwsh.exe",
    );
    assert_eq!(discover(&[("LOCALAPPDATA", root.path())]), None);

    let preview = install(
        root.path(),
        r"Microsoft\WindowsApps\Microsoft.PowerShellPreview_8wekyb3d8bbwe\pwsh.exe",
    );
    assert_eq!(discover(&[("LOCALAPPDATA", root.path())]), Some(preview));

    let stable = install(
        root.path(),
        r"Microsoft\WindowsApps\Microsoft.PowerShell_8wekyb3d8bbwe\pwsh.exe",
    );
    assert_eq!(discover(&[("LOCALAPPDATA", root.path())]), Some(stable));
}

#[test]
fn custom_scoop_root_overrides_default_user_profile_installation() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    let scoop = root.path().join("custom scoop 中文");
    install(&profile, r"scoop\shims\pwsh.exe");
    let custom = install(&scoop, r"shims\pwsh.exe");

    assert_eq!(
        discover(&[("USERPROFILE", &profile), ("SCOOP", &scoop)]),
        Some(custom),
    );
    assert_eq!(
        discover(&[("SCOOP", &scoop)]),
        Some(scoop.join(r"shims\pwsh.exe")),
    );
}

#[test]
fn default_scoop_root_is_used_only_when_scoop_is_unset() {
    let root = tempfile::tempdir().unwrap();
    let scoop = install(root.path(), r"scoop\shims\pwsh.exe");
    assert_eq!(discover(&[("USERPROFILE", root.path())]), Some(scoop));

    let missing = root.path().join("missing-custom-scoop");
    assert_eq!(
        discover(&[("USERPROFILE", root.path()), ("SCOOP", &missing)]),
        None,
    );
}

#[test]
fn discovers_dotnet_tools_after_scoop() {
    let root = tempfile::tempdir().unwrap();
    let dotnet = install(root.path(), r".dotnet\tools\pwsh.exe");
    fs::create_dir_all(root.path().join(r"scoop\shims\pwsh.exe")).unwrap();
    assert_eq!(discover(&[("USERPROFILE", root.path())]), Some(dotnet));

    let custom_scoop = root.path().join("custom-scoop");
    let scoop = install(&custom_scoop, r"shims\pwsh.exe");
    assert_eq!(
        discover(&[("USERPROFILE", root.path()), ("SCOOP", &custom_scoop)]),
        Some(scoop),
    );
}

#[test]
fn path_powershell_precedes_windows_powershell_fallback() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        r"System32\WindowsPowerShell\v1.0\powershell.exe",
    );
    let path_pwsh = root.path().join("path-pwsh.exe");
    let path_legacy = root.path().join("path-powershell.exe");
    let environment = |key: &str| (key == "SystemRoot").then(|| root.path().as_os_str().to_owned());
    let found = find_powershell(environment, |program| match program {
        "pwsh.exe" => Some(path_pwsh.clone()),
        "powershell.exe" => Some(path_legacy.clone()),
        _ => None,
    });
    assert_eq!(found, Some(path_pwsh));
    assert_eq!(
        find_powershell(environment, |program| {
            (program == "powershell.exe").then(|| path_legacy.clone())
        }),
        Some(path_legacy),
    );
}

#[test]
fn discovers_windows_powershell_without_path() {
    let root = tempfile::tempdir().unwrap();
    let powershell = install(
        root.path(),
        r"System32\WindowsPowerShell\v1.0\powershell.exe",
    );
    assert_eq!(discover(&[("SystemRoot", root.path())]), Some(powershell));
}

#[test]
fn returns_none_when_only_cmd_is_installed() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), r"System32\cmd.exe");
    assert_eq!(discover(&[("SystemRoot", root.path())]), None);
    assert_eq!(find_powershell(|_| None::<OsString>, |_| None), None);
}
