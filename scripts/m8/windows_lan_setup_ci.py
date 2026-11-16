"""Actual package enrollment in a disposable hosted Windows runner."""
import os
from pathlib import Path
import sys
import tempfile
import shutil
import stat
from package_windows_lan_setup import payload
from windows_lan_setup import setup,SESSION
from windows_setup_native import Refused

if __name__=='__main__':
    if os.name!='nt' or os.environ.get('GITHUB_ACTIONS')!='true' or os.environ.get('RUNNER_ENVIRONMENT')!='github-hosted':
        raise SystemExit('hosted_windows_required')
    # Keep partial profiles on failure for the disposable VM lifetime. No owner profiles/keys.
    root=Path(tempfile.mkdtemp(prefix='cryptex-lan-setup-ci-'))
    package=root/'payload';package.mkdir();(package/'transport').mkdir()
    for name,data in payload(Path(sys.argv[1])).items(): (package/name).write_bytes(data)
    base=root/'appdata';base.mkdir()
    report={}
    app=setup(package,base,SESSION,report)
    assert all(report[k] for k in ('first_note_committed','transport_enrolled_local_only','persistent_profile_preserved'))
    runtime=base/'Cryptex-Dev/runtime/p0-2'
    from hashlib import sha256
    def events(): return {str(p.relative_to(runtime)):sha256(p.read_bytes()).hexdigest()
                          for p in (runtime/'canonical/events').rglob('*') if p.is_file() and p.suffix!='.lock'}
    before=events();assert before
    try: setup(package,base,SESSION,{})
    except Refused as error: assert str(error)=='directory_exists_or_unavailable'
    else: raise AssertionError('existing_profile_adopted')
    assert events()==before
    assert not list(runtime.rglob('transport.active'))
    # Delete successful disposable state while its owning standard-user SID
    # still exists; the elevated parent cannot traverse owner-only canonical ACLs.
    temporary=Path(tempfile.gettempdir()).resolve(strict=True)
    if root.resolve(strict=True).parent!=temporary or not root.name.startswith('cryptex-lan-setup-ci-'):
        raise RuntimeError('ci_cleanup_target_refused')
    def readonly_retry(function,path,error):
        candidate=Path(path)
        if root not in candidate.parents or candidate.is_symlink(): raise error
        if not candidate.stat().st_file_attributes & stat.FILE_ATTRIBUTE_READONLY: raise error
        candidate.chmod(stat.S_IWRITE)
        function(path)
    shutil.rmtree(root,onexc=readonly_retry)
    print('windows_actual_persistent_enrollment_and_reuse_refusal=passed')
    print('normal_window_restart_and_network=not_tested')
