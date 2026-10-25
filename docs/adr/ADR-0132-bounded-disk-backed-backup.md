# ADR-0132: Bounded disk-backed encrypted backup

- Status: Accepted
- Date: 2026-10-25
- Task / Contract: [#75](https://github.com/Ricardo121380/cpa-rust-gateway/issues/75), OPT-08;
  [BC-MGMT-007](../contracts/BC-MGMT-007-encrypted-control-plane-backup.md)

## Context

The [frozen experiment](../reports/issue-75-capacity-plan-20261025.md) generated
25,000 complete synthetic lineages plus 40,000 quota observations on schema39.
The unchanged baseline's 98,484,224-byte database cannot pass the 16 MiB backup
limit. Its selective Usage capture, explicit full-window limit and quota page
completed; that evidence does not require changing the query model or limits.

CPA remains fixed at v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`:
`internal/api/handlers/management/usage.go::GetUsageQueue` and
`internal/redisqueue/queue.go::PopOldest` consume a queue. They do not define this
CPAR SQLite snapshot/restore contract. No upstream source/fixture is copied.

## Decision

Extend ADR-0074 with a disk-backed v2 artifact and retain v1 compatibility.
Keep the existing configured target, independent Backup Key, integrity/migration
checks and atomic no-replacement publication. Do not add an HTTP download route,
schema migration, retention, rollup, archive service or replacement ledger.

V2 is exactly: magic `CPABKP02` (8 bytes), version 2 (1 byte), source schema
(4 bytes big endian), random nonce (24 bytes), plaintext length (8 bytes big
endian), then chunks. Plaintext length is 1..134217728 bytes (128 MiB). Each chunk
contains up to 65536 plaintext bytes plus its 16-byte XChaCha20-Poly1305 tag;
the final size is determined by the authenticated length. Artifact maximum is
134250541 bytes. The complete 45-byte header plus the 8-byte big-endian chunk
index is each chunk's AAD. The chunk nonce XORs that index into the last eight
bytes of the fresh random nonce. Indexes never repeat within an artifact.
Exact chunk count/length and EOF are required: deletion, reordering, splicing,
truncation, altered header/length and appended bytes fail closed.

SQLite pins one source read transaction for the size check and coherent disk snapshot; an embedding-owned active transaction remains owned by its caller. Creation requires `StepResult::Done`;
Busy/Locked is an unavailable snapshot, with no unbounded retries. Before copying,
page count/size rejects an oversized source; the completed snapshot size is checked
again. Encryption/decryption uses one 64 KiB buffer with tag allowance. Disk-backed
readers support legacy v1 under its original 16 MiB ceiling; legacy Vec APIs retain
their old behavior and explicitly reject v2. Operator file creation publishes only
after full encryption and sync. Raw writer callers must discard failed output.

Management binary upload spools into a configured 0600 anonymous file. A single
admission permit travels with the file through every blocking upload write and
validation/publication, including HTTP cancellation after blocking work begins. A concurrent operation fails 503 rather
than allocating another full artifact. Actual body length and Content-Length obey
the artifact ceiling. Existing read/export budgets and authorization remain intact.

## Consequences

Operators need space for source, a coherent snapshot, encrypted artifact/upload
and decrypted staging. The finite 128 MiB database bound is an implementation limit,
not a production SLA or a claim about arbitrarily large historical databases.
Disk-backed v2 needs this reader or a compatible newer reader; older readers reject
it. Retain the original binary/artifact and all original Master Key versions.
The backup excludes external key directories and independent admin-auth storage.
Staging remains private plaintext: SIGKILL can retain an unpublished 0600 staging
file. Preserve it for diagnosis; do not blindly retry into a published target or
delete source history. No physical secure-erasure or power-loss durability claim.

## Alternatives considered

Raising the Vec/HTTP memory ceiling would multiply whole-database buffers and
does not satisfy the measured capacity need with bounded memory. Compression alone
would make qualification depend on compressibility. An unbounded export service
or history deletion changes the product and is unnecessary for this dataset.

## Validation and rollback

Store and protected HTTP regressions retain legacy wrong-key, tamper, unsupported
history and existing-target behavior. New checks cover v2 chunk authentication,
size bounds, writer failure and interrupted readers. The owned process experiment
measures large backup/restore, one concurrent writer, file-size write-limit failure,
SIGKILL during restoration, explicit retry to an empty target and actual new serve
adoption of schema39, old ledger/evidence and quota history. Full results are
recorded separately, not inferred from this decision. Rollback to an older reader
requires its original supported-format artifact; never down-migrate or discard
schema39 history merely to make the older binary start.
