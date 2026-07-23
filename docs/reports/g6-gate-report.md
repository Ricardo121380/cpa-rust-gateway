# G6 Grok Build phase gate report

| Field | Value |
|---|---|
| Plan | `v1.25` |
| Gate | `G6` |
| Date | `2026-07-23` |
| Verification branch | `codex/p6-grok-build` |
| Local closeout target | `e2496108949dd79870d43fbb5aedd0841e207ce7`; annotated tag `phase-p6-complete` |
| Local result | `PASS` |
| Remote Delivery Gate | `PASS`; [GitHub Actions 29971355397](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29971355397) accepted the tag SHA with Fast, Full supply-chain, and Required. |

## Local acceptance conclusion

P6-01 through P6-08 satisfy the local G6 conditions and are now `DONE`: the
`phase-p6-complete` tag Gate accepted closeout SHA `e2496108949dd79870d43fbb5aedd0841e207ce7`.
The implementation introduces no server,
account, route, proxy/TUN, or management-HTTP change. It confines Grok Build credential state,
catalog/Billing/quota observations, cache continuity, response ownership, and encrypted reasoning
replay to Provider-private state and synthetic tests.

`CR-P6-03-013` is explicitly limited: it permits the local P6-04 through P6-08 continuation but
does not rewrite direct-provider evidence. The closed T18 direct request remains a safe,
`unattributed` 4xx; T19 remains unsent. The independent grok2api proxy reference remains only a
separate route-availability fact, not a direct-Build acceptance claim.

## G6 conditions and evidence

| G6 condition | Evidence | Result |
|---|---|---|
| Two Credentials stay isolated through refresh/runtime state and quota observations | P6-02 CAS/singleflight coverage and P6-04's exact-Credential catalog/quota test keep records separate; stale observations cannot overwrite newer ones. | PASS |
| Cache Identity/Affinity is stable and never crosses Client Key scope | P6-05 uses versioned HMAC-SHA256 identities bound to tenant secret, Client Key, model, and cache key; the request builder rejects raw cache keys and rebinding requires a durable atomic break record. | PASS |
| Response continuation never silently changes account | P6-06 keys ownership by Client Key and downstream response, persists the exact Credential plus upstream response ID, and rejects either Credential or upstream-ID conflict. | PASS |
| Old request state cannot overwrite a newer credential or permanently poison it after transient/egress failure | P6-02 revision/CAS and P6-07's bounded 401/403/429/quota/408/5xx matrix retain the scope of evidence: plain 403 is egress-scoped, while 408/5xx cool the Provider rather than disabling a Credential. | PASS |

## Verification and independent review

The final local `CHECK_REPORT_PATH=tmp/p6-full-check.md ./scripts/check.sh full` completed in 77
seconds. It passed shell, workflow/classifier and plan guards, format, workspace Clippy/tests,
source policy, Secret scanner, crate boundaries, document links, whitespace, pinned dependency
policy, and RustSec audit. Focused P6 package tests and Clippy also passed before the full gate.

Independent review re-checked the version-7 migration's ordered create/drop sequence and the
existing all-schema rollback test; the raw cache key's exclusion from outbound requests; affinity
transactionality; exact ownership idempotency/conflicts; replay AEAD associated-data binding; and
the failure classifier's non-permanent 403/5xx paths. No release-blocking finding remained.

## Delivery boundary

The branch was pushed for reachability, then the annotated `phase-p6-complete` tag triggered the
only formal P6 delivery run. GitHub accepted it: Fast passed in 2m50s, Full supply-chain passed in
42s, and Required passed in 3s. The Full gate restored the default-ref fixed quality-tool cache,
then still version-checked and ran `cargo-deny` and `cargo-audit`. No pull request was opened,
because it would create an extra CI event contrary to the Phase-level delivery contract. P7 remains
out of scope until the user explicitly starts it; this closeout does not authorize a deployment,
server change, or any new direct Provider tuple.
