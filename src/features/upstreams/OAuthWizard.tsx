// Credential OAuth wizard — drives the real contract ops
// (startCredentialOAuth / getCredentialOAuthStatus / cancelCredentialOAuth).
// The contract's OAuthOperation carries only {credential_id, state,
// expires_at_ms}: device-code display (user_code / verification_uri) needs a
// contract extension — recorded in the shapes proposal. Until then the wizard
// shows lifecycle state honestly.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { oauthPollIntervalMs, oauthStateBadge, type OAuthState } from "./model";

type OAuthOperation = Readonly<{
  credential_id: string;
  state: OAuthState;
  expires_at_ms?: number | null;
}>;

export function OAuthWizard({
  credentialId,
  onClose,
}: Readonly<{ credentialId: string; onClose: () => void }>) {
  const queryClient = useQueryClient();
  const [started, setStarted] = useState(false);
  const [error, setError] = useState<string | undefined>();

  const status = useQuery({
    queryKey: ["oauth-status", credentialId],
    queryFn: () =>
      call<OAuthOperation>(
        "getCredentialOAuthStatus",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
    enabled: started,
    refetchInterval: (query) => oauthPollIntervalMs(query.state.data?.state),
  });

  const start = useMutation({
    mutationFn: () =>
      call<OAuthOperation>(
        "startCredentialOAuth",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
    onSuccess: (operation) => {
      setStarted(true);
      queryClient.setQueryData(["oauth-status", credentialId], operation);
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const cancel = useMutation({
    mutationFn: () =>
      call<undefined>(
        "cancelCredentialOAuth",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["oauth-status", credentialId] }),
    onError: (cause) => setError(asAppError(cause).message),
  });

  const operation = status.data;
  const expiresIn =
    operation?.expires_at_ms != null
      ? Math.max(0, Math.round((operation.expires_at_ms - Date.now()) / 1000))
      : undefined;

  return (
    <Sheet title={`OAuth 授权 · ${credentialId}`} onEscape={onClose}>
      {error !== undefined ? (
        <p role="alert" className="reveal-warning">
          {error}
        </p>
      ) : null}

      {!started ? (
        <>
          <p>
            启动后网关将发起设备授权流并轮询其状态。
            <br />
            <small style={{ color: "var(--ink-3)" }}>
              设备码(user_code / verification_uri)的展示需要契约扩展 —— 已记录在形状提案中;
              当前按契约仅呈现操作生命周期。
            </small>
          </p>
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={onClose}>
              关闭
            </button>
            <button type="button" disabled={start.isPending} onClick={() => start.mutate()}>
              启动授权
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="oauth-state">
            状态:
            {operation !== undefined ? (
              <StatusBadge status={oauthStateBadge(operation.state)}>{operation.state}</StatusBadge>
            ) : (
              "查询中…"
            )}
            {operation?.state === "pending" && expiresIn !== undefined ? (
              <span className="mono" style={{ marginLeft: 8, color: "var(--ink-2)" }}>
                {expiresIn}s 后过期
              </span>
            ) : null}
          </p>
          {operation?.state === "pending" ? (
            <p style={{ color: "var(--ink-2)", fontSize: 13 }}>每 2 秒轮询一次(页签隐藏时暂停)。</p>
          ) : null}
          <div className="sheet-actions">
            {operation?.state === "pending" ? (
              <button
                type="button"
                className="danger"
                disabled={cancel.isPending}
                onClick={() => cancel.mutate()}
              >
                取消授权
              </button>
            ) : null}
            {operation?.state === "failed" || operation?.state === "cancelled" ? (
              <button type="button" className="secondary" disabled={start.isPending} onClick={() => start.mutate()}>
                重新启动
              </button>
            ) : null}
            <button type="button" onClick={onClose}>
              {operation?.state === "complete" ? "完成" : "关闭"}
            </button>
          </div>
        </>
      )}
    </Sheet>
  );
}
