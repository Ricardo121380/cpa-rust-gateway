// Egress policy pure model — mirrors contract bounds:
// schemes fixed to https; hosts ≤128; ports 1-65535 (≤128); cidrs ≤128;
// redirect_mode deny|revalidate; max_redirects 0-5 with deny ⇒ 0.
export type EgressPolicy = Readonly<{
  id: string;
  name: string;
  allowed_schemes: readonly string[];
  allowed_hosts: readonly string[];
  allowed_ports: readonly number[];
  allowed_cidrs: readonly string[];
  redirect_mode: "deny" | "revalidate";
  max_redirects: number;
}>;

export function normalizedMaxRedirects(
  mode: EgressPolicy["redirect_mode"],
  requested: number,
): number {
  if (mode === "deny") {
    return 0;
  }
  return Math.min(5, Math.max(1, Math.trunc(requested)));
}

export function validatePortEntry(entry: string): string | undefined {
  if (!/^\d+$/u.test(entry)) {
    return "端口必须是数字";
  }
  const port = Number(entry);
  if (port < 1 || port > 65535) {
    return "端口范围 1-65535";
  }
  return undefined;
}

export function validateHostEntry(entry: string): string | undefined {
  if (entry.length > 253 || entry.includes("*") || entry.includes("/")) {
    return "主机名需为精确域名(无通配符、无路径)";
  }
  return undefined;
}

export function referencingUpstreams(
  policyId: string,
  upstreams: ReadonlyArray<{ id: string; egress_policy_id?: string | null }>,
): string[] {
  return upstreams
    .filter((upstream) => upstream.egress_policy_id === policyId)
    .map((upstream) => upstream.id);
}
