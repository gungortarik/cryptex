"""Windows-only bounded child jobs and no-reparse namespace pins for setup."""
import ctypes as c
from ctypes import wintypes as w
import os
from pathlib import Path
import subprocess
import threading
import time

class Refused(Exception): pass

if os.name == 'nt':
    k = c.WinDLL('kernel32', use_last_error=True)
    class SA(c.Structure): _fields_=[('size',w.DWORD),('descriptor',w.LPVOID),('inherit',w.BOOL)]
    class SI(c.Structure):
        _fields_=[('cb',w.DWORD),('reserved',w.LPWSTR),('desktop',w.LPWSTR),('title',w.LPWSTR),
                  ('x',w.DWORD),('y',w.DWORD),('sx',w.DWORD),('sy',w.DWORD),('cx',w.DWORD),('cy',w.DWORD),
                  ('fill',w.DWORD),('flags',w.DWORD),('show',w.WORD),('reserved2',w.WORD),('bytes',w.LPVOID),
                  ('stdin',w.HANDLE),('stdout',w.HANDLE),('stderr',w.HANDLE)]
    class PI(c.Structure): _fields_=[('process',w.HANDLE),('thread',w.HANDLE),('pid',w.DWORD),('tid',w.DWORD)]
    class BASIC(c.Structure):
        _fields_=[('p_time',c.c_longlong),('j_time',c.c_longlong),('flags',w.DWORD),('min',c.c_size_t),
                  ('max',c.c_size_t),('active',w.DWORD),('affinity',c.c_size_t),('priority',w.DWORD),('scheduling',w.DWORD)]
    class EXT(c.Structure): _fields_=[('basic',BASIC),('io',c.c_ulonglong*6),('pm',c.c_size_t),('jm',c.c_size_t),('pp',c.c_size_t),('jp',c.c_size_t)]
    class ACCOUNTING(c.Structure): _fields_=[('times',c.c_longlong*4),('faults',w.DWORD),('total',w.DWORD),('active',w.DWORD),('terminated',w.DWORD)]
    class INFO(c.Structure):
        _fields_=[('attributes',w.DWORD),('creation',w.FILETIME),('access',w.FILETIME),('write',w.FILETIME),
                  ('volume',w.DWORD),('size_hi',w.DWORD),('size_lo',w.DWORD),('links',w.DWORD),('index_hi',w.DWORD),('index_lo',w.DWORD)]
    def api(name, args, result=w.BOOL):
        fn=getattr(k,name);fn.argtypes=args;fn.restype=result;return fn
    close=api('CloseHandle',[w.HANDLE])
    create_file=api('CreateFileW',[w.LPCWSTR,w.DWORD,w.DWORD,w.LPVOID,w.DWORD,w.DWORD,w.HANDLE],w.HANDLE)
    info_file=api('GetFileInformationByHandle',[w.HANDLE,c.POINTER(INFO)])
    create_dir=api('CreateDirectoryW',[w.LPCWSTR,w.LPVOID])
    create_job=api('CreateJobObjectW',[w.LPVOID,w.LPCWSTR],w.HANDLE)
    set_job=api('SetInformationJobObject',[w.HANDLE,c.c_int,w.LPVOID,w.DWORD])
    assign=api('AssignProcessToJobObject',[w.HANDLE,w.HANDLE])
    terminate=api('TerminateJobObject',[w.HANDLE,w.UINT])
    create_process=api('CreateProcessW',[w.LPCWSTR,w.LPWSTR,w.LPVOID,w.LPVOID,w.BOOL,w.DWORD,w.LPVOID,w.LPCWSTR,c.POINTER(SI),c.POINTER(PI)])
    resume=api('ResumeThread',[w.HANDLE],w.DWORD)
    wait=api('WaitForSingleObject',[w.HANDLE,w.DWORD],w.DWORD)
    exit_code=api('GetExitCodeProcess',[w.HANDLE,c.POINTER(w.DWORD)])
    terminate_process=api('TerminateProcess',[w.HANDLE,w.UINT])
    query_job=api('QueryInformationJobObject',[w.HANDLE,c.c_int,w.LPVOID,w.DWORD,w.LPVOID])
    open_process=api('OpenProcess',[w.DWORD,w.BOOL,w.DWORD],w.HANDLE)
    in_job=api('IsProcessInJob',[w.HANDLE,w.HANDLE,c.POINTER(w.BOOL)])

def settle_job(job):
    handles=[]
    try:
        # Retain exact job-member handles before termination; zero active count
        # can precede the final process-object signal by a short kernel interval.
        class IDS(c.Structure): _fields_=[('assigned',w.DWORD),('count',w.DWORD),('ids',c.c_size_t*1024)]
        ids=IDS()
        if not query_job(job,3,c.byref(ids),c.sizeof(ids),None) or ids.count>1024:
            raise Refused('job_members_unknown')
        for pid in ids.ids[:ids.count]:
            handle=open_process(0x101000,False,pid)
            if not handle:
                if c.get_last_error()==87: continue  # Already exited; never adopt a replacement.
                raise Refused('job_member_unavailable')
            handles.append(handle)
            member=w.BOOL()
            if not in_job(handle,job,c.byref(member)) or not member.value:
                raise Refused('job_member_changed')
        if not terminate(job,2): raise Refused('job_cleanup_failed')
        deadline=time.monotonic()+5
        while True:
            counts=ACCOUNTING()
            if not query_job(job,1,c.byref(counts),c.sizeof(counts),None): raise Refused('job_cleanup_unknown')
            if counts.active==0 and all(wait(handle,0)==0 for handle in handles): return
            if time.monotonic()>deadline: raise Refused('job_cleanup_deadline')
            time.sleep(.01)
    finally:
        for handle in handles: close(handle)

class Pins:
    def __init__(self): self.handles=[]
    def pin(self,path,directory=True):
        path=Path(path)
        if not path.is_absolute() or '..' in path.parts: raise Refused('unsafe_path')
        handle=create_file(str(path),0x80 if directory else 0x80000000,3 if directory else 1,None,3,0x02200000,None)
        if handle == c.c_void_p(-1).value: raise Refused('path_pin_failed')
        self.handles.append(handle)
        info=INFO()
        if not info_file(handle,c.byref(info)) or info.attributes & 0x400 or bool(info.attributes & 0x10)!=directory:
            raise Refused('unsafe_path_object')
        if not directory and info.links != 1: raise Refused('unsafe_file_links')
    def chain(self,path):
        for item in reversed([Path(path),*Path(path).parents]): self.pin(item)
    def child(self,parent,name,fresh=False):
        path=Path(parent)/name
        if not create_dir(str(path),None):
            if fresh or c.get_last_error()!=183: raise Refused('directory_exists_or_unavailable')
        self.pin(path)
        return path
    def close(self):
        for h in reversed(self.handles): close(h)
        self.handles=[]
    def __enter__(self): return self
    def __exit__(self,*_): self.close()

def run(args,timeout=60,input_data=b''):
    """Create suspended, assign kill-on-close job, then resume; bounded output."""
    import msvcrt
    if len(input_data)>4096: raise Refused('input_bound')
    job=None; pi=PI(); assigned=False; fds=[]; reader=None; reader_started=False
    chunks=[]; size=0; limited=threading.Event(); failed=threading.Event()
    try:
        job=create_job(None,None)
        limits=EXT(); limits.basic.flags=0x2000
        if not job or not set_job(job,9,c.byref(limits),c.sizeof(limits)): raise Refused('job_create_failed')
        ir,iw=os.pipe();orr,ow=os.pipe();fds=[ir,iw,orr,ow]
        for fd in fds: os.set_inheritable(fd,fd in (ir,ow))
        si=SI();si.cb=c.sizeof(si);si.flags=0x100
        si.stdin=msvcrt.get_osfhandle(ir);si.stdout=si.stderr=msvcrt.get_osfhandle(ow)
        command=c.create_unicode_buffer(subprocess.list2cmdline([str(a) for a in args]))
        # Fixed environment: no daemon override, Python injection, or custom DLL search.
        env={n:os.environ[n] for n in ('SYSTEMROOT','WINDIR','TEMP','TMP','LOCALAPPDATA','APPDATA','USERPROFILE') if n in os.environ}
        env['PATH']=str(Path(os.environ['SYSTEMROOT'])/'System32')
        block=c.create_unicode_buffer('\0'.join(n+'='+v for n,v in sorted(env.items()))+'\0\0')
        if not create_process(str(args[0]),command,None,None,True,0x08000404,block,None,c.byref(si),c.byref(pi)):
            raise Refused('process_create_failed')
        if not assign(job,pi.process): raise Refused('job_assign_failed')
        assigned=True
        def read_output():
            nonlocal size
            try:
                while True:
                    data=os.read(orr,4096)
                    if not data: break
                    size+=len(data)
                    if size>65536: limited.set();break
                    chunks.append(data)
            except OSError: failed.set()
        reader=threading.Thread(target=read_output,daemon=True);reader.start()
        reader_started=True
        for fd in (ir,ow): os.close(fd);fds.remove(fd)
        if resume(pi.thread)==0xffffffff: raise Refused('process_resume_failed')
        # At most 4096 bytes, below the anonymous pipe buffer size.
        if input_data and os.write(iw,input_data)!=len(input_data): raise Refused('stdin_failed')
        os.close(iw);fds.remove(iw)
        deadline=time.monotonic()+timeout
        while wait(pi.process,20)==258:
            if limited.is_set() or failed.is_set(): raise Refused('process_output_bound')
            if time.monotonic()>deadline: raise Refused('process_deadline')
        code=w.DWORD()
        if not exit_code(pi.process,c.byref(code)): raise Refused('process_exit_unknown')
        # Retained descendants cannot keep pipes open or outlive this command.
        settle_job(job)
        reader.join(5)
        if reader.is_alive() or limited.is_set() or failed.is_set(): raise Refused('process_output_bound')
        if code.value: raise Refused('command_failed')
        return b''.join(chunks).decode('utf-8',errors='replace')
    finally:
        cleanup_error=None
        if pi.process:
            try:
                if assigned: settle_job(job)
                elif not terminate_process(pi.process,2): raise Refused('suspended_cleanup_failed')
                if wait(pi.process,5000)!=0: raise Refused('process_cleanup_unknown')
            except Refused as error: cleanup_error=error
        if reader_started: reader.join(5)
        for fd in fds:
            try: os.close(fd)
            except OSError: pass
        for h in (pi.thread,pi.process,job):
            if h: close(h)
        if cleanup_error: raise cleanup_error
