"""Prepare the pinned native worker on a disposable Windows CI runner."""
import hashlib, io, os, pathlib, urllib.request, zipfile
root = pathlib.Path(os.environ['RUNNER_TEMP']) / 'exchange-package'
root.mkdir()
url = 'https://github.com/syncthing/syncthing/releases/download/v2.1.5/syncthing-windows-amd64-v2.1.5.zip'
with urllib.request.urlopen(url, timeout=60) as response:
    raw = response.read(64*1024*1024+1)
assert len(raw) <= 64*1024*1024
with zipfile.ZipFile(io.BytesIO(raw)) as archive:
    data = archive.read('syncthing-windows-amd64-v2.1.5/syncthing.exe')
assert hashlib.sha256(data).hexdigest() == '36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344'
(root/'syncthing.exe').write_bytes(data)
for name in ('LICENSE.txt','AUTHORS.txt'):
    (root/name).write_bytes((pathlib.Path('third_party/syncthing/2.1.5')/name).read_bytes())
