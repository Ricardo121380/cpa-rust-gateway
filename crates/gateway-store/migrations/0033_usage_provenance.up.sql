ALTER TABLE billing_ledger_entries ADD COLUMN usage_evidence_json TEXT NOT NULL
    DEFAULT '{"provenance":"unknown","input_accounting":"unknown"}'
    CHECK (json_valid(usage_evidence_json) AND json_type(usage_evidence_json) = 'object');
