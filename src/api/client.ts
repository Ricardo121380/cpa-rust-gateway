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
import { networkError, toAppError } from "./errors";

// Dev-only fixture backend via the sanctioned options.fetch seam (C5 intact).
// Guarded by import.meta.env.DEV: release builds eliminate this branch and the
// fixtures module never reaches the bundle.
let fetchOverride: typeof fetch | undefined;
if (import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1") {
  fetchOverride = (await import("../dev/fixtures")).fixtureFetch;
}

const api = new ManagementApi({
  managementKey: readManagementKey,
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

export async function call<T>(
  operation: ManagementOperationName,
  request: ManagementRequest = {},
  options: CallOptions = {},
): Promise<T> {
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
    if (error.kind === "conflict") {
      version.markConflict();
    }
    if (error.kind === "session_invalid") {
      // Session state owns the lock transition; api layer just reports.
      version.reset();
    }
    throw error;
  }

  version.advanceFromEtag(response.headers.get("ETag"));

  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}
