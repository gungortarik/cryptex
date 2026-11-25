"""Upgrade a signed, paired 1.0.1 transport in the installed updater's disposable Mac profile."""
import hashlib
import io
import json
import os
from pathlib import Path
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
        profile.mkdir(mode=0o700, exist_ok=False)
        helper = profile / "managed-transport"
        helper.mkdir(mode=0o700)
        previous_package(helper / "package")
        for name, value in (("transport.marker", b"m8-native-transport-helper-v1\n"),
                            ("ports.json", b'{"api":18384,"sync":22001}')):
            (helper / name).write_bytes(value)
            (helper / name).chmod(0o600)
        (profile / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
        with CredentialKeychain(work, run_bounded) as scope:
            test = "managed_profile::exchange_install::tests::installed_previous_pair::installed_previous_pair"
            result = scope.run(["/usr/bin/env", "CRYPTEX_TEST_ENROLLED_ROOT=" + str(profile), tool,
                                test, "--ignored", "--exact"], "seed_installed_previous_pair", 150)
            if f"test {test} ... ok" not in result:
                raise RuntimeError("Historical paired enrollment did not execute")
            scope.run(["/usr/bin/xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
                       "-framework", "Foundation", "-framework", "Security",
                       ROOT / "scripts/m8/mac_keychain_item_probe.m", "-o", scope.root / "item"], "compile_probe", 90)
            enrollment = helper / ("0" * 31 + "1")
            device = uuid.UUID(_metadata(profile / "identity/local_device.json")["device_id"])
            for account in ["device-event-signing-seed:" + str(device),
                            "api:" + _metadata(enrollment / "profile.json")["credential_id"]]:
                scope.authorize(scope.root / "item", account, tool, target)
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
