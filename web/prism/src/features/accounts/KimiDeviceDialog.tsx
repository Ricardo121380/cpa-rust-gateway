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
}>;
type PreparedTarget = Readonly<{ upstream_id: string }>;

/** Exact existing owner is required for renewal; first authorization alone may prepare a target. */
export function existingKimiTarget(credentialId:string|undefined,providerId:string|undefined):PreparedTarget|undefined {
  return credentialId&&providerId?{upstream_id:providerId}:undefined;
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
  const mergeSession = (next: Session) => setSession((current) => mergeKimiDeviceSession(current, next));
  const start = useMutation({
    mutationFn: async () => {
      const task = await beginConfigurationTask("授权 Kimi 账号");
      const id = credentialId ?? `kimi-${crypto.randomUUID()}`;
      const target = existingKimiTarget(credentialId,providerId)??await task.mutate<PreparedTarget>("prepareKimiAccountTarget");
      current.current = {
        task,
        id,
        providerId: target.upstream_id,
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
        setCompleted(await enrollment.task.finish());
      } catch (error) {
        // The terminal poll already persisted the credential. Keep that
        // receipt terminal and offer configuration recovery; retrying poll
        // would replay a consuming provider operation.
        setCompletionError(error);
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
  const busy = start.isPending || poll.isPending || cancel.isPending;
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
  const footer = completed ? <SheetDismissButton disabled={busy}>完成</SheetDismissButton> : !session ? <><SheetDismissButton className="secondary" disabled={busy}>取消</SheetDismissButton><button type="button" disabled={busy || start.isError} onClick={() => start.mutate()}>开始授权</button></> : unresolvedResult ? <SheetDismissButton disabled={busy}>关闭并标记结果未确认</SheetDismissButton> : awaitingConsent ? <SheetDismissButton className="secondary" disabled={busy}>取消授权</SheetDismissButton> : <SheetDismissButton disabled={busy}>关闭</SheetDismissButton>;
  return <Sheet title={credentialId ? "重新授权 Kimi 账号" : "授权 Kimi 账号"} description={credentialId ? "仅更新这个已有账号的授权；当前连接会保留。" : "开始后在 Kimi 官方页面完成设备授权，面板会自动核对结果。"} onEscape={close} onBeforeDismiss={dismissAuthorization} busy={busy} blockNavigation={awaitingConsent} footer={footer}>
    {completed ? <p role="status">Kimi 账号已保存。</p> : !session ? <>
      <p>将打开 Kimi 官方设备授权。完成登录后，此窗口会自动保存授权。</p>
    </> : <>
      <p role={unresolvedResult ? "alert" : "status"}>{unresolvedResult ? "授权结果暂时无法确认；不会重复提交授权请求。请稍后在账号管理中核对。" : pollInFlight && awaitingConsent ? "正在检查 Kimi 授权…" : status[session.state]}</p>
      {awaitingConsent ? <>
        <p>在 Kimi 官方页面输入此验证码：</p>
        <strong className="mono">{session.user_code}</strong>
        {url ? <p><a className="button" href={url} target="_blank" rel="noopener noreferrer">打开 Kimi 授权页</a></p> : null}
        {session.expires_at_ms ? <p className="muted">有效期至 {new Date(session.expires_at_ms).toLocaleTimeString()}。完成后会自动检查结果。</p> : null}
      </> : null}
    </>}
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
