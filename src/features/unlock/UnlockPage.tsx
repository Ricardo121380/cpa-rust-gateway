// Unlock = the only scene where glass carries content (over ambient gradient,
// not data). "Login" is a probe: listConfigVersions succeeds or we show the
// single non-probeable failure message (backend returns uniform 404).
import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { call } from "../../api/client";
import type { AppError } from "../../api/errors";
import { GlassSurface } from "../../components/glass/GlassSurface";
import { messages } from "../../i18n/messages";
import {
  isValidCsrfTokenShape,
  isValidManagementKeyShape,
  useSessionStore,
} from "../../session/sessionStore";
import type { ConfigVersionSummary } from "../config-versions/versionStore";

export function UnlockPage() {
  const navigate = useNavigate();
  const unlock = useSessionStore((s) => s.unlock);
  const lock = useSessionStore((s) => s.lock);
  const [key, setKey] = useState("");
  const [csrf, setCsrf] = useState("");
  const [error, setError] = useState<string | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError(undefined);
    if (!isValidManagementKeyShape(key)) {
      setError(messages.unlock.invalidShape);
      return;
    }
    if (csrf.length > 0 && !isValidCsrfTokenShape(csrf)) {
      setError(messages.unlock.invalidShape);
      return;
    }
    setBusy(true);
    unlock(key, csrf.length > 0 ? csrf : undefined);
    try {
      await call<ConfigVersionSummary[]>("listConfigVersions");
      setKey("");
      setCsrf("");
      navigate("/", { replace: true });
    } catch (cause) {
      lock();
      const appError = cause as AppError;
      setError(
        appError.kind === "network"
          ? `${messages.unlock.failed}(${appError.message})`
          : messages.unlock.failed,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="unlock-scene">
      <GlassSurface className="unlock-card" layer="modal">
        <h1>{messages.unlock.title}</h1>
        <form onSubmit={(event) => void onSubmit(event)}>
          <label>
            {messages.unlock.managementKey}
            <input
              className="mono"
              type="password"
              autoComplete="off"
              value={key}
              onChange={(event) => setKey(event.target.value)}
              required
            />
          </label>
          <label>
            {messages.unlock.csrfToken}
            <input
              className="mono"
              type="password"
              autoComplete="off"
              value={csrf}
              onChange={(event) => setCsrf(event.target.value)}
            />
          </label>
          {error !== undefined ? (
            <p role="alert" className="unlock-error">
              {error}
            </p>
          ) : null}
          <button type="submit" disabled={busy}>
            {messages.unlock.submit}
          </button>
          <p className="unlock-hint">{messages.unlock.hint}</p>
        </form>
      </GlassSurface>
    </div>
  );
}
