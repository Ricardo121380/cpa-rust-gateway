// Credential OAuth wizard — drives the real contract ops:
// startCredentialOAuth / getCredentialOAuthStatus / completeCredentialOAuth /
// cancelCredentialOAuth.
//
// This is an authorization-code flow, not a device flow. The gateway returns
// `authorization_url` on start; the operator opens it, authorizes, and the
// provider redirects to a callback address carrying `state` plus `code` or
// `error`. Pasting that address back completes the operation.
//
// The previous version modelled only {credential_id, state, expires_at_ms}: it
// started a flow, never showed the authorization URL, and had no completion
// call — so it could only ever sit at "pending". Nobody hit the dead end
// because its one mount point was behind the G1 fixture.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import {
  oauthFailureLabel,
  oauthPollIntervalMs,
  oauthStateBadge,
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

export function OAuthWizard({
  credentialId,
  onClose,
}: Readonly<{ credentialId: string; onClose: () => void }>) {
  const queryClient = useQueryClient();
  const [started, setStarted] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [pasted, setPasted] = useState("");
  const [pasteError, setPasteError] = useState<string | undefined>();

  const statusKey = ["oauth-status", credentialId];
  const status = useQuery({
    queryKey: statusKey,
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
      setError(undefined);
      setPasted("");
      setPasteError(undefined);
      queryClient.setQueryData(statusKey, operation);
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const complete = useMutation({
    mutationFn: (body: unknown) =>
      call<OAuthOperation>(
        "completeCredentialOAuth",
        { path: { credential_id: credentialId }, body },
        { versionScoped: true },
      ),
    onSuccess: (operation) => queryClient.setQueryData(statusKey, operation),
    onError: (cause) => setError(asAppError(cause).message),
  });

  const cancel = useMutation({
    mutationFn: () =>
      call<undefined>(
        "cancelCredentialOAuth",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: statusKey }),
    onError: (cause) => setError(asAppError(cause).message),
  });

  const operation = status.data;
  const expiresIn =
    operation?.expires_at_ms != null
      ? Math.max(0, Math.round((operation.expires_at_ms - Date.now()) / 1000))
      : undefined;
  const authorizeUrl = safeExternalUrl(operation?.authorization_url);
  const pending = operation?.state === "pending";

  function submitCallback(): void {
    const parsed = parseOAuthCallback(pasted);
    if (!parsed.ok) {
      setPasteError(parsed.reason);
      return;
    }
    setPasteError(undefined);
    setError(undefined);
    complete.mutate(parsed.input);
  }

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
            启动后网关会生成一个授权地址。在浏览器里完成授权,provider 会跳转回一个回调地址
            —— 把那条地址整个粘回来即可完成。
            <br />
            <small className="muted">
              授权码流:令牌交换发生在网关侧,本页只传递 state 与 code。
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
            {pending && expiresIn !== undefined ? (
              <span className="mono muted">{expiresIn}s 后过期</span>
            ) : null}
          </p>

          {operation?.failure_class != null ? (
            <p role="alert" className="reveal-warning">
              {oauthFailureLabel(operation.failure_class)}
              <br />
              <small className="mono">{operation.failure_class}</small>
            </p>
          ) : null}

          {pending ? (
            <>
              <h4>1 · 打开授权地址</h4>
              {authorizeUrl !== undefined ? (
                <p>
                  <a href={authorizeUrl} target="_blank" rel="noreferrer noopener">
                    在新标签页打开授权页 →
                  </a>
                  <br />
                  <small className="mono oauth-url">{authorizeUrl}</small>
                </p>
              ) : (
                <p className="muted small">
                  网关这次没有返回可用的授权地址(
                  <span className="mono">authorization_url</span> 缺失,或不是 http/https)。
                  取消后重试,或检查网关侧的 provider 配置。
                </p>
              )}

              <h4>2 · 粘回回调地址</h4>
              <p className="muted small">
                授权后浏览器会跳转到一个回调地址 —— 通常指向本机,页面很可能打不开。
                地址栏里的那一整条就是需要的内容。
              </p>
              <textarea
                aria-label="回调地址"
                rows={3}
                className="oauth-callback-input"
                value={pasted}
                placeholder="http://127.0.0.1:8085/callback?code=…&state=…"
                onChange={(event) => {
                  setPasted(event.target.value);
                  setPasteError(undefined);
                }}
              />
              {pasteError !== undefined ? (
                <p role="alert" className="reveal-warning">
                  {pasteError}
                </p>
              ) : null}
              <p className="muted small">
                若网关自己收到了回调,上面的状态会自行变为 complete —— 那就不必粘贴。
              </p>
            </>
          ) : null}

          <div className="sheet-actions">
            {pending ? (
              <>
                <button
                  type="button"
                  className="danger"
                  disabled={cancel.isPending}
                  onClick={() => cancel.mutate()}
                >
                  取消授权
                </button>
                <button
                  type="button"
                  disabled={complete.isPending || pasted.trim().length === 0}
                  onClick={submitCallback}
                >
                  {complete.isPending ? "完成中…" : "完成授权"}
                </button>
              </>
            ) : null}
            {operation?.state === "failed" ||
            operation?.state === "cancelled" ||
            operation?.state === "expired" ? (
              <button
                type="button"
                className="secondary"
                disabled={start.isPending}
                onClick={() => start.mutate()}
              >
                重新启动
              </button>
            ) : null}
            <button type="button" className={pending ? "secondary" : undefined} onClick={onClose}>
              {operation?.state === "complete" ? "完成" : "关闭"}
            </button>
          </div>
        </>
      )}
    </Sheet>
  );
}
