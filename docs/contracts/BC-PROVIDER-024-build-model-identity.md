# BC-PROVIDER-024 Grok Build three-part model identity

| Field | Value |
| --- | --- |
| Contract | BC-PROVIDER-024 |
| Task | #60; parent #58 T02/T09 |
| Decision | [ADR-0110](../adr/ADR-0110-build-model-identity.md) |

## Public and execution boundary

The public Responses handler preserves the original requested alias and the separately resolved
public identity. `ResponsesExecution` carries the latter to the production leased driver; it is
not reconstructed from a later inventory. Request events keep both names. Standard Responses
`model` continues to name that admitted public model only after the Build identity check passes.

The driver binds the actual selected candidate and lease before dispatch. Public/upstream must
be literally equal or covered by independently approved frozen authority. Body `model` and
`x-grok-model-override` both use the actual selected upstream name. A known mismatch fails before
sending, with independent receiver count zero. Non-retryable identity failure never advances a
spare attempt, candidate, credential/account or model.

## Frozen proof trust and applicability

`GrokBuildModelAliasProof::freeze` consumes an explicit approved assertion and hash-matching source
bytes. This is an authority injection API for the composition owner: computing a hash, classifying
a source as official, or writing a route does not establish its trustworthiness. Source approval
is a prerequisite, never inferred by this API. Bytes are bounded and discarded after freezing;
there is no fetch, ambient file reader or current-inventory lookup.

| Frozen fact | Required match |
| --- | --- |
| model names | exact public, actual upstream, and the one permitted native model |
| source | approved authority category, nonempty reference and SHA256 of exact supplied evidence |
| time | `valid_from_ms <= dispatch admission time < valid_until_ms` |
| configuration | exact pinned version and revision |
| binding | provider/upstream/endpoint/route/candidate and actual credential ID/revision |
| owner/channel | exact logical Client Key (or explicit absent Pin owner) and `grok.build` |
| representation | exact downstream protocol, JSON/SSE mode and Build profile version |
| invocation | resolved binding belongs to this Request ID and outbound upstream model |

A claiming proof that is foreign, expired, premature or ambiguous is `AliasNotApplicable`.
No proof and unequal public/upstream names is `RequestMismatch`. Unrelated proof entries cannot
supply missing authority. The deployment composition currently supplies an empty proof set;
no real alias is admitted. Clearly labelled synthetic test proof cannot qualify any real sample.

## Native JSON/SSE boundary and safe failure

The actual inference adapter always supplies the expected native name. Structural-only decoder
entry points without an execution binding make no model identity claim. Native snapshots checked:

- JSON terminal response;
- SSE created, in-progress, completed, incomplete and failed response objects.

Each requires a string of 1–512 UTF-8 bytes with no leading/trailing whitespace or control characters.
Missing is `MissingNativeModel`; null/non-string/invalid bounded shape is `InvalidNativeModel`.
A first unproven name is `NativeModelMismatch`; a later changed name is `ResponseModelConflict`.
The comparison is exact and case-sensitive. Alias acceptance cannot weaken phase consistency.

Errors retain existing categories/status/framing and fixed value-free text. No native model, source
reference, token, body or proof value enters public errors or ordinary logs. Initial failures use
the existing HTTP error path; after committed SSE headers there is one legal failure and no
`response.completed`. Independent receiver counts remain one for a response-time failure.
No extra upstream read is made to discover identity or Usage after rejection.

## Existing evidence and scope

Actual Request/Attempt and lease/configuration/channel ownership remain unchanged. No new native
model history is fabricated or backfilled from inventory. #59 remains an independent contract:
[BC-OBS-006](BC-OBS-006-native-usage-evidence.md). Its model-valid fixtures still fail at unrepresented
Usage and prove exact encrypted retention before the public failure. The old synthetic missing-model
case remains separately rejected at identity; its unread/unvalidated Usage is not claimed retained.
Known price-like values never create a bill. Source evidence, historical FAIL/oracles and stopped
epochs are not edited. Ordinary metadata, reasoning delivery, native-v1 and #61 are outside this change.

## Corresponding tests

- `public_build_model_identity_rejects_native_mismatch_without_resend`
- `public_build_model_identity_rejects_unproven_request_before_send`
- `public_build_model_identity_literal_alias_and_exact_request_lineage`
- `public_build_model_identity_rejects_foreign_expired_and_conflicting_alias_proofs`
- `public_build_model_identity_missing_invalid_and_phase_conflicts_are_distinct`
- `public_build_model_identity_failure_precedes_unvalidated_usage`
- `public_responses_rejection_retains_complete_encrypted_usage`
- `public_responses_reports_private_recording_failure_without_losing_counters`
