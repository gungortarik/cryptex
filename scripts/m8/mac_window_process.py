"""Observe normal window launches without confusing collector cleanup with Quit."""
import json
import os
from pathlib import Path
import signal
import subprocess
import threading
import time

from mac_identity_preflight import (PreflightError, OUTPUT_LIMIT, wait_without_reaping,
                                    darwin_group_is_dead)
from mac_development_diagnostic_owner import progress
from mac_development_startup_check import NATIVE_ERRORS


def window_progress(output):
    """Export fixed diagnostic codes only, never raw error strings or fields."""
    result = progress(output)
    stages = {'content_preflight', 'content', 'start', 'bind', 'ping', 'close',
              'window_reopen', 'window_hide', 'exit_cleanup'}
    errors = {'existing_profile_missing', 'development_content_changed',
              'development_startup_cancelled', 'development_lifecycle_lock',
              'development_owner_lock', 'development_window', 'development_tray'} | NATIVE_ERRORS
    for line in output.splitlines():
        if line.startswith('development_startup: '):
            code = line.removeprefix('development_startup: ')
            if code in errors:
                result.append({'startup_error': code})
        elif line.startswith('{') and len(line) <= 512:
            try:
                value = json.loads(line)
            except (ValueError, RecursionError):
                continue
            name = value.get('developmentStage')
            if isinstance(name, str) and name in stages:
                result.append({'development_stage': name})
    return result


def parse_group(raw, leader):
    if len(raw) > OUTPUT_LIMIT:
        raise PreflightError('process_snapshot_overflow')
    result = {}
    for line in raw.splitlines():
        fields = line.strip().split(None, 4)
        if len(fields) != 5 or not all(x.isdigit() for x in fields[:3]):
            raise PreflightError('process_snapshot_invalid')
        pid, parent, group = map(int, fields[:3])
        if group != leader or pid in result:
            raise PreflightError('process_snapshot_invalid')
        result[pid] = (parent, fields[3], Path(fields[4]).name)
    return result


def natural_quit_ok(result):
    return (result.get('exit_code') == 0 and result.get('natural_exit') is True
            and result.get('same_group_live_descendants_before_cleanup') == 0
            and result.get('group_reaped') is True
            and not any(result.get(k, True) for k in ('timed_out', 'output_limited', 'observer_failed')))


class WindowProcess:
    def __init__(self, binary, home, timeout=600):
        self.env = os.environ.copy()
        self.env.update(HOME=str(home), PATH='/usr/bin:/bin:/usr/sbin:/sbin')
        for key in list(self.env):
            if key.startswith(('DYLD_', 'CRYPTEX_', 'CX_')) or key in (
                    'SDKROOT', 'CPATH', 'LIBRARY_PATH', 'CARGO_MANIFEST_DIR'):
                self.env.pop(key, None)
        self.binary = binary
        self.process = None
        self.reader = None
        self.monitor = None
        self.lock = threading.Lock()
        self.ended = threading.Event()
        self.done = threading.Event()
        self.output = bytearray()
        self.limited = False
        self.failed = False
        self.timed_out = False
        self.natural = False
        self.stopped = False
        self.reaped = False
        self.seen_daemons = set()
        self.deadline = time.monotonic() + timeout
        self.result = None

    def start(self):
        # The collector registers this object before any child or thread starts.
        if self.process is not None:
            raise PreflightError('window_already_started')
        self.process = subprocess.Popen([str(self.binary)], env=self.env, start_new_session=True,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=0)
        try:
            self.reader = threading.Thread(target=self._read, daemon=True)
            self.monitor = threading.Thread(target=self._monitor, daemon=True)
            self.reader.start()
            self.monitor.start()
        except BaseException:
            self.failed = True
            raise  # Registered ownership is retained for the collector's finally.

    def snapshot(self):
        # Never reap the leader here: its PID reserves the process-group ID.
        with self.lock:
            if self.reaped:
                raise PreflightError('process_already_reaped')
            output = subprocess.run(['/bin/ps', '-x', '-g', str(self.process.pid), '-o',
                                     'pid=,ppid=,pgid=,stat=,comm='], env=self.env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=3)
            if output.returncode not in (0, 1) or output.stderr:
                raise PreflightError('process_snapshot_failed')
            return parse_group(output.stdout.decode('utf-8', 'strict'), self.process.pid)

    def _stop_locked(self):
        if self.reaped or self.stopped:
            return
        try:
            os.killpg(self.process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except PermissionError:
            if not darwin_group_is_dead(self.process.pid, self.env):
                raise
        self.stopped = True

    def _read(self):
        try:
            while True:
                chunk = self.process.stdout.read(4096)
                if not chunk:
                    break
                if len(self.output) + len(chunk) > OUTPUT_LIMIT:
                    self.limited = True
                    with self.lock:
                        self._stop_locked()
                    break
                self.output.extend(chunk)
        except (OSError, ValueError):
            self.failed = True
        finally:
            self.process.stdout.close()

    def _monitor(self):
        try:
            while not self.done.is_set():
                group = self.snapshot()
                self.seen_daemons.update(pid for pid, (_, _, name) in group.items() if name == 'cryptexd')
                if wait_without_reaping(self.process.pid, 0.15):
                    self.natural = not self.stopped
                    break
                if time.monotonic() >= self.deadline:
                    self.timed_out = True
                    with self.lock:
                        self._stop_locked()
                    break
        except (OSError, ValueError, PreflightError, subprocess.SubprocessError):
            self.failed = True
        finally:
            self.ended.set()

    def daemon_pin(self):
        if self.ended.is_set() or self.failed:
            raise PreflightError('window_exited_or_observation_failed')
        group = self.snapshot()
        daemons = {pid for pid, (_, state, name) in group.items()
                   if name == 'cryptexd' and not state.startswith('Z')}
        if not daemons:
            raise PreflightError('owned_daemon_not_observed')
        return daemons

    def finish(self, wait=0):
        if self.result is not None:
            return self.result
        if self.process is None:
            self.result = {'group_reaped': True, 'natural_exit': False, 'observer_failed': True}
            return self.result
        self.ended.wait(wait)
        self.done.set()
        if self.monitor is not None and self.monitor.ident is not None:
            self.monitor.join(timeout=5)
            if self.monitor.is_alive():
                raise PreflightError('process_observer_unresolved')
        try:
            group = self.snapshot()
            live = sum(pid != self.process.pid and not state.startswith('Z')
                       for pid, (_, state, _) in group.items())
        except (OSError, ValueError, PreflightError, subprocess.SubprocessError):
            self.failed = True
            live = None
        natural = self.natural and not self.stopped
        with self.lock:
            self._stop_locked()
            # Verify the whole captured group is dead BEFORE releasing its ID.
            deadline = time.monotonic() + 5
            while not darwin_group_is_dead(self.process.pid, self.env):
                if time.monotonic() >= deadline:
                    raise PreflightError('process_group_unresolved')
                time.sleep(0.05)
            self.process.wait(timeout=5)
            self.reaped = True
        if self.reader is not None and self.reader.ident is not None:
            self.reader.join(timeout=3)
            self.failed |= self.reader.is_alive()
        elif self.process.stdout is not None:
            self.process.stdout.close()
        self.result = {'exit_code': self.process.returncode, 'natural_exit': natural,
                       'same_group_live_descendants_before_cleanup': live, 'group_reaped': True,
                       'timed_out': self.timed_out, 'output_limited': self.limited,
                       'observer_failed': self.failed, 'sampled_daemon_count': len(self.seen_daemons),
                       'progress': window_progress(bytes(self.output).decode('utf-8', 'replace'))}
        return self.result
