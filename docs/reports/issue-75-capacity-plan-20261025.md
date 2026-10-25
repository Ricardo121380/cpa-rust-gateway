# #75 frozen synthetic capacity experiment

Frozen before measurement, 2026-10-25 (Asia/Shanghai). Source baseline:
`84fbfbf778b582f746c1cafe31a2ce58962e8f77`; Cargo.lock SHA-256
`a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d`.
Independent branch `feature/75-history-backup-capacity`; owned target/logs/databases only.
ADR-0132 reserved for this ticket; no migration reserved or required (schema39).

## Fixed input

- 25,000 synthetic complete Request lineages, each Request/Attempt/Usage/terminal
  (100,000 events), in addition to the small public-HTTP seed. Dates start at
  1700000000000 ms, evenly spaced across 30 days. Identity prefix `capacity75-`.
- Distribution cycles through ordinary successful, native failed Usage and native
  Messages successful templates captured through the existing #65 controlled TLS
  peer, in a 8:1:1 ratio. All six token dimensions and null/zero provenance stay as
  captured; exact source-event ledger identity is rewritten with each lineage.
  Existing unpriced costs remain null. No synthetic clone claims Provider evidence.
- 40,000 quota observations spread across the same 30 days; two account namespaces,
  two structural windows; observed/exhausted/unknown/failed and partial states.
  These direct SQL scale fixtures are distinct from the #74 revision-fenced append
  acceptance. No ciphertext cloning under different AAD identities.
- Small real public-HTTP synthetic samples retain encrypted native Usage, Messages
  summary and request metadata with their exact original owner/Request/Attempt.
  Active/disabled ordinary and native accounts, two routes, owner/sibling/restricted
  keys and access groups come from the existing #65 seed.
- One concurrent append thread, in batches of 10 complete lineages, at most 100
  additional lineages. Record its committed IDs; backup must contain complete
  committed transactions, and the original source remains intact.

## Measurement and boundaries

First measure the unchanged baseline: database/WAL bytes; selective #72 capture,
JSON/CSV/JSONL pages, full-window explicit 413, quota history; encrypted-backup
success or exact failure. Record wall time, OS process high-water RSS separately
from sampled serve RSS, artifact bytes and receiver counts. Compiler/driver RSS
is not gateway RSS. Limit cargo jobs to two and load concurrency to one writer.
No production SLA or universal maximum supported history follows from one machine.

Then change only observed capacity limitations, preserving selected-source/result
bounds, exact lineage, authenticated format, validation and empty-target publication.
Repeat on unchanged input. Restore to a new target and let a new stock gateway
process adopt it; compare streaming table digests and protected/public behavior,
including schema39 quota history and #72 frozen query/export. Cover size rejection,
write/space failure and interrupted restore without replacing source or target.
Reuse wrong-key/tamper/incompatible/existing-target negatives.

No history deletion, retention, rollup, shared database, remote change, deployment,
real Provider or actual-client qualification. Raw artifacts/keys/diagnostics stay
in private ignored output. Results and preserved failures are reported separately.
