import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { asAppError } from "../../api/errors";
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
      setCompleted(await enrollment.task.finish());
      return result;
    },
    onSuccess: mergeSession,
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
    onSuccess: onClose,
  });
  const busy = start.isPending || poll.isPending || cancel.isPending;
  const pending = session?.state === "pending" && !poll.isError && !poll.isPending;
  useEffect(() => {
    if (!pending || !session?.session_id || completed) return;
    const generation = ++pollingGeneration.current;
    const timer = window.setTimeout(() => {
      if (generation === pollingGeneration.current) poll.mutate();
    }, Math.max(1_000, session.interval_ms ?? 5_000));
    return () => {
      pollingGeneration.current += 1;
      window.clearTimeout(timer);
    };
  }, [completed, pending, poll.mutate, session?.interval_ms, session?.session_id]);
  const close = () => {
    pollingGeneration.current += 1;
    if (busy) return;
    if (completed) {
      useVersionStore.getState().select(completed);
      onComplete("Kimi 账号授权已保存。");
      return;
    }
    if (pending) {
      cancel.mutate();
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
  return <Sheet title={credentialId ? "重新授权 Kimi 账号" : "授权 Kimi 账号"} description={credentialId ? "仅更新这个已有账号的授权；当前连接会保留。" : "开始后在 Kimi 官方页面完成设备授权，面板会自动核对结果。"} onEscape={close} busy={busy}>
    {completed ? <p role="status">Kimi 账号已保存。</p> : !session ? <>
      <p>将打开 Kimi 官方设备授权。完成登录后，此窗口会自动保存授权。</p>
      <button disabled={busy || start.isError} onClick={() => start.mutate()}>开始授权</button>
    </> : <>
      <p role="status">{status[session.state]}</p>
      {pending ? <>
        <p>在 Kimi 官方页面输入此验证码：</p>
        <strong className="mono">{session.user_code}</strong>
        {url ? <p><a className="button" href={url} target="_blank" rel="noopener noreferrer">打开 Kimi 授权页</a></p> : null}
        {session.expires_at_ms ? <p className="muted">有效期至 {new Date(session.expires_at_ms).toLocaleTimeString()}。完成后会自动检查结果。</p> : null}
      </> : null}
    </>}
    <ConfigurationTaskNotice
      workingId={workingId}
      error={start.error ?? poll.error}
      onReview={(version) => {
        useVersionStore.getState().select(version);
        onComplete("请核对已保存的 Kimi 授权修改。");
      }}
    />
    {cancel.isError ? <p role="alert">{asAppError(cancel.error).message}</p> : null}
    <div className="sheet-actions">
      <SheetDismissButton className="secondary" disabled={busy}>
        {completed ? "完成" : pending ? "取消授权" : "关闭"}
      </SheetDismissButton>
    </div>
  </Sheet>;
}
