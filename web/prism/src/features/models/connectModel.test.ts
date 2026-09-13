import { describe, expect, it, vi } from "vitest";
import { connectModel, parseModelIds, type ConfigurationTask } from "./connectModel";

function task(existing = false, candidate: Record<string,unknown> | null = null) {
  const read = vi.fn(async (op:string)=>op==="listPublicModels"?(existing?[{id:"model-a",model_name:"Exact/Model-v2",status:"disabled"}]:[]):{revision:"rev-4",items:op==="listRoutes"?[{id:"route-a",public_model_id:"model-a"}]:candidate?[candidate]:[],next_cursor:null});
  const mutate=vi.fn(async()=>({}));
  return {reader:{read,mutate} as unknown as ConfigurationTask,read,mutate};
}
const input={upstreamModel:"Exact/Model-v2",endpointId:"endpoint-b",allowUnlisted:false};
describe("original model identities and multiple sources",()=>{
  it("uses the exact source name by default and does not manufacture a capability",async()=>{
    const t=task();await connectModel(t.reader,input);
    expect(t.mutate).toHaveBeenCalledWith("createPublicModel",expect.objectContaining({body:expect.objectContaining({model_name:input.upstreamModel,display_name:input.upstreamModel,capabilities:{}})}));
    expect(t.mutate).toHaveBeenCalledWith("createRouteCandidate",expect.objectContaining({body:expect.objectContaining({upstream_model:input.upstreamModel,transform_mode:"canonical_bridge"})}));
  });
  it("adds a second source to the existing model without changing status or grants",async()=>{
    const t=task(true,{route_id:"route-a",endpoint_id:"endpoint-a",upstream_model:input.upstreamModel});
    await connectModel(t.reader,input);
    expect(t.mutate).toHaveBeenCalledTimes(1);
    expect(t.mutate).toHaveBeenCalledWith("createRouteCandidate",expect.objectContaining({path:{route_id:"route-a"},body:expect.objectContaining({endpoint_id:"endpoint-b"})}));
  });
  it("keeps a duplicate connection's settings and disabled state",async()=>{
    const t=task(true,{route_id:"route-a",endpoint_id:"endpoint-b",upstream_model:input.upstreamModel,enabled:false,weight:7});
    expect((await connectModel(t.reader,input)).added).toBe(false);expect(t.mutate).not.toHaveBeenCalled();
  });
  it("does not merge a custom mapping into a different source model",async()=>{
    const t=task(true,{route_id:"route-a",endpoint_id:"endpoint-a",upstream_model:"Different-Model"});
    await expect(connectModel(t.reader,input)).rejects.toThrow("自定义模型映射");expect(t.mutate).not.toHaveBeenCalled();
  });
  it("keeps one canonical model and adds an optional alias",async()=>{
    const t=task();await connectModel(t.reader,{...input,alias:"my-optional-alias"});
    expect(t.mutate).toHaveBeenCalledWith("createPublicModel",expect.objectContaining({body:expect.objectContaining({model_name:input.upstreamModel})}));
    expect(t.mutate).toHaveBeenCalledWith("createRouteCandidate",expect.objectContaining({body:expect.objectContaining({upstream_model:input.upstreamModel,transform_mode:"canonical_bridge"})}));
    expect(t.mutate).toHaveBeenCalledWith("createModelAlias",expect.objectContaining({body:{alias:"my-optional-alias"}}));
  });
  it("keeps distinct versions and case while removing only exact repeated lines",()=>{
    expect(parseModelIds("Exact/Model-v2\nExact/Model-v3\nExact/Model-v2\nexact/Model-v2")).toEqual(["Exact/Model-v2","Exact/Model-v3","exact/Model-v2"]);
    expect(()=>parseModelIds(Array.from({length:21},(_,i)=>`model-${i}`).join("\n"))).toThrow();
  });
});
