import hashlib
import unittest
from windows_lan_receipt import validate,Refused,SESSION,PRODUCT,STAGES

class WindowsReceiptTests(unittest.TestCase):
    def receipt(self):
        return dict(schema=1,purpose='cryptex-windows-persistent-lan-setup',platform='Windows',run_id=SESSION,
                    product_source=PRODUCT,source_commit='a'*40,pairing_authorized=False,network_delivery_verified=False,
                    convergence_verified=False,normal_window_verified=False,initialized_startup_restart=False,
                    outcome='enrolled_local_only',first_note_sha256=hashlib.sha256(('Cryptex LAN acceptance '+SESSION+': Windows first public note.').encode()).hexdigest(),**{s:True for s in STAGES})
    def test_bound_success_does_not_claim_window_or_delivery(self):
        value=self.receipt();self.assertEqual(validate(value,'a'*40),value)
        for key,bad in [('run_id','foreign'),('source_commit','b'*40),('product_source','c'*40),
                         ('normal_window_verified',True),('initialized_startup_restart',True),
                         ('network_delivery_verified',True),('pairing_authorized',True),('first_note_sha256','0'*64)]:
            with self.assertRaises(Refused):validate(dict(value,**{key:bad}),'a'*40)
        for key in STAGES:
            altered=dict(value);del altered[key]
            with self.assertRaises(Refused):validate(altered,'a'*40)
    def test_partial_failure_is_preserved_without_false_success(self):
        value=self.receipt();value.update(outcome='refused_or_incomplete',failed_stage='local_key_enrollment',error='command_failed')
        for key in STAGES-{'fresh_profile_created'}:del value[key]
        del value['first_note_sha256']
        self.assertEqual(validate(value,'a'*40),value)
        with self.assertRaises(Refused):validate(dict(value,error='/private/raw stderr'),'a'*40)
if __name__=='__main__':unittest.main()
