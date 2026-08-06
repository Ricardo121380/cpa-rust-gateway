# P12-10I-17 Grok three-channel CPAR HTTP E2E review

Decision: `BLOCKED_WITH_EVIDENCE`

1. **PASS — exact artifact.** The ARM64 release artifact was independently verified against the
   exact tested revision and its signed receipt before installation.
2. **PASS — boundary.** The test used isolated loopback listeners, separate client keys and
   provider-specific routes. Production service state, active configuration and public listeners
   were checked after rollback and remained unchanged.
3. **PASS — stop discipline.** Build and Console each stopped after the first failed Responses
   request; no automatic retry, fallback, Chat/Messages continuation or SSE request was sent.
   Web stopped before import because the source candidate failed the local expiry admission.
4. **BLOCKED — Build/Console upstream result.** Both routes passed CPAR model preflight but their
   first real provider attempt projected only `http_5xx`. The value-free result is insufficient to
   distinguish external egress, account state or upstream protocol failure; no speculative repair
   or repeated request is authorized by this receipt.
5. **BLOCKED — Web credential.** The available source pool produced no Web credential satisfying
   CPAR's bounded lifetime rule, so a public Web request was correctly not attempted.

No code change is justified by this run alone. A future rerun requires a separately authorized
valid Web session and a new diagnostic classification for the Build/Console `http_5xx` boundary.
