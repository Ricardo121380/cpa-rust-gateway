import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api/client",()=>({call:vi.fn(),callRevisioned:vi.fn()}));
vi.mock("../config-versions/configurationTask",async importOriginal=>({...await importOriginal<typeof import("../config-versions/configurationTask")>(),beginConfigurationTask:vi.fn()}));

import { call, callRevisioned } from "../../api/client";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { useVersionStore } from "../config-versions/versionStore";
import { captureBillingOwner, runBillingCatalogTask, runBillingPolicyTask } from "./billingTask";
import type { Catalog } from "./model";

const draft={id:"draft",revision:"rev-4",status:"draft" as const,created_at_ms:1,description:""};
const catalog:Catalog={catalog_version_id:"cat-next",effective_at_ms:1000,created_at_ms:2,source:"operator",entries:[{provider_id:"provider",channel_id:"endpoint",model:"Exact/Model",input_microunits_per_million:0,output_microunits_per_million:5,reasoning_microunits_per_million:0,cache_read_microunits_per_million:0,cache_creation_microunits_per_million:0,cached_microunits_per_million:0}]};
const imported={catalog_version_id:catalog.catalog_version_id,effective_at_ms:catalog.effective_at_ms,source:"operator",entry_count:1,operation:"imported" as const,rolled_back_from:null};

describe("billing task receipts",()=>{
  beforeEach(()=>{
    vi.resetAllMocks();
    useVersionStore.getState().reset();
    useVersionStore.getState().select(draft);
    vi.mocked(call).mockImplementation(async(operation)=>operation==="getConfigVersion"?draft:[] as never);
  });

  it("rejects duplicate global IDs before any task fork or write",async()=>{
    vi.mocked(call).mockImplementation(async(operation)=>operation==="getConfigVersion"?draft:[catalog] as never);
    await expect(runBillingCatalogTask(captureBillingOwner(),{kind:"import",target:catalog.catalog_version_id,effectiveAt:catalog.effective_at_ms,source:catalog.source,entries:catalog.entries})).rejects.toThrow("已存在");
    expect(beginConfigurationTask).not.toHaveBeenCalled();
  });

  it("reports global persistence in a draft without claiming publication",async()=>{
    const task={version:draft,autoApply:false,assertOwner:vi.fn(),revision:()=>"rev-5",mutate:vi.fn().mockResolvedValue(imported),finish:vi.fn()};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    const receipt=await runBillingCatalogTask(captureBillingOwner(),{kind:"import",target:catalog.catalog_version_id,effectiveAt:catalog.effective_at_ms,source:catalog.source,entries:catalog.entries});
    expect(receipt).toMatchObject({kind:"catalog_saved_draft",catalog:imported,workingVersion:{revision:"rev-5"}});
    expect(receipt.message).toContain("尚未发布");
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("defers active-context publication after global catalog persistence",async()=>{
    const active={...draft,id:"active",revision:"rev-8",status:"active" as const};
    const working={...draft,id:"work",parent_id:active.id,revision:"rev-1"};
    useVersionStore.getState().select(active);
    vi.mocked(call).mockImplementation(async(operation,request)=>operation==="getConfigVersion"?(request?.path?.["config_version_id"]==="work"?working:active):[] as never);
    const task={version:{...working,revision:"rev-0"},autoApply:true,assertOwner:vi.fn(),revision:()=>"rev-1",mutate:vi.fn().mockResolvedValue(imported),finish:vi.fn().mockRejectedValue(new Error("publication failed"))};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    const receipt=await runBillingCatalogTask(captureBillingOwner(),{kind:"import",target:catalog.catalog_version_id,effectiveAt:catalog.effective_at_ms,source:catalog.source,entries:catalog.entries});
    expect(receipt).toMatchObject({kind:"catalog_saved_draft",catalog:imported,workingVersion:{id:"work"}});
    expect(receipt.message).toContain("目录已保存");
    expect(task.mutate).toHaveBeenCalledTimes(1);
    expect(task.finish).not.toHaveBeenCalled();
    expect(beginConfigurationTask).toHaveBeenCalledWith("导入价格目录",{id:"active",revision:"rev-8"},"deferred");
  });

  it("does not replay a lost response even when a matching catalog is later visible",async()=>{
    let reads=0;
    vi.mocked(call).mockImplementation(async(operation)=>{
      if(operation==="getConfigVersion")return draft;
      if(operation==="listBillingCatalogs")return ++reads===1?[]:[catalog];
      return [] as never;
    });
    const task={version:draft,autoApply:false,assertOwner:vi.fn(),revision:()=>"rev-4",mutate:vi.fn().mockRejectedValue(new Error("response lost")),finish:vi.fn()};
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    const receipt=await runBillingCatalogTask(captureBillingOwner(),{kind:"import",target:catalog.catalog_version_id,effectiveAt:catalog.effective_at_ms,source:catalog.source,entries:catalog.entries});
    expect(receipt).toMatchObject({kind:"unconfirmed",observedCatalog:true});
    expect(task.mutate).toHaveBeenCalledTimes(1);
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("policy binding verifies the selected draft and never chooses a catalog implicitly",async()=>{
    vi.mocked(call).mockImplementation(async(operation)=>{
      if(operation==="getConfigVersion")return draft;
      if(operation==="getRoutingPricePolicy")throw {status:404,code:"management_resource_not_found"};
      return [catalog] as never;
    });
    vi.mocked(callRevisioned).mockResolvedValue({revision:"rev-5",value:{catalog_version_id:catalog.catalog_version_id,comparison:"rate_dominance_v1"}} as never);
    const receipt=await runBillingPolicyTask(captureBillingOwner(),null,{kind:"bind",catalogId:catalog.catalog_version_id});
    expect(receipt.kind).toBe("policy_saved_draft");
    expect(callRevisioned).toHaveBeenCalledTimes(1);
    expect(beginConfigurationTask).not.toHaveBeenCalled();
  });

  it("refuses a stale draft revision before policy mutation",async()=>{
    const owner=captureBillingOwner();
    vi.mocked(call).mockResolvedValue({...draft,revision:"rev-5"} as never);
    await expect(runBillingPolicyTask(owner,null,{kind:"bind",catalogId:catalog.catalog_version_id})).rejects.toThrow("配置已变化");
    expect(callRevisioned).not.toHaveBeenCalled();
  });

  it("does not treat an unavailable policy read as an empty policy",async()=>{
    vi.mocked(call).mockImplementation(async(operation)=>{
      if(operation==="getConfigVersion")return draft;
      throw {status:503,code:"management_unavailable",message:"unavailable"};
    });
    await expect(runBillingPolicyTask(captureBillingOwner(),null,{kind:"clear"})).rejects.toBeDefined();
    expect(callRevisioned).not.toHaveBeenCalled();
  });

  it("keeps a lost policy response non-replayable",async()=>{
    vi.mocked(call).mockImplementation(async(operation)=>operation==="getConfigVersion"?draft:{catalog_version_id:catalog.catalog_version_id,comparison:"rate_dominance_v1"} as never);
    vi.mocked(callRevisioned).mockRejectedValue(new Error("response lost"));
    const receipt=await runBillingPolicyTask(captureBillingOwner(),{catalog_version_id:catalog.catalog_version_id,comparison:"rate_dominance_v1"},{kind:"clear"});
    expect(receipt.kind).toBe("unconfirmed");
    expect(callRevisioned).toHaveBeenCalledTimes(1);
  });

  it("rejects a duplicate restore ID before creating another catalog",async()=>{
    vi.mocked(call).mockImplementation(async(operation)=>operation==="getConfigVersion"?draft:[catalog] as never);
    await expect(runBillingCatalogTask(captureBillingOwner(),{kind:"restore",target:catalog.catalog_version_id,effectiveAt:2000,predecessor:catalog})).rejects.toThrow("已存在");
    expect(beginConfigurationTask).not.toHaveBeenCalled();
  });
});

describe("pending batch admission",()=>{
  beforeEach(()=>{vi.resetAllMocks();useVersionStore.getState().reset();});
  for(const action of [{kind:"bind",catalogId:"cat-next"},{kind:"clear"}] as const){
    it(`rejects ${action.kind} into a different draft before a write`,async()=>{
      useVersionStore.getState().rememberPending({...draft,id:"pending-first"});
      useVersionStore.getState().select({...draft,id:"other-draft"});
      await expect(runBillingPolicyTask(captureBillingOwner(),{catalog_version_id:"cat-old",comparison:"rate_dominance_v1"},action)).rejects.toThrow("已有待应用配置");
      expect(callRevisioned).not.toHaveBeenCalled();
      expect(beginConfigurationTask).not.toHaveBeenCalled();
      expect(useVersionStore.getState().pending?.id).toBe("pending-first");
    });
  }
});
