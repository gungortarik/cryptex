#!/usr/bin/python3
"""Read-only, session-bound Mac process/native-log observations; no application control."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import selectors
import stat
import subprocess
import time
import uuid
import zipfile
from lan_transfer_diagnostic import BRAIN, MAC, Refused, directory, read_metadata, observe

MARKERS = {'listener_started': ('listener starting', 'listener started'), 'listener_stopped': ('listener shutting down', 'listener stopped'), 'listen_failure': ('failed to listen', 'listen tcp', 'bind:'), 'connection_established': ('established secure connection',), 'connection_closed': ('connection closed', 'connection was closed'), 'unexpected_device': ('unexpected device', 'unknown device'), 'certificate_error': ('certificate verification', 'certificate is not valid', 'bad certificate'), 'folder_error': ('failed initial scan', 'error on folder', 'folder marker missing'), 'access_denied': ('access is denied', 'permission denied'), 'database_stop_timeout': ('database failed to stop within 10s',), 'shutdown_requested': ('shutting down', 'exiting'), 'panic': ('panic:', 'fatal error:')}


def summarize(raw, truncated):
    counts={key:0 for key in MARKERS}
    recent=[]
    for line in raw.decode('utf-8',errors='replace').splitlines():
        lower=line.lower()
        found=[key for key,patterns in MARKERS.items() if any(p in lower for p in patterns)]
        for key in found: counts[key]+=1
        if found: recent=(recent+[found])[-24:]
    return dict(bytes_read=len(raw),tail_only=truncated,marker_counts=counts,
                recent_marker_groups=recent,raw_lines_exported=False)


def read_log(path):
    with directory(path.parent) as parent:
        fd=os.open(path.name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC,dir_fd=parent)
        with os.fdopen(fd,'rb') as stream:
            info=os.fstat(stream.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_nlink!=1 or info.st_uid!=os.getuid():
                raise Refused('unsafe_log')
            start=max(0,info.st_size-1024*1024)
            stream.seek(start)
            return summarize(stream.read(1024*1024),start>0)


def run_bounded(arguments,timeout=4,limit=256*1024):
    # Only our ps/lsof observer child may be terminated on timeout/overflow.
    # Never signal/adopt any Cryptex or Syncthing process.
    env={'PATH':'/usr/bin:/bin:/usr/sbin:/sbin','LC_ALL':'C'}
    child=subprocess.Popen(arguments,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
    output={'out':bytearray(),'err':bytearray()}
    deadline=time.monotonic()+timeout
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout,selectors.EVENT_READ,'out')
            selector.register(child.stderr,selectors.EVENT_READ,'err')
            while selector.get_map():
                left=deadline-time.monotonic()
                if left<=0: raise Refused('observer_timeout')
                for key,_ in selector.select(min(left,0.2)):
                    chunk=os.read(key.fileobj.fileno(),4096)
                    if not chunk: selector.unregister(key.fileobj);continue
                    output[key.data].extend(chunk)
                    if sum(map(len,output.values()))>limit: raise Refused('observer_output_bound')
            try: code=child.wait(timeout=max(0.01,deadline-time.monotonic()))
            except subprocess.TimeoutExpired: raise Refused('observer_timeout') from None
        if code not in (0,1) or output['err']: raise Refused('observer_failed')
        return bytes(output['out']).decode('utf-8','strict')
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
        child.stdout.close();child.stderr.close()


def parse_processes(raw):
    if len(raw)>256*1024: raise Refused('process_bound')
    rows={}
    for line in raw.splitlines():
        fields=line.strip().split(None,2)
        if len(fields)!=3 or not fields[0].isdigit() or not fields[1].isdigit():
            raise Refused('process_shape')
        pid,parent=map(int,fields[:2])
        if pid in rows: raise Refused('process_shape')
        rows[pid]=(parent,Path(fields[2]).name)
    desktop={'cryptex-desktop','Cryptex'}
    counts=dict(desktop=0,daemon=0,engine=0,daemon_parent_desktop=0,daemon_parent_daemon=0)
    for parent,name in rows.values():
        if name in desktop: counts['desktop']+=1
        if name=='syncthing': counts['engine']+=1
        if name=='cryptexd':
            counts['daemon']+=1
            parent_name=rows.get(parent,(0,''))[1]
            counts['daemon_parent_desktop']+=int(parent_name in desktop)
            counts['daemon_parent_daemon']+=int(parent_name=='cryptexd')
    return counts


def parse_listeners(raw):
    if len(raw)>256*1024: raise Refused('listener_bound')
    result=dict(listeners=0,expected_lan=0,wildcard=0,loopback=0)
    for line in raw.splitlines():
        if not line: continue
        # macOS lsof includes the numeric file descriptor even with -Fpn.
        if line[:1] in ('p','f') and line[1:].isdigit(): continue
        if not line.startswith('n') or not line.endswith(':61942'): raise Refused('listener_shape')
        result['listeners']+=1
        address=line[1:]
        result['expected_lan']+=int(address=='192.168.50.104:61942')
        result['wildcard']+=int(address=='*:61942')
        result['loopback']+=int(address in ('127.0.0.1:61942','[::1]:61942'))
    return result


def processes():
    return parse_processes(run_bounded(['/bin/ps','-x','-o','pid=,ppid=,comm=']))


def listeners():
    return parse_listeners(run_bounded(['/usr/sbin/lsof','-nP','-a','-u',str(os.getuid()),
                                       '-iTCP:61942','-sTCP:LISTEN','-Fpn']))


def collect(root,log_reader=read_log,process_reader=processes,listener_reader=listeners):
    brain=read_metadata(root/'identity/brain_identity.json')
    local=read_metadata(root/'identity/local_device.json')
    if brain.get('brain_id')!=BRAIN or local.get('brain_id')!=BRAIN or local.get('device_id')!=MAC:
        raise Refused('wrong_mac_profile')
    return dict(schema=1,purpose='cryptex-mac-live-service-diagnostic',outcome='observed',
        profile_writes=False,application_processes_changed=False,credentials_read=False,
        canonical_bodies_read=False,atomic_snapshot=False,
        native_log=observe(lambda:log_reader(root/'managed-transport/00000000000000000000000000000001/native/native.log')),
        process_names_not_identity_proof=observe(process_reader),
        port_61942_not_identity_proof=observe(listener_reader),
        limits='Log may include earlier runs; names and listeners do not establish process ownership or delivery.')


def main():
    os.umask(0o077)
    report=dict(schema=1,purpose='cryptex-mac-live-service-diagnostic',outcome='refused')
    try:
        if platform.system()!='Darwin' or os.geteuid()==0: raise Refused('standard_mac_user_required')
        root=Path.home()/'Library/Application Support/Cryptex-Dev/runtime/p0-2'
        report=collect(root)
    except Refused as error: report['error']=str(error)
    except Exception: report['error']='collector_unavailable'
    report['collector_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    output=Path(__file__).absolute().parent/('MAC-SERVIS-TANI-SONUC-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2)+'\n')
    print('Uygulamalar ve profil degistirilmedi. Gonderilecek sonuc: '+str(output))

if __name__=='__main__': main()
