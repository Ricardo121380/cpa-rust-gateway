import "./accountMetadata.css";
import {Fragment} from "react";
import {formatObservedAt} from "../runtime/model";

export type KimiObservation = Readonly<{
  observed_at_ms: number;
  profile_available: boolean;
  quota_available: boolean;
  identity: Readonly<{email: string|null; phone: string|null; username: string|null}>;
  plan: string|null;
  quota_windows: readonly Readonly<{window: string; used_ratio: number; reset_at: string|null}>[];
}>;
export type KimiMetadata = Readonly<{kimi?: KimiObservation|null; kimi_error?: string|null}>;
const windows: Record<string,string> = {limit_5h:"5 小时额度",limit_7d:"7 天额度",limit_month_total:"月度总额度",limit_month_code:"月度 Coding 额度"};
export function kimiQuotaSummary(observation: KimiObservation|undefined|null): string|undefined {
  if (!observation?.quota_available) return undefined;
  return observation.quota_windows.map(window=>`${windows[window.window]??window.window} · 已用 ${(window.used_ratio*100).toLocaleString(undefined,{maximumFractionDigits:1})}%`).join("；");
}
export function kimiMetadataError(error: string|undefined|null) {
  return error === "not_connected" ? "账号尚未连接到已启用的 Kimi 接口，暂时无法读取账号信息。"
    : error === "configuration_changed" ? "当前配置已变化，请重新读取。"
    : error ? "Kimi 账号信息暂时读取失败，请稍后重试。" : null;
}
export function KimiQuotaEvidence({observation,error,loading,onRefresh}:{observation?: KimiObservation|null;error?: string|null;loading:boolean;onRefresh:()=>void}) {
  const message = kimiMetadataError(error);
  return <section className="account-evidence-card" aria-label="Kimi 官方额度">
    <h4>Kimi Coding 额度</h4>
    {loading ? <p role="status">正在读取官方额度…</p> : message ? <p role="alert">{message}</p> : !observation?.quota_available ? <p>暂未取得可识别的额度数据，不能据此判断剩余额度。</p> : <dl className="fact-grid kimi-quota-facts">
      {observation.quota_windows.map(window => <Fragment key={window.window}><dt>{windows[window.window]??window.window}</dt><dd>已使用 {(window.used_ratio*100).toLocaleString(undefined,{maximumFractionDigits:1})}%{window.reset_at ? <span className="kimi-quota-reset">重置：{Number.isFinite(Date.parse(window.reset_at))?new Date(window.reset_at).toLocaleString():window.reset_at}</span> : null}</dd></Fragment>)}
    </dl>}
    {observation ? <p className="muted">观测时间：{formatObservedAt(observation.observed_at_ms)} · 官方用量比例，不是 Token 余额</p> : null}
    <button className="secondary" disabled={loading} onClick={onRefresh}>重新读取额度</button>
  </section>;
}
