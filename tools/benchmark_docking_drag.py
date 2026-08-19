#!/usr/bin/env python3
"""Run comparable Open GPUI and Dear ImGui docking-preview microbenchmarks."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
IMGUI_SOURCE = ROOT / "tools" / "benchmarks" / "imgui_docking_preview.cpp"
IMGUI_DIR = ROOT / "repo-ref" / "imgui"
IMGUI_BINARY = ROOT / "target" / "benchmarks" / "imgui_docking_preview.exe"


def run(command: list[str], *, cwd: Path = ROOT) -> str:
    environment = os.environ.copy()
    environment.setdefault("CARGO_BUILD_JOBS", "1")
    environment.setdefault("CARGO_TERM_COLOR", "never")
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


def last_json_line(output: str) -> dict[str, float]:
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise RuntimeError(f"command did not emit a JSON object: {output[-1000:]}")


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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="emit one JSON object")
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
    result["open_gpui_to_imgui_ratio"] = (
        result["open_gpui_ns_per_move"] / result["imgui_ns_per_preview"]
    )
    result["benchmark_passed"] = int(result.get("benchmark_passed", 0) == 1)

    if arguments.json:
        print(json.dumps(result, sort_keys=True))
    else:
        for key, value in sorted(result.items()):
            print(f"{key}: {value}")


if __name__ == "__main__":
    main()
