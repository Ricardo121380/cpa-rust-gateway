import { Link } from "react-router-dom";
import { accountName, protocolName } from "../accounts/presentation";
import { ResourceIdInput } from "../../components/ResourceIdentity";
import { resourceName } from "../../utils/resourceNames";
import { ResourceIdentity } from "../../components/ResourceIdentity";
// Configuration inventories own endpoint/account enumeration; runtime pools supply observed bindings.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useState, useRef, type FormEvent, type ReactNode } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useVersionStore } from "../config-versions/versionStore";
import type { ManagementOperationName, ManagementRequest } from "../../generated/management-client";
import { CredentialSheet } from "./CredentialSheet";
import { accountStatusLabel, authenticationLabel, endpointLabel, runtimeConnectionLabel, sameEndpointConfiguration } from "./subresourceModel";
import { useManagedInventory, type ManagedEndpoint } from "../accounts/inventory";
import { runProviderResourceTask, type ProviderResourceReceipt } from "./providerResourceTask";
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

/** Full endpoint record. The operational inventory is URL-free by contract, so
 *  editing needs this read: PATCH takes a whole EndpointInput and a form that
 *  cannot pre-fill base_url would blank it on save. */
type EndpointRecord = Readonly<{
  id: string;
  upstream_id: string;
  adapter_id: string;
  api_format: string;
  base_url: string;
  inference_path: string;
  models_path?: string | null;
  transport: string;
  enabled: boolean;
}>;

type ChannelForm = { mode: "create" } | { mode: "edit"; channelId: string };
type AccountForm = { mode: "create" } | { mode: "edit"; accountId: string; kind: string; status:string; revision:number };
type DeleteTarget = { kind: "channel" | "account"; id: string; expected: unknown };
type ProviderAction =
  | { kind: "reconcile"; channelId: string }
  | { kind: "account-inspector"; accountId: string }
  | { kind: "channel-form"; form: ChannelForm }
  | { kind: "account-form"; form: AccountForm }
  | { kind: "binding-form"; channelId: string }
  | { kind: "confirm-delete"; target: DeleteTarget }
  | { kind: "receipt"; receipt: ProviderResourceReceipt };

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

type BindingInput = Readonly<{
  credential_id: string;
  enabled: boolean;
  priority: number;
  weight: number;
  concurrency: number;
}>;

type SheetSubmit = (body: unknown, existing: string | undefined, expected?:unknown) => void;

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
    <Sheet
      title="核对接口连接"
      description="核对此接口的已保存连接；不一致的连接需要在高级配置中处理。"
      layout="inspector"
      onEscape={onClose}
      footer={<SheetDismissButton>关闭</SheetDismissButton>}
    >
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
        <table className="responsive-table">
          <thead>
            <tr>
              <th>credential</th>
              <th>upstream</th>
              <th>启用</th>
              <th>优先级</th>
              <th>权重</th>
              <th>并发上限</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.credential_id}>
                <td className="mono" data-label="账号">
                  <ResourceIdentity id={row.credential_id} kind="account" />
                  {operationalCredentialIds.has(row.credential_id) ? null : (
                    <strong> · 运营库存里没有</strong>
                  )}
                </td>
                <td data-label="提供商"><ResourceIdentity id={row.upstream_id} kind="upstream" /></td>
                <td data-label="状态">{row.enabled ? "已启用" : "已停用"}</td>
                <td className="mono" data-label="优先级">{row.priority}</td>
                <td className="mono" data-label="权重">{row.weight}</td>
                <td className="mono" data-label="并发上限">{row.concurrency}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      {hidden.length > 0 ? (
        <p role="alert" className="reveal-warning">
          有 {hidden.length} 条绑定只存在于配置里:{hidden.map((r) => resourceName(r.credential_id, "account")).join("、")}。
          它们指向的凭据无法解析,所以运营库存不显示 —— 但校验与发布仍然会看到它们。
        </p>
      ) : null}
    </Sheet>
  );
}

function BindingSheet({
  initialEndpointId,
  channels,
  accounts,
  pending,
  onCancel,
  onSubmit,
  feedback,
}: Readonly<{
  initialEndpointId: string;
  channels: readonly { channel_id: string; display: string }[];
  accounts: readonly { account_id: string; display: string; provider: string }[];
  pending: boolean;
  feedback?: ReactNode;
  onCancel: () => void;
  onSubmit: (input: { endpointId: string; body: BindingInput }) => void;
}>) {
  const formId = useId();
  return (
    <Sheet
      title="连接账号"
      description="选择一个接口和账号；调度策略默认使用均衡、安全的值。"
      onEscape={onCancel}
      busy={pending}
      footer={<><SheetDismissButton className="secondary" disabled={pending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={pending}>保存连接</button></>}
    >
      {feedback}
      <form
        id={formId}
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          onSubmit({
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
          接口
          <select name="channel_id" required defaultValue={initialEndpointId}>
            <option value="" disabled>选择接口</option>
            {channels.map((channel) => <option key={channel.channel_id} value={channel.channel_id}>{channel.display}</option>)}
          </select>
        </label>
        <label>
          账号
          <select name="credential_id" required defaultValue="">
            <option value="" disabled>选择账号</option>
            {accounts.map((account) => <option key={account.account_id} value={account.account_id}>{account.display} · {account.provider}</option>)}
          </select>
        </label>
        <details className="sheet-advanced">
          <summary>调度设置</summary>
          <label>
            优先级
            <input name="priority" type="number" defaultValue={0} min={0} required />
          </label>
          <label>
            权重
            <input name="weight" type="number" defaultValue={1} min={1} max={10000} required />
          </label>
          <label>
            并发上限
            <input name="concurrency" type="number" defaultValue={1} min={1} max={100000} required />
          </label>
          <label className="check-row">
            <input name="enabled" type="checkbox" defaultChecked />
            立即启用连接
          </label>
        </details>
      </form>
    </Sheet>
  );
}

function ChannelSheet({
  form,
  pending,
  onCancel,
  onSubmit,
  feedback,
}: Readonly<{ form: ChannelForm; pending: boolean; feedback?:ReactNode; onCancel: () => void; onSubmit: SheetSubmit }>) {
  const formId = useId();
  const editing = form.mode === "edit" ? form.channelId : undefined;
  const original=useRef<EndpointRecord|undefined>(undefined);
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
      <Sheet
        title="读取接口"
        description="正在读取现有接口设置。"
        onEscape={onCancel}
        footer={<SheetDismissButton>关闭</SheetDismissButton>}
      >
        <p className="muted">{record.isError ? "读取端点失败" : "读取端点…"}</p>
      </Sheet>
    );
  }
  if(!original.current&&record.data)original.current=record.data;
  const current = original.current;

  return (
    <Sheet
      title={editing === undefined ? "添加接口" : "编辑接口"}
      description={editing === undefined ? "填写接口地址和请求格式；账号连接在保存后单独设置。" : "修改接口地址、协议或目录路径。保存后会重新读取配置。"}
      onEscape={onCancel}
      busy={pending}
      footer={<><SheetDismissButton className="secondary" disabled={pending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={pending}>{editing === undefined ? "创建接口" : "保存修改"}</button></>}
    >
      {feedback}
      <form
        id={formId}
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
            current,
          );
        }}
      >
        <label>
          {editing === undefined ? "接口标识" : "接口"}
          <ResourceIdInput kind="endpoint"
            name="id"
            className="mono"
            required
            maxLength={128}
            readOnly={editing !== undefined}
            defaultValue={current?.id ?? `endpoint-${crypto.randomUUID()}`}
          />
        </label>
        <label>
          接口实现
          <input name="adapter_id" className="mono" required defaultValue={current?.adapter_id ?? ""} />
        </label>
        <label>
          请求协议
          <input name="api_format" className="mono" required defaultValue={current?.api_format ?? ""} />
        </label>
        <label>
          接口地址
          <input name="base_url" className="mono" required defaultValue={current?.base_url ?? ""} />
        </label>
        <label>
          请求路径
          <input
            name="inference_path"
            className="mono"
            required
            defaultValue={current?.inference_path ?? ""}
          />
        </label>
        <details className="sheet-advanced">
          <summary>高级设置</summary>
          <label>
            模型目录路径（可选）
          <input name="models_path" className="mono" defaultValue={current?.models_path ?? ""} />
          </label>
          <label className="check-row">
            <input name="enabled" type="checkbox" defaultChecked={current?.enabled ?? true} />
            启用此接口
          </label>
        </details>
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
  displayName,
  pending,
  onCancel,
  onSubmit,
  feedback,
}: Readonly<{ form: AccountForm; displayName?: string; pending: boolean; feedback?:ReactNode; onCancel: () => void; onSubmit: SheetSubmit }>) {
  const formId = useId();
  const editing = form.mode === "edit" ? form.accountId : undefined;
  return (
    <Sheet
      title={editing === undefined ? "添加原始凭据" : "高级编辑凭据"}
      description="仅供高级维护使用。日常授权或导入请使用“添加账号”。"
      onEscape={onCancel}
      busy={pending}
      footer={<><SheetDismissButton className="secondary" disabled={pending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={pending}>{editing === undefined ? "保存原始凭据" : "保存凭据"}</button></>}
    >
      {feedback}
      <form
        id={formId}
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const secretField=event.currentTarget.elements.namedItem("secret");if(secretField instanceof HTMLInputElement)secretField.value="";
          onSubmit(
            {
              id: String(data.get("id") ?? "").trim(),
              kind: String(data.get("kind") ?? "").trim(),
              secret: String(data.get("secret") ?? ""),
              status: String(data.get("status") ?? "active"),
            },
            editing,
            form.mode==="edit"?form.revision:undefined,
          );
        }}
      >
        <label>
          {editing === undefined ? "凭据标识" : "账号"}
          <ResourceIdInput
            name="id" kind="account" displayName={displayName??"未提供账号身份"}
            className="mono"
            required
            maxLength={128}
            readOnly={editing !== undefined}
            defaultValue={editing ?? ""}
          />
        </label>
        <label>
          认证方式
          <select name="kind" required defaultValue={form.mode === "edit" ? form.kind : "bearer"}>
            <option value="bearer">API Key / Token</option>
            <option value="oauth_json">OAuth 数据（高级）</option>
            {form.mode === "edit" && !["bearer", "oauth_json"].includes(form.kind) ? (
              <option value={form.kind}>{form.kind}（现有类型）</option>
            ) : null}
          </select>
        </label>
        <label>
          状态
          <select name="status" defaultValue={form.mode==="edit"?form.status:"active"}>
            <option value="active">active</option>
            <option value="disabled">disabled</option>
            <option value="revoked">revoked</option>
          </select>
        </label>
        <label>
          授权资料
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
        {editing!==undefined?<p className="muted">更新授权资料会重新校验账号。只需启停时，可在全部账号中操作。</p>:null}
      </form>
    </Sheet>
  );
}

function ProviderReceipt({ receipt, onDone }: Readonly<{ receipt: ProviderResourceReceipt; onDone: () => void }>) {
  const review = receipt.kind === "saved_draft" || receipt.kind === "saved_unapplied" || receipt.kind === "unconfirmed";
  return <div className="card subresource-panel" role="status">
    <h3>{receipt.kind === "saved_applied" ? "修改已应用" : receipt.kind === "saved_draft" ? "修改已保存到草稿" : receipt.kind === "unconfirmed" ? "修改结果未确认" : "修改需要核对"}</h3>
    <p>{receipt.message}</p>
    <p className="muted">工作配置：{resourceName(receipt.workingVersion.id, "config")}</p>
    <div className="page-actions"><button type="button" className="secondary" onClick={onDone}>{review ? "查看工作配置" : "完成"}</button></div>
  </div>;
}

export function SubresourcePanel({ upstreamId, onAddAccount, onActionActiveChange }: Readonly<{ upstreamId: string; onAddAccount: () => void; onActionActiveChange: (active: boolean) => void }>) {
  const context = useVersionStore((s) => s.context);
  const editable = context!==undefined;
  const scope = context?.configVersionId;
  const [testResults, setTestResults] = useState<Record<string, EndpointTest>>({});
  const [action, setAction] = useState<ProviderAction | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [savingAccount, setSavingAccount] = useState(false);
  const savingAccountRef = useRef(false);
  const editResource=async(operation:ManagementOperationName,request:ManagementRequest,expected?:unknown)=>{
    return runProviderResourceTask("修改提供商连接", async(task, write)=>{
      if(operation==="updateEndpoint"&&!sameEndpointConfiguration(await task.read<EndpointRecord>("getEndpoint",{path:request.path}), expected as ManagedEndpoint))throw new Error("接口已被修改，请重新读取后编辑。");
      if(operation==="updateCredential"&&(await task.read<{revision:number}>("getCredential",{path:request.path})).revision!==expected)throw new Error("账号已被修改，请重新读取后编辑。");
      if(operation==="deleteEndpoint"&&!sameEndpointConfiguration(await task.read<EndpointRecord>("getEndpoint",{path:request.path}), expected as ManagedEndpoint))throw new Error("接口已变化，请重新核对后删除。");
      if(operation==="deleteCredential"&&(await task.read<{revision:number}>("getCredential",{path:request.path})).revision!==expected)throw new Error("账号已变化，请重新核对后删除。");
      await write(operation,request);
    });
  };
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

  const managedEndpoints = useManagedInventory("endpoints", upstreamId);
  const managedCredentials = useManagedInventory("credentials", upstreamId);

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

  function refresh(): void {
    void queryClient.invalidateQueries({ queryKey: ["account-pools", scope, upstreamId] });
    void queryClient.resetQueries({ queryKey: ["managed-inventory", scope] });
  }

  // Every one of these takes a WHOLE Input on PATCH — the contract has no
  // partial update for subresources, so each form is seeded with current
  // values and saving replaces the record.
  const saveChannel = useMutation({
    mutationFn: (input: { existing: string | undefined; body: unknown; expected?:unknown }) => editResource(input.existing?"updateEndpoint":"createEndpoint",{path:input.existing?{endpoint_id:input.existing}:{upstream_id:upstreamId},body:input.body},input.expected),
    onSuccess:({receipt})=>setAction({ kind: "receipt", receipt }),
    onError:(cause)=>setError(asAppError(cause).message),
  });
  const saveAdvancedAccount = async (input:{existing:string|undefined;body:unknown;expected?:unknown}) => {
    if (savingAccountRef.current) return;
    savingAccountRef.current = true;
    setSavingAccount(true);
    setError(undefined);
    try {
      const result = await editResource(input.existing ? "updateCredential" : "createCredential", { path: input.existing ? { credential_id: input.existing } : { upstream_id: upstreamId }, body: input.body }, input.expected);
      setAction({ kind: "receipt", receipt: result.receipt });
    } catch (cause) {
      setError(asAppError(cause).message);
    } finally {
      savingAccountRef.current = false;
      setSavingAccount(false);
    }
  };
  const saveBinding=useMutation({
    mutationFn:(input:{endpointId:string;body:BindingInput})=>editResource("createEndpointCredentialBinding",{path:{endpoint_id:input.endpointId},body:input.body}),
    onSuccess:({receipt})=>setAction({ kind: "receipt", receipt }),
    onError:(cause)=>setError(asAppError(cause).message),
  });
  const remove=useMutation({
    mutationFn:(target:DeleteTarget)=>editResource(target.kind==="channel"?"deleteEndpoint":"deleteCredential",{path:target.kind==="channel"?{endpoint_id:target.id}:{credential_id:target.id}},target.expected),
    onSuccess:({receipt})=>setAction({ kind: "receipt", receipt }),
    onError:(cause)=>setError(asAppError(cause).message),
  });
  const feedback=error ? <p role="alert" className="action-error">{error}</p> : null;

  useEffect(() => {
    onActionActiveChange(action !== undefined);
    return () => onActionActiveChange(false);
  }, [action, onActionActiveChange]);

  if (action?.kind === "receipt") {
    return <ProviderReceipt receipt={action.receipt} onDone={() => {
      useVersionStore.getState().select(action.receipt.workingVersion);
      setAction(undefined);
      refresh();
    }} />;
  }

  const inventoryError = managedEndpoints.error ?? managedCredentials.error;
  // An active Sheet owns its own input and exact target. A background
  // inventory refresh must not destroy that operation (or its receipt) merely
  // because its list request failed or a just-deleted row disappeared.
  if (inventoryError && action === undefined) {
    return <div className="card empty-state" role="alert">
      <p>{asAppError(inventoryError).message}</p>
      <button type="button" onClick={refresh}>重新读取账号与端点</button>
    </div>;
  }
  if ((!managedEndpoints.data || !managedCredentials.data) && action === undefined) {
    return <div className="card empty-state">读取账号与端点…</div>;
  }
  const observed = providerPool(pools.data?.items ?? [], upstreamId);
  const runtimeConnectionState = pools.isError
    ? "unavailable"
    : pools.data === undefined
      ? "loading"
      : pools.data.next_cursor !== null && pools.data.next_cursor !== undefined
        ? "partial"
        : "observed";
  const pool = {
    channels: (managedEndpoints.data?.pages ?? []).flatMap((page) => page.items).map((endpoint) => ({
      channel_id: endpoint.id, display: endpointLabel(endpoint), record: endpoint, adapter_id: endpoint.adapter_id, api_format: endpoint.api_format,
      transport: endpoint.transport, channel_enabled: endpoint.enabled,
      account_ids: observed?.channels.find((channel) => channel.channel_id === endpoint.id)?.account_ids ?? [],
    })),
    accounts: (managedCredentials.data?.pages ?? []).flatMap((page) => page.items).map((account) => ({
      ...account,
      credential: account.credential,
      identity: account.identity,
      display: accountName(account.identity) ?? "未提供账号身份",
      provider: account.provider,
      account_id: account.credential.id,
      account_kind: account.credential.kind,
      account_status: account.credential.status,
      account_revision: account.credential.revision,
    })),
    bindings: observed?.bindings ?? [],
  };
  const truncated = pools.data?.next_cursor != null;
  const accountForm = action?.kind === "account-form" ? action.form : undefined;

  return (
    <div className="card subresource-panel">
      {inventoryError !== undefined ? (
        <p role="alert" className="action-error">
          {asAppError(inventoryError).message}
          <button type="button" onClick={refresh}>重新读取账号与端点</button>
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

      {pools.isError ? <p role="alert">运行绑定读取失败，账号与端点仍可管理。<button className="secondary" onClick={() => void pools.refetch()}>重新读取绑定</button></p> : null}
      {truncated ? (
        <p className="action-notice">
          该 provider 的绑定超过 {POOL_PAGE_LIMIT} 条,下面只显示第一页。
        </p>
      ) : null}

      <h3>
        接口 <span className="idchip mono">{pool.channels.length}</span>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "请先读取当前配置"}
          onClick={() => setAction({ kind: "channel-form", form: { mode: "create" } })}
        >
          新建接口
        </button>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "请先读取当前配置"}
          onClick={() => setAction({ kind: "binding-form", channelId: "" })}
        >
          连接账号
        </button>
      </h3>
      <div className="subresource-list" aria-label="接口列表">
          {pool.channels.map((channel) => {
            const result = testResults[channel.channel_id];
            return (
              <article className="subresource-row" data-resource-id={channel.channel_id} key={channel.channel_id}>
                <div className="subresource-identity">
                  <span className="subresource-label">接口</span>
                  <strong>{channel.display}</strong>
                  <span className="subresource-meta">{runtimeConnectionLabel(channel.account_ids.length, runtimeConnectionState)}</span>
                </div>
                <dl className="subresource-facts">
                  <div><dt>协议</dt><dd>{protocolName(channel.api_format)}</dd></div>
                  <div><dt>配置状态</dt><dd>
                  <StatusBadge status={channel.channel_enabled ? "active" : "disabled"}>
                    {channel.channel_enabled ? "已启用" : "已停用"}
                  </StatusBadge>
                  </dd></div>
                </dl>
                <div className="subresource-actions" aria-label={`${channel.display} 操作`}>
                  <span className="subresource-action-label">接口测试</span>
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
                </div>
                <div className="subresource-actions" aria-label={`${channel.display} 维护操作`}>
                  <Link to={`/catalog?endpoint_id=${encodeURIComponent(channel.channel_id)}`}>模型目录</Link>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setAction({ kind: "binding-form", channelId: channel.channel_id })}
                  >
                    连接账号
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={pools.isError || pools.data === undefined}
                    onClick={() => setAction({ kind: "reconcile", channelId: channel.channel_id })}
                  >
                    核对绑定
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!editable}
                    onClick={() => setAction({ kind: "channel-form", form: { mode: "edit", channelId: channel.channel_id } })}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={!editable}
                    onClick={() => setAction({ kind: "confirm-delete", target: { kind: "channel", id: channel.channel_id, expected: channel.record } })}
                  >
                    删除
                  </button>
                </div>
              </article>
            );
          })}
      </div>
      {managedEndpoints.hasNextPage ? <button className="secondary" disabled={managedEndpoints.isFetchingNextPage} onClick={() => void managedEndpoints.fetchNextPage()}>加载更多端点</button> : null}
      {pool.channels.length === 0 ? <p className="empty-state">尚未添加端点。</p> : null}

      {managedCredentials.hasNextPage ? <button className="secondary" disabled={managedCredentials.isFetchingNextPage} onClick={() => void managedCredentials.fetchNextPage()}>加载更多账号</button> : null}
      <h3>
        账号 <span className="idchip mono">{pool.accounts.length}</span>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "请先读取当前配置"}
          onClick={onAddAccount}
        >
          添加账号
        </button>
      </h3>
      <div className="subresource-list" aria-label="账号列表">
          {pool.accounts.map((account) => (
            <article className="subresource-row" data-resource-id={account.account_id} key={account.account_id}>
              <div className="subresource-identity">
                <span className="subresource-label">账号</span>
                <strong>{account.display}</strong>
                <span className="subresource-meta">{account.provider}</span>
              </div>
              <dl className="subresource-facts">
                <div><dt>接入方式</dt><dd>{authenticationLabel(account)}</dd></div>
                <div><dt>状态</dt><dd>
                <StatusBadge status={account.account_status === "revoked" ? "disabled" : accountStatusTone(account.account_status)}>
                  {accountStatusLabel(account.account_status)}
                </StatusBadge>
                </dd></div>
              </dl>
              <div className="subresource-actions" aria-label={`${account.display} 操作`}>
                <button type="button" onClick={() => setAction({ kind: "account-inspector", accountId: account.account_id })}>
                  详情
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={!editable || ["cooling", "unauthorized"].includes(account.account_status)}
                  title={["cooling", "unauthorized"].includes(account.account_status) ? "请先在账号管理中处理当前运行状态。" : undefined}
                  onClick={() =>
                    setAction({
                      kind: "account-form",
                      form: {
                        mode: "edit",
                        accountId: account.account_id,
                        kind: account.account_kind,
                        status: account.account_status,
                        revision:account.account_revision,
                      },
                    })
                  }
                >
                    高级编辑凭据
                </button>
                <button
                  type="button"
                  className="danger"
                  disabled={!editable}
                  onClick={() => setAction({ kind: "confirm-delete", target: { kind: "account", id: account.account_id, expected: account.account_revision } })}
                >
                  删除
                </button>
              </div>
            </article>
          ))}
      </div>

      <details className="provider-scheduling">
        <summary>高级凭据维护</summary>
        <p className="stat-sub">原始凭据只用于受控维护；日常账号接入请使用“添加账号”。</p>
        <button type="button" className="secondary" disabled={!editable} onClick={() => setAction({ kind: "account-form", form: { mode: "create" } })}>添加原始凭据</button>
      </details>

      <details className="provider-scheduling"><summary>高级调度配置</summary>
      <h3>
        绑定 <span className="idchip mono">{pools.isError || !pools.data ? "—" : pool.bindings.length}</span>
      </h3>
      <div className="tablewrap provider-scheduling-table"><table className="responsive-table">
        <thead>
          <tr>
            <th>接口</th>
            <th>账号</th>
            <th>连接状态</th>
            <th>优先级</th>
            <th>权重</th>
            <th>并发上限</th>
            <th>路由</th>
          </tr>
        </thead>
        <tbody>
          {pool.bindings.map((binding) => (
            <tr key={`${binding.channel_id}:${binding.account_id}`} data-endpoint-id={binding.channel_id} data-account-id={binding.account_id}>
              <td data-label="接口"><ResourceIdentity id={binding.channel_id} kind="endpoint" name={pool.channels.find(c=>c.channel_id===binding.channel_id)?.display} /></td>
              <td data-label="账号"><ResourceIdentity id={binding.account_id} kind="account" name={pool.accounts.find(a=>a.account_id===binding.account_id)?.display} /></td>
              <td data-label="状态">
                <StatusBadge status={binding.configured_enabled ? "active" : "disabled"}>
                  {binding.configured_enabled ? "已启用" : "已停用"}
                </StatusBadge>
              </td>
              <td className="mono" data-label="优先级">{binding.priority}</td>
              <td className="mono" data-label="权重">{binding.weight}</td>
              <td className="mono" data-label="并发上限">{binding.concurrency}</td>
              <td className="mono" data-label="路由">
                {binding.route_ids.length === 0 ? "—" : binding.route_ids.map((id)=>resourceName(id,"route")).join(" · ")}
              </td>
            </tr>
          ))}
        </tbody>
      </table></div>
      <p className="stat-sub">
        认证、额度和调度情况见账号管理的运行状态。
      </p>

      </details>
      {action?.kind === "reconcile" ? (
        <BindingReconcileSheet
          channelId={action.channelId}
          operationalCredentialIds={
            new Set(
              pool.bindings
                .filter((binding) => binding.channel_id === action.channelId)
                .map((binding) => binding.account_id),
            )
          }
          onClose={() => setAction(undefined)}
        />
      ) : null}

      {action?.kind === "channel-form" ? (
        <ChannelSheet
          form={action.form}
          pending={saveChannel.isPending}
          feedback={feedback}
          onCancel={() => !saveChannel.isPending&&setAction(undefined)}
          onSubmit={(body, existing,expected) => saveChannel.mutate({ existing, body,expected })}
        />
      ) : null}

      {accountForm !== undefined ? (
        <AccountSheet
          form={accountForm}
          displayName={accountForm.mode === "edit" ? pool.accounts.find((account) => account.account_id === accountForm.accountId)?.display : undefined}
          pending={savingAccount}
          feedback={feedback}
          onCancel={() => !savingAccount&&setAction(undefined)}
          onSubmit={(body, existing,expected) => void saveAdvancedAccount({ existing, body,expected })}
        />
      ) : null}

      {action?.kind === "binding-form" ? <BindingSheet initialEndpointId={action.channelId} channels={pool.channels} accounts={pool.accounts} pending={saveBinding.isPending} feedback={feedback} onCancel={() => !saveBinding.isPending && setAction(undefined)} onSubmit={(input) => saveBinding.mutate(input)} /> : null}

      {action?.kind === "confirm-delete" ? (
        <Sheet title="确认删除" description="此操作会修改当前配置；保存后会显示应用结果。" layout="confirm" tone="danger" busy={remove.isPending} onEscape={() => !remove.isPending&&setAction(undefined)} footer={<><SheetDismissButton className="secondary" disabled={remove.isPending}>取消</SheetDismissButton><button type="button" className="danger" disabled={remove.isPending} onClick={() => remove.mutate(action.target)}>确认删除</button></>}>
          {feedback}
          <p>
            删除 <span className="mono">{resourceName(action.target.id,action.target.kind==="channel"?"endpoint":"account")}</span>
            {action.target.kind === "channel"
              ? " 会连带移除它的全部绑定,引用它的路由候选将失去目标 —— 该配置版本可能因此验证失败。"
              : " 会连带移除它的全部绑定。若某个 Channel 只剩这一个可用凭据,相关路由将无候选可选。"}
          </p>
        </Sheet>
      ) : null}

      {action?.kind === "account-inspector" ? (
        <CredentialSheet
          credentialId={action.accountId}
          accountName={pool.accounts.find(a=>a.account_id===action.accountId)?.display}
          providerName={pool.accounts.find(a=>a.account_id===action.accountId)?.provider}
          category={(() => { const category = pool.accounts.find(a=>a.account_id===action.accountId)?.category; return category === "codex" || category === "kimi" ? category : undefined; })()}
          onClose={() => setAction(undefined)}
        />
      ) : null}
    </div>
  );
}
