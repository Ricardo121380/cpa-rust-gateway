import { describe, expect, it } from "vitest";
import { existingKiroTarget, kiroStartOptions, mergeKiroDeviceSession } from "./KiroDeviceDialog";

describe("Kiro device session projection", () => {
  it("keeps the official challenge while polling only updates its interval", () => {
    const started = {
      state: "pending" as const,
      session_id: "session-1",
      user_code: "ABCD-EFGH",
      verification_uri: "https://view.awsapps.com/start/#/device",
      expires_at_ms: 1_000,
      interval_ms: 5_000,
    };
    const pending = mergeKiroDeviceSession(started, { state: "pending", interval_ms: 10_000 });
    expect(pending).toMatchObject({ ...started, interval_ms: 10_000 });
    expect(mergeKiroDeviceSession(pending, { state: "completed", credential_id: "kiro-account" })).toMatchObject({
      state: "completed",
      session_id: "session-1",
      credential_id: "kiro-account",
    });
  });

  it("retains the exact existing owner for reauthorization even without a displayed binding", () => {
    expect(existingKiroTarget("kiro-existing", "")).toEqual({upstream_id:"kiro-existing",endpoint_id:""});
    expect(existingKiroTarget(undefined, "endpoint-only")).toBeUndefined();
    expect(kiroStartOptions("existing", false, "ap-southeast-1", "https://view.awsapps.com/start")).toEqual({});
    expect(kiroStartOptions("existing", true, "ap-southeast-1", "https://view.awsapps.com/start")).toEqual({region:"ap-southeast-1",start_url:"https://view.awsapps.com/start"});
  });
});
