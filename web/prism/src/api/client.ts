// Thin layer above the generated client (the ONLY fetch path, C5):
// injects session secrets via closures, threads X-Config-Version / If-Match,
// advances the revision from response ETags, and normalizes errors.
import {
  ManagementApi,
  type ManagementOperationName,
  type ManagementRequest,
  managementOperations,
} from "../generated/management-client";
import { readCsrfToken, readManagementKey } from "../session/sessionStore";
import { useVersionStore } from "../features/config-versions/versionStore";
import { isRuntimeConflict, networkError, toAppError } from "./errors";

// Dev-only fixture backend via the sanctioned options.fetch seam (C5 intact).
// Guarded by import.meta.env.DEV: release builds eliminate this branch and the
// fixtures module never reaches the bundle.
let fetchOverride: typeof fetch | undefined;
if (import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1") {
  fetchOverride = (await import("../dev/fixtures")).fixtureFetch;
}

const api = new ManagementApi({
  managementKey: () => readManagementKey(),
  csrfToken: readCsrfToken,
  ...(fetchOverride === undefined ? {} : { fetch: fetchOverride }),
});

type CallOptions = Readonly<{
  versionScoped?: boolean; // adds X-Config-Version from the version context
  mutating?: boolean; // adds If-Match and expects an ETag advance
}>;

function declaredHeaderNames(operation: ManagementOperationName): ReadonlySet<string> {
  return new Set(
    managementOperations[operation].parameters
      .filter((parameter) => parameter.in === "header")
      .map((parameter) => parameter.name.toLowerCase()),
  );
}

async function send(
  operation: ManagementOperationName,
  request: ManagementRequest,
  options: CallOptions,
): Promise<Response> {
  const version = useVersionStore.getState();
  const declared = declaredHeaderNames(operation);
  const headers: Record<string, string> = { ...(request.headers as Record<string, string> | undefined) };

  if (options.versionScoped === true && declared.has("x-config-version")) {
    if (version.context === undefined) {
      throw new Error("no config version selected");
    }
    headers["X-Config-Version"] = version.context.configVersionId;
  }
  if (options.mutating === true && declared.has("if-match")) {
    if (version.context === undefined) {
      throw new Error("no config version selected");
    }
    headers["If-Match"] = version.context.revision;
  }

  let response: Response;
  try {
    response = await api.request(operation, { ...request, headers });
  } catch (cause) {
    throw networkError(cause);
  }

  if (!response.ok) {
    const error = await toAppError(response);
    if (error.kind === "conflict" && !isRuntimeConflict(error)) {
      // A runtime snapshot rotating is not "someone edited your config", and
      // the shell's banner says exactly that. See isRuntimeConflict.
      version.markConflict();
    }
    if (error.kind === "session_invalid") {
      // Session state owns the lock transition; api layer just reports.
      version.reset();
    }
    throw error;
  }

  version.advanceFromEtag(response.headers.get("ETag"));
  return response;
}

export async function call<T>(
  operation: ManagementOperationName,
  request: ManagementRequest = {},
  options: CallOptions = {},
): Promise<T> {
  const response = await send(operation, request, options);
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

/**
 * Same request path, text body. The observability exposition is served as
 * `text/plain; version=0.0.4`, so it cannot go through call<T>() — parsing it
 * is the caller's job (src/api/prometheus.ts).
 */
export async function callText(
  operation: ManagementOperationName,
  request: ManagementRequest = {},
  options: CallOptions = {},
): Promise<string> {
  return (await send(operation, request, options)).text();
}
