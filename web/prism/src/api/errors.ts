// Uniform management error envelope mapping (docs/07 §6.3).
// The backend intentionally returns one opaque 404 for every auth/network/origin
// failure; 503-style rejections from injected facades are a first-class
// "projection unavailable" state, not an error toast.

export type AppErrorKind =
  | "session_invalid" // 404 management_access_denied → back to unlock
  | "invalid_request" // 400 → form-level message
  | "conflict" // 409 → refetch + conflict bar, never replay
  | "unavailable" // 503 → projection-unavailable empty state
  | "network"
  | "unknown";

export type AppError = Readonly<{
  kind: AppErrorKind;
  code: string;
  message: string;
  status: number | undefined;
}>;

type ErrorEnvelope = Readonly<{ error?: { code?: string; message?: string } }>;

export function classifyStatus(status: number, code: string): AppErrorKind {
  if (status === 404 && code === "management_access_denied") {
    return "session_invalid";
  }
  if (status === 400) {
    return "invalid_request";
  }
  if (status === 409) {
    return "conflict";
  }
  if (status === 503) {
    return "unavailable";
  }
  return "unknown";
}

export async function toAppError(response: Response): Promise<AppError> {
  let code = "unknown";
  let message = "";
  try {
    const parsed = (await response.json()) as ErrorEnvelope;
    code = parsed.error?.code ?? code;
    message = parsed.error?.message ?? message;
  } catch {
    // non-JSON body: keep defaults
  }
  return {
    kind: classifyStatus(response.status, code),
    code,
    message,
    status: response.status,
  };
}

/**
 * 409s that mean a RUNTIME snapshot moved, not that the configuration changed.
 *
 * The shell's conflict bar says "配置已被其他会话修改" — true for an If-Match
 * failure, false for every code below. An operational cursor going stale, or an
 * action's target account moving between read and write, happens with nobody
 * editing anything; raising the config banner for it sends the operator to look
 * for a config change that never occurred.
 *
 * `management_provider_egress_status_config_conflict` is deliberately NOT here:
 * that one really does mean the selected version is no longer this snapshot's
 * source, so the banner is the correct response to it.
 *
 * Callers still see kind === "conflict" and still must not replay the request —
 * this only governs whether the global version banner fires.
 */
const RUNTIME_CONFLICT_CODES: ReadonlySet<string> = new Set([
  "management_operations_cursor_conflict",
  "management_provider_account_pool_cursor_conflict",
  "management_provider_egress_status_cursor_conflict",
  "management_provider_account_action_target_changed",
  "management_channel_pin_target_changed",
]);

export function isRuntimeConflict(error: AppError): boolean {
  return error.kind === "conflict" && RUNTIME_CONFLICT_CODES.has(error.code);
}

export function networkError(cause: unknown): AppError {
  return {
    kind: "network",
    code: "network_error",
    message: cause instanceof Error ? cause.message : "network failure",
    status: undefined,
  };
}

export function asAppError(error: unknown): AppError {
  if (typeof error === "object" && error !== null && "kind" in error) {
    return error as AppError;
  }
  return {
    kind: "unknown",
    code: "unknown",
    message: error instanceof Error ? error.message : String(error),
    status: undefined,
  };
}
