import { AccountEvidenceTabs } from "../accounts/AccountEvidenceTabs";
import { useVersionStore } from "../config-versions/versionStore";
import { IdentityDetails } from "../../components/ResourceIdentity";
// Shared account inspector for complete inventory and runtime projections.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { OAuthWizard } from "./OAuthWizard";
import { KimiDeviceDialog } from "../accounts/KimiDeviceDialog";

type Credential = Readonly<{
  id: string;
  upstream_id: string;
  kind: string;
  status: string;
  revision: number;
  secret_present: boolean;
}>;

type CredentialMetadata = Readonly<{
  credential_id: string;
  kind: string;
  revision: number;
  plan?: string | null;
  quota?: string | null;
  platform?: string | null;
  email?: string | null;
  source_format?: "cpa" | "sub2api" | "direct_oauth" | null;
}>;

const META_FIELDS = [
  { key: "platform", label: "平台" },
  { key: "email", label: "账号" },
  { key: "plan", label: "套餐" },
  { key: "quota", label: "配额" },
  { key: "source_format", label: "来源格式" },
] as const;

export function CredentialSheet({
  credentialId,
  accountName,
  providerName,
  category,
  plan,
  onClose,
}: Readonly<{ credentialId: string; accountName?: string; providerName?: string; category?: "codex"|"kimi"; plan?:string|null; onClose: () => void }>) {
  const queryClient = useQueryClient();
  const scope=useVersionStore(s=>s.context?.configVersionId);
  const [oauthOpen, setOauthOpen] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [rotated, setRotated] = useState<number | undefined>();
  useEffect(()=>{setRotated(undefined);setError(undefined);setOauthOpen(false);},[scope,credentialId]);

  const credential = useQuery({
    queryKey: ["credential", scope, credentialId],
    enabled:!!scope,
    queryFn: () =>
      call<Credential>(
        "getCredential",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
  });

  const metadata = useQuery({
    queryKey: ["credential-metadata", scope, credentialId],
    enabled:!!scope,
    queryFn: () =>
      call<CredentialMetadata>(
        "getCredentialMetadata",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
    retry: false,
  });

  const refresh = useMutation({
    mutationFn: () =>
      call<Readonly<{ state: string; revision: number }>>(
        "refreshCredentialOAuth",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
    onSuccess: (operation) => {
      setError(undefined);
      setRotated(operation.revision);
      void queryClient.invalidateQueries({ queryKey: ["credential", scope, credentialId] });
      void queryClient.invalidateQueries({ queryKey: ["credential-metadata", scope, credentialId] });
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const row = credential.data;
  // `oauth_json` is storage shape, not a provider identity. Only the account inventory's
  // server-projected category may select a channel-specific reauthorization flow.
  const isKimiOAuth = row?.kind === "oauth_json" && category === "kimi";
  if (oauthOpen && isKimiOAuth) {
    return <KimiDeviceDialog credentialId={credentialId} providerId={row?.upstream_id} onClose={() => setOauthOpen(false)} onComplete={() => setOauthOpen(false)} />;
  }
  if (oauthOpen) {
    return <OAuthWizard credentialId={credentialId} accountName={accountName ?? metadata.data?.email ?? undefined} onClose={() => setOauthOpen(false)} />;
  }

  const meta = metadata.data;
  const present = META_FIELDS.map((field) => ({ ...field, value: meta?.[field.key] ?? null })).filter(
    (field) => field.value !== null && field.value !== "",
  );
  // Unknown context fails closed: it must never send a Kimi or foreign OAuth envelope to Codex.
  const isCodexOAuth = row?.kind === "oauth_json" && category === "codex";
  const authenticationLabel=isKimiOAuth?"Kimi Coding 授权":isCodexOAuth?"Codex / ChatGPT 授权":row?.kind==="bearer"?"API Key / Token":"已保存渠道凭据";

  return (
    <Sheet title="账号详情" layout="inspector" onEscape={onClose}>
      <h3>{accountName ?? meta?.email ?? "未提供账号身份"}</h3>
      {credential.isError?<p role="alert">{asAppError(credential.error).message}</p>:null}
      <AccountEvidenceTabs accountId={credentialId} onNavigate={onClose} overview={<dl className="fact-grid"><dt>渠道</dt><dd>{providerName??"未观测"}</dd><dt>状态</dt><dd>{row?<StatusBadge status={row.status}>{row.status==="active"?"已启用":row.status==="disabled"?"已停用":row.status}</StatusBadge>:"读取中"}</dd><dt>套餐</dt><dd>{plan??meta?.plan??"未观测"}</dd><dt>授权资料</dt><dd>{row?.secret_present?"已保存":"未观测"}</dd></dl>} configuration={<>
      <h4>授权配置</h4>
      {error !== undefined ? (
        <p role="alert" className="reveal-warning">
          {error}
        </p>
      ) : null}

      {credential.isError ? (
        <p role="alert" className="reveal-warning">
          {asAppError(credential.error).message}
        </p>
      ) : row === undefined ? (
        <p className="muted">读取凭据…</p>
      ) : (
        <table>
          <tbody>
            <tr>
              <td>渠道</td>
              <td>{providerName ?? "未观测渠道"}</td>
            </tr>
            <tr>
              <td>接入方式</td>
              <td>{authenticationLabel}</td>
            </tr>
            <tr>
              <td>状态</td>
              <td>
                <StatusBadge status={row.status}>{row.status==="active"?"已启用":row.status==="disabled"?"已停用":row.status}</StatusBadge>
              </td>
            </tr>
            <tr>
              <td>秘密</td>
              <td>
                {row.secret_present ? (
                  <span className="badge badge-good">已配置</span>
                ) : (
                  <span className="badge badge-warn">缺失</span>
                )}
              </td>
            </tr>
          </tbody>
        </table>
      )}

      <h4>账号资料</h4>
      {metadata.isError ? (
        <p className="muted small">
          该渠道暂未返回账号元数据。
        </p>
      ) : present.length === 0 ? (
        <p className="muted small">
          尚无账号、套餐或额度观测。
        </p>
      ) : (
        <table>
          <tbody>
            {present.map((field) => (
              <tr key={field.key}>
                <td>{field.label}</td>
                <td>{field.value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {rotated !== undefined ? (
        <p className="action-notice">
          令牌已轮换并已从服务端重新读取。
        </p>
      ) : null}

      <IdentityDetails entries={[["账号", credentialId, accountName ?? meta?.email ?? "未提供账号身份"], ...(row ? [["提供商", row.upstream_id, providerName] as const] : [])]} />

      <div className="sheet-actions">
        {isCodexOAuth ? (
          <>
            <button
              type="button"
              className="secondary"
              disabled={refresh.isPending}
              onClick={() => refresh.mutate()}
            >
              {refresh.isPending ? "轮换中…" : "轮换令牌"}
            </button>
            <button type="button" className="secondary" onClick={() => setOauthOpen(true)}>
              重新授权
            </button>
          </>
        ) : null}
        {isKimiOAuth ? <button type="button" className="secondary" onClick={() => setOauthOpen(true)}>重新授权</button> : null}
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </div>
      </>}/>
    </Sheet>
  );
}
