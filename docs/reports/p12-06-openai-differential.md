# P12-06 OpenAI-compatible live differential

| Field | Value |
|---|---|
| Plan version | `v1.83` |
| Task | `P12-06` (OpenAI-compatible portion) |
| Status | `DEFERRED_EXTERNAL_CREDENTIAL` |
| Task Card | `M`: operational graph helper, bounded differential harness, live preflight, evidence and review |

## Outcome first

The new gateway's Krill-backed candidate arm passed a single loopback, non-streaming preflight: it
returned `2xx`, completed with `end_turn`, exposed a valid Canonical message projection and carried
Usage. The incumbent CPA arm did not provide a successful reference response. A corrected
Responses-shaped request reached its existing `/v1/responses` route but returned `5xx` with the
source-defined safe projection `server_error/internal_server_error`.

The paired stream, non-stream, Tool and performance corpus was therefore **not run**. Running ten
rounds against one functioning arm and one known-failing arm would measure availability failure,
not gateway compatibility. Under `CR-P12-06-010`, the Grok/xAI-backed reference line is deferred;
P12-08 remains pending and must not start without a separate gate change or a working non-Grok
reference arm.

## CR-P12-06-009 diagnosis and deferred closeout

Read-only review after the original receipt established a narrower root cause. The live incumbent is
`v7.2.101`; `v7.2.80` remains the plan's frozen behavior-reference version, not the currently running
binary. The enabled xAI account pool had expired access tokens and persisted unauthorized cooldown
state. A bounded refresh sample was rejected by the upstream token endpoint as `invalid_grant`, and
the official local CLI login did not persist a usable replacement credential. No incumbent config or
authentication record was written.

Before diagnosis, a root-owned, integrity-checked server preimage was created for the incumbent data,
unit and container identity. It remains as audit and rollback evidence. Since the attempted repair
never reached a write, no data rollback was necessary. There is no remaining login process, and no
additional account scan, refresh or live probe is authorized under this line.

The user explicitly chose to skip the Grok line for now. This records an external-credential deferral,
not a differential pass: the candidate preflight remains valid, while the paired stream,
non-streaming, Tool and performance corpus remains unexecuted.

## Fixed execution boundary

- Both arms are exact loopback HTTP endpoints. No public hostname, Caddy split or production traffic
  was used.
- The incumbent arm uses its existing client key and account pool without configuration mutation.
- The candidate arm uses the already-approved effective Krill Bearer through a one-candidate,
  one-attempt successor graph.
- Prompts are fixed synthetic inputs. Durable evidence contains no endpoint, credential, model,
  request body, response body, response ID or token value.
- Per `CR-P12-06-008`, Kiro stays `DEFERRED_EXTERNAL_CREDENTIAL`; no Kiro credential was read,
  refreshed or called.

The added harness interleaves the two arms and, only after both pass preflight, compares lifecycle
event ordering, terminal semantics, complete Tool arguments, Usage presence/non-negativity/
conservation and Canonical projections. It reports TTFB, FirstSemanticEvent, first text, total time,
inter-delta quantiles and post-first-text output-token rate separately per arm. Inputs must be
root-only files; reports are created once with mode `0600`; bodies and SSE frames are bounded.

## Incumbent diagnosis and correction

The first diagnostic treated the incumbent's ordinary `/v1/models` result as proof that an entry
was usable on `/v1/responses`. That inference was invalid. Source review of the exact incumbent tag
(`v7.2.80`, commit `09da52ad509e2c18e7b9540db3b98c2214c280aa`) shows that the unified models
route dispatches to the generic OpenAI models handler, whereas `/v1/responses` is handled by the
separate Responses handler. Both currently read the broad `openai` registry, so the listing is an
inventory view, not an endpoint-specific success probe.

Requests selected from that inventory returned `4xx` or `5xx` and are invalid diagnostics for the
required paired comparison. A naive serial inventory check was started, then stopped when it became
clear that it could spend one timeout per entry without producing endpoint-specific evidence. Its
remote processes were terminated cleanly. Because the wrapper buffered its output and did not
persist a receipt, an exact request count is not asserted.

The corrected request used the incumbent's built-in Responses/Codex catalog family and the typed
message input shape expected by the frozen behavior reference. The route exists (`OPTIONS` succeeds
and the request reaches the handler), but the result is still `5xx`. Safe log projection identifies
only `server_error/internal_server_error`; it does not attribute the failure to quota, account,
OAuth, model availability or transport. The incumbent has a non-empty OAuth inventory, so an empty
credential directory is also ruled out. No incumbent configuration was changed.

## Candidate and graph evidence

The candidate successor graph was entered, restarted, validated, published, restarted and explained.
Explain selected exactly one Candidate. A direct Krill models preflight returned `2xx`, and the
configured model was present. The subsequent request through the new gateway passed the safe
non-streaming structural classifier.

Current server review:

| Check | Result |
|---|---|
| New gateway service | `active`, `disabled-at-boot` |
| New gateway listeners | exactly two loopback listeners |
| Incumbent service | `active` |
| Control database | SQLite `quick_check=ok` |
| Preimage backup | root-owned mode `0600` |
| Graph ledger, both key files and both model selector files | root-owned mode `0600` |

The active successor graph remains isolated and does not receive production-hostname traffic. The
preimage database backup is retained for rollback.

## Review verdict and unblock condition

The graph and differential helpers are suitable for the intended bounded comparison after the
incumbent reference is healthy. No model, endpoint or secret value is present in their output. The
review tightened two initially weak predicates before acceptance: FirstSemanticEvent now begins at
the first output item/delta rather than `response.created`, and Usage must be non-negative and obey
`total = input + output` on both arms rather than merely having equal shapes. A final control-flow
review also replaced short-circuit Tool inspection with complete output traversal: wrong-mode,
duplicate or structurally incomplete Tool calls now fail closed in both JSON and SSE.

P12-06 is deferred. It can resume only after one of these is separately authorized and demonstrated
in a new Change Request:

1. provide a newly valid incumbent account pool and make the fixed non-streaming preflight
   structurally successful; or
2. nominate another already-working, immutable OpenAI Responses reference arm.

The current Grok/xAI repair authorization is closed rather than left active. Changing the incumbent
production configuration or weakening the P12-06-to-P12-08 dependency requires its own Change
Request. Until then, P12-08 remains `PENDING`.

## Local verification

- OpenAI graph admission tests: 3 passed.
- Differential classifier tests: 6 passed, including value omission, bounds, Usage conservation,
  wrong-mode Tool and duplicate Tool rejection.
- `./scripts/check.sh docs`: passed.
- `./scripts/check.sh fast`: passed after the final review fixes.
- Staged Secret scan and Git whitespace check: passed.
- Deferred closeout review: `./scripts/check.sh docs` passed with 115 Tasks and zero
  `IN_PROGRESS`; tracked Secret scan and Git whitespace check passed.
