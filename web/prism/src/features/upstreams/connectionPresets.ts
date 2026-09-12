// Existing runtime adapter / target pairs. These are not discovery results or model allowlists.
export const CONNECTION_PRESETS = [
  {id:"responses",name:"OpenAI 兼容 · Responses",kind:"openai-compatible",channel:"openai-compatible",adapter:"openai-compatible.responses",format:"openai/responses",base:"https://api.openai.com/v1",path:"/responses",fixed:false,native:false},
  {id:"chat",name:"OpenAI 兼容 · Chat Completions",kind:"openai-compatible",channel:"openai-compatible",adapter:"openai-compatible.chat-completions",format:"openai/chat-completions",base:"https://api.openai.com/v1",path:"/chat/completions",fixed:false,native:false},
  {id:"messages",name:"Anthropic 兼容 · Messages",kind:"anthropic-compatible",channel:"anthropic-compatible",adapter:"anthropic-compatible.messages",format:"anthropic/messages",base:"https://api.anthropic.com",path:"/v1/messages",fixed:false,native:false},
  {id:"codex",name:"Codex / ChatGPT",kind:"codex",channel:"codex",adapter:"openai-compatible.responses",format:"openai/responses",base:"https://chatgpt.com/backend-api/codex",path:"/responses",fixed:true,native:false},
  {id:"claude",name:"Claude",kind:"claude",channel:"claude",adapter:"anthropic-compatible.messages",format:"anthropic/messages",base:"https://api.anthropic.com",path:"/v1/messages",fixed:true,native:false},
  {id:"kimi",name:"Kimi · API",kind:"kimi",channel:"kimi",adapter:"openai-compatible.chat-completions",format:"openai/chat-completions",base:"https://api.moonshot.cn/v1",path:"/chat/completions",fixed:false,native:false},
  {id:"kiro",name:"Kiro",kind:"kiro",channel:"kiro",adapter:"kiro.messages",format:"anthropic/messages",base:"https://runtime.us-east-1.kiro.dev",path:"/",fixed:false,native:false},
  {id:"grok-api",name:"Grok · API",kind:"grok.official",channel:"grok.official",adapter:"grok.official.responses",format:"openai/responses",base:"https://api.x.ai",path:"/v1/responses",fixed:true,native:false},
  {id:"grok-web",name:"Grok Web",kind:"grok-web-native",channel:"grok.web",adapter:"grok.web.responses",format:"openai/responses",base:"https://grok.com",path:"/rest/app-chat/conversations/new",fixed:true,native:true},
  {id:"grok-console",name:"Grok Console",kind:"grok-console-native",channel:"grok.console",adapter:"grok.console.responses",format:"openai/responses",base:"https://console.x.ai",path:"/v1/responses",fixed:true,native:true},
  {id:"grok-build",name:"Grok Build",kind:"grok-build-native",channel:"grok.build",adapter:"grok.build.responses",format:"openai/responses",base:"https://cli-chat-proxy.grok.com/v1",path:"/responses",fixed:true,native:true},
] as const;

export function providerAddress(base:string,path:string) {
  const url=new URL(base);
  if(url.protocol!=="https:"||url.username||url.password||url.search||url.hash)throw new Error("接口地址应为 HTTPS，且不包含账号、密码或查询参数。");
  if(!path.startsWith("/")||path.startsWith("//")||/[?#]/u.test(path))throw new Error("请求路径应以 / 开始，不包含查询参数。");
  return {base:base.replace(/\/+$/u,""),host:url.hostname,port:Number(url.port||443)};
}
