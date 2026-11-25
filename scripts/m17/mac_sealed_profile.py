"""Reopen a signed synthetic note in the installed macOS WebView after a normal quit."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid


def observe(desktop):
    # The outer bounded runner owns the entire process group, including AppKit.
    with subprocess.Popen([desktop], stdout=subprocess.PIPE, text=True) as app:
        for line in app.stdout:
            if line.strip() == "sealed_profile_search_visible=passed":
                subprocess.run(["/usr/bin/swift", Path(__file__).with_name("mac_reopened_window.swift"),
                                desktop, str(app.pid)], check=True, timeout=75)
                if app.wait(timeout=5) == 0:
                    return
                break
        raise RuntimeError("Installed window did not return the signed note and quit cleanly")


def check(work, desktop, pin, creator, upgrade=None):
    from mac_ci_credential_keychain import CredentialKeychain
    from mac_identity_preflight import run_bounded
    from mac_product_key_startup import profile_snapshot
    from mac_transport_check import _metadata

    profile = Path(tempfile.gettempdir()).resolve() / ("cryptex-m17-profile-" + pin)
    if os.path.lexists(profile):
        raise RuntimeError("Disposable profile slot is occupied")
    denied = subprocess.run([desktop], capture_output=True, text=True, timeout=45)
    if denied.returncode != 2 or "existing_profile_missing" not in denied.stderr.splitlines() or profile.exists():
        raise RuntimeError("Missing profile did not fail closed")
    profile.mkdir(mode=0o700)
    (profile / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    with CredentialKeychain(work, run_bounded) as scope:
        daemon = desktop.with_name("cryptexd")
        scope.run([creator, "pilot-fixture", "signed-transfer-v1", "--runtime-root", profile,
                   "--confirm-isolated-pilot"], "seed_disposable_signed_note", 45)
        baseline = profile_snapshot(profile)
        if not baseline["events"]:
            raise RuntimeError("Empty canonical baseline")
        descriptor = profile / "signed-probe.json"
        expected = hashlib.sha256(descriptor.read_bytes()).digest()
        if creator != daemon:
            # The hosted local certificates bind this test credential to an image.
            # Reuse the M8 disposable Keychain grant. It checks both partitions
            # before and after the operation and never reads the seed.
            probe = scope.root / "item-probe"
            source = Path(__file__).resolve().parents[1] / "m8/mac_keychain_item_probe.m"
            scope.run(["/usr/bin/xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
                       "-framework", "Foundation", "-framework", "Security", source, "-o", probe],
                      "compile_item_probe", 90)
            device = uuid.UUID(_metadata(profile / "identity/local_device.json")["device_id"])
            scope.authorize(probe, "device-event-signing-seed:" + str(device), creator, daemon)
            print("disposable_keychain_previous_image_only_then_exact_candidate_grant=passed", flush=True)
        scope.run([daemon, "transport", "enroll-local-dev", "--runtime-root", profile,
                   "--confirm-local-only"], "enroll_installed_transport", 160)
        installed = desktop.parent.parent / "Resources/transport/syncthing"
        managed = profile / "managed-transport/package/syncthing"
        if hashlib.sha256(managed.read_bytes()).digest() != hashlib.sha256(installed.read_bytes()).digest():
            raise RuntimeError("Enrolled transport differs from installed transport")
        for attempt in range(2):
            outcome = scope.fence.outcome([sys.executable, "-B", __file__, desktop], 100)
            if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
                raise RuntimeError(f"Installed search session {attempt + 1} failed "
                                   f"(exit={outcome.code}, timeout={outcome.timed_out})")
            if profile_snapshot(profile) != baseline or hashlib.sha256(descriptor.read_bytes()).digest() != expected:
                raise RuntimeError("Reopening changed the signed note or its identity")
        if upgrade is not None:
            upgrade(scope, profile)
        descriptor.rename(profile / "held-descriptor")
        denied = scope.fence.outcome([desktop], 40)
        if (denied.code != 2 or denied.timed_out or denied.limited or denied.pipe_failed
                or profile_snapshot(profile) != baseline):
            raise RuntimeError("Unverified synthetic profile reopened or its history changed")
    shutil.rmtree(profile)
    print("sealed_installed_search_same_note_after_quit_and_reopen=passed", flush=True)


if __name__ == "__main__":
    observe(Path(sys.argv[1]))
