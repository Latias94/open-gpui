"""Qualify native failure handling with an existing dx-built Windows consumer.

Install requirements.txt, then supply --app BASE_CONSUMER.exe and --artifacts DIR.
The artifact parent also accepts --target-dir; every run uses a fresh subdirectory.
The consumer must support OPEN_GPUI_HOT_RELOAD_CHECKPOINT: changed file content
must produce a fresh live frame observation, including the checkpoint string,
without notifying or redrawing the window. No compiler or dx process is launched.

Messages follow the serde representation of dioxus-devtools-types 0.7.10 and
subsecond-types 0.7.10. A missing absolute DLL exercises rejection before any
executable patch loads; successful executable patching belongs to run.py.
"""

import argparse
import json
import math
import os
from pathlib import Path
from queue import Queue
import sys
import tempfile
from threading import Event, Thread
import time
from urllib.parse import parse_qs, urlsplit

from websockets import __version__ as websockets_version
from websockets.sync.server import serve

from run import launch_dx, process_exit_code, windows_api


BUILD_ID = 71


def check_frame(row):
    if not (
        row["stage"] == "frame" and row["phase"] == "READY"
        and row["process_start_nonce"] and row["pid"] > 0
        and row["pixels_match"] and row["entity_ids_match"]
        and row["accepted"] > 0
        and row["accepted"] == row["submitted"] == row["rendered"] == row["non_empty_presented"]
        and row["native_visible"] and row["native_created"] and not row["test_scheduler"]
        and row["initial_presentation"] == "Completed"
        and row["readback_source"] == "accepted_framework_scene"
        and row["colors"] == [0x334455, 0x556677, 0x778899]
        and row["recipe_outputs"] == [101, 202, 303]
        and row["state"] == [43, 47, 53] and row["live_state"] == [43, 47]
        and row["root_counter"] == 41 and row["state_mutations"] == 1
        and row["initializations"] == [1, 1, 1]
    ):
        raise RuntimeError(f"invalid state-mutated READY native frame: {row}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path,
                        help="existing dx-built base consumer EXE with the checkpoint hook")
    parser.add_argument("--artifacts", "--target-dir", dest="artifacts", required=True, type=Path,
                        help="parent for a fresh failure-acceptance artifact directory")
    parser.add_argument("--timeout", default=30, type=float, help="seconds per bounded wait")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("this acceptance trajectory requires native Windows")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be a finite positive number")
    if websockets_version != "16.0":
        parser.error("install the pinned websockets==16.0 from requirements.txt")
    app = args.app.resolve(strict=True)
    if not app.is_file() or app.suffix.lower() != ".exe":
        parser.error("--app must name an existing dx-built base consumer EXE")
    target = args.artifacts.resolve()
    target.mkdir(parents=True, exist_ok=True)
    artifacts = Path(tempfile.mkdtemp(prefix="hot-reload-failures-", dir=target))
    observations = artifacts / "observations.jsonl"
    checkpoint_path = artifacts / "checkpoint"
    stop = artifacts / "stop"
    app_log = artifacts / "app.log"
    missing_dll = artifacts / "nonexistent-patch.dll"
    checkpoint_path.write_text("initial", encoding="utf-8")
    print(f"Artifacts: {artifacts}", flush=True)
    print(f"Consumer: {app}", flush=True)

    api = windows_api()
    connections = Queue()
    finished = Event()
    connection = None

    def accept_connection(websocket):
        connections.put(websocket)
        finished.wait()

    with serve(
        accept_connection, "127.0.0.1", 0, compression=None, ping_interval=None,
        open_timeout=min(args.timeout, 5), close_timeout=min(args.timeout, 2),
    ) as server:
        server_thread = Thread(target=server.serve_forever, daemon=True)
        server_thread.start()
        try:
            env = os.environ.copy()
            env.update(
                RUST_LOG="info",
                OPEN_GPUI_HOT_RELOAD_OBSERVATIONS=str(observations),
                OPEN_GPUI_HOT_RELOAD_STOP=str(stop),
                OPEN_GPUI_HOT_RELOAD_CHECKPOINT=str(checkpoint_path),
                DIOXUS_DEVSERVER_IP="127.0.0.1",
                DIOXUS_DEVSERVER_PORT=str(server.socket.getsockname()[1]),
                DIOXUS_BUILD_ID=str(BUILD_ID),
            )
            with app_log.open("w", encoding="utf-8") as log, launch_dx(
                api, [str(app)], app.parent, env, log,
            ) as process:
                try:
                    def wait_for(predicate, description, allow_exit=False):
                        deadline = time.monotonic() + args.timeout
                        while time.monotonic() < deadline:
                            code = process_exit_code(api, process)
                            if code is not None and (not allow_exit or code != 0):
                                raise RuntimeError(
                                    f"consumer exited with {code:#x} before {description}; see {app_log}"
                                )
                            if connection is not None and not connections.empty():
                                raise RuntimeError("unexpected additional development connection")
                            rows = []
                            if observations.exists():
                                rows = [json.loads(line) for line in
                                        observations.read_text(encoding="utf-8").split("\n")[:-1]]
                            for row in rows:
                                if row.get("stage") in ("error", "retirement_timeout"):
                                    raise RuntimeError(f"native consumer failed: {row}")
                            text = app_log.read_text(encoding="utf-8", errors="replace")
                            result = predicate(rows, text)
                            if result is not None and result is not False:
                                return result
                            if code is not None:
                                raise RuntimeError(f"consumer exited before {description}; see {app_log}")
                            time.sleep(0.05)
                        raise TimeoutError(f"timed out waiting for {description}; see {artifacts}")

                    initial = wait_for(
                        lambda rows, text: next((row for row in rows
                            if row.get("stage") == "frame" and row.get("phase") == "READY"
                            and row.get("root_counter") == 41 and row.get("state") == [43, 47, 53]
                            and row.get("live_state") == [43, 47]), None),
                        "state-mutated READY submitted native frame",
                    )
                    check_frame(initial)
                    wait_for(lambda rows, text: not connections.empty(), "native websocket handshake")
                    connection = connections.get_nowait()
                    request = connection.request.path
                    endpoint = urlsplit(request)
                    query = parse_qs(endpoint.query, keep_blank_values=True, strict_parsing=True)
                    protocol = dict(
                        types="dioxus-devtools-types 0.7.10 / subsecond-types 0.7.10",
                        websockets=websockets_version, request=request, query=query, messages=[],
                    )
                    protocol_path = artifacts / "protocol.json"
                    protocol_path.write_text(json.dumps(protocol, indent=2), encoding="utf-8")
                    if endpoint.path != "/_dioxus" or any(
                        len(query.get(key, [])) != 1 for key in ("build_id", "pid", "aslr_reference")
                    ):
                        raise RuntimeError(f"unexpected native websocket request: {request}")
                    build_id = int(query["build_id"][0])
                    process_id = int(query["pid"][0])
                    reference = int(query["aslr_reference"][0])
                    if build_id != BUILD_ID or process_id != initial["pid"] or not 0 < reference < 2**64:
                        raise RuntimeError(f"native handshake does not identify the READY consumer: {query}")
                    wait_for(lambda rows, text: "GPUI hot reload connected" in text,
                             "native development connection log")

                    def check_logs(failed=0, restarted=0, disconnected=0):
                        text = app_log.read_text(encoding="utf-8", errors="replace")
                        if "native window terminal was observed without a registered presentation-shutdown authority" in text:
                            raise RuntimeError("normal retirement reported a missing presentation-shutdown authority")
                        expected = {
                            "GPUI hot reload connected": 1,
                            "GPUI hot reload applied": 0,
                            "GPUI hot reload failed": failed,
                            "GPUI hot reload requires a full application restart": restarted,
                            "GPUI hot reload disconnected": disconnected,
                        }
                        for marker, count in expected.items():
                            if text.count(marker) != count:
                                raise RuntimeError(f"expected {count} occurrences of {marker!r}; see {app_log}")
                        return expected

                    def send(message):
                        if missing_dll.exists():
                            raise RuntimeError(f"invalid-patch fixture unexpectedly exists: {missing_dll}")
                        connection.send(json.dumps(message))
                        protocol["messages"].append(message)
                        protocol_path.write_text(json.dumps(protocol, indent=2), encoding="utf-8")

                    def patch(for_build, for_process):
                        return {"HotReload": {
                            "templates": [], "assets": [], "ms_elapsed": 0,
                            "jump_table": {
                                "lib": str(missing_dll), "map": {}, "aslr_reference": reference,
                                "new_base_address": 0, "ifunc_count": 0,
                            },
                            "for_build_id": for_build, "for_pid": for_process,
                        }}

                    def checkpoint(label, failed=0, restarted=0, disconnected=0):
                        checkpoint_path.write_text(label, encoding="utf-8")
                        row = wait_for(
                            lambda rows, text: next((row for row in rows
                                if row.get("stage") == "frame" and row.get("checkpoint") == label), None),
                            f"live {label} checkpoint without redraw",
                        )
                        check_frame(row)
                        for key, value in initial.items():
                            if key not in ("elapsed_ms", "checkpoint") and row[key] != value:
                                raise RuntimeError(f"{label} changed {key}: {value!r} -> {row[key]!r}")
                        check_logs(failed, restarted, disconnected)
                        print(f"Preserved native frame: {label}", flush=True)
                        return row

                    check_logs()
                    send(patch(build_id + 1, process_id))
                    pong = connection.ping()
                    wait_for(lambda rows, text: pong.is_set(), "wrong-build websocket pong")
                    time.sleep(0.5)
                    wrong_build = checkpoint("wrong-build")

                    wrong_process_id = process_id + 1 if process_id < 2**32 - 1 else process_id - 1
                    send(patch(build_id, wrong_process_id))
                    pong = connection.ping()
                    wait_for(lambda rows, text: pong.is_set(), "wrong-pid websocket pong")
                    time.sleep(0.5)
                    wrong_pid = checkpoint("wrong-pid")

                    send(patch(build_id, process_id))
                    wait_for(lambda rows, text: "GPUI hot reload failed" in text,
                             "foreground invalid-DLL failure log")
                    invalid_dll = checkpoint("invalid-dll", failed=1)

                    send("FullReloadCommand")
                    wait_for(lambda rows, text: "GPUI hot reload requires a full application restart" in text,
                             "full application restart warning")
                    full_restart = checkpoint("full-restart", failed=1, restarted=1)

                    connection.close(code=1000, reason="native failure acceptance disconnect")
                    wait_for(lambda rows, text: "GPUI hot reload disconnected" in text,
                             "native development disconnect log")
                    disconnected = checkpoint("disconnected", failed=1, restarted=1, disconnected=1)

                    stop.touch()
                    retired = wait_for(
                        lambda rows, text: next((row for row in rows if row.get("stage") == "retired"), None),
                        "normal window and entity retirement", allow_exit=True,
                    )
                    quit_row = wait_for(
                        lambda rows, text: next((row for row in rows if row.get("stage") == "quit"), None),
                        "normal App quit", allow_exit=True,
                    )
                    for row in (retired, quit_row):
                        if not (
                            row["pid"] == initial["pid"]
                            and row["process_start_nonce"] == initial["process_start_nonce"]
                            and row["remaining_windows"] == 0 and row["root_released"]
                            and row["children_released"] == [True, True, True]
                            and row["initializations"] == row["releases"] == [1, 1, 1]
                        ):
                            raise RuntimeError(f"incomplete normal native retirement/quit: {row}")
                    exit_code = process_exit_code(api, process, int(min(args.timeout, 30) * 1000))
                    if exit_code is None:
                        raise TimeoutError("consumer emitted quit but did not exit within the bounded wait")
                    if exit_code != 0:
                        raise RuntimeError(f"consumer exited with {exit_code:#x} after quit")
                    log_counts = check_logs(failed=1, restarted=1, disconnected=1)
                    summary = dict(
                        app=str(app), build_id=BUILD_ID, protocol=protocol, initial=initial,
                        checkpoints=[wrong_build, wrong_pid, invalid_dll, full_restart, disconnected],
                        retired=retired, quit=quit_row, app_exit_code=exit_code,
                        expected_log_counts=log_counts, mismatch_settle_seconds=0.5,
                        limits=[
                            "The supplied EXE is not rebuilt; dx build provenance is supplied separately.",
                            "A nonexistent absolute DLL rejects loading; no executable patch is loaded or invoked.",
                            "Successful executable patching is qualified separately by run.py.",
                            "Mismatch silence is observed after pong, 0.5-second settle and a live checkpoint.",
                            "FullReloadCommand requires a restart; this trajectory does not perform one.",
                            "Disconnect covers a graceful websocket close, not an abrupt network failure.",
                            "Pixel checks read the accepted framework scene, not OS compositor scanout.",
                        ],
                    )
                finally:
                    stop.touch(exist_ok=True)
        finally:
            finished.set()
            if connection is not None:
                connection.close()
            server.shutdown()
            server_thread.join(timeout=5)
            if server_thread.is_alive():
                raise TimeoutError("development websocket server did not stop")

    (artifacts / "result.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print("Passed native failure preservation, normal retirement and quit with process exit 0", flush=True)


if __name__ == "__main__":
    main()
