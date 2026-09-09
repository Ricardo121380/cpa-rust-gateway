import { useQuery } from "@tanstack/react-query";
import { call } from "../../api/client";
import { ReadStatus } from "../../components/ReadStatus";

export type BillingProcessingStatus = Readonly<{
  state:
    | "disabled"
    | "starting"
    | "current"
    | "catching_up"
    | "needs_repair"
    | "failed"
    | "stopped";
  observed_at_ms: number | null;
  source_ordinal: number | null;
  checkpoint_ordinal: number | null;
  checkpoint_updated_at_ms: number | null;
  unresolved_failures: number | null;
  failure_code: "batch_unavailable" | null;
}>;

const copy: Record<
  BillingProcessingStatus["state"],
  readonly [string, string]
> = {
  disabled: ["未启用", "计费消费未启动，账本不能代表全部已发生用量。"],
  starting: ["启动中", "正在读取持久事件，尚无成功处理观测。"],
  current: ["已追平", "已处理当前观测到的持久事件，且没有待修复记录。"],
  catching_up: ["处理中", "持久事件仍有积压，账本暂不完整。"],
  needs_repair: [
    "待修复",
    "部分事件已保留供重试；即使水位追平，账本仍可能缺少这些事件。",
  ],
  failed: [
    "处理异常",
    "最近批次未完成。以下保留上次成功观测，持久事件将在恢复后继续处理。",
  ],
  stopped: ["已停止", "后台消费已停止，重启后从持久进度继续。"],
};

export function ProcessingStatus() {
  const query = useQuery({
    queryKey: ["billing-processing"],
    queryFn: () =>
      call<BillingProcessingStatus>("getBillingProcessingStatus", {}),
    retry: false,
    refetchInterval: (query) =>
      query.state.status === "error" ? false : 5_000,
  });
  const data = query.data;
  return (
    <aside className="data-panel data-panel--padded" aria-label="计费处理状态" data-gap="top">
      <header className="page-head">
        <h3>计费处理</h3>
        <button
          className="secondary"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          刷新处理状态
        </button>
      </header>
      <ReadStatus
        pending={query.isPending}
        error={query.error}
        hasData={data !== undefined}
        retry={() => void query.refetch()}
      />
      {data !== undefined ? (
        <>
          <p>
            <strong>{copy[data.state][0]}</strong> · {copy[data.state][1]}
          </p>
          <details>
            <summary>处理水位与观测</summary>
            <dl className="fact-grid">
              <div>
                <dt>已处理事件序号</dt>
                <dd>{data.checkpoint_ordinal ?? "未观测"}</dd>
              </div>
              <div>
                <dt>源事件水位</dt>
                <dd>{data.source_ordinal ?? "未观测"}</dd>
              </div>
              <div>
                <dt>待修复事件</dt>
                <dd>{data.unresolved_failures ?? "未观测"}</dd>
              </div>
              <div>
                <dt>上次成功观测</dt>
                <dd>
                  {data.observed_at_ms === null
                    ? "未观测"
                    : new Date(data.observed_at_ms).toLocaleString()}
                </dd>
              </div>
            </dl>
          </details>
          {data.failure_code !== null ? (
            <p className="small mono">{data.failure_code}</p>
          ) : null}
        </>
      ) : null}
      <p className="small muted">处理状态跨配置版本；空账本不等于零消费。</p>
    </aside>
  );
}
