import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";

import { GrokDeviceWizard } from "./GrokDeviceWizard";
type Channel = Readonly<{id: string; name: string; credential_format: string; import_available: boolean;
  authorization_flow: string; authorization_available: boolean; upstream_kinds: readonly string[]}>;
const formats: Record<string, string> = {
  api_key: "API Key / Token", cpa_sub2api_json: "CPA / Sub2API / Codex 凭据 JSON",
  claude_json: "Claude 凭据 JSON 或 API Key", kiro_json_or_key: "Kiro 凭据 JSON 或 ksk_ Key",
  grok_build_json: "Grok Build 凭据 JSON", sso: "SSO 凭据 JSON",
};

export function AddAccountDialog({onClose, onCreated}: Readonly<{onClose: () => void; onCreated: (notice?:string) => void}>) {
  const [error, setError] = useState<string>();
  const [channelId, setChannelId] = useState("openai-compatible");
  const [oauthName, setOauthName] = useState<string>();
  const native = ["grok.build", "grok.console", "grok.web"].includes(channelId);
  const secret = useRef<HTMLTextAreaElement>(null);
  const channels = useQuery({queryKey: ["account-channels"], queryFn: () => call<readonly Channel[]>("listAccountChannels")});
  const providers = useQuery({queryKey: ["account-providers"], queryFn: () =>
    call<readonly {id: string; name: string; kind: string}[]>("listUpstreams", {}, {versionScoped: true})});
  const channel = channels.data?.find((entry) => entry.id === channelId);
  const matches = providers.data?.filter((p) => channel?.upstream_kinds.includes(p.kind)) ?? [];
  const create = useMutation({
    gcTime: 0,
    mutationFn: ({provider, name, material}: {provider: string; name: string; material: string}) =>
      native ? call("importNativeAccount", {body:{id:name,channel:channelId,secret:material}}) : call("importChannelAccount", {path: {upstream_id: provider}, body: {id: name, channel: channelId, secret: material}}, {versionScoped: true, mutating: true}),
    onSuccess: (result)=>onCreated(native&&typeof result==="object"&&result!==null&&"identity_state" in result&&result.identity_state==="unavailable"?"账号已保存，暂未读到渠道身份，可在账号列表重试。":undefined),
    onSettled: (): void => { create.reset(); },
    onError: (cause) => setError(asAppError(cause).message),
  });
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const values = new FormData(event.currentTarget);
    create.mutate({provider: String(values.get("provider")), name: String(values.get("name")).trim(), material: String(values.get("secret"))});
    if (secret.current) secret.current.value = "";
  };
  if (oauthName !== undefined) return <GrokDeviceWizard name={oauthName} onClose={() => setOauthName(undefined)} onComplete={onCreated} />;
  return <Sheet title="添加账号" onEscape={() => !create.isPending && onClose()}>
    {channels.isError || providers.isError ? <p role="alert">{asAppError(channels.error ?? providers.error).message}</p> : channels.isPending || providers.isPending ? <p>读取接入方式…</p> : <>
      <label>渠道<select aria-label="渠道" value={channelId} disabled={create.isPending} onChange={(e) => {setChannelId(e.target.value); setError(undefined); if (secret.current) secret.current.value = "";}}>
        {channels.data.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}
      </select></label>
      {channel?.authorization_available ? <button type="button" onClick={()=>{if(secret.current)secret.current.value="";setOauthName("");}}>授权登录</button> : null}
      {channel?.import_available ? !native && matches.length === 0 ? <p>先添加此渠道的 <Link to="/upstreams" onClick={onClose}>AI 提供商</Link>。</p> :
        <form className="sheet-form" onSubmit={submit} autoComplete="off" key={channelId}>
          {!native ? <label>提供商<select name="provider" required>{matches.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label> : null}
          <label>导入标记<input name="name" required maxLength={128} placeholder="用于追溯本次导入" /></label>
          <label>{formats[channel.credential_format] ?? "凭据"}<textarea ref={secret} name="secret" required maxLength={65536} autoComplete="off" spellCheck={false} className="credential-input" /></label>
          {channel.credential_format !== "api_key" ? <label>读取凭据文件<input type="file" accept=".json,application/json" onChange={async (event) => {
            const file = event.currentTarget.files?.[0]; event.currentTarget.value = "";
            if (!file) return;
            if (file.size > 65536) {setError("凭据文件不能超过 64 KiB"); return;}
            const target = secret.current;
            try { const contents = await file.text(); if (target && secret.current === target) target.value = contents; }
            catch { setError("无法读取文件，请重新选择"); }
          }} /></label> : null}
          {error ? <p role="alert">{error}</p> : null}
          <div className="sheet-actions"><button type="button" className="secondary" disabled={create.isPending} onClick={onClose}>取消</button><button disabled={create.isPending}>添加账号</button></div>
        </form> : <p role="status">此渠道暂不可从面板接入，已有账号可继续使用。</p>}
    </>}
  </Sheet>;
}
