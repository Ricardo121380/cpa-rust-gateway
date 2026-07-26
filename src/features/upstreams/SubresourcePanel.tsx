// Per-upstream subresource panel: endpoints / credentials / bindings, sliced
// from the PROPOSED G1 graph (fixture-backed until the contract lands).
// Endpoint test + catalog discovery drive REAL contract operations.
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { fetchProposedGraph, graphAvailable } from "../../api/proposed";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { useVersionStore } from "../config-versions/versionStore";
import { upstreamSubresources } from "./model";
import { OAuthWizard } from "./OAuthWizard";

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
  const [oauthTarget, setOauthTarget] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();

  const graph = useQuery({
    queryKey: ["graph", scope],
    queryFn: () => fetchProposedGraph(scope as string),
    enabled: scope !== undefined && graphAvailable(),
    staleTime: 10_000,
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

  if (!graphAvailable()) {
    return (
      <div className="card empty-state" data-kind="unwired">
        <p>
          <span className="mono">{upstreamId}</span> 的子资源枚举依赖 G1 全图契约
          (提案已交后端会话:CR-FE-001-shapes)。契约落地并 sync-contract 后此面板自动点亮。
        </p>
      </div>
    );
  }
  if (graph.isLoading || graph.data === undefined) {
    return (
      <div className="card empty-state" data-kind="empty">
        <p>加载配置图…</p>
      </div>
    );
  }

  const sub = upstreamSubresources(graph.data, upstreamId);

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

      <h3>
        端点 <span className="idchip mono">{sub.endpoints.length}</span>
      </h3>
      <table>
        <thead>
          <tr>
            <th>ID</th>
            <th>协议</th>
            <th>地址</th>
            <th>状态</th>
            <th>测试</th>
            <th>目录发现</th>
          </tr>
        </thead>
        <tbody>
          {sub.endpoints.map((endpoint) => {
            const result = testResults[endpoint.id];
            return (
              <tr key={endpoint.id}>
                <td className="mono">{endpoint.id}</td>
                <td className="mono">{endpoint.api_format}</td>
                <td className="mono">
                  {endpoint.base_url}
                  {endpoint.inference_path}
                </td>
                <td>
                  <StatusBadge status={endpoint.enabled ? "active" : "disabled"}>
                    {endpoint.enabled ? "enabled" : "disabled"}
                  </StatusBadge>
                </td>
                <td className="row-actions">
                  <button
                    type="button"
                    className="secondary"
                    disabled={test.isPending}
                    onClick={() => test.mutate({ endpointId: endpoint.id, mode: "non_streaming" })}
                  >
                    非流式
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={test.isPending}
                    onClick={() => test.mutate({ endpointId: endpoint.id, mode: "sse" })}
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
                    onClick={() => preview.mutate(endpoint.id)}
                  >
                    预览
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>

      <h3>
        凭据 <span className="idchip mono">{sub.credentials.length}</span>
      </h3>
      <table>
        <thead>
          <tr>
            <th>ID</th>
            <th>类型</th>
            <th>状态</th>
            <th>revision</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          {sub.credentials.map((credential) => (
            <tr key={credential.id}>
              <td className="mono">{credential.id}</td>
              <td className="mono">{credential.kind}</td>
              <td>
                <StatusBadge status={credential.status} />
              </td>
              <td className="mono">{credential.revision}</td>
              <td className="row-actions">
                {credential.kind === "oauth" ? (
                  <button
                    type="button"
                    disabled={!editable}
                    onClick={() => setOauthTarget(credential.id)}
                  >
                    OAuth 授权
                  </button>
                ) : (
                  <span className="muted-3">write-only</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>
        绑定 <span className="idchip mono">{sub.bindings.length}</span>
      </h3>
      <table>
        <thead>
          <tr>
            <th>端点</th>
            <th>凭据</th>
            <th>状态</th>
            <th>priority</th>
            <th>weight</th>
            <th>concurrency</th>
          </tr>
        </thead>
        <tbody>
          {sub.bindings.map((binding) => (
            <tr key={`${binding.endpoint_id}:${binding.credential_id}`}>
              <td className="mono">{binding.endpoint_id}</td>
              <td className="mono">{binding.credential_id}</td>
              <td>
                <StatusBadge status={binding.enabled ? "active" : "disabled"}>
                  {binding.enabled ? "enabled" : "disabled"}
                </StatusBadge>
              </td>
              <td className="mono">{binding.priority}</td>
              <td className="mono">{binding.weight}</td>
              <td className="mono">{binding.concurrency}</td>
            </tr>
          ))}
        </tbody>
      </table>

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

      {oauthTarget !== undefined ? (
        <OAuthWizard credentialId={oauthTarget} onClose={() => setOauthTarget(undefined)} />
      ) : null}
    </div>
  );
}
