// Public models: client-visible model names + capabilities + 1:1 route.
// Top-level CRUD works against the existing contract; alias/route/candidate
// ENUMERATION waits for G1 (creation works today — insert-only contract).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { messages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import {
  enabledCapabilities,
  ROUTE_POLICY,
  SEMANTIC_CAPABILITIES,
  toggleCapability,
  validRouteParams,
  type PublicModel,
} from "./model";

type DraftModel = {
  id: string;
  model_name: string;
  status: "active" | "disabled";
  display_name: string;
  capabilities: Record<string, boolean>;
  isNew: boolean;
};

function emptyDraft(): DraftModel {
  return {
    id: "",
    model_name: "",
    status: "active",
    display_name: "",
    capabilities: { streaming: true },
    isNew: true,
  };
}

function toDraft(model: PublicModel): DraftModel {
  return {
    id: model.id,
    model_name: model.model_name,
    status: model.status,
    display_name: model.display_name,
    capabilities: { ...model.capabilities },
    isNew: false,
  };
}

function toInput(draft: DraftModel) {
  return {
    id: draft.id,
    model_name: draft.model_name,
    status: draft.status,
    display_name: draft.display_name,
    capabilities: draft.capabilities,
  };
}

export function ModelsPage() {
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [draft, setDraft] = useState<DraftModel | undefined>();
  const [confirmDelete, setConfirmDelete] = useState<PublicModel | undefined>();
  const [aliasTarget, setAliasTarget] = useState<PublicModel | undefined>();
  const [routeTarget, setRouteTarget] = useState<PublicModel | undefined>();
  const [notice, setNotice] = useState<string | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const models = useQuery({
    queryKey: ["public-models", scope],
    queryFn: () => call<PublicModel[]>("listPublicModels", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const invalidate = () =>
    void queryClient.invalidateQueries({ queryKey: ["public-models", scope] });

  const save = useMutation({
    mutationFn: (input: DraftModel) =>
      input.isNew
        ? call<PublicModel>("createPublicModel", { body: toInput(input) }, { versionScoped: true, mutating: true })
        : call<PublicModel>(
            "updatePublicModel",
            { path: { public_model_id: input.id }, body: toInput(input) },
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
        "deletePublicModel",
        { path: { public_model_id: id } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setConfirmDelete(undefined);
      invalidate();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const addAlias = useMutation({
    mutationFn: (input: { modelId: string; alias: string }) =>
      call<{ alias: string }>(
        "createModelAlias",
        { path: { public_model_id: input.modelId }, body: { alias: input.alias } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: (created) => {
      setAliasTarget(undefined);
      setNotice(`别名 ${created.alias} 已创建(现有别名的枚举等待 G1 契约)`);
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const createRoute = useMutation({
    mutationFn: (input: {
      modelId: string;
      id: string;
      max_attempts: number;
      bootstrap_timeout_ms: number;
    }) =>
      call<{ id: string }>(
        "createRoute",
        {
          path: { public_model_id: input.modelId },
          body: {
            id: input.id,
            policy: ROUTE_POLICY,
            max_attempts: input.max_attempts,
            bootstrap_timeout_ms: input.bootstrap_timeout_ms,
          },
        },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: (created) => {
      setRouteTarget(undefined);
      setNotice(
        `路由 ${created.id} 已创建。候选(Candidate)编辑与 Route Prism 视图等待 G1 契约解锁。`,
      );
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  function onSaveSubmit(event: FormEvent) {
    event.preventDefault();
    if (draft !== undefined) {
      save.mutate(draft);
    }
  }

  if (scope === undefined) {
    return (
      <section>
        <h2>{messages.nav.models}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本。</p>
        </div>
      </section>
    );
  }

  return (
    <section>
      <header className="page-head">
        <h2>{messages.nav.models}</h2>
        <div className="page-actions">
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : messages.version.readOnly}
            onClick={() => setDraft(emptyDraft())}
          >
            新建公开模型
          </button>
        </div>
      </header>

      {notice !== undefined ? (
        <p className="action-notice">
          {notice}
          <button type="button" onClick={() => setNotice(undefined)}>
            知道了
          </button>
        </p>
      ) : null}
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
              <th>模型名(客户端可见)</th>
              <th>显示名</th>
              <th>状态</th>
              <th>能力</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(models.data ?? []).map((model) => (
              <tr key={model.id}>
                <td className="mono">{model.model_name}</td>
                <td>{model.display_name}</td>
                <td>
                  <StatusBadge status={model.status} />
                </td>
                <td>
                  {enabledCapabilities(model.capabilities).map((capability) => (
                    <span key={capability} className="idchip">
                      {capability}
                    </span>
                  ))}
                </td>
                <td className="row-actions">
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setDraft(toDraft(model))}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setAliasTarget(model)}
                  >
                    加别名
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setRouteTarget(model)}
                  >
                    建路由
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={!editable}
                    onClick={() => setConfirmDelete(model)}
                  >
                    删除
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {models.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{messages.state.empty}</p>
          </div>
        ) : null}
      </div>

      <div className="card empty-state" data-kind="unwired" data-gap="top">
        <p>
          别名清单、路由参数与候选(Candidate)的枚举视图,以及 Route Prism 光路图,
          依赖 G1 全图契约(后端会话)—— 创建操作今天可用,列表在 G1 后解锁。
        </p>
      </div>

      {draft !== undefined ? (
        <Sheet
          title={draft.isNew ? "新建公开模型" : `编辑 ${draft.model_name}`}
          onEscape={() => setDraft(undefined)}
        >
          <form className="sheet-form" onSubmit={onSaveSubmit}>
            {draft.isNew ? (
              <label>
                模型 ID
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
              模型名(客户端请求用,版本内唯一)
              <input
                className="mono"
                required
                maxLength={256}
                value={draft.model_name}
                onChange={(event) => setDraft({ ...draft, model_name: event.target.value })}
              />
            </label>
            <label>
              显示名
              <input
                maxLength={256}
                value={draft.display_name}
                onChange={(event) => setDraft({ ...draft, display_name: event.target.value })}
              />
            </label>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={draft.status === "active"}
                onChange={(event) =>
                  setDraft({ ...draft, status: event.target.checked ? "active" : "disabled" })
                }
              />
              启用(active)
            </label>
            <fieldset className="capability-set">
              <legend>必需能力(候选端点缺任一能力将被准入拒绝)</legend>
              {SEMANTIC_CAPABILITIES.map((capability) => (
                <label key={capability} className="toggle-row">
                  <input
                    type="checkbox"
                    checked={draft.capabilities[capability] === true}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        capabilities: toggleCapability(
                          draft.capabilities,
                          capability,
                          event.target.checked,
                        ),
                      })
                    }
                  />
                  <span className="mono">{capability}</span>
                  {capability === "parallel_tools" ? (
                    <small>(隐含 tools)</small>
                  ) : null}
                </label>
              ))}
            </fieldset>
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

      {aliasTarget !== undefined ? (
        <Sheet title={`为 ${aliasTarget.model_name} 添加别名`} onEscape={() => setAliasTarget(undefined)}>
          <form
            className="sheet-form"
            onSubmit={(event) => {
              event.preventDefault();
              const alias = String(new FormData(event.currentTarget).get("alias") ?? "");
              addAlias.mutate({ modelId: aliasTarget.id, alias });
            }}
          >
            <label>
              别名(不得与任何活动模型名冲突)
              <input name="alias" className="mono" required maxLength={256} />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setAliasTarget(undefined)}>
                取消
              </button>
              <button type="submit" disabled={addAlias.isPending}>
                创建
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {routeTarget !== undefined ? (
        <Sheet title={`为 ${routeTarget.model_name} 创建路由(1:1)`} onEscape={() => setRouteTarget(undefined)}>
          <form
            className="sheet-form"
            onSubmit={(event) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              const maxAttempts = Number(data.get("max_attempts") ?? 3);
              const bootstrapTimeoutMs = Number(data.get("bootstrap_timeout_ms") ?? 30000);
              if (!validRouteParams(maxAttempts, bootstrapTimeoutMs)) {
                setActionError("路由参数越界:max_attempts 1-16,bootstrap_timeout_ms 1-120000");
                return;
              }
              createRoute.mutate({
                modelId: routeTarget.id,
                id: String(data.get("id") ?? ""),
                max_attempts: maxAttempts,
                bootstrap_timeout_ms: bootstrapTimeoutMs,
              });
            }}
          >
            <label>
              路由 ID
              <input name="id" className="mono" required maxLength={128} />
            </label>
            <label>
              调度策略(契约当前唯一值)
              <input className="mono" value={ROUTE_POLICY} disabled />
            </label>
            <label>
              max_attempts(1-16;透明重试仅发生在首字节抵达客户端前)
              <input name="max_attempts" type="number" min={1} max={16} defaultValue={3} />
            </label>
            <label>
              bootstrap_timeout_ms(1-120000;全部透明重试共享的累计预算)
              <input
                name="bootstrap_timeout_ms"
                type="number"
                min={1}
                max={120000}
                defaultValue={30000}
              />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setRouteTarget(undefined)}>
                取消
              </button>
              <button type="submit" disabled={createRoute.isPending}>
                创建
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDelete !== undefined ? (
        <Sheet title="确认删除" onEscape={() => setConfirmDelete(undefined)}>
          <p className="reveal-warning">
            删除公开模型 <span className="mono">{confirmDelete.model_name}</span>
            将级联删除其全部别名与路由(含候选),客户端将无法再解析该模型名。
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
