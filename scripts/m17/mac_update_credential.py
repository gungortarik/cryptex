"""Installed candidate must retain a one-device grant across two distinct daemons."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

from build_owner_mac_package import fingerprint
from prepare_mac_inventory import prepare
from update_fixture import ROOT, image, receipt


def exercise(root, desktop, updater, delivery, revision, pin, scope, profile):
    from mac_product_key_startup import profile_snapshot
    from mac_transport_check import _metadata

    active = desktop.parents[2]
    pending = active.with_suffix(".app.pending")
    candidate = pending / "Contents/MacOS/cryptexd"
    original = desktop.with_name("cryptexd")
    helper = Path(os.environ["CX_UPDATE_CI_CMS_HELPER"])
    # Preserve the hosted account's Keychain and WebKit preferences through
    # the updater, its probes and the Desktop that it reopens.
    if active != Path.home() / "Applications/Cryptex.app":
        raise RuntimeError("Expected the disposable account's installed Desktop")
    initial = profile_snapshot(profile)
    staging = root / "credential-delivery"
    staging.mkdir()
    package = staging / "Cryptex.app"
    subprocess.run(["/usr/bin/ditto", active, package], check=True, timeout=90)
    # Keep the same restricted daemon identifier, public signer and hardened
    # runtime. The additional hard flag produces a distinct genuine CDHash.
    subprocess.run(["/usr/bin/codesign", "--force", "--timestamp=none", "--options", "runtime,hard",
                    "--keychain", os.environ["CX_NATIVE_TEST_KEYCHAIN"], "--sign",
                    os.environ["CX_NATIVE_TEST_SIGNER"], "--identifier", "com.cryptex.daemon",
                    package / "Contents/MacOS/cryptexd"], check=True, timeout=60)
    # Distinguish the two signed frontend resource envelopes as well as the daemons.
    with (package / "Contents/Resources/notices/frontend.json").open("ab") as out:
        out.write(b"\n")
    image(package, "com.cryptex.desktop")
    subprocess.run(["/usr/bin/ditto", package, pending], check=True, timeout=90)
    digest = hashlib.sha256((package / "Contents/MacOS/cryptex-desktop").read_bytes()).hexdigest()
    prepare(package, revision, pin, digest, helper, active.parent)
    statement = staging / "candidate.cms"
    receipt(updater, package, statement)
    for path, cms in ((active, delivery / "update.cms"), (pending, statement)):
        subprocess.run([updater, "admit", path, cms], check=True, timeout=90)
    working_sum, staged_sum = fingerprint(updater, active), fingerprint(updater, pending)
    if staged_sum == working_sum:
        raise RuntimeError("Missing distinct candidate")
    probe = scope.root / "update-item-metadata"
    scope.run(["/usr/bin/xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
               "-framework", "Foundation", "-framework", "Security",
               ROOT / "scripts/m8/mac_keychain_item_probe.m", "-o", probe], "compile_update_probe", 90)
    account = "device-event-signing-seed:" + str(uuid.UUID(_metadata(profile / "identity/local_device.json")["device_id"]))
    from mac_ci_credential_keychain import require_item
    require_item(scope.run([probe, account, original, candidate, scope.path], "original_only"), False)

    def apply(outcome, selected):
        report = json.loads(subprocess.check_output([updater, "apply", staged_sum],
                                                    timeout=260, text=True))
        if report["outcome"] != outcome or fingerprint(updater, active) != selected:
            raise RuntimeError("Installed credential update selected the wrong package or outcome")
        window = subprocess.run(["/usr/bin/swift", ROOT / "scripts/m17/mac_reopened_window.swift", desktop,
                                 str(report["reopened_pid"])], timeout=90)
        # Keep the exact reopened-PID result and independently check the note
        # in the next session. Only fixed diagnostic stages may reach CI.
        scope.fence.checked([sys.executable, "-B", Path(__file__).with_name("mac_sealed_profile.py"), desktop],
                            "search_note_after_credential_update", 110)
        window.check_returncode()
        if initial != profile_snapshot(profile):
            raise RuntimeError("Credential update changed the signed note or public identity")

    apply("rolled_back_keychain_check", working_sum)
    print("unavailable_existing_key_restored_working_image_and_note=passed", flush=True)
    scope.authorize(probe, account, original, candidate)
    apply("rolled_back_keychain_check", working_sum)
    print("missing_transport_grant_restored_working_image_and_note=passed", flush=True)
    transport = profile / "managed-transport" / ("0" * 31 + "1") / "profile.json"
    scope.authorize(probe, "api:" + _metadata(transport)["credential_id"], original, candidate)
    apply("updated", staged_sum)
    print("granted_existing_key_unattended_reopen_same_signed_note=passed", flush=True)
