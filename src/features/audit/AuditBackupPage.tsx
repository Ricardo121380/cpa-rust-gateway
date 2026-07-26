// Audit trail (append-only, read-only UI) + backup preflight.
// Restore upload is deliberately deferred: the flow only succeeds into an
// absent target DB — a live panel session can never satisfy that, so the UI
// documents the operator-side CLI path instead of faking a button.
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { StatusBadge } from "../../components/StatusBadge";
import { messages } from "../../i18n/messages";

type AuditEvent = Readonly<{
  id: number;
  action: string;
  actor: string;
  occurred_at_ms: number;
  config_version_id: string;
  replaced_config_version_id?: string | null;
}>;

type BackupPreflight = Readonly<{ schema_version: number; secret_key_required: boolean }>;

const ACTION_STATUS: Record<string, string> = {
  config_created: "draft",
  config_published: "active",
  config_rolled_back: "recovery_required",
};

function formatTime(ms: number): string {
  return new Date(ms).toISOString().replace("T", " ").slice(0, 19);
}

export function AuditBackupPage() {
  const [preflight, setPreflight] = useState<BackupPreflight | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const events = useQuery({
    queryKey: ["audit-events"],
    queryFn: () => call<AuditEvent[]>("listManagementAuditEvents"),
    refetchInterval: 60_000,
  });

  const runPreflight = useMutation({
    mutationFn: () => call<BackupPreflight>("previewBackup"),
    onSuccess: setPreflight,
    onError: (error) => setActionError(asAppError(error).message),
  });

  return (
    <section>
      <h2>{messages.nav.audit}</h2>

      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <div className="card tablewrap">
        <h3>配置生命周期审计(append-only)</h3>
        <table>
          <thead>
            <tr>
              <th>#</th>
              <th>动作</th>
              <th>执行者</th>
              <th>时间</th>
              <th>配置版本</th>
              <th>被替换版本</th>
            </tr>
          </thead>
          <tbody>
            {[...(events.data ?? [])]
              .sort((a, b) => b.id - a.id)
              .map((event) => (
                <tr key={event.id}>
                  <td className="mono">{event.id}</td>
                  <td>
                    <StatusBadge status={ACTION_STATUS[event.action] ?? "archived"}>
                      {event.action}
                    </StatusBadge>
                  </td>
                  <td className="mono">{event.actor}</td>
                  <td className="mono">{formatTime(event.occurred_at_ms)}</td>
                  <td className="mono">{event.config_version_id}</td>
                  <td className="mono">{event.replaced_config_version_id ?? "—"}</td>
                </tr>
              ))}
          </tbody>
        </table>
        {events.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{messages.state.empty}</p>
          </div>
        ) : null}
      </div>

      <div className="card" style={{ marginTop: 14 }}>
        <h3>备份</h3>
        <p style={{ color: "var(--ink-2)", fontSize: 13 }}>
          备份工件由运维侧 CLI 生成(设计如此,面板无下载端点);此处仅做源库预检。
          恢复只能进入<strong>空目标库</strong>,须由部署侧执行 —— 详见
          <span className="mono"> docs/07 §7.6</span>。
        </p>
        <div className="page-actions" style={{ marginTop: 8 }}>
          <button type="button" disabled={runPreflight.isPending} onClick={() => runPreflight.mutate()}>
            源库备份预检
          </button>
        </div>
        {preflight !== undefined ? (
          <p style={{ marginTop: 12, fontSize: 13 }}>
            schema 版本:<span className="mono">{preflight.schema_version}</span> ·
            {preflight.secret_key_required
              ? " 恢复时需要独立备份密钥(与凭据 Master Key 分离)"
              : " 无需备份密钥"}
          </p>
        ) : null}
      </div>
    </section>
  );
}
