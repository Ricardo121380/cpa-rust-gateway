// Per-provider subresource panel, driven by the REAL operational inventory
// (P13-04A `listOperationalAccountPools`) instead of the proposed G1 graph.
//
// Vocabulary is the contract's: this endpoint answers in provider / channel /
// account, so that is what the tables say. The config plane (upstream /
// endpoint / credential, status active|disabled|revoked) is a different
// contract and keeps its own words on its own pages.
//
// Two boundaries the panel has to state rather than paper over:
//   - one row IS one binding, so unbound channels and accounts do not appear;
//   - the projection is URL-free by design (no base_url / inference_path).
// Endpoint test and catalog discovery still drive their own real operations.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useVersionStore } from "../config-versions/versionStore";
import { CredentialSheet } from "./CredentialSheet";
import {
  accountStatusTone,
  POOL_PAGE_LIMIT,
  providerPool,
  type AccountPoolPage,
} from "./pools";

type EndpointTest = Readonly<{
  outcome: "pass" | "rejected" | "transport_failed" | "protocol_failed";
  status_class?: string;
  canonical_lifecycle?: boolean;
}>;
type CatalogDiff = Readonly<{ added: number; removed: number; unchanged: number }>;

/** Full endpoint record. The operational inventory is URL-free by contract, so
 *  editing needs this read: PATCH takes a whole EndpointInput and a form that
 *  cannot pre-fill base_url would blank it on save. */
type EndpointRecord = Readonly<{
  id: string;
  adapter_id: string;
  api_format: string;
  base_url: string;
  inference_path: string;
  models_path?: string | null;
  transport: string;
  enabled: boolean;
}>;

type ChannelForm = { mode: "create" } | { mode: "edit"; channelId: string };
type AccountForm = { mode: "create" } | { mode: "edit"; accountId: string; kind: string };

const TEST_TONE: Record<EndpointTest["outcome"], string> = {
  pass: "active",
  rejected: "quota_blocked",
  transport_failed: "credential_forbidden",
  protocol_failed: "circuit_open",
};

type ConfigBinding = Readonly<{
  endpoint_id: string;
  upstream_id: string;
  credential_id: string;
  enabled: boolean;
  priority: number;
  weight: number;
  concurrency: number;
}>;

type SheetSubmit = (body: unknown, existing: string | undefined) => void;

/** Channel create/edit. PATCH replaces the whole EndpointInput, and the pool
 *  inventory omits base_url by design, so editing reads the full record first
 *  — a form that could not pre-fill the URL would blank it on save. */
/** Config-plane bindings for ONE channel.
 *
 * The table on the panel is driven by the operational inventory, which is
 * binding-driven AND join-driven: a row only appears when its channel, account
 * and provider all resolve. So a binding whose credential was deleted is
 * invisible there while still sitting in the configuration — and it is exactly
 * that binding that makes a version fail to publish with nothing on screen to
 * explain it. listEndpointCredentialBindings is the config's own answer, so the
 * two can be compared instead of assumed equal. */
function BindingReconcileSheet({
  channelId,
  operationalCredentialIds,
  onClose,
}: Readonly<{
  channelId: string;
  operationalCredentialIds: ReadonlySet<string>;
  onClose: () => void;
}>) {
  const bindings = useQuery({
    queryKey: ["endpoint-credential-bindings", channelId],
    queryFn: () =>
      call<ConfigBinding[]>(
        "listEndpointCredentialBindings",
        { path: { endpoint_id: channelId } },
        { versionScoped: true },
      ),
    retry: false,
  });
  const rows = bindings.data ?? [];
  const hidden = rows.filter((row) => !operationalCredentialIds.has(row.credential_id));

  return (
    <Sheet title={`配置侧绑定 · ${channelId}`} onEscape={onClose}>
      <p className="stat-sub">
        上面的绑定表来自<strong>运营库存</strong>,一行需要 channel、account、provider
        三者都能解析才会出现。这里是<strong>配置自己</strong>的回答 ——
        两边不一致时,差的那条就是发布会被卡住、而面板上看不出来的那条。
      </p>
      {bindings.isLoading ? <p className="stat-sub">读取中…</p> : null}
      {bindings.isError ? (
        <p role="alert" className="action-error">
          {asAppError(bindings.error).code} · {asAppError(bindings.error).message}
        </p>
      ) : null}
      {bindings.data !== undefined && rows.length === 0 ? (
        <p className="stat-sub">配置里这个 channel 没有任何绑定。</p>
      ) : null}
      {rows.length > 0 ? (
        <table>
          <thead>
            <tr>
              <th>credential</th>
              <th>upstream</th>
              <th>enabled</th>
              <th>priority</th>
              <th>weight</th>
              <th>concurrency</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.credential_id}>
                <td className="mono">
                  {row.credential_id}
                  {operationalCredentialIds.has(row.credential_id) ? null : (
                    <strong> · 运营库存里没有</strong>
                  )}
                </td>
                <td className="mono">{row.upstream_id}</td>
                <td>{row.enabled ? "是" : "否"}</td>
                <td className="mono">{row.priority}</td>
                <td className="mono">{row.weight}</td>
                <td className="mono">{row.concurrency}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      {hidden.length > 0 ? (
        <p role="alert" className="reveal-warning">
          有 {hidden.length} 条绑定只存在于配置里:{hidden.map((r) => r.credential_id).join("、")}。
          它们指向的凭据无法解析,所以运营库存不显示 —— 但校验与发布仍然会看到它们。
        </p>
      ) : null}
      <div className="sheet-actions">
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </div>
    </Sheet>
  );
}

function ChannelSheet({
  form,
  pending,
  onCancel,
  onSubmit,
}: Readonly<{ form: ChannelForm; pending: boolean; onCancel: () => void; onSubmit: SheetSubmit }>) {
  const editing = form.mode === "edit" ? form.channelId : undefined;
  const record = useQuery({
    queryKey: ["endpoint", editing],
    queryFn: () =>
      call<EndpointRecord>(
        "getEndpoint",
        { path: { endpoint_id: editing as string } },
        { versionScoped: true },
      ),
    enabled: editing !== undefined,
  });

  if (editing !== undefined && record.data === undefined) {
    return (
      <Sheet title={`编辑 Channel · ${editing}`} onEscape={onCancel}>
        <p className="muted">{record.isError ? "读取端点失败" : "读取端点…"}</p>
        <div className="sheet-actions">
          <button type="button" onClick={onCancel}>
            关闭
          </button>
        </div>
      </Sheet>
    );
  }
  const current = record.data;

  return (
    <Sheet
      title={editing === undefined ? "新建 Channel" : `编辑 Channel · ${editing}`}
      onEscape={onCancel}
    >
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const modelsPath = String(data.get("models_path") ?? "").trim();
          onSubmit(
            {
              id: String(data.get("id") ?? "").trim(),
              adapter_id: String(data.get("adapter_id") ?? "").trim(),
              api_format: String(data.get("api_format") ?? "").trim(),
              base_url: String(data.get("base_url") ?? "").trim(),
              inference_path: String(data.get("inference_path") ?? "").trim(),
              models_path: modelsPath === "" ? null : modelsPath,
              transport: "https",
              enabled: data.get("enabled") === "on",
            },
            editing,
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
            readOnly={editing !== undefined}
            defaultValue={current?.id ?? ""}
          />
        </label>
        <label>
          adapter_id
          <input name="adapter_id" className="mono" required defaultValue={current?.adapter_id ?? ""} />
        </label>
        <label>
          api_format
          <input name="api_format" className="mono" required defaultValue={current?.api_format ?? ""} />
        </label>
        <label>
          base_url
          <input name="base_url" className="mono" required defaultValue={current?.base_url ?? ""} />
        </label>
        <label>
          inference_path
          <input
            name="inference_path"
            className="mono"
            required
            defaultValue={current?.inference_path ?? ""}
          />
        </label>
        <label>
          models_path(可空)
          <input name="models_path" className="mono" defaultValue={current?.models_path ?? ""} />
        </label>
        <label className="check-row">
          <input name="enabled" type="checkbox" defaultChecked={current?.enabled ?? true} />
          启用
        </label>
        <p className="stat-sub">
          契约的 transport 目前只有 <span className="mono">https</span> 一个取值,故不作为可选项。
          保存等于整体替换。
        </p>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            {editing === undefined ? "创建" : "保存"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}

/** Account create/edit.
 *
 *  CredentialInput.secret is REQUIRED, and the read model never returns a
 *  secret (only secret_present). So PATCH — a whole-object replace — cannot be
 *  done without re-entering it: there is no way to change only the status.
 *  The form says so instead of quietly posting a blank and earning a 400. */
function AccountSheet({
  form,
  pending,
  onCancel,
  onSubmit,
}: Readonly<{ form: AccountForm; pending: boolean; onCancel: () => void; onSubmit: SheetSubmit }>) {
  const editing = form.mode === "edit" ? form.accountId : undefined;
  return (
    <Sheet
      title={editing === undefined ? "新建 Account" : `编辑 Account · ${editing}`}
      onEscape={onCancel}
    >
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          onSubmit(
            {
              id: String(data.get("id") ?? "").trim(),
              kind: String(data.get("kind") ?? "").trim(),
              secret: String(data.get("secret") ?? ""),
              status: String(data.get("status") ?? "active"),
            },
            editing,
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
            readOnly={editing !== undefined}
            defaultValue={editing ?? ""}
          />
        </label>
        <label>
          kind
          <input
            name="kind"
            className="mono"
            required
            defaultValue={form.mode === "edit" ? form.kind : "api_key"}
          />
        </label>
        <label>
          status
          <select name="status" defaultValue="active">
            <option value="active">active</option>
            <option value="disabled">disabled</option>
            <option value="revoked">revoked</option>
          </select>
        </label>
        <label>
          secret
          {/* Not type="password": Safari's password manager covers the field and
              swallows paste, and these are machine credentials that are always
              pasted. Same reason the unlock screen uses a masked text input. */}
          <input
            name="secret"
            className="mono"
            type="text"
            required
            autoComplete="off"
            spellCheck={false}
          />
        </label>
        {editing !== undefined ? (
          <p className="stat-sub">
            契约的 <span className="mono">CredentialInput.secret</span> 是必填,而读模型
            <strong>永不返回密钥</strong> —— 所以哪怕只想改状态,也必须重新输入密钥。
            这不是本页的限制,是 PATCH 整体替换语义的直接后果。
          </p>
        ) : null}
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            {editing === undefined ? "创建" : "保存"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}

export function SubresourcePanel({ upstreamId }: Readonly<{ upstreamId: string }>) {
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [testResults, setTestResults] = useState<Record<string, EndpointTest>>({});
  const [reconcile, setReconcile] = useState<string | undefined>();
  const [discovery, setDiscovery] = useState<
    { endpointId: string; diff: CatalogDiff; applied: boolean } | undefined
  >();
  const [accountTarget, setAccountTarget] = useState<string | undefined>();
  const [channelForm, setChannelForm] = useState<ChannelForm | undefined>();
  const [accountForm, setAccountForm] = useState<AccountForm | undefined>();
  const [bindingForm, setBindingForm] = useState<{ channelId: string } | undefined>();
  const [confirmDelete, setConfirmDelete] = useState<
    { kind: "channel" | "account"; id: string } | undefined
  >();
  const [error, setError] = useState<string | undefined>();
  const queryClient = useQueryClient();

  const pools = useQuery({
    queryKey: ["account-pools", scope, upstreamId],
    queryFn: () =>
      call<AccountPoolPage>(
        "listOperationalAccountPools",
        { query: { provider_id: upstreamId, limit: POOL_PAGE_LIMIT } },
        { versionScoped: true },
      ),
    enabled: scope !== undefined,
    staleTime: 10_000,
    retry: false,
  });

  const test = useMutation({
    mutationFn: (input: { endpointId: string; mode: "non_streaming" | "sse" }) =>
      call<EndpointTest>(
        "testEndpoint",
        { path: { endpoint_id: input.endpointId }, body: { mode: input.mode } },
        { versionScoped: true },
      ),
    onSuccess: (result, input) =>
      setTestResults((current) => ({ ...current, [input.endpointId]: result })),
    onError: (cause) => setError(asAppError(cause).message),
  });

  const preview = useMutation({
    mutationFn: (endpointId: string) =>
      call<CatalogDiff>(
        "previewCatalogDiscovery",
        { path: { endpoint_id: endpointId } },
        { versionScoped: true },
      ),
    onSuccess: (diff, endpointId) => setDiscovery({ endpointId, diff, applied: false }),
    onError: (cause) => setError(asAppError(cause).message),
  });

  const apply = useMutation({
    mutationFn: (endpointId: string) =>
      call<CatalogDiff>(
        "applyCatalogDiscovery",
        { path: { endpoint_id: endpointId } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: (diff, endpointId) => setDiscovery({ endpointId, diff, applied: true }),
    onError: (cause) => setError(asAppError(cause).message),
  });

  function refresh(): void {
    void queryClient.invalidateQueries({ queryKey: ["account-pools", scope, upstreamId] });
  }

  // Every one of these takes a WHOLE Input on PATCH — the contract has no
  // partial update for subresources, so each form is seeded with current
  // values and saving replaces the record.
  const saveChannel = useMutation({
    mutationFn: (input: { existing: string | undefined; body: unknown }) =>
      input.existing === undefined
        ? call<EndpointRecord>(
            "createEndpoint",
            { path: { upstream_id: upstreamId }, body: input.body },
            { versionScoped: true, mutating: true },
          )
        : call<EndpointRecord>(
            "updateEndpoint",
            { path: { endpoint_id: input.existing }, body: input.body },
            { versionScoped: true, mutating: true },
          ),
    onSuccess: () => {
      setChannelForm(undefined);
      refresh();
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const saveAccount = useMutation({
    mutationFn: (input: { existing: string | undefined; body: unknown }) =>
      input.existing === undefined
        ? call<unknown>(
            "createCredential",
            { path: { upstream_id: upstreamId }, body: input.body },
            { versionScoped: true, mutating: true },
          )
        : call<unknown>(
            "updateCredential",
            { path: { credential_id: input.existing }, body: input.body },
            { versionScoped: true, mutating: true },
          ),
    onSuccess: () => {
      setAccountForm(undefined);
      refresh();
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const saveBinding = useMutation({
    mutationFn: (input: { endpointId: string; body: unknown }) =>
      call<unknown>(
        "createEndpointCredentialBinding",
        { path: { endpoint_id: input.endpointId }, body: input.body },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setBindingForm(undefined);
      refresh();
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  const remove = useMutation({
    mutationFn: (target: { kind: "channel" | "account"; id: string }) =>
      target.kind === "channel"
        ? call<undefined>(
            "deleteEndpoint",
            { path: { endpoint_id: target.id } },
            { versionScoped: true, mutating: true },
          )
        : call<undefined>(
            "deleteCredential",
            { path: { credential_id: target.id } },
            { versionScoped: true, mutating: true },
          ),
    onSuccess: () => {
      setConfirmDelete(undefined);
      refresh();
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  if (pools.isError) {
    return (
      <div className="card empty-state" data-kind="unavailable">
        <p>
          读取运营库存失败
          <br />
          <small className="muted">{asAppError(pools.error).message}</small>
        </p>
      </div>
    );
  }
  if (pools.data === undefined) {
    return (
      <div className="card empty-state" data-kind="empty">
        <p>读取运营库存…</p>
      </div>
    );
  }

  const pool = providerPool(pools.data.items, upstreamId);
  const truncated = pools.data.next_cursor != null;

  if (pool === undefined) {
    return (
      <div className="card empty-state" data-kind="empty">
        <p>
          <span className="mono">{upstreamId}</span> 在本配置版本下没有任何绑定
          <br />
          <small className="muted">
            运营库存按<strong>绑定</strong>成行 —— 建了端点或凭据但尚未绑定,这里就不会出现。
            先在配置面建立 endpoint-credential 绑定。
          </small>
        </p>
      </div>
    );
  }

  return (
    <div className="card subresource-panel">
      {error !== undefined ? (
        <p role="alert" className="action-error">
          {error}
          <button type="button" onClick={() => setError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      {truncated ? (
        <p className="action-notice">
          该 provider 的绑定超过 {POOL_PAGE_LIMIT} 条,下面只显示第一页。
        </p>
      ) : null}

      <h3>
        Channel <span className="idchip mono">{pool.channels.length}</span>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "仅草稿版本可编辑"}
          onClick={() => setChannelForm({ mode: "create" })}
        >
          新建 Channel
        </button>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "仅草稿版本可编辑"}
          onClick={() => setBindingForm({ channelId: "" })}
        >
          加绑定
        </button>
      </h3>
      <p className="stat-sub">
        新建的 Channel 在<strong>绑定之前不会出现在下表</strong> —— 库存按绑定成行。
        用上面的「加绑定」把它接上凭据,它才会现身。
      </p>
      <table>
        <thead>
          <tr>
            <th>channel_id</th>
            <th>adapter</th>
            <th>api_format</th>
            <th>transport</th>
            <th>状态</th>
            <th>测试</th>
            <th>目录发现</th>
          </tr>
        </thead>
        <tbody>
          {pool.channels.map((channel) => {
            const result = testResults[channel.channel_id];
            return (
              <tr key={channel.channel_id}>
                <td className="mono">{channel.channel_id}</td>
                <td className="mono">{channel.adapter_id}</td>
                <td className="mono">{channel.api_format}</td>
                <td className="mono">{channel.transport}</td>
                <td>
                  <StatusBadge status={channel.channel_enabled ? "active" : "disabled"}>
                    {channel.channel_enabled ? "enabled" : "disabled"}
                  </StatusBadge>
                </td>
                <td className="row-actions">
                  <button
                    type="button"
                    className="secondary"
                    disabled={test.isPending}
                    onClick={() =>
                      test.mutate({ endpointId: channel.channel_id, mode: "non_streaming" })
                    }
                  >
                    非流式
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={test.isPending}
                    onClick={() => test.mutate({ endpointId: channel.channel_id, mode: "sse" })}
                  >
                    SSE
                  </button>
                  {result !== undefined ? (
                    <StatusBadge status={TEST_TONE[result.outcome]}>
                      {result.outcome}
                      {result.status_class !== undefined ? ` · ${result.status_class}` : ""}
                    </StatusBadge>
                  ) : null}
                </td>
                <td className="row-actions">
                  <button
                    type="button"
                    className="secondary"
                    disabled={preview.isPending}
                    onClick={() => preview.mutate(channel.channel_id)}
                  >
                    预览
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setBindingForm({ channelId: channel.channel_id })}
                  >
                    加绑定
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => setReconcile(channel.channel_id)}
                  >
                    核对绑定
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setChannelForm({ mode: "edit", channelId: channel.channel_id })}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={!editable}
                    onClick={() => setConfirmDelete({ kind: "channel", id: channel.channel_id })}
                  >
                    删除
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="stat-sub">
        运营库存不含地址 —— <span className="mono">base_url</span> 与
        <span className="mono">inference_path</span> 属于配置面,该投影按设计不返回 URL。
      </p>

      <h3>
        Account <span className="idchip mono">{pool.accounts.length}</span>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "仅草稿版本可编辑"}
          onClick={() => setAccountForm({ mode: "create" })}
        >
          新建 Account
        </button>
      </h3>
      <table>
        <thead>
          <tr>
            <th>account_id</th>
            <th>kind</th>
            <th>status</th>
            <th>revision</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          {pool.accounts.map((account) => (
            <tr key={account.account_id}>
              <td className="mono">{account.account_id}</td>
              <td className="mono">{account.account_kind}</td>
              <td>
                <StatusBadge status={accountStatusTone(account.account_status)}>
                  {account.account_status}
                </StatusBadge>
              </td>
              <td className="mono">{account.account_revision}</td>
              <td className="row-actions">
                <button type="button" onClick={() => setAccountTarget(account.account_id)}>
                  详情
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={!editable}
                  onClick={() =>
                    setAccountForm({
                      mode: "edit",
                      accountId: account.account_id,
                      kind: account.account_kind,
                    })
                  }
                >
                  编辑
                </button>
                <button
                  type="button"
                  className="danger"
                  disabled={!editable}
                  onClick={() => setConfirmDelete({ kind: "account", id: account.account_id })}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>
        绑定 <span className="idchip mono">{pool.bindings.length}</span>
      </h3>
      <table>
        <thead>
          <tr>
            <th>channel</th>
            <th>account</th>
            <th>静态启用</th>
            <th>priority</th>
            <th>weight</th>
            <th>concurrency</th>
            <th>route</th>
          </tr>
        </thead>
        <tbody>
          {pool.bindings.map((binding) => (
            <tr key={`${binding.channel_id}:${binding.account_id}`}>
              <td className="mono">{binding.channel_id}</td>
              <td className="mono">{binding.account_id}</td>
              <td>
                <StatusBadge status={binding.configured_enabled ? "active" : "disabled"}>
                  {binding.configured_enabled ? "enabled" : "disabled"}
                </StatusBadge>
              </td>
              <td className="mono">{binding.priority}</td>
              <td className="mono">{binding.weight}</td>
              <td className="mono">{binding.concurrency}</td>
              <td className="mono">
                {binding.route_ids.length === 0 ? "—" : binding.route_ids.join(" ")}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="stat-sub">
        「静态启用」= <span className="mono">provider &amp;&amp; channel &amp;&amp; binding</span>{" "}
        三者皆开。它<strong>不</strong>代表凭据健康、有额度或当前可路由 ——
        运行时状态要等 P13-06 的 Provider 池投影。
      </p>

      {reconcile !== undefined ? (
        <BindingReconcileSheet
          channelId={reconcile}
          operationalCredentialIds={
            new Set(
              pool.bindings
                .filter((binding) => binding.channel_id === reconcile)
                .map((binding) => binding.account_id),
            )
          }
          onClose={() => setReconcile(undefined)}
        />
      ) : null}

      {discovery !== undefined ? (
        <Sheet
          title={`目录发现 · ${discovery.endpointId}`}
          onEscape={() => setDiscovery(undefined)}
        >
          <div className="diff-row">
            <StatusBadge status="active">新增 {discovery.diff.added}</StatusBadge>
            <StatusBadge status="credential_forbidden">移除 {discovery.diff.removed}</StatusBadge>
            <StatusBadge status="archived">不变 {discovery.diff.unchanged}</StatusBadge>
          </div>
          {discovery.applied ? <p>已应用到草稿(revision 已推进)。</p> : null}
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setDiscovery(undefined)}>
              关闭
            </button>
            {!discovery.applied ? (
              <button
                type="button"
                disabled={!editable || apply.isPending}
                title={editable ? undefined : "仅草稿版本可应用"}
                onClick={() => apply.mutate(discovery.endpointId)}
              >
                应用到草稿
              </button>
            ) : null}
          </div>
        </Sheet>
      ) : null}

      {channelForm !== undefined ? (
        <ChannelSheet
          form={channelForm}
          pending={saveChannel.isPending}
          onCancel={() => setChannelForm(undefined)}
          onSubmit={(body, existing) => saveChannel.mutate({ existing, body })}
        />
      ) : null}

      {accountForm !== undefined ? (
        <AccountSheet
          form={accountForm}
          pending={saveAccount.isPending}
          onCancel={() => setAccountForm(undefined)}
          onSubmit={(body, existing) => saveAccount.mutate({ existing, body })}
        />
      ) : null}

      {bindingForm !== undefined ? (
        <Sheet
          title={bindingForm.channelId === "" ? "加绑定" : `加绑定 · ${bindingForm.channelId}`}
          onEscape={() => setBindingForm(undefined)}
        >
          <form
            className="sheet-form"
            onSubmit={(event: FormEvent<HTMLFormElement>) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              saveBinding.mutate({
                endpointId: String(data.get("channel_id") ?? "").trim(),
                body: {
                  credential_id: String(data.get("credential_id") ?? "").trim(),
                  enabled: data.get("enabled") === "on",
                  priority: Number(data.get("priority") ?? 0),
                  weight: Number(data.get("weight") ?? 1),
                  concurrency: Number(data.get("concurrency") ?? 1),
                },
              });
            }}
          >
            <label>
              channel_id
              <input
                name="channel_id"
                className="mono"
                required
                maxLength={128}
                list="pool-channels"
                defaultValue={bindingForm.channelId}
              />
            </label>
            <datalist id="pool-channels">
              {pool.channels.map((channel) => (
                <option key={channel.channel_id} value={channel.channel_id} />
              ))}
            </datalist>
            <p className="stat-sub">
              建议列表只含<strong>已有绑定</strong>的 Channel。刚建的那个不在其中 —— 直接把 id 输进来。
            </p>
            <label>
              credential_id
              <input name="credential_id" className="mono" required maxLength={128} list="pool-accounts" />
            </label>
            <datalist id="pool-accounts">
              {pool.accounts.map((account) => (
                <option key={account.account_id} value={account.account_id} />
              ))}
            </datalist>
            <label>
              priority
              <input name="priority" type="number" defaultValue={0} min={0} required />
            </label>
            <label>
              weight
              <input name="weight" type="number" defaultValue={1} min={0} required />
            </label>
            <label>
              concurrency
              <input name="concurrency" type="number" defaultValue={1} min={0} required />
            </label>
            <label className="check-row">
              <input name="enabled" type="checkbox" defaultChecked />
              启用
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setBindingForm(undefined)}>
                取消
              </button>
              <button type="submit" disabled={saveBinding.isPending}>
                添加
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDelete !== undefined ? (
        <Sheet title="确认删除" onEscape={() => setConfirmDelete(undefined)}>
          <p>
            删除 <span className="mono">{confirmDelete.id}</span>
            {confirmDelete.kind === "channel"
              ? " 会连带移除它的全部绑定,引用它的路由候选将失去目标 —— 该配置版本可能因此验证失败。"
              : " 会连带移除它的全部绑定。若某个 Channel 只剩这一个可用凭据,相关路由将无候选可选。"}
          </p>
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setConfirmDelete(undefined)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={remove.isPending}
              onClick={() => remove.mutate(confirmDelete)}
            >
              确认删除
            </button>
          </div>
        </Sheet>
      ) : null}

      {accountTarget !== undefined ? (
        <CredentialSheet
          credentialId={accountTarget}
          onClose={() => setAccountTarget(undefined)}
        />
      ) : null}
    </div>
  );
}
