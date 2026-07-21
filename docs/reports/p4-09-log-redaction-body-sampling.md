# P4-09 Default-deny log redaction and bounded Body sampling

| Field | Value |
|---|---|
| Plan version | `v1.7` |
| Task | `P4-09` |
| Status | `LOCAL_PASS_PENDING_CI`; implementation, review, targeted tests, Secret scan, and local complete Gate passed; GitHub Code Gate is pending for the code delivery commit. |
| Scope level / execution budget | `L`; security-sensitive observability boundary, 45-minute scoped code-delivery target excluding external Gates |
| Task Card | `gateway-observability` default-deny HTTP log projection and bounded explicit JSON Body sampling only; no raw Body/header/URL logging, HTTP/Router hot-path export, Store migration, second receiver, external log transport, real Provider request, or P5 work |
| References | `G01`, `G03`, `G15`, `G22`, `K08`; [ADR-0031](../adr/ADR-0031-default-deny-log-redaction-and-body-sampling.md); [BC-OBS-004](../contracts/BC-OBS-004-default-deny-log-redaction-and-body-sampling.md) |

## Detailed subplan and invariant

1. Keep the default policy at zero Body retention and zero raw header-value retention.
2. Add an explicit deterministic sampling ratio with a finite 16 KiB JSON-only parser limit.
3. Create one safe HTTP log record that contains only fixed metadata, summary headers, and either a
   reasoned omission or recursively redacted JSON.
4. Prove serialized and `Debug` output never carries sentinel Client Key/Credential/Body/Header
   strings; make malformed, duplicate, non-JSON, and oversize input fail closed.

No code in this Task may capture a raw body prefix, trust a duplicate `Content-Type`, log an
arbitrary header name/value, alter P4-08 event schema, write SQLite, initiate a Provider request,
or add P5 behavior.

## Implemented scope

- Added `LogRedactionPolicy`, default-disabled `BodySamplingPolicy`, fixed safe HTTP directions,
  safe header summary, omission reasons, and `SanitizedHttpLogRecord` to
  `gateway-observability`.
- Explicit sampling uses only the Request ID for deterministic bounded selection; it accepts only
  unambiguous JSON under the configured 16 KiB maximum.
- Added recursive JSON key/value redaction, bounded header classification, safe `tracing` emission,
  and regression tests for default denial, explicit sampling, oversize/non-JSON, duplicate content
  type, safe `Debug`/JSON forms, and invalid configurations.

## Local targeted verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS after formatter normalization. |
| `cargo test --locked -p gateway-observability` | PASS; 13 tests, including P4-08 correlation plus P4-09 default-deny, sampled redaction, finite body, duplicate-header, invalid-body, non-selected, and invalid-policy regressions. |
| `cargo clippy --locked -p gateway-observability --all-targets --all-features -- -D warnings` | PASS after two direct idiomatic control-flow/allocation corrections. |
| `scripts/secret-scan.sh --all` | PASS; tracked working tree contains no detected Secret. |
| `CHECK_REPORT_PATH=tmp/p4-09-full-check.md ./scripts/check.sh full` | PASS in 33 seconds (started `2026-07-21T20:40:45+08:00`, completed `2026-07-21T20:41:18+08:00`); shell/CI/plan guards, format, workspace Clippy/tests, source and crate policy, document links, Secret scan, whitespace, pinned quality tools, `cargo deny`, and RustSec audit all passed. |

No ignored real-test harness ran and no Provider request was sent.

## Review and execution measurement

Focused security review verified that sampling cannot be enabled implicitly, no body parser runs for
a nonselected or non-JSON input, no raw body prefix survives any rejected path, duplicate
`Content-Type` fails closed, and recursive redaction applies before every serializable/Debug form.
It added direct coverage for broad key names and for `not_selected`, invalid UTF-8, and malformed
JSON omission paths. The first focused Clippy run found two direct idiomatic control-flow/allocation
corrections; after those and the review coverage additions, one final complete Gate passed. GitHub
evidence is added only after its immutable Code Gate finishes.

## Closeout boundary

This code-delivery commit creates the P4-09 ADR, contract, report, and index entries. After the
GitHub Code Gate passes, exactly one docs-only closeout records that immutable evidence and changes
P4-09 to `DONE`. G4 remains blocked until that docs-only Gate passes.
