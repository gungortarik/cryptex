"""Verify live peers from a sealed installed Mac bundle in a disposable hosted account."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

from update_fixture import ROOT, image, signer

sys.path.insert(0, str(ROOT / "scripts/m8"))
from mac_lan_setup import inventory


def run(*args, **kwargs):
    return subprocess.run(list(args), check=True, **kwargs)


def inspect_image(args, _stage, timeout):
    return run(*args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=timeout).stdout


def exercise(root, release):
    config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.bundle.conf.json").read_text())
    config["bundle"]["resources"]["../dist"] = "frontend"
    overlay = root / "bundle.json"
    overlay.write_text(json.dumps(config), encoding="utf-8")
    run("node", "--input-type=module", "-e",
        "import {prepareSidecar} from './scripts/prepare-cryptexd-sidecar.mjs'; prepareSidecar(undefined,true)",
        cwd=ROOT / "apps/desktop", timeout=1200)
    run("pnpm", "--filter", "@cryptex/desktop", "exec", "tauri", "build", "--bundles", "app",
        "--features", "m17-sealed-startup-check", "--config", overlay, cwd=ROOT, timeout=1200)
    installed = root / "Applications/Cryptex.app"
    installed.parent.mkdir()
    run("/usr/bin/ditto", ROOT / "target/release/bundle/macos/Cryptex.app", installed)
    desktop = installed / "Contents/MacOS/cryptex-desktop"
    daemon = desktop.with_name("cryptexd")
    image(daemon, "com.cryptex.daemon")
    image(installed, "com.cryptex.desktop")
    run("/usr/bin/codesign", "--verify", "--strict", "--all-architectures", installed)
    pin = hashlib.sha256(Path(os.environ["CRYPTEX_M17_SIGNER_DER"]).read_bytes()).hexdigest()
    run(sys.executable, "-B", ROOT / "scripts/m17/verify_first_install_mac.py", installed,
        pin, hashlib.sha256(desktop.read_bytes()).hexdigest())
    def slot():
        return installed.parent / ("cryptex-release-" + hashlib.sha256(desktop.read_bytes()).hexdigest())

    def publish(release_id):
        metadata = slot()
        metadata.mkdir()
        manifest = metadata / "inventory.json"
        signature = metadata / "inventory.cms"
        contents = inventory(installed, platform.machine(), release_id, inspect_image, sealed=True)
        if "Contents/Resources/frontend/index.html" not in {item["path"] for item in contents["files"]}:
            raise RuntimeError("Installed release is missing its protected frontend")
        manifest.write_bytes(json.dumps(contents, separators=(",", ":")).encode())
        run(os.environ["CX_UPDATE_CI_CMS_HELPER"], pin, manifest, signature, timeout=60)

    def startup(case, refusal=None):
        runtime = root / case
        result = subprocess.run([desktop, "--cryptex-m8-local-startup-check", runtime],
                                capture_output=True, text=True, timeout=90)
        if refusal:
            if (result.returncode != 2 or refusal not in result.stderr.splitlines()
                    or "=passed" in result.stdout or (runtime.exists() and any(runtime.iterdir()))):
                raise RuntimeError(f"{case} did not produce the expected refusal: {result.returncode} {result.stderr[-1024:]}")
        else:
            result.check_returncode()
            for check in ("startup", "restart", "old_channel_refused", "cleanup"):
                if f"local_signed_{check}=passed" not in result.stdout.splitlines():
                    raise RuntimeError(f"Missing installed {check} evidence")

    publish(release)
    run("/usr/bin/codesign", "--verify", "--strict", installed)
    startup("connected")
    window = subprocess.run(["/usr/bin/swift", ROOT / "scripts/m8/mac_ui_quit_check.swift", desktop],
                            capture_output=True, text=True, timeout=120)
    if window.returncode or "sealed_window_verified_note_and_cleanup=passed" not in window.stdout.splitlines():
        raise RuntimeError(f"Ordinary window launch failed its receipt check: {window.returncode} {window.stderr[-1024:]}")
    refused = subprocess.run([desktop, "--cryptex-m8-local-content-existing-profile-ui", root / "unused"],
                             capture_output=True, text=True, timeout=30)
    if refused.returncode != 2 or "sealed_rehearsal_arguments" not in refused.stderr.splitlines() or (root / "unused").exists():
        raise RuntimeError("Sealed rehearsal accepted an existing-profile launch")
    original_slot = slot()
    previous = installed.with_suffix(".app.previous")
    installed.rename(previous)
    run("/usr/bin/ditto", previous, installed)
    notice = installed / "Contents/Resources/notices/frontend.json"
    with notice.open("ab") as out:
        out.write(b"\n")
    image(installed, "com.cryptex.desktop")
    run("/usr/bin/codesign", "--verify", "--strict", installed)
    candidate_slot = slot()
    if candidate_slot == original_slot:
        raise RuntimeError("Sealed resource change retained the previous release identity")
    # Both images have the same source revision, publisher and roles.
    candidate_slot.mkdir()
    for name in ("inventory.json", "inventory.cms"):
        shutil.copyfile(original_slot / name, candidate_slot / name)
    startup("stale-inventory", "local_signed_startup_Inventory")
    candidate_slot.rename(root / "stale-metadata")
    publish(release)
    startup("new-release")
    installed.rename(installed.with_suffix(".app.pending"))
    previous.rename(installed)
    if slot() != original_slot:
        raise RuntimeError("Rollback did not select the original inventory")
    startup("restored-release")
    image(daemon, "com.cryptex.unrelated")
    image(installed, "com.cryptex.desktop")
    run("/usr/bin/codesign", "--verify", "--strict", installed)
    publish(release)
    startup("replaced-role", "local_signed_startup_Image")
    print("Verified sealed installation, live startup/restart, stale channel refusal, protected UI,"
          " stale inventory refusal and replaced role refusal.")


if __name__ == "__main__":
    with signer(), tempfile.TemporaryDirectory() as work:
        revision = os.environ["GITHUB_SHA"]
        os.environ["CRYPTEX_M17_SIGNER_DER"] = os.environ["CRYPTEX_UPDATE_SIGNER_DER"]
        os.environ["CRYPTEX_M17_SOURCE_REVISION"] = revision
        exercise(Path(work).resolve(), "cryptex-m17-" + revision)
