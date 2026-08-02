#!/usr/bin/env python3
"""Regression tests for the P12-08F3 isolated OpenClaw rehearsal."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
DRIVER = REPO_ROOT / "scripts" / "p12-08f3-openclaw-migration-dry-run.py"


def fixture() -> dict[str, object]:
    return {
        "models": {
            "mode": "merge",
            "providers": {
                "fixture-provider": {
                    "baseUrl": "https://legacy.invalid/v1",
                    "apiKey": "fake",
                    "api": "openai-responses",
                    "models": [
                        {
                            "id": "fixture-model",
                            "name": "Fixture",
                            "api": "openai-responses",
                        }
                    ],
                }
            },
        },
        "agents": {
            "defaults": {
                "model": {"primary": "fixture-provider/fixture-model", "fallbacks": []},
                "models": {"fixture-provider/fixture-model": {"alias": "fixture"}},
            }
        },
    }


class OpenClawMigrationDryRunTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="p12-08f3-test-")
        self.root = Path(self.temp.name)
        self.source = self.root / "openclaw.json"
        self.source.write_text(json.dumps(fixture()), encoding="utf-8")
        self.source.chmod(0o600)
        self.validator = self.root / "openclaw"
        self.validator.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os\n"
            "with open(os.environ['OPENCLAW_CONFIG_PATH'], encoding='utf-8') as handle:\n"
            "    json.load(handle)\n"
            "print(json.dumps({'valid': True, 'warnings': []}))\n",
            encoding="utf-8",
        )
        self.validator.chmod(0o700)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def invoke(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                str(DRIVER),
                "--source",
                str(self.source),
                "--provider-id",
                "fixture-provider",
                "--validator",
                str(self.validator),
                *extra,
            ],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_rehearsal_migrates_rolls_back_and_preserves_source(self) -> None:
        before = self.source.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("temporary_migration=PASS", result.stdout)
        self.assertIn("protocol_selection_preserved=PASS", result.stdout)
        self.assertIn("temporary_rollback=PASS", result.stdout)
        self.assertEqual(self.source.read_bytes(), before)

    def test_rejects_group_readable_source(self) -> None:
        self.source.chmod(0o640)
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("source_config_permissions_too_broad", result.stderr)

    def test_rejects_symlink_source(self) -> None:
        target = self.source
        link = self.root / "linked.json"
        link.symlink_to(target)
        self.source = link
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("source_config_must_be_regular_file", result.stderr)

    def test_rejects_unknown_provider_without_touching_source(self) -> None:
        before = self.source.read_bytes()
        result = subprocess.run(
            [
                str(DRIVER),
                "--source",
                str(self.source),
                "--provider-id",
                "missing-provider",
                "--validator",
                str(self.validator),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("provider_not_found", result.stderr)
        self.assertEqual(self.source.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
