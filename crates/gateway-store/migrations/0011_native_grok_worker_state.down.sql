DROP TABLE grok_account_quota_windows;
DROP INDEX grok_accounts_worker_due_idx;
ALTER TABLE grok_accounts DROP COLUMN worker_claim_expires_at_ms;
ALTER TABLE grok_accounts DROP COLUMN worker_claim_id;
ALTER TABLE grok_accounts DROP COLUMN worker_claim_kind;
ALTER TABLE grok_accounts DROP COLUMN quota_sync_failure_count;
ALTER TABLE grok_accounts DROP COLUMN last_quota_sync_at_ms;
ALTER TABLE grok_accounts DROP COLUMN quota_sync_due_at_ms;
