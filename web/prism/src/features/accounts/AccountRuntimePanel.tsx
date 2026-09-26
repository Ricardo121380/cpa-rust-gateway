import { PagedReadStatus } from "../../components/PagedReadStatus";
import { IdentityDetails } from "../../components/ResourceIdentity";
import {accountName, accountGroups, protocolName} from "./presentation";
import {
  isCancelledError,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  useQuery,
} from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import { useSessionStore } from "../../session/sessionStore";
import { EntitlementEvidence } from "../runtime/EntitlementEvidence";
import {
  authStatusMeta,
  runtimeStatusMeta,
  formatObservedAt,
  type PoolAccount,
  type PoolSnapshot,
} from "../runtime/model";
import { CredentialSheet } from "../upstreams/CredentialSheet";
import { PoolActionSheet } from "../runtime/PoolActionSheet";
import { NativeAccountDialog } from "./NativeAccountDialog";
import { GrokDeviceWizard } from "./GrokDeviceWizard";
import type { NativeAccount } from "./NativeAccounts";
import {
  receiptMeta,
  type PoolAction,
  type ActionReceipt,
} from "../runtime/model";

const runtimeName=(row:PoolAccount)=>accountName(row.presentation?.identity)??"未提供账号身份";
const runtimeProvider=(row:PoolAccount)=>row.presentation?.provider??({grok_build_oauth:"Grok Build",grok_console_sso:"Grok Console",grok_web_sso:"Grok Web",oauth_json:"Codex"}[row.account_kind]??"API");
const runtimeConnection=(row:PoolAccount)=>row.presentation?`${protocolName(row.presentation.api_format)}${row.presentation.host?` · ${row.presentation.host}`:""}`:"连接信息未提供";
type RuntimeSelection=Readonly<{account:PoolAccount;snapshotId:string|undefined;observedAt:number|undefined}>;
type RuntimeActionTarget=RuntimeSelection&Readonly<{action:PoolAction;upstreamModel?:string}>;
type RuntimeActionReceipt=Readonly<{target:RuntimeActionTarget;kind:"received";value:ActionReceipt}|{target:RuntimeActionTarget;kind:"unconfirmed";message:string}>;
type CredentialMaintenance=Readonly<{kind:"native";account:NativeAccount}|{kind:"ordinary";credentialId:string;account:PoolAccount}>;
const nativeProviderForKind=(kind:string):NativeAccount["provider"]|undefined=>({grok_build_oauth:"grok_build",grok_console_sso:"grok_console",grok_web_sso:"grok_web"}[kind] as NativeAccount["provider"]|undefined);

export function AccountRuntimePanel({navigation}: Readonly<{navigation?: ReactNode}>) {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const sessionGeneration = useSessionStore((s) => s.generation);
  const scope = context?.configVersionId;
  const [params, setParams] = useSearchParams();
  const [selected, setSelected] = useState<RuntimeSelection>();
  const [tab, setTab] = useState("runtime");
  const [credential, setCredential] = useState<RuntimeSelection>();
  const [nativeResolution, setNativeResolution] = useState(0);
  const [ordinaryResolution, setOrdinaryResolution] = useState(0);
  const [maintenance, setMaintenance] = useState<CredentialMaintenance>();
  const [nativeOauth, setNativeOauth] = useState<NativeAccount>();
  const [target, setTarget] = useState<RuntimeActionTarget>();
  const [receipt, setReceipt] = useState<RuntimeActionReceipt>();
  const [actionError, setActionError] = useState<string>();
  const [targetError, setTargetError] = useState<string>();
  const provider = params.get("provider") ?? "";
  const auth = params.get("auth") ?? "";
  const runtime = params.get("runtime") ?? "";
  const query = params.get("q") ?? "";
  const exactAccount=params.get("account_id")??"";
  const exactChannel=params.get("channel_id")??"";
  const category=params.get("category")??"";
  const key = ["accounts", provider, auth, runtime, exactChannel];
  const act = useMutation({
    mutationFn: ({body}: {target:RuntimeActionTarget;body:Readonly<Record<string, unknown>>}) =>
      call<ActionReceipt>(
        "applyProviderAccountPoolAction",
        { body },
        { versionScoped: true },
    ),
    onSuccess: (result, input) => {
      setTarget(undefined);
      setReceipt({target:{...input.target,upstreamModel:typeof input.body.upstream_model==="string"?input.body.upstream_model:undefined},kind:"received",value:result});
      void queryClient.resetQueries({ queryKey: ["accounts"] });
      void queryClient.invalidateQueries({ queryKey: ["provider-pools"] });
    },
    onError: (cause, input) => {
      if(isCancelledError(cause))return;
      const error = asAppError(cause);
      if (error.kind === "conflict") {
        setTarget(undefined);
        void queryClient.resetQueries({ queryKey: ["accounts"] });
        setActionError("目标快照已改变，已重新读取。请重新选取账号后再操作。");
      } else if(error.kind === "invalid_request") setTargetError(`${error.code} · ${error.message}`);
      else {
        setTarget(undefined);
        setReceipt({target:{...input.target,upstreamModel:typeof input.body.upstream_model==="string"?input.body.upstream_model:undefined},kind:"unconfirmed",message:`操作请求的结果未确认：${error.message}。已重新读取运行状态；请不要重复提交本次操作。`});
        void queryClient.resetQueries({ queryKey: ["accounts"] });
      }
    },
  });
  const pools = useInfiniteQuery({
    queryKey: key,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      call<PoolSnapshot>("listProviderAccountPools", {
        query: {
          limit: 100,
          ...(provider ? { provider_id: provider } : {}),
          ...(exactChannel?{channel_id:exactChannel}:{}),
          ...(auth ? { auth_status: auth } : {}),
          ...(runtime ? { runtime_status: runtime } : {}),
          ...(pageParam ? { cursor: pageParam } : {}),
        },
      }),
    getNextPageParam: (last) => last.next_cursor ?? undefined,
    retry: false,
  });
  const selectedNativeProvider=credential===undefined?undefined:nativeProviderForKind(credential.account.account_kind);
  const ordinaryCredential=useQuery({
    queryKey:["runtime-credential-resolution",sessionGeneration,ordinaryResolution,scope,context?.revision,credential?.account.provider_id,credential?.account.channel_id,credential?.account.account_id],
    enabled:credential!==undefined&&selectedNativeProvider===undefined&&scope!==undefined,
    retry:false,
    queryFn:async()=>{
      const target=credential!;
      const value=await call<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:target.account.account_id}},{versionScoped:true});
      if(value.id!==target.account.account_id||value.upstream_id!==target.account.provider_id)throw new Error("当前配置中的凭据不属于所选运行时提供商。");
      const bindings=await call<readonly {credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:target.account.channel_id}},{versionScoped:true});
      if(!bindings.some((binding)=>binding.credential_id===target.account.account_id))throw new Error("当前配置未将该凭据绑定到所选接口。");
      return value;
    },
  });
  const nativeInventory=useInfiniteQuery({
    queryKey:["runtime-native-resolution",sessionGeneration,nativeResolution,credential?.account.account_id,selectedNativeProvider],
    enabled:credential!==undefined&&selectedNativeProvider!==undefined,
    retry:false,
    initialPageParam:undefined as string|undefined,
    queryFn:({pageParam})=>call<{items:readonly NativeAccount[];next_cursor:string|null}>("listNativeAccounts",{query:{limit:100,...(pageParam?{cursor:pageParam}:{})}}),
    getNextPageParam:(page)=>page.next_cursor??undefined,
  });
  const nativeMatch=(nativeInventory.data?.pages.flatMap((page)=>page.items)??[])
    .find((row)=>credential!==undefined&&selectedNativeProvider!==undefined&&row.id===credential.account.account_id&&row.provider===selectedNativeProvider);
  const resolvingCredential=credential!==undefined&&(selectedNativeProvider===undefined?ordinaryCredential.isPending||ordinaryCredential.isFetching:nativeInventory.isPending||nativeInventory.isFetching||(!nativeInventory.isError&&nativeInventory.hasNextPage));
  useEffect(()=>{
    if(credential===undefined||selectedNativeProvider===undefined||nativeInventory.isPending||nativeInventory.isFetchingNextPage||nativeInventory.isError||!nativeInventory.hasNextPage)return;
    // Native inventory is the only supported source for native account ID and
    // provider resolution. Consume its cursor before declaring absence: a
    // human identity filter must never decide an opaque account target.
    void nativeInventory.fetchNextPage();
  },[credential,nativeInventory.fetchNextPage,nativeInventory.hasNextPage,nativeInventory.isError,nativeInventory.isFetchingNextPage,nativeInventory.isPending,selectedNativeProvider]);
  useEffect(()=>{
    if(credential===undefined||resolvingCredential)return;
    if(selectedNativeProvider!==undefined){
      if(nativeInventory.isError||nativeMatch===undefined)return;
      setMaintenance({kind:"native",account:nativeMatch});
    }else{
      if(ordinaryCredential.isError||ordinaryCredential.data===undefined)return;
      setMaintenance({kind:"ordinary",credentialId:ordinaryCredential.data.id,account:credential.account});
    }
    // The resolved object becomes the operator-owned editor target. Later
    // inventory refreshes must not replace its revision or unmount its form.
    setCredential(undefined);
  },[credential,nativeMatch,nativeInventory.isError,ordinaryCredential.data,ordinaryCredential.isError,resolvingCredential,selectedNativeProvider]);
  const loaded = pools.data?.pages.flatMap((page) => page.items) ?? [];
  const rows = loaded.filter((row) => (!exactAccount||row.account_id===exactAccount) && (!category||row.presentation?.category===category) &&
    [runtimeName(row),runtimeProvider(row),runtimeConnection(row),row.account_id,row.channel_id].join(" ").toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  const findingTarget=!!exactAccount&&!loaded.some(row=>row.account_id===exactAccount)&&pools.hasNextPage&&!pools.isError&&(pools.data?.pages.length??0)<100;
  useEffect(()=>{
    if(findingTarget&&!pools.isFetching)void pools.fetchNextPage();
  },[findingTarget,pools.isFetching,pools.fetchNextPage]);
  // Each runtime row remains one exact binding even when several rows share a human identity.
  const groups = new Map<string, PoolAccount[]>();
  for (const row of rows) {
    const label=runtimeProvider(row);const group=groups.get(label)??[];
    group.push(row);groups.set(label,group);
  }
  const update = (name: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(name, value);
    else next.delete(name);
    setParams(next, { replace: true });
  };
  const open = (row: PoolAccount) => {
    const page=pools.data?.pages.find((candidate)=>candidate.items.includes(row));
    setSelected({account:row,snapshotId:page?.snapshot_id,observedAt:page?.observed_at_ms});
    setTab("runtime");
  };
  const observed = pools.data?.pages[0]?.observed_at_ms;

  return (
    <section className="accounts-page">
      <header className="page-head">
        <div>
          <h2>{t.nav.accounts}</h2>
          <p className="scope-row">
            当前运行快照 · 每条接口连接单独显示状态
          </p>
        </div>
        <button
          className="secondary"
          onClick={() => {
            setSelected(undefined);
            void pools.refetch();
          }}
        >
          刷新账号
        </button>
      </header>
      {navigation}
      {actionError === undefined ? null : (
        <p className="action-error" role="alert">
          {actionError}
          <button
            className="secondary"
            onClick={() => setActionError(undefined)}
          >
            清除
          </button>
        </p>
      )}
      {receipt === undefined ? null : (
        <p className="action-notice" role="status">
          {runtimeName(receipt.target.account)} · {runtimeConnection(receipt.target.account)} · {receipt.target.action === "cool_down" ? "冷却" : "请求恢复"}{receipt.target.upstreamModel?` · ${receipt.target.upstreamModel}`:""} · {receipt.kind==="received"?`${receiptMeta(receipt.value.state).label} · ${receiptMeta(receipt.value.state).detail}`:receipt.message}
          {receipt.kind==="unconfirmed"?<button className="secondary" onClick={()=>void queryClient.resetQueries({queryKey:key,exact:true})}>重新读取运行状态</button>:null}
          <button className="secondary" onClick={() => setReceipt(undefined)}>
            知道了
          </button>
        </p>
      )}
      <div className="stat-row">
        {[
          ["账号绑定", loaded.length],
          ["认证有效", loaded.filter((r) => r.auth_status === "active").length],
          [
            "调度冷却",
            loaded.filter((r) => r.runtime_status === "cooling").length,
          ],
          ["权益已观测", loaded.filter((r) => r.entitlement !== null).length],
        ].map(([label, value]) => (
          <div className="stat-tile" key={label}>
            <span className="stat-label">{label}</span>
            <strong className="stat-value">
              {pools.data === undefined ? "—" : value}
            </strong>
          </div>
        ))}
      </div>
      <div className="data-panel">
        <div className="data-toolbar">
          <input
            aria-label="搜索已加载账号"
            placeholder="搜索邮箱、渠道或接口"
            value={query}
            onChange={(e) => update("q", e.target.value)}
          />
          <select aria-label="账号类别" value={category} onChange={(e)=>update("category",e.target.value)}>
            <option value="">全部渠道</option>{accountGroups.map((group)=><option key={group.id} value={group.id}>{group.name}</option>)}
          </select>
          {provider?<button className="secondary" onClick={()=>update("provider","")}>清除提供商限定</button>:null}
          <select
            aria-label="认证状态"
            value={auth}
            onChange={(e) => update("auth", e.target.value)}
          >
            <option value="">全部认证状态</option>
            {["active", "reauth_required", "disabled", "expired"].map((s) => (
              <option key={s} value={s}>
                {authStatusMeta(s).label}
              </option>
            ))}
          </select>
          <select
            aria-label="调度状态"
            value={runtime}
            onChange={(e) => update("runtime", e.target.value)}
          >
            <option value="">全部调度状态</option>
            {[
              "available",
              "cooling",
              "circuit_open",
              "quota_blocked",
              "unauthorized",
              "recovery_in_flight",
              "expired",
            ].map((s) => (
              <option key={s} value={s}>
                {runtimeStatusMeta(s).label}
              </option>
            ))}
          </select>
        </div>
        <PagedReadStatus query={pools}/>
        {exactAccount?<p className="scope-row">当前定位请求关联的账号与接口。<button className="secondary" onClick={()=>{const next=new URLSearchParams(params);next.delete("account_id");next.delete("channel_id");setParams(next);}}>查看全部绑定</button></p>:null}
        {findingTarget?<p role="status">正在分页定位账号…</p>:null}
        {pools.data === undefined ? null : rows.length === 0 ? (
          <div className="empty-state">
            {findingTarget?"正在定位，请稍候。":exactAccount&&pools.hasNextPage?"尚未在已读页找到此账号，可继续加载更多。":provider || auth || runtime || query || exactAccount
              ? t.state.filteredEmpty
              : t.state.empty}
          </div>
        ) : (
          <>
            <div className="account-desktop">
              {[...groups].map(([providerId, accounts]) => <section className="account-provider" key={providerId}>
                <header className="account-group-head"><span>{providerId}</span><small>{accounts.length} 个绑定</small></header>
                <div className="tablewrap"><table>
                <thead>
                  <tr>
                    <th>账号 / 接口连接</th>
                    <th>认证</th>
                    <th>调度</th>
                    <th>并发</th>
                    <th>权益</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {accounts.map((row) => (
                    <tr
                      key={`${row.provider_id}/${row.channel_id}/${row.account_id}`} data-account-id={row.account_id}
                    >
                      <td>
                        <div className="account-identity"><span className="account-avatar" aria-hidden="true">{runtimeName(row).slice(0, 1).toUpperCase()}</span><div>
                        <div className="entity-name">{runtimeName(row)}</div>
                        <div className="entity-meta">
                          {runtimeConnection(row)}
                        </div></div></div>
                      </td>
                      <td>
                        <StatusBadge status={row.auth_status}>
                          {authStatusMeta(row.auth_status).label}
                        </StatusBadge>
                      </td>
                      <td>
                        <StatusBadge status={row.runtime_status}>
                          {runtimeStatusMeta(row.runtime_status).label}
                        </StatusBadge>
                        {row.enabled ? null : (
                          <div className="entity-meta">已停用</div>
                        )}
                      </td>
                      <td className="mono">
                        {row.active_leases} / {row.max_concurrency}
                      </td>
                      <td>
                        {row.entitlement === null ? (
                          <span className="muted">未观测</span>
                        ) : (
                          <>
                            <div>{row.entitlement.tier}</div>
                            <div className="entity-meta">
                              {row.entitlement.domain}
                            </div>
                          </>
                        )}
                      </td>
                      <td>
                        <button className="secondary" onClick={() => open(row)}>
                          详情
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table></div>
              </section>)}
            </div>
            <div className="account-mobile">
              {[...groups].map(([providerId, accounts]) => <section className="account-provider" key={providerId}>
                <header className="account-group-head"><span>{providerId}</span><small>{accounts.length} 个绑定</small></header>
              {accounts.map((row) => (
                <article
                  key={`${row.provider_id}/${row.channel_id}/${row.account_id}`} data-account-id={row.account_id}
                >
                  <div className="entity-name">{runtimeName(row)}</div>
                  <div className="entity-meta">
                    {runtimeConnection(row)}
                  </div>
                  <p>
                    认证：
                    <StatusBadge status={row.auth_status}>
                      {authStatusMeta(row.auth_status).label}
                    </StatusBadge>
                  </p>
                  <p>
                    调度：
                    <StatusBadge status={row.runtime_status}>
                      {runtimeStatusMeta(row.runtime_status).label}
                    </StatusBadge>
                    {row.enabled?null:<> <StatusBadge status="disabled">已停用</StatusBadge></>}
                  </p>
                  <p className="small">
                    并发 {row.active_leases} / {row.max_concurrency} ·{" "}
                    {row.entitlement === null
                      ? "权益未观测"
                      : `${row.entitlement.domain} / ${row.entitlement.tier}`}
                  </p>
                  <button className="secondary" onClick={() => open(row)}>
                    详情
                  </button>
                </article>
              ))}
              </section>)}
            </div>
          </>
        )}
        <div className="data-footer">
          <span>
            显示 {rows.length} / 已加载 {loaded.length} ·{" "}
            {observed === undefined ? "尚未观测" : formatObservedAt(observed)}
            {pools.hasNextPage ? " · 还有更多账号" : ""}
          </span>
          {pools.hasNextPage ? (
            <button
              className="secondary"
              disabled={pools.isFetchingNextPage || pools.isError}
              onClick={() => void pools.fetchNextPage()}
            >
              加载更多
            </button>
          ) : null}
        </div>
      </div>
      {selected === undefined ? null : (
        <Sheet
          title={runtimeName(selected.account)}
          description="查看这个账号连接的实时认证、调度和权益证据；未观测不等于不可用。"
          layout="inspector"
          onEscape={() => setSelected(undefined)}
          footer={<SheetDismissButton>关闭</SheetDismissButton>}
        >
          <p className="entity-meta">
            {runtimeProvider(selected.account)} · {runtimeConnection(selected.account)} ·{" "}
            {selected.observedAt===undefined?"观测时间未提供":formatObservedAt(selected.observedAt)}
          </p>
          <IdentityDetails entries={[["账号", selected.account.account_id, runtimeName(selected.account)], ["提供商", selected.account.provider_id, runtimeProvider(selected.account)], ["接口", selected.account.channel_id, runtimeConnection(selected.account)]]} />
          <div className="detail-tabs">
            {[
              ["runtime", "运行状态"],
              ["entitlement", "权益证据"],
              ["config", "配置与失败"],
            ].map(([value, label]) => (
              <button
                key={value}
                aria-pressed={tab === value}
                onClick={() => setTab(value!)}
              >
                {label}
              </button>
            ))}
          </div>
          {tab === "runtime" ? (
            <dl className="fact-grid">
              {[
                ["认证", authStatusMeta(selected.account.auth_status).label],
                ["调度", runtimeStatusMeta(selected.account.runtime_status).label],
                ["启用", selected.account.enabled ? "已启用" : "已停用"],
                [
                  "并发 / 上限",
                  `${selected.account.active_leases} / ${selected.account.max_concurrency}`,
                ],
                ["优先级 / 权重", `${selected.account.priority} / ${selected.account.weight}`],
                [
                  "认证到期",
                  selected.account.expires_at_ms === null
                    ? "未观测 / 不适用"
                    : formatObservedAt(selected.account.expires_at_ms),
                ],
                [
                  "续期时间",
                  selected.account.refresh_due_at_ms === null
                    ? "未观测 / 不适用"
                    : formatObservedAt(selected.account.refresh_due_at_ms),
                ],
                [
                  "Quota 同步",
                  selected.account.quota_sync_due_at_ms === null
                    ? "未观测 / 不适用"
                    : formatObservedAt(selected.account.quota_sync_due_at_ms),
                ],
              ].map(([label, value]) => (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
          ) : tab === "entitlement" ? (
            <>
              <EntitlementEvidence
                entitlement={selected.account.entitlement}
                expanded
              />
              <p className="small muted">
                权益不代表模型授权、quota 或调度状态；未观测不等于 Free。
              </p>
            </>
          ) : (
            <>
              <p className="small">
                账号状态来自运行时；凭据配置与诊断使用当前选择的配置版本。
              </p>
              <button
                className="secondary"
                disabled={scope === undefined}
                onClick={() => {
                  setCredential(selected);
                  setSelected(undefined);
                }}
              >
                查看凭据与绑定
              </button>
              {scope === undefined ? (
                <p className="small muted">先选择配置版本以查看配置和诊断。</p>
              ) : null}
              <div className="detail-links">
                <Link
                  to={`/catalog?q=${encodeURIComponent(selected.account.account_id)}`}
                >
                  目录证据
                </Link>
                <Link
                  to={`/monitoring?tab=failures&account_id=${encodeURIComponent(selected.account.account_id)}&provider_id=${encodeURIComponent(selected.account.provider_id)}&channel_id=${encodeURIComponent(selected.account.channel_id)}`}
                >
                  失败记录
                </Link>
                <Link
                  to={`/runtime?account_id=${encodeURIComponent(selected.account.account_id)}`}
                >
                  运行诊断与恢复
                </Link>
              </div>
            </>
          )}
          {tab !== "runtime" ? null : (
            <>
              <div className="sheet-actions">
                {(["cool_down", "request_recovery"] as const).map((action) => (
                  <button
                    key={action}
                    className="secondary"
                    disabled={scope === undefined}
                    onClick={() => {
                      setTarget({...selected,action});
                      setSelected(undefined);
                      setActionError(undefined);
                      setTargetError(undefined);
                      setReceipt(undefined);
                    }}
                  >
                    {action === "cool_down" ? "冷却账号" : "请求恢复"}
                  </button>
                ))}
              </div>
              <p className="small muted">
                {scope === undefined
                  ? "先选择配置版本以操作精确账号。"
                  : "操作针对当前选中的账号连接，恢复结果由运行时判定。"}
              </p>
            </>
          )}
        </Sheet>
      )}
      {target === undefined ? null : (
        <PoolActionSheet
          account={target.account}
          action={target.action}
          pending={act.isPending}
          onCancel={() => {setTarget(undefined);setTargetError(undefined);}}
          onInvalid={setTargetError}
          error={targetError}
          onSubmit={(body) => {setTargetError(undefined);act.mutate({target,body});}}
        />
      )}
      {credential === undefined ? null : (
        resolvingCredential||(selectedNativeProvider!==undefined&&!nativeInventory.isError&&nativeMatch!==undefined)||(selectedNativeProvider===undefined&&!ordinaryCredential.isError&&ordinaryCredential.data!==undefined)?<Sheet title="读取凭据配置" description="正在按选中的账号连接核对配置。" layout="confirm" onEscape={()=>setCredential(undefined)} footer={<SheetDismissButton>取消</SheetDismissButton>}><p>正在读取配置…</p></Sheet>:
        <Sheet title="未建立精确凭据映射" description="此运行时连接没有匹配的当前配置凭据，不能按账号 ID 猜测打开其他授权。" layout="confirm" onEscape={()=>setCredential(undefined)} footer={<><SheetDismissButton className="secondary">关闭</SheetDismissButton>{selectedNativeProvider!==undefined&&nativeInventory.isError?<button onClick={()=>setNativeResolution((value)=>value+1)}>重新读取原生账号</button>:selectedNativeProvider===undefined&&ordinaryCredential.isError?<button onClick={()=>setOrdinaryResolution((value)=>value+1)}>重新读取凭据配置</button>:null}</>}><IdentityDetails entries={[["账号",credential.account.account_id,runtimeName(credential.account)],["提供商",credential.account.provider_id,runtimeProvider(credential.account)],["接口",credential.account.channel_id,runtimeConnection(credential.account)]]}/>{selectedNativeProvider===undefined&&ordinaryCredential.isError?<p role="alert">{asAppError(ordinaryCredential.error).message}</p>:selectedNativeProvider!==undefined&&nativeInventory.isError?<p role="alert">{asAppError(nativeInventory.error).message}</p>:<p className="muted">请在账号管理中核对该连接的授权与接口绑定。</p>}</Sheet>
      )}
      {maintenance?.kind==="native"?<NativeAccountDialog account={maintenance.account} onClose={()=>setMaintenance(undefined)} onAuthorize={()=>{setNativeOauth(maintenance.account);setMaintenance(undefined);}} onChanged={(notice)=>{setMaintenance(undefined);setActionError(notice);void queryClient.resetQueries({queryKey:["accounts"]});}}/>:maintenance?.kind==="ordinary"?<CredentialSheet credentialId={maintenance.credentialId} accountName={runtimeName(maintenance.account)} providerName={runtimeProvider(maintenance.account)} category={maintenance.account.presentation?.category==="codex"?"codex":maintenance.account.presentation?.category==="kimi"?"kimi":undefined} onClose={()=>setMaintenance(undefined)}/>:null}
      {nativeOauth===undefined?null:<GrokDeviceWizard name={accountName(nativeOauth.identity)??"Grok Build 账号"} target={{account_id:nativeOauth.id,revision:nativeOauth.revision}} onClose={()=>{setNativeOauth(undefined);void queryClient.resetQueries({queryKey:["accounts"]});}}/>}
    </section>
  );
}
