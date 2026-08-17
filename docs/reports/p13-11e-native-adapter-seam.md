# P13-11E E2 Grok Build/Console native adapter seam report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-17

## Outcome first

P13-11E E2 now wires the E1 Provider-specific egress boundary into CPAR's native Grok Build and
Console adapter paths. The implementation reuses the existing CPAR account-pool compilation,
exact `CredentialLease`, adapter transport and Router lease lifecycle. It does not create a second
scheduler, read Autoreg, refresh an account, select a sibling Provider, or perform a real Provider,
proxy, DNS, server, staging or production request.

## Implemented boundary

- `apps/gateway/src/runtime.rs` compiles one native Build/Console egress runtime beside the already
  merged native account pools. A request must first hold the exact CPAR lease; the composition seam
  then verifies the candidate's owning Upstream/Endpoint and the fixed `grok.build` or
  `grok.console` namespace before constructing the adapter.
- `crates/provider-grok/src/provider_egress.rs` adds the exact attempt ledger. It rechecks the
  egress state at construction and submission, binds the credential ID/revision, keeps Console
  session state in a separate namespace, counts auxiliary/bootstrap and pre-submit recovery
  actions, and closes replay/recovery after the first Canonical event. No secret or URL is retained
  by the ledger or its Debug/receipt projection.
- Build uses the existing OAuth credential parser and transport. Its production transport records
  the inference submission only after egress admission and request construction, immediately before
  the single client-pool send; injected fixtures retain the same one-shot default.
- Console uses the existing SSO credential parser and DPoP transport. A missing/expired/challenged
  session consumes one bounded bootstrap/recovery slot; the token exchange is counted as auxiliary
  traffic. A `401` invalidates only the exact Console session and marks it `challenge_required` when
  an E2 attempt is active; the legacy refresh-and-second-inference path remains available only to
  the pre-existing non-E2 transport call without an attempt ledger.
- Build and Console capabilities use distinct fixed Provider IDs, credential kinds and session
  namespaces. A Build lease cannot construct a Console attempt, and Console state cannot be used
  by Build. Generic compatible, Grok Web, Official, Codex/ChatGPT, Kiro, Claude-compatible and
  arbitrary `base_url + api_key` endpoints remain outside this adapter-local slice.

## Local evidence

The new `crates/provider-grok/tests/p13_11e_native_egress.rs` fixture proves:

1. an imported Build credential and exact lease produce one synthetic transport call, one inference
   submission, and a closed ledger after Canonical output;
2. an active Console session produces one synthetic inference call without auxiliary traffic;
3. an absent Console session consumes one bounded bootstrap auxiliary slot, keeps unknown `403`
   evidence ambiguous, and maps confirmed account evidence to exact credential replacement; and
4. Build and Console namespaces reject cross-use of the same-shaped lease.

The fixture uses deterministic time, in-memory credentials, a rejecting-by-default local transport
double and non-sensitive response fixtures. It performs zero Provider, DNS, proxy, Store, Autoreg or
server calls.

## Verification

| Check | Result |
|---|---|
| `cargo test --locked -p provider-grok --all-targets` | PASS; all provider unit/integration suites, including E2 `4/4` |
| `cargo test --locked -p gateway-router --all-targets` | PASS; `158/158` |
| `cargo test --locked -p gateway --bin gateway -- --nocapture` | PASS; `109/109` |
| `cargo clippy --locked -p provider-grok -p gateway-router -p gateway --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS before final documentation staging |

## Explicit non-evidence and review boundary

This slice does not prove that any current Grok Build/Console account, SSO/DPoP token, endpoint,
proxy node, DNS route, public CPAR URL or upstream response is usable. Autoreg registration,
browser login, OAuth/SSO refresh, account entitlement and replenishment remain an independent
project boundary. The E2 adapter also does not implement Grok Web sticky egress/clearance or a
generic proxy pool; those are E3/P13-11D-owned boundaries and require their own local evidence.

No OpenAPI, Prism, frontend, management HTTP, deployment, production or server surface changed in
E2. Therefore no Claude Code frontend handoff is required for this slice. If E4 later exposes the
state or an operator action, it must first create a separate management contract and cross-boundary
log entry.

## Review conclusion

The E2 implementation is locally ready for the single P13-11E phase review. It remains
`LOCAL_PASS_PENDING_PHASE_GATE`; no formal Delivery Gate is run for this sub-slice. The next
bounded implementation is E3's fake-only Grok Web sticky egress/session/clearance seam. A real
Provider or network canary remains E5 and requires a new explicit authorization.
