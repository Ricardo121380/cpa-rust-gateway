import { beforeEach, describe, expect, it, vi } from "vitest";
import { CancelledError } from "@tanstack/react-query";
import { useSessionStore, readCsrfToken, readManagementKey } from "../session/sessionStore";
import { useVersionStore } from "../features/config-versions/versionStore";
import { queryClient } from "./queryClient";
import { call, callText } from "./client";

const transport = vi.hoisted(() => vi.fn<typeof fetch>());
vi.mock("../dev/fixtures", () => ({ fixtureFetch: transport }));

function select(id: string, revision = "rev-2") {
  useVersionStore.getState().select({ id, revision, status: "draft", created_at_ms: 0, description: "" });
}
function pending() {
  let resolve!: (response: Response) => void;
  transport.mockReturnValueOnce(new Promise<Response>((done) => { resolve = done; }));
  return (status = 200, revision = "rev-8", code = "management_lifecycle_conflict") =>
    resolve(new Response(JSON.stringify(status === 200 ? [] : { error: { code } }), {
      status, headers: { ETag: `"${revision}"` },
    }));
}
const scopedRead = () => call("listClientKeys", {}, { versionScoped: true });

beforeEach(() => {
  transport.mockReset();
  useSessionStore.getState().unlock(`mgmt_${"a".repeat(40)}`, `csrf_${"b".repeat(40)}`);
  select("a");
});

describe("request ownership through the generated transport", () => {
  it("rejects a late response after A → B or A → B → A", async () => {
    for (const backToA of [false, true]) {
      select("a");
      const complete = pending();
      const request = scopedRead();
      select("b");
      if (backToA) select("a");
      complete();
      await expect(request).rejects.toBeInstanceOf(CancelledError);
      expect(useVersionStore.getState().context?.revision).toBe("rev-2");
    }
  });

  it("keeps same-version revisions monotonic when reads finish out of order", async () => {
    const finishOlder = pending();
    const older = scopedRead();
    const finishNewer = pending();
    const newer = scopedRead();
    finishNewer(200, "rev-12");
    await newer;
    finishOlder(200, "rev-9");
    await older;
    expect(useVersionStore.getState().context?.revision).toBe("rev-12");
  });

  it("does not apply an unscoped or other-version ETag to the selected draft", async () => {
    let complete = pending();
    const unscoped = call("listConfigVersions");
    complete();
    await unscoped;
    complete = pending();
    const other = call("getConfigVersion", { path: { config_version_id: "b" } });
    complete();
    await other;
    expect(useVersionStore.getState().context?.revision).toBe("rev-2");
  });

  it("rejects bodies decoded after a version switch", async () => {
    let finishBody!: (body: unknown) => void;
    const response = new Response("[]", { headers: { ETag: '"rev-8"' } });
    const body = vi.spyOn(response, "json").mockImplementation(() => new Promise((done) => { finishBody = done; }));
    transport.mockResolvedValueOnce(response);
    const request = scopedRead();
    await vi.waitFor(() => expect(body).toHaveBeenCalledOnce());
    select("b");
    finishBody([]);
    await expect(request).rejects.toBeInstanceOf(CancelledError);
    expect(useVersionStore.getState().context?.revision).toBe("rev-2");
  });

  it("locks and clears secrets, versions, cached queries and mutations only on auth denial", async () => {
    queryClient.setQueryData(["protected"], { id: "private-test-row" });
    queryClient.getMutationCache().build(queryClient, { mutationKey: ["protected-write"] });
    const complete = pending();
    const request = scopedRead();
    complete(404, "rev-2", "management_access_denied");
    await expect(request).rejects.toMatchObject({ kind: "session_invalid" });
    expect(useSessionStore.getState().unlocked).toBe(false);
    expect(readManagementKey()).toBeUndefined();
    expect(readCsrfToken()).toBeUndefined();
    expect(useVersionStore.getState().context).toBeUndefined();
    expect(queryClient.getQueryCache().getAll()).toHaveLength(0);
    expect(queryClient.getMutationCache().getAll()).toHaveLength(0);
    expect(transport.mock.calls[0]?.[1]?.signal?.aborted).toBe(true);
  });

  it("keeps an ordinary resource 404 local", async () => {
    const complete = pending();
    const request = scopedRead();
    complete(404, "rev-2", "management_resource_not_found");
    await expect(request).rejects.toMatchObject({ kind: "unknown" });
    expect(useSessionStore.getState().unlocked).toBe(true);
    expect(useVersionStore.getState().context?.configVersionId).toBe("a");
  });

  it.each([200, 404, 409])("discards an old-session %s without contaminating its replacement", async (status) => {
    const complete = pending();
    const request = scopedRead();
    useSessionStore.getState().unlock(`mgmt_${"c".repeat(40)}`, undefined);
    select("a", "rev-3");
    complete(status, "rev-20", "management_access_denied");
    await expect(request).rejects.toBeInstanceOf(CancelledError);
    expect(useSessionStore.getState().unlocked).toBe(true);
    expect(useVersionStore.getState().context?.revision).toBe("rev-3");
    expect(useVersionStore.getState().conflict).toBe(false);
  });

  it("does not mark B conflicted for A's late failure or replay a write", async () => {
    const complete = pending();
    const request = call("updateClientKey", { path: { client_key_id: "test" }, body: {} }, { versionScoped: true, mutating: true });
    select("b");
    complete(409);
    await expect(request).rejects.toBeInstanceOf(CancelledError);
    expect(useVersionStore.getState().conflict).toBe(false);
    expect(transport).toHaveBeenCalledOnce();
  });

  it("does not mark the selected version conflicted for a different explicit version", async () => {
    const complete = pending();
    const request = call("validateConfigVersion", { path: { config_version_id: "b" } });
    complete(409);
    await expect(request).rejects.toMatchObject({ kind: "conflict" });
    expect(useVersionStore.getState().conflict).toBe(false);
  });

  it("does not regress when reselecting the same version from a cached picker list", () => {
    useVersionStore.getState().advanceFromEtag('"rev-12"');
    select("a", "rev-2");
    expect(useVersionStore.getState().context?.revision).toBe("rev-12");
  });

  it("drops cached object details when switching configuration versions", () => {
    queryClient.setQueryData(["credential", "shared-id"], { upstream_id: "version-a-only" });
    select("b");
    expect(queryClient.getQueryData(["credential", "shared-id"])).toBeUndefined();
  });

  it("also guards the text response path after lock", async () => {
    const complete = pending();
    const request = callText("getObservabilityMetrics");
    useSessionStore.getState().lock();
    complete();
    await expect(request).rejects.toBeInstanceOf(CancelledError);
  });
});

describe("administrator login ownership", () => {
  const grant = (digit = "a", initial = false) => ({ username: "admin", session_token: `session_${digit.repeat(64)}`,
    csrf_token: `csrf_${"b".repeat(64)}`, expires_at_ms: Date.now() + 60_000, password_change_required: initial });

  it("sends no management key or CSRF during password login and accepts only the returned session", async () => {
    const { loginAdministrator } = await import("./client");
    transport.mockResolvedValueOnce(new Response(JSON.stringify(grant("a", true))));
    await loginAdministrator("admin", "synthetic-password");
    const headers = new Headers(transport.mock.calls[0]?.[1]?.headers);
    expect(headers.has("X-Management-Key")).toBe(false);
    expect(headers.has("X-Management-CSRF-Token")).toBe(false);
    expect(useSessionStore.getState().passwordChangeRequired).toBe(true);
    expect(readManagementKey()).toBe(grant().session_token);
  });

  it("a late successful login cannot resurrect a locked page", async () => {
    const { loginAdministrator } = await import("./client");
    let finish!: (response: Response) => void;
    transport.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const request = loginAdministrator("admin", "synthetic-password");
    const rejection = expect(request).rejects.toBeInstanceOf(CancelledError);
    useSessionStore.getState().lock();
    finish(new Response(JSON.stringify(grant())));
    await rejection;
    expect(useSessionStore.getState().unlocked).toBe(false);
  });

  it("a previous login's delayed JSON body cannot replace a newer session", async () => {
    const { loginAdministrator } = await import("./client");
    let finish!: (body: unknown) => void;
    const response = new Response("{}");
    const json = vi.spyOn(response, "json").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    transport.mockResolvedValueOnce(response);
    const older = loginAdministrator("admin", "synthetic-password");
    const rejection = expect(older).rejects.toBeInstanceOf(CancelledError);
    await vi.waitFor(() => expect(json).toHaveBeenCalledOnce());
    transport.mockResolvedValueOnce(new Response(JSON.stringify(grant("c"))));
    await loginAdministrator("admin", "synthetic-password");
    finish(grant());
    await rejection;
    expect(readManagementKey()).toBe(grant("c").session_token);
  });

  it("logout clears caches immediately and revokes using captured session credentials", async () => {
    const { logoutAdministrator } = await import("./client");
    useSessionStore.getState().acceptSession(grant());
    queryClient.setQueryData(["private"], "private-data");
    let finish!: (response: Response) => void;
    transport.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const logout = logoutAdministrator();
    expect(useSessionStore.getState().unlocked).toBe(false);
    expect(queryClient.getQueryCache().getAll()).toHaveLength(0);
    const headers = new Headers(transport.mock.calls[0]?.[1]?.headers);
    expect(headers.get("X-Management-Key")).toBe(grant().session_token);
    expect(transport.mock.calls[0]?.[1]?.signal?.aborted).toBe(false);
    finish(new Response(null, { status: 204 }));
    await logout;
  });

  it("absolute expiry clears an idle session and its caches", () => {
    vi.useFakeTimers();
    try {
      useSessionStore.getState().acceptSession(grant());
      queryClient.setQueryData(["private"], "private-data");
      vi.advanceTimersByTime(60_001);
      expect(useSessionStore.getState().unlocked).toBe(false);
      expect(queryClient.getQueryCache().getAll()).toHaveLength(0);
    } finally { vi.useRealTimers(); }
  });
});
