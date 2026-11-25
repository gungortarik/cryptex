"""Upgrade a signed, paired 1.0.1 transport in the installed updater's disposable Mac profile."""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import uuid
import zipfile

from paired_release_handoff import REVISION, ROOT, build, checkout, inventory
from test_owner_transport import ARCHIVES
from update_fixture import image


def seed_tool(work):
    tree = checkout(work, "a", REVISION)
    tests = tree / "services/cryptexd/src/managed_profile/exchange_install/tests"
    shutil.copyfile(Path(__file__).with_name("installed_previous_pair.rs"), tests / "installed_previous_pair.rs")
    with tests.with_suffix(".rs").open("ab") as source:
        source.write(b"\nmod installed_previous_pair;\n")
    previous = os.environ.get("CRYPTEX_M8_LOCAL_STARTUP_CERT_DER")
    os.environ["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = os.environ["CRYPTEX_M17_SIGNER_DER"]
    try:
        tool = work / "seed-pair"
        build(tree, False, work / "target", tool)
    finally:
        if previous is None:
            del os.environ["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"]
        else:
            os.environ["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = previous
    image(tool, "com.cryptex.test-seed")
    return tool


def previous_package(folder):
    stem = "syncthing-macos-arm64-v2.1.5"
    url = f"https://github.com/syncthing/syncthing/releases/download/v2.1.5/{stem}.zip"
    with urllib.request.urlopen(url, timeout=60) as response:
        raw = response.read(64 * 1024 * 1024 + 1)
    if hashlib.sha256(raw).hexdigest() != ARCHIVES["macos-arm64"][0]:
        raise RuntimeError("Unexpected historical Mac package")
    folder.mkdir(mode=0o700)
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        for leaf in ("syncthing", "LICENSE.txt", "AUTHORS.txt"):
            file = folder / leaf
            file.write_bytes(archive.read(f"{stem}/{leaf}"))
            file.chmod(0o500 if leaf == "syncthing" else 0o400)


def install_previous_app(work, active, updater, pin):
    from build_owner_mac_package import fingerprint
    from prepare_mac_inventory import stage_delivery

    tree = Path(os.environ["RUNNER_TEMP"]) / "m17-previous/source"
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=tree, text=True).strip() != REVISION:
        raise RuntimeError("Unexpected enrollment service source")
    target_dir = tree.parent / "target"
    env = dict(os.environ, CRYPTEX_M17_SOURCE_REVISION=REVISION, GITHUB_SHA=REVISION,
               CARGO_TARGET_DIR=str(target_dir))
    subprocess.run(["pnpm", "install", "--frozen-lockfile"], cwd=tree, env=env, check=True, timeout=900)
    desktop_dir = tree / "apps/desktop"
    subprocess.run(["node", "--input-type=module", "-e",
                    "import {prepareSidecar} from './scripts/prepare-cryptexd-sidecar.mjs';"
                    "prepareSidecar(undefined,'profile-trial')"], cwd=desktop_dir, env=env, check=True, timeout=1200)
    config = json.loads((desktop_dir / "src-tauri/tauri.bundle.conf.json").read_bytes())
    config["bundle"]["resources"]["../dist"] = "frontend"
    overlay = work / "previous-bundle.json"
    overlay.write_text(json.dumps(config), encoding="utf-8")
    subprocess.run(["pnpm", "--filter", "@cryptex/desktop", "exec", "tauri", "build", "--bundles", "app",
                    "--features", "m17-sealed-profile-trial", "--config", overlay],
                   cwd=tree, env=env, check=True, timeout=1200)
    unsigned = target_dir / "release/bundle/macos/Cryptex.app"
    # The 1.0.1 test build takes its pinned transport as a separate input.
    previous_package(unsigned / "Contents/Resources/transport")
    delivery = work / "previous-delivery"
    subprocess.run([sys.executable, "-B", tree / "scripts/m17/build_owner_mac_package.py", unsigned,
                    fingerprint(updater, unsigned), REVISION, env["CRYPTEX_M17_SIGNER_DER"], pin, delivery,
                    "--updater", updater, "--mac-helper", env["CX_UPDATE_CI_CMS_HELPER"],
                    "--keychain", env["CX_NATIVE_TEST_KEYCHAIN"]], cwd=tree, env=env, check=True, timeout=240)
    manifest = json.loads((delivery / "release.json").read_bytes())
    if (manifest["version"], manifest["source"], manifest["certificate_sha256"]) != ("1.0.1", REVISION, pin):
        raise RuntimeError("Historical app version, source or signer differs")
    active.rename(work / "completed-credential-trial.app")
    subprocess.run(["/usr/bin/ditto", delivery / "Cryptex.app", active], check=True, timeout=90)
    slot = delivery / ("cryptex-release-" + manifest["desktop_sha256"])
    stage_delivery(active, slot, pin, manifest["desktop_sha256"], active.parent)
    subprocess.run([updater, "admit", active, delivery / "update.cms"], check=True, timeout=90)
    return active / "Contents/MacOS/cryptexd"


def seed_phase(scope, tool, profile, name):
    test = "managed_profile::exchange_install::tests::installed_previous_pair::" + name
    seed = scope.fence.outcome(["/usr/bin/env", "CRYPTEX_TEST_ENROLLED_ROOT=" + str(profile), tool,
                                test, "--ignored", "--exact", "--nocapture"], 150)
    if seed.code or seed.timed_out:
        stages = re.findall(r"\bpair_fixture_stage=(profile|credential|journal|anchor|history|native)$",
                            seed.output, re.MULTILINE)
        if stages:
            print("historical_pair_last_stage=" + stages[-1], flush=True)
        status = re.search(r"\bpair_fixture_credential_status=(-?\d{1,10})$", seed.output, re.MULTILINE)
        if status:
            print("historical_pair_credential_status=" + status[1], flush=True)
        line = re.search(r"installed_previous_pair\.rs:(\d+):", seed.output)
        if line:
            print("historical_mac_enrollment_failed_at_fixture_line=" + line[1], flush=True)
    from mac_identity_preflight import checked
    result = checked(lambda *_: seed, [], name, 150)
    if test not in result or not re.search(r"^test result: ok\. 1 passed; 0 failed; 0 ignored;", result, re.MULTILINE):
        raise RuntimeError("Historical pairing fixture did not execute")


def exercise(work, desktop, updater, pin):
    from mac_ci_credential_keychain import CredentialKeychain, require_hosted_mac
    from mac_identity_preflight import run_bounded
    from mac_product_key_startup import profile_snapshot
    from mac_transport_check import _metadata
    require_hosted_mac()
    active = desktop.parents[2]
    pending = active.with_suffix(".app.pending")
    if active != Path.home() / "Applications/Cryptex.app":
        raise RuntimeError("Expected the disposable installed app")
    target = pending / "Contents/MacOS/cryptexd"
    approved = subprocess.check_output([updater, "fingerprint", pending], text=True, timeout=90).strip()
    profile = Path(tempfile.gettempdir()).resolve() / ("cryptex-m17-profile-" + pin)
    with tempfile.TemporaryDirectory(dir=work, prefix="old-enrollment-") as tmp:
        tool = seed_tool(Path(tmp))
        daemon = install_previous_app(Path(tmp), active, updater, pin)
        profile.mkdir(mode=0o700, exist_ok=False)
        helper = profile / "managed-transport"
        (profile / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
        with CredentialKeychain(work, run_bounded) as scope:
            scope.run([daemon, "pilot-fixture", "signed-transfer-v1", "--runtime-root", profile,
                       "--confirm-isolated-pilot"], "create_note_with_101_service", 45)
            probe = scope.root / "item"
            scope.run(["/usr/bin/xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
                       "-framework", "Foundation", "-framework", "Security",
                       ROOT / "scripts/m8/mac_keychain_item_probe.m", "-o", probe], "compile_probe", 90)
            enrollment = helper / ("0" * 31 + "1")
            device = uuid.UUID(_metadata(profile / "identity/local_device.json")["device_id"])
            account = "device-event-signing-seed:" + str(device)
            outcome = scope.run([daemon, "transport", "enroll-local-dev", "--runtime-root", profile,
                                 "--confirm-local-only"], "enroll_historical_signed_service", 180)
            if "transport_enrollment=local_only" not in outcome.splitlines():
                raise RuntimeError("Missing historical signed enrollment receipt")
            scope.authorize(probe, account, daemon, tool)
            seed_phase(scope, tool, profile, "installed_previous_pair")
            scope.authorize(probe, account, daemon, target, previous_grantee=tool)
            scope.authorize(probe, "api:" + _metadata(enrollment / "profile.json")["credential_id"], daemon, target)
            scope.fence.checked([sys.executable, "-B", Path(__file__).with_name("mac_sealed_profile.py"), desktop],
                                "search_in_historical_101_before_update", 110)
            notes = profile_snapshot(profile)
            original = inventory(enrollment / "native")
            old_files = inventory(helper / "package")
            report = json.loads(subprocess.check_output([updater, "apply", approved], text=True, timeout=420))
            if report["outcome"] != "updated":
                raise RuntimeError("Paired installed update selected " + report["outcome"])
            subprocess.run(["/usr/bin/swift", ROOT / "scripts/m17/mac_reopened_window.swift", desktop,
                            str(report["reopened_pid"])], check=True, timeout=90)
            scope.fence.checked([sys.executable, "-B", Path(__file__).with_name("mac_sealed_profile.py"), desktop],
                                "search_after_paired_transport_update", 110)
            if (profile_snapshot(profile) != notes or inventory(profile / "managed-transport-previous-native") != original
                    or inventory(profile / "managed-transport-previous-package") != old_files
                    or inventory(enrollment / "native").get(Path("exchange-install.json")) == original[Path("exchange-install.json")]
                    or (helper / "package/syncthing").read_bytes() != (active / "Contents/Resources/transport/syncthing").read_bytes()):
                raise RuntimeError("Updated transport, retained enrollment or canonical history mismatch")
        shutil.rmtree(profile)
    print("installed_previous_paired_transport_switched_retained_and_note_reopened=passed", flush=True)
