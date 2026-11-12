#!/usr/bin/env python3
"""Build two revisions once, then alternate bounded GPUI benchmark runs.

Usage: python scripts/compare_benchmarks.py BASELINE CANDIDATE
       python scripts/compare_benchmarks.py BASELINE CANDIDATE --filter TextPanels/

Ports Zed's script/gpui-bench-compare at
26abd1c9590827ffc5cca3058ff2a94ea9a34715, including its final fixed-target suite.
The invoking checkout supplies identical benchmark fixtures and the bench macro
to both revisions. Each revision must already provide their APIs and manifest
registrations. No Rust parsing or manifest rewriting is performed.

Each invocation creates two worktrees on new benchmark branches, even when a
requested revision is main. Worktrees, branches, revision-specific build artifacts,
copied executables, logs, Criterion samples and summary.json are retained under
target/gpui-bench-compare. Existing directories and branches are never removed.

The default core suite uses foreground retired instructions on macOS/Linux.
Linux requires perf access; macOS requires Apple Silicon. --measurement wall-time
also works on hosts without counters. Windows currently has no default headless
GPU renderer; its focus cleanup suite can run with --targets focus_cleanup
--measurement wall-time. Windows timeout/interrupt cleanup is best effort;
remaining descendants are unverified even when taskkill reports success.
GPU execution is separate from foreground CPU instructions. Spread is the range
of independent run medians divided by their median, not a confidence interval.
--slow restores the benchmarks' Criterion settings. --full selects all variants
of the selected targets; --targets also exposes focus cleanup and Metal uploads.
Builds and measurements run serially. Requires Python 3.11+ and Git.
"""

import argparse
from contextlib import contextmanager
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import statistics
import subprocess
import sys
import time
import uuid


DEFAULT_TARGETS = ("randomized_element_tree", "primitives", "inspector_render")
TARGETS = (*DEFAULT_TARGETS, "atlas_upload", "focus_cleanup")
FIXTURE_FILES = tuple(
    Path("crates/gpui_platform/benches") / f"{target}.rs"
    for target in TARGETS if target != "focus_cleanup"
) + (
    Path("crates/gpui/benches/focus_cleanup.rs"),
    Path("crates/gpui/benches/support/scene.rs"),
    Path("crates/gpui_macros/src/bench.rs"),
    Path("crates/gpui/src/randomized_element_tree.rs"),
)
CORE = {
    "randomized_element_tree": (
        r"^((Randomized|Cached)Tree/mixed/tree/mixed/|"
        r"RandomizedTree/full refresh/tree/|"
        r"PathlessTree/full refresh/tree/|"
        r"RandomizedTree/element style/tree/mixed/|"
        r"CachedTree/element style/tree/mixed/|"
        r"RandomizedTree/hover/tree/mixed/|"
        r"Scroll/workload/|"
        r"TextPanels/installed font two updates/panels/32$|"
        r"TextPanels/first populated installed font/panels/32$)"
    ),
    "primitives": r"^Primitives/(scene/[a-z]+-1024(-seed-\d+)?|paint/case/grid-1024)$",
    "inspector_render": (
        r"^closed_inspector_(grid/case/1000-nodes|panel/case/500-rows)-0-cycles$"
    ),
    "atlas_upload": "",
    "focus_cleanup": "",
}
FAST = [
    "--warm-up-time", "0.05",
    "--measurement-time", "0.3",
    "--sample-size", "10",
    "--nresamples", "1000",
]


@contextmanager
def comparison_lock(path):
    with path.open("a+b") as lock:
        if os.name == "nt":
            import msvcrt

            lock.seek(0, os.SEEK_END)
            if lock.tell() == 0:
                lock.write(b"\0")
                lock.flush()
            lock.seek(0)
            try:
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
            except OSError as error:
                raise RuntimeError(f"another comparison is using {path.parent}") from error
            try:
                yield
            finally:
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl

            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise RuntimeError(f"another comparison is using {path.parent}") from error
            try:
                yield
            finally:
                fcntl.flock(lock, fcntl.LOCK_UN)


def run(command, *, cwd, environment, log, timeout):
    started = time.monotonic()
    options = (
        {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
        if os.name == "nt" else {"start_new_session": True}
    )
    with log.open("x", encoding="utf-8") as output:
        process = subprocess.Popen(
            command, cwd=cwd, env=environment, stdout=output,
            stderr=subprocess.STDOUT, **options,
        )
        try:
            returncode = process.wait(timeout=timeout)
        except BaseException as error:
            if os.name == "nt":
                output.write(f"\nInterrupted {command[0]} (PID {process.pid}): {type(error).__name__}: {error}\n")
                output.flush()
                cleanup = []
                try:
                    result = subprocess.run(
                        ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                        stdout=output, stderr=subprocess.STDOUT,
                        timeout=10, check=True,
                    )
                    cleanup.append(f"taskkill exited with status {result.returncode}")
                except (OSError, subprocess.SubprocessError) as cleanup_error:
                    cleanup.append(f"taskkill failed: {type(cleanup_error).__name__}: {cleanup_error}")
                try:
                    if process.poll() is None:
                        process.kill()
                        cleanup.append("termination requested for the direct child only")
                    returncode = process.wait(timeout=10)
                    cleanup.append(f"direct child exited with status {returncode}")
                except (OSError, subprocess.TimeoutExpired) as cleanup_error:
                    cleanup.append(f"direct child cleanup failed: {type(cleanup_error).__name__}: {cleanup_error}")
                message = (
                    f"{type(error).__name__}: {error}; {'; '.join(cleanup)}; "
                    f"remaining Windows process tree is unverified; see {log}"
                )
                output.write(message + "\n")
                output.flush()
                raise RuntimeError(message) from error
            else:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            process.wait()
            if isinstance(error, subprocess.TimeoutExpired):
                raise RuntimeError(f"{error}; see {log}") from error
            raise
    if returncode != 0:
        raise RuntimeError(f"{command[0]} exited with status {returncode}; see {log}")
    return time.monotonic() - started


def git(root, *arguments):
    return subprocess.check_output(
        ["git", "-C", str(root), *arguments], text=True, encoding="utf-8", timeout=60,
    ).strip()


def sha256(path):
    with path.open("rb") as contents:
        return hashlib.file_digest(contents, "sha256").hexdigest()


def fixture_destination(source, relative):
    if relative.is_absolute() or not relative.parts or ".." in relative.parts:
        raise RuntimeError(f"invalid fixture path: {relative}")
    if source.is_symlink():
        raise RuntimeError(f"refusing symlink at fixture root: {source}")
    root = source.resolve(strict=True)
    destination = root
    for component in relative.parts:
        destination /= component
        if destination.is_symlink():
            raise RuntimeError(f"refusing symlink in fixture path: {destination}")
    if not destination.resolve().is_relative_to(root):
        raise RuntimeError(f"fixture path escapes its worktree: {destination}")
    return destination


def prepare(root, cache, output, run_id, role, revision, args, environment, fixtures):
    commit = git(root, "rev-parse", "--verify", "--end-of-options", f"{revision}^{{commit}}")
    branch = f"benchmark-compare/{run_id}/{role}"
    source = cache / "worktrees" / f"{run_id}-{role}-{commit[:12]}"
    source.parent.mkdir(parents=True, exist_ok=True)
    run(
        ["git", "worktree", "add", "-b", branch, str(source), commit],
        cwd=root, environment=environment, log=output / f"worktree-{role}.log",
        timeout=args.timeout,
    )
    digest = hashlib.sha256()
    fixture_hashes = {}
    for relative, content in fixtures.items():
        digest.update(relative.as_posix().encode())
        digest.update(b"\0")
        digest.update(content)
        fixture_hashes[relative.as_posix()] = hashlib.sha256(content).hexdigest()
        destination = fixture_destination(source, relative)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
    build_key = hashlib.sha256(
        (commit + digest.hexdigest() + args.features + args.profile).encode()
    ).hexdigest()[:20]
    artifacts = output / role
    artifacts.mkdir()
    environment = dict(environment, CARGO_TARGET_DIR=str(cache / "build" / build_key))
    feature_log = artifacts / "features.log"
    run(
        ["cargo", "tree", "--locked", "-p", "open-gpui-platform", "--features", args.features,
         "-e", "normal,build,dev", "--format", "{p} features={f}"],
        cwd=source, environment=environment, log=feature_log, timeout=args.timeout,
    )
    feature_text = feature_log.read_text(encoding="utf-8", errors="replace")
    core_features = re.findall(r"\bopen-gpui v[^\r\n]*? features=([^\s]*)", feature_text)
    if not core_features or any(
        {"test-support", "leak-detection"}.intersection(features.split(","))
        for features in core_features
    ):
        raise RuntimeError(f"missing Core feature graph or test/leak instrumentation enabled: {feature_log}")
    command = [
        "cargo", "bench", "--locked", "-p", "open-gpui-platform", "--features", args.features,
        "--profile", args.profile, "--no-run", "--message-format=json",
    ]
    for target in args.targets:
        command += ["--bench", target]
    elapsed = run(command, cwd=source, environment=environment,
                  log=artifacts / "build.log", timeout=args.timeout)
    binaries = {}
    for line in (artifacts / "build.log").read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact" or not message.get("executable"):
            continue
        name = message["target"]["name"]
        if name in args.targets and "bench" in message["target"]["kind"]:
            executable = Path(message["executable"])
            destination = artifacts / (name + executable.suffix)
            shutil.copy2(executable, destination)
            binaries[name] = destination
    if set(binaries) != set(args.targets):
        raise RuntimeError(f"missing benchmark executable; see {artifacts / 'build.log'}")
    return {
        "revision": revision, "commit": commit, "branch": branch,
        "source": str(source), "build_seconds": elapsed,
        "gpui_tree": git(root, "rev-parse", f"{commit}:crates/gpui"),
        "binary_sha256": {target: sha256(binary) for target, binary in binaries.items()},
        "benchmark_sha256": digest.hexdigest(), "fixture_sha256": fixture_hashes,
        "lockfile_sha256": sha256(source / "Cargo.lock"), "binaries": binaries,
    }


def verify_distinct_artifacts(revisions, targets):
    baseline, candidate = revisions
    if baseline["benchmark_sha256"] != candidate["benchmark_sha256"]:
        raise RuntimeError("the revisions were built with different benchmark fixtures")
    if baseline["gpui_tree"] != candidate["gpui_tree"]:
        for target in targets:
            if baseline["binary_sha256"][target] == candidate["binary_sha256"][target]:
                raise RuntimeError(
                    f"{target} binaries are byte-identical despite different crates/gpui trees; "
                    "refusing a potentially contaminated comparison"
                )


def measure(binary, target, identifier, directory, environment, timeout, slow):
    directory.mkdir()
    environment = dict(environment, CRITERION_HOME=str(directory / "criterion"))
    command = [str(binary), "--bench", "^" + re.escape(identifier) + "$", "--noplot"]
    if not slow:
        command += FAST
    elapsed = run(command, cwd=directory, environment=environment,
                  log=directory / "run.log", timeout=timeout)
    samples = list((directory / "criterion").glob("**/new/sample.json"))
    if len(samples) != 1:
        raise RuntimeError(f"expected one measured case, got {len(samples)}: {directory}")
    sample = json.loads(samples[0].read_text(encoding="utf-8"))
    totals, iterations = sample["times"], sample["iters"]
    if not totals or len(totals) != len(iterations) or any(
        not math.isfinite(total) or total <= 0 or not math.isfinite(count) or count <= 0
        for total, count in zip(totals, iterations)
    ):
        raise RuntimeError(f"invalid Criterion samples: {samples[0]}")
    operations = int(identifier.rsplit("_", 1)[1]) if target == "atlas_upload" else 1
    counts = [total / count / operations for total, count in zip(totals, iterations)]
    log_text = (directory / "run.log").read_text(encoding="utf-8", errors="replace")
    wall = re.search(
        r"wall time per iteration: median ([\d.]+) (\S+) \(min ([\d.]+), max ([\d.]+)", log_text,
    )
    if environment["BENCH_MEASUREMENT"] == "wall-time":
        wall_seconds = statistics.median(counts) * 1e-9
    elif wall:
        units = {"ns": 1e-9, "µs": 1e-6, "μs": 1e-6, "us": 1e-6, "ms": 1e-3, "s": 1}
        wall_seconds = float(wall[1]) * units[wall[2]]
    else:
        raise RuntimeError(f"missing wall-time secondary: {directory / 'run.log'}")
    return {
        "primary": statistics.median(counts),
        "sample_min": min(counts), "sample_max": max(counts),
        "wall_seconds": wall_seconds, "elapsed_seconds": elapsed,
        "iterations": sum(iterations), "operations_per_iteration": operations,
        "samples": str(samples[0]), "log": str(directory / "run.log"),
    }


def spread(values):
    return (max(values) - min(values)) / statistics.median(values) * 100


def write_summary(output, summary):
    temporary = output / "summary.json.tmp"
    temporary.write_text(json.dumps(summary, indent=2, default=str, allow_nan=False) + "\n", encoding="utf-8")
    temporary.replace(output / "summary.json")


def compare(root, cache, args):
    started = time.monotonic()
    run_id = time.strftime("%Y%m%d-%H%M%S") + "-" + uuid.uuid4().hex[:8]
    output = cache / "runs" / run_id
    output.mkdir(parents=True)
    fixtures = {relative: (root / relative).read_bytes() for relative in FIXTURE_FILES}
    for relative, content in fixtures.items():
        destination = output / "fixtures" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
    environment = dict(os.environ, BENCH_MEASUREMENT=args.measurement,
                       SEED=str(args.seed), ITERATIONS="1")
    environment.setdefault("CARGO_BUILD_JOBS", "2")
    environment["PATH"] = str(Path.home() / ".cargo/bin") + os.pathsep + environment.get("PATH", "")
    revisions, cases, results = [], {}, {}
    summary = {
        "configuration": vars(args) | {"cache": str(cache)}, "revisions": revisions,
        "results": results, "cases": cases, "status": "building",
        "primary_unit": "nanoseconds" if args.measurement == "wall-time" else "instructions",
        "normalization": "per workload iteration; atlas_upload is per atlas lookup",
        "environment": {name: value for name, value in environment.items()
                        if name in ("BENCH_MEASUREMENT", "SEED", "ITERATIONS", "RUSTFLAGS",
                                    "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_JOBS")},
    }
    write_summary(output, summary)
    print(f"Artifacts: {output}", flush=True)
    try:
        for role, revision in zip(("baseline", "candidate"), (args.baseline, args.candidate)):
            print(f"Building {role}: {revision} (once)...", flush=True)
            revisions.append(prepare(root, cache, output, run_id, role, revision, args, environment, fixtures))
            write_summary(output, summary)
        verify_distinct_artifacts(revisions, args.targets)
        if revisions[0]["lockfile_sha256"] != revisions[1]["lockfile_sha256"]:
            print("NOTE: Cargo.lock differs; dependency changes may affect the comparison.", flush=True)
        summary["status"] = "discovering"
        for target in args.targets:
            listings = []
            pattern = args.filter if args.filter is not None else "" if args.full else CORE[target]
            for index, revision in enumerate(revisions):
                log = output / f"list-{index}-{target}.log"
                run([str(revision["binaries"][target]), "--list", pattern],
                    cwd=output, environment=environment, log=log, timeout=args.timeout)
                listings.append([
                    line.removesuffix(": benchmark")
                    for line in log.read_text(encoding="utf-8", errors="replace").splitlines()
                    if line.endswith(": benchmark")
                ])
            if listings[0] != listings[1]:
                raise RuntimeError(f"benchmark IDs differ for {target}; see list logs")
            cases[target] = listings[0]
        if not any(cases.values()):
            raise RuntimeError("filter selected no benchmarks")
        summary["status"] = "smoke"
        write_summary(output, summary)
        for index, revision in enumerate(revisions):
            for target, identifiers in cases.items():
                if not identifiers:
                    continue
                binary = revision["binaries"][target]
                pattern = "^(" + "|".join(re.escape(value) for value in identifiers) + ")$"
                for mode, selection in [("--test", pattern),
                                        ("--quick", "^" + re.escape(identifiers[0]) + "$")]:
                    run([str(binary), "--bench", selection, mode, "--noplot",
                         "--measurement-time", "0.3"], cwd=output, environment=environment,
                        log=output / f"smoke-{index}-{target}{mode}.log", timeout=args.timeout)
        summary["status"] = "measuring"
        print(f"Measuring sequentially: {args.measurement} per workload iteration "
              "(atlas_upload: per lookup).", flush=True)
        print(f"{'benchmark':76} {'base':>12} {'candidate':>12} {'change':>9} "
              f"{'spread B/N':>14} {'wall ms B/N':>17}", flush=True)
        for target, identifiers in cases.items():
            for identifier in identifiers:
                key = f"{target}/{identifier}"
                case_number = len(results)
                pair = [[], []]
                results[key] = {"target": target, "identifier": identifier, "runs": pair}
                for repetition in range(args.repetitions):
                    for index in ([0, 1] if repetition % 2 == 0 else [1, 0]):
                        revision = revisions[index]
                        directory = output / f"{case_number:03}-{repetition}-{index}"
                        pair[index].append(measure(revision["binaries"][target], target, identifier, directory,
                                                   environment, args.timeout, args.slow))
                        write_summary(output, summary)
                primary_runs = [[value["primary"] for value in side] for side in pair]
                baseline, candidate = map(statistics.median, primary_runs)
                walls = [statistics.median(value["wall_seconds"] for value in side) * 1000 for side in pair]
                noise = (f"{spread(primary_runs[0]):5.2f}/{spread(primary_runs[1]):5.2f}%"
                         if args.repetitions > 1 else "           n/a")
                change = (candidate / baseline - 1) * 100
                results[key]["change_percent"] = change
                print(f"{identifier:76} {baseline:12.3f} {candidate:12.3f} {change:+8.2f}% "
                      f"{noise} {walls[0]:7.3f}/{walls[1]:7.3f}", flush=True)
                if args.repetitions > 1 and any(spread(values) > 1 for values in primary_runs):
                    print("  NOTE: >1% run spread; confirm with --slow before judging a change.", flush=True)
                ranges = [max((value["sample_max"] - value["sample_min"]) / value["primary"] * 100
                              for value in side) for side in pair]
                if any(value > 1 for value in ranges):
                    print(f"  NOTE: within-run range B/N {ranges[0]:.2f}/{ranges[1]:.2f}%; "
                          "inspect raw samples for drift/outliers.", flush=True)
                write_summary(output, summary)
        summary["status"] = "complete"
    except BaseException as error:
        summary["status"] = "failed"
        summary["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        summary["elapsed_seconds"] = time.monotonic() - started
        write_summary(output, summary)
    for index, revision in enumerate(revisions):
        seconds = sum(value["elapsed_seconds"] for result in results.values()
                      for value in result["runs"][index])
        print(f"{revision['commit'][:12]}: build {revision['build_seconds']:.2f}s, "
              f"measured processes {seconds:.2f}s ({args.repetitions} runs/case)")
    print(f"Total including builds, discovery and smoke: {summary['elapsed_seconds']:.2f}s")
    print(f"Worktrees and branches retained. Raw samples and summary: {output}")


def main():
    if sys.version_info < (3, 11):
        raise RuntimeError("compare_benchmarks.py requires Python 3.11 or newer")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--filter", help="Criterion regex applied to selected binaries")
    selection.add_argument("--full", action="store_true")
    parser.add_argument("--slow", action="store_true")
    parser.add_argument("--targets", nargs="+", choices=TARGETS, default=list(DEFAULT_TARGETS))
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--features", default={
        "linux": "bench-support,x11", "freebsd": "bench-support,x11",
        "darwin": "bench-support,font-kit",
    }.get(sys.platform, "bench-support"))
    parser.add_argument("--profile", default="bench")
    parser.add_argument("--measurement", choices=("foreground-instructions", "instructions", "wall-time"),
                        default="foreground-instructions")
    parser.add_argument("--timeout", type=int, default=300, help="timeout per build/run, 1..300 seconds")
    parser.add_argument("--cache", type=Path)
    args = parser.parse_args()
    if not 1 <= args.timeout <= 300 or args.repetitions < 1 or not 0 <= args.seed < 2**64:
        parser.error("timeout must be 1..300, repetitions positive and seed a u64")
    features = args.features.split(",")
    if "bench-support" not in features or any(
        feature.rsplit("/", 1)[-1] in ("test-support", "leak-detection") for feature in features
    ):
        parser.error("features must include bench-support and exclude test-support/leak-detection")
    if "GPUI_LEAK_DETECTION" in os.environ:
        parser.error("unset GPUI_LEAK_DETECTION before measuring")
    if len(set(args.targets)) != len(args.targets):
        parser.error("targets must be unique")
    root = Path(git(Path(__file__).resolve().parent, "rev-parse", "--show-toplevel"))
    cache = (args.cache or root / "target/gpui-bench-compare").resolve()
    cache.mkdir(parents=True, exist_ok=True)
    with comparison_lock(cache / "compare.lock"):
        compare(root, cache, args)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, subprocess.SubprocessError, ValueError) as error:
        sys.exit(str(error))
