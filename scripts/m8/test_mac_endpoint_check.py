import contextlib
import io
from pathlib import Path
import tempfile
import unittest

from mac_endpoint_check import CASES, check_native_endpoints
from mac_identity_preflight import PreflightError


class EndpointProbeTests(unittest.TestCase):
    def test_exact_cases_signed_probe_and_fixed_bounds(self):
        calls = []
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "probe"
            def run(args, stage, timeout):
                calls.append((args, stage, timeout))
                if stage.startswith("native_endpoint_"):
                    return "mac_transport_endpoint_" + args[-1] + "=passed\n"
                return ""
            with contextlib.redirect_stdout(io.StringIO()) as output:
                check_native_endpoints(root, run)
            self.assertEqual(len(calls), 10)
            self.assertEqual(calls[0][0][:6], ["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror"])
            self.assertEqual(calls[0][0][6], Path.cwd() / "services/cryptexd/tests/mac_transport_endpoint.c")
            self.assertEqual(calls[0][2], 30)
            self.assertEqual(calls[1][0][3], "-")
            self.assertNotIn("--keychain", calls[1][0])
            self.assertEqual([call[0] for call in calls[2:]], [[root / "endpoint-probe", case] for case in CASES])
            self.assertTrue(all(call[2] == 15 for call in calls[2:]))
            self.assertEqual(output.getvalue().splitlines()[-1], "mac_transport_endpoint_races=passed")

    def test_absent_duplicate_or_wrong_marker_never_passes(self):
        for text in ("", "mac_transport_endpoint_fd_reuse=passed\n",
                     "mac_transport_endpoint_ipv4_close=passed\n" * 2):
            with tempfile.TemporaryDirectory() as directory:
                def run(_args, stage, _timeout):
                    return text if stage.startswith("native_endpoint_") else ""
                with self.assertRaisesRegex(PreflightError, "native_endpoint_marker_mismatch"):
                    check_native_endpoints(Path(directory) / "probe", run)

    def test_compile_failure_prevents_signing_or_execution(self):
        calls = []
        with tempfile.TemporaryDirectory() as directory:
            def run(args, _stage, _timeout):
                calls.append(args)
                raise PreflightError("compile_failed")
            with self.assertRaisesRegex(PreflightError, "compile_failed"):
                check_native_endpoints(Path(directory) / "probe", run)
        self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
