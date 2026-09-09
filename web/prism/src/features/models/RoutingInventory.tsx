import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { useVersionStore } from "../config-versions/versionStore";
import type {
  RoutingPage,
  RouteListItem,
  CandidateRecord,
  AliasRecord,
} from "./model";

const tabs = [
  ["listRoutes", "路由"],
  ["listRouteCandidates", "候选"],
  ["listModelAliases", "别名"],
] as const;
type Operation = (typeof tabs)[number][0];
type Item = RouteListItem | CandidateRecord | AliasRecord;

export function RoutingInventory({
  onOpen,
}: Readonly<{ onOpen: (id: string) => void }>) {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  const [operation, setOperation] = useState<Operation>("listRoutes");
  const client = useQueryClient();
  const key = ["routing-inventory", scope, operation];
  const query = useInfiniteQuery({
    queryKey: key,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      call<RoutingPage<Item>>(
        operation,
        {
          query: {
            limit: 100,
            ...(pageParam === undefined ? {} : { cursor: pageParam }),
          },
        },
        { versionScoped: true },
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    enabled: scope !== undefined,
    retry: false,
  });
  const items = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <section aria-label="完整配置资源" className="card" data-gap="top">
      <header className="page-head">
        <h3>配置资源</h3>
        <button
          className="secondary"
          onClick={() => void client.resetQueries({ queryKey: key })}
        >
          重新读取
        </button>
      </header>
      <div className="rw-actions" aria-label="资源类型">
        {tabs.map(([value, label]) => (
          <button
            key={value}
            className="secondary"
            aria-pressed={value === operation}
            onClick={() => setOperation(value)}
          >
            {label}
          </button>
        ))}
      </div>
      <p className="stat-sub">
        包含未绑定访问组的草稿资源 · 已载入 {items.length} 项
        {query.hasNextPage ? " · 还有更多" : ""}
      </p>
      {query.isError ? (
        <div role="alert" className="action-error">
          {asAppError(query.error).message}
          。分页已停止，请重新读取以获取一致版本。
        </div>
      ) : null}
      {query.isPending ? <p role="status">正在读取配置资源…</p> : null}
      {!query.isPending && !query.isError && items.length === 0 ? (
        <p className="empty-state">此版本暂无该类资源</p>
      ) : null}
      {items.length > 0 ? (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>标识</th>
                <th>所属对象</th>
                <th>配置</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => {
                const id = "alias" in item ? item.alias : item.id;
                const owner =
                  "route_id" in item ? item.route_id : item.public_model_id;
                const routeId =
                  "policy" in item
                    ? item.id
                    : "route_id" in item
                      ? item.route_id
                      : undefined;
                return (
                  <tr key={id}>
                    <td className="mono">{id}</td>
                    <td className="mono">{owner}</td>
                    <td>
                      {"policy" in item
                        ? item.policy
                        : "upstream_model" in item
                          ? `${item.upstream_model} · ${item.enabled ? "启用" : "停用"} · 权重 ${item.weight}`
                          : "模型别名"}
                    </td>
                    <td>
                      {routeId === undefined ? (
                        "—"
                      ) : (
                        <button
                          className="secondary"
                          disabled={query.isError}
                          onClick={() => onOpen(routeId)}
                        >
                          打开路由
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : null}
      {query.hasNextPage ? (
        <button
          className="secondary"
          disabled={query.isFetching || query.isError}
          onClick={() => void query.fetchNextPage()}
        >
          {query.isFetching ? "读取中…" : "加载更多资源"}
        </button>
      ) : null}
    </section>
  );
}
