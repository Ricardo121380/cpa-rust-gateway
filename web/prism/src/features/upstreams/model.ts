// Upstream-domain pure model: the credential OAuth lifecycle.
//
// Graph slicing lived here until P13-04A landed listOperationalAccountPools —
// per-provider subresources now come from that real projection (pools.ts),
// so the proposed-G1 slicer is gone rather than kept "just in case".
export type OAuthState = "pending" | "complete" | "cancelled" | "failed" | "expired";

/** TanStack Query refetchInterval: poll every 2s while pending, else stop. */
export function oauthPollIntervalMs(state: OAuthState | undefined): number | false {
  return state === "pending" ? 2000 : false;
}

export function oauthStateBadge(state: OAuthState): string {
  // maps onto the shared badge vocabulary (StatusBadge tones)
  switch (state) {
    case "pending":
      return "recovery_required"; // tint tone: in progress
    case "complete":
      return "active";
    case "cancelled":
      return "disabled";
    case "expired":
      return "disabled";
    case "failed":
      return "credential_forbidden";
  }
}

/** The contract's closed failure set — the wizard used to show only "failed". */
export type OAuthFailureClass =
  | "session_start_failed"
  | "session_missing"
  | "state_mismatch"
  | "token_exchange_failed"
  | "provider_rejected"
  | "persistence_failed";

export function oauthFailureLabel(failure: OAuthFailureClass): string {
  switch (failure) {
    case "session_start_failed":
      return "网关未能发起授权会话";
    case "session_missing":
      return "授权会话已不存在(可能已过期或被取消)";
    case "state_mismatch":
      return "回调的 state 与会话不匹配 —— 请用本次授权产生的地址";
    case "token_exchange_failed":
      return "换取令牌失败";
    case "provider_rejected":
      return "上游拒绝了这次授权";
    case "persistence_failed":
      return "令牌已取得但写入失败";
  }
}

// The contract caps callback_url at 20480 and state at 512.
const MAX_CALLBACK_URL = 20480;
const MAX_STATE = 512;

/**
 * A server-supplied string becomes an href here, so the scheme is checked:
 * `javascript:` in an anchor runs on click, and "the gateway sent it" is not
 * the same as "safe to hand to the browser as code".
 */
export function safeExternalUrl(raw: string | null | undefined): string | undefined {
  if (raw === null || raw === undefined || raw.length === 0) {
    return undefined;
  }
  try {
    const url = new URL(raw);
    return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : undefined;
  } catch {
    return undefined;
  }
}

export type OAuthCallbackInput = Readonly<{
  state: string;
  code?: string;
  error?: string;
  callback_url?: string;
}>;

export type ParsedCallback =
  | Readonly<{ ok: true; input: OAuthCallbackInput }>
  | Readonly<{ ok: false; reason: string }>;

/**
 * Reads what the provider redirected the operator to, so they can paste the
 * whole address instead of hand-picking parameters out of it.
 *
 * `state` is required by the contract and is what binds the paste to the
 * session this wizard started — a paste without it cannot be completed, and
 * saying so here is better than a 400 from the gateway.
 */
export function parseOAuthCallback(raw: string): ParsedCallback {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { ok: false, reason: "请粘贴授权后浏览器跳转到的完整地址。" };
  }
  if (trimmed.length > MAX_CALLBACK_URL) {
    return { ok: false, reason: "地址超出契约允许的长度(20480 字符)。" };
  }

  let params: URLSearchParams;
  let absolute: string | undefined;
  try {
    const url = new URL(trimmed);
    params = url.searchParams;
    absolute = trimmed;
  } catch {
    // Not an absolute URL — accept a bare query string, with or without "?".
    params = new URLSearchParams(trimmed.startsWith("?") ? trimmed.slice(1) : trimmed);
  }

  const state = params.get("state") ?? "";
  if (state.length === 0) {
    return {
      ok: false,
      reason: "地址里没有 state 参数 —— 这不是本次授权的回调地址。",
    };
  }
  if (state.length > MAX_STATE) {
    return { ok: false, reason: "state 超出契约允许的长度(512 字符)。" };
  }

  const code = params.get("code") ?? "";
  const error = params.get("error") ?? "";
  if (code.length === 0 && error.length === 0) {
    return {
      ok: false,
      reason: "地址里既没有 code 也没有 error —— 授权可能没有完成。",
    };
  }

  return {
    ok: true,
    input: {
      state,
      ...(code.length > 0 ? { code } : {}),
      ...(error.length > 0 ? { error } : {}),
      ...(absolute === undefined ? {} : { callback_url: absolute }),
    },
  };
}
