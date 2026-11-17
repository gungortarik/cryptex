import os
from pathlib import Path
import unittest
from unittest.mock import patch
import tempfile
import json
import zipfile
if os.name=='nt':
    import windows_worker_diagnostic_update as d

@unittest.skipUnless(os.name=='nt','Windows helper imports')
class WaitTests(unittest.TestCase):
    def test_first_valid_failure_finishes_without_sleep(self):
        self.assertTrue(d.wait_for_failure(Path('fixture'),reader=lambda _: {},clock=lambda:0,
            sleep=lambda _:self.fail('unexpected sleep')))

    def test_absence_is_bounded_and_never_reported_as_success(self):
        now=[0]
        def missing(_): raise FileNotFoundError()
        def sleep(n): now[0]+=n
        self.assertFalse(d.wait_for_failure(Path('fixture'),seconds=5,reader=missing,
            clock=lambda:now[0],sleep=sleep))
        self.assertEqual(now[0],5)

    def test_failed_observation_still_emits_redacted_result(self):
        with tempfile.TemporaryDirectory() as folder:
            base=Path(folder)
            (base/'candidate.json').write_text(json.dumps({'product_source':'a'*40}))
            with patch.object(d,'__file__',str(base/'runner.py')), \
                 patch.object(d.update,'main',return_value=True), \
                 patch.object(d,'wait_for_failure',return_value=False), \
                 patch.object(d.diagnostic,'collect',side_effect=RuntimeError('PRIVATE BODY')), \
                 patch.dict(os.environ,{'LOCALAPPDATA':folder}):
                d.main()
            files=list(base.glob('VM-ILK-HATA-SONUC-*.zip'))
            self.assertEqual(len(files),1)
            with zipfile.ZipFile(files[0]) as archive:
                raw=archive.read('result.json')
            self.assertNotIn(b'PRIVATE BODY',raw)
            self.assertEqual(json.loads(raw)['error'],'observation_unavailable')

if __name__=='__main__': unittest.main()
