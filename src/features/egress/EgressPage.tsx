// Egress policies: allowlist-based SSRF boundary (docs/07 §7.7).
// PATCH is full-replacement (C11) — the edit sheet always loads and submits
// the complete EgressPolicyInput.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ChipsInput } from "../../components/ChipsInput";
import { Sheet } from "../../components/Sheet";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import {
  normalizedMaxRedirects,
  referencingUpstreams,
  validateHostEntry,
  validatePortEntry,
  type EgressPolicy,
} from "./model";

type UpstreamSummary = Readonly<{ id: string; egress_policy_id?: string | null }>;

type DraftPolicy = {
  id: string;
  name: string;
  hosts: string[];
  ports: string[];
  cidrs: string[];
  redirect_mode: "deny" | "revalidate";
  max_redirects: number;
  isNew: boolean;
};

function emptyDraft(): DraftPolicy {
  return {
    id: "",
    name: "",
    hosts: [],
    ports: ["443"],
    cidrs: [],
    redirect_mode: "deny",
    max_redirects: 0,
    isNew: true,
  };
}

function toDraft(policy: EgressPolicy): DraftPolicy {
  return {
    id: policy.id,
    name: policy.name,
    hosts: [...policy.allowed_hosts],
    ports: policy.allowed_ports.map(String),
    cidrs: [...policy.allowed_cidrs],
    redirect_mode: policy.redirect_mode,
    max_redirects: policy.max_redirects,
    isNew: false,
  };
}

function toInput(draft: DraftPolicy) {
  return {
    id: draft.id,
    name: draft.name,
    allowed_schemes: ["https"],
    allowed_hosts: draft.hosts,
    allowed_ports: draft.ports.map(Number),
    allowed_cidrs: draft.cidrs,
    redirect_mode: draft.redirect_mode,
    max_redirects: normalizedMaxRedirects(draft.redirect_mode, draft.max_redirects),
  };
}

export function EgressPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [draft, setDraft] = useState<DraftPolicy | undefined>();
  const [confirmDelete, setConfirmDelete] = useState<EgressPolicy | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const policies = useQuery({
    queryKey: ["egress", scope],
    queryFn: () => call<EgressPolicy[]>("listEgressPolicies", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });
  const upstreams = useQuery({
    queryKey: ["upstreams", scope],
    queryFn: () => call<UpstreamSummary[]>("listUpstreams", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["egress", scope] });
    void queryClient.invalidateQueries({ queryKey: ["upstreams", scope] });
  };

  const save = useMutation({
    mutationFn: (input: DraftPolicy) =>
      input.isNew
        ? call<EgressPolicy>("createEgressPolicy", { body: toInput(input) }, { versionScoped: true, mutating: true })
        : call<EgressPolicy>(
            "updateEgressPolicy",
            { path: { egress_policy_id: input.id }, body: toInput(input) },
            { versionScoped: true, mutating: true },
          ),
    onSuccess: () => {
      setDraft(undefined);
      invalidate();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const remove = useMutation({
    mutationFn: (id: string) =>
      call<undefined>(
        "deleteEgressPolicy",
        { path: { egress_policy_id: id } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setConfirmDelete(undefined);
      invalidate();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (draft !== undefined) {
      save.mutate(draft);
    }
  }

  if (scope === undefined) {
    return (
      <section>
        <h2>{t.nav.egress}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本。</p>
        </div>
      </section>
    );
  }

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.egress}</h2>
        <div className="page-actions">
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => setDraft(emptyDraft())}
          >
            新建出口策略
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
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>名称</th>
              <th>主机</th>
              <th>端口</th>
              <th>重定向</th>
              <th>被引用</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(policies.data ?? []).map((policy) => {
              const refs = referencingUpstreams(policy.id, upstreams.data ?? []);
              return (
                <tr key={policy.id}>
                  <td className="mono">{policy.id}</td>
                  <td>{policy.name}</td>
                  <td className="mono">{policy.allowed_hosts.length} 条</td>
                  <td className="mono">{policy.allowed_ports.join(", ")}</td>
                  <td className="mono">
                    {policy.redirect_mode}
                    {policy.redirect_mode === "revalidate" ? ` ≤${policy.max_redirects}` : ""}
                  </td>
                  <td className="mono">{refs.length > 0 ? refs.join(", ") : "—"}</td>
                  <td className="row-actions">
                    <button
                      type="button"
                      className="secondary"
                      disabled={!editable}
                      onClick={() => setDraft(toDraft(policy))}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={!editable}
                      onClick={() => setConfirmDelete(policy)}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {policies.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      {draft !== undefined ? (
        <Sheet
          title={draft.isNew ? "新建出口策略" : `编辑 ${draft.id}`}
          onEscape={() => setDraft(undefined)}
        >
          <form className="sheet-form" onSubmit={onSubmit}>
            {draft.isNew ? (
              <label>
                策略 ID
                <input
                  className="mono"
                  required
                  maxLength={128}
                  value={draft.id}
                  onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                />
              </label>
            ) : null}
            <label>
              名称
              <input
                required
                maxLength={256}
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              />
            </label>
            <label>
              允许主机(精确域名,回车添加)
              <ChipsInput
                value={draft.hosts}
                onChange={(hosts) => setDraft({ ...draft, hosts })}
                placeholder="relay.example.com"
                validate={validateHostEntry}
              />
            </label>
            <label>
              允许端口
              <ChipsInput
                value={draft.ports}
                onChange={(ports) => setDraft({ ...draft, ports })}
                placeholder="443"
                validate={validatePortEntry}
              />
            </label>
            <label>
              允许 CIDR(可空)
              <ChipsInput
                value={draft.cidrs}
                onChange={(cidrs) => setDraft({ ...draft, cidrs })}
                placeholder="203.0.113.0/24"
              />
            </label>
            <label>
              重定向模式
              <select
                value={draft.redirect_mode}
                onChange={(event) => {
                  const mode = event.target.value as DraftPolicy["redirect_mode"];
                  setDraft({
                    ...draft,
                    redirect_mode: mode,
                    max_redirects: normalizedMaxRedirects(mode, draft.max_redirects || 1),
                  });
                }}
              >
                <option value="deny">deny(拒绝一切重定向)</option>
                <option value="revalidate">revalidate(重定向目标重新过策略)</option>
              </select>
            </label>
            <label>
              最大重定向次数{draft.redirect_mode === "deny" ? "(deny 模式锁定为 0)" : "(1-5)"}
              <input
                type="number"
                min={draft.redirect_mode === "deny" ? 0 : 1}
                max={draft.redirect_mode === "deny" ? 0 : 5}
                disabled={draft.redirect_mode === "deny"}
                value={normalizedMaxRedirects(draft.redirect_mode, draft.max_redirects)}
                onChange={(event) =>
                  setDraft({ ...draft, max_redirects: Number(event.target.value) })
                }
              />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setDraft(undefined)}>
                取消
              </button>
              <button type="submit" disabled={save.isPending || draft.hosts.length === 0}>
                保存
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDelete !== undefined ? (
        <Sheet title="确认删除" onEscape={() => setConfirmDelete(undefined)}>
          <p>
            删除 <span className="mono">{confirmDelete.id}</span> 后,引用它的上游的
            egress_policy_id 将被清空(不会级联删除上游)。
          </p>
          {referencingUpstreams(confirmDelete.id, upstreams.data ?? []).length > 0 ? (
            <p className="reveal-warning">
              当前被引用:{referencingUpstreams(confirmDelete.id, upstreams.data ?? []).join(", ")}
            </p>
          ) : null}
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setConfirmDelete(undefined)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={remove.isPending}
              onClick={() => remove.mutate(confirmDelete.id)}
            >
              确认删除
            </button>
          </div>
        </Sheet>
      ) : null}
    </section>
  );
}
