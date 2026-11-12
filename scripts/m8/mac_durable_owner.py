#!/usr/bin/python3
"""Run the signed durable fixture, exporting only aggregate test outcomes."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import tempfile
import uuid
import zipfile

from mac_durable_check import check_durable_profile
from mac_identity_preflight import PreflightError


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    meta = json.loads((package / "fixture.json").read_text(encoding="utf-8"))
    report = {"source": meta["source"], "outcome": "incomplete",
              "scope": "isolated signed native Keychain profile; automated approvals"}
    print("Kalıcı profil testi başladı. Bu pencereyi açık bırak.\n"
          "Test, kendi profilinde kesinti, yeniden başlatma ve anahtar kaybını otomatik sınayacak.",
          flush=True)
    if sys.platform != "darwin" or platform.machine() != meta["architecture"]:
        report["outcome"] = "platform_mismatch"
    elif datetime.now(timezone.utc) >= datetime.fromisoformat(meta["expires_utc"]):
        report["outcome"] = "fixture_expired"
    else:
        temp = Path(tempfile.mkdtemp(prefix="cryptex-owner-durable-")).resolve()
        try:
            check_durable_profile(package / "positive.app/Contents/MacOS/cryptex-desktop",
                                  temp / "runtime", report)
            report["outcome"] = "passed"
        except PreflightError as error:
            report["outcome"] = "failed"
            report["diagnostic"] = str(error)
        except (OSError, ValueError, TypeError, KeyError):
            report["outcome"] = "collector_incomplete"
        finally:
            if report.get("credential_cleanup_verified") is True:
                try:
                    shutil.rmtree(temp)
                    report["runtime_cleanup_verified"] = True
                except OSError:
                    report["outcome"] = "cleanup_incomplete"
            else:
                print(f"Temizlik tamamlanamadı. Test kaydı burada korundu: {temp}", flush=True)
    output = package / ("MAC-KALICI-SONUC-" + uuid.uuid4().hex[:8] + ".zip")
    with zipfile.ZipFile(output, "x") as archive:
        archive.writestr("result.json", json.dumps(report, indent=2) + "\n")
    print(f"Sonuç: {report['outcome']}\nPaylaşılacak dosya: {output}", flush=True)
    return 0 if report["outcome"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
