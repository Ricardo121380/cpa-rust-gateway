# BC-MGMT-019: Provider-specific egress runtime status

| Field | Value |
|---|---|
| Contract | `BC-MGMT-019` |
| Task | `P13-11E4` |
| ADR | [ADR-0098](../adr/ADR-0098-provider-specific-egress-management-projection.md) |
| Status | **Accepted; local implementation passed, formal phase Gate pending** |
| Domain | Protected, read-only Provider egress/session/clearance management projection |

## 1. Endpoint and admission

The E4B implementation adds:

`GET /admin/operations/provider-egress-status`

The route is mounted only under the existing management scope and reuses BC-MGMT-003 Management
Key, peer/origin admission, selected `X-Config-Version`, safe errors and no-store response policy.
It has no unsafe method and therefore adds no operator mutation or recovery action.

The first governance slice did not modify the authoritative OpenAPI or Prism. E4B subsequently
updated the authoritative OpenAPI first, synchronized Prism and its generated client, and recorded
the exact Claude Code handoff in `docs/cross-boundary-log.md` in the same implementation change.

## 2. Query contract

The route accepts at most one value for each bounded filter:

- `provider_id`;
- `upstream_id`;
- `channel_id`, carrying the exact configured Endpoint ID;
- `domain` (`egress`, `session`, `clearance`);
- `state` from the closed domain-specific state sets;
- optional `credential_id`;
- `limit`, default 50 and maximum 100;
- opaque `cursor`, maximum 4096 characters. Runtime identities are additionally byte-bounded so a
  server-generated cursor always remains replayable within this transport limit.

Unknown parameters, duplicate parameters, empty/invalid identifiers, incompatible domain/state
pairs, zero/over-limit pages, or malformed cursors return the standard safe invalid-input error.

## 3. Snapshot response

The response contains only:

- `config_version_id` and `config_revision`;
- opaque `snapshot_id` and independent `runtime_revision`;
- fixed `sampled_at_ms`;
- at most 100 `items`;
- nullable `next_cursor`.

Rows are ordered by exact Provider, Upstream and Endpoint identity, then closed source-domain rank,
optional Credential identity plus Credential/session revisions, optional target kind/identity, and
optional clearance revision. `channel_kind` remains an explicit row field and snapshot construction
requires one unique kind for each exact
Provider/Upstream/Endpoint identity; it is therefore not duplicated in the cursor key. Null
domain-inapplicable fields are ordered deterministically and validated, not guessed.

## 4. Row contract

Every row includes:

- opaque `provider_id`, `upstream_id`, `channel_id` (the exact configured Endpoint ID);
- closed `channel_kind`: `generic_compatible`, `grok_build`, `grok_console`, `grok_web`,
  `official_api`, `codex_chatgpt`, `kiro`, `claude_compatible`, or `other_compatible`;
- `domain` and one domain-compatible `state`;
- optional safe deadline corresponding exactly to the state;
- exact domain-specific opaque identity fields.

The domain-specific identity and state rules are:

| Domain | Required identity | Forbidden identity | Closed states |
|---|---|---|---|
| Egress | target kind and optional opaque named target ID | Credential/session/clearance revisions | `available`, `cooling_down`, `circuit_open`, `probe_due`, `probe_in_flight`, `disabled` |
| Session | Credential ID, credential revision, session revision | clearance revision; unrelated target | `absent`, `active`, `expired`, `challenge_required`, `invalid` |
| Clearance | Credential/session lineage, target kind/ID, clearance revision | none of its exact lineage may be omitted | `absent`, `fresh`, `expired`, `refresh_required`, `refresh_in_flight`, `invalid` |

`direct` has no named target ID. A named target is a bounded opaque runtime identity, never an
endpoint or proxy URL. Refresh owner generation and ticket identity are never projected.
Grok Web egress and clearance rows require a named sticky target; only Grok Console and Grok Web
may own Provider-session rows, and only Grok Web may own clearance rows. Unsupported
channel/domain/target combinations make the complete source snapshot unavailable.

There is no `overall_status`, `healthy`, inferred account status, inferred quota status, or
operator recommendation. Consumers group source facts but may not erase the source domain.

## 5. Effective-time rules

All rows in one snapshot use the same `sampled_at_ms`:

- egress cooling deadlines become `available` at or after the exclusive deadline;
- egress circuit/probe-in-flight deadlines become `probe_due`;
- active sessions become `expired`;
- fresh clearances become `expired`;
- expired refresh ownership becomes `refresh_required`.

These are pure effective-state projections. A read never writes the runtime, removes an owner,
opens/closes a circuit, begins a probe, rebuilds a session, or refreshes clearance.

## 6. Cursor and consistency rules

A cursor binds:

- exact Config Version ID and revision;
- exact runtime snapshot ID/revision;
- fixed observation time;
- complete filter fingerprint;
- last stable row key.

Config/filter/key lineage mismatch, an unknown or expired retained snapshot, or a cursor whose
recorded runtime revision/observation does not match that immutable snapshot returns a safe `409`.
Live runtime mutation after a page was read does not invalidate a still-retained immutable
snapshot; it is visible only in a newer cursorless read. Pages from different snapshots are never
combined. Runtime revision is independent from Config revision; neither substitutes for the
other.

## 7. Source and channel isolation

The initial application facade may supply only native Grok Build/Console rows. Web and clearance
may therefore be empty. Empty means no projected source row; it does not mean available, fresh,
healthy, tested, or production-ready.

Generic compatible egress has a separate P13-11B/D owner. It is excluded until an explicit
compatible-source facade supplies exact Upstream/Endpoint/Credential-owned rows. Native and
compatible rows are never joined by a name or credential format. Duplicate exact keys,
cross-Provider identities, unsupported session/clearance capability, or conflicting source owners
make the snapshot unavailable rather than selecting one silently.
`refresh_in_flight` is projected only when the retained clearance state and its exact atomic owner
agree on key and deadline; missing, orphaned or mismatched ownership makes the source unavailable.

## 8. Error and redaction contract

- malformed query/cursor: safe `400`;
- snapshot/config/filter drift: safe `409`;
- missing/rejecting/poisoned/inconsistent source: safe `503`;
- all successful and error responses: `Cache-Control: no-store`.

No response, error, Debug value, cursor, audit metadata or test receipt contains endpoint/proxy
URLs, DNS answers, API keys, OAuth/SSO/Cookie/Header material, credential plaintext/ciphertext,
request/response bodies, raw Provider errors, client-key digests, refresh tickets or ownership
generations.

## 9. Required verification

1. Atomic three-domain snapshot and monotonic runtime revision under concurrent state changes.
2. Stable sorting, default/max page limits, every exact filter, and snapshot/filter/config cursor
   conflicts.
3. Deadline-derived effective state at one deterministic observation time.
4. Cross-Provider/Upstream/Endpoint/Credential/revision isolation and duplicate rejection.
5. Unsupported session/clearance, invalid field/state combinations, source poison and over-limit
   input fail closed.
6. Protected HTTP authentication/origin/version behavior, strict query decoding, safe status codes
   and no-store headers.
7. Closed OpenAPI schema, generated-client freshness and forbidden-field assertions.
8. Existing account inventory, Provider account pools/actions/failures, compatible egress runtime
   and public serving regressions remain unchanged.
9. Zero Provider, proxy, DNS, recovery, Store decrypt, serving lease, Autoreg, server, staging or
   production activity.

## 10. Local verification receipt

The focused local implementation gate passed with these exact counts:

- gateway-router full suite: `170/170`;
- gateway-control full suite: `87/87`;
- gateway binary full suite: `114/114`;
- P13-11E4 management HTTP: `4/4`;
- management OpenAPI contract: `12/12`;
- existing management runtime regression: `3/3`;
- provider-grok full suite: passed, with its authorized/live tests intentionally ignored under the
  frozen no-network boundary; historical E3 Web evidence remains `11/11`;
- strict Clippy passed for gateway-control, gateway-router, gateway-http-actix, gateway and
  provider-grok; fmt and Prism contract/client freshness check passed.

Independent final review passed with no remaining P0-P3 finding. Aggregate local Full and the
GitHub formal Delivery Gate have not run, so this contract remains `LOCAL_PASS_PENDING_PHASE_GATE` rather than
`DONE_WITH_BOUNDARY`.

## 11. Non-evidence

Passing this contract proves only that already-injected CPAR runtime facts can be observed through
an exact, bounded, value-free management snapshot. It is not proof that an account, Provider,
endpoint, proxy, DNS path, Build/Console fixed/pool transport, production Web session/clearance,
public CPAR route, staging environment, production deployment, Autoreg, or real health works. The
synthetic HTTP clearance fixture is serializer-contract evidence only. E5 remains separately
unauthorized.
