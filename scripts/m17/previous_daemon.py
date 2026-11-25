"""Build the 1.0.1 daemon that creates the disposable pre-update history."""
import json
import os
from pathlib import Path
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


if __name__ == "__main__":
    main()
