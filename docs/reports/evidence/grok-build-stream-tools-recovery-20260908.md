# Grok Build streaming tool-call recovery — 2026-09-08

## Reproduction and cause

A synthetic public `grok-4.6` streaming tool request returned HTTP 200 followed by
`response.failed / UpstreamProtocolError`, without tool arguments. A direct Build
request from the VPS completed with tool events. Upstream argument delta/done
records carried `item_id` but omitted `call_id`; the preceding output-item-added
record had already declared the mapping. The Build parser incorrectly required
`call_id` on each argument record. A local isolated reproduction failed without
that field and passed when it was added.

## Correction and regression boundary

Revision `e45f5fc853614999439fa426753cefbb8a919df1` resolves argument records through
the registered item-to-call mapping. An optional explicit call ID must still be
valid and match that mapping. Unknown items, invalid/mismatched explicit IDs and
arguments after call completion remain rejected. No model-specific switch or
synthetic ID is introduced.

Both the generic Responses upstream decoder and the gateway's separate Responses
stream decoder already associate these events by item ID; they do not have this
same mandatory-call-ID defect. This is a scoped inspection, not acceptance of
all providers or all tool types.

The provider-grok and protocol-openai-responses packages passed 236 tests. New
regressions cover absent argument-event call IDs, legacy IDs, arbitrary byte
chunking, semantic equivalence to non-streaming output, two interleaved calls,
unknown items, explicit ID mismatches/nulls and late deltas. These tests are part
of the existing Rust test suites and do not depend on a model-name whitelist.

## Production acceptance

Signed deployment and two-turn testing through the locally installed Pi Responses
client passed for both current Build models (see final verdict below). The test uses a synthetic echo
function, checks its name and parsed JSON arguments, supplies a synthetic tool
result, and requires a subsequent text answer. No actual user tool is executed.
No credentials, user content, reasoning text or raw provider response is recorded.

## Two-turn acceptance exposed a second defect

The first deployed fix delivered toolcall_start/delta/end to the actual Pi client.
The initial harness omitted Pi's normalized cost metadata and failed locally when
reading `tiers`; supplying that metadata corrected the harness. The next request
then returned CredentialUnavailable even after a delay, with no active leases.
Pi replays the Responses function-call item ID. The canonical router rejected this
known field as an unknown content extension, excluding the candidates before
inference. A bounded comparison removing only that ID passed both models' two-turn
loops, confirming the cause. That diagnostic omission is not the production fix.

The router now preserves a bounded nonempty printable function-call `id` for
Responses-to-Responses canonical requests. Other extensions, invalid IDs and
cross-protocol replay remain rejected. The regression includes a function call
plus its function_call_output, field preservation and negative cases.

## Repeatable model acceptance

`scripts/verify-pi-responses-tools.mjs` imports the actual installed Pi Responses
client. It requires explicit `--execute`, module path, model configuration path,
provider name and model IDs. It sends two requests per model, never executes real
tools, uses no retries or artificial inter-turn delay, and checks tool name, parsed
arguments, stop reason and continuation text. It neither edits Pi configuration nor
prints credentials/model content. Run only when live provider calls are authorized:

```sh
node scripts/verify-pi-responses-tools.mjs --execute \
  /absolute/path/to/pi-ai/dist/api/openai-responses.js \
  /absolute/path/to/models.json provider-name model-id [another-model-id]
```

New models should pass this full tool loop separately from model discovery and
plain-text acceptance. Synthetic tests prevent recurrence of these two known
protocol assumptions; no claim is made that future upstream changes cannot break
other behavior. Third-party Pi updates can also require harness compatibility work.

## Final production verdict: PASS

- Signed runtime: `fb93c489bbbc6e2d5aa35b41b3da413b7ab5851e`.
- Release: https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34240044882
  (ARM64 and x86_64 both passed).
- ARM64 SHA-256: `30690b5e29fd07dd25524768959ebe1e64ad9422e809497fc51bb67311b66a65`.
- Independent Cosign and repository artifact checks passed; upload/install hashes
  matched, database backup passed integrity/foreign-key checks, service health passed.
- Final backup: `/var/backups/cpa-rust-gateway/tools-fix-20260908T145128Z`.
- The committed Pi verification script passed both `grok-4.6` and `grok-4.5` on this
  binary. Each emitted toolcall_start/delta/end and completed with toolUse; the
  immediately following tool-result request completed with text and stop. No
  payload workaround, retries, inter-turn delay or Pi configuration edit was used.
- Combined router/provider/protocol tests: 412 passed. Strict Clippy for router and
  provider all-targets passed after equivalent boolean/ownership and fixture-style
  cleanup. Those source cleanups follow the signed runtime revision; they do not
  change protocol behavior and are not claimed as part of its binary identity.

Only CPAR was restarted. No account import, OAuth, model whitelist, API shape,
frontend file, proxy setting or unrelated service was changed. This closes the
reported Build streaming tool-call/replay defect; it is not acceptance of every
provider or every possible future model behavior.
