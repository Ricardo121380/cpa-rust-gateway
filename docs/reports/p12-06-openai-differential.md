# P12-06 OpenAI-compatible live differential

| Field | Value |
|---|---|
| Plan version | `v1.88` |
| Task | `P12-06` (OpenAI-compatible portion) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Task Card | `M`: operational graph helper, bounded differential harness, live preflight, evidence and review |

## Outcome first

P12-06 passed its approved Krill-backed differential boundary. The incumbent CPA used a temporary,
uniquely prefixed native `codex-api-key` reference arm; the new gateway used its already accepted
Krill candidate graph. Both arms passed the non-streaming, ordinary SSE and Tool SSE preflights.
The paired corpus then produced 10/10 successful SSE samples and zero failures on each arm, with
equal Canonical lifecycle projections and terminal semantics, complete Tool arguments, and valid
Usage presence/non-negativity/conservation in every mode.

The first OpenAI-compatibility attempt remains useful negative evidence: its generic Chat-to-
Responses translator emitted only the initial lifecycle and text-delta events, then ended without
`output_item.done`, `response.completed` or `[DONE]`. It was rolled back and replaced by the native
Responses executor rather than weakening the lifecycle requirement.

The original schema-1 receipt returned a false negative only because it compared optional Usage
detail presence across arms. The incumbent retained a legal cache-read detail while the candidate
omitted it; both arms still satisfied all approved core Usage invariants. `CR-P12-06-014` corrected
that over-strict comparison, added positive and negative no-network regressions, and reevaluated the
unchanged value-free receipt offline. All nine differential predicates passed with zero new network
requests.

All temporary incumbent Provider configuration and the temporary model selector were restored from
the root-only preimage. The incumbent and candidate services are active with exactly their original
loopback listener counts; Caddy, DNS, Client Keys, the candidate graph and production traffic were
not changed. P12-08 remains pending and was not started.

## Performance baseline

The ten samples were interleaved against the same Krill channel. Values are descriptive baselines,
not a statistically powered superiority claim.

| Metric | Incumbent mean / p50 / p95 | Candidate mean / p50 / p95 |
|---|---:|---:|
| TTFB (ms) | 1140.834 / 811.669 / 2915.372 | 673.545 / 583.286 / 1140.074 |
| First semantic event (ms) | 2294.622 / 2068.901 / 4080.010 | 2233.129 / 2074.666 / 3483.512 |
| First text (ms) | 2377.290 / 2163.585 / 4113.608 | 2233.180 / 2074.712 / 3483.559 |
| Total (ms) | 2568.159 / 2309.469 / 4645.361 | 2386.641 / 2261.084 / 3622.448 |
| Inter-delta gap (ms) | 12.215 / 2.030 / 41.899 | 9.814 / 0.016 / 70.538 |
| Output tokens/s | 51.809 / 44.617 / 156.729 | 53.244 / 49.762 / 69.387 |

The candidate's sample mean was about 41% lower for TTFB, 3% lower for first semantic event and 7%
lower for total time; output rate was similar. Tail values have only ten samples and should not be
treated as stable production percentiles.

Server-only, root-owned evidence:

- `p12-06-openai-krill-native-differential-20260802T035813Z.json` — original immutable schema-1
  value-free paired receipt.
- `p12-06-cr014-offline-review-20260802T040416Z.json` — schema-2 evaluator result; 9/9 predicates
  passed, source receipt unchanged, zero network requests.
- `p12-06-cr012-sse-shape-20260802T035424Z.json` — value-free negative diagnosis for the generic
  Chat-to-Responses stream path.

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

P12-06 meets its approved OpenAI-compatible differential boundary through the isolated Krill native
Responses reference arm. Grok/xAI and Kiro remain deferred and disabled; this result does not count
as either Provider Gate. P12-08 remains `PENDING` until its own controlled Canary execution begins.

## Local verification

- OpenAI graph admission tests: 3 passed.
- Differential classifier tests: 6 passed, including value omission, bounds, Usage conservation,
  wrong-mode Tool and duplicate Tool rejection.
- `./scripts/check.sh docs`: passed.
- `./scripts/check.sh fast`: passed after the final review fixes.
- Staged Secret scan and Git whitespace check: passed.
- Deferred closeout review: `./scripts/check.sh docs` passed with 115 Tasks and zero
  `IN_PROGRESS`; tracked Secret scan and Git whitespace check passed.
- Krill native reference: incumbent and candidate preflights passed; both arms produced 10/10 SSE
  samples, zero failures, valid non-streaming/Tool projections and valid Usage invariants.
- Comparator correction: 7 no-network classifier tests passed; `./scripts/check.sh docs` and
  `./scripts/check.sh fast` passed; offline review accepted 9/9 predicates without a network send.
