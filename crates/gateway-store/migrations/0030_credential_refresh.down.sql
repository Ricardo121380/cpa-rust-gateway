DROP TABLE credential_refresh_state;
ALTER TABLE grok_accounts DROP COLUMN continuation_current_revision;
ALTER TABLE grok_accounts DROP COLUMN continuation_floor_revision;
