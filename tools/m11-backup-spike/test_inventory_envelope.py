"""A missing or invalid archive signature must never select an unsigned fallback."""
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

from daemon_roundtrip import envelope, open_payload
from rehearse import NODE


class FormatSelection(unittest.TestCase):
    def test_rejects_unsigned_legacy_untrusted_signer_and_ambiguous_inner_zip(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for key in ("writer", "other", "recovery"):
                subprocess.run(NODE + ["keygen", root / (key + ".pub"), root / (key + ".sec")],
                               check=True, timeout=30)
            recipient = root / "recovery.pub"
            for n, (signer, members) in enumerate((
                (None, ["identity/brain_identity.json"]),
                ("other", ["identity/brain_identity.json"]),
                (None, ["inventory.json", "content.zip", "extra"]),
                ("writer", ["inventory.json", "content.zip"]),
            )):
                with self.subTest(n=n):
                    plain, backup, stage = (root / f"{name}-{n}" for name in ("zip", "pgp", "stage"))
                    with zipfile.ZipFile(plain, "x") as archive:
                        for name in members:
                            archive.writestr(name, b"fixture")
                    if signer:
                        envelope("sign-encrypt", [recipient, root / (signer + ".sec")], plain, backup)
                    else:
                        envelope("encrypt", [recipient], plain, backup)
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        open_payload(root, backup, stage)
                    self.assertFalse(stage.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
