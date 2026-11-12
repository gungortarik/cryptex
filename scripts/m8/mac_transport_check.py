"""Default-off signed native pairing fixture, with exact fixture-key cleanup."""
import json
import os
import re
import selectors
import stat
import subprocess
import time
import uuid

from mac_identity_preflight import PreflightError, darwin_group_is_dead

MARKERS = ("native_managed_transport_pairing=passed",
           "native_managed_pairing_clean_shutdown=passed",
           "native_managed_pairing_restart_idempotent=passed")


def _metadata(path):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size > 8192:
        raise PreflightError("native_pairing_cleanup_metadata_unsafe")
    return json.loads(path.read_text(encoding="utf-8"))


def check_native_pairing(binary, root, run):
    try:
        output = run([binary, "--cryptex-m8-local-content-transport-check", root],
                     "real_native_managed_pairing", 240)
        if not all(marker in output.splitlines() for marker in MARKERS):
            raise PreflightError("native_managed_pairing_marker_missing")
    finally:
        # These are fresh CI-only roots. Never enumerate/adopt the login user's
        # other credentials, and never export keys or full transcripts.
        failures = []
        for role in ("initiator", "responder"):
            profile = root / role
            for kind in ("device", "api"):
                try:
                    if kind == "device":
                        path = profile / "identity/local_device.json"
                        if not path.exists():
                            continue
                        device = str(uuid.UUID(_metadata(path)["device_id"]))
                        service, account = "Cryptex-Dev-DeviceCredentials", "device-event-signing-seed:" + device
                    else:
                        path = profile / "managed-transport/00000000000000000000000000000001/profile.json"
                        if not path.exists():
                            continue
                        credential = _metadata(path)["credential_id"]
                        if not isinstance(credential, str) or not re.fullmatch(r"[0-9a-f]{32}", credential):
                            raise PreflightError("native_pairing_cleanup_credential_invalid")
                        service, account = "Cryptex-Dev-ManagedSyncthing", "api:" + credential
                    run(["/usr/bin/security", "delete-generic-password", "-s", service, "-a", account],
                        "cleanup_native_pairing_" + kind + "_key")
                except (PreflightError, OSError, ValueError, KeyError, TypeError) as error:
                    failures.append(error)
        if failures:
            raise PreflightError("native_pairing_cleanup_failed") from failures[0]
    for marker in MARKERS:
        print(marker)


def loss_group_snapshot(data, driver, group):
    if len(data) > 16384:
        raise PreflightError("native_transport_loss_group_shape")
    try:
        rows = [line.split() for line in data.decode("ascii", errors="strict").splitlines() if line.strip()]
        if not 3 <= len(rows) <= 65 or any(len(row) != 4 or int(row[2]) != group
                or int(row[0]) <= 1 or int(row[1]) < 1 for row in rows):
            raise ValueError("shape")
    except (UnicodeError, ValueError) as error:
        raise PreflightError("native_transport_loss_group_shape") from error
    if not any(int(row[0]) == group and int(row[1]) == driver and "Z" not in row[3] for row in rows):
        raise PreflightError("native_transport_loss_parent_mismatch")


def check_parent_loss(binary, root, run):
    env = os.environ.copy()
    env["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    for key in list(env):
        if key.startswith("DYLD_") or key in ("SDKROOT", "MACOSX_DEPLOYMENT_TARGET", "CPATH",
                "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "OBJC_INCLUDE_PATH", "LIBRARY_PATH"):
            env.pop(key, None)
    process = subprocess.Popen([str(binary), "--m8-local-transport-loss-driver", str(root)],
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               start_new_session=True, bufsize=0, env=env)
    group = None
    stopped = False
    try:
        output = bytearray()
        deadline = time.monotonic() + 55
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while group is None:
                if time.monotonic() >= deadline:
                    raise PreflightError("native_transport_loss_ready_timeout")
                if not selector.select(0.1):
                    continue
                block = os.read(process.stdout.fileno(), 1024)
                if not block or len(output) + len(block) > 8192:
                    raise PreflightError("native_transport_loss_ready_failed")
                output.extend(block)
                for line in output.splitlines():
                    match = re.fullmatch(rb"mac_transport_loss_ready=([0-9]{1,10})", line)
                    if match:
                        group = int(match[1])
                        if group <= 1 or group == process.pid:
                            raise PreflightError("native_transport_loss_group_shape")
        before = subprocess.run(["/bin/ps", "-x", "-g", str(group), "-o", "pid=,ppid=,pgid=,stat="],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=2, env=env, check=False)
        if before.returncode or before.stderr:
            raise PreflightError("native_transport_loss_group_unavailable")
        loss_group_snapshot(before.stdout, process.pid, group)
        # Only our unreaped Popen child is signalled. Group numbers are used
        # for read-only evidence, never for adopting/killing a foreign process.
        process.kill()
        stopped = True
        deadline = time.monotonic() + 8
        while not darwin_group_is_dead(group, env):
            if time.monotonic() >= deadline:
                raise PreflightError("native_transport_parent_loss_cleanup_unresolved")
            time.sleep(0.05)
        process.wait(timeout=5)
        active = root / "transport.active"
        if not re.fullmatch(rb"[0-9a-f]{32}", active.read_bytes()):
            raise PreflightError("native_transport_loss_marker_missing")
        profile = root / "00000000000000000000000000000001"
        names = ("profile.json", "api-binding.json", "native-ready.json", "native-init.json",
                 "native/config.xml", "native/cert.pem", "native/key.pem")
        snapshot = {name: (profile / name).read_bytes() for name in names}
        output = run([binary, "--m8-local-transport-blocked-check", root],
                     "native_transport_unresolved_restart", 45)
        if "mac_transport_unresolved_restart_refused=passed" not in output.splitlines() or snapshot != {
                name: (profile / name).read_bytes() for name in names}:
            raise PreflightError("native_transport_unresolved_restart_mutated_profile")
        print("mac_transport_daemon_loss_group_exit=passed")
        print("mac_transport_unresolved_restart_refused=passed")
    finally:
        if not stopped:
            process.kill()
        process.wait(timeout=10)
        process.stdout.close()
        metadata = root / "00000000000000000000000000000001/profile.json"
        if metadata.exists():
            credential = _metadata(metadata)["credential_id"]
            if not isinstance(credential, str) or not re.fullmatch(r"[0-9a-f]{32}", credential):
                raise PreflightError("native_transport_loss_cleanup_metadata_unsafe")
            run(["/usr/bin/security", "delete-generic-password", "-s", "Cryptex-Dev-ManagedSyncthing",
                 "-a", "api:" + credential], "cleanup_native_transport_parent_loss_key")
