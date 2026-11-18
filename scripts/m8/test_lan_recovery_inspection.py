import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import lan_device_recovery as r
import lan_recovery_inspection as d
from test_lan_device_recovery import driver, receipt

PLAN=dict(run_id=d.TARGET_RUN,source_commit='a'*40)


def output(error=None, observation=None):
    value=dict(schema=1,purpose='cryptex-device-recovery-inspection',run_id=PLAN['run_id'],brain_id=r.BRAIN,
               local_device_id=r.WINDOWS,peer_device_id=r.MAC,outcome='incomplete' if error else 'inspection_completed',
               error=error,capture_invoked=False,settings_updated=False,process_interrupted=False)
    lines=[json.dumps(dict(recovery_stage='inspect_existing_profile'))]
    if observation is not None: lines.append(json.dumps(dict(recovery_observation=observation)))
    if not error: lines.append(json.dumps(dict(recovery_stage='inspection_completed')))
    lines.append(json.dumps(value))
    return '\n'.join(lines)


class InspectionTests(unittest.TestCase):
    def test_search_failure_retains_the_closed_daemon_hold_reason(self):
        plan = dict(PLAN, run_id=d.QUEUED_RUN)
        for reason in ('runtime_busy', 'runtime_policy_conflict', 'unresolved_dependency',
                       'unsupported_semantics', 'v2_semantics_not_integrated',
                       'local_enrollment_required', 'search_worker_unavailable', 'search_hold'):
            note = dict(step='guarded_search',outcome='held',code=reason,os_code=None)
            raw = output('search',note).replace(PLAN['run_id'],d.QUEUED_RUN).replace(
                'cryptex-device-recovery-inspection','cryptex-queued-note-inspection')
            driver = d.parse_inspection(raw,2,plan,r.WINDOWS,r.MAC,queued_note=True)
            self.assertEqual(driver['observations'],[note])
            self.assertEqual(driver['receipt']['outcome'],'incomplete')

    def test_queued_note_requires_matching_origin_and_both_restart_observations(self):
        plan = dict(PLAN, run_id=d.QUEUED_RUN)
        for local, peer in [(r.WINDOWS,r.MAC), (r.MAC,r.WINDOWS)]:
            events = [('connections_read','ready')]
            if local == r.MAC:
                events.extend([('resolve_queued','ok'), ('resolve_queued','committed')])
            events.extend([('queued_before_restart','found'), ('queued_after_restart','found')])
            def raw(items):
                lines = output().splitlines()
                final = json.loads(lines[-1])
                final.update(run_id=plan['run_id'],local_device_id=local,peer_device_id=peer,
                             purpose='cryptex-queued-note-inspection')
                obs = [json.dumps({'recovery_observation':dict(step=s,outcome=o,code=None,os_code=None)})
                       for s,o in items]
                return '\n'.join([lines[0],*obs,lines[-2],json.dumps(final)])
            parsed = d.parse_inspection(raw(events),0,plan,local,peer,queued_note=True)
            self.assertEqual(parsed['receipt']['outcome'],'inspection_completed')
            invalid_sequences = [events[::-1], events+[events[-1]]]
            if local == r.MAC:
                invalid_sequences.append([events[0], events[2], events[1], *events[3:]])
            invalid_sequences.extend(events[:i]+events[i+1:] for i in range(len(events)))
            for missing in invalid_sequences:
                with self.assertRaises(r.Refused):
                    d.parse_inspection(raw(missing),0,plan,local,peer,queued_note=True)
            with self.assertRaises(r.Refused):
                d.parse_inspection(raw(events),0,plan,peer,local,queued_note=True)

    def test_queued_note_does_not_accept_the_old_eleven_record_set(self):
        plan = dict(PLAN, run_id=d.QUEUED_RUN)
        before = dict(receipt(r.WINDOWS,r.MAC,True), run_id=d.QUEUED_RUN,
                      canonical_records=r.BASELINE_RECORDS, canonical_set_sha256=r.BASELINE)
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp)/'binary'
            exe.write_bytes(b'fixture')
            for final_count in [r.BASELINE_RECORDS,r.BASELINE_RECORDS+1]:
                after = dict(before,canonical_records=final_count)
                if final_count != r.BASELINE_RECORDS:
                    after['canonical_set_sha256']='a'*64
                report = {}
                driver = dict(observations=[],receipt=dict(outcome='inspection_completed'))
                with patch.object(d,'verify',side_effect=[before,after]), patch.object(d,'parse_inspection',return_value=driver):
                    def execute(args,timeout):
                        self.assertEqual(args[1],'--cryptex-inspect-queued-note')
                        return 0,'fixture'
                    def run():
                        d.collect(Path(tmp),plan,'Windows',report,
                                  prepare=lambda *_:(exe,exe,Path(tmp),execute),queued_note=True)
                    if final_count == r.BASELINE_RECORDS:
                        with self.assertRaisesRegex(r.Refused,'queued_note_set_inconsistent'):
                            run()
                    else:
                        run()
                        self.assertTrue(report['queued_note_verified_after_restart'])
    def test_start_only_packet_requires_its_own_receipt_and_settings_observation(self):
        note = dict(step='connections_read', outcome='ready', code=None, os_code=None)
        raw = output(observation=note)
        with self.assertRaises(r.Refused):
            d.parse_inspection(raw, 0, PLAN, r.WINDOWS, r.MAC, start_only=True)
        purpose = 'cryptex-connection-start-inspection'
        raw = raw.replace('cryptex-device-recovery-inspection', purpose)
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp)/'binary'
            binary.write_bytes(b'fixture')
            baseline = dict(receipt(r.WINDOWS, r.MAC, True), run_id=PLAN['run_id'])
            calls = []
            def execute(args, timeout):
                calls.append((args[1], timeout))
                return 0, raw if len(calls)==2 else json.dumps(baseline)
            report = {}
            d.collect(Path(tmp), PLAN, 'Windows', report, prepare=lambda *_:(binary,binary,Path(tmp),execute), start_only=True)
            self.assertEqual(calls, [('replication',90),('--cryptex-inspect-connection-start',300),('replication',90)])
            self.assertTrue(report['native_inspection_completed'])
            self.assertTrue(report['same_local_set_before_after'])
        with self.assertRaises(r.Refused):
            d.parse_inspection(output().replace('cryptex-device-recovery-inspection', purpose), 0,
                               PLAN, r.WINDOWS, r.MAC, start_only=True)

    def test_current_failure_is_bound_to_exact_settings_reply(self):
        def parse(stage='exchange', code=604, detail=5, reason='settings_start', before=False):
            note = dict(step='connections_read', outcome='error', code=reason, os_code=None)
            lines = output('connections', note).splitlines()
            lines.insert(1 if before else 2, json.dumps({'current_start_failure': dict(stage=stage, code=code, detail=detail)}))
            return d.parse_inspection('\n'.join(lines), 2, PLAN, r.WINDOWS, r.MAC)
        current = parse()
        self.assertIn('guncel_hata=exchange/604/process.platform alt_kod=5', d.support_line(dict(os='Windows', driver=current)))
        self.assertEqual(parse(stage='monitor', code=3, detail=None)['current_start_failure']['label'], 'worker.StatusUnavailable')
        self.assertEqual(parse(code=402, detail=None, reason='settings_start_rest_deadline')['current_start_failure']['label'], 'rest.Deadline')
        for invalid in [dict(before=True), dict(code=402), dict(reason='settings_authority'),
                        dict(stage='private'), dict(code=999), dict(detail='private')]:
            with self.assertRaises(r.Refused):
                parse(**invalid)

    def test_only_current_settings_response_can_attribute_402(self):
        for code in ('settings_start', 'settings_start_rest_deadline', 'settings_authority'):
            note = dict(step='connections_read', outcome='error', code=code, os_code=None)
            driver = d.parse_inspection(output('connections', note), 2, PLAN, r.WINDOWS, r.MAC)
            for retained_code in (None, 402, 403):
                retained = ({'status': 'observed', 'value': {'code': retained_code}}
                            if retained_code is not None else {'status': 'unavailable'})
                line = d.support_line(dict(driver=driver, os='Windows', run_id=PLAN['run_id'],
                                          windows_transport={'first_worker_failure': retained}))
                self.assertEqual('bu_baslatmada_402=dogrulandi' in line, code == 'settings_start_rest_deadline')
                self.assertEqual('bu_baslatmada_402=belirlenemedi' in line, code == 'settings_start')
                self.assertEqual('saklanan_ilk_hata=402' in line, retained_code == 402)

    def test_only_windows_transport_start_collects_passive_evidence(self):
        from unittest.mock import Mock
        root = Path('fixture')
        report = {}
        collector = Mock(return_value={'outcome': 'observed'})
        for system, code in [('Darwin', 'settings_start'), ('Windows', 'settings_authority'),
                             ('Windows', 'settings_start')]:
            evidence = dict(observations=[dict(step='connections_read', outcome='error', code=code)])
            d.include_windows_start_evidence(report, root, system, evidence, collector)
        collector.assert_called_once_with(root)
        self.assertEqual(report['windows_transport'], {'outcome': 'observed'})
        collector.side_effect = OSError('private path')
        d.include_windows_start_evidence(report, root, 'Windows', evidence, collector)
        self.assertEqual(report['windows_transport'], {'outcome': 'unavailable'})

    def failed_recovery(self, tmp, cleanup=False, bad_digest=False, bad_inspection=False):
        binary = Path(tmp)/'binary'
        binary.write_bytes(b'fixture')
        calls = []
        before = dict(receipt(r.WINDOWS, r.MAC, True), run_id=PLAN['run_id'])
        failure = dict(driver(), outcome='incomplete', error='connections', run_id=PLAN['run_id'])
        native = json.dumps({'recovery_stage': 'start_existing_profile'}) + '\n' + json.dumps(failure)
        if cleanup:
            native += '\nrecovery_cleanup_unconfirmed'
        observation = dict(step='connections_read', outcome='error', code='settings_authority', os_code=None)
        def execute(args, timeout):
            calls.append(args[1])
            if len(calls) == 1:
                return 0, json.dumps(before)
            if len(calls) == 2:
                return 2, native
            if len(calls) == 3:
                return (2, 'private data') if bad_digest else (0, json.dumps(before))
            return 2, 'private data' if bad_inspection else output('connections', observation)
        report = {}
        with patch.object(r, 'prepare', return_value=(binary, binary, Path(tmp), execute)) as prepare:
            with self.assertRaisesRegex(r.Refused, 'native_steps_incomplete'):
                r.collect(Path(tmp), PLAN, 'Windows', report)
        prepare.assert_called_once()
        self.assertEqual(report['driver']['receipt']['error'], 'connections')
        self.assertNotIn('private data', json.dumps(report))
        self.assertNotIn('local_recovery_steps_verified', report.values())
        return calls, report

    def test_recovery_failure_collects_digest_then_read_only_inspection_in_same_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            calls, report = self.failed_recovery(tmp)
        self.assertEqual(calls, ['replication', '--cryptex-device-recovery-check', 'replication',
                                 '--cryptex-device-recovery-inspect'])
        self.assertEqual(report['inspection_status'], 'incomplete')
        self.assertEqual(report['after'], report['before'])
        summary = d.support_line(dict(report, os='Windows', run_id=PLAN['run_id'], driver=report['inspection']))
        self.assertIn('connection=settings_authority', summary)
        self.assertIn('first=connections_read/settings_authority', summary)

    def test_unsafe_shutdown_or_unverifiable_set_never_starts_inspection(self):
        for options, status, call_count in [
                (dict(cleanup=True), 'cleanup_unconfirmed', 2),
                (dict(bad_digest=True), 'stopped_profile_verification_refused', 3),
                (dict(bad_inspection=True), 'inspection_evidence_unavailable', 4)]:
            with self.subTest(status=status), tempfile.TemporaryDirectory() as tmp:
                calls, report = self.failed_recovery(tmp, **options)
                self.assertEqual(len(calls), call_count)
                self.assertEqual(report['inspection_status'], status)
                self.assertNotIn('inspection', report)

    def test_first_startup_failure_is_retained_and_cannot_pass(self):
        note=dict(step='brain_read',outcome='error',code='brain_operation_mismatch',os_code=None)
        parsed=d.parse_inspection(output('identity',note),2,PLAN,r.WINDOWS,r.MAC)
        self.assertEqual(parsed['observations'],[note])
        self.assertEqual(parsed['receipt']['error'],'identity')
        with self.assertRaises(r.Refused): d.parse_inspection(output('identity',note),0,PLAN,r.WINDOWS,r.MAC)
        with self.assertRaises(r.Refused): d.parse_inspection(output(None,note),0,PLAN,r.WINDOWS,r.MAC)

    def test_typed_runtime_and_capture_failures_survive_collection(self):
        for step, code in [('brain_read','runtime_rebuild_required'),
                           ('resolve_queued','receipt_missing'), ('guarded_search','runtime_policy_conflict'),
                           ('guarded_search','invalid_query'), ('connections_read','settings_authority'),
                           ('connections_read','settings_profile_mismatch')]:
            note=dict(step=step,outcome='error',code=code,os_code=None)
            parsed=d.parse_inspection(output('capture_uncertain',note),2,PLAN,r.WINDOWS,r.MAC)
            self.assertEqual(parsed['observations'],[note])

    def test_mutation_claims_wrong_run_unknown_logs_and_duplicate_keys_refused(self):
        for raw in [output().replace('"capture_invoked": false','"capture_invoked": true'),
                    output().replace(PLAN['run_id'],'b'*36),output()+'\nprivate memory body',
                    output().replace('"schema": 1','"schema": 1, "schema": 1')]:
            with self.assertRaises(Exception): d.parse_inspection(raw,0,PLAN,r.WINDOWS,r.MAC)
        for code in ['private body','/Users/person/keychain']:
            note=dict(step='brain_read',outcome='error',code=code,os_code=None)
            with self.assertRaises(r.Refused): d.parse_inspection(output('identity',note),2,PLAN,r.WINDOWS,r.MAC)

    def test_failed_native_inspection_still_verifies_changed_post_run_set_without_repeating_capture(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary=Path(tmp)/'binary';binary.write_bytes(b'fixture')
            calls=[]
            before=receipt(r.WINDOWS,r.MAC,True);before['run_id']=PLAN['run_id']
            after=dict(before,canonical_records=11,canonical_set_sha256='d'*64)
            def execute(args,timeout):
                calls.append([str(v) for v in args])
                if len(calls)==1:return 0,json.dumps(before)
                if len(calls)==2:return 2,output('identity')
                return 0,json.dumps(after)
            report={}
            d.collect(Path(tmp),PLAN,'Windows',report,
                prepare=lambda *_:(binary,binary,Path(tmp),execute))
            self.assertEqual(report['outcome'],'diagnostics_collected')
            self.assertFalse(report['native_inspection_completed'])
            self.assertFalse(report['same_local_set_before_after'])
            self.assertEqual(report['after']['canonical_records'],11)
            self.assertEqual(len(calls),3)
            self.assertEqual(calls[1][1],'--cryptex-device-recovery-inspect')
            self.assertTrue(all('memory.capture' not in arg for call in calls for arg in call))
            self.assertEqual(calls[0],calls[2])

    def test_unknown_output_still_attempts_final_verification_and_never_retries_driver(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary=Path(tmp)/'binary';binary.write_bytes(b'fixture');calls=[]
            baseline=receipt(r.WINDOWS,r.MAC,True);baseline['run_id']=PLAN['run_id']
            def execute(args,timeout):
                calls.append(args)
                return (2,'unexpected private text') if len(calls)==2 else (0,json.dumps(baseline))
            report={}
            with self.assertRaises(Exception):
                d.collect(Path(tmp),PLAN,'Windows',report,prepare=lambda *_:(binary,binary,Path(tmp),execute))
            self.assertEqual(len(calls),3)
            self.assertEqual(report['before'],report['after'])


if __name__=='__main__': unittest.main()
