// Public-model pure model. Capabilities use the backend's frozen semantic
// capability keys; parallel_tools implies tools (mirrored client-side).
export type PublicModel = Readonly<{
  id: string;
  model_name: string;
  status: "active" | "disabled";
  display_name: string;
  capabilities: Readonly<Record<string, boolean>>;
}>;

export const SEMANTIC_CAPABILITIES = [
  "streaming",
  "tools",
  "parallel_tools",
  "reasoning",
  "json_schema",
  "vision",
] as const;

export type SemanticCapability = (typeof SEMANTIC_CAPABILITIES)[number];

/** parallel_tools ⇒ tools; turning tools off drops parallel_tools. */
export function toggleCapability(
  current: Readonly<Record<string, boolean>>,
  capability: SemanticCapability,
  enabled: boolean,
): Record<string, boolean> {
  const next: Record<string, boolean> = { ...current };
  if (enabled) {
    next[capability] = true;
    if (capability === "parallel_tools") {
      next["tools"] = true;
    }
  } else {
    delete next[capability];
    if (capability === "tools") {
      delete next["parallel_tools"];
    }
  }
  return next;
}

export function enabledCapabilities(
  capabilities: Readonly<Record<string, boolean>>,
): string[] {
  return Object.entries(capabilities)
    .filter(([, enabled]) => enabled)
    .map(([name]) => name)
    .sort();
}

export const ROUTE_POLICY = "smooth_weighted_round_robin" as const;

export function validRouteParams(maxAttempts: number, bootstrapTimeoutMs: number): boolean {
  return (
    Number.isInteger(maxAttempts) &&
    maxAttempts >= 1 &&
    maxAttempts <= 16 &&
    Number.isInteger(bootstrapTimeoutMs) &&
    bootstrapTimeoutMs >= 1 &&
    bootstrapTimeoutMs <= 120000
  );
}
