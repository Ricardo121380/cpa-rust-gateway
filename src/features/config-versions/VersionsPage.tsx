// Config-version workspace: the lifecycle hub (docs/07 §7.4 / v0.1 §7.3).
// List → create draft → validate → publish (If-Match) → rollback.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { messages } from "../../i18n/messages";
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
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const select = useVersionStore((s) => s.select);
  const [creating, setCreating] = useState(false);
  const [validation, setValidation] = useState<{ id: string; result: Validation } | undefined>();
  const [publication, setPublication] = useState<Publication | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const versions = useQuery({
    queryKey: ["config-versions"],
    queryFn: () => call<ConfigVersionSummary[]>("listConfigVersions"),
    staleTime: 10_000,
  });

  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["config-versions"] });

  const validate = useMutation({
    mutationFn: (id: string) =>
      call<Validation>("validateConfigVersion", { path: { config_version_id: id } }),
    onSuccess: (result, id) => setValidation({ id, result }),
    onError: (error) => setActionError(asAppError(error).message),
  });

  const publish = useMutation({
    mutationFn: (id: string) =>
      call<Publication>(
        "publishConfigVersion",
        { path: { config_version_id: id } },
        { mutating: true },
      ),
    onSuccess: (result) => {
      setPublication(result);
      refresh();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const rollback = useMutation({
    mutationFn: () => call<Publication>("rollbackConfigVersion", {}, { mutating: true }),
    onSuccess: (result) => {
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
        <h2>{messages.nav.versions}</h2>
        <div className="page-actions">
          <button type="button" onClick={() => setCreating(true)}>
            创建草稿
          </button>
          <button
            type="button"
            className="secondary"
            disabled={rollback.isPending}
            onClick={() => rollback.mutate()}
          >
            回滚到上一版本
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
              <th>版本</th>
              <th>状态</th>
              <th>revision</th>
              <th>创建时间</th>
              <th>描述</th>
              <th>父版本</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(versions.data ?? []).map((version) => {
              const selected = context?.configVersionId === version.id;
              return (
                <tr key={version.id} data-selected={selected}>
                  <td className="mono">{version.id}</td>
                  <td>
                    <StatusBadge status={version.status} />
                  </td>
                  <td className="mono">{version.revision}</td>
                  <td className="mono">{formatTime(version.created_at_ms)}</td>
                  <td>{version.description}</td>
                  <td className="mono">{version.parent_id ?? "—"}</td>
                  <td className="row-actions">
                    <button type="button" disabled={selected} onClick={() => select(version)}>
                      {selected ? "已选择" : "选择"}
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
                        title={selected ? undefined : "先选择该版本(If-Match 需要其 revision)"}
                        onClick={() => publish.mutate(version.id)}
                      >
                        发布
                      </button>
                    ) : null}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {versions.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{messages.state.empty}</p>
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
