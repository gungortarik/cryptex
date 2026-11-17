"""Existing-profile candidate installation plus a bounded, read-only observation window."""
import hashlib
import json
import os
from pathlib import Path
import time
import uuid
import zipfile

import windows_vm_update as update
import windows_transport_failure_diagnostic as diagnostic
from windows_pairing_diagnostic import read_json


def wait_for_failure(root, seconds=180, reader=diagnostic.first_failure, clock=time.monotonic, sleep=time.sleep):
    path=root/'managed-transport/00000000000000000000000000000001/worker-first-failure-v1.json'
    deadline=clock()+seconds
    while clock()<deadline:
        try:
            reader(path)
            return True
        except Exception:
            # Collection below reports unavailable/invalid metadata explicitly.
            # A missing diagnostic is never a health or transfer success claim.
            sleep(min(2, max(0,deadline-clock())))
    return False


def main():
    if not update.main():
        print('Kurulum tamamlanmadi. VM-GUNCELLEME-SONUC dosyasini gonder.')
        return
    print('Cryptex normal profille aciliyor. Iki cihaz acik kalsin. Ayarlari degistirme.')
    print('En fazla 3 dakika gozlem yapilacak; hata daha erken kaydedilirse sonuc hemen hazirlanir.')
    root=Path(os.environ['LOCALAPPDATA'])/'Cryptex-Dev/runtime/p0-2'
    found=wait_for_failure(root)
    report=diagnostic.collect(root)
    report['first_failure_seen_during_wait']=found
    report['maximum_wait_seconds']=180
    report['collector_sha256']=hashlib.sha256(Path(diagnostic.__file__).read_bytes()).hexdigest()
    report['candidate_source']=read_json(Path(__file__).with_name('candidate.json'))['product_source']
    output=Path(__file__).resolve().parent/('VM-ILK-HATA-SONUC-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2)+'\n')
    print('Uygulamalar acik kalsin. Bu dosyayi gonder: '+str(output))


if __name__=='__main__':
    main()
