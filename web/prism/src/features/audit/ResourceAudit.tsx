import { ResourceIdentity } from "../../components/ResourceIdentity";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { ReadStatus } from "../../components/ReadStatus";
import { ObjectInspector } from "../../components/ObjectInspector";
import { useVersionStore } from "../config-versions/versionStore";

type Event = Readonly<{
  id: string;
  action: string;
  actor: string;
  occurred_at_ms: number;
  config_version_id: string;
  resource_kind: string;
  resource_id: string;
}>;
type Page = Readonly<{
  items: readonly Event[];
  next_before_id: string | null;
}>;

export function ResourceAudit() {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  const [selected, setSelected] = useState<Event>();
  const client = useQueryClient();
  const key = ["resource-audit", scope];
  const query = useInfiniteQuery({
    queryKey: key,
    enabled: scope !== undefined,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      call<Page>(
        "listManagementResourceAuditEvents",
        {
          query: {
            limit: 50,
            ...(pageParam === undefined ? {} : { before_id: pageParam }),
          },
        },
        { versionScoped: true },
      ),
    getNextPageParam: (page) => page.next_before_id ?? undefined,
    retry: false,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <section
      className="data-panel data-panel--padded"
      aria-label="资源修改审计"
      data-gap="top"
    >
      <header className="page-head">
        <h3>资源修改审计</h3>
        <button
          className="secondary"
          disabled={!scope || query.isFetching}
          onClick={() => {
            setSelected(undefined);
            void client.resetQueries({ queryKey: key });
          }}
        >
          刷新资源审计
        </button>
      </header>
      <p className="stat-sub">
        配置 {scope ? <ResourceIdentity id={scope} kind="config" /> : "未选择"} · 已载入 {rows.length} 条 ·
        最新在前，仅记录安全操作元数据。
      </p>
      {!scope ? (
        <p className="empty-state">选择配置版本以查看资源修改。</p>
      ) : (
        <>
          <ReadStatus
            pending={query.isPending}
            error={query.error}
            hasData={query.data !== undefined}
            retry={() => void client.resetQueries({ queryKey: key })}
          />
          {rows.length > 0 ? (
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>动作</th>
                    <th>资源</th>
                    <th>执行者</th>
                    <th>时间</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((event) => (
                    <tr key={event.id}>
                      <td className="mono">{event.action}</td>
                      <td>
                        {event.resource_kind}
                        <br />
                        <span><ResourceIdentity id={event.resource_id} kind="resource" /></span>
                      </td>
                      <td>{event.actor}</td>
                      <td>{new Date(event.occurred_at_ms).toLocaleString()}</td>
                      <td>
                        <button
                          className="secondary"
                          onClick={() => setSelected(event)}
                        >
                          查看修改记录
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : !query.isPending && !query.isError ? (
            <p className="empty-state">此版本暂无资源修改记录。</p>
          ) : null}
          {query.hasNextPage ? (
            <button
              className="secondary"
              disabled={query.isFetching || query.isError}
              onClick={() => void query.fetchNextPage()}
            >
              加载更早记录
            </button>
          ) : null}
        </>
      )}
      {selected ? (
        <ObjectInspector
          title="资源修改记录"
          scope={`配置 ${selected.config_version_id}`}
          onClose={() => setSelected(undefined)}
          facts={[
            ["追加 ID", selected.id],
            ["动作", selected.action],
            ["资源类型", selected.resource_kind],
            ["资源 ID", selected.resource_id],
            ["执行者", selected.actor],
            ["发生时间", new Date(selected.occurred_at_ms).toLocaleString()],
          ]}
        />
      ) : null}
    </section>
  );
}
