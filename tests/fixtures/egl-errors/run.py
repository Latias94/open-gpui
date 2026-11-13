"""Run the actual EGL binding against mock native symbols using Cargo artifact messages."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry-control", action="store_true")
    parser.add_argument("--locked", action="store_true")
    parser.add_argument("--target-dir", type=Path)
    args = parser.parse_args()
    fixture = Path(__file__).resolve().parent
    root = fixture.parents[2]
    manifest = fixture / ("registry-control/Cargo.toml" if args.registry_control else "Cargo.toml")
    common = ["--manifest-path", str(manifest), "--target-dir", str(args.target_dir or root / "target")]
    if args.locked:
        common.append("--locked")
    build = subprocess.run(
        ["cargo", "build", *common, "--lib", "--message-format=json-render-diagnostics"],
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    libraries = [
        Path(filename)
        for line in build.stdout.splitlines()
        for message in [json.loads(line)]
        if message.get("reason") == "compiler-artifact"
        and message["target"]["name"] == "egl_error_mock"
        for filename in message["filenames"]
        if Path(filename).suffix in {".dylib", ".so", ".dll"}
    ]
    if len(libraries) != 1:
        raise RuntimeError(f"expected one native mock artifact, found {libraries}")
    with tempfile.TemporaryDirectory(prefix="open-gpui-egl-") as directory:
        library = Path(directory) / libraries[0].name
        shutil.copy2(libraries[0], library)
        environment = os.environ.copy()
        environment["OPEN_GPUI_EGL_MOCK"] = str(library)
        return subprocess.run(
            ["cargo", "nextest", "run", *common, "--test", "egl_errors", "--test-threads=1", "--no-fail-fast"],
            env=environment,
        ).returncode


if __name__ == "__main__":
    sys.exit(main())
