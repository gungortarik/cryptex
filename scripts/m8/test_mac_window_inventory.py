"""New user-wide observer failures and real out-of-group Darwin children."""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

from mac_identity_preflight import CommandResult, PreflightError
from mac_process_inventory import ImageInventory, compile_probe, parse_inventory, shutdown_ok
from test_mac_window import clean


class InventoryTests(unittest.TestCase):
    def test_parser_refuses_truncation_duplicates_and_ambiguous_fields(self):
        for text in ('', '1 0 1 10 0 0\n1 0 1 10 0 0', '1 0 1 10 0 9',
                     '1 0 1 10 1000000 0', '1 0 1 10 0 private', '1 '*40000):
            with self.subTest(text=text[:25]), self.assertRaises(PreflightError):
                parse_inventory(text)

    def test_exec_and_reparent_do_not_hide_captured_instance_but_pid_reuse_is_distinct(self):
        with tempfile.TemporaryDirectory() as tmp:
            desktop = Path(tmp) / 'desktop'
            desktop.touch()
            output = '99 1 99 99 0 0'
            inventory = ImageInventory('/probe', desktop, lambda *_: CommandResult(0, output))
            inventory.establish_baseline()
            output = '10 1 10 100 0 1\n11 10 10 101 0 2\n12 11 12 102 0 2\n13 12 12 103 0 3'
            inventory.sample(10)
            self.assertTrue(inventory.topology_seen)
            output = '13 1 12 103 0 0'  # same instance, different executable
            with self.assertRaisesRegex(PreflightError, 'still_live'):
                inventory.require_empty()
            output = '13 1 13 200 0 0'  # reused PID, unrelated birth
            inventory.require_empty()
            output = '21 1 21 201 0 3'  # previously unseen target descendant
            with self.assertRaisesRegex(PreflightError, 'still_live'):
                inventory.require_empty()
            inventory.runner = lambda *_: CommandResult(2, '')
            with self.assertRaisesRegex(PreflightError, 'unavailable'):
                inventory.require_empty()

    def test_only_frozen_pre_spawn_opaque_births_can_be_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            desktop = Path(tmp) / 'desktop'
            desktop.touch()
            output = '99 1 99 99 0 4'
            inventory = ImageInventory('/probe', desktop, lambda *_: CommandResult(0, output))
            with self.assertRaisesRegex(PreflightError, 'baseline_missing'):
                inventory.require_empty()
            inventory.establish_baseline()
            inventory.require_empty()
            with self.assertRaisesRegex(PreflightError, 'already_frozen'):
                inventory.establish_baseline()
            for unknown in ('99 1 99 100 0 4', '100 1 100 99 0 4'):
                output = unknown  # reused PID or new unknown birth must fail
                with self.assertRaisesRegex(PreflightError, 'path_unavailable'):
                    inventory.require_empty()
            output = '99 1 99 99 0 3'  # former baseline instance execs a target
            with self.assertRaisesRegex(PreflightError, 'still_live'):
                inventory.require_empty()
            output = '99 1 99 99 0 4'
            with self.assertRaisesRegex(PreflightError, 'path_unavailable'):
                inventory.require_empty()

    def test_second_census_cannot_expand_baseline_and_known_targets_refuse(self):
        with tempfile.TemporaryDirectory() as tmp:
            desktop = Path(tmp) / 'desktop'
            desktop.touch()
            outputs = iter(('99 1 99 99 0 4', '99 1 99 99 0 4\n100 1 100 100 0 4'))
            inventory = ImageInventory('/probe', desktop, lambda *_: CommandResult(0, next(outputs)))
            with self.assertRaisesRegex(PreflightError, 'path_unavailable'):
                inventory.establish_baseline()
            self.assertEqual(inventory.baseline_opaque, frozenset({(99, 99, 0)}))
            for role in (1, 2, 3):
                inventory = ImageInventory('/probe', desktop,
                                           lambda *_: CommandResult(0, f'99 1 99 99 0 {role}'))
                with self.assertRaisesRegex(PreflightError, 'still_live'):
                    inventory.establish_baseline()

    def test_group_cleanup_never_substitutes_for_independent_natural_absence(self):
        good = dict(clean(), independent_helper_topology_observed=True,
                    user_wide_absent_before_group_cleanup=True, user_wide_quiet_after_exit=True)
        self.assertTrue(shutdown_ok(good))
        for key in ('natural_exit', 'independent_helper_topology_observed',
                    'user_wide_absent_before_group_cleanup', 'user_wide_quiet_after_exit'):
            self.assertFalse(shutdown_ok(dict(good, **{key: False})))


FIXTURE = r'''
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
int main(int argc, char **argv) {
    if (argc != 4) return 2;
    int stage = atoi(argv[2]);
    if (stage == 2 && setsid() < 0) return 3;
    if (stage == 3) { char b; return read(0, &b, 1) < 0 ? 4 : 0; }
    char path[4096], next[8];
    snprintf(path, sizeof(path), "%s/%s", argv[1], stage == 2 ? "Contents/Resources/transport/syncthing" : "Contents/MacOS/cryptexd");
    snprintf(next, sizeof(next), "%d", stage+1);
    pid_t child = fork();
    if (child == 0) { execl(path, path, argv[1], next, argv[3], NULL); _exit(5); }
    if (child < 0) return 6;
    if (stage == 0 && argv[3][0] == '1') { sleep(2); return 0; }
    int status; return waitpid(child, &status, 0) == child && WIFEXITED(status) ? WEXITSTATUS(status) : 7;
}
'''


@unittest.skipUnless(sys.platform == 'darwin', 'real Darwin libproc required')
class NativeInventoryTests(unittest.TestCase):
    def test_real_live_birth_with_injected_path_error_is_opaque_not_absent(self):
        with tempfile.TemporaryDirectory(prefix='cx-inventory-opaque-') as temporary:
            root = Path(temporary).resolve()
            desktop = root / 'desktop'
            desktop.touch()
            probe = root / 'fixture-probe'
            source = root / 'fixture-probe.c'
            # Injection exists only in this test translation unit. The shipped
            # observer has no environment hooks or alternate libproc providers.
            source.write_text('''#include <libproc.h>
#include <errno.h>
#include <stdlib.h>
static int fixture_pidpath(int pid, void *buffer, unsigned int size) {
    const char *target = getenv("CX_INVENTORY_TEST_PID");
    if (target && pid == atoi(target)) {
        errno = atoi(getenv("CX_INVENTORY_TEST_ERRNO"));
        return 0;
    }
    return proc_pidpath(pid, buffer, size);
}
#define proc_pidpath fixture_pidpath
#include "''' + str(Path(__file__).with_name('mac_process_inventory.c')) + '"\n', encoding='utf-8')
            subprocess.run(['/usr/bin/xcrun', 'clang', '-Wall', '-Wextra', '-Werror',
                            source, '-o', probe], check=True)
            children = []
            try:
                def child():
                    process = subprocess.Popen(['/bin/cat'], stdin=subprocess.PIPE,
                                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    children.append(process)
                    return process
                preexisting = child()
                with patch.dict('os.environ', CX_INVENTORY_TEST_PID=str(preexisting.pid),
                                CX_INVENTORY_TEST_ERRNO='2'):
                    inventory = ImageInventory(probe, desktop)
                    inventory.establish_baseline()
                    self.assertIn(preexisting.pid, {birth[0] for birth in inventory.baseline_opaque})
                    inventory.require_empty()
                    self.assertIsNone(preexisting.poll())  # opaque process is still alive
                    with patch.dict('os.environ', CX_INVENTORY_TEST_ERRNO='1'):
                        with self.assertRaisesRegex(PreflightError, 'unavailable'):
                            inventory.require_empty()
                    newcomer = child()
                    with patch.dict('os.environ', CX_INVENTORY_TEST_PID=str(newcomer.pid)):
                        with self.assertRaisesRegex(PreflightError, 'path_unavailable'):
                            inventory.require_empty()
            finally:
                for process in children:
                    process.stdin.close()
                    process.wait(timeout=10)

    def exercise(self, orphan):
        with tempfile.TemporaryDirectory(prefix='cx-inventory-test-') as temporary:
            root = Path(temporary).resolve()
            desktop = root / 'Contents/MacOS/cryptex-desktop'
            engine = root / 'Contents/Resources/transport/syncthing'
            desktop.parent.mkdir(parents=True)
            engine.parent.mkdir(parents=True)
            source = root / 'fixture.c'
            source.write_text(FIXTURE, encoding='utf-8')
            subprocess.run(['/usr/bin/xcrun', 'clang', source, '-o', desktop], check=True)
            shutil.copy2(desktop, desktop.with_name('cryptexd'))
            shutil.copy2(desktop, engine)
            inventory = ImageInventory(compile_probe(root), desktop)
            inventory.establish_baseline()
            child = subprocess.Popen([desktop, root, '0', str(int(orphan))], stdin=subprocess.PIPE,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                     start_new_session=True)
            try:
                deadline = time.monotonic()+10
                while not inventory.topology_seen and time.monotonic() < deadline:
                    inventory.sample(child.pid)
                    time.sleep(0.05)
                self.assertTrue(inventory.topology_seen)
                if orphan:
                    child.wait(timeout=5)
                    with self.assertRaisesRegex(PreflightError, 'still_live'):
                        inventory.require_empty()
            finally:
                # Only fixture-owned pipe controls descendants; no discovered PID signals.
                child.stdin.close()
                child.wait(timeout=10)
                deadline = time.monotonic()+10
                while time.monotonic() < deadline:
                    if inventory.sample()['captured_instances_live'] == 0:
                        break
                    time.sleep(0.05)
                inventory.quiet()

    def test_real_helper_and_engine_in_separate_session_are_observed_and_disappear(self):
        self.exercise(False)

    def test_surviving_reparented_helper_is_not_a_clean_shutdown(self):
        self.exercise(True)


if __name__ == '__main__':
    unittest.main()
