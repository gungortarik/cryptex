"""Bounded synthetic signed XPC observations; never production authentication."""

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
import stat
import sys
import uuid

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError, checked, identity_counts, run_bounded, scalar, unique_object

FILES = ("MAC-IMZA.command", "mac_xpc_probe.py", "mac_xpc_probe.c",
         "mac_identity_preflight.py", "MAC-IMZA-BASLA.txt")
SCOPE = "synthetic_signed_xpc_observations_only"
CLIENT = "com.cryptex.m8.synthetic.client"
SERVER = "com.cryptex.m8.synthetic.server"


def verify_package(package):
    manifest_path = package / "MANIFEST.json"
    if manifest_path.is_symlink() or manifest_path.stat().st_size > 8192:
        raise PreflightError("manifest_invalid")
    data = json.loads(manifest_path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if (set(data) != {"schema", "scope", "source_commit", "files"}
            or type(data["schema"]) is not int or data["schema"] != 1
            or data["scope"] != SCOPE or not re.fullmatch(r"[0-9a-f]{40}", data["source_commit"])
            or set(data["files"]) != set(FILES)):
        raise PreflightError("manifest_invalid")
    for name in FILES:
        path = package / name
        info = path.lstat()
        if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > 65536
                or hashlib.sha256(path.read_bytes()).hexdigest() != data["files"][name]):
            raise PreflightError("package_member_invalid")
    return data["source_commit"]


def validate_work(work, package):
    info = work.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700
            or not re.fullmatch(r"cryptex-mac-imza\.[A-Za-z0-9]{6,32}", work.name)
            or work.resolve().parent != Path("/private/tmp")
            or package == work.resolve() or package in work.resolve().parents
            or any((p / ".git").exists() or (p / ".git").is_symlink()
                   for p in (work.resolve(), *work.resolve().parents))):
        raise PreflightError("unsafe_work_directory")


def select_identity(inventory):
    identity_counts(inventory)  # Validate the complete inventory before selecting.
    hashes = sorted(set(re.findall(r'^\s*\d+\) ([A-Fa-f0-9]{40}) "[^"\r\n]+"\s*$', inventory, re.M)))
    if not hashes:
        raise PreflightError("existing_signing_identity_missing")
    if len(hashes) != 1:
        raise PreflightError("local_identity_selection_needed")
    return hashes[0].upper()


def requirement(identifier, certificate):
    if identifier not in (CLIENT, SERVER) or not re.fullmatch(r"[0-9A-F]{40}", certificate):
        raise PreflightError("requirement_invalid")
    return 'identifier "' + identifier + '" and certificate leaf = H"' + certificate + '"'


def client_result(result):
    if result.limited or result.timed_out or result.pipe_failed:
        return "runner_bound_failure"
    if result.code != 0:
        return "client_launch_or_exit_failure"
    try:
        value = json.loads(result.output, object_pairs_hook=unique_object)
    except (ValueError, PreflightError):
        return "invalid_client_output"
    allowed = {"echo", "message_rejected", "stored", "peer_rejected", "connection_error",
               "invalid_reply", "relay_refused", "operation_refused"}
    if not isinstance(value, dict) or set(value) != {"result"} or value["result"] not in allowed:
        return "invalid_client_output"
    return value["result"]


def read_events(path):
    if not path.exists():
        return []
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > 65536:
        raise PreflightError("server_log_invalid")
    events = []
    for line in path.read_text(encoding="utf-8").splitlines():
        event = json.loads(line, object_pairs_hook=unique_object)
        if (not isinstance(event, dict) or set(event) != {"lane", "nonce", "valid", "op"}
                or event["lane"] not in ("guard", "observe", "relay")
                or not re.fullmatch(r"[0-9a-f]{32}", event["nonce"])
                or type(event["valid"]) is not bool or event["op"] not in ("echo", "endpoint")):
            raise PreflightError("server_log_invalid")
        events.append(event)
    return events


def conclude(cases, events):
    """Missing delivery, timeouts and launch failures must never become negatives."""
    def delivered(name, result, lane, valid):
        matches = [c for c in cases if c["name"] == name]
        if len(matches) != 1 or matches[0]["client_result"] != result:
            return False
        observed = [e for e in events if e["nonce"] == matches[0]["nonce"] and e["op"] == "echo"]
        return len(observed) == 1 and observed[0]["valid"] is valid and observed[0]["lane"] == lane

    guard_controls = (delivered("expected_guard", "echo", "guard", True)
                      and delivered("expected_after_negatives", "echo", "guard", True))
    observer_control = delivered("expected_observer", "echo", "observe", True)
    # Compute before removing nonces. A client-side server requirement failure
    # can also return peer_rejected; require the paired observer to prove that
    # this executable reached the pinned server and failed message validation.
    paired_observers = {c["name"][:-6]: delivered(c["name"][:-6] + "_observer", "message_rejected", "observe", False)
                        for c in cases if c["name"].endswith("_guard")}
    for case in cases:
        if case["name"] == "origin_endpoint_stored":
            # This nonce is later reused by the recipient. Do not attribute the
            # recipient's echo to the origin's endpoint-courier control.
            case["verdict"] = "not_an_authorization_pass"
            del case["nonce"]
            continue
        observed = [e for e in events if e["nonce"] == case["nonce"] and e["op"] == "echo"]
        case["message_validation"] = [e["valid"] for e in observed]
        case["observed_lanes"] = [e["lane"] for e in observed]
        result = case["client_result"]
        if case["name"] in ("expected_guard", "expected_observer", "expected_after_negatives", "fresh_process"):
            lane = "observe" if case["name"] == "expected_observer" else "guard"
            case["verdict"] = "pass" if result == "echo" and len(observed) == 1 and observed[0]["valid"] and observed[0]["lane"] == lane else "inconclusive_or_failed"
        elif case["name"].endswith("_observer"):
            case["verdict"] = "pass" if observer_control and guard_controls and result == "message_rejected" and len(observed) == 1 and not observed[0]["valid"] and observed[0]["lane"] == "observe" else "inconclusive_or_failed"
        elif case["name"].endswith("_guard"):
            controlled = guard_controls and observer_control and paired_observers.get(case["name"][:-6], False)
            case["verdict"] = "pass" if controlled and result == "peer_rejected" and not observed else "inconclusive_or_failed"
        elif case["name"] == "forwarded_endpoint_after_origin_exit":
            case["verdict"] = "signer_only_insufficient" if result == "echo" and len(observed) == 1 and observed[0]["valid"] and observed[0]["lane"] == "guard" else "inconclusive_or_failed"
        else:
            case["verdict"] = "not_an_authorization_pass"
        del case["nonce"]
    return cases


def launch_target(work):
    label = "com.cryptex.m8.synthetic." + work.name.split(".")[-1]
    return f"gui/{os.getuid()}", label


def cleanup(work, run=run_bounded):
    marker = work / "job-owned.json"
    if not marker.exists():
        return "not_registered"
    if marker.is_symlink() or marker.stat().st_size > 1024:
        return "cleanup_marker_invalid"
    domain, label = launch_target(work)
    value = json.loads(marker.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if value != {"domain": domain, "label": label}:
        return "cleanup_marker_invalid"
    target = domain + "/" + label
    stopped = run(["/bin/launchctl", "bootout", target], 10)
    absent = run(["/bin/launchctl", "print", target], 5)
    if (stopped.code in (0, 113) and absent.code == 113
            and not any((r.limited or r.timed_out or r.pipe_failed) for r in (stopped, absent))):
        marker.unlink()
        return "registration_removed"
    return "cleanup_unconfirmed"


def collect(report, package, work):
    report["macos_version"] = scalar(run_bounded, ["/usr/bin/sw_vers", "-productVersion"], r"\d{1,3}(?:\.\d{1,3}){1,2}", "os_version")
    report["macos_build"] = scalar(run_bounded, ["/usr/bin/sw_vers", "-buildVersion"], r"[A-Za-z0-9.]{1,32}", "os_build")
    report["architecture"] = scalar(run_bounded, ["/usr/bin/uname", "-m"], r"arm64|x86_64", "architecture")
    report["sdk_version"] = scalar(run_bounded, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], r"\d{1,3}(?:\.\d{1,3}){1,2}", "sdk_version")
    report["native_source_sha256"] = hashlib.sha256((package / "mac_xpc_probe.c").read_bytes()).hexdigest()
    sign = select_identity(checked(run_bounded, ["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"], "identity_inventory"))
    report["identity_selection"] = "single_existing_certificate"
    client_req = requirement(CLIENT, sign)
    server_req = requirement(SERVER, sign)
    report["requirement_sha256"] = {"client": hashlib.sha256(client_req.encode()).hexdigest(),
                                     "server": hashlib.sha256(server_req.encode()).hexdigest()}
    sdk = checked(run_bounded, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
    if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
        raise PreflightError("sdk_path_invalid")
    binary = work / "native"
    checked(run_bounded, ["/usr/bin/xcrun", "--sdk", "macosx", "clang", "-std=c11", "-fblocks",
                         "-Wall", "-Wextra", "-Werror", "-mmacosx-version-min=12.0", "-isysroot", sdk,
                         str(package / "mac_xpc_probe.c"), "-framework", "Security", "-framework", "CoreFoundation",
                         "-o", str(binary)], "native_compile", 60)
    report["native_compile"] = "pass"
    identities = {"server": SERVER, "expected": CLIENT, "wrong_id": CLIENT + ".wrong", "adhoc": CLIENT, "unsigned": CLIENT}
    binaries = {}
    for name, identifier in identities.items():
        path = work / name
        shutil.copyfile(binary, path)
        path.chmod(0o700)
        if name == "unsigned":
            checked(run_bounded, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", "-",
                                 "--identifier", identifier, str(path)], "unsigned_fixture_prepare")
            checked(run_bounded, ["/usr/bin/codesign", "--remove-signature", str(path)], "unsigned_fixture")
        else:
            if name != "adhoc":
                report["private_key_use"] = "requested_for_synthetic_fixture_signing"
            checked(run_bounded, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign",
                                 "-" if name == "adhoc" else sign, "--identifier", identifier, str(path)], "fixture_sign", 90)
            checked(run_bounded, ["/usr/bin/codesign", "--verify", "--strict", str(path)], "fixture_verify")
        binaries[name] = path
    report["private_key_use"] = "synthetic_fixture_signing_succeeded"
    report["binary_sha256"] = {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()}
    report["synthetic_signing_identifiers"] = identities
    exercise(report, binaries, client_req, server_req, work)


def exercise(report, binaries, client_req, server_req, work,
             negative_fixtures=("wrong_id", "adhoc", "unsigned")):
    # Only our unique current-user job. Nothing is installed in LaunchAgents.
    domain, label = launch_target(work)
    before = run_bounded(["/bin/launchctl", "print", domain + "/" + label], 5)
    if before.code != 113 or before.limited or before.timed_out or before.pipe_failed:
        raise PreflightError("job_namespace_not_proven_empty")
    with (work / "job.plist").open("xb") as output:
        plistlib.dump({"Label": label, "ProgramArguments": [str(binaries["server"]), "server", label, client_req],
                      "MachServices": {label + "." + lane: True for lane in ("guard", "observe", "relay")},
                      "StandardOutPath": str(work / "server.jsonl"), "StandardErrorPath": "/dev/null",
                      "Umask": 0o077, "ProcessType": "Background", "ExitTimeOut": 3}, output)
    with (work / "job-owned.json").open("x", encoding="utf-8") as output:
        json.dump({"domain": domain, "label": label}, output)
    checked(run_bounded, ["/bin/launchctl", "bootstrap", domain, str(work / "job.plist")], "job_bootstrap")
    cases = []
    report["cases"] = cases

    def attempt(name, fixture, mode, nonce=None):
        nonce = nonce or uuid.uuid4().hex
        result = run_bounded([str(binaries[fixture]), mode, label, server_req, nonce], 8)
        case = {"name": name, "nonce": nonce, "client_result": client_result(result), "exit_code": result.code}
        cases.append(case)
        return case

    attempt("expected_guard", "expected", "guard")
    attempt("expected_observer", "expected", "observe")
    for fixture in negative_fixtures:
        attempt(fixture + "_guard", fixture, "guard")
        attempt(fixture + "_observer", fixture, "observe")
    attempt("expected_after_negatives", "expected", "guard")
    origin = attempt("origin_endpoint_stored", "expected", "put")
    if origin["client_result"] == "stored" and origin["exit_code"] == 0:
        # run_bounded has reaped origin. A distinct exec obtains its forwarded
        # endpoint via the synthetic relay, then connects to the guarded listener.
        attempt("forwarded_endpoint_after_origin_exit", "expected", "take", origin["nonce"])
    attempt("fresh_process", "expected", "guard")
    report["cases"] = conclude(cases, read_events(work / "server.jsonl"))
    report["native_xpc_matrix"] = "partial_observations_collected"
    report["full_feasibility_outcome"] = "INCOMPLETE"
    report["different_certified_signer"] = "not_run_second_identity_not_selected"
    report["stale_connection_reuse"] = "not_run_endpoint_after_origin_exit_is_separate"
    report["exact_instance_binding"] = "not_proven"
    if any(c["verdict"] == "signer_only_insufficient" for c in cases):
        report["signer_only_binding"] = "insufficient_observed"
    report["outcome"] = "incomplete_full_feasibility_matrix"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--cleanup", action="store_true")
    args = parser.parse_args()
    if sys.platform != "darwin":
        return 2
    package = args.package.resolve()
    work = args.work
    validate_work(work, package)
    if args.cleanup:
        return 0 if cleanup(work) in ("registration_removed", "not_registered") else 2
    os.environ["TMPDIR"] = str(work.resolve())
    report = {"schema": 1, "scope": SCOPE, "collected_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "native_xpc_matrix": "not_run", "exact_instance_binding": "not_tested",
              "product_minimum_os": "unconfirmed", "probe_compile_target": "12.0",
              "private_key_use": "not_requested", "production_changed": False, "m8_complete": False}
    code = 0

    def interrupted(*_):
        raise KeyboardInterrupt

    for name in ("SIGINT", "SIGTERM", "SIGHUP"):
        signal.signal(getattr(signal, name), interrupted)
    try:
        report["source_commit"] = verify_package(package)
        collect(report, package, work)
    except PreflightError as error:
        report.update(outcome="probe_incomplete", diagnostic=str(error))
        code = 2
    except KeyboardInterrupt:
        report.update(outcome="probe_interrupted", diagnostic="interrupted")
        code = 130
    except Exception:
        report.update(outcome="probe_incomplete", diagnostic="internal_error")
        code = 2
    finally:
        try:
            report["temporary_job_cleanup"] = cleanup(work)
        except Exception:
            report["temporary_job_cleanup"] = "cleanup_unconfirmed"
        report["server_process_exit"] = "not_independently_observed"
        if report["temporary_job_cleanup"] not in ("registration_removed", "not_registered"):
            code = 2
        report["runner_exit"] = code
        with (work / "result.json").open("x", encoding="utf-8", newline="\n") as output:
            json.dump(report, output, indent=2, ensure_ascii=True)
            output.write("\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
