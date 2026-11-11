"""Signed two-binary public Mach bootstrap candidate; never product authorization."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import plistlib
import secrets
import shutil
import signal
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import mac_bootstrap_mechanism as mechanism
from mac_identity_preflight import PreflightError, checked, run_bounded, scalar, unique_object
from mac_xpc_probe import new_private_work, select_identity, validate_work, verify_package

SCOPE = "macos_public_message_bootstrap_signed_two_binary_candidate_only"
DESKTOP = "com.cryptex.m8.bootstrap.desktop"
DAEMON = "com.cryptex.m8.bootstrap.daemon"
FILES = ("MAC-MESAJ.command", "MAC-MESAJ-BASLA.txt", "mac_bootstrap_signed.py",
         "mac_bootstrap_mechanism.py", "mac_bootstrap_mechanism.c", "mac_bootstrap_bundle.h",
         "mac_identity_preflight.py", "mac_xpc_probe.py")
DIAGNOSTICS = {"packaged_self_path_refused", "apple_anchor_or_fixed_self_identifier_refused",
               "certified_team_unavailable", "sealed_bundle_or_counterpart_refused",
               "controller_role_refused", "verified_self_organization_two_binary_bundle"}
NEGATIVES = {"wrong_role_image", "adhoc_peer_image"}


def parse_signed(output):
    if len(output.encode("utf-8")) > 65536:
        raise PreflightError("native_output_limit")
    try:
        values = [json.loads(line, object_pairs_hook=unique_object) for line in output.splitlines()]
    except (ValueError, PreflightError) as error:
        raise PreflightError("native_output_invalid") from error
    if not values or type(values[0]) is not dict:
        raise PreflightError("bundle_capability_missing")
    capability = values[0]
    if (set(capability) != {"capability", "eligible", "diagnostic"}
            or capability["capability"] != "certified_two_binary_bundle" or type(capability["eligible"]) is not bool
            or capability["diagnostic"] not in DIAGNOSTICS
            or capability["eligible"] != (capability["diagnostic"] == "verified_self_organization_two_binary_bundle")):
        raise PreflightError("bundle_capability_invalid")
    if not capability["eligible"]:
        if len(values) != 1:
            raise PreflightError("ineligible_bundle_has_cases")
        return {"bundle": capability, "signature_cases": [], "cases": []}
    if len(values) < 3:
        raise PreflightError("signature_inventory_missing")
    signature_cases = values[1:3]
    seen = set()
    for row in signature_cases:
        if (type(row) is not dict or set(row) != {"case", "observed", "stage", "code", "pass"}
                or row["case"] not in NEGATIVES or row["case"] in seen
                or type(row["pass"]) is not bool or type(row["code"]) is not int
                or row["stage"] != "static_code" or row["code"] not in (55, 62)
                or row["observed"] not in {"code_requirement_refused", "unexpected"}
                or (row["pass"] and (row["code"] != 55 or row["observed"] != "code_requirement_refused"))):
            raise PreflightError("signature_case_invalid")
        seen.add(row["case"])
    if seen != NEGATIVES:
        raise PreflightError("signature_inventory_invalid")
    cases = mechanism.parse_native("\n".join(json.dumps(row) for row in values[3:]))
    return {"bundle": capability, "signature_cases": signature_cases, "cases": cases}


def complete(native, exit_code, ci):
    return (ci is False and native["bundle"]["eligible"] and len(native["signature_cases"]) == 2
            and all(row["pass"] for row in native["signature_cases"])
            and mechanism.complete(native["cases"], exit_code))


def collect(report, package, work, ci=False, run=run_bounded):
    for key, args, pattern in (
        ("macos_version", ["/usr/bin/sw_vers", "-productVersion"], r"\d{1,3}(?:\.\d{1,3}){1,2}"),
        ("sdk_version", ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], r"\d{1,3}(?:\.\d{1,3}){1,2}"),
        ("architecture", ["/usr/bin/uname", "-m"], r"arm64|x86_64")):
        report[key] = scalar(run, args, pattern, key)
    sdk = checked(run, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
    if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
        raise PreflightError("sdk_path_invalid")
    source = package / "mac_bootstrap_mechanism.c"
    report["native_source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    report["native_policy_source_sha256"] = hashlib.sha256((package / "mac_bootstrap_bundle.h").read_bytes()).hexdigest()
    bundle = work / "MessageCandidate.app"
    binaries = bundle / "Contents" / "MacOS"
    binaries.mkdir(parents=True)
    desktop, daemon = binaries / "Desktop", binaries / "Daemon"
    compiled = run(["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
                    "-DCPX_BUNDLE_IDENTITY=1", "-Werror=unguarded-availability-new", "-mmacosx-version-min=12.0",
                    "-isysroot", sdk, str(source), "-framework", "Security", "-framework", "CoreFoundation",
                    "-lbsm", "-o", str(desktop)], 60)
    if ci and (compiled.code or compiled.limited or compiled.timed_out or compiled.pipe_failed):
        print(compiled.output, file=sys.stderr)
    checked(lambda *_: compiled, [], "native_compile", 60)
    report["native_compile"] = "pass"
    wrong, adhoc = work / "WrongRole", work / "AdhocDaemon"
    for binary in (daemon, wrong, adhoc):
        shutil.copyfile(desktop, binary)
        binary.chmod(0o700)
    with (bundle / "Contents" / "Info.plist").open("xb") as target:
        plistlib.dump({"CFBundleIdentifier": DESKTOP, "CFBundleExecutable": "Desktop",
                       "CFBundlePackageType": "APPL", "CFBundleVersion": "1"}, target)
    signer = "-" if ci else select_identity(checked(run,
        ["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"], "identity_inventory"))
    report["signing_mode"] = "ci_adhoc_negative_control" if ci else "single_existing_certificate"
    if not ci:
        report["private_key_use"] = "requested_for_isolated_candidate_bundle"
    for binary, identifier, identity in ((daemon, DAEMON, signer), (desktop, DESKTOP, signer),
                                        (wrong, "com.cryptex.m8.bootstrap.wrong", signer), (adhoc, DAEMON, "-")):
        checked(run, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", identity,
                      "--identifier", identifier, str(binary)], "fixture_sign", 90)
        checked(run, ["/usr/bin/codesign", "--verify", "--strict", str(binary)], "fixture_verify")
    checked(run, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", signer,
                  "--identifier", DESKTOP, str(bundle)], "bundle_sign", 90)
    checked(run, ["/usr/bin/codesign", "--verify", "--deep", "--strict", str(bundle)], "bundle_verify")
    if not ci:
        report["private_key_use"] = "isolated_candidate_signing_succeeded"
    report["signed_binary_sha256"] = {name: hashlib.sha256(path.read_bytes()).hexdigest()
                                      for name, path in (("desktop", desktop), ("daemon", daemon))}
    mechanism.fault_controls(desktop, run, report)
    prefix = "com.cryptex.m8.bootstrap." + secrets.token_hex(12)
    result = run([str(desktop), "probe", prefix, str(wrong), str(adhoc)], 120)
    report["native_exit"] = result.code
    if result.limited or result.timed_out or result.pipe_failed:
        raise PreflightError("native_runner_bound_failed")
    native = parse_signed(result.output)
    report["native"] = native
    if ci:
        if result.code != 0 or native["bundle"]["eligible"] or native["bundle"]["diagnostic"] != "apple_anchor_or_fixed_self_identifier_refused":
            raise PreflightError("ci_adhoc_self_was_not_explicitly_refused")
        report["outcome"] = "ci_adhoc_self_refusal_observed_owner_positive_cases_not_run"
        return
    if not native["bundle"]["eligible"]:
        report["outcome"] = "candidate_incomplete_self_bundle_refused"
        return
    forced = run([str(desktop), "forced-exit", prefix + ".forced"], 10)
    report["forced_exit"] = {"stage": "child_signal" if forced.code < 0 else "child_exit", "code": abs(forced.code)}
    if forced.limited or forced.timed_out or forced.pipe_failed or forced.code != -signal.SIGKILL:
        raise PreflightError("forced_exit_control_failed")
    cleanup = run([str(desktop), "lookup-missing", prefix + ".forced"], 5)
    report["forced_exit_namespace_removed"] = (cleanup.code == 0 and not cleanup.limited
        and not cleanup.timed_out and not cleanup.pipe_failed and cleanup.output == "")
    report["forced_cleanup_exit"] = cleanup.code
    if not report["forced_exit_namespace_removed"]:
        raise PreflightError("forced_exit_namespace_cleanup_failed")
    if not complete(native, result.code, ci):
        raise PreflightError("native_signed_candidate_incomplete")
    report["candidate_complete"] = True
    report["outcome"] = "signed_two_binary_bootstrap_and_hostile_endpoint_cases_observed"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--work", type=Path)
    parser.add_argument("--ci-refusal", action="store_true")
    args = parser.parse_args()
    ci = args.ci_refusal
    if sys.platform != "darwin" or (ci and (args.work or os.environ.get("GITHUB_ACTIONS") != "true")):
        return 2
    work = new_private_work() if ci else args.work
    if work is None:
        return 2
    package = args.package.resolve()
    validate_work(work, package)
    os.environ["TMPDIR"] = str(work)
    report = {"schema": 1, "scope": SCOPE, "ci_only": ci, "candidate_complete": False,
              "collected_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "private_key_use": "not_requested", "production_integration": "not_implemented",
              "production_changed": False, "m8_complete": False, "publisher_or_notarization_acceptance": False,
              "product_minimum_os": "unchanged_unconfirmed", "owner_session_restart": "not_tested",
              "concurrent_bundle_replacement": "not_tested", "unix_frame_sender_attribution": "not_proven"}
    def interrupted(*_):
        raise KeyboardInterrupt
    for name in ("SIGINT", "SIGTERM", "SIGHUP"):
        signal.signal(getattr(signal, name), interrupted)
    code = 0
    try:
        if not ci:
            report["source_commit"] = verify_package(package, FILES, SCOPE)
        collect(report, package, work, ci)
    except PreflightError as error:
        report.update(outcome="candidate_incomplete", diagnostic=str(error), candidate_complete=False)
        code = 2
    except KeyboardInterrupt:
        report.update(outcome="candidate_interrupted", diagnostic="interrupted", candidate_complete=False)
        code = 130
    except Exception:
        report.update(outcome="candidate_incomplete", diagnostic="internal_error", candidate_complete=False)
        code = 2
    report["runner_exit"] = code
    with (work / "result.json").open("x", encoding="utf-8", newline="\n") as target:
        json.dump(report, target, ensure_ascii=True, indent=2)
        target.write("\n")
    if ci:
        print(json.dumps(report, indent=2))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
