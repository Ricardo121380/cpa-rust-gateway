#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("cleanup", ROOT / "scripts/prism-retire-test-drafts.py")
cleanup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cleanup)


class CleanupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.directory = Path(self.temp.name)
        self.path = self.directory / "control.sqlite3"
        with sqlite3.connect(self.path) as db:
            for migration in sorted((ROOT / "crates/gateway-store/migrations").glob("*.up.sql")):
                db.executescript(migration.read_text())
            for identifier, status in [("live", "active"), ("test-draft", "draft")]:
                db.execute("INSERT INTO config_versions(id,status,created_at_ms,revision) VALUES(?,?,1,0)", (identifier, status))
                db.execute("INSERT INTO upstreams(config_version_id,id,name,kind,enabled) VALUES(?, 'upstream', 'Example', 'relay', 1)", (identifier,))
                db.execute("INSERT INTO management_audit_events(action,actor,occurred_at_ms,config_version_id) VALUES('config_created','test',1,?)", (identifier,))
            db.execute("INSERT INTO gateway_event_log(event_type,event_id,payload_json) VALUES('request','history','{}')")

    def tearDown(self):
        self.temp.cleanup()

    def test_retains_audit_history_and_active_resources_with_restorable_backup(self):
        dry = cleanup.retire(self.path, {"test-draft": 0})
        self.assertEqual(dry["mode"], "dry-run")
        backup = self.directory / "backup.sqlite3"
        result = cleanup.retire(self.path, {"test-draft": 0}, backup)
        self.assertTrue(result["protected_data_unchanged"])
        self.assertEqual(backup.stat().st_mode & 0o777, 0o600)
        with sqlite3.connect(self.path) as db:
            self.assertEqual(db.execute("SELECT status, revision FROM config_versions WHERE id='test-draft'").fetchone(), ("archived", 1))
            self.assertEqual(db.execute("SELECT config_version_id FROM upstreams").fetchall(), [("live",)])
            self.assertEqual(db.execute("SELECT count(*) FROM gateway_event_log").fetchone()[0], 1)
            self.assertEqual(db.execute("SELECT count(*) FROM management_audit_events").fetchone()[0], 2)
            self.assertEqual(db.execute("SELECT action FROM management_resource_audit_events").fetchone()[0], "test_draft_retired")
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute("DELETE FROM management_audit_events")
        with sqlite3.connect(backup) as db:
            self.assertEqual(db.execute("SELECT count(*) FROM upstreams WHERE config_version_id='test-draft'").fetchone()[0], 1)

    def test_rejects_active_or_changed_target(self):
        for targets in [{"live": 0}, {"test-draft": 1}]:
            with self.assertRaisesRegex(ValueError, "not a draft"):
                cleanup.retire(self.path, targets)

    def test_rejects_persistent_event_reference(self):
        with sqlite3.connect(self.path) as db:
            db.execute("INSERT INTO gateway_event_log(event_type,event_id,payload_json) VALUES('usage','observed',?)", ('{"context":{"config_version_id":"test-draft"}}',))
        with self.assertRaisesRegex(ValueError, "persistent events"):
            cleanup.retire(self.path, {"test-draft": 0})

    def test_rejects_descendant_and_published_history(self):
        with sqlite3.connect(self.path) as db:
            db.execute("INSERT INTO config_versions(id,parent_id,status,created_at_ms) VALUES('child','test-draft','draft',1)")
        with self.assertRaisesRegex(ValueError, "descendants"):
            cleanup.retire(self.path, {"test-draft": 0})
        with sqlite3.connect(self.path) as db:
            db.execute("DELETE FROM config_versions WHERE id='child'")
            db.execute("INSERT INTO management_audit_events(action,actor,occurred_at_ms,config_version_id) VALUES('config_published','test',1,'test-draft')")
        with self.assertRaisesRegex(ValueError, "publication"):
            cleanup.retire(self.path, {"test-draft": 0})


if __name__ == "__main__":
    unittest.main()
