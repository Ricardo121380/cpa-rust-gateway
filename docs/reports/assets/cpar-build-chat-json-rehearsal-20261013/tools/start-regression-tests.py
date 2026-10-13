"""Run the actual startup probe and listener checks offline; no process or network."""
import ast
import os
import pathlib
import tempfile
import unittest
import urllib.error

SOURCE = pathlib.Path(__file__).with_name('remote-start.py')


class StartupBoundary(unittest.TestCase):
    def test_first_management_failure_is_never_retried(self):
        tree = ast.parse(SOURCE.read_text())
        launch = next(node for node in tree.body if isinstance(node, ast.Try))
        begin = next(i for i, node in enumerate(launch.body)
                     if isinstance(node, ast.Assign) and ast.unparse(node.targets[0]) == 'deadline')
        end = next(i for i, node in enumerate(launch.body)
                   if isinstance(node, ast.Assign) and ast.unparse(node.targets[0]) == 'active')
        probe = compile(ast.Module(body=launch.body[begin:end], type_ignores=[]), 'actual-startup-probe', 'exec')
        calls = []
        def get(path):
            calls.append(path)
            if len(calls) == 1:
                raise urllib.error.HTTPError('http://loopback/', 401, 'synthetic unauthorized', {}, None)
            return []
        class Clock:
            def monotonic(self): return 0
            def sleep(self, seconds): pass
        identity = {'pid': 123, 'process_start': '456'}
        with self.assertRaises(urllib.error.HTTPError):
            exec(probe, {'get': get, 'time': Clock(), 'urllib': __import__('urllib'),
                         'listener_ports': lambda proc: [19180, 19181], 'proc': object(),
                         'read_owned_identity': lambda: identity, 'startup_identity': identity})
        self.assertEqual(calls, ['/admin/config-versions'])

    def test_both_loopback_listeners_must_belong_to_the_actual_pid(self):
        tree = ast.parse(SOURCE.read_text())
        function = next((node for node in tree.body if isinstance(node, ast.FunctionDef)
                         and node.name == 'listener_ports'), None)
        self.assertIsNotNone(function, 'actual startup must verify listener socket ownership')
        namespace = {'os': os, 'pathlib': pathlib}
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'actual-listener-check', 'exec'), namespace)
        with tempfile.TemporaryDirectory() as root:
            root = pathlib.Path(root); proc = root / '123'
            (proc / 'fd').mkdir(parents=True); (root / 'net').mkdir()
            (proc / 'fd/3').symlink_to('socket:[101]'); (proc / 'fd/4').symlink_to('socket:[102]')
            def row(port, inode, address='0100007F'):
                return f'0: {address}:{port:04X} 00000000:0000 0A 0:0 00:0 0 0 0 {inode}\n'
            (root / 'net/tcp6').write_text('header\n')
            (root / 'net/tcp').write_text('header\n' + row(19180, 101) + row(19181, 102))
            self.assertEqual(namespace['listener_ports'](proc), [19180, 19181])
            (root / 'net/tcp').write_text('header\n' + row(19180, 999) + row(19181, 102))
            with self.assertRaises(AssertionError): namespace['listener_ports'](proc)
            (root / 'net/tcp').write_text('header\n' + row(19180, 101, '00000000') + row(19181, 102))
            with self.assertRaises(AssertionError): namespace['listener_ports'](proc)


if __name__ == '__main__':
    unittest.main()
