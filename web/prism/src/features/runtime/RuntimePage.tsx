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
import { useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { useMessages } from "../../i18n/messages";
import { useNowTick } from "../../utils/useNowTick";
import { useVersionStore } from "../config-versions/versionStore";
import { Sheet } from "../../components/Sheet";
import { CredentialSheet } from "../upstreams/CredentialSheet";
import {
  abnormalRows,
  ageStage,
  ageStageLabel,
  availabilityMeta,
  buildAvailabilityMatrix,
  cellKey,
  countByState,
  authStatusMeta,
  COOLDOWN_MAX_MS,
  COOLDOWN_MIN_MS,
  decisionMeta,
  explainCounts,
  explainScopeHint,
  formatDue,
  receiptMeta,
  runtimeStatusMeta,
  validCooldown,
  formatAge,
  formatObservedAt,
  freshnessMeta,
  isProjectionUnavailable,
  normalizeExplainQuery,
  priceEvidenceMeta,
  PROTOCOLS,
  recoverableRows,
  recoveryMeta,
  stateAttr,
  AVAILABILITY_STATES,
  CATALOG_REMOVAL_MISSES,
  type ActionReceipt,
  type AvailabilityRow,
  type CatalogRow,
  type PoolAccount,
  type PoolAction,
  type PoolSnapshot,
  type ExplainQuery,
  type Protocol,
  type RecoveryResponse,
  type RouteExplain,
  type StateMeta,
} from "./model";
import "./runtime.css";

const POLL_MS = 10_000;

/**
 * A credential id that opens its detail sheet.
 *
 * These projections are the only production enumeration of credentials until
 * G1 lands (there is no listCredentials), so this is where the credential
 * surface hangs. Each button owns its own sheet state: the two call sites are
 * in different tables and only one can be clicked at a time, so a page-level
 * selection would be plumbing for nothing.
 */
function CredentialButton({ id }: Readonly<{ id: string }>) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" className="idbtn mono" onClick={() => setOpen(true)}>
        {id}
      </button>
      {open ? <CredentialSheet credentialId={id} onClose={() => setOpen(false)} /> : null}
    </>
  );
}

/** Poll only while the tab is actually being looked at.
 *
 * NOT for saving requests — TanStack already skips interval fetches while
 * document.visibilityState is "hidden" (queryObserver.js:215). This drives the
 * visible poll-state indicator below, which is a different job. See
 * DESIGN.md §13.6 before "fixing" the other polling pages. */
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
                      <CredentialButton id={credential} />
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
                  <td className="mono">
                    <CredentialButton id={row.credential_id} />
                  </td>
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
      {/* price_policy is required and nullable. `null` is a real answer — the
          policy is off — and must not read as "missing data" or as zero. */}
      <p className="rt-price-policy">
        {explain.price_policy === null ? (
          <>
            价格策略<strong>未启用</strong> —— 本次没有做任何费率比较,下表的证据列会全部是
            <span className="mono"> disabled</span>。
          </>
        ) : (
          <>
            价格证据绑定目录{" "}
            <span className="mono">{explain.price_policy.catalog_version_id}</span>,比较方式{" "}
            <span className="mono">{explain.price_policy.comparison}</span>。
            比较的是<strong>费率</strong>,不是本次请求的花费。
          </>
        )}
      </p>
      {explain.candidates.length === 0 ? (
        <StateBlock
          kind="empty"
          text="该路由没有候选"
          detail="路由存在但候选集为空 —— 请求会以 route_missing_active_candidate 失败。在「公开模型」页的路由工作台里加一个候选。"
        />
      ) : (
        <table>
          <thead>
            <tr>
              <th scope="col">candidate</th>
              <th scope="col">决策</th>
              <th scope="col">价格证据(闭集)</th>
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
                <td>
                  <StateChip
                    meta={priceEvidenceMeta(candidate.price_evidence)}
                    attr={candidate.price_evidence}
                    raw={candidate.price_evidence}
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
  // Explain resolves against a COMPILED SNAPSHOT
  // (apps/gateway/src/runtime.rs::explain_route calls snapshot_for first), and
  // only a published version has one. On a draft the facade answers 503 —
  // identical on the wire to "this deployment does not wire the projection",
  // which is what this card used to claim. Measured against a real gateway on
  // 2026-08-18: a draft with a perfectly valid route still 503s. The panel
  // knows which case it is in, so it says.
  const isDraft = useVersionStore((s) => s.context?.status) !== "active";
  const [params] = useSearchParams();
  const [form, setForm] = useState<ExplainQuery>({
    // The workbench on the models page links here with the route it just fixed;
    // routes are not enumerable, so a deep link is the only way to arrive with
    // the id already filled in.
    route_id: params.get("route_id") ?? "",
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
      submitted?.provider_id,
    ],
    queryFn: () =>
      call<RouteExplain>(
        "explainRoute",
        {
          path: { route_id: submitted?.route_id ?? "" },
          query: {
            requested_model: submitted?.requested_model ?? "",
            protocol: submitted?.protocol ?? "openai_responses",
            // Optional parameter: sending it empty is not the same as omitting
            // it, so normalizeExplainQuery drops the key entirely when blank.
            ...(submitted?.provider_id === undefined
              ? {}
              : { provider_id: submitted.provider_id }),
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
  const scopeHint =
    error === null || error === undefined ? undefined : explainScopeHint(asAppError(error).code);

  return (
    <div className="card rt-card" data-gap="top">
      <CardHead
        title="路由解释 · Explain"
        operation="explainRoute"
        help={
          <>
            对一次假想请求求解候选:返回 candidate_id、闭集决策与价格证据,永远不含请求原文。
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
        <label>
          provider_id(跨 Provider 的路由必填)
          <input
            className="mono"
            maxLength={128}
            placeholder="留空 = 单 Provider 路由"
            value={form.provider_id ?? ""}
            onChange={(event) => setForm({ ...form, provider_id: event.target.value })}
          />
        </label>
        <button type="submit" disabled={explain.isFetching}>
          {explain.isFetching ? "解释中…" : "解释"}
        </button>
      </form>

      {submitted === undefined ? (
        <StateBlock
          kind="empty"
          text="填入 route_id 与请求模型后解释"
          detail="契约没有 listRoutes,面板给不出路由清单 —— route_id 需要手输,或从「公开模型」页的路由工作台跳过来。"
        />
      ) : error !== null && error !== undefined ? (
        isProjectionUnavailable(error) ? (
          isDraft ? (
            <StateBlock
              kind="unavailable"
              text="草稿版本没有可解释的快照"
              detail="Explain 对已编译快照求解,而快照只在版本发布后存在 —— 草稿上它按契约失败关闭(503),与“本部署未接线”在协议层无法区分。先发布该版本,或改选一个 active 版本。"
            />
          ) : (
            <UnavailableBlock operation="explainRoute" />
          )
        ) : (
          <StateBlock
            kind="error"
            text={scopeHint === undefined ? "解释失败" : "需要显式指定 Provider"}
            detail={scopeHint ?? `${asAppError(error).code} · ${asAppError(error).message}`}
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

// ---------------------------------------------------------------------------
// 5. provider account pools (P13-06B/C)
//
// SCOPE SPLIT, stated on screen because it is genuinely surprising: the LIST
// declares no X-Config-Version and works with nothing selected, while the
// ACTION requires one. Reading is always possible; acting is not. Letting an
// operator find that out from a failed POST would be poor manners.
// ---------------------------------------------------------------------------

function PoolActionSheet({
  account,
  action,
  pending,
  onCancel,
  onInvalid,
  onSubmit,
}: Readonly<{
  account: PoolAccount;
  action: PoolAction;
  pending: boolean;
  onCancel: () => void;
  onInvalid: (message: string) => void;
  onSubmit: (body: Readonly<Record<string, unknown>>) => void;
}>) {
  const isCooldown = action === "cool_down";
  return (
    <Sheet title={isCooldown ? "冷却这个账号" : "为这个账号请求恢复"} onEscape={onCancel}>
      <p className="reveal-warning">
        作用对象是<strong>精确到账号</strong>的一条:
        <br />
        <span className="mono">
          {account.provider_id} / {account.channel_id} / {account.account_id}
        </span>
        <br />
        {isCooldown
          ? "冷却会把它移出调度直到到期,同 Provider 下的其他账号继续服务。"
          : "请求恢复只是登记意图 —— 是否放行仍由运行时与上游决定,不保证恢复。"}
      </p>
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const body: Record<string, unknown> = {
            provider_id: account.provider_id,
            channel_id: account.channel_id,
            account_id: account.account_id,
            action,
          };
          const model = String(data.get("upstream_model") ?? "").trim();
          if (model !== "") {
            body["upstream_model"] = model;
          }
          if (isCooldown) {
            const ms = Number(data.get("cooldown_ms"));
            if (!validCooldown(ms)) {
              onInvalid(
                `冷却时长越界:契约要求 ${COOLDOWN_MIN_MS}–${COOLDOWN_MAX_MS} 毫秒(1 秒–24 小时)。`,
              );
              return;
            }
            body["cooldown_ms"] = ms;
          }
          onSubmit(body);
        }}
      >
        {isCooldown ? (
          <label>
            冷却时长(毫秒,{COOLDOWN_MIN_MS}–{COOLDOWN_MAX_MS})
            <input
              name="cooldown_ms"
              type="number"
              min={COOLDOWN_MIN_MS}
              max={COOLDOWN_MAX_MS}
              defaultValue={60_000}
            />
            <small>留空不是"用默认值" —— 契约的字段可空,但这里必须给一个明确时长。</small>
          </label>
        ) : null}
        <label>
          upstream_model(可选)
          <input name="upstream_model" className="mono" maxLength={256} />
          <small>只想影响某一个上游模型时填写;留空表示整个账号。</small>
        </label>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" className={isCooldown ? "danger" : undefined} disabled={pending}>
            {isCooldown ? "确认冷却" : "确认请求恢复"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}

function ProviderPoolCard({ nowMs }: Readonly<{ nowMs: number }>) {
  const queryClient = useQueryClient();
  const scope = useVersionStore((s) => s.context?.configVersionId);
  const visible = useDocumentVisible();
  const [target, setTarget] = useState<
    Readonly<{ account: PoolAccount; action: PoolAction }> | undefined
  >();
  const [receipt, setReceipt] = useState<ActionReceipt | undefined>();
  const [error, setError] = useState<string | undefined>();

  const pools = useQuery({
    // NOT version-scoped: this is live runtime state, not configuration.
    queryKey: ["provider-pools"],
    queryFn: () => call<PoolSnapshot>("listProviderAccountPools", { query: { limit: 100 } }),
    refetchInterval: visible ? POLL_MS : false,
    refetchIntervalInBackground: false,
    placeholderData: (previous) => previous,
    retry: false,
  });

  const act = useMutation({
    mutationFn: (body: Readonly<Record<string, unknown>>) =>
      call<ActionReceipt>(
        "applyProviderAccountPoolAction",
        { body },
        // Version-scoped, but NO If-Match: it acts on runtime, not on config,
        // so there is no revision to guard.
        { versionScoped: true },
      ),
    onSuccess: (result) => {
      setTarget(undefined);
      setReceipt(result);
      void queryClient.invalidateQueries({ queryKey: ["provider-pools"] });
    },
    onError: (cause) => {
      const app = asAppError(cause);
      setTarget(undefined);
      // A 409 means the snapshot moved under us. Re-read before retrying, and
      // say so instead of leaving a stale table on screen.
      if (app.kind === "conflict") {
        void queryClient.invalidateQueries({ queryKey: ["provider-pools"] });
        setError("目标已过期(快照已变)—— 已重新读取,请确认后重试。");
        return;
      }
      setError(app.message);
    },
  });

  const rows = pools.data?.items ?? [];

  return (
    <div className="card rt-card" data-gap="top">
      <CardHead
        title="Provider 账号池 · 实时"
        operation="listProviderAccountPools"
        help={
          <>
            认证状态与运行时状态是<strong>两个独立维度</strong>,本表不把它们合成一个"健康"值 ——
            一个账号可以认证正常而运行时正在冷却,反之亦然。
          </>
        }
      />
      <p className="rt-help">
        <strong>本表不需要配置版本</strong>(它是实时状态),但下面的操作需要:
        <span className="mono"> applyProviderAccountPoolAction</span> 带{" "}
        <span className="mono">X-Config-Version</span>。
        {scope === undefined ? <strong> 当前未选择版本,操作按钮不可用。</strong> : null}
      </p>

      {error !== undefined ? (
        <p role="alert" className="action-error">
          {error}
          <button type="button" onClick={() => setError(undefined)}>
            清除
          </button>
        </p>
      ) : null}
      {receipt === undefined ? null : (
        <p className="action-notice">
          <StateChip
            meta={receiptMeta(receipt.state)}
            attr={receipt.state}
            raw={receipt.state}
          />{" "}
          {receiptMeta(receipt.state).detail}
          {receipt.cooldown_until_ms === null
            ? ""
            : ` · 冷却至 ${formatObservedAt(receipt.cooldown_until_ms)}`}
          <button type="button" onClick={() => setReceipt(undefined)}>
            知道了
          </button>
        </p>
      )}

      {pools.isError ? (
        isProjectionUnavailable(pools.error) ? (
          <UnavailableBlock operation="listProviderAccountPools" />
        ) : (
          <StateBlock
            kind="error"
            text="读取失败"
            // Measured against a real gateway on 2026-08-20: an unwired pool
            // source maps to internal_error() (500), NOT the 503 every other
            // injected projection here uses when it is not enabled. So a 500
            // on this one read is ambiguous, and the panel says which two
            // things it could be instead of implying a gateway defect.
            detail={`${asAppError(pools.error).code} · ${asAppError(pools.error).message}${
              asAppError(pools.error).status === 500
                ? " —— 注意:本投影未接线时也返回 500(其余投影用 503),所以这既可能是真的内部错误,也可能是这台部署没有提供账号池来源。"
                : ""
            }`}
          />
        )
      ) : pools.data === undefined ? (
        <StateBlock kind="loading" text="读取账号池…" />
      ) : rows.length === 0 ? (
        <StateBlock
          kind="empty"
          text="没有 Provider 账号池"
          detail="运行时没有报告任何账号 —— 这与「投影未接线」不同:接线正常,只是池是空的。"
        />
      ) : (
        <>
          <p className="rt-help">
            快照 <span className="mono">{pools.data.snapshot_id}</span> · 观测于{" "}
            <span className="mono">{formatObservedAt(pools.data.observed_at_ms)}</span>
            {pools.data.next_cursor === null ? "" : " · 还有更多(本卡只读第一页)"}
          </p>
          <table>
            <thead>
              <tr>
                <th scope="col">Provider / Channel / 账号</th>
                <th scope="col">种类</th>
                <th scope="col">认证</th>
                <th scope="col">运行时</th>
                <th scope="col">并发</th>
                <th scope="col">过期</th>
                <th scope="col">操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((account) => (
                <tr key={`${account.provider_id}/${account.channel_id}/${account.account_id}`}>
                  <th scope="row" className="mono rt-rowhead">
                    {account.provider_id} / {account.channel_id} / {account.account_id}
                  </th>
                  <td className="mono">{account.account_kind}</td>
                  <td>
                    <StateChip
                      meta={authStatusMeta(account.auth_status)}
                      attr={account.auth_status}
                      raw={account.auth_status}
                    />
                  </td>
                  <td>
                    <StateChip
                      meta={runtimeStatusMeta(account.runtime_status)}
                      attr={account.runtime_status}
                      raw={account.runtime_status}
                    />
                    {account.enabled ? null : <span className="rt-off">已禁用</span>}
                  </td>
                  <td className="mono">
                    {account.active_leases} / {account.max_concurrency}
                  </td>
                  <td className="mono">{formatDue(account.expires_at_ms, nowMs)}</td>
                  <td className="row-actions">
                    <button
                      type="button"
                      className="secondary"
                      disabled={scope === undefined}
                      title={scope === undefined ? "操作需要选择一个配置版本" : undefined}
                      onClick={() => setTarget({ account, action: "cool_down" })}
                    >
                      冷却
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={scope === undefined}
                      title={scope === undefined ? "操作需要选择一个配置版本" : undefined}
                      onClick={() => setTarget({ account, action: "request_recovery" })}
                    >
                      请求恢复
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {target === undefined ? null : (
        <PoolActionSheet
          account={target.account}
          action={target.action}
          pending={act.isPending}
          onCancel={() => setTarget(undefined)}
          onInvalid={setError}
          onSubmit={(body) => act.mutate(body)}
        />
      )}
    </div>
  );
}

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
    // The pool card is NOT version-scoped, so it renders here rather than
    // hiding behind a blanket "pick a version" state that would be false for it.
    return (
      <section className="runtime-page">
        <h2>{t.nav.runtime}</h2>
        <ProviderPoolCard nowMs={nowMs} />
        <div className="card empty-state" data-kind="empty" data-gap="top">
          <p>
            其余三个投影需要一个配置版本。
            <br />
            <small className="muted-3">
              可用性矩阵、目录新鲜度与 Route Explain 都带 X-Config-Version;
              上面的账号池不带,所以它现在就能读。
            </small>
          </p>
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

      <ProviderPoolCard nowMs={nowMs} />
      <ExplainCard scope={scope} />
    </section>
  );
}
