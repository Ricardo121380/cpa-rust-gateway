import { asAppError } from "../api/errors";
import { useMessages } from "../i18n/messages";

export function ReadStatus({
  pending,
  error,
  hasData,
  retry,
}: Readonly<{
  pending: boolean;
  error: unknown;
  hasData: boolean;
  retry: () => void;
}>) {
  const t = useMessages();
  if (error !== null && error !== undefined) {
    const detail = asAppError(error);
    return (
      <div className="card read-status" role="alert" data-gap="top">
        <strong>{t.state.readFailed}</strong>
        <p className="small">
          {detail.code} · {detail.message}
        </p>
        {hasData ? <p className="small muted">{t.state.previousData}</p> : null}
        <button className="secondary" onClick={retry}>
          {t.state.retry}
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
