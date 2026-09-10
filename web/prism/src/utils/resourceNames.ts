/** Presentation only. Original IDs must remain the values used by APIs, URLs,
 * filters, React keys and audit references. Model protocol names are excluded. */
export type ResourceKind = "account" | "upstream" | "endpoint" | "route" | "candidate" | "group" | "policy" | "config" | "catalog" | "resource";

const nouns: Record<ResourceKind, string> = {
  account: "账号", upstream: "上游", endpoint: "端点", route: "路由", candidate: "候选",
  group: "访问组", policy: "出口策略", config: "配置", catalog: "目录", resource: "资源",
};
const words: Record<string, string> = {
  codex: "Codex", grok: "Grok", openai: "OpenAI", anthropic: "Anthropic", claude: "Claude", chatgpt: "ChatGPT", go: "Go",
  kiro: "Kiro", krill: "Krill", oauth: "OAuth", api: "API", bridge: "桥接", official: "官方",
  primary: "主", secondary: "备用", native: "原生", build: "Build", console: "Console",
  chat: "Chat", responses: "Responses", messages: "Messages", direct: "直连", proxy: "代理",
};

export function isInternalLabel(value: string): boolean {
  return /^p\d{1,2}[-_]/iu.test(value) || /(?:^|[-_])[a-f0-9]{24,}$/iu.test(value);
}

/** Short visual discriminator, never an identifier or authorization input. */
export function resourceCode(id: string): string {
  let hash = 2166136261;
  for (const character of id) hash = Math.imul(hash ^ character.codePointAt(0)!, 16777619);
  return (hash >>> 0).toString(16).toUpperCase().padStart(8, "0");
}

export function resourceName(id: string, kind: ResourceKind = "resource", name?: string | null): string {
  const source = name?.trim() || id;
  if (!isInternalLabel(source)) return source;
  const readable = source
    .replace(/^p\d{1,2}[-_]\d+[a-z\d]*[-_\s]+/iu, "")
    .replace(/^p\d{1,2}[-_]/iu, "")
    .replace(/(?:^|[-_])[a-f0-9]{24,}$/iu, "")
    .split(/[\s_-]+/u)
    .filter((part) => !/^(production|existing|test|preview|staging|credential|credentials|upstream|endpoint|route|candidate|egress|policy|group|key|config|version|\d{10,13})$/iu.test(part))
    .map((part) => words[part.toLowerCase()] ?? part)
    .join(" ").trim();
  return `${readable}${readable ? " " : ""}${nouns[kind]}`;
}

/** Native option elements need plain text while retaining a discriminator. */
export function resourceOption(id: string, kind: ResourceKind = "resource", name?: string | null): string {
  const label = resourceName(id, kind, name);
  return isInternalLabel(id) || isInternalLabel(name?.trim() || id)
    ? `${label} · ${resourceCode(id)}` : label;
}
