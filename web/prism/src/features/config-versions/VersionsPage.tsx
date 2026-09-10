import { ConfigurationDiff } from "./ConfigurationDiff";
import { LifecycleConfirmation } from "./LifecycleConfirmation";
import { ReadStatus } from "../../components/ReadStatus";
// Config-version workspace: the lifecycle hub (docs/07 §7.4 / v0.1 §7.3).
// List → create draft → validate → publish (If-Match) → rollback.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";

type Validation = Readonly<{ valid: boolean; error_codes?: readonly string[] }>;
type Publication = Readonly<{
  active_config_version_id: string;
  replaced_config_version_id?: string | null;
}>;

function formatTime(ms: number): string {
  return new Date(ms).toISOString().replace("T", " ").slice(0, 16);
}

export function VersionsPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const select = useVersionStore((s) => s.select);
  const [confirmation, setConfirmation] = useState<{ mode: "publish" | "rollback"; id: string }>();
  const [diffTarget, setDiffTarget] = useState<ConfigVersionSummary>();
  const [creating, setCreating] = useState(false);
  const [collection, setCollection] = useState<"draft" | "archived">("draft");
  const [validation, setValidation] = useState<{ id: string; result: Validation } | undefined>();
  const [publication, setPublication] = useState<Publication | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const versions = useQuery({
    queryKey: ["config-versions"],
    queryFn: () => call<ConfigVersionSummary[]>("listConfigVersions"),
    staleTime: 10_000,
  });

  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["config-versions"] });
  const active = versions.data?.find((version) => version.status === "active");
  const visibleVersions = versions.data?.filter((version) => version.status === collection) ?? [];

  const validate = useMutation({
    mutationFn: (id: string) =>
      call<Validation>("validateConfigVersion", { path: { config_version_id: id } }),
    onSuccess: (result, id) => setValidation({ id, result }),
    onError: (error) => setActionError(asAppError(error).message),
  });

  const publish = useMutation({
    mutationFn: ({ id, expectedActive, lifecycleEvent }: { id: string; expectedActive: string; lifecycleEvent: string }) =>
      call<Publication>(
        "publishConfigVersion",
        { path: { config_version_id: id }, headers: { "X-Expected-Active-Version": expectedActive, "X-Expected-Lifecycle-Event": lifecycleEvent } },
        { mutating: true },
      ),
    onSuccess: (result) => {
      setConfirmation(undefined);
      setPublication(result);
      refresh();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const rollback = useMutation({
    mutationFn: ({ expectedActive, lifecycleEvent }: { expectedActive: string; lifecycleEvent: string }) => call<Publication>("rollbackConfigVersion", { headers: { "X-Expected-Active-Version": expectedActive, "X-Expected-Lifecycle-Event": lifecycleEvent } }, { mutating: true }),
    onSuccess: (result) => {
      setConfirmation(undefined);
      setPublication(result);
      refresh();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const create = useMutation({
    mutationFn: (input: { id: string; parent_id: string | null; description: string }) =>
      call<ConfigVersionSummary>("createConfigVersion", { body: input }),
    onSuccess: (row) => {
      setCreating(false);
      refresh();
      select(row);
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  function onCreateSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    create.mutate({
      id: String(data.get("id") ?? ""),
      parent_id: String(data.get("parent_id") ?? "") || null,
      description: String(data.get("description") ?? ""),
    });
  }

  return (
    <section>
      <header className="page-head">
        <div><h2>{t.nav.versions}</h2><p className="page-subtitle">在草稿中调整路由，核对后再发布。</p></div>
        <div className="page-actions">
          <button type="button" onClick={() => setCreating(true)}>
            创建草稿
          </button>
          <button
            type="button"
            className="secondary"
            disabled={rollback.isPending || context?.status !== "active"}
            title="先选择当前活动版本，再核对回滚目标"
            onClick={() => { if (context) { setActionError(undefined); setConfirmation({ mode: "rollback", id: context.configVersionId }); } }}
          >
            回滚到上一版本
          </button>
        </div>
      </header>

      {diffTarget ? <ConfigurationDiff target={diffTarget} versions={versions.data ?? []} onClose={() => setDiffTarget(undefined)} /> : null}

      {confirmation ? <LifecycleConfirmation {...confirmation} pending={publish.isPending || rollback.isPending} error={actionError} onCancel={() => setConfirmation(undefined)} onConfirm={(expectedActive, lifecycleEvent) => confirmation.mode === "publish" ? publish.mutate({ id: confirmation.id, expectedActive, lifecycleEvent }) : rollback.mutate({ expectedActive, lifecycleEvent })} /> : null}

      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <ReadStatus pending={versions.isPending} error={versions.error} hasData={versions.data !== undefined} retry={() => void versions.refetch()} />

      {active !== undefined ? (
        <article className="card published-configuration" data-version-id={active.id}>
          <div>
            <span className="configuration-state"><span className="context-dot" />已发布配置</span>
            <h3>{active.description || "网关配置"}</h3>
            <p className="entity-meta mono">{active.id} · {active.revision}</p>
            <details className="reading-notes configuration-explainer">
              <summary>配置与应用版本有什么区别？</summary>
              <p>这里只切换查看或编辑的配置。发布和回滚需要单独确认，更换程序版本属于部署。当前网关在重启时加载已发布配置。</p>
              <p>创建于 {formatTime(active.created_at_ms)} · 来源 {active.parent_id ?? "无"}</p>
            </details>
          </div>
          <div className="row-actions">
            <button className="secondary" onClick={() => setDiffTarget(active)}>查看差异</button>
            <button disabled={context?.configVersionId === active.id} onClick={() => select(active)}>
              {context?.configVersionId === active.id ? "正在查看" : "查看已发布配置"}
            </button>
          </div>
        </article>
      ) : null}
      <div className="configuration-tabs" role="group" aria-label="配置集合">
        <button className="secondary" aria-pressed={collection === "draft"} onClick={() => setCollection("draft")}>
          草稿 · {versions.data?.filter((version) => version.status === "draft").length ?? "…"}
        </button>
        <button className="secondary" aria-pressed={collection === "archived"} onClick={() => setCollection("archived")}>
          历史 · {versions.data?.filter((version) => version.status === "archived").length ?? "…"}
        </button>
      </div>
      <div className="data-panel configuration-list"><div className="tablewrap">
        <table>
          <thead>
            <tr>
              <th>配置</th>
              <th>创建时间</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {visibleVersions.map((version) => {
              const selected = context?.configVersionId === version.id;
              return (
                <tr key={version.id} data-selected={selected} data-version-id={version.id}>
                  <td>
                    <div className="entity-name">{version.description || (version.status === "draft" ? "未发布草稿" : "历史配置")}</div>
                    <div className="entity-meta mono">{version.id} · {version.revision}</div>
                    <details className="configuration-lineage"><summary>来源与状态</summary>
                      <p className="entity-meta">{version.parent_id ?? "无来源"} · <StatusBadge status={version.status} /></p>
                    </details>
                  </td>
                  <td className="entity-meta">{formatTime(version.created_at_ms)}</td>
                  <td><div className="row-actions">
                    <button className="secondary" onClick={() => setDiffTarget(version)}>查看差异</button>
                    <button type="button" disabled={selected} onClick={() => select(version)}>
                      {selected ? "正在查看" : version.status === "draft" ? "编辑草稿" : "查看历史"}
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={validate.isPending}
                      onClick={() => validate.mutate(version.id)}
                    >
                      验证
                    </button>
                    {version.status === "draft" ? (
                      <button
                        type="button"
                        disabled={!selected || publish.isPending}
                        title={selected ? undefined : "先进入此草稿，再核对发布内容"}
                        onClick={() => { setActionError(undefined); setConfirmation({ mode: "publish", id: version.id }); }}
                      >
                        发布
                      </button>
                    ) : null}
                  </div></td>
                </tr>
              );
            })}
          </tbody>
        </table></div>
        {versions.data !== undefined && visibleVersions.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{collection === "draft" ? "没有待编辑的草稿。需要调整时，创建一份新草稿。" : "暂无历史配置。"}</p>
          </div>
        ) : null}
      </div>

      {validation !== undefined ? (
        <div className="card validation-card">
          <h3>
            验证结果 · <span className="mono">{validation.id}</span>
          </h3>
          {validation.result.valid ? (
            <StatusBadge status="active">通过</StatusBadge>
          ) : (
            <ul>
              {(validation.result.error_codes ?? []).map((code) => (
                <li key={code} className="mono">
                  {code}
                </li>
              ))}
            </ul>
          )}
          <button type="button" className="secondary" onClick={() => setValidation(undefined)}>
            关闭
          </button>
        </div>
      ) : null}

      {creating ? (
        <Sheet title="创建草稿版本" onEscape={() => setCreating(false)}>
          <form className="sheet-form" onSubmit={onCreateSubmit}>
            <label>
              版本 ID
              <input name="id" className="mono" required maxLength={128} />
            </label>
            <label>
              父版本(谱系,可空)
              <input name="parent_id" className="mono" maxLength={128} />
            </label>
            <label>
              描述
              <input name="description" maxLength={1024} />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setCreating(false)}>
                取消
              </button>
              <button type="submit" disabled={create.isPending}>
                创建
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {publication !== undefined ? (
        <Sheet title="发布结果">
          <p>
            当前活动版本:<span className="mono">{publication.active_config_version_id}</span>
          </p>
          {publication.replaced_config_version_id != null ? (
            <p>
              被替换版本:<span className="mono">{publication.replaced_config_version_id}</span>
              (保留为一步回滚目标)
            </p>
          ) : null}
          <div className="sheet-actions">
            <button type="button" onClick={() => setPublication(undefined)}>
              完成
            </button>
          </div>
        </Sheet>
      ) : null}
    </section>
  );
}
