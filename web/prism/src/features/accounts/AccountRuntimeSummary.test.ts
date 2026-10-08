import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {MemoryRouter} from "react-router-dom";
import {useVersionStore} from "../config-versions/versionStore";
import {beforeEach,describe,expect,it,vi} from "vitest";
vi.mock("../../api/client",()=>({call:vi.fn()}));
import {call} from "../../api/client";
import {AccountMainStatus,AccountRuntimeSummary,type useAccountRuntimeSummary,readAccountRuntimeSummary} from "./AccountRuntimeSummary";
vi.mock("./callEvidence",async importOriginal=>({...await importOriginal<typeof import("./callEvidence")>(),useAccountCallEvidence:()=>({receipt:undefined,current:{data:undefined,isFetching:false,isError:false}})}));
vi.mock("../config-versions/versionStore",async importOriginal=>{const actual=await importOriginal<typeof import("../config-versions/versionStore")>();return {...actual,useVersionStore:Object.assign((select:(state:ReturnType<typeof actual.useVersionStore.getState>)=>unknown)=>select(actual.useVersionStore.getState()),actual.useVersionStore)};});
const read=vi.mocked(call);
beforeEach(()=>read.mockReset());
describe("account list runtime snapshot",()=>{
 it("consumes all pages before reporting absence",async()=>{
  read.mockResolvedValueOnce({snapshot_id:"s",observed_at_ms:1,items:[],next_cursor:"next"}).mockResolvedValueOnce({snapshot_id:"s",observed_at_ms:1,items:[{account_id:"later"}],next_cursor:null});
  expect((await readAccountRuntimeSummary()).rows).toEqual([{account_id:"later"}]);
  expect(read.mock.calls[1]?.[1]?.query).toEqual({limit:100,cursor:"next"});
 });
 it("rejects mixed snapshots",async()=>{
  read.mockResolvedValueOnce({snapshot_id:"s",items:[],next_cursor:"next"}).mockResolvedValueOnce({snapshot_id:"changed",items:[],next_cursor:null});
  await expect(readAccountRuntimeSummary()).rejects.toThrow("快照已变化");
 });
 it("bounds enumeration without treating truncation as absence",async()=>{
  read.mockResolvedValue({snapshot_id:"s",items:[],next_cursor:"more"});
  await expect(readAccountRuntimeSummary()).rejects.toThrow("超出本次范围");expect(read).toHaveBeenCalledTimes(100);
 });
});

describe("authorization and runtime lineage in actual account components",()=>{
 function snapshot(revision?:number,scope="active-config"){
  return {isError:true,isFetching:false,isPending:false,data:{observedAt:1000,snapshotId:"old-snapshot",rows:[{account_id:"account",provider_id:"owner",channel_id:"channel",account_kind:"bearer",credential_revision:revision,config_version_id:scope,auth_status:"expired",runtime_status:"expired",enabled:true}]}} as unknown as ReturnType<typeof useAccountRuntimeSummary>;
 }
 function main(value:ReturnType<typeof useAccountRuntimeSummary>){
  useVersionStore.getState().select({id:"active-config",status:"active",revision:"rev-3",created_at_ms:1,description:""});
  return renderToStaticMarkup(createElement(AccountMainStatus,{snapshot:value,id:"account",providerId:"owner",credentialRevision:2,enabled:true,auth:"active",authentication:"api_key",connections:1}));
 }
 it("keeps new authorization across retained stale rows and retains observation scope",()=>{
  const value=snapshot(1);const html=main(value);
  expect(html).toContain("当前授权：active · revision 2");expect(html).toContain("运行观测读取失败");expect(html).not.toContain("更新或替换 API Key");
  const old=renderToStaticMarkup(createElement(MemoryRouter,null,createElement(AccountRuntimeSummary,{snapshot:value,ids:["account"],providerId:"owner"})));
  expect(old).toContain("旧观察");expect(old).toContain("old-snapshot");expect(old).toContain("revision 1");expect(old).toContain("owner/channel");
 });
 it("keeps a real auth failure for the current revision even when the refetch fails",()=>{expect(main(snapshot(2))).toContain("更新或替换 API Key");});
 it("does not borrow unobserved or another configuration's authorization lineage",()=>{
  expect(main(snapshot())).not.toContain("更新或替换 API Key");expect(main(snapshot(2,"other-config"))).not.toContain("更新或替换 API Key");
 });
});
