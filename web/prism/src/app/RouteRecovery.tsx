import { useEffect } from "react";
import { useLocation, useNavigate, useRouteError } from "react-router-dom";
import { useSessionStore } from "../session/sessionStore";
import "./recovery.css";

/** A stable fingerprint permits correlation without logging a stack, URL query,
 * thrown message or serialized application state (all may contain secrets). */
export function renderFailureReference(error: unknown): string {
  const source = error instanceof Error ? `${error.name}\n${error.stack ?? error.message}` : typeof error;
  let hash = 2166136261;
  for (const char of source) hash = Math.imul(hash ^ char.charCodeAt(0), 16777619);
  return `UI-${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

export function RouteRecovery() {
  const error = useRouteError();
  const location = useLocation();
  const navigate = useNavigate();
  const unlocked = useSessionStore(state => state.unlocked);
  const reference = renderFailureReference(error);
  useEffect(() => { console.error("Prism render failure", reference); }, [reference]);
  return <main className="route-recovery"><section className="card route-recovery-panel" role="alert">
    <span className="route-recovery-brand">Prism</span>
    <h1>页面暂时无法显示</h1>
    <p>可以重新打开此页面，或返回工作区。若刚刚提交过修改，请先核对最新状态，再决定是否继续操作。</p>
    <div className="page-actions">
      <button onClick={() => void navigate(unlocked ? `${location.pathname}${location.search}` : "/unlock", { replace: true })}>重新打开页面</button>
      {unlocked ? <button className="secondary" onClick={() => void navigate("/", { replace: true })}>回到工作区</button> : null}
      <button className="secondary" onClick={() => { useSessionStore.getState().lock(); void navigate("/unlock", { replace: true }); }}>重新登录</button>
    </div>
    <small className="muted">问题编号 <code>{reference}</code> · 持续出现时，请提供此编号和触发步骤。</small>
  </section></main>;
}
