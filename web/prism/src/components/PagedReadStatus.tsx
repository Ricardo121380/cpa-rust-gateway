import { asAppError } from "../api/errors";

/** Keep the last successful snapshot visible. A cursor conflict requires a new
 * snapshot; a transient next-page failure retries only that cursor. */
export function pagedRecovery(error: unknown, hasData: boolean, nextPage: boolean) {
  if (asAppError(error).kind === "conflict") return { title: "读取快照已变化", action: "从头重新读取", nextPage: false };
  if (hasData && nextPage) return { title: "下一页读取失败", action: "重试下一页", nextPage: true };
  return { title: hasData ? "刷新失败" : "读取失败", action: "重新读取", nextPage: false };
}

export function PagedReadStatus({ query }: { query: {
  data: unknown; error: unknown; isPending: boolean; isFetching: boolean;
  isFetchNextPageError?: boolean; dataUpdatedAt: number;
  refetch: () => Promise<unknown>; fetchNextPage?: () => Promise<unknown>;
} }) {
  if (!query.error) return query.isPending ? <p role="status">正在读取…</p> : null;
  const hasData = query.data !== undefined;
  const recovery = pagedRecovery(query.error, hasData, query.isFetchNextPageError === true);
  const retry = recovery.nextPage && query.fetchNextPage ? query.fetchNextPage : query.refetch;
  return <div className="card read-status" role="alert">
    <strong>{recovery.title}</strong>
    <p className="small">{hasData ? "已保留先前记录与筛选。" : "暂时无法取得记录，筛选条件已保留。"}{hasData && query.dataUpdatedAt > 0 ? `上次成功读取：${new Date(query.dataUpdatedAt).toLocaleString()}。` : ""}</p>
    <button className="secondary" disabled={query.isFetching} onClick={() => void retry()}>{query.isFetching ? "正在读取…" : recovery.action}</button>
  </div>;
}
