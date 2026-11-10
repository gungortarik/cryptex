"""Synthetic actual-sender Mach audit proof; no production transport migration."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError, checked, run_bounded, scalar, unique_object
from mac_xpc_probe import new_private_work, select_identity, validate_work, verify_package

SCOPE = "raw_mach_message_instance_probe_only"
IDENTIFIER = "com.cryptex.m8.synthetic.process"
FILES = ("MAC-SUREC.command", "MAC-SUREC-BASLA.txt", "mac_message_identity.py",
         "mac_message_identity.c", "mac_xpc_probe.py", "mac_identity_preflight.py")
EXPECTED = {
    "forged_enrollment_pid_token_pair": "rejected",
    "expected_sender": "accepted",
    "same_binary_shared_send_right_forged_payload": "sender_instance_rejected",
    "same_sender_replay": "replay_rejected",
    "expected_after_negative": "accepted",
    "same_pid_exec_old_epoch": "sender_instance_rejected",
    "fresh_enrolled_execution": "accepted",
    "queued_message_after_sender_exit": "sender_not_live_or_code_invalid",
}


def observations(output):
    rows = [json.loads(line, object_pairs_hook=unique_object) for line in output.splitlines()]
    if len(rows) != len(EXPECTED) or {r.get("case") for r in rows} != set(EXPECTED):
        raise PreflightError("native_case_inventory_invalid")
    for row in rows:
        keys = {"case", "result", "pass"}
        if row["case"] != "forged_enrollment_pid_token_pair":
            keys |= {"same_pid", "same_generation"}
        if set(row) != keys or row["result"] != EXPECTED[row["case"]] or row["pass"] is not True:
            raise PreflightError("native_case_failed")
        if keys != {"case", "result", "pass"} and any(type(row[k]) is not bool for k in ("same_pid", "same_generation")):
            raise PreflightError("native_case_types_invalid")
        if row["case"] == "same_pid_exec_old_epoch" and (row["same_pid"] is not True or row["same_generation"] is not False):
            raise PreflightError("exec_generation_not_observed")
        if row["case"] == "same_binary_shared_send_right_forged_payload" and row["same_pid"] is not False:
            raise PreflightError("second_sender_not_observed")
    return rows


def collect(report, package, work, ci=False):
    for key, args, pattern in (
            ("macos_version", ["/usr/bin/sw_vers", "-productVersion"], r"\d{1,3}(?:\.\d{1,3}){1,2}"),
            ("sdk_version", ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], r"\d{1,3}(?:\.\d{1,3}){1,2}"),
            ("architecture", ["/usr/bin/uname", "-m"], r"arm64|x86_64")):
        report[key] = scalar(run_bounded, args, pattern, key)
    sdk = checked(run_bounded, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
    if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
        raise PreflightError("sdk_path_invalid")
    binary = work / "message-identity"
    source = package / "mac_message_identity.c"
    report["native_source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    compiled = run_bounded(["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
                           "-mmacosx-version-min=12.0", "-isysroot", sdk, str(source), "-framework", "Security",
                           "-framework", "CoreFoundation", "-lbsm", "-o", str(binary)], 60)
    if ci:
        print(compiled.output, file=sys.stderr)
    checked(lambda *_: compiled, [], "native_compile", 60)
    report["native_compile"] = "pass"
    sign = "-" if ci else select_identity(checked(run_bounded, ["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"], "identity_inventory"))
    report["signing_mode"] = "disposable_ci_adhoc" if ci else "single_existing_certificate"
    if not ci:
        report["private_key_use"] = "requested_for_synthetic_fixture"
    checked(run_bounded, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", sign,
                         "--identifier", IDENTIFIER, str(binary)], "fixture_sign", 90)
    checked(run_bounded, ["/usr/bin/codesign", "--verify", "--strict", str(binary)], "fixture_verify")
    if ci:
        details = checked(run_bounded, ["/usr/bin/codesign", "-d", "--verbose=4", str(binary)], "ci_code_hash")
        hashes = re.findall(r"^CDHash=([0-9a-f]{40})$", details, re.M)
        if len(hashes) != 1:
            raise PreflightError("ci_code_hash_invalid")
        requirement = 'identifier "' + IDENTIFIER + '" and cdhash H"' + hashes[0] + '"'
    else:
        report["private_key_use"] = "synthetic_fixture_signing_succeeded"
        requirement = 'identifier "' + IDENTIFIER + '" and certificate leaf = H"' + sign + '"'
    report["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    report["requirement_sha256"] = hashlib.sha256(requirement.encode()).hexdigest()
    result = run_bounded([str(binary), "probe", requirement], 40)
    if ci:
        # Native source emits only fixed codes/case booleans, never audit bytes.
        print(result.output, file=sys.stderr)
    report["cases"] = observations(checked(lambda *_: result, [], "native_probe", 40))
    report["raw_mach_sender_binding"] = "observed_in_synthetic_cases"
    report["outcome"] = "synthetic_raw_mach_observations_collected"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--work", type=Path)
    parser.add_argument("--ci", action="store_true")
    args = parser.parse_args()
    if sys.platform != "darwin" or (args.ci and os.environ.get("GITHUB_ACTIONS") != "true"):
        return 2
    if args.ci:
        if args.work:
            return 2
        work = new_private_work()
    else:
        if not args.work:
            return 2
        work = args.work
    package = args.package.resolve()
    validate_work(work, package)
    os.environ["TMPDIR"] = str(work)
    report = {"schema": 1, "scope": SCOPE, "collected_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "private_key_use": "not_requested", "raw_mach_sender_binding": "not_tested",
              "xpc_exact_message_sender_binding": "not_proven", "production_integration": "not_implemented",
              "product_minimum_os": "unconfirmed", "actual_pid_reuse": "not_forced",
              "production_changed": False, "m8_complete": False, "ci_only": args.ci}
    code = 0

    def interrupted(*_):
        raise KeyboardInterrupt

    for name in ("SIGINT", "SIGTERM", "SIGHUP"):
        signal.signal(getattr(signal, name), interrupted)
    try:
        if not args.ci:
            report["source_commit"] = verify_package(package, FILES, SCOPE)
        collect(report, package, work, args.ci)
    except PreflightError as error:
        report.update(outcome="probe_incomplete", diagnostic=str(error))
        code = 2
    except KeyboardInterrupt:
        report.update(outcome="probe_interrupted", diagnostic="interrupted")
        code = 130
    except Exception:
        report.update(outcome="probe_incomplete", diagnostic="internal_error")
        code = 2
    report["runner_exit"] = code
    with (work / "result.json").open("x", encoding="utf-8", newline="\n") as output:
        json.dump(report, output, ensure_ascii=True, indent=2)
        output.write("\n")
    if args.ci:
        print(json.dumps(report, indent=2))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
