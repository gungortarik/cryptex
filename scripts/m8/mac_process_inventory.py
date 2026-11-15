"""Independent user-wide image/instance observation; never a kill authority."""
from pathlib import Path
import time

from mac_identity_preflight import PreflightError, run_bounded, OUTPUT_LIMIT


def parse_inventory(raw):
    if len(raw) > OUTPUT_LIMIT:
        raise PreflightError('inventory_output_limit')
    rows = {}
    for line in raw.splitlines():
        values = line.split()
        if len(values) != 6 or not all(v.isascii() and v.isdecimal() and len(v) <= 20 for v in values):
            raise PreflightError('inventory_invalid')
        pid, parent, group, sec, usec, role = map(int, values)
        if not (0 < pid < 2**31 and 0 <= parent < 2**31 and 0 <= group < 2**31
                and 0 < sec < 2**63 and 0 <= usec < 1000000 and role in range(5)) or pid in rows:
            raise PreflightError('inventory_invalid')
        rows[pid] = (parent, group, sec, usec, role)
    if not rows:
        raise PreflightError('inventory_empty')
    return rows


class ImageInventory:
    def __init__(self, probe, desktop, runner=run_bounded, engine_paths=()):
        self.probe = probe
        desktop = Path(desktop).resolve(strict=True)
        self.targets = [desktop, desktop.with_name('cryptexd'),
                        desktop.parent.parent / 'Resources/transport/syncthing']
        self.targets.extend(Path(path).resolve() for path in engine_paths)
        self.runner = runner
        self.seen = set()
        self.topology_seen = False
        self.last = None
        self.baseline_opaque = None

    def read(self):
        result = self.runner([self.probe, *self.targets], 5)
        if result.code or result.timed_out or result.limited or result.pipe_failed:
            raise PreflightError('inventory_unavailable')
        return parse_inventory(result.output)

    def establish_baseline(self):
        if self.baseline_opaque is not None or self.seen:
            raise PreflightError('inventory_baseline_already_frozen')
        rows = self.read()
        if any(row[4] in (1, 2, 3) for row in rows.values()):
            raise PreflightError('inventory_processes_still_live')
        self.baseline_opaque = frozenset((pid, row[2], row[3]) for pid, row in rows.items() if row[4] == 4)
        self.require_empty()  # A second census may not broaden the frozen set.

    def sample(self, leader=None):
        if self.baseline_opaque is None:
            raise PreflightError('inventory_baseline_missing')
        rows = self.read()
        opaque = {(pid, row[2], row[3]) for pid, row in rows.items() if row[4] == 4}
        if not opaque.issubset(self.baseline_opaque) or opaque & self.seen:
            raise PreflightError('inventory_new_or_tracked_path_unavailable')
        targets = {pid: row for pid, row in rows.items() if row[4] in (1, 2, 3)}
        self.seen.update((pid, row[2], row[3]) for pid, row in targets.items())
        if len(self.seen) > 256:
            raise PreflightError('inventory_instances_limit')
        if leader is not None:
            # Real helper has a distinct session/group; engine can have a monitor.
            daemons = {pid for pid, row in targets.items() if row[4] == 2 and row[0] == leader}
            helpers = {pid for pid, row in targets.items() if row[4] == 2
                       and row[0] in daemons and row[1] == pid and row[1] != leader}
            engines = {pid for pid, row in targets.items() if row[4] == 3 and row[1] in helpers}
            self.topology_seen |= bool(daemons and helpers and engines)
        live = {(pid, row[2], row[3]) for pid, row in rows.items()}
        # A captured process that execs a different image is still unresolved.
        self.last = {'matching_images': len(targets), 'captured_instances_live': len(self.seen & live)}
        return self.last

    def require_empty(self):
        status = self.sample()
        if status['matching_images'] or status['captured_instances_live']:
            raise PreflightError('inventory_processes_still_live')

    def quiet(self):
        # Bounded repeated absence, not a promise of no future respawn.
        for _ in range(3):
            self.require_empty()
            time.sleep(0.2)


def compile_probe(root, runner=run_bounded):
    probe = root / 'process-inventory'
    result = runner(['/usr/bin/xcrun', 'clang', '-Wall', '-Wextra', '-Werror',
                     Path(__file__).with_suffix('.c'), '-o', probe], 60)
    if result.code or result.timed_out or result.limited or result.pipe_failed:
        raise PreflightError('inventory_compile_failed')
    return probe


from mac_window_process import WindowProcess, natural_quit_ok


class ObservedCommand(WindowProcess):
    def __init__(self, arguments, inventory, timeout=240):
        super().__init__(arguments[0], None, timeout)
        self.command = [str(a) for a in arguments]
        self.inventory = inventory
        self.pre_cleanup = None

    def arguments(self):
        return self.command

    def start(self):
        self.inventory.establish_baseline()
        super().start()

    def snapshot(self):
        group = super().snapshot()
        status = self.inventory.sample(self.process.pid)
        if self.done.is_set():
            self.pre_cleanup = status
        return group

    def finish(self, wait=0):
        if self.result is not None:
            return self.result
        result = super().finish(wait)
        result['independent_helper_topology_observed'] = self.inventory.topology_seen
        result['pre_spawn_opaque_instances'] = len(self.inventory.baseline_opaque or ())
        result['user_wide_absent_before_group_cleanup'] = self.pre_cleanup == {
            'matching_images': 0, 'captured_instances_live': 0}
        result['user_wide_quiet_after_exit'] = False
        try:
            self.inventory.quiet()
            result['user_wide_quiet_after_exit'] = True
        except PreflightError:
            result['observer_failed'] = True
        return result


def shutdown_ok(result):
    return natural_quit_ok(result) and all(result.get(k) is True for k in (
        'independent_helper_topology_observed', 'user_wide_absent_before_group_cleanup',
        'user_wide_quiet_after_exit'))


def check_observed_pairing(binary, root, probe, run, report):
    from mac_transport_check import check_native_pairing, MARKERS
    inventory = ImageInventory(probe, binary, engine_paths=[
        root / role / 'managed-transport/package/syncthing' for role in ('initiator', 'responder')])
    process = ObservedCommand([binary, '--cryptex-m8-local-content-transport-check', root], inventory)
    try:
        process.start()
        result = process.finish(wait=245)
    finally:
        # Never signal IDs obtained from the independent inventory. Existing
        # WindowProcess cleanup owns only its captured, unreaped process group.
        report['independent_shutdown'] = process.finish()
    if not shutdown_ok(result):
        raise PreflightError('independent_shutdown_unverified')
    output = bytes(process.output).decode('utf-8', 'strict')
    if not all(marker in output.splitlines() for marker in MARKERS):
        raise PreflightError('native_managed_pairing_marker_missing')
    def completed(args, stage, timeout=30):
        if stage == 'real_native_managed_pairing':
            return output  # Already completed, do not execute a second time.
        inventory.require_empty()  # Exact-key cleanup only after independent absence.
        return run(args, stage, timeout)
    check_native_pairing(binary, root, completed)
    inventory.quiet()
