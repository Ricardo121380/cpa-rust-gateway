# Schema27 rollback compatibility

Deployment fallback for the user's explicitly authorized 2026-09-13 rollout. Based on production
57e32d1, with only schema27 request-terminal decoding/migration/skip handling and the explicit-model
serving guard backported. The production frontend and other runtime behavior remain at that baseline.

The fallback reads all existing/new terminal records without dropping or rewriting them. It does not
produce new terminal observations, so its new requests have unknown terminal/latency when viewed by
a later full build. Billing continues to ignore terminal events and retains idempotent checkpoints.
Never run migration27 down or restore stale credentials/admin databases for this fallback.

Signing and isolated production-copy forward/fallback/forward evidence is required before cutover.
No inference requests against real providers form part of this release acceptance.
