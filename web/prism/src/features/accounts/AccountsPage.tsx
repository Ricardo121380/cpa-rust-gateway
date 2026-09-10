import {
  useInfiniteQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
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
import {
  receiptMeta,
  type PoolAction,
  type ActionReceipt,
} from "../runtime/model";

export function AccountsPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const scope = useVersionStore((s) => s.context?.configVersionId);
  const [params, setParams] = useSearchParams();
  const [selected, setSelected] = useState<PoolAccount>();
  const [tab, setTab] = useState("runtime");
  const [credential, setCredential] = useState<string>();
  const [target, setTarget] = useState<{
    account: PoolAccount;
    action: PoolAction;
  }>();
  const [receipt, setReceipt] = useState<ActionReceipt>();
  const [actionError, setActionError] = useState<string>();
  const provider = params.get("provider") ?? "";
  const auth = params.get("auth") ?? "";
  const runtime = params.get("runtime") ?? "";
  const query = params.get("q") ?? "";
  const key = ["accounts", provider, auth, runtime];
  const act = useMutation({
    mutationFn: (body: Readonly<Record<string, unknown>>) =>
      call<ActionReceipt>(
        "applyProviderAccountPoolAction",
        { body },
        { versionScoped: true },
      ),
    onSuccess: (result) => {
      setTarget(undefined);
      setReceipt(result);
      void queryClient.resetQueries({ queryKey: ["accounts"] });
      void queryClient.invalidateQueries({ queryKey: ["provider-pools"] });
    },
    onError: (cause) => {
      const error = asAppError(cause);
      setTarget(undefined);
      if (error.kind === "conflict") {
        void queryClient.resetQueries({ queryKey: ["accounts"] });
        setActionError("目标快照已改变，已重新读取。请重新选取账号后再操作。");
      } else setActionError(`${error.code} · ${error.message}`);
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
          ...(auth ? { auth_status: auth } : {}),
          ...(runtime ? { runtime_status: runtime } : {}),
          ...(pageParam ? { cursor: pageParam } : {}),
        },
      }),
    getNextPageParam: (last) => last.next_cursor ?? undefined,
    retry: false,
  });
  const loaded = pools.data?.pages.flatMap((page) => page.items) ?? [];
  const rows = loaded.filter((row) =>
    `${row.provider_id} ${row.channel_id} ${row.account_id}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const update = (name: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(name, value);
    else next.delete(name);
    setParams(next, { replace: true });
  };
  const open = (row: PoolAccount) => {
    setSelected(row);
    setTab("runtime");
  };
  const observed = pools.data?.pages[0]?.observed_at_ms;
  const error = pools.isError ? asAppError(pools.error) : undefined;

  return (
    <section>
      <header className="page-head">
        <div>
          <h2>{t.nav.accounts}</h2>
          <p className="scope-row">
            当前已加载账号 · 跨配置版本
          </p>
        </div>
        <button
          className="secondary"
          onClick={() => {
            setSelected(undefined);
            void queryClient.resetQueries({ queryKey: key, exact: true });
          }}
        >
          刷新账号
        </button>
      </header>
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
          {receiptMeta(receipt.state).label} ·{" "}
          {receiptMeta(receipt.state).detail}
          <button className="secondary" onClick={() => setReceipt(undefined)}>
            知道了
          </button>
        </p>
      )}
      <div className="stat-row">
        {[
          ["已加载账号", loaded.length],
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
            placeholder="搜索账号、Channel"
            value={query}
            onChange={(e) => update("q", e.target.value)}
          />
          <input
            aria-label="Provider ID"
            placeholder="全部 Provider"
            value={provider}
            onChange={(e) => update("provider", e.target.value)}
          />
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
        {error !== undefined ? (
          <div className="empty-state" role="alert">
            {error.kind === "unavailable"
              ? t.state.unavailable
              : `${error.code} · ${error.message}`}
            <p>刷新列表后再继续；未把旧快照显示为最新结果。</p>
          </div>
        ) : pools.isPending ? (
          <div className="empty-state">读取账号…</div>
        ) : rows.length === 0 ? (
          <div className="empty-state">
            {provider || auth || runtime || query
              ? t.state.filteredEmpty
              : t.state.empty}
          </div>
        ) : (
          <>
            <div className="tablewrap account-desktop">
              <table>
                <thead>
                  <tr>
                    <th>账号 / Provider</th>
                    <th>认证</th>
                    <th>调度</th>
                    <th>并发</th>
                    <th>权益</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((row) => (
                    <tr
                      key={`${row.provider_id}/${row.channel_id}/${row.account_id}`}
                    >
                      <td>
                        <div className="entity-name">{row.account_id}</div>
                        <div className="entity-meta">
                          {row.provider_id} / {row.channel_id}
                        </div>
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
              </table>
            </div>
            <div className="account-mobile">
              {rows.map((row) => (
                <article
                  key={`${row.provider_id}/${row.channel_id}/${row.account_id}`}
                >
                  <div className="entity-name">{row.account_id}</div>
                  <div className="entity-meta">
                    {row.provider_id} / {row.channel_id}
                  </div>
                  <p>
                    <StatusBadge status={row.auth_status}>
                      {authStatusMeta(row.auth_status).label}
                    </StatusBadge>{" "}
                    <StatusBadge status={row.runtime_status}>
                      {runtimeStatusMeta(row.runtime_status).label}
                    </StatusBadge>
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
          title={selected.account_id}
          layout="inspector"
          onEscape={() => setSelected(undefined)}
        >
          <p className="entity-meta">
            {selected.provider_id} / {selected.channel_id} ·{" "}
            {formatObservedAt(observed ?? 0)}
          </p>
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
                ["认证", authStatusMeta(selected.auth_status).label],
                ["调度", runtimeStatusMeta(selected.runtime_status).label],
                ["启用", selected.enabled ? "已启用" : "已停用"],
                [
                  "并发 / 上限",
                  `${selected.active_leases} / ${selected.max_concurrency}`,
                ],
                ["优先级 / 权重", `${selected.priority} / ${selected.weight}`],
                [
                  "认证到期",
                  selected.expires_at_ms === null
                    ? "未观测 / 不适用"
                    : formatObservedAt(selected.expires_at_ms),
                ],
                [
                  "续期时间",
                  selected.refresh_due_at_ms === null
                    ? "未观测 / 不适用"
                    : formatObservedAt(selected.refresh_due_at_ms),
                ],
                [
                  "Quota 同步",
                  selected.quota_sync_due_at_ms === null
                    ? "未观测 / 不适用"
                    : formatObservedAt(selected.quota_sync_due_at_ms),
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
                entitlement={selected.entitlement}
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
                  setCredential(selected.account_id);
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
                  to={`/catalog?q=${encodeURIComponent(selected.account_id)}`}
                >
                  目录证据
                </Link>
                <Link
                  to={`/monitoring?tab=failures&account_id=${encodeURIComponent(selected.account_id)}&provider_id=${encodeURIComponent(selected.provider_id)}&channel_id=${encodeURIComponent(selected.channel_id)}`}
                >
                  失败记录
                </Link>
                <Link
                  to={`/runtime?account_id=${encodeURIComponent(selected.account_id)}`}
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
                      setTarget({ account: selected, action });
                      setSelected(undefined);
                      setActionError(undefined);
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
                  : `操作使用配置版本 ${scope}。恢复结果由运行时判定。`}
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
          onCancel={() => setTarget(undefined)}
          onInvalid={setActionError}
          onSubmit={(body) => act.mutate(body)}
        />
      )}
      {credential === undefined ? null : (
        <CredentialSheet
          credentialId={credential}
          onClose={() => setCredential(undefined)}
        />
      )}
    </section>
  );
}
