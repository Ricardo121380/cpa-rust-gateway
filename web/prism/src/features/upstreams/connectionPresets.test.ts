import officialSource from "../../../../../crates/provider-grok/src/official.rs?raw";
import responsesSource from "../../../../../crates/provider-grok/src/official_responses.rs?raw";
import {describe,expect,it} from "vitest";
import {CONNECTION_PRESETS,providerAddress} from "./connectionPresets";

function productionConstant(source:string,name:string){
  const value=source.match(new RegExp(`pub const ${name}: &str = "([^"]+)";`))?.[1];
  if(!value)throw new Error(`Missing production constant ${name}`);
  return value;
}

describe("Grok Official production configuration",()=>{
  it("uses the exact components required by runtime preparation",()=>{
    const preset=CONNECTION_PRESETS.find(row=>row.id==="grok-api")!;
    expect(preset.base).toBe(productionConstant(officialSource,"GROK_OFFICIAL_API_BASE_URL"));
    expect(preset.path).toBe(productionConstant(responsesSource,"GROK_OFFICIAL_RESPONSES_PATH"));
    expect(providerAddress(preset.base,preset.path).base+preset.path).toBe(productionConstant(responsesSource,"GROK_OFFICIAL_RESPONSES_URL"));
    expect(preset.fixed).toBe(true);
  });
});
