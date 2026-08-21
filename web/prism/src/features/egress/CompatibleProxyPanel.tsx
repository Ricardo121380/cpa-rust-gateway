// Compatible proxy pools / nodes / egress bindings (P13-11 A–D) — 15 contract
// operations, mounted under 出口 because this is the other half of "how a
// request leaves": the policy above says where it may go, this says down which
// wire.
//
// Three things this panel refuses to do, each for a stated reason:
//
//   - It never renders or reconstructs a proxy address. The read model carries
//     `proxy_configured: boolean` and nothing else; assembling a SOCKS5 URL in
//     the browser would recreate the secret the backend just sealed.
//   - It never opens a connection. `validateProxyEndpoint` checks a typed
//     string so a bad address gets a sentence instead of an opaque 400 — the
//     gateway remains the only thing that ever dials the proxy.
//   - Its create buttons live at SECTION level, never on a row. A pool with no
//     nodes has no rows, and a freshly created pool is exactly that pool; a
//     row-level "add node" would make the thing you just created unreachable.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent, type ReactNode } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { useVersionStore } from "../config-versions/versionStore";
import {
  ATTEMPTS_MAX,
  ATTEMPTS_MIN,
  CONCURRENCY_MAX,
  CONCURRENCY_MIN,
  FAILURE_SCOPES,
  STICKINESS_MODES,
  TARGET_KINDS,
  WEIGHT_MAX,
  WEIGHT_MIN,
  bindingKey,
  failureScopeLabel,
  groupNodesByPool,
  nodeReferences,
  poolReferences,
  stickinessLabel,
  targetIdSource,
  targetKindLabel,
  validateProxyEndpoint,
  type EgressBinding,
  type ProxyNode,
  type ProxyPool,
} from "./compatible";

type UpstreamOption = Readonly<{ id: string }>;

const OPS = {
  pool: {
    create: "createCompatibleProxyPool",
    update: "updateCompatibleProxyPool",
    remove: "deleteCompatibleProxyPool",
  },
  node: {
    create: "createCompatibleProxyNode",
    update: "updateCompatibleProxyNode",
    remove: "deleteCompatibleProxyNode",
  },
  binding: {
    create: "createCompatibleEgressBinding",
    update: "updateCompatibleEgressBinding",
    remove: "deleteCompatibleEgressBinding",
  },
} as const;

type Entity = keyof typeof OPS;

type Draft =
  | Readonly<{ kind: "pool"; existing: ProxyPool | undefined }>
  | Readonly<{ kind: "node"; existing: ProxyNode | undefined }>
  | Readonly<{ kind: "binding"; existing: EgressBinding | undefined }>;

type Doomed = Readonly<{ entity: Entity; label: string; blockers: readonly string[] }> &
  Readonly<{ path: Record<string, string> }>;

type SaveInput = Readonly<{
  entity: Entity;
  body: Record<string, unknown>;
  path: Record<string, string> | undefined;
}>;

function Section({
  title,
  operation,
  help,
  onCreate,
  editable,
  children,
}: Readonly<{
  title: string;
  operation: string;
  help: ReactNode;
  onCreate: () => void;
  editable: boolean;
  children: ReactNode;
}>) {
  return (
    <section className="cp-section">
      <div className="cp-head">
        <div className="cp-head-text">
          <h3>
            {title}
            <span className="cp-op mono">{operation}</span>
          </h3>
          <p className="cp-help">{help}</p>
        </div>
        <button type="button" onClick={onCreate} disabled={!editable}>
          新建
        </button>
      </div>
      {children}
    </section>
  );
}

function PoolSheet({
  existing,
  upstreams,
  pending,
  onCancel,
  onSubmit,
}: Readonly<{
  existing: ProxyPool | undefined;
  upstreams: readonly UpstreamOption[];
  pending: boolean;
  onCancel: () => void;
  onSubmit: (body: Record<string, unknown>, path: Record<string, string> | undefined) => void;
}>) {
  return (
    <Sheet title={existing === undefined ? "新建代理池" : `编辑代理池 · ${existing.id}`} onEscape={onCancel}>
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const id = String(data.get("id") ?? "").trim();
          onSubmit(
            {
              id,
              upstream_id: String(data.get("upstream_id") ?? "").trim(),
              name: String(data.get("name") ?? "").trim(),
              enabled: data.get("enabled") === "on",
            },
            existing === undefined ? undefined : { pool_id: existing.id },
          );
        }}
      >
        <label>
          id
          <input
            name="id"
            className="mono"
            required
            maxLength={128}
            readOnly={existing !== undefined}
            defaultValue={existing?.id ?? ""}
          />
        </label>
        <label>
          upstream
          <select name="upstream_id" defaultValue={existing?.upstream_id ?? ""} required>
            <option value="" disabled>
              选择一个 upstream
            </option>
            {upstreams.map((upstream) => (
              <option key={upstream.id} value={upstream.id}>
                {upstream.id}
              </option>
            ))}
          </select>
        </label>
        <label>
          名称
          <input name="name" required maxLength={256} defaultValue={existing?.name ?? ""} />
        </label>
        <label className="checkline">
          <input type="checkbox" name="enabled" defaultChecked={existing?.enabled ?? true} />
          启用
        </label>
        <p className="stat-sub">
          池本身不带任何代理地址 —— 地址只存在于池下的节点上,并且落库即被封存。
        </p>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            {existing === undefined ? "创建" : "保存"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}

function NodeSheet({
  existing,
  upstreams,
  pools,
  pending,
  onCancel,
  onSubmit,
}: Readonly<{
  existing: ProxyNode | undefined;
  upstreams: readonly UpstreamOption[];
  pools: readonly ProxyPool[];
  pending: boolean;
  onCancel: () => void;
  onSubmit: (body: Record<string, unknown>, path: Record<string, string> | undefined) => void;
}>) {
  const [endpointError, setEndpointError] = useState<string | undefined>();
  const creating = existing === undefined;

  return (
    <Sheet title={creating ? "新建代理节点" : `编辑代理节点 · ${existing.id}`} onEscape={onCancel}>
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const endpoint = String(data.get("proxy_endpoint") ?? "").trim();
          // On create the endpoint is required. On edit, blank means "keep the
          // sealed one" — so it is only validated when the operator typed
          // something, i.e. when they mean to rotate it.
          if (creating || endpoint !== "") {
            const reason = validateProxyEndpoint(endpoint);
            if (reason !== undefined) {
              setEndpointError(reason);
              return;
            }
          }
          setEndpointError(undefined);
          const poolId = String(data.get("pool_id") ?? "");
          onSubmit(
            {
              id: String(data.get("id") ?? "").trim(),
              upstream_id: String(data.get("upstream_id") ?? "").trim(),
              pool_id: poolId === "" ? null : poolId,
              name: String(data.get("name") ?? "").trim(),
              ...(endpoint === "" ? {} : { proxy_endpoint: endpoint }),
              enabled: data.get("enabled") === "on",
              weight: Number(data.get("weight") ?? WEIGHT_MIN),
              maximum_concurrency: Number(data.get("maximum_concurrency") ?? CONCURRENCY_MIN),
            },
            creating ? undefined : { node_id: existing.id },
          );
        }}
      >
        <label>
          id
          <input
            name="id"
            className="mono"
            required
            maxLength={128}
            readOnly={!creating}
            defaultValue={existing?.id ?? ""}
          />
        </label>
        <label>
          upstream
          <select name="upstream_id" defaultValue={existing?.upstream_id ?? ""} required>
            <option value="" disabled>
              选择一个 upstream
            </option>
            {upstreams.map((upstream) => (
              <option key={upstream.id} value={upstream.id}>
                {upstream.id}
              </option>
            ))}
          </select>
        </label>
        <label>
          所属池(可空)
          <select name="pool_id" defaultValue={existing?.pool_id ?? ""}>
            <option value="">不属于任何池</option>
            {pools.map((pool) => (
              <option key={pool.id} value={pool.id}>
                {pool.id}
              </option>
            ))}
          </select>
        </label>
        <label>
          名称
          <input name="name" required maxLength={256} defaultValue={existing?.name ?? ""} />
        </label>
        <label>
          proxy_endpoint
          <input
            name="proxy_endpoint"
            className="mono"
            type="text"
            required={creating}
            autoComplete="off"
            spellCheck={false}
            placeholder="socks5://127.0.0.1:1080"
            onChange={() => setEndpointError(undefined)}
          />
        </label>
        {endpointError === undefined ? null : (
          <p role="alert" className="field-error">
            {endpointError}
          </p>
        )}
        <p className="stat-sub">
          只接受 <span className="mono">socks5://主机:端口</span> —— 不带用户名 / 密码 / 路径 /
          查询串。落库即以 AEAD 封存,<strong>读模型永远不会回显它</strong>,列表里只能看到
          「已配置」。
          {creating ? null : (
            <>
              {" "}
              <strong>留空表示保留现有地址</strong>;填入新值才会轮换。这一点与 Account
              的密钥相反 —— 那里 PATCH 必须重填,这里的契约明说"省略或 null 保留原值"。
            </>
          )}
        </p>
        <div className="cp-pair">
          <label>
            weight
            <input
              name="weight"
              type="number"
              className="mono"
              required
              min={WEIGHT_MIN}
              max={WEIGHT_MAX}
              defaultValue={existing?.weight ?? 1}
            />
          </label>
          <label>
            maximum_concurrency
            <input
              name="maximum_concurrency"
              type="number"
              className="mono"
              required
              min={CONCURRENCY_MIN}
              max={CONCURRENCY_MAX}
              defaultValue={existing?.maximum_concurrency ?? 1}
            />
          </label>
        </div>
        <label className="checkline">
          <input type="checkbox" name="enabled" defaultChecked={existing?.enabled ?? true} />
          启用
        </label>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            {creating ? "创建" : "保存"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}

function BindingSheet({
  existing,
  pools,
  nodes,
  pending,
  onCancel,
  onSubmit,
}: Readonly<{
  existing: EgressBinding | undefined;
  pools: readonly ProxyPool[];
  nodes: readonly ProxyNode[];
  pending: boolean;
  onCancel: () => void;
  onSubmit: (body: Record<string, unknown>, path: Record<string, string> | undefined) => void;
}>) {
  const creating = existing === undefined;
  const [kind, setKind] = useState<string>(existing?.target_kind ?? "direct");
  const source = targetIdSource(kind);
  const options = source === "node" ? nodes : source === "pool" ? pools : [];

  return (
    <Sheet
      title={
        creating ? "新建兼容出口绑定" : `编辑绑定 · ${existing.endpoint_id}/${existing.credential_id}`
      }
      onEscape={onCancel}
    >
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const targetId = String(data.get("target_id") ?? "");
          onSubmit(
            {
              endpoint_id: String(data.get("endpoint_id") ?? "").trim(),
              credential_id: String(data.get("credential_id") ?? "").trim(),
              target_kind: kind,
              // direct MUST send no id and the other two MUST send one — the
              // backend matches on the pair and 400s anything else.
              target_id: source === "none" || targetId === "" ? null : targetId,
              failure_scope: String(data.get("failure_scope") ?? "endpoint"),
              stickiness: String(data.get("stickiness") ?? "none"),
              pre_submit_max_attempts: Number(data.get("pre_submit_max_attempts") ?? 1),
            },
            creating
              ? undefined
              : { endpoint_id: existing.endpoint_id, credential_id: existing.credential_id },
          );
        }}
      >
        <div className="cp-pair">
          <label>
            endpoint_id
            <input
              name="endpoint_id"
              className="mono"
              required
              maxLength={128}
              readOnly={!creating}
              defaultValue={existing?.endpoint_id ?? ""}
            />
          </label>
          <label>
            credential_id
            <input
              name="credential_id"
              className="mono"
              required
              maxLength={128}
              readOnly={!creating}
              defaultValue={existing?.credential_id ?? ""}
            />
          </label>
        </div>
        <p className="stat-sub">
          绑定的主键是 <strong>(endpoint, credential) 这一对</strong>,不是单个 id,所以编辑时两者都
          不可改 —— 改成另一对等于另一条绑定。契约里没有枚举端点与凭据的操作,因此这里是手输。
        </p>
        <label>
          target_kind
          <select name="target_kind" value={kind} onChange={(event) => setKind(event.target.value)}>
            {TARGET_KINDS.map((value) => (
              <option key={value} value={value}>
                {targetKindLabel(value)}({value})
              </option>
            ))}
          </select>
        </label>
        {source === "none" ? (
          <p className="stat-sub">直连不带目标 id;后端对 direct + 任何 id 一律拒绝。</p>
        ) : (
          <label>
            target_id · {source === "node" ? "从代理节点里选" : "从代理池里选"}
            <select name="target_id" defaultValue={existing?.target_id ?? ""} required>
              <option value="" disabled>
                {options.length === 0
                  ? source === "node"
                    ? "还没有任何代理节点"
                    : "还没有任何代理池"
                  : "选择一个目标"}
              </option>
              {options.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.id}
                </option>
              ))}
            </select>
          </label>
        )}
        <label>
          failure_scope
          <select name="failure_scope" defaultValue={existing?.failure_scope ?? "endpoint"}>
            {FAILURE_SCOPES.map((value) => (
              <option key={value} value={value}>
                {failureScopeLabel(value)}({value})
              </option>
            ))}
          </select>
        </label>
        <label>
          stickiness
          <select name="stickiness" defaultValue={existing?.stickiness ?? "none"}>
            {STICKINESS_MODES.map((value) => (
              <option key={value} value={value}>
                {stickinessLabel(value)}({value})
              </option>
            ))}
          </select>
        </label>
        <label>
          pre_submit_max_attempts
          <input
            name="pre_submit_max_attempts"
            type="number"
            className="mono"
            required
            min={ATTEMPTS_MIN}
            max={ATTEMPTS_MAX}
            defaultValue={existing?.pre_submit_max_attempts ?? 1}
          />
        </label>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            {creating ? "创建" : "保存"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}

export function CompatibleProxyPanel({
  upstreams,
}: Readonly<{ upstreams: readonly UpstreamOption[] }>) {
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;
  const editable = context?.status === "draft";
  const [draft, setDraft] = useState<Draft | undefined>();
  const [doomed, setDoomed] = useState<Doomed | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const pools = useQuery({
    queryKey: ["compatible-pools", scope],
    queryFn: () => call<ProxyPool[]>("listCompatibleProxyPools", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });
  const nodes = useQuery({
    queryKey: ["compatible-nodes", scope],
    queryFn: () => call<ProxyNode[]>("listCompatibleProxyNodes", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });
  const bindings = useQuery({
    queryKey: ["compatible-bindings", scope],
    queryFn: () =>
      call<EgressBinding[]>("listCompatibleEgressBindings", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const invalidate = () => {
    for (const key of ["compatible-pools", "compatible-nodes", "compatible-bindings"]) {
      void queryClient.invalidateQueries({ queryKey: [key, scope] });
    }
  };

  const save = useMutation({
    mutationFn: ({ entity, body, path }: SaveInput) =>
      path === undefined
        ? call<unknown>(OPS[entity].create, { body }, { versionScoped: true, mutating: true })
        : call<unknown>(
            OPS[entity].update,
            { path, body },
            { versionScoped: true, mutating: true },
          ),
    onSuccess: () => {
      setDraft(undefined);
      setActionError(undefined);
      invalidate();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const remove = useMutation({
    mutationFn: ({ entity, path }: Readonly<{ entity: Entity; path: Record<string, string> }>) =>
      call<undefined>(OPS[entity].remove, { path }, { versionScoped: true, mutating: true }),
    onSuccess: () => {
      setDoomed(undefined);
      setActionError(undefined);
      invalidate();
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const poolRows = pools.data ?? [];
  const nodeRows = nodes.data ?? [];
  const bindingRows = bindings.data ?? [];
  const loading = pools.isLoading || nodes.isLoading || bindings.isLoading;
  const failure = pools.error ?? nodes.error ?? bindings.error;

  if (scope === undefined) {
    return null;
  }

  return (
    <div className="card compatible-proxy" data-gap="top">
      <h2>兼容出口 · 代理池 / 节点 / 绑定</h2>
      <p className="cp-help">
        出口策略决定<strong>请求可以去哪里</strong>,这里决定<strong>请求从哪条线出去</strong>。
        三层是 池 → 节点 → 绑定,每层都是本配置版本自己的资源,改动全部走 If-Match。
        {editable ? null : <strong> 当前版本不是草稿,以下只读。</strong>}
      </p>

      {actionError === undefined ? null : (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      )}
      {failure !== null && failure !== undefined ? (
        <p role="alert" className="action-error">
          读取失败:{asAppError(failure).code} · {asAppError(failure).message}
        </p>
      ) : null}
      {loading ? <p className="cp-empty">读取中…</p> : null}

      <Section
        title="代理池"
        operation="listCompatibleProxyPools"
        help="一个池是一组可互换的出口节点。池本身不持有任何地址。"
        editable={editable === true}
        onCreate={() => setDraft({ kind: "pool", existing: undefined })}
      >
        {poolRows.length === 0 ? (
          <p className="cp-empty">还没有代理池。</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th scope="col">id</th>
                <th scope="col">upstream</th>
                <th scope="col">名称</th>
                <th scope="col">启用</th>
                <th scope="col">节点数</th>
                <th scope="col">操作</th>
              </tr>
            </thead>
            <tbody>
              {poolRows.map((pool) => (
                <tr key={pool.id}>
                  <th scope="row" className="mono cp-rowhead">
                    {pool.id}
                  </th>
                  <td className="mono">{pool.upstream_id}</td>
                  <td>{pool.name}</td>
                  <td>{pool.enabled ? "是" : "否"}</td>
                  <td className="mono">
                    {nodeRows.filter((node) => node.pool_id === pool.id).length}
                  </td>
                  <td className="row-actions">
                    <button
                      type="button"
                      className="secondary"
                      disabled={editable !== true}
                      onClick={() => setDraft({ kind: "pool", existing: pool })}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={editable !== true}
                      onClick={() =>
                        setDoomed({
                          entity: "pool",
                          label: `代理池 ${pool.id}`,
                          path: { pool_id: pool.id },
                          blockers: poolReferences(pool.id, nodeRows, bindingRows),
                        })
                      }
                    >
                      删除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>

      <Section
        title="代理节点"
        operation="listCompatibleProxyNodes"
        help={
          <>
            节点持有出口地址,但<strong>读模型只回一个「已配置」布尔值</strong> ——
            地址落库即封存,任何界面都拿不回它。
          </>
        }
        editable={editable === true}
        onCreate={() => setDraft({ kind: "node", existing: undefined })}
      >
        {nodeRows.length === 0 && poolRows.length === 0 ? (
          <p className="cp-empty">还没有代理节点。</p>
        ) : (
          groupNodesByPool(poolRows, nodeRows).map((group) => (
            <div key={group.pool?.id ?? "__loose__"} className="cp-group">
              <h4 className="mono">{group.pool === undefined ? "不属于任何池" : group.pool.id}</h4>
              {group.nodes.length === 0 ? (
                // The state a pool is in the moment it is created. Saying so
                // beats an empty area that reads as a rendering bug.
                <p className="cp-empty">这个池还没有节点 —— 用上面的「新建」加一个。</p>
              ) : (
                <table>
                  <thead>
                    <tr>
                      <th scope="col">id</th>
                      <th scope="col">upstream</th>
                      <th scope="col">名称</th>
                      <th scope="col">代理地址</th>
                      <th scope="col">权重</th>
                      <th scope="col">并发上限</th>
                      <th scope="col">启用</th>
                      <th scope="col">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {group.nodes.map((node) => (
                      <tr key={node.id}>
                        <th scope="row" className="mono cp-rowhead">
                          {node.id}
                        </th>
                        <td className="mono">{node.upstream_id}</td>
                        <td>{node.name}</td>
                        <td>
                          <span className="cp-sealed" data-configured={node.proxy_configured}>
                            {node.proxy_configured ? "已配置(封存)" : "未配置"}
                          </span>
                        </td>
                        <td className="mono">{node.weight}</td>
                        <td className="mono">{node.maximum_concurrency}</td>
                        <td>{node.enabled ? "是" : "否"}</td>
                        <td className="row-actions">
                          <button
                            type="button"
                            className="secondary"
                            disabled={editable !== true}
                            onClick={() => setDraft({ kind: "node", existing: node })}
                          >
                            编辑
                          </button>
                          <button
                            type="button"
                            className="secondary"
                            disabled={editable !== true}
                            onClick={() =>
                              setDoomed({
                                entity: "node",
                                label: `代理节点 ${node.id}`,
                                path: { node_id: node.id },
                                blockers: nodeReferences(node.id, bindingRows),
                              })
                            }
                          >
                            删除
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          ))
        )}
      </Section>

      <Section
        title="兼容出口绑定"
        operation="listCompatibleEgressBindings"
        help="把一对 (endpoint, credential) 绑到直连、某个固定节点,或某个池。"
        editable={editable === true}
        onCreate={() => setDraft({ kind: "binding", existing: undefined })}
      >
        {bindingRows.length === 0 ? (
          <p className="cp-empty">还没有兼容出口绑定。</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th scope="col">endpoint / credential</th>
                <th scope="col">目标</th>
                <th scope="col">失败归因</th>
                <th scope="col">粘滞</th>
                <th scope="col">提交前重试上限</th>
                <th scope="col">操作</th>
              </tr>
            </thead>
            <tbody>
              {bindingRows.map((binding) => (
                <tr key={bindingKey(binding.endpoint_id, binding.credential_id)}>
                  <th scope="row" className="mono cp-rowhead">
                    {binding.endpoint_id} / {binding.credential_id}
                  </th>
                  <td>
                    {targetKindLabel(binding.target_kind)}
                    {binding.target_id === null ? null : (
                      <span className="mono cp-target"> {binding.target_id}</span>
                    )}
                  </td>
                  <td>{failureScopeLabel(binding.failure_scope)}</td>
                  <td>{stickinessLabel(binding.stickiness)}</td>
                  <td className="mono">{binding.pre_submit_max_attempts}</td>
                  <td className="row-actions">
                    <button
                      type="button"
                      className="secondary"
                      disabled={editable !== true}
                      onClick={() => setDraft({ kind: "binding", existing: binding })}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={editable !== true}
                      onClick={() =>
                        setDoomed({
                          entity: "binding",
                          label: `绑定 ${binding.endpoint_id}/${binding.credential_id}`,
                          path: {
                            endpoint_id: binding.endpoint_id,
                            credential_id: binding.credential_id,
                          },
                          blockers: [],
                        })
                      }
                    >
                      删除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>

      {draft?.kind === "pool" ? (
        <PoolSheet
          existing={draft.existing}
          upstreams={upstreams}
          pending={save.isPending}
          onCancel={() => setDraft(undefined)}
          onSubmit={(body, path) => save.mutate({ entity: "pool", body, path })}
        />
      ) : null}
      {draft?.kind === "node" ? (
        <NodeSheet
          existing={draft.existing}
          upstreams={upstreams}
          pools={poolRows}
          pending={save.isPending}
          onCancel={() => setDraft(undefined)}
          onSubmit={(body, path) => save.mutate({ entity: "node", body, path })}
        />
      ) : null}
      {draft?.kind === "binding" ? (
        <BindingSheet
          existing={draft.existing}
          pools={poolRows}
          nodes={nodeRows}
          pending={save.isPending}
          onCancel={() => setDraft(undefined)}
          onSubmit={(body, path) => save.mutate({ entity: "binding", body, path })}
        />
      ) : null}

      {doomed === undefined ? null : (
        <Sheet title={`删除 ${doomed.label}`} onEscape={() => setDoomed(undefined)}>
          <p>删除后不可撤销(可以回滚整个配置版本)。</p>
          {doomed.blockers.length > 0 ? (
            // The backend refuses to delete a referenced pool or node and there
            // is no cascade. Both lists are already here, so the refusal is
            // predicted rather than delivered as a failed request.
            <p role="alert" className="reveal-warning">
              仍被引用,后端会拒绝这次删除:{doomed.blockers.join("、")}。先解除这些引用。
            </p>
          ) : null}
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setDoomed(undefined)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={remove.isPending}
              onClick={() => remove.mutate({ entity: doomed.entity, path: doomed.path })}
            >
              确认删除
            </button>
          </div>
        </Sheet>
      )}
    </div>
  );
}
