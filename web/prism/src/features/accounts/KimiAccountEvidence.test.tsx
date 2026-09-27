import {describe,it,expect} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {KimiQuotaEvidence, kimiQuotaFailure, kimiMetadataError} from "./KimiAccountEvidence";
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

describe("Kimi quota failure diagnostics",()=>{
 it.each([401,403,429,503])("shows HTTP %s without inventing a balance",status=>{
  const html=renderToStaticMarkup(<KimiQuotaEvidence loading={false} onRefresh={()=>{}} observation={{observed_at_ms:100,profile_available:true,quota_available:false,identity:{email:null,phone:null,username:null},plan:null,quota_windows:[],quota_error:{code:"http",status}}}/>);
  expect(html).toContain(`HTTP ${status}`);expect(html).toContain('role="alert"');expect(html).not.toContain("0%");
 });
 it("shows an upstream empty result without a fake zero balance",()=>{
  const html=renderToStaticMarkup(<KimiQuotaEvidence loading={false} onRefresh={()=>{}} observation={{observed_at_ms:100,profile_available:true,quota_available:false,identity:{email:null,phone:null,username:null},plan:null,quota_windows:[],quota_error:{code:"empty_response"}}}/>);
  expect(html).toContain("上游暂未提供额度数据");
  expect(html).toContain('role="status"');expect(html).not.toContain('role="alert"');
  expect(html).not.toContain("已使用 0%");
 });
 it("distinguishes transport, timeout, and unrecognized payloads",()=>{
  expect(kimiQuotaFailure({code:"timeout"})).toContain("超时");
  expect(kimiQuotaFailure({code:"transport"})).toContain("连接");
  expect(kimiQuotaFailure({code:"invalid_json"})).toContain("无法解析");
  expect(kimiQuotaFailure({code:"unrecognized_response"})).toContain("没有可识别");
  expect(kimiMetadataError("busy")).toContain("续期");
 });
 it("keeps a new request error visible over an earlier empty observation",()=>{
  const html=renderToStaticMarkup(<KimiQuotaEvidence loading={false} error="timeout" onRefresh={()=>{}} observation={{observed_at_ms:100,profile_available:true,quota_available:false,identity:{email:null,phone:null,username:null},plan:null,quota_windows:[],quota_error:{code:"empty_response"}}}/>);
  expect(html).toContain('role="alert"');expect(html).toContain("超时");
  expect(html).not.toContain("上游暂未提供额度数据");
 });
});
