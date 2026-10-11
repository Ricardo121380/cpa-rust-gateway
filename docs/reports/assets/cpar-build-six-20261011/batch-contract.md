# Successor Linux ARM64 candidate and six basic combinations

This is the user-authorized 2026-10-11 slice for #47, #57 and parent #9. The
review base is `5d7efdb0a108a2820939af9f39a568c58e611a3d`. Product source stays
`abd454313f6a3707d6af5a5d653b731e2e54b08b`; existing product Full and reviews are
reused only for unchanged source/file hashes. No product changes are in this slice.

## Candidate contract

Keep the historical builder, binary, manifest and all failed attempts. Prepare
minimal Ruby/system dependencies from the existing trusted cache or official
repositories in an isolated successor image only. Preserve the original builder
digest. Record package versions, origins and checksums. Do not change macOS Ruby,
use production for building, or add a remote builder.

The actual successful recipe is `candidate-build.sh`, executed in an owned
container from the recorded successor digest, with `--pull=never --network=none`.
The source is the clean frozen archive, verified before extraction. Run the full
existing `scripts/build-release.sh`, including Ruby normalization, before strip,
ELF/AArch64/dependency/help and embedded metadata checks. Extract only after the
whole container exits zero. Bind the two-payload candidate manifest to actual
source and binary; distinguish raw manifest SHA256 from canonical-object SHA256.
This candidate is not the signed full P12 release package.

`candidate.Dockerfile` and its failed metadata-resolution receipt are retained
as an unsuccessful first recipe. The failed bind-mount attempt is also retained.
Neither failed attempt supplies the candidate binary.

## Execution contract

Derive owned scripts from the archived remaining-five drivers; keep every old
epoch and assertion, updating only verified batch bindings or strengthening
checks. Use the current formal entry, collector and audit with exact file hashes.

Order is Chat JSON, Chat SSE, Messages JSON, Messages SSE, Responses JSON,
Responses SSE. Each permits one new Pin and nine basic requests (text 3, single
tool 2, consecutive two-tool cycles 4). Chat keeps `max_completion_tokens=4096`;
Chat SSE keeps `stream_options.include_usage=true`; all other controls are frozen.
Preserve actual tool result, call ID, native items, ordered history, Usage and
exact ledger linkage. A Pin does not replace public-interface evidence. Basic
PASS does not replace missing declared extensions or overall formal acceptance.

Historical budget intents are 33; historical gateway/Pin-confirmed sends are 32.
New intent ceilings are Pin 6, basics 54, total 60, cumulative 93. Reserve before
transport, count local HTTP rejections, latch stop before an operation, reject
replays and exhausted limits. Failure, unknown or contradiction stops the entire
batch; no retry, fallback, new sub-batch, supplemental Pin or post-fix real rerun.
Independent Provider receives and actual cost stay UNKNOWN without evidence.
Unknown send totals stay null; resource exit is a separate verdict.

Use existing new-vps, the same approved Grok Build account/source, grok-4.5,
temporary unit `cpar-build-acceptance-20261010.service`, loopback 19180/19181 and
the existing protected root. New private epoch is `abd45431-six-20261011`.
Initial read-only evidence finds material revision 136, expected lease 137,
continuous floor 77/current 137, same non-empty identity digest and only allowed
normal-refresh fields changed from the prior authorized revision 135. The actual
pool and each new Pin must independently prove lease 137. Do not infer lease
from a native revision or ETag. Never actively refresh, reauthorize or replace.
The durable Build same-authority range is governed by BC-RESP-003; no exception
is added here.

Before sending, freeze actual source/binary/manifest/process, config, endpoint,
route/candidate/model, Client Key, account/material/lease, direct egress, serving
declaration and expiry. Require one account, one enabled candidate, max_attempts
one, fresh observations, exclusive state access, guard/audit, disabled background
OAuth/refresh/catalog, and remote file bytes equal to the reviewed freeze.
The explicitly inherited unlimited monetary policy does not relax these limits.
No real extensions, upstream catalogue/quota queries, second Client Key, fault
injection or production changes are authorized.

## Exit and delivery contract

Stop only the owned temporary process; verify PID, both ports and five seconds
of unchanged post-stop audit. Compare production PID/start, service/binary/current,
active config and Caddy with the initial baseline. Preserve history and protect
private account/wire/session material with 0700/0600. Restore Colima to Stopped.
Only owned files are committed, with no push, release or production deployment.

Update the existing progress comments of #47/#57/#9, retaining their full old
content as a prefix. Independently reread the updates and verify issue body/AC,
state, metadata and paginated dependencies remain unchanged. All three remain
OPEN while declared extensions and wider delivery promises are incomplete.
Report build, both review axes, local checks, successor matrix, historical matrix,
counts, resource exit, remote CI, release, deployment and production separately.
