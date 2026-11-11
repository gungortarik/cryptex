#!/usr/bin/python3
"""Run the isolated interactive fixture. Return only bounded outcome evidence."""
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
import uuid
import zipfile


def summarize(runtime):
    journal = json.loads((runtime / "bootstrap/pending.json").read_text(encoding="utf-8"))
    operations = sorted((item["state"], len(item["append_attempts"])) for item in journal["transactions"])
    try:
        receipt = json.loads((runtime / "ui-receipt.json").read_text(encoding="utf-8"))
    except FileNotFoundError:
        receipt = None
    return {
        "two_events_original_prefix_preserved": None if receipt is None else (
            receipt.get("eventCount") == 2 and receipt.get("prefixPreserved") is True
        ),
        "one_cancel_and_one_confirm": operations == [("anchored", 1), ("cancelled", 0)],
    }


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    meta = json.loads((package / "fixture.json").read_text(encoding="utf-8"))
    report = {"source": meta["source"], "outcome": "incomplete"}
    print("Yalnızca “Cryptex · İzole onay testi” penceresinde şu adımları uygula:\n"
          "1. “Restrict a device or retire this one” altında “Retire this device” seç.\n"
          "2. “Prepare owner review” ile incelemeyi aç. Kimlikler görününce “Cancel” seç.\n"
          "3. Tekrar “Prepare owner review” seç. Aynı test cihazını görüyorsan “Confirm security change” seç.\n"
          "4. Sonucu gördükten sonra uygulamadan Cmd+Q ile çık. Terminal sonucu kaydedecek.", flush=True)
    if sys.platform != "darwin" or platform.machine() != meta["architecture"]:
        report["outcome"] = "platform_mismatch"
    elif datetime.now(timezone.utc) >= datetime.fromisoformat(meta["expires_utc"]):
        report["outcome"] = "fixture_expired"
    else:
        with tempfile.TemporaryDirectory(prefix="cryptex-owner-ui-") as temp:
            runtime = Path(temp).resolve() / "runtime"
            try:
                result = subprocess.run([str(package / "positive.app/Contents/MacOS/cryptex-desktop"),
                                         "--cryptex-m8-local-content-ui", str(runtime)],
                                        cwd=temp, stdout=subprocess.DEVNULL, check=False)
                report["exit_code"] = result.returncode
                report.update(summarize(runtime))
                if result.returncode == 0 and report["one_cancel_and_one_confirm"] and report["two_events_original_prefix_preserved"]:
                    report["outcome"] = "expected_local_events"
                elif result.returncode == 0 and report["one_cancel_and_one_confirm"] and report["two_events_original_prefix_preserved"] is None:
                    report["outcome"] = "journal_verified_shutdown_receipt_missing"
            except (OSError, ValueError, TypeError, KeyError):
                report["outcome"] = "launch_or_receipt_incomplete"
    output = package / ("MAC-ARAYUZ-SONUC-" + uuid.uuid4().hex[:8] + ".zip")
    with zipfile.ZipFile(output, "x") as archive:
        archive.writestr("result.json", json.dumps(report, indent=2) + "\n")
    print(f"Sonuç: {report['outcome']}\nPaylaşılacak dosya: {output}", flush=True)
    return 0 if report["outcome"] == "expected_local_events" else 1


if __name__ == "__main__":
    sys.exit(main())
