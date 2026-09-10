import { ResourceIdentity } from "../../components/ResourceIdentity";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { ObjectInspector } from "../../components/ObjectInspector";
import { useRoutingPages } from "./useRoutingPages";
import { asAppError } from "../../api/errors";
import { useVersionStore } from "../config-versions/versionStore";
import type { RouteListItem, CandidateRecord, AliasRecord } from "./model";

const tabs = [
  ["listRoutes", "路由"],
  ["listRouteCandidates", "候选"],
  ["listModelAliases", "别名"],
] as const;
type Operation = (typeof tabs)[number][0];
type Item = RouteListItem | CandidateRecord | AliasRecord;

export function RoutingInventory({
  onOpen,
  onEdit,
  onDelete,
  editable,
}: Readonly<{
  onOpen: (id: string) => void;
  editable: boolean;
  onEdit: (candidate: CandidateRecord) => void;
  onDelete: (candidate: CandidateRecord) => void;
}>) {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  const [operation, setOperation] = useState<Operation>("listRoutes");
  const [legacy, setLegacy] = useState<RouteListItem>();
  const client = useQueryClient();
  const key = ["routing-inventory", scope, operation];
  const query = useRoutingPages<Item>(operation);
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
                    <td>{"alias" in item ? <span className="mono">{id}</span> : <ResourceIdentity id={id} kind={"policy" in item ? "route" : "candidate"} />}</td>
                    <td><ResourceIdentity id={owner} kind={"route_id" in item ? "route" : "resource"} /></td>
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
                          onClick={() => {
                            if (
                              "policy" in item &&
                              item.policy !== "smooth_weighted_round_robin"
                            )
                              setLegacy(item);
                            else onOpen(routeId);
                          }}
                        >
                          打开路由
                        </button>
                      )}
                      {"route_id" in item ? (
                        <>
                          <button
                            className="secondary"
                            disabled={!editable || query.isError}
                            onClick={() => onEdit(item)}
                          >
                            编辑候选
                          </button>
                          <button
                            className="danger"
                            disabled={!editable || query.isError}
                            onClick={() => onDelete(item)}
                          >
                            删除候选
                          </button>
                        </>
                      ) : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : null}
      {legacy !== undefined ? (
        <ObjectInspector
          title="路由详情"
          scope={`配置 ${scope ?? "—"} · 已读取的路由记录`}
          onClose={() => setLegacy(undefined)}
          facts={[
            ["路由 ID", legacy.id],
            ["公开模型", legacy.public_model_id],
            ["调度策略", legacy.policy],
            ["最大尝试次数", legacy.max_attempts],
            ["启动超时（ms）", legacy.bootstrap_timeout_ms],
          ]}
        >
          <p className="stat-sub">
            此版本使用旧调度策略，当前路由编辑接口仅支持
            smooth_weighted_round_robin。候选可在配置资源的「候选」中维护。
          </p>
          <button
            className="secondary"
            onClick={() => {
              setLegacy(undefined);
              setOperation("listRouteCandidates");
            }}
          >
            查看候选
          </button>
        </ObjectInspector>
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
