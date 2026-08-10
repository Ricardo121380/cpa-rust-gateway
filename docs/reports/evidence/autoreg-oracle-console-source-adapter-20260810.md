# Autoreg Oracle Console source adapter receipt

Date: 2026-08-10
Status: `PASS_FOR_BOUNDED_CANARY`
Scope: one completed Oracle Autoreg Console task → root-only FIFO → CPAR `grok_console` batch

## Source and adapter

- The source selector accepted exactly one completed task (`target_count=1`,
  `completed_count=1`, `failed_count=0`) and exactly one SSO file.
- The task session was required to match the source identity and the SSO value on all four
  required cookies: `.x.ai`/`.grok.com` × `sso`/`sso-rw`.
- Each required cookie was required to be `secure`, `httpOnly`, `path=/`, future-dated, and
  non-duplicated. The adapter rejects unknown session fields, wrong provider shapes, stale
  files, symlinks, unsafe permissions, oversized input, and non-finite expiry values.
- The normalized identity is carried only through the root-only pipe. CPAR hashes it in the
  provider-scoped identity boundary; it is not written to this receipt or diagnostics.
- `refresh_due_at_ms` is derived from the earliest required cookie expiry minus 24 hours,
  clamped to the current time when the lead window has elapsed.
- The adapter is committed at `05da177` and its deployed source hash was checked against the
  local file. Console model input remains `allowlisted_pending_native_probe` until the same
  batch native probe succeeds; it is not inferred from the Autoreg task name or plan.

## Offline and isolated evidence

Local checks completed before the server operation:

```text
Autoreg exporter contract tests: 9/9 PASS
Python compile, shell syntax, whitespace and tracked-secret checks: PASS
provider-grok Console/account-pool/migration/reauth tests: PASS
strict provider-grok Clippy and cargo fmt: PASS
```

On Oracle, the root-only adapter emitted one record and the CPAR importer accepted one record
and created one encrypted account. The same batch-scoped native Console probe reported:

```text
native_grok_import=PASS source_records=1 accepted_accounts=1 rejected_records=0 created_accounts=1
native_grok_probe=PASS provider=grok_console account_attributed=true health=available quota=available canonical_complete=true chat=true responses=true messages=true
```

The isolated gateway copy then passed one request for each of Responses, Chat Completions and
Messages in both JSON and SSE mode: `6/6`, with valid content types and terminal semantic
events. The isolated batch was deliberately rolled back afterwards; SQLite integrity remained
`ok`, the batch account count was zero, and the temporary listeners were stopped.

## Deliberate boundaries

This receipt proves a bounded source adapter and an isolated Console canary only. It does not
enable an Autoreg scheduler, implement a live Console reauthentication executor, or turn the
adapter into an unattended replenishment service. Those require a separate bounded reauth/worker
change and operator approval. Plan/quota/platform/email display metadata is a later management
projection; the source email is used only for stable identity before CPAR hashing.
