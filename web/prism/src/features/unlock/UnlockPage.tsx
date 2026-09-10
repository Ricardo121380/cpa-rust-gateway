import { useEffect, useRef, useState, type FormEvent } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { isCancelledError } from "@tanstack/react-query";
import { call, loginAdministrator, logoutAdministrator } from "../../api/client";
import { asAppError } from "../../api/errors";
import { GlassSurface } from "../../components/glass/GlassSurface";
import { useMessages } from "../../i18n/messages";
import { useSessionStore } from "../../session/sessionStore";
import { PasswordField } from "./PasswordField";

export function UnlockPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const t = useMessages();
  const unlocked = useSessionStore((s) => s.unlocked);
  const requiredChange = useSessionStore((s) => s.passwordChangeRequired);
  const changing = unlocked && (requiredChange || params.get("change-password") === "1");
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [busy, setBusy] = useState(false);
  const attempt = useRef<AbortController | undefined>(undefined);
  useEffect(() => () => attempt.current?.abort(), []);
  useEffect(() => { setPassword(""); setNewPassword(""); setConfirm(""); setError(undefined); }, [changing]);
  useEffect(() => { if (unlocked && !changing) navigate("/", { replace: true }); }, [unlocked, changing, navigate]);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    setError(undefined); setNotice(undefined);
    if (!password || (!changing && !username.trim())) { setError(t.unlock.required); return; }
    if (changing) {
      const length = [...newPassword].length;
      if (length < 12 || length > 128 || new TextEncoder().encode(newPassword).length > 512 || !newPassword.trim() || newPassword === password) {
        setError(t.unlock.passwordPolicy); return;
      }
      if (newPassword !== confirm) { setError(t.unlock.passwordMismatch); return; }
    }
    const controller = new AbortController(); attempt.current?.abort(); attempt.current = controller;
    setBusy(true);
    try {
      if (changing) {
        await call<void>("changeAdministratorPassword", { body: { current_password: password, new_password: newPassword }, signal: controller.signal });
        useSessionStore.getState().lock();
        navigate("/unlock", { replace: true });
        setNotice(t.unlock.passwordChanged);
      } else {
        await loginAdministrator(username.trim(), password, controller.signal);
      }
      setPassword(""); setNewPassword(""); setConfirm("");
    } catch (cause) {
      if (isCancelledError(cause) || controller.signal.aborted) return;
      const code = asAppError(cause).code;
      setError(code === "management_login_invalid_credentials" ? (changing ? t.unlock.currentPasswordWrong : t.unlock.failed)
        : code === "management_login_rate_limited" ? t.unlock.rateLimited
          : code === "management_login_invalid_password" ? t.unlock.passwordPolicy : t.unlock.unavailable);
      setPassword("");
    } finally {
      if (attempt.current === controller) { setBusy(false); attempt.current = undefined; }
    }
  }
  const errorId = error === undefined ? undefined : "login-error";
  return <div className="unlock-scene"><div className="ambient" aria-hidden="true" />
    <GlassSurface className="unlock-card" layer="modal">
      <div className="login-brand"><svg width="26" height="26" viewBox="0 0 28 28" fill="none" aria-hidden="true"><path d="M14 2 26 14 14 26 2 14Z" stroke="currentColor" strokeWidth="1.5" /><path d="M14 2v24M2 14h24" stroke="currentColor" strokeOpacity=".25" /></svg><span>Prism</span></div>
      <h1>{changing ? t.unlock.changeTitle : t.unlock.title}</h1>
      {requiredChange ? <p className="login-note">{t.unlock.firstChange}</p> : null}
      <form onSubmit={(event) => void onSubmit(event)} noValidate aria-busy={busy}>
        {changing ? null : <div className="login-field"><label htmlFor="login-username">{t.unlock.username}</label><input id="login-username" name="username" autoComplete="username" autoCapitalize="none" spellCheck={false} value={username} onChange={(e) => setUsername(e.target.value)} autoFocus maxLength={64} required aria-invalid={error !== undefined} aria-describedby={errorId} /></div>}
        <PasswordField key={changing ? "current" : "login"} label={changing ? (requiredChange ? t.unlock.initialPassword : t.unlock.currentPassword) : t.unlock.password} value={password} onChange={setPassword} autoFocus={changing} errorId={errorId} />
        {changing ? <><PasswordField label={t.unlock.newPassword} value={newPassword} onChange={setNewPassword} autoComplete="new-password" hint={t.unlock.passwordPlaceholder} errorId={errorId} /><PasswordField label={t.unlock.confirmPassword} value={confirm} onChange={setConfirm} autoComplete="new-password" errorId={errorId} /></> : null}
        {error === undefined ? null : <p id="login-error" role="alert" className="unlock-error">{error}</p>}
        {notice === undefined ? null : <p role="status" className="login-note">{notice}</p>}
        <button type="submit" className="unlock-submit" disabled={busy}>{busy ? t.unlock.busy : changing ? t.unlock.savePassword : t.unlock.submit}</button>
        {changing ? <button type="button" className="login-back" disabled={busy} onClick={() => { void logoutAdministrator(); navigate("/unlock", { replace: true }); }}>{t.unlock.back}</button> : null}
      </form>
    </GlassSurface>
  </div>;
}
