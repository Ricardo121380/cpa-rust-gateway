// Runtime (docs/07 §7.5) — three projections and one action, all against real
// contract operations:
//
//   getRuntimeAvailability  GET  /admin/runtime/availability     → six-state matrix
//   getCatalogStatus        GET  /admin/catalog/status           → freshness lifecycle
//   requestQuotaRecovery    POST /admin/runtime/quota/reset      → tri-state, inline
//   explainRoute            GET  /admin/routes/{id}/explain      → candidate decisions
//
// All four are INJECTED FACADES in the gateway and fail closed, so each panel
// carries its own "projection not enabled in this deployment" state, kept
// strictly distinct from "enabled and empty" and from "empty after filtering".
//
// Everything here is SOLID: cards, tables and the matrix are content, never
// glass. The page adds zero backdrop-filter panes to the shell's budget of 3.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { useMessages } from "../../i18n/messages";
import { useNowTick } from "../../utils/useNowTick";
import { useVersionStore } from "../config-versions/versionStore";
import {
  abnormalRows,
  ageStage,
  ageStageLabel,
  availabilityMeta,
  buildAvailabilityMatrix,
  cellKey,
  countByState,
  decisionMeta,
  explainCounts,
  formatAge,
  formatObservedAt,
  freshnessMeta,
  isProjectionUnavailable,
  normalizeExplainQuery,
  PROTOCOLS,
  recoverableRows,
  recoveryMeta,
  stateAttr,
  AVAILABILITY_STATES,
  CATALOG_REMOVAL_MISSES,
  type AvailabilityRow,
  type CatalogRow,
  type ExplainQuery,
  type Protocol,
  type RecoveryResponse,
  type RouteExplain,
  type StateMeta,
} from "./model";
import "./runtime.css";

const POLL_MS = 10_000;

/** Poll only while the tab is actually being looked at. */
function useDocumentVisible(): boolean {
  const [visible, setVisible] = useState(() => document.visibilityState === "visible");
  useEffect(() => {
    const onChange = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);
  return visible;
}

function clockUtc(ms: number | undefined): string {
  if (ms === undefined || ms === 0) {
    return "—";
  }
  return `${new Date(ms).toISOString().slice(11, 19)}Z`;
}

// ---------------------------------------------------------------------------
// shared state blocks — the four honest empties
// ---------------------------------------------------------------------------

type StateKind = "unavailable" | "empty" | "filtered" | "loading" | "error";

function StateBlock({
  kind,
  text,
  detail,
}: Readonly<{ kind: StateKind; text: string; detail?: string }>) {
  return (
    <div className="empty-state" data-kind={kind}>
      <p>
        {text}
        {detail !== undefined ? (
          <>
            <br />
            <small className="muted-3">{detail}</small>
          </>
        ) : null}
      </p>
    </div>
  );
}

function UnavailableBlock({ operation }: Readonly<{ operation: string }>) {
  const t = useMessages();
  return (
    <StateBlock
      kind="unavailable"
      text={t.state.unavailable}
      detail={`${operation} 为注入式投影,未接线时按契约失败关闭(503)—— 这不是“没有数据”,而是“这台部署不提供该投影”。`}
    />
  );
}

/** Chip carrying colour + shape + text; used by every projection on this page. */
function StateChip({
  meta,
  attr,
  raw,
}: Readonly<{ meta: StateMeta; attr: string; raw: string }>) {
  return (
    <span className="rt-chip" data-state={attr} title={`${raw} · ${meta.detail}`}>
      <span className="rt-glyph" aria-hidden="true">
        {meta.glyph}
      </span>
      {meta.label}
      <span className="visually-hidden">({raw})</span>
    </span>
  );
}

function CardHead({
  title,
  operation,
  help,
  aside,
}: Readonly<{ title: string; operation: string; help: ReactNode; aside?: ReactNode }>) {
  return (
    <div className="rt-head">
      <div className="rt-head-text">
        <h3>
          {title}
          <span className="rt-op mono">{operation}</span>
        </h3>
        <p className="rt-help">{help}</p>
      </div>
      {aside !== undefined ? <div className="rt-head-aside">{aside}</div> : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 1. availability matrix
// ---------------------------------------------------------------------------

function AvailabilityLegend() {
  return (
    <dl className="rt-legend">
      {AVAILABILITY_STATES.map((state) => {
        const meta = availabilityMeta(state);
        return (
          <div key={state} className="rt-legend-row">
            <dt>
              <StateChip meta={meta} attr={state} raw={state} />
            </dt>
            <dd className="mono rt-legend-enum">{state}</dd>
            <dd className="rt-legend-detail">{meta.detail}</dd>
          </div>
        );
      })}
      <div className="rt-legend-row">
        <dt>
          <span className="rt-chip" data-state="none">
            <span className="rt-glyph" aria-hidden="true">
              ·
            </span>
            无投影
          </span>
        </dt>
        <dd className="mono rt-legend-enum">—</dd>
        <dd className="rt-legend-detail">该 endpoint × credential 组合未出现在投影中(未绑定)</dd>
      </div>
    </dl>
  );
}

function AvailabilityMatrixCard({
  rows,
  onlyAbnormal,
  onToggle,
}: Readonly<{
  rows: readonly AvailabilityRow[];
  onlyAbnormal: boolean;
  onToggle: (next: boolean) => void;
}>) {
  const t = useMessages();
  const shown = onlyAbnormal ? abnormalRows(rows) : rows;
  const matrix = buildAvailabilityMatrix(shown);

  return (
    <div className="card tablewrap rt-card" data-gap="top">
      <CardHead
        title="可用性矩阵 · endpoint × credential"
        operation="getRuntimeAvailability"
        help="调度器对每个绑定组合的实时判定,六态闭集。颜色、形状与文字三重编码;单元格悬停显示枚举原值。"
        aside={
          <label className="rt-toggle">
            <input
              type="checkbox"
              checked={onlyAbnormal}
              onChange={(event) => onToggle(event.target.checked)}
            />
            仅显示异常
          </label>
        }
      />
      {matrix.endpoints.length === 0 ? (
        onlyAbnormal ? (
          <StateBlock
            kind="filtered"
            text={t.state.filteredEmpty}
            detail="全部组合当前为 available —— 取消过滤可看到完整矩阵。"
          />
        ) : (
          <StateBlock
            kind="empty"
            text={t.state.empty}
            detail="投影已启用,但这个配置版本下没有任何 endpoint × credential 绑定。"
          />
        )
      ) : (
        <>
          <div className="rt-matrix-scroll">
            <table className="rt-matrix">
              <thead>
                <tr>
                  <th scope="col" className="rt-corner">
                    endpoint \ credential
                  </th>
                  {matrix.credentials.map((credential) => (
                    <th key={credential} scope="col" className="mono">
                      {credential}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {matrix.endpoints.map((endpoint) => (
                  <tr key={endpoint}>
                    <th scope="row" className="mono rt-rowhead">
                      {endpoint}
                    </th>
                    {matrix.credentials.map((credential) => {
                      const cell = matrix.cells.get(cellKey(endpoint, credential));
                      if (cell === undefined) {
                        return (
                          <td key={credential} className="rt-cell">
                            <span
                              className="rt-chip"
                              data-state="none"
                              title="该组合未出现在可用性投影中(未绑定)"
                            >
                              <span className="rt-glyph" aria-hidden="true">
                                ·
                              </span>
                              无投影
                            </span>
                          </td>
                        );
                      }
                      return (
                        <td key={credential} className="rt-cell">
                          <StateChip
                            meta={availabilityMeta(cell.availability)}
                            attr={stateAttr(cell.availability)}
                            raw={cell.availability}
                          />
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <AvailabilityLegend />
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 2. quota recovery — inline on the rows that can carry it
// ---------------------------------------------------------------------------

type RecoveryOutcome =
  | Readonly<{ ok: true; state: string }>
  | Readonly<{ ok: false; message: string }>;

function RecoveryCard({
  rows,
  scope,
}: Readonly<{ rows: readonly AvailabilityRow[]; scope: string }>) {
  const t = useMessages();
  const targets = recoverableRows(rows);
  const [outcomes, setOutcomes] = useState<Readonly<Record<string, RecoveryOutcome>>>({});
  const [pendingKey, setPendingKey] = useState<string | undefined>();

  const recover = useMutation({
    mutationFn: (target: AvailabilityRow) =>
      call<RecoveryResponse>(
        "requestQuotaRecovery",
        {
          body: { endpoint_id: target.endpoint_id, credential_id: target.credential_id },
        },
        { versionScoped: true },
      ),
    onMutate: (target) => setPendingKey(cellKey(target.endpoint_id, target.credential_id)),
    onSuccess: (data, target) =>
      setOutcomes((current) => ({
        ...current,
        [cellKey(target.endpoint_id, target.credential_id)]: { ok: true, state: data.state },
      })),
    onError: (error, target) =>
      setOutcomes((current) => ({
        ...current,
        [cellKey(target.endpoint_id, target.credential_id)]: {
          ok: false,
          message: isProjectionUnavailable(error)
            ? t.state.unavailable
            : asAppError(error).message || "请求失败",
        },
      })),
    onSettled: () => setPendingKey(undefined),
  });

  return (
    <div className="card tablewrap rt-card" data-gap="top">
      <CardHead
        title="恢复探测"
        operation="requestQuotaRecovery"
        help={
          <>
            仅 <span className="mono">quota_blocked</span> 与{" "}
            <span className="mono">credential_forbidden</span> 两态提供入口。返回三态如实呈现:
            <span className="mono">probe_scheduled</span>(已排程探测)、
            <span className="mono">recovery_required</span>(已登记但未放行)、
            <span className="mono">rejected</span>(拒绝)。
            <strong className="rt-warn">
              真实部署当前恒返回 rejected —— 受控恢复仍须人工介入;此处的其他两态只在 fixture
              演示模式出现。
            </strong>
          </>
        }
      />
      {targets.length === 0 ? (
        <StateBlock
          kind="empty"
          text="当前没有可发起恢复的组合"
          detail="没有处于 quota_blocked 或 credential_forbidden 的绑定 —— 其余五态不接受此操作。"
        />
      ) : (
        <table>
          <thead>
            <tr>
              <th scope="col">endpoint</th>
              <th scope="col">credential</th>
              <th scope="col">当前状态</th>
              <th scope="col">操作</th>
              <th scope="col">结果</th>
            </tr>
          </thead>
          <tbody>
            {targets.map((row) => {
              const key = cellKey(row.endpoint_id, row.credential_id);
              const outcome = outcomes[key];
              return (
                <tr key={key}>
                  <td className="mono">{row.endpoint_id}</td>
                  <td className="mono">{row.credential_id}</td>
                  <td>
                    <StateChip
                      meta={availabilityMeta(row.availability)}
                      attr={stateAttr(row.availability)}
                      raw={row.availability}
                    />
                  </td>
                  <td className="row-actions">
                    <button
                      type="button"
                      className="secondary"
                      disabled={pendingKey !== undefined}
                      onClick={() => recover.mutate(row)}
                    >
                      {pendingKey === key ? "请求中…" : "发起恢复"}
                    </button>
                  </td>
                  <td>
                    {outcome === undefined ? (
                      <span className="muted-3">—</span>
                    ) : outcome.ok ? (
                      <StateChip
                        meta={recoveryMeta(outcome.state)}
                        attr={outcome.state}
                        raw={outcome.state}
                      />
                    ) : (
                      <span className="rt-error-text">{outcome.message}</span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      <p className="rt-footnote">
        操作作用于配置版本 <span className="mono">{scope}</span>,不改写配置,也不产生草稿修订。
      </p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// 3. catalog freshness
// ---------------------------------------------------------------------------

function CatalogCard({
  rows,
  nowMs,
  unavailable,
  loading,
}: Readonly<{
  rows: readonly CatalogRow[] | undefined;
  nowMs: number;
  unavailable: boolean;
  loading: boolean;
}>) {
  const t = useMessages();
  return (
    <div className="card tablewrap rt-card" data-gap="top">
      <CardHead
        title="目录新鲜度 · endpoint × credential"
        operation="getCatalogStatus"
        help={
          <>
            模型目录按凭据维度观测,生命周期为三段:<strong>6 小时</strong>内视为新鲜,
            超过 <strong>24 小时</strong>应触发刷新,超过 <strong>72 小时</strong>硬过期。
            条目移除需要<strong>同时</strong>满足连续 {CATALOG_REMOVAL_MISSES} 次探测缺失
            <strong>与 24 小时隔离期</strong> —— 单次缺失永远不会删除任何模型。
            「阶段」一列由观测时间戳与本地时钟推算,与后端给出的 freshness 并列显示,不覆盖它。
          </>
        }
      />
      {unavailable ? (
        <UnavailableBlock operation="getCatalogStatus" />
      ) : loading ? (
        <StateBlock kind="loading" text="加载目录新鲜度…" />
      ) : rows === undefined || rows.length === 0 ? (
        <StateBlock
          kind="empty"
          text={t.state.empty}
          detail="投影已启用,但尚无任何目录观测记录。"
        />
      ) : (
        <table>
          <thead>
            <tr>
              <th scope="col">endpoint</th>
              <th scope="col">credential</th>
              <th scope="col">freshness</th>
              <th scope="col">最近观测</th>
              <th scope="col">阶段(按时钟)</th>
              <th scope="col">观测时刻(UTC)</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const stage = ageStage(row.observed_at_ms, nowMs);
              return (
                <tr key={`${row.endpoint_id} ${row.credential_id}`}>
                  <td className="mono">{row.endpoint_id}</td>
                  <td className="mono">{row.credential_id}</td>
                  <td>
                    <StateChip
                      meta={freshnessMeta(row.freshness)}
                      attr={row.freshness}
                      raw={row.freshness}
                    />
                  </td>
                  <td className="mono">{formatAge(row.observed_at_ms, nowMs)}</td>
                  <td className="rt-stage" data-stage={stage}>
                    {stage === "unobserved" ? "—" : ageStageLabel(stage)}
                  </td>
                  <td className="mono muted">{formatObservedAt(row.observed_at_ms)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 4. route explain
//
// COMPONENT BOUNDARY. `RouteExplainResult` owns the *presentation* of one
// explain response and nothing else — it takes a RouteExplain and renders it.
// The signature "Route Prism" light-path visual (docs/07 §8.5) is a later
// piece, and it drops in exactly here, behind this same prop, against this same
// contract shape: no new endpoint, no new query, no change above this line.
// ---------------------------------------------------------------------------

function RouteExplainResult({ explain }: Readonly<{ explain: RouteExplain }>) {
  const counts = explainCounts(explain.candidates);
  return (
    <>
      <p className="rt-explain-summary">
        <span className="mono">{explain.route_id}</span> · 选中 {counts.selected} / 排除{" "}
        {counts.excluded}
        {counts.other > 0 ? ` / 其他 ${counts.other}` : ""}
      </p>
      {explain.candidates.length === 0 ? (
        <StateBlock
          kind="empty"
          text="该路由没有候选"
          detail="路由存在但候选集为空 —— 请求会以 route_missing_active_candidate 失败。"
        />
      ) : (
        <table>
          <thead>
            <tr>
              <th scope="col">candidate</th>
              <th scope="col">决策</th>
              <th scope="col">原因(闭集)</th>
            </tr>
          </thead>
          <tbody>
            {explain.candidates.map((candidate) => (
              <tr key={candidate.candidate_id}>
                <td className="mono">{candidate.candidate_id}</td>
                <td>
                  <StateChip
                    meta={decisionMeta(candidate.decision)}
                    attr={candidate.decision}
                    raw={candidate.decision}
                  />
                </td>
                <td className="mono">
                  {candidate.reason === null || candidate.reason === undefined
                    ? "—"
                    : candidate.reason}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </>
  );
}

function ExplainCard({ scope }: Readonly<{ scope: string }>) {
  const [form, setForm] = useState<ExplainQuery>({
    route_id: "",
    requested_model: "",
    protocol: "openai_responses",
  });
  const [submitted, setSubmitted] = useState<ExplainQuery | undefined>();

  const explain = useQuery({
    queryKey: [
      "route-explain",
      scope,
      submitted?.route_id,
      submitted?.requested_model,
      submitted?.protocol,
    ],
    queryFn: () =>
      call<RouteExplain>(
        "explainRoute",
        {
          path: { route_id: submitted?.route_id ?? "" },
          query: {
            requested_model: submitted?.requested_model ?? "",
            protocol: submitted?.protocol ?? "openai_responses",
          },
        },
        { versionScoped: true },
      ),
    enabled: submitted !== undefined,
    retry: false,
  });

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitted(normalizeExplainQuery(form));
  }

  const error = explain.error;

  return (
    <div className="card rt-card" data-gap="top">
      <CardHead
        title="路由解释 · Explain"
        operation="explainRoute"
        help={
          <>
            对一次假想请求求解候选:返回 candidate_id 与闭集决策,永远不含请求原文。
            这是紧凑视图;完整的 Route Prism 光路图为后续签名件,数据形状与此处完全一致。
          </>
        }
      />
      <form className="rt-explain-form" onSubmit={onSubmit}>
        <label>
          route_id
          <input
            className="mono"
            required
            maxLength={128}
            placeholder="route-minimax-m3"
            value={form.route_id}
            onChange={(event) => setForm({ ...form, route_id: event.target.value })}
          />
        </label>
        <label>
          请求模型
          <input
            className="mono"
            required
            maxLength={256}
            placeholder="minimax-m3"
            value={form.requested_model}
            onChange={(event) => setForm({ ...form, requested_model: event.target.value })}
          />
        </label>
        <label>
          协议
          <select
            value={form.protocol}
            onChange={(event) =>
              setForm({ ...form, protocol: event.target.value as Protocol })
            }
          >
            {PROTOCOLS.map((protocol) => (
              <option key={protocol} value={protocol}>
                {protocol}
              </option>
            ))}
          </select>
        </label>
        <button type="submit" disabled={explain.isFetching}>
          {explain.isFetching ? "解释中…" : "解释"}
        </button>
      </form>

      {submitted === undefined ? (
        <StateBlock
          kind="empty"
          text="填入 route_id 与请求模型后解释"
          detail="面板没有路由清单可选:列出路由需要 G1(配置全图读取),契约尚未提供。"
        />
      ) : error !== null && error !== undefined ? (
        isProjectionUnavailable(error) ? (
          <UnavailableBlock operation="explainRoute" />
        ) : (
          <StateBlock
            kind="error"
            text="解释失败"
            detail={`${asAppError(error).code} · ${asAppError(error).message}`}
          />
        )
      ) : explain.data === undefined ? (
        <StateBlock kind="loading" text="求解候选…" />
      ) : (
        <RouteExplainResult explain={explain.data} />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// page
// ---------------------------------------------------------------------------

export function RuntimePage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;
  const visible = useDocumentVisible();
  const nowMs = useNowTick(60_000);
  const [onlyAbnormal, setOnlyAbnormal] = useState(false);

  const availability = useQuery({
    queryKey: ["runtime-availability", scope],
    queryFn: () =>
      call<AvailabilityRow[]>("getRuntimeAvailability", {}, { versionScoped: true }),
    enabled: scope !== undefined,
    // Poll while the tab is visible; stop dead when it is hidden.
    refetchInterval: visible ? POLL_MS : false,
    refetchIntervalInBackground: false,
    placeholderData: (previous) => previous,
  });

  const catalog = useQuery({
    queryKey: ["catalog-status", scope],
    queryFn: () => call<CatalogRow[]>("getCatalogStatus", {}, { versionScoped: true }),
    enabled: scope !== undefined,
    staleTime: 30_000,
  });

  if (scope === undefined) {
    return (
      <section className="runtime-page">
        <h2>{t.nav.runtime}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本。</p>
        </div>
      </section>
    );
  }

  const rows = availability.data ?? [];
  const availabilityUnavailable = isProjectionUnavailable(availability.error);
  const catalogUnavailable = isProjectionUnavailable(catalog.error);
  const counts = countByState(rows);

  return (
    <section className="runtime-page">
      <header className="page-head">
        <h2>{t.nav.runtime}</h2>
        <div className="page-actions rt-poll">
          <span className="rt-poll-state" data-live={visible && !availabilityUnavailable}>
            {availabilityUnavailable
              ? "投影未启用,已停止轮询"
              : visible
                ? `每 ${POLL_MS / 1000} 秒刷新 · ${clockUtc(availability.dataUpdatedAt)}`
                : "页面不可见,轮询已暂停"}
          </span>
          <button
            type="button"
            className="secondary"
            onClick={() => {
              void queryClient.invalidateQueries({ queryKey: ["runtime-availability", scope] });
              void queryClient.invalidateQueries({ queryKey: ["catalog-status", scope] });
            }}
          >
            立即刷新
          </button>
        </div>
      </header>

      {availabilityUnavailable ? (
        <div className="card rt-card" data-gap="top">
          <CardHead
            title="可用性矩阵 · endpoint × credential"
            operation="getRuntimeAvailability"
            help="调度器对每个绑定组合的实时判定,六态闭集。"
          />
          <UnavailableBlock operation="getRuntimeAvailability" />
        </div>
      ) : availability.data === undefined ? (
        <div className="card rt-card" data-gap="top">
          <StateBlock kind="loading" text="加载可用性投影…" />
        </div>
      ) : (
        <>
          {counts.length > 0 ? (
            <div className="rt-summary" data-gap="top">
              {counts.map((entry) => {
                const meta = availabilityMeta(entry.state);
                return (
                  <span key={entry.state} className="rt-count" data-state={stateAttr(entry.state)}>
                    <span className="rt-glyph" aria-hidden="true">
                      {meta.glyph}
                    </span>
                    <span className="rt-count-value mono">{entry.count}</span>
                    <span className="rt-count-label">{meta.label}</span>
                    <span className="mono rt-count-enum">{entry.state}</span>
                  </span>
                );
              })}
            </div>
          ) : null}

          <AvailabilityMatrixCard
            rows={rows}
            onlyAbnormal={onlyAbnormal}
            onToggle={setOnlyAbnormal}
          />
          <RecoveryCard rows={rows} scope={scope} />
        </>
      )}

      <CatalogCard
        rows={catalog.data}
        nowMs={nowMs}
        unavailable={catalogUnavailable}
        loading={catalog.isLoading}
      />

      <ExplainCard scope={scope} />
    </section>
  );
}
