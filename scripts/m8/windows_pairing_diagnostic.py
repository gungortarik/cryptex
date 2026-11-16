"""Read-only, session-bound disk observations; no IPC, processes or trust claims."""
import ctypes
import json
import os
from pathlib import Path
import uuid
import zipfile

from windows_setup_native import Pins, Refused

BRAIN='b2ad8bb6-2d65-4690-b3c6-6a7b3f5adac4'
LOCAL='bde7cefd-2f40-40f1-a22e-ae1fb123e8a8'
PEER='1d17c886-a6cf-4422-aba5-fc985656741d'
TRANSACTION='3eb268f509abb291f105078ffde1d81be6bf14b2af26640c86bf24e269642454'
LIMIT=1024*1024
STATES={'offer_created','offer_received','proof_received','proof_created',
        'peer_approval_received','local_approval_signed','transcript_ready',
        'owner_pending','owner_approved','append_unknown','anchored','cancelled',
        'cancelled_after_release'}


def unique(pairs):
    result={}
    for key,value in pairs:
        if key in result: raise Refused('duplicate_json_key')
        result[key]=value
    return result


def read_json(path):
    with Pins() as pins:
        pins.chain(path.parent)
        pins.pin(path,False)
        with path.open('rb') as stream: raw=stream.read(LIMIT+1)
        if len(raw)>LIMIT: raise Refused('metadata_size_bound')
        value=json.loads(raw,object_pairs_hook=unique)
        if not isinstance(value,dict): raise Refused('metadata_shape')
        return value


def journal_summary(value):
    version=value.get('schema_version')
    if type(version) is not int or version not in (1,2,3): raise Refused('journal_schema')
    found=[]
    for key in ('transactions','terminal_receipts'):
        records=value.get(key,[])
        if not isinstance(records,list) or len(records)>4096: raise Refused('journal_shape')
        for record in records:
            if not isinstance(record,dict) or not isinstance(record.get('snapshot'),dict):
                raise Refused('journal_shape')
            if record['snapshot'].get('transaction_id')!=TRANSACTION: continue
            state=record.get('state',record.get('kind'))
            if state not in STATES: raise Refused('journal_state')
            messages=record.get('transcript_messages',[])
            attempts=record.get('append_attempts',[])
            if not isinstance(messages,list) or not isinstance(attempts,list): raise Refused('journal_shape')
            found.append(dict(state=state,terminal=key=='terminal_receipts',
                              message_count=len(messages),append_attempt_count=len(attempts),
                              verified_anchor_present=record.get('verified_anchor') is not None))
    return dict(schema_version=version,matching_records=found)


def directory_summary(path):
    # No traversal into unknown children and no file bodies or names exported.
    if not path.exists(): return {'present':False}
    with Pins() as pins:
        pins.chain(path)
        result=dict(present=True,files=0,directories=0,reparse_entries=0,pack_files=0,lock_files=0)
        with os.scandir(path) as entries:
            for index,entry in enumerate(entries):
                if index>=4096: raise Refused('inventory_bound')
                stat=entry.stat(follow_symlinks=False)
                if getattr(stat,'st_file_attributes',0)&0x400:
                    result['reparse_entries']+=1
                elif entry.is_dir(follow_symlinks=False): result['directories']+=1
                elif entry.is_file(follow_symlinks=False):
                    result['files']+=1
                    result['pack_files']+=entry.name.endswith('.pack')
                    result['lock_files']+=entry.name.endswith('.lock')
        return result


def observation(operation):
    try: return dict(status='observed',value=operation())
    except Refused as error: return dict(status='unavailable',error=str(error))
    except (ValueError,TypeError,KeyError,RecursionError):
        return dict(status='unavailable',error='invalid_metadata')
    except OSError as error:
        return dict(status='unavailable',error='io_error',winerror=getattr(error,'winerror',None))


def collect(root):
    # Refuse the host profile or an old VM pilot before any other inspection.
    brain=read_json(root/'identity/brain_identity.json')
    local=read_json(root/'identity/local_device.json')
    if (brain.get('brain_id')!=BRAIN or local.get('brain_id')!=BRAIN
            or local.get('device_id')!=LOCAL): raise Refused('wrong_vm_profile')
    report=dict(schema=1,purpose='cryptex-vm-pairing-disk-diagnostic',
                brain_id=BRAIN,device_id=LOCAL,transaction_id=TRANSACTION,
                outcome='observed',atomic_snapshot=False,
                canonical_signatures_verified=False,live_connection_error_captured=False)
    report['journal']=observation(lambda:journal_summary(read_json(root/'bootstrap/pending.json')))
    report['directories']={}
    paths={'canonical_streams':'canonical/events',
           'local_stream':'canonical/events/'+LOCAL.encode().hex(),
           'peer_stream':'canonical/events/'+PEER.encode().hex(),
           'connection_settings':'connection-settings',
           'managed_transport':'managed-transport'}
    for name,relative in paths.items():
        report['directories'][name]=observation(lambda p=root/relative:directory_summary(p))
    def preferences():
        value=read_json(root/'connection-settings/connections.json')
        if type(value.get('paused')) is not bool or not isinstance(value.get('peers'),list):
            raise Refused('settings_shape')
        return dict(paused=value['paused'],listen_configured=value.get('listen') is not None,
                    peer_count=len(value['peers']))
    report['saved_settings']=observation(preferences)
    return report


def main():
    report=dict(schema=1,purpose='cryptex-vm-pairing-disk-diagnostic',outcome='refused')
    try:
        if os.name!='nt' or ctypes.windll.shell32.IsUserAnAdmin(): raise Refused('standard_windows_user_required')
        report=collect(Path(os.environ['LOCALAPPDATA'])/'Cryptex-Dev/runtime/p0-2')
    except Refused as error: report['error']=str(error)
    except Exception: report['error']='diagnostic_unavailable'
    output=Path(os.environ['USERPROFILE'])/'Downloads'/('VM-ESLESTIRME-TANI-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2))
    print('Tanilama bitti. Profil degistirilmedi. Sonuc ZIP dosyasini gonder:\n'+str(output))


if __name__=='__main__': main()
