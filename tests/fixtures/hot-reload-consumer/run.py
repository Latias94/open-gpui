"""Qualify a real Windows dx patch in a disposable copy of this consumer."""

import argparse
from contextlib import contextmanager
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time


class JobBasicLimits(ctypes.Structure):
    _fields_ = [
        ("PerProcessUserTimeLimit", ctypes.c_longlong),
        ("PerJobUserTimeLimit", ctypes.c_longlong),
        ("LimitFlags", wintypes.DWORD),
        ("MinimumWorkingSetSize", ctypes.c_size_t),
        ("MaximumWorkingSetSize", ctypes.c_size_t),
        ("ActiveProcessLimit", wintypes.DWORD),
        ("Affinity", ctypes.c_size_t),
        ("PriorityClass", wintypes.DWORD),
        ("SchedulingClass", wintypes.DWORD),
    ]


class JobExtendedLimits(ctypes.Structure):
    _fields_ = [
        ("BasicLimitInformation", JobBasicLimits),
        ("IoInfo", ctypes.c_ulonglong * 6),
        ("ProcessMemoryLimit", ctypes.c_size_t),
        ("JobMemoryLimit", ctypes.c_size_t),
        ("PeakProcessMemoryUsed", ctypes.c_size_t),
        ("PeakJobMemoryUsed", ctypes.c_size_t),
    ]


class StartupInfo(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("lpReserved", wintypes.LPWSTR),
        ("lpDesktop", wintypes.LPWSTR),
        ("lpTitle", wintypes.LPWSTR),
        ("dwX", wintypes.DWORD), ("dwY", wintypes.DWORD),
        ("dwXSize", wintypes.DWORD), ("dwYSize", wintypes.DWORD),
        ("dwXCountChars", wintypes.DWORD), ("dwYCountChars", wintypes.DWORD),
        ("dwFillAttribute", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("wShowWindow", wintypes.WORD),
        ("cbReserved2", wintypes.WORD),
        ("lpReserved2", ctypes.POINTER(wintypes.BYTE)),
        ("hStdInput", wintypes.HANDLE),
        ("hStdOutput", wintypes.HANDLE),
        ("hStdError", wintypes.HANDLE),
    ]


class ProcessInfo(ctypes.Structure):
    _fields_ = [
        ("hProcess", wintypes.HANDLE),
        ("hThread", wintypes.HANDLE),
        ("dwProcessId", wintypes.DWORD),
        ("dwThreadId", wintypes.DWORD),
    ]


def windows_api():
    api = ctypes.WinDLL("kernel32", use_last_error=True)
    signatures = {
        "CreateJobObjectW": ([ctypes.c_void_p, wintypes.LPCWSTR], wintypes.HANDLE),
        "SetInformationJobObject": (
            [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD], wintypes.BOOL),
        "CreateProcessW": (
            [wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p,
             wintypes.BOOL, wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR,
             ctypes.POINTER(StartupInfo), ctypes.POINTER(ProcessInfo)], wintypes.BOOL),
        "AssignProcessToJobObject": ([wintypes.HANDLE, wintypes.HANDLE], wintypes.BOOL),
        "ResumeThread": ([wintypes.HANDLE], wintypes.DWORD),
        "OpenProcess": ([wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
        "WaitForSingleObject": ([wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
        "GetExitCodeProcess": ([wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
        "TerminateProcess": ([wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
        "CloseHandle": ([wintypes.HANDLE], wintypes.BOOL),
    }
    for name, (arguments, result) in signatures.items():
        function = getattr(api, name)
        function.argtypes = arguments
        function.restype = result
    return api


def process_exit_code(api, handle, timeout_ms=0):
    status = api.WaitForSingleObject(handle, timeout_ms)
    if status == 258:  # WAIT_TIMEOUT
        return None
    if status != 0:  # WAIT_OBJECT_0
        raise ctypes.WinError(ctypes.get_last_error())
    code = wintypes.DWORD()
    if not api.GetExitCodeProcess(handle, ctypes.byref(code)):
        raise ctypes.WinError(ctypes.get_last_error())
    return code.value


@contextmanager
def launch_dx(api, command, workspace, env, log):
    from msvcrt import get_osfhandle

    job = api.CreateJobObjectW(None, None)
    if not job:
        raise ctypes.WinError(ctypes.get_last_error())
    child = ProcessInfo()
    assigned = False
    try:
        limits = JobExtendedLimits()
        limits.BasicLimitInformation.LimitFlags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not api.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            raise ctypes.WinError(ctypes.get_last_error())
        # Keep the primary thread handle so job assignment precedes any child code.
        with open(os.devnull, "rb") as stdin:
            output = os.dup(log.fileno())
            try:
                os.set_inheritable(stdin.fileno(), True)
                os.set_inheritable(output, True)
                startup = StartupInfo()
                startup.cb = ctypes.sizeof(startup)
                startup.dwFlags = 0x100  # STARTF_USESTDHANDLES
                startup.hStdInput = get_osfhandle(stdin.fileno())
                startup.hStdOutput = startup.hStdError = get_osfhandle(output)
                command_line = ctypes.create_unicode_buffer(subprocess.list2cmdline(command))
                environment = ctypes.create_unicode_buffer("\0".join(
                    f"{key}={value}" for key, value in sorted(env.items(), key=lambda item: item[0].upper())
                ) + "\0\0")
                flags = 0x4 | 0x400 | 0x08000000  # CREATE_SUSPENDED | UNICODE_ENVIRONMENT | NO_WINDOW
                if not api.CreateProcessW(
                    command[0], command_line, None, None, True, flags, environment,
                    str(workspace), ctypes.byref(startup), ctypes.byref(child),
                ):
                    raise ctypes.WinError(ctypes.get_last_error())
            finally:
                os.close(output)
        if not api.AssignProcessToJobObject(job, child.hProcess):
            raise ctypes.WinError(ctypes.get_last_error())
        assigned = True
        if api.ResumeThread(child.hThread) == 0xFFFFFFFF:
            raise ctypes.WinError(ctypes.get_last_error())
        yield child.hProcess
    finally:
        try:
            # Assignment failure leaves only this exact, still-suspended process outside the job.
            if child.hProcess and not assigned and process_exit_code(api, child.hProcess) is None:
                if not api.TerminateProcess(child.hProcess, 1):
                    raise ctypes.WinError(ctypes.get_last_error())
        finally:
            api.CloseHandle(job)
            if child.hThread:
                api.CloseHandle(child.hThread)
            if child.hProcess:
                try:
                    if process_exit_code(api, child.hProcess, 30000) is None:
                        raise TimeoutError("dx did not exit after closing its job")
                finally:
                    api.CloseHandle(child.hProcess)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dx", required=True, type=Path)
    parser.add_argument("--target-dir", required=True, type=Path)
    parser.add_argument("--toolchain", default="1.95.0")
    parser.add_argument("--timeout", default=1200, type=float)
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("this acceptance trajectory requires native Windows")
    api = windows_api()
    dx = args.dx.resolve(strict=True)
    version = subprocess.check_output([str(dx), "--version"], text=True).strip()
    if not version.startswith("dioxus 0.7.10 "):
        parser.error(f"expected the pinned dx 0.7.10, got {version}")
    with dx.open("rb") as executable:
        driver_sha256 = hashlib.file_digest(executable, "sha256").hexdigest()
    fixture = Path(__file__).resolve().parent
    repo = fixture.parents[2]
    target = args.target_dir.resolve()
    target.mkdir(parents=True, exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix="hot-reload-", dir=target))
    workspace = run / "consumer"
    shutil.copytree(fixture, workspace, ignore=shutil.ignore_patterns("__pycache__", "target"))
    manifest = workspace / "Cargo.toml"
    manifest.write_text(
        manifest.read_text(encoding="utf-8").replace("../../../crates/", f"{repo.as_posix()}/crates/"),
        encoding="utf-8",
    )
    observations = run / "observations.jsonl"
    stop = run / "stop"
    env = os.environ.copy()
    env.update(
        CARGO_BUILD_JOBS="1",
        CARGO_TARGET_DIR=str(target),
        RUSTUP_TOOLCHAIN=args.toolchain,
        OPEN_GPUI_HOT_RELOAD_OBSERVATIONS=str(observations),
        OPEN_GPUI_HOT_RELOAD_STOP=str(stop),
        RUST_LOG="info",
    )
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    command = [
        str(dx), "serve", "--hot-patch", "--bundle", "windows",
        "--target", "x86_64-pc-windows-msvc", "--skip-assets",
        "--package", "open-gpui-hot-reload-consumer", "--bin", "open-gpui-hot-reload-consumer",
        "--locked", "--interactive", "false",
        "--open", "false", "--always-on-top", "false", "--addr", "127.0.0.1",
        "--port", str(port), "--windows-subsystem", "CONSOLE",
    ]
    print(f"Artifacts: {run}", flush=True)
    print(f"Driver: {version}", flush=True)
    print(f"Driver SHA-256: {driver_sha256}", flush=True)
    dx_log = run / "dx.log"
    with dx_log.open("w", encoding="utf-8") as log, launch_dx(
        api, command, workspace, env, log,
    ) as dx_process:
        consumer_process = None
        try:
            def wait_for(predicate, description, before_ready=False):
                deadline = time.monotonic() + args.timeout
                report_at = time.monotonic() + 10
                exited = None
                while time.monotonic() < deadline:
                    if observations.exists():
                        for line in observations.read_text(encoding="utf-8").splitlines():
                            try:
                                row = json.loads(line)
                            except json.JSONDecodeError:
                                continue
                            if row.get("stage") in ("error", "retirement_timeout"):
                                raise RuntimeError(row)
                            if predicate(row):
                                return row
                    if exited is not None:
                        raise RuntimeError(f"{exited} before {description}; see {dx_log}")
                    if before_ready:
                        for line in dx_log.read_text(encoding="utf-8", errors="replace").splitlines():
                            if "Application" in line and "exited" in line:
                                raise RuntimeError(f"consumer exited before READY: {line}; see {dx_log}")
                    code = process_exit_code(api, dx_process)
                    if code is not None:
                        exited = f"dx exited with {code:#x}"
                        continue  # Read final flushed observations once more before failing.
                    if consumer_process is not None:
                        code = process_exit_code(api, consumer_process)
                        if code is not None:
                            exited = f"consumer exited with {code:#x}"
                            continue
                    if time.monotonic() >= report_at:
                        print(f"Waiting for {description}", flush=True)
                        report_at = time.monotonic() + 10
                    time.sleep(0.2)
                raise TimeoutError(f"timed out waiting for {description}; see {run / 'dx.log'}")

            def check_frame(row, colors, outputs):
                assert row["pixels_match"] and row["entity_ids_match"], row
                assert row["accepted"] == row["submitted"] == row["rendered"] == row["non_empty_presented"], row
                assert row["native_visible"] and not row["test_scheduler"], row
                assert row["colors"] == colors and row["recipe_outputs"] == outputs, row
                assert row["state"] == [43, 47, 53] and row["live_state"] == [43, 47], row
                assert row["root_counter"] == 41 and row["initializations"] == [1, 1, 1], row
                assert row["state_mutations"] == 1, row

            initial = wait_for(
                lambda row: row.get("stage") == "frame" and row.get("phase") == "READY"
                and row.get("root_counter") == 41 and row.get("state") == [43, 47, 53]
                and row.get("live_state") == [43, 47],
                "state-mutated READY submitted native frame", before_ready=True,
            )
            check_frame(initial, [0x334455, 0x556677, 0x778899], [101, 202, 303])
            assert initial["process_start_nonce"], initial
            # Hold this process object through quit, even if dx exits or a PID is later reused.
            consumer_process = api.OpenProcess(0x00100000 | 0x1000, False, initial["pid"])
            if not consumer_process:  # SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION
                raise ctypes.WinError(ctypes.get_last_error())
            panel = workspace / "panel/src/lib.rs"
            source = panel.read_text(encoding="utf-8")
            for before, after in [
                ("0x334455", "0x884422"), ("0x556677", "0x226688"), ("0x778899", "0x448822"),
                ("output: 101", "output: 111"), ("output: 202", "output: 222"),
                ("output: 303", "output: 333"), ("-READY", "-PATCHED"),
            ]:
                assert before in source, before
                source = source.replace(before, after)
            edited_at = time.monotonic()
            panel.write_text(source, encoding="utf-8")
            patched = wait_for(
                lambda row: row.get("stage") == "frame" and row.get("phase") == "PATCHED",
                "compiled patch and changed pixels",
            )
            observed_ms = (time.monotonic() - edited_at) * 1000
            check_frame(patched, [0x884422, 0x226688, 0x448822], [111, 222, 333])
            for key in ("process_start_nonce", "pid", "window_id", "root_entity_id", "entity_ids",
                        "state", "live_state", "root_counter", "state_mutations", "initializations"):
                assert initial[key] == patched[key], (key, initial[key], patched[key])
            assert patched["accepted"] > initial["accepted"], (initial, patched)
            stop.touch()
            retired = wait_for(lambda row: row.get("stage") == "retired", "normal window and entity retirement")
            assert retired["pid"] == initial["pid"] and retired["remaining_windows"] == 0, retired
            assert retired["root_released"] and all(retired["children_released"]), retired
            assert retired["initializations"] == retired["releases"] == [1, 1, 1], retired
            assert retired["process_start_nonce"] == initial["process_start_nonce"], retired
            quit_row = wait_for(
                lambda row: row.get("stage") == "quit" and row.get("pid") == initial["pid"]
                and row.get("process_start_nonce") == initial["process_start_nonce"],
                "normal App quit in the original process",
            )
            assert quit_row["remaining_windows"] == 0 and quit_row["root_released"], quit_row
            assert all(quit_row["children_released"]), quit_row
            assert quit_row["initializations"] == quit_row["releases"] == [1, 1, 1], quit_row
            exit_code = process_exit_code(api, consumer_process, int(min(args.timeout, 30) * 1000))
            if exit_code is None:
                raise TimeoutError("consumer emitted quit but did not exit within 30 seconds")
            if exit_code != 0:
                raise RuntimeError(f"consumer exited with {exit_code:#x} after quit")
            if "native window terminal was observed without a registered presentation-shutdown authority" in dx_log.read_text(encoding="utf-8", errors="replace"):
                raise RuntimeError("normal retirement reported a missing presentation-shutdown authority")
            summary = dict(driver=version, driver_sha256=driver_sha256,
                           initial=initial, patched=patched, retired=retired,
                           quit=quit_row, app_exit_code=exit_code,
                           edit_to_observed_ms=observed_ms, sampler_ms=100)
            (run / "result.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
            print(f"Passed real edit/build/apply/readback/retirement/quit with exit 0 in one process; observation {observed_ms:.1f} ms", flush=True)
        finally:
            try:
                stop.touch(exist_ok=True)
            finally:
                if consumer_process is not None:
                    api.CloseHandle(consumer_process)


if __name__ == "__main__":
    main()
