/** Presentation only. Original IDs must remain the values used by APIs, URLs,
 * filters, React keys and audit references. Model protocol names are excluded. */
export type ResourceKind = "account" | "upstream" | "endpoint" | "route" | "candidate" | "group" | "key" | "policy" | "config" | "catalog" | "resource";

const nouns: Record<ResourceKind, string> = {
  account: "账号", upstream: "提供商", endpoint: "接口", route: "路由", candidate: "候选",
  group: "访问组", key:"访问密钥", policy: "出口策略", config: "配置", catalog: "目录", resource: "资源",
};
const words: Record<string, string> = {
  codex: "Codex", grok: "Grok", openai: "OpenAI", anthropic: "Anthropic", claude: "Claude", chatgpt: "ChatGPT", go: "Go",
  kiro: "Kiro", krill: "Krill", oauth: "OAuth", api: "API", bridge: "桥接", official: "官方",
  primary: "主", secondary: "备用", native: "原生", build: "Build", console: "Console",
  chat: "Chat", responses: "Responses", messages: "Messages", direct: "直连", proxy: "代理",
};

export function isInternalLabel(value: string): boolean {
  return /^p\d{1,2}[-_]/iu.test(value)
    || /(?:^|[-_])(?:test|testing|e2e|fixture|smoke|preview|staging|acceptance)(?:[-_]|$)/iu.test(value)
    || /(?:^|[-_])[a-f0-9]{24,}$/iu.test(value)
    || /[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/iu.test(value);
}

export function resourceName(id: string, kind: ResourceKind = "resource", name?: string | null): string {
  const source = name?.trim() || id;
  if (kind === "config" && !name?.trim() && /^edit-/u.test(id)) return "配置副本";
  if (kind === "account" && name?.trim()) return name.trim();
  if (/^p\d{1,2}[-_]\d+[a-z\d]*$/iu.test(source)) return kind === "resource" ? "历史标签" : nouns[kind];
  if (!isInternalLabel(source)) return source.replace(/p\d{1,2}[-_][\w.-]+/giu, value=>resourceName(value,kind));
  const readable = source
    .replace(/^p\d{1,2}[-_]\d+[a-z\d]*[-_\s]+/iu, "")
    .replace(/^p\d{1,2}[-_]/iu, "")
    .replace(/(?:^|[-_])[a-f0-9]{24,}$/iu, "")
    .replace(/[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/iu, "")
    .split(/[\s_-]+/u)
    .filter((part) => !/^(production|existing|test|testing|e2e|fixture|smoke|preview|staging|acceptance|bridge|credential|credentials|upstream|endpoint|route|candidate|egress|policy|group|key|config|version|v\d+|\d{6,})$/iu.test(part))
    .map((part) => words[part.toLowerCase()] ?? part)
    .join(" ").trim();
  return `${readable}${readable ? " " : ""}${nouns[kind]}`;
}

/** Option values remain exact IDs; labels never add invented hash suffixes. */
export function resourceOption(id: string, kind: ResourceKind = "resource", name?: string | null): string {
  return resourceName(id, kind, name);
}

export function referenceKind(label: string): ResourceKind {
  if (/凭据|账号|credential|account/iu.test(label)) return "account";
  if (/提供商|上游|provider|upstream/iu.test(label)) return "upstream";
  if (/接口|端点|endpoint|channel/iu.test(label)) return "endpoint";
  if (/候选|candidate/iu.test(label)) return "candidate";
  if (/路由|route/iu.test(label)) return "route";
  if (/访问组|group/iu.test(label)) return "group";
  if (/key/iu.test(label)) return "key";
  if (/策略|policy/iu.test(label)) return "policy";
  if (/目录|catalog/iu.test(label)) return "catalog";
  if (/配置|版本|config|version/iu.test(label)) return "config";
  return "resource";
}

/** Display-only prose (not API payloads, model names or exported audit evidence). */
export function referenceText(text: string): string {
  return text.replace(/[\w][\w.-]*/gu, (value) => isInternalLabel(value) ? resourceName(value, referenceKind(value)) : value);
}
