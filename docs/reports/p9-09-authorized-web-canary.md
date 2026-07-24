# P9-09 Grok Web authorized Canary report

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P9-09` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `DONE`; the annotated `phase-p9-complete` tag passed the [P9 Delivery Gate](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30009735294): Classify, Fast, Full supply-chain, and Required. |
| Authorization | `CR-P9-CANARY-001/002`; a temporary `grok_web/sso` export from the current grok2api service, with the user's expanded testing approval. |
| Scope / budget | Exactly three fixed Conversation `POST` requests; two server-local, response-bounded signing assists. No further live Web request is permitted for P9. |
| References | P9 Matrix `C29-C34`、`D28-D30`、`E27-E29`、`F17`、`G24-G28`; [P9-01](p9-01-grok-web-sso-credentials.md) through [P9-08](p9-08-grok-web-explicit-tool-emulation.md); `CR-P9-CANARY-001/002`. |

## Fixed boundary

The ignored `p9_09_authorized_web_canary` harness has one execution path and a per-process
atomic one-send guard. It accepts only the dedicated authorization, the fixed request cap, an
opaque target label, a temporary SSO token, a current signature, and a loopback-only SOCKS5
forward. It fixes the HTTPS target, request envelope, short text-only prompt, no attachment,
default-off Tool emulation, redirect denial, single pooled connection, and finite timeouts.

No generic provider configuration, browser profile, Cookie store, proxy/TUN configuration, server
database, account mutation, route, production Feature Flag, or fallback credential is read or
changed. Credentials, Cookie/Header values, signing material, account identifiers, endpoint query,
and response text exist only in process memory and are not logged, stored, or committed.

## Observations

| Probe | Controlled condition | Value-free result | Classification |
|---|---|---|---|
| 1 | Local direct egress without current signing material | `4xx`; no accepted Conversation frame | Egress/WAF-shaped rejection only. It does not prove SSO/account failure and maps to rebuilding the egress session, not reauthorization. |
| 2 | Current server-origin egress through a temporary loopback SOCKS5 forward, plus current signing material | `2xx`; admitted Conversation-frame shape | The repository Rust transport reached the accepted response boundary under the server's current Web runtime profile. |
| 3 | Same source egress/signing condition with live JSON-object Canonical projection | `2xx`; accepted Conversation-frame sequence and complete Canonical text lifecycle | `ResponseStart`, text, and clean `ResponseEnd` were produced without retaining the response values. |

For probes 2 and 3, one bounded server-local page retrieval and one bounded signer operation were
used only to obtain current process-memory signing material. They were not Conversation requests,
did not reveal an SSO/Cookie/signature/body, and did not alter grok2api, the server, routing, or
production configuration.

## Delivered live boundary

`GrokWebCanaryRequestBuilder` creates only the fixed request after exact shared egress admission.
It rejects any scheme, host, port, path, origin, or query substitution. Its diagnostics expose
only approved header names and byte counts. The live JSON-object decoder is deliberately separate
from P9-03's synthetic SSE fixture decoder: it accepts only the observed bounded object sequence,
requires a stable Conversation identity and final model-response envelope, and projects a
Canonical lifecycle.

The P9-09 review corrected a final-text condition before closeout: a final message that is shorter
than already emitted text now fails closed rather than being accepted as a prefix. Regression tests
also cover identity changes and a duplicate final envelope. Malformed/incomplete objects,
post-final data, text rewinds, and diagnostic values all stop or remain redacted.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_09_live_web_decoder --test p9_09_authorized_web_canary` | PASS; 12 default synthetic tests passed and the one live harness remains ignored by default. |
| `cargo clippy --locked -p provider-grok --test p9_09_live_web_decoder --test p9_09_authorized_web_canary -- -D warnings` | PASS. |
| `cargo fmt --all -- --check`, `git diff --check` | PASS. |
| [P9 local Full gate](p9-09-local-full-check.md) | PASS; workspace checks, supply-chain checks, documentation checks, and Secret scan passed. Ignored live probes stayed ignored. |
| Focused review | PASS: exact target and loopback-only proxy admission, one-send cap, redacted values, bounded response handling, no ambient credential/proxy discovery, distinct live/fixture protocol parsers, EOF/final/identity/text-rewind handling, and default-off Tool behavior were checked. |

## G9 consequence and remaining boundary

P9-09 supplies the live request/egress/protocol proof that P9-01 through P9-08 did not claim. It
does not turn the local direct rejection into an account verdict, prove any quota/Billing semantics,
enable Tool emulation, copy grok2api runtime code, enable a production Web route, or substitute for
the deferred P7 Kiro OAuth or P8 Official API-key validation.

All three permitted Conversation requests are consumed. The one annotated P9 closeout tag has
passed its GitHub Delivery Gate, so P9/G9 are complete. P10 is now eligible but remains unstarted;
no additional P9 live probe is authorized or necessary.
