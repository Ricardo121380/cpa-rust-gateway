#!/usr/bin/env python3
"""Source archives must preserve removals as well as additions in a dirty checkout."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("candidate_source", Path(__file__).with_name("cpar66-source.py"))
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


class SourceArchiveTests(unittest.TestCase):
    def test_deleted_frontend_is_absent_and_bound_as_an_overlay(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            root.mkdir()
            def git(*args):
                return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.DEVNULL)
            git("init", "--quiet")
            (root / "Cargo.lock").write_text("version = 4\n")
            frontend = root / "web/prism/index.html"
            frontend.parent.mkdir(parents=True)
            frontend.write_text("old frontend\n")
            git("add", "Cargo.lock", "web/prism/index.html")
            git("-c", "user.name=Source fixture", "-c", "user.email=fixture@example.invalid",
                "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "--quiet", "-m", "fixture")
            frontend.unlink()
            output = Path(temporary) / "evidence"
            with contextlib.redirect_stdout(io.StringIO()):
                source.freeze(root, output)
            binding = json.loads((output / "source-binding.json").read_text())
            self.assertEqual(binding["source_form"], "base_commit_plus_overlay")
            self.assertEqual(binding["deleted_paths"], ["web/prism/index.html"])
            with tarfile.open(output / "source.tar") as archive:
                self.assertEqual(archive.getnames(), ["Cargo.lock"])
            self.assertEqual(source.verify(root, output / "input-manifest.json"), 1)
            frontend.write_text("old frontend restored\n")
            with self.assertRaisesRegex(ValueError, "deleted source input reappeared"):
                source.verify(root, output / "input-manifest.json")
            frontend.unlink()
            (root / "Cargo.lock").write_text("changed\n")
            with self.assertRaises(ValueError):
                source.verify(root, output / "input-manifest.json")


if __name__ == "__main__":
    unittest.main()
