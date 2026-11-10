"""Atomic inheritance and self-anchored bundle candidate, never production IPC."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import signal
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError, checked, run_bounded, scalar, unique_object
from mac_xpc_probe import new_private_work, select_identity, validate_work, verify_package

SCOPE = "macos_atomic_launch_same_organization_candidate_only"
DESKTOP = "com.cryptex.m8.custody.desktop"
DAEMON = "com.cryptex.m8.custody.daemon"
FILES = ("MAC-BAGLANTI.command", "MAC-BAGLANTI-BASLA.txt", "mac_launch_security.py",
         "mac_launch_security.c", "mac_xpc_probe.py", "mac_identity_preflight.py")
ANCHOR_CASES = {"wrong_identifier_refused": "refused", "adhoc_counterpart_refused": "refused"}
COMMON_CASES = {"supervisor_timeout_distinct": "not_child_deadline"}
CHANNEL_CASES = {
    "raw_fork_excludes_both_ends": "excluded",
    "unrelated_spawn_excludes_both_ends": "excluded",
    "intended_child_live_parent": "accepted",
    "short_handoff": "refused", "extra_handoff": "refused",
    "forged_parent_token": "refused", "missing_eof": "deadline_refused",
    "fresh_child_restart": "accepted",
}
DIAGNOSTICS = {"apple_anchor_or_fixed_self_identifier_refused", "certified_team_unavailable",
               "packaged_self_path_refused", "sealed_bundle_or_counterpart_refused",
               "verified_same_organization_bundle"}
PUBLIC_HEADERS = "#include <fcntl.h>\n#include <unistd.h>\n"
PUBLIC_ATOMIC_EXPRESSIONS = {
    "pipe2_declaration": "if (__builtin_available(macOS 27.0, *)) { (void)&pipe2; }",
    "open_clofork_declaration": "(void)O_CLOFORK;",
    "fd_clofork_declaration": "(void)FD_CLOFORK;",
    "combined_atomic_expression": "if (__builtin_available(macOS 27.0, *)) { int fds[2]; (void)pipe2(fds, O_CLOEXEC | O_CLOFORK | O_NONBLOCK); (void)fcntl(fds[0], F_SETFD, FD_CLOEXEC | FD_CLOFORK); }",
}


def public_atomic_checks(work, sdk, run):
    checks = {}
    for name, expression in {"baseline_public_headers": "", **PUBLIC_ATOMIC_EXPRESSIONS}.items():
        source = work / (name + ".c")
        with source.open("x", encoding="utf-8", newline="\n") as target:
            target.write(PUBLIC_HEADERS + "void probe(void) { " + expression + " }\n")
        result = run(["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
                      "-Werror=unguarded-availability-new", "-mmacosx-version-min=12.0",
                      "-isysroot", sdk, "-fsyntax-only", str(source)], 30)
        if result.limited or result.timed_out or result.pipe_failed:
            raise PreflightError("public_sdk_probe_bound_failed")
        if result.code not in (0, 1):
            raise PreflightError("public_sdk_compiler_failed")
        checks[name] = result.code == 0
        if name == "baseline_public_headers" and not checks[name]:
            raise PreflightError("public_sdk_header_baseline_failed")
    return checks


def atomic_build_enabled(checks):
    return (set(checks) == {"baseline_public_headers", *PUBLIC_ATOMIC_EXPRESSIONS}
            and all(type(value) is bool and value for value in checks.values()))


def parse_native(output):
    if len(output.encode("utf-8")) > 65536:
        raise PreflightError("native_output_limit")
    rows = [json.loads(line, object_pairs_hook=unique_object) for line in output.splitlines()]
    if not 2 <= len(rows) <= 13 or any(type(row) is not dict for row in rows):
        raise PreflightError("native_inventory_invalid")
    api, anchor = rows[:2]
    if (set(api) != {"capability", "headers", "runtime"} or api["capability"] != "atomic_api"
            or any(type(api[k]) is not bool for k in ("headers", "runtime"))
            or (api["runtime"] and not api["headers"])):
        raise PreflightError("native_capability_invalid")
    if (set(anchor) != {"capability", "eligible", "diagnostic"}
            or anchor["capability"] != "self_anchored_bundle" or type(anchor["eligible"]) is not bool
            or anchor["diagnostic"] not in DIAGNOSTICS
            or anchor["eligible"] != (anchor["diagnostic"] == "verified_same_organization_bundle")):
        raise PreflightError("native_anchor_invalid")
    cases = rows[2:]
    expected = dict(COMMON_CASES)
    if anchor["eligible"]:
        expected.update(ANCHOR_CASES)
    if anchor["eligible"] and api["runtime"]:
        expected.update(CHANNEL_CASES)
    if len(cases) != len(expected) or {r.get("case") for r in cases} != set(expected):
        raise PreflightError("native_case_inventory_incomplete")
    for row in cases:
        if (set(row) != {"case", "result", "pass"} or row["result"] != expected[row["case"]]
                or type(row["pass"]) is not bool):
            raise PreflightError("native_case_invalid")
    return {"atomic_api": api, "anchor": anchor, "cases": cases}


def complete(native):
    return (native["atomic_api"]["runtime"] and native["anchor"]["eligible"]
            and len(native["cases"]) == len(COMMON_CASES) + len(ANCHOR_CASES) + len(CHANNEL_CASES)
            and all(row["pass"] for row in native["cases"]))


def collect(report, package, work, ci=False):
    for key, args, pattern in (
        ("macos_version", ["/usr/bin/sw_vers", "-productVersion"], r"\d{1,3}(?:\.\d{1,3}){1,2}"),
        ("sdk_version", ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], r"\d{1,3}(?:\.\d{1,3}){1,2}"),
        ("architecture", ["/usr/bin/uname", "-m"], r"arm64|x86_64")):
        report[key] = scalar(run_bounded, args, pattern, key)
    sdk = checked(run_bounded, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
    if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
        raise PreflightError("sdk_path_invalid")
    source = package / "mac_launch_security.c"
    report["native_source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    report["public_sdk_checks"] = public_atomic_checks(work, sdk, run_bounded)
    api_enabled = atomic_build_enabled(report["public_sdk_checks"])
    bundle = work / "Candidate.app"
    binaries = bundle / "Contents" / "MacOS"
    binaries.mkdir(parents=True)
    desktop, daemon = binaries / "Desktop", binaries / "Daemon"
    compiled = run_bounded(["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
                           "-DCPX_PUBLIC_ATOMIC_API=" + ("1" if api_enabled else "0"),
                           "-Werror=unguarded-availability-new", "-mmacosx-version-min=12.0",
                           "-isysroot", sdk, str(source), "-framework", "Security", "-framework",
                           "CoreFoundation", "-lbsm", "-o", str(desktop)], 60)
    if ci:
        print(compiled.output, file=sys.stderr)
    checked(lambda *_: compiled, [], "native_compile", 60)
    report["native_compile"] = "pass"
    shutil.copyfile(desktop, daemon)
    daemon.chmod(0o700)
    wrong, adhoc = work / "WrongIdentity", work / "AdhocDaemon"
    for binary in (wrong, adhoc):
        shutil.copyfile(desktop, binary)
        binary.chmod(0o700)
    with (bundle / "Contents" / "Info.plist").open("xb") as target:
        plistlib.dump({"CFBundleIdentifier": DESKTOP, "CFBundleExecutable": "Desktop",
                       "CFBundlePackageType": "APPL", "CFBundleVersion": "1"}, target)
    sign = "-" if ci else select_identity(checked(run_bounded,
        ["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"], "identity_inventory"))
    report["signing_mode"] = "disposable_ci_adhoc_negative_control" if ci else "single_existing_certificate"
    if not ci:
        report["private_key_use"] = "requested_for_isolated_candidate_bundle"
    for binary, identifier, signer in ((daemon, DAEMON, sign), (desktop, DESKTOP, sign),
                                     (wrong, "com.cryptex.m8.custody.wrong", sign), (adhoc, DAEMON, "-")):
        checked(run_bounded, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", signer,
                             "--identifier", identifier, str(binary)], "fixture_sign", 90)
    checked(run_bounded, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", sign,
                         "--identifier", DESKTOP, str(bundle)], "bundle_sign", 90)
    checked(run_bounded, ["/usr/bin/codesign", "--verify", "--deep", "--strict", str(bundle)], "bundle_verify")
    if not ci:
        report["private_key_use"] = "isolated_candidate_signing_succeeded"
    report["signed_binary_sha256"] = {name: hashlib.sha256(path.read_bytes()).hexdigest()
                                      for name, path in (("desktop", desktop), ("daemon", daemon))}
    result = run_bounded([str(desktop), "probe", str(wrong), str(adhoc)], 60)
    native = parse_native(checked(lambda *_: result, [], "native_probe", 60))
    report["native"] = native
    if native["atomic_api"]["headers"] != api_enabled:
        raise PreflightError("native_public_api_build_mismatch")
    report["atomic_runtime_gate"] = ("not_exercised_public_headers_refused" if not api_enabled else
                                     "os_guard_passed" if native["atomic_api"]["runtime"] else "os_guard_refused")
    if any(not row["pass"] for row in native["cases"]):
        raise PreflightError("native_candidate_case_failed")
    observed = complete(native)
    if ci:
        if native["anchor"]["eligible"] or native["anchor"]["diagnostic"] != "apple_anchor_or_fixed_self_identifier_refused":
            raise PreflightError("ci_adhoc_self_was_not_explicitly_refused")
        report["outcome"] = "ci_adhoc_self_refusal_observed_positive_native_cases_not_run"
    elif not native["anchor"]["eligible"]:
        report["outcome"] = "candidate_incomplete_self_anchor_refused"
    elif not native["atomic_api"]["runtime"]:
        report["outcome"] = "candidate_incomplete_atomic_public_api_unavailable"
    else:
        report["outcome"] = "atomic_parent_handoff_and_same_organization_bundle_observed"
    report["candidate_complete"] = observed and not ci


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--work", type=Path)
    parser.add_argument("--ci", action="store_true")
    args = parser.parse_args()
    if sys.platform != "darwin" or (args.ci and (args.work or os.environ.get("GITHUB_ACTIONS") != "true")):
        return 2
    work = new_private_work() if args.ci else args.work
    if work is None:
        return 2
    package = args.package.resolve()
    validate_work(work, package)
    os.environ["TMPDIR"] = str(work)
    report = {"schema": 1, "scope": SCOPE, "collected_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "ci_only": args.ci, "candidate_complete": False, "private_key_use": "not_requested",
              "production_integration": "not_implemented", "production_changed": False, "m8_complete": False,
              "product_minimum_os": "unchanged_unconfirmed", "candidate_atomic_api_os_floor": "27.0",
              "publisher_or_notarization_acceptance": False, "unix_frame_sender_attribution": "not_proven",
              "concurrent_bundle_replacement": "not_tested", "bidirectional_daemon_instance_pin": "not_implemented",
              "owner_session_restart": "not_tested"}
    def interrupted(*_):
        raise KeyboardInterrupt
    for name in ("SIGINT", "SIGTERM", "SIGHUP"):
        signal.signal(getattr(signal, name), interrupted)
    code = 0
    try:
        if not args.ci:
            report["source_commit"] = verify_package(package, FILES, SCOPE)
        collect(report, package, work, args.ci)
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
    if args.ci:
        print(json.dumps(report, indent=2))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
