import {readFileSync} from "node:fs";
import {describe,expect,it} from "vitest";
import {CONNECTION_PRESETS,providerAddress} from "./connectionPresets";

function productionConstant(file:string,name:string){
  const source=readFileSync(new URL(`../../../../../crates/provider-grok/src/${file}`,import.meta.url),"utf8");
  const value=source.match(new RegExp(`pub const ${name}: &str = "([^"]+)";`))?.[1];
  if(!value)throw new Error(`Missing production constant ${name}`);
  return value;
}

describe("Grok Official production configuration",()=>{
  it("uses the exact components required by runtime preparation",()=>{
    const preset=CONNECTION_PRESETS.find(row=>row.id==="grok-api")!;
    expect(preset.base).toBe(productionConstant("official.rs","GROK_OFFICIAL_API_BASE_URL"));
    expect(preset.path).toBe(productionConstant("official_responses.rs","GROK_OFFICIAL_RESPONSES_PATH"));
    expect(providerAddress(preset.base,preset.path).base+preset.path).toBe(productionConstant("official_responses.rs","GROK_OFFICIAL_RESPONSES_URL"));
    expect(preset.fixed).toBe(true);
  });
});
