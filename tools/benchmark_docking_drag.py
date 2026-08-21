#!/usr/bin/env python3
"""Run Open GPUI and Dear ImGui docking benchmarks with strict row pairing."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
IMGUI_SOURCE = ROOT / "tools" / "benchmarks" / "imgui_docking_preview.cpp"
IMGUI_DIR = ROOT / "repo-ref" / "imgui"
IMGUI_BINARY = ROOT / "target" / "benchmarks" / "imgui_docking_preview.exe"
DOCKSPACE_SCENARIO_FIXTURES = (
    ROOT
    / "crates"
    / "gpui_docking"
    / "src"
    / "fixtures"
    / "dockspace_scenarios_v2.json"
)
IMGUI_SCENARIO_HEADER = (
    ROOT / "target" / "benchmarks" / "three_dockspace_rehome.generated.h"
)
NATIVE_LATENCY_OUTPUT = ROOT / "target" / "benchmarks" / "native_three_hwnd_lifecycle.json"
NATIVE_LATENCY_RAW_DIR = ROOT / "target" / "benchmarks" / "native_three_hwnd_lifecycle_runs"
NATIVE_LATENCY_TEST = (
    "native_interactive_tests::native_interactive_tab_drag_input_to_submit_latency_benchmark"
)
NATIVE_LATENCY_OUTPUT_ENV = "OPEN_GPUI_DOCK_DRAG_LATENCY_OUTPUT"
NATIVE_INTERACTIVE_ENV = "OPEN_GPUI_NATIVE_INTERACTIVE"
NATIVE_LATENCY_PROCESS_SAMPLES_ENV = "OPEN_GPUI_NATIVE_LATENCY_PROCESS_SAMPLES"
NATIVE_LATENCY_DEFAULT_PROCESS_SAMPLES = 5
NATIVE_LIFECYCLE_BOUNDARIES = (
    "drag_activation",
    "accepted_preview_submitted",
    "release_lock",
    "graph_receipt",
    "destination_semantics_submitted",
    "focus_settled",
)
NATIVE_LIFECYCLE_BOUNDARY_ORIGINS = {
    "drag_activation": "drag_threshold_move",
    "accepted_preview_submitted": "target_route_move",
    "release_lock": "primary_release",
    "graph_receipt": "primary_release",
    "destination_semantics_submitted": "primary_release",
    "focus_settled": "primary_release",
}
COMPARISON_KEY_FIELDS = (
    "scenario_id",
    "step_id",
    "operation_family",
    "measurement_layer",
    "scope",
)
THREE_DOCKSPACE_COMPARISON_KEY = (
    "three_dockspace_rehome_cycle",
    None,
    "named_item_docking_sequence",
    "headless_logical_dockspace_resolve_and_commit",
    "three_dockspaces_8_steps_2_edge_6_center_no_platform_windows",
)
REQUIRED_COMPARISON_KEYS = frozenset({THREE_DOCKSPACE_COMPARISON_KEY})


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


def load_three_dockspace_rehome_scenario() -> dict[str, Any]:
    with DOCKSPACE_SCENARIO_FIXTURES.open("r", encoding="utf-8") as fixture_file:
        suite = json.load(fixture_file)
    if not isinstance(suite, dict) or suite.get("schema_version") != 2:
        raise RuntimeError("dockspace benchmark fixture must use schema version 2")
    scenarios = suite.get("scenarios")
    if not isinstance(scenarios, list):
        raise RuntimeError("dockspace benchmark fixture scenarios must be a list")
    matches = [
        scenario
        for scenario in scenarios
        if isinstance(scenario, dict)
        and scenario.get("id") == "three_dockspace_rehome_cycle"
    ]
    if len(matches) != 1:
        raise RuntimeError(
            "dockspace fixtures must contain exactly one three_dockspace_rehome_cycle"
        )
    return matches[0]


def render_imgui_rehome_scenario_header(scenario: dict[str, Any]) -> str:
    items: list[str] = []
    item_indexes: dict[str, int] = {}
    node_items: list[int] = []
    node_children: list[int] = []
    nodes: list[dict[str, Any] | None] = []

    def normalized_split_fractions(node: dict[str, Any], child_count: int) -> list[float]:
        fractions = node.get("fractions", [])
        if fractions:
            if (
                not isinstance(fractions, list)
                or len(fractions) != child_count
                or any(
                    isinstance(fraction, bool)
                    or not isinstance(fraction, (int, float))
                    or not math.isfinite(float(fraction))
                    or fraction < 0
                    for fraction in fractions
                )
            ):
                raise RuntimeError(
                    "dockspace benchmark split fractions must be finite and non-negative"
                )
            normalized = [float(fraction) for fraction in fractions]
        else:
            normalized = [1.0] * child_count
        total = sum(normalized)
        if total <= 1.0e-6:
            return [1.0 / child_count] * child_count
        normalized = [fraction / total for fraction in normalized]
        normalized[-1] = min(1.0, max(0.0, 1.0 - sum(normalized[:-1])))
        return normalized

    def contract_fraction(value: float) -> str:
        return f"{value:.6f}".rstrip("0").rstrip(".")

    def cpp_float(value: float) -> str:
        rendered = f"{value:.9g}"
        if "." not in rendered and "e" not in rendered.lower():
            rendered += ".0"
        return rendered + "f"

    def item_index(item: Any) -> int:
        if not isinstance(item, str) or not item:
            raise RuntimeError("dockspace benchmark item ids must be non-empty strings")
        if item in item_indexes:
            raise RuntimeError(f"dockspace benchmark repeats item id {item!r}")
        index = len(items)
        items.append(item)
        item_indexes[item] = index
        return index

    def topology_item_index(item: Any, topology_items: set[str] | None) -> int:
        if topology_items is None:
            return item_index(item)
        if not isinstance(item, str) or not item:
            raise RuntimeError("dockspace benchmark item ids must be non-empty strings")
        if item not in item_indexes:
            raise RuntimeError(f"dockspace benchmark checkpoint references unknown item {item!r}")
        if item in topology_items:
            raise RuntimeError(f"dockspace benchmark checkpoint repeats item {item!r}")
        topology_items.add(item)
        return item_indexes[item]

    def append_topology_contract(parts: list[str], node: dict[str, Any]) -> None:
        kind = node.get("kind")
        if kind == "tabs":
            tab_items = node.get("items")
            if not isinstance(tab_items, list) or not tab_items:
                raise RuntimeError("dockspace benchmark tabs must contain items")
            selected = node.get("selected") or tab_items[0]
            if selected not in tab_items:
                raise RuntimeError("dockspace benchmark selected item must belong to its tabs")
            parts.extend(("T[selected=", selected, ";items="))
            parts.append(",".join(tab_items))
            parts.append("]")
            return
        if kind != "split":
            raise RuntimeError(f"unsupported dockspace benchmark node kind {kind!r}")
        axis = normalized_axis(node.get("axis"))
        children = node.get("children")
        if not isinstance(children, list) or len(children) != 2:
            raise RuntimeError("ImGui benchmark projection requires binary split nodes")
        fractions = normalized_split_fractions(node, len(children))
        parts.extend(
            (
                "S",
                axis,
                "[fractions=",
                ",".join(contract_fraction(fraction) for fraction in fractions),
                "](",
            )
        )
        for index, child in enumerate(children):
            if index > 0:
                parts.append(",")
            if not isinstance(child, dict):
                raise RuntimeError("dockspace benchmark split children must be objects")
            append_topology_contract(parts, child)
        parts.append(")")

    def normalized_axis(value: Any) -> str:
        if value in ("Horizontal", "horizontal"):
            return "horizontal"
        if value in ("Vertical", "vertical"):
            return "vertical"
        raise RuntimeError(f"unsupported dockspace benchmark split axis {value!r}")

    def add_node(node: Any, topology_items: set[str] | None = None) -> int:
        if not isinstance(node, dict):
            raise RuntimeError("dockspace benchmark topology nodes must be objects")
        index = len(nodes)
        nodes.append(None)
        kind = node.get("kind")
        if kind == "tabs":
            tab_items = node.get("items")
            if not isinstance(tab_items, list) or not tab_items:
                raise RuntimeError("dockspace benchmark tabs must contain items")
            selected = node.get("selected")
            if selected is not None and selected not in tab_items:
                raise RuntimeError("dockspace benchmark selected item must belong to its tabs")
            first_item = len(node_items)
            node_items.extend(
                topology_item_index(item, topology_items) for item in tab_items
            )
            selected_item = item_indexes[selected or tab_items[0]]
            nodes[index] = {
                "kind": "RehomeNode_Tabs",
                "axis": "ImGuiAxis_None",
                "ratio": 0.0,
                "first_item": first_item,
                "item_count": len(tab_items),
                "selected_item": selected_item,
                "first_child": 0,
                "child_count": 0,
            }
            return index
        if kind != "split":
            raise RuntimeError(f"unsupported dockspace benchmark node kind {kind!r}")
        axis = normalized_axis(node.get("axis"))
        children = node.get("children")
        if not isinstance(children, list) or len(children) != 2:
            raise RuntimeError("ImGui benchmark projection requires binary split nodes")
        child_indexes = [add_node(child, topology_items) for child in children]
        first_child = len(node_children)
        node_children.extend(child_indexes)
        fractions = normalized_split_fractions(node, len(children))
        nodes[index] = {
            "kind": "RehomeNode_Split",
            "axis": "ImGuiAxis_X" if axis == "horizontal" else "ImGuiAxis_Y",
            "ratio": fractions[1],
            "first_item": 0,
            "item_count": 0,
            "selected_item": -1,
            "first_child": first_child,
            "child_count": 2,
        }
        return index

    spaces = scenario.get("spaces")
    if not isinstance(spaces, list) or not spaces:
        raise RuntimeError("dockspace benchmark scenario must declare spaces")
    space_indexes: dict[str, int] = {}
    generated_spaces: list[tuple[str, int]] = []
    contract_parts = ["dockspace-sequence-v2|spaces="]
    for index, space in enumerate(spaces):
        if not isinstance(space, dict):
            raise RuntimeError("dockspace benchmark spaces must be objects")
        space_id = space.get("id")
        if not isinstance(space_id, str) or not space_id or space_id in space_indexes:
            raise RuntimeError(f"invalid or duplicate dockspace benchmark space {space_id!r}")
        if space.get("floatings"):
            raise RuntimeError("ImGui rehome benchmark does not project floating containers")
        if space.get("keep_central_when_empty") or space.get("passthrough_when_empty"):
            raise RuntimeError(
                "ImGui rehome benchmark does not project central-region empty-space policy"
            )
        root = space.get("root")
        if not isinstance(root, dict):
            raise RuntimeError(f"dockspace benchmark space {space_id!r} must have a root")
        space_indexes[space_id] = index
        generated_spaces.append((space_id, add_node(root)))
        if index > 0:
            contract_parts.append(";")
        contract_parts.extend((space_id, "="))
        append_topology_contract(contract_parts, root)

    steps = scenario.get("steps")
    if not isinstance(steps, list) or not steps:
        raise RuntimeError("dockspace benchmark scenario must declare steps")
    generated_steps = []
    step_ids: set[str] = set()
    contract_parts.append("|steps=")
    directions = {
        "center": "ImGuiDir_None",
        "left": "ImGuiDir_Left",
        "right": "ImGuiDir_Right",
        "top": "ImGuiDir_Up",
        "bottom": "ImGuiDir_Down",
    }
    for index, step in enumerate(steps):
        if not isinstance(step, dict):
            raise RuntimeError("dockspace benchmark steps must be objects")
        operation = step.get("operation")
        if not isinstance(operation, dict) or operation.get("type") != "move_item_to_item":
            raise RuntimeError("ImGui rehome benchmark only supports named item moves")
        step_id = step.get("id")
        if not isinstance(step_id, str) or not step_id or step_id in step_ids:
            raise RuntimeError(f"invalid or duplicate dockspace benchmark step {step_id!r}")
        step_ids.add(step_id)
        item = operation.get("item")
        target_item = operation.get("target_item")
        source_space = operation.get("source_space")
        target_space = operation.get("target_space")
        zone = operation.get("zone")
        if item not in item_indexes or target_item not in item_indexes:
            raise RuntimeError("dockspace benchmark step references an unknown item")
        if source_space not in space_indexes or target_space not in space_indexes:
            raise RuntimeError("dockspace benchmark step references an unknown space")
        if zone not in directions:
            raise RuntimeError(f"unsupported dockspace benchmark drop zone {zone!r}")
        generated_steps.append(
            (
                step_id,
                item_indexes[item],
                space_indexes[source_space],
                item_indexes[target_item],
                space_indexes[target_space],
                directions[zone],
                zone,
            )
        )
        if index > 0:
            contract_parts.append(";")
        contract_parts.append(
            f"{item}@{source_space}>{target_item}@{target_space}:{zone}"
        )

    checkpoints = scenario.get("checkpoints")
    if not isinstance(checkpoints, list) or len(checkpoints) != len(generated_steps):
        raise RuntimeError(
            "dockspace benchmark scenario must declare one checkpoint per step"
        )
    expected_space_ids = list(space_indexes)
    expected_items = set(item_indexes)
    checkpoint_roots: list[int] = []
    generated_checkpoints: list[tuple[str, int, int]] = []
    for index, checkpoint in enumerate(checkpoints):
        if not isinstance(checkpoint, dict):
            raise RuntimeError("dockspace benchmark checkpoints must be objects")
        after_step = checkpoint.get("after_step")
        expected_step = generated_steps[index][0]
        if after_step != expected_step:
            raise RuntimeError(
                "dockspace benchmark checkpoints must follow step order: "
                f"expected {expected_step!r}, found {after_step!r}"
            )
        checkpoint_spaces = checkpoint.get("spaces")
        if not isinstance(checkpoint_spaces, list):
            raise RuntimeError("dockspace benchmark checkpoint spaces must be a list")
        checkpoint_space_ids = [
            space.get("id") if isinstance(space, dict) else None
            for space in checkpoint_spaces
        ]
        if checkpoint_space_ids != expected_space_ids:
            raise RuntimeError(
                "dockspace benchmark checkpoint spaces must match scenario space order"
            )

        topology_items: set[str] = set()
        first_root = len(checkpoint_roots)
        for space in checkpoint_spaces:
            assert isinstance(space, dict)
            if space.get("floatings"):
                raise RuntimeError(
                    "ImGui rehome benchmark does not project checkpoint floating containers"
                )
            if space.get("keep_central_when_empty") or space.get(
                "passthrough_when_empty"
            ):
                raise RuntimeError(
                    "ImGui rehome benchmark does not project checkpoint empty-space policy"
                )
            root = space.get("root")
            if not isinstance(root, dict):
                raise RuntimeError(
                    f"dockspace benchmark checkpoint space {space.get('id')!r} "
                    "must have a root"
                )
            checkpoint_roots.append(add_node(root, topology_items))
        if topology_items != expected_items:
            missing = sorted(expected_items - topology_items)
            extra = sorted(topology_items - expected_items)
            raise RuntimeError(
                "dockspace benchmark checkpoint item ownership is incomplete: "
                f"missing={missing!r}, extra={extra!r}"
            )
        generated_checkpoints.append(
            (after_step, first_root, len(checkpoint_spaces))
        )
    contract_parts.append("|final=initial")
    contract = "".join(contract_parts)

    def cpp_string(value: str) -> str:
        return json.dumps(value, ensure_ascii=True)

    lines = [
        "// Generated from dockspace_scenarios_v2.json. Do not edit.",
        "static constexpr RehomeItemSpec RehomeItems[] = {",
    ]
    lines.extend(
        f"    {{{cpp_string(item)}, {cpp_string(f'Benchmark Cycle {item}')}}},"
        for item in items
    )
    lines.extend(("};", "", "static constexpr int RehomeNodeItems[] = {"))
    lines.append("    " + ", ".join(map(str, node_items)) + ",")
    lines.extend(("};", "", "static constexpr int RehomeNodeChildren[] = {"))
    lines.append("    " + ", ".join(map(str, node_children)) + ",")
    lines.extend(("};", "", "static constexpr RehomeTopologyNode RehomeNodes[] = {"))
    for node in nodes:
        assert node is not None
        lines.append(
            "    {"
            f"{node['kind']}, {node['axis']}, {cpp_float(node['ratio'])}, "
            f"{node['first_item']}, {node['item_count']}, "
            f"{node['selected_item']}, "
            f"{node['first_child']}, {node['child_count']}"
            "},"
        )
    lines.extend(("};", "", "static constexpr RehomeSpaceSpec RehomeSpaces[] = {"))
    lines.extend(
        "    {"
        f"{cpp_string(space_id)}, {cpp_string(f'Benchmark Rehome {space_id}')}, {root_node}"
        "},"
        for space_id, root_node in generated_spaces
    )
    lines.extend(("};", "", "static constexpr RehomeStep RehomeSteps[] = {"))
    lines.extend(
        "    {"
        f"{cpp_string(step_id)}, {payload}, {source}, {target}, {target_space}, "
        f"{direction}, {cpp_string(zone)}"
        "},"
        for step_id, payload, source, target, target_space, direction, zone in generated_steps
    )
    lines.extend(("};", "", "static constexpr int RehomeCheckpointRoots[] = {"))
    lines.append("    " + ", ".join(map(str, checkpoint_roots)) + ",")
    lines.extend(("};", "", "static constexpr RehomeCheckpointSpec RehomeCheckpoints[] = {"))
    lines.extend(
        f"    {{{cpp_string(after_step)}, {first_root}, {root_count}}},"
        for after_step, first_root, root_count in generated_checkpoints
    )
    lines.extend(
        (
            "};",
            "",
            "static constexpr const char RehomeWorkloadContract[] =",
            f"    {cpp_string(contract)};",
            "",
        )
    )
    return "\n".join(lines)


def generate_imgui_rehome_scenario_header() -> None:
    contents = render_imgui_rehome_scenario_header(
        load_three_dockspace_rehome_scenario()
    )
    IMGUI_SCENARIO_HEADER.parent.mkdir(parents=True, exist_ok=True)
    if (
        IMGUI_SCENARIO_HEADER.exists()
        and IMGUI_SCENARIO_HEADER.read_text(encoding="utf-8") == contents
    ):
        return
    IMGUI_SCENARIO_HEADER.write_text(contents, encoding="utf-8", newline="\n")


def build_imgui_benchmark() -> None:
    compiler = shutil.which("g++")
    if compiler is None:
        raise RuntimeError("g++ is required for the local Dear ImGui comparison")
    generate_imgui_rehome_scenario_header()

    dependencies = [
        IMGUI_SOURCE,
        DOCKSPACE_SCENARIO_FIXTURES,
        IMGUI_SCENARIO_HEADER,
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
            f"-I{IMGUI_SCENARIO_HEADER.parent}",
            str(IMGUI_SOURCE),
            str(IMGUI_DIR / "imgui_draw.cpp"),
            str(IMGUI_DIR / "imgui_tables.cpp"),
            str(IMGUI_DIR / "imgui_widgets.cpp"),
            "-o",
            str(IMGUI_BINARY),
        ]
    )


def native_latency_process_sample_count() -> int:
    raw = os.environ.get(
        NATIVE_LATENCY_PROCESS_SAMPLES_ENV,
        str(NATIVE_LATENCY_DEFAULT_PROCESS_SAMPLES),
    )
    try:
        count = int(raw)
    except ValueError as error:
        raise RuntimeError(
            f"{NATIVE_LATENCY_PROCESS_SAMPLES_ENV} must be an integer"
        ) from error
    if count < 2:
        raise RuntimeError(
            f"{NATIVE_LATENCY_PROCESS_SAMPLES_ENV} must be at least 2 to measure process noise"
        )
    return count


def percentile(values: list[int], quantile: float) -> int:
    if not values:
        raise RuntimeError("native latency percentile requires at least one sample")
    ordered = sorted(values)
    index = max(0, math.ceil(quantile * len(ordered)) - 1)
    return ordered[index]


def validate_native_lifecycle_run(
    report: dict[str, Any], *, source: str
) -> list[dict[str, Any]]:
    if report.get("schema_version") != 1:
        raise RuntimeError(f"{source} must use native lifecycle schema version 1")
    if report.get("fixture_id") != "three_dockspace_rehome_cycle":
        raise RuntimeError(f"{source} must execute three_dockspace_rehome_cycle")
    if report.get("adapter") != "windows_real_three_hwnd":
        raise RuntimeError(f"{source} must use the real three-HWND adapter")
    if report.get("window_count") != 3 or report.get("distinct_hwnd_count") != 3:
        raise RuntimeError(f"{source} did not retain three distinct HWNDs")
    if tuple(report.get("boundary_names", ())) != NATIVE_LIFECYCLE_BOUNDARIES:
        raise RuntimeError(f"{source} declares an unexpected lifecycle boundary set")
    samples = report.get("samples")
    if not isinstance(samples, list) or not samples:
        raise RuntimeError(f"{source} must contain lifecycle samples")
    if report.get("sample_count") != len(samples):
        raise RuntimeError(f"{source} sample_count does not match samples")

    for sample_index, sample in enumerate(samples):
        sample_source = f"{source} sample {sample_index}"
        if not isinstance(sample, dict):
            raise RuntimeError(f"{sample_source} must be an object")
        if sample.get("source_hwnd") == sample.get("target_hwnd"):
            raise RuntimeError(f"{sample_source} must cross distinct HWNDs")
        origins = sample.get("origins")
        if not isinstance(origins, list) or {
            origin.get("name") for origin in origins if isinstance(origin, dict)
        } != {"drag_threshold_move", "target_route_move", "primary_release"}:
            raise RuntimeError(f"{sample_source} is missing a timing origin")
        raw_events = sample.get("raw_events")
        if not isinstance(raw_events, list) or not raw_events:
            raise RuntimeError(f"{sample_source} must preserve raw events")
        boundaries = sample.get("boundaries")
        if not isinstance(boundaries, list):
            raise RuntimeError(f"{sample_source} boundaries must be a list")
        indexed = {}
        for boundary in boundaries:
            if not isinstance(boundary, dict):
                raise RuntimeError(f"{sample_source} boundary must be an object")
            name = boundary.get("name")
            if name in indexed:
                raise RuntimeError(f"{sample_source} repeats boundary {name!r}")
            indexed[name] = boundary
        if tuple(indexed) != NATIVE_LIFECYCLE_BOUNDARIES:
            raise RuntimeError(f"{sample_source} has an incomplete boundary sequence")
        for name, boundary in indexed.items():
            status = boundary.get("status")
            timing = boundary.get("timing")
            elapsed_ns = boundary.get("elapsed_ns")
            if boundary.get("origin") != NATIVE_LIFECYCLE_BOUNDARY_ORIGINS[name]:
                raise RuntimeError(
                    f"{sample_source} boundary {name!r} has the wrong timing origin"
                )
            if status == "observed":
                valid = timing == "exact_callback" and type(elapsed_ns) is int
            elif status == "estimated":
                valid = timing == "observation_upper_bound" and type(elapsed_ns) is int
            elif status == "unavailable":
                valid = timing == "unavailable" and elapsed_ns is None
            else:
                valid = False
            if not valid or (type(elapsed_ns) is int and elapsed_ns < 0):
                raise RuntimeError(
                    f"{sample_source} boundary {name!r} has inconsistent timing evidence"
                )
        graph_receipt = indexed["graph_receipt"]
        if (
            graph_receipt.get("status") != "observed"
            or graph_receipt.get("timing") != "exact_callback"
            or type(graph_receipt.get("revision")) is not int
        ):
            raise RuntimeError(
                f"{sample_source} must carry an exact graph receipt callback"
            )
    return samples


def summarize_native_latency(evidence: dict[str, Any]) -> dict[str, Any]:
    runs = evidence.get("runs")
    if not isinstance(runs, list) or len(runs) < 2:
        raise RuntimeError("native lifecycle evidence requires at least two process runs")

    samples_by_run = [
        validate_native_lifecycle_run(run, source=f"native lifecycle run {index}")
        for index, run in enumerate(runs)
        if isinstance(run, dict)
    ]
    if len(samples_by_run) != len(runs):
        raise RuntimeError("native lifecycle runs must be JSON objects")

    boundary_summary: dict[str, dict[str, Any]] = {}
    flattened: dict[str, Any] = {}
    for boundary_name in NATIVE_LIFECYCLE_BOUNDARIES:
        observations = []
        per_process_values: list[list[int]] = []
        status_counts = {"observed": 0, "estimated": 0, "unavailable": 0}
        for samples in samples_by_run:
            process_values = []
            for sample in samples:
                boundary = next(
                    value
                    for value in sample["boundaries"]
                    if value["name"] == boundary_name
                )
                status_counts[boundary["status"]] += 1
                elapsed_ns = boundary["elapsed_ns"]
                if type(elapsed_ns) is int:
                    observations.append(elapsed_ns)
                    process_values.append(elapsed_ns)
            per_process_values.append(process_values)
        if not observations:
            timing = {
                "p50_ms": None,
                "p95_ms": None,
                "p99_ms": None,
                "max_ms": None,
            }
            process_medians = []
            noise_floor = {
                "process_median_min_ms": None,
                "process_median_max_ms": None,
                "process_median_span_ms": None,
                "process_median_span_ratio": None,
            }
        else:
            timing = {
                "p50_ms": percentile(observations, 0.50) / 1_000_000.0,
                "p95_ms": percentile(observations, 0.95) / 1_000_000.0,
                "p99_ms": percentile(observations, 0.99) / 1_000_000.0,
                "max_ms": max(observations) / 1_000_000.0,
            }
            process_medians = [
                int(statistics.median(values)) for values in per_process_values if values
            ]
            median_min = min(process_medians)
            median_max = max(process_medians)
            median_center = statistics.median(process_medians)
            median_span = median_max - median_min
            noise_floor = {
                "process_median_min_ms": median_min / 1_000_000.0,
                "process_median_max_ms": median_max / 1_000_000.0,
                "process_median_span_ms": median_span / 1_000_000.0,
                "process_median_span_ratio": (
                    median_span / median_center if median_center > 0 else None
                ),
            }
        boundary_summary[boundary_name] = {
            "sample_count": sum(status_counts.values()),
            "timed_sample_count": len(observations),
            "status_counts": status_counts,
            "timing": timing,
            "noise_floor": noise_floor,
            "process_medians_ns": process_medians,
        }
        for quantile in ("p50_ms", "p95_ms", "p99_ms", "max_ms"):
            flattened[f"native_latency_{boundary_name}_{quantile}"] = timing[quantile]

    workflow_sample_count = sum(len(samples) for samples in samples_by_run)
    return {
        "native_latency_benchmark_passed": 1,
        "native_latency_process_sample_count": len(runs),
        "native_latency_workflow_sample_count": workflow_sample_count,
        "native_latency_boundaries": boundary_summary,
        **flattened,
    }


def imgui_scenario_rows(result: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "scenario_id": "synthetic_imgui_single_host_preview",
            "step_id": None,
            "operation_family": "dock_preview_setup",
            "measurement_layer": "imgui_internal_preview_setup",
            "scope": "one_window_one_payload",
            "imgui_ns_per_operation": result["imgui_ns_per_preview"],
        },
        {
            "scenario_id": "synthetic_imgui_eight_window_preview",
            "step_id": None,
            "operation_family": "dock_preview_setup",
            "measurement_layer": "imgui_internal_preview_setup",
            "scope": "eight_windows_one_imgui_viewport_one_payload",
            "imgui_ns_per_operation": result["imgui_multiview_known_host_ns_per_preview"],
        },
        {
            "scenario_id": "synthetic_imgui_single_host_dockable_target",
            "step_id": None,
            "operation_family": "accepted_dockable_target",
            "measurement_layer": "imgui_drag_drop_target",
            "scope": "one_window_one_payload",
            "imgui_ns_per_operation": result["imgui_dockable_target_ns_per_preview"],
        },
        {
            "scenario_id": "synthetic_imgui_eight_window_dockable_target",
            "step_id": None,
            "operation_family": "accepted_dockable_target",
            "measurement_layer": "imgui_drag_drop_target",
            "scope": "eight_windows_one_imgui_viewport_one_payload",
            "imgui_ns_per_operation": result[
                "imgui_multiview_dockable_target_known_host_ns_per_preview"
            ],
        },
        {
            "scenario_id": "three_dockspace_rehome_cycle",
            "step_id": None,
            "operation_family": "named_item_docking_sequence",
            "measurement_layer": "headless_logical_dockspace_resolve_and_commit",
            "scope": "three_dockspaces_8_steps_2_edge_6_center_no_platform_windows",
            "imgui_ns_per_operation": result[
                "imgui_three_dockspace_rehome_cycle_ns_per_sequence"
            ],
            "operation_step_count": result[
                "imgui_three_dockspace_rehome_operation_step_count"
            ],
            "workload_contract": result[
                "imgui_three_dockspace_rehome_workload_contract"
            ],
        },
    ]


def comparison_row_key(
    row: dict[str, Any], *, source: str
) -> tuple[str, str | None, str, str, str]:
    values: list[str | None] = []
    for field in COMPARISON_KEY_FIELDS:
        value = row.get(field)
        if field == "step_id" and value is None:
            values.append(None)
            continue
        if not isinstance(value, str) or not value:
            raise RuntimeError(f"{source} benchmark row has invalid {field}: {value!r}")
        values.append(value)
    return (values[0], values[1], values[2], values[3], values[4])  # type: ignore[return-value]


def index_scenario_rows(
    rows: Any, *, source: str
) -> dict[tuple[str, str | None, str, str, str], dict[str, Any]]:
    if not isinstance(rows, list):
        raise RuntimeError(f"{source} benchmark rows must be a list")
    indexed = {}
    for row in rows:
        if not isinstance(row, dict):
            raise RuntimeError(f"{source} benchmark row must be an object")
        key = comparison_row_key(row, source=source)
        if key in indexed:
            raise RuntimeError(f"{source} benchmark rows repeat comparison key {key!r}")
        indexed[key] = row
    return indexed


def merge_open_gpui_scenario_rows(
    timing_rows: Any, allocation_rows: Any
) -> list[dict[str, Any]]:
    timing_index = index_scenario_rows(timing_rows, source="Open GPUI timing")
    allocation_index = index_scenario_rows(
        allocation_rows, source="Open GPUI allocation"
    )
    unknown_allocation_keys = allocation_index.keys() - timing_index.keys()
    if unknown_allocation_keys:
        raise RuntimeError(
            "Open GPUI allocation rows have no timing row for "
            f"{sorted(unknown_allocation_keys, key=repr)!r}"
        )
    missing_allocation_keys = timing_index.keys() - allocation_index.keys()
    if missing_allocation_keys:
        raise RuntimeError(
            "Open GPUI timing rows have no allocation row for "
            f"{sorted(missing_allocation_keys, key=repr)!r}"
        )

    merged_rows = []
    for timing_row in timing_rows:
        key = comparison_row_key(timing_row, source="Open GPUI timing")
        merged = dict(timing_row)
        allocation_row = allocation_index[key]
        for field, value in allocation_row.items():
            if field in COMPARISON_KEY_FIELDS:
                continue
            if field in merged and merged[field] != value:
                raise RuntimeError(
                    f"Open GPUI benchmark row {key!r} disagrees on {field}"
                )
            merged[field] = value
        merged_rows.append(merged)
    return merged_rows


def positive_finite_number(value: Any) -> bool:
    return (
        not isinstance(value, bool)
        and isinstance(value, (int, float))
        and math.isfinite(float(value))
        and value > 0
    )


def positive_step_count(value: Any) -> bool:
    return type(value) is int and value > 0


def require_passing_benchmark_reports(result: dict[str, Any]) -> None:
    for field in ("benchmark_passed", "allocation_benchmark_passed"):
        value = result.get(field)
        if type(value) is not int or value != 1:
            raise RuntimeError(
                f"benchmark comparison requires {field}=1, found {value!r}"
            )


def scenario_comparison_rows(result: dict[str, Any]) -> list[dict[str, Any]]:
    require_passing_benchmark_reports(result)
    open_gpui_rows = index_scenario_rows(
        result.get("open_gpui_scenario_rows", []), source="Open GPUI"
    )
    imgui_rows = index_scenario_rows(
        result.get("imgui_scenario_rows", []), source="Dear ImGui"
    )
    rows = []
    for key in sorted(open_gpui_rows.keys() & imgui_rows.keys(), key=repr):
        open_gpui_row = open_gpui_rows[key]
        imgui_row = imgui_rows[key]
        open_gpui_ns = open_gpui_row.get("open_gpui_ns_per_operation")
        imgui_ns = imgui_row.get("imgui_ns_per_operation")
        if not positive_finite_number(open_gpui_ns):
            raise RuntimeError(f"Open GPUI benchmark row {key!r} has invalid timing")
        if not positive_finite_number(imgui_ns):
            raise RuntimeError(f"Dear ImGui benchmark row {key!r} has invalid timing")
        open_gpui_step_count = open_gpui_row.get("operation_step_count")
        imgui_step_count = imgui_row.get("operation_step_count")
        if not positive_step_count(open_gpui_step_count):
            raise RuntimeError(f"Open GPUI benchmark row {key!r} has invalid step count")
        if not positive_step_count(imgui_step_count):
            raise RuntimeError(f"Dear ImGui benchmark row {key!r} has invalid step count")
        if open_gpui_step_count != imgui_step_count:
            raise RuntimeError(f"benchmark row {key!r} has mismatched step counts")
        open_gpui_contract = open_gpui_row.get("workload_contract")
        imgui_contract = imgui_row.get("workload_contract")
        if not isinstance(open_gpui_contract, str) or not open_gpui_contract:
            raise RuntimeError(f"Open GPUI benchmark row {key!r} has invalid workload contract")
        if not isinstance(imgui_contract, str) or not imgui_contract:
            raise RuntimeError(f"Dear ImGui benchmark row {key!r} has invalid workload contract")
        if open_gpui_contract != imgui_contract:
            raise RuntimeError(f"benchmark row {key!r} has mismatched workload contracts")
        rows.append(
            {
                "scenario_id": key[0],
                "step_id": key[1],
                "operation_family": key[2],
                "measurement_layer": key[3],
                "scope": key[4],
                "open_gpui_ns_per_operation": open_gpui_ns,
                "imgui_ns_per_operation": imgui_ns,
                "open_gpui_to_imgui_ratio": open_gpui_ns / imgui_ns,
                "operation_step_count": open_gpui_step_count,
                "workload_contract": open_gpui_contract,
                "comparison_kind": "semantic_end_to_end_logical_dockspace_transaction",
            }
        )
    return rows


def require_comparison_keys(
    rows: list[dict[str, Any]],
    required_keys: frozenset[tuple[str, str | None, str, str, str]],
) -> None:
    observed_keys = {
        comparison_row_key(row, source="scenario comparison") for row in rows
    }
    missing = required_keys - observed_keys
    if missing:
        raise RuntimeError(
            f"required benchmark comparison rows are missing: {sorted(missing, key=repr)!r}"
        )


def native_benchmark_git_metadata() -> dict[str, Any]:
    commit = run(["git", "rev-parse", "HEAD"]).strip()
    status = run(["git", "status", "--porcelain=v1"])
    return {
        "commit": commit,
        "dirty": bool(status.strip()),
        "changed_path_count": len(status.splitlines()),
    }


def native_benchmark_environment() -> dict[str, Any]:
    return {
        "platform": platform.platform(),
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "python": platform.python_version(),
        "cpu_count": os.cpu_count(),
        "cargo_build_jobs": os.environ.get("CARGO_BUILD_JOBS", "1"),
    }


def run_native_latency_benchmark() -> dict[str, Any]:
    if platform.system() != "Windows":
        raise RuntimeError("native three-HWND lifecycle benchmark requires Windows")

    NATIVE_LATENCY_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    NATIVE_LATENCY_RAW_DIR.mkdir(parents=True, exist_ok=True)
    process_sample_count = native_latency_process_sample_count()
    command = [
        "cargo",
        "nextest",
        "run",
        "--release",
        "--locked",
        "-p",
        "open-gpui-docking-native",
        "--run-ignored",
        "only",
        "--test-threads",
        "1",
        "--no-fail-fast",
        "-E",
        f"test(={NATIVE_LATENCY_TEST})",
    ]
    runs: list[dict[str, Any]] = []
    for process_index in range(process_sample_count):
        run_output = NATIVE_LATENCY_RAW_DIR / f"run-{process_index + 1:03d}.json"
        run_output.unlink(missing_ok=True)
        run(
            command,
            extra_env={
                NATIVE_LATENCY_OUTPUT_ENV: str(run_output),
                NATIVE_INTERACTIVE_ENV: "1",
            },
        )
        with run_output.open("r", encoding="utf-8") as report_file:
            report = json.load(report_file)
        if not isinstance(report, dict):
            raise RuntimeError(
                f"native lifecycle benchmark run {process_index + 1} did not emit a JSON object"
            )
        validate_native_lifecycle_run(
            report, source=f"native lifecycle benchmark run {process_index + 1}"
        )
        report["process_index"] = process_index
        report["raw_output"] = str(run_output.relative_to(ROOT))
        runs.append(report)

    evidence = {
        "schema_version": 1,
        "evidence_kind": "native_three_hwnd_lifecycle_baseline",
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "git": native_benchmark_git_metadata(),
        "environment": native_benchmark_environment(),
        "command": command,
        "profile": {"release": True, "locked": True, "nextest_threads": 1},
        "process_sample_count": process_sample_count,
        "runs": runs,
    }
    summary = summarize_native_latency(evidence)
    evidence["summary"] = summary
    NATIVE_LATENCY_OUTPUT.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return {**summary, "native_latency_evidence": evidence}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="emit one JSON object")
    parser.add_argument(
        "--native-latency",
        action="store_true",
        help=(
            "also run the ignored Windows native three-HWND lifecycle baseline; "
            "this opens windows and injects mouse input"
        ),
    )
    arguments = parser.parse_args()

    rust_timing = last_json_line(
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
    rust_allocations = last_json_line(
        run(
            [
                "cargo",
                "bench",
                "-p",
                "open-gpui-docking",
                "--bench",
                "tab_drag_allocations",
                "--features",
                "test-support",
            ]
        )
    )
    open_gpui_scenario_rows = merge_open_gpui_scenario_rows(
        rust_timing.get("open_gpui_scenario_rows", []),
        rust_allocations.get("open_gpui_allocation_rows", []),
    )
    build_imgui_benchmark()
    imgui = last_json_line(run([str(IMGUI_BINARY)]))

    result = {**rust_timing, **rust_allocations, **imgui}
    result["open_gpui_scenario_rows"] = open_gpui_scenario_rows
    result["imgui_scenario_rows"] = imgui_scenario_rows(result)
    result["scenario_comparison_rows"] = scenario_comparison_rows(result)
    require_comparison_keys(result["scenario_comparison_rows"], REQUIRED_COMPARISON_KEYS)
    result["scenario_comparison_key"] = "+".join(COMPARISON_KEY_FIELDS)
    result["scenario_comparison_contract_fields"] = (
        "operation_step_count+workload_contract"
    )
    result["scenario_comparison_pair_count"] = len(result["scenario_comparison_rows"])
    result["benchmark_passed"] = int(
        result.get("benchmark_passed", 0) == 1
        and result.get("allocation_benchmark_passed", 0) == 1
    )
    if arguments.native_latency:
        result.update(run_native_latency_benchmark())

    if arguments.json:
        print(json.dumps(result, sort_keys=True))
    else:
        for key, value in sorted(result.items()):
            print(f"{key}: {value}")


if __name__ == "__main__":
    main()
