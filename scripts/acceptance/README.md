# Backend local workflow acceptance

Run from the repository root after `cargo build -p gateway --bin gateway`:

```sh
python3 scripts/acceptance/prism-local-gateway.py /absolute/path/to/receipt-directory
```

This starts an isolated real gateway and an owned loopback TLS mock. It creates a temporary
state directory, private synthetic credentials and a private CA used only by the gateway process.
It never changes system trust, production state or real providers. Keep the controller running.
The printed URL identifies the management API; no frontend page is served.

In another terminal:

```sh
python3 scripts/acceptance/prism-request-chain.py /absolute/path/to/receipt-directory
```

This verifies two-page catalog refresh without permission expansion, JSON/SSE success, HTTP failure,
truncated streaming, explicit TCP cancellation, durable terminal statistics and billing. It waits
for actual runtime recovery between intentional failures instead of disabling health protection.

The old Prism browser acceptance tools are retired with the frontend. The remaining Python
helpers exercise backend API/inference behavior; their historical filenames do not imply a UI.
`scripts/prism-v4-local-acceptance.py --configuration-diff` retains backend diff/cursor checks
and publication/rollback stale-confirmation rejection without a browser. Use `--binary <path>`
for a gateway built outside the default target directory. Older browser evidence can be
inspected at its original Git revision.

Terminate only the controller belonging to this receipt; cleanup stops its gateway and mock.
These checks do not prove real Provider authorization or model-directory completeness.

## Agent multi-turn regression

`python3 scripts/test-agent-roundtrip.py` launches and cleans up a fresh owned gateway/mock,
runs four three-turn flows (JSON/SSE × client-managed/stored history), with two tool-result
continuations each, then the
existing error/truncation/cancellation/ledger scenarios. It is part of `scripts/check.sh` after
the gateway build. The client replays the gateway's exact output items (including reasoning and
tool status) instead of manually deleting unsupported history. The mock verifies the next
request, and authenticated decode failures must be counted without attempts or billed usage.
Successful temporary fixtures are removed; failed runs retain their private controller log,
receipt and synthetic state for diagnosis. The fixture explicitly opts its generic Responses
candidate into stored-response support; production capabilities are unchanged.
There are zero external Provider calls. For an already running owned fixture, run
`python3 scripts/acceptance/prism-agent-roundtrip.py <receipt-directory>` once.

## Legacy alias retirement

`prism-retire-legacy-aliases.py` performs an explicit plan/apply migration using
existing management credentials. First use an isolated production copy. `--base`,
`--origin`, `--credential-dir` and an unused `--receipt` path are required. Optional
`--upstream-names` reads a reviewed JSON object mapping exact upstream IDs to formal
names; it changes only the name field. Never place credential material in that file.

The initial invocation writes an owner-only plan. Apply that same plan with
`--apply-plan <plan-file> --target <new-draft-id>` and the same name mapping. The script
requires all six approved legacy aliases, preserves custom aliases, brackets reads
with the active revision and compares models/routes/candidates/groups/grants/keys
before publication and after readback. It does not edit original model IDs, account
credentials, historical requests or ledger rows, and makes no Provider calls.
A changed plan or existing target stops execution; inspect an interrupted draft
instead of replaying it. A successful local synthetic run is not production-copy
or real-channel acceptance.


### Exact-target catalog acceptance without background renewal

`gateway catalog-check` runs the same assembled metadata discovery as `/admin/catalog/refresh`,
but never starts listeners, inference, periodic maintenance or startup credential renewal.
Run it only on a fresh operator-owned state copy: use SQLite backup for control/admin stores,
copy the existing required credential files with owner-only permissions, and create
`<copy-state>/catalog-check.marker` containing exactly `isolated-metadata-only\n`.
Missing/symlink/nonmatching markers are rejected before opening the database.

```
gateway catalog-check --state-dir <absolute-copy-state> --credential-dir <absolute-copy-credentials> --endpoint-id <exact-endpoint> --credential-id <exact-account>
```

It writes only directory observations in the isolated copy and emits counts/closed errors, never
credentials or inference output. A stale access token is reported, never silently renewed from a
copied rotating grant. Re-create the copy from the live service after separately authorized renewal
if needed. This command is for production-copy acceptance; it does not replace the production HTTP
refresh workflow or prove official interactive authorization completed.

For an explicitly authorized isolated serving instance, use
`gateway serve ... --background-provider-tasks disabled`. The default remains `enabled`.
Disabled mode skips startup credential renewal and the automatic refresh/Catalog worker
subscription, including workers supplied by later runtime publications. Event persistence,
billing and explicit management/data requests remain available. This flag does not authorize
any request, prevent an operator from explicitly requesting renewal, or extend credential expiry.

`python3 scripts/acceptance/cpar-background-provider-process.py <new-receipt-directory>`
compares the default with disabled mode using owned stock processes, synthetic device grants,
fixed-host TLS loopback and an OS outbound deny. It observes receiver counts across restart,
publication and a real 65-second worker interval, then checks an explicit public inference.
Use a binary built from the exact recorded source; it never uses real accounts.

`cpar-build-egress-guard.c` is an additional Linux acceptance-only `LD_PRELOAD` guard.
Compile it with `cc -fPIC -shared -O2 -Wall -Wextra -Werror ... -ldl -lpthread` and set
`CPAR_BUILD_EGRESS_AUDIT` to a new absolute owner-only audit file in the isolated instance.
It admits only `cli-chat-proxy.grok.com` DNS resolution and TCP/443 connections to those
resolved addresses, plus loopback/Unix sockets; `auth.x.ai` and other outbound targets are
denied. Audit records contain event classes only. Verify its actual loaded mapping and hash,
positive loopback and negative DNS/socket controls before using the candidate. Resolver/socket
events are not HTTP request counts: reconcile catalog, Pin and inference separately with their
receipts and the durable Attempt evidence. Do not add this guard to the production service.

Alias retirement also accepts `--catalog-paths <reviewed-json>` for exact endpoint IDs and model-list paths. These paths join the static graph fingerprint and expected readback; normal request timestamps do not invalidate the reviewed configuration plan. Infer no path from a provider name: verify the base path and its documented model-list endpoint first.

## M4 bounded Pi acceptance

`reliability-pi-live.mjs --check-client <installed-pi-responses-module>` performs an offline
503 wire check. `node --test scripts/acceptance/reliability-pi-live.test.mjs` proves invalid
preflights cannot invoke the SDK or reserve allowance.

Live mode requires explicit arguments:
`--execute <module> <0600-private-config> <persistent-allowance-ledger> <new-receipt> <preflight-json> <exact-release-sha>`.
The authenticated operator preflight supplies `candidate_revision`, `observed_at_ms` and
`routes[exact-model].max_attempts`, derived from the active published graph, not guessed names.
Each send requires a matching revision, observation under ten minutes old and max_attempts=1.
Freeze graph edits during this short acceptance window; the observation is not an atomic pin.
Four synthetic Pi turns are intended, with 512 output tokens, low reasoning, no SDK/route retry,
and a pre-send fsync allowance record. Stop at the first failure; never restart this script
as an automatic retry or reset the ledger. An HTTP200 stream failure is still failure.

## CPAR new-build channel acceptance (#57)

The existing `cpar-batch-c-live.py --new-build` entry point provides read-only
runtime inspection, exact Usage/Attempt/ledger collection, frozen-target basic
execution and controlled-evidence audit. See
[the complete commands and evidence contract](cpar-new-build-acceptance.md).
Unknown runtime, target revisions or declarations block execution. The isolated
regression is included in `scripts/check.sh fast|full`; its receipts are
`LOCAL_SIMULATED`. The historical C smoke path remains `LIVE_EXISTING_RUNTIME`.
