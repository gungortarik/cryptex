"""The offline Kit proves its public backup certificate can recover ciphertext."""
import hashlib
import io
from pathlib import Path
import secrets
import subprocess
import tempfile
import unittest
import zipfile

from recovery_kit import MEMBERS, NODE, export_recipient, seal, unseal


class PublicRecipientTest(unittest.TestCase):
    def test_reject_mismatched_kit_and_export_only_public_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "source"
            source.mkdir()
            key, secret = source / "recovery.pub", source / "recovery.sec"
            subprocess.run(NODE + ["keygen", key, secret], check=True)
            password = root / "unlock"
            password.write_text(secrets.token_hex(32), encoding="ascii")
            (source / "writer.pub").write_bytes(key.read_bytes())
            bootstrap_key = root / "bootstrap-for-test"
            script = ("const {privateKey}=require('node:crypto').generateKeyPairSync('ed25519');"
                      "process.stdout.write(privateKey.export({type:'pkcs8',format:'der'}))")
            with bootstrap_key.open("xb") as out:
                subprocess.run(["node", "-e", script], stdout=out, check=True)
            bootstrap = source / "bootstrap.pk8"
            bootstrap.write_bytes(bootstrap_key.read_bytes())
            brain = hashlib.sha256(b"test identity").digest()
            (source / "brain-identity.sha256").write_bytes(brain)
            sealed = root / "kit.pgp"
            seal(source, password, sealed)
            public = root / "public"
            digest = export_recipient(sealed, password, public)
            self.assertEqual(sorted(p.name for p in public.iterdir()), ["brain-identity.sha256", "recovery.pub"])
            self.assertEqual((public / "brain-identity.sha256").read_bytes(), brain)
            certificate = (public / "recovery.pub").read_bytes()
            self.assertTrue(certificate.startswith(b"-----BEGIN PGP PUBLIC KEY BLOCK-----"))
            self.assertEqual(digest, hashlib.sha256(certificate).hexdigest())
            with self.assertRaises(OSError):
                export_recipient(sealed, password, public)
            self.assertEqual((public / "recovery.pub").read_bytes(), certificate)

            bootstrap.write_bytes(b"damaged bootstrap credential")
            with self.assertRaises(subprocess.CalledProcessError):
                seal(source, password, root / "invalid-bootstrap.pgp")
            self.assertFalse((root / "invalid-bootstrap.pgp").exists())
            bootstrap.write_bytes(bootstrap_key.read_bytes())

            key.unlink()
            subprocess.run(NODE + ["keygen", key, root / "different.sec"], check=True)
            with self.assertRaises(subprocess.CalledProcessError):
                seal(source, password, root / "rejected.pgp")
            self.assertFalse((root / "rejected.pgp").exists())
            # Earlier Kit writers could seal mismatched keys. Check on import too.
            raw = io.BytesIO()
            with zipfile.ZipFile(raw, "w") as archive:
                for name in MEMBERS:
                    archive.writestr(name, (source / name).read_bytes())
            with (root / "mismatched.pgp").open("xb") as out:
                subprocess.run(NODE + ["lock-kit", password], input=raw.getvalue(), stdout=out, check=True)
            for operation in (export_recipient, unseal):
                destination = root / operation.__name__
                with self.assertRaises(subprocess.CalledProcessError):
                    operation(root / "mismatched.pgp", password, destination)
                self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
