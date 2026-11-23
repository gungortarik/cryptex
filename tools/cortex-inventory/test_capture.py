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

    def run_phase(self, mode, trial_id, *decisions, expected_exit=0, review_digest=None):
        env = self.env.copy()
        if sys.platform == "darwin" and os.environ.get("GITHUB_ACTIONS") == "true":
            # securityd resolves the runner's selected default Keychain through
            # its login home. The daemon's trial profile still lives in TMPDIR.
            env["HOME"] = os.environ["HOME"]
        source_free = mode in ("--provenance", "--compare-selection", "--open-accepted", "--accept-selection",
                               "--cancel-selection", "--remove-trial-key")
        source_args = [] if source_free else [self.source]
        digest_args = [] if review_digest is None else [review_digest]
        run = subprocess.run([DAEMON, "cortex-import-disposable", *source_args, mode, trial_id, *digest_args],
                             input=b"".join(json.dumps(d).encode() + b"\n" for d in decisions),
                             capture_output=True, cwd=self.profile, env=env, timeout=90)
        self.assertNotIn(b"private body marker", run.stdout + run.stderr)
        if expected_exit is None:
            self.assertNotEqual(run.returncode, 0)
            if source_free:
                self.assertEqual(run.stdout, b"")
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

    def test_batch_provenance_reopens_without_cortex(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        other_text = b"# Travel\nsecond private body marker"
        (self.notes / "travel.md").write_bytes(other_text)
        other = dict(self.approval, capture_id=str(uuid.uuid4()), source_path="Notes/travel.md",
                     source_sha256=hashlib.sha256(other_text).hexdigest(), classification="internal")
        before = snapshot(self.source)
        header, first, second, retry, refusal, readback = self.run_phase(
            "--batch-create", trial_id, self.approval, other, self.approval,
            dict(self.approval, capture_id=str(uuid.uuid4())))
        self.assertEqual(refusal, {"status": "rejected", "issue": "approval_conflict"})
        self.assertEqual(retry, first)
        self.assertNotEqual(first["event_id"], second["event_id"])
        self.assertEqual(len(readback["verified_notes"]), 2)
        self.assertEqual(snapshot(self.source), before)
        self.source.rename(self.source.with_name("disconnected-export"))
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        packs = canonical_packs(root)
        report, = self.run_phase("--provenance", trial_id)
        self.assertEqual(report, {"schema": "cryptex.cortex.provenance.v1", "sources": [
            {"schema": "cryptex.m15.disposable-source.v1", "event_id": commit["event_id"],
             "brain_id": trial_id, "device_id": header["device_id"], "content_hash": commit["content_hash"],
             "approval": decision} for commit, decision in ((first, self.approval), (second, other))]})
        self.assertEqual(canonical_packs(root), packs)

    def test_batch_provenance_rejects_altered_missing_and_swapped_receipts(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        text = b"another synthetic note"
        (self.notes / "b.md").write_bytes(text)
        other = dict(self.approval, capture_id=str(uuid.uuid4()), source_path="Notes/b.md",
                     source_sha256=hashlib.sha256(text).hexdigest())
        _, first, second, _ = self.run_phase("--batch-create", trial_id, self.approval, other)
        directory = self.temp / f"cryptex-m15-restart-{trial_id}" / "cortex-source-receipts"
        path = directory / (first["event_id"] + ".json")
        original = path.read_bytes()
        altered = json.loads(original)
        altered["source"]["approval"]["source_path"] = "Notes/substituted.md"
        path.write_text(json.dumps(altered), encoding="utf-8")
        self.assertIn(b"source_receipt_signature", self.run_phase("--provenance", trial_id, expected_exit=None))
        path.write_bytes((directory / (second["event_id"] + ".json")).read_bytes())
        self.assertIn(b"source_receipt_mismatch", self.run_phase("--provenance", trial_id, expected_exit=None))
        path.unlink()
        self.assertIn(b"source_receipt_missing", self.run_phase("--provenance", trial_id, expected_exit=None))
        path.write_bytes(original)
        (directory / "orphan.json").write_bytes(original)
        self.assertIn(b"source_receipt_inventory_mismatch", self.run_phase("--provenance", trial_id, expected_exit=None))

    def selection(self, trial_id, *decisions):
        return {"schema": "cryptex.cortex.selection.v1", "brain_id": trial_id, "selected": decisions}

    def test_selection_comparison_shows_missing_extra_and_changed_offline(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        other = dict(self.approval, capture_id=str(uuid.uuid4()), source_path="Notes/b.md",
                     source_sha256=hashlib.sha256(b"second note").hexdigest())
        (self.notes / "b.md").write_bytes(b"second note")
        selected = self.selection(trial_id, self.approval, other)
        _, first, second, _ = self.run_phase("--batch-create", trial_id, self.approval, other)
        self.source.rename(self.source.with_name("disconnected-export"))
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        before = canonical_packs(root)
        evidence = snapshot(root / "cortex-source-receipts")
        report, = self.run_phase("--compare-selection", trial_id, selected)
        self.assertTrue(report["ready_for_owner_review"])
        self.assertEqual(report["brain_id"], trial_id)
        self.assertEqual((report["selected_count"], report["verified_count"]), (2, 2))
        self.assertEqual({r["event_id"] for r in report["matched"]}, {first["event_id"], second["event_id"]})
        self.assertEqual([report[k] for k in ("missing", "extra", "changed")], [[], [], []])
        self.assertEqual(self.run_phase("--compare-selection", trial_id,
                                        self.selection(trial_id, other, self.approval)), [report])

        absent = dict(other, source_path="Notes/absent.md", capture_id=str(uuid.uuid4()),
                      source_sha256=hashlib.sha256(b"missing note").hexdigest())
        report, = self.run_phase("--compare-selection", trial_id, self.selection(trial_id, self.approval, absent))
        self.assertFalse(report["ready_for_owner_review"])
        self.assertEqual(report["missing"], [absent])
        self.assertEqual([r["approval"] for r in report["extra"]], [other])
        self.assertEqual([r["approval"] for r in report["matched"]], [self.approval])
        self.assertEqual(report["changed"], [])

        for field, value in (("classification", "public"), ("source_path", "Notes/renamed.md"),
                             ("source_sha256", absent["source_sha256"]), ("review_sha256", "1" * 64)):
            with self.subTest(field=field):
                changed = dict(self.approval, **{field: value})
                report, = self.run_phase("--compare-selection", trial_id, self.selection(trial_id, changed, other))
                self.assertFalse(report["ready_for_owner_review"])
                difference, = report["changed"]
                self.assertEqual(difference["selected"], changed)
                self.assertEqual(difference["verified"]["approval"], self.approval)
                self.assertEqual(difference["verified"]["event_id"], first["event_id"])
        self.assertEqual(canonical_packs(root), before)
        self.assertEqual(snapshot(root / "cortex-source-receipts"), evidence)

    def test_selection_comparison_refuses_invalid_inputs_and_corrupt_receipt(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        _, receipt, _ = self.run_phase("--batch-create", trial_id, self.approval)
        for selection, issue in (
            (self.selection(str(uuid.uuid4()), self.approval), b"selection_scope_mismatch"),
            (self.selection(trial_id, self.approval, self.approval), b"ambiguous_selection"),
            (self.selection(trial_id, dict(self.approval, source_path="../note.md")), b"invalid_selection"),
            (self.selection(trial_id, dict(self.approval, review_sha256="x")), b"invalid_selection"),
            ({"brain_id": trial_id, "schema": "cryptex.cortex.selection.v1"}, b"invalid_selection"),
        ):
            self.assertIn(issue, self.run_phase("--compare-selection", trial_id, selection, expected_exit=None))
        selection = self.selection(trial_id, self.approval)
        path = (self.temp / f"cryptex-m15-restart-{trial_id}" / "cortex-source-receipts" /
                (receipt["event_id"] + ".json"))
        altered = json.loads(path.read_bytes())
        altered["source"]["approval"]["classification"] = "public"
        path.write_text(json.dumps(altered), encoding="utf-8")
        self.assertIn(b"source_receipt_signature", self.run_phase("--compare-selection", trial_id,
                                                                   selection, expected_exit=None))

    def test_selection_comparison_empty_batch(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        self.run_phase("--batch-create", trial_id)
        empty, = self.run_phase("--compare-selection", trial_id, self.selection(trial_id))
        self.assertFalse(empty["ready_for_owner_review"])
        self.assertEqual((empty["selected_count"], empty["verified_count"]), (0, 0))
        report, = self.run_phase("--compare-selection", trial_id, self.selection(trial_id, self.approval))
        self.assertFalse(report["ready_for_owner_review"])
        self.assertEqual(report["missing"], [self.approval])
        self.assertEqual(report["extra"], [])

    def test_explicit_accept_and_cancel_survive_restart(self):
        before = snapshot(self.source)
        for action, expected, opposite in (("--accept-selection", "accepted", "--cancel-selection"),
                                           ("--cancel-selection", "cancelled", "--accept-selection")):
            with self.subTest(expected=expected):
                trial_id = str(uuid.uuid4())
                self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
                self.run_phase("--batch-create", trial_id, self.approval)
                root = self.temp / f"cryptex-m15-restart-{trial_id}"
                path = root / "cortex-batch-decision.json"
                packs = canonical_packs(root)
                receipts = snapshot(root / "cortex-source-receipts")
                selection = self.selection(trial_id, self.approval)
                preview, = self.run_phase("--compare-selection", trial_id, selection)
                self.assertTrue(preview["ready_for_owner_review"])
                self.assertIsNone(preview["batch_decision"])
                self.assertFalse(path.exists())
                digest = preview["review_sha256"]
                self.assertRegex(digest, r"^[0-9a-f]{64}$")
                completed, = self.run_phase(action, trial_id, selection, review_digest=digest)
                self.assertEqual(completed["batch_decision"]["choice"], expected)
                self.assertEqual(completed["batch_decision"]["review_sha256"], digest)
                self.assertEqual(completed["batch_decision"]["brain_id"], trial_id)
                self.assertFalse(completed["ready_for_owner_review"])
                saved = path.read_bytes()
                self.assertEqual(self.run_phase("--compare-selection", trial_id, selection), [completed])
                different, = self.run_phase("--compare-selection", trial_id, self.selection(trial_id))
                self.assertFalse(different["ready_for_owner_review"])
                self.assertIsNone(different["batch_decision"])
                self.assertEqual(self.run_phase(action, trial_id, selection, review_digest=digest), [completed])
                error = self.run_phase(opposite, trial_id, selection, review_digest=digest, expected_exit=None)
                self.assertIn(b"batch_decision_conflict", error)
                self.assertEqual(path.read_bytes(), saved)
                self.assertEqual(canonical_packs(root), packs)
                self.assertEqual(snapshot(root / "cortex-source-receipts"), receipts)
                altered = json.loads(saved)
                altered["source"]["choice"] = "cancelled" if expected == "accepted" else "accepted"
                path.write_text(json.dumps(altered), encoding="utf-8")
                self.assertIn(b"source_receipt_signature", self.run_phase(
                    "--compare-selection", trial_id, selection, expected_exit=None))
        self.assertEqual(snapshot(self.source), before)

    def test_final_decision_rechecks_selection_digest_and_signed_receipts(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        _, note, _ = self.run_phase("--batch-create", trial_id, self.approval)
        self.source.rename(self.source.with_name("disconnected-export"))
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        selection = self.selection(trial_id, self.approval)
        preview, = self.run_phase("--compare-selection", trial_id, selection)
        digest = preview["review_sha256"]
        changed = self.selection(trial_id, dict(self.approval, classification="public"))
        for action in ("--accept-selection", "--cancel-selection"):
            for manifest, token, issue in ((selection, "0" * 64, b"stale_selection_review"),
                                           (changed, digest, b"selection_not_ready"),
                                           (self.selection(trial_id), digest, b"selection_not_ready")):
                error = self.run_phase(action, trial_id, manifest, review_digest=token, expected_exit=None)
                self.assertIn(issue, error)
        evidence = root / "cortex-source-receipts" / (note["event_id"] + ".json")
        receipt = json.loads(evidence.read_bytes())
        receipt["source"]["approval"]["source_path"] = "Notes/substitution.md"
        evidence.write_text(json.dumps(receipt), encoding="utf-8")
        self.assertIn(b"source_receipt_signature", self.run_phase(
            "--accept-selection", trial_id, selection, review_digest=digest, expected_exit=None))
        self.assertFalse((root / "cortex-batch-decision.json").exists())

    def test_accepted_brain_rebuilds_guarded_reads_without_export(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        body = b"second public synthetic note"
        (self.notes / "b.md").write_bytes(body)
        other = dict(self.approval, capture_id=str(uuid.uuid4()), classification="public",
                     source_path="Notes/b.md", source_sha256=hashlib.sha256(body).hexdigest())
        selection = self.selection(trial_id, other, self.approval)
        self.run_phase("--batch-create", trial_id, self.approval, other)
        self.source.rename(self.source.with_name("disconnected-export"))
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        packs = canonical_packs(root)
        review, = self.run_phase("--compare-selection", trial_id, selection)
        self.assertIn(b"owner_decision_pending", self.run_phase("--open-accepted", trial_id, selection, expected_exit=None))
        self.run_phase("--accept-selection", trial_id, selection, review_digest=review["review_sha256"])
        self.assertIn(b"selection_not_ready", self.run_phase("--open-accepted", trial_id,
                      self.selection(trial_id, other), expected_exit=None))
        opened, = self.run_phase("--open-accepted", trial_id, selection)
        self.assertEqual(opened, {"schema":"cryptex.m15.disposable-cutover.v1", "brain_id":trial_id, "verified_notes":2})
        (root / "derived" / "projection" / "projection.sqlite").unlink()
        self.assertEqual(self.run_phase("--open-accepted", trial_id, selection), [opened])
        self.assertEqual(canonical_packs(root), packs)
        decision = root / "cortex-batch-decision.json"
        signed = json.loads(decision.read_bytes())
        signed["source"]["review_sha256"] = "0" * 64
        decision.write_text(json.dumps(signed), encoding="utf-8")
        self.assertIn(b"source_receipt_signature", self.run_phase("--open-accepted", trial_id, selection, expected_exit=None))

    def test_cancelled_brain_blocks_guarded_cutover(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        self.run_phase("--batch-create", trial_id, self.approval)
        selection = self.selection(trial_id, self.approval)
        preview, = self.run_phase("--compare-selection", trial_id, selection)
        self.run_phase("--cancel-selection", trial_id, selection, review_digest=preview["review_sha256"])
        self.assertIn(b"batch_not_accepted", self.run_phase("--open-accepted", trial_id, selection, expected_exit=None))

    def interrupt_batch(self):
        trial_id = str(uuid.uuid4())
        self.addCleanup(self.run_phase, "--remove-trial-key", trial_id)
        before = snapshot(self.source)
        header, = self.run_phase("--batch-exit-before-source-receipt", trial_id, self.approval,
                                 expected_exit=87)
        self.assertEqual(snapshot(self.source), before)
        self.source.rename(self.source.with_name("disconnected-export"))
        root = self.temp / f"cryptex-m15-restart-{trial_id}"
        receipt = (root / "derived" / "capture_receipts" / trial_id / header["device_id"] /
                   (self.approval["capture_id"] + ".json"))
        self.assertEqual(json.loads(receipt.read_bytes())["state"], "committed")
        self.assertTrue((root / "cortex-pending-source.json").is_file())
        self.assertEqual(list((root / "cortex-source-receipts").iterdir()), [])
        return trial_id, root, receipt

    def test_interrupted_batch_repairs_only_source_receipt_offline(self):
        trial_id, root, capture = self.interrupt_batch()
        stored = json.loads(capture.read_bytes())
        packs = canonical_packs(root)
        self.assertTrue(packs)
        pending = root / "cortex-pending-source.json"
        intent = pending.read_bytes()
        report, = self.run_phase("--provenance", trial_id)
        source, = report["sources"]
        self.assertEqual(source["approval"], self.approval)
        self.assertEqual(source["brain_id"], trial_id)
        self.assertEqual(source["event_id"], stored["event_id"])
        self.assertFalse(pending.exists())
        receipt = root / "cortex-source-receipts" / (stored["event_id"] + ".json")
        published = receipt.read_bytes()
        self.assertEqual(json.loads(published)["source"], source)
        # This is also the disk state if the process exits after receipt
        # publication but before removing its signed intent.
        pending.write_bytes(intent)
        for _ in range(2):
            self.assertEqual(self.run_phase("--provenance", trial_id), [report])
            self.assertEqual(receipt.read_bytes(), published)
            self.assertEqual(canonical_packs(root), packs)
            self.assertFalse(pending.exists())

    def test_interrupted_batch_refuses_unverifiable_evidence(self):
        trial_id, root, capture = self.interrupt_batch()
        packs = canonical_packs(root)
        pending = root / "cortex-pending-source.json"
        original = pending.read_bytes()
        altered = json.loads(original)
        altered["source"]["approval"]["source_path"] = "Notes/other.md"
        pending.write_text(json.dumps(altered), encoding="utf-8")
        directory = root / "cortex-source-receipts"
        self.assertIn(b"source_receipt_signature", self.run_phase("--provenance", trial_id, expected_exit=None))
        self.assertEqual(list(directory.iterdir()), [])
        pending.write_bytes(original)
        stored = capture.read_bytes()
        capture.unlink()
        self.assertIn(b"source_commit_unverified", self.run_phase("--provenance", trial_id, expected_exit=None))
        self.assertEqual(list(directory.iterdir()), [])
        capture.write_bytes(stored)
        path = directory / (json.loads(stored)["event_id"] + ".json")
        path.write_bytes(b"{}")
        self.assertIn(b"source_receipt_invalid", self.run_phase("--provenance", trial_id, expected_exit=None))
        self.assertEqual(path.read_bytes(), b"{}")
        path.unlink()
        (directory / "extra.json").write_bytes(b"{}")
        self.assertIn(b"source_receipt_inventory_mismatch", self.run_phase("--provenance", trial_id, expected_exit=None))
        self.assertFalse(path.exists())
        self.assertEqual(pending.read_bytes(), original)
        self.assertEqual(canonical_packs(root), packs)

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
