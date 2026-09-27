import {describe,it,expect} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {KimiQuotaEvidence} from "./KimiAccountEvidence";
describe("Kimi official quota evidence",()=>{
 it("keeps missing observations distinct from zero usage",()=>{
  const empty=renderToStaticMarkup(<KimiQuotaEvidence loading={false} onRefresh={()=>{}}/>);
  expect(empty).toContain("暂未取得");expect(empty).not.toContain("0%");
  const zero=renderToStaticMarkup(<KimiQuotaEvidence loading={false} onRefresh={()=>{}} observation={{observed_at_ms:100,profile_available:true,quota_available:true,identity:{email:null,phone:null,username:null},plan:null,quota_windows:[{window:"limit_5h",used_ratio:0,reset_at:null}]}}/>);
  expect(zero).toContain("已使用 0%");expect(zero).toContain("不是 Token 余额");
 });
 it("exposes a missing binding instead of an empty successful quota",()=>{
  const html=renderToStaticMarkup(<KimiQuotaEvidence loading={false} error="not_connected" onRefresh={()=>{}}/>);
  expect(html).toContain('role="alert"');expect(html).toContain("尚未连接");
 });
});
