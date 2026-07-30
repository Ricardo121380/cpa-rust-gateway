// Unlock = the only scene where glass carries content (over ambient gradient,
// not data). "Login" is a probe: listConfigVersions succeeds or we show the
// single non-probeable failure message (backend returns uniform 404).
//
// Secrets go through SecretField, which never uses type="password" — see the
// note there: password-typed fields summon Safari's strong-password popover
// and password-manager widgets, which cover the input and swallow paste.
import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { call } from "../../api/client";
import type { AppError } from "../../api/errors";
import { GlassSurface } from "../../components/glass/GlassSurface";
import { useMessages } from "../../i18n/messages";
import {
  isValidCsrfTokenShape,
  isValidManagementKeyShape,
  useSessionStore,
} from "../../session/sessionStore";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import { SecretField } from "./SecretField";

const ERROR_ID = "unlock-error";

function fixturesEnabled(): boolean {
  return import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1";
}

export function UnlockPage() {
  const navigate = useNavigate();
  const unlock = useSessionStore((s) => s.unlock);
  const lock = useSessionStore((s) => s.lock);
  const t = useMessages();
  const [key, setKey] = useState("");
  const [csrf, setCsrf] = useState("");
  const [error, setError] = useState<string | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError(undefined);
    if (!isValidManagementKeyShape(key)) {
      setError(t.unlock.invalidShape);
      return;
    }
    if (csrf.length > 0 && !isValidCsrfTokenShape(csrf)) {
      setError(t.unlock.invalidShape);
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
          ? `${t.unlock.failed}(${appError.message})`
          : t.unlock.failed,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="unlock-scene">
      <GlassSurface className="unlock-card" layer="modal">
        <h1>{t.unlock.title}</h1>
        <form onSubmit={(event) => void onSubmit(event)} noValidate>
          <SecretField
            label={t.unlock.managementKey}
            hint={t.unlock.managementKeyHint}
            value={key}
            onChange={setKey}
            invalid={error !== undefined}
            errorId={ERROR_ID}
            required
          />
          <SecretField
            label={t.unlock.csrfToken}
            hint={t.unlock.csrfTokenHint}
            value={csrf}
            onChange={setCsrf}
            invalid={error !== undefined}
            errorId={ERROR_ID}
          />
          <p id={ERROR_ID} role="alert" aria-live="assertive" className="unlock-error">
            {error ?? ""}
          </p>
          <button type="submit" className="unlock-submit" disabled={busy}>
            {t.unlock.submit}
          </button>
          {fixturesEnabled() ? (
            <button
              type="button"
              className="secondary"
              onClick={() => {
                setKey(`mgmt_${"a".repeat(40)}`);
                setCsrf(`csrf_${"b".repeat(40)}`);
              }}
            >
              {t.unlock.fillDemo}
            </button>
          ) : null}
          <p className="unlock-hint">{t.unlock.hint}</p>
        </form>
      </GlassSurface>
    </div>
  );
}
