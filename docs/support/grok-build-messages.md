# Grok Build → standard Messages: Claude D2 restricted support

As of **2026-10-23**, the user has selected **Claude D2 for the first release** under
[ADR-0123](../adr/ADR-0123-first-release-claude-d2-restricted-support.md).
The exact four-variable configuration below has historical **PASS_LOCAL_CONTROLLED** evidence.
Fixed Build service qualification remains **BLOCKED**, #64/#66 remain **OPEN**, and real Provider
acceptance is **NOT_RUN**. First-release READY still requires Codex, Claude D2 and Pi all to
complete the agreed Grok Build gates. See the [candidate inventory](first-release-candidates.md)
for versions, configurations, evidence sources and separate qualification states.

This path does **not display Grok reasoning summary**. The gateway retains confirmed summary,
item/part identity, hierarchy and order as encrypted diagnostics, readable only through the
authorized local reader. Missing reasoning body remains missing. Summary is never inserted into
answer text or presented as thinking/signature.

The implemented scope is Grok Build with CanonicalBridge, standard `/v1/messages` JSON/SSE and
explicit client-supplied Messages text/tool history. This limited delivery contract does not claim
complete LosslessBridge; Passthrough remains exact. Responses still delivers native summary;
Chat and other Providers do not inherit this exception.

Successful closure requires native validation, legal standard projection and confirmed complete
encrypted retention. Tool name/ID/object arguments and results, known Usage and model identity
retain their existing rules. Unsupported thinking budgets, unrepresentable history, cipher/signature/citation, unknown content
and unqualified Usage remain rejected. Other reasoning keeps the existing strict projection; a
body-only representation is not hidden, and a mixed summary/body item remains unsupported. `cost_in_usd_ticks`
is still NOT_ADMITTED. Missing costs remain UNKNOWN.

The controlled continuation replays only what the standard client received. It does not obtain
hidden reasoning from diagnostics or claim a server-side native continuation mode. A mode requiring
unrepresented native reasoning/identity remains outside this scope.

Implementation and controlled loopback evidence are separate from real use. Claude D2's actual
controlled acceptance is recorded in the [D2 report](../reports/cpar-64-d2-candidate-20261022.md);
the original Claude configuration remains blocked under
[#64](https://github.com/Ricardo121380/cpa-rust-gateway/issues/64). The formal
[#61](https://github.com/Ricardo121380/cpa-rust-gateway/issues/61) dependency and its actual
field/profile/model qualification remain. No real Provider, paid request, release or deployment
is implied. See [the delivery contract](../contracts/BC-OBS-007-messages-summary-evidence.md).

## Fixed client and complete local configuration

The evidenced macOS arm64 client is **Claude Code 2.1.302**, entry
`/Users/huangrui/.local/bin/claude`, resolving to
`/Users/huangrui/.local/share/claude/versions/2.1.302`, SHA-256
`5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37`.
The receipt freezes both entry and binary at mode0755. This is a version-bound result, not a
claim for newer/older clients, other platforms, interactive/desktop modes or the default profile.
Before a later authorized run, compare the installed identity with the receipt; do not silently
upgrade, downgrade or reuse the result for a different executable.

All four values are mandatory together:

```sh
DISABLE_PROMPT_CACHING=1
CLAUDE_CODE_DISABLE_THINKING=1
CLAUDE_CODE_MAX_RETRIES=0
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
```

The first two remove captured cache controls and thinking; this version also omitted
context_management. **Omitting thinking does not mean the model stops reasoning.** Actual D2
does not exercise M3 keep-all; its independent contract tests remain the M3 evidence. D2 does
not grant adaptive/display, overall effort or block prompt-cache semantics. Those original
Claude requirements remain open follow-up work under #58/#64, without deletion or completion.

MAX_RETRIES=0 disables ordinary API retries but did not by itself prevent this version's
non-streaming fallback after a stream failure. Both earlier extra-send REDs remain in the
D2 report. The separate fallback variable is required in normal, failure and cancellation
runs. A two-variable or MAX_RETRIES-only profile is outside the adopted configuration.

The following configuration-only example reuses the existing driver's isolated HOME/config/tmp
layout. It uses placeholder CPAR endpoint/key values and pins all model selections to the
evidenced `grok-4.5` public model. It creates only a new private task directory; the client command
is deliberately commented, so copying this block sends no request and changes no global settings.
The environment is cleared before the local shell is created, so existing account credentials
and alternate Provider/proxy settings are not inherited.

```sh
env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin /bin/sh <<'CPAR_D2_CONFIG'
set -eu
umask 077
CPAR_D2_ROOT="$(mktemp -d /private/tmp/cpar-claude-d2.XXXXXX)"
mkdir -p "$CPAR_D2_ROOT/home" "$CPAR_D2_ROOT/config" "$CPAR_D2_ROOT/cache" \
  "$CPAR_D2_ROOT/tmp" "$CPAR_D2_ROOT/work"
export HOME="$CPAR_D2_ROOT/home" CLAUDE_CONFIG_DIR="$CPAR_D2_ROOT/config"
export XDG_CONFIG_HOME="$CPAR_D2_ROOT/config" XDG_CACHE_HOME="$CPAR_D2_ROOT/cache"
export TMPDIR="$CPAR_D2_ROOT/tmp" CLAUDE_CODE_TMPDIR="$CPAR_D2_ROOT/tmp" TERM=dumb
export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 DISABLE_AUTOUPDATER=1
export NO_PROXY=127.0.0.1,localhost
export ANTHROPIC_BASE_URL='http://127.0.0.1:<CPAR_DATA_PORT>'
export ANTHROPIC_API_KEY='<CPAR_CLIENT_KEY>'
export ANTHROPIC_MODEL=grok-4.5 ANTHROPIC_DEFAULT_HAIKU_MODEL=grok-4.5
export ANTHROPIC_DEFAULT_SONNET_MODEL=grok-4.5 ANTHROPIC_DEFAULT_OPUS_MODEL=grok-4.5
export DISABLE_PROMPT_CACHING=1
export CLAUDE_CODE_DISABLE_THINKING=1
export CLAUDE_CODE_MAX_RETRIES=0
export CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
cd "$CPAR_D2_ROOT/work"
# Existing controlled profile, shown only as a command shape; no invocation here:
# /Users/huangrui/.local/share/claude/versions/2.1.302 --bare --print \
#   --no-session-persistence --verbose --output-format stream-json \
#   --include-partial-messages --input-format stream-json --model grok-4.5 \
#   --tools Read,Bash --permission-mode bypassPermissions
CPAR_D2_CONFIG
```

The settings belong to that local shell. For an authorized controlled run, replace placeholders
and manually invoke the command inside the same shell with the existing bounded stream-json
driver; its bypassPermissions setting applies only to the disposable Read/Bash test workspace.
This recipe supplies configuration, not the driver's OS network/file fence or its tool/terminal
oracle. Use [cpar64-claude.py](../../scripts/acceptance/cpar64-claude.py) with `--d2`, a new
`--output` directory and an isolated `--target-dir` when reproducing controlled acceptance.
No new launcher is needed. A genuine CPAR Key belongs only in an explicitly authorized secure
input path; do not replace the placeholder in committed files or ordinary logs.

The example does not activate fixed Build support. A route, Client Key and model access must be
prepared under the existing management flow, and fixed field/profile/model qualification must
be satisfied independently before real use. The current delivery's default Build qualification
is not changed by these environment variables. Do not enable controlled-test-support in an artifact.

## Initial effort rejection and client-owned negotiation

The first D2 request still contains output_config.effort=high. Only when the complete matching
candidate set proves that effort is the sole remaining conversion rejection does CPAR return
the fixed Messages HTTP400 / invalid_request_error:

```text
output_config.effort: Extra inputs are not permitted by the current gateway conversion; no upstream request was sent
```

This accurately identifies **the gateway conversion** as the rejecting party. The rejected
Request has receiver/lease/Attempt/Usage/ledger=0 and may retain valid Request-only M4 evidence.
The frozen client then initiates a new authenticated Request without output_config. CPAR never
deletes a field from the first request, fabricates an upstream error or transparently resends it.
Each dispatched new request has its own actual Attempt; Usage and ledger entries record only
validated evidence, with missing values remaining UNKNOWN. MAX_RETRIES=0 and disabled
non-streaming fallback do not suppress this independently evidenced client capability negotiation.
Other controls, unknown fields and mixed rejection reasons retain their accurate rejection.

Messages still does not display Grok summary. The [D2 evidence](../reports/cpar-64-d2-candidate-20261022.md)
demonstrates actual Read/Bash tool results, follow-up, full ordered visible history, complete
encrypted summary/M4 readback, bounded permanent failure and same-PID-alive cancellation.
Those results are local controlled evidence on code `4015bd45`; they are not new Linux/OCI
client executions or real fixed-service acceptance. See [BC-PROTOCOL-016](../contracts/BC-PROTOCOL-016-claude-restricted-candidate.md)
for the exact admission and rejection conditions.

## Historical original-configuration observation (2026-10-20)

The following observation predates D2 adoption and applies to the original configuration.

Actual CLI compatibility is **BLOCKED**. In the frozen macOS `--bare --print` profile using
literal `grok-4.5`, the unmodified client sends adaptive thinking (initially with `display: updates`),
`output_config.effort: high`, `context_management`, `metadata.user_id`, and block-local ephemeral
cache controls. These controls lack an admitted exact Messages-to-Build mapping. Both observed
Messages requests return HTTP 400 before any independent upstream TCP accept or complete request.
The second is a client-side fallback that omits `thinking.display`; the gateway does not rewrite
or resend either request. No successful answer, actual tool execution, summary readback, or
in-flight client cancellation is qualified by that rejection.

The existing exact output-limit and tool-choice/serial mappings remain supported by separate
controlled protocol probes. They do not remove the other fields from the actual CLI request.
An exit or result labelled `success` by the client is insufficient when `is_error` is true.
`HEAD /`, title generation, and `count_tokens` were not observed in this particular profile;
the existing explicit unsupported token-count response remains unchanged.

Reproduce only the isolated observation with
`python3 scripts/acceptance/cpar64-claude.py --diagnostic --output /private/tmp/new-cpar64-run`.
Omitting `--diagnostic` preserves the observation and exits unsuccessfully because the required
client tool loop is blocked. This runner neither installs/updates the client nor uses a real
Provider account. See [the #64 report](../reports/cpar-64-claude-messages-20261020.md) for exact
client/source identities, evidence, acceptance gaps, and the unchanged #64 → #63 → #61 dependencies.
