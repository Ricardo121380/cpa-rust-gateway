// Upstream top-level CRUD. Child resources (endpoints / credentials /
// bindings) need the G1 graph projection — owned by the backend session —
// so their panel renders an honest "waiting for contract" state, not a fake.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ChipsInput } from "../../components/ChipsInput";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import type { EgressPolicy } from "../egress/model";
import { SubresourcePanel } from "./SubresourcePanel";

type Upstream = Readonly<{
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  tags: readonly string[];
  egress_policy_id?: string | null;
}>;

const KIND_SUGGESTIONS = [
  "grok.official",
  "grok.build",
  "grok.web",
  "kiro",
  "openai-compatible",
  "anthropic-compatible",
];

type DraftUpstream = {
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  tags: string[];
  egress_policy_id: string;
  isNew: boolean;
};

function emptyDraft(): DraftUpstream {
  return { id: "", name: "", kind: "", enabled: true, tags: [], egress_policy_id: "", isNew: true };
}

function toDraft(upstream: Upstream): DraftUpstream {
  return {
    id: upstream.id,
    name: upstream.name,
    kind: upstream.kind,
    enabled: upstream.enabled,
    tags: [...upstream.tags],
    egress_policy_id: upstream.egress_policy_id ?? "",
    isNew: false,
  };
}

function toInput(draft: DraftUpstream) {
  return {
    id: draft.id,
    name: draft.name,
    kind: draft.kind,
    enabled: draft.enabled,
    tags: draft.tags,
    egress_policy_id: draft.egress_policy_id === "" ? null : draft.egress_policy_id,
  };
}

export function UpstreamsPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [draft, setDraft] = useState<DraftUpstream | undefined>();
  const [confirmDelete, setConfirmDelete] = useState<Upstream | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const upstreams = useQuery({
    queryKey: ["upstreams", scope],
    queryFn: () => call<Upstream[]>("listUpstreams", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });
  const policies = useQuery({
    queryKey: ["egress", scope],
    queryFn: () => call<EgressPolicy[]>("listEgressPolicies", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const invalidate = () => void queryClient.invalidateQueries({ queryKey: ["upstreams", scope] });

  const save = useMutation({
    mutationFn: (input: DraftUpstream) =>
      input.isNew
        ? call<Upstream>("createUpstream", { body: toInput(input) }, { versionScoped: true, mutating: true })
        : call<Upstream>(
            "updateUpstream",
            { path: { upstream_id: input.id }, body: toInput(input) },
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
      call<undefined>("deleteUpstream", { path: { upstream_id: id } }, { versionScoped: true, mutating: true }),
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
        <h2>{t.nav.upstreams}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本。</p>
        </div>
      </section>
    );
  }

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.upstreams}</h2>
        <div className="page-actions">
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => setDraft(emptyDraft())}
          >
            新建上游
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
              <th>Provider 家族</th>
              <th>状态</th>
              <th>标签</th>
              <th>出口策略</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(upstreams.data ?? []).map((upstream) => (
              <tr key={upstream.id}>
                <td className="mono">{upstream.id}</td>
                <td>{upstream.name}</td>
                <td className="mono">{upstream.kind}</td>
                <td>
                  <StatusBadge status={upstream.enabled ? "active" : "disabled"}>
                    {upstream.enabled ? "enabled" : "disabled"}
                  </StatusBadge>
                </td>
                <td>
                  {upstream.tags.length > 0
                    ? upstream.tags.map((tag) => (
                        <span key={tag} className="idchip">
                          {tag}
                        </span>
                      ))
                    : "—"}
                </td>
                <td className="mono">{upstream.egress_policy_id ?? "—"}</td>
                <td className="row-actions">
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => setExpanded(expanded === upstream.id ? undefined : upstream.id)}
                  >
                    {expanded === upstream.id ? "收起" : "子资源"}
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setDraft(toDraft(upstream))}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={!editable}
                    onClick={() => setConfirmDelete(upstream)}
                  >
                    删除
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {upstreams.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      {expanded !== undefined ? <SubresourcePanel upstreamId={expanded} /> : null}

      {draft !== undefined ? (
        <Sheet title={draft.isNew ? "新建上游" : `编辑 ${draft.id}`} onEscape={() => setDraft(undefined)}>
          <form className="sheet-form" onSubmit={onSubmit}>
            {draft.isNew ? (
              <label>
                上游 ID(创建后不可变)
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
              Provider 家族
              <input
                className="mono"
                required
                maxLength={128}
                list="kind-suggestions"
                value={draft.kind}
                onChange={(event) => setDraft({ ...draft, kind: event.target.value })}
              />
              <datalist id="kind-suggestions">
                {KIND_SUGGESTIONS.map((kind) => (
                  <option key={kind} value={kind} />
                ))}
              </datalist>
            </label>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={draft.enabled}
                onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })}
              />
              启用
            </label>
            <label>
              标签
              <ChipsInput
                value={draft.tags}
                onChange={(tags) => setDraft({ ...draft, tags })}
                placeholder="回车添加"
              />
            </label>
            <label>
              出口策略(可空;删除策略会静默清空此引用)
              <select
                value={draft.egress_policy_id}
                onChange={(event) => setDraft({ ...draft, egress_policy_id: event.target.value })}
              >
                <option value="">(无)</option>
                {(policies.data ?? []).map((policy) => (
                  <option key={policy.id} value={policy.id}>
                    {policy.name}({policy.id})
                  </option>
                ))}
              </select>
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setDraft(undefined)}>
                取消
              </button>
              <button type="submit" disabled={save.isPending}>
                保存
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDelete !== undefined ? (
        <Sheet title="确认删除" onEscape={() => setConfirmDelete(undefined)}>
          <p className="reveal-warning">
            删除上游 <span className="mono">{confirmDelete.id}</span>
            将级联删除其全部端点、凭据与绑定,且引用这些端点的路由候选一并失效。
          </p>
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
