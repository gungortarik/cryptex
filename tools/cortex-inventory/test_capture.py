"""Submit synthetic owner approvals to a real, disposable-only daemon process."""
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import uuid

from test_cli import BINARY, snapshot


DAEMON = Path(os.environ["CRYPTEX_M15_DAEMON"]).resolve()


def canonical_packs(root):
    return {p.relative_to(root): p.read_bytes() for p in (root / "canonical").rglob("*")
            if p.is_file() and p.suffix in (".seg", ".open")}


class ReviewedCaptureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if sys.platform == "darwin" and os.environ.get("GITHUB_ACTIONS") == "true":
            # Use an unlocked disposable Keychain in the unattended runner.
            sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts/m8"))
            cls.addClassCleanup(sys.path.pop, 0)
            from mac_ci_credential_keychain import CredentialKeychain
            from mac_identity_preflight import run_bounded

            stack = ExitStack()
            cls.addClassCleanup(stack.close)
            stack.enter_context(CredentialKeychain(Path(tempfile.gettempdir()), run_bounded))

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        base = Path(temp.name)
        self.source = base / "legacy"
        self.notes = self.source / "Notes"
        self.notes.mkdir(parents=True)
        self.profile = base / "user"
        self.profile.mkdir()
        self.temp = base / "scratch"
        self.temp.mkdir()
        self.env = os.environ.copy()
        for name in ("TMP", "TEMP", "TMPDIR"):
            self.env[name] = str(self.temp)
        for name in ("HOME", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "XDG_DATA_HOME",
                     "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"):
            self.env[name] = str(self.profile)
        self.text = "---\r\nclassification: public\r\n---\r\nprivate body marker — ilk not\r\n".encode()
        self.file = self.notes / "ilk.MD"
        self.file.write_bytes(self.text)
        preview = subprocess.run([BINARY, "--preview-note", self.source, "Notes/ilk.MD"],
                                 cwd=self.profile, env=self.env, capture_output=True, check=True)
        report = json.loads(preview.stdout)
        self.approval = {"capture_id": str(uuid.uuid4()), "source_path": report["source_path"],
                         "source_sha256": report["source_sha256"], "classification": "restricted"}

    def submit(self, *decisions):
        before = snapshot(self.source)
        data = b"".join(json.dumps(decision).encode() + b"\n" for decision in decisions)
        run = subprocess.run([DAEMON, "cortex-import-disposable", self.source], input=data,
                             capture_output=True, cwd=self.profile, env=self.env, timeout=90)
        self.assertNotIn(b"private body marker", run.stdout + run.stderr)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(before, snapshot(self.source))
        self.assertEqual(list(self.profile.iterdir()), [])
        self.assertEqual(list(self.temp.iterdir()), [])
        *replies, readback = map(json.loads, run.stdout.splitlines())
        self.assertEqual(len(replies), len(decisions))
        return replies, readback["verified_notes"]

    def test_signed_cycle_and_retry(self):
        for classification in ("public", "internal", "private", "restricted"):
            with self.subTest(classification=classification):
                decision = dict(self.approval, classification=classification)
                replies, events = self.submit(decision, decision)
                committed = replies[0]
                self.assertEqual(committed["status"], "committed", committed)
                self.assertEqual(committed["approval"], decision)
                self.assertEqual(committed, replies[1])
                self.assertEqual(events, [{
                    "event_id": committed["event_id"], "brain_id": committed["brain_id"],
                    "content_hash": committed["content_hash"], "event_type": "memory.note",
                    "classification": classification, "text_sha256": hashlib.sha256(self.text).hexdigest(),
                }])

    def test_changed_source_never_commits(self):
        self.file.write_bytes(self.text.replace(b"ilk", b"son"))
        replies, events = self.submit(self.approval)
        self.assertEqual(replies, [{"status": "rejected", "issue": "digest_mismatch"}])
        self.assertEqual(events, [])

    def test_invalid_or_missing_decision_never_commits(self):
        missing = self.approval.copy()
        del missing["classification"]
        replies, events = self.submit(missing, dict(self.approval, classification="unknown"),
                                      dict(self.approval, source_sha256=""), dict(self.approval, capture_id=""))
        self.assertEqual(replies, [{"status": "rejected", "issue": "invalid_approval"}] * 4)
        self.assertEqual(events, [])

    def test_replay_cannot_substitute_source_or_class(self):
        (self.notes / "copy.md").write_bytes(self.text)
        blocked, events = self.submit(self.approval)
        self.assertEqual(events, [])
        self.assertEqual(blocked[0]["status"], "needs_review")
        self.assertEqual(blocked[0]["review"]["exact_duplicates"], [["Notes/copy.md", "Notes/ilk.MD"]])
        choice = dict(self.approval, review_sha256=blocked[0]["review"]["review_sha256"])
        replies, events = self.submit(choice, dict(choice, classification="public"),
                                      dict(choice, source_path="Notes/copy.md"))
        self.assertEqual(replies[0]["status"], "committed", replies[0])
        self.assertEqual(replies[1:], [{"status": "rejected", "issue": "approval_conflict"}] * 2)
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]["classification"], "restricted")

    def test_conflict_requires_current_review_before_capture(self):
        self.text = b"# Trip\nprivate body marker: Monday"
        self.file.write_bytes(self.text)
        self.approval["source_sha256"] = hashlib.sha256(self.text).hexdigest()
        other = self.notes / "second.md"
        other.write_bytes(b"# Trip\nTuesday")
        replies, events = self.submit(self.approval)
        self.assertEqual(events, [])
        self.assertEqual(replies[0]["status"], "needs_review")
        report = replies[0]["review"]
        self.assertEqual(report["possible_conflicts"], [["Notes/ilk.MD", "Notes/second.md"]])
        preview = subprocess.run([BINARY, "--review-notes", self.source], env=self.env,
                                 cwd=self.profile, capture_output=True, timeout=30)
        self.assertEqual(preview.returncode, 2)
        self.assertEqual(json.loads(preview.stdout), report)
        choice = dict(self.approval, review_sha256=json.loads(preview.stdout)["review_sha256"])
        other.write_bytes(b"# Trip\nWednesday")
        replies, events = self.submit(choice)
        self.assertEqual(events, [])
        self.assertEqual(replies[0]["status"], "needs_review")
        choice["review_sha256"] = replies[0]["review"]["review_sha256"]
        replies, events = self.submit(self.approval, choice, choice)
        self.assertEqual(replies[0]["status"], "needs_review")
        self.assertEqual(replies[1], replies[2])
        self.assertEqual(replies[1]["approval"], choice)
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]["event_id"], replies[1]["event_id"])
        self.assertEqual(events[0]["text_sha256"], self.approval["source_sha256"])
        self.assertEqual(events[0]["classification"], "restricted")

        other.write_bytes(b"\xff")
        replies, events = self.submit(choice)
        self.assertEqual(events, [])
        self.assertEqual(replies[0]["status"], "needs_review")
        self.assertNotIn("review_sha256", replies[0]["review"])

    def run_phase(self, mode, trial_id, *decisions, expected_exit=0):
        env = self.env.copy()
        if sys.platform == "darwin" and os.environ.get("GITHUB_ACTIONS") == "true":
            # securityd resolves the runner's selected default Keychain through
            # its login home. The daemon's trial profile still lives in TMPDIR.
            env["HOME"] = os.environ["HOME"]
        run = subprocess.run([DAEMON, "cortex-import-disposable", self.source, mode, trial_id],
                             input=b"".join(json.dumps(d).encode() + b"\n" for d in decisions),
                             capture_output=True, cwd=self.profile, env=env, timeout=90)
        self.assertNotIn(b"private body marker", run.stdout + run.stderr)
        if expected_exit is None:
            self.assertNotEqual(run.returncode, 0)
            return run.stderr
        self.assertEqual(run.returncode, expected_exit, run.stderr)
        return [json.loads(line) for line in run.stdout.splitlines()]

    def start_retained_trial(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        session, receipt, report = self.run_phase("--restart-create", trial_id, self.approval)
        self.assertEqual(receipt["status"], "committed", receipt)
        self.assertEqual(receipt["approval"], self.approval)
        self.assertEqual(session["brain_id"], trial_id)
        self.assertEqual(len(report["verified_notes"]), 1)
        note = report["verified_notes"][0]
        self.assertEqual(note["event_id"], receipt["event_id"])
        self.assertEqual(note["classification"], "restricted")
        self.assertEqual(note["text_sha256"], hashlib.sha256(self.text).hexdigest())
        return trial_id, session, receipt, report

    def test_restart_replays_one_signed_note(self):
        source_before = snapshot(self.source)
        trial_id, first, receipt, report = self.start_retained_trial()
        root = self.temp / f"cryptex-m15-restart-{trial_id}"

        original = canonical_packs(root)
        self.assertTrue(original)
        second, refusal, replay, reopened = self.run_phase(
            "--restart-replay", trial_id, dict(self.approval, classification="public"), self.approval)
        self.assertNotEqual(first["pid"], second["pid"])
        for key in ("brain_id", "device_id", "signing_key_id"):
            self.assertEqual(first[key], second[key])
        self.assertEqual(refusal, {"status": "rejected", "issue": "approval_conflict"})
        self.assertEqual(replay, receipt)
        self.assertEqual(reopened, report)
        self.assertEqual(canonical_packs(root), original)
        self.assertEqual(snapshot(self.source), source_before)

    def test_retry_after_commit_before_receipt_delivery(self):
        source_before = snapshot(self.source)
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        # Only the initial session header reached the caller.
        first, = self.run_phase("--exit-before-reply", trial_id, self.approval, expected_exit=86)
        self.assertEqual(set(first), {"pid", "brain_id", "device_id", "signing_key_id"})
        self.assertEqual(first["brain_id"], trial_id)
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        original = canonical_packs(root)
        self.assertTrue(original)
        capture_id = self.approval["capture_id"]
        receipt_file = root / "derived" / "capture_receipts" / trial_id / first["device_id"] / (capture_id + ".json")
        stored = json.loads(receipt_file.read_bytes())
        self.assertEqual(stored["state"], "committed")
        self.assertEqual(stored["capture_id"], capture_id)

        second, retry, report = self.run_phase("--restart-replay", trial_id, self.approval)
        for key in ("brain_id", "device_id", "signing_key_id"):
            self.assertEqual(first[key], second[key])
        self.assertEqual(retry["status"], "committed")
        self.assertEqual(retry["approval"], self.approval)
        self.assertEqual(retry["brain_id"], trial_id)
        self.assertEqual(retry["event_id"], stored["event_id"])
        self.assertEqual(report["verified_notes"], [{
            "brain_id": trial_id, "event_id": stored["event_id"], "content_hash": retry["content_hash"],
            "event_type": "memory.note", "classification": "restricted",
            "text_sha256": hashlib.sha256(self.text).hexdigest(),
        }])
        self.assertEqual(canonical_packs(root), original)
        self.assertEqual(snapshot(self.source), source_before)

    def test_restart_refuses_missing_native_key(self):
        trial_id, _, _, _ = self.start_retained_trial()
        self.run_phase("--remove-trial-key", trial_id)
        diagnostic = self.run_phase("--restart-replay", trial_id, self.approval, expected_exit=None)
        self.assertIn(b"trial_key_unavailable", diagnostic)

    def test_restart_refuses_missing_capture_receipt(self):
        trial_id, session, _, report = self.start_retained_trial()
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        (root / "derived" / "capture_receipts" / trial_id / session["device_id"] /
         (self.approval["capture_id"] + ".json")).unlink()
        _, refusal, reopened = self.run_phase("--restart-replay", trial_id, self.approval)
        self.assertEqual(refusal, {"status": "uncertain", "issue": "prior_commit_unverified"})
        self.assertEqual(reopened, report)


if __name__ == "__main__":
    unittest.main()
