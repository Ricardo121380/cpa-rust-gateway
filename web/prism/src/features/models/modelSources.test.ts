import { describe,expect,it } from "vitest";
import { modelSources } from "./modelSources";
import type { CandidateRecord,RouteListItem } from "./model";

describe("model source projection",()=>{
  it("retains all routes and disabled sources without mixing another model",()=>{
    const routes=[{id:"r1",public_model_id:"m1"},{id:"r2",public_model_id:"m1"},{id:"r3",public_model_id:"m2"}] as RouteListItem[];
    const candidates=[{id:"a",route_id:"r1",enabled:true},{id:"b",route_id:"r2",enabled:false},{id:"c",route_id:"r3",enabled:true}] as CandidateRecord[];
    expect(modelSources("m1",routes,candidates).map(row=>row.id)).toEqual(["a","b"]);
    expect(modelSources("missing",routes,candidates)).toEqual([]);
  });
});
