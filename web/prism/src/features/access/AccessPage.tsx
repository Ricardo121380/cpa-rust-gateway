import { ResourceIdInput } from "../../components/ResourceIdentity";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { resourceName, resourceOption } from "../../utils/resourceNames";
import { useRoutingPages } from "../models/useRoutingPages";
import type { RouteListItem } from "../models/model";
import { ReadStatus } from "../../components/ReadStatus";
// Access control: groups + client keys. Signature safety flow lives here —
// the reveal-once sheet (docs/07 §6.4): the full rgw_ key exists only in the
// 201 issue response; closing the sheet erases it from memory permanently.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Fragment, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { ObjectInspector } from "../../components/ObjectInspector";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { IssueKeyDialog } from "./IssueKeyDialog";
import {
  displayKeyStatus,
  formatExpiry,
  formatLimits,
  parseLimits,
  type AccessGroupRecord,
  isReactivation,
  toLocalInput,
  editedExpiry,
  type ClientKeyRecord,
  type IssuedClientKey,
} from "./model";

type AccessGroupRoute = Readonly<{
  access_group_id: string;
  route_id: string;
  enabled: boolean;
}>;

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
    queryKey: ["group-routes", scope, groupId],
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
        <table>
          <thead>
            <tr>
              <th>路由</th>
              <th>状态</th>
            </tr>
          </thead>
          <tbody>
            {(grants.data ?? []).map((row) => (
              <tr key={row.route_id}>
                <td><ResourceIdentity id={row.route_id} kind="route" /></td>
                <td>
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
        <Sheet title={`授权路由 · ${resourceName(groupId,"group")}`} onEscape={() => setAdding(false)}>
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
              <button type="button" className="secondary" onClick={() => void queryClient.resetQueries({ queryKey: ["routing-inventory", scope, "listRoutes"] })}>重新读取路由</button>
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
  const [workingId,setWorkingId]=useState<string>();
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const [issuing, setIssuing] = useState(false);
  const [issued, setIssued] = useState<IssuedClientKey | undefined>();
  const [editKey, setEditKey] = useState<ClientKeyRecord | undefined>();
  // Live status inside the edit sheet, so the reactivation warning can appear
  // the moment the operator selects it rather than after they submit.
  const [editStatus, setEditStatus] = useState<string>("active");
  const [copied, setCopied] = useState(false);
  const [actionError, setActionError] = useState<string | undefined>();
  const [confirmRevoke, setConfirmRevoke] = useState<string | undefined>();
  // undefined = closed; null = creating; record = editing that group
  const [groupForm, setGroupForm] = useState<AccessGroupRecord | null | undefined>();
  const [confirmDeleteGroup, setConfirmDeleteGroup] = useState<string | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();
  const [inspectedGroup, setInspectedGroup] = useState<AccessGroupRecord>();
  const [inspectedKey, setInspectedKey] = useState<ClientKeyRecord>();

  const scope = context?.configVersionId;

  const groups = useQuery({
    queryKey: ["access-groups", scope],
    queryFn: () => call<AccessGroupRecord[]>("listAccessGroups", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const keys = useQuery({
    queryKey: ["client-keys", scope],
    queryFn: () => call<ClientKeyRecord[]>("listClientKeys", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const issue = useMutation({
    gcTime: 0, // never cache a response carrying the one-time key
    mutationFn: (input: {
      id: string;
      access_group_id: string;
      status: "active";
      expires_at_ms: number | null;
    }) =>
      call<IssuedClientKey>(
        "issueClientKey",
        { body: input },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: (record) => {
      setIssuing(false);
      setIssued(record);
      setCopied(false);
      void queryClient.invalidateQueries({ queryKey: ["client-keys", scope] });
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const updateKey=useMutation({mutationFn:async(input:Omit<ClientKeyRecord,"prefix">)=>{
    const task=await beginConfigurationTask("更新客户端密钥");setWorkingId(task.version.id);
    const current=(await task.read<ClientKeyRecord[]>("listClientKeys")).find((row)=>row.id===input.id);
    if(!editKey||!current||current.status!==editKey.status||current.access_group_id!==editKey.access_group_id||(current.expires_at_ms??null)!==(editKey.expires_at_ms??null))throw new Error("密钥已被修改，请重新读取后操作。");
    await task.mutate("updateClientKey",{path:{client_key_id:input.id},body:{...input,expires_at_ms:input.expires_at_ms??null}});
    return task.finish();
  },onSuccess:(version)=>{setEditKey(undefined);void queryClient.invalidateQueries({queryKey:["client-keys"]});useVersionStore.getState().select(version);},onError:(error)=>setActionError(asAppError(error).message)});
  const revoke=useMutation({mutationFn:async(id:string)=>{
    const task=await beginConfigurationTask("吊销客户端密钥");setWorkingId(task.version.id);
    await task.mutate("revokeClientKey",{path:{client_key_id:id}});return task.finish();
  },onSuccess:(version)=>{setConfirmRevoke(undefined);void queryClient.invalidateQueries({queryKey:["client-keys"]});useVersionStore.getState().select(version);},onError:(error)=>setActionError(asAppError(error).message)});

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
    const parsed = parseLimits(String(data.get("limits") ?? ""));
    if (!parsed.ok) {
      setActionError(parsed.reason);
      return;
    }
    setActionError(undefined);
    saveGroup.mutate({
      existing: groupForm !== null && groupForm !== undefined,
      id: String(data.get("id") ?? "").trim(),
      name: String(data.get("name") ?? "").trim(),
      status: data.get("status") === "disabled" ? "disabled" : "active",
      limits: parsed.limits,
    });
  }

  function onIssueSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const expiresRaw = String(data.get("expires_at") ?? "");
    issue.mutate({
      id: String(data.get("id") ?? ""),
      access_group_id: String(data.get("access_group_id") ?? ""),
      status: "active",
      expires_at_ms: expiresRaw === "" ? null : new Date(expiresRaw).getTime(),
    });
  }

  function closeReveal() {
    issue.reset();
    setIssued(undefined); // the key is gone for good — by design
    setCopied(false);
  }

  const nowMs = Date.now();

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.access}</h2>
        <div className="page-actions">
          <button onClick={()=>setCreating(true)}>创建客户端密钥</button>
          <button
            type="button"
            className="secondary"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => setGroupForm(null)}
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

      <details className="card"><summary>高级访问组</summary><div className="tablewrap">
        <table>
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
                  <td><ResourceIdentity id={group.id} name={group.name} kind="group" /></td>
                  <td>
                    <StatusBadge status={group.status} />
                  </td>
                  <td className="mono">{formatLimits(group.limits) || "—"}</td>
                  <td className="row-actions">
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
                      onClick={() => setGroupForm(group)}
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
      <div className="card tablewrap">
        <h3>客户端密钥</h3>
        <table>
          <thead>
            <tr>
              <th>前缀</th>
              <th>访问组</th>
              <th>状态</th>
              <th>过期</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(keys.data ?? []).map((record) => {
              const status = displayKeyStatus(record, nowMs);
              return (
                <tr key={record.id}>
                  <td className="mono">{record.prefix}</td>
                  <td>{record.access_group_id ? <ResourceIdentity id={record.access_group_id} kind="group" name={groups.data?.find((group)=>group.id===record.access_group_id)?.name}/> : "—"}</td>
                  <td>
                    <StatusBadge status={status}>{({active:"已启用",disabled:"已停用",revoked:"已吊销",expired:"已过期"})[status]}</StatusBadge>
                  </td>
                  <td className="mono">{formatExpiry(record.expires_at_ms)}</td>
                  <td className="row-actions">
                    <button className="secondary" onClick={() => setInspectedKey(record)}>详情</button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => {
                        updateKey.reset();setActionError(undefined);setWorkingId(undefined);
                        setEditKey(record);
                        setEditStatus(record.status);
                      }}
                    >
                      编辑
                    </button>
                    {record.status === "active" ? (
                      <button
                        type="button"
                        className="danger"
                        onClick={() => {revoke.reset();setActionError(undefined);setWorkingId(undefined);setConfirmRevoke(record.id);}}
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
        <button disabled={!editable} onClick={() => { setGroupForm(inspectedGroup); setInspectedGroup(undefined); }}>编辑访问组</button></div></ObjectInspector>}

      {inspectedKey === undefined ? null : <ObjectInspector title="Client Key" scope={`配置版本 ${resourceName(scope ?? "—", "config")} · 只显示公开元数据`} onClose={() => setInspectedKey(undefined)} facts={[
        ["Key ID", inspectedKey.id], ["前缀", inspectedKey.prefix], ["访问组", inspectedKey.access_group_id],
        ["配置状态", inspectedKey.status], ["当前显示状态", displayKeyStatus(inspectedKey, nowMs)], ["到期时间", formatExpiry(inspectedKey.expires_at_ms)],
      ]}><p className="small muted">完整密钥仅在签发时显示一次，详情不会重新显示。</p>
        <div className="sheet-actions"><button disabled={!editable} onClick={() => { setEditKey(inspectedKey); setEditStatus(inspectedKey.status); setInspectedKey(undefined); }}>编辑 Client Key</button></div></ObjectInspector>}

      {groupForm !== undefined ? (
        <Sheet
          title={groupForm === null ? "新建访问组" : `编辑访问组 · ${resourceName(groupForm.id,"group",groupForm.name)}`}
          onEscape={() => setGroupForm(undefined)}
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
            <label>
              限制
              <input
                name="limits"
                className="mono"
                placeholder="max_concurrency=4 rpm=600"
                defaultValue={groupForm === null ? "" : formatLimits(groupForm.limits)}
              />
            </label>
            <p className="stat-sub">
              形式与表格里显示的一致:空格分隔的 <span className="mono">key=value</span>,
              值为非负整数,最多 16 项。留空表示不设限。
            </p>
            {groupForm !== null ? (
              <p className="stat-sub">
                契约的 PATCH 收的是完整对象 —— 保存等于整体替换,未改的字段也会一并写回。
              </p>
            ) : null}
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
        <Sheet title="确认删除访问组" onEscape={() => setConfirmDeleteGroup(undefined)}>
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

      {issuing ? (
        <Sheet title="签发 Client Key" onEscape={() => setIssuing(false)}>
          <form className="sheet-form" onSubmit={onIssueSubmit}>
            <label>
              Key ID
              <input name="id" className="mono" required maxLength={128} />
            </label>
            <label>
              访问组
              <select name="access_group_id" required>
                {(groups.data ?? []).map((group) => (
                  <option key={group.id} value={group.id}>
                    {resourceOption(group.id, "group", group.name)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              过期时间(可空 = 永不过期)
              <input name="expires_at" type="datetime-local" />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setIssuing(false)}>
                取消
              </button>
              <button type="submit" disabled={issue.isPending}>
                签发
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {editKey !== undefined ? (
        <Sheet title={`编辑客户端密钥 · ${editKey.prefix}`} onEscape={() => !updateKey.isPending&&setEditKey(undefined)}>
          <ConfigurationTaskNotice workingId={workingId} error={updateKey.error} onReview={(version)=>{setEditKey(undefined);useVersionStore.getState().select(version);}}/>
          <form
            className="sheet-form"
            onSubmit={(event: FormEvent<HTMLFormElement>) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              const expiresRaw = String(data.get("expires_at") ?? "");
              updateKey.mutate({
                id: editKey.id,
                access_group_id: String(data.get("access_group_id") ?? ""),
                status: editStatus as ClientKeyRecord["status"],
                expires_at_ms: editedExpiry(expiresRaw,editKey.expires_at_ms),
              });
            }}
          >
            <label>
              访问组
              <select name="access_group_id" defaultValue={editKey.access_group_id} required>
                {(groups.data ?? []).map((group) => (
                  <option key={group.id} value={group.id}>
                    {resourceOption(group.id, "group", group.name)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              状态
              <select
                name="status"
                value={editStatus}
                onChange={(event) => setEditStatus(event.target.value)}
              >
                <option value="active">启用</option>
                <option value="disabled">停用</option>
                <option value="revoked">吊销</option>
              </select>
            </label>
            {isReactivation(editKey.status, editStatus) ? (
              // Measured from the backend: update_client_key applies status with
              // no transition check, and revoking RETAINS the redacted record.
              // So this is not "re-enable an inert row" — the original secret
              // authenticates again.
              <p role="alert" className="reveal-warning">
                这会让一把<strong>已吊销</strong>的 Key 重新生效 ——
                吊销保留了密钥记录,所以当初发出去的那串密钥会<strong>再次可用</strong>。
                如果当初是因为泄露才吊销的,请改为签发一把新的。
              </p>
            ) : null}
            <label>
              过期时间(留空 = 永不过期)
              <input
                name="expires_at"
                type="datetime-local"
                defaultValue={toLocalInput(editKey.expires_at_ms)}
              />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setEditKey(undefined)}>
                取消
              </button>
              <button type="submit" disabled={updateKey.isPending}>
                保存
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {issued !== undefined ? (
        <Sheet title="Client Key 已签发 — 只显示这一次">
          <p className="reveal-warning">关闭此窗口后,完整密钥将永远无法再次查看。</p>
          <code className="reveal-key mono">{issued.key}</code>
          <div className="sheet-actions">
            <button
              type="button"
              className="secondary"
              onClick={() => {
                void navigator.clipboard.writeText(issued.key).then(() => setCopied(true));
              }}
            >
              {copied ? "已复制 ✓" : "复制"}
            </button>
            <button type="button" onClick={closeReveal}>
              我已保存,关闭
            </button>
          </div>
        </Sheet>
      ) : null}

      {confirmRevoke !== undefined ? (
        <Sheet title="确认吊销" onEscape={() => !revoke.isPending&&setConfirmRevoke(undefined)}>
          <ConfigurationTaskNotice workingId={workingId} error={revoke.error} onReview={(version)=>{setConfirmRevoke(undefined);useVersionStore.getState().select(version);}}/>
          <p>吊销 {keys.data?.find((row)=>row.id===confirmRevoke)?.prefix}。应用后，该密钥不能再发起请求，历史记录保留。</p>
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setConfirmRevoke(undefined)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={revoke.isPending}
              onClick={() => revoke.mutate(confirmRevoke)}
            >
              确认吊销
            </button>
          </div>
        </Sheet>
      ) : null}
    </section>
  );
}
