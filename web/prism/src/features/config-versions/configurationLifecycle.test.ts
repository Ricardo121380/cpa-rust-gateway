import {beforeEach,expect,it,vi} from "vitest";
vi.mock("../../api/client",()=>({call:vi.fn()}));
import {call} from "../../api/client";
import {useVersionStore,type ConfigVersionSummary} from "./versionStore";
import {captureLifecycleOwner,prepareConfigurationLifecycle,commitConfigurationLifecycle,observeConfigurationLifecycle,latestLifecycleEvent} from "./configurationLifecycle";

const active:ConfigVersionSummary={id:"active",status:"active",revision:"rev-2",parent_id:null,created_at_ms:0,description:"Current"};
const draft:ConfigVersionSummary={...active,id:"draft",status:"draft",revision:"rev-7",parent_id:"active"};
const events=[{id:42,action:"config_published",config_version_id:"active",replaced_config_version_id:"prior"}];
beforeEach(()=>{
 vi.resetAllMocks();useVersionStore.getState().reset();useVersionStore.getState().select(draft);
 vi.mocked(call).mockImplementation(async operation=>{
  if(operation==="getConfigVersion")return draft as never;
  if(operation==="listConfigVersions")return [active,draft] as never;
  if(operation==="listManagementAuditEvents")return events as never;
  if(operation==="validateConfigVersion")return {valid:true} as never;
  return {active_config_version_id:"draft",replaced_config_version_id:"active"} as never;
 });
});
it("publishes only with the captured revision, active identity and lifecycle event",async()=>{
 const prepared=await prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish",{targetRevision:"rev-7",baseId:"active",baseRevision:"rev-2"});
 expect((await commitConfigurationLifecycle(prepared)).kind).toBe("acknowledged");
 expect(call).toHaveBeenLastCalledWith("publishConfigVersion",{path:{config_version_id:"draft"},headers:{"If-Match":"rev-7","X-Expected-Active-Version":'"active"',"X-Expected-Lifecycle-Event":"42"}});
});
it("rejects a changed source after validation without publishing",async()=>{
 const original=vi.mocked(call).getMockImplementation()!;let reads=0;
 vi.mocked(call).mockImplementation(async(operation,request,options)=>operation==="getConfigVersion"&&++reads===2?{...draft,revision:"rev-8"} as never:original(operation,request,options));
 await expect(prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish")).rejects.toThrow("修订已变化");
 expect(vi.mocked(call).mock.calls.some(([operation])=>operation==="publishConfigVersion")).toBe(false);
});
it("rejects stale net-change proof before validation",async()=>{
 await expect(prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish",{targetRevision:"rev-7",baseId:"active",baseRevision:"rev-1"})).rejects.toThrow("摘要已过期");
 expect(vi.mocked(call).mock.calls.some(([operation])=>operation==="validateConfigVersion")).toBe(false);
});
it("failed validation leaves no publication request",async()=>{
 const original=vi.mocked(call).getMockImplementation()!;
 vi.mocked(call).mockImplementation(async(operation,request,options)=>operation==="validateConfigVersion"?{valid:false,error_codes:["route_missing"]} as never:original(operation,request,options));
 await expect(prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish")).rejects.toThrow("route_missing");
});
it("keeps a lost response non-replayable and does not silently reconcile",async()=>{
 const prepared=await prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish");vi.mocked(call).mockRejectedValue(new Error("response lost"));
 const before=vi.mocked(call).mock.calls.length;
 expect((await commitConfigurationLifecycle(prepared)).kind).toBe("unconfirmed");
 expect(vi.mocked(call).mock.calls.length-before).toBe(1);
});
it("acknowledgement survives a subsequent failed observation",async()=>{
 const prepared=await prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish");const receipt=await commitConfigurationLifecycle(prepared);
 vi.mocked(call).mockRejectedValue(new Error("read unavailable"));
 await expect(observeConfigurationLifecycle(receipt)).rejects.toThrow("read unavailable");
 expect(receipt.kind).toBe("acknowledged");
});
it("does not surface a response or run recovery under a replacement owner",async()=>{
 const prepared=await prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish");
 vi.mocked(call).mockImplementation(async()=>{useVersionStore.getState().select(active);return {active_config_version_id:"draft",replaced_config_version_id:"active"} as never;});
 await expect(commitConfigurationLifecycle(prepared)).rejects.toMatchObject({silent:true});
});
it("preserves string lifecycle identity and rejects lossy numeric identity",()=>{
 expect(latestLifecycleEvent([{...events[0]!,id:"9007199254740993"}])).toBe("9007199254740993");
 expect(()=>latestLifecycleEvent([{...events[0]!,id:9007199254740992}])).toThrow("无法精确读取");
});
it("rollback follows durable lifecycle history rather than the first archived row",async()=>{
 const prior={...active,id:"prior",status:"archived" as const};const unrelated={...prior,id:"unrelated"};
 useVersionStore.getState().select(active);
 vi.mocked(call).mockImplementation(async(operation,request)=>{
  if(operation==="getConfigVersion")return (request?.path?.["config_version_id"]==="prior"?prior:active) as never;
  if(operation==="listConfigVersions")return [unrelated,active,prior] as never;
  if(operation==="listManagementAuditEvents")return events as never;
  if(operation==="validateConfigVersion")return {valid:true} as never;
  return {active_config_version_id:"prior",replaced_config_version_id:"active"} as never;
 });
 const prepared=await prepareConfigurationLifecycle(captureLifecycleOwner(active),"rollback");
 expect(prepared.target.id).toBe("prior");expect((await commitConfigurationLifecycle(prepared)).kind).toBe("acknowledged");
});

it("read-only validation cannot be reused as application permission",async()=>{
 const prepared=await prepareConfigurationLifecycle(captureLifecycleOwner(draft),"publish",undefined,true);
 await expect(commitConfigurationLifecycle(prepared)).rejects.toThrow("只读校验");
 expect(vi.mocked(call).mock.calls.some(([operation])=>operation==="publishConfigVersion")).toBe(false);
});
