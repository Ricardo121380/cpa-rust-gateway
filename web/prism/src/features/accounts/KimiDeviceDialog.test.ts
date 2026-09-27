import { describe, expect, it, vi } from "vitest";
import { existingKimiTarget, finishKimiEnrollment, mergeKimiDeviceSession } from "./KimiDeviceDialog";

describe("Kimi device session projection", () => {
  it("retains the challenge across compact pending poll responses", () => {
    const started = {
      state: "pending" as const,
      session_id: "session-1",
      user_code: "KIMI-1234",
      verification_uri: "https://auth.kimi.com/device",
      expires_at_ms: 1000,
      interval_ms: 5000,
    };
    const pending = mergeKimiDeviceSession(started, { state: "pending", interval_ms: 10000 });
    expect(pending).toMatchObject({ ...started, interval_ms: 10000 });
    expect(mergeKimiDeviceSession(pending, { state: "completed", credential_id: "kimi-account" })).toMatchObject({
      state: "completed",
      session_id: "session-1",
      credential_id: "kimi-account",
    });
  });

  it("uses only the selected account owner during reauthorization", () => {
    expect(existingKimiTarget("kimi-account", "kimi-coding-secondary")).toEqual({upstream_id:"kimi-coding-secondary"});
    expect(existingKimiTarget("kimi-account", undefined)).toBeUndefined();
    expect(existingKimiTarget(undefined, "kimi-coding-secondary")).toBeUndefined();
  });
});


describe("Kimi authorization publication", () => {
  function enrollment(endpointId?: string, bindings: {credential_id: string}[] = []) {
    const task = {
      read: vi.fn().mockResolvedValue(bindings),
      mutate: vi.fn().mockResolvedValue({}),
      finish: vi.fn().mockResolvedValue({id: "applied", status: "active"}),
    };
    return {task: task as unknown as Parameters<typeof finishKimiEnrollment>[0]["task"], id: "kimi-account", providerId: "kimi-coding", endpointId};
  }
  it("connects the returned credential before applying a first authorization", async () => {
    const entry = enrollment("kimi-responses");
    await finishKimiEnrollment(entry, "returned-account");
    expect(entry.task.mutate).toHaveBeenCalledWith("createEndpointCredentialBinding", {
      path: {endpoint_id: "kimi-responses"},
      body: {credential_id: "returned-account", enabled: true, priority: 0, weight: 1, concurrency: 1},
    });
    expect(vi.mocked(entry.task.mutate).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(entry.task.finish).mock.invocationCallOrder[0]!);
  });
  it("preserves existing bindings during renewal", async () => {
    const entry = enrollment();
    await finishKimiEnrollment(entry, entry.id);
    expect(entry.task.read).not.toHaveBeenCalled();
    expect(entry.task.mutate).not.toHaveBeenCalled();
    expect(entry.task.finish).toHaveBeenCalledOnce();
  });
  it("does not recreate or enable an existing binding", async () => {
    const entry = enrollment("kimi-responses", [{credential_id: "kimi-account"}]);
    await finishKimiEnrollment(entry, entry.id);
    expect(entry.task.mutate).not.toHaveBeenCalled();
    expect(entry.task.finish).toHaveBeenCalledOnce();
  });
  it("does not publish when binding fails", async () => {
    const entry = enrollment("kimi-responses");
    vi.mocked(entry.task.mutate).mockRejectedValue(new Error("conflict"));
    await expect(finishKimiEnrollment(entry, entry.id)).rejects.toThrow("conflict");
    expect(entry.task.finish).not.toHaveBeenCalled();
  });
});
