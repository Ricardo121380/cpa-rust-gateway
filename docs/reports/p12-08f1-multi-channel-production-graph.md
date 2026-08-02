# P12-08F1 multi-channel production graph and capability ledger

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-02

Plan: `v1.107`

## Result

P12 now compiles Endpoint capabilities from one immutable, code-reviewed adapter ledger instead of
assigning every stored Endpoint an empty set. The Route Compiler receives only capabilities proved
by the D/E local vertical slices. Unknown adapter labels fail closed, and one Endpoint identity
cannot silently acquire a different capability profile in another stored Config Version.

This slice did not publish a Config Version, import a credential, contact an upstream, change a
server, or switch traffic. It establishes the production-graph composition contract and a
value-free representative graph; F2 owns loopback protocol/channel execution, F3 owns client
migration rehearsal, and G1 owns controlled real-credential evidence.

## Adapter and capability ledger

`T` = Tools, `PT` = Parallel Tools, `R` = Reasoning/Thinking, `S` = Streaming. JSON Schema and
Vision remain absent from every production profile in this slice even where a wire format can
represent them: protocol syntax alone is not Provider evidence.

| Channel class | Adapter / wire format | Capability profile | Runtime composition |
|---|---|---|---|
| Codex / OpenAI-compatible Chat | `openai-compatible.chat-completions` / Chat | T, PT, S | eligible only with an active encrypted Credential and binding |
| Codex / OpenAI-compatible Responses | `openai-compatible.responses` / Responses | T, PT, R, S | eligible only with an active encrypted Credential and binding |
| Claude / Anthropic-compatible | `anthropic-compatible.messages` / Messages | T, PT, R, S | eligible only with an active encrypted Credential and binding |
| Grok Build | `grok.build.responses` / Responses | T, R, S | eligible only with a valid unexpired Build credential |
| Grok Official | `grok.official.responses` / Responses | T, PT, R, S | locally composable; no current external credential claim |
| Kiro | `kiro.messages` / Messages | T, R, S | locally composable; no current external credential claim |
| Grok Web | `grok.web.responses` / Responses | none | known identifier, no production runtime binding, never selectable |

The ledger is deliberately conservative. Candidate overrides may only narrow the Endpoint profile;
they cannot add a capability. Request-local D3 protocol conversion and capability predicates still
run before a Credential lease or upstream Attempt.

## Value-free graph inventory

The production Config Version is composed from the following record classes. Values and Secret
material are not stored in this report.

| Record | Permitted content | Admission rule |
|---|---|---|
| Alias / Public Model / Route | stable opaque identity and capability requirement | resolves to a bounded SWRR Route with at least one eligible Candidate |
| Access Group / Client Key | opaque group/key identity and active status | key hash/pepper remain outside reports; every exposed Route is explicitly bound |
| Endpoint | upstream identity, adapter/wire pair and egress policy | adapter pair must exist in both the capability ledger and runtime registry |
| Credential / Binding | encrypted envelope metadata, revision and scheduling bounds | plaintext never enters the graph; active binding must share the Endpoint upstream |
| Candidate | Route/Endpoint identity, transform mode, priority and weight | effective capabilities are Endpoint profile minus any explicit narrowing |

Channels without an approved current credential are **not** represented by synthetic Secrets or
enabled Candidates. They stay outside the active production Config Version, which is the effective
`disabled-at-boot` state and makes them impossible for the router to select. Grok Web is stricter:
even an accidentally enabled Endpoint fails whole-Version runtime composition because its adapter
factory is intentionally absent.

## Verification

- `production_adapter_capability_ledger_is_conservative_and_fail_closed` proves the exact positive
  and negative capability matrix, the empty Web profile, and unknown-adapter rejection.
- `production_registry_composes_only_locally_passed_channel_pairs` proves every admitted pair has a
  runtime factory while Grok Web does not.
- `deployment_compiler_profiles_only_stored_endpoints` proves a stored representative graph carries
  T/PT/R/S into its compiled Candidates while JSON Schema and Vision remain absent.
- The existing widened graph fixture covers two Endpoints, two models, one Alias, one Access Group,
  two Client Keys, four encrypted Credentials and three Candidates without performing a send.

Targeted gateway tests and the task Full gate passed. Review found no Secret, endpoint value,
credential value, model value, request/response body, or server mutation in this change. The
user-owned untracked Kiro helper remained outside the commit.

## Remaining boundary

P12-08F2 next runs the three client protocols against four locally controlled channel classes. It
must test only combinations marked supported by this ledger, prove stable zero-attempt rejection
for unsupported/native-only combinations, and must not use production credentials or change the
public hostname.
