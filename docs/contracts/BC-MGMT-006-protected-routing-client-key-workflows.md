# BC-MGMT-006 Protected routing and Client Key workflows

| Field | Value |
|---|---|
| Contract | `BC-MGMT-006` |
| Task | `P10-05` |
| Status | Accepted |
| Domain | Protected Versioned routing and access management |

## Entry and scope

`configure_management_resources` extends the P10-02-protected `/admin` Scope with only the
frozen P10-01 operations for `PublicModel`, Alias, Route, Candidate, `AccessGroup`,
`AccessGroupRoute`, and Client Key issue/read/update/revoke. Every read needs one valid
`X-Config-Version`; every mutation needs the exact draft `If-Match: rev-N`, management principal,
network admission and CSRF enforcement that P10-02 performs before a body is decoded.

This contract covers draft topology only. It does not publish a Snapshot, choose a Candidate,
make an inference or Provider request, inspect Health/Quota/403 state, perform Route Explain,
serve a management listener, or start backup/restore work.

## Graph and transaction invariants

1. Public Model, Alias, Route, Candidate, Access Group, grant and Client Key mutations use the
   existing exact-revision SQLite transaction. The resource change, bounded non-secret audit
   event and revision increment commit together; success returns the new quoted ETag and stale
   writes return the fixed 409 conflict without a partial graph or audit event.
2. Parent identities are path-owned: an Alias and a Route use the Public Model path, a Candidate
   uses the Route path, and a grant uses the Access Group path. The body owns only its frozen
   OpenAPI fields. All referenced parents and Candidate Endpoints must be present in the same
   draft Version.
3. Delete semantics follow the schema-owned topology. Deleting a Route removes its Candidates and
   Access Group grants; deleting a Public Model removes its Aliases and Routes; deleting an Access
   Group removes its grants and Client Keys. No handler publishes or recompiles a replacement
   Snapshot after such a mutation.
4. P10-05 accepts and emits only `smooth_weighted_round_robin`. A legacy stored `round_robin` or
   `priority_failover` record is rejected by the response boundary with the fixed internal-error
   envelope, never relabelled as a different policy. Route validation is structural topology-only
   and has no Router/Provider handle.

## Client Key invariants

1. `gateway_auth::client_key::ClientKeyService` is the sole issuer and is explicitly injected into
   `ManagementMutationService`. The HTTP layer, request, browser, environment and database never
   load a Pepper or construct a Client Key issuer.
2. Issuance obtains a service-produced Prefix and HMAC digest, persists only the redacted record
   in the audit/revision transaction, and returns the complete Key only after that transaction
   commits. A failed mutation exposes no complete Key.
3. Get, list, update, revoke, Debug, audit and report paths expose only `id`, Access Group,
   Prefix, status and optional expiry. The key's digest is never serialized; changing an Access
   Group, status or expiry and revocation do not rotate or re-present the Key.
4. The SPA removes an issued Key from the general operation result and displays it only in an
   explicit transient `Display once` pane. It clears that pane before the next operation, on a
   failure, on explicit clear and on reload. No Management Key, CSRF token or Client Key is placed
   in URL, Cookie, local/session/indexedDB storage, clipboard, request preview or normal result.
5. The local static-browser fixture supplies `frame-ancestors 'none'` through a CSP response
   header, rather than an ineffective HTML meta directive. P10-09 must carry that header into the
   final embedded static-asset host; P10-05 does not add the production listener.

## Corresponding tests

`management_mutation_service` uses in-memory SQLite and a synthetic injected issuer to verify
HMAC-only persistence, immediate presentation, revision/audit ordering, update/revoke and cascade
semantics. `p10_05_management_routing` covers protected HTTP graph creation through the public
name `minimax-m3`, stale writes, structural validation, redaction and cascades. The management
SPA check verifies generated-client-only operations, all P10-05 wrappers, transient Client Key
handling and deterministic assets. The local P10-05 browser fixture creates the synthetic graph,
then confirms that a subsequent metadata read and page reload remove the display-once value.
