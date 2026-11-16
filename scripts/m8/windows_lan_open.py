"""Owner-invoked ordinary app opening. No runtime or daemon overrides."""
import os
from pathlib import Path
import subprocess
from windows_setup_native import Pins
from windows_lan_setup import verify_payload

if __name__=='__main__':
    app=Path(__file__).resolve().parent
    try:
        with Pins() as pins:
            verify_payload(app,pins)
            env=os.environ.copy()
            for key in list(env):
                if key.startswith('CRYPTEX'): del env[key]
            subprocess.Popen([str(app/'cryptex-desktop.exe')],env=env,cwd=app,close_fds=True)
        print('Cryptex aciliyor. Pencerenin sonucunu sohbette bildir.')
    except Exception:
        print('Cryptex acilamadi. Dosyalari veya profili silmeden sonucu bildir.')
        raise SystemExit(2)
