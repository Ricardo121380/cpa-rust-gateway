import { AccountEvidenceTabs } from "../accounts/AccountEvidenceTabs";
import { useVersionStore } from "../config-versions/versionStore";
import { ResourceIdentity, IdentityDetails } from "../../components/ResourceIdentity";
// Shared account inspector for complete inventory and runtime projections.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { OAuthWizard } from "./OAuthWizard";

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
  plan,
  onClose,
}: Readonly<{ credentialId: string; accountName?: string; providerName?: string; plan?:string|null; onClose: () => void }>) {
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

  if (oauthOpen) {
    return <OAuthWizard credentialId={credentialId} accountName={accountName ?? metadata.data?.email ?? undefined} onClose={() => setOauthOpen(false)} />;
  }

  const row = credential.data;
  const meta = metadata.data;
  const present = META_FIELDS.map((field) => ({ ...field, value: meta?.[field.key] ?? null })).filter(
    (field) => field.value !== null && field.value !== "",
  );
  // oauth_json is the normalized Codex envelope accepted by the real refresh endpoint.
  // Other provider credentials must not be sent to the Codex OAuth workflow.
  const isOAuth = row?.kind === "oauth_json";

  return (
    <Sheet title="账号详情" layout="inspector" onEscape={onClose}>
      <h3>{accountName ?? meta?.email ?? "未提供账号身份"}</h3>
      {credential.isError?<p role="alert">{asAppError(credential.error).message}</p>:null}
      <AccountEvidenceTabs accountId={credentialId} onNavigate={onClose} overview={<dl className="fact-grid"><dt>渠道</dt><dd>{providerName??"未观测"}</dd><dt>状态</dt><dd>{row?<StatusBadge status={row.status}>{row.status==="active"?"已启用":row.status==="disabled"?"已停用":row.status}</StatusBadge>:"读取中"}</dd><dt>套餐</dt><dd>{plan??meta?.plan??"未观测"}</dd><dt>授权资料</dt><dd>{row?.secret_present?"已保存":"未观测"}</dd></dl>} configuration={<>
      <IdentityDetails entries={[["账号", credentialId, accountName ?? meta?.email ?? "未提供账号身份"], ...(row ? [["提供商", row.upstream_id, providerName] as const] : [])]} />
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
              <td>{providerName ? "渠道" : "上游"}</td>
              <td>{providerName ?? <ResourceIdentity id={row.upstream_id} kind="upstream" />}</td>
            </tr>
            <tr>
              <td>类型</td>
              <td className="mono">{row.kind}</td>
            </tr>
            <tr>
              <td>状态</td>
              <td>
                <StatusBadge status={row.status}>{row.status==="active"?"已启用":row.status==="disabled"?"已停用":row.status}</StatusBadge>
              </td>
            </tr>
            <tr>
              <td>修订</td>
              <td className="mono">{rotated ?? row.revision}</td>
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

      <h4>元数据</h4>
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
                <td className="mono">{field.value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {rotated !== undefined ? (
        <p className="action-notice">
          令牌已轮换,凭据修订推进至 <span className="mono">{rotated}</span>。
        </p>
      ) : null}

      <div className="sheet-actions">
        {isOAuth ? (
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
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </div>
      </>}/>
    </Sheet>
  );
}
