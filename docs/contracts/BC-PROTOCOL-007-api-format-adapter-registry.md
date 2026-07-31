# BC-PROTOCOL-007 api_format adapter registry

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-007` |
| Domain | Validated `api_format` vocabulary and adapter dispatch |

## Boundary

```text
EndpointConfiguration.api_format + EndpointConfiguration.adapter_id
  | RouteCompiler: ApiFormat::parse + ApiFormat::serves -> unsupported_endpoint_api_format
  v
published RouteSnapshot (SnapshotRouteCandidate.endpoint_api_format)
  | composition: ApiFormatAdapterRegistry<factory> -> adapter_id -> EndpointId -> EndpointAdapter
  v
AttemptDriver::start: Candidate format == bound adapter format, else NonRetryable
```

## Invariants

1. `gateway-protocol` owns the only `api_format` string table; `ProtocolFormat` and every
   compiler/composition check render and parse through it.
2. Compile-time admission answers "can this product serve the format"; it is deployment
   independent and rejects the complete Config Version.
3. Composition-time admission answers "does this build bind an adapter"; a missing binding fails
   the whole Version's composition, never a per-Endpoint skip.
4. Adapter binding happens once, from the Config Version and `RouteSnapshot` the executor pins, so
   a request cannot cross adapter generations mid-flight (BL-08).
5. The per-attempt guard compares the Candidate's format to the bound adapter before any Secret
   read, URL composition, or socket, and fails non-retryably (BL-05, BL-06).
6. Registry diagnostics print presence flags only; no adapter value, Endpoint, URL, or Credential
   ever reaches `Debug`, a log, or an error (BL-11).
7. One `api_format` may be served by several implementations, so `ApiFormat::adapter_ids` — not a
   single canonical `adapter_id` — is the source of truth, and the Endpoint's `adapter_id` selects
   one member of that set. Both admission layers test set membership against the same table, so a
   pair one layer accepts cannot be rejected by the other. An `adapter_id` outside its format's set
   is rejected at publish time, and binding one under a foreign format fails registry construction.
   BL-06 is unchanged: one Endpoint still binds to exactly one API Format and exactly one adapter.