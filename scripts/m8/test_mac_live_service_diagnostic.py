import json
import os
from pathlib import Path
import platform
import socket
import shutil
import subprocess
import sys
import tempfile
import unittest
import mac_live_service_diagnostic as d

class ParserTests(unittest.TestCase):
    def test_process_names_and_relations_are_redacted(self):
        raw='1 0 /PRIVATE/cryptex-desktop\n2 1 /PRIVATE/cryptexd\n3 2 /PRIVATE/cryptexd\n4 3 /PRIVATE/syncthing\n5 4 /PRIVATE/syncthing\n'
        result=d.parse_processes(raw)
        self.assertEqual(result,dict(desktop=1,daemon=2,engine=2,daemon_parent_desktop=1,daemon_parent_daemon=1))
        self.assertNotIn('PRIVATE',json.dumps(result))
        with self.assertRaises(d.Refused): d.parse_processes(raw+'1 0 private\n')

    def test_listener_parser_exports_only_counts(self):
        self.assertEqual(d.parse_listeners('p42\nf7\nn192.168.50.104:61942\n'),
                         dict(listeners=1,expected_lan=1,wildcard=0,loopback=0))
        self.assertEqual(d.parse_listeners('')['listeners'],0)
        for raw in ('PRIVATE RAW ERROR','fPRIVATE','n127.0.0.1:1234'):
            with self.assertRaises(d.Refused): d.parse_listeners(raw)

    def test_log_markers_never_export_raw_lines(self):
        raw=b'PRIVATE SECRET Listener starting\nPRIVATE PATH exiting\n'
        result=d.summarize(raw,False)
        self.assertEqual(result['marker_counts']['listener_started'],1)
        self.assertNotIn('PRIVATE',json.dumps(result))

@unittest.skipIf(os.name=='nt','POSIX handle reader')
class NativeTests(unittest.TestCase):
    def test_owned_log_is_bounded_and_never_changed(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder).resolve();path=root/'native.log'
            raw=b'PRIVATE\n'*200000+b'Listener starting\n'
            path.write_bytes(raw)
            report=d.read_log(path)
            self.assertTrue(report['tail_only'])
            self.assertEqual(report['bytes_read'],1024*1024)
            self.assertEqual(path.read_bytes(),raw)
            os.link(path,root/'hardlink')
            with self.assertRaises(d.Refused): d.read_log(path)

    def test_alias_and_fifo_are_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder).resolve();path=root/'native.log';path.write_bytes(b'private')
            (root/'alias').symlink_to(path)
            with self.assertRaises(OSError): d.read_log(root/'alias')
            os.mkfifo(root/'fifo')
            with self.assertRaises(d.Refused): d.read_log(root/'fifo')

    def test_wrong_profile_never_reads_log_or_processes(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder).resolve();(root/'identity').mkdir()
            for name in ('brain_identity.json','local_device.json'):
                (root/'identity'/name).write_text('{}')
            with self.assertRaises(d.Refused):
                d.collect(root,lambda _:self.fail('log'),lambda:self.fail('process'),lambda:self.fail('listener'))

    def test_observer_output_and_time_are_bounded(self):
        with self.assertRaisesRegex(d.Refused,'observer_output_bound'):
            d.run_bounded([sys.executable,'-c','print("x"*4096)'],limit=64)
        with self.assertRaisesRegex(d.Refused,'observer_timeout'):
            d.run_bounded([sys.executable,'-c','import time;time.sleep(5)'],timeout=0.05)

    @unittest.skipUnless(platform.system()=='Darwin','actual Mac long executable path')
    def test_process_counts_include_long_bundle_paths(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder).resolve()/('a'*100)/('b'*100)/'Contents/MacOS'
            root.mkdir(parents=True)
            binary=root/'cryptexd'
            shutil.copyfile('/bin/sleep',binary);binary.chmod(0o700)
            before=d.processes()['daemon']
            child=subprocess.Popen([str(binary),'15'])
            try:
                self.assertIsNone(child.poll())
                self.assertEqual(d.processes()['daemon'],before+1)
            finally:
                if child.poll() is None: child.terminate()
                try: child.wait(timeout=3)
                except subprocess.TimeoutExpired: child.kill();child.wait()

    @unittest.skipUnless(platform.system()=='Darwin','actual Mac ps/lsof')
    def test_actual_mac_commands_and_session_gate(self):
        result=d.processes()
        self.assertEqual(set(result),{'desktop','daemon','engine','daemon_parent_desktop','daemon_parent_daemon'})
        with socket.socket() as server:
            server.bind(('127.0.0.1',61942));server.listen()
            try:
                result=d.listeners()
            except d.Refused:
                raw=d.run_bounded(['/usr/sbin/lsof','-nP','-a','-u',str(os.getuid()),'-iTCP:61942','-sTCP:LISTEN','-Fpn'])
                self.fail('lsof field shapes: '+json.dumps([(line[:1],line[1:].isdigit(),line.endswith(':61942')) for line in raw.splitlines()]))
            self.assertGreaterEqual(result['loopback'],1)
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder).resolve();(root/'identity').mkdir()
            (root/'identity/brain_identity.json').write_text(json.dumps({'brain_id':d.BRAIN}))
            (root/'identity/local_device.json').write_text(json.dumps({'brain_id':d.BRAIN,'device_id':d.MAC}))
            result=d.collect(root)
            self.assertEqual(result['native_log']['status'],'absent')
            self.assertEqual(result['process_names_not_identity_proof']['status'],'observed')
            self.assertEqual(result['port_61942_not_identity_proof']['status'],'observed')
            self.assertFalse(result['application_processes_changed'])

if __name__=='__main__': unittest.main()
