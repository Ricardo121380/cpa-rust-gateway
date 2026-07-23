# P8-01 Grok Official API-key catalog report

| Field | Value |
|---|---|
| Plan version | `v1.38` |
| Task | `P8-01` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001` |
| Scope / budget | `M`; API-key/catalog boundary only; no Official inference, real Provider traffic, or server change |
| References | Matrix `C01`、`C31`、`C33`、`G24`、`G28`; [ADR-0054](../adr/ADR-0054-grok-official-api-key-catalog-boundary.md); [BC-PROVIDER-012](../contracts/BC-PROVIDER-012-grok-official-api-key-catalog.md) |

## Delivered evidence

`provider-grok` now owns a separate native `grok.official` catalog boundary. It stores an xAI
API key only in zeroizing request-scoped memory, creates one fixed `GET https://api.x.ai/v1/models`
request with standard Bearer authorization, and hands it to the shared client only after an exact
DNS-pinned egress admission. The request has no body and carries none of the Grok Build CLI or
browser-session headers.

The `GrokOfficialCatalogAdapter` implements P4's `ModelCatalogSource` for exactly one injected
Endpoint ID plus Credential ID. A sibling Credential or Endpoint target fails before dispatch.
Successful catalog data is bounded to 1 MiB and parsed with strict duplicate-name rejection. Only
unique, printable `data[*].id` model identities are accepted. Diagnostics redact every target,
key, bearer value, ID, model, and body value.

The strict JSON parser was moved from the Build request module to a neutral crate-private helper.
This keeps Official code from taking a module dependency on Build while retaining the same
duplicate-field rule for both isolated paths.

No xAI endpoint, credential, OAuth cache, server process, route, proxy/TUN rule, or production
configuration was read or changed. `CR-P7-G7-001` permits this local evidence only; G7 remains
blocked on Kiro account reauthentication and P8 cannot close or release before it passes.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_01_official_catalog` | PASS; 3 synthetic tests cover exact GET/headers/egress, scope isolation/strict fixture, and safe invalid-payload/status rejection. |
| `cargo test --locked -p provider-grok` | PASS; 51 active tests passed, 2 separately authorized P6 live harness tests remained ignored. |
| `cargo fmt --all -- --check` and `git diff --check` | PASS. |
| [`./scripts/check.sh full`](p8-01-local-full-check.md) | PASS; workspace format/Clippy/tests, source and crate-boundary policies, document links, Secret checks, dependency policy, and RustSec audit all passed. |
| Focused code review | PASS after extracting strict JSON parsing into an independent helper rather than coupling Official catalog code to the Build request module. |

## Rollback and next task

Rollback removes the P8-01 Official catalog module, test, narrow local dependency, ADR, contract,
and report. It has no external effect. P8-02 is the next local task: it will add the separate
Official Responses HTTP/SSE boundary and its synthetic Fixture tests. It must not run a real
Official E2E before G7 and P7's Delivery Gate satisfy `CR-P7-G7-001`.
