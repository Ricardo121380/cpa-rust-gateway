// Operational account-pool inventory (P13-04A) — the projection that finally
// enumerates channels, accounts and bindings for one provider.
//
// Vocabulary is the contract's, verbatim: provider / channel / account, and
// the account_status set active|cooling|unauthorized|disabled. The config
// plane calls the same entities upstream / endpoint / credential and carries
// its own status set — Prism does not invent a translation between the two,
// it shows whichever plane answered.
//
// One row IS one binding. That is the coverage boundary: a channel with no
// binding, or an account bound to nothing, does not appear here at all.

export type AccountStatus = "active" | "cooling" | "unauthorized" | "disabled";

export type AccountPoolItem = Readonly<{
  provider_id: string;
  provider_name: string;
  provider_kind: string;
  provider_enabled: boolean;
  egress_policy_id?: string | null;
  channel_id: string;
  adapter_id: string;
  api_format: string;
  transport: string;
  channel_enabled: boolean;
  account_id: string;
  account_kind: string;
  account_status: AccountStatus;
  account_revision: number;
  binding_enabled: boolean;
  configured_enabled: boolean;
  priority: number;
  weight: number;
  concurrency: number;
  route_ids: readonly string[];
}>;

export type AccountPoolPage = Readonly<{
  config_version_id: string;
  revision: number;
  items: readonly AccountPoolItem[];
  next_cursor?: string | null;
}>;

/** Contract default is 50, max 100 (keyset on provider_id, channel_id, account_id). */
export const POOL_PAGE_LIMIT = 100;

export type ChannelView = Readonly<{
  channel_id: string;
  adapter_id: string;
  api_format: string;
  transport: string;
  channel_enabled: boolean;
  /** Accounts bound to this channel, in keyset order. */
  account_ids: readonly string[];
}>;

export type AccountView = Readonly<{
  account_id: string;
  account_kind: string;
  account_status: AccountStatus;
  account_revision: number;
}>;

export type BindingView = Readonly<{
  channel_id: string;
  account_id: string;
  binding_enabled: boolean;
  configured_enabled: boolean;
  priority: number;
  weight: number;
  concurrency: number;
  route_ids: readonly string[];
}>;

export type ProviderPool = Readonly<{
  provider_id: string;
  provider_name: string;
  provider_kind: string;
  provider_enabled: boolean;
  egress_policy_id: string | null;
  channels: readonly ChannelView[];
  accounts: readonly AccountView[];
  bindings: readonly BindingView[];
}>;

/**
 * Collapses the flat binding rows into the three tables the panel shows.
 * Rows arrive in keyset order (provider, channel, account); first-seen order
 * is preserved so the tables read the same way the server paginates.
 */
export function providerPool(
  items: readonly AccountPoolItem[],
  providerId: string,
): ProviderPool | undefined {
  const rows = items.filter((row) => row.provider_id === providerId);
  const head = rows[0];
  if (head === undefined) {
    return undefined;
  }

  const channels = new Map<string, { view: ChannelView; accounts: string[] }>();
  const accounts = new Map<string, AccountView>();
  const bindings: BindingView[] = [];

  for (const row of rows) {
    let channel = channels.get(row.channel_id);
    if (channel === undefined) {
      const accountIds: string[] = [];
      channel = {
        accounts: accountIds,
        view: {
          channel_id: row.channel_id,
          adapter_id: row.adapter_id,
          api_format: row.api_format,
          transport: row.transport,
          channel_enabled: row.channel_enabled,
          account_ids: accountIds,
        },
      };
      channels.set(row.channel_id, channel);
    }
    if (!channel.accounts.includes(row.account_id)) {
      channel.accounts.push(row.account_id);
    }

    if (!accounts.has(row.account_id)) {
      accounts.set(row.account_id, {
        account_id: row.account_id,
        account_kind: row.account_kind,
        account_status: row.account_status,
        account_revision: row.account_revision,
      });
    }

    bindings.push({
      channel_id: row.channel_id,
      account_id: row.account_id,
      binding_enabled: row.binding_enabled,
      configured_enabled: row.configured_enabled,
      priority: row.priority,
      weight: row.weight,
      concurrency: row.concurrency,
      route_ids: row.route_ids,
    });
  }

  return {
    provider_id: head.provider_id,
    provider_name: head.provider_name,
    provider_kind: head.provider_kind,
    provider_enabled: head.provider_enabled,
    egress_policy_id: head.egress_policy_id ?? null,
    channels: [...channels.values()].map((entry) => entry.view),
    accounts: [...accounts.values()],
    bindings,
  };
}

/**
 * Badge tone for a stored account status.
 *
 * `cooling` and `unauthorized` are new in the operations plane — the config
 * plane's Credential.status only knows active/disabled/revoked. They are not
 * folded into an existing tone, because "cooling" is a wait and
 * "unauthorized" is a stop.
 */
export function accountStatusTone(status: AccountStatus): string {
  switch (status) {
    case "active":
      return "active";
    case "cooling":
      return "quota_blocked";
    case "unauthorized":
      return "credential_forbidden";
    case "disabled":
      return "disabled";
  }
}

/**
 * What `configured_enabled` actually asserts, per the P13-04A report:
 * provider_enabled && channel_enabled && binding_enabled. It says nothing
 * about the credential being healthy, under quota, or routable — the panel
 * has to spell that out or an operator will read a green row as "working".
 */
export function enabledConjunction(row: {
  provider_enabled: boolean;
  channel_enabled: boolean;
  binding_enabled: boolean;
}): boolean {
  return row.provider_enabled && row.channel_enabled && row.binding_enabled;
}
