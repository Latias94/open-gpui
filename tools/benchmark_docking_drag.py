#!/usr/bin/env python3
"""Run comparable Open GPUI and Dear ImGui docking-preview microbenchmarks."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
IMGUI_SOURCE = ROOT / "tools" / "benchmarks" / "imgui_docking_preview.cpp"
IMGUI_DIR = ROOT / "repo-ref" / "imgui"
IMGUI_BINARY = ROOT / "target" / "benchmarks" / "imgui_docking_preview.exe"
NATIVE_LATENCY_OUTPUT = ROOT / "target" / "benchmarks" / "native_tab_drag_input_to_submit.json"
NATIVE_LATENCY_TEST = (
    "native_interactive_tests::native_interactive_tab_drag_input_to_submit_latency_benchmark"
)
NATIVE_LATENCY_OUTPUT_ENV = "OPEN_GPUI_DOCK_DRAG_LATENCY_OUTPUT"


def run(
    command: list[str],
    *,
    cwd: Path = ROOT,
    extra_env: dict[str, str] | None = None,
) -> str:
    environment = os.environ.copy()
    environment.setdefault("CARGO_BUILD_JOBS", "1")
    environment.setdefault("CARGO_TERM_COLOR", "never")
    if extra_env is not None:
        environment.update(extra_env)
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=environment,
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stdout or "")
        sys.stderr.write(completed.stderr or "")
        raise SystemExit(completed.returncode)
    return completed.stdout


def last_json_line(output: str) -> dict[str, Any]:
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise RuntimeError(f"command did not emit a JSON object: {output[-1000:]}")


def number_at(report: dict[str, Any], path: tuple[str, ...]) -> float:
    value: Any = report
    for segment in path:
        if not isinstance(value, dict) or segment not in value:
            raise RuntimeError(f"native latency report is missing {'.'.join(path)}")
        value = value[segment]
    if not isinstance(value, (int, float)):
        raise RuntimeError(f"native latency report field {'.'.join(path)} is not numeric")
    return float(value)


def build_imgui_benchmark() -> None:
    compiler = shutil.which("g++")
    if compiler is None:
        raise RuntimeError("g++ is required for the local Dear ImGui comparison")

    dependencies = [
        IMGUI_SOURCE,
        IMGUI_DIR / "imgui.cpp",
        IMGUI_DIR / "imgui_draw.cpp",
        IMGUI_DIR / "imgui_tables.cpp",
        IMGUI_DIR / "imgui_widgets.cpp",
        IMGUI_DIR / "imgui.h",
        IMGUI_DIR / "imgui_internal.h",
    ]
    latest_input = max(path.stat().st_mtime_ns for path in dependencies)
    if IMGUI_BINARY.exists() and IMGUI_BINARY.stat().st_mtime_ns >= latest_input:
        return

    IMGUI_BINARY.parent.mkdir(parents=True, exist_ok=True)
    run(
        [
            compiler,
            "-std=c++17",
            "-O3",
            "-DNDEBUG",
            "-static",
            "-static-libgcc",
            "-static-libstdc++",
            f"-I{IMGUI_DIR}",
            str(IMGUI_SOURCE),
            str(IMGUI_DIR / "imgui_draw.cpp"),
            str(IMGUI_DIR / "imgui_tables.cpp"),
            str(IMGUI_DIR / "imgui_widgets.cpp"),
            "-o",
            str(IMGUI_BINARY),
        ]
    )


def summarize_native_latency(report: dict[str, Any]) -> dict[str, float]:
    return {
        "native_latency_benchmark_passed": 1.0,
        "native_latency_injected_moves": number_at(report, ("injected_moves",)),
        "native_latency_observed_wndproc_moves": number_at(report, ("observed_wndproc_moves",)),
        "native_latency_observed_input_hz": number_at(report, ("observed_input_hz",)),
        "native_latency_measurement_elapsed_ms": number_at(report, ("measurement_elapsed_ms",)),
        "native_latency_source_submitted_fps": number_at(report, ("source_submitted_fps",)),
        "native_latency_target_submitted_fps": number_at(report, ("target_submitted_fps",)),
        "native_latency_source_input_to_submit_p50_ms": number_at(
            report, ("source", "input_to_submit_ms", "p50")
        ),
        "native_latency_source_input_to_submit_p95_ms": number_at(
            report, ("source", "input_to_submit_ms", "p95")
        ),
        "native_latency_source_input_to_submit_p99_ms": number_at(
            report, ("source", "input_to_submit_ms", "p99")
        ),
        "native_latency_source_input_to_submit_max_ms": number_at(
            report, ("source", "input_to_submit_ms", "max")
        ),
        "native_latency_target_input_to_submit_p50_ms": number_at(
            report, ("target", "input_to_submit_ms", "p50")
        ),
        "native_latency_target_input_to_submit_p95_ms": number_at(
            report, ("target", "input_to_submit_ms", "p95")
        ),
        "native_latency_target_input_to_submit_p99_ms": number_at(
            report, ("target", "input_to_submit_ms", "p99")
        ),
        "native_latency_target_input_to_submit_max_ms": number_at(
            report, ("target", "input_to_submit_ms", "max")
        ),
    }


def run_native_latency_benchmark() -> dict[str, float]:
    if platform.system() != "Windows":
        raise RuntimeError("native input-to-submit latency benchmark requires Windows")

    NATIVE_LATENCY_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    NATIVE_LATENCY_OUTPUT.unlink(missing_ok=True)
    run(
        [
            "cargo",
            "nextest",
            "run",
            "-p",
            "open-gpui-docking-native",
            "--run-ignored",
            "only",
            "--test-threads",
            "1",
            "--no-fail-fast",
            "-E",
            f"test(={NATIVE_LATENCY_TEST})",
        ],
        extra_env={NATIVE_LATENCY_OUTPUT_ENV: str(NATIVE_LATENCY_OUTPUT)},
    )
    with NATIVE_LATENCY_OUTPUT.open("r", encoding="utf-8") as report_file:
        report = json.load(report_file)
    if not isinstance(report, dict):
        raise RuntimeError("native latency benchmark did not emit a JSON object")
    return summarize_native_latency(report)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="emit one JSON object")
    parser.add_argument(
        "--native-latency",
        action="store_true",
        help=(
            "also run the ignored Windows native input-to-submit latency benchmark; "
            "this opens windows and injects mouse input"
        ),
    )
    arguments = parser.parse_args()

    rust = last_json_line(
        run(
            [
                "cargo",
                "bench",
                "-p",
                "open-gpui-docking",
                "--bench",
                "tab_drag_latency",
                "--features",
                "test-support",
            ]
        )
    )
    build_imgui_benchmark()
    imgui = last_json_line(run([str(IMGUI_BINARY)]))

    result = {**rust, **imgui}
    result["resolver_to_imgui_setup_ratio"] = (
        result["open_gpui_ns_per_move"] / result["imgui_ns_per_preview"]
    )
    result["multiview_known_target_to_imgui_known_host_ratio"] = (
        result["open_gpui_multiview_known_target_ns_per_move"]
        / result["imgui_multiview_known_host_ns_per_preview"]
    )
    result["resolver_to_imgui_dockable_target_ratio"] = (
        result["open_gpui_ns_per_move"] / result["imgui_dockable_target_ns_per_preview"]
    )
    result["multiview_known_target_to_imgui_dockable_target_known_host_ratio"] = (
        result["open_gpui_multiview_known_target_ns_per_move"]
        / result["imgui_multiview_dockable_target_known_host_ns_per_preview"]
    )
    result["benchmark_passed"] = int(result.get("benchmark_passed", 0) == 1)
    if arguments.native_latency:
        result.update(run_native_latency_benchmark())

    if arguments.json:
        print(json.dumps(result, sort_keys=True))
    else:
        for key, value in sorted(result.items()):
            print(f"{key}: {value}")


if __name__ == "__main__":
    main()
