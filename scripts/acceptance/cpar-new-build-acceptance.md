# CPAR new-build real-channel acceptance

This extends `cpar-batch-c-live.py`; the historical entry point and its
`LIVE_EXISTING_RUNTIME` smoke receipts retain their original meaning. Python 3.9+
and the standard library suffice. Run all commands from the repository root.

## Observe the actual runtime and environment

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build inspect \
  --host new-vps --service cpa-rust-gateway.service --sudo \
  --manifest /absolute/path/to/reviewed-artifact-manifest.json \
  --out /absolute/path/to/current-environment.json
```

`--sudo` uses existing noninteractive read permission. Omit `--manifest` for an
inventory-only observation. The remote code reads `/proc/<MainPID>/exe`, hashes
the running bytes, extracts the embedded release revision, and verifies the PID
and process start before and after inspection. A formal real run requires a
matching manifest from `scripts/build-release.sh`, actual embedded SHA equal to
the frozen source SHA, and an observation at most ten minutes old. Passing a
commit argument alone cannot attest a runtime. The manifest must use
`cpa-rust-gateway-artifact-manifest-v1`, `revision`, `target` and a matching
`files[].name = gateway-<target>` / `sha256` record.

Inventory uses GET only on the management loopback. Management authentication
stays on the host. The output projects allowlisted operational facts and opaque
account identities. It records resource ETags, completeness, active config
stability and observation time. This is a series of observations, not an atomic
credential/config snapshot or a Provider reachability test. A projection error
or unknown field remains visible even when the inspection command succeeds.
Configured capability overrides and an empty public capability object do not
prove the effective declaration; freeze the actual declaration before execution.

## Freeze one combination and run its basics

Copy [the plan template](fixtures/cpar-new-build-plan.json) into an operator-owned
private directory. Its null values intentionally prevent execution. Required
target fields are `channel`, `endpoint_id`, `route_id`, `route_candidate_id`,
`upstream_model`, `credential_evidence_id`, `credential_revision`,
`config_version_id`, `config_revision`, `egress_evidence_id`, `egress_revision`,
`public_model`, `response_model`, `protocol`, `mode`, `parameters`, `capabilities`
and `declaration_observed_at_utc`, plus `declaration_evidence`. Opaque identities use SHA-256 of
`cpar-evidence:` followed by the actual identity; never put account materials in
the plan. Each of `reasoning`, `parallel`, `stored`, `continuation`, `compact`,
`websocket` must be an observed boolean. Null means unknown and blocks the plan;
false is allowed only after reading the effective declaration. Keep exact
revision scope from the corresponding actual observation. Credential revision is the integer
revision of the actual lease; configuration revision is the integer loaded graph revision. An ETag
such as `rev-1` or a native account revision is not a lease revision. Direct egress uses
`{"source":"not_applicable"}`; version-owned egress uses
`{"source":"config_version","config_version_id":"...","revision":1}`; process proxy uses
`{"source":"transport_fingerprint","sha256":"..."}`. None of these invents a node revision.

An inspection now GETs `/admin/models/effective` for each visible Client Key and retains each
source's optional `capability_evidence`. The source must be `serving_snapshot`, from the loaded
configuration and the same route/candidate/endpoint/upstream model and Client Key scope. The
formal preflight requires this fresh projection and compares it with the frozen declaration.
Public capabilities, configured overrides and management inventory do not replace this proof.

Given an **existing authorized** protected Channel Pin receipt, freeze the observed combination
without sending inference:

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build freeze \
  --plan /absolute/path/to/private/draft-plan.json \
  --runtime /absolute/path/to/current-environment.json \
  --pin /absolute/path/to/private/channel-pin.json \
  --out /absolute/path/to/private/frozen-plan.json
```

The draft retains protocol/mode/public and response models, parameters and source. Unknown identity
fields and capability booleans can be filled only from this actual receipt plus its matching
current declaration; conflicting previously frozen facts fail. The receipt must be successful,
single-attempt, fresh, Client Key scoped and from the same running build. Obtaining a new Channel
Pin is itself one inference request and needs the appropriate target/budget authorization.

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build run \
  --plan /absolute/path/to/private/frozen-plan.json \
  --runtime /absolute/path/to/current-environment.json \
  --base-url https://cpar.142857142.xyz/v1 \
  --out /absolute/path/to/preflight.json
```

Without `--execute`, this performs preflight without inference. After specific
inference/environment authorization and successful preflight, use the same
command with `--execute`. It reads the already provisioned client secret only
from `CPAR_ACCEPTANCE_CLIENT_KEY` (or `--client-key-env NAME`). Do not put secrets
in command arguments, reports or shell history. No authentication files are
opened by the local runner. Redirects and implicit retries are disabled; plain
HTTP is limited to loopback. Response size is bounded and body reads enforce a
deadline, including a peer that keeps sending small fragments.

One combination sends seven basic requests: two independent text turns, one
tool call plus result/final answer, two consecutive tool cycles plus final
answer. Chat, Messages and Responses each support JSON and SSE. Actual IDs,
names, complete arguments, full ordered history and fresh second-cycle IDs are
checked. Final text must reproduce the remembered/tool-result token. Native
Usage is retained; the gateway's eight-field `cpar_usage`, durable Usage and
ledger must agree without inventing zeroes. Frames, item/block lifecycle,
matching delta/done snapshots, exact success terminal and no output after it are
validated. Explicit parameters are preserved; Messages requires `max_tokens`.

After **each** response, `collector` performs read-only process and exact
response-ID Usage/Attempt/ledger collection via SSH. The runtime must remain the
same process/artifact. The Usage request-ID must be unique; a ledger's source
event must match exactly. Every attempt must match all frozen target fields;
another account, endpoint, model or candidate cannot substitute. Missing facts
block; contradictory facts fail. The runner stops at the first error or missing
attribution and sends no later turn. It does not refresh accounts, publish
configuration, restart services or deploy an artifact.

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build collect \
  --host new-vps --sudo --response-id <existing-response-id> \
  --out /absolute/path/to/attribution.json
```

Schema 34 adds a bounded, append-only execution sidecar bound to the exact original Attempt
payload by Request ID, Attempt ID and SHA256. Original strict event bytes remain unchanged. The
collector reads actual channel, material lease revision, loaded configuration and selected egress
and declarations from that record. It also requires the Request's actual Client Key/protocol/mode
and known terminal outcome, then exact Usage/ledger sources. Missing legacy sidecars remain null;
malformed or mismatched bindings fail. A time-window join or current inventory is insufficient.
For an existing protected collector, `observation_dir` can replace `collector`:
each file is `<sha256(cpar-evidence:<response-id>)>.json` containing `correlation`,
`request_id`, exact `attempts`, `usages`, `ledger`, and the per-response observed
`runtime`. A wrapper with `runtime` and `evidence` is also accepted. Both paths
verify the same process, artifact, source and observation freshness after every
turn; missing runtime blocks and contradictory runtime fails. Neither input supports a
manual success override.

## Audit controlled boundaries and declared extensions

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build audit \
  --bundle /absolute/path/to/private/observations.json \
  --out /absolute/path/to/public-receipt.json
```

Private bundles use `schema_version: 2`, `layer`, `source_sha`, `runtime`, `target`,
`scenarios`, `boundaries`, `extensions`, and optional `evidence_paths`. Scenario
turns contain the actual `request`, `wire`, `http_status`, lineage `evidence`,
`next_token` or final `expected_text`. Wire bodies remain private. The automatic
runner drives the seven basics; it imports separately captured controlled
boundary/extension observations from the plan. It does not automatically inject
upstream faults, restart a service or perform cross-owner/delete operations.
Those observations must come from the existing controlled facilities or an
authorized operator and retain actual target and event/send/resource evidence.
Absent observations stay `NOT_RUN`; they never become passes.

Required boundary cases are `error`, `truncation`, `cancel_before_send`,
`cancel_bootstrap`, `cancel_after_output`, `timeout_bootstrap`,
`timeout_after_output`, `retry_before_output`, `no_retry_after_output`. Include
the target, ordered event observations and exact attempts, actual upstream send
count, closed observation window, resource release, and zero post-terminal
sends. After-output cases require observed semantic output; cancellation and
timeout require their respective observations. A retry must start before the
first semantic output and obey the frozen attempt budget.

Declared extensions require all checks below, not one representative sample:

| Declaration | Checks |
| --- | --- |
| reasoning | visible reasoning, reasoning preserved in full history |
| parallel | two distinct calls, interleaving for SSE, paired results, serial control |
| stored | exact read, foreign owner, deleted, expired, actual process restart, persistence failure |
| continuation | same-target exact continuation, lineage rejection |
| compact | CPAR compaction output consumed, foreign owner, lineage rejection |
| websocket | validated upgrade and multiple turns, handshake rejection, cancel, failed root |

`EXTENSION_CHECKS` and `extension_check` in
[the implementation](cpar_new_build_acceptance.py) define the exact private
observation fields. Rejection samples require authenticated operation facts and
no upstream sends; foreign-owner samples require distinct client identities.
Successful samples require actual wire and Usage/Attempt/ledger attribution.
An unknown declaration blocks the whole plan. A known true declaration with
missing checks yields `NOT_RUN`. Explicitly false checks are shown as
inapplicable `NOT_RUN`; this does not redefine the specification.
The six compiled declarations are kept intact. Stored/continuation/compact/WebSocket checks are
separately inapplicable to Chat/Messages; a true declaration is not rewritten to false. The existing
Grok Build owned-reasoning continuation requires Reasoning and remains a distinct kind.

Public receipts allowlist operational observations, parameters, Usage, checks,
opaque identities, time and evidence paths. They contain no secret, raw request,
response body or account display identity. Keep raw bundles outside tracked
reports. Exit 0 means `PASS`, 2 means `BLOCKED`/`NOT_RUN`, 1 means `FAIL`.
`basic_status: PASS` with overall `NOT_RUN` means only basics passed; it cannot
close C or #57. Layers are `LOCAL_SIMULATED`, `LIVE_NEW_BUILD`, `PRODUCTION`;
read-only inventory and historical collection are separate observation layers.

## Isolated regression

```sh
python3 scripts/test-cpar-new-build-acceptance.py
CARGO_NET_OFFLINE=true bash scripts/check.sh full
```

The regression starts only ephemeral loopback HTTP servers with synthetic
accounts and token data. It checks all six combinations, semantic negatives,
wrong/missing target and runtime evidence, exact ledger source, slow-body
deadline and no redirect/retry. `CPAR_TEST_RECEIPT_DIR=/absolute/path` optionally
exports value-free synthetic basic receipts. The suite is included in Fast/Full.
No real Provider request is made by this suite.

The extended real-gateway regression additionally runs against an isolated SQLite database, local
TLS upstream and two actual loopback SOCKS5 relays. It verifies all six ordinary execution paths,
error/cancel/timeout identities, changed retry candidates, in-flight publication and material
rotation, protected pin redaction, and formal collection through actual Usage/ledger sources:

```sh
cargo build --locked -p gateway --bin gateway
python3 scripts/acceptance/cpar-batch-b-http.py /absolute/path/to/owned-output
```

`CPAR_GATEWAY_BINARY=/absolute/path/to/reviewed/gateway` selects an immutable release candidate
for the same harness. Receipts bind the actual binary SHA256 and checkout SHA. Raw synthetic keys
and wire captures stay in its private temporary directory. Schema 34 compatibility/rollback and
atomic tamper regressions live in `gateway-store::execution_identity_tests`; old binaries reject
newer schemas, so a binary-only deployment rollback is not supported.
