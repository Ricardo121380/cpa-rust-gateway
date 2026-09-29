import {AuthorizationRecovery,continueAuthorizationAfterReview} from "./AuthorizationRecovery";
import {ConfigurationTaskReview} from "../config-versions/ConfigurationTaskReview";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { safeExternalUrl } from "../upstreams/model";

type Session = Readonly<{
  state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
  session_id?: string;
  user_code?: string;
  verification_uri?: string;
  expires_at_ms?: number;
  interval_ms?: number;
  credential_id?: string;
}>;
type Enrollment = Readonly<{
  task: Awaited<ReturnType<typeof beginConfigurationTask>>;
  id: string;
  providerId: string;
  endpointId?: string;
}>;
type PreparedTarget = Readonly<{ upstream_id: string; endpoint_id?: string }>;

/** Exact existing owner is required for renewal; first authorization alone may prepare a target. */
export function existingKimiTarget(credentialId:string|undefined,providerId:string|undefined):PreparedTarget|undefined {
  return credentialId&&providerId?{upstream_id:providerId}:undefined;
}

/** Attach a newly authorized account before publication; never alter renewal bindings. */
export async function finishKimiEnrollment(enrollment: Enrollment, credentialId: string) {
  if (enrollment.endpointId) {
    const bindings = await enrollment.task.read<{credential_id: string}[]>(
      "listEndpointCredentialBindings", {path: {endpoint_id: enrollment.endpointId}},
    );
    if (!bindings.some(binding => binding.credential_id === credentialId)) {
      await enrollment.task.mutate("createEndpointCredentialBinding", {
        path: {endpoint_id: enrollment.endpointId},
        body: {credential_id: credentialId, enabled: true, priority: 0, weight: 1, concurrency: 1},
      });
    }
  }
  return enrollment.task.finish();
}

/** Retains the one browser-visible challenge while the server returns compact poll states. */
export function mergeKimiDeviceSession(current: Session | undefined, next: Session): Session {
  return { ...current, ...next };
}

/** Kimi Coding device authorization never presents Provider or Endpoint implementation details. */
export function KimiDeviceDialog({
  credentialId,
  providerId,
  onClose,
  onComplete,
}: Readonly<{
  credentialId?: string;
  providerId?: string;
  onClose: () => void;
  onComplete: (notice?: string) => void;
}>) {
  const current = useRef<Enrollment | undefined>(undefined);
  const pollingGeneration = useRef(0);
  const [session, setSession] = useState<Session>();
  const [workingId, setWorkingId] = useState<string>();
  const [completed, setCompleted] = useState<ConfigVersionSummary>();
  const [completionError, setCompletionError] = useState<unknown>();
  const [unresolvedResult, setUnresolvedResult] = useState(false);
  const [pendingRecovery,setPendingRecovery]=useState(false);
  const [recoveredId,setRecoveredId]=useState<string>();const [reviewBusy,setReviewBusy]=useState(false);
  const [connected,setConnected]=useState(false);
  const mergeSession = (next: Session) => setSession((current) => mergeKimiDeviceSession(current, next));
  const start = useMutation({
    mutationFn: async () => {
      const task = await beginConfigurationTask("授权 Kimi 账号");
      const id = credentialId ?? `kimi-${crypto.randomUUID()}`;
      const target = existingKimiTarget(credentialId,providerId)??await task.mutate<PreparedTarget>("prepareKimiAccountTarget",providerId?{query:{upstream_id:providerId}}:undefined);
      if (!credentialId && !target.endpoint_id) throw new Error("Kimi 接口准备结果不完整，请重新读取配置。");
      current.current = {
        task,
        id,
        providerId: target.upstream_id,
        endpointId: target.endpoint_id,
      };
      setWorkingId(task.version.id);
      return task.mutate<Session>("startKimiEnrollment", {
        path: { upstream_id: target.upstream_id },
        body: { id, replace_existing: !!credentialId },
      });
    },
    onSuccess: mergeSession,
  });
  const poll = useMutation({
    mutationFn: async () => {
      const enrollment = current.current;
      if (!enrollment || !session?.session_id) throw new Error("请先开始授权。");
      const result = await enrollment.task.mutate<Session>("pollKimiEnrollment", {
        path: { upstream_id: enrollment.providerId },
        body: {
          id: enrollment.id,
          session_id: session.session_id,
          replace_existing: !!credentialId,
        },
      });
      if (result.state !== "completed" || !result.credential_id) return result;
      try {
        setCompleted(await finishKimiEnrollment(enrollment, result.credential_id));
        setConnected(true);
      } catch (error) {
        // The terminal poll already persisted the credential. Keep that
        // receipt terminal and offer configuration recovery; retrying poll
        // would replay a consuming provider operation.
        setCompletionError(error);
        setRecoveredId(result.credential_id);
      }
      return result;
    },
    onSuccess: mergeSession,
    onError: () => setUnresolvedResult(true),
  });
  const cancel = useMutation({
    mutationFn: async () => {
      const enrollment = current.current;
      if (!enrollment || !session?.session_id) return;
      await enrollment.task.read("cancelKimiEnrollment", {
        path: { upstream_id: enrollment.providerId },
        headers: { "If-Match": enrollment.task.revision() },
        body: {
          id: enrollment.id,
          session_id: session.session_id,
          replace_existing: !!credentialId,
        },
      });
    },
  });
  const resume=useMutation({mutationFn:async()=>{const entry=current.current!;const version=await entry.task.read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:entry.task.version.id}});if(version.status!=="draft"||version.revision!==entry.task.revision())throw new Error("工作配置已变化，请核对完整清单；不会再次轮询已完成的授权。");return finishKimiEnrollment(entry,recoveredId!);},onSuccess:version=>{setConnected(true);setCompleted(version);setCompletionError(undefined);setRecoveredId(undefined);},onError:setCompletionError});
  const busy = start.isPending || poll.isPending || cancel.isPending||resume.isPending||reviewBusy;
  const awaitingConsent = session?.state === "pending" && completed === undefined && completionError === undefined && !unresolvedResult;
  const pollInFlight = poll.isPending;
  const canSchedulePoll = awaitingConsent && !pollInFlight && !poll.isError;
  useEffect(() => {
    if (!canSchedulePoll || !session?.session_id) return;
    const generation = ++pollingGeneration.current;
    const timer = window.setTimeout(() => {
      if (generation === pollingGeneration.current) poll.mutate();
    }, Math.max(1_000, session.interval_ms ?? 5_000));
    return () => {
      pollingGeneration.current += 1;
      window.clearTimeout(timer);
    };
  }, [canSchedulePoll, poll.mutate, session?.interval_ms, session?.session_id]);
  const dismissAuthorization = async () => {
    pollingGeneration.current += 1;
    if (completed !== undefined || completionError !== undefined || unresolvedResult || !awaitingConsent) return true;
    try { await cancel.mutateAsync(); return true; } catch { return false; }
  };
  const close = () => {
    pollingGeneration.current += 1;
    if (completed) {
      useVersionStore.getState().select(completed);
      onComplete("Kimi 账号授权已保存。");
      return;
    }
    if (completionError !== undefined) {
      onComplete("Kimi 账号授权已保存，配置尚未应用，请核对待应用的修改。");
      return;
    }
    onClose();
  };
  const url = safeExternalUrl(session?.verification_uri);
  const status: Record<Session["state"], string> = {
    pending: "等待 Kimi 官方授权",
    completed: "Kimi 授权已保存",
    cancelled: "授权已取消",
    denied: "授权被拒绝",
    expired: "授权已过期",
    failed: "授权未完成",
  };
  const footer = completed ? <SheetDismissButton disabled={busy}>完成</SheetDismissButton> : !session ? <><SheetDismissButton className="secondary" disabled={busy}>取消</SheetDismissButton><button type="button" className="primary" disabled={busy || start.isError} onClick={() => start.mutate()}>开始授权</button></> : unresolvedResult ? <SheetDismissButton disabled={busy}>关闭并标记结果未确认</SheetDismissButton> : awaitingConsent ? <SheetDismissButton className="secondary" disabled={busy}>取消授权</SheetDismissButton> : <SheetDismissButton disabled={busy}>关闭</SheetDismissButton>;
  return <Sheet title={credentialId ? "重新授权 Kimi 账号" : "授权 Kimi 账号"} description={credentialId ? "仅更新这个已有账号的授权；当前连接会保留。" : "开始后在 Kimi 官方页面完成设备授权，面板会自动核对结果。"} onEscape={close} onBeforeDismiss={dismissAuthorization} busy={busy} blockNavigation={awaitingConsent} footer={footer}>
    <div className="operation-summary"><span>Kimi · 设备授权</span><strong>连接 Kimi 账号</strong><small>获取验证码 → 官方确认 → 自动保存</small></div>
    {completed ? <p className="operation-receipt" role="status">Kimi 账号已保存。</p> : !session ? <>
      <p>将打开 Kimi 官方设备授权。完成登录后，此窗口会自动保存授权。</p>
    </> : <>
      <p role={unresolvedResult ? "alert" : "status"}>{unresolvedResult ? "授权结果暂时无法确认；不会重复提交授权请求。请稍后在账号管理中核对。" : pollInFlight && awaitingConsent ? "正在检查 Kimi 授权…" : status[session.state]}</p>
      {awaitingConsent ? <>
        <p>在 Kimi 官方页面输入此验证码：</p>
        <strong className="authorization-device-code mono">{session.user_code}</strong>
        {url ? <p><a className="button" href={url} target="_blank" rel="noopener noreferrer">打开 Kimi 授权页</a></p> : null}
        {session.expires_at_ms ? <p className="muted">有效期至 {new Date(session.expires_at_ms).toLocaleTimeString()}。完成后会自动检查结果。</p> : null}
      </> : null}
    </>}
    {unresolvedResult&&session?.session_id&&current.current?<AuthorizationRecovery task={current.current.task} sessionId={session.session_id} onReceipt={(receipt,canContinue)=>{
      setPendingRecovery(receipt.state==="pending"&&canContinue);
      if(receipt.state==="completed"){mergeSession({state:"completed",credential_id:receipt.credential_id});setUnresolvedResult(false);setCompletionError(new Error("授权已保存，连接与应用尚待核对。"));if(canContinue)setRecoveredId(receipt.credential_id);}
      else if(["failed","denied","expired","cancelled"].includes(receipt.state)){mergeSession({state:receipt.state as Session["state"]});setUnresolvedResult(false);}
    }}/>:null}
    {unresolvedResult&&pendingRecovery?<button className="primary" disabled={busy} onClick={()=>{poll.reset();setPendingRecovery(false);setUnresolvedResult(false);}}>继续等待此授权会话</button>:null}
    {recoveredId?<button className="primary" disabled={busy} onClick={()=>resume.mutate()}>继续连接与应用</button>:null}
    {session?.state==="completed"&&session.credential_id&&current.current&&completed?.status!=="active"?<ConfigurationTaskReview task={current.current.task} onBusyChange={setReviewBusy} onReviewed={!connected?async review=>{
      const entry=current.current!;
      const version=await continueAuthorizationAfterReview(entry.task,review,{upstream_id:entry.providerId,endpoint_id:entry.endpointId},session.credential_id!);
      setConnected(true);return version;
    }:undefined} onApplied={version=>{if(version.status==="active"){setCompleted(version);setCompletionError(undefined);setRecoveredId(undefined);}else setCompletionError(new Error("接口连接已核对，配置尚未应用；请核对更新后的完整清单。"));}}/>:null}
    <ConfigurationTaskNotice
      workingId={workingId}
      error={completionError ?? start.error ?? poll.error ?? cancel.error}
      onReview={(version) => {
        useVersionStore.getState().select(version);
        onComplete("请核对已保存的 Kimi 授权修改。");
      }}
    />
  </Sheet>;
}
