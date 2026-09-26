# Agent gate initialization boundary — 2026-09-26

The first formal delivery run for `86ea204` failed and was not waived:
[run 36224360962](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36224360962).
Its third nonstreaming stored-response round returned `503 CredentialUnavailable`.
The signing run for that candidate was cancelled; production was not switched.

The fixture reported ready immediately after configuration publication. Publication starts
the model catalog worker asynchronously; its initial pass acquires the same concurrency-one
credentials and publishes a new catalog snapshot. Both capacity contention and a changed
ingress-pinned snapshot legitimately reject a selection. The existing
`ingress_pinned_catalog_never_selects_from_a_later_publication` regression explicitly protects
that boundary. The CI failure did not retain enough instantaneous state to distinguish these
two mechanisms or another transient cause. The historical 503 is not retroactively attributed.

`scripts/test-agent-roundtrip.py` now waits, under its existing setup deadline, for the actual
worker completion receipt with all five synthetic credentials observed successfully. It fails
on incomplete discovery, worker failure, process exit or timeout. It does not delay or retry
an inference, increase concurrency, disable discovery, or relax snapshot/credential checks.
The following request-chain test still exercises a real paginated catalog refresh and confirms
it does not widen model/key permissions.

`scripts/acceptance/prism-agent-roundtrip.py` supplies the fixture's configuration context to
management diagnostics and retains the runtime pool snapshot alongside availability on failure.
The old diagnostic omitted that context and only returned `invalid_management_request`.
These diagnostics run solely against owned synthetic fixtures; no production secrets are read.

The local focused gate passed after this change: actual initial catalog receipt 5 attempted /
5 succeeded, four three-round Agent flows (12 successful requests/attempts/ledger entries),
three malformed ingress requests rejected without attempts, plus the separate success/failure/
truncation/cancellation/catalog-permission chain. No real Provider inference was performed.
Raw local evidence: `/private/tmp/cpar-stability-release-20260926/agent-ready-check.log`.

This fixes the gate's false readiness and missing diagnostics. It does not claim that all
production `CredentialUnavailable` results are defects or that already-authorized runtime
capacity and snapshot invalidation should be bypassed. Formal delivery, signing and isolated
production-copy rehearsal must pass on the replacement candidate before deployment.
