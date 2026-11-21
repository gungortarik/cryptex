"""Exercise the installed read-only helper with an encrypted, disposable Kit."""
import io
import base64
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

from rehearse import HERE, NODE


class RestoreCodec(unittest.TestCase):
    def test_only_authenticated_bounded_manual_payload_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            # Node checks the resolved module filename against its permission list.
            work = Path(temporary).resolve()
            entry = work / "restore.cjs"
            shutil.copyfile(HERE.parents[1] / "services/cryptexd/backup-codec/restore.cjs", entry)
            library, archive = work / "openpgp.min.cjs", work / "fflate.cjs"
            shutil.copyfile(HERE / "node_modules/openpgp/dist/node/openpgp.min.cjs", library)
            shutil.copyfile(HERE / "node_modules/fflate/umd/index.js", archive)
            public, secret, unlock = (work / name for name in ("recipient.pub", "recipient.sec", "unlock"))
            unlock.write_bytes(os.urandom(32).hex().encode("ascii"))
            subprocess.run(NODE + ["keygen", public, secret], check=True)
            keygen = ("const c=require('node:crypto'),k=c.generateKeyPairSync('ed25519');"
                      "console.log(JSON.stringify([k.privateKey.export({format:'der',type:'pkcs8'}).toString('base64'),"
                      "k.publicKey.export({format:'jwk'}).x]));")
            der, pin = json.loads(subprocess.check_output(["node", "-e", keygen]))
            pin = base64.urlsafe_b64decode(pin + "=")
            brain = os.urandom(32)
            kit = io.BytesIO()
            with zipfile.ZipFile(kit, "w") as members:
                members.writestr("recovery.sec", secret.read_bytes())
                members.writestr("recovery.pub", public.read_bytes())
                members.writestr("writer.pub", public.read_bytes())
                members.writestr("brain-identity.sha256", brain)
                members.writestr("bootstrap.pk8", base64.b64decode(der))
            locked = subprocess.check_output(NODE + ["lock-kit", unlock], input=kit.getvalue())
            payload = os.urandom(300_001)
            cipher = subprocess.check_output(NODE + ["encrypt", public], input=payload)
            command = ["node", "--permission", "--no-addons", f"--allow-fs-read={library}",
                       f"--allow-fs-read={archive}", entry]

            def verify(expected_brain, measured_size, content, authority=None):
                scope = b"\0" if authority is None else b"\1" + authority
                header = expected_brain + unlock.read_bytes() + measured_size.to_bytes(8, "big") + scope + locked
                framed = len(header).to_bytes(4, "big") + header + content
                return subprocess.run(command, input=framed, capture_output=True, timeout=30)

            result = verify(brain, len(cipher), cipher)
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
            self.assertEqual(result.stdout, payload)
            paired = verify(brain, len(cipher), cipher, pin)
            self.assertEqual((paired.returncode, paired.stdout), (0, payload + b"\0"))
            self.assertNotEqual(verify(brain, len(cipher), cipher, os.urandom(32)).returncode, 0)
            legacy = subprocess.check_output(NODE + ["sign-encrypt", public, secret], input=payload)
            historical = verify(brain, len(legacy), legacy, pin)
            self.assertEqual((historical.returncode, historical.stdout), (0, payload + b"\1"))
            self.assertNotEqual(verify(brain, len(legacy), legacy).returncode, 0)
            for expected_brain, size, content in [(brain, len(payload) - 1, cipher),
                                                  (brain, len(cipher), cipher[:-1]),
                                                  (os.urandom(32), len(cipher), cipher)]:
                self.assertNotEqual(verify(expected_brain, size, content).returncode, 0)


if __name__ == "__main__":
    unittest.main()
