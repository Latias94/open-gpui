use super::{new_std_command, restart_command};
use std::{
    ffi::OsString,
    fs,
    os::unix::{
        ffi::{OsStrExt as _, OsStringExt as _},
        fs::PermissionsExt as _,
    },
    path::{Path, PathBuf},
    process::Command,
};

struct RestartFixture {
    directory: PathBuf,
}

impl RestartFixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("open-gpui-restart-tests-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let open = directory.join("open");
        fs::write(&open, "#!/bin/sh\nprintf '%s\\0' \"$@\"\n").unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o700)).unwrap();
        Self { directory }
    }

    fn command(&self, app_path: &Path, arguments: Vec<OsString>) -> Command {
        let mut child = new_std_command("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let pid = child.id().to_string();
        assert!(child.wait().unwrap().success());
        let mut command = restart_command(&pid, app_path, arguments);
        command.env(
            "PATH",
            std::env::join_paths([
                self.directory.as_path(),
                Path::new("/usr/bin"),
                Path::new("/bin"),
            ])
            .unwrap(),
        );
        command
    }
}

impl Drop for RestartFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[test]
fn forwards_native_arguments_after_open_args_for_bundles_and_executables() {
    let fixture = RestartFixture::new();
    for app_path in [
        PathBuf::from("/Applications/Open GPUI ' \"$; bundle.app"),
        PathBuf::from(OsString::from_vec(b"/tmp/native-\xff executable".to_vec())),
    ] {
        let mut arguments: Vec<OsString> = [
            "--user-data-dir",
            "/tmp/data with spaces/\\'\"",
            "",
            "文書 🦀",
            "line\nbreak",
            "$(exit 97); `exit 98` $HOME * ? [abc]",
            "-n",
            "--",
            "trailing\\",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        arguments.push(OsString::from_vec(b"native-\xff-argument".to_vec()));
        let mut expected = vec![app_path.as_os_str().to_owned(), OsString::from("--args")];
        expected.extend(arguments.iter().cloned());
        let expected: Vec<u8> = expected
            .iter()
            .flat_map(|argument| argument.as_bytes().iter().copied().chain([0]))
            .collect();

        let output = fixture.command(&app_path, arguments).output().unwrap();

        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn empty_arguments_omit_open_args_for_bundles_and_executables() {
    let fixture = RestartFixture::new();
    for app_path in [
        PathBuf::from("/Applications/Open GPUI.app"),
        PathBuf::from("/tmp/Open GPUI executable"),
    ] {
        let output = fixture.command(&app_path, Vec::new()).output().unwrap();
        let mut expected = app_path.as_os_str().as_bytes().to_vec();
        expected.push(0);

        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, expected);
    }
}
