import { resourceName } from "../../utils/resourceNames";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { EffectiveModels } from "./EffectiveModels";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import {
  formatObservedAt,
  freshnessMeta,
  FRESHNESS_STATES,
  type CatalogRow,
} from "../runtime/model";

export function CatalogPage() {
  const t = useMessages();
  const scope = useVersionStore((s) => s.context?.configVersionId);
  const [params, setParams] = useSearchParams();
  const [selected, setSelected] = useState<CatalogRow>();
  const query = params.get("q") ?? "";
  const state = params.get("state") ?? "";
  const catalog = useQuery({
    queryKey: ["catalog-status", scope],
    enabled: scope !== undefined,
    queryFn: () =>
      call<CatalogRow[]>("getCatalogStatus", {}, { versionScoped: true }),
    staleTime: 30_000,
  });
  const rows = (catalog.data ?? []).filter(
    (row) =>
      (!state || row.freshness === state) &&
      `${row.endpoint_id} ${row.credential_id}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const update = (key: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(key, value);
    else next.delete(key);
    setParams(next, { replace: true });
  };
  return (
    <section>
      <header className="page-head">
        <div>
          <h2>{t.nav.catalog}</h2>
          <p className="scope-row">
            {scope === undefined ? "未选择配置版本" : `配置版本 ${resourceName(scope ?? "—", "config")}`} · 逐
            Endpoint × Credential 的目录证据
          </p>
        </div>
        <button
          className="secondary"
          disabled={scope === undefined || catalog.isFetching}
          onClick={() => {
            setSelected(undefined);
            void catalog.refetch();
          }}
        >
          刷新目录
        </button>
      </header>
      <EffectiveModels />
      <div className="stat-row">
        {FRESHNESS_STATES.map((state) => (
          <div className="stat-tile" key={state}>
            <span className="stat-label">{freshnessMeta(state).label}</span>
            <strong className="stat-value">
              {catalog.data === undefined
                ? "—"
                : catalog.data.filter((r) => r.freshness === state).length}
            </strong>
            <span className="stat-sub">目录 target</span>
          </div>
        ))}
      </div>
      <div className="data-panel">
        <div className="data-toolbar">
          <input
            aria-label="搜索目录目标"
            placeholder="搜索 Endpoint、Credential"
            value={query}
            onChange={(e) => update("q", e.target.value)}
          />
          <select
            aria-label="目录状态"
            value={state}
            onChange={(e) => update("state", e.target.value)}
          >
            <option value="">全部状态</option>
            {FRESHNESS_STATES.map((s) => (
              <option key={s} value={s}>
                {freshnessMeta(s).label}
              </option>
            ))}
          </select>
        </div>
        {scope === undefined ? (
          <div className="empty-state">选择配置版本后查看目录证据。</div>
        ) : catalog.isError ? (
          <div className="empty-state" role="alert">
            {asAppError(catalog.error).kind === "unavailable"
              ? t.state.unavailable
              : asAppError(catalog.error).code}
          </div>
        ) : catalog.isPending ? (
          <div className="empty-state">读取目录…</div>
        ) : rows.length === 0 ? (
          <div className="empty-state">
            {query || state ? t.state.filteredEmpty : t.state.empty}
          </div>
        ) : (
          <div className="tablewrap">
            <table>
              <thead>
                <tr>
                  <th>目录目标</th>
                  <th>新鲜度</th>
                  <th>模型数</th>
                  <th>最近成功观测</th>
                  <th>刷新 / 失败</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr key={`${row.endpoint_id}/${row.credential_id}`}>
                    <td>
                      <div className="entity-name"><ResourceIdentity id={row.endpoint_id} kind="endpoint" /></div>
                      <div className="entity-meta"><ResourceIdentity id={row.credential_id} kind="account" /></div>
                    </td>
                    <td>
                      <StatusBadge status={row.freshness}>
                        {freshnessMeta(row.freshness).label}
                      </StatusBadge>
                    </td>
                    <td className="mono">{row.model_count ?? "—"}</td>
                    <td className="mono">
                      {formatObservedAt(row.observed_at_ms)}
                    </td>
                    <td>
                      <div>
                        {row.refresh_due === undefined
                          ? "未观测"
                          : row.refresh_due
                            ? "待刷新"
                            : "未到刷新期"}
                      </div>
                      <div className="entity-meta">
                        {row.last_failure_class ?? "无失败观测"}
                      </div>
                    </td>
                    <td>
                      <button
                        className="secondary"
                        onClick={() => setSelected(row)}
                      >
                        详情
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <div className="data-footer">
          <span>
            {rows.length} 个 target · 目录模型数不等于授权可调用模型数
          </span>
        </div>
      </div>
      <details className="card" data-gap="top">
        <summary>目录生命周期</summary>
        <p className="small">
          6 小时内 Fresh；之后 Stale；24 小时后要求刷新；达到 72
          小时硬过期。状态以服务端返回为准。Missing
          表示没有成功目录，不能解释为刷新成功但模型数为零。
        </p>
      </details>
      {selected === undefined ? null : (
        <Sheet
          layout="inspector"
          title="目录目标"
          onEscape={() => setSelected(undefined)}
        >
          <div className="entity-name"><ResourceIdentity id={selected.endpoint_id} kind="endpoint" /></div>
          <p className="entity-meta"><ResourceIdentity id={selected.credential_id} kind="account" /></p>
          <dl className="fact-grid">
            {[
              ["新鲜度", freshnessMeta(selected.freshness).label],
              ["目录快照", selected.snapshot_version ?? "未观测"],
              ["目录模型数", selected.model_count ?? "未观测"],
              [
                "需要刷新",
                selected.refresh_due === undefined
                  ? "未观测"
                  : selected.refresh_due
                    ? "是"
                    : "否",
              ],
              ["成功观测", formatObservedAt(selected.observed_at_ms)],
              ["最近失败", formatObservedAt(selected.last_failure_at_ms ?? 0)],
              ["失败分类", selected.last_failure_class ?? "未观测"],
            ].map(([label, value]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
          <div className="detail-links">
            <Link
              to={`/accounts?q=${encodeURIComponent(selected.credential_id)}`}
            >
              查看账号
            </Link>
            <Link to="/models">模型与路由</Link>
            <Link
              to={`/runtime?account_id=${encodeURIComponent(selected.credential_id)}`}
            >
              运行诊断
            </Link>
          </div>
        </Sheet>
      )}
    </section>
  );
}
