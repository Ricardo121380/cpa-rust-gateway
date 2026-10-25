# BC-MGMT-022: Versioned request rules and custom upstream headers

- Task: #71 / OPT-04
- Decision: [ADR-0134](../adr/ADR-0134-versioned-request-rules-upstream-headers.md)
- Public scope, limits, examples and intentional CPA differences:
  [request policy support](../support/request-rules-upstream-headers.md)

## Boundary

The existing Endpoint management input/output carries `request_policy`; existing draft revision,
audit, config diff, fork, validation, atomic publication and retained-version rollback own its
lifecycle. Schema41 appends one column after the unchanged1..40 prefix. Empty policy preserves
prior behavior. The OpenAPI Endpoint schemas describe all new shapes; unknown fields reject.

Public model/Key permission and existing source/history validation run before rule application.
Endpoint channel/target, resolved public name and optional downstream protocol form the scope.
Only documented target fields can change. Three global phases implement first-default,
last-override, then filter. Final target validation catches collisions before any transport send.
A configured filter is a persistent audited compatibility choice, never lossless delivery.

Header names and all referenced encrypted values validate before activation. References cannot
cross version/upstream, borrow account material or enter serving account bindings. Values remain
in the existing write-only Secret Store and redacted runtime Secret representation; ordinary
DTOs/audits/diagnostics contain no plaintext. Auth/provider/transport/internal headers are reserved.

Count receives only applicable additional native-count headers, never generation rule changes;
local estimate has no upstream send. Both preserve zero inference Attempts/Usage/ledger/quota.
History ownership, tool correlation, #69 name/control admission, #73 precise price identity and
old/frozen billing data, #75 formats/keys/restore boundaries remain independently enforced.

## Corresponding tests

- `request_policy_schema40_upgrade_preserves_configuration_and_ciphertext`
- `request_policy_checks_overlapping_limit_rules_after_final_filters`
- `management_contract_has_no_dangling_local_references`
- `request_policy_phases_scope_and_protected_fields`
- `request_policy_rejects_invalid_values_conflicts_and_headers`
- `request_rules_real_management_publish_public_json_sse_and_rollback`
- `request_rules_header_references_and_scope_fail_closed`
- `request_rules_count_uses_only_headers_and_never_infers_or_bills`

These are local controlled tests. Actual clients, real Provider, fixed services, CI/release and
deployment are not established by them. Historical failures and ignored tests keep their identity.
