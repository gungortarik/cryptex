"""Disposable CI public Mach bootstrap feasibility; never signed product custody."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError, checked, run_bounded, unique_object
from mac_xpc_probe import new_private_work, validate_work

SCOPE = "public_mach_bootstrap_sender_binding_ci_mechanism_only"
IDENTIFIER = "com.cryptex.m8.bootstrap.mechanism"
EXPECTED = {
    "active_name_collision": ("collision_refused", "check_in", (1101, 1103)),
    "same_image_imposter": ("sender_pid_refused", "validate", (52,)),
    "two_way_after_imposter": ("authenticated", "validate", (0,)),
    "same_sender_replay": ("replay_refused", "validate", (56,)),
    "wrong_session": ("session_refused", "validate", (51,)),
    "short_frame": ("layout_refused", "validate", (50,)),
    "complex_frame": ("layout_refused", "validate", (50,)),
    "same_pid_exec": ("sender_generation_refused", "validate", (53,)),
    "queued_after_exit": ("sender_not_live", "validate", (54,)),
    "fresh_child_session": ("authenticated", "validate", (0,)),
    "supervisor_deadline_distinct": ("supervisor_deadline", "supervisor", (63,)),
    "substituted_endpoint": ("sender_pid_refused", "child_exit", (52,)),
    "transferred_reply": ("sender_pid_refused", "child_exit", (52,)),
    "transferred_service": ("sender_pid_refused", "validate", (52,)),
    "genuine_after_transfer": ("authenticated", "validate", (0,)),
    "normal_close_namespace_removal": ("namespace_removed", "independent_lookup", (1102,)),
    "owned_right_cleanup": ("cleanup_calls_succeeded", "cleanup", (0,)),
}
OBSERVED = {value[0] for value in EXPECTED.values()} | {
    "bootstrap_error", "collision_not_refused", "spawn_error", "unexpected",
    "child_failed", "receive_deadline", "namespace_not_removed", "cleanup_error",
}
STAGES = {value[1] for value in EXPECTED.values()} | {"spawn", "child_exit", "child_signal", "wait", "kill"}
CONTROL_STAGES = {"receive_allocate", "reply_allocate", "insert_send", "queue_limit", "seed_send",
                  "extract_right", "extracted_type", "receive_drop", "unexpected_send_success"}


def control_diagnostic(output):
    try:
        value = json.loads(output, object_pairs_hook=unique_object)
    except (ValueError, PreflightError) as error:
        raise PreflightError("native_control_diagnostic_invalid") from error
    if (type(value) is not dict or set(value) != {"stage", "code"} or value["stage"] not in CONTROL_STAGES
            or type(value["code"]) is not int or not 0 <= value["code"] <= 2147483647):
        raise PreflightError("native_control_diagnostic_invalid")
    return value


def parse_native(output):
    if len(output.encode("utf-8")) > 65536:
        raise PreflightError("native_output_limit")
    try:
        rows = [json.loads(line, object_pairs_hook=unique_object) for line in output.splitlines()]
    except (ValueError, PreflightError) as error:
        raise PreflightError("native_output_invalid") from error
    seen = set()
    for row in rows:
        if (type(row) is not dict or set(row) != {"case", "observed", "stage", "code", "pass"}
                or row["case"] not in EXPECTED or row["case"] in seen
                or row["observed"] not in OBSERVED or row["stage"] not in STAGES
                or type(row["code"]) is not int or not 0 <= row["code"] <= 2147483647
                or type(row["pass"]) is not bool):
            raise PreflightError("native_row_invalid")
        expected, stage, codes = EXPECTED[row["case"]]
        if row["pass"] and (row["observed"] != expected or row["stage"] != stage or row["code"] not in codes):
            raise PreflightError("native_pass_inconsistent")
        seen.add(row["case"])
    if not rows or len(rows) > len(EXPECTED):
        raise PreflightError("native_inventory_invalid")
    return rows


def complete(rows, native_exit):
    return (type(native_exit) is int and native_exit == 0 and len(rows) == len(EXPECTED)
            and {row["case"] for row in rows} == set(EXPECTED) and all(row["pass"] for row in rows))


def fault_controls(binary, run, report):
    report["fault_controls"] = {}
    for mode, expected in (("reaper-controls", 0), ("cleanup-control", 68),
                           ("send-timeout-copy", 67), ("send-error-once-header", 59)):
        control = run([str(binary), mode], 10)
        report["fault_controls"][mode] = {"stage": "child_signal" if control.code < 0 else "child_exit", "code": abs(control.code)}
        if control.output:
            report["fault_controls"][mode]["diagnostic"] = control_diagnostic(control.output)
        if (control.code != expected or control.output != "" or control.limited or control.timed_out or control.pipe_failed):
            raise PreflightError("native_fault_control_failed")
    report["failed_send_cleanup"] = "fixture_exit_releases_ipc_space_supervisor_stops_owned_group"


def collect(work, run=run_bounded):
    report = {"schema": 1, "scope": SCOPE, "ci_only": True, "phase_one_complete": False,
              "identity_policy": "fixed_identifier_same_image_adhoc_ci_fixture_only",
              "certified_bundle_acceptance": False, "production_changed": False,
              "owner_session_acceptance": False, "m8_complete": False,
              "product_minimum_os": "unchanged", "owner_package": "not_produced"}
    source = Path(__file__).with_suffix(".c")
    report["native_source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    report["native_policy_source_sha256"] = hashlib.sha256(source.with_name("mac_bootstrap_bundle.h").read_bytes()).hexdigest()
    binary = work / "bootstrap-mechanism"
    try:
        sdk = checked(run, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
        if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
            raise PreflightError("sdk_path_invalid")
        compiled = run(["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
                        "-Werror=unguarded-availability-new", "-mmacosx-version-min=12.0", "-isysroot", sdk,
                        str(source), "-framework", "Security", "-framework", "CoreFoundation", "-lbsm", "-o", str(binary)], 60)
        if compiled.code or compiled.limited or compiled.timed_out or compiled.pipe_failed:
            # Fixed source compiler diagnostics on disposable CI, never keychain output.
            print(compiled.output, file=sys.stderr)
        checked(lambda *_: compiled, [], "native_compile", 60)
        checked(run, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", "-",
                      "--identifier", IDENTIFIER, str(binary)], "adhoc_fixture_sign")
        fault_controls(binary, run, report)
        prefix = "com.cryptex.m8.bootstrap." + secrets.token_hex(12)
        native = run([str(binary), "probe", prefix], 120)
        report["native_exit"] = native.code
        if native.limited or native.timed_out or native.pipe_failed:
            raise PreflightError("native_runner_bound_failed")
        report["cases"] = parse_native(native.output)
        # Preserve partial inventories/typed failures before rejecting completion.
        native_complete = complete(report["cases"], native.code)
        forced = run([str(binary), "forced-exit", prefix + ".forced"], 10)
        report["forced_exit"] = {"stage": "child_signal" if forced.code < 0 else "child_exit", "code": abs(forced.code)}
        if forced.limited or forced.timed_out or forced.pipe_failed or forced.code != -signal.SIGKILL:
            raise PreflightError("forced_exit_control_failed")
        cleanup = run([str(binary), "lookup-missing", prefix + ".forced"], 5)
        report["forced_exit_namespace_removed"] = (cleanup.code == 0 and not cleanup.limited
            and not cleanup.timed_out and not cleanup.pipe_failed and cleanup.output == "")
        report["forced_cleanup_exit"] = cleanup.code
        if not report["forced_exit_namespace_removed"]:
            raise PreflightError("forced_exit_namespace_cleanup_failed")
        if not native_complete:
            raise PreflightError("native_mechanism_incomplete")
        report["phase_one_complete"] = True
        report["outcome"] = "disposable_public_bootstrap_and_fixture_sender_binding_observed"
    except PreflightError as error:
        report.update(outcome="mechanism_incomplete", diagnostic=str(error), phase_one_complete=False)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ci", action="store_true", required=True)
    args = parser.parse_args()
    if not args.ci or sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true":
        return 2
    work = new_private_work()
    validate_work(work, Path(__file__).parent.resolve())
    os.environ["TMPDIR"] = str(work)
    report = collect(work)
    print(json.dumps(report, indent=2))
    return 0 if report["phase_one_complete"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
