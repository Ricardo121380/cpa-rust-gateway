/** Stable, non-secret identity for a captured ordinary/native account action. */
export type AccountActionTarget = Readonly<{ id: string; native: boolean }>;

export function accountActionTargetKey(target: AccountActionTarget): string {
  return `${target.native ? "native" : "ordinary"}:${target.id}`;
}

/**
 * Freezes the displayed selection for a later confirmation. Account identity
 * groups are presentation only: an ordinary/native namespace plus exact ID is
 * the only deduplication key, preserving order without pulling later pages in.
 */
export function freezeAccountActionTargets<T extends AccountActionTarget>(targets: readonly T[]): readonly T[] {
  const seen = new Set<string>();
  return targets.filter((target) => {
    const key = accountActionTargetKey(target);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export type AccountActionOutcomeKind =
  | "pending"
  | "skipped"
  | "unexecuted"
  | "rejected"
  | "saved_draft"
  | "saved_unapplied"
  | "applied"
  | "unconfirmed";

export type AccountActionOutcome = Readonly<{
  kind: AccountActionOutcomeKind;
  detail?: string;
}>;

export function accountActionOutcomeLabel(outcome: AccountActionOutcome): string {
  const label: Record<AccountActionOutcomeKind, string> = {
    pending: "待处理",
    skipped: "无需更改",
    unexecuted: "未执行",
    rejected: "未执行：被拒绝",
    saved_draft: "已保存到草稿",
    saved_unapplied: "已保存，待应用",
    applied: "已应用",
    unconfirmed: "结果未确认",
  };
  return outcome.detail === undefined ? label[outcome.kind] : `${label[outcome.kind]}：${outcome.detail}`;
}
