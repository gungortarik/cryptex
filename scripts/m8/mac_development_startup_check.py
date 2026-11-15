"""Native development-entry acceptance inside the disposable CI signing scope.
No owner signing identity is provisioned and no public release is produced.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import uuid

from mac_identity_preflight import PreflightError


def canonical_hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in (root / "canonical/events").rglob("*.seg")}


def require_refusal(outcome, code):
    if (outcome.code != 2 or outcome.timed_out or outcome.limited or outcome.pipe_failed
            or code not in outcome.output.splitlines()):
        raise PreflightError("development_refusal_not_verified")


def check(root, cargo, clippy, run, run_bounded, sign, inventory, template, fixture_daemon):
    from mac_local_startup_ci import fixture_rust_command
    keys = ("CRYPTEX_M8_DEVELOPMENT_CERT_DER", "CRYPTEX_M8_DEVELOPMENT_RELEASE_ID", "CARGO")
    saved = {key: os.environ.get(key) for key in keys}
    release = "cryptex-dev-native-ci-v1"
    try:
        os.environ["CRYPTEX_M8_DEVELOPMENT_CERT_DER"] = str(root / "cert.der")
        os.environ["CRYPTEX_M8_DEVELOPMENT_RELEASE_ID"] = release
        os.environ["CARGO"] = cargo
        features = "cryptex-desktop/m8-development-product"
        run(fixture_rust_command(clippy, "clippy", "--locked", "-p", "cryptexd", "-p", "cryptex-desktop",
                                "--features", features, "--", "-D", "warnings"), "lint_development_entry", 900)
        run(fixture_rust_command(cargo, "build", "--locked", "-p", "cryptexd", "-p", "cryptex-desktop",
                                "--features", features), "build_development_entry", 900)
        run(fixture_rust_command(cargo, "test", "--locked", "-p", "cryptex-desktop", "--features",
                                "m8-development-product", "development_launch::tests::"), "test_development_entry_shape", 300)
        run(fixture_rust_command(cargo, "test", "--locked", "-p", "cryptexd", "--features",
                                "m8-development-product", "development_product_"), "test_development_fixture_refusals", 300)
    finally:
        for key, value in saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    bundle = root / "development.app"
    shutil.copytree(template, bundle)
    for name, role in (("cryptex-desktop", "desktop"), ("cryptexd", "daemon")):
        target = bundle / "Contents/MacOS" / name
        shutil.copy2(Path("target/debug") / name, target)
        sign(target, "com.cryptex.development." + role)
    inventory(bundle, release=release)
    desktop = bundle / "Contents/MacOS/cryptex-desktop"
    daemon = bundle / "Contents/MacOS/cryptexd"
    for binary, args, code in (
        (desktop, ["--cryptex-m8-local-content-ui", str(root / "must-not-create")], "development_arguments_refused"),
        (daemon, ["--m8-local-transport-driver", str(root / "must-not-create")], "mac_transport_Arguments"),
    ):
        require_refusal(run_bounded([str(binary), *args], 30), code)
    if (root / "must-not-create").exists():
        raise PreflightError("development_refusal_created_profile")
    missing = root / "development-missing-profile"
    require_refusal(run_bounded([str(desktop), "--cryptex-development-check", str(missing)], 30),
                    "existing_profile_missing")
    if missing.exists():
        raise PreflightError("development_missing_profile_created")

    profile = root / "development-pilot-runtime"
    profile.mkdir(mode=0o700)
    (profile / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    # Only the separate fixture binary creates test input. The development app
    # cannot enter this command or create a signing identity during startup.
    run([fixture_daemon, "pilot-fixture", "signed-transfer-v1", "--runtime-root", profile,
         "--confirm-isolated-pilot"], "prepare_development_existing_profile", 45)
    device = str(uuid.UUID(json.loads((profile / "identity/local_device.json").read_text())["device_id"]))
    before = canonical_hashes(profile)
    if not before:
        raise PreflightError("development_canonical_baseline_missing")
    # On an uncertain child exit, deliberately leave key/profile for diagnosis.
    # Success requires the native close acknowledgement and actual child reaping.
    for attempt in range(2):
        output = run([desktop, "--cryptex-development-check", profile],
                     "development_normal_startup_restart", 60)
        if "development_existing_profile_startup_and_cleanup=passed" not in output.splitlines():
            raise PreflightError("development_startup_marker_missing")
        if canonical_hashes(profile) != before:
            raise PreflightError("development_restart_changed_canonical")
    manifest = bundle / "Contents/Resources/m8-startup.json"
    original = manifest.read_bytes()
    try:
        manifest.write_bytes(original + b" ")
        outcome = run_bounded([str(desktop), "--cryptex-development-check", str(profile)], 30)
        require_refusal(outcome, "local_signed_startup_Inventory")
    finally:
        manifest.write_bytes(original)
    if canonical_hashes(profile) != before:
        raise PreflightError("development_refusal_changed_canonical")
    run(["/usr/bin/security", "delete-generic-password", "-s", "Cryptex-Dev-DeviceCredentials",
         "-a", "device-event-signing-seed:" + device], "cleanup_development_device_key", 30)
    shutil.rmtree(profile)
    print("development_native_existing_profile_restart_and_refusals=passed")
