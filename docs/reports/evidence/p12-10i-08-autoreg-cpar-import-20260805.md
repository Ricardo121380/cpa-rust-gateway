# P12-10I-08 Autoreg repair and CPAR account-pool import

Status: `BLOCKED_WITH_EVIDENCE`

## Scope

Repair the Jakarta Autoreg failure observed after grok2api was stopped, complete one bounded
registration, and import the resulting Build OAuth into CPAR without exposing a management
listener or publishing a Grok route.

No credential value, account identity, endpoint value, request body, response body, token
fingerprint, or password is recorded in this receipt.

## Root causes and repairs

1. Jakarta root storage reached 100%. FlareSolverr could not create its temporary Chrome
   directory and returned `Errno 28`. Nineteen unopen temporary cache files (about 1.6 GB) were
   removed after an `lsof` check; the filesystem returned to 97% with free space.
2. The x.ai sign-up page changed Next.js bundle URLs to include a query string after `.js`.
   Autoreg's `fetch_config()` regex did not retain that query string, so it found no JavaScript
   bundle and failed closed with `Config fetch failed`. The regex now accepts the optional query
   suffix; the previous file is retained in a root-only backup.
3. grok2api is intentionally stopped. Autoreg now treats that legacy sink as optional when
   `api.mode=disabled`, skips its login/import retries, and does not mark OAuth/CPA completion as
   incomplete because the stopped legacy sink is unavailable. The patched module passes
   `py_compile`; a synthetic sink-contract check returned `oauth_ok=true`, `cpa_ok=true`,
   `grok2api_ok=true`, `grok2api_skipped=true`; the console container was restarted from the
   bind-mounted source.

## Bounded registration evidence

- Autoreg task target: `1`.
- Result: `1/1` completed with registration success after the bundle-regex repair.
- FlareSolverr prewarm: all five configured x.ai hosts completed successfully.
- The old CPA-compatible sink succeeded for this task; the grok2api sink failed only on the
  pre-patch task and was not retried after the optional-sink repair.

## CPAR import evidence

- The exact ARM artifact already present on the Singapore VPS: `82ceb1d` release directory.
- First, a production database copy accepted one bounded `grok_build` record and then rolled it
  back; the copy ended with `quick_check=ok` and zero records for that batch.
- The production service was briefly stopped for a consistent preimage. The current symlink was
  moved from the schema-incompatible legacy artifact to the already-reviewed v12-compatible
  `82ceb1d` artifact; the previous target is retained as a rollback symlink.
- Batch `autoreg-task55-prod-20260805`: `source_records=1`, `accepted_accounts=1`,
  `rejected_records=0`, `created_accounts=1`, `unchanged_accounts=0`.
- Post-import metadata: one active enabled Build account, priority/weight/concurrency preserved;
  credential stored as CPAR ciphertext. `quick_check=ok` and no foreign-key errors.
- Service postcondition: systemd `active`, unauthenticated data probe `401`, management probe
  `200`, and the management listener remains loopback-only.
- Active configuration remained `p12-09-codex-production-v2`; no Grok route or public traffic was
  published.

## Review and remaining boundary

The account is now present in CPAR's native encrypted pool, but this receipt does not claim live
Grok traffic success: the active production graph still has no native Grok route. The one-off
operator stream used the existing Mac SSH aliases because Jakarta has no pre-authorized secure
cross-host import command. Automatic future Autoreg-to-CPAR delivery remains a separate transport
task; no public management exposure is acceptable.

Rollback is the recorded database preimage plus the retained prior artifact symlink. Removing the
batch with the reviewed `grok-rollback` command is also available before any native route binding.
