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


def enrollment_daemon(work, pin):
    from mac_identity_preflight import checked, run_bounded
    from mac_lan_setup import compile_products, inventory as app_inventory, tool
    from platform import machine

    tree = Path(os.environ["RUNNER_TEMP"]) / "m17-previous/source"
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=tree, text=True).strip() != REVISION:
        raise RuntimeError("Unexpected enrollment service source")
    tool_path = os.pathsep.join(dict.fromkeys([*(str(tool(name).parent) for name in ("node", "cargo", "pnpm")),
                                              "/usr/bin", "/bin", "/usr/sbin", "/sbin"]))

    def run(args, label, timeout=60):
        outcome = run_bounded(args, timeout, tool_path=tool_path)
        return checked(lambda *_: outcome, args, label, timeout)

    _, release = compile_products(tree, work, REVISION, Path(os.environ["CRYPTEX_M17_SIGNER_DER"]), run, {})
    app = work / "Cryptex-LAN.app"
    binaries, resources = app / "Contents/MacOS", app / "Contents/Resources"
    binaries.mkdir(parents=True, mode=0o700)
    resources.mkdir(mode=0o700)
    shutil.copytree(tree / "apps/desktop/dist", resources / "frontend")
    previous_package(resources / "transport")
    for name, role in (("cryptex-desktop", "desktop"), ("cryptexd", "daemon")):
        shutil.copy2(work / "target/debug" / name, binaries / name)
        image(binaries / name, "com.cryptex.development." + role)
    manifest = resources / "m8-startup.json"
    manifest.write_text(json.dumps(app_inventory(app, machine(), release, run), separators=(",", ":")),
                        encoding="utf-8")
    run([os.environ["CX_UPDATE_CI_CMS_HELPER"], pin, manifest, resources / "m8-startup.cms"],
        "sign_historical_enrollment_inventory")
    return binaries / "cryptexd"


def seed_phase(scope, tool, profile, name):
    test = "managed_profile::exchange_install::tests::installed_previous_pair::" + name
    seed = scope.fence.outcome(["/usr/bin/env", "CRYPTEX_TEST_ENROLLED_ROOT=" + str(profile), tool,
                                test, "--ignored", "--exact"], 150)
    if seed.code:
        line = re.search(r"installed_previous_pair\.rs:(\d+):", seed.output)
        if line:
            print("historical_mac_enrollment_failed_at_fixture_line=" + line[1], flush=True)
    from mac_identity_preflight import checked
    result = checked(lambda *_: seed, [], name, 150)
    if f"test {test} ... ok" not in result:
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
        daemon = enrollment_daemon(Path(tmp), pin)
        profile.mkdir(mode=0o700, exist_ok=False)
        helper = profile / "managed-transport"
        (profile / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
        with CredentialKeychain(work, run_bounded) as scope:
            seed_phase(scope, tool, profile, "installed_previous_note")
            probe = scope.root / "item"
            scope.run(["/usr/bin/xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
                       "-framework", "Foundation", "-framework", "Security",
                       ROOT / "scripts/m8/mac_keychain_item_probe.m", "-o", probe], "compile_probe", 90)
            enrollment = helper / ("0" * 31 + "1")
            device = uuid.UUID(_metadata(profile / "identity/local_device.json")["device_id"])
            account = "device-event-signing-seed:" + str(device)
            scope.authorize(probe, account, tool, daemon)
            outcome = scope.run([daemon, "transport", "enroll-local-dev", "--runtime-root", profile,
                                 "--confirm-local-only"], "enroll_historical_signed_service", 180)
            if "transport_enrollment=local_only" not in outcome.splitlines():
                raise RuntimeError("Missing historical signed enrollment receipt")
            seed_phase(scope, tool, profile, "installed_previous_pair")
            scope.authorize(probe, account, tool, target, previous_grantee=daemon)
            scope.authorize(probe, "api:" + _metadata(enrollment / "profile.json")["credential_id"], daemon, target)
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
