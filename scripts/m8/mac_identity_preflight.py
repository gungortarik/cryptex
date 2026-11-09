"""Read-only Mac identity prerequisites; never a native XPC authorization pass."""

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import select
import signal
import stat
import subprocess
import sys
import threading
import time


FILES = ("MAC-KIMLIK.command", "mac_identity_preflight.py", "mac_identity_api.c", "MAC-KIMLIK-BASLA.txt")
SYMBOLS = ("xpc_connection_set_peer_code_signing_requirement", "SecCodeCreateWithXPCMessage", "SecCodeCheckValidity")
OUTPUT_LIMIT = 65536


class PreflightError(Exception):
    """Only fixed diagnostic codes may leave the collector."""


@dataclass
class CommandResult:
    code: int
    output: str
    limited: bool = False
    timed_out: bool = False
    pipe_failed: bool = False


def wait_without_reaping(pid, timeout):
    """Keep the leader PID reserved until its owned group has been stopped."""
    if sys.platform == "darwin":
        # Apple's system Python predates os.waitid support on macOS. kqueue's
        # process exit notification does not reap our child or release its PID.
        with select.kqueue() as queue:
            event = select.kevent(pid, filter=select.KQ_FILTER_PROC,
                                 flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
                                 fflags=select.KQ_NOTE_EXIT)
            try:
                events = queue.control([event], 1, timeout)
            except OSError as error:
                if error.errno == errno.ESRCH:
                    return True  # Already exited, but still our unreaped child.
                raise
            if events and events[0].flags & select.KQ_EV_ERROR:
                if events[0].data == errno.ESRCH:
                    return True
                raise OSError(events[0].data, "process notification failed")
            return bool(events)
    deadline = time.monotonic() + timeout
    while True:
        event = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if event is not None:
            return True
        if time.monotonic() >= deadline:
            return False
        time.sleep(min(0.01, max(0, deadline - time.monotonic())))


def run_bounded(args, timeout=20):
    env = os.environ.copy()
    env["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    for key in ("SDKROOT", "MACOSX_DEPLOYMENT_TARGET", "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "OBJC_INCLUDE_PATH", "LIBRARY_PATH"):
        env.pop(key, None)
    for key in list(env):
        if key.startswith("DYLD_"):
            env.pop(key)
    process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               start_new_session=True, bufsize=0, env=env)
    output = bytearray()
    limited = threading.Event()
    pipe_failed = threading.Event()
    lifecycle = threading.Lock()
    reaped = False

    def stop_locked():
        if reaped:
            return
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    def read():
        try:
            while True:
                chunk = process.stdout.read(4096)
                if not chunk:
                    break
                if len(output) + len(chunk) > OUTPUT_LIMIT:
                    limited.set()
                    with lifecycle:
                        stop_locked()
                    break
                output.extend(chunk)
        except (OSError, ValueError):
            pipe_failed.set()
        finally:
            process.stdout.close()

    reader = threading.Thread(target=read, daemon=True)
    reader.start()
    timed_out = False
    try:
        timed_out = not wait_without_reaping(process.pid, timeout)
    finally:
        # Do this even after clean leader exit and EOF: a descendant can close
        # its pipe and otherwise survive. The leader is not reaped until here,
        # so this group ID cannot have been recycled to an unrelated process.
        with lifecycle:
            stop_locked()
            process.wait(timeout=5)
            reaped = True
        reader.join(timeout=2)
        if reader.is_alive():
            timed_out = True  # Never block closing a pipe retained outside our group.
    return CommandResult(process.returncode, bytes(output).decode("utf-8", "replace"), limited.is_set(), timed_out, pipe_failed.is_set())


def checked(run, args, diagnostic, timeout=20):
    result = run(args, timeout)
    if result.limited:
        raise PreflightError(diagnostic + "_output_limit")
    if result.timed_out:
        raise PreflightError(diagnostic + "_deadline")
    if result.pipe_failed:
        raise PreflightError(diagnostic + "_pipe_read_failed")
    if result.code != 0:
        raise PreflightError(diagnostic + "_failed")
    return result.output


def scalar(run, args, pattern, diagnostic):
    value = checked(run, args, diagnostic).strip()
    if not re.fullmatch(pattern, value):
        raise PreflightError(diagnostic + "_invalid_output")
    return value


def identity_counts(text):
    """Validate summary/inventory together; discard names/hashes/team IDs."""
    entries = []
    summaries = []
    teams = set()
    for line in text.splitlines():
        match = re.fullmatch(r'\s*\d+\) ([0-9a-fA-F]{40}) "([^"\r\n]+)"\s*', line)
        summary = re.fullmatch(r"\s*(\d+) valid identities found\s*", line)
        if match:
            entries.append(match.group(1).lower())
            team = re.fullmatch(r"(?:Apple Development|Apple Distribution|Developer ID Application|Mac Developer|3rd Party Mac Developer Application): .+ \(([A-Z0-9]{10})\)", match.group(2))
            if team:
                teams.add(team.group(1))
        elif summary:
            summaries.append(int(summary.group(1)))
        elif line.strip():
            raise PreflightError("identity_inventory_invalid_output")
    if len(summaries) != 1 or summaries[0] != len(entries) or len(entries) > 1024:
        raise PreflightError("identity_inventory_count_mismatch")
    return {"valid_file_keychain_entries": len(entries),
            "distinct_file_keychain_certificates": len(set(entries)),
            "distinct_apple_team_labels_observed": len(teams)}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise PreflightError("package_duplicate_key")
        value[key] = item
    return value


def verify_package(package):
    path = package / "MANIFEST.json"
    if path.is_symlink() or path.stat().st_size > 8192:
        raise PreflightError("package_manifest_invalid")
    manifest = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if (not isinstance(manifest, dict) or set(manifest) != {"schema", "scope", "source_commit", "files"}
            or type(manifest["schema"]) is not int or manifest["schema"] != 1
            or manifest["scope"] != "mac_identity_preflight_only"
            or not isinstance(manifest["source_commit"], str)
            or not re.fullmatch("[0-9a-f]{40}", manifest["source_commit"])
            or not isinstance(manifest["files"], dict) or set(manifest["files"]) != set(FILES)):
        raise PreflightError("package_manifest_invalid")
    for name in FILES:
        path = package / name
        if path.is_symlink() or not path.is_file() or path.stat().st_nlink != 1 or path.stat().st_size > 65536:
            raise PreflightError("package_member_invalid")
        digest = manifest["files"][name]
        if not isinstance(digest, str) or not re.fullmatch("[0-9a-f]{64}", digest) or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise PreflightError("package_hash_mismatch")
    return manifest["source_commit"]


def collect(facts, run, package, work):
    facts["macos_version"] = scalar(run, ["/usr/bin/sw_vers", "-productVersion"], r"\d{1,3}(?:\.\d{1,3}){1,2}", "os_version")
    facts["macos_build"] = scalar(run, ["/usr/bin/sw_vers", "-buildVersion"], r"[A-Za-z0-9.]{1,32}", "os_build")
    facts["architecture"] = scalar(run, ["/usr/bin/uname", "-m"], r"arm64|x86_64", "architecture")
    facts.update(identity_counts(checked(run, ["/usr/bin/security", "find-identity", "-v", "-p", "codesigning"], "identity_inventory")))
    facts["sdk_version"] = scalar(run, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], r"\d{1,3}(?:\.\d{1,3}){1,2}", "sdk_version")
    if int(facts["macos_version"].split(".")[0]) < 12:
        raise PreflightError("probe_os_below_12")
    sdk = checked(run, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
    if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
        raise PreflightError("sdk_path_invalid_output")
    source = package / "mac_identity_api.c"
    binary = work / "api-availability"
    checked(run, ["/usr/bin/xcrun", "--sdk", "macosx", "clang", "-std=c11", "-fblocks",
                  "-Wall", "-Wextra", "-Werror", "-mmacosx-version-min=12.0", "-isysroot", sdk,
                  str(source), "-framework", "Security", "-o", str(binary)], "api_compile", 60)
    facts["api_compile"] = "pass"
    lines = checked(run, [str(binary)], "api_presence").splitlines()
    if len(lines) != len(SYMBOLS) or set(lines) != {name + "=present" for name in SYMBOLS}:
        raise PreflightError("api_presence_invalid_output")
    facts["api_presence"] = "pass"
    facts["outcome"] = "prerequisites_collected"
    facts["signing_selection_needed"] = facts["distinct_file_keychain_certificates"] > 0
    facts["signed_matrix_prerequisites_proven"] = False
    # Enumeration is not a key-use test, different-signer proof or exact-instance proof.
    facts["next"] = "select_existing_signing_identities" if facts["signing_selection_needed"] else "resolve_signing_identity_availability"


def new_report():
    return {"schema": 1, "scope": "mac_identity_preflight_only", "probe_id": "m8-macos-identity-preflight-20261109-v1",
            "collected_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "native_xpc_matrix": "not_run", "exact_instance_binding": "not_tested",
            "product_minimum_os": "unconfirmed", "probe_api_compile_target": "12.0",
            "data_protection_keychain": "not_enumerated", "private_key_use": "not_requested",
            "production_changed": False, "m8_complete": False}


def interrupted(*_):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "darwin":
        print("mac_identity_preflight_requires_macos", file=sys.stderr)
        return 2
    work = args.work
    package = args.package.resolve()
    # A supplied TMPDIR must not redirect native runtime into source control.
    if (work.is_symlink() or not work.is_dir() or not work.name.startswith("cryptex-mac-kimlik.")
            or work.stat().st_uid != os.getuid() or stat.S_IMODE(work.stat().st_mode) & 0o077
            or package == work.resolve() or package in work.resolve().parents
            or any((parent / ".git").exists() for parent in (work.resolve(), *work.resolve().parents))):
        print("unsafe_preflight_work_directory", file=sys.stderr)
        return 2
    for name in ("SIGINT", "SIGTERM", "SIGHUP"):
        signal.signal(getattr(signal, name), interrupted)
    os.environ["TMPDIR"] = str(work.resolve())
    facts = new_report()
    code = 0
    try:
        facts["source_commit"] = verify_package(package)
        collect(facts, run_bounded, package, work)
    except PreflightError as error:
        facts.update(outcome="collection_failed", diagnostic=str(error))
        code = 2
    except KeyboardInterrupt:
        facts.update(outcome="collection_interrupted", diagnostic="interrupted")
        code = 130
    except Exception:
        # Never export raw exception text, paths or command output from private inventory.
        facts.update(outcome="collection_failed", diagnostic="internal_error")
        code = 2
    facts["runner_exit"] = code
    with (work / "result.json").open("x", encoding="utf-8", newline="\n") as output:
        json.dump(facts, output, ensure_ascii=True, indent=2)
        output.write("\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
