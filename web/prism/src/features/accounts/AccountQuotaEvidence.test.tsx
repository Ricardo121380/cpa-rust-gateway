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
 it("shows partial Kiro credits without claiming a token balance",()=>{
  const html=renderToStaticMarkup(<AccountQuotaEvidence loading={false} onRefresh={()=>{}} observation={{source:"kiro",observed_at_ms:100,partial:true,windows:[{name:"kiro.resource.0",used_percent:12.5,used:12.5,limit:100,unit:"CREDITS",reset_at:null,reset_at_ms:null,duration_seconds:null}]}}/>);
  expect(html).toContain("部分额度");expect(html).toContain("CREDITS");expect(html).toContain("基础额度");expect(html).not.toContain("kiro.resource.0");
 });
 it("does not turn a zero native limit into zero percent",()=>{
  const html=renderToStaticMarkup(<AccountQuotaEvidence loading={false} onRefresh={()=>{}} observation={{source:"grok_console",observed_at_ms:100,windows:[{name:"console.chat",used_percent:null,used:0,limit:0,unit:"requests",reset_at:null,reset_at_ms:null,duration_seconds:null}]}}/>);
  expect(html).toContain("比例未知");expect(html).toContain("对话额度");expect(html).not.toContain("已使用 0%");
 });
 it("uses the returned duration rather than assuming a 5h window",()=>{
  const html=renderToStaticMarkup(<AccountQuotaEvidence loading={false} onRefresh={()=>{}} observation={{source:"codex",observed_at_ms:100,windows:[{name:"rate_limit.primary_window",used_percent:0,reset_at:null,reset_at_ms:null,duration_seconds:3600}]}}/>);
  expect(html).toContain("已使用 0%");expect(html).toContain("1 小时窗口");expect(html).not.toContain("5 小时");
 });
});
