import { describe,it,expect } from "vitest";
import {priceDrafts,parsePriceDrafts,serializePriceDrafts,applyPriceQuote} from "./PriceEntriesEditor";
import {RATE_FIELDS} from "./model";
describe("price edit evidence",()=>{
 it("keeps empty rates null and explicit zero valid",()=>{const row=priceDrafts("")[0]!;row.input_microunits_per_million="0";const saved=JSON.parse(serializePriceDrafts([row]))[0];expect(saved.input_microunits_per_million).toBe(0);expect(saved.output_microunits_per_million).toBeNull();});
 it("does not fill missing rates or flatten tiers or rename models",()=>{const row=priceDrafts('[{"model":"Exact","input_microunits_per_million":9}]')[0]!;const quote={provider:"source",model:"Exact",tiered:false,...Object.fromEntries(RATE_FIELDS.map(f=>[f,null]))} as Parameters<typeof applyPriceQuote>[1];quote.output_microunits_per_million=2;expect(applyPriceQuote(row,quote).input_microunits_per_million).toBe("9");expect(applyPriceQuote(row,quote).output_microunits_per_million).toBe("2");expect(applyPriceQuote(row,{...quote,tiered:true})).toBe(row);expect(applyPriceQuote(row,{...quote,model:"exact"})).toBe(row);});
 it("rejects overlarge or malformed JSON rather than truncating or blanking",()=>{
  expect(parsePriceDrafts("[{" ).ok).toBe(false);
  expect(parsePriceDrafts(JSON.stringify(Array.from({length:513},(_,index)=>({model:`m-${index}`})))).ok).toBe(false);
  expect(parsePriceDrafts('[{"model":"Exact","ignored":true}]').ok).toBe(false);
 });
 it("preserves invalid table input as invalid JSON values for preview validation",()=>{
  const row=priceDrafts("")[0]!;
  row.input_microunits_per_million="1.5";
  const saved=JSON.parse(serializePriceDrafts([row]))[0];
  expect(saved.input_microunits_per_million).toBe("1.5");
  row.input_microunits_per_million=String(Number.MAX_SAFE_INTEGER+1);
  expect(JSON.parse(serializePriceDrafts([row]))[0].input_microunits_per_million).toBe(String(Number.MAX_SAFE_INTEGER+1));
 });
});
