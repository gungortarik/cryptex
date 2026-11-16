"""Actual package enrollment in a disposable hosted Windows runner."""
import os
from pathlib import Path
import sys
import tempfile
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
    print('windows_actual_persistent_enrollment_and_reuse_refusal=passed')
    print('normal_window_restart_and_network=not_tested')
