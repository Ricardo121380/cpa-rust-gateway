import { useMutation, useQuery } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";

/** Creates a managed bearer identity, including one with no endpoint binding yet. */
export function AddApiKeyAccount({onClose, onCreated}: Readonly<{onClose: () => void; onCreated: () => void}>) {
  const [error, setError] = useState<string>();
  const providers = useQuery({queryKey: ["account-providers"], queryFn: () =>
    call<readonly {id: string; name: string}[]>("listUpstreams", {}, {versionScoped: true})});
  const create = useMutation({
    gcTime: 0,
    mutationFn: ({provider, name, secret}: {provider: string; name: string; secret: string}) =>
      call("createCredential", {path: {upstream_id: provider}, body: {id: name, kind: "bearer", status: "active", secret}}, {versionScoped: true, mutating: true}),
    onSuccess: onCreated,
    onSettled: (): void => { create.reset(); },
    onError: (cause) => setError(asAppError(cause).message),
  });
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const values = new FormData(event.currentTarget);
    create.mutate({provider: String(values.get("provider")), name: String(values.get("name")).trim(), secret: String(values.get("secret"))});
    // Do not retain submitted secret in the form or mutation cache after settlement.
    event.currentTarget.reset();
  };
  return <Sheet title="添加 API Key 账号" onEscape={() => !create.isPending && onClose()}>
    {providers.isError ? <p role="alert">{asAppError(providers.error).message}</p> : providers.isPending ? <p>读取提供商…</p> : providers.data.length === 0 ?
      <p>先添加此账号所属的 <Link to="/upstreams" onClick={onClose}>AI 提供商</Link>。</p> :
      <form className="sheet-form" onSubmit={submit} autoComplete="off">
        <label>提供商<select name="provider" required>{providers.data.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
        <label>账号名称<input name="name" required maxLength={128} placeholder="例如：团队 API 账号" /></label>
        <label>API Key / Token<input name="secret" type="password" required autoComplete="new-password" /></label>
        <p className="small muted">保存后可在账号列表关联端点，再应用配置。</p>
        {error ? <p role="alert">{error}</p> : null}
        <div className="sheet-actions"><button type="button" className="secondary" disabled={create.isPending} onClick={onClose}>取消</button><button disabled={create.isPending}>添加账号</button></div>
      </form>}
  </Sheet>;
}
