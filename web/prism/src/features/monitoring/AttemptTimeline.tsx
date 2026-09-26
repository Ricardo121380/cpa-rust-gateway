import {Link} from "react-router-dom";
import {ResourceIdentity} from "../../components/ResourceIdentity";
import {attemptRecovery,errorCodeLabel,errorScopeLabel,formatTime,retryLabel,stageLabel,type AttemptRow} from "./model";

export function AttemptTimeline({items,onNavigate}:{items:readonly AttemptRow[];onNavigate:()=>void}) {
  if(!items.length)return <p className="muted">暂无可读取的尝试记录。</p>;
  return <ol className="request-attempts">{items.map(attempt=>{
    const observed=attempt.observation;
    const target=attempt.endpoint_id&&attempt.credential_id?new URLSearchParams({endpoint_id:attempt.endpoint_id,credential_id:attempt.credential_id,account_id:attempt.credential_id}):undefined;
    const recovery=attemptRecovery(observed?.error_code);
    return <li key={attempt.attempt_id}>
      <strong>尝试 {observed?.attempt_number??"编号未观测"} · {attempt.outcome==="succeeded"?"已建立上游响应":attempt.outcome==="failed"?"失败":attempt.outcome==="cancelled"?"已取消":"结果未知"}</strong>
      <dl className="request-facts"><dt>耗时</dt><dd>{observed?`${observed.duration_ms.toLocaleString()} ms`:"未观测"}</dd><dt>观测时间</dt><dd>{observed?formatTime(observed.ended_at_ms):"未观测"}</dd>
      {observed?.error_code?<><dt>原因</dt><dd>{errorCodeLabel(observed.error_code)??"未分类错误"}{observed.error_scope?` · ${errorScopeLabel(observed.error_scope)}`:""}</dd></>:null}
      {observed?<><dt>调度决定</dt><dd>{retryLabel(observed.retry_decision)}</dd></>:null}
      {attempt.stage?<><dt>失败阶段</dt><dd>{stageLabel(attempt.stage)}</dd></>:null}
      {attempt.credential_id?<><dt>账号</dt><dd><ResourceIdentity id={attempt.credential_id} kind="account"/></dd></>:null}</dl>
      {recovery?<p>{recovery}</p>:null}
      <div className="page-actions">
        {attempt.credential_id?<Link to={`/accounts?${new URLSearchParams({view:"runtime",account_id:attempt.credential_id,...(observed?{provider:observed.upstream_id}:{}),...(attempt.endpoint_id?{channel_id:attempt.endpoint_id}:{})})}`} onClick={onNavigate}>账号状态</Link>:null}
        {observed?<Link to={`/upstreams?${new URLSearchParams({upstream_id:observed.upstream_id})}`} onClick={onNavigate}>提供商</Link>:null}
        {target?<><Link to={`/catalog?${target}`} onClick={onNavigate}>模型目录</Link><Link to={`/runtime?${target}`} onClick={onNavigate}>运行诊断</Link></>:null}
      </div>
    </li>;
  })}</ol>;
}
