import json
from pathlib import Path
import tempfile
import unittest

import lan_device_recovery as r
import lan_recovery_inspection as d
from test_lan_device_recovery import receipt

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
    def test_first_startup_failure_is_retained_and_cannot_pass(self):
        note=dict(step='brain_read',outcome='error',code='brain_operation_mismatch',os_code=None)
        parsed=d.parse_inspection(output('identity',note),2,PLAN,r.WINDOWS,r.MAC)
        self.assertEqual(parsed['observations'],[note])
        self.assertEqual(parsed['receipt']['error'],'identity')
        with self.assertRaises(r.Refused): d.parse_inspection(output('identity',note),0,PLAN,r.WINDOWS,r.MAC)
        with self.assertRaises(r.Refused): d.parse_inspection(output(None,note),0,PLAN,r.WINDOWS,r.MAC)

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
