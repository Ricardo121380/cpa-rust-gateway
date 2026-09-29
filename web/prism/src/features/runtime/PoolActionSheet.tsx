import { useId, useState, type FormEvent } from "react";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { IdentityDetails } from "../../components/ResourceIdentity";
import {accountName,protocolName} from "../accounts/presentation";
import {
  validCooldown,
  COOLDOWN_MIN_MS,
  COOLDOWN_MAX_MS,
  type PoolAccount,
  type PoolAction,
} from "./model";

export function PoolActionSheet({
  account,
  action,
  pending,
  onCancel,
  onInvalid,
  error,
  onSubmit,
}: Readonly<{
  account: PoolAccount;
  action: PoolAction;
  pending: boolean;
  onCancel: () => void;
  onInvalid: (message: string) => void;
  error?: string;
  onSubmit: (body: Readonly<Record<string, unknown>>) => void;
}>) {
  const isCooldown = action === "cool_down";
  const formId = useId();
  const [validation, setValidation] = useState<string>();
  return (
    <Sheet
      title={isCooldown ? "冷却这个账号" : "高级：解除本地隔离"}
      description={isCooldown ? "让当前账号连接在指定时间内退出调度，其他同渠道账号不受影响。" : "只解除选中账号连接的本地冷却或隔离；不验证上游，也不解除停用、授权失效或未到重置时间的额度限制。"}
      layout="confirm"
      tone={isCooldown ? "danger" : "default"}
      onEscape={onCancel}
      busy={pending}
      footer={<><SheetDismissButton className="secondary" disabled={pending}>取消</SheetDismissButton><button type="submit" form={formId} className={isCooldown ? "danger" : "primary"} disabled={pending}>{isCooldown ? "确认冷却" : "确认解除本地隔离"}</button></>}
    >
      <p className="reveal-warning">
        <strong>{accountName(account.presentation?.identity)??"未提供账号身份"}</strong>
        <br />
        {account.presentation?`${account.presentation.provider} · ${protocolName(account.presentation.api_format)}${account.presentation.host?` · ${account.presentation.host}`:""}`:"当前选中的账号连接"}
        <br />
        {isCooldown
          ? "冷却会把它移出调度直到到期，同渠道的其他账号继续服务。"
          : `当前本地原因：${account.runtime_status}。完成后允许再次尝试，尚未验证。`}
      </p>
      <IdentityDetails entries={[["账号",account.account_id,accountName(account.presentation?.identity)??"未提供账号身份"],["提供商",account.provider_id,account.presentation?.provider],["接口",account.channel_id,account.presentation?protocolName(account.presentation.api_format):undefined]]} />
      <form
        id={formId}
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const body: Record<string, unknown> = {
            provider_id: account.provider_id,
            channel_id: account.channel_id,
            account_id: account.account_id,
            action,
          };
          const model = String(data.get("upstream_model") ?? "").trim();
          if (model !== "") {
            body["upstream_model"] = model;
          }
          if (isCooldown) {
            const ms = Number(data.get("cooldown_ms"));
            if (!validCooldown(ms)) {
              const message=`冷却时长越界:契约要求 ${COOLDOWN_MIN_MS}–${COOLDOWN_MAX_MS} 毫秒(1 秒–24 小时)。`;
              setValidation(message);
              onInvalid(message);
              return;
            }
            body["cooldown_ms"] = ms;
          }
          onSubmit(body);
        }}
      >
        {isCooldown ? (
          <label>
            冷却时长(毫秒,{COOLDOWN_MIN_MS}–{COOLDOWN_MAX_MS})
            <input
              name="cooldown_ms"
              type="number"
              min={COOLDOWN_MIN_MS}
              max={COOLDOWN_MAX_MS}
              defaultValue={60_000}
            />
            <small>
              留空不是"用默认值" —— 契约的字段可空,但这里必须给一个明确时长。
            </small>
          </label>
        ) : null}
        <label>
          上游模型（可选）
          <input name="upstream_model" className="mono" maxLength={256} />
          <small>只想影响某一个上游模型时填写;留空表示整个账号。</small>
        </label>
        {!isCooldown?<label className="check-row"><input type="checkbox" required disabled={pending}/>已核对目标、当前原因与解除范围；不会发送测试请求</label>:null}
        {validation?<p role="alert" className="action-error">{validation}</p>:null}
        {error===undefined?null:<p role="alert" className="action-error">{error}</p>}
      </form>
    </Sheet>
  );
}
