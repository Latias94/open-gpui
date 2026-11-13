"""Run a built macOS native probe through LaunchServices and remove its temporary app."""

import argparse
from contextlib import closing
import os
from pathlib import Path
import plistlib
import re
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def success_marker(executable, arguments):
    if executable == "native_windowing":
        mode = arguments[0] if arguments else "default"
        return {
            "activity": "native platform idle-sleep consumer completed",
            "native-close": "native close gate passed",
        }.get(mode, f"native windowing {mode} gate passed")
    return {
        "open-gpui-dockspace-editor": "DOCKSPACE NATIVE SMOKE OK:",
        "native_layer_shutdown": "NATIVE LAYER SHUTDOWN OK:",
        "native_window_appearance": (
            "native application appearance, inheritance, callback, and cleanup probe passed"
        ),
        "native_termination": "NATIVE TERMINATION: real AppKit terminal observed",
    }.get(executable)


def matching_process(native):
    processes = subprocess.check_output(
        ["ps", "-axo", "pid=,command="], text=True, timeout=5
    )
    native_paths = (str(native), str(native.resolve()))
    for line in processes.splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) == 2 and any(
            fields[1] == native_path or fields[1].startswith(native_path + " ")
            for native_path in native_paths
        ):
            return int(fields[0])
    return None


def wait_for_native_exit(command, native):
    # NOTE_EXITSTATUS is available to observers allowed to signal the target PID.
    # Python exposes NOTE_EXIT but does not name Darwin's exit-status flag.
    note_exitstatus = 0x04000000
    deadline = time.monotonic() + 45
    pid = None
    native_exited = False
    launcher = subprocess.Popen(command)
    try:
        with closing(select.kqueue()) as queue:
            while launcher.poll() is None and time.monotonic() < deadline:
                pid = matching_process(native)
                if pid is not None:
                    break
                time.sleep(0.01)
            if pid is None:
                raise RuntimeError("the native process could not be observed before exit or timeout")
            event = select.kevent(
                pid,
                filter=select.KQ_FILTER_PROC,
                flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
                fflags=select.KQ_NOTE_EXIT | note_exitstatus,
            )
            events = queue.control([event], 1, max(0, deadline - time.monotonic()))
            if not events:
                raise RuntimeError("the native process exceeded 45 seconds")
            event = events[0]
            if event.flags & select.KQ_EV_ERROR:
                raise RuntimeError(f"native process monitoring failed with errno {event.data}")
            if (
                event.ident != pid
                or event.filter != select.KQ_FILTER_PROC
                or not event.fflags & select.KQ_NOTE_EXIT
                or not event.fflags & note_exitstatus
            ):
                raise RuntimeError("the operating system did not supply the native exit status")
            native_exited = True
            launcher.wait(timeout=max(0, deadline - time.monotonic()))
            return launcher.returncode, event.data
    finally:
        if not native_exited:
            try:
                remaining_pid = matching_process(native)
                if remaining_pid is not None:
                    os.kill(remaining_pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except (OSError, subprocess.SubprocessError) as error:
                print(f"Native process cleanup failed: {error}", file=sys.stderr)
        if launcher.poll() is None:
            try:
                launcher.kill()
            except ProcessLookupError:
                pass
            launcher.wait(timeout=5)


def remove_probe_bundle(app, native):
    if matching_process(native) is not None:
        raise RuntimeError("the native process is still running; its bundle was retained")
    unregister = Path(
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/"
        "LaunchServices.framework/Support/lsregister"
    )
    result = subprocess.run(
        [str(unregister), "-u", str(app)], capture_output=True, text=True, timeout=5
    )
    # kLSApplicationNotFoundErr also means an earlier failure never registered this bundle.
    if result.returncode and "-10814" not in result.stdout + result.stderr:
        result.check_returncode()
    shutil.rmtree(app)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path, help="Path to a built native example")
    parser.add_argument("arguments", nargs=argparse.REMAINDER, help="Example arguments")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("this probe requires macOS and a GUI session")
    executable = args.executable.resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        parser.error(f"not an executable file: {executable}")
    arguments = args.arguments
    if arguments[:1] == ["--"]:
        arguments = arguments[1:]
    marker = success_marker(executable.name, arguments)
    if marker is None:
        parser.error("expected a supported Open-GPUI native probe executable")

    output = Path(tempfile.mkdtemp(prefix="open-gpui-macos-probe-"))
    app = output / "GPUIProbe.app"
    native = app / "Contents" / "MacOS" / executable.name
    native.parent.mkdir(parents=True)
    shutil.copy2(executable, native)
    with (app / "Contents" / "Info.plist").open("wb") as file:
        plistlib.dump(
            {
                "CFBundleExecutable": executable.name,
                "CFBundleIdentifier": "dev.open-gpui.native-probe",
                "CFBundleName": f"Open GPUI {executable.name}",
                "CFBundlePackageType": "APPL",
                "CFBundleDevelopmentRegion": "en",
                "CFBundleVersion": "1",
                "LSMinimumSystemVersion": "12.0",
                "NSHighResolutionCapable": True,
            },
            file,
        )
    stdout = output / "stdout.log"
    stderr = output / "stderr.log"
    command = [
        "open", "-n", "-W", "--stdout", str(stdout), "--stderr", str(stderr),
        str(app), "--args", *arguments,
    ]
    print(f"Native probe logs: {output}; temporary app removed after exit", flush=True)
    exit_code = 1
    try:
        subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
        launcher_exit, native_status = wait_for_native_exit(command, native)
        text = "\n".join(
            path.read_text(errors="replace") for path in (stdout, stderr) if path.exists()
        )
        print(text, end="" if text.endswith("\n") else "\n")
        # `open -W` does not propagate the app's status; require both OS status and receipt.
        failed = re.search(r"panicked at|fatal runtime error|NATIVE [^\n]*FAILED", text)
        normal_exit = os.WIFEXITED(native_status) and os.WEXITSTATUS(native_status) == 0
        print(f"Native wait status: {native_status}; normal exit 0: {normal_exit}")
        if not launcher_exit and normal_exit and marker in text and not failed:
            exit_code = 0
        else:
            print("Native probe failed; see the retained logs", file=sys.stderr)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Native probe failed: {error}; see the retained logs", file=sys.stderr)
    finally:
        try:
            remove_probe_bundle(app, native)
        except (OSError, RuntimeError, subprocess.SubprocessError) as error:
            print(f"Native probe cleanup failed: {error}; see {output}", file=sys.stderr)
            exit_code = 1
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
