import hashlib
import io
from pathlib import Path
import stat
import tempfile
import unittest
import zipfile

from mac_identity_preflight import PreflightError
from mac_transport_package import prepare, selected_members


def archive(entries):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as result:
        for name, body, mode in entries:
            info = zipfile.ZipInfo(name)
            info.external_attr = mode << 16
            result.writestr(info, body)
    return buffer.getvalue()


class TransportPackageTests(unittest.TestCase):
    def test_selects_exact_member_while_ignoring_same_named_nested_examples(self):
        data = archive([("release/etc/syncthing", b"example", stat.S_IFREG | 0o644),
                        ("release/syncthing", b"native", stat.S_IFREG | 0o755),
                        ("../../escape", b"unrelated", stat.S_IFREG | 0o644)])
        result = selected_members(data, "release/", {"syncthing": (6, hashlib.sha256(b"native").hexdigest())})
        self.assertEqual(result, {"syncthing": b"native"})

    def test_duplicate_case_alias_symlink_and_wrong_bytes_refuse(self):
        spec = {"syncthing": (6, hashlib.sha256(b"native").hexdigest())}
        for entries in [
            [("release/Syncthing", b"native", stat.S_IFREG | 0o755)],
            [("release/syncthing", b"native", stat.S_IFLNK | 0o777)],
            [("release/syncthing", b"native", stat.S_IFREG | 0o755),
             ("release/SYNCTHING", b"native", stat.S_IFREG | 0o755)],
            [("release/syncthing", b"edited", stat.S_IFREG | 0o755)],
        ]:
            with self.subTest(entries=entries), self.assertRaises(PreflightError):
                selected_members(archive(entries), "release/", spec)

    def test_bad_download_does_not_create_destination_or_invoke_codesign(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "transport"
            with self.assertRaises(PreflightError):
                prepare(target, lambda *_: self.fail("codesign must not run"),
                        machine="arm64", download=lambda _: b"foreign archive")
            self.assertFalse(target.exists())

    def test_unknown_architecture_does_not_download(self):
        with self.assertRaises(PreflightError):
            prepare(Path("unused"), None, machine="unknown", download=lambda _: self.fail("download"))


if __name__ == "__main__":
    unittest.main()
