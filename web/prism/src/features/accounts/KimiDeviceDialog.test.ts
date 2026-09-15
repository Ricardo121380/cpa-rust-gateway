import { describe, expect, it } from "vitest";
import { existingKimiTarget, mergeKimiDeviceSession } from "./KimiDeviceDialog";

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
