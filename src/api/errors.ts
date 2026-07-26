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
