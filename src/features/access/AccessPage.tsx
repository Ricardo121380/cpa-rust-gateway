// Access control: groups + client keys. Signature safety flow lives here —
// the reveal-once sheet (docs/07 §6.4): the full rgw_ key exists only in the
// 201 issue response; closing the sheet erases it from memory permanently.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Fragment, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import {
  displayKeyStatus,
  formatExpiry,
  formatLimits,
  parseLimits,
  type AccessGroupRecord,
  type ClientKeyRecord,
  type IssuedClientKey,
} from "./model";

type AccessGroupRoute = Readonly<{
  access_group_id: string;
  route_id: string;
  enabled: boolean;
}>;

/** Routes are not enumerable: the contract has createRoute/getRoute but no
 *  listRoutes. The operational inventory carries route_ids per binding, so
 *  those become datalist suggestions — the field stays free text because the
 *  suggestions are known to be incomplete. */
type PoolRow = Readonly<{ route_ids: readonly string[] }>;

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

  const grant = useMutation({
    mutationFn: (input: { route_id: string; enabled: boolean }) =>
      call<AccessGroupRoute>(
        "grantAccessGroupRoute",
        { path: { access_group_id: groupId }, body: input },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setAdding(false);
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
              <th>route_id</th>
              <th>状态</th>
            </tr>
          </thead>
          <tbody>
            {(grants.data ?? []).map((row) => (
              <tr key={row.route_id}>
                <td className="mono">{row.route_id}</td>
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
        <Sheet title={`授权路由 · ${groupId}`} onEscape={() => setAdding(false)}>
          <form className="sheet-form" onSubmit={onGrantSubmit}>
            <label>
              route_id
              <input name="route_id" className="mono" required maxLength={128} list="route-ids" />
            </label>
            <datalist id="route-ids">
              {routeIds.map((id) => (
                <option key={id} value={id} />
              ))}
            </datalist>
            <p className="stat-sub">
              契约没有 listRoutes —— 上面的建议来自运营库存里出现过的 route_id,并不完整。
            </p>
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
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const [issuing, setIssuing] = useState(false);
  const [issued, setIssued] = useState<IssuedClientKey | undefined>();
  const [copied, setCopied] = useState(false);
  const [actionError, setActionError] = useState<string | undefined>();
  const [confirmRevoke, setConfirmRevoke] = useState<string | undefined>();
  // undefined = closed; null = creating; record = editing that group
  const [groupForm, setGroupForm] = useState<AccessGroupRecord | null | undefined>();
  const [confirmDeleteGroup, setConfirmDeleteGroup] = useState<string | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();

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

  const revoke = useMutation({
    mutationFn: (id: string) =>
      call<undefined>(
        "revokeClientKey",
        { path: { client_key_id: id } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setConfirmRevoke(undefined);
      void queryClient.invalidateQueries({ queryKey: ["client-keys", scope] });
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

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
    setIssued(undefined); // the key is gone for good — by design
    setCopied(false);
  }

  if (scope === undefined) {
    return (
      <section>
        <h2>{t.nav.access}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本。</p>
        </div>
      </section>
    );
  }

  const nowMs = Date.now();

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.access}</h2>
        <div className="page-actions">
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
            签发 Client Key
          </button>
        </div>
      </header>

      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <div className="card tablewrap">
        <h3>访问组</h3>
        <table>
          <thead>
            <tr>
              <th>ID</th>
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
                  <td className="mono">{group.id}</td>
                  <td>{group.name}</td>
                  <td>
                    <StatusBadge status={group.status} />
                  </td>
                  <td className="mono">{formatLimits(group.limits) || "—"}</td>
                  <td className="row-actions">
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
                    <td colSpan={5}>
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

      <div className="card tablewrap">
        <h3>Client Key(仅前缀,完整密钥永不回显)</h3>
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
                  <td className="mono">{record.access_group_id}</td>
                  <td>
                    <StatusBadge status={status} />
                  </td>
                  <td className="mono">{formatExpiry(record.expires_at_ms)}</td>
                  <td className="row-actions">
                    {record.status === "active" ? (
                      <button
                        type="button"
                        className="danger"
                        disabled={!editable}
                        onClick={() => setConfirmRevoke(record.id)}
                      >
                        吊销
                      </button>
                    ) : (
                      "—"
                    )}
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

      {groupForm !== undefined ? (
        <Sheet
          title={groupForm === null ? "新建访问组" : `编辑访问组 · ${groupForm.id}`}
          onEscape={() => setGroupForm(undefined)}
        >
          <form className="sheet-form" onSubmit={onGroupSubmit}>
            <label>
              ID
              <input
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
              <input name="name" required maxLength={128} defaultValue={groupForm?.name ?? ""} />
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
            删除 <span className="mono">{confirmDeleteGroup}</span> 会同时移除它的路由授权。
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
                    {group.name}({group.id})
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
        <Sheet title="确认吊销" onEscape={() => setConfirmRevoke(undefined)}>
          <p>
            吊销 <span className="mono">{confirmRevoke}</span> 不可逆:记录保留、状态转为
            revoked,该 Key 立即不能再认证。
          </p>
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
