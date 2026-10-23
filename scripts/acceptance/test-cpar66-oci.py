#!/usr/bin/env python3
"""Offline artifact-binding negatives and failure-path cleanup regression for #66."""
import argparse
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("oci66", Path(__file__).with_name("cpar66-oci.py"))
oci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oci)


class Acceptance(unittest.TestCase):
    def test_complete_archive_binding(self):
        binding = oci.archive_binding(args.artifact, args.docker_archive)
        self.assertEqual(binding["status"], "PASS")
        self.assertEqual(len(binding["layers"]), 4)

    def test_docker_bridge_config_and_layer_order_are_load_bearing(self):
        for field, expected in (("Config", "Docker config mismatch"), ("Layers", "Docker layer order mismatch")):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temp:
                altered = Path(temp) / "altered.tar"
                with tarfile.open(args.docker_archive) as source, tarfile.open(altered, "w") as target:
                    for row in source:
                        data = source.extractfile(row).read() if row.isfile() else None
                        if row.name == "manifest.json":
                            bridge = json.loads(data)
                            if field == "Config":
                                bridge[0][field] = "blobs/sha256/" + "0" * 64
                            else:
                                bridge[0][field].reverse()
                            data = json.dumps(bridge).encode()
                            row.size = len(data)
                        target.addfile(row, io.BytesIO(data) if data is not None else None)
                with self.assertRaisesRegex(AssertionError, expected):
                    oci.archive_binding(args.artifact, altered)

    def test_wrong_image_binding_never_contacts_engine(self):
        with tempfile.TemporaryDirectory() as temp:
            run_args = SimpleNamespace(output=Path(temp) / "run", artifact=args.artifact,
                                       docker_archive=args.docker_archive, image="sha256:" + "0" * 64,
                                       docker_host="unix:///not-used.sock")
            with patch.object(subprocess, "run", side_effect=AssertionError("unexpected engine contact")) as docker:
                with self.assertRaisesRegex(AssertionError, "wrong image/artifact binding"):
                    oci.outside(run_args)
                docker.assert_not_called()
            self.assertEqual(json.loads((run_args.output / "run.json").read_text())["status"], "FAIL")

    def failure_matrix(self):
        matrix = Mock()
        matrix.process = None
        matrix.report = {"status": "RUNNING"}
        matrix.peer_thread.is_alive.return_value = False
        return matrix

    def test_target_cleanup_failure_cannot_leave_pass(self):
        matrix = self.failure_matrix()
        matrix.cleanup.side_effect = RuntimeError("injected target cleanup failure")
        with patch.object(oci.socket, "socket") as sock:
            sock.return_value.__enter__.return_value.connect_ex.return_value = 111
            with self.assertRaisesRegex(RuntimeError, "injected target cleanup failure"):
                oci.qualify(matrix)
        self.assertEqual(matrix.report["status"], "FAIL")
        matrix.save.assert_called_once()

    def test_receiver_cleanup_failure_still_cleans_targets_and_fails(self):
        matrix = self.failure_matrix()
        matrix.peer.server_close.side_effect = RuntimeError("injected receiver cleanup failure")
        with self.assertRaisesRegex(RuntimeError, "injected receiver cleanup failure"):
            oci.qualify(matrix)
        self.assertEqual(matrix.report["status"], "FAIL")
        matrix.cleanup.assert_called_once()
        matrix.save.assert_called_once()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--docker-archive", type=Path, required=True)
    args = parser.parse_args()
    unittest.main(argv=[__file__])
