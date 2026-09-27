import {describe,it,expect} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {AccountQuotaEvidence} from "./AccountQuotaEvidence";
describe("account quota states",()=>{
 it("never turns missing or failed quota into zero",()=>{
  for(const error of [undefined,"unavailable","not_implemented_or_not_connected"]){
   const html=renderToStaticMarkup(<AccountQuotaEvidence loading={false} error={error} onRefresh={()=>{}}/>);
   expect(html).not.toContain("已使用 0%");
  }
 });
 it("uses the returned duration rather than assuming a 5h window",()=>{
  const html=renderToStaticMarkup(<AccountQuotaEvidence loading={false} onRefresh={()=>{}} observation={{source:"codex",observed_at_ms:100,windows:[{name:"rate_limit.primary_window",used_percent:0,reset_at:null,reset_at_ms:null,duration_seconds:3600}]}}/>);
  expect(html).toContain("已使用 0%");expect(html).toContain("1 小时窗口");expect(html).not.toContain("5 小时");
 });
});
