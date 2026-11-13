"""Two real supervisor losses under explicit fixture-only group containment."""
import os
import re
import selectors
import signal
import stat
import subprocess
import time

from mac_identity_preflight import PreflightError, darwin_group_is_dead, run_bounded
from mac_transport_check import _cleanup_api

MARKERS = ("mac_transport_combined_supervisors_dead_engine_live=passed",
           "mac_transport_combined_restart_refused_unchanged=passed",
           "mac_transport_combined_guardian_cleanup=passed",
           "mac_transport_combined_lost_output_cleanup=passed")


def capture_fixed(path, limit=262144):
    def stamp(info):
        return (info.st_dev, info.st_ino, info.st_mode, info.st_size, info.st_mtime_ns)
    flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_CLOEXEC', 0) | getattr(os, 'O_NONBLOCK', 0)
    with os.fdopen(os.open(path, flags), 'rb') as file:
        info = os.fstat(file.fileno())
        if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > limit
                or info.st_uid != os.geteuid() or not stat.S_ISREG(path.lstat().st_mode)):
            raise PreflightError("combined_loss_snapshot_unsafe")
        data = file.read(limit + 1)
        if len(data) != info.st_size or stamp(os.fstat(file.fileno())) != stamp(info) or stamp(path.lstat()) != stamp(info):
            raise PreflightError("combined_loss_snapshot_changed")
    return stamp(info), data


def read_fixed(path, limit=262144):
    return capture_fixed(path, limit)[1]


def guardian_identity(root):
    match = re.fullmatch(rb"([0-9]{1,10}) ([0-9]{1,10})", read_fixed(root / "guardian.ready", 32))
    if not match:
        raise PreflightError("combined_loss_guardian_shape")
    group, guard = (int(value) for value in match.groups())
    if min(group, guard) <= 1 or group == guard:
        raise PreflightError("combined_loss_guardian_shape")
    return group, guard


def live_engine_snapshot(data, group, guard, driver=None, engines=None, helper_exited=True):
    """Read-only evidence; none of these identifiers is ever a signal target."""
    if len(data) > 16384:
        raise PreflightError("combined_loss_group_shape")
    try:
        rows = [line.split() for line in data.decode("ascii").splitlines() if line.strip()]
        parsed = [(int(pid), int(parent), int(pgid), state) for pid, parent, pgid, state in rows]
        if not 3 <= len(parsed) <= 65 or len({r[0] for r in parsed}) != len(parsed):
            raise ValueError("shape")
        if any(pid <= 1 or parent < 1 or pgid != group or not re.fullmatch(r"[A-Za-z0-9+<>-]+", state)
               for pid, parent, pgid, state in parsed):
            raise ValueError("shape")
    except (UnicodeError, ValueError) as error:
        raise PreflightError("combined_loss_group_shape") from error
    if not any(pid == guard and "Z" not in state for pid, _, _, state in parsed):
        raise PreflightError("combined_loss_guardian_not_live")
    leaders = [r for r in parsed if r[0] == group]
    if driver is not None and not any(parent == driver and ("Z" in state) == helper_exited for _, parent, _, state in leaders):
        raise PreflightError("combined_loss_helper_not_exited")
    if helper_exited and any("Z" not in row[3] for row in leaders):
        raise PreflightError("combined_loss_helper_not_exited")
    living = {pid for pid, _, _, state in parsed if pid not in (group, guard) and "Z" not in state}
    if len(living) < 2 or (engines is not None and not engines.issubset(living)):
        raise PreflightError("combined_loss_engines_not_live")
    return living


def profile_snapshot(root):
    prefix = "00000000000000000000000000000001/"
    names = ["transport.active", "transport.marker", "ports.json"]
    names += [prefix + name for name in ("profile.json", "api-binding.json", "native-ready.json",
        "native-init.json", "native-generation-started", "native-generation-complete", "api-ready",
        "native/config.xml", "native/cert.pem", "native/key.pem")]
    return {name: capture_fixed(root / name) for name in names}


def verify_exclusion(binary, root, run, engine_live, before):
    if profile_snapshot(root) != before:
        raise PreflightError("combined_loss_transition_mutated_profile")
    if not re.fullmatch(rb"[0-9a-f]{32}", before["transport.active"][1]):
        raise PreflightError("combined_loss_marker_shape")
    def credential():
        output = run([binary, "--m8-local-transport-combined-integrity-check", root],
                     "combined_loss_existing_credential", 30)
        if "mac_transport_combined_credential_continuity=passed" not in output.splitlines():
            raise PreflightError("combined_loss_credential_unverified")
    credential()
    for _ in range(2):
        engine_live()
        output = run([binary, "--m8-local-transport-blocked-check", root],
                     "combined_loss_unresolved_restart", 45)
        if "mac_transport_unresolved_restart_refused=passed" not in output.splitlines():
            raise PreflightError("combined_loss_restart_not_refused")
        if profile_snapshot(root) != before:
            raise PreflightError("combined_loss_restart_mutated_profile")
        credential()
        engine_live()
    return before


def _check_case(binary, root, run, *, lost_output):
    root = root.resolve()
    env = os.environ.copy()
    env["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    for key in list(env):
        if key.startswith("DYLD_") or key in ("SDKROOT", "MACOSX_DEPLOYMENT_TARGET", "CPATH",
                "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "OBJC_INCLUDE_PATH", "LIBRARY_PATH"):
            env.pop(key, None)
    process = subprocess.Popen([str(binary), "--m8-local-transport-combined-loss-driver", str(root)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        start_new_session=True, bufsize=0, env=env)
    group = None
    bound = False
    stopped = False
    passed = False
    try:
        deadline = time.monotonic() + 55
        def ready_line(prefix):
            output = bytearray()
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                while True:
                    if time.monotonic() >= deadline:
                        raise PreflightError("combined_loss_ready_timeout")
                    if not selector.select(0.1):
                        continue
                    block = os.read(process.stdout.fileno(), 1024)
                    if not block or len(output) + len(block) > 8192:
                        raise PreflightError("combined_loss_ready_failed")
                    output.extend(block)
                    for line in output.splitlines():
                        match = re.fullmatch(prefix + rb"=([0-9]{1,10})", line)
                        if match:
                            return int(match[1])
        group = ready_line(b"mac_transport_combined_prepared")
        actual_group, guard = guardian_identity(root)
        if group != actual_group or group == process.pid or guard == process.pid:
            raise PreflightError("combined_loss_group_mismatch")
        bound = True
        direct = run_bounded([str(binary), "--m8-local-transport-fixture-guardian", "a" * 32, str(root)], 15)
        if (direct.code != 2 or direct.timed_out or direct.limited or direct.pipe_failed
                or "mac_transport_Policy" not in direct.output.splitlines()):
            raise PreflightError("combined_loss_direct_guardian_not_refused")

        def snapshot():
            data = subprocess.run(["/bin/ps", "-x", "-g", str(group), "-o", "pid=,ppid=,pgid=,stat="],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=2, env=env, check=False)
            if data.returncode or data.stderr:
                raise PreflightError("combined_loss_group_unavailable")
            return data.stdout

        engines = live_engine_snapshot(snapshot(), group, guard, driver=process.pid, helper_exited=False)
        before = profile_snapshot(root)  # BEFORE either supervisor is killed.
        process.stdin.write(b"X")
        process.stdin.flush()
        if ready_line(b"mac_transport_combined_ready") != group:
            raise PreflightError("combined_loss_group_mismatch")
        live_engine_snapshot(snapshot(), group, guard, driver=process.pid, engines=engines)
        process.kill()  # ONLY the captured, still-unreaped daemon Popen child.
        stopped = True
        if process.wait(timeout=5) != -signal.SIGKILL:
            raise PreflightError("combined_loss_daemon_death_unverified")
        deadline = time.monotonic() + 3
        while not (root / "guardian.parent-lost").exists():
            if time.monotonic() >= deadline:
                raise PreflightError("combined_loss_helper_death_unverified")
            time.sleep(0.01)
        if read_fixed(root / "guardian.parent-lost", 16) != b"lost\n":
            raise PreflightError("combined_loss_helper_death_unverified")
        def engine_live():
            live_engine_snapshot(snapshot(), group, guard, engines=engines)
        engine_live()
        verify_exclusion(binary, root, run, engine_live, before)
        if lost_output:
            # An invalid byte must be refused, but its failed diagnostic write
            # to this now-closed pipe must not bypass own-group cleanup.
            process.stdout.close()
        process.stdin.write(b"!" if lost_output else b"K")
        process.stdin.flush()
        process.stdin.close()
        until = time.monotonic() + 8
        while not darwin_group_is_dead(group, env):
            if time.monotonic() >= until:
                raise PreflightError("combined_loss_guardian_cleanup_unresolved")
            time.sleep(0.05)
        if profile_snapshot(root) != before:
            raise PreflightError("combined_loss_cleanup_mutated_profile")
        passed = True
    finally:
        # EOF is an authenticated guardian's cleanup trigger, not PID adoption.
        # It also has a fixed watchdog if this collector disappears mid-test.
        process.stdin.close()
        if not stopped:
            process.kill()
        process.wait(timeout=10)
        process.stdout.close()
        if not bound:
            group = None
            if (root / "guardian.ready").exists():
                group, _ = guardian_identity(root)  # read-only evidence only
        if group is not None:
            until = time.monotonic() + 8
            while not darwin_group_is_dead(group, env):
                if time.monotonic() >= until:
                    raise PreflightError("combined_loss_cleanup_unresolved")
                time.sleep(0.05)
        elif (root / "00000000000000000000000000000001/profile.json").exists():
            raise PreflightError("combined_loss_cleanup_ownership_unresolved")
        _cleanup_api(binary, root, run)
    if not passed:
        raise PreflightError("combined_loss_case_incomplete")


def check_combined_loss(binary, root, run):
    # Fresh independent profiles, no repeat of accepted identity/pairing flows.
    root.mkdir(mode=0o700)
    _check_case(binary, root / "normal", run, lost_output=False)
    _check_case(binary, root / "lost-output", run, lost_output=True)
    for marker in MARKERS:
        print(marker)
