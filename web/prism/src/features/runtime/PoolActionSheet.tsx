import type { FormEvent } from "react";
import { Sheet } from "../../components/Sheet";
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
  onSubmit,
}: Readonly<{
  account: PoolAccount;
  action: PoolAction;
  pending: boolean;
  onCancel: () => void;
  onInvalid: (message: string) => void;
  onSubmit: (body: Readonly<Record<string, unknown>>) => void;
}>) {
  const isCooldown = action === "cool_down";
  return (
    <Sheet
      title={isCooldown ? "冷却这个账号" : "为这个账号请求恢复"}
      onEscape={onCancel}
    >
      <p className="reveal-warning">
        作用对象是<strong>精确到账号</strong>的一条:
        <br />
        <span className="mono">
          {account.provider_id} / {account.channel_id} / {account.account_id}
        </span>
        <br />
        {isCooldown
          ? "冷却会把它移出调度直到到期,同 Provider 下的其他账号继续服务。"
          : "请求恢复只是登记意图 —— 是否放行仍由运行时与上游决定,不保证恢复。"}
      </p>
      <form
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
              onInvalid(
                `冷却时长越界:契约要求 ${COOLDOWN_MIN_MS}–${COOLDOWN_MAX_MS} 毫秒(1 秒–24 小时)。`,
              );
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
          upstream_model(可选)
          <input name="upstream_model" className="mono" maxLength={256} />
          <small>只想影响某一个上游模型时填写;留空表示整个账号。</small>
        </label>
        <div className="sheet-actions">
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button
            type="submit"
            className={isCooldown ? "danger" : undefined}
            disabled={pending}
          >
            {isCooldown ? "确认冷却" : "确认请求恢复"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}
