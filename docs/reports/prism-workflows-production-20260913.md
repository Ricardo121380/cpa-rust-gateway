# Prism workflows production release — 2026-09-13

The user explicitly authorized deploying the completed batches before the remaining alignment work.
Production now serves `8578e5540d00f3bdecf5d96f6ae3a131fd033d8a`, schema27, on the existing
[Prism domain](https://cpar.142857142.xyz/admin-ui/#/unlock). This is a partial-plan release, not a claim
that the entire alignment plan is complete.

## Released behavior

- Durable request terminal observations, true request counts/trends/latency, snapshot-bound history,
  safe export and six-category usage/price evidence. Historical missing latency remains unknown.
- Unified ordinary/native account identity search over the complete bounded inventory, status/category
  filtering, sorting and paging; compact provider workspace and consistent mobile account/key/price rows.
- Explicit model discovery/opening separation, direct model permissions for client keys, price-difference
  confirmation and catalog import from the active configuration.
- Deployment preparation fixed Rust formatting/runtime lint and replaced the obsolete contract assertion
  that request history must not exist with a read-only/no-request-body contract check.

## Verified for this release

- Frontend: 290 tests passed. Type/build and134-operation four-file deterministic embed gate passed.
- Rust: locked workspace/all-features run,1245 passed,0 failed,9 ignored; all-target/all-feature Clippy passed.
- [Exact-revision delivery gate](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34763355806)
  passed Fast plus supply-chain checks. Both native architecture artifacts were built and signed in
  [release-artifact](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34763357469).
- Independently verified ARM64 artifact/SBOM/checksums and Sigstore identity locally. ARM64 synthetic
  administrator initialization/change/logout/restart and exact four embedded assets passed on Oracle.
- Network-isolated production-copy26→27 upgrade, candidate→compatible fallback→candidate passed.
  An injected synthetic terminal event was retained and checkpointed by fallback; original tables,
  administrator database and effective model permissions were preserved. No external provider reachable.
- Live stop-to-ready1022ms. Public HTTPS four assets match signed candidate, CSP prohibits inline/eval,
  unauthenticated management endpoints deny access, host-local authenticated new APIs return successfully.
  Existing loopback listeners remain unchanged. EgoLite16 verified the actual public login at390×844
  and was handed to the user for manual login; no claim of user-completed production login is made.

## Retained production data and scope

Active configuration stays `production-models-20260913`. Seven managed accounts (two configured plus
five native), original administrator store,2229 historical event rows and593 ledger rows are retained.
Existing access groups/keys/model grants are unchanged. Counts for versioned resource tables in the
machine receipt include archived configurations and are not active-account counts.

The six active legacy model aliases are still retained in this partial release. Production formal-name/
alias migration, complete real upstream directory acceptance, remaining first-authorization flows,
price-source synchronization and whole-workspace acceptance remain in
[the alignment progress record](prism-complete-alignment-progress.md). No DNS/Caddy/Autoreg edits,
production data cleanup or manual real-provider inference were performed.

## Recovery

Compatible standby: `915983b8e91f00fe3d205d070bb5249cad814c67`, branch
`codex/prism-schema27-rollback`. It is based on prior production57e32d1 with only schema27 event
compatibility and the explicit-model permission guard backported. It retains old UI/runtime behavior,
reads existing terminal records, and does not emit new terminal records itself. Requests made during
fallback therefore have unknown terminal/latency in later analytics.

Use the signed compatible standby with the latest state/credential/admin files. **Never swap directly
back to57e32d1, run migration27 down, or restore old token/admin databases.** Review any subsequent
user configuration changes before rollback. The live switch procedure has a same-schema fallback;
no fallback was needed in this deployment.

Private backups, verified candidates and scripts are at
`/var/backups/cpa-rust-gateway/prism-workflows-20260913` on `new-vps`. Local reviewable scripts and logs
are under `output/prism-workflows-release-20260913/`; no private credentials were committed.

Machine receipts and public screenshot: [deployment](evidence/prism-workflows-production-20260913/production-receipt.json),
[rehearsal](evidence/prism-workflows-production-20260913/rehearsal.json),
[auth](evidence/prism-workflows-production-20260913/isolated-auth.json),
[login screenshot](evidence/prism-workflows-production-20260913/public-login.png).
