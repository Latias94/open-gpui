import json
import math
import unittest
from contextlib import redirect_stdout
from copy import deepcopy
from io import StringIO
from unittest.mock import patch

from tools import benchmark_docking_drag as benchmark
from tools.benchmark_docking_drag import (
    COMPARISON_KEY_FIELDS,
    IMGUI_SOURCE,
    LEGACY_ADAPTER,
    NATIVE_LIFECYCLE_BOUNDARIES,
    THREE_DOCKSPACE_COMPARISON_KEY,
    load_three_dockspace_rehome_scenario,
    merge_open_gpui_scenario_rows,
    render_imgui_rehome_scenario_header,
    require_comparison_keys,
    scenario_comparison_rows,
    summarize_native_latency,
    validate_native_lifecycle_run,
)


def benchmark_row(**overrides):
    row = {
        "scenario_id": "shared_scenario",
        "step_id": "shared_step",
        "operation_family": "shared_operation",
        "measurement_layer": "shared_layer",
        "scope": "shared_scope",
        "operation_step_count": 8,
        "workload_contract": "shared_contract",
    }
    row.update(overrides)
    return row


def passing_benchmark_result(**overrides):
    result = {
        "open_gpui_adapter": LEGACY_ADAPTER,
        "benchmark_passed": 1,
        "allocation_benchmark_passed": 1,
    }
    result.update(overrides)
    return result


def shared_session_report(**overrides):
    report = {
        "schema_version": 1,
        "adapter": "shared_session",
        "measurement_level": "gpui_retained_host",
        "renderer": "gpui_test_platform",
        "native_windows": False,
        "benchmark_passed": 1,
        "rows": [{"scenario_id": "shared_retained_host", "ns_per_cycle": 240.0}],
    }
    report.update(overrides)
    return report


def native_lifecycle_run(process_index=0, *, graph_status="observed"):
    samples = []
    for step_index in range(2):
        boundaries = []
        for name in NATIVE_LIFECYCLE_BOUNDARIES:
            observed = name == "graph_receipt" and graph_status == "observed"
            boundaries.append(
                {
                    "name": name,
                    "status": "observed" if observed else "estimated",
                    "timing": "exact_callback"
                    if observed
                    else "observation_upper_bound",
                    "origin": {
                        "drag_activation": "drag_threshold_move",
                        "accepted_preview_submitted": "target_route_move",
                    }.get(name, "primary_release"),
                    "elapsed_ns": 1_000_000 + step_index * 100_000,
                    "source": "test",
                    "detail": "test evidence",
                    "generation": 10 + step_index,
                    "sequence": 20 + step_index,
                    "revision": 30 + step_index,
                }
            )
        samples.append(
            {
                "step_index": step_index,
                "source_hwnd": 101,
                "target_hwnd": 102,
                "origins": [
                    {"name": "drag_threshold_move"},
                    {"name": "target_route_move"},
                    {"name": "primary_release"},
                ],
                "raw_events": [{"name": "test", "timeline_elapsed_ns": 1}],
                "boundaries": boundaries,
            }
        )
    return {
        "schema_version": 1,
        "scenario_id": "native.test",
        "fixture_id": "three_dockspace_rehome_cycle",
        "adapter": "windows_real_three_hwnd",
        "window_count": 3,
        "distinct_hwnd_count": 3,
        "boundary_names": list(NATIVE_LIFECYCLE_BOUNDARIES),
        "sample_count": len(samples),
        "samples": samples,
        "process_index": process_index,
    }


class ScenarioComparisonRowsTests(unittest.TestCase):
    def test_imgui_fixture_is_generated_from_the_shared_scenario(self):
        scenario = deepcopy(load_three_dockspace_rehome_scenario())
        scenario["steps"][0]["operation"]["zone"] = "left"
        scenario["spaces"][0]["root"]["selected"] = "EEE"
        scenario["spaces"][2]["root"]["fractions"] = [1, 3]

        header = render_imgui_rehome_scenario_header(scenario)

        self.assertIn("ImGuiDir_Left", header)
        self.assertIn("AAA@source>CCC@target:left", header)
        self.assertIn("T[selected=EEE;items=EEE,AAA]", header)
        self.assertIn("Svertical[fractions=0.25,0.75]", header)

    def test_imgui_fixture_tracks_checkpoint_semantics(self):
        baseline_scenario = deepcopy(load_three_dockspace_rehome_scenario())
        baseline = render_imgui_rehome_scenario_header(baseline_scenario)

        selected = deepcopy(baseline_scenario)
        selected["checkpoints"][0]["spaces"][1]["root"]["selected"] = "FFF"
        self.assertNotEqual(render_imgui_rehome_scenario_header(selected), baseline)

        fraction = deepcopy(baseline_scenario)
        fraction["checkpoints"][0]["spaces"][2]["root"]["fractions"] = [1, 3]
        fraction_header = render_imgui_rehome_scenario_header(fraction)
        self.assertNotEqual(fraction_header, baseline)
        self.assertIn("0.75f", fraction_header)

        rebound_step = deepcopy(baseline_scenario)
        rebound_step["steps"][0]["id"] = "checkpoint_step_rebound"
        rebound_step["checkpoints"][0]["after_step"] = "checkpoint_step_rebound"
        rebound_header = render_imgui_rehome_scenario_header(rebound_step)
        self.assertNotEqual(rebound_header, baseline)
        self.assertEqual(rebound_header.count('"checkpoint_step_rebound"'), 2)

        added_item = deepcopy(baseline_scenario)
        added_item["spaces"][0]["root"]["items"].append("GGG")
        for checkpoint in added_item["checkpoints"]:
            checkpoint["spaces"][0]["root"]["items"].append("GGG")
        added_item_header = render_imgui_rehome_scenario_header(added_item)
        self.assertNotEqual(added_item_header, baseline)
        self.assertIn('{"GGG", "Benchmark Cycle GGG"}', added_item_header)

    def test_imgui_fixture_initializes_windows_from_generated_items(self):
        source = IMGUI_SOURCE.read_text(encoding="utf-8")
        first_frame = source[
            source.index("ImGui::NewFrame();") : source.index("ImGui::Render();")
        ]

        self.assertIn("for (const RehomeItemSpec& item : RehomeItems)", first_frame)
        self.assertIn("BeginBenchmarkHost(item.WindowName", first_frame)
        self.assertNotIn('BeginBenchmarkHost("Benchmark Cycle', first_frame)

    def test_pairs_only_rows_with_the_complete_comparison_key(self):
        result = passing_benchmark_result(
            open_gpui_scenario_rows=[
                benchmark_row(open_gpui_ns_per_operation=120.0)
            ],
            imgui_scenario_rows=[benchmark_row(imgui_ns_per_operation=80.0)],
        )

        self.assertEqual(
            scenario_comparison_rows(result),
            [
                benchmark_row(
                    open_gpui_adapter=LEGACY_ADAPTER,
                    open_gpui_ns_per_operation=120.0,
                    imgui_ns_per_operation=80.0,
                    open_gpui_to_imgui_ratio=1.5,
                    comparison_kind="semantic_end_to_end_logical_dockspace_transaction",
                )
            ],
        )

    def test_shared_or_unlabeled_adapter_cannot_supply_legacy_comparison_ratios(self):
        for adapter in ("shared_session", None):
            with self.subTest(adapter=adapter):
                result = passing_benchmark_result(
                    open_gpui_adapter=adapter,
                    open_gpui_scenario_rows=[
                        benchmark_row(open_gpui_ns_per_operation=120.0)
                    ],
                    imgui_scenario_rows=[benchmark_row(imgui_ns_per_operation=80.0)],
                )
                with self.assertRaisesRegex(RuntimeError, "explicit legacy_docking adapter"):
                    scenario_comparison_rows(result)

    def test_rejects_cross_layer_or_cross_scope_ratios(self):
        result = passing_benchmark_result(
            open_gpui_scenario_rows=[
                benchmark_row(open_gpui_ns_per_operation=120.0)
            ],
            imgui_scenario_rows=[
                benchmark_row(
                    measurement_layer="different_layer",
                    scope="different_scope",
                    imgui_ns_per_operation=80.0,
                )
            ],
        )

        self.assertEqual(scenario_comparison_rows(result), [])

    def test_rejects_cross_step_ratios(self):
        result = passing_benchmark_result(
            open_gpui_scenario_rows=[
                benchmark_row(open_gpui_ns_per_operation=120.0)
            ],
            imgui_scenario_rows=[
                benchmark_row(step_id="different_step", imgui_ns_per_operation=80.0)
            ],
        )

        self.assertEqual(scenario_comparison_rows(result), [])

    def test_duplicate_keys_fail_closed(self):
        row = benchmark_row(open_gpui_ns_per_operation=120.0)
        result = passing_benchmark_result(
            open_gpui_scenario_rows=[row, row.copy()],
            imgui_scenario_rows=[benchmark_row(imgui_ns_per_operation=80.0)],
        )

        with self.assertRaisesRegex(RuntimeError, "repeat comparison key"):
            scenario_comparison_rows(result)

    def test_mismatched_workload_contracts_fail_closed(self):
        result = passing_benchmark_result(
            open_gpui_scenario_rows=[
                benchmark_row(open_gpui_ns_per_operation=120.0)
            ],
            imgui_scenario_rows=[
                benchmark_row(
                    workload_contract="different_contract",
                    imgui_ns_per_operation=80.0,
                )
            ],
        )

        with self.assertRaisesRegex(RuntimeError, "mismatched workload contracts"):
            scenario_comparison_rows(result)

    def test_mismatched_step_counts_fail_closed(self):
        result = passing_benchmark_result(
            open_gpui_scenario_rows=[
                benchmark_row(open_gpui_ns_per_operation=120.0)
            ],
            imgui_scenario_rows=[
                benchmark_row(operation_step_count=7, imgui_ns_per_operation=80.0)
            ],
        )

        with self.assertRaisesRegex(RuntimeError, "mismatched step counts"):
            scenario_comparison_rows(result)

    def test_non_finite_and_boolean_timings_fail_closed(self):
        for invalid_timing in [True, math.nan, math.inf]:
            with self.subTest(invalid_timing=invalid_timing):
                result = passing_benchmark_result(
                    open_gpui_scenario_rows=[
                        benchmark_row(open_gpui_ns_per_operation=invalid_timing)
                    ],
                    imgui_scenario_rows=[
                        benchmark_row(imgui_ns_per_operation=80.0)
                    ],
                )
                with self.assertRaisesRegex(RuntimeError, "invalid timing"):
                    scenario_comparison_rows(result)

    def test_failed_reports_do_not_generate_comparison_ratios(self):
        rows = {
            "open_gpui_scenario_rows": [
                benchmark_row(open_gpui_ns_per_operation=120.0)
            ],
            "imgui_scenario_rows": [benchmark_row(imgui_ns_per_operation=80.0)],
        }
        for field in ("benchmark_passed", "allocation_benchmark_passed"):
            with self.subTest(field=field):
                result = passing_benchmark_result(**rows)
                result[field] = 0
                with self.assertRaisesRegex(RuntimeError, f"{field}=1"):
                    scenario_comparison_rows(result)

    def test_merges_allocation_rows_only_into_matching_timing_rows(self):
        timing = benchmark_row(open_gpui_ns_per_operation=120.0)
        allocation = benchmark_row(
            open_gpui_allocations_per_operation=2.0,
            open_gpui_bytes_per_operation=64.0,
        )

        self.assertEqual(
            merge_open_gpui_scenario_rows([timing], [allocation]),
            [
                benchmark_row(
                    open_gpui_ns_per_operation=120.0,
                    open_gpui_allocations_per_operation=2.0,
                    open_gpui_bytes_per_operation=64.0,
                )
            ],
        )

    def test_rejects_timing_rows_without_allocation_measurements(self):
        timing = benchmark_row(open_gpui_ns_per_operation=120.0)

        with self.assertRaisesRegex(RuntimeError, "have no allocation row"):
            merge_open_gpui_scenario_rows([timing], [])

    def test_required_comparison_key_must_be_present(self):
        required = frozenset(
            {
                (
                    "required",
                    None,
                    "operation",
                    "layer",
                    "scope",
                )
            }
        )
        with self.assertRaisesRegex(RuntimeError, "required benchmark comparison"):
            require_comparison_keys([], required)


class BenchmarkRunnerTests(unittest.TestCase):
    def test_legacy_runner_enables_both_features_and_labels_the_comparator(self):
        key = dict(zip(COMPARISON_KEY_FIELDS, THREE_DOCKSPACE_COMPARISON_KEY))
        timing = passing_benchmark_result(
            open_gpui_scenario_rows=[benchmark_row(**key, open_gpui_ns_per_operation=120.0)]
        )
        allocations = {
            "open_gpui_adapter": LEGACY_ADAPTER,
            "allocation_benchmark_passed": 1,
            "open_gpui_allocation_rows": [
                benchmark_row(
                    **key,
                    open_gpui_allocations_per_operation=2.0,
                    open_gpui_bytes_per_operation=64.0,
                )
            ],
        }
        imgui = {
            "imgui_ns_per_preview": 10.0,
            "imgui_multiview_known_host_ns_per_preview": 10.0,
            "imgui_dockable_target_ns_per_preview": 10.0,
            "imgui_multiview_dockable_target_known_host_ns_per_preview": 10.0,
            "imgui_three_dockspace_rehome_cycle_ns_per_sequence": 80.0,
            "imgui_three_dockspace_rehome_operation_step_count": 8,
            "imgui_three_dockspace_rehome_workload_contract": "shared_contract",
        }
        with (
            patch.object(
                benchmark, "run",
                side_effect=[json.dumps(report) for report in (timing, allocations, imgui)],
            ) as run,
            patch.object(benchmark, "build_imgui_benchmark") as build_imgui,
        ):
            result = benchmark.run_legacy_comparison()

        names = ("legacy_tab_drag_latency", "legacy_tab_drag_allocations")
        for command_call, name in zip(run.call_args_list[:2], names):
            command = command_call.args[0]
            self.assertEqual(command[command.index("--bench") + 1], name)
            self.assertEqual(
                set(command[command.index("--features") + 1].split(",")),
                {"test-support", "legacy-docking"},
            )
        build_imgui.assert_called_once_with()
        self.assertEqual(result["scenario_comparison_pair_count"], 1)
        comparison = result["scenario_comparison_rows"][0]
        self.assertEqual(comparison["open_gpui_adapter"], LEGACY_ADAPTER)
        self.assertEqual(comparison["open_gpui_to_imgui_ratio"], 1.5)
        self.assertEqual(
            result["open_gpui_scenario_rows"][0]["open_gpui_allocations_per_operation"], 2.0
        )

    def test_shared_only_cli_skips_legacy_and_imgui_and_nests_host_data(self):
        report = shared_session_report()
        output = StringIO()
        with (
            patch("sys.argv", ["benchmark_docking_drag.py", "--suite", "shared", "--json"]),
            patch.object(benchmark, "run", return_value=json.dumps(report)) as run,
            patch.object(benchmark, "build_imgui_benchmark") as build_imgui,
            redirect_stdout(output),
        ):
            benchmark.main()

        result = json.loads(output.getvalue())
        self.assertEqual(result["shared_session"], report)
        self.assertNotIn("open_gpui_scenario_rows", result)
        self.assertNotIn("scenario_comparison_rows", result)
        build_imgui.assert_not_called()
        run.assert_called_once_with(
            [
                "cargo", "bench", "--locked", "-p", "open-gpui-docking", "--bench",
                "shared_session_cpu", "--features", "test-support",
            ]
        )

    def test_all_suites_preserve_legacy_metrics_and_separate_shared_data(self):
        legacy = passing_benchmark_result(
            open_gpui_scenario_rows=[benchmark_row(open_gpui_ns_per_operation=120.0)],
            scenario_comparison_rows=[{"open_gpui_to_imgui_ratio": 1.5}],
        )
        shared = shared_session_report()
        output = StringIO()
        with (
            patch("sys.argv", ["benchmark_docking_drag.py", "--suite", "all", "--json"]),
            patch.object(benchmark, "run_legacy_comparison", return_value=deepcopy(legacy)),
            patch.object(
                benchmark, "run_shared_session_benchmark", return_value=deepcopy(shared)
            ),
            redirect_stdout(output),
        ):
            benchmark.main()

        result = json.loads(output.getvalue())
        self.assertEqual(result["shared_session"], shared)
        self.assertEqual(result["open_gpui_scenario_rows"], legacy["open_gpui_scenario_rows"])
        self.assertEqual(result["scenario_comparison_rows"], legacy["scenario_comparison_rows"])
        self.assertNotIn("rows", result)

    def test_shared_runner_rejects_failed_core_only_or_invalid_timing_reports(self):
        for overrides in (
            {"benchmark_passed": 0},
            {"benchmark_passed": True},
            {"adapter": LEGACY_ADAPTER},
            {"measurement_level": "core_only"},
            {"native_windows": True},
            {"rows": []},
            {"rows": [{"ns_per_cycle": math.nan}]},
        ):
            with self.subTest(overrides=overrides):
                with patch.object(
                    benchmark, "run_rust_benchmark",
                    return_value=shared_session_report(**overrides),
                ):
                    with self.assertRaises(RuntimeError):
                        benchmark.run_shared_session_benchmark()


class NativeLifecycleEvidenceTests(unittest.TestCase):
    def test_validates_three_hwnd_six_boundary_evidence(self):
        report = native_lifecycle_run()
        self.assertEqual(validate_native_lifecycle_run(report, source="fixture"), report["samples"])

    def test_requires_exact_graph_receipt_callback(self):
        report = native_lifecycle_run(graph_status="estimated")
        with self.assertRaisesRegex(RuntimeError, "exact graph receipt"):
            validate_native_lifecycle_run(report, source="fixture")

    def test_summarizes_process_noise_and_preserves_status_counts(self):
        evidence = {"runs": [native_lifecycle_run(0), native_lifecycle_run(1)]}
        summary = summarize_native_latency(evidence)
        self.assertEqual(summary["native_latency_process_sample_count"], 2)
        self.assertEqual(summary["native_latency_workflow_sample_count"], 4)
        graph = summary["native_latency_boundaries"]["graph_receipt"]
        self.assertEqual(graph["status_counts"]["observed"], 4)
        self.assertEqual(graph["noise_floor"]["process_median_span_ratio"], 0.0)

    def test_rejects_non_three_hwnd_report(self):
        report = native_lifecycle_run()
        report["distinct_hwnd_count"] = 2
        with self.assertRaisesRegex(RuntimeError, "three distinct HWNDs"):
            validate_native_lifecycle_run(report, source="fixture")


if __name__ == "__main__":
    unittest.main()
