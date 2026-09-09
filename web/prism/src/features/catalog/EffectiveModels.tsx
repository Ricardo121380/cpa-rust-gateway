import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ObjectInspector } from "../../components/ObjectInspector";
import { useVersionStore } from "../config-versions/versionStore";

type ContextRow = Readonly<{ id: string; status: string }>;
export type EffectiveModel = Readonly<{
  id: string;
  public_model_id: string;
  public_model_name: string;
  route_id: string;
  sources: readonly Readonly<{
    candidate_id: string;
    endpoint_id: string;
    upstream_id: string;
    api_format: string;
    catalog_evidence: readonly Readonly<{
      credential_id: string;
      version: number;
      observed_at_ms: number;
      stale_at_ms: number;
      expires_at_ms: number;
      catalog_eligible: boolean;
    }>[];
    catalog_admission:
      | "manual"
      | "fresh"
      | "stale"
      | "expired"
      | "allowed_unlisted";
  }>[];
}>;
export type EffectiveModelPage = Readonly<{
  config_version: string;
  access_group_id: string;
  client_key_id: string | null;
  projection_id: string;
  observed_at_ms: number;
  items: readonly EffectiveModel[];
  next_cursor: string | null;
}>;

export function EffectiveModels() {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  const [kind, setKind] = useState<"access_group_id" | "client_key_id">(
    "access_group_id",
  );
  const [id, setId] = useState("");
  const [selected, setSelected] = useState<EffectiveModel>();
  const client = useQueryClient();
  const identities = useQuery({
    queryKey: ["effective-model-contexts", scope, kind],
    queryFn: () =>
      call<ContextRow[]>(
        kind === "access_group_id" ? "listAccessGroups" : "listClientKeys",
        {},
        { versionScoped: true },
      ),
    enabled: scope !== undefined,
  });
  const key = ["effective-models", scope, kind, id];
  const models = useInfiniteQuery({
    queryKey: key,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      call<EffectiveModelPage>(
        "listEffectiveModels",
        {
          query: {
            [kind]: id,
            limit: 100,
            ...(pageParam === undefined ? {} : { cursor: pageParam }),
          },
        },
        { versionScoped: true },
      ),
    enabled: scope !== undefined && id !== "",
    retry: false,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
  const rows = models.data?.pages.flatMap((page) => page.items) ?? [];
  const first = models.data?.pages[0];
  return (
    <section className="data-panel data-panel--padded" aria-label="授权有效模型">
      <header className="page-head">
        <div>
          <h3>授权有效模型</h3>
          <p className="stat-sub">
            选择 serving 配置中的既有授权身份。这里只列 exact 模型
            ID；临时冷却与配额不改变授权。
          </p>
        </div>
      </header>
      <div className="data-toolbar">
        <select
          aria-label="模型授权上下文类型"
          value={kind}
          onChange={(event) => {
            setKind(event.target.value as typeof kind);
            setId("");
            setSelected(undefined);
          }}
        >
          <option value="access_group_id">Access Group</option>
          <option value="client_key_id">Key ID</option>
        </select>
        <select
          aria-label="模型授权身份"
          value={id}
          onChange={(event) => {
            setId(event.target.value);
            setSelected(undefined);
          }}
        >
          <option value="">选择既有身份</option>
          {(identities.data ?? []).map((row) => (
            <option key={row.id} value={row.id}>
              {row.id} · {row.status}
            </option>
          ))}
        </select>
        <button
          className="secondary"
          disabled={!id || models.isFetching}
          onClick={() => {
            setSelected(undefined);
            void client.resetQueries({ queryKey: key });
          }}
        >
          重新读取模型
        </button>
      </div>
      {identities.isError ? (
        <p role="alert">
          授权身份读取失败：{asAppError(identities.error).message}
        </p>
      ) : null}
      {!id ? (
        <p className="empty-state">
          先选择授权身份。Key 只使用 ID，不提交 Client Key secret。
        </p>
      ) : models.isError ? (
        <p role="alert" className="empty-state">
          {asAppError(models.error).message}。请检查 serving
          版本与身份；投影变化后重新读取。
        </p>
      ) : models.isPending ? (
        <p role="status">正在读取授权模型…</p>
      ) : rows.length === 0 ? (
        <p className="empty-state">此授权身份当前没有可见模型。</p>
      ) : null}
      {first !== undefined ? (
        <p className="scope-row">
          Serving {first.config_version} · Access Group {first.access_group_id}{" "}
          · 已载入 {rows.length} 项{models.hasNextPage ? " · 还有更多" : ""}
        </p>
      ) : null}
      {rows.length > 0 ? (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Exact 模型 ID</th>
                <th>公开模型</th>
                <th>来源</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((model) => (
                <tr key={model.id}>
                  <td className="mono">{model.id}</td>
                  <td>{model.public_model_name}</td>
                  <td>{model.sources.length} 个候选</td>
                  <td>
                    <button
                      className="secondary"
                      disabled={models.isError}
                      onClick={() => setSelected(model)}
                    >
                      模型来源
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {models.hasNextPage ? (
        <button
          className="secondary"
          disabled={models.isFetching || models.isError}
          onClick={() => void models.fetchNextPage()}
        >
          加载更多模型
        </button>
      ) : null}
      {selected !== undefined && first !== undefined ? (
        <ObjectInspector
          title={selected.id}
          scope={`Serving ${first.config_version} · ${first.access_group_id}`}
          onClose={() => setSelected(undefined)}
          facts={[
            ["公开模型 ID", selected.public_model_id],
            ["公开模型名", selected.public_model_name],
            ["Route", selected.route_id],
            ["观察时间", new Date(first.observed_at_ms).toLocaleString()],
            ["投影指纹", first.projection_id],
          ]}
        >
          {selected.sources.map((source) => (
            <dl className="fact-grid" key={source.candidate_id}>
              <div>
                <dt>Candidate</dt>
                <dd>{source.candidate_id}</dd>
              </div>
              <div>
                <dt>Endpoint</dt>
                <dd>{source.endpoint_id}</dd>
              </div>
              <div>
                <dt>Upstream</dt>
                <dd>{source.upstream_id}</dd>
              </div>
              <div>
                <dt>协议</dt>
                <dd>{source.api_format}</dd>
              </div>
              <div>
                <dt>编译目录准入</dt>
                <dd>{source.catalog_admission}</dd>
              </div>
              <div>
                <dt>草稿操作</dt>
                <dd>
                  <Link
                    to={`/models?${new URLSearchParams({ from_model: selected.id, from_endpoint: source.endpoint_id, from_version: first?.config_version ?? "" })}`}
                  >
                    用于草稿候选
                  </Link>
                </dd>
              </div>
              {source.catalog_evidence.length === 0 ? (
                <div>
                  <dt>目录观测</dt>
                  <dd>未观测</dd>
                </div>
              ) : (
                source.catalog_evidence.map((evidence) => (
                  <div key={evidence.credential_id}>
                    <dt>
                      {evidence.credential_id} · 目录 v{evidence.version}
                    </dt>
                    <dd>
                      观测 {new Date(evidence.observed_at_ms).toLocaleString()}
                      <br />
                      软过期 {new Date(evidence.stale_at_ms).toLocaleString()}
                      <br />
                      硬过期 {new Date(evidence.expires_at_ms).toLocaleString()}
                      <br />
                      {evidence.catalog_eligible
                        ? "目录准入有效"
                        : "目录不准入"}
                    </dd>
                  </div>
                ))
              )}
            </dl>
          ))}
          <Link
            to={`/runtime?route_id=${encodeURIComponent(selected.route_id)}&requested_model=${encodeURIComponent(selected.id)}`}
          >
            在诊断中检查该模型
          </Link>
        </ObjectInspector>
      ) : null}
    </section>
  );
}
