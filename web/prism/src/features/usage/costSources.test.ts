import { describe, expect, it } from "vitest";
import { costSources, supportsCostFilters } from "./costSources";
import type { LedgerRow } from "../monitoring/model";
const row=(model:string,cost:number|null,confidence:LedgerRow["cost_confidence"])=>({model,cost_microunits:cost,cost_confidence:confidence}) as LedgerRow;
describe("ledger-backed cost sources",()=>{
 it("retains exact IDs and combines multiple sources",()=>{
  expect(costSources([row("Exact/M",3,"exact"),row("Exact/M",4,"partial"),row("exact/m",2,"exact")])).toEqual([
   {model:"Exact/M",records:2,known:7,incomplete:1},{model:"exact/m",records:1,known:2,incomplete:0}]);
 });
 it("preserves unpriced as unknown and distinguishes observed zero",()=>{
  expect(costSources([row("unknown",null,"unpriced"),row("zero",0,"exact")])).toEqual([
   {model:"zero",records:1,known:0,incomplete:0},{model:"unknown",records:1,known:null,incomplete:1}]);
 });
 it("does not turn missing contributors into full confidence",()=>{
  expect(costSources([row("M",5,"exact"),row("M",null,"unknown")])[0]).toEqual({model:"M",records:2,known:5,incomplete:1});
 });
 it("blocks unsupported filter dimensions instead of showing unfiltered costs",()=>{
  expect(supportsCostFilters({provider_id:"p",account_id:"a",model:"M"})).toBe(true);
  for(const key of ["client_key_id","access_group_id","protocol"])expect(supportsCostFilters({[key]:"x"})).toBe(false);
 });
});
