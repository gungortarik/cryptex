"""Bounded Windows setup receipt; expressly no pairing/network authority."""
import hashlib
import io
import json
from pathlib import Path
import re
import zipfile
from lan_acceptance_receipt import unique_object
from windows_lan_setup import SESSION,PRODUCT

class Refused(Exception): pass
BASE={'schema','purpose','platform','run_id','product_source','source_commit','pairing_authorized',
      'network_delivery_verified','convergence_verified','normal_window_verified','initialized_startup_restart','outcome'}
STAGES={'fresh_profile_created','first_note_committed','transport_enrolled_local_only','persistent_profile_preserved'}

def validate(report,source):
    if not re.fullmatch('[0-9a-f]{40}',source) or not isinstance(report,dict) or not BASE<=set(report):
        raise Refused('windows_receipt_shape')
    if (type(report['schema']) is not int or report['schema']!=1 or report['purpose']!='cryptex-windows-persistent-lan-setup'
            or report['platform']!='Windows' or report['run_id']!=SESSION or report['product_source']!=PRODUCT
            or report['source_commit']!=source): raise Refused('windows_receipt_binding')
    if any(report[k] is not False for k in ('pairing_authorized','network_delivery_verified','convergence_verified',
                                            'normal_window_verified','initialized_startup_restart')):
        raise Refused('windows_receipt_scope')
    if set(report)-BASE-STAGES-{'first_note_sha256','failed_stage','error'}: raise Refused('windows_receipt_fields')
    if any(report[k] is not True for k in STAGES if k in report): raise Refused('windows_receipt_stage')
    if 'first_note_sha256' in report:
        expected=hashlib.sha256(('Cryptex LAN acceptance '+SESSION+': Windows first public note.').encode()).hexdigest()
        if report['first_note_sha256']!=expected: raise Refused('windows_receipt_note')
    if report['outcome']=='enrolled_local_only':
        if set(report)!=BASE|STAGES|{'first_note_sha256'}: raise Refused('windows_receipt_incomplete')
    elif report['outcome']=='refused_or_incomplete':
        if not {'failed_stage','error'}<=set(report): raise Refused('windows_receipt_failure')
        if any(not isinstance(report[k],str) or not re.fullmatch('[a-z_]{1,100}',report[k]) for k in ('failed_stage','error')):
            raise Refused('windows_receipt_diagnostic')
    else: raise Refused('windows_receipt_outcome')
    return report

def read(path,source):
    with Path(path).open('rb') as stream: raw=stream.read(65537)
    if len(raw)>65536: raise Refused('windows_receipt_size')
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        infos=archive.infolist()
        if (len(infos)!=1 or infos[0].filename!='result.json' or infos[0].orig_filename!='result.json'
                or infos[0].file_size>16384 or infos[0].flag_bits&1): raise Refused('windows_receipt_archive')
        report=json.loads(archive.read(infos[0]),object_pairs_hook=unique_object)
    return validate(report,source),hashlib.sha256(raw).hexdigest()
