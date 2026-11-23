"""Packaged interactive rehearsal. Create only disposable runtime profiles."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import sys
import tempfile
import uuid

from suspend_clock import observe_sleep
from sync_stress import ROUNDS, exercise


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    package = Path(__file__).resolve().parents[2]
    manifest = json.loads((package / "build.json").read_bytes())
    system = platform.system()
    if manifest["system"] != system or not re.fullmatch("[0-9a-f]{40}", manifest["revision"]):
        raise RuntimeError("package_platform_or_revision")
    suffix = ".exe" if system == "Windows" else ""
    runner = package / "bin" / ("cryptexd-test" + suffix)
    if hashlib.sha256(runner.read_bytes()).hexdigest() != manifest["runner_sha256"]:
        raise RuntimeError("package_runner_digest")
    native = package / "bin" / ("syncthing" + suffix)
    with tempfile.TemporaryDirectory(prefix="cryptex-m16-soak-") as tmp:
        seconds = exercise(Path(tmp), native, runner, None if args.smoke else observe_sleep)
    if args.smoke:
        print("m16_package_smoke=passed sleep=unmeasured", flush=True)
        return
    if seconds is None or seconds < 5:
        raise RuntimeError("sleep_not_observed")
    receipt = dict(schema=1, revision=manifest["revision"], runner_sha256=manifest["runner_sha256"],
                   platform=system, architecture=platform.machine(), scope="two-local-disposable-peers",
                   suspended_seconds_lower_bound=seconds, rounds=ROUNDS, verified_notes_per_replica=2 + 8 * ROUNDS,
                   retained_instance_resume=True, cold_read=True)
    path = package / f"M16-UYKU-SONUC-{system}-{uuid.uuid4().hex[:10]}.json"
    with path.open("x", encoding="utf-8") as f:
        json.dump(receipt, f, indent=2)
    print(f"Sonuc dosyasi: {path}", flush=True)


if __name__ == "__main__":
    main()
