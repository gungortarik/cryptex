"""Feature-only actual Desktop/daemon startup fixture; called inside CI keychain scope."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys

from mac_identity_preflight import PreflightError


def run_real_startup_checks(root, checked, run_bounded, cargo, openssl, keychain,
                            password, fingerprint):
    def run(args, stage, timeout=30):
        outcome = run_bounded([str(arg) for arg in args], timeout)
        if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
            diagnostic = outcome.output.replace(password, "[redacted]").replace(
                fingerprint, "[fixture]").replace(str(root), "[fixture-root]")
            print(diagnostic[:8192], file=sys.stderr)
        return checked(lambda *_: outcome, [], stage, timeout)

    # This build-time pin is independent of adjacent runtime metadata. It is
    # embedded before signing; neither binary embeds the other's final CDHash.
    saved = {key: os.environ.get(key) for key in
             ("CRYPTEX_M8_LOCAL_STARTUP_CERT_DER", "CARGO_PROFILE_DEV_DEBUG", "CARGO")}
    try:
        clippy = shutil.which("cargo-clippy")
        if not clippy:
            raise PreflightError("ci_clippy_missing")
        os.environ["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = str(root / "cert.der")
        os.environ["CARGO_PROFILE_DEV_DEBUG"] = "0"
        os.environ["CARGO"] = str(cargo)
        features = "cryptex-desktop/m8-local-startup-check"
        # run_bounded intentionally removes plugin discovery from PATH.
        # cargo-clippy expects the Cargo subcommand token, then invokes CARGO.
        run([clippy, "clippy", "--locked", "-p", "cryptexd", "-p", "cryptex-desktop",
             "--all-targets", "--features", features, "--", "-D", "warnings"],
            "lint_real_signed_startup", 900)
        run([cargo, "build", "--locked", "-p", "cryptexd", "-p", "cryptex-desktop",
             "--features", features], "build_real_signed_startup", 900)
        tests = run([cargo, "test", "--locked", "-p", "cryptexd", "--features",
             "m8-local-startup-check", "--lib", "m8_owner_channel::", "--", "--test-threads=1"],
            "test_signed_startup_shapes_and_closed_gates", 300)
        for name in ("local_startup::tests::startup_frames_bind_fixed_role_epoch_and_inventory",
                     "tests::candidate_has_no_enabled_or_configuration_fallback_path",
                     "image_policy::tests::product_policy_has_no_configured_publisher"):
            if "test m8_owner_channel::" + name + " ... ok" not in tests:
                raise PreflightError("signed_startup_gate_test_missing")
    finally:
        for key, value in saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    template = root / "template"
    binaries = template / "Contents" / "MacOS"
    resources = template / "Contents" / "Resources"
    binaries.mkdir(parents=True)
    resources.mkdir()
    for name, role in (("cryptex-desktop", "desktop"), ("cryptexd", "daemon")):
        source = Path(os.environ.get("CARGO_TARGET_DIR", "target")) / "debug" / name
        target = binaries / name
        shutil.copyfile(source, target)
        target.chmod(0o700)
        run(["/usr/bin/codesign", "--force", "--sign", fingerprint, "--timestamp=none",
             "--identifier", "com.cryptex.m8.local-startup." + role,
             "--keychain", keychain, target], "sign_real_" + role)
    (resources / "startup-check.txt").write_bytes(b"Public local startup fixture.\n")
    shutil.copytree(Path("apps/desktop/dist"), resources / "frontend")
    architecture = {"arm64": "arm64", "x86_64": "x86_64"}[platform.machine()]

    def inventory(bundle, *, role_hash=None, parent_hash=None, release="m8-local-startup-v1"):
        images, files = {}, []
        for name, role in (("cryptex-desktop", "desktop"), ("cryptexd", "daemon")):
            relative = "Contents/MacOS/" + name
            text = run(["/usr/bin/codesign", "-d", "--verbose=4", bundle / relative],
                       "inspect_real_" + role)
            hashes = [line[7:] for line in text.splitlines() if line.startswith("CDHash=")]
            if len(hashes) != 1 or len(hashes[0]) != 40:
                raise PreflightError("real_image_hash_missing")
            images[role] = {"path": relative, "cdhash": hashes[0]}
        if role_hash:
            images["daemon"]["cdhash"] = role_hash
        if parent_hash:
            images["desktop"]["cdhash"] = parent_hash
        listed = ["Contents/MacOS/cryptex-desktop", "Contents/MacOS/cryptexd",
                  "Contents/Resources/startup-check.txt"]
        listed += [path.relative_to(bundle).as_posix()
                   for path in sorted((bundle / "Contents/Resources/frontend").rglob("*")) if path.is_file()]
        for relative in listed:
            data = (bundle / relative).read_bytes()
            files.append({"path": relative, "size": len(data),
                          "sha256": hashlib.sha256(data).hexdigest()})
        manifest = bundle / "Contents/Resources/m8-startup.json"
        manifest.write_text(json.dumps({"schema": 1, "purpose": "cryptex-m8-release-inventory",
            "release_id": release, "architecture": architecture, **images, "files": files},
            separators=(",", ":")), encoding="utf-8")
        run([openssl, "cms", "-sign", "-binary", "-nodetach", "-in", manifest,
             "-signer", root / "cert.pem", "-inkey", root / "key.pem", "-outform", "DER",
             "-out", bundle / "Contents/Resources/m8-startup.cms"], "sign_real_inventory")

    inventory(template)
    positive = root / "positive.app"
    shutil.copytree(template, positive)
    result = run([positive / "Contents/MacOS/cryptex-desktop",
                  "--cryptex-m8-local-startup-check", root / "positive-runtime"],
                 "real_signed_startup_restart", 45)
    markers = ("local_signed_startup=passed", "local_signed_restart=passed",
               "local_signed_old_channel_refused=passed", "local_signed_cleanup=passed")
    if not all(marker in result.splitlines() for marker in markers):
        raise PreflightError("real_startup_marker_missing")
    print("real_signed_desktop_daemon_startup_restart=passed")
    content_result = run([positive / "Contents/MacOS/cryptex-desktop",
                  "--cryptex-m8-local-content-check", root / "content-runtime"],
                 "real_verified_content_and_native_sessions", 90)
    for marker in ("verified_tauri_assets=passed", "native_synthetic_approval_sessions=passed",
                   "native_lost_session_and_old_handle_refused=passed",
                   "native_prepare_and_actor_failures_burn_grants=passed"):
        if marker not in content_result.splitlines():
            raise PreflightError("native_content_session_marker_missing")
        print(marker)
    for runtime, expected in (("content-runtime", "anchored"), ("content-runtime.lost", "owner_pending")):
        journal = json.loads((root / runtime / "bootstrap/pending.json").read_text(encoding="utf-8"))
        if not journal["transactions"] or journal["transactions"][-1]["state"] != expected:
            raise PreflightError("native_session_journal_outcome_mismatch")
    for suffix in ("missing-snapshot", "handle-allocation", "lost-receiver"):
        journal = json.loads((root / ("content-runtime." + suffix) / "bootstrap/pending.json").read_text(encoding="utf-8"))
        if len(journal["transactions"]) != 1 or journal["transactions"][0]["state"] != "owner_pending":
            raise PreflightError("native_failed_delivery_did_not_preserve_pending_journal")
    if (root / "content-runtime.expired-queue/bootstrap/pending.json").exists():
        raise PreflightError("native_expired_queue_prepared_a_grant")

    cases = ("modified_content", "wrong_release", "wrong_cdhash",
             "wrong_identifier", "missing_leaf", "missing_rendezvous",
             "wrong_desktop_identifier", "wrong_parent_cdhash")
    for case in cases:
        bundle = root / (case + ".app")
        shutil.copytree(template, bundle)
        daemon = bundle / "Contents/MacOS/cryptexd"
        if case == "modified_content":
            (bundle / "Contents/Resources/startup-check.txt").write_bytes(b"Changed fixture.\n")
        elif case == "wrong_release":
            inventory(bundle, release="foreign-release")
        elif case == "wrong_cdhash":
            inventory(bundle, role_hash="ab" * 20)
        elif case == "wrong_parent_cdhash":
            inventory(bundle, parent_hash="ab" * 20)
        elif case in ("wrong_identifier", "missing_leaf", "wrong_desktop_identifier"):
            signer = "-" if case == "missing_leaf" else fingerprint
            identifier = ("com.cryptex.wrong-role" if case != "missing_leaf"
                          else "com.cryptex.m8.local-startup.daemon")
            target = (bundle / "Contents/MacOS/cryptex-desktop"
                      if case == "wrong_desktop_identifier" else daemon)
            args = ["/usr/bin/codesign", "--force", "--sign", signer,
                    "--timestamp=none", "--identifier", identifier]
            if signer != "-":
                args.extend(["--keychain", keychain])
            run([*args, target], "resign_" + case)
            inventory(bundle)
        runtime = root / (case + "-runtime")
        if case == "missing_rendezvous":
            args = [daemon, "lifecycle", "hold", "--runtime-root", runtime,
                    "--ready-file", runtime / "ready", "--stop-file", runtime / "stop",
                    "--m8-local-startup-epoch", "01" * 16]
        else:
            args = [bundle / "Contents/MacOS/cryptex-desktop",
                    "--cryptex-m8-local-startup-check", runtime]
        outcome = run_bounded([str(arg) for arg in args], 20)
        expected = ("Inventory" if case in ("modified_content", "wrong_release") else
                    "Rendezvous" if case == "missing_rendezvous" else "Image")
        if (outcome.code != 2 or outcome.timed_out or outcome.limited or outcome.pipe_failed
                or "local_signed_startup_" + expected not in outcome.output.splitlines()
                or any(marker in outcome.output for marker in markers)
                or (runtime.exists() and any(runtime.iterdir()))):
            raise PreflightError("real_startup_refusal_failed_" + case)
        print("real_signed_startup_refused_" + case + "=passed")
