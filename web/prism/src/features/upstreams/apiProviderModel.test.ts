import {describe,it,expect} from "vitest";
import {matchingApiConnections,chooseApiConnection} from "./apiProviderModel";
const endpoint={id:"e",upstream_id:"u",adapter_id:"openai-compatible.responses",api_format:"openai/responses",base_url:"https://api.test/v1/",inference_path:"/responses",models_path:null,transport:"http",enabled:false};
const providers=[{id:"u",name:"Kept",kind:"openai-compatible",enabled:false}];
const input={kind:"openai-compatible",adapter:"openai-compatible.responses",format:"openai/responses",base:"https://api.test/v1",path:"/responses"};
describe("API connection selection",()=>{
 it("reuses an exact disabled connection without enabling it",()=>{const matches=matchingApiConnections(providers,[endpoint],input);expect(chooseApiConnection(matches)).toEqual({provider:providers[0]!,endpoint});});
 it("creates only with zero matches and requires a choice with multiple",()=>{expect(chooseApiConnection([])).toBeUndefined();const matches=matchingApiConnections(providers,[endpoint,{...endpoint,id:"e2"}],input);expect(()=>chooseApiConnection(matches)).toThrow();expect(chooseApiConnection(matches,"e2")?.endpoint.id).toBe("e2");});
 it("never merges owners or protocols by display name",()=>{expect(matchingApiConnections([{...providers[0]!,kind:"kimi"}],[endpoint],input)).toEqual([]);expect(matchingApiConnections(providers,[{...endpoint,api_format:"openai/chat-completions"}],input)).toEqual([]);});
});
