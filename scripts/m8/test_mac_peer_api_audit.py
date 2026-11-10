from contextlib import redirect_stderr
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import mac_peer_api_audit as audit
from mac_identity_preflight import CommandResult, PreflightError


class PublicSdkAuditTests(unittest.TestCase):
    def test_deadline_output_and_pipe_failures_are_not_api_absence_or_presence(self):
        for result in (CommandResult(0, '', timed_out=True), CommandResult(1, '', limited=True),
                       CommandResult(0, '', pipe_failed=True)):
            with self.assertRaises(PreflightError):
                audit.compile_observation(result)

    def test_failed_header_baseline_cannot_turn_every_symbol_into_absence(self):
        with tempfile.TemporaryDirectory() as directory:
            def run(args, timeout=20):
                return CommandResult(0, '/SDK') if '--show-sdk-path' in args else CommandResult(1, 'compile error')
            with patch.object(audit, 'scalar', return_value='27.2'), self.assertRaisesRegex(PreflightError, 'baseline'):
                audit.audit(Path(directory), run)

    def test_mixed_compile_results_remain_surface_observations_not_security_proof(self):
        with tempfile.TemporaryDirectory() as directory:
            calls = []
            def run(args, timeout=20):
                if '--show-sdk-path' in args:
                    return CommandResult(0, '/SDK')
                calls.append(args)
                return CommandResult(1 if Path(args[-1]).stem == 'xpc_message_audit_getter' else 0, '')
            with patch.object(audit, 'scalar', return_value='27.2'), redirect_stderr(io.StringIO()):
                report = audit.audit(Path(directory), run)
            self.assertEqual(report['observations']['xpc_message_audit_getter'], 'compile_failed')
            self.assertEqual(report['observations']['task_name_right'], 'compile_pass')
            self.assertEqual(report['exact_instance_binding'], 'not_proven')
            self.assertEqual(report['native_authorization'], 'not_tested')
            self.assertFalse(report['m8_complete'])
            self.assertTrue(all('-fsyntax-only' in args for args in calls))


if __name__ == '__main__':
    unittest.main()
