"""Build and package an Open GPUI example for the iOS simulator."""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--target",
        choices=("aarch64-apple-ios-sim", "x86_64-apple-ios"),
        default=(
            "aarch64-apple-ios-sim"
            if platform.machine() == "arm64"
            else "x86_64-apple-ios"
        ),
    )
    parser.add_argument("--example", default="ios")
    options, cargo_arguments = parser.parse_known_args()
    if cargo_arguments[:1] == ["--"]:
        cargo_arguments = cargo_arguments[1:]
    if sys.platform != "darwin":
        parser.error("Packaging an iOS simulator application requires macOS and Xcode")

    repository = Path(__file__).resolve().parents[4]
    target = Path(os.environ.get("CARGO_TARGET_DIR", repository / "target"))
    if not target.is_absolute():
        target = repository / target
    application = target / "gpui-ios-example" / "GPUIIosExample.app"
    environment = os.environ.copy()
    environment.setdefault("IPHONEOS_DEPLOYMENT_TARGET", "15.0")
    command = [
        "cargo", "build", "-p", "open-gpui-ios",
        "--example", options.example, "--target", options.target,
        "--message-format=json-render-diagnostics", *cargo_arguments,
    ]
    executable = None
    with subprocess.Popen(
        command, cwd=repository, env=environment, stdout=subprocess.PIPE, text=True
    ) as process:
        for line in process.stdout:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                print(line, end="")
                continue
            if event.get("reason") == "compiler-message":
                diagnostic = event.get("message", {}).get("rendered")
                if diagnostic:
                    print(diagnostic, end="", file=sys.stderr)
            if (
                event.get("reason") == "compiler-artifact"
                and event.get("target", {}).get("name") == options.example
                and "example" in event.get("target", {}).get("kind", [])
                and event.get("executable")
            ):
                if executable is not None:
                    raise RuntimeError("Expected one example executable in Cargo output")
                executable = Path(event["executable"])
        if process.wait() != 0:
            return process.returncode
    if executable is None:
        raise RuntimeError("Cargo did not report the example executable")

    application.mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, application / "gpui_ios_example")
    shutil.copy2(Path(__file__).parent / "app" / "Info.plist", application / "Info.plist")
    subprocess.run(["codesign", "--force", "--sign", "-", str(application)], check=True)
    print(f"Built simulator app: {application}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
