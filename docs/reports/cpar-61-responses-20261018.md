# #61 standard Responses preparation and blocking evidence

Date: 2026-10-18 (Asia/Shanghai). Ticket outcome: **PARTIAL / BLOCKED; keep OPEN**.
This run implements independently justified rejection/Usage fixes and controlled regression
evidence. It does not implement an unqualified metadata success rule.

## Frozen baseline and scope

- Branch: `codex/prism-v4-delivery`.
- Starting HEAD: `f2453ff25d7b1c56d42dfb1d0cf26cba674d4db4`; the supplied reference was not a reset.
- Initial index empty; 9 tracked dirty files and 49,041 dirty/untracked Git entries were frozen
  before changes: 48,981 regular files by SHA256/mode, 10 symlink targets/modes, and 50 nested
  repository directory entries by size/mode only. The entry sizes total 21,787,017,265 bytes;
  this is not a recursive descendant-byte claim for those 50 repositories. Only this run's
  increments may be committed.
- Latest #58/#59/#60/#61 bodies, comments and paginated native dependencies were read. #59 closed
  at `2026-10-18T07:38:12Z`; #60 at `2026-10-18T10:08:12Z`. #61's native blockers reference those
  closed issues; its 14 acceptance criteria remain unchanged. Only #61 was claimed.
- No real Provider inference/account budget, code push, other-ticket mutation, deployment or
  publication is authorized or performed. Actual clients and remote CI are NOT_RUN.

## Field decision and implementation

The [field qualification record](cpar-61-field-qualification-20261018.md) covers root/item/part,
Usage and model identity. First-party Build source supplies concrete SSE cost semantics, improving
the previous evidence. It does not complete JSON/profile applicability; there is no newly admitted
extra field. Source/tool counts, the anonymous historical object and real model aliases retain
their separate blockers. A test-only allowlist is not installed.

Public production-factory RED tests establish five defects:

1. JSON/SSE discarded unknown response root fields and could succeed.
2. Added item status `42` was normalized away before SSE success.
3. Failed SSE terminal Usage was not recorded even when valid and already received.
4. A created/in-progress SSE snapshot could claim `completed` and still lead to success.
5. Fixed-candidate Spec review found that a failed frame and conflicting tail in one transport
   chunk could roll back verified Usage. The expanded public test reproduced zero Usage rows.

The Build decoder now uses a common consumed-field root schema, checks supplied status before
normalization and confirms SSE snapshot phases. Failed SSE parses Usage into the existing
Attempt-scoped observer before its safe failure. The [decision](../adr/ADR-0111-build-response-rejection-integrity.md)
and [contract](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md) describe the stricter
rejection scope. It can reject previously ignored native timestamps/metadata/control echoes.
No new schema, reader, keys, TTL, dependencies, frontend or native-v1 representation is added.
Failure now stops decoding at its validated record and finishes the source, preserving Usage
with separate or coalesced transport frames. Already received tail bytes are not additional
evidence. Success retains whole-chunk validation and strict terminal-conflict rejection.

## Acceptance criteria, in current body order

`PARTIAL` below never means the whole criterion passed. Scoped standard-path regression does
not prove behavior with admitted metadata.

| AC | Required result | Status and exact evidence scope |
| --- | --- | --- |
| 1 | Actual field source/path/shape/mode/semantics across root/item/part/Usage | PARTIAL: field review complete as an inventory; cost SSE evidence found, common JSON/SSE applicability unresolved |
| 2 | Public JSON → production Build → projection → exact encrypted retention/readback before success bytes | BLOCKED: no admitted metadata success path; #59 proves strict Usage **failure** retention only |
| 3 | Answers, tool identity/arguments/results, native summary, Usage, two linked loops and intermediate questions, model identity | PARTIAL: controlled standard JSON/SSE test covers five linked requests, exact native output, summary and model, two tools plus intermediate/final/follow-up answers; admitted-metadata variant unavailable |
| 4 | Known incompatible request controls rejected before send, independent count zero | Scoped PASS: JSON/SSE `background`, `conversation`, `stream_options` are rejected at the public boundary with independent TCP receive 0, Attempt 0 and Usage 0; no field is removed to simulate support |
| 5 | Unknown shapes/identity/order/terminal/precision/write failures cannot succeed or resend | PARTIAL: root/status/phase RED→GREEN and failed Usage exact readback; #59/#60 and strict decoder regressions required; admitted-metadata write failures unavailable |
| 6 | Ordinary metadata stays distinct from reasoning; no Messages omission/native-v1 | Scoped PASS: native summary survives standard response/history; no new envelope or summary policy |
| 7 | At least one actual approved field with applicable evidence and public positive/negative/readback | BLOCKED: no common JSON/SSE field qualified; synthetic controls do not satisfy this gate |
| 8 | Unexplained Usage and real aliases stay blocked, no real/client claim | Scoped PASS: #59/#60 public regressions pass; no new authority or real requests |
| 9 | Shared JSON/SSE reviewed policy, actual incremental path/retention, pre-send zero | BLOCKED for metadata: shared strict root decoder exists, but a reviewed success policy is absent |
| 10 | Real fragmentation/interleaving of text/summary/arguments, executable tools only after valid completion, JSON parity | PARTIAL: production Build transport receives 1/7/31-byte slices of TCP-sourced events; interleaved native summary/tool or summary/text and final output match JSON; malformed-tool regressions remain distinct |
| 11 | Retention before successful SSE terminal; unique legal late failure on write/timeout/unknown/conflict | BLOCKED for admitted success metadata; root/phase/failure-Usage tests prove unique strict failure; #59 private-recording failure regression is independent |
| 12 | Cancel/disconnect/timeout stop work/release resources, preserve verified Usage and incomplete state | PARTIAL: existing public generic socket/drop and storage-timeout regressions are applicable preparation; comprehensive production-Build metadata cancellation/timeout evidence is NOT_RUN |
| 13 | Independent receives plus Attempt/Usage/ledger show no added attempt/send/double counting | PARTIAL: controlled five-turn standard test has five receives/Attempts/Usage; native failed SSE has receive 1, exact owner readback and ledger 1 with UNKNOWN cost; admitted metadata success/cancellation matrix unavailable |
| 14 | Two public SSE tool cycles plus follow-up, rejected unapproved fields, JSON regression | PARTIAL: controlled standard five-turn JSON/SSE plus strict negative regressions; admitted-metadata variant unavailable |

## Validation and review

Directed regression is **73 passed, 0 failed, 1 ignored** across the public production Build
matrix (24), strict native parser suites (40), Usage storage/reader/CLI lifecycle (8), and public
socket-disconnect regression (1). The ignored private-capture test was not executed. All these
Rust tests ran under loopback-only Seatbelt; a separate TCP probe passed loopback and rejected
`192.0.2.1:443` with EPERM. This generic socket-disconnect proof is not relabelled as a complete
Build metadata cancellation matrix.

The initial targeted Clippy check found a needless value parameter in a new test helper; it was
changed to a reference and the same check passed. The first Full stopped in the standalone RSS
soak guard because the validation wrapper also sandboxed its `ps` process. An independent probe
reproduced `ps` exit 71. The wrapper was corrected to sandbox workspace Rust tests only; repository
scripts and assertions were not changed. Both targeted Clippy and first Full failures remain
in the evidence archive. The first preservation checker also tried opening nested repository directories as files; its harness error
and the corrected entry-type-aware check are retained separately.

A subsequent Full reached Git whitespace after all workspace tests passed, then rejected blank
EOF lines in two rendered specification snapshots. Only those rendered Markdown copies were
normalized; exact GitHub body snapshots remain unchanged. The next Full failed the existing
`gateway-upstream` slow-body total-timeout test with `EgressUnavailable`. The unchanged exact test
passed alone under the same loopback policy. The cause is not established by that pass. Its
failed Full remains a distinct epoch; no upstream assertion or production timeout was changed.

The first Standards review found an inherited tree ID in the final-source receipt. Its original
bytes are preserved as `full-source-final-reviewed-error.json`; the corrected receipt resolves
the actual source commit tree directly. The first Spec review found the coalesced-failure Usage
bug above plus the acknowledged field/persistence and cancellation qualification gaps. The new
public RED/GREEN and the final review remain separate from those original findings.

The immutable Full source identity, raw RED/GREEN/Full logs, separate Standards/Spec reviews
and preservation checks are recorded under
[`assets/cpar-61-20261018/`](assets/cpar-61-20261018/validation-summary.json).
Full uses an isolated immutable snapshot with the current pre-existing documentation/rule overlays.
Those user overlays are not included in this run's branch increment. Product hashes bind the
validation snapshot to the final candidate. Final Full **PASS** on source `8ea7d88086d219dbdb91348335bc4fd31fb56411`: 45 steps passed in 418.17 seconds, with Rust **1538 passed, 0 failed, 13 ignored**.
Strict Clippy, local serve/agent regressions, source/boundary/doc/secret/whitespace checks,
`cargo deny` and RustSec audit all passed. Workspace tests used four threads and loopback-only
Seatbelt; no test or production assertion was changed for the earlier timeout failure.
The expanded coalesced-failure public regression separately passed after its preserved RED.
Final [fixed-candidate reviews](assets/cpar-61-20261018/review-final.md) compare baseline
`f2453ff25d7b1c56d42dfb1d0cf26cba674d4db4` with candidate
`d4ac176b149bf427820235b4a69902cd9fa4d340`. Standards **PASS**: hard findings 0, one nonblocking
possible Duplicated Code judgement in test setup. Spec **PARTIAL/BLOCKED**: two completion-blocking
groups (actual qualification/success retention; complete Build metadata cancel/fault matrix),
new behavioral bugs 0. Both reviewers confirm the first-round behavior/receipt findings resolved.
Full and Standards do not satisfy the missing field qualification. Only review/closeout documents
and receipts change after this frozen product candidate.

## Completion boundary and minimum next input

The necessary next input is scoped first-party Build field evidence (or an expressly approved
channel contract) establishing an actual field's shape and JSON/SSE/profile applicability. Then
the matching success-path persistence/projection/fault/cancellation matrix must be implemented
and verified. Root/item/part retention cannot be inferred from #59. Native failed JSON evidence
and complete Build cancellation/timeout qualification are not delivered here.

Keep #61 OPEN. No #62/#63/client gate is advanced. Local controlled and passing Full evidence
remain separate from real Provider, actual Codex/Pi/Claude Code, remote CI, artifacts, deployment
and production. Historical FAIL, UNKNOWN, oracles, captures and stopped epochs remain intact.

The dependency readback comparator initially compared a plain API array with `--slurp` pages;
its DRIFT receipt is retained. After explicit page normalization, edge identities and states
match. Nested #61 assignment/update metadata changed only as expected from the authorized claim.
