import "./access.css";
import { ResourceIdInput } from "../../components/ResourceIdentity";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { resourceName } from "../../utils/resourceNames";
import { routingInventoryKey, useRoutingPages } from "../models/useRoutingPages";
import type { RouteListItem } from "../models/model";
import { ReadStatus } from "../../components/ReadStatus";
// Access control: groups + client keys. Signature safety flow lives here —
// the reveal-once sheet (docs/07 §6.4): the full rgw_ key exists only in the
// 201 issue response; closing the sheet erases it from memory permanently.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Fragment, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { ObjectInspector } from "../../components/ObjectInspector";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
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
    mutationFn: (input: { route_id: string; enabled: boolean }) =>
      call<AccessGroupRoute>(
        "grantAccessGroupRoute",
        { path: { access_group_id: groupId }, body: input },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setAdding(false);
      void queryClient.resetQueries({ queryKey: ["routing-inventory", scope] });
      void queryClient.invalidateQueries({ queryKey: ["group-routes", scope, groupId] });
    },
    onError: (error) => onError(asAppError(error).message),
  });

  function onGrantSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
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
          onClick={() => setAdding(true)}
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

      {adding ? (
        <Sheet title={`授权路由 · ${resourceName(groupId,"group")}`} onEscape={() => setAdding(false)} busy={grant.isPending}>
          <form className="sheet-form" onSubmit={onGrantSubmit}>
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
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setAdding(false)}>
                取消
              </button>
              <button type="submit" disabled={grant.isPending}>
                授权
              </button>
            </div>
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
  const [groupError, setGroupError] = useState<string>();
  const [confirmDeleteGroup, setConfirmDeleteGroup] = useState<string | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();
  const [inspectedGroup, setInspectedGroup] = useState<AccessGroupRecord>();
  const [inspectedKey, setInspectedKey] = useState<ClientKeyRecord>();

  const scope = context?.configVersionId;

  const groups = useQuery({
    queryKey: ["access-groups", scope, context?.revision],
    queryFn: () => call<AccessGroupRecord[]>("listAccessGroups", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const keys = useQuery({
    queryKey: ["client-keys", scope, context?.revision],
    queryFn: () => call<ClientKeyRecord[]>("listClientKeys", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const revoke=useMutation({mutationFn:async(target:NonNullable<typeof confirmRevoke>)=>runModelTask("吊销客户端密钥",async task=>{
    const keys=await task.read<ClientKeyRecord[]>("listClientKeys");
    const current=keys.find(key=>key.id===target.record.id);
    if(!current||current.access_group_id!==target.record.access_group_id||current.status!==target.record.status||(current.expires_at_ms??null)!==(target.record.expires_at_ms??null))throw new Error("密钥已变化，请重新打开后核对。");
    await task.mutate("revokeClientKey",{path:{client_key_id:target.record.id}});
  },{expectedSource:target.source,probeUnchanged:true}),onSuccess:({receipt})=>setRevokeReceipt(receipt),onError:(error)=>setActionError(asAppError(error).message)});

  // PATCH takes the whole AccessGroupInput, not a partial — editing is a
  // full replacement, so the form is seeded with the current record.
  const saveGroup = useMutation({
    mutationFn: (input: {
      existing: boolean;
      id: string;
      name: string;
      status: "active" | "disabled";
      limits: Readonly<Record<string, number>>;
    }) => {
      const body = {
        id: input.id,
        name: input.name,
        status: input.status,
        limits: input.limits,
      };
      return input.existing
        ? call<AccessGroupRecord>(
            "updateAccessGroup",
            { path: { access_group_id: input.id }, body },
            { versionScoped: true, mutating: true },
          )
        : call<AccessGroupRecord>(
            "createAccessGroup",
            { body },
            { versionScoped: true, mutating: true },
          );
    },
    onSuccess: () => {
      setGroupForm(undefined);
      void queryClient.invalidateQueries({ queryKey: ["access-groups", scope] });
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const deleteGroup = useMutation({
    mutationFn: (id: string) =>
      call<undefined>(
        "deleteAccessGroup",
        { path: { access_group_id: id } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setConfirmDeleteGroup(undefined);
      void queryClient.invalidateQueries({ queryKey: ["access-groups", scope] });
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  function onGroupSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const limits = data.get("clear_limits") === "on" ? {} : (groupForm?.limits ?? {});
    const status = data.get("status") === "disabled" ? "disabled" : "active";
    if (status === "active" && Object.keys(limits).length > 0) {
      setGroupError("当前网关不支持访问组限额。请明确清除历史限制，或将访问组停用后保存。");
      return;
    }
    setGroupError(undefined);
    setActionError(undefined);
    saveGroup.mutate({
      existing: groupForm !== null && groupForm !== undefined,
      id: String(data.get("id") ?? "").trim(),
      name: String(data.get("name") ?? "").trim(),
      status,
      limits,
    });
  }

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
        <h2>{t.nav.access}</h2>
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

      <details className="card"><summary>高级访问组</summary><div className="page-actions">          <button
            type="button"
            className="secondary"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => { setGroupError(undefined); saveGroup.reset(); setGroupForm(null); }}
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
                      onClick={() => { setGroupError(undefined); saveGroup.reset(); setGroupForm(group); }}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={!editable}
                      title={editable ? undefined : t.version.readOnly}
                      onClick={() => setConfirmDeleteGroup(group.id)}
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
      <div className="card tablewrap key-list-wrap">
        <h3>客户端密钥</h3>
        <table className="key-list">
          <thead>
            <tr>
              <th>名称</th>
              <th>密钥标识</th>
              <th>状态</th>
              <th>过期</th>
              <th>最近请求</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(keys.data ?? []).map((record) => {
              const status = displayKeyStatus(record, nowMs);
              return (
                <tr key={record.id}>
                  <td>{record.access_group_id ? <ResourceIdentity id={record.access_group_id} kind="group" name={groups.data?.find((group)=>group.id===record.access_group_id)?.name}/> : "未命名密钥"}</td>
                  <td data-label="密钥标识" className="mono">{record.prefix}</td>
                  <td data-label="状态">
                    <StatusBadge status={status}>{({active:"已启用",disabled:"已停用",revoked:"已吊销",expired:"已过期"})[status]}</StatusBadge>
                  </td>
                  <td data-label="有效期" className="mono">{formatExpiry(record.expires_at_ms)}</td>
                  <td data-label="最近请求"><a href={`#/monitoring?tab=requests&client_key_id=${encodeURIComponent(record.id)}&from_ms=${Math.max(0,(record.last_request_at_ms??Date.now())-3600000)}&to_ms=${Date.now()}`} title="已持久化终态请求的开始时间；未观测不代表从未使用">{record.last_request_at_ms == null ? "未观测" : new Date(record.last_request_at_ms).toLocaleString()}</a></td>
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

      {inspectedGroup === undefined ? null : <ObjectInspector title={resourceName(inspectedGroup.id, "group", inspectedGroup.name)} scope={`配置版本 ${resourceName(scope ?? "—", "config")} · 访问组`} onClose={() => setInspectedGroup(undefined)} facts={[
        ["访问组 ID", inspectedGroup.id], ["状态", inspectedGroup.status], ["限制", formatLimits(inspectedGroup.limits) || "未设置"],
      ]}><div className="sheet-actions"><button className="secondary" onClick={() => { setExpanded(inspectedGroup.id); setInspectedGroup(undefined); }}>查看授权路由</button>
        <button disabled={!editable} onClick={() => { setGroupError(undefined); saveGroup.reset(); setGroupForm(inspectedGroup); setInspectedGroup(undefined); }}>编辑访问组</button></div></ObjectInspector>}

      {inspectedKey === undefined ? null : <ObjectInspector title="Client Key" scope={`配置版本 ${resourceName(scope ?? "—", "config")} · 只显示公开元数据`} onClose={() => setInspectedKey(undefined)} facts={[
        ["Key ID", inspectedKey.id], ["前缀", inspectedKey.prefix], ["访问组", inspectedKey.access_group_id],
        ["配置状态", inspectedKey.status], ["当前显示状态", displayKeyStatus(inspectedKey, nowMs)], ["到期时间", formatExpiry(inspectedKey.expires_at_ms)],
      ]}><p className="small muted">完整密钥仅在签发时显示一次，详情不会重新显示。</p>
        <div className="sheet-actions"><button onClick={() => { setEditKey(inspectedKey); setInspectedKey(undefined); }}>编辑 Client Key</button></div></ObjectInspector>}

      {groupForm !== undefined ? (
        <Sheet
          title={groupForm === null ? "新建访问组" : `编辑访问组 · ${resourceName(groupForm.id,"group",groupForm.name)}`}
          onEscape={() => setGroupForm(undefined)}
          busy={saveGroup.isPending}
        >
          <form className="sheet-form" onSubmit={onGroupSubmit}>
            <label>
              {groupForm === null ? "访问组标识" : "访问组"}
              <ResourceIdInput kind="group"
                name="id"
                className="mono"
                required
                maxLength={128}
                readOnly={groupForm !== null}
                defaultValue={groupForm?.id ?? ""}
              />
            </label>
            <label>
              名称
              <input name="name" required maxLength={128} defaultValue={groupForm ? resourceName(groupForm.id,"group",groupForm.name) : ""} />
            </label>
            <label>
              状态
              <select name="status" defaultValue={groupForm?.status ?? "active"}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
            {groupForm && Object.keys(groupForm.limits).length > 0 ? (
              <div>
                <p>历史限制 <code>{formatLimits(groupForm.limits)}</code></p>
                <p className="stat-sub">当前网关不支持执行这些限制。保留限制时仅可保存为停用状态。</p>
                <label className="check-row">
                  <input type="checkbox" name="clear_limits" />
                  清除历史限制
                </label>
              </div>
            ) : <p className="stat-sub">当前网关不支持访问组限额，此访问组不设置限额。</p>}
            {groupError ? <p role="alert">{groupError}</p> : null}
            {saveGroup.isError ? <p role="alert">{asAppError(saveGroup.error).message}</p> : null}
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setGroupForm(undefined)}>
                取消
              </button>
              <button type="submit" disabled={saveGroup.isPending}>
                {groupForm === null ? "创建" : "保存"}
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDeleteGroup !== undefined ? (
        <Sheet title="确认删除访问组" onEscape={() => setConfirmDeleteGroup(undefined)} busy={deleteGroup.isPending}>
          <p>
            删除 <span className="mono">{resourceName(confirmDeleteGroup,"group")}</span> 会同时移除它的路由授权。
            指向该组的 Client Key 会失去访问组 —— 请先确认没有在用的 Key 挂在它下面。
          </p>
          <div className="sheet-actions">
            <button
              type="button"
              className="secondary"
              onClick={() => setConfirmDeleteGroup(undefined)}
            >
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={deleteGroup.isPending}
              onClick={() => deleteGroup.mutate(confirmDeleteGroup)}
            >
              确认删除
            </button>
          </div>
        </Sheet>
      ) : null}

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
