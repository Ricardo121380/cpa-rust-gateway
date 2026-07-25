# P12-05 CR-006 Responses classifier capture failure

| Field | Result |
|---|---|
| Authorized scope | One server-local, non-streaming, value-free `/responses` structural classifier under `CR-P12-05-006`. |
| Input precondition | The local memory-only selector passed: the active Krill Codex configuration contained the required OAuth-field shape, a distinct selected Bearer, base URL, and model. No value was emitted. |
| Capture defect | The local zsh wrapper captured the remote classifier output, then attempted to assign the shell's reserved `status` parameter before printing it. The assignment failed, so no classifier result or receipt was retained locally. |
| Remote cleanup check | A subsequent read-only server check found zero matching ephemeral classifier files. This confirms only that the remote process had exited and cleaned up; it cannot prove whether the remote request had reached HTTP send. |
| Conservative accounting | Treat CR-006's sole request as consumed. Do not replay it or infer an HTTP/decoder result from the missing output. |
| State effect | No Staging SQLite, service, listener, Client Key, encrypted Credential envelope, route, incumbent CPA, Caddy, Cloudflare, DNS, or public traffic was changed. |
| Follow-up | `CR-P12-05-007` permits one separate replacement classifier that first persists a root-only safe receipt on the server, then emits it. |

## Review conclusion

The failure was in local result collection, not evidence of credential refresh, upstream behavior,
or a P12 source defect.  Conservatively closing the original one-request allowance prevents an
accidental duplicate.  The replacement must remain isolated, no-body-retaining, and receipt-first.
