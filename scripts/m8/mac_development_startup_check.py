"""Native development-entry acceptance inside the disposable CI signing scope.
No owner signing identity is provisioned and no public release is produced.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import uuid

from mac_identity_preflight import PreflightError, checked
from mac_transport_check import _metadata


NATIVE_ERRORS = {"local_signed_startup_" + code for code in (
    "Arguments", "Inventory", "Image", "Channel", "Rendezvous", "Instance",
    "Deadline", "Spawn", "Readiness", "Cleanup", "StaleAccepted")}
NATIVE_STAGES = {"content_bind", "owner_exchange", "child_inventory", "child_verification_hint",
                 "daemon_image", "desktop_inventory", "native_accept", "ready_file", "runtime_ready",
                 "startup_ack", "startup_hello", "verification_hint"}
PROFILE_STAGES = {"key_lock", "key_store", "validate", "backend", "prepare", "context", "unix_data", "recover", "namespace",
                  "recover_evidence", "initialize_slots", "rebuild", "rebuild_key", "rebuild_db", "rebuild_scan",
                  "slot_key", "slot_scan", "slot_locks"}
DEVELOPMENT_STAGES = {"start", "content", "bind", "ping", "profile_status", "unix_health", "close"}
IO_KINDS = {"NotFound", "PermissionDenied", "ConnectionRefused", "ConnectionReset", "BrokenPipe",
            "AlreadyExists", "WouldBlock", "InvalidInput", "InvalidData", "TimedOut", "Interrupted",
            "UnexpectedEof", "OutOfMemory", "Unsupported", "Other"}


def canonical_hashes(root):
    records = root / "canonical/events"
    if not records.is_dir() or records.is_symlink():
        raise PreflightError("development_canonical_baseline_missing")
    result = {}
    for directory, dirs, files in os.walk(records, followlinks=False):
        for name in dirs:
            if (Path(directory) / name).is_symlink():
                raise PreflightError("development_canonical_object_unsafe")
        for name in files:
            path = Path(directory) / name
            metadata = path.lstat()
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise PreflightError("development_canonical_object_unsafe")
            if name not in ("writer.lock", "writer_tip.json"):
                result[str(path.relative_to(root))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


class FencedRun:
    """Only a returned bounded result proves our captured process group was reaped."""
    def __init__(self, run_bounded):
        self.run_bounded = run_bounded
        self.stopped = True

    def outcome(self, args, timeout):
        self.stopped = False
        result = self.run_bounded([str(arg) for arg in args], timeout)
        self.stopped = True
        return result

    def checked(self, args, label, timeout):
        outcome = self.outcome(args, timeout)
        try:
            return checked(lambda *_: outcome, args, label, timeout)
        except PreflightError:
            # Preserve only fixed native diagnostics, never arbitrary process
            # output (which can include paths, notes or identity material).
            print(json.dumps({"developmentStage": label, "exitCode": outcome.code,
                              "timedOut": outcome.timed_out, "outputLimited": outcome.limited,
                              "pipeFailed": outcome.pipe_failed}), flush=True)
            for line in outcome.output.splitlines():
                if len(line) > 512:
                    continue
                if line in NATIVE_ERRORS or line in {"existing_profile_missing", "development_arguments_refused"}:
                    print(line, flush=True)
                elif line.startswith("{"):
                    try:
                        value = json.loads(line)
                    except (ValueError, RecursionError):
                        continue
                    stage = value.get("developmentStage")
                    if isinstance(stage, str) and stage in DEVELOPMENT_STAGES:
                        print(json.dumps({"developmentStage": stage}), flush=True)
                    stage = value.get("m8LocalStage")
                    error = value.get("error")
                    if (isinstance(stage, str) and stage in NATIVE_STAGES
                            and isinstance(error, str) and error in NATIVE_ERRORS):
                        print(json.dumps({"m8LocalStage": stage, "error": error}), flush=True)
                    stage = value.get("m8ProfileStage")
                    kind = value.get("kind")
                    phase = value.get("phase")
                    if (isinstance(stage, str) and stage in PROFILE_STAGES
                            and isinstance(phase, str) and phase in {"begin", "complete", "failed"}):
                        print(json.dumps({"m8ProfileStage": stage, "phase": phase}), flush=True)
                    if (isinstance(stage, str) and stage in PROFILE_STAGES
                            and isinstance(kind, str) and kind in IO_KINDS):
                        print(json.dumps({"m8ProfileStage": stage, "kind": kind}), flush=True)
            raise


def exercise_profile(root, desktop, fixture_daemon, manifest, run_bounded, run):
    profile = root / "development-pilot-runtime"
    profile.mkdir(mode=0o700)  # Exclusive fresh fixture; never adopt an owner profile.
    (profile / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    fence = FencedRun(run_bounded)
    completed = False
    try:
        fence.checked([fixture_daemon, "pilot-fixture", "signed-transfer-v1", "--runtime-root", profile,
                       "--confirm-isolated-pilot"], "prepare_development_existing_profile", 45)
        before = canonical_hashes(profile)
        if not before:
            raise PreflightError("development_canonical_baseline_missing")
        for attempt in range(2):
            output = fence.checked([desktop, "--cryptex-development-check", profile],
                                   "development_normal_startup_restart", 60)
            if "development_existing_profile_startup_and_cleanup=passed" not in output.splitlines():
                raise PreflightError("development_startup_marker_missing")
            if canonical_hashes(profile) != before:
                raise PreflightError("development_restart_changed_canonical")
        original = manifest.read_bytes()
        try:
            manifest.write_bytes(original + b" ")
            require_refusal(fence.outcome([desktop, "--cryptex-development-check", profile], 30),
                            "local_signed_startup_Inventory")
        finally:
            manifest.write_bytes(original)
        if canonical_hashes(profile) != before:
            raise PreflightError("development_refusal_changed_canonical")
        completed = True
    finally:
        # Even a nonzero exit, timeout or missing marker gets exact-key cleanup
        # once the bounded runner has reaped its captured process group. A raw
        # runner exception has no such guarantee: preserve evidence and refuse.
        if not fence.stopped:
            raise PreflightError("development_cleanup_writer_unresolved")
        metadata = profile / "identity/local_device.json"
        if metadata.exists():
            device = str(uuid.UUID(_metadata(metadata)["device_id"]))
            run(["/usr/bin/security", "delete-generic-password", "-s", "Cryptex-Dev-DeviceCredentials",
                 "-a", "device-event-signing-seed:" + device], "cleanup_development_device_key", 30)
        elif completed:
            raise PreflightError("development_cleanup_identity_missing")
        if completed:
            shutil.rmtree(profile)


def require_refusal(outcome, code):
    if (outcome.code != 2 or outcome.timed_out or outcome.limited or outcome.pipe_failed
            or code not in outcome.output.splitlines()):
        raise PreflightError("development_refusal_not_verified")


def require_same_designated_identity(creator, daemon, run):
    requirements = []
    for binary in (creator, daemon):
        text = run(["/usr/bin/codesign", "-d", "-r-", binary], "inspect_development_designated_identity", 30)
        lines = [line.removeprefix("designated => ") for line in text.splitlines()
                 if line.startswith("designated => ")]
        if len(lines) != 1 or not lines[0] or len(lines[0]) > 4096:
            raise PreflightError("development_designated_identity_unavailable")
        requirements.append(lines[0])
    if requirements[0] != requirements[1]:
        raise PreflightError("development_designated_identity_mismatch")
    # Equality is not a signature check. Verify both actual images against the
    # shared requirement before creating the fresh test-only credential.
    for binary in (creator, daemon):
        run(["/usr/bin/codesign", "--verify", "--strict", "-R", "=" + requirements[0], binary],
            "verify_development_designated_identity", 30)
    print("development_creator_designated_identity=matched", flush=True)


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

    # Keychain tracks its creator's designated code requirement. Provision
    # this fresh test profile with the development daemon identity, not the
    # unrelated fixture identity. The creator stays outside the shipped bundle;
    # no ACL change, key export or fixture command in the product is enabled.
    creator = root / "development-profile-creator"
    shutil.copy2(fixture_daemon, creator)
    sign(creator, "com.cryptex.development.daemon")
    require_same_designated_identity(creator, daemon, run)
    exercise_profile(root, desktop, creator, bundle / "Contents/Resources/m8-startup.json",
                     run_bounded, run)
    print("development_native_existing_profile_restart_and_refusals=passed")
