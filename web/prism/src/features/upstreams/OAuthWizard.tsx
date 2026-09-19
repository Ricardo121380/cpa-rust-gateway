// Legacy credential OAuth renewal. It renews exactly one existing, server-
// projected Codex credential; first-account enrollment, target preparation,
// bindings, drafts and publication deliberately remain outside this flow.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import {
  oauthFailureLabel,
  oauthPollIntervalMs,
  parseOAuthCallback,
  safeExternalUrl,
  type OAuthFailureClass,
  type OAuthState,
} from "./model";

type OAuthOperation = Readonly<{
  credential_id: string;
  state: OAuthState;
  expires_at_ms?: number | null;
  authorization_url?: string | null;
  failure_class?: OAuthFailureClass | null;
}>;

type CredentialSnapshot = Readonly<{
  id: string;
  revision: number;
  kind: string;
  status: string;
  secret_present: boolean;
}>;

type Attempt = Readonly<{
  id: number;
  credentialId: string;
  configVersionId: string;
  selectionGeneration: number;
  sessionGeneration: number;
  baselineCredentialRevision: number;
}>;

type Phase = "ready" | "awaiting" | "completing" | "cancelling" | "verifying" | "receipt" | "terminal" | "unresolved";

// Module ownership makes this identity monotonic across remounts. A per-mount
// counter would allow a completed old query to be reused by a new wizard.
let nextAttemptId = 0;

function displayState(state: OAuthState | undefined): string {
  switch (state) {
    case "pending": return "等待官方授权";
    case "complete": return "服务端报告账号已保存";
    case "cancelled": return "授权已取消";
    case "expired": return "授权已过期";
    case "failed": return "授权未完成";
    default: return "正在准备授权";
  }
}

function statusKey(attempt: Attempt): readonly [string, string, string, number] {
  return ["credential-oauth", attempt.configVersionId, attempt.credentialId, attempt.id];
}

export function OAuthWizard({
  credentialId,
  accountName,
  onClose,
}: Readonly<{ credentialId: string; accountName?: string; onClose: () => void }>) {
  const queryClient = useQueryClient();
  const context = useVersionStore((state) => state.context);
  const selectionGeneration = useVersionStore((state) => state.selectionGeneration);
  const sessionGeneration = useSessionStore((state) => state.generation);
  const activeAttemptRef = useRef<Attempt | undefined>(undefined);
  const completionInFlightRef = useRef(false);
  const completionAcknowledgedRef = useRef<number | undefined>(undefined);
  const completionUncertainRef = useRef<number | undefined>(undefined);
  const cancellationAcknowledgedRef = useRef<number | undefined>(undefined);
  const rejectedCallbackReconciliationRef = useRef<number | undefined>(undefined);
  const inputErrorRef = useRef<HTMLParagraphElement>(null);
  const formId = `credential-oauth-callback-${useId().replace(/:/gu, "")}`;
  const callbackErrorId = `${formId}-error`;
  const [attempt, setAttempt] = useState<Attempt>();
  const [phase, setPhase] = useState<Phase>("ready");
  const [operation, setOperation] = useState<OAuthOperation>();
  const [challenge, setChallenge] = useState<string>();
  const [callback, setCallback] = useState("");
  const [inputError, setInputError] = useState<string>();
  const [lifecycleError, setLifecycleError] = useState<string>();
  const [rereadError, setRereadError] = useState<string>();
  const [ownershipLost, setOwnershipLost] = useState(false);
  const [pollEpoch, setPollEpoch] = useState(0);
  const [completing, setCompleting] = useState(false);

  const ownsAttempt = (candidate: Attempt): boolean => {
    const version = useVersionStore.getState();
    return version.context?.configVersionId === candidate.configVersionId
      && version.selectionGeneration === candidate.selectionGeneration
      && useSessionStore.getState().generation === candidate.sessionGeneration;
  };

  const isCurrentAttempt = (candidate: Attempt): boolean =>
    activeAttemptRef.current?.id === candidate.id && ownsAttempt(candidate);

  const clearCallback = (): void => {
    setCallback("");
    setInputError(undefined);
  };

  const removeAttemptQueries = (candidate: Attempt): void => {
    void queryClient.cancelQueries({ queryKey: statusKey(candidate) });
    queryClient.removeQueries({ queryKey: statusKey(candidate) });
  };

  const pauseStatus = (candidate: Attempt): void => {
    setPollEpoch((value) => value + 1);
    void queryClient.cancelQueries({ queryKey: statusKey(candidate) });
  };

  const retireAttempt = (candidate: Attempt | undefined): void => {
    if (candidate !== undefined) removeAttemptQueries(candidate);
    if (candidate === undefined || activeAttemptRef.current?.id === candidate.id) {
      activeAttemptRef.current = undefined;
      completionAcknowledgedRef.current = undefined;
      completionUncertainRef.current = undefined;
      cancellationAcknowledgedRef.current = undefined;
      rejectedCallbackReconciliationRef.current = undefined;
    }
  };

  const invalidateCredentialViews = (configVersionId: string): void => {
    for (const key of ["credential", "credential-metadata", "account-directory", "managed-inventory", "accounts", "account-pools", "runtime-availability"] as const) {
      void queryClient.invalidateQueries({ queryKey: [key, configVersionId] });
    }
  };

  const readCredential = (candidate: Attempt): Promise<CredentialSnapshot> => {
    if (!isCurrentAttempt(candidate)) return Promise.reject(new Error("授权工作区已切换。"));
    return call<CredentialSnapshot>(
      "getCredential",
      { path: { credential_id: candidate.credentialId } },
      { versionScoped: true },
    );
  };

  const readStatus = (candidate: Attempt, signal?: AbortSignal): Promise<OAuthOperation> => {
    if (!isCurrentAttempt(candidate)) return Promise.reject(new Error("授权工作区已切换。"));
    return call<OAuthOperation>(
      "getCredentialOAuthStatus",
      { path: { credential_id: candidate.credentialId }, signal },
      { versionScoped: true },
    );
  };

  const describeUnconfirmedComplete = async (candidate: Attempt): Promise<void> => {
    try {
      const current = await readCredential(candidate);
      if (!isCurrentAttempt(candidate)) return;
      setLifecycleError(
        current.revision === candidate.baselineCredentialRevision
          ? "该账号原本已保存；临时授权会话已丢失，无法确认本次续授权是否完成。"
          : "账号资料已有变化，但无法确认这项变化来自本次续授权。",
      );
    } catch (cause) {
      if (isCurrentAttempt(candidate)) setLifecycleError(asAppError(cause).message || "授权结果无法确认，请稍后核对账号状态。");
    }
  };

  const recordStatus = (candidate: Attempt, next: OAuthOperation): void => {
    if (!isCurrentAttempt(candidate)) return;
    setOperation(next);
    if (next.state === "pending") return;
    clearCallback();
    pauseStatus(candidate);
    if (next.state === "complete") {
      // A status-only complete can be the backend's durable-account fallback
      // after a process restart. Only an acknowledged callback can issue a
      // success receipt for this attempt.
      setPhase("unresolved");
      void describeUnconfirmedComplete(candidate);
      return;
    }
    setPhase("terminal");
  };

  const status = useQuery({
    queryKey: attempt === undefined ? ["credential-oauth", "inactive"] : [...statusKey(attempt), pollEpoch],
    queryFn: ({ signal }) => {
      if (attempt === undefined) throw new Error("请先开始授权。");
      return readStatus(attempt, signal);
    },
    enabled: attempt !== undefined && phase === "awaiting" && !ownershipLost,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    refetchInterval: (query) =>
      attempt !== undefined && phase === "awaiting" && !ownershipLost && query.state.status !== "error"
        ? oauthPollIntervalMs(query.state.data?.state)
        : false,
  });

  useEffect(() => {
    if (attempt === undefined || status.data === undefined || phase !== "awaiting") return;
    recordStatus(attempt, status.data);
  }, [attempt, phase, status.data, status.dataUpdatedAt]);

  useEffect(() => {
    if (attempt === undefined || !status.isError || phase !== "awaiting" || !isCurrentAttempt(attempt)) return;
    setLifecycleError(asAppError(status.error).message || "暂时无法读取授权状态。");
    setPhase("unresolved");
  }, [attempt, phase, status.error, status.isError]);

  useEffect(() => {
    if (attempt === undefined || ownsAttempt(attempt)) return;
    retireAttempt(attempt);
    clearCallback();
    setChallenge(undefined);
    setOwnershipLost(true);
    setAttempt(undefined);
    setOperation(undefined);
    setPhase("terminal");
  }, [attempt, context?.configVersionId, selectionGeneration, sessionGeneration]);

  useEffect(() => () => { retireAttempt(activeAttemptRef.current); }, [queryClient]);

  useEffect(() => {
    if (inputError !== undefined) inputErrorRef.current?.focus();
  }, [inputError]);

  const start = useMutation({
    gcTime: 0,
    mutationFn: async () => {
      const version = useVersionStore.getState();
      const session = useSessionStore.getState();
      if (version.context === undefined) throw new Error("请先选择一个配置版本。");
      const baseline = await call<CredentialSnapshot>(
        "getCredential",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      );
      const candidate: Attempt = {
        id: ++nextAttemptId,
        credentialId,
        configVersionId: version.context.configVersionId,
        selectionGeneration: version.selectionGeneration,
        sessionGeneration: session.generation,
        baselineCredentialRevision: baseline.revision,
      };
      const next = await call<OAuthOperation>(
        "startCredentialOAuth",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      );
      return { candidate, next };
    },
    onSuccess: ({ candidate, next }) => {
      if (!ownsAttempt(candidate)) return;
      activeAttemptRef.current = candidate;
      completionAcknowledgedRef.current = undefined;
      completionUncertainRef.current = undefined;
      cancellationAcknowledgedRef.current = undefined;
      rejectedCallbackReconciliationRef.current = undefined;
      setAttempt(candidate);
      setOwnershipLost(false);
      setLifecycleError(undefined);
      setRereadError(undefined);
      clearCallback();
      setOperation(next);
      setChallenge(safeExternalUrl(next.authorization_url));
      setPhase(next.state === "pending" ? "awaiting" : "terminal");
    },
    onError: (cause) => setLifecycleError(asAppError(cause).message),
  });

  const reconcile = useMutation({
    gcTime: 0,
    mutationFn: async (candidate: Attempt) => ({ candidate, next: await readStatus(candidate) }),
    onSuccess: ({ candidate, next }) => {
      if (!isCurrentAttempt(candidate)) return;
      setLifecycleError(undefined);
      if (next.state === "pending") {
        setOperation(next);
        if (rejectedCallbackReconciliationRef.current === candidate.id) {
          rejectedCallbackReconciliationRef.current = undefined;
          setLifecycleError("回调不属于本次授权；原会话仍在等待，可使用正确的回调地址重试。");
          setPollEpoch((value) => value + 1);
          setPhase("awaiting");
        } else if (completionUncertainRef.current === candidate.id || cancellationAcknowledgedRef.current === candidate.id) {
          setPhase("unresolved");
          setLifecycleError("网关仍显示等待状态；为避免重复操作，请稍后继续核对。" );
        } else {
          setPollEpoch((value) => value + 1);
          setPhase("awaiting");
        }
        return;
      }
      recordStatus(candidate, next);
    },
    onError: (cause) => setLifecycleError(asAppError(cause).message || "授权结果仍无法确认。"),
  });

  const cancel = useMutation({
    gcTime: 0,
    mutationFn: async (candidate: Attempt) => {
      if (!isCurrentAttempt(candidate)) throw new Error("授权工作区已切换。");
      await call<undefined>(
        "cancelCredentialOAuth",
        { path: { credential_id: candidate.credentialId } },
        { versionScoped: true },
      );
      return candidate;
    },
  });

  const completeCallback = async (candidate: Attempt, body: unknown): Promise<void> => {
    if (completionInFlightRef.current || !isCurrentAttempt(candidate)) return;
    completionInFlightRef.current = true;
    setCompleting(true);
    setPhase("completing");
    pauseStatus(candidate);
    try {
      const next = await call<OAuthOperation>(
        "completeCredentialOAuth",
        { path: { credential_id: candidate.credentialId }, body },
        { versionScoped: true },
      );
      if (!isCurrentAttempt(candidate)) return;
      completionAcknowledgedRef.current = candidate.id;
      clearCallback();
      setOperation(next);
      setChallenge(undefined);
      setPhase("verifying");
      try {
        await readCredential(candidate);
        if (isCurrentAttempt(candidate)) setRereadError(undefined);
      } catch (cause) {
        if (isCurrentAttempt(candidate)) setRereadError(asAppError(cause).message || "授权已确认，但账号资料暂时无法重新读取。");
      }
      if (!isCurrentAttempt(candidate)) return;
      invalidateCredentialViews(candidate.configVersionId);
      setPhase("receipt");
    } catch (cause) {
      if (!isCurrentAttempt(candidate)) return;
      clearCallback();
      const error = asAppError(cause);
      if (error.code === "oauth_callback_rejected") {
        // The legacy endpoint uses this code for more than one provider-side
        // outcome. Re-read the current operation before allowing another
        // consuming callback: only a still-pending session proves retry safe.
        setPhase("verifying");
        try {
          const next = await readStatus(candidate);
          if (!isCurrentAttempt(candidate)) return;
          if (next.state === "pending") {
            completionUncertainRef.current = undefined;
            rejectedCallbackReconciliationRef.current = undefined;
            setOperation(next);
            setLifecycleError("回调不属于本次授权；原会话仍在等待，可使用正确的回调地址重试。");
            setPollEpoch((value) => value + 1);
            setPhase("awaiting");
          } else {
            recordStatus(candidate, next);
          }
        } catch (statusCause) {
          if (!isCurrentAttempt(candidate)) return;
          rejectedCallbackReconciliationRef.current = candidate.id;
          setLifecycleError(asAppError(statusCause).message || "回调结果暂时无法确认；不会重复提交。" );
          setPhase("unresolved");
        }
      } else {
        completionUncertainRef.current = candidate.id;
        setLifecycleError(error.message || "回调结果暂时无法确认；不会重复提交。" );
        setPhase("unresolved");
      }
    } finally {
      completionInFlightRef.current = false;
      setCompleting(false);
    }
  };

  const awaitingCallback = attempt !== undefined && operation?.state === "pending" && phase === "awaiting";
  const unresolved = phase === "unresolved";
  const busy = start.isPending || cancel.isPending || reconcile.isPending || completing;
  const expiry = operation?.expires_at_ms == null ? undefined : Math.max(0, Math.round((operation.expires_at_ms - Date.now()) / 1000));

  const submitCallback = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (attempt === undefined || !awaitingCallback || completionInFlightRef.current) return;
    const parsed = parseOAuthCallback(callback);
    if (!parsed.ok) {
      setInputError(parsed.reason);
      return;
    }
    setInputError(undefined);
    setLifecycleError(undefined);
    // Do not pass this transient value to a mutation cache. The async function
    // owns it only until the single request settles, then releases its frame.
    clearCallback();
    void completeCallback(attempt, parsed.input);
  };

  const dismissAuthorization = async (): Promise<boolean> => {
    if (attempt === undefined || ownershipLost || phase === "receipt" || phase === "terminal" || unresolved) return true;
    if (!awaitingCallback) return false;
    pauseStatus(attempt);
    clearCallback();
    setPhase("cancelling");
    try {
      const candidate = await cancel.mutateAsync(attempt);
      cancellationAcknowledgedRef.current = candidate.id;
      try {
        const next = await readStatus(candidate);
        if (!isCurrentAttempt(candidate)) return false;
        if (next.state === "cancelled" || next.state === "failed" || next.state === "expired") return true;
        if (next.state === "pending") {
          setOperation(next);
          setLifecycleError("取消请求已确认，但服务端仍在处理授权结果。不会重复取消或提交回调。");
          setPhase("unresolved");
          return false;
        }
        recordStatus(candidate, next);
        return false;
      } catch (cause) {
        if (isCurrentAttempt(candidate)) {
          const detail = asAppError(cause).message;
          setLifecycleError(`取消请求已确认，但结果尚未重新读取。不会重复取消。${detail ? ` ${detail}` : ""}`);
          setPhase("unresolved");
        }
        return false;
      }
    } catch (cause) {
      if (isCurrentAttempt(attempt)) {
        setLifecycleError(asAppError(cause).message || "取消授权失败，请重试或稍后核对结果。");
        setPollEpoch((value) => value + 1);
        setPhase("awaiting");
      }
      return false;
    }
  };

  const restart = (): void => {
    retireAttempt(attempt);
    clearCallback();
    setChallenge(undefined);
    setAttempt(undefined);
    setOperation(undefined);
    setOwnershipLost(false);
    setLifecycleError(undefined);
    setRereadError(undefined);
    setPhase("ready");
    start.mutate();
  };

  const close = (): void => {
    retireAttempt(attempt);
    clearCallback();
    setChallenge(undefined);
    onClose();
  };

  const footer = ownershipLost ? (
    <SheetDismissButton disabled={busy}>关闭</SheetDismissButton>
  ) : phase === "receipt" ? (
    <SheetDismissButton disabled={busy}>完成</SheetDismissButton>
  ) : unresolved ? (
    <>
      {attempt !== undefined ? <button type="button" className="secondary" disabled={busy} onClick={() => reconcile.mutate(attempt)}>重新读取状态</button> : null}
      <SheetDismissButton disabled={busy}>关闭并标记结果未确认</SheetDismissButton>
    </>
  ) : attempt === undefined ? (
    <>
      <SheetDismissButton className="secondary" disabled={busy}>关闭</SheetDismissButton>
      <button type="button" disabled={busy} onClick={() => start.mutate()}>{start.isPending ? "正在启动…" : "启动授权"}</button>
    </>
  ) : awaitingCallback ? (
    <>
      <SheetDismissButton className="secondary" disabled={busy}>取消授权</SheetDismissButton>
      <button type="submit" form={formId} disabled={busy || callback.trim().length === 0}>完成授权</button>
    </>
  ) : phase === "completing" || phase === "cancelling" || phase === "verifying" ? (
    <SheetDismissButton disabled>正在确认…</SheetDismissButton>
  ) : (
    <>
      <SheetDismissButton className="secondary" disabled={busy}>关闭</SheetDismissButton>
      <button type="button" disabled={busy} onClick={restart}>重新启动授权</button>
    </>
  );

  return (
    <Sheet
      title="重新授权"
      description={accountName === undefined ? "为当前账号更新授权；不会新建连接或改动已配置接口。" : `为 ${accountName} 更新授权；不会新建连接或改动已配置接口。`}
      onEscape={close}
      onBeforeDismiss={dismissAuthorization}
      busy={busy}
      isDirty={callback.trim().length > 0}
      blockNavigation={awaitingCallback}
      footer={footer}
    >
      {ownershipLost ? <p role="alert">配置或登录状态已变化，已停止这次授权。请关闭后在当前工作区重新开始。</p> : null}
      {phase === "receipt" ? <p role="status">本次授权回调已由服务端确认并保存。{rereadError === undefined ? "账号资料已重新读取。" : "账号资料将稍后重新读取。"}</p> : null}
      {attempt === undefined && !ownershipLost ? <p>启动后将在官方页面完成登录，然后把浏览器跳转的完整回调地址粘贴回来。</p> : null}
      {attempt !== undefined && !ownershipLost && phase !== "receipt" ? (
        <>
          <p role={unresolved || phase === "terminal" ? "alert" : "status"}>{displayState(operation?.state)}</p>
          {operation?.failure_class !== undefined && operation.failure_class !== null ? <p role="alert" className="reveal-warning">{oauthFailureLabel(operation.failure_class)}</p> : null}
          {awaitingCallback ? (
            <>
              <p>在官方页面完成授权后，复制浏览器地址栏中的完整回调地址。</p>
              {challenge !== undefined ? <p><a className="button" href={challenge} target="_blank" rel="noreferrer noopener">打开官方授权页</a></p> : <p className="reveal-warning">网关没有返回可用的授权链接。请取消后重新启动，或检查渠道配置。</p>}
              {expiry !== undefined ? <p className="muted small">本次授权约在 {expiry} 秒后过期。</p> : null}
              <form id={formId} className="sheet-form" onSubmit={submitCallback}>
                <label htmlFor={`${formId}-input`}>回调地址</label>
                <textarea
                  id={`${formId}-input`}
                  aria-label="回调地址"
                  rows={3}
                  value={callback}
                  maxLength={20480}
                  autoComplete="off"
                  spellCheck={false}
                  aria-invalid={inputError === undefined ? undefined : true}
                  aria-describedby={inputError === undefined ? undefined : callbackErrorId}
                  disabled={busy}
                  placeholder="http://127.0.0.1:8085/callback?code=…&state=…"
                  onChange={(event) => { setCallback(event.target.value); setInputError(undefined); setLifecycleError(undefined); }}
                />
                <p className="muted small">本机回调页可能打不开；复制地址栏内容即可。授权码只在提交期间保留。</p>
                {inputError !== undefined ? <p ref={inputErrorRef} id={callbackErrorId} tabIndex={-1} role="alert" className="reveal-warning">{inputError}</p> : null}
              </form>
            </>
          ) : null}
          {unresolved ? <p role="alert" className="reveal-warning">授权结果暂时无法确认；不会重复提交回调或取消请求。请重新读取状态，或稍后在账号管理中核对。</p> : null}
        </>
      ) : null}
      {lifecycleError !== undefined ? <p role="alert" className="reveal-warning">{lifecycleError}</p> : null}
      {rereadError !== undefined ? <p role="alert" className="reveal-warning">{rereadError}</p> : null}
    </Sheet>
  );
}
