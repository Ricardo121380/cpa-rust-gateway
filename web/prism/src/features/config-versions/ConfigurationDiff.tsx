import "./configuration-diff.css";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { Sheet } from "../../components/Sheet";
import { ReadStatus } from "../../components/ReadStatus";
import type { ConfigVersionSummary } from "./versionStore";

type Page = {
  base: { id: string; revision: string };
  target: { id: string; revision: string };
  items: {
    resource_kind: string;
    resource_key: string;
    change: "added" | "removed" | "changed";
    changed_fields: string[];
  }[];
  next_cursor: string | null;
};
const kinds: Record<string, string> = {
  egress_policy: "出口策略",
  upstream: "上游",
  proxy_pool: "代理池",
  proxy_node: "代理节点",
  endpoint: "端点",
  credential: "凭据",
  credential_binding: "凭据绑定",
  egress_binding: "出口绑定",
  public_model: "公开模型",
  alias: "别名",
  route: "路由",
  candidate: "候选",
  access_group: "访问组",
  route_grant: "路由授权",
  client_key: "Client Key",
  routing_price_policy: "路由价格策略",
};
const changes = { added: "新增", removed: "删除", changed: "修改" };
function identity(value: string): string {
  try {
    const parts: unknown = JSON.parse(value);
    if (Array.isArray(parts) && parts.every((part) => typeof part === "string"))
      return parts.join(" · ");
  } catch {
    /* Retain the opaque identity if a future server changes its encoding. */
  }
  return value;
}
export function ConfigurationDiff({
  target,
  versions,
  onClose,
}: Readonly<{
  target: ConfigVersionSummary;
  versions: readonly ConfigVersionSummary[];
  onClose: () => void;
}>) {
  const [base, setBase] = useState(
    target.parent_id ??
      versions.find(
        (version) => version.status === "active" && version.id !== target.id,
      )?.id ??
      versions.find((version) => version.id !== target.id)?.id ??
      target.id,
  );
  const client = useQueryClient();
  const key = ["configuration-diff", target.id, base];
  const query = useInfiniteQuery({
    queryKey: key,
    initialPageParam: undefined as string | undefined,
    retry: false,
    queryFn: ({ pageParam }) =>
      call<Page>("compareConfigVersions", {
        path: { config_version_id: target.id },
        query: {
          base_id: base,
          limit: 50,
          ...(pageParam === undefined ? {} : { cursor: pageParam }),
        },
      }),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  const snapshot = query.data?.pages[0];
  return (
    <Sheet title="配置资源差异" onEscape={onClose}>
      <div className="data-toolbar">
        <label>
          比较基线
          <select
            value={base}
            onChange={(event) => setBase(event.target.value)}
          >
            {versions.map((version) => (
              <option key={version.id} value={version.id}>
                {version.id} · {version.status}
              </option>
            ))}
          </select>
        </label>
        <button
          className="secondary"
          disabled={query.isFetching}
          onClick={() => void client.resetQueries({ queryKey: key })}
        >
          重新比较
        </button>
      </div>
      <dl className="fact-grid">
        <div><dt>基线版本</dt><dd>{base}</dd></div>
        <div><dt>目标版本</dt><dd>{target.id}</dd></div>
      </dl>
      {snapshot ? (
        <p className="scope-row">
          基线 {snapshot.base.revision} → 目标 {snapshot.target.revision} ·
          已载入 {rows.length} 项差异{query.hasNextPage ? "，还有更多" : ""}
        </p>
      ) : null}
      <p className="stat-sub">
        比较持久化资源记录，仅展示变化字段名。加密载荷不同不代表已判断秘密明文不同；运行观测与全局价格目录不参与比较。
      </p>
      <ReadStatus
        pending={query.isPending}
        error={query.error}
        hasData={query.data !== undefined}
        retry={() => void client.resetQueries({ queryKey: key })}
      />
      {query.isError ? (
        <p role="status">分页已停止。版本变化时，请重新比较。</p>
      ) : null}
      {rows.length ? (
        <div className="table-scroll configuration-diff-table">
          <table>
            <thead>
              <tr>
                <th>对象</th>
                <th>变化</th>
                <th>字段</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={`${row.resource_kind}:${row.resource_key}`}>
                  <td>
                    {kinds[row.resource_kind] ?? row.resource_kind}
                    <br />
                    <span className="mono">{identity(row.resource_key)}</span>
                  </td>
                  <td>{changes[row.change]}</td>
                  <td className="mono">
                    {row.changed_fields.join("、") || "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : !query.isPending && !query.isError ? (
        <p className="empty-state">两个版本的资源记录没有差异。</p>
      ) : null}
      <div className="sheet-actions">
        {query.hasNextPage ? (
          <button
            className="secondary"
            disabled={query.isFetching || query.isError}
            onClick={() => void query.fetchNextPage()}
          >
            加载更多差异
          </button>
        ) : null}
        <button onClick={onClose}>关闭</button>
      </div>
    </Sheet>
  );
}
