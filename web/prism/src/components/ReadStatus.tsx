import { asAppError } from "../api/errors";
import { useMessages } from "../i18n/messages";

export function ReadStatus({
  pending,
  fetching = false,
  error,
  hasData,
  dataUpdatedAt,
  retry,
}: Readonly<{
  pending: boolean;
  fetching?: boolean;
  error: unknown;
  hasData: boolean;
  dataUpdatedAt?: number;
  retry: () => void;
}>) {
  const t = useMessages();
  if (error !== null && error !== undefined) {
    const detail = asAppError(error);
    return (
      <div className="card read-status" role="alert" data-gap="top">
        <strong>{hasData ? t.state.refreshFailed : t.state.readFailed}</strong>
        <p className="small">
          {detail.code} · {detail.message}
        </p>
        {detail.kind === "unavailable" ? <p className="small">{t.state.temporarilyUnavailable}</p> : null}
        {hasData ? <p className="small muted">{t.state.previousData}</p> : null}
        {hasData && dataUpdatedAt ? <p className="small muted">{t.state.lastRead}：{new Date(dataUpdatedAt).toLocaleString()}</p> : null}
        <button type="button" className="secondary" disabled={fetching} onClick={retry}>
          {fetching ? t.state.loading : t.state.retry}
        </button>
      </div>
    );
  }
  return pending ? (
    <p className="scope-row" role="status">
      {t.state.loading}
    </p>
  ) : null;
}
