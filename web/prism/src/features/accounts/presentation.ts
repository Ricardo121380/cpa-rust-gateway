export type AccountIdentity = Readonly<{email: string | null; phone: string | null; username: string | null}>;
export const accountGroups = [
  {id:"api",name:"API",description:"密钥与兼容接口"},
  {id:"codex",name:"Codex / ChatGPT",description:"OpenAI 账号"},
  {id:"claude",name:"Claude",description:"Anthropic 账号"},
  {id:"kimi",name:"Kimi",description:"Moonshot 账号"},
  {id:"kiro",name:"Kiro",description:"Kiro 账号"},
  {id:"grok",name:"Grok",description:"Web · Console · Build"},
] as const;
export type Category = typeof accountGroups[number]["id"];
// Operator-provided labels are useful; generated identifiers and provenance are not identities.
export function operatorName(value: string): string | undefined {
  value=value.trim();
  if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/u.test(value) || /^\+?[0-9 ()-]{7,20}$/u.test(value) && value.replace(/\D/gu,"").length>=7 && value.replace(/\D/gu,"").length<=15) return value;
  if (!value || value.length > 80 || /(?:test|测试|验收|autoreg|^p\d+[\s-]|^grok-|^cred-|^account-\d|^sk-|^ksk_|^mgmt_|^csrf_|^eyJ)/iu.test(value)) return;
  if (/\b(?:credential|upstream|endpoint|bridge|production)\b/iu.test(value)) return;
  if (value.split(/[^a-z0-9]/iu).some((p)=>/^[a-f0-9]{8,}$/iu.test(p))) return;
  return value;
}
export function accountName(identity: AccountIdentity | undefined, label?: string): string | undefined {
  return identity?.email || identity?.phone || identity?.username || (label ? operatorName(label) : undefined);
}
export function accountSource(value: string): string | undefined {
  return /autoreg/iu.test(value) ? "Autoreg" : undefined;
}
export function protocolName(format: string): string {
  const names:Record<string,string>={"openai/responses":"Responses", "openai/chat-completions":"Chat Completions", "anthropic/messages":"Messages", openai_responses:"Responses", "openai-responses":"Responses",responses:"Responses",openai_chat:"Chat Completions","openai-chat":"Chat Completions",chat_completions:"Chat Completions",anthropic_messages:"Messages","anthropic-messages":"Messages",messages:"Messages"};
  return names[format] ?? "兼容接口";
}
