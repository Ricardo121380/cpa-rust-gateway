// Route workbench — the missing half of the config chain.
//
// Until now the panel could CREATE a route (ModelsPage) and nothing else, and
// a route with no candidate is rejected by the backend:
//
//   crates/gateway-control/src/management_mutation_service.rs:2074
//     if active_candidates.is_empty() {
//         error_codes.push("route_missing_active_candidate");
//     }
//
// so every route made here put the draft into a state the panel itself could
// not repair. This card closes that loop.
//
// Three contract facts are stated on screen rather than designed around:
//
//  1. ROUTES ARE NOT ENUMERABLE. There is no listRoutes; getRoute needs an id
//     you already have. The datalist below is best-effort (route_ids carried by
//     the operational inventory, the same source AccessPage uses) and is known
//     to be incomplete — a route whose candidates are not yet bound, which is
//     exactly the route you come here to fix, does not appear in it. The field
//     stays free text for that reason.
//
//  2. CANDIDATES ARE INSERT-ONLY. createRouteCandidate exists; there is no
//     list, update or delete. Their only read path is explainRoute, which needs
//     a requested_model and a protocol to answer. So this card creates them and
//     hands off to Explain for reading — it never implies one can be edited.
//
//  3. validateRoute CHECKS DRAFT TOPOLOGY ONLY. The backend's own words:
//     "Full compiler/capability admission remains the later publication
//     boundary." A green check here is not a promise that publish succeeds.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { useVersionStore } from "../config-versions/versionStore";
import {
  CREDENTIAL_SCOPE,
  parseCapabilityOverride,
  ROUTE_POLICY,
  routeErrorLabel,
  TRANSFORM_MODES,
  transformModeHint,
  validCandidateParams,
  validRouteParams,
  type RouteRecord,
  type RouteValidation,
  type TransformMode,
} from "./model";

type PoolRow = Readonly<{ route_ids: readonly string[] }>;

type CandidateInput = Readonly<{
  id: string;
  endpoint_id: string;
  upstream_model: string;
  credential_scope: typeof CREDENTIAL_SCOPE;
  transform_mode: TransformMode;
  enabled: boolean;
  priority: number;
  weight: number;
  capability_override: Readonly<Record<string, boolean>>;
}>;

export function RouteWorkbench({
  focusRouteId,
  editable,
}: Readonly<{ focusRouteId: string | undefined; editable: boolean }>) {
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;

  const [field, setField] = useState("");
  const [loaded, setLoaded] = useState<string | undefined>();
  const [addingCandidate, setAddingCandidate] = useState(false);
  const [editingRoute, setEditingRoute] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [notice, setNotice] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();

  // The route ModelsPage just created is the one you almost certainly want, and
  // it is precisely the one no enumeration can suggest yet.
  const pending = focusRouteId !== undefined && focusRouteId !== loaded ? focusRouteId : undefined;

  const route = useQuery({
    queryKey: ["route", scope, loaded],
    queryFn: () =>
      call<RouteRecord>("getRoute", { path: { route_id: loaded ?? "" } }, { versionScoped: true }),
    enabled: scope !== undefined && loaded !== undefined,
    retry: false,
  });

  const suggestions = useQuery({
    queryKey: ["pool-routes", scope],
    queryFn: () =>
      call<Readonly<{ items: readonly PoolRow[] }>>(
        "listOperationalAccountPools",
        { query: { limit: 100 } },
        { versionScoped: true },
      ),
    enabled: scope !== undefined,
    retry: false,
  });
  const routeIds = [...new Set((suggestions.data?.items ?? []).flatMap((row) => row.route_ids))];

  const validation = useMutation({
    mutationFn: (routeId: string) =>
      call<RouteValidation>(
        "validateRoute",
        { path: { route_id: routeId } },
        // No If-Match: validateRoute is declared without one, and it does not
        // advance the revision.
        { versionScoped: true },
      ),
    onError: (cause) => setError(asAppError(cause).message),
  });

  const addCandidate = useMutation({
    mutationFn: (input: Readonly<{ routeId: string; body: CandidateInput }>) =>
      call<Readonly<{ id: string }>>(
        "createRouteCandidate",
        { path: { route_id: input.routeId }, body: input.body },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: (created, variables) => {
      setAddingCandidate(false);
      setNotice(`候选 ${created.id} 已加入。重新校验以确认路由现在通得过。`);
      validation.mutate(variables.routeId);
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const saveRoute = useMutation({
    mutationFn: (input: Readonly<{ routeId: string; body: Omit<RouteRecord, "public_model_id"> }>) =>
      call<RouteRecord>(
        "updateRoute",
        { path: { route_id: input.routeId }, body: input.body },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setEditingRoute(false);
      void queryClient.invalidateQueries({ queryKey: ["route", scope, loaded] });
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const removeRoute = useMutation({
    mutationFn: (routeId: string) =>
      call<undefined>(
        "deleteRoute",
        { path: { route_id: routeId } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setConfirmDelete(false);
      setLoaded(undefined);
      setNotice("路由已删除,其候选一并移除。");
      validation.reset();
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  function onLoad(event: FormEvent) {
    event.preventDefault();
    const next = field.trim();
    if (next === "") {
      return;
    }
    validation.reset();
    setError(undefined);
    setLoaded(next);
  }

  const record = route.data;

  return (
    <div className="card route-workbench" data-gap="top">
      <header className="page-head">
        <h3>路由工作台</h3>
        <code className="idchip mono">getRoute · validateRoute · createRouteCandidate</code>
      </header>

      <p className="stat-sub">
        契约里<strong>没有 listRoutes</strong>,路由只能按 id 打开。下面的建议来自运营库存里
        已绑定的 route_id,<strong>刚建好、还没有候选的路由不会出现在建议里</strong> ——
        那正是要来这里修的那一类,所以输入框保持自由文本。
      </p>

      {pending !== undefined ? (
        <p className="action-notice">
          刚创建了路由 <span className="mono">{pending}</span>。
          <button
            type="button"
            onClick={() => {
              setField(pending);
              validation.reset();
              setLoaded(pending);
            }}
          >
            打开它
          </button>
        </p>
      ) : null}

      {notice !== undefined ? (
        <p className="action-notice">
          {notice}
          <button type="button" onClick={() => setNotice(undefined)}>
            知道了
          </button>
        </p>
      ) : null}
      {error !== undefined ? (
        <p role="alert" className="action-error">
          {error}
          <button type="button" onClick={() => setError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <form className="rw-load" onSubmit={onLoad}>
        <label>
          route_id
          <input
            className="mono"
            required
            maxLength={128}
            list="rw-route-ids"
            placeholder="route-minimax-m3"
            value={field}
            onChange={(event) => setField(event.target.value)}
          />
        </label>
        <datalist id="rw-route-ids">
          {routeIds.map((id) => (
            <option key={id} value={id} />
          ))}
        </datalist>
        <button type="submit" disabled={route.isFetching}>
          {route.isFetching ? "载入中…" : "载入"}
        </button>
      </form>

      {loaded !== undefined && route.isError ? (
        <div className="empty-state" data-kind="error">
          <p>{asAppError(route.error).message}</p>
        </div>
      ) : null}

      {record !== undefined ? (
        <>
          <table>
            <tbody>
              <tr>
                <th scope="row">所属公开模型</th>
                <td className="mono">{record.public_model_id}</td>
              </tr>
              <tr>
                <th scope="row">调度策略</th>
                <td className="mono">{record.policy}</td>
              </tr>
              <tr>
                <th scope="row">max_attempts</th>
                <td className="mono">{record.max_attempts}</td>
              </tr>
              <tr>
                <th scope="row">bootstrap_timeout_ms</th>
                <td className="mono">{record.bootstrap_timeout_ms}</td>
              </tr>
            </tbody>
          </table>

          <div className="rw-actions">
            <button
              type="button"
              disabled={!editable}
              title={editable ? undefined : "仅草稿版本可编辑"}
              onClick={() => setAddingCandidate(true)}
            >
              加候选
            </button>
            <button
              type="button"
              className="secondary"
              disabled={validation.isPending}
              onClick={() => validation.mutate(record.id)}
            >
              {validation.isPending ? "校验中…" : "校验"}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={!editable}
              title={editable ? undefined : "仅草稿版本可编辑"}
              onClick={() => setEditingRoute(true)}
            >
              编辑路由
            </button>
            <Link className="rw-link" to={`/runtime?route_id=${encodeURIComponent(record.id)}`}>
              去 Explain 看候选
            </Link>
            <button
              type="button"
              className="danger"
              disabled={!editable}
              title={editable ? undefined : "仅草稿版本可编辑"}
              onClick={() => setConfirmDelete(true)}
            >
              删除路由
            </button>
          </div>

          <p className="stat-sub">
            候选在契约里<strong>只能新增</strong>(没有列出、修改或删除算子)。要看一条路由现在
            有哪些候选、各自是否被选中,唯一的读取路径是 Route Explain —— 它需要一个请求模型和协议
            才能作答。
          </p>

          {validation.data !== undefined ? (
            <div className="rw-validation" data-valid={validation.data.valid ? "true" : "false"}>
              <p>
                {validation.data.valid
                  ? "草稿拓扑校验通过"
                  : `草稿拓扑校验未通过 · ${validation.data.error_codes?.length ?? 0} 项`}
              </p>
              {validation.data.valid ? null : (
                <ul className="rw-codes">
                  {(validation.data.error_codes ?? []).map((code) => {
                    const label = routeErrorLabel(code);
                    return (
                      <li key={code}>
                        <span className="mono">{code}</span>
                        {label === undefined ? null : <span className="rw-code-help">{label}</span>}
                      </li>
                    );
                  })}
                </ul>
              )}
              <p className="stat-sub">
                此处只校验<strong>草稿拓扑</strong>(候选是否存在、端点是否在且启用、是否有 active
                凭据绑定)。能力准入与完整编译在<strong>发布</strong>时才发生 ——
                这里通过不等于发布会通过。
              </p>
            </div>
          ) : null}
        </>
      ) : null}

      {addingCandidate && record !== undefined ? (
        <CandidateSheet
          routeId={record.id}
          pending={addCandidate.isPending}
          onCancel={() => setAddingCandidate(false)}
          onInvalid={setError}
          onSubmit={(body) => addCandidate.mutate({ routeId: record.id, body })}
        />
      ) : null}

      {editingRoute && record !== undefined ? (
        <Sheet title={`编辑路由 ${record.id}`} onEscape={() => setEditingRoute(false)}>
          <p className="stat-sub">
            PATCH 是整体替换,所以下面每个字段都已用 <span className="mono">getRoute</span>{" "}
            的当前值预填 —— 留空会把它写成空,而不是"不改"。
          </p>
          <form
            className="sheet-form"
            onSubmit={(event) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              const maxAttempts = Number(data.get("max_attempts"));
              const bootstrapTimeoutMs = Number(data.get("bootstrap_timeout_ms"));
              if (!validRouteParams(maxAttempts, bootstrapTimeoutMs)) {
                setError("路由参数越界:max_attempts 1-16,bootstrap_timeout_ms 1-120000");
                return;
              }
              saveRoute.mutate({
                routeId: record.id,
                body: {
                  id: record.id,
                  policy: ROUTE_POLICY,
                  max_attempts: maxAttempts,
                  bootstrap_timeout_ms: bootstrapTimeoutMs,
                },
              });
            }}
          >
            <label>
              路由 ID(不可改)
              <input className="mono" value={record.id} disabled />
            </label>
            <label>
              调度策略(契约当前唯一值)
              <input className="mono" value={ROUTE_POLICY} disabled />
            </label>
            <label>
              max_attempts(1-16)
              <input
                name="max_attempts"
                type="number"
                min={1}
                max={16}
                defaultValue={record.max_attempts}
              />
            </label>
            <label>
              bootstrap_timeout_ms(1-120000)
              <input
                name="bootstrap_timeout_ms"
                type="number"
                min={1}
                max={120000}
                defaultValue={record.bootstrap_timeout_ms}
              />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setEditingRoute(false)}>
                取消
              </button>
              <button type="submit" disabled={saveRoute.isPending}>
                保存
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDelete && record !== undefined ? (
        <Sheet title="确认删除路由" onEscape={() => setConfirmDelete(false)}>
          <p className="reveal-warning">
            删除路由 <span className="mono">{record.id}</span> 会一并移除它的全部候选。
            公开模型 <span className="mono">{record.public_model_id}</span>{" "}
            将没有可用路由,客户端解析到它的请求会失败。
          </p>
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setConfirmDelete(false)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={removeRoute.isPending}
              onClick={() => removeRoute.mutate(record.id)}
            >
              确认删除
            </button>
          </div>
        </Sheet>
      ) : null}
    </div>
  );
}

function CandidateSheet({
  routeId,
  pending,
  onCancel,
  onInvalid,
  onSubmit,
}: Readonly<{
  routeId: string;
  pending: boolean;
  onCancel: () => void;
  onInvalid: (message: string) => void;
  onSubmit: (body: CandidateInput) => void;
}>) {
  const [mode, setMode] = useState<TransformMode>("passthrough");

  return (
    <Sheet title={`为 ${routeId} 添加候选`} onEscape={onCancel}>
      <p className="stat-sub">
        候选<strong>只能新增</strong>:契约没有修改或删除算子。写错了只能删掉整条路由重建,
        所以提交前请核对 endpoint_id 与 upstream_model。
      </p>
      <form
        className="sheet-form"
        onSubmit={(event) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const priority = Number(data.get("priority"));
          const weight = Number(data.get("weight"));
          if (!validCandidateParams(priority, weight)) {
            onInvalid("候选参数越界:priority ≥ 0,weight 1-10000");
            return;
          }
          const parsed = parseCapabilityOverride(String(data.get("capability_override") ?? ""));
          if (!parsed.ok) {
            onInvalid(parsed.reason);
            return;
          }
          onSubmit({
            id: String(data.get("id") ?? "").trim(),
            endpoint_id: String(data.get("endpoint_id") ?? "").trim(),
            upstream_model: String(data.get("upstream_model") ?? "").trim(),
            credential_scope: CREDENTIAL_SCOPE,
            transform_mode: mode,
            enabled: data.get("enabled") === "on",
            priority,
            weight,
            capability_override: parsed.override,
          });
        }}
      >
        <label>
          候选 ID
          <input name="id" className="mono" required maxLength={128} />
        </label>
        <label>
          endpoint_id(必须是本版本里已存在且启用的端点)
          <input name="endpoint_id" className="mono" required maxLength={128} />
        </label>
        <label>
          upstream_model(上游侧的真实模型名)
          <input name="upstream_model" className="mono" required maxLength={256} />
        </label>
        <label>
          credential_scope(契约当前唯一值)
          <input className="mono" value={CREDENTIAL_SCOPE} disabled />
        </label>
        <label>
          transform_mode
          <select value={mode} onChange={(event) => setMode(event.target.value as TransformMode)}>
            {TRANSFORM_MODES.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
          <small>{transformModeHint(mode)}</small>
        </label>
        <label className="toggle-row">
          <input name="enabled" type="checkbox" defaultChecked />
          启用(只有启用的候选参与校验与调度)
        </label>
        <label>
          priority(≥ 0,小的先试)
          <input name="priority" type="number" min={0} defaultValue={0} />
        </label>
        <label>
          weight(1-10000,同优先级内的加权轮询份额)
          <input name="weight" type="number" min={1} max={10000} defaultValue={1} />
        </label>
        <label>
          capability_override(可留空 —— 空表示不覆盖任何能力)
          <input
            name="capability_override"
            className="mono"
            maxLength={512}
            placeholder="vision=true tools=false"
          />
          <small>
            形如 <span className="mono">key=true key=false</span>,最多 32 项。
            键不限于本面板列出的语义能力。
          </small>
        </label>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            创建候选
          </button>
        </div>
      </form>
    </Sheet>
  );
}
