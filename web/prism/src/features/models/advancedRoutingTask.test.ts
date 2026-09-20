import { beforeEach, describe, expect, it, vi } from "vitest";
import { CancelledError } from "@tanstack/react-query";

vi.mock("../../api/client",()=>({call:vi.fn(),callRevisioned:vi.fn()}));

import { call, callRevisioned } from "../../api/client";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { captureDraftRoutingOwner, runDraftRoutingWrite } from "./advancedRoutingTask";

const version={id:"draft-route",revision:"rev-4",status:"draft" as const,created_at_ms:1,description:""};

describe("advanced routing draft writes",()=>{
  beforeEach(()=>{
    vi.resetAllMocks();
    useSessionStore.getState().unlock(`mgmt_${"x".repeat(27)}`,undefined);
    useVersionStore.getState().select(version);
    vi.mocked(call).mockResolvedValue(version as never);
  });

  it("uses a fixed draft revision and returns a saved-only receipt",async()=>{
    vi.mocked(callRevisioned).mockResolvedValue({value:{id:"route"},revision:"rev-5"} as never);
    const owner=captureDraftRoutingOwner();
    const result=await runDraftRoutingWrite(owner,"updateRoute",{path:{route_id:"route"},body:{id:"route"}},async()=>{});
    expect(result.receipt).toMatchObject({kind:"saved_draft",acknowledgedWrites:1,workingVersion:{id:"draft-route",revision:"rev-5"}});
    expect(callRevisioned).toHaveBeenCalledWith("updateRoute",{path:{route_id:"route"},body:{id:"route"},headers:{"X-Config-Version":"draft-route","If-Match":"rev-4"}});
  });

  it("rejects a changed revision before dispatching a write",async()=>{
    const owner=captureDraftRoutingOwner();
    vi.mocked(call).mockResolvedValue({...version,revision:"rev-5"} as never);
    await expect(runDraftRoutingWrite(owner,"deleteRoute",{path:{route_id:"route"}},async()=>{})).rejects.toThrow("草稿已变化");
    expect(callRevisioned).not.toHaveBeenCalled();
  });

  it("keeps response loss non-replayable",async()=>{
    vi.mocked(callRevisioned).mockRejectedValue(new Error("response lost"));
    const result=await runDraftRoutingWrite(captureDraftRoutingOwner(),"deleteRoute",{path:{route_id:"route"}},async()=>{});
    expect(result.receipt.kind).toBe("unconfirmed");
    expect(callRevisioned).toHaveBeenCalledTimes(1);
  });

  it("does not turn a known conflict into a saved receipt",async()=>{
    vi.mocked(callRevisioned).mockRejectedValue({kind:"conflict",code:"revision_conflict",message:"stale",status:409});
    await expect(runDraftRoutingWrite(captureDraftRoutingOwner(),"deleteRoute",{path:{route_id:"route"}},async()=>{})).rejects.toMatchObject({kind:"conflict"});
  });

  it("drops a late response after a replacement session without a receipt",async()=>{
    vi.mocked(callRevisioned).mockImplementation(async()=>{
      useSessionStore.getState().unlock(`mgmt_${"y".repeat(27)}`,undefined);
      return {value:undefined,revision:"rev-5"} as never;
    });
    await expect(runDraftRoutingWrite(captureDraftRoutingOwner(),"deleteRoute",{path:{route_id:"route"}},async()=>{})).rejects.toBeInstanceOf(CancelledError);
    expect(callRevisioned).toHaveBeenCalledTimes(1);
  });
});
