"""Apply the owner-signed update to a paired 1.0.1 profile in the hosted standard account."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

from paired_release_handoff import ROOT, inventory
from windows_previous_note import park_test_profile

sys.path.insert(0, str(ROOT / "scripts/m14"))
from installed_update import wait_for_desktop_exit


def reopened(root, pid):
    deadline = time.monotonic() + 60
    snapshot = root / "gui-proof.snapshot"
    while time.monotonic() < deadline:
        if snapshot.is_file():
            values = dict(line.split("=", 1) for line in snapshot.read_text().splitlines() if "=" in line)
            if (values.get("desktop_pid") == str(pid) and values.get("daemon_pid_observation") == "live"
                    and values.get("window_visible") == "true" and values.get("note") == "desktop_launched"):
                (root / "quit.request").touch(exist_ok=False)
                wait_for_desktop_exit(pid)
                return
        time.sleep(0.15)
    raise RuntimeError("Updated signed Desktop did not reopen with a live daemon")


def exercise(active):
    local = Path(os.environ["LOCALAPPDATA"])
    if (sys.platform != "win32" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or not re.fullmatch(r"cpxci[0-9a-f]{12}", os.environ["USERNAME"].lower())
            or active != local / "Programs/Cryptex"):
        raise RuntimeError("Expected the disposable standard user's installed Cryptex")
    scratch = Path(os.environ["RUNNER_TEMP"])
    fixture = scratch / "paired-update"
    previous = scratch / "exchange-package"
    tool = fixture / "cryptex-updater.exe"
    runtime = local / "Cryptex-Dev/runtime/p0-2"
    runtime.mkdir(exist_ok=False)
    (runtime / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    target = "managed_profile::exchange_install::tests::installed_previous_pair::installed_previous_pair"
    seed_env = dict(os.environ, CRYPTEX_TEST_ENROLLED_ROOT=str(runtime),
                    CRYPTEX_TEST_PREVIOUS_PACKAGE=str(previous))
    seed = subprocess.run([fixture / "seed-pair.exe", target, "--ignored", "--exact"],
                          env=seed_env, capture_output=True, text=True, timeout=180)
    if seed.returncode or f"test {target} ... ok" not in seed.stdout:
        line = re.search(r"installed_previous_pair\.rs:(\d+):", seed.stdout + seed.stderr)
        raise RuntimeError("Historical Windows enrollment failed at fixture line " + (line[1] if line else "unknown"))
    events = runtime / "canonical/events"
    identity = runtime / "identity"
    history = inventory(events), inventory(identity)
    helper = runtime / "managed-transport"
    native = helper / ("0" * 31 + "1") / "native"
    baseline = inventory(native)
    old_package = inventory(helper / "package")
    journal = Path("exchange-install.json")
    if journal not in baseline or not list(events.rglob("*.seg")):
        raise RuntimeError("Missing committed previous journal or signed note")
    pending = active.with_name("Cryptex.pending")
    active.rename(pending)
    shutil.copytree(fixture / "working", active)
    for directory, name in ((active, "working"), (pending, "candidate")):
        subprocess.run([tool, "admit", directory, fixture / (name + ".cms")], check=True, timeout=90)
    approved = subprocess.check_output([tool, "fingerprint", pending], text=True, timeout=90).strip()
    report = json.loads(subprocess.check_output([tool, "apply", approved], text=True, timeout=360))
    if report["outcome"] != "updated" or [c["status"] for c in report["checks"]] != ["connected", "connected"]:
        raise RuntimeError("Installed paired upgrade returned " + report["outcome"])
    reopened(runtime, report["reopened_pid"])
    installed = inventory(helper / "package")
    if (subprocess.check_output([tool, "fingerprint", active], text=True, timeout=90).strip() != approved
            or history != (inventory(events), inventory(identity))
            or inventory(runtime / "managed-transport-previous-native") != baseline
            or inventory(runtime / "managed-transport-previous-package") != old_package
            or inventory(native).get(journal) in (None, baseline[journal])
            or installed[Path("syncthing.exe")] != hashlib.sha256((active / "transport/syncthing.exe").read_bytes()).digest()):
        raise RuntimeError("The installed update did not retain its prior database, package and signed history")
    park_test_profile(runtime, runtime.with_name("verified-paired-upgrade"))
    print("installed_101_to_102_paired_transport_reopened_same_history_and_retained_database=passed", flush=True)


if __name__ == "__main__":
    exercise(Path(sys.argv[1]))
