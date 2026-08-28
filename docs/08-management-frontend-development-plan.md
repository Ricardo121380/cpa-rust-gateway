# Public management frontend roadmap

The management frontend exposes only protected, schema-bounded control-plane workflows.

Public development priorities:

1. Keep generated client types aligned with the versioned management OpenAPI contract.
2. Preserve explicit loading, empty, error and stale-revision states.
3. Prevent credentials, upstream URLs and sensitive request bodies from entering telemetry.
4. Maintain keyboard navigation, responsive layouts and reduced-motion support.
5. Cover configuration preview/apply, account inventory and usage projections with synthetic
   fixtures and local tests.

Environment-specific deployment plans and operator runbooks are maintained outside the public
repository.
