// Thin layer above the generated client (the ONLY fetch path, C5):
// injects session secrets via closures, threads X-Config-Version / If-Match,
// advances the revision from response ETags, and normalizes errors.
import {
  ManagementApi,
  type ManagementOperationName,
  type ManagementRequest,
  managementOperations,
} from "../generated/management-client";
import { isAdministratorSession } from "../session/administrator";
import { CancelledError } from "@tanstack/react-query";
import { readCsrfToken, readManagementKey, useSessionStore } from "../session/sessionStore";
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

let sessionRequests = new AbortController();
useSessionStore.subscribe((state, previous) => {
  if (state.generation !== previous.generation) {
    sessionRequests.abort();
    sessionRequests = new AbortController();
  }
});

type CallOptions = Readonly<{
  versionScoped?: boolean; // adds X-Config-Version from the version context
  mutating?: boolean; // adds If-Match and expects an ETag advance
}>;

/**
 * Which operations are version-scoped is NOT a list anyone maintains here — it
 * is in the generated client, one `X-Config-Version` parameter per operation,
 * and `declaredHeaderNames` reads it. The split is per-operation and does not
 * follow the plane: within `/admin/operations/*`, account-pools, billing
 * catalogs, account failures and egress status are scoped, while usage,
 * billing, provider pools and request attempts are not. To see the current
 * split rather than trust a copy of it:
 *
 *   node -e 'const t=require("fs").readFileSync("src/generated/management-client.ts","utf8");
 *     for(const m of t.matchAll(/"([a-zA-Z]+)": \{\n    "method"[\s\S]*?"bodyEncoding"/g))
 *       console.log(/X-Config-Version/.test(m[0])?"scoped ":"unscoped", m[1])'
 *
 * Measured 2026-08-24: 84 of 99 operations declare X-Config-Version and 45
 * declare If-Match — and in every case the declaration is `required: true`.
 * Neither header is ever optional, which means both options below are strictly
 * derivable from the contract and could be deleted outright. See
 * docs/08 §3.0 for that as a follow-up.
 */
function declaredHeaderNames(operation: ManagementOperationName): ReadonlySet<string> {
  return new Set(
    managementOperations[operation].parameters
      .filter((parameter) => parameter.in === "header")
      .map((parameter) => parameter.name.toLowerCase()),
  );
}

async function send<T>(
  operation: ManagementOperationName,
  request: ManagementRequest,
  options: CallOptions,
  consume: (response: Response) => Promise<T>,
): Promise<T> {
  const version = useVersionStore.getState();
  const session = useSessionStore.getState();
  const declared = declaredHeaderNames(operation);
  const versionBound = declared.has("x-config-version") || request.path?.["config_version_id"] !== undefined
    || (managementOperations[operation].method !== "GET" && !managementOperations[operation].path.startsWith("/admin/auth/"));
  const assertSession = () => {
    if (!session.unlocked || session.generation !== useSessionStore.getState().generation) {
      throw new CancelledError({ silent: true });
    }
  };
  const assertOwner = () => {
    assertSession();
    if (request.signal?.aborted === true) throw new CancelledError({ silent: true });
    if (versionBound && version.selectionGeneration !== useVersionStore.getState().selectionGeneration) {
      throw new CancelledError({ silent: true });
    }
  };
  assertOwner();
  const headers: Record<string, string> = { ...(request.headers as Record<string, string> | undefined) };

  // Passing an option the operation does not declare used to be a SILENT no-op:
  // the header was never added and nothing said so, so a call site could carry
  // `versionScoped: true` while its read spanned every config version. That is
  // the failure mode worth refusing — believing a read is version-filtered when
  // it is not. The condition is fixed per operation, never data-dependent, so a
  // call site that runs once anywhere proves itself for good.
  if (options.versionScoped === true && !declared.has("x-config-version")) {
    throw new Error(
      `${operation} declares no X-Config-Version — versionScoped would be silently dropped`,
    );
  }
  if (options.mutating === true && !declared.has("if-match")) {
    throw new Error(`${operation} declares no If-Match — mutating would be silently dropped`);
  }

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
  const responseVersion = headers["X-Config-Version"] ?? request.path?.["config_version_id"]
    ?? (options.mutating === true ? version.context?.configVersionId : undefined);

  let response: Response;
  try {
    response = await api.request(operation, {
      ...request, headers,
      signal: request.signal === undefined ? sessionRequests.signal
        : AbortSignal.any([request.signal, sessionRequests.signal]),
    });
  } catch (cause) {
    assertOwner();
    throw networkError(cause);
  }

  assertSession();
  if (!response.ok) {
    const error = await toAppError(response);
    assertSession();
    if (error.kind === "session_invalid") {
      useSessionStore.getState().lock();
      throw error;
    }
    assertOwner();
    if (error.kind === "conflict" && !isRuntimeConflict(error)
        && responseVersion !== undefined && responseVersion === version.context?.configVersionId) {
      // A runtime snapshot rotating is not "someone edited your config", and
      // the shell's banner says exactly that. See isRuntimeConflict.
      version.markConflict();
    }
    throw error;
  }

  assertOwner();
  let value: T;
  try {
    value = await consume(response);
  } catch (cause) {
    assertOwner();
    throw cause;
  }
  assertOwner();
  // Unscoped reads and operations on another explicit version must not lend
  // their ETag to the selected draft. Check after body decoding as well.
  if (responseVersion !== undefined && responseVersion === version.context?.configVersionId) {
    version.advanceFromEtag(response.headers.get("ETag"));
  }
  return value;
}

export async function call<T>(
  operation: ManagementOperationName,
  request: ManagementRequest = {},
  options: CallOptions = {},
): Promise<T> {
  return send(operation, request, options, async (response) =>
    response.status === 204 ? undefined as T : (await response.json()) as T);
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
  return send(operation, request, options, (response) => response.text());
}

/** Login alone is unauthenticated. Ownership checks also cover response-body decoding. */
export async function loginAdministrator(username: string, password: string, signal?: AbortSignal): Promise<void> {
  useSessionStore.getState().lock();
  const generation = useSessionStore.getState().generation;
  const assertOwner = () => {
    if (generation !== useSessionStore.getState().generation || signal?.aborted === true) {
      throw new CancelledError({ silent: true });
    }
  };
  const signals = [sessionRequests.signal, AbortSignal.timeout(15_000), ...(signal === undefined ? [] : [signal])];
  try {
    const response = await api.request("loginAdministrator", { body: { username, password }, signal: AbortSignal.any(signals) });
    assertOwner();
    if (!response.ok) {
      const error = await toAppError(response);
      assertOwner();
      throw error;
    }
    const grant: unknown = await response.json();
    assertOwner();
    if (!isAdministratorSession(grant)) throw new Error("Invalid administrator session response");
    useSessionStore.getState().acceptSession(grant);
  } catch (cause) {
    assertOwner();
    throw cause;
  }
}

/** Clear locally immediately; use captured credentials only for the bounded revocation request. */
export async function logoutAdministrator(): Promise<void> {
  const { managementKey, csrfToken } = useSessionStore.getState();
  const logoutApi = new ManagementApi({ managementKey: () => managementKey, csrfToken: () => csrfToken,
    ...(fetchOverride === undefined ? {} : { fetch: fetchOverride }) });
  const request = managementKey?.startsWith("session_") === true
    ? logoutApi.request("logoutAdministrator", { signal: AbortSignal.timeout(5_000) }) : undefined;
  useSessionStore.getState().lock();
  // Local logout still succeeds offline; an unreachable server expires its session absolutely.
  await request?.catch(() => undefined);
}
