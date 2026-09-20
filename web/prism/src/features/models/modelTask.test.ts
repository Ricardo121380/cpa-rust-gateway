import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api/client",()=>({call:vi.fn()}));
vi.mock("../config-versions/configurationTask",()=>({beginConfigurationTask:vi.fn()}));

import { call } from "../../api/client";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { runModelTask, sameCandidate } from "./modelTask";
import type { CandidateRecord } from "./model";
import { useVersionStore } from "../config-versions/versionStore";

const version={id:"draft",revision:"rev-4",status:"draft" as const,created_at_ms:1,description:""};
const candidate:CandidateRecord={id:"candidate",route_id:"route",endpoint_id:"endpoint",upstream_model:"Exact/Model-v2",credential_scope:"all_active",transform_mode:"canonical_bridge",enabled:true,priority:0,weight:1,capability_override:{streaming:true,tools:true}};

describe("model write receipts",()=>{
  beforeEach(()=>vi.resetAllMocks());

  it("compares exact candidate fields without relying on object key order",()=>{
    expect(sameCandidate({...candidate,capability_override:{tools:true,streaming:true}},candidate)).toBe(true);
    expect(sameCandidate({...candidate,upstream_model:"exact/model-v2"},candidate)).toBe(false);
    expect(sameCandidate({...candidate,capability_override:{tools:false,streaming:true}},candidate)).toBe(false);
  });

  it("reports an earlier saved stage when a later write is rejected",async()=>{
    let revision="rev-4";
    const conflict={kind:"conflict",code:"revision_conflict",message:"changed",status:409};
    const mutate=vi.fn().mockImplementationOnce(async()=>{revision="rev-5";return {};}).mockRejectedValueOnce(conflict);
    const task={version,autoApply:true,assertOwner:vi.fn(),revision:()=>revision,mutate,finish:vi.fn()};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockResolvedValue({...version,revision:"rev-5"} as never);
    const result=await runModelTask("connect",async(owned)=>{
      await owned.mutate("createPublicModel");
      await owned.mutate("createRoute");
    });
    expect(result.receipt).toMatchObject({kind:"saved_partial",acknowledgedWrites:1,workingVersion:{revision:"rev-5"}});
    expect(mutate).toHaveBeenCalledTimes(2);
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("does not infer failure or replay after a lost first write response",async()=>{
    const mutate=vi.fn().mockRejectedValue(new Error("response lost"));
    const task={version,autoApply:true,assertOwner:vi.fn(),revision:()=>"rev-4",mutate,finish:vi.fn()};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockResolvedValue(version as never);
    const result=await runModelTask("connect",async(owned)=>{await owned.mutate("createPublicModel");});
    expect(result.receipt.kind).toBe("unconfirmed");
    expect(mutate).toHaveBeenCalledTimes(1);
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("keeps a pre-write rejection editable",async()=>{
    const task={version,autoApply:true,assertOwner:vi.fn(),revision:()=>"rev-4",mutate:vi.fn(),finish:vi.fn()};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    await expect(runModelTask("connect",async()=>{throw new Error("selection changed");})).rejects.toThrow("selection changed");
    expect(task.mutate).not.toHaveBeenCalled();
  });

  it("does not publish an unchanged connection",async()=>{
    const task={version,autoApply:true,assertOwner:vi.fn(),revision:()=>"rev-4",mutate:vi.fn(),finish:vi.fn()};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    const result=await runModelTask("connect",async()=>({added:false}));
    expect(result.receipt).toMatchObject({kind:"saved_unapplied",acknowledgedWrites:0});
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("recognizes an active no-op before creating a working draft",async()=>{
    const active={...version,id:"active",revision:"rev-8",status:"active" as const};
    useVersionStore.getState().reset();
    useVersionStore.getState().select(active);
    vi.mocked(call).mockResolvedValue(active as never);
    const result=await runModelTask("connect",async()=>({added:false}),{probeUnchanged:true});
    expect(result.receipt).toMatchObject({kind:"unchanged",workingVersion:active});
    expect(beginConfigurationTask).not.toHaveBeenCalled();
    expect(vi.mocked(call).mock.calls.map(([operation])=>operation)).toEqual(["getConfigVersion","getConfigVersion"]);
  });

  it("refuses a stale captured source even when a newer no-op would look unchanged",async()=>{
    useVersionStore.getState().reset();
    useVersionStore.getState().select(version);
    await expect(runModelTask("edit",async()=>({unchanged:true}),{probeUnchanged:true,expectedSource:{id:"draft",revision:"rev-3"}})).rejects.toThrow("当前配置已变化");
    expect(beginConfigurationTask).not.toHaveBeenCalled();
    expect(call).not.toHaveBeenCalled();
  });
});
