import copy
import json
import unittest
import lan_device_recovery as r

RUN = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
PLAN = dict(run_id=RUN, source_commit='a'*40)


def receipt(local, peer, before=False):
    return dict(schema=1, purpose='cryptex-local-canonical-verification', run_id=RUN,
                brain_id=r.BRAIN, local_device_id=local, peer_device_id=peer,
                canonical_set_sha256=r.BASELINE if before else 'b'*64,
                canonical_records=11 if before else 15, canonical_devices=2,
                signatures_and_chains_verified=True, remote_durability_verified=False,
                network_delivery_verified=False, derived_results_verified=False)


def driver(local=r.WINDOWS, peer=r.MAC):
    return dict(schema=1, purpose='cryptex-device-recovery', run_id=RUN, brain_id=r.BRAIN,
                local_device_id=local, peer_device_id=peer, outcome='local_steps_passed', error=None,
                interrupted_component='desktop' if local==r.MAC else 'daemon',
                midstream_bytes_interrupted=False, remote_durability_verified=False)


def report(local, peer):
    steps=['resolve_queued','resolve_received']
    if local==r.MAC:
        steps.insert(0,'resolve_prior_mac')
    return dict(outcome='local_recovery_steps_verified', **PLAN, before=receipt(local,peer,True),
                after=receipt(local,peer), driver=dict(receipt=driver(local,peer),
                stages=r.STAGES.copy(), cleanup_unconfirmed=False,
                observations=[dict(step=s, outcome='committed', code=None, os_code=None)
                              for s in steps]))


class RecoveryEvidenceTests(unittest.TestCase):
    def output(self, value=None, stages=None):
        return '\n'.join([*(json.dumps(dict(recovery_stage=s)) for s in
                            (r.STAGES if stages is None else stages)), json.dumps(value or driver())])

    def test_complete_ordered_evidence_stays_bounded(self):
        parsed = r.parse_driver(self.output(), 0, PLAN, r.WINDOWS, r.MAC)
        self.assertEqual(parsed['stages'], r.STAGES)
        result = r.compare_results(report(r.WINDOWS,r.MAC), report(r.MAC,r.WINDOWS), PLAN)
        self.assertTrue(result['queued_work_process_interruption_recovered'])
        self.assertFalse(result['midstream_bytes_interrupted'])
        self.assertFalse(result['network_power_loss_tested'])
        self.assertFalse(result['remote_protection_claim'])

    def test_completion_alone_nonzero_exit_or_cleanup_failure_cannot_pass(self):
        for output, code in [(self.output(stages=['completed']),0), (self.output(),2),
                             (self.output()+'\nrecovery_cleanup_unconfirmed',0),
                             (self.output()+'\n'+json.dumps(driver()),0)]:
            with self.assertRaises(Exception):
                r.parse_driver(output,code,PLAN,r.WINDOWS,r.MAC)

    def test_failure_stage_is_retained_without_raw_logs(self):
        value=driver();value.update(outcome='incomplete',error='connections')
        parsed=r.parse_driver(self.output(value,r.STAGES[:4])+'\nrecovery_check_refused: recovery_incomplete',
                              2,PLAN,r.WINDOWS,r.MAC)
        self.assertEqual(parsed['receipt']['error'],'connections')
        self.assertEqual(parsed['stages'][-1],'restart_and_receive')
        with self.assertRaises(Exception):
            r.parse_driver(self.output(value,r.STAGES[:4])+'\nprivate note body',2,PLAN,r.WINDOWS,r.MAC)

    def test_mixed_runs_duplicate_keys_unknown_fields_and_claims_refused(self):
        for key,value in [('run_id','bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'),
                          ('local_device_id',r.MAC),('interrupted_component','desktop'),
                          ('remote_durability_verified',True),
                          ('midstream_bytes_interrupted',True),('secret','not allowed'),('schema',True)]:
            changed=driver();changed[key]=value
            with self.assertRaises(Exception):
                r.parse_driver(self.output(changed),0,PLAN,r.WINDOWS,r.MAC)
        raw=self.output().replace('"schema": 1','"schema": 1, "schema": 1')
        with self.assertRaises(Exception):r.parse_driver(raw,0,PLAN,r.WINDOWS,r.MAC)

    def test_wrong_baseline_missing_note_and_changed_source_refuse(self):
        a,b=report(r.WINDOWS,r.MAC),report(r.MAC,r.WINDOWS)
        for field,key,value in [('before','canonical_set_sha256','c'*64),
                                ('before','canonical_records',10),('after','canonical_records',14),
                                ('after','signatures_and_chains_verified',False)]:
            changed=copy.deepcopy(a);changed[field][key]=value
            with self.assertRaises(Exception):r.compare_results(changed,b,PLAN)
        a['source_commit']='c'*40
        with self.assertRaises(Exception):r.compare_results(a,b,PLAN)

    def test_missing_offline_receipt_prevents_acceptance(self):
        a,b=report(r.WINDOWS,r.MAC),report(r.MAC,r.WINDOWS)
        a['driver']['observations'].pop()
        with self.assertRaisesRegex(r.Refused,'missing_restarted_capture_receipts'):
            r.compare_results(a,b,PLAN)

    def test_different_valid_final_sets_do_not_pass_recovery(self):
        a,b=report(r.WINDOWS,r.MAC),report(r.MAC,r.WINDOWS)
        b['after']['canonical_set_sha256']='c'*64
        result=r.compare_results(a,b,PLAN)
        self.assertFalse(result['same_verified_canonical_set'])
        self.assertFalse(result['queued_work_process_interruption_recovered'])


if __name__ == '__main__': unittest.main()
