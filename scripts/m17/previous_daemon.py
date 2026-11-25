"""Build the 1.0.1 daemon that creates the disposable pre-update history."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


REVISION = "a8e1d3fe1fb4c31b370fec0baac649e94a982688"


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted":
        raise RuntimeError("The historical build requires a disposable native runner")
    work = Path(os.environ["RUNNER_TEMP"]).resolve() / "m17-previous"
    work.mkdir()
    checkout = work / "source"
    subprocess.run(["git", "worktree", "add", "--detach", checkout, REVISION], check=True)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
    config = json.loads((checkout / "apps/desktop/src-tauri/tauri.conf.json").read_bytes())
    if commit != REVISION or config["version"] != "1.0.1":
        raise RuntimeError("Unexpected previous release source")
    target = work / "target"
    subprocess.run(["cargo", "build", "--release", "--locked", "-p", "cryptexd",
                    "--bin", "cryptexd", "--target-dir", target], cwd=checkout, check=True)
    executable = "cryptexd.exe" if sys.platform == "win32" else "cryptexd"
    (target / "release" / executable).rename(work / executable)
    if sys.platform == "win32":
        # The working Desktop must exercise the real 1.0.1 protocol and pins.
        pnpm = shutil.which("pnpm.cmd")
        subprocess.run([pnpm, "install", "--frozen-lockfile"], cwd=checkout, check=True)
        subprocess.run([pnpm, "--filter", "@cryptex/desktop", "build"], cwd=checkout, check=True)
        subprocess.run(["cargo", "build", "--release", "--locked", "-p", "cryptex-desktop",
                        "--features", "tauri/custom-protocol", "--target-dir", target],
                       cwd=checkout, check=True, timeout=1200)
        shutil.copy2(target / "release/cryptex-desktop.exe", work / "cryptex-desktop.exe")
        from paired_release_handoff import build
        tests = checkout / "services/cryptexd/src/managed_profile/exchange_install/tests"
        tests.mkdir(exist_ok=True)
        shutil.copyfile(Path(__file__).with_name("installed_previous_pair.rs"), tests / "installed_previous_pair.rs")
        with tests.with_suffix(".rs").open("ab") as module:
            module.write(b"\nmod installed_previous_pair;\n")
        build(checkout, False, target, work / "seed-pair.exe")


if __name__ == "__main__":
    main()
