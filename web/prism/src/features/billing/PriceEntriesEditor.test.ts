import { describe,it,expect } from "vitest";
import {priceDrafts,serializePriceDrafts,applyPriceQuote} from "./PriceEntriesEditor";
import {RATE_FIELDS} from "./model";
describe("price edit evidence",()=>{
 it("keeps empty rates null and explicit zero valid",()=>{const row=priceDrafts("")[0]!;row.input_microunits_per_million="0";const saved=JSON.parse(serializePriceDrafts([row]))[0];expect(saved.input_microunits_per_million).toBe(0);expect(saved.output_microunits_per_million).toBeNull();});
 it("does not fill missing rates or flatten tiers or rename models",()=>{const row=priceDrafts('[{"model":"Exact","input_microunits_per_million":9}]')[0]!;const quote={provider:"source",model:"Exact",tiered:false,...Object.fromEntries(RATE_FIELDS.map(f=>[f,null]))} as Parameters<typeof applyPriceQuote>[1];quote.output_microunits_per_million=2;expect(applyPriceQuote(row,quote).input_microunits_per_million).toBe("9");expect(applyPriceQuote(row,quote).output_microunits_per_million).toBe("2");expect(applyPriceQuote(row,{...quote,tiered:true})).toBe(row);expect(applyPriceQuote(row,{...quote,model:"exact"})).toBe(row);});
});
