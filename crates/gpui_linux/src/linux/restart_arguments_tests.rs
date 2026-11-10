use super::{new_std_command, restart_command};
use std::{
    ffi::OsString,
    fs,
    os::unix::{
        ffi::{OsStrExt as _, OsStringExt as _},
        fs::symlink,
    },
    path::PathBuf,
};

struct RestartFixture {
    directory: PathBuf,
    executable: PathBuf,
}

impl RestartFixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("open-gpui-restart-tests-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let executable = directory.join(OsString::from_vec(
            b"native executable ' \"$; \xff".to_vec(),
        ));
        symlink("/bin/sh", &executable).unwrap();
        Self {
            directory,
            executable,
        }
    }
}

impl Drop for RestartFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn exited_pid() -> String {
    let mut child = new_std_command("/bin/sh")
        .args(["-c", "exit 0"])
        .spawn()
        .unwrap();
    let pid = child.id().to_string();
    assert!(child.wait().unwrap().success());
    pid
}

#[test]
fn forwards_native_arguments_without_shell_expansion() {
    let fixture = RestartFixture::new();
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
    let expected: Vec<u8> = arguments
        .iter()
        .flat_map(|argument| argument.as_bytes().iter().copied().chain([0]))
        .collect();
    let mut shell_arguments = vec![
        OsString::from("-c"),
        OsString::from(r#"printf '%s\0' "$@""#),
        OsString::from("restart-argument-recorder"),
    ];
    shell_arguments.extend(arguments);

    let output = restart_command(&exited_pid(), &fixture.executable, shell_arguments)
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, expected);
}

#[test]
fn empty_arguments_do_not_reintroduce_the_executable_path() {
    let fixture = RestartFixture::new();
    let output = restart_command(&exited_pid(), &fixture.executable, Vec::new())
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);
    assert!(output.stdout.is_empty());
}
