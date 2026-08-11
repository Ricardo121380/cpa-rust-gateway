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
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
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

const TEST_TONE: Record<EndpointTest["outcome"], string> = {
  pass: "active",
  rejected: "quota_blocked",
  transport_failed: "credential_forbidden",
  protocol_failed: "circuit_open",
};

export function SubresourcePanel({ upstreamId }: Readonly<{ upstreamId: string }>) {
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [testResults, setTestResults] = useState<Record<string, EndpointTest>>({});
  const [discovery, setDiscovery] = useState<
    { endpointId: string; diff: CatalogDiff; applied: boolean } | undefined
  >();
  const [accountTarget, setAccountTarget] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();

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
      </h3>
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

      {accountTarget !== undefined ? (
        <CredentialSheet
          credentialId={accountTarget}
          onClose={() => setAccountTarget(undefined)}
        />
      ) : null}
    </div>
  );
}
