import "./access.css";
import { readKeyPermissionSummaries } from "./keyPermissionSummary";
import { GroupMaintenanceDialog } from "./GroupMaintenanceDialog";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { resourceName } from "../../utils/resourceNames";
import { routingInventoryKey, useRoutingPages } from "../models/useRoutingPages";
import type { RouteListItem } from "../models/model";
import { ReadStatus } from "../../components/ReadStatus";
// Access control: groups + client keys. Signature safety flow lives here —
// the reveal-once sheet (docs/07 §6.4): the full rgw_ key exists only in the
// 201 issue response; closing the sheet erases it from memory permanently.
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Fragment, useState, useRef, useEffect, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { ObjectInspector } from "../../components/ObjectInspector";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { runModelTask, type ModelTaskReceipt } from "../models/modelTask";
import { KeyPermissionsDialog } from "./KeyPermissionsDialog";
import { IssueKeyDialog } from "./IssueKeyDialog";
import { GroupKeyDialog } from "./GroupKeyDialog";
import {
  displayKeyStatus,
  formatExpiry,
  formatLimits,
  type AccessGroupRecord,
  type ClientKeyRecord,
} from "./model";

type AccessGroupRoute = Readonly<{
  access_group_id: string;
  route_id: string;
  enabled: boolean;
}>;
type RevokeTarget=Readonly<{record:ClientKeyRecord;source:Readonly<{id:string;revision:string}>}>;

function GroupRoutes({
  groupId,
  editable,
  onError,
}: Readonly<{ groupId: string; editable: boolean; onError: (message: string) => void }>) {
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;
  const [adding, setAdding] = useState(false);
  const [source,setSource]=useState<{id:string;revision:string}>();
  const [receipt,setReceipt]=useState<ConfigVersionSummary>();
  const [workingId,setWorkingId]=useState<string>();
  const submitted=useRef(false),live=useRef(true);
  const [owner]=useState(()=>({session:useSessionStore.getState().generation,selection:useVersionStore.getState().selectionGeneration}));
  useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
  const owned=()=>live.current&&owner.session===useSessionStore.getState().generation&&owner.selection===useVersionStore.getState().selectionGeneration;


  const grants = useQuery({
    queryKey: ["group-routes", scope, context?.revision, groupId],
    queryFn: () =>
      call<AccessGroupRoute[]>(
        "listAccessGroupRoutes",
        { path: { access_group_id: groupId } },
        { versionScoped: true },
      ),
    enabled: scope !== undefined,
  });

  const suggestions = useRoutingPages<RouteListItem>("listRoutes", adding);
  const routeIds = suggestions.data?.pages.flatMap((page) => page.items.map((row) => row.id)) ?? [];

  const grant = useMutation({
    mutationFn:async(input:{route_id:string;enabled:boolean})=>{
      if(!source)throw new Error("请重新打开授权操作。");
      const task=await beginConfigurationTask("访问组路由授权",source,"deferred");
      if(!owned())return;setWorkingId(task.version.id);useVersionStore.getState().rememberPending(task.version);
      await task.mutate("grantAccessGroupRoute",{path:{access_group_id:groupId},body:input});
      return task.finish();
    },
    onSuccess:version=>{if(version&&owned()){useVersionStore.getState().rememberPending(version);setReceipt(version);}},
    onError:error=>{if(owned())onError(asAppError(error).message);},
  });

  const finishGrant=()=>{setAdding(false);if(receipt&&owned()){useVersionStore.getState().advanceFromEtag(receipt.revision);void queryClient.invalidateQueries({queryKey:["group-routes",scope]});}};

  function onGrantSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if(submitted.current)return;submitted.current=true;
    const data = new FormData(event.currentTarget);
    grant.mutate({
      route_id: String(data.get("route_id") ?? "").trim(),
      enabled: data.get("enabled") === "on",
    });
  }

  return (
    <div className="group-routes">
      <h4>
        路由授权 <span className="idchip mono">{grants.data?.length ?? "…"}</span>
        <button
          type="button"
          className="secondary"
          disabled={!editable}
          title={editable ? undefined : "仅草稿版本可编辑"}
          onClick={() => {submitted.current=false;setReceipt(undefined);setWorkingId(undefined);grant.reset();setSource(context?{id:context.configVersionId,revision:context.revision}:undefined);setAdding(true);}}
        >
          授权路由
        </button>
      </h4>
      {grants.data !== undefined && grants.data.length === 0 ? (
        <p className="stat-sub">
          该组没有任何路由授权 —— 组内的 Client Key 现在到不了任何模型。
        </p>
      ) : (
        <table className="responsive-table">
          <thead>
            <tr>
              <th>路由</th>
              <th>状态</th>
            </tr>
          </thead>
          <tbody>
            {(grants.data ?? []).map((row) => (
              <tr key={row.route_id}>
                <td data-label="路由"><ResourceIdentity id={row.route_id} kind="route" /></td>
                <td data-label="状态">
                  <StatusBadge status={row.enabled ? "active" : "disabled"}>
                    {row.enabled ? "enabled" : "disabled"}
                  </StatusBadge>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {adding&&receipt?<Sheet title="路由授权结果" guardUnsaved={false} onEscape={finishGrant} footer={<SheetDismissButton onDismiss={finishGrant}>完成</SheetDismissButton>}><p role="status">授权已保存到草稿，尚未应用。</p></Sheet>:adding ? (
        <Sheet title={`授权路由 · ${resourceName(groupId,"group")}`} onEscape={() => setAdding(false)} busy={grant.isPending} footer={<><SheetDismissButton className="secondary" disabled={grant.isPending}>取消</SheetDismissButton><button type="submit" form="group-route-form" disabled={submitted.current||suggestions.isPending||suggestions.isError}>授权</button></>}>
          <ConfigurationTaskNotice workingId={workingId} error={grant.error} onReview={version=>{if(owned()){useVersionStore.getState().rememberPending(version);useVersionStore.getState().select(version);}}}/>
          <form id="group-route-form" className="sheet-form" onSubmit={onGrantSubmit}><fieldset disabled={submitted.current}>
            <label>
              路由
              <select name="route_id" required defaultValue="">
                <option value="" disabled>选择路由</option>
                {routeIds.map((id) => <option key={id} value={id}>{resourceName(id,"route")}</option>)}
              </select>
            </label>
            <p className="stat-sub">
              包含未绑定草稿路由 · 已载入 {routeIds.length} 项{suggestions.hasNextPage ? " · 还有更多" : ""}
            </p>
            {suggestions.isError ? <p role="alert">{asAppError(suggestions.error).message}。请重新读取路由列表。</p> : null}
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => void queryClient.resetQueries({ queryKey: routingInventoryKey(scope,context?.revision,"listRoutes"), exact:true })}>重新读取路由</button>
              {suggestions.hasNextPage ? <button type="button" className="secondary" disabled={suggestions.isFetching || suggestions.isError}
                onClick={() => void suggestions.fetchNextPage()}>加载更多路由</button> : null}
            </div>
            <label className="check-row">
              <input name="enabled" type="checkbox" defaultChecked />
              启用
            </label>
            </fieldset>
          </form>
        </Sheet>
      ) : null}
    </div>
  );
}

export function AccessPage() {
  const [creating,setCreating]=useState(false);
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const [issuing, setIssuing] = useState(false);
  const [editKey, setEditKey] = useState<ClientKeyRecord | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();
  const [confirmRevoke, setConfirmRevoke] = useState<RevokeTarget>();
  const [revokeReceipt,setRevokeReceipt]=useState<ModelTaskReceipt>();
  // undefined = closed; null = creating; record = editing that group
  const [groupForm, setGroupForm] = useState<AccessGroupRecord | null | undefined>();
  const [confirmDeleteGroup, setConfirmDeleteGroup] = useState<AccessGroupRecord | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();
  const [inspectedGroup, setInspectedGroup] = useState<AccessGroupRecord>();
  const [inspectedKey, setInspectedKey] = useState<ClientKeyRecord>();

  const scope = context?.configVersionId;

  const groups = useQuery({
    queryKey: ["access-groups", scope, context?.revision],
    placeholderData: keepPreviousData,
    queryFn: () => call<AccessGroupRecord[]>("listAccessGroups", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const keys = useQuery({
    queryKey: ["client-keys", scope, context?.revision],
    queryFn: () => call<ClientKeyRecord[]>("listClientKeys", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const permissionGroups=[...new Set(keys.data?.map(key=>key.access_group_id)??[])].sort();
  const permissions=useQuery({queryKey:["key-permission-summaries",scope,context?.revision,permissionGroups],enabled:!!context&&permissionGroups.length>0,retry:false,
    queryFn:()=>readKeyPermissionSummaries({id:context!.configVersionId,revision:context!.revision},permissionGroups)});

  const revoke=useMutation({mutationFn:async(target:NonNullable<typeof confirmRevoke>)=>runModelTask("吊销客户端密钥",async task=>{
    const keys=await task.read<ClientKeyRecord[]>("listClientKeys");
    const current=keys.find(key=>key.id===target.record.id);
    if(!current||current.access_group_id!==target.record.access_group_id||current.status!==target.record.status||(current.expires_at_ms??null)!==(target.record.expires_at_ms??null))throw new Error("密钥已变化，请重新打开后核对。");
    await task.mutate("revokeClientKey",{path:{client_key_id:target.record.id}});
  },{expectedSource:target.source,probeUnchanged:true}),onSuccess:({receipt})=>setRevokeReceipt(receipt),onError:(error)=>setActionError(asAppError(error).message)});

  function finishRevoke(){
    const receipt=revokeReceipt;
    setConfirmRevoke(undefined);
    setRevokeReceipt(undefined);
    if(receipt&&receipt.kind!=="unchanged"){
      void queryClient.invalidateQueries({queryKey:["client-keys"]});
      useVersionStore.getState().select(receipt.workingVersion);
    }
  }

  const nowMs = Date.now();

  return (
    <section className="access-page">
      <header className="page-head">
        <div><h2>{t.nav.access}</h2><p className="page-description">每把密钥都有明确的模型边界。创建后秘密只显示一次。</p></div>
        <div className="page-actions">
          <button onClick={()=>setCreating(true)}>创建客户端密钥</button>
        </div>
      </header>
      {creating?<IssueKeyDialog onClose={()=>setCreating(false)} onSaved={(version)=>{setCreating(false);useVersionStore.getState().select(version);}}/>:null}

      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <ReadStatus pending={!!scope&&groups.isPending} error={groups.error} hasData={groups.data !== undefined} retry={() => void groups.refetch()} />
      <ReadStatus pending={!!scope&&keys.isPending} error={keys.error} hasData={keys.data !== undefined} retry={() => void keys.refetch()} />

      <div className="card tablewrap key-list-wrap">
        <table className="key-list" aria-label="客户端密钥">
          <thead>
            <tr>
              <th>名称</th>
              <th>秘密不可回读</th>
              <th>模型权限</th>
              <th>状态</th>
              <th>使用记录</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(keys.data ?? []).map((record) => {
              const status = displayKeyStatus(record, nowMs);
              const allowed=permissions.data?.[record.access_group_id];
              return (
                <tr key={record.id}>
                  <td>{record.access_group_id ? <ResourceIdentity id={record.access_group_id} kind="group" name={groups.data?.find((group)=>group.id===record.access_group_id)?.name}/> : "未命名密钥"}</td>
                  <td data-label="密钥标识"><span className="mono">{record.prefix}••••</span><span className="entity-meta">仅创建时显示完整密钥</span></td>
                  <td data-label="模型权限"><div className="key-permission-summary">{permissions.isError?<button className="secondary" onClick={()=>void permissions.refetch()}>权限未确认 · 重读</button>:!allowed?<span className="muted">正在读取…</span>:<><span className="mono">{allowed.slice(0,2).map(model=>model.name+(model.enabled?"":"（已关闭）")).join("、")||"未授予模型权限"}</span><span className="entity-meta">明确允许 {allowed.length} 个模型</span></>}</div></td>
                  <td data-label="状态">
                    <StatusBadge status={status}>{({active:"已启用",disabled:"已停用",revoked:"已吊销",expired:"已过期"})[status]}</StatusBadge>
                  </td>
                  <td data-label="使用记录"><span className="entity-meta">有效期 · {formatExpiry(record.expires_at_ms)}</span><a href={`#/monitoring?tab=requests&client_key_id=${encodeURIComponent(record.id)}&from_ms=${Math.max(0,(record.last_request_at_ms??Date.now())-3600000)}&to_ms=${Date.now()}`} title="查看该密钥的请求记录；未观测不代表从未使用">{record.last_request_at_ms == null ? "查看请求记录" : new Date(record.last_request_at_ms).toLocaleString()}</a></td>
                  <td className="row-actions">
                    <button className="secondary" onClick={() => setInspectedKey(record)}>详情</button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => {
                        setActionError(undefined);
                        setEditKey(record);
                      }}
                    >
                      编辑
                    </button>
                    {record.status === "active" ? (
                      <button
                        type="button"
                        className="danger"
                        onClick={() => {if(!context)return;revoke.reset();setRevokeReceipt(undefined);setActionError(undefined);setConfirmRevoke({record,source:{id:context.configVersionId,revision:context.revision}});}}
                      >
                        吊销
                      </button>
                    ) : null}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {keys.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      <p className="inventory-note"><span>{keys.data?.length ?? "—"} 把密钥</span><span>模型开放不会自动扩大现有密钥的权限。</span></p>
      <details className="card"><summary>高级访问组</summary><div className="page-actions">          <button
            type="button"
            className="secondary"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => { setGroupForm(null); }}
          >
            新建访问组
          </button>
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => setIssuing(true)}
          >
            按访问组签发
          </button>
</div><div className="tablewrap">
        <table className="responsive-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>状态</th>
              <th>限制</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(groups.data ?? []).map((group) => (
              <Fragment key={group.id}>
                <tr>
                  <td data-label="名称"><ResourceIdentity id={group.id} name={group.name} kind="group" /></td>
                  <td data-label="状态">
                    <StatusBadge status={group.status} />
                  </td>
                  <td data-label="限制" className="mono">{formatLimits(group.limits) || "—"}</td>
                  <td data-label="操作" className="row-actions">
                    <button className="secondary" onClick={() => setInspectedGroup(group)}>详情</button>
                    <button
                      type="button"
                      className="secondary"
                      aria-expanded={expanded === group.id}
                      onClick={() => setExpanded(expanded === group.id ? undefined : group.id)}
                    >
                      {expanded === group.id ? "收起路由" : "路由"}
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={!editable}
                      title={editable ? undefined : t.version.readOnly}
                      onClick={() => { setGroupForm(group); }}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={!editable}
                      title={editable ? undefined : t.version.readOnly}
                      onClick={() => setConfirmDeleteGroup(group)}
                    >
                      删除
                    </button>
                  </td>
                </tr>
                {expanded === group.id ? (
                  <tr>
                    <td colSpan={4}>
                      <GroupRoutes
                        groupId={group.id}
                        editable={editable}
                        onError={setActionError}
                      />
                    </td>
                  </tr>
                ) : null}
              </Fragment>
            ))}
          </tbody>
        </table>
        {groups.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      </details>

      {inspectedGroup === undefined ? null : <ObjectInspector title={resourceName(inspectedGroup.id, "group", inspectedGroup.name)} scope={`配置版本 ${resourceName(scope ?? "—", "config")} · 访问组`} onClose={() => setInspectedGroup(undefined)} facts={[
        ["访问组 ID", inspectedGroup.id], ["状态", inspectedGroup.status], ["限制", formatLimits(inspectedGroup.limits) || "未设置"],
      ]}><div className="sheet-actions"><button className="secondary" onClick={() => { setExpanded(inspectedGroup.id); setInspectedGroup(undefined); }}>查看授权路由</button>
        <button disabled={!editable} onClick={() => { setGroupForm(inspectedGroup); setInspectedGroup(undefined); }}>编辑访问组</button></div></ObjectInspector>}

      {inspectedKey === undefined ? null : <Sheet title="密钥详情" layout="inspector" onEscape={()=>setInspectedKey(undefined)} footer={<><SheetDismissButton className="secondary">关闭</SheetDismissButton><button onClick={() => { setEditKey(inspectedKey); setInspectedKey(undefined); }}>编辑密钥与权限</button></>}>
        <header className="key-inspector-heading"><h3>{resourceName(inspectedKey.access_group_id,"group",groups.data?.find(group=>group.id===inspectedKey.access_group_id)?.name)}</h3><StatusBadge status={displayKeyStatus(inspectedKey,nowMs)}>{({active:"已启用",disabled:"已停用",revoked:"已吊销",expired:"已过期"})[displayKeyStatus(inspectedKey,nowMs)]}</StatusBadge><code>{inspectedKey.prefix}••••</code></header>
        <section className="key-inspector-section"><h4>使用与有效期</h4><dl className="key-inspector-facts"><dt>有效期</dt><dd>{formatExpiry(inspectedKey.expires_at_ms)}</dd><dt>最近请求</dt><dd>{inspectedKey.last_request_at_ms == null ? "未观测" : new Date(inspectedKey.last_request_at_ms).toLocaleString()}</dd><dt>完整密钥</dt><dd>仅签发时显示，不可回读</dd></dl></section>
        <section className="key-inspector-section"><h4>明确允许的模型</h4>{permissions.isError?<button className="secondary" onClick={()=>void permissions.refetch()}>权限未确认 · 重新读取</button>:!permissions.data?.[inspectedKey.access_group_id]?<p role="status">正在读取权限…</p>:permissions.data[inspectedKey.access_group_id]!.length===0?<p className="muted">未授予模型权限</p>:<ul className="key-inspector-models">{permissions.data[inspectedKey.access_group_id]!.map(model=><li key={model.id}><code>{model.name}</code>{!model.enabled?<span className="muted">模型已关闭</span>:null}</li>)}</ul>}<p className="small muted">模型开放不会自动扩大此密钥的权限。</p></section>
      </Sheet>}


      {groupForm!==undefined?<GroupMaintenanceDialog record={groupForm} onClose={()=>{setGroupForm(undefined);void queryClient.invalidateQueries({queryKey:["access-groups"]});}} onSelected={version=>{setGroupForm(undefined);useVersionStore.getState().select(version);void queryClient.resetQueries({queryKey:["access-groups"]});void queryClient.resetQueries({queryKey:["client-keys"]});}}/>:null}
      {confirmDeleteGroup?<GroupMaintenanceDialog record={confirmDeleteGroup} removing onClose={()=>{setConfirmDeleteGroup(undefined);void queryClient.invalidateQueries({queryKey:["access-groups"]});}} onSelected={version=>{setConfirmDeleteGroup(undefined);useVersionStore.getState().select(version);void queryClient.resetQueries({queryKey:["access-groups"]});}}/>:null}

      {issuing?<GroupKeyDialog groups={groups.data??[]} onClose={()=>setIssuing(false)} onSettled={()=>{setIssuing(false);void queryClient.invalidateQueries({queryKey:["client-keys",scope]});}}/>:null}

      {editKey ? <KeyPermissionsDialog record={editKey} onClose={()=>setEditKey(undefined)} onSaved={(version)=>{setEditKey(undefined);useVersionStore.getState().select(version);}}/> : null}

      {confirmRevoke !== undefined ? (
        <Sheet title={revokeReceipt?"密钥吊销结果":"吊销 API 密钥"} description={revokeReceipt?"请核对保存与应用状态。":"吊销后该密钥不能再发起请求；历史记录仍会保留。"} layout="confirm" tone={revokeReceipt?"default":"danger"} onEscape={() => !revoke.isPending&&(revokeReceipt?finishRevoke():setConfirmRevoke(undefined))} busy={revoke.isPending} footer={revokeReceipt?<SheetDismissButton onDismiss={finishRevoke}>{revokeReceipt.kind==="saved_applied"?"完成":"核对配置"}</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={revoke.isPending}>取消</SheetDismissButton><button type="button" className="danger" disabled={revoke.isPending} onClick={() => revoke.mutate(confirmRevoke)}>确认吊销</button></>}>
          {revokeReceipt?<div role="status"><p>{revokeReceipt.message}</p><p>已确认保存 {revokeReceipt.acknowledgedWrites} 步。</p></div>:<p>吊销 {confirmRevoke.record.prefix}。应用后，该密钥不能再发起请求，历史记录保留。</p>}
          {revoke.isError?<p role="alert">{asAppError(revoke.error).message}</p>:null}
        </Sheet>
      ) : null}
    </section>
  );
}
