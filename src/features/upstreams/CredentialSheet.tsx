// Credential detail — the production-reachable home for a credential.
//
// Until G1 lands there is no listCredentials, so the only enumeration is the
// runtime's own projections (runtime/availability and catalog/status both
// carry credential_id). That is where this sheet is opened from, and it is
// also its limit: credentials that exist but the runtime has never observed do
// not appear anywhere yet.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
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
  onClose,
}: Readonly<{ credentialId: string; onClose: () => void }>) {
  const queryClient = useQueryClient();
  const [oauthOpen, setOauthOpen] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [rotated, setRotated] = useState<number | undefined>();

  const credential = useQuery({
    queryKey: ["credential", credentialId],
    queryFn: () =>
      call<Credential>(
        "getCredential",
        { path: { credential_id: credentialId } },
        { versionScoped: true },
      ),
  });

  const metadata = useQuery({
    queryKey: ["credential-metadata", credentialId],
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
      void queryClient.invalidateQueries({ queryKey: ["credential", credentialId] });
      void queryClient.invalidateQueries({ queryKey: ["credential-metadata", credentialId] });
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  if (oauthOpen) {
    return <OAuthWizard credentialId={credentialId} onClose={() => setOauthOpen(false)} />;
  }

  const row = credential.data;
  const meta = metadata.data;
  const present = META_FIELDS.map((field) => ({ ...field, value: meta?.[field.key] ?? null })).filter(
    (field) => field.value !== null && field.value !== "",
  );
  const isOAuth = row?.kind === "oauth";

  return (
    <Sheet title={`凭据 · ${credentialId}`} onEscape={onClose}>
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
              <td>上游</td>
              <td className="mono">{row.upstream_id}</td>
            </tr>
            <tr>
              <td>类型</td>
              <td className="mono">{row.kind}</td>
            </tr>
            <tr>
              <td>状态</td>
              <td>
                <StatusBadge status={row.status} />
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
          网关未提供该凭据的元数据(<span className="mono">getCredentialMetadata</span> 不可用)。
        </p>
      ) : present.length === 0 ? (
        <p className="muted small">
          网关没有记录平台、账号、套餐或配额 —— 这些字段在契约里全部可空。
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
    </Sheet>
  );
}
