import {Fragment} from "react";
import {formatObservedAt} from "../runtime/model";
import "./accountMetadata.css";
export type AccountQuota=Readonly<{source:string;observed_at_ms:number;partial?:boolean;windows:readonly Readonly<{name:string;used_percent:number|null;used?:number|null;limit?:number|null;unit?:string|null;reset_at:string|null;reset_at_ms:number|null;duration_seconds:number|null}>[]}>;
const names:Record<string,string>={"build.credits":"订阅 Credits","console.chat":"对话额度","console.image":"图片额度","console.video":"视频额度","web.credits":"Credits 周期额度","web.auto":"自动模式","web.fast":"快速模式",five_hour:"5 小时",seven_day:"7 天",seven_day_oauth_apps:"OAuth 应用 · 7 天",seven_day_opus:"Opus · 7 天",seven_day_sonnet:"Sonnet · 7 天",seven_day_cowork:"Cowork · 7 天","rate_limit.primary_window":"主要窗口","rate_limit.secondary_window":"次要窗口","code_review_rate_limit.primary_window":"代码审查 · 主要窗口","code_review_rate_limit.secondary_window":"代码审查 · 次要窗口"};
function quotaName(name:string) {return name.startsWith("kiro.trial.")?"试用额度":name.startsWith("kiro.bonus.")?"奖励额度":name.startsWith("kiro.overage.")?"超额使用":name.startsWith("kiro.resource.")?"基础额度":names[name]??name;}
export function accountQuotaSummary(observation:AccountQuota|undefined|null):string|undefined {
 const summary=observation?.windows.map(window=>`${quotaName(window.name)} · 已用 ${window.used_percent==null?"未知":`${window.used_percent.toLocaleString(undefined,{maximumFractionDigits:1})}%`}`).join("；");
 return summary?(observation?.partial?`部分额度 · ${summary}`:summary):undefined;
}
const quotaErrors:Record<string,string>={unauthorized:"账号授权已失效，请重新授权或更新凭据。",forbidden:"渠道或访问防护拒绝了额度查询，请检查会话与出口。",egress_denied:"当前出口策略未允许渠道的额度接口。",invalid_response:"渠道额度响应格式不受支持，无法可靠显示。",busy:"账号正在使用或额度读取繁忙，请稍后重试。"};
export function AccountQuotaEvidence({observation,error,loading,onRefresh}:{observation?:AccountQuota|null;error?:string|null;loading:boolean;onRefresh:()=>void}) {
 return <section className="account-evidence-card"><h4>账号额度</h4>
 {loading?<p role="status">正在读取账号额度…</p>:error?<p role={error==="not_implemented_or_not_connected"?undefined:"alert"}>{error==="not_implemented_or_not_connected"?"此账号尚未接入可读取的额度来源，请检查渠道能力及接口连接。套餐声明不代表剩余额度。":error==="configuration_changed"?"配置已变化，请重新读取。":quotaErrors[error]??"额度读取失败，请稍后重试；不能据此判断剩余额度。"}</p>:observation?<>{observation.partial?<p role="status">仅取得部分额度信息，不能作为全部剩余额度。</p>:null}{observation.windows.length===0?<p>尚未取得可核验的额度数值。</p>:null}<dl className="fact-grid kimi-quota-facts">{observation.windows.map(window=>{
 const reset=window.reset_at_ms??(window.reset_at?Date.parse(window.reset_at):NaN);
 return <Fragment key={window.name}><dt>{quotaName(window.name)}{window.duration_seconds?<span className="kimi-quota-reset">{window.duration_seconds/3600} 小时窗口</span>:null}</dt><dd>已使用 {window.used_percent==null?"比例未知":`${window.used_percent.toLocaleString(undefined,{maximumFractionDigits:1})}%`}{window.used!=null&&window.limit!=null?<span className="kimi-quota-reset">{window.used.toLocaleString()} / {window.limit.toLocaleString()}{window.unit?` ${window.unit}`:""}</span>:null}{Number.isFinite(reset)?<span className="kimi-quota-reset">重置：{new Date(reset).toLocaleString()}</span>:null}</dd></Fragment>;
 })}</dl><p className="muted">观测时间：{formatObservedAt(observation.observed_at_ms)} · 上游用量比例，不是 Token 余额</p></>:<p>尚未取得额度观测。</p>}
 <button className="secondary" disabled={loading} onClick={onRefresh}>重新读取额度</button></section>;
}
