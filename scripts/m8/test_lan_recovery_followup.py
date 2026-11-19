import copy
import json
from pathlib import Path
import tempfile
import unittest

import lan_device_recovery as r
import lan_recovery_followup as f
from test_lan_device_recovery import receipt, driver

PLAN = dict(run_id=r.FOLLOWUP_RUN, source_commit='a' * 40)


def observation(step, outcome):
    return dict(step=step, outcome=outcome, code=None, os_code=None)


def native_output(local, peer):
    final = dict(driver(local, peer), purpose='cryptex-existing-marker-followup',
                 run_id=r.FOLLOWUP_RUN, interrupted_component=None)
    events = []
    for stage in r.FOLLOWUP_STAGES:
        events.append(dict(recovery_stage=stage))
        if stage == 'verify_existing_receipts':
            events.extend(dict(recovery_observation=observation(step, 'committed'))
                          for step in ('resolve_queued', 'resolve_received'))
        if stage == 'offline_restart':
            events.extend(dict(recovery_observation=observation(step, 'found')) for step in f.OFFLINE)
    return '\n'.join(json.dumps(value) for value in [*events, final])


class FollowupTests(unittest.TestCase):
    def collect(self, system):
        local, peer = (r.WINDOWS, r.MAC) if system == 'Windows' else (r.MAC, r.WINDOWS)
        final = dict(receipt(local, peer), run_id=r.FOLLOWUP_RUN)
        before = dict(final, canonical_records=15, canonical_set_sha256=f.OLD_DIGEST[local])
        report = dict(PLAN, purpose='cryptex-existing-marker-followup-result', os=system)
        commands = []
        def execute(args, timeout):
            commands.append(args[1])
            if args[1] == 'replication':
                return 0, json.dumps(before if len(commands) == 1 else final)
            return 0, native_output(local, peer)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / 'fixture'
            exe.write_bytes(b'test')
            f.collect(root, PLAN, system, report, lambda *_: (exe, exe, root, execute))
        self.assertEqual(commands, ['replication', '--cryptex-resume-existing-markers', 'replication'])
        return report

    def test_both_existing_runs_resume_and_require_identical_signed_sets(self):
        windows = self.collect('Windows')
        mac = self.collect('Darwin')
        self.assertTrue(f.compare_results(windows, mac, PLAN))
        mac['after']['canonical_set_sha256'] = 'c'*64
        self.assertFalse(f.compare_results(windows, mac, PLAN))

    def test_incomplete_or_wrong_history_is_refused(self):
        windows = self.collect('Windows')
        for key, field, val in [
            ('before', 'canonical_set_sha256', f.OLD_DIGEST[r.MAC]),
            ('after', 'canonical_records', 15), ('after', 'peer_device_id', r.WINDOWS),
            ('driver', 'cleanup_unconfirmed', True), ('driver', 'observations', []),
        ]:
            changed = copy.deepcopy(windows)
            changed[key][field] = val
            with self.subTest(field=field), self.assertRaises(r.Refused):
                f.check_local(changed, PLAN, r.WINDOWS, r.MAC)
        windows['before'] = dict(windows['after'])
        f.check_local(windows, PLAN, r.WINDOWS, r.MAC)
        windows['before']['canonical_set_sha256'] = 'c'*64
        with self.assertRaises(r.Refused):
            f.check_local(windows, PLAN, r.WINDOWS, r.MAC)

    def test_offline_evidence_requires_reopened_session_and_original_run(self):
        raw = native_output(r.WINDOWS, r.MAC)
        rows = raw.splitlines()
        offline = rows.index(json.dumps(dict(recovery_stage='offline_restart')))
        rows[offline], rows[offline+1] = rows[offline+1], rows[offline]
        for text in ['\n'.join(rows), raw.replace(r.FOLLOWUP_RUN, 'different'),
                     raw.replace('cryptex-existing-marker-followup', 'cryptex-device-recovery')]:
            with self.assertRaises(r.Refused):
                r.parse_driver(text, 0, PLAN, r.WINDOWS, r.MAC, followup=True)
