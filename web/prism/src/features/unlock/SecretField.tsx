// Machine-secret input. Deliberately NOT type="password".
//
// Two password-typed fields with no username is the canonical "change your
// password" signature: Safari's Automatic Strong Password popover and
// password-manager inline widgets then cover the field, swallowing the click
// and the ⌘V that follows — silently, with no error. These two values are a
// management key and a CSRF token copied out of a server log, not a website
// password, so they gain nothing from the password type.
//
// Masking is therefore visual only, via a CSS class (-webkit-text-security;
// Chrome/Safari all versions, Firefox 114+). Known cost: the value is exposed
// to the accessibility tree, so a screen reader will read it out character by
// character — accepted deliberately in exchange for paste actually working.
import { useId, useState } from "react";
import { useMessages } from "../../i18n/messages";

export function SecretField({
  label,
  hint,
  value,
  onChange,
  invalid,
  errorId,
  required = false,
}: Readonly<{
  label: string;
  hint: string;
  value: string;
  onChange: (next: string) => void;
  invalid: boolean;
  errorId: string;
  required?: boolean;
}>) {
  const id = useId();
  const hintId = `${id}-hint`;
  const t = useMessages();
  const [revealed, setRevealed] = useState(false);

  return (
    <div className="secret-field">
      <label htmlFor={id}>{label}</label>
      <div className="secret-row">
        <input
          id={id}
          name={`prism-${id}`}
          className={revealed ? "secret-input mono" : "secret-input mono is-masked"}
          type="text"
          value={value}
          onChange={(event) => onChange(normalizeSecret(event.target.value))}
          required={required}
          aria-describedby={invalid ? `${hintId} ${errorId}` : hintId}
          aria-invalid={invalid ? true : undefined}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          // Password-manager opt-outs (unknown attributes are ignored).
          data-1p-ignore
          data-op-ignore
          data-lpignore="true"
          data-bwignore="true"
          data-protonpass-ignore="true"
          data-form-type="other"
        />
        <button
          type="button"
          className="secret-toggle"
          aria-pressed={revealed}
          aria-controls={id}
          aria-label={t.unlock.revealToggle}
          onClick={() => setRevealed((current) => !current)}
        >
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
            <path
              d="M1 8s2.5-4.5 7-4.5S15 8 15 8s-2.5 4.5-7 4.5S1 8 1 8Z"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.3"
            />
            <circle cx="8" cy="8" r="2" fill="none" stroke="currentColor" strokeWidth="1.3" />
            {revealed ? <path d="M2.5 13.5 13.5 2.5" stroke="currentColor" strokeWidth="1.3" /> : null}
          </svg>
        </button>
      </div>
      <p id={hintId} className="unlock-hint">
        {hint}
      </p>
    </div>
  );
}

/**
 * Secrets pasted out of logs or config arrive with trailing newlines, soft
 * wraps, wrapping quotes or an assignment prefix. Strip all of that instead of
 * failing validation on something the user cannot see.
 */
export function normalizeSecret(raw: string): string {
  return raw
    .replace(/\s+/gu, "")
    .replace(/^["'`]+|["'`]+$/gu, "")
    .replace(/^(?:Bearer|[A-Za-z_]*(?:KEY|TOKEN|key|token))[:=]/u, "")
    .replace(/^["'`]+/u, "");
}
