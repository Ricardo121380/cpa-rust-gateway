"""Pure oracle regressions. These tests do not execute or impersonate an actual client."""
from pathlib import Path
import runpy
import unittest


READBACK = runpy.run_path(str(Path(__file__).with_name("cpar62-cli.py")))["c4_readback_matches_presence"]


class C4PresenceTests(unittest.TestCase):
    def test_later_absence_needs_independent_no_capture_proof(self):
        self.assertTrue(READBACK({"fields": {"client_metadata": {"present": False}},
                                 "c4": {"present": False, "absent_field_no_capture": True}}))
        self.assertFalse(READBACK({"fields": {"client_metadata": {"present": False}},
                                  "c4": {"present": False, "receipt": "UNKNOWN"}}))

    def test_present_field_needs_exact_raw_readback(self):
        row = {"fields": {"client_metadata": {"present": True}},
               "c4": {"present": True, "exact_raw_readback": True}}
        self.assertTrue(READBACK(row))
        row["c4"].pop("exact_raw_readback")
        self.assertFalse(READBACK(row))
        row["c4"] = {"present": False, "absent_field_no_capture": True}
        self.assertFalse(READBACK(row))

    def test_common_missing_fields_cannot_pass(self):
        self.assertFalse(READBACK({}))
        self.assertFalse(READBACK({"fields": {"client_metadata": {}}, "c4": {}}))


if __name__ == "__main__":
    unittest.main()
