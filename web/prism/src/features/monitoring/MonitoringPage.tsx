import { ResourcePicker, resourceFilterKinds } from "../../components/ResourcePicker";
import { resourceName } from "../../utils/resourceNames";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { ProcessingStatus } from "../billing/ProcessingStatus";
import { RequestHistoryPanel } from "./RequestHistory";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError, shouldRetryManagementRead } from "../../api/errors";
import { editedExpiry, toLocalInput } from "../access/model";
import { Sheet } from "../../components/Sheet";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import { buildJsonl, downloadText, exportFilename, toExportRow, type ExportMeta } from "./export";
import "./monitoring.css";
import {
  COST_CONFIDENCES,
  costConfidenceDetail,
  costConfidenceLabel,
  costConfidenceTone,
  errorCodeLabel,
  errorScopeLabel,
  exactShare,
  FAILURE_FILTER_KEYS,
  filterLabel,
  formatCount,
  formatMicrounits,
  formatPercent,
  formatTime,
  formatTokens,
  LEDGER_FILTER_KEYS,
  PAGE_LIMIT,
  parseFilters,
  retryDetail,
  retryLabel,
  retryTone,
  stageLabel,
  summaryIsPartitioned,
  tally,
  type AttemptRow,
  type BillingResponse,
  type FailureResponse,
  type FilterKey,
  type LedgerRow,
} from "./model";

function Chip({
  tone,
  label,
  raw,
  detail,
}: Readonly<{ tone: string; label: string; raw: string; detail: string }>) {
  return (
    <span className="mon-chip" data-tone={tone} title={`${raw} · ${detail}`}>
      {label}
      <span className="visually-hidden">({raw})</span>
    </span>
  );
}

function FilterForm({
  keys,
  values,
  onApply,
  onClear,
}: Readonly<{
  keys: readonly FilterKey[];
  values: Readonly<Record<string, string>>;
  onApply: (next: Readonly<Record<string, string>>) => void;
  onClear: () => void;
}>) {
  const [rangeError,setRangeError]=useState("");
  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const next=Object.fromEntries(keys.map(key=>{
      const raw=String(data.get(key)??"").trim();
      if(key==="from_ms"||key==="to_ms") {
        const previous=values[key]&&Number.isFinite(Number(values[key]))?Number(values[key]):null;
        return [key,String(editedExpiry(raw,previous)??"")];
      }
      return [key,raw];
    }));
    if(next.from_ms&&next.to_ms&&Number(next.from_ms)>Number(next.to_ms)){setRangeError("结束时间不得早于开始时间。");return;}
    setRangeError("");onApply(next);
  }
  return (
    <form className="card mon-filters" onSubmit={onSubmit}>
      {keys.map((key) =>
        key === "status" ? (
          <label key={key}>
            {filterLabel(key)}
            <select name={key} defaultValue={values[key] ?? ""}>
              <option value="">全部</option>
              {COST_CONFIDENCES.map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <label key={key}>
            {filterLabel(key)}
            {key==="from_ms"||key==="to_ms"?<input type="datetime-local" name={key} defaultValue={values[key]&&Number.isFinite(Number(values[key]))?toLocalInput(Number(values[key])):""}/>:resourceFilterKinds[key] ? <ResourcePicker key={values[key]??""} allowCustom name={key} kind={resourceFilterKinds[key]} defaultValue={values[key]??""} runtime={key==="account_id"}/> : <input name={key} className="mono" maxLength={256} defaultValue={values[key] ?? ""} />}
          </label>
        ),
      )}
      {rangeError?<p role="alert">{rangeError}</p>:null}
      <div className="mon-filter-actions">
        <button type="submit">应用筛选</button>
        <button type="button" className="secondary" onClick={onClear}>
          清除
        </button>
      </div>
    </form>
  );
}

function diagnosticTarget(endpoint: string, credential: string): string {
  return `/runtime?${new URLSearchParams({ endpoint_id: endpoint, credential_id: credential, account_id: credential })}`;
}

/** The per-request attempt trail. A bare array by contract — no cursor, no
 *  paging, no time filter — and not version-scoped. */
function AttemptsSheet({
  requestId,
  onClose,
}: Readonly<{ requestId: string; onClose: () => void }>) {
  const attempts = useQuery({
    queryKey: ["request-attempts", requestId],
    queryFn: () =>
      call<readonly AttemptRow[]>("listRequestAttempts", { path: { request_id: requestId } }),
    retry: shouldRetryManagementRead,
    retryDelay: (attempt) => 250 * (attempt + 1),
  });

  return (
    <Sheet title={`请求 ${requestId} 的尝试`} layout="inspector" onEscape={onClose}>
      <p className="stat-sub">
        此请求的上游尝试记录，跨配置版本读取。执行结果保留上游原始标识；未观测的阶段或对象显示为“—”。
      </p>
      {attempts.isError ? (
        <p role="alert" className="action-error">
          {asAppError(attempts.error).message}
        </p>
      ) : attempts.isPending ? (
        <p className="stat-sub">读取中…</p>
      ) : attempts.data.length === 0 ? (
        <p className="stat-sub">该请求没有尝试记录。</p>
      ) : (
        <table className="mon-attempts">
          <thead>
            <tr>
              <th scope="col">attempt</th>
              <th scope="col">outcome(原样)</th>
              <th scope="col">阶段</th>
              <th scope="col">endpoint</th>
              <th scope="col">credential</th>
            </tr>
          </thead>
          <tbody>
            {attempts.data.map((attempt) => (
              <tr key={attempt.attempt_id}>
                <td className="mono">{attempt.attempt_id}</td>
                <td className="mono">{attempt.outcome}</td>
                <td>
                  {attempt.stage === null || attempt.stage === undefined
                    ? "—"
                    : stageLabel(attempt.stage)}
                </td>
                <td>{attempt.endpoint_id ? <ResourceIdentity id={attempt.endpoint_id} kind="endpoint" /> : "—"}</td>
                <td>{attempt.credential_id ? <ResourceIdentity id={attempt.credential_id} kind="account" /> : "—"}
                  {attempt.endpoint_id && attempt.credential_id ? <div><Link to={diagnosticTarget(attempt.endpoint_id, attempt.credential_id)}>诊断此绑定</Link></div> : null}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="sheet-actions">
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </div>
    </Sheet>
  );
}

function LedgerPanel({
  filters,
  onApply,
  onClear,
}: Readonly<{
  filters: Readonly<Record<string, string>>;
  onApply: (next: Readonly<Record<string, string>>) => void;
  onClear: () => void;
}>) {
  const [drill, setDrill] = useState<string | undefined>();

  const ledger = useInfiniteQuery({
    // NOT version-scoped: listOperationalBilling declares no X-Config-Version.
    queryKey: ["billing", JSON.stringify(filters)],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      call<BillingResponse>("listOperationalBilling", {
        query: {
          ...filters,
          limit: PAGE_LIMIT,
          ...(pageParam === null ? {} : { cursor: pageParam }),
        },
      }),
    getNextPageParam: (last) => last.next_cursor,
    retry: shouldRetryManagementRead,
    retryDelay: (attempt) => 250 * (attempt + 1),
  });

  if (ledger.isError) {
    return (
      <div className="card empty-state" data-kind="error">
        <p>{asAppError(ledger.error).message}</p>
      </div>
    );
  }
  if (ledger.isPending) {
    return (
      <div className="card empty-state" data-kind="loading">
        <p>读取账本…</p>
      </div>
    );
  }

  const rows = ledger.data.pages.flatMap((page) => page.items);
  // Every page carries the same summary — the backend computes it over the
  // whole filtered set BEFORE the cursor applies — so page one is authoritative.
  const summary = ledger.data.pages[0]?.summary;

  function onExport(): void {
    const meta: ExportMeta = {
      row_count: rows.length,
      filters,
      partial: ledger.hasNextPage === true,
    };
    downloadText(
      exportFilename(meta),
      buildJsonl(
        meta,
        rows.map((row) => toExportRow(row)),
      ),
    );
  }

  return (
    <>
      <FilterForm keys={LEDGER_FILTER_KEYS} values={filters} onApply={onApply} onClear={onClear} />

      {summary === undefined ? null : (
        <div className="card mon-summary">
          <div className="mon-kpi">
            <span className="mon-kpi-value mono">{formatCount(summary.records)}</span>
            <span className="mon-kpi-label">账本记录</span>
          </div>
          <div className="mon-kpi">
            <span className="mon-kpi-value mono">{formatPercent(exactShare(summary))}</span>
            <span className="mon-kpi-label">成本精确占比</span>
          </div>
          <div className="mon-kpi">
            <span className="mon-kpi-value mono">
              {formatMicrounits(summary.known_cost_microunits)}
            </span>
            <span className="mon-kpi-label">已知成本(microunits)</span>
          </div>
          <p className="mon-note">
            这些数<strong>覆盖整个筛选窗口</strong>,不是当前已加载的页 ——
            后端在游标生效前就算完了。契约没有声明币种,所以成本只以{" "}
            <span className="mono">microunits</span> 原样显示,不折算也不加货币符号。
            {summaryIsPartitioned(summary) ? null : (
              <>
                <br />
                <strong>注意:四类置信度记录数之和不等于总记录数。</strong>
                后端把它们声明为一个划分,这里出现了偏差。
              </>
            )}
          </p>
          <ul className="mon-conf-breakdown">
            {COST_CONFIDENCES.map((value) => (
              <li key={value}>
                <Chip
                  tone={costConfidenceTone(value)}
                  label={costConfidenceLabel(value)}
                  raw={value}
                  detail={costConfidenceDetail(value)}
                />
                <span className="mono mon-num">
                  {formatCount(
                    summary[`${value}_records` as keyof typeof summary] as unknown as number,
                  )}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {rows.length === 0 ? (
        <div className="card empty-state" data-kind="empty">
          <p>该筛选下没有账本记录。</p>
        </div>
      ) : (
        <div className="card tablewrap">
          <div className="mon-toolbar">
            <button type="button" className="secondary" onClick={onExport}>
              导出 JSONL({formatCount(rows.length)} 行{ledger.hasNextPage === true ? " · 部分" : ""})
            </button>
          </div>
          <table className="mon-table">
            <thead>
              <tr>
                <th scope="col">时间(UTC)</th>
                <th scope="col">request</th>
                <th scope="col">模型</th>
                <th scope="col">Provider / Channel / 账号</th>
                <th scope="col">输入</th>
                <th scope="col">输出</th>
                <th scope="col">成本</th>
                <th scope="col">计价</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row: LedgerRow) => (
                <tr key={row.ledger_id}>
                  <td className="mono">{formatTime(row.occurred_at_ms)}</td>
                  <td>
                    <button
                      type="button"
                      className="linklike mono"
                      onClick={() => setDrill(row.request_id)}
                    >
                      {row.request_id}
                    </button>
                  </td>
                  <td className="mono">{row.model}</td>
                  <td className="mono mon-triple">
                    <ResourceIdentity id={row.provider_id} kind="upstream" /> / <ResourceIdentity id={row.channel_id} kind="endpoint" /> / <ResourceIdentity id={row.account_id} kind="account" />
                  </td>
                  <td className="mono mon-num">{formatTokens(row.input_tokens)}</td>
                  <td className="mono mon-num">{formatTokens(row.output_tokens)}</td>
                  <td className="mono mon-num">{formatMicrounits(row.cost_microunits)}</td>
                  <td>
                    <Chip
                      tone={costConfidenceTone(row.cost_confidence)}
                      label={costConfidenceLabel(row.cost_confidence)}
                      raw={row.cost_confidence}
                      detail={costConfidenceDetail(row.cost_confidence)}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {ledger.hasNextPage === true ? (
            <div className="mon-more">
              <button
                type="button"
                className="secondary"
                disabled={ledger.isFetchingNextPage}
                onClick={() => void ledger.fetchNextPage()}
              >
                {ledger.isFetchingNextPage ? "读取中…" : "再读一页"}
              </button>
            </div>
          ) : null}
        </div>
      )}

      {drill === undefined ? null : (
        <AttemptsSheet requestId={drill} onClose={() => setDrill(undefined)} />
      )}
    </>
  );
}

function FailurePanel({
  filters,
  onApply,
  onClear,
}: Readonly<{
  filters: Readonly<Record<string, string>>;
  onApply: (next: Readonly<Record<string, string>>) => void;
  onClear: () => void;
}>) {
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;

  const [drill, setDrill] = useState<string>();
  const failures = useInfiniteQuery({
    // IS version-scoped, unlike the ledger next door.
    queryKey: ["failures", scope, JSON.stringify(filters)],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      call<FailureResponse>(
        "listProviderAccountFailures",
        {
          query: {
            ...filters,
            limit: PAGE_LIMIT,
            ...(pageParam === null ? {} : { cursor: pageParam }),
          },
        },
        { versionScoped: true },
      ),
    getNextPageParam: (last) => last.next_cursor,
    enabled: scope !== undefined,
    retry: false,
  });

  if (scope === undefined) {
    return (
      <div className="card empty-state" data-kind="empty">
        <p>
          失败归因需要一个配置版本。
          <br />
          <small className="muted-3">
            这条投影带 <span className="mono">X-Config-Version</span>,而账本不带 ——
            两者的作用域不同,不是同一个总体的两半。
          </small>
        </p>
      </div>
    );
  }
  if (failures.isError) {
    return (
      <div className="card empty-state" data-kind="error">
        <p>{asAppError(failures.error).message}</p>
      </div>
    );
  }
  if (failures.isPending) {
    return (
      <div className="card empty-state" data-kind="loading">
        <p>读取失败归因…</p>
      </div>
    );
  }

  const rows = failures.data.pages.flatMap((page) => page.items);
  const byCode = tally(rows, "error_code");
  const byScope = tally(rows, "error_scope");

  return (
    <>
      {drill === undefined ? null : <AttemptsSheet requestId={drill} onClose={() => setDrill(undefined)} />}
      <FilterForm keys={FAILURE_FILTER_KEYS} values={filters} onApply={onApply} onClear={onClear} />

      <div className="card mon-summary">
        <div className="mon-kpi">
          <span className="mon-kpi-value mono">{formatCount(rows.length)}</span>
          <span className="mon-kpi-label">已加载失败尝试</span>
        </div>
        <p className="mon-note">
          下面的分布只统计<strong>已加载的这些行</strong> —— 这条投影没有服务端汇总,
          所以它不是整个窗口的分布。要更完整就继续翻页。
          <br />
          一次请求可以产生<strong>多条</strong>失败尝试,所以行数不是失败请求数。
        </p>
      </div>

      {rows.length === 0 ? (
        <div className="card empty-state" data-kind="empty">
          <p>该配置版本下没有归因到账号的失败尝试。</p>
        </div>
      ) : (
        <>
          <div className="card mon-breakdown">
            <div>
              <h3>按错误码</h3>
              <ul>
                {byCode.map((entry) => (
                  <li key={entry.key}>
                    <span className="mono">{entry.key}</span>
                    <span className="mon-bd-label">{errorCodeLabel(entry.key) ?? ""}</span>
                    <span className="mono mon-num">{formatCount(entry.count)}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <h3>按归因层</h3>
              <ul>
                {byScope.map((entry) => (
                  <li key={entry.key}>
                    <span className="mono">{entry.key}</span>
                    <span className="mon-bd-label">{errorScopeLabel(entry.key)}</span>
                    <span className="mono mon-num">{formatCount(entry.count)}</span>
                  </li>
                ))}
              </ul>
            </div>
          </div>

          <div className="card tablewrap">
            <table className="mon-table">
              <thead>
                <tr>
                  <th scope="col">结束时间(UTC)</th>
                  <th scope="col">request / attempt</th>
                  <th scope="col">Provider / Channel / 账号</th>
                  <th scope="col">错误码</th>
                  <th scope="col">归因层</th>
                  <th scope="col">重试决策</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr key={row.attempt_id}>
                    <td className="mono">{formatTime(row.ended_at_ms)}</td>
                    <td className="mono mon-triple">
                      <button type="button" className="linklike mono" onClick={() => setDrill(row.request_id)}>{row.request_id}</button>
                      <br />
                      {row.attempt_id}
                    </td>
                    <td className="mono mon-triple">
                      <ResourceIdentity id={row.provider_id} kind="upstream" /> / <ResourceIdentity id={row.channel_id} kind="endpoint" /> / <ResourceIdentity id={row.account_id} kind="account" />
                      <div><Link to={diagnosticTarget(row.channel_id, row.account_id)}>诊断此绑定</Link></div>
                    </td>
                    <td>
                      <span className="mono">{row.error_code}</span>
                      {errorCodeLabel(row.error_code) === undefined ? null : (
                        <span className="mon-bd-label">{errorCodeLabel(row.error_code)}</span>
                      )}
                    </td>
                    <td>{errorScopeLabel(row.error_scope)}</td>
                    <td>
                      <Chip
                        tone={retryTone(row.retry_decision)}
                        label={retryLabel(row.retry_decision)}
                        raw={row.retry_decision}
                        detail={retryDetail(row.retry_decision)}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {failures.hasNextPage === true ? (
              <div className="mon-more">
                <button
                  type="button"
                  className="secondary"
                  disabled={failures.isFetchingNextPage}
                  onClick={() => void failures.fetchNextPage()}
                >
                  {failures.isFetchingNextPage ? "读取中…" : "再读一页"}
                </button>
              </div>
            ) : null}
          </div>
        </>
      )}
    </>
  );
}

export function MonitoringPage() {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  const t = useMessages();
  const [params, setParams] = useSearchParams();
  const tab = params.get("tab")==="failures"?"failures":params.get("tab")==="ledger"||(!params.has("tab")&&["provider_id","channel_id","account_id","status"].some(key=>params.has(key)))?"ledger":"requests";
  const keys = tab === "ledger" ? LEDGER_FILTER_KEYS : FAILURE_FILTER_KEYS;
  const filters = parseFilters(keys, (key) => params.get(key));

  function patch(next: Readonly<Record<string, string | null>>): void {
    const merged = new URLSearchParams(params);
    for (const [key, value] of Object.entries(next)) {
      if (value === null || value === "") {
        merged.delete(key);
      } else {
        merged.set(key, value);
      }
    }
    setParams(merged, { replace: true });
  }

  return (
    <section className="monitoring-page">
      <header className="page-head">
        <h2>{t.nav.monitoring}</h2>
        <span className="scope-row">{tab === "requests" ? "请求记录 · 跨配置版本" : tab === "ledger" ? "账本 · 跨配置版本" : `失败归因 · ${scope === undefined ? "未选择版本" : resourceName(scope, "config")}`}</span>
      </header>
      <ProcessingStatus compact />

      <div className="mon-tabs" role="tablist">
        <button type="button" role="tab" aria-selected={tab==="requests"} onClick={()=>patch({tab:"requests"})}>请求记录</button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "ledger"}
          onClick={() => patch({ tab: "ledger" })}
        >
          计费账本
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "failures"}
          onClick={() => patch({ tab: "failures" })}
        >
          失败归因
        </button>
      </div>

      {tab === "requests" ? <RequestHistoryPanel/> : tab === "ledger" ? (
        <LedgerPanel
          filters={filters}
          onApply={(next) => patch(next)}
          onClear={() => patch(Object.fromEntries(keys.map((key) => [key, null])))}
        />
      ) : (
        <FailurePanel
          filters={filters}
          onApply={(next) => patch(next)}
          onClear={() => patch(Object.fromEntries(keys.map((key) => [key, null])))}
        />
      )}
    </section>
  );
}
