import io
import sys
from pathlib import Path
import unittest
from contextlib import redirect_stdout

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError
from mac_local_startup_ci import CONTENT_MARKERS, require_content_markers


class ContentDiagnosticsTests(unittest.TestCase):
    def test_missing_marker_stays_failed_with_only_closed_progress_labels(self):
        output = "\n".join(["private note", "m8_candidate_scenario=private path",
                            "m8_candidate_scenario=lost_connection", CONTENT_MARKERS[0]])
        captured = io.StringIO()
        with redirect_stdout(captured), self.assertRaisesRegex(PreflightError, "marker_missing"):
            require_content_markers(output)
        self.assertNotIn("private", captured.getvalue())
        self.assertIn('"lost_connection"', captured.getvalue())
        self.assertIn('"verified_tauri_assets": true', captured.getvalue())
        self.assertIn('"native_synthetic_approval_sessions": false', captured.getvalue())

    def test_all_exact_markers_are_required(self):
        with redirect_stdout(io.StringIO()):
            require_content_markers("\n".join(CONTENT_MARKERS))
            for marker in CONTENT_MARKERS:
                with self.subTest(marker=marker), self.assertRaises(PreflightError):
                    require_content_markers("\n".join(line if line != marker else "prefix " + line
                                                       for line in CONTENT_MARKERS))


if __name__ == "__main__":
    unittest.main()
