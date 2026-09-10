import { useId, useState } from "react";
import { useMessages } from "../../i18n/messages";

export function PasswordField({ label, value, onChange, autoComplete = "current-password", autoFocus = false, hint, errorId }: {
  label: string; value: string; onChange: (value: string) => void;
  autoComplete?: "current-password" | "new-password"; autoFocus?: boolean; hint?: string; errorId?: string | undefined;
}) {
  const id = useId();
  const [visible, setVisible] = useState(false);
  const t = useMessages();
  return <div className="login-field"><label htmlFor={id}>{label}</label><div className="login-password-row">
    <input id={id} name={autoComplete === "new-password" ? id : "password"} type={visible ? "text" : "password"}
      value={value} onChange={(event) => onChange(event.target.value)} autoComplete={autoComplete}
      autoFocus={autoFocus} required maxLength={512} placeholder={hint}
      aria-invalid={errorId !== undefined} aria-describedby={errorId} />
    <button type="button" className="login-eye" aria-label={visible ? t.unlock.hidePassword : t.unlock.showPassword}
      aria-pressed={visible} onClick={() => setVisible(!visible)}>
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
        <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7S2 12 2 12Z" /><circle cx="12" cy="12" r="3" />
        {visible ? <path d="m4 3 16 18" /> : null}
      </svg>
    </button>
  </div></div>;
}
