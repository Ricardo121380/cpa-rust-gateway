import { protocolName } from "../accounts/presentation";
import type { ManagedCredential, ManagedEndpoint } from "../accounts/inventory";
import type { NativeAccount } from "../accounts/NativeAccounts";
import type { PoolAccount } from "../runtime/model";

export function nativeProviderForAdapter(adapter: string): NativeAccount["provider"] | undefined {
  const providers: Readonly<Record<string, NativeAccount["provider"]>> = { "grok.build.responses": "grok_build", "grok.console.responses": "grok_console", "grok.web.responses": "grok_web" };
  return providers[adapter];
}

/** A configured credential binding is not the native channel's runtime pool. */
export function observedEndpointAccounts(rows: readonly Pick<PoolAccount, "provider_id" | "channel_id" | "account_id">[], upstreamId: string, endpointId: string): readonly string[] {
  return [...new Set(rows.filter(row => row.provider_id === upstreamId && row.channel_id === endpointId).map(row => row.account_id))];
}

export function nativeAccountsForEndpoints(accounts: readonly NativeAccount[], endpoints: readonly Pick<ManagedEndpoint, "adapter_id">[]): readonly NativeAccount[] {
  const providers = new Set(endpoints.map(endpoint => nativeProviderForAdapter(endpoint.adapter_id)));
  return accounts.filter(account => providers.has(account.provider));
}

export function endpointLabel(endpoint: Pick<ManagedEndpoint, "api_format" | "base_url">): string {
  try {
    return `${protocolName(endpoint.api_format)} · ${new URL(endpoint.base_url).host}`;
  } catch {
    return protocolName(endpoint.api_format);
  }
}

type EndpointConfiguration = Pick<
  ManagedEndpoint,
  "id" | "upstream_id" | "adapter_id" | "api_format" | "base_url" | "inference_path" | "transport" | "enabled"
> & Readonly<{ models_path?: string | null }>;

/** The inventory and detail APIs serialize equivalent endpoint objects through
 * different server projections. Compare the contract fields, never JSON key
 * order, before replacing or deleting the captured target. */
export function sameEndpointConfiguration(left: EndpointConfiguration, right: EndpointConfiguration): boolean {
  return left.id === right.id
    && left.upstream_id === right.upstream_id
    && left.adapter_id === right.adapter_id
    && left.api_format === right.api_format
    && left.base_url === right.base_url
    && left.inference_path === right.inference_path
    && (left.models_path ?? null) === (right.models_path ?? null)
    && left.transport === right.transport
    && left.enabled === right.enabled;
}

export function authenticationLabel(account: Pick<ManagedCredential, "authentication" | "category">): string {
  if (account.authentication === "oauth") return "OAuth 授权";
  if (account.authentication === "api_key") return account.category === "kimi" ? "Kimi API Key" : "API Key / Token";
  return "接入方式未确认";
}

export function runtimeConnectionLabel(
  accountCount: number,
  state: "loading" | "unavailable" | "partial" | "observed",
): string {
  if (state === "loading") return "正在读取运行连接";
  if (state === "unavailable") return "运行连接暂不可读取";
  if (state === "partial") return accountCount === 0 ? "已加载的运行调度中未观测到账号" : `已加载的运行调度中观测到 ${accountCount} 个账号`;
  return accountCount === 0 ? "运行调度中未观测到账号" : `运行调度中观测到 ${accountCount} 个账号`;
}

export function accountStatusLabel(status: ManagedCredential["credential"]["status"]): string {
  return ({ active: "已启用", disabled: "已停用", revoked: "已撤销", cooling: "冷却中", unauthorized: "需要重新授权" })[status];
}
