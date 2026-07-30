// Access control: groups + client keys. Signature safety flow lives here —
// the reveal-once sheet (docs/07 §6.4): the full rgw_ key exists only in the
// 201 issue response; closing the sheet erases it from memory permanently.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import {
  displayKeyStatus,
  formatExpiry,
  type AccessGroupRecord,
  type ClientKeyRecord,
  type IssuedClientKey,
} from "./model";

export function AccessPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const [issuing, setIssuing] = useState(false);
  const [issued, setIssued] = useState<IssuedClientKey | undefined>();
  const [copied, setCopied] = useState(false);
  const [actionError, setActionError] = useState<string | undefined>();
  const [confirmRevoke, setConfirmRevoke] = useState<string | undefined>();

  const scope = context?.configVersionId;

  const groups = useQuery({
    queryKey: ["access-groups", scope],
    queryFn: () => call<AccessGroupRecord[]>("listAccessGroups", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const keys = useQuery({
    queryKey: ["client-keys", scope],
    queryFn: () => call<ClientKeyRecord[]>("listClientKeys", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const issue = useMutation({
    gcTime: 0, // never cache a response carrying the one-time key
    mutationFn: (input: {
      id: string;
      access_group_id: string;
      status: "active";
      expires_at_ms: number | null;
    }) =>
      call<IssuedClientKey>(
        "issueClientKey",
        { body: input },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: (record) => {
      setIssuing(false);
      setIssued(record);
      setCopied(false);
      void queryClient.invalidateQueries({ queryKey: ["client-keys", scope] });
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const revoke = useMutation({
    mutationFn: (id: string) =>
      call<undefined>(
        "revokeClientKey",
        { path: { client_key_id: id } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setConfirmRevoke(undefined);
      void queryClient.invalidateQueries({ queryKey: ["client-keys", scope] });
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  function onIssueSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const expiresRaw = String(data.get("expires_at") ?? "");
    issue.mutate({
      id: String(data.get("id") ?? ""),
      access_group_id: String(data.get("access_group_id") ?? ""),
      status: "active",
      expires_at_ms: expiresRaw === "" ? null : new Date(expiresRaw).getTime(),
    });
  }

  function closeReveal() {
    setIssued(undefined); // the key is gone for good — by design
    setCopied(false);
  }

  if (scope === undefined) {
    return (
      <section>
        <h2>{t.nav.access}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本。</p>
        </div>
      </section>
    );
  }

  const nowMs = Date.now();

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.access}</h2>
        <div className="page-actions">
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => setIssuing(true)}
          >
            签发 Client Key
          </button>
        </div>
      </header>

      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <div className="card tablewrap">
        <h3>访问组</h3>
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>名称</th>
              <th>状态</th>
              <th>限制</th>
            </tr>
          </thead>
          <tbody>
            {(groups.data ?? []).map((group) => (
              <tr key={group.id}>
                <td className="mono">{group.id}</td>
                <td>{group.name}</td>
                <td>
                  <StatusBadge status={group.status} />
                </td>
                <td className="mono">
                  {Object.entries(group.limits)
                    .map(([key, value]) => `${key}=${value}`)
                    .join(" ") || "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {groups.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      <div className="card tablewrap">
        <h3>Client Key(仅前缀,完整密钥永不回显)</h3>
        <table>
          <thead>
            <tr>
              <th>前缀</th>
              <th>访问组</th>
              <th>状态</th>
              <th>过期</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(keys.data ?? []).map((record) => {
              const status = displayKeyStatus(record, nowMs);
              return (
                <tr key={record.id}>
                  <td className="mono">{record.prefix}</td>
                  <td className="mono">{record.access_group_id}</td>
                  <td>
                    <StatusBadge status={status} />
                  </td>
                  <td className="mono">{formatExpiry(record.expires_at_ms)}</td>
                  <td className="row-actions">
                    {record.status === "active" ? (
                      <button
                        type="button"
                        className="danger"
                        disabled={!editable}
                        onClick={() => setConfirmRevoke(record.id)}
                      >
                        吊销
                      </button>
                    ) : (
                      "—"
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {keys.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      {issuing ? (
        <Sheet title="签发 Client Key" onEscape={() => setIssuing(false)}>
          <form className="sheet-form" onSubmit={onIssueSubmit}>
            <label>
              Key ID
              <input name="id" className="mono" required maxLength={128} />
            </label>
            <label>
              访问组
              <select name="access_group_id" required>
                {(groups.data ?? []).map((group) => (
                  <option key={group.id} value={group.id}>
                    {group.name}({group.id})
                  </option>
                ))}
              </select>
            </label>
            <label>
              过期时间(可空 = 永不过期)
              <input name="expires_at" type="datetime-local" />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setIssuing(false)}>
                取消
              </button>
              <button type="submit" disabled={issue.isPending}>
                签发
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {issued !== undefined ? (
        <Sheet title="Client Key 已签发 — 只显示这一次">
          <p className="reveal-warning">关闭此窗口后,完整密钥将永远无法再次查看。</p>
          <code className="reveal-key mono">{issued.key}</code>
          <div className="sheet-actions">
            <button
              type="button"
              className="secondary"
              onClick={() => {
                void navigator.clipboard.writeText(issued.key).then(() => setCopied(true));
              }}
            >
              {copied ? "已复制 ✓" : "复制"}
            </button>
            <button type="button" onClick={closeReveal}>
              我已保存,关闭
            </button>
          </div>
        </Sheet>
      ) : null}

      {confirmRevoke !== undefined ? (
        <Sheet title="确认吊销" onEscape={() => setConfirmRevoke(undefined)}>
          <p>
            吊销 <span className="mono">{confirmRevoke}</span> 不可逆:记录保留、状态转为
            revoked,该 Key 立即不能再认证。
          </p>
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setConfirmRevoke(undefined)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={revoke.isPending}
              onClick={() => revoke.mutate(confirmRevoke)}
            >
              确认吊销
            </button>
          </div>
        </Sheet>
      ) : null}
    </section>
  );
}
