# Grok Build OMP assistant-text replay recovery — 2026-09-08

## Scope and diagnosis

Source task: `codex://threads/01a050c1-d402-7253-9366-7a90677a69b9`
(`调研 Pi Agent 扩展配置 (3)`). Its C12 resume receipt reported that a simple
tool stream worked, but a longer OMP Inspect conversation returned HTTP 503
`CredentialUnavailable`. The protected OMP coding acceptance was not completed.

On the previously deployed `fb93c489bbbc6e2d5aa35b41b3da413b7ab5851e`, the
controlled OMP installation's actual Pi Responses client reproduced this split:
the initial tool call and tool-result response passed; replaying the resulting
assistant text into a third request failed. A minimal synthetic request with
assistant `id`, `status: completed`, and output-text `annotations: []` also
returned HTTP 503 `CredentialUnavailable`. The target Build credential was
runtime `active / available` with no outstanding lease or refresh failure.

The canonical router rejected those known history fields as unknown extensions,
excluding the candidate before inference. Previous two-request acceptance never
replayed an assistant text response, so it did not cover this case.

## Change

- Preserve bounded assistant message IDs, completed status, known commentary /
  final-answer phase, and empty text annotations for Responses-to-Responses.
- Keep unknown extensions, invalid values, and cross-protocol conversion rejected.
- Extend `scripts/verify-pi-responses-tools.mjs` to four requests per model:
  tool call, text response, another tool call with the full history, final text.
  The supplied Pi module must come from the client installation being diagnosed.
- No frontend, management schema, credential, or client-protocol change.

## Local validation

- `cargo test --locked -p gateway-router -p provider-grok -p protocol-openai-responses`:
  413 passed, 5 existing ignored, 0 failed.
- `cargo clippy --locked -p gateway-router --all-targets -- -D warnings`: passed.
- Node syntax check and `git diff --check`: passed.
- Regression exercises lossless preservation, both cross-protocol rejections,
  malformed IDs/status/phase, unexpected metadata, and invalid annotations.

## Acceptance boundary

This is a CPAR repair. It does not mark OMP C12, other provider channels, or the
P13-15 Delivery Gate complete. Synthetic SDK cost coefficients in the diagnostic
script are not evidence of zero real billing; provider cost remains unverified.

## Production and controlled-client acceptance: PASS

- Runtime/source revision: `4bb55b147518d32ac0ce6210ce652b4bb1668663`.
- Release workflow: https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34245264701
  (ARM64 and x86_64 both succeeded).
- ARM64 binary SHA-256:
  `44955dee3abde3e9a40060b19300971e64667309d78243e2143a1d7c9a3bb2af`.
- Independent Cosign verification and repository artifact verification passed;
  uploaded and installed binary hashes matched. Database backup passed SQLite
  quick_check and foreign_key_check before switching the binary.
- Recovery backup: `/var/backups/cpa-rust-gateway/omp-text-replay-20260908T153952Z`.
- Oracle service is active on the recorded revision; `/healthz` returned status ok.
- Client: OMP controlled stack's `@earendil-works/pi-ai` version `0.84.3`, stack
  `aaa97ca62ab5479528a35c6bdbd62df543bb9497a4b2bf7919111550a8c2a5eb`.
- `grok-4.6`: all four requests passed, with stop reasons
  `toolUse / stop / toolUse / stop`.
- `grok-4.5`: all four requests passed, with the same stop reasons.
- Each tool request emitted toolcall_start/delta/end and a completed event;
  parsed name/arguments matched the synthetic diagnostic. Each result request
  returned expected text. The third request included unchanged assistant text
  history from the second response. Script exited zero.
- No payload stripping, Pi configuration changes, retries, artificial inter-turn
  delay, real tool execution, account import, or OAuth was used for acceptance.

Only CPAR was restarted. The OMP C12 matrix and its evidence files were not changed.
The earlier two-request report remains historical evidence; this four-request
acceptance covers the additional assistant-text replay defect it did not exercise.
