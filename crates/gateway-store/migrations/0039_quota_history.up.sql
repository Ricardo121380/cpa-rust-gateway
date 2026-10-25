-- Observation evidence is independent of account deletion and serving quota state.
CREATE TABLE quota_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_kind TEXT NOT NULL CHECK (account_kind IN ('credential','native')),
    config_version_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    provider TEXT NOT NULL,
    source TEXT NOT NULL,
    window TEXT NOT NULL,
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    evidence_json TEXT NOT NULL CHECK (json_valid(evidence_json)),
    UNIQUE(account_kind,config_version_id,account_id,revision,provider,source,window,observed_at_ms)
) STRICT;
CREATE INDEX quota_history_time ON quota_history(observed_at_ms,id);
CREATE INDEX quota_history_series ON quota_history(account_kind,config_version_id,account_id,provider,source,window,observed_at_ms,id);
