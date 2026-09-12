import { beforeEach, expect, it, vi } from "vitest";
vi.mock("../../api/client",()=>({call:vi.fn(),callRevisioned:vi.fn()}));
import { call, callRevisioned } from "../../api/client";
import { beginConfigurationTask } from "./configurationTask";
import { beginConfigurationEdit } from "./beginEdit";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";
import { useSessionStore } from "../../session/sessionStore";

const active:ConfigVersionSummary={id:"online",revision:"rev-7",status:"active",created_at_ms:1,description:"Online"};
const draft:ConfigVersionSummary={...active,id:"working",parent_id:active.id,status:"draft",revision:"rev-0"};
beforeEach(()=>{vi.resetAllMocks();useVersionStore.getState().reset();useVersionStore.getState().select(active);});

it("keeps the active page while editing a private revision, then validates and applies it",async()=>{
  let lists=0;
  vi.mocked(call).mockImplementation(async(operation)=>{
    if(operation==="listConfigVersions")return ++lists===1?[active]:[active,{...draft,revision:"rev-1"}];
    if(operation==="forkConfigVersion")return draft;
    if(operation==="listManagementAuditEvents")return [{id:12,action:"config_published"}];
    if(operation==="validateConfigVersion")return {valid:true};
    if(operation==="publishConfigVersion")return {active_config_version_id:draft.id};
    if(operation==="getConfigVersion")return {...draft,status:"active",revision:"rev-1"};
    return {};
  });
  vi.mocked(callRevisioned).mockResolvedValue({value:{id:"provider"},revision:"rev-1"});
  const task=await beginConfigurationTask("Add provider");
  await task.mutate("createUpstream",{body:{id:"provider"}});
  expect(callRevisioned).toHaveBeenCalledWith("createUpstream",expect.objectContaining({headers:{"X-Config-Version":"working","If-Match":"rev-0"}}));
  expect(useVersionStore.getState().context?.configVersionId).toBe("online");
  expect((await task.finish()).status).toBe("active");
  expect(call).toHaveBeenCalledWith("publishConfigVersion",expect.objectContaining({headers:{"If-Match":"rev-1","X-Expected-Active-Version":'"online"',"X-Expected-Lifecycle-Event":"12"}}));
  expect(useVersionStore.getState().context?.configVersionId).toBe("online");
});

it("does not silently publish other changes in an explicitly selected draft",async()=>{
  useVersionStore.getState().select(draft);
  vi.mocked(call).mockResolvedValue([active,draft]);
  const task=await beginConfigurationTask("Add account");
  expect(task.autoApply).toBe(false);
  expect((await task.finish()).status).toBe("draft");
  expect(call).toHaveBeenCalledTimes(1);
});

it("does not apply an action from a historical view to a different active configuration",async()=>{
  useVersionStore.getState().select({...active,status:"archived"});
  await expect(beginConfigurationTask("Edit old account")).rejects.toThrow("历史配置");
  expect(call).not.toHaveBeenCalled();
});

it("never retries a conflicting write or continues after the owner changes",async()=>{
  vi.mocked(call).mockImplementation(async(operation)=>operation==="listConfigVersions"?[active]:draft);
  const task=await beginConfigurationTask("Edit");
  vi.mocked(callRevisioned).mockRejectedValue(new Error("409"));
  await expect(task.mutate("createUpstream")).rejects.toThrow("409");
  expect(callRevisioned).toHaveBeenCalledTimes(1);
  useSessionStore.getState().lock();
  await expect(task.mutate("createUpstream")).rejects.toMatchObject({silent:true});
  expect(callRevisioned).toHaveBeenCalledTimes(1);
});

it("a late unscoped version list cannot create a fork after a selection change",async()=>{
  let resolve!:(value:ConfigVersionSummary[])=>void;
  vi.mocked(call).mockImplementationOnce(()=>new Promise((done)=>{resolve=done;}));
  const started=beginConfigurationEdit("Edit");
  useVersionStore.getState().select(draft);
  resolve([active,draft]);
  await expect(started).rejects.toMatchObject({silent:true});
  expect(call).toHaveBeenCalledTimes(1);
});
