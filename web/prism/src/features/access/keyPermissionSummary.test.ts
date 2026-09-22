import {describe,it,expect} from "vitest";
import {summarizePermissions} from "./keyPermissionSummary";
import type {RouteListItem,PublicModel} from "../models/model";
const routes=[{id:"r1",public_model_id:"m1"},{id:"r2",public_model_id:"m1"},{id:"r3",public_model_id:"m2"}] as RouteListItem[];
const models=[{id:"m1",model_name:"Exact/ID",status:"disabled"},{id:"m2",model_name:"other",status:"active"}] as PublicModel[];
describe("key permission summary",()=>{
 it("deduplicates routes without including disabled grants or disguising closed models",()=>{
  expect(summarizePermissions(routes,models,[{route_id:"r1",enabled:true},{route_id:"r2",enabled:true},{route_id:"r3",enabled:false}])).toEqual([{id:"m1",name:"Exact/ID",enabled:false}]);
 });
 it("does not turn incomplete evidence into zero permissions",()=>{
  expect(()=>summarizePermissions(routes,models,[{route_id:"missing",enabled:true}])).toThrow();
  expect(()=>summarizePermissions(routes,[],[{route_id:"r1",enabled:true}])).toThrow();
  expect(summarizePermissions(routes,models,[])).toEqual([]);
 });
});
