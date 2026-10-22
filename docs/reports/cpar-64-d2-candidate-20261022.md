# #64 — actual Claude Code isolated D2 candidate

Date: 2026-10-22. Scope: the user's independent D2 evaluation, with synthetic credentials and
an independent controlled loopback upstream. **D2 is PASS_LOCAL_CONTROLLED. Original Claude
remains BLOCKED_BEFORE_FIRST_RESPONSE. Neither result admits D2 to the first release or closes
#58/#64.**

Decision: [ADR-0122](../adr/ADR-0122-isolated-claude-restricted-candidate.md).
Contract: [BC-PROTOCOL-016](../contracts/BC-PROTOCOL-016-claude-restricted-candidate.md).
The earlier [local-controls report](cpar-64-local-controls-20261022.md), ADR-0121 and M1–M5
remain applicable. No historical Accepted ADR was rewritten.

## Frozen baseline and evidence binding

- Working branch: `feature/62-client-metadata` in
  `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`.
- Actual starting HEAD: `eeada0c403d21cb4b753c2a1c466f9da254a19aa`. The supplied historical
  candidate `9035551dcef79e130e17f7591fa3ca8a0ff3a265` locates prior evidence; no rollback.
- Implementation commits: `7c246c58bd2f0f399be65a4fd754d271b3a2087a`, followed by
  `4015bd4542dbc52fde8984c14c01b34843ef3c59`. The latter is the **final code candidate**.
- Final validation worktree:
  `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar64-d2-full-20261022-4015bd45`.
  It contains the explicit inherited overlay; this is an overlay Full, not a bare-tree Full.
- Fresh target for this candidate:
  `/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-d2-20261022T072236Z/target-full-4015bd45`.
- Cargo.lock SHA256: `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858`.
- 7,307 source/overlay inputs, manifest SHA256:
  `7f68ea2fc32647a9fa31ae2f47678132c97c7070c4a83ac9ce4ee46aaa126c81`.
  The post-Full/post-client comparison preserved bytes, modes, uid/gid and symlink targets.
- Actual-client test executable SHA256:
  `23e48a351c673f53d2f7a39ea93ee17a2ae311cff4ad38212480c05115145e73`.
  D2, original Claude and strict Codex/Pi used this same executable. Full separately uses its
  documented all-features checks; the actual-client driver records its default-feature build.
- All 18 inherited files retain their original bytes and attributes. The shared ADR index and
  traceability file retain their exact inherited prefix plus this task's appended entries.
  Those two entries were committed from HEAD plus the owned suffix, without staging inherited
  edits. The initial index was empty. No reset, clean, stash or broad staging occurred.
- Remote readback: main `c6a981b0f9eda09b0a2e58df11d272adf14f9552`; existing #64 branch
  `370604297a956047fda4d02c78ca3b0f7fc8cbd5`; no remote `feature/62-client-metadata`.
  #58/#61/#63/#64 were OPEN in the read-only issue snapshot; #64 depends on #63, which depends
  on #61. No GitHub or remote write was performed.

Bindings and exact inventories are in
[candidate-final-binding.json](assets/cpar64-d2-candidate-20261022/candidate-final-binding.json),
[input manifest](assets/cpar64-d2-candidate-20261022/final-source-inputs-before.json.gz),
[post-run verification](assets/cpar64-d2-candidate-20261022/final-input-verification.json),
[baseline](assets/cpar64-d2-candidate-20261022/baseline.json), and
[preservation](assets/cpar64-d2-candidate-20261022/preservation.json).
The later evidence/report commit is not a new tested code candidate.

## A. Original Claude — BLOCKED_BEFORE_FIRST_RESPONSE

The unmodified installed client was frozen again for each final branch:

| Identity | Observed value |
| --- | --- |
| Version | Claude Code 2.1.302 |
| Entry | `/Users/huangrui/.local/bin/claude` |
| Symlink target / real binary | `/Users/huangrui/.local/share/claude/versions/2.1.302` |
| Binary SHA256 | `5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37` |
| Target and entry modes | `0755`, `0755` |
| Architecture | Mach-O arm64 |
| Before/after installed bytes, mode, link and frozen-copy check | PASS |

The original branch receives the same isolated HOME/config/workdir, synthetic key and
`grok-4.5` identity. It does not receive the D2 control variables. Two captured Messages
requests retain adaptive thinking, block cache controls and `output_config.effort=high`.
Both return HTTP400. Independent TCP receives=0, leases=0, Attempts=0, Usage=0, ledger=0,
summary=0. Valid M4 is read back as Request-only encrypted raw-v2 evidence on both Requests.

The existing rejection observation test exits 0 because its rejection assertions pass.
The **strict outer driver exits 1**, `client_success=false`, compatibility=BLOCKED. This is
neither native success nor an actual tool/summary/cancel qualification. No HEAD or count_tokens
preflight, title request, upstream receive or live cancellation transition was observed.
See [original observations](assets/cpar64-d2-candidate-20261022/original-observations.json) and
[strict driver result](assets/cpar64-d2-candidate-20261022/original-run.json).

## B. D2 request shape and client-owned effort recovery — PASS

The complete evaluated D2 configuration is:

```sh
DISABLE_PROMPT_CACHING=1
CLAUDE_CODE_DISABLE_THINKING=1
CLAUDE_CODE_MAX_RETRIES=0
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
```

The first two switches remove every captured `cache_control`, the `thinking` parameter and,
in this version, `context_management`. They initially leave `output_config.effort=high`.
This is observed omission; it does not establish that the model stops reasoning. Actual D2
does not exercise M3's identity operation because it sends no context_management field.
The existing M3 independent contract tests were regressed instead.

The current official [environment documentation](https://code.claude.com/docs/en/env-vars)
documents the controls, ordinary retry limit and separate non-streaming fallback switch.
The [gateway protocol documentation](https://code.claude.com/docs/en/llm-gateway-protocol#automatic-retry-and-error-forwarding)
documents effort recovery from HTTP400 containing the field name and rejection phrase.
Capability declarations for cloud-provider modes have no effect in Base URL mode. The
[model documentation](https://code.claude.com/docs/en/model-config) defines auto as a default,
not proof of field omission. Rolling documentation does not prove a minimum supported version
or the installed client's internal matcher. This run proves this version's actual behavior.

The gateway now emits this fixed Messages invalid_request_error only after the full nonempty
matching candidate set establishes that the remaining conversion rejection is effort:

```text
output_config.effort: Extra inputs are not permitted by the current gateway conversion; no upstream request was sent
```

The error names CPAR's conversion as the rejecting party. It contains no supplied value,
model, URL, user_id or tool content. The initial Request is rejected before lease/send and
retains only its legitimate M4 evidence. The client then sends a **new authenticated HTTP
Request**, with output_config absent. Each later request in that process also omits it.
No proxy field deletion, gateway replay, patched client, cloud-provider switch, capability
declaration, effort=auto or Claude model alias was used.

| Captured branch | thinking / cache controls | effort | Public result |
| --- | --- | --- | --- |
| Original RED and final original | Present | high | 400, 400 |
| Two-switch RED, before diagnostic fix | Absent | high | Generic 400; no recovery |
| Final D2, first Request of each process | Absent | high | Specific local 400, no Attempt |
| Final D2, client-owned new Requests | Absent | Absent | Actual leased execution |

[Request shapes](assets/cpar64-d2-candidate-20261022/request-shapes.json) contain recursive
presence checks, root keys, raw request hashes and separate response statuses. Full original
requests and responses remain in the private archives, without publication of metadata values.

The runtime classification preserves independent M1/M2/M5 failures, unknown root/content/tool
fields, output-limit/history/capability failures, authentication, missing adapter evidence,
continuation and already-sent errors. Empty or model-mismatched sets and any potentially
compatible candidate do not establish this effort-specific claim. All matching candidates,
including those without active bindings, are considered. Mixed rejection reasons remain generic.

## C. Actual tools, summary, metadata, failure and cancellation — PASS_LOCAL_CONTROLLED

All scenarios pass through the public listener and authentication, production compiler/factory,
the real leased Grok Build adapter and an independent loopback receiver. Only the external
service is replaced with a fixed controlled peer. Model/account/channel joins are verified
per actual Request/Attempt against `grok-4.5` / `grok.build`; the receiver records the exact
body and independent receive ordinal. Background and preflight counts are zero in these D2 runs.

| Scenario | Public Requests | Negotiation rejects | Actual Attempt / receive | Usage events / ledger | Durable executed outcome | Actual CLI |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| Two tools and follow-up | 6 | 1 | 5 / 5 | 5 / 5 | Succeeded x5 | exit 0; three success, is_error=false results |
| Permanent execution failure | 2 | 1 | 1 / 1 | 1 / 1 | Failed | exit 1; is_error=true, terminal_reason=api_error |
| Client-alive cancellation | 2 | 1 | 1 / 1 | 0 / 0 | Cancelled | exit 1; error_during_execution / aborted_streaming |

Each negotiation Request has zero Attempt/Usage/ledger and no receive. It is never joined to
the subsequent execution as if it had sent. A Usage event count of zero means absent evidence,
not a token value of zero. All scenarios finish with lease=0.

### Real tool execution and continuation

The peer asks the actual Claude to invoke `Read` with the exact seed.txt path. Claude reads
`41\n` and submits its own tool_result. A later task asks for `Bash`, which really runs:

```sh
IFS= read -r n < seed.txt; printf '%s\n' "$((n + 1))" > result.txt; cat result.txt
```

The actual native Bash result has stdout `42`, empty stderr, interrupted=false, and the file is
`42\n`. Two different real tool-use/result IDs, names, arguments, ordering and associations are
checked. The harness only submits user turns and an interruption control; it never supplies
tool results. The five executed model requests cover Read, its result/answer, Bash, its
result/answer and the follow-up answer. Every native final result is simultaneously success
and is_error=false. Every round independently reconstructs and compares the full ordered
incoming history with the receiver's projected history, including both real tool results.

### Private summary and metadata

Each successful Attempt carries two complete ordered Grok reasoning-summary items. All ten
items across the five successful Attempts are encrypted and read back exactly through #63's
authorized production reader. No summary, Messages thinking or signature is synthesized into
the public response or continuation history. No terminal or tool completion is invented.

All ten D2 Requests, including the three effort rejections, retain M4's exact raw-v2 metadata
bytes. The authorized reader verifies owner, Request, source and actual Attempt association
against the retained database; negotiation evidence remains Request-only. The finished writer
is snapshotted with SQLite VACUUM INTO before the fixture drops. Three coherent encrypted
database copies are retained privately with mode0600. Both summary and metadata readback are
checked from those copies. Production schema38 and persistence formats are unchanged, so the
completed #65 recovery matrix was not rerun or claimed as new recovery evidence.

### Permanent failure and bounded sending

The independent peer emits a real partial stream followed by response.failed. The client
surfaces an error without tool completion or an extra model send. The already confirmed
measured input=20/output=7 remains attached to the failed Attempt and its single ledger row.
Unknown cache/reasoning/cost fields remain unknown. In this client, the failed native result's
subtype is still success while is_error=true, so subtype alone is explicitly insufficient.

Two real earlier failures explain the final configuration: with default policy the client
performed additional sends and the failure branch hit its deadline; MAX_RETRIES=0 alone still
allowed one non-streaming fallback. Their original logs, requests and responses are retained.
The fourth variable disables that separate fallback, with the same accurate rejection wording.
No timeout or assertion was relaxed, and the peer/gateway never retried a modified request.

### Native interruption with the same PID alive

The run observes a real public text delta, then lease=1. It sends the installed client's stdin
control_request with subtype=interrupt, following the official
[SDK control mechanism](https://raw.githubusercontent.com/anthropics/claude-agent-sdk-python/main/src/claude_agent_sdk/_internal/query.py).
The actual process acknowledges it. PID `67747` stays alive while the harness independently
waits for peer close=1, lease=0 and public persisted RequestFinished::Cancelled. Only after all
three conditions are durable is the cleanup marker published; the driver then confirms the
same PID is still alive and permits stdin/process cleanup.

Recorded order: interrupt sent at `1792658280120` ms; durable release/Cancelled observed at
`1792658280131` ms; same PID alive at `1792658280134` ms. This is a local observation, not a
latency SLA. No Usage arrived before cancellation, so Usage and cost remain UNKNOWN and no
ledger row is manufactured. Killing the process or independently closing the peer was not used
as the acceptance event.

See [D2 strict result](assets/cpar64-d2-candidate-20261022/d2-run.json),
[standard joins](assets/cpar64-d2-candidate-20261022/d2-standard-observations.json),
[failure joins](assets/cpar64-d2-candidate-20261022/d2-failure-observations.json),
[cancel joins](assets/cpar64-d2-candidate-20261022/d2-cancel-observations.json), and
[cancel diagnostic](assets/cpar64-d2-candidate-20261022/d2-cancel-diagnostic.json).

## D. RED, regression, Full and independent review

The original-client RED, two-switch shape RED and the public diagnostic RED precede the fix.
The public test first received the old generic400 instead of the expected accurate diagnostic.
The minimal implementation adds a closed value-free rejection reason, scans the complete
projection/candidate boundary, and renders the fixed existing Messages error envelope. The
acceptance work extends the existing Claude driver, tool-round renderer and lifecycle helpers;
it introduces no general test framework or dependency.

The [development history](assets/cpar64-d2-candidate-20261022/development-history.json) distinguishes
expected REDs, build failures, the intermediate one-response harness accounting mistake,
default retry/fallback failures, directed passes and development-only passes. All originals
are retained. A first launcher abort produced no client artifacts and is separately recorded;
it is not used as the original-client RED.

| Verification | Result and scope |
| --- | --- |
| Gateway core + router directed | 49 + 190 passed; zero failures |
| Messages/control/summary directed | 31 passed, 0 failed, 2 ignored; M1/M2/M5 independent and combined, M3/M4, JSON/SSE and summary/lifecycle |
| Directed Clippy after fixture borrow correction | PASS |
| Final overlay Full at 4015bd45 | 45/45 PASS, exit0; 1,659 Rust tests passed, 0 failed, 18 ignored |
| Final D2 actual-client ignored test, explicitly invoked | PASS; strict outer exit0, client_success=true |
| Final original Claude ignored observation, explicitly invoked | Observation PASS; strict outer exit1, compatibility BLOCKED |
| Final Codex/Pi actual-client regression, explicitly invoked | PASS with --require-prompt-release and --require-codex-success |
| Final input/lock/overlay verification | PASS, 7,307 inputs unchanged |
| Inherited file/attribute protection | PASS, 18 files preserved |
| Evidence/report addition | Document links PASS (1,285 Markdown files); 335 contract test references resolved; staged secret scan and whitespace PASS |

Full includes shell/CI policy fixtures, management SPA build, formatting, all-target/all-feature
Clippy, workspace tests, serving envelope, agent streaming regressions, source/boundary/secret
checks, doc/contract checks, dependency policy and RustSec. The advisory database query succeeded
on this run. Its policy/workflow harnesses are local checks, not remote CI, deployment or live
Provider qualification. Full's 18 ignored tests are not counted as executed; the actual-client
tests above were invoked independently.

Final Codex CLI is 0.161.0, Pi is 0.84.3 and its installed Node is 26.8.2; identities and package
bytes were checked unchanged. This CPA regression does not change only-my-pi's separate Node
policy or execute its private runtime. Standard branches each perform two real tools and
follow-up with five joined successful executions; failure branches preserve known Usage;
cancellation branches keep the client alive while the independent peer closes and the lease
releases. Their persisted outcomes are Cancelled. The unqualified Codex negative still rejects
before send. See [strict regression receipt](assets/cpar64-d2-candidate-20261022/codex-pi-run.json)
and [counts](assets/cpar64-d2-candidate-20261022/evidence-summary.json).

Independent Standards and Spec reviews of eeada0c4..7c246c58 found no hard/spec defect. Standards
identified duplicated test assertions, which were removed while retaining the independent
outer assertions. A fresh one-shot review of the three-file follow-up through 4015bd45 found
zero Standards or Spec findings. Reviewers did not claim dynamic acceptance; the main agent
ran the evidence above. The earlier Full on 7c246c58 also passed but is not substituted for
the final candidate's Full. See [initial review](assets/cpar64-d2-candidate-20261022/independent-reviews.json)
and [follow-up review](assets/cpar64-d2-candidate-20261022/independent-fix-review.json).

Final [Full report](assets/cpar64-d2-candidate-20261022/full-check.md),
[machine summary](assets/cpar64-d2-candidate-20261022/final-full-summary.json), and
[complete compressed log](assets/cpar64-d2-candidate-20261022/final-full.log.gz) bind the code
candidate above. No report commit is represented as a new runtime rerun.

## E. Original #64 and first-release conditions remain open

| Boundary | Current status |
| --- | --- |
| Original unmodified Claude configuration | BLOCKED_BEFORE_FIRST_RESPONSE; M1/M2/M5 independent unsupported controls remain |
| D2 as an isolated controlled candidate | PASS_LOCAL_CONTROLLED under the exact four-variable configuration and frozen client |
| Adopt D2 for first release | NOT_DECIDED; separate user/product decision |
| Adaptive/display, overall effort and block prompt-cache promises | Not granted by D2; original #58/#64 promises remain outstanding |
| M3/M4 and #63 local contracts | Regressed; no semantic relaxation or new recovery claim |
| #61 cost_in_usd_ticks | NOT_ADMITTED; all cost claims remain UNKNOWN |
| Fixed Build service activation/qualification | BLOCKED; local peer success does not establish it |
| Actual external Provider behavior | NOT_RUN; synthetic loopback only |
| Remote CI | NOT_RUN |
| Release and deployment | NOT_RUN |
| Original #58/#61/#63/#64 closure | Not performed; existing dependency/acceptance conditions remain |

The installed executable was neither upgraded nor downgraded. No real credential file was
read, no paid Provider was called, and no global client configuration, GitHub state or remote
branch was changed. This task's commits contain only owned increments. The retained inherited
dirty/untracked work remains present for its owner.

## Private originals and repeat commands

Private durable root:
`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-d2-20261022T072236Z` (mode0700).
`raw-final/` retains final raw HTTP requests/responses, native stdout/control events, source
receipts and three coherent encrypted D2 databases. `raw-development/` retains the failed
and intermediate original evidence. Metadata values, synthetic credential values, full native
stdout and database bytes are deliberately kept in that private root rather than in Git.
The public [final manifest](assets/cpar64-d2-candidate-20261022/raw-final-manifest.json.gz) and
[development manifest](assets/cpar64-d2-candidate-20261022/development-run-manifest.json.gz)
locate exact files by SHA256. Frozen client executables remain at the original run paths and
are identified by their binary hashes; copies are not committed.

Executed from the final immutable validation worktree, with new isolated output directories:

```sh
CARGO_TARGET_DIR=<fresh-target> CHECK_REPORT_PATH=<full-report> ./scripts/check.sh full
python3 scripts/acceptance/cpar64-claude.py --d2 --output <new-private-tmp-dir> --target-dir <fresh-target>
python3 scripts/acceptance/cpar64-claude.py --output <different-private-tmp-dir> --target-dir <fresh-target>
CARGO_TARGET_DIR=<fresh-target> python3 scripts/acceptance/cpar62-cli.py --output <new-OS-temp-dir> --require-prompt-release --require-codex-success
```

The original Claude command's exit1 is the expected strict BLOCKED result. It must not be
hidden with a diagnostic-only exit0 or counted as successful Claude compatibility.
